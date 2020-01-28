extern crate env_logger;
extern crate lzma_rs;
#[macro_use]
extern crate log;

fn round_trip(x: &[u8]) {
    let mut compressed: Vec<u8> = Vec::new();
    lzma_rs::lzma_compress(&mut std::io::BufReader::new(x), &mut compressed).unwrap();
    info!("Compressed {} -> {} bytes", x.len(), compressed.len());
    debug!("Compressed content: {:?}", compressed);
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
    info!("Compressed {} -> {} bytes", x.len(), compressed.len());
    debug!("Compressed content: {:?}", compressed);
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
    let _ = env_logger::try_init();
    round_trip(b"");
    // Note: we use vec! to avoid storing the slice in the binary
    round_trip(vec![0x00; 1_000_000].as_slice());
    round_trip(vec![0xFF; 1_000_000].as_slice());
}

#[test]
fn round_trip_hello() {
    let _ = env_logger::try_init();
    round_trip(b"Hello world");
}

#[test]
fn round_trip_files() {
    let _ = env_logger::try_init();
    round_trip_file("tests/files/foo.txt");
    round_trip_file("tests/files/range-coder-edge-case");
}

#[test]
fn big_file() {
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
    let _ = env_logger::try_init();
    let mut x: &[u8] = b"\x5d\x00\x00\x80\x00\xff\xff\xff\xff\xff\xff\xff\xff\x00\x83\xff\
                         \xfb\xff\xff\xc0\x00\x00\x00";
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma_decompress(&mut x, &mut decomp).unwrap();
    assert_eq!(decomp, b"")
}

#[test]
fn decompress_hello_world() {
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

#[test]
fn unpacked_size_with_specified_length_and_last_byte_is_zero() {
    let mut data: &[u8] = &[
        93, 0, 0, 1, 0, 0, 0, 111, 253, 255, 255, 163, 183, 255, 71, 62, 72, 21, 114, 57, 97, 81,
        184, 146, 40, 230, 143, 221, 66, 251, 179, 253, 113, 133, 36, 209, 157, 136, 6, 166, 184,
        144, 144, 180, 72, 27, 108, 146, 211, 153, 161, 58, 255, 52, 129, 75, 240, 91, 145, 234,
        14, 20, 173, 77, 167, 21, 218, 124, 215, 37, 87, 175, 123, 84, 42, 90, 42, 15, 40, 156,
        200, 228, 82, 146, 100, 78, 137, 120, 145, 121, 117, 60, 144, 172, 178, 50, 13, 116, 246,
        17, 195, 181, 90, 136, 248, 128, 160, 103, 203, 131, 61, 101, 79, 13, 188, 166, 86, 177,
        61, 29, 24, 147, 226, 211, 42, 16, 116, 153, 103, 9, 17, 112, 188, 159, 117, 114, 125, 209,
        157, 150, 224, 44, 197, 39, 232, 193, 190, 15, 0, 4, 130, 28, 84, 73, 91, 189, 120, 8, 69,
        78, 165, 182, 187, 252, 105, 241, 61, 199, 210, 26, 194, 15, 70, 225, 186, 144, 150, 195,
        46, 150, 103, 144, 224, 196, 136, 25, 140, 45, 169, 29, 100, 201, 225, 234, 59, 16, 254,
        147, 168, 89, 240, 42, 238, 251, 69, 135, 217, 29, 243, 218, 10, 172, 191, 192, 95, 186,
        36, 117, 158, 138, 110, 8, 207, 141, 154, 9, 159, 181, 3, 71, 95, 111, 99, 247, 247, 33,
        89, 114, 7, 61, 46, 250, 138, 21, 2, 105, 135, 90, 83, 215, 223, 60, 180, 69, 243, 112,
        226, 228, 100, 144, 11, 167, 204, 83, 148, 112, 122, 31, 30, 71, 230, 64, 211, 22, 193,
        147, 121, 76, 180, 3, 79, 198, 164, 40, 176, 206, 62, 34, 200, 114, 9, 81, 33, 129, 115,
        94, 77, 166, 124, 38, 148, 20, 62, 133, 46, 21, 63, 37, 112, 202, 221, 26, 34, 4, 13, 189,
        74, 75, 162, 189, 241, 123, 154, 163, 59, 7, 148, 203, 156, 18, 125, 126, 147, 209, 158,
        105, 231, 27, 203, 191, 132, 50, 146, 226, 22, 201, 251, 40, 255, 101, 201, 255, 75, 201,
        60, 5, 36, 246, 121, 87, 144, 239, 19, 138, 52, 229, 23, 193, 207, 4, 113, 151, 154, 147,
        223, 52, 140, 114, 174, 146, 90, 0, 42, 38, 113, 62, 58, 164, 224, 122, 82, 205, 66, 43,
        153, 64, 134, 64, 140, 123, 119, 237, 154, 159, 175, 94, 254, 119, 160, 234, 217, 50, 124,
        84, 137, 204, 160, 36, 83, 32, 91, 171, 136, 100, 221, 214, 36, 161, 168, 31, 105, 199,
        188, 91, 14, 248, 37, 175, 98, 22, 164, 68, 234, 76, 175, 144, 32, 39, 10, 60, 201, 181,
        100, 52, 184, 202, 194, 77, 159, 147, 177, 98, 172, 139, 31, 185, 230, 46, 171, 105, 55,
        106, 24, 254, 236, 255, 110, 189, 247, 139, 213, 200, 241, 113, 20, 28, 232, 144, 194, 54,
        188, 180, 193, 196, 73, 234, 60, 111, 87, 228, 113, 186, 65, 174, 66, 219, 80, 167, 249,
        36, 43, 57, 144, 101, 25, 188, 250, 28, 217, 2, 203, 195, 217, 6, 52, 125, 206, 106, 211,
        148, 190, 119, 126, 34, 100, 117, 218, 183, 135, 108, 77, 244, 54, 116, 167, 24, 113, 104,
        211, 29, 14, 143, 255, 124, 241, 74, 135, 140, 131, 196, 245, 234, 245, 213, 189, 35, 139,
        127, 212, 247, 0,
    ];
    let unpacked_size = 5048;
    let decode_options = lzma_rs::decompress::Options {
        unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(unpacked_size)),
    };
    let mut decomp: Vec<u8> = Vec::new();
    lzma_rs::lzma_decompress_with_options(&mut data, &mut decomp, &decode_options).unwrap();
    assert_eq!(decomp.len() as u64, unpacked_size);
}
