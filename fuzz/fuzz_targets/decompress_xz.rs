#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate lzma_rs;

use lzma_rs::error::Result;

fn decode_xz(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut bf = std::io::Cursor::new(compressed);

    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::xz_decompress(&mut bf, &mut decomp)?;
    Ok(decomp)
}

fuzz_target!(|data: &[u8]| {
    let decomp = decode_xz(data);
});
