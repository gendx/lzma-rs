extern crate lzma;
extern crate env_logger;

fn decomp_big_file(compfile: &str, plainfile: &str) {
    use std::io::Read;

    let mut expected = Vec::new();
    std::fs::File::open(plainfile)
        .unwrap()
        .read_to_end(&mut expected)
        .unwrap();
    let mut f = std::io::BufReader::new(std::fs::File::open(compfile).unwrap());
    let mut decomp: Vec<u8> = Vec::new();
    lzma::xz_decompress(&mut f, &mut decomp).unwrap();
    assert!(decomp == expected)
}

#[test]
fn big_file() {
    let _ = env_logger::init();
    decomp_big_file("tests/files/foo.txt.xz", "tests/files/foo.txt");
}

#[test]
fn decompress_empty_world() {
    let _ = env_logger::init();
    let mut x: &[u8] = b"\xfd\x37\x7a\x58\x5a\x00\x00\x04\xe6\xd6\xb4\x46\x00\x00\x00\x00\
                         \x1c\xdf\x44\x21\x1f\xb6\xf3\x7d\x01\x00\x00\x00\x00\x04\x59\x5a\
                         ";
    let mut decomp: Vec<u8> = Vec::new();
    lzma::xz_decompress(&mut x, &mut decomp).unwrap();
    assert_eq!(decomp, b"")
}

#[test]
fn decompress_hello_world() {
    let _ = env_logger::init();
    let mut x: &[u8] = b"\xfd\x37\x7a\x58\x5a\x00\x00\x04\xe6\xd6\xb4\x46\x02\x00\x21\x01\
                         \x16\x00\x00\x00\x74\x2f\xe5\xa3\x01\x00\x0b\x48\x65\x6c\x6c\x6f\
                         \x20\x77\x6f\x72\x6c\x64\x0a\x00\xca\xec\x49\x05\x66\x3f\x67\x98\
                         \x00\x01\x24\x0c\xa6\x18\xd8\xd8\x1f\xb6\xf3\x7d\x01\x00\x00\x00\
                         \x00\x04\x59\x5a";
    let mut decomp: Vec<u8> = Vec::new();
    lzma::xz_decompress(&mut x, &mut decomp).unwrap();
    assert_eq!(decomp, b"Hello world\x0a")
}
