#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate lzma;

use lzma::error::Result;

fn round_trip(x: &[u8]) -> Result<Vec<u8>> {
    let mut compressed: Vec<u8> = Vec::new();
    lzma::lzma_compress(&mut std::io::BufReader::new(x), &mut compressed)?;
    let mut bf = std::io::BufReader::new(compressed.as_slice());

    let mut decomp: Vec<u8> = Vec::new();
    lzma::lzma_decompress(&mut bf, &mut decomp).expect("Can't decompress what we just compressed");
    Ok(decomp)
}

fuzz_target!(|data: &[u8]| {
    if let Ok(decomp) = round_trip(data) {
        assert_eq!(decomp, data)
    }
});
