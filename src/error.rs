//! Error handling.

use std::io;
use std::result;

/// Library errors.
#[derive(Debug)]
pub enum Error {
    /// I/O error.
    IOError(io::Error),
    /// LZMA error.
    LZMAError(String),
    /// XZ error.
    XZError(String),
}

/// Library result alias.
pub type Result<T> = result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IOError(e)
    }
}
