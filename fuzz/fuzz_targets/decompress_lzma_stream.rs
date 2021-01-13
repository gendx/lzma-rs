#![no_main]
#[macro_use]
extern crate libfuzzer_sys;

use lzma_rs::error::Result;
use std::io::Write;

fn decode_lzma(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut bf = std::io::Cursor::new(compressed);
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma_decompress(&mut bf, &mut decomp)?;
    Ok(decomp)
}

fn decode_lzma_stream(compressed: &[u8], chunk_size: usize) -> Result<Vec<u8>> {
    let mut stream = lzma_rs::decompress::Stream::new(Vec::new());
    for chunk in compressed.chunks(chunk_size) {
        stream.write_all(chunk).unwrap();
    }
    let decomp = stream.finish().unwrap();
    Ok(decomp)
}

fuzz_target!(|input: &[u8]| {
    let chunk_size = 2;
    let mut input = std::io::Cursor::new(input);
    let mut data = Vec::new();
    lzma_rs::lzma_compress(&mut input, &mut data).unwrap();
    let decomp_ref = decode_lzma(&data).unwrap();
    let decomp_act = decode_lzma_stream(&data, chunk_size as usize).unwrap();
    if decomp_act.len() != decomp_ref.len() {
        panic!(
            "chunk size: {}, ref len: {}, act len: {}",
            chunk_size,
            decomp_ref.len(),
            decomp_act.len()
        );
    }
    assert_eq!(decomp_act, decomp_ref);
});
