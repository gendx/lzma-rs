use std::io;
use error;

// A circular buffer for LZ sequences
pub struct LZBuffer<'a, W>
where
    W: 'a + io::Write,
{
    stream: &'a mut W, // Output sink
    buf: Vec<u8>, // Circular buffer
    dict_size: usize, // Length of the buffer
    cursor: usize, // Current position
    len: usize, // Total number of bytes sent through the buffer
}

impl<'a, W> LZBuffer<'a, W>
where
    W: io::Write,
{
    pub fn from_stream(stream: &'a mut W, dict_size: usize) -> Self {
        Self {
            stream: stream,
            buf: vec![0; dict_size],
            dict_size: dict_size,
            cursor: 0,
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    // Retrieve the last byte or return a default
    pub fn last_or(&self, lit: u8) -> u8 {
        if self.len == 0 {
            lit
        } else {
            self.buf[(self.cursor + self.dict_size - 1) % self.dict_size]
        }
    }

    // Retrieve the n-th last byte
    pub fn last_n(&self, dist: usize) -> error::Result<u8> {
        if dist > self.dict_size {
            return Err(error::Error::LZMAError(format!(
                "Match distance {} is beyond dictionary size {}",
                dist,
                self.dict_size
            )));
        }
        if dist > self.len {
            return Err(error::Error::LZMAError(format!(
                "Match distance {} is beyond output size {}",
                dist,
                self.len
            )));
        }

        let offset = (self.cursor + self.dict_size - dist) % self.dict_size;
        Ok(self.buf[offset])
    }

    // Append a literal
    pub fn append_literal(&mut self, lit: u8) -> io::Result<()> {
        self.buf[self.cursor] = lit;
        self.cursor += 1;
        self.len += 1;

        // Flush the circular buffer to the output
        if self.cursor == self.dict_size {
            self.stream.write_all(self.buf.as_slice())?;
            self.cursor = 0;
        }

        Ok(())
    }

    // Fetch an LZ sequence (length, distance) from inside the buffer
    pub fn append_lz(&mut self, len: usize, dist: usize) -> error::Result<()> {
        debug!("LZ {{ len: {}, dist: {} }}", len, dist);
        if dist > self.dict_size {
            return Err(error::Error::LZMAError(format!(
                "LZ distance {} is beyond dictionary size {}",
                dist,
                self.dict_size
            )));
        }
        if dist > self.len {
            return Err(error::Error::LZMAError(format!(
                "LZ distance {} is beyond output size {}",
                dist,
                self.len
            )));
        }

        let mut offset = (self.cursor + self.dict_size - dist) % self.dict_size;
        for _ in 0..len {
            let x = self.buf[offset];
            self.append_literal(x)?;
            offset += 1;
            if offset == self.dict_size {
                offset = 0
            }
        }
        Ok(())
    }

    // Flush the buffer to the output
    pub fn finish(&mut self) -> io::Result<()> {
        if self.cursor > 0 {
            self.stream.write_all(&self.buf[0..self.cursor])?;
            self.stream.flush()?;
        }
        Ok(())
    }
}
