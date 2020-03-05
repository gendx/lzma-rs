use crate::decode::util;
use crate::error;
use byteorder::{BigEndian, ReadBytesExt};
use std::io;

pub struct RangeDecoder<'a, R>
where
    R: 'a + io::BufRead,
{
    stream: &'a mut R,
    range: u32,
    code: u32,
}

impl<'a, R> RangeDecoder<'a, R>
where
    R: io::BufRead,
{
    pub fn new(stream: &'a mut R) -> io::Result<Self> {
        let mut dec = Self {
            stream,
            range: 0xFFFF_FFFF,
            code: 0,
        };
        let _ = dec.stream.read_u8()?;
        dec.code = dec.stream.read_u32::<BigEndian>()?;
        lzma_debug!("0 {{ range: {:08x}, code: {:08x} }}", dec.range, dec.code);
        Ok(dec)
    }

    #[inline]
    pub fn is_finished_ok(&mut self) -> io::Result<bool> {
        Ok(self.code == 0 && util::is_eof(self.stream)?)
    }

    #[inline]
    fn normalize(&mut self) -> io::Result<()> {
        lzma_trace!("  {{ range: {:08x}, code: {:08x} }}", self.range, self.code);
        if self.range < 0x0100_0000 {
            self.range <<= 8;
            self.code = (self.code << 8) ^ (self.stream.read_u8()? as u32);

            lzma_debug!("+ {{ range: {:08x}, code: {:08x} }}", self.range, self.code);
        }
        Ok(())
    }

    #[inline]
    fn get_bit(&mut self) -> error::Result<bool> {
        self.range >>= 1;

        let bit = self.code >= self.range;
        if bit {
            self.code -= self.range
        }

        self.normalize()?;
        Ok(bit)
    }

    pub fn get(&mut self, count: usize) -> error::Result<u32> {
        let mut result = 0u32;
        for _ in 0..count {
            result = (result << 1) ^ (self.get_bit()? as u32)
        }
        Ok(result)
    }

    #[inline]
    pub fn decode_bit(&mut self, prob: &mut u16) -> io::Result<bool> {
        let bound: u32 = (self.range >> 11) * (*prob as u32);

        lzma_trace!(
            " bound: {:08x}, prob: {:04x}, bit: {}",
            bound,
            prob,
            (self.code > bound) as u8
        );
        if self.code < bound {
            *prob += (0x800_u16 - *prob) >> 5;
            self.range = bound;

            self.normalize()?;
            Ok(false)
        } else {
            *prob -= *prob >> 5;
            self.code -= bound;
            self.range -= bound;

            self.normalize()?;
            Ok(true)
        }
    }

    fn parse_bit_tree(&mut self, num_bits: usize, probs: &mut [u16]) -> io::Result<u32> {
        let mut tmp: u32 = 1;
        for _ in 0..num_bits {
            let bit = self.decode_bit(&mut probs[tmp as usize])?;
            tmp = (tmp << 1) ^ (bit as u32);
        }
        Ok(tmp - (1 << num_bits))
    }

    pub fn parse_reverse_bit_tree(
        &mut self,
        num_bits: usize,
        probs: &mut [u16],
        offset: usize,
    ) -> io::Result<u32> {
        let mut result = 0u32;
        let mut tmp: usize = 1;
        for i in 0..num_bits {
            let bit = self.decode_bit(&mut probs[offset + tmp])?;
            tmp = (tmp << 1) ^ (bit as usize);
            result ^= (bit as u32) << i;
        }
        Ok(result)
    }
}

// TODO: parametrize by constant and use [u16; 1 << num_bits] as soon as Rust supports this
#[derive(Clone)]
pub struct BitTree {
    num_bits: usize,
    probs: Vec<u16>,
}

impl BitTree {
    pub fn new(num_bits: usize) -> Self {
        BitTree {
            num_bits,
            probs: vec![0x400; 1 << num_bits],
        }
    }

    pub fn parse<R: io::BufRead>(&mut self, rangecoder: &mut RangeDecoder<R>) -> io::Result<u32> {
        rangecoder.parse_bit_tree(self.num_bits, self.probs.as_mut_slice())
    }

    pub fn parse_reverse<R: io::BufRead>(
        &mut self,
        rangecoder: &mut RangeDecoder<R>,
    ) -> io::Result<u32> {
        rangecoder.parse_reverse_bit_tree(self.num_bits, self.probs.as_mut_slice(), 0)
    }
}

pub struct LenDecoder {
    choice: u16,
    choice2: u16,
    low_coder: Vec<BitTree>,
    mid_coder: Vec<BitTree>,
    high_coder: BitTree,
}

impl LenDecoder {
    pub fn new() -> Self {
        LenDecoder {
            choice: 0x400,
            choice2: 0x400,
            low_coder: vec![BitTree::new(3); 16],
            mid_coder: vec![BitTree::new(3); 16],
            high_coder: BitTree::new(8),
        }
    }

    pub fn decode<R: io::BufRead>(
        &mut self,
        rangecoder: &mut RangeDecoder<R>,
        pos_state: usize,
    ) -> io::Result<usize> {
        if !rangecoder.decode_bit(&mut self.choice)? {
            Ok(self.low_coder[pos_state].parse(rangecoder)? as usize)
        } else if !rangecoder.decode_bit(&mut self.choice2)? {
            Ok(self.mid_coder[pos_state].parse(rangecoder)? as usize + 8)
        } else {
            Ok(self.high_coder.parse(rangecoder)? as usize + 16)
        }
    }
}
