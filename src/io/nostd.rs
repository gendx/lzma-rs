///
/// Minimal implementations of [`std::io`] constructs used
///
/// Readers are implemented for all [`Cursor<AsRef<[u8]>>`]
/// As all data is already stored in buffers, BufRead is implemented for all
/// readers
///
/// Writers are implemented for all [`Cursor<AsRef<[u8]>>`] as well as
/// [`Cursor<Vec<u8>>`] and [`Vec<u8>`]
use alloc::vec::Vec;
use byteorder;
use core::cmp;
use core::mem::size_of;

#[derive(Debug, PartialEq, Eq)]
/// An IO error
pub enum Error {
    /// A write failed due to a lack of buffer space
    OutOfSpace,

    /// An unexpected end-of-input was reached
    EndOfInput,

    /// A cursor's position is invalid
    InvalidCursor,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// A `no_std` analogue of `std::io::Result``
pub type Result<T> = core::result::Result<T, Error>;

/// A tailored `no_std` Read trait that assumes an underlying data slice.
///
/// This trait defines the behavior of a positioned `no_std` reader that is able
/// to return all remaining data on demand. It is intended as a `no_std`
/// adaptatation and extension of `std::io::Read` that is tailored for reading
/// data already fully loaded into memory.
pub trait Read
where
    Self: Sized,
{
    /// Get the position of the reader, returning an error if the position is
    /// invalid.
    fn reader_position(&self) -> Result<usize>;

    /// Advance the reader position by a specified amount, returning an error if
    /// the position is out-of-bounds.
    fn advance_reader_position(&mut self, amt: usize) -> Result<()>;

    /// Get all data after the current reader position as a slice.
    fn get_remaining(&self) -> Result<&[u8]>;

    /// Attempt to fill a destination buffer with data from the reader,
    /// returning the number of bytes read. Intended to be an equivalent of
    /// `std::io::Read::read`.
    fn read(&mut self, dest: &mut [u8]) -> Result<usize>;

    /// Attempt to completely fill a destination buffer with data from the
    /// reader, returning an error if not enough data is available. Intended
    /// to be an equivalent of `std::io::Read::read_exact`.
    fn read_exact(&mut self, dest: &mut [u8]) -> Result<()>;

    /// Creates a `Take` adapter that limits the number of bytes read from this
    /// reader. Intended to be an equivalent of `std::io::Read::take`.
    fn take(&mut self, limit: u64) -> Take<'_, Self> {
        Take {
            inner: self,
            bytes_remaining: limit as usize,
        }
    }

    /// Returns an iterator over the bytes of this reader.
    /// Intended to be an equivalent of `std::io::Read::bytes`.
    fn bytes(self) -> ByteIterator<Self> {
        ByteIterator { reader: self }
    }
}

/// An iterator over the bytes of a `Read` type, intended to be an equivalent of
/// `std::io::Read::bytes`.
#[derive(Debug)]
pub struct ByteIterator<R: Read> {
    reader: R,
}

impl<R: Read> Iterator for ByteIterator<R> {
    type Item = Result<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        let buffer_result = self.reader.get_remaining();

        if let Ok(buffer) = buffer_result {
            if buffer.is_empty() {
                return None;
            }
            let byte: u8 = buffer[0];

            if let Err(e) = self.reader.advance_reader_position(1) {
                return Some(Err(e));
            }

            return Some(Ok(byte));
        }

        Some(Err(buffer_result.unwrap_err()))
    }
}

/// A `no_std` analogue of `std::io::BufRead` that is implemented
/// for all [`Read`] types.
pub trait BufRead: Read {
    /// Analogue of `std::io::BufRead::fill_buf`, returning the remaining data
    /// in the reader as if the remaining data in memory has been buffered.
    fn fill_buf(&self) -> Result<&[u8]> {
        self.get_remaining()
    }

    /// Analogue of `std::io::BufRead::consume`, advancing the reader position
    /// by a specified amount
    fn consume(&mut self, amount: usize) {
        self.advance_reader_position(amount).unwrap()
    }
}

/// All [`Read`] types automatically implement [`BufRead`].
impl<R: Read> BufRead for R {}

impl<T: AsRef<[u8]>> Read for Cursor<T> {
    fn reader_position(&self) -> Result<usize> {
        // Ensure the position is within bounds
        if self.pos > self.inner.as_ref().len() {
            Err(Error::InvalidCursor)
        } else {
            Ok(self.pos)
        }
    }

    fn advance_reader_position(&mut self, amt: usize) -> Result<()> {
        // Ensure the position would be within bounds after advancing
        if self.pos + amt > self.inner.as_ref().len() {
            Err(Error::InvalidCursor)
        } else {
            self.pos += amt;
            Ok(())
        }
    }

    #[inline]
    fn get_remaining(&self) -> Result<&[u8]> {
        Ok(&self.inner.as_ref()[self.reader_position()?..])
    }

    fn read(&mut self, dest: &mut [u8]) -> Result<usize> {
        let num_bytes = core::cmp::min(dest.len(), self.get_remaining()?.len());
        dest[..num_bytes].copy_from_slice(&self.inner.as_ref()[self.pos..(self.pos + num_bytes)]);
        self.advance_reader_position(num_bytes)?;
        Ok(num_bytes)
    }

    fn read_exact(&mut self, dest: &mut [u8]) -> Result<()> {
        if dest.len() > self.get_remaining()?.len() {
            Err(Error::EndOfInput)
        } else {
            dest.copy_from_slice(&self.get_remaining()?[..dest.len()]);
            self.advance_reader_position(dest.len())?;
            Ok(())
        }
    }
}

/// Implement `Read` for a mutable reference to a `Read` type
impl<T: Read> Read for &mut T {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (**self).read(buf)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        (**self).read_exact(buf)
    }

    fn reader_position(&self) -> Result<usize> {
        (**self).reader_position()
    }

    fn advance_reader_position(&mut self, amt: usize) -> Result<()> {
        (**self).advance_reader_position(amt)
    }

    fn get_remaining(&self) -> Result<&[u8]> {
        (**self).get_remaining()
    }
}

/// A `no_std` equivalent of `std::io::Write` that is intended for writing bytes
/// directly to memory without flushing.
pub trait Write {
    /// Write as much of the provided buffer as possible, returning the number
    /// of bytes written.
    /// Analogue of `std::io::Write::write`.
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    /// Attempt to write the entire provided buffer
    /// Analogue of `std::io::Write::write_all`.
    fn write_all(&mut self, buf: &[u8]) -> Result<()>;

    /// Analogue of `std::io::Write::flush`.
    fn flush(&mut self) -> Result<()> {
        // There is nothign to flush in a `no_std` context.
        Ok(())
    }
}

/// All `Cursor<&mut [u8]>` can be written to.
/// This implementation will write to the underlying non-resizable slice,
/// overwriting the data at the current cursor position.
impl Write for Cursor<&mut [u8]> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        // Return an error if the cursor is already out-of-bounds
        if self.pos > self.inner.len() {
            return Err(Error::InvalidCursor);
        }

        if buf.is_empty() {
            return Ok(0);
        }

        let num_bytes = cmp::min(buf.len(), self.inner.len() - self.pos);
        self.inner[self.pos..(self.pos + num_bytes)].copy_from_slice(&buf[..num_bytes]);
        self.pos += num_bytes;
        Ok(num_bytes)
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        if self.pos + buf.len() > self.inner.len() {
            return Err(Error::OutOfSpace);
        }

        self.inner[self.pos..(self.pos + buf.len())].copy_from_slice(buf);
        self.pos += buf.len();
        Ok(())
    }
}

// All `Vec<u8>` can be written to.
// This implementation will append to the end of the vector.
impl Write for Vec<u8> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.extend_from_slice(buf);
        Ok(())
    }
}

// All `Cursor<Vec<u8>>` can be written to.
// This implementation will write to the underlying resizable vector,
// overwriting the data at the current cursor position and extending the vector
// as necessary.
impl Write for Cursor<Vec<u8>> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        // Return an error if the cursor is already out-of-bounds
        if self.pos > self.inner.len() {
            return Err(Error::InvalidCursor);
        }

        // Calculate the number of bytes that the vector needs to be extended by
        let bytes_over = (self.pos + buf.len()).saturating_sub(self.inner.len());
        let (bytes_to_splice, bytes_to_extend) = buf.split_at(buf.len() - bytes_over);

        if !bytes_to_splice.is_empty() {
            self.inner.splice(
                self.pos..self.pos + bytes_to_splice.len(),
                bytes_to_splice.iter().cloned(),
            );
        }

        if !bytes_to_extend.is_empty() {
            self.inner.extend_from_slice(bytes_to_extend)
        }

        self.set_position(self.position() + buf.len() as u64);

        Ok(buf.len())
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        if self.write(buf)? < buf.len() {
            Err(Error::OutOfSpace)
        } else {
            Ok(())
        }
    }
}

impl<T: Write + ?Sized> Write for &mut T {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (*self).write(buf)
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        (*self).write_all(buf)
    }

    fn flush(&mut self) -> Result<()> {
        (*self).flush()
    }
}

/// A `no_std` analogue of `byteorder::ReadBytesExt` that allows for reading up
/// to 64-bit integers.
pub trait ReadBytes: Read {
    /// Read a single byte from the reader.
    fn read_u8(&mut self) -> Result<u8> {
        let mut buf: [u8; 1] = [0];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    /// Read a 16-bit unsigned integer from the reader with the specified byte
    /// order.
    fn read_u16<T: byteorder::ByteOrder>(&mut self) -> Result<u16> {
        let mut buf: [u8; size_of::<u16>()] = [0; size_of::<u16>()];
        self.read_exact(&mut buf)?;
        Ok(T::read_u16(&buf))
    }

    /// Read a 32-bit unsigned integer from the reader with the specified byte
    /// order.
    fn read_u32<T: byteorder::ByteOrder>(&mut self) -> Result<u32> {
        let mut buf: [u8; size_of::<u32>()] = [0; size_of::<u32>()];
        self.read_exact(&mut buf)?;
        Ok(T::read_u32(&buf))
    }

    /// Read a 64-bit unsigned integer from the reader with the specified byte
    /// order.
    fn read_u64<T: byteorder::ByteOrder>(&mut self) -> Result<u64> {
        let mut buf: [u8; size_of::<u64>()] = [0; size_of::<u64>()];
        self.read_exact(&mut buf)?;
        Ok(T::read_u64(&buf))
    }
}

/// A `no_std` analogue of `byteorder::WriteBytesExt` that allows for writing up
/// to 64-bit integers.
pub trait WriteBytes: Write {
    /// Write a single byte to the writer.
    fn write_u8(&mut self, n: u8) -> Result<()> {
        let mut buf: [u8; 1] = [0];
        buf[0] = n;
        self.write_all(&buf)
    }

    /// Write a 16-bit unsigned integer to the writer with the specified byte
    /// order.
    fn write_u16<T: byteorder::ByteOrder>(&mut self, n: u16) -> Result<()> {
        let mut buf: [u8; size_of::<u16>()] = [0; size_of::<u16>()];
        T::write_u16(&mut buf, n);
        self.write_all(&buf)
    }

    /// Write a 32-bit unsigned integer to the writer with the specified byte
    /// order.
    fn write_u32<T: byteorder::ByteOrder>(&mut self, n: u32) -> Result<()> {
        let mut buf: [u8; size_of::<u32>()] = [0; size_of::<u32>()];
        T::write_u32(&mut buf, n);
        self.write_all(&buf)
    }

    /// Write a 64-bit unsigned integer to the writer with the specified byte
    /// order.
    fn write_u64<T: byteorder::ByteOrder>(&mut self, n: u64) -> Result<()> {
        let mut buf: [u8; size_of::<u64>()] = [0; size_of::<u64>()];
        T::write_u64(&mut buf, n);
        self.write_all(&buf)
    }
}

impl<R: Read> ReadBytes for R {}
impl<W: Write> WriteBytes for W {}

/// A `no_std` analogue of `std::io::Cursor` that allows defines a position
/// within an inner type.
#[derive(Debug)]
pub struct Cursor<T> {
    inner: T,
    pos: usize,
}

impl<T> Cursor<T> {
    /// Create a new `Cursor` with the specified inner type.
    pub fn new(inner: T) -> Cursor<T> {
        Cursor { inner, pos: 0 }
    }

    /// Get the inner value of the cursor.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Get a reference to the inner value of the cursor.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the inner value of the cursor.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Get the position of the cursor.
    pub fn position(&self) -> u64 {
        self.pos as u64
    }

    /// Set the position of the cursor.
    pub fn set_position(&mut self, pos: u64) {
        self.pos = pos as usize
    }
}

/// A `no_std` analogue of `std::io::Take` that acts as an adapter for limiting
/// the number of bytes read from a `Read` type.
#[derive(Debug)]
pub struct Take<'a, R: Read> {
    inner: &'a mut R,
    bytes_remaining: usize,
}

impl<R: Read> Read for Take<'_, R> {
    fn reader_position(&self) -> Result<usize> {
        self.inner.reader_position()
    }

    fn advance_reader_position(&mut self, amt: usize) -> Result<()> {
        if amt > self.bytes_remaining {
            return Err(Error::InvalidCursor);
        }
        self.inner.advance_reader_position(amt)?;
        self.bytes_remaining -= amt;

        Ok(())
    }

    fn get_remaining(&self) -> Result<&[u8]> {
        Ok(&self.inner.get_remaining()?[..self.bytes_remaining])
    }

    fn read(&mut self, dest: &mut [u8]) -> Result<usize> {
        let bytes_to_request = cmp::min(dest.len(), self.bytes_remaining);
        let bytes_read = self.inner.read(&mut dest[..bytes_to_request])?;
        self.bytes_remaining -= bytes_read;
        Ok(bytes_read)
    }

    fn read_exact(&mut self, dest: &mut [u8]) -> Result<()> {
        if dest.len() > self.bytes_remaining {
            return Err(Error::EndOfInput);
        }
        self.inner.read_exact(dest)?;
        self.bytes_remaining -= dest.len();
        Ok(())
    }
}

/// Helper trait to convert various types into a `Read` type for `BufReader`.
pub trait IntoReader<R: Read> {
    /// Convert the type into a `Read` type.
    fn into_reader(self) -> R;
}

impl<R: Read> IntoReader<R> for R {
    fn into_reader(self) -> R {
        self
    }
}

impl<T: AsRef<[u8]>> IntoReader<Cursor<T>> for T {
    fn into_reader(self) -> Cursor<T> {
        Cursor::new(self)
    }
}

/// A dummy equivalent of `std::io::BufReader` type that is used to provide a
/// consistent interface for creating buffered readers. As all readers already
/// have an internal buffer, this type does not provide any additional
/// functionality.
#[derive(Debug)]
pub struct BufReader {}
impl BufReader {
    /// Convert the provided type into a `Read` type. Types which already
    /// implement `Read` will return themselves, and types that implement
    /// `AsRef<[u8]>` will return a `Cursor` over the data.
    #[allow(clippy::new_ret_no_self)]
    pub fn new<T: IntoReader<R>, R: Read>(reader: T) -> R {
        reader.into_reader()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn read_whole_cursor() {
        const READ_BUFFER: &[u8] = &[1, 2, 3, 4, 5, 6];
        let mut cursor = Cursor::new(READ_BUFFER);

        let dest_buffer: &mut [u8; READ_BUFFER.len()] = &mut [0; READ_BUFFER.len()];
        assert!(cursor
            .read(dest_buffer)
            .is_ok_and(|len| len == READ_BUFFER.len()));
        assert!(dest_buffer == READ_BUFFER);

        assert_eq!(cursor.reader_position().unwrap(), READ_BUFFER.len());

        // We shouldn't be able to read any more
        assert!(cursor.read(dest_buffer).is_ok_and(|len| len == 0));
    }

    #[test]
    fn read_partial_cursor() {
        const READ_BUFFER: &[u8] = &[1, 2, 3, 4, 5, 6];

        // The following test is only valid if READ_BUFFER is of even length
        assert!(READ_BUFFER.len() % 2 == 0);

        let mut cursor = Cursor::new(READ_BUFFER);

        let dest_buffer: &mut [u8; READ_BUFFER.len() / 2] = &mut [0; READ_BUFFER.len() / 2];

        // Read the first half of the buffer
        assert!(cursor
            .read(dest_buffer)
            .is_ok_and(|len| len == READ_BUFFER.len() / 2));
        assert!(dest_buffer[..] == READ_BUFFER[..(READ_BUFFER.len() / 2)]);
        assert_eq!(cursor.reader_position().unwrap(), READ_BUFFER.len() / 2);

        // Read the second half of the buffer
        assert!(cursor
            .read(dest_buffer)
            .is_ok_and(|len| len == READ_BUFFER.len() / 2));
        assert!(dest_buffer[..] == READ_BUFFER[(READ_BUFFER.len() / 2)..]);
        assert_eq!(cursor.reader_position().unwrap(), READ_BUFFER.len());
    }

    #[test]
    fn read_exact_cursor() {
        const READ_BUFFER: &[u8] = &[1, 2, 3, 4, 5, 6];
        let mut cursor = Cursor::new(READ_BUFFER);

        let dest_buffer: &mut [u8; READ_BUFFER.len()] = &mut [0; READ_BUFFER.len()];
        assert!(cursor.read_exact(dest_buffer).is_ok());
        assert!(dest_buffer == READ_BUFFER);

        assert_eq!(cursor.reader_position().unwrap(), READ_BUFFER.len());

        // We shouldn't be able to read any more
        assert_eq!(
            cursor.read_exact(dest_buffer).unwrap_err(),
            Error::EndOfInput
        );
    }

    #[test]
    fn read_exact_too_small() {
        const READ_BUFFER: &[u8] = &[1, 2, 3, 4, 5, 6];
        let mut cursor = Cursor::new(READ_BUFFER);

        let dest_buffer: &mut [u8; READ_BUFFER.len() * 2] = &mut [0; READ_BUFFER.len() * 2];
        assert_eq!(
            cursor.read_exact(dest_buffer).unwrap_err(),
            Error::EndOfInput
        );

        // We didn't read anything, so the cursor shouldn't have moved
        assert_eq!(cursor.reader_position().unwrap(), 0);
    }

    #[test]
    fn bufread_cursor() {
        const READ_BUFFER: &[u8] = &[1, 2, 3, 4, 5, 6];
        let mut cursor = Cursor::new(READ_BUFFER);

        assert_eq!(cursor.get_remaining().unwrap(), cursor.fill_buf().unwrap());
        assert_eq!(cursor.fill_buf().unwrap(), READ_BUFFER);

        cursor.consume(READ_BUFFER.len() / 2);

        assert_eq!(
            cursor.get_remaining().unwrap(),
            &READ_BUFFER[(READ_BUFFER.len() / 2)..]
        );
        assert_eq!(cursor.get_remaining().unwrap(), cursor.fill_buf().unwrap());
    }

    #[test]
    fn write_cursor() {
        const SOURCE_BUFFER: &[u8] = &[1, 2, 3, 4, 5, 6];

        let mut writer_buffer = [0 as u8; SOURCE_BUFFER.len()];
        let mut cursor = Cursor::new(writer_buffer.as_mut_slice());

        assert_eq!(cursor.write(SOURCE_BUFFER).unwrap(), SOURCE_BUFFER.len());
        assert_eq!(cursor.position(), SOURCE_BUFFER.len() as u64);
    }

    #[test]
    fn write_buffer_cursor_over() {
        const SOURCE_BUFFER: &[u8] = &[1, 2, 3, 4, 5, 6];

        let mut writer_buffer = [0 as u8; SOURCE_BUFFER.len() / 2];
        let mut cursor: Cursor<&mut [u8]> = Cursor::new(writer_buffer.as_mut_slice());

        assert_eq!(
            cursor.write(SOURCE_BUFFER).unwrap(),
            SOURCE_BUFFER.len() / 2
        );
        assert_eq!(
            SOURCE_BUFFER[..(SOURCE_BUFFER.len() / 2)],
            **cursor.get_ref()
        );

        assert_eq!(cursor.write(SOURCE_BUFFER).unwrap(), 0);
        assert_eq!(
            cursor.write_all(SOURCE_BUFFER).unwrap_err(),
            Error::OutOfSpace
        );
    }

    #[test]
    fn write_vec() {
        const SOURCE_BUFFER: &[u8] = &[1, 2, 3, 4, 5, 6];
        let mut vec: Vec<u8> = Vec::new();

        assert_eq!(vec.write(SOURCE_BUFFER).unwrap(), SOURCE_BUFFER.len());
        assert_eq!(SOURCE_BUFFER, vec.as_slice());
    }

    #[test]
    fn write_vec_cursor() {
        const SOURCE_BUFFER: &[u8] = &[1, 2, 3, 4, 5, 6];
        let mut cursor = Cursor::new(Vec::new());

        assert_eq!(
            cursor
                .write(&SOURCE_BUFFER[..(SOURCE_BUFFER.len() / 2)])
                .unwrap(),
            SOURCE_BUFFER.len() / 2
        );
        assert_eq!(cursor.position() as usize, SOURCE_BUFFER.len() / 2);
        assert_eq!(
            cursor.get_ref().as_slice(),
            &SOURCE_BUFFER[..(SOURCE_BUFFER.len() / 2)]
        );

        // Move cursor back, then write the source buffer again
        cursor.set_position(1);
        assert_eq!(cursor.write(SOURCE_BUFFER).unwrap(), SOURCE_BUFFER.len());
        assert_eq!(cursor.position() as usize, SOURCE_BUFFER.len() + 1);
        assert_eq!(cursor.get_ref().as_slice()[0], SOURCE_BUFFER[0]);
        assert_eq!(&cursor.get_ref().as_slice()[1..], SOURCE_BUFFER);
    }
}
