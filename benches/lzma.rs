#![feature(test)]

extern crate env_logger;
extern crate lzma_rs;
extern crate test;

use std::io::Read;
use test::Bencher;

fn compress_bench(x: &[u8], b: &mut Bencher) {
    b.iter(|| {
        let mut compressed: Vec<u8> = Vec::new();
        lzma_rs::lzma_compress(&mut std::io::BufReader::new(x), &mut compressed).unwrap();
    });
}

fn decompress_bench(x: &[u8], b: &mut Bencher) {
    let mut compressed: Vec<u8> = Vec::new();
    lzma_rs::lzma_compress(&mut std::io::BufReader::new(x), &mut compressed).unwrap();

    b.iter(|| {
        let mut bf = std::io::BufReader::new(compressed.as_slice());
        let mut decomp: Vec<u8> = Vec::new();
        lzma_rs::lzma_decompress(&mut bf, &mut decomp).unwrap();
    });
}

fn decompress_bench_file(compfile: &str, b: &mut Bencher) {
    let mut f = std::fs::File::open(compfile).unwrap();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();
    decompress_bench(&buf, b);
}

#[bench]
fn compress_empty(b: &mut Bencher) {
    let _ = env_logger::try_init();
    compress_bench(b"", b);
}

#[bench]
fn decompress_empty(b: &mut Bencher) {
    let _ = env_logger::try_init();
    decompress_bench(b"", b);
}

#[bench]
fn compress_hello(b: &mut Bencher) {
    let _ = env_logger::try_init();
    compress_bench(b"Hello world", b);
}

#[bench]
fn decompress_hello(b: &mut Bencher) {
    let _ = env_logger::try_init();
    decompress_bench(b"Hello world", b);
}

#[bench]
fn compress_65536(b: &mut Bencher) {
    let _ = env_logger::try_init();
    compress_bench(&[0; 0x10000], b);
}

#[bench]
fn decompress_65536(b: &mut Bencher) {
    let _ = env_logger::try_init();
    decompress_bench(&[0; 0x10000], b);
}

#[bench]
fn decompress_big_file(b: &mut Bencher) {
    let _ = env_logger::try_init();
    decompress_bench_file("tests/files/foo.txt.lzma", b);
}
