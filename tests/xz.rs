#[cfg(feature = "enable_logging")]
use log::{debug, info};
use std::{fs, io::{BufRead, BufReader, Cursor, Read}};
use xz2::stream;

fn round_trip(x: &[u8]) {
    let mut compressed: Vec<u8> = Vec::new();
    lzma_rs::xz_compress(&mut std::io::BufReader::new(x), &mut compressed).unwrap();
    #[cfg(feature = "enable_logging")]
    info!("Compressed {} -> {} bytes", x.len(), compressed.len());
    #[cfg(feature = "enable_logging")]
    debug!("Compressed content: {:?}", compressed);
    let mut bf = BufReader::new(compressed.as_slice());
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::xz_decompress(&mut bf, &mut decomp).unwrap();
    assert_eq!(decomp, x)
}

fn round_trip_file(filename: &str) {
    let x = fs::read(filename).unwrap();
    round_trip(x.as_slice());
}

#[test]
fn round_trip_basics() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    round_trip(b"");
    // Note: we use vec! to avoid storing the slice in the binary
    round_trip(vec![0x00; 1_000_000].as_slice());
    round_trip(vec![0xFF; 1_000_000].as_slice());
}

#[test]
fn round_trip_hello() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    round_trip(b"Hello world");
}

#[test]
fn round_trip_files() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    round_trip_file("tests/files/foo.txt");
}

fn decode_xz_xz2<R: BufRead>(f: R) -> Vec<u8> {
    // create new XZ decompression stream with 8Gb memory limit and checksum
    // verification disabled
    let xz_stream =
        stream::Stream::new_stream_decoder(8 * 1024 * 1024 * 1024, stream::IGNORE_CHECK)
            .expect("Failed to create stream");
    let mut decomp: Vec<u8> = Vec::new();
    xz2::bufread::XzDecoder::new_stream(f, xz_stream).read_to_end(&mut decomp).unwrap();
    decomp
}

fn decomp_big_file(compfile: &str, plainfile: &str) {
    let expected = fs::read(plainfile).unwrap();

    // Decode with the reference implementation to ensure our test case is accurate
    let mut f = BufReader::new(fs::File::open(compfile).unwrap());
    let decomp = decode_xz_xz2(f);
    assert!(decomp == expected);

    let mut f = BufReader::new(fs::File::open(compfile).unwrap());
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::xz_decompress(&mut f, &mut decomp).unwrap();
    assert!(decomp == expected)
}

#[test]
fn big_file() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    decomp_big_file("tests/files/foo.txt.xz", "tests/files/foo.txt");
    decomp_big_file(
        "tests/files/good-1-lzma2-1.xz",
        "tests/files/good-1-lzma2-1",
    );
    decomp_big_file(
        "tests/files/good-1-lzma2-2.xz",
        "tests/files/good-1-lzma2-2",
    );
    decomp_big_file(
        "tests/files/good-1-lzma2-3.xz",
        "tests/files/good-1-lzma2-3",
    );
    decomp_big_file(
        "tests/files/good-1-lzma2-4.xz",
        "tests/files/good-1-lzma2-4",
    );
}

#[test]
fn decompress_empty_world() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    let mut x: &[u8] = b"\xfd\x37\x7a\x58\x5a\x00\x00\x04\xe6\xd6\xb4\x46\x00\x00\x00\x00\
                         \x1c\xdf\x44\x21\x1f\xb6\xf3\x7d\x01\x00\x00\x00\x00\x04\x59\x5a\
                         ";
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::xz_decompress(&mut x, &mut decomp).unwrap();
    assert_eq!(decomp, b"")
}

#[test]
fn decompress_hello_world() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    let mut x: &[u8] = b"\xfd\x37\x7a\x58\x5a\x00\x00\x04\xe6\xd6\xb4\x46\x02\x00\x21\x01\
                         \x16\x00\x00\x00\x74\x2f\xe5\xa3\x01\x00\x0b\x48\x65\x6c\x6c\x6f\
                         \x20\x77\x6f\x72\x6c\x64\x0a\x00\xca\xec\x49\x05\x66\x3f\x67\x98\
                         \x00\x01\x24\x0c\xa6\x18\xd8\xd8\x1f\xb6\xf3\x7d\x01\x00\x00\x00\
                         \x00\x04\x59\x5a";
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::xz_decompress(&mut x, &mut decomp).unwrap();
    assert_eq!(decomp, b"Hello world\x0a")
}

#[test]
fn test_xz_block_check_crc32() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();

    decomp_big_file(
        "tests/files/block-check-crc32.txt.xz",
        "tests/files/block-check-crc32.txt",
    );
}

#[test]
fn test_xz_block_check_crc32_invalid() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();

    let testcase = "tests/files/block-check-crc32.txt.xz";
    let mut corrupted = {
        let mut buf = fs::read(testcase).unwrap();
        // Mangle the "Block Check" field.
        buf[0x54] = 0x67;
        buf[0x55] = 0x45;
        buf[0x56] = 0x23;
        buf[0x57] = 0x01;
        BufReader::new(Cursor::new(buf))
    };
    let mut decomp = Vec::new();

    let err_msg = lzma_rs::xz_decompress(&mut corrupted, &mut decomp)
        .unwrap_err()
        .to_string();
    assert_eq!(
        err_msg,
        "xz error: Invalid footer CRC32: expected 0x01234567 but got 0x8b0d303e"
    )
}

#[test]
fn test_xz_delta_filter() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();

    decomp_big_file(
        "tests/files/delta-filter-3.dat.xz",
        "tests/files/delta-filter-3.dat",
    );
}
