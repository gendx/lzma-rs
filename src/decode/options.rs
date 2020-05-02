/// Options to tweak decompression behavior.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// Defines whether the unpacked size should be read from the header or provided.
    ///
    /// The default is
    /// [`UnpackedSize::ReadFromHeader`](enum.UnpackedSize.html#variant.ReadFromHeader).
    pub unpacked_size: UnpackedSize,
}

/// Alternatives for defining the unpacked size of the decoded data.
#[derive(Clone, Copy, Debug)]
pub enum UnpackedSize {
    /// Assume that the 8 bytes used to specify the unpacked size are present in the header.
    /// If the bytes are `0xFFFF_FFFF_FFFF_FFFF`, assume that there is an end-of-payload marker in
    /// the file.
    /// If not, read the 8 bytes as a little-endian encoded u64.
    ReadFromHeader,
    /// Assume that there are 8 bytes representing the unpacked size present in the header.
    /// Read it, but ignore it and use the provided value instead.
    /// If the provided value is `None`, assume that there is an end-of-payload marker in the file.
    /// Note that this is a non-standard way of reading LZMA data,
    /// but is used by certain libraries such as
    /// [OpenCTM](http://openctm.sourceforge.net/).
    ReadHeaderButUseProvided(Option<u64>),
    /// Assume that the 8 bytes typically used to represent the unpacked size are *not* present in
    /// the header. Use the provided value.
    /// If the provided value is `None`, assume that there is an end-of-payload marker in the file.
    UseProvided(Option<u64>),
}

impl Default for UnpackedSize {
    fn default() -> UnpackedSize {
        UnpackedSize::ReadFromHeader
    }
}
