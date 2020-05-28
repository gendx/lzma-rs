#![no_main]
#[macro_use]
extern crate libfuzzer_sys;

use lzma_rs::error::Result;

fn decode_lzma(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut bf = std::io::Cursor::new(compressed);

    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma_decompress(&mut bf, &mut decomp)?;
    Ok(decomp)
}

fuzz_target!(|data: &[u8]| {
    let _decomp = decode_lzma(data);
});
