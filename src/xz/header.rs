//! XZ header.

use crate::error;
use crc::{crc32, Hasher32};

/// File format magic header signature, see sect. 2.1.1.1.
pub(crate) const XZ_MAGIC: &[u8] = &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00];

/// Stream Header, see sect. 2.1.1.
///
/// This does not store the null byte in Stream Flags, which is currently unused.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StreamHeader {
    pub(crate) check_method: CheckMethod,
}

impl StreamHeader {
    /// Parse a Stream Header from a byte buffer.
    pub(crate) fn parse(input: &[u8; 12]) -> error::Result<Self> {
        use std::hash::Hasher;

        let magic = &input[0..6];
        let null_byte = input[6];
        let check_id = input[7];
        let crc32 = u32::from_le_bytes([input[8], input[9], input[10], input[11]]);

        if magic != XZ_MAGIC {
            return Err(error::Error::XZError(format!(
                "Invalid XZ magic: {:?}",
                magic
            )));
        }

        let digested = {
            let mut digest = crc32::Digest::new(crc32::IEEE);
            digest.write_u8(null_byte);
            digest.write_u8(check_id);
            digest.sum32()
        };

        if crc32 != digested {
            return Err(error::Error::XZError(format!(
                "Invalid header CRC32: expected 0x{:08x} but got 0x{:08x}",
                crc32, digested
            )));
        }

        if null_byte != 0x00 {
            return Err(error::Error::XZError(format!(
                "Invalid null byte in Stream Flags: {:x}",
                null_byte
            )));
        }

        let check_method = CheckMethod::try_from(check_id)?;
        let header = Self { check_method };

        lzma_info!("XZ check method: {:?}", check_method);
        Ok(header)
    }
}

/// Stream check type, see sect. 2.1.1.2.
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub(crate) enum CheckMethod {
    None = 0x00,
    CRC32 = 0x01,
    CRC64 = 0x04,
    SHA256 = 0x0A,
}

impl CheckMethod {
    /// Parse Check ID (second byte in Stream Flags).
    pub(crate) fn try_from(id: u8) -> error::Result<CheckMethod> {
        match id {
            0x00 => Ok(CheckMethod::None),
            0x01 => Ok(CheckMethod::CRC32),
            0x04 => Ok(CheckMethod::CRC64),
            0x0A => Ok(CheckMethod::SHA256),
            _ => Err(error::Error::XZError(format!(
                "Invalid check method {:x}, expected one of [0x00, 0x01, 0x04, 0x0A]",
                id
            ))),
        }
    }
}

impl Into<u8> for CheckMethod {
    fn into(self) -> u8 {
        self as u8
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_checkmethod_roundtrip() {
        let mut valid = 0;
        for input in 0..std::u8::MAX {
            if let Ok(check) = CheckMethod::try_from(input) {
                let output: u8 = check.into();
                assert_eq!(input, output);
                valid += 1;
            }
        }
        assert_eq!(valid, 4);
    }
}
