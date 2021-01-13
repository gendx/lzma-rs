#![no_main]
#[macro_use]
extern crate libfuzzer_sys;

use lzma_rs::error::Result;
use std::io::Write;

fn decode_lzma(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma_decompress(&mut std::io::Cursor::new(compressed), &mut decomp)?;
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
    if !input.is_empty() {
        let (chunk_size, input) = input.split_at(1);
        // use input length if chunk_size is zero because std::slice::chunks
        // will otherwise panic
        let chunk_size = if chunk_size[0] == 0 {
            input.len()
        } else {
            chunk_size[0] as usize
        };
        let mut compressed = Vec::new();
        lzma_rs::lzma_compress(&mut std::io::Cursor::new(input), &mut compressed).unwrap();
        let decompressed = decode_lzma(&compressed).unwrap();
        let decompressed_stream = decode_lzma_stream(&compressed, chunk_size).unwrap();
        if decompressed_stream.len() != decompressed.len() {
            panic!(
                "chunk size: {}, ref len: {}, act len: {}",
                chunk_size,
                decompressed.len(),
                decompressed_stream.len()
            );
        }
        assert_eq!(decompressed_stream, decompressed);
        assert_eq!(decompressed_stream, input);
    }
});
