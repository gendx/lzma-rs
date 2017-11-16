use std::io;

// A subset of a BufRead, limited to the first len bytes.
pub struct SubBufRead<'a, R>
where
    R: 'a + io::BufRead,
{
    read: &'a mut R, // underlying reader
    len: usize, // number of bytes left to read
}

impl<'a, R> SubBufRead<'a, R>
where
    R: io::BufRead,
{
    pub fn new(read: &'a mut R, len: usize) -> Self {
        Self {
            read: read,
            len: len,
        }
    }
}

impl<'a, R> io::Read for SubBufRead<'a, R>
where
    R: io::BufRead,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let sub_buf = if buf.len() <= self.len {
            buf
        } else {
            &mut buf[..self.len]
        };

        let result = self.read.read(sub_buf)?;
        self.len -= result;
        Ok(result)
    }
}

impl<'a, R> io::BufRead for SubBufRead<'a, R>
where
    R: io::BufRead,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let buf = self.read.fill_buf()?;
        if buf.len() <= self.len {
            Ok(buf)
        } else {
            Ok(&buf[..self.len])
        }
    }

    fn consume(&mut self, amt: usize) {
        let len = if amt <= self.len { amt } else { self.len };
        self.read.consume(len);
        self.len -= len;
    }
}
