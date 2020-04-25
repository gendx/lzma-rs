//! Logic for handling `.xz` file format.
//!
//! Format specifications are at [https://tukaani.org/xz/xz-file-format.txt](spec).
//!
//! [spec]: https://tukaani.org/xz/xz-file-format.txt

#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

pub(crate) mod footer;
pub(crate) mod header;
