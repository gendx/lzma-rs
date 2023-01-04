#[cfg(feature = "enable_logging")]
use log::{debug, info};
use std::io::Read;

/// Utility function to read a file into memory
fn read_all_file(filename: &str) -> std::io::Result<Vec<u8>> {
    let mut data = Vec::new();
    std::fs::File::open(filename).and_then(|mut file| file.read_to_end(&mut data))?;
    Ok(data)
}

fn round_trip(x: &[u8], decoder_options: lzma_rs::decompress::Options) {
    let mut compressed: Vec<u8> = Vec::new();
    lzma_rs::lzma2_compress(&mut std::io::BufReader::new(x), &mut compressed).unwrap();
    #[cfg(feature = "enable_logging")]
    info!("Compressed {} -> {} bytes", x.len(), compressed.len());
    #[cfg(feature = "enable_logging")]
    debug!("Compressed content: {:?}", compressed);
    let mut bf = std::io::BufReader::new(compressed.as_slice());
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma2_decompress(&mut bf, &mut decomp, decoder_options).unwrap();
    assert_eq!(decomp, x)
}

fn round_trip_file(filename: &str, decoder_options: lzma_rs::decompress::Options) {
    let x = read_all_file(filename).unwrap();
    round_trip(x.as_slice(), decoder_options);
}

#[test]
fn round_trip_basics() {
    let options = lzma_rs::decompress::Options::default();
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    round_trip(b"", options.clone());
    // Note: we use vec! to avoid storing the slice in the binary
    round_trip(vec![0x00; 1_000_000].as_slice(), options.clone());
    round_trip(vec![0xFF; 1_000_000].as_slice(), options.clone());
}

#[test]
fn round_trip_hello() {
    let options = Default::default();
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    round_trip(b"Hello world", options);
}

#[test]
fn round_trip_files() {
    let options = Default::default();
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    round_trip_file("tests/files/foo.txt", options);
}
