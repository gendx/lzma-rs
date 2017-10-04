use std::io;
use std::result;

#[derive(Debug)]
pub enum Error {
    IOError(io::Error),
    LZMAError(String),
}

pub type Result<T> = result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IOError(e)
    }
}
