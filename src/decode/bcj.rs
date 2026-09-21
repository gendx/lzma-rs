//! x86 BCJ (filter id 4) inverse.
//!
//! Public-domain algorithm from XZ/7-Zip: E8/E9 relative calls become absolute
//! on encode; decode subtracts the running offset. MIT extract, not a host xz wrap.

/// Inverse x86 BCJ. `start` is the uncompressed offset (filter properties, default 0).
pub fn x86_decode(buf: &mut [u8], start: usize) {
    if buf.len() < 5 {
        return;
    }
    let mut prev_pos: isize = -1;
    let mut prev_mask: u32 = 0;
    let mut i = 0usize;
    let end = buf.len() - 4;
    while i < end {
        let b = buf[i];
        if b != 0xE8 && b != 0xE9 {
            i += 1;
            continue;
        }
        prev_pos = i as isize - prev_pos;
        if (prev_pos & !3) != 0 {
            prev_mask = 0;
        } else {
            prev_mask = (prev_mask << (prev_pos as u32 - 1)) & 7;
            if prev_mask != 0 {
                let idx = MASK_TO_BIT_NUMBER[prev_mask as usize] as usize;
                if !MASK_TO_ALLOWED[prev_mask as usize] || test_msbyte(buf[i + 4 - idx]) {
                    prev_pos = i as isize;
                    prev_mask = (prev_mask << 1) | 1;
                    i += 1;
                    continue;
                }
            }
        }
        prev_pos = i as isize;
        if test_msbyte(buf[i + 4]) {
            let mut src = i32::from_le_bytes([buf[i + 1], buf[i + 2], buf[i + 3], buf[i + 4]]);
            loop {
                let dest = src.wrapping_sub((start.wrapping_add(i)) as i32);
                if prev_mask == 0 {
                    buf[i + 1..i + 5].copy_from_slice(&dest.to_le_bytes());
                    break;
                }
                let index = MASK_TO_BIT_NUMBER[prev_mask as usize] * 8;
                if !test_msbyte(((dest >> (24 - index)) & 0xFF) as u8) {
                    buf[i + 1..i + 5].copy_from_slice(&dest.to_le_bytes());
                    break;
                }
                src = dest ^ ((1 << (32 - index)) - 1);
            }
            prev_mask = (prev_mask << 1) | 1;
        } else {
            prev_mask = (prev_mask << 1) | 1;
        }
        i += 5;
        continue;
    }
}

const MASK_TO_ALLOWED: [bool; 8] = [true, true, true, false, true, false, false, false];
const MASK_TO_BIT_NUMBER: [u8; 8] = [0, 1, 2, 2, 3, 3, 3, 3];

fn test_msbyte(b: u8) -> bool {
    b == 0x00 || b == 0xFF
}

/// Forward x86 BCJ (tests only): relative → absolute.
#[cfg(test)]
pub fn x86_encode(buf: &mut [u8], start: usize) {
    if buf.len() < 5 {
        return;
    }
    let mut i = 0usize;
    let end = buf.len() - 4;
    while i < end {
        let b = buf[i];
        if b != 0xE8 && b != 0xE9 {
            i += 1;
            continue;
        }
        if test_msbyte(buf[i + 4]) {
            let src = i32::from_le_bytes([buf[i + 1], buf[i + 2], buf[i + 3], buf[i + 4]]);
            let dest = src.wrapping_add((start.wrapping_add(i)) as i32);
            buf[i + 1..i + 5].copy_from_slice(&dest.to_le_bytes());
            i += 5;
            continue;
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x86_bcj_roundtrip() {
        let mut buf = vec![0u8; 32];
        buf[4] = 0xE8;
        buf[5..9].copy_from_slice(&0x10i32.to_le_bytes());
        buf[9] = 0x90;
        let orig = buf.clone();
        x86_encode(&mut buf, 0);
        assert_ne!(buf, orig);
        x86_decode(&mut buf, 0);
        assert_eq!(buf, orig);
    }
}
