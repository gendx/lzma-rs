use std::hash;
use std::io;

// A Write computing a digest on the bytes written.
pub struct HasherWrite<'a, W, H>
where
    W: 'a + io::Write,
    H: 'a + hash::Hasher,
{
    write: &'a mut W,  // underlying writer
    hasher: &'a mut H, // hasher
}

impl<'a, W, H> HasherWrite<'a, W, H>
where
    W: io::Write,
    H: hash::Hasher,
{
    pub fn new(write: &'a mut W, hasher: &'a mut H) -> Self {
        Self { write, hasher }
    }
}

impl<'a, W, H> io::Write for HasherWrite<'a, W, H>
where
    W: io::Write,
    H: hash::Hasher,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let result = self.write.write(buf)?;
        self.hasher.write(&buf[..result]);
        Ok(result)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.write.flush()
    }
}

// A Write counting the bytes written.
pub struct CountWrite<'a, W>
where
    W: 'a + io::Write,
{
    write: &'a mut W, // underlying writer
    count: usize,     // number of bytes written
}

impl<'a, W> CountWrite<'a, W>
where
    W: io::Write,
{
    pub fn new(write: &'a mut W) -> Self {
        Self { write, count: 0 }
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

impl<'a, W> io::Write for CountWrite<'a, W>
where
    W: io::Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let result = self.write.write(buf)?;
        self.count += result;
        Ok(result)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.write.flush()
    }
}
