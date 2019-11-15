use byteorder::WriteBytesExt;
use std::io;

pub struct RangeEncoder<'a, W>
where
    W: 'a + io::Write,
{
    stream: &'a mut W,
    range: u32,
    low: u64,
    cache: u8,
    cachesz: u32,
}

impl<'a, W> RangeEncoder<'a, W>
where
    W: io::Write,
{
    pub fn new(stream: &'a mut W) -> Self {
        let enc = Self {
            stream,
            range: 0xFFFF_FFFF,
            low: 0,
            cache: 0,
            cachesz: 1,
        };
        debug!("0 {{ range: {:08x}, low: {:010x} }}", enc.range, enc.low);
        enc
    }

    fn write_low(&mut self) -> io::Result<()> {
        if self.low < 0xFF00_0000 || self.low > 0xFFFF_FFFF {
            let mut tmp = self.cache;
            loop {
                let byte = tmp.wrapping_add((self.low >> 32) as u8);
                self.stream.write_u8(byte)?;
                debug!("> byte: {:02x}", byte);
                tmp = 0xFF;
                self.cachesz -= 1;
                if self.cachesz == 0 {
                    break;
                }
            }
            self.cache = (self.low >> 24) as u8;
        }

        self.cachesz += 1;
        self.low = (self.low << 8) & 0xFFFF_FFFF;
        Ok(())
    }

    pub fn finish(&mut self) -> io::Result<()> {
        for _ in 0..5 {
            self.write_low()?;

            debug!("$ {{ range: {:08x}, low: {:010x} }}", self.range, self.low);
        }
        Ok(())
    }

    fn normalize(&mut self) -> io::Result<()> {
        while self.range < 0x1000000 {
            debug!(
                "+ {{ range: {:08x}, low: {:010x}, cache: {:02x}, {} }}",
                self.range, self.low, self.cache, self.cachesz
            );
            self.range <<= 8;
            self.write_low()?;
            debug!(
                "* {{ range: {:08x}, low: {:010x}, cache: {:02x}, {} }}",
                self.range, self.low, self.cache, self.cachesz
            );
        }
        trace!("  {{ range: {:08x}, low: {:010x} }}", self.range, self.low);
        Ok(())
    }

    pub fn encode_bit(&mut self, prob: &mut u16, bit: bool) -> io::Result<()> {
        let bound: u32 = (self.range >> 11) * (*prob as u32);
        trace!(
            "  bound: {:08x}, prob: {:04x}, bit: {}",
            bound,
            prob,
            bit as u8
        );

        if bit {
            *prob -= *prob >> 5;
            self.low += bound as u64;
            self.range -= bound;
        } else {
            *prob += (0x800_u16 - *prob) >> 5;
            self.range = bound;
        }

        self.normalize()
    }
}
