This directory contains fuzzing targets to verify implementation correctness: 

- `roundtrip_*` targets check that we can successfully decode what we've encoded.
- `decompress_*` targets check that we don't panic or abort on decoding a crafted file.
- `compare_*` targets check that we produce identical output to liblzma on decompression.

The command to run fuzzer is:

`cargo +nightly fuzz run --release -s none <fuzzing_target>`

For example,

`cargo +nightly fuzz run --release -s none compare_xz`

We use `-s none` because this crate does not contain unsafe code, so we don't
need sanitizers to detect memory or concurrency errors for us.

For more info see `cargo +nightly fuzz help`
