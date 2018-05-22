extern crate byteorder;
#[macro_use]
extern crate log;
extern crate crc;

mod decode;
mod encode;
pub mod error;

use decode::lzbuffer::LZBuffer;
use std::io;

pub fn lzma_decompress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> error::Result<()> {
    let params = decode::lzma::LZMAParams::read_header(input)?;
    let mut decoder = decode::lzma::new_circular(output, params)?;
    let mut rangecoder = try!(
        decode::rangecoder::RangeDecoder::new(input).or_else(|e| {
            Err(error::Error::LZMAError(format!(
                "LZMA stream too short: {}",
                e
            )))
        })
    );
    decoder.process(&mut rangecoder)?;
    decoder.output.finish()?;
    Ok(())
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

pub fn lzma2_compress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> io::Result<()> {
    encode::lzma2::encode_stream(input, output)
}

pub fn xz_decompress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> error::Result<()> {
    decode::xz::decode_stream(input, output)
}

pub fn xz_compress<R: io::BufRead, W: io::Write>(input: &mut R, output: &mut W) -> io::Result<()> {
    encode::xz::encode_stream(input, output)
}
