use crate::decode::lzbuffer;
use crate::decode::rangecoder;
use crate::error;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io;

use crate::decompress::Options;
use crate::decompress::UnpackedSize;

pub struct LZMAParams {
    // most lc significant bits of previous byte are part of the literal context
    lc: u32, // 0..8
    lp: u32, // 0..4
    // context for literal/match is plaintext offset modulo 2^pb
    pb: u32, // 0..4
    dict_size: u32,
    unpacked_size: Option<u64>,
}

impl LZMAParams {
    pub fn read_header<R>(input: &mut R, options: &Options) -> error::Result<LZMAParams>
    where
        R: io::BufRead,
    {
        // Properties
        let props = input.read_u8().or_else(|e| {
            Err(error::Error::LZMAError(format!(
                "LZMA header too short: {}",
                e
            )))
        })?;

        let mut pb = props as u32;
        if pb >= 225 {
            return Err(error::Error::LZMAError(format!(
                "LZMA header invalid properties: {} must be < 225",
                pb
            )));
        }

        let lc: u32 = pb % 9;
        pb /= 9;
        let lp: u32 = pb % 5;
        pb /= 5;

        lzma_info!("Properties {{ lc: {}, lp: {}, pb: {} }}", lc, lp, pb);

        // Dictionary
        let dict_size_provided = input.read_u32::<LittleEndian>().or_else(|e| {
            Err(error::Error::LZMAError(format!(
                "LZMA header too short: {}",
                e
            )))
        })?;
        let dict_size = if dict_size_provided < 0x1000 {
            0x1000
        } else {
            dict_size_provided
        };

        lzma_info!("Dict size: {}", dict_size);

        // Unpacked size
        let unpacked_size: Option<u64> = match options.unpacked_size {
            UnpackedSize::ReadFromHeader => {
                let unpacked_size_provided = input.read_u64::<LittleEndian>().or_else(|e| {
                    Err(error::Error::LZMAError(format!(
                        "LZMA header too short: {}",
                        e
                    )))
                })?;
                let marker_mandatory: bool = unpacked_size_provided == 0xFFFF_FFFF_FFFF_FFFF;
                if marker_mandatory {
                    None
                } else {
                    Some(unpacked_size_provided)
                }
            }
            UnpackedSize::ReadHeaderButUseProvided(x) => {
                input.read_u64::<LittleEndian>()?;
                x
            }
            UnpackedSize::UseProvided(x) => x,
        };

        lzma_info!("Unpacked size: {:?}", unpacked_size);

        let params = LZMAParams {
            lc,
            lp,
            pb,
            dict_size,
            unpacked_size,
        };

        Ok(params)
    }
}

pub struct DecoderState<LZB>
where
    LZB: lzbuffer::LZBuffer,
{
    pub output: LZB,
    // most lc significant bits of previous byte are part of the literal context
    pub lc: u32, // 0..8
    pub lp: u32, // 0..4
    // context for literal/match is plaintext offset modulo 2^pb
    pub pb: u32, // 0..4
    unpacked_size: Option<u64>,
    literal_probs: Vec<Vec<u16>>,
    pos_slot_decoder: Vec<rangecoder::BitTree>,
    align_decoder: rangecoder::BitTree,
    pos_decoders: [u16; 115],
    is_match: [u16; 192], // true = LZ, false = literal
    is_rep: [u16; 12],
    is_rep_g0: [u16; 12],
    is_rep_g1: [u16; 12],
    is_rep_g2: [u16; 12],
    is_rep_0long: [u16; 192],
    state: usize,
    rep: [usize; 4],
    len_decoder: rangecoder::LenDecoder,
    rep_len_decoder: rangecoder::LenDecoder,
}

// Initialize decoder with accumulating buffer
pub fn new_accum<'a, W>(
    output: lzbuffer::LZAccumBuffer<'a, W>,
    lc: u32,
    lp: u32,
    pb: u32,
    unpacked_size: Option<u64>,
) -> DecoderState<lzbuffer::LZAccumBuffer<'a, W>>
where
    W: io::Write,
{
    DecoderState {
        output,
        lc,
        lp,
        pb,
        unpacked_size,
        literal_probs: vec![vec![0x400; 0x300]; 1 << (lc + lp)],
        pos_slot_decoder: vec![rangecoder::BitTree::new(6); 4],
        align_decoder: rangecoder::BitTree::new(4),
        pos_decoders: [0x400; 115],
        is_match: [0x400; 192],
        is_rep: [0x400; 12],
        is_rep_g0: [0x400; 12],
        is_rep_g1: [0x400; 12],
        is_rep_g2: [0x400; 12],
        is_rep_0long: [0x400; 192],
        state: 0,
        rep: [0; 4],
        len_decoder: rangecoder::LenDecoder::new(),
        rep_len_decoder: rangecoder::LenDecoder::new(),
    }
}

// Initialize decoder with circular buffer
pub fn new_circular<'a, W>(
    output: &'a mut W,
    params: LZMAParams,
) -> error::Result<DecoderState<lzbuffer::LZCircularBuffer<'a, W>>>
where
    W: io::Write,
{
    // Decoder
    let decoder = DecoderState {
        output: lzbuffer::LZCircularBuffer::from_stream(output, params.dict_size as usize),
        lc: params.lc,
        lp: params.lp,
        pb: params.pb,
        unpacked_size: params.unpacked_size,
        literal_probs: vec![vec![0x400; 0x300]; 1 << (params.lc + params.lp)],
        pos_slot_decoder: vec![rangecoder::BitTree::new(6); 4],
        align_decoder: rangecoder::BitTree::new(4),
        pos_decoders: [0x400; 115],
        is_match: [0x400; 192],
        is_rep: [0x400; 12],
        is_rep_g0: [0x400; 12],
        is_rep_g1: [0x400; 12],
        is_rep_g2: [0x400; 12],
        is_rep_0long: [0x400; 192],
        state: 0,
        rep: [0; 4],
        len_decoder: rangecoder::LenDecoder::new(),
        rep_len_decoder: rangecoder::LenDecoder::new(),
    };

    Ok(decoder)
}

impl<LZB> DecoderState<LZB>
where
    LZB: lzbuffer::LZBuffer,
{
    pub fn reset_state(&mut self, lc: u32, lp: u32, pb: u32) {
        self.lc = lc;
        self.lp = lp;
        self.pb = pb;
        self.literal_probs = vec![vec![0x400; 0x300]; 1 << (lc + lp)];
        self.pos_slot_decoder = vec![rangecoder::BitTree::new(6); 4];
        self.align_decoder = rangecoder::BitTree::new(4);
        self.pos_decoders = [0x400; 115];
        self.is_match = [0x400; 192];
        self.is_rep = [0x400; 12];
        self.is_rep_g0 = [0x400; 12];
        self.is_rep_g1 = [0x400; 12];
        self.is_rep_g2 = [0x400; 12];
        self.is_rep_0long = [0x400; 192];
        self.state = 0;
        self.rep = [0; 4];
        self.len_decoder = rangecoder::LenDecoder::new();
        self.rep_len_decoder = rangecoder::LenDecoder::new();
    }

    pub fn set_unpacked_size(&mut self, unpacked_size: Option<u64>) {
        self.unpacked_size = unpacked_size;
    }

    pub fn process<'a, R: io::BufRead>(
        &mut self,
        rangecoder: &mut rangecoder::RangeDecoder<'a, R>,
    ) -> error::Result<()> {
        loop {
            if let Some(unpacked_size) = self.unpacked_size {
                if self.output.len() as u64 >= unpacked_size {
                    break;
                }
            } else if rangecoder.is_finished_ok()? {
                break;
            }

            let pos_state = self.output.len() & ((1 << self.pb) - 1);

            // Literal
            if !rangecoder.decode_bit(
                // TODO: assumes pb = 2 ??
                &mut self.is_match[(self.state << 4) + pos_state],
            )? {
                let byte: u8 = self.decode_literal(rangecoder)?;
                lzma_debug!("Literal: {}", byte);
                self.output.append_literal(byte)?;

                self.state = if self.state < 4 {
                    0
                } else if self.state < 10 {
                    self.state - 3
                } else {
                    self.state - 6
                };
                continue;
            }

            // LZ
            let mut len: usize;
            // Distance is repeated from LRU
            if rangecoder.decode_bit(&mut self.is_rep[self.state])? {
                // dist = rep[0]
                if !rangecoder.decode_bit(&mut self.is_rep_g0[self.state])? {
                    // len = 1
                    if !rangecoder
                        .decode_bit(&mut self.is_rep_0long[(self.state << 4) + pos_state])?
                    {
                        // update state (short rep)
                        self.state = if self.state < 7 { 9 } else { 11 };
                        let dist = self.rep[0] + 1;
                        self.output.append_lz(1, dist)?;
                        continue;
                    }
                // dist = rep[i]
                } else {
                    let idx: usize;
                    if !rangecoder.decode_bit(&mut self.is_rep_g1[self.state])? {
                        idx = 1;
                    } else if !rangecoder.decode_bit(&mut self.is_rep_g2[self.state])? {
                        idx = 2;
                    } else {
                        idx = 3;
                    }
                    // Update LRU
                    let dist = self.rep[idx];
                    for i in (0..idx).rev() {
                        self.rep[i + 1] = self.rep[i];
                    }
                    self.rep[0] = dist
                }

                len = self.rep_len_decoder.decode(rangecoder, pos_state)?;
                // update state (rep)
                self.state = if self.state < 7 { 8 } else { 11 };
            // New distance
            } else {
                // Update LRU
                self.rep[3] = self.rep[2];
                self.rep[2] = self.rep[1];
                self.rep[1] = self.rep[0];
                len = self.len_decoder.decode(rangecoder, pos_state)?;

                // update state (match)
                self.state = if self.state < 7 { 7 } else { 10 };
                self.rep[0] = self.decode_distance(rangecoder, len)?;

                if self.rep[0] == 0xFFFF_FFFF {
                    if rangecoder.is_finished_ok()? {
                        break;
                    }
                    return Err(error::Error::LZMAError(String::from(
                        "Found end-of-stream marker but more bytes are available",
                    )));
                }
            }

            len += 2;

            let dist = self.rep[0] + 1;
            self.output.append_lz(len, dist)?;
        }

        if let Some(len) = self.unpacked_size {
            if self.output.len() as u64 != len {
                return Err(error::Error::LZMAError(format!(
                    "Expected unpacked size of {} but decompressed to {}",
                    len,
                    self.output.len()
                )));
            }
        }

        Ok(())
    }

    fn decode_literal<'a, R: io::BufRead>(
        &mut self,
        rangecoder: &mut rangecoder::RangeDecoder<'a, R>,
    ) -> error::Result<u8> {
        let def_prev_byte = 0u8;
        let prev_byte = self.output.last_or(def_prev_byte) as usize;

        let mut result: usize = 1;
        let lit_state =
            ((self.output.len() & ((1 << self.lp) - 1)) << self.lc) + (prev_byte >> (8 - self.lc));
        let probs = &mut self.literal_probs[lit_state];

        if self.state >= 7 {
            let mut match_byte = self.output.last_n(self.rep[0] + 1)? as usize;

            while result < 0x100 {
                let match_bit = (match_byte >> 7) & 1;
                match_byte <<= 1;
                let bit =
                    rangecoder.decode_bit(&mut probs[((1 + match_bit) << 8) + result])? as usize;
                result = (result << 1) ^ bit;
                if match_bit != bit {
                    break;
                }
            }
        }

        while result < 0x100 {
            result = (result << 1) ^ (rangecoder.decode_bit(&mut probs[result])? as usize);
        }

        Ok((result - 0x100) as u8)
    }

    fn decode_distance<'a, R: io::BufRead>(
        &mut self,
        rangecoder: &mut rangecoder::RangeDecoder<'a, R>,
        length: usize,
    ) -> error::Result<usize> {
        let len_state = if length > 3 { 3 } else { length };

        let pos_slot = self.pos_slot_decoder[len_state].parse(rangecoder)? as usize;
        if pos_slot < 4 {
            return Ok(pos_slot);
        }

        let num_direct_bits = (pos_slot >> 1) - 1;
        let mut result = (2 ^ (pos_slot & 1)) << num_direct_bits;

        if pos_slot < 14 {
            result += rangecoder.parse_reverse_bit_tree(
                num_direct_bits,
                &mut self.pos_decoders,
                result - pos_slot,
            )? as usize;
        } else {
            result += (rangecoder.get(num_direct_bits - 4)? as usize) << 4;
            result += self.align_decoder.parse_reverse(rangecoder)? as usize;
        }

        Ok(result)
    }
}
