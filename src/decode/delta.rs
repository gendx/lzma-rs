use crate::decode::lzbuffer;
use crate::decode::lzbuffer::LzBuffer;
use crate::error;
use byteorder::ReadBytesExt;
use std::io;

#[derive(Debug)]
/// Decoder for XZ delta-encoded blocks (filter 3).
pub struct DeltaDecoder {
    distance: usize,
    pos: u8,
    delta: [u8; 256],
}

impl DeltaDecoder {
    /// Creates a new object ready for transforming data that it's given.
    pub fn new(property_distance: u8) -> Self {
        DeltaDecoder {
            distance: property_distance as usize + 1,
            pos: 0,
            delta: [0u8; 256],
        }
    }

    /// Performs the equivalent of replacing this decompression state with a
    /// freshly allocated copy.
    ///
    /// This function may not allocate memory and will attempt to reuse any
    /// previously allocated resources.
    #[cfg(feature = "raw_decoder")]
    pub fn reset(&mut self) {
        self.pos = 0;
        self.delta = [0u8; 256];
    }

    /// Decompresses the input data into the output, consuming only as much
    /// input as needed and writing as much output as possible.
    pub fn decompress<W: io::Write, R: io::BufRead>(
        &mut self,
        input: &mut R,
        output: &mut W,
    ) -> error::Result<()> {
        let mut accum = lzbuffer::LzAccumBuffer::from_stream(output, usize::MAX);

        // See xz-file-format.txt for the C pseudocode this is implementing.
        loop {
            let byte = if let Ok(byte) = input.read_u8() {
                byte
            } else {
                lzma_info!("Delta end of input");
                break;
            };

            let tmp = self.delta[(self.distance + self.pos as usize) as u8 as usize];
            let tmp = byte.wrapping_add(tmp);
            self.delta[self.pos as usize] = tmp;

            accum.append_literal(tmp)?;
            self.pos = self.pos.wrapping_sub(1);
        }

        accum.finish()?;
        Ok(())
    }
}
