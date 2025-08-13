//! Error handling.

use crate::io;
use alloc::string::String;

/// Library errors.
#[derive(Debug)]
pub enum Error {
    /// I/O error.
    IoError(io::Error),
    /// Not enough bytes to complete header
    HeaderTooShort(io::Error),
    /// LZMA error.
    LzmaError(String),
    /// XZ error.
    XzError(String),
}

/// Library result alias.
pub type Result<T> = core::result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IoError(e)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::IoError(e) => write!(fmt, "io error: {}", e),
            Error::HeaderTooShort(e) => write!(fmt, "header too short: {}", e),
            Error::LzmaError(e) => write!(fmt, "lzma error: {}", e),
            Error::XzError(e) => write!(fmt, "xz error: {}", e),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::IoError(e) | Error::HeaderTooShort(e) => Some(e),
            Error::LzmaError(_) | Error::XzError(_) => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::Error;
    use crate::io;
    use alloc::string::ToString;

    #[test]
    fn test_display() {
        #[cfg(feature = "std")]
        assert_eq!(
            Error::IoError(io::Error::other("this is an error")).to_string(),
            "io error: this is an error"
        );

        #[cfg(not(feature = "std"))]
        assert_eq!(
            Error::IoError(io::Error::OutOfSpace).to_string(),
            "io error: OutOfSpace"
        );

        assert_eq!(
            Error::LzmaError("this is an error".to_string()).to_string(),
            "lzma error: this is an error"
        );
        assert_eq!(
            Error::XzError("this is an error".to_string()).to_string(),
            "xz error: this is an error"
        );
    }
}
