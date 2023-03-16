#![no_main]
#[macro_use]
extern crate libfuzzer_sys;

use lzma_rs::error::Result;
use std::io::Read;
use xz2::stream;

fn encode_xz_lzmars(x: &[u8]) -> Result<Vec<u8>> {
    let mut compressed: Vec<u8> = Vec::new();
    lzma_rs::xz_compress(&mut std::io::BufReader::new(x), &mut compressed)?;
    Ok(compressed)
}

fn decode_xz_xz2(compressed: &[u8]) -> Result<Vec<u8>> {
    let bf = std::io::Cursor::new(compressed);
    let mut decomp: Vec<u8> = Vec::new();
    // create new XZ decompression stream with 8Gb memory limit and checksum
    // verification disabled
    let xz_stream =
        stream::Stream::new_stream_decoder(8 * 1024 * 1024 * 1024, stream::IGNORE_CHECK)
            .expect("Failed to create stream");
    xz2::bufread::XzDecoder::new_stream(bf, xz_stream).read_to_end(&mut decomp)?;
    Ok(decomp)
}

fuzz_target!(|data: &[u8]| {
    let compressed = encode_xz_lzmars(data).expect("Compression failed");
    let decoded =
        decode_xz_xz2(&compressed).expect("liblzma failed to decompress what we've compressed");
    assert!(
        data == decoded.as_slice(),
        "Decompressed data is different from the original"
    );
});
