use crate::io;
use alloc::vec;

pub fn read_tag<R: io::BufRead>(input: &mut R, tag: &[u8]) -> io::Result<bool> {
    let mut buf = vec![0; tag.len()];
    input.read_exact(buf.as_mut_slice())?;
    Ok(buf.as_slice() == tag)
}

pub fn is_eof<R: io::BufRead>(input: &mut R) -> io::Result<bool> {
    let buf = input.fill_buf()?;
    Ok(buf.is_empty())
}

pub fn flush_zero_padding<R: io::BufRead>(input: &mut R) -> io::Result<bool> {
    loop {
        let len = {
            let buf = input.fill_buf()?;
            let len = buf.len();

            if len == 0 {
                return Ok(true);
            }

            for x in buf {
                if *x != 0u8 {
                    return Ok(false);
                }
            }
            len
        };

        input.consume(len);
    }
}

/// An [`io::Read`] computing a digest on the bytes read.
pub struct CrcDigestRead<'a, 'b, R, S>
where
    R: 'a + io::Read,
    S: crc::Width,
{
    /// Underlying reader
    read: &'a mut R,
    /// Hasher
    digest: &'a mut crc::Digest<'b, S>,
}

impl<'a, 'b, R, S> CrcDigestRead<'a, 'b, R, S>
where
    R: io::Read,
    S: crc::Width,
{
    pub fn new(read: &'a mut R, digest: &'a mut crc::Digest<'b, S>) -> Self {
        Self { read, digest }
    }
}

impl<R> io::Read for CrcDigestRead<'_, '_, R, u32>
where
    R: io::Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self.read.read(buf)?;
        self.digest.update(&buf[..result]);
        Ok(result)
    }

    #[cfg(not(feature = "std"))]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.read.read_exact(buf)?;
        self.digest.update(buf);
        Ok(())
    }

    #[cfg(not(feature = "std"))]
    fn reader_position(&self) -> io::Result<usize> {
        self.read.reader_position()
    }

    #[cfg(not(feature = "std"))]
    fn advance_reader_position(&mut self, amt: usize) -> io::Result<()> {
        self.digest.update(&self.read.get_remaining()?[..amt]);
        self.read.advance_reader_position(amt)
    }

    #[cfg(not(feature = "std"))]
    fn get_remaining(&self) -> io::Result<&[u8]> {
        self.read.get_remaining()
    }
}

/// An [`io::BufRead`] counting the bytes read.
pub struct CountBufRead<'a, R>
where
    R: 'a + io::BufRead,
{
    /// Underlying reader
    read: &'a mut R,
    /// Number of bytes read
    count: usize,
}

impl<'a, R> CountBufRead<'a, R>
where
    R: io::BufRead,
{
    pub fn new(read: &'a mut R) -> Self {
        Self { read, count: 0 }
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

impl<R> io::Read for CountBufRead<'_, R>
where
    R: io::BufRead,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self.read.read(buf)?;
        self.count += result;
        Ok(result)
    }

    #[cfg(not(feature = "std"))]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.read.read_exact(buf)?;
        self.count += buf.len();
        Ok(())
    }

    #[cfg(not(feature = "std"))]
    fn reader_position(&self) -> io::Result<usize> {
        self.read.reader_position()
    }

    #[cfg(not(feature = "std"))]
    fn advance_reader_position(&mut self, amt: usize) -> io::Result<()> {
        self.read.advance_reader_position(amt)?;
        self.count += amt;
        Ok(())
    }

    #[cfg(not(feature = "std"))]
    fn get_remaining(&self) -> io::Result<&[u8]> {
        self.read.get_remaining()
    }
}

#[cfg(feature = "std")]
impl<R> std::io::BufRead for CountBufRead<'_, R>
where
    R: std::io::BufRead,
{
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.read.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.read.consume(amt);
        self.count += amt;
    }
}
