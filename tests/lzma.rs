extern crate lzma;
extern crate env_logger;

#[macro_use]
extern crate log;

fn round_trip(x: &[u8]) {
    let mut compressed: Vec<u8> = Vec::new();
    lzma::compress(&mut std::io::BufReader::new(x), &mut compressed).unwrap();
    info!("Compressed {} -> {} bytes", x.len(), compressed.len());
    debug!("Compressed content: {:?}", compressed);
    let mut bf = std::io::BufReader::new(compressed.as_slice());
    assert_eq!(lzma::decompress(&mut bf).unwrap(), x)
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

fn decomp_big_file(cipherfile: &str, plainfile: &str) {
    use std::io::Read;

    let mut expected = Vec::new();
    std::fs::File::open(plainfile)
        .unwrap()
        .read_to_end(&mut expected)
        .unwrap();
    let mut f = std::io::BufReader::new(std::fs::File::open(cipherfile).unwrap());
    assert!(lzma::decompress(&mut f).unwrap() == expected)
}

#[test]
fn decompress_short_header() {
    let _ = env_logger::init();
    // TODO: compare io::Errors?
    assert_eq!(
        format!("{:?}", lzma::decompress(&mut "".as_bytes()).unwrap_err()),
        String::from(
            "LZMAError(\"LZMA header too short: failed to fill whole buffer\")",
        )
    )
}

#[test]
fn round_trip_basics() {
    let _ = env_logger::init();
    round_trip(b"");
    // Note: we use vec! to avoid storing the slice in the binary
    round_trip(vec![0x00; 1_000_000].as_slice());
    round_trip(vec![0xFF; 1_000_000].as_slice());
}

#[test]
fn round_trip_hello() {
    let _ = env_logger::init();
    round_trip(b"Hello world");
}

#[test]
fn round_trip_files() {
    let _ = env_logger::init();
    round_trip_file("tests/files/foo.txt");
}

#[test]
fn big_file() {
    let _ = env_logger::init();
    decomp_big_file("tests/files/foo.txt.lzma", "tests/files/foo.txt");
}

#[test]
fn decompress_empty_world() {
    let _ = env_logger::init();
    assert_eq!(
        {
            let mut x: &[u8] = &[
                0x5d,
                0x00,
                0x00,
                0x80,
                0x00,
                0xff,
                0xff,
                0xff,
                0xff,
                0xff,
                0xff,
                0xff,
                0xff,
                0x00,
                0x83,
                0xff,
                0xfb,
                0xff,
                0xff,
                0xc0,
                0x00,
                0x00,
                0x00,
            ];
            lzma::decompress(&mut x).unwrap()
        },
        b""
    )
}

#[test]
fn decompress_hello_world() {
    let _ = env_logger::init();
    assert_eq!(
        {
            let mut x: &[u8] = &[
                0x5d,
                0x00,
                0x00,
                0x80,
                0x00,
                0xff,
                0xff,
                0xff,
                0xff,
                0xff,
                0xff,
                0xff,
                0xff,
                0x00,
                0x24,
                0x19,
                0x49,
                0x98,
                0x6f,
                0x10,
                0x19,
                0xc6,
                0xd7,
                0x31,
                0xeb,
                0x36,
                0x50,
                0xb2,
                0x98,
                0x48,
                0xff,
                0xfe,
                0xa5,
                0xb0,
                0x00,
            ];
            lzma::decompress(&mut x).unwrap()
        },
        b"Hello world\x0a"
    )
}
