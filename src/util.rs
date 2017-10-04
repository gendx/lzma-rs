use std::io;

// Read
#[inline(always)]
pub fn read_u8<R: io::Read>(stream: &mut R) -> io::Result<u8> {
    let mut buf = [0; 1];
    stream.read_exact(&mut buf)?;
    Ok(buf[0])
}

#[inline(always)]
fn get_u32_le(buf: &[u8; 4]) -> u32 {
    (buf[0] as u32) | ((buf[1] as u32) << 8) | ((buf[2] as u32) << 16) | ((buf[3] as u32) << 24)
}

pub fn read_u32_le<R: io::Read>(stream: &mut R) -> io::Result<u32> {
    let mut buf = [0; 4];
    stream.read_exact(&mut buf)?;
    Ok(get_u32_le(&buf))
}

#[inline(always)]
fn get_u32_be(buf: &[u8; 4]) -> u32 {
    ((buf[0] as u32) << 24) | ((buf[1] as u32) << 16) | ((buf[2] as u32) << 8) | (buf[3] as u32)
}

pub fn read_u32_be<R: io::Read>(stream: &mut R) -> io::Result<u32> {
    let mut buf = [0; 4];
    stream.read_exact(&mut buf)?;
    Ok(get_u32_be(&buf))
}

#[inline(always)]
fn get_u64_be(buf: &[u8; 8]) -> u64 {
    ((buf[0] as u64) << 56) | ((buf[1] as u64) << 48) | ((buf[2] as u64) << 40) |
        ((buf[3] as u64) << 32) | ((buf[4] as u64) << 24) | ((buf[5] as u64) << 16) |
        ((buf[6] as u64) << 8) | (buf[7] as u64)
}

pub fn read_u64_be<R: io::Read>(stream: &mut R) -> io::Result<u64> {
    let mut buf = [0; 8];
    stream.read_exact(&mut buf)?;
    Ok(get_u64_be(&buf))
}

// Write
pub fn write_u8<W: io::Write>(stream: &mut W, val: u8) -> io::Result<()> {
    let buf = [val; 1];
    stream.write_all(&buf)
}

#[inline(always)]
fn set_u32_le(buf: &mut [u8; 4], val: u32) {
    buf[0] = val as u8;
    buf[1] = (val >> 8) as u8;
    buf[2] = (val >> 16) as u8;
    buf[3] = (val >> 24) as u8;
}

pub fn write_u32_le<W: io::Write>(stream: &mut W, val: u32) -> io::Result<()> {
    let mut buf = [0; 4];
    set_u32_le(&mut buf, val);
    stream.write_all(&buf)
}

#[inline(always)]
fn set_u32_be(buf: &mut [u8; 4], val: u32) {
    buf[0] = (val >> 24) as u8;
    buf[1] = (val >> 16) as u8;
    buf[2] = (val >> 8) as u8;
    buf[3] = val as u8;
}

pub fn write_u32_be<W: io::Write>(stream: &mut W, val: u32) -> io::Result<()> {
    let mut buf = [0; 4];
    set_u32_be(&mut buf, val);
    stream.write_all(&buf)
}

fn set_u64_be(buf: &mut [u8; 8], val: u64) {
    buf[0] = (val >> 56) as u8;
    buf[1] = (val >> 48) as u8;
    buf[2] = (val >> 40) as u8;
    buf[3] = (val >> 32) as u8;
    buf[4] = (val >> 24) as u8;
    buf[5] = (val >> 16) as u8;
    buf[6] = (val >> 8) as u8;
    buf[7] = val as u8;
}

pub fn write_u64_be<W: io::Write>(stream: &mut W, val: u64) -> io::Result<()> {
    let mut buf = [0; 8];
    set_u64_be(&mut buf, val);
    stream.write_all(&buf)
}
