#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate lzma_rs;
extern crate xz2;

use std::io::Read;
use lzma_rs::error::Result;
use xz2::stream;

fn decode_xz_lzmars(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut bf = std::io::Cursor::new(compressed);
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::xz_decompress(&mut bf, &mut decomp)?;
    Ok(decomp)
}

fn decode_xz_xz2(compressed: &[u8]) -> Result<Vec<u8>> {
    let bf = std::io::Cursor::new(compressed);
    let mut decomp: Vec<u8> = Vec::new();
    // create new XZ decompression stream with 8Gb memory limit and checksum verification disabled
    let xz_stream = stream::Stream::new_stream_decoder(8*1024*1024*1024, stream::IGNORE_CHECK).expect("Failed to create stream");
    xz2::bufread::XzDecoder::new_stream(bf, xz_stream).read_to_end(&mut decomp)?;
    Ok(decomp)
}

fuzz_target!(|data: &[u8]| {
    let result_lzmars = decode_xz_lzmars(data);
    let result_xz2 = decode_xz_xz2(data);
    match (result_lzmars, result_xz2) {
        (Err(_), Err(_)) => (), // both failed, so behavior matches
        (Ok(_), Err(_)) => panic!("lzma-rs succeeded but xz2 failed"),
        (Err(_), Ok(_)) => panic!("xz2 succeeded but lzma-rs failed"),
        (Ok(a), Ok(b)) => assert!(a == b)
    }
});
