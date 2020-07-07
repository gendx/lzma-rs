use crate::decode::lzbuffer;
use crate::decode::rangecoder;
use crate::error;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io;
use std::marker::PhantomData;

use crate::decompress::Options;
use crate::decompress::UnpackedSize;

/// Minimum header length to be read.
/// - props: u8 (1 byte)
/// - dict_size: u32 (4 bytes)
pub const MIN_HEADER_LEN: usize = 5;

/// Max header length to be read.
/// - unpacked_size: u64 (8 bytes)
pub const MAX_HEADER_LEN: usize = MIN_HEADER_LEN + 8;

/// Required bytes after the header.
/// - ignore: u8 (1 byte)
/// - code: u32 (4 bytes)
pub const START_BYTES: usize = 5;

/// Maximum input data that can be processed in one iteration
const MAX_REQUIRED_INPUT: usize = 20;

/// Processing mode for decompression.
///
/// Tells the decompressor if we should expect more data after parsing the
/// current input.
#[derive(Debug, PartialEq)]
pub enum Mode {
    /// Streaming mode. Process the input bytes but assume there will be more
    /// chunks of input data to receive in future calls to `process_mode()`.
    Run,
    /// Sync mode. Process the input bytes and confirm end of stream has been reached.
    /// Use this mode if you are processing a fixed buffer of compressed data, or after
    /// using `Mode::Run` to check for the end of stream.
    Finish,
}

/// Used during stream processing to identify the next state.
pub enum CheckState {
    Lit,
    Rep,
    Match,
}

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

pub struct DecoderState<W, LZB>
where
    W: io::Write,
    LZB: lzbuffer::LZBuffer<W>,
{
    _phantom: PhantomData<W>,
    // buffer input data here if we need more for decompression, 20 is the max
    // number of bytes that can be consumed during one iteration
    tmp: [u8; MAX_REQUIRED_INPUT],
    tmp_len: usize,
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
pub fn new_accum<W>(
    output: lzbuffer::LZAccumBuffer<W>,
    lc: u32,
    lp: u32,
    pb: u32,
    unpacked_size: Option<u64>,
) -> DecoderState<W, lzbuffer::LZAccumBuffer<W>>
where
    W: io::Write,
{
    DecoderState {
        _phantom: PhantomData,
        tmp: [0; MAX_REQUIRED_INPUT],
        tmp_len: 0,
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
pub fn new_circular<W>(
    output: W,
    params: LZMAParams,
) -> error::Result<DecoderState<W, lzbuffer::LZCircularBuffer<W>>>
where
    W: io::Write,
{
    new_circular_with_memlimit(output, params, std::usize::MAX)
}

// Initialize decoder with circular buffer
pub fn new_circular_with_memlimit<W>(
    output: W,
    params: LZMAParams,
    memlimit: usize,
) -> error::Result<DecoderState<W, lzbuffer::LZCircularBuffer<W>>>
where
    W: io::Write,
{
    // Decoder
    let decoder = DecoderState {
        _phantom: PhantomData,
        output: lzbuffer::LZCircularBuffer::from_stream_with_memlimit(
            output,
            params.dict_size as usize,
            memlimit,
        ),
        tmp: [0; MAX_REQUIRED_INPUT],
        tmp_len: 0,
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

impl<W, LZB> DecoderState<W, LZB>
where
    W: io::Write,
    LZB: lzbuffer::LZBuffer<W>,
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
        self.process_mode(rangecoder, Mode::Finish)
    }

    pub fn process_stream<'a, R: io::BufRead>(
        &mut self,
        rangecoder: &mut rangecoder::RangeDecoder<'a, R>,
    ) -> error::Result<()> {
        self.process_mode(rangecoder, Mode::Run)
    }

    /// Process the next iteration of the loop.
    ///
    /// Returns true if we should continue processing the loop, false otherwise.
    fn process_next<'a, R: io::BufRead>(
        &mut self,
        rangecoder: &mut rangecoder::RangeDecoder<'a, R>,
    ) -> error::Result<bool> {
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
            return Ok(true);
        }

        // LZ
        let mut len: usize;
        // Distance is repeated from LRU
        if rangecoder.decode_bit(&mut self.is_rep[self.state])? {
            // dist = rep[0]
            if !rangecoder.decode_bit(&mut self.is_rep_g0[self.state])? {
                // len = 1
                if !rangecoder.decode_bit(&mut self.is_rep_0long[(self.state << 4) + pos_state])? {
                    // update state (short rep)
                    self.state = if self.state < 7 { 9 } else { 11 };
                    let dist = self.rep[0] + 1;
                    self.output.append_lz(1, dist)?;
                    return Ok(true);
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
                    return Ok(false);
                }
                return Err(error::Error::LZMAError(String::from(
                    "Found end-of-stream marker but more bytes are available",
                )));
            }
        }

        len += 2;

        let dist = self.rep[0] + 1;
        self.output.append_lz(len, dist)?;

        Ok(true)
    }

    /// Try to process the next iteration of the loop.
    ///
    /// This will check to see if there is enough data to consume and advance the
    /// decompressor. Needed in streaming mode to avoid corrupting the state while
    /// processing incomplete chunks of data.
    #[allow(clippy::if_same_then_else)]
    fn try_process_next<'a>(
        &self,
        buf: &'a [u8],
        range: u32,
        code: u32,
    ) -> error::Result<CheckState> {
        let mut temp = std::io::Cursor::new(buf);
        let mut rangecoder = rangecoder::RangeDecoder::from_parts(&mut temp, range, code);
        let pos_state = self.output.len() & ((1 << self.pb) - 1);

        // Literal
        if !rangecoder.decode_bit_check(self.is_match[(self.state << 4) + pos_state])? {
            self.decode_literal_check(&mut rangecoder)?;
            return Ok(CheckState::Lit);
        }

        // LZ
        if rangecoder.decode_bit_check(self.is_rep[self.state])? {
            if !rangecoder.decode_bit_check(self.is_rep_g0[self.state])? {
                if !rangecoder.decode_bit_check(self.is_rep_0long[(self.state << 4) + pos_state])? {
                    return Ok(CheckState::Rep);
                }
            } else if !rangecoder.decode_bit_check(self.is_rep_g1[self.state])? {
            } else if !rangecoder.decode_bit_check(self.is_rep_g2[self.state])? {
            }

            self.rep_len_decoder
                .decode_check(&mut rangecoder, pos_state)?;
            Ok(CheckState::Rep)
        // New distance
        } else {
            let len = self.len_decoder.decode_check(&mut rangecoder, pos_state)?;
            self.decode_distance_check(&mut rangecoder, len)?;
            Ok(CheckState::Match)
        }
    }

    pub fn process_mode<'a, R: io::BufRead>(
        &mut self,
        mut rangecoder: &mut rangecoder::RangeDecoder<'a, R>,
        mode: Mode,
    ) -> error::Result<()> {
        loop {
            if let Some(unpacked_size) = self.unpacked_size {
                if self.output.len() as u64 >= unpacked_size {
                    break;
                }
            } else if match mode {
                Mode::Run => rangecoder.is_eof()?,
                Mode::Finish => rangecoder.is_finished_ok()?,
            } {
                break;
            }

            if self.tmp_len > 0 {
                // Fill as much of the tmp buffer as possible
                self.tmp_len += rangecoder.read_into(&mut self.tmp[self.tmp_len..])?;

                // Check if we need more data to advance the decompressor
                if Mode::Run == mode
                    && self.tmp_len < MAX_REQUIRED_INPUT
                    && self
                        .try_process_next(
                            &self.tmp[0..self.tmp_len],
                            rangecoder.range(),
                            rangecoder.code(),
                        )
                        .is_err()
                {
                    return Ok(());
                }

                // Run the decompressor on the tmp buffer
                let tmp = self.tmp;
                let mut tmp_reader = io::Cursor::new(&tmp[0..self.tmp_len]);
                let mut tmp_rangecoder = rangecoder::RangeDecoder::from_parts(
                    &mut tmp_reader,
                    rangecoder.range(),
                    rangecoder.code(),
                );
                let res = self.process_next(&mut tmp_rangecoder)?;

                // Update the actual rangecoder
                let (range, code) = tmp_rangecoder.into_parts();
                rangecoder.set(range, code);

                // Update tmp buffer
                let new_len = self.tmp_len - tmp_reader.position() as usize;
                self.tmp[0..new_len]
                    .copy_from_slice(&tmp[tmp_reader.position() as usize..self.tmp_len]);
                self.tmp_len = new_len;

                if !res {
                    break;
                };
            } else {
                if (Mode::Run == mode) && (rangecoder.remaining()? < MAX_REQUIRED_INPUT) {
                    let range = rangecoder.range();
                    let code = rangecoder.code();
                    let buf = rangecoder.buf()?;

                    if self.try_process_next(buf, range, code).is_err() {
                        self.tmp_len = rangecoder.read_into(&mut self.tmp)?;
                        return Ok(());
                    }
                }

                if !self.process_next(&mut rangecoder)? {
                    break;
                };
            }
        }

        if let Some(len) = self.unpacked_size {
            if Mode::Finish == mode && self.output.len() as u64 != len {
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

    /// Attempts to decode a literal without mutating state
    fn decode_literal_check<'a, R: io::BufRead>(
        &self,
        rangecoder: &mut rangecoder::RangeDecoder<'a, R>,
    ) -> error::Result<u8> {
        let def_prev_byte = 0u8;
        let prev_byte = self.output.last_or(def_prev_byte) as usize;

        let mut result: usize = 1;
        let lit_state =
            ((self.output.len() & ((1 << self.lp) - 1)) << self.lc) + (prev_byte >> (8 - self.lc));
        let probs = &self.literal_probs[lit_state];

        if self.state >= 7 {
            let mut match_byte = self.output.last_n(self.rep[0] + 1)? as usize;

            while result < 0x100 {
                let match_bit = (match_byte >> 7) & 1;
                match_byte <<= 1;
                let bit =
                    rangecoder.decode_bit_check(probs[((1 + match_bit) << 8) + result])? as usize;
                result = (result << 1) ^ bit;
                if match_bit != bit {
                    break;
                }
            }
        }

        while result < 0x100 {
            result = (result << 1) ^ (rangecoder.decode_bit_check(probs[result])? as usize);
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

    /// Attempts to decode distance without mutating state
    fn decode_distance_check<'a, R: io::BufRead>(
        &self,
        rangecoder: &mut rangecoder::RangeDecoder<'a, R>,
        length: usize,
    ) -> error::Result<usize> {
        let len_state = if length > 3 { 3 } else { length };

        let pos_slot = self.pos_slot_decoder[len_state].parse_check(rangecoder)? as usize;
        if pos_slot < 4 {
            return Ok(pos_slot);
        }

        let num_direct_bits = (pos_slot >> 1) - 1;
        let mut result = (2 ^ (pos_slot & 1)) << num_direct_bits;

        if pos_slot < 14 {
            result += rangecoder.parse_reverse_bit_tree_check(
                num_direct_bits,
                &self.pos_decoders,
                result - pos_slot,
            )? as usize;
        } else {
            result += (rangecoder.get(num_direct_bits - 4)? as usize) << 4;
            result += self.align_decoder.parse_reverse_check(rangecoder)? as usize;
        }

        Ok(result)
    }
}
