use crate::{error, decode::lzbuffer::{self, LzBuffer}};
use byteorder::ReadBytesExt;
use std::{num::Wrapping, io};

#[derive(Debug)]
/// Decoder for XZ delta-encoded blocks (filter 3).
pub struct DeltaDecoder {
    distance: Wrapping<u8>,
    pos: Wrapping<u8>,
    delta: [Wrapping<u8>; 256],
}

impl DeltaDecoder {
    /// Creates a new object ready for transforming data that it's given.
    pub fn new(property_distance: u8) -> Self {
        DeltaDecoder {
            distance: Wrapping(property_distance) + Wrapping(1),
            pos: Wrapping(0u8),
            delta: [Wrapping(0u8); 256],
        }
    }

    /// Performs the equivalent of replacing this decompression state with a
    /// freshly allocated copy.
    ///
    /// This function may not allocate memory and will attempt to reuse any
    /// previously allocated resources.
    #[cfg(feature = "raw_decoder")]
    pub fn reset(&mut self) {
        self.pos = Wrapping(0u8);
        self.delta = [Wrapping(0u8); 256];
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
                Wrapping(byte)
            } else {
                lzma_info!("Delta end of input");
                break;
            };

            self.delta[self.pos.0 as usize] = byte + self.delta[(self.pos - self.distance).0 as usize];
            accum.append_literal(self.delta[self.pos.0 as usize].0)?;
            self.pos += 1;
        }

        accum.finish()?;
        Ok(())
    }
}
