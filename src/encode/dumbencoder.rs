use std::io;
use error;
use encode::stream;
use util;

pub struct Encoder<'a, W>
where
    W: 'a + io::Write,
{
    dict_size: u32,
    stream: stream::EncodeStream<'a, W>,
    literal_probs: [[u16; 0x300]; 8],
    is_match: [u16; 4], // true = LZ, false = literal
}

const lc: u32 = 3;
const lp: u32 = 0;
const pb: u32 = 2;

impl<'a, W> Encoder<'a, W>
where
    W: io::Write,
{
    pub fn from_stream(stream: &'a mut W) -> error::Result<Self> {
        let dict_size = 0x800000;

        // Properties
        let props = (lc + 9 * (lp + 5 * pb)) as u8;
        info!("Properties {{ lc: {}, lp: {}, pb: {} }}", lc, lp, pb);
        util::write_u8(stream, props)?;

        // Dictionary
        info!("Dict size: {}", dict_size);
        util::write_u32_le(stream, dict_size)?;

        // Unpacked size
        info!("Unpacked size: unknown");
        util::write_u64_be(stream, 0xFFFF_FFFF_FFFF_FFFF)?;

        let encoder = Encoder {
            dict_size: dict_size,
            stream: stream::EncodeStream::new(stream)?,
            literal_probs: [[0x400; 0x300]; 8],
            is_match: [0x400; 4],
        };

        Ok(encoder)
    }

    pub fn process<R>(mut self, input: R) -> error::Result<()>
    where
        R: io::Read,
    {
        let mut prev_byte = 0u8;
        let mut input_len = 0;

        for (out_len, byte_result) in input.bytes().enumerate() {
            let byte = byte_result?;
            let pos_state = out_len & 3;
            input_len = out_len;

            // Literal
            self.stream.encode_bit(&mut self.is_match[pos_state], false)?;

            self.encode_literal(byte, prev_byte, out_len)?;
            prev_byte = byte;
        }

        self.finish(input_len + 1)?;
        Ok(())
    }

    fn finish(&mut self, input_len: usize) -> io::Result<()> {
        // Write end-of-stream marker
        let pos_state = input_len & 3;

        // Match
        self.stream.encode_bit(&mut self.is_match[pos_state], true)?;
        // New distance
        self.stream.encode_bit(&mut 0x400, false)?;

        // Dummy len, as small as possible (len = 0)
        for _ in 0..4 {
            self.stream.encode_bit(&mut 0x400, false)?;
        }

        // Distance marker = 0xFFFFFFFF
        // pos_slot = 63
        for _ in 0..6 {
            self.stream.encode_bit(&mut 0x400, true)?;
        }
        // num_direct_bits = 30
        // result = 3 << 30 = C000_0000
        //        + 3FFF_FFF0  (26 bits)
        //        + F          ( 4 bits)
        for _ in 0..30 {
            self.stream.encode_bit(&mut 0x400, true)?;
        }
        //        = FFFF_FFFF

        // Flush range coder
        self.stream.finish()
    }

    fn encode_literal(&mut self, byte: u8, prev_byte: u8, out_len: usize) -> io::Result<()> {
        let prev_byte = prev_byte as usize;

        let mut result: usize = 1;
        let lit_state = prev_byte >> 5;
        let probs = &mut self.literal_probs[lit_state];

        for i in 0..8 {
            let bit = ((byte >> (7 - i)) & 1) != 0;
            self.stream.encode_bit(&mut probs[result], bit)?;
            result = (result << 1) ^ (bit as usize);
        }

        Ok(())
    }
}
