#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate lzma_rs;
extern crate xz2;

use lzma_rs::error::Result;
use std::io::Read;

fn decode_xz_lzmars(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut bf = std::io::Cursor::new(compressed);
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::xz_decompress(&mut bf, &mut decomp)?;
    Ok(decomp)
}

fn encode_xz_xz2(data: &[u8]) -> Result<Vec<u8>> {
    let bf = std::io::Cursor::new(data);
    let mut compressed: Vec<u8> = Vec::new();
    xz2::bufread::XzEncoder::new(bf, 6).read_to_end(&mut compressed)?;
    Ok(compressed)
}

fuzz_target!(|data: &[u8]| {
    let compressed = encode_xz_xz2(data).expect("liblzma failed to compress data");
    let decoded =
        decode_xz_lzmars(&compressed).expect("We've failed to decompress what liblzma compressed");
    assert!(
        data == decoded.as_slice(),
        "Decompressed data is different from the original"
    );
});
