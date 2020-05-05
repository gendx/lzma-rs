## 0.1.3

- Minimum supported Rust version: 1.32.0.
- Update dependencies:
  - `log`: ^0.4.0 -> ^0.4.8
  - `env_logger`: 0.6.0 -> ^0.7.1
- Gate logging behind an opt-in feature. This improves decoding performance by
  ~25% (https://github.com/gendx/lzma-rs/pull/31).
- Lazily allocate the circular buffer (https://github.com/gendx/lzma-rs/pull/22).
  This improves memory usage (especially for WebAssembly targets) at the expense
  of a ~5%  performance regression (https://github.com/gendx/lzma-rs/issues/27).
- Return an error instead of panicking on unsupported SHA-256 checksum for XZ
  decoding (https://github.com/gendx/lzma-rs/pull/40).
- Add Clippy to CI.
- Document public APIs.
- Deny missing docs, missing Debug implementations and build warnings.
- Forbid unsafe code.
- Remove extern statements that are unnecessary on the 2018 edition.

## 0.1.2

- Fix bug in the range coder (https://github.com/gendx/lzma-rs/issues/15).
- Add support for specifying the unpacked size outside of the header
  (https://github.com/gendx/lzma-rs/pull/17).
- Migrate to Rust 2018 edition.
- Add benchmarks.
- Fix some Clippy warnings.

## 0.1.1

- Upgrade `env_logger` dependency.
- Refactoring to use `std::io::Take`, operator `?`.

## 0.1.0

- Initial release.
