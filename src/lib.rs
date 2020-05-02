//! Pure-Rust codecs for LZMA, LZMA2, and XZ.

#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![forbid(unsafe_code)]

#[macro_use]
mod macros;

mod decode;
mod encode;
pub mod error;
mod xz;

use crate::decode::lzbuffer::LZBuffer;
use std::io;

/// Compression helpers.
pub mod compress {
    pub use crate::encode::options::*;
}

/// Decompression helpers.
pub mod decompress {
    pub use crate::decode::options::*;
}

/// Decompress LZMA data with default [`Options`](decompress/struct.Options.html).
pub fn lzma_decompress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> error::Result<()> {
    lzma_decompress_with_options(input, output, &decompress::Options::default())
}

/// Decompress LZMA data with the provided options.
pub fn lzma_decompress_with_options<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
    options: &decompress::Options,
) -> error::Result<()> {
    let params = decode::lzma::LZMAParams::read_header(input, options)?;
    let mut decoder = decode::lzma::new_circular(output, params)?;
    let mut rangecoder = decode::rangecoder::RangeDecoder::new(input).or_else(|e| {
        Err(error::Error::LZMAError(format!(
            "LZMA stream too short: {}",
            e
        )))
    })?;
    decoder.process(&mut rangecoder)?;
    decoder.output.finish()?;
    Ok(())
}

/// Compresses data with LZMA and default [`Options`](compress/struct.Options.html).
pub fn lzma_compress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> io::Result<()> {
    lzma_compress_with_options(input, output, &compress::Options::default())
}

/// Compress LZMA data with the provided options.
pub fn lzma_compress_with_options<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
    options: &compress::Options,
) -> io::Result<()> {
    let encoder = encode::dumbencoder::Encoder::from_stream(output, options)?;
    encoder.process(input)
}

/// Decompress LZMA2 data with default [`Options`](decompress/struct.Options.html).
pub fn lzma2_decompress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> error::Result<()> {
    decode::lzma2::decode_stream(input, output)
}

/// Compress data with LZMA2 and default [`Options`](compress/struct.Options.html).
pub fn lzma2_compress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> io::Result<()> {
    encode::lzma2::encode_stream(input, output)
}

/// Decompress XZ data with default [`Options`](decompress/struct.Options.html).
pub fn xz_decompress<R: io::BufRead, W: io::Write>(
    input: &mut R,
    output: &mut W,
) -> error::Result<()> {
    decode::xz::decode_stream(input, output)
}

/// Compress data with XZ and default [`Options`](compress/struct.Options.html).
pub fn xz_compress<R: io::BufRead, W: io::Write>(input: &mut R, output: &mut W) -> io::Result<()> {
    encode::xz::encode_stream(input, output)
}
