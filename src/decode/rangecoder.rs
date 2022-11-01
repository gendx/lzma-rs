use crate::decode::util;
use crate::error;
use crate::util::const_assert;
use byteorder::{BigEndian, ReadBytesExt};
use std::io;

pub struct RangeDecoder<'a, R>
where
    R: 'a + io::BufRead,
{
    pub stream: &'a mut R,
    pub range: u32,
    pub code: u32,
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

    pub fn from_parts(stream: &'a mut R, range: u32, code: u32) -> Self {
        Self {
            stream,
            range,
            code,
        }
    }

    pub fn set(&mut self, range: u32, code: u32) {
        self.range = range;
        self.code = code;
    }

    pub fn read_into(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        self.stream.read(dst)
    }

    #[inline]
    pub fn is_finished_ok(&mut self) -> io::Result<bool> {
        Ok(self.code == 0 && self.is_eof()?)
    }

    #[inline]
    pub fn is_eof(&mut self) -> io::Result<bool> {
        util::is_eof(self.stream)
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
    pub fn decode_bit(&mut self, prob: &mut u16, update: bool) -> io::Result<bool> {
        let bound: u32 = (self.range >> 11) * (*prob as u32);

        lzma_trace!(
            " bound: {:08x}, prob: {:04x}, bit: {}",
            bound,
            prob,
            (self.code > bound) as u8
        );
        if self.code < bound {
            if update {
                *prob += (0x800_u16 - *prob) >> 5;
            }
            self.range = bound;

            self.normalize()?;
            Ok(false)
        } else {
            if update {
                *prob -= *prob >> 5;
            }
            self.code -= bound;
            self.range -= bound;

            self.normalize()?;
            Ok(true)
        }
    }

    fn parse_bit_tree(
        &mut self,
        num_bits: usize,
        probs: &mut [u16],
        update: bool,
    ) -> io::Result<u32> {
        let mut tmp: u32 = 1;
        for _ in 0..num_bits {
            let bit = self.decode_bit(&mut probs[tmp as usize], update)?;
            tmp = (tmp << 1) ^ (bit as u32);
        }
        Ok(tmp - (1 << num_bits))
    }

    pub fn parse_reverse_bit_tree(
        &mut self,
        num_bits: usize,
        probs: &mut [u16],
        offset: usize,
        update: bool,
    ) -> io::Result<u32> {
        let mut result = 0u32;
        let mut tmp: usize = 1;
        for i in 0..num_bits {
            let bit = self.decode_bit(&mut probs[offset + tmp], update)?;
            tmp = (tmp << 1) ^ (bit as usize);
            result ^= (bit as u32) << i;
        }
        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct BitTree<const PROBS_ARRAY_LEN: usize> {
    probs: [u16; PROBS_ARRAY_LEN],
}

impl<const PROBS_ARRAY_LEN: usize> BitTree<PROBS_ARRAY_LEN> {
    pub fn new() -> Self {
        // The validity of PROBS_ARRAY_LEN is checked at compile-time with a macro
        // that confirms that the argument P passed is indeed 1 << N for
        // some N using usize::trailing_zeros to calculate floor(log_2(P)).
        //
        // Thus, BitTree<const P: usize> is only valid for any P such that
        // P = 2 ** floor(log_2(P)), where P is the length of the probability array
        // of the BitTree. This maintains the invariant that P = 1 << N.
        //
        // This precondition must be checked for any way to construct a new, valid instance of BitTree.
        // Here it is checked for BitTree::new(), but if another function is added that returns a
        // new instance of BitTree, this assertion must be checked there as well.
        const_assert!("BitTree's PROBS_ARRAY_LEN parameter must be a power of 2",
            PROBS_ARRAY_LEN: usize => (1 << (PROBS_ARRAY_LEN.trailing_zeros() as usize)) == PROBS_ARRAY_LEN);
        BitTree {
            probs: [0x400; PROBS_ARRAY_LEN],
        }
    }

    // NUM_BITS is derived from PROBS_ARRAY_LEN because of the lack of
    // generic const expressions. Where PROBS_ARRAY_LEN is a power of 2,
    // NUM_BITS can be derived by the number of trailing zeroes.
    const NUM_BITS: usize = PROBS_ARRAY_LEN.trailing_zeros() as usize;

    pub fn parse<R: io::BufRead>(
        &mut self,
        rangecoder: &mut RangeDecoder<R>,
        update: bool,
    ) -> io::Result<u32> {
        rangecoder.parse_bit_tree(Self::NUM_BITS, &mut self.probs, update)
    }

    pub fn parse_reverse<R: io::BufRead>(
        &mut self,
        rangecoder: &mut RangeDecoder<R>,
        update: bool,
    ) -> io::Result<u32> {
        rangecoder.parse_reverse_bit_tree(Self::NUM_BITS, &mut self.probs, 0, update)
    }
}

#[derive(Debug)]
pub struct LenDecoder {
    choice: u16,
    choice2: u16,
    low_coder: [BitTree<{ 1 << 3 }>; 16],
    mid_coder: [BitTree<{ 1 << 3 }>; 16],
    high_coder: BitTree<{ 1 << 8 }>,
}

impl LenDecoder {
    pub fn new() -> Self {
        LenDecoder {
            choice: 0x400,
            choice2: 0x400,
            low_coder: [
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
            ],
            mid_coder: [
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
                BitTree::new(),
            ],
            high_coder: BitTree::new(),
        }
    }

    pub fn decode<R: io::BufRead>(
        &mut self,
        rangecoder: &mut RangeDecoder<R>,
        pos_state: usize,
        update: bool,
    ) -> io::Result<usize> {
        if !rangecoder.decode_bit(&mut self.choice, update)? {
            Ok(self.low_coder[pos_state].parse(rangecoder, update)? as usize)
        } else if !rangecoder.decode_bit(&mut self.choice2, update)? {
            Ok(self.mid_coder[pos_state].parse(rangecoder, update)? as usize + 8)
        } else {
            Ok(self.high_coder.parse(rangecoder, update)? as usize + 16)
        }
    }
}
