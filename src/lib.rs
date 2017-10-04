#[macro_use]
extern crate log;

mod encode;
mod decode;
pub mod error;
mod util;

use std::io;

pub fn decompress<R: io::BufRead>(stream: &mut R) -> error::Result<Vec<u8>> {
    let decoder = decode::decoder::Decoder::from_stream(stream)?;
    decoder.process()
}

pub fn compress<R: io::BufRead, W: io::Write>(input: &mut R, output: &mut W) -> io::Result<()> {
    let encoder = encode::dumbencoder::Encoder::from_stream(output)?;
    encoder.process(input)
}