use std::io;
use error;
use decode::rangecoder;
use util;

pub struct Decoder<'a, R>
where
    R: 'a + io::BufRead,
{
    // most lc significant bits of previous byte are part of the literal context
    lc: u32, // 0..8
    lp: u32, // 0..4
    // context for literal/match is plaintext offset modulo 2^pb
    pb: u32, // 0..4
    dict_size: u32,
    unpacked_size: Option<u64>,
    rangecoder: rangecoder::RangeDecoder<'a, R>,
    literal_probs: Vec<Vec<u16>>,
    pos_slot_decoder: Vec<rangecoder::BitTree>,
    align_decoder: rangecoder::BitTree,
    pos_decoders: [u16; 115],
    output: Vec<u8>, // TODO: buffer this
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

impl<'a, R> Decoder<'a, R>
where
    R: io::BufRead,
{
    // Read LZMA header and initialize decoder
    pub fn from_stream(stream: &'a mut R) -> error::Result<Self> {
        // Properties
        let props = try!(util::read_u8(stream).or_else(|e| {
            Err(error::Error::LZMAError(
                format!("LZMA header too short: {}", e),
            ))
        }));

        let mut pb = props as u32;
        if pb >= 225 {
            return Err(error::Error::LZMAError(format!(
                "LZMA header invalid properties: {} should be < 225",
                pb
            )));
        }

        let lc: u32 = pb % 9;
        pb /= 9;
        let lp: u32 = pb % 5;
        pb /= 5;

        info!("Properties {{ lc: {}, lp: {}, pb: {} }}", lc, lp, pb);

        // Dictionary
        let dict_size_provided = try!(util::read_u32_le(stream).or_else(|e| {
            Err(error::Error::LZMAError(
                format!("LZMA header too short: {}", e),
            ))
        }));
        let dict_size = if dict_size_provided < 0x1000 {
            0x1000
        } else {
            dict_size_provided
        };

        info!("Dict size: {}", dict_size);

        // Unpacked size
        let unpacked_size_provided = try!(util::read_u64_le(stream).or_else(|e| {
            Err(error::Error::LZMAError(
                format!("LZMA header too short: {}", e),
            ))
        }));
        let marker_mandatory: bool = unpacked_size_provided == 0xFFFF_FFFF_FFFF_FFFF;
        let unpacked_size = if marker_mandatory {
            None
        } else {
            Some(unpacked_size_provided)
        };

        info!("Unpacked size: {:?}", unpacked_size);

        // Decoder
        let decoder = Decoder {
            lc: lc,
            lp: lp,
            pb: pb,
            dict_size: dict_size,
            unpacked_size: unpacked_size,
            rangecoder: try!(rangecoder::RangeDecoder::new(stream).or_else(|e| {
                Err(error::Error::LZMAError(
                    format!("LZMA stream too short: {}", e),
                ))
            })),
            literal_probs: vec![vec![0x400; 0x300]; 1 << (lc + lp)],
            pos_slot_decoder: vec![rangecoder::BitTree::new(6); 4],
            align_decoder: rangecoder::BitTree::new(4),
            pos_decoders: [0x400; 115],
            output: Vec::new(),
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

    fn append_lz(&mut self, len: usize, dist: usize) -> error::Result<()> {
        debug!("LZ {{ len: {}, dist: {} }}", len, dist);
        if dist > self.output.len() {
            return Err(error::Error::LZMAError(
                String::from("LZ distance before input"),
            ));
        }
        if dist > (self.dict_size as usize) {
            return Err(error::Error::LZMAError(format!(
                "LZ distance {} is beyond dictionary size {}",
                dist,
                self.dict_size
            )));
        }
        let offset = self.output.len() - dist;
        for i in 0..len {
            let x = self.output[offset + i];
            self.output.push(x)
        }
        Ok(())
    }

    pub fn process(mut self) -> error::Result<Vec<u8>> {
        loop {
            if let Some(_) = self.unpacked_size {
                if self.rangecoder.is_finished_ok()? {
                    break;
                }
            }

            let pos_state = self.output.len() & ((1 << self.pb) - 1);

            // Literal
            if !self.rangecoder.decode_bit(
                // TODO: assumes pb = 2 ??
                &mut self.is_match[(self.state << 4) +
                                       pos_state],
            )?
            {
                let byte: u8 = self.decode_literal()?;
                debug!("Literal: {}", byte);
                self.output.push(byte);

                self.state = if self.state < 4 {
                    0
                } else {
                    if self.state < 10 {
                        self.state - 3
                    } else {
                        self.state - 6
                    }
                };
                continue;
            }

            // LZ
            let mut len: usize;
            // Distance is repeated from LRU
            if self.rangecoder.decode_bit(&mut self.is_rep[self.state])? {
                // dist = rep[0]
                if !self.rangecoder.decode_bit(&mut self.is_rep_g0[self.state])? {
                    // len = 1
                    if !self.rangecoder.decode_bit(
                        &mut self.is_rep_0long[(self.state << 4) +
                                                   pos_state],
                    )?
                    {
                        // update state (short rep)
                        self.state = if self.state < 7 { 9 } else { 11 };
                        let dist = self.rep[0] + 1;
                        self.append_lz(1, dist)?;
                        continue;
                    }
                // dist = rep[i]
                } else {
                    let idx: usize;
                    if !self.rangecoder.decode_bit(&mut self.is_rep_g1[self.state])? {
                        idx = 1;
                    } else {
                        if !self.rangecoder.decode_bit(&mut self.is_rep_g2[self.state])? {
                            idx = 2;
                        } else {
                            idx = 3;
                        }
                    }
                    // Update LRU
                    let dist = self.rep[idx];
                    for i in (0..idx).rev() {
                        self.rep[i + 1] = self.rep[i];
                    }
                    self.rep[0] = dist
                }

                len = self.rep_len_decoder.decode(&mut self.rangecoder, pos_state)?;
                // update state (rep)
                self.state = if self.state < 7 { 8 } else { 11 };
            // New distance
            } else {
                // Update LRU
                self.rep[3] = self.rep[2];
                self.rep[2] = self.rep[1];
                self.rep[1] = self.rep[0];
                len = self.len_decoder.decode(&mut self.rangecoder, pos_state)?;

                // update state (match)
                self.state = if self.state < 7 { 7 } else { 10 };
                self.rep[0] = self.decode_distance(len)?;

                if self.rep[0] == 0xFFFF_FFFF {
                    if self.rangecoder.is_finished_ok()? {
                        break;
                    }
                    return Err(error::Error::LZMAError(String::from(
                        "Found end-of-stream marker but more bytes are available",
                    )));
                }
            }

            len += 2;

            let dist = self.rep[0] + 1;
            self.append_lz(len, dist)?;
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
        Ok(self.output)
    }

    fn decode_literal(&mut self) -> io::Result<u8> {
        let def_prev_byte = 0u8;
        let prev_byte = *self.output.last().unwrap_or(&def_prev_byte) as usize;

        let mut result: usize = 1;
        let lit_state = ((self.output.len() & ((1 << self.lp) - 1)) << self.lc) +
            (prev_byte >> (8 - self.lc));
        let probs = &mut self.literal_probs[lit_state];

        if self.state >= 7 {
            let mut match_byte = self.output[self.output.len() - self.rep[0] - 1] as usize;

            while result < 0x100 {
                let match_bit = (match_byte >> 7) & 1;
                match_byte <<= 1;
                let bit = self.rangecoder.decode_bit(
                    &mut probs[((1 + match_bit) << 8) + result],
                )? as usize;
                result = (result << 1) ^ bit;
                if match_bit != bit {
                    break;
                }
            }
        }

        while result < 0x100 {
            result = (result << 1) ^ (self.rangecoder.decode_bit(&mut probs[result])? as usize);
        }

        Ok((result - 0x100) as u8)
    }

    fn decode_distance(&mut self, length: usize) -> error::Result<usize> {
        let len_state = if length > 3 { 3 } else { length };

        let pos_slot = self.pos_slot_decoder[len_state].parse(&mut self.rangecoder)? as usize;
        if pos_slot < 4 {
            return Ok(pos_slot);
        }

        let num_direct_bits = (pos_slot >> 1) - 1;
        let mut result = (2 ^ (pos_slot & 1)) << num_direct_bits;

        if pos_slot < 14 {
            result += self.rangecoder.parse_reverse_bit_tree(
                num_direct_bits,
                &mut self.pos_decoders,
                result - pos_slot,
            )? as usize;
        } else {
            result += (self.rangecoder.get(num_direct_bits - 4)? as usize) << 4;
            result += self.align_decoder.parse_reverse(&mut self.rangecoder)? as usize;
        }

        Ok(result)
    }
}
