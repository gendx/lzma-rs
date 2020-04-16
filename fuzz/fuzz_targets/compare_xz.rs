#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate lzma_rs;
extern crate xz2;

use lzma_rs::error::Result;
use xz2::stream;

fn decode_xz_lzmars(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut bf = std::io::Cursor::new(compressed);
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::xz_decompress(&mut bf, &mut decomp)?;
    Ok(decomp)
}

fn decode_xz_xz2(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut bf = std::io::Cursor::new(compressed);
    let mut decomp: Vec<u8> = Vec::new();
    // create new XZ decompression stream with 8Gb memory limit and checksum verification disabled
    let xz_stream = Stream::new_stream_decoder(8589935000, IGNORE_CHECK);
    xz2::bufread::XzDecoder::new_stream(b, xz_stream).read_to_end(&mut decomp)?;
    Ok(decomp)
}

fuzz_target!(|data: &[u8]| {
    let result_lzmars = decode_xz_lzmars(data);
    let result_xz2 = decode_xz_xz2(data);
    match (result_lzmars, result_xz2) {
        Err, Err => (), // both failed, so behavior matches
        Ok, Err => panic!("lzma-rs succeeded but xz2 failed"),
        Err, Ok => panic!("xz2 succeeded but lzma-rs failed"),
        Ok(a), Ok(b) => assert!(a.as_slice() == b.as_slice())
    }
});
