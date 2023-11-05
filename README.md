# lzma-rs

[![Crate](https://img.shields.io/crates/v/lzma-rs.svg)](https://crates.io/crates/lzma-rs)
[![Documentation](https://docs.rs/lzma-rs/badge.svg)](https://docs.rs/lzma-rs)
[![Safety Dance](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
![Build Status](https://github.com/gendx/lzma-rs/workflows/Build%20and%20run%20tests/badge.svg)
[![Minimum Rust 1.65](https://img.shields.io/badge/rust-1.65%2B-orange.svg?logo=rust)](https://releases.rs/docs/1.65.0/)
[![Codecov](https://codecov.io/gh/gendx/lzma-rs/branch/master/graph/badge.svg?token=HVo74E0wzh)](https://codecov.io/gh/gendx/lzma-rs)
[![Lines of Code](https://tokei.rs/b1/github/gendx/lzma-rs?category=code)](https://github.com/gendx/lzma-rs)
[![Downloads (crates.io)](https://img.shields.io/crates/d/lzma-rs?label=downloads)](https://crates.io/crates/lzma-rs)

This project is a decoder for LZMA and its variants written in pure Rust, with focus on clarity.
It already supports LZMA, LZMA2 and a subset of the `.xz` file format.

## Usage

Decompress a `.xz` file.

```rust
let filename = "foo.xz";
let mut f = std::io::BufReader::new(std::fs::File::open(filename).unwrap());
// "decomp" can be anything that implements "std::io::Write"
let mut decomp: Vec<u8> = Vec::new();
lzma_rs::xz_decompress(&mut f, &mut decomp).unwrap();
// Decompressed content is now in "decomp"
```

## Encoder

For now, there is also a dumb encoder that only uses byte literals, with many hard-coded constants for code simplicity.
Better encoders are welcome!

## Contributing

Pull-requests are welcome, to improve the decoder, add better encoders, or more tests.
Ultimately, this project should also implement .xz and .7z files.

## License

MIT

