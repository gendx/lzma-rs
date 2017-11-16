extern crate byteorder;
#[macro_use]
extern crate log;

mod encode;
mod decode;
pub mod error;

use std::io;

pub fn lzma_decompress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> error::Result<()> {
    let params = decode::decoder::LZMAParams::read_header(input)?;
    let mut decoder = decode::decoder::new_circular(output, params)?;
    let mut rangecoder = try!(decode::rangecoder::RangeDecoder::new(input).or_else(|e| {
        Err(error::Error::LZMAError(
            format!("LZMA stream too short: {}", e),
        ))
    }));
    decoder.process(&mut rangecoder)
}

pub fn lzma_compress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> io::Result<()> {
    let encoder = encode::dumbencoder::Encoder::from_stream(output)?;
    encoder.process(input)
}

pub fn lzma2_decompress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> error::Result<()> {
    decode::lzma2::decode_stream(input, output)
}
