#[cfg(feature = "enable_logging")]
extern crate env_logger;
#[macro_use]
extern crate lzma_rs;
#[cfg(feature = "enable_logging")]
#[macro_use]
extern crate log;

fn round_trip(x: &[u8]) {
    round_trip_no_options(x);

    // Do another round trip, but this time also write it to the header
    let encode_options = lzma_rs::compress::Options {
        unpacked_size: lzma_rs::compress::UnpackedSize::WriteToHeader(Some(x.len() as u64)),
    };
    let decode_options = lzma_rs::decompress::Options {
        unpacked_size: lzma_rs::decompress::UnpackedSize::ReadFromHeader,
    };
    round_trip_with_options(x, &encode_options, &decode_options);
}

fn round_trip_no_options(x: &[u8]) {
    let mut compressed: Vec<u8> = Vec::new();
    lzma_rs::lzma_compress(&mut std::io::BufReader::new(x), &mut compressed).unwrap();
    lzma_info!("Compressed {} -> {} bytes", x.len(), compressed.len());
    lzma_debug!("Compressed content: {:?}", compressed);
    let mut bf = std::io::BufReader::new(compressed.as_slice());
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma_decompress(&mut bf, &mut decomp).unwrap();
    assert_eq!(decomp, x)
}

fn round_trip_with_options(
    x: &[u8],
    encode_options: &lzma_rs::compress::Options,
    decode_options: &lzma_rs::decompress::Options,
) {
    let mut compressed: Vec<u8> = Vec::new();
    lzma_rs::lzma_compress_with_options(
        &mut std::io::BufReader::new(x),
        &mut compressed,
        encode_options,
    )
    .unwrap();
    lzma_info!("Compressed {} -> {} bytes", x.len(), compressed.len());
    lzma_debug!("Compressed content: {:?}", compressed);
    let mut bf = std::io::BufReader::new(compressed.as_slice());
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma_decompress_with_options(&mut bf, &mut decomp, decode_options).unwrap();
    assert_eq!(decomp, x)
}

fn round_trip_file(filename: &str) {
    use std::io::Read;

    let mut x = Vec::new();
    std::fs::File::open(filename)
        .unwrap()
        .read_to_end(&mut x)
        .unwrap();
    round_trip(x.as_slice());
}

fn decomp_big_file(compfile: &str, plainfile: &str) {
    use std::io::Read;

    let mut expected = Vec::new();
    std::fs::File::open(plainfile)
        .unwrap()
        .read_to_end(&mut expected)
        .unwrap();
    let mut f = std::io::BufReader::new(std::fs::File::open(compfile).unwrap());
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma_decompress(&mut f, &mut decomp).unwrap();
    assert!(decomp == expected)
}

#[test]
fn decompress_short_header() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    let mut decomp: Vec<u8> = Vec::new();
    // TODO: compare io::Errors?
    assert_eq!(
        format!(
            "{:?}",
            lzma_rs::lzma_decompress(&mut (b"" as &[u8]), &mut decomp).unwrap_err()
        ),
        String::from("LZMAError(\"LZMA header too short: failed to fill whole buffer\")")
    )
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
    round_trip_file("tests/files/range-coder-edge-case");
}

#[test]
fn big_file() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    decomp_big_file("tests/files/foo.txt.lzma", "tests/files/foo.txt");
    decomp_big_file("tests/files/hugedict.txt.lzma", "tests/files/foo.txt");
    decomp_big_file(
        "tests/files/range-coder-edge-case.lzma",
        "tests/files/range-coder-edge-case",
    );
}

#[test]
fn decompress_empty_world() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    let mut x: &[u8] = b"\x5d\x00\x00\x80\x00\xff\xff\xff\xff\xff\xff\xff\xff\x00\x83\xff\
                         \xfb\xff\xff\xc0\x00\x00\x00";
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma_decompress(&mut x, &mut decomp).unwrap();
    assert_eq!(decomp, b"")
}

#[test]
fn decompress_hello_world() {
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    let mut x: &[u8] = b"\x5d\x00\x00\x80\x00\xff\xff\xff\xff\xff\xff\xff\xff\x00\x24\x19\
                         \x49\x98\x6f\x10\x19\xc6\xd7\x31\xeb\x36\x50\xb2\x98\x48\xff\xfe\
                         \xa5\xb0\x00";
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma_decompress(&mut x, &mut decomp).unwrap();
    assert_eq!(decomp, b"Hello world\x0a")
}

#[test]
fn decompress_huge_dict() {
    // Hello world with a dictionary of size 0x7F7F7F7F
    #[cfg(feature = "enable_logging")]
    let _ = env_logger::try_init();
    let mut x: &[u8] = b"\x5d\x7f\x7f\x7f\x7f\xff\xff\xff\xff\xff\xff\xff\xff\x00\x24\x19\
                         \x49\x98\x6f\x10\x19\xc6\xd7\x31\xeb\x36\x50\xb2\x98\x48\xff\xfe\
                         \xa5\xb0\x00";
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma_decompress(&mut x, &mut decomp).unwrap();
    assert_eq!(decomp, b"Hello world\x0a")
}

#[test]
fn unpacked_size_write_to_header() {
    let data = b"Some data";
    let encode_options = lzma_rs::compress::Options {
        unpacked_size: lzma_rs::compress::UnpackedSize::WriteToHeader(Some(data.len() as u64)),
    };
    let decode_options = lzma_rs::decompress::Options {
        unpacked_size: lzma_rs::decompress::UnpackedSize::ReadFromHeader,
    };
    round_trip_with_options(&data[..], &encode_options, &decode_options);
}

#[test]
fn unpacked_size_provided_outside() {
    let data = b"Some data";
    let encode_options = lzma_rs::compress::Options {
        unpacked_size: lzma_rs::compress::UnpackedSize::SkipWritingToHeader,
    };
    let decode_options = lzma_rs::decompress::Options {
        unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(data.len() as u64)),
    };
    round_trip_with_options(&data[..], &encode_options, &decode_options);
}

#[test]
fn unpacked_size_write_some_to_header_but_use_provided_on_read() {
    let data = b"Some data";
    let encode_options = lzma_rs::compress::Options {
        unpacked_size: lzma_rs::compress::UnpackedSize::WriteToHeader(Some(data.len() as u64)),
    };
    let decode_options = lzma_rs::decompress::Options {
        unpacked_size: lzma_rs::decompress::UnpackedSize::ReadHeaderButUseProvided(Some(
            data.len() as u64,
        )),
    };
    round_trip_with_options(&data[..], &encode_options, &decode_options);
}

#[test]
fn unpacked_size_write_none_to_header_and_use_provided_on_read() {
    let data = b"Some data";
    let encode_options = lzma_rs::compress::Options {
        unpacked_size: lzma_rs::compress::UnpackedSize::WriteToHeader(None),
    };
    let decode_options = lzma_rs::decompress::Options {
        unpacked_size: lzma_rs::decompress::UnpackedSize::ReadHeaderButUseProvided(Some(
            data.len() as u64,
        )),
    };
    round_trip_with_options(&data[..], &encode_options, &decode_options);
}

#[test]
fn unpacked_size_write_none_to_header_and_use_provided_none_on_read() {
    let data = b"Some data";
    let encode_options = lzma_rs::compress::Options {
        unpacked_size: lzma_rs::compress::UnpackedSize::WriteToHeader(None),
    };
    let decode_options = lzma_rs::decompress::Options {
        unpacked_size: lzma_rs::decompress::UnpackedSize::ReadHeaderButUseProvided(None),
    };
    round_trip_with_options(&data[..], &encode_options, &decode_options);
}
