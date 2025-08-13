//! IO handling.
//!
//! With the `std` feature enabled, the IO module re-exports relevant IO
//! constructs from `std::io`. Under `no_std` conditions, the IO module
//! exports a minimal implementation of IO for reading from
//! [`Cursor<AsRef<[u8]>>`], and writing to [`Cursor<&[u8]>`],
//! [`Cursor<Vec<u8>>`], and `[Vec<u8>`].

#[cfg(feature = "std")]
#[doc(no_inline)]
pub use byteorder::{ReadBytesExt as ReadBytes, WriteBytesExt as WriteBytes};
#[cfg(feature = "std")]
#[doc(no_inline)]
pub use std::io::{BufRead, BufReader, Cursor, Error, Read, Result, Write};

#[cfg(not(feature = "std"))]
mod nostd;
#[cfg(not(feature = "std"))]
pub use nostd::*;
