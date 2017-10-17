extern crate byteorder;
#[macro_use]
extern crate log;

mod encode;
mod decode;
pub mod error;

use std::io;

pub fn decompress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> error::Result<()> {
    let decoder = decode::decoder::Decoder::from_stream(input, output)?;
    decoder.process()
}

pub fn compress<R: io::BufRead, W: io::Write>(input: &mut R, output: &mut W) -> io::Result<()> {
    let encoder = encode::dumbencoder::Encoder::from_stream(output)?;
    encoder.process(input)
}
