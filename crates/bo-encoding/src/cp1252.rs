//! Windows-1252 (WHATWG / Unicode.org `CP1252.TXT` mapping).
//!
//! The five unassigned bytes (0x81, 0x8D, 0x8F, 0x90, 0x9D) map to the C1
//! control with the same value, as the Windows API does.

/// Code points of bytes 0x80..=0x9F.
const HIGH: [u16; 32] = [
    0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, // 80
    0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x008D, 0x017D, 0x008F, // 88
    0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, // 90
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x009D, 0x017E, 0x0178, // 98
];

/// Decode one byte.
pub fn decode(b: u8) -> char {
    if (0x80..0xA0).contains(&b) {
        char::from_u32(u32::from(HIGH[usize::from(b - 0x80)])).unwrap_or('\u{FFFD}')
    } else {
        char::from(b)
    }
}

/// Encode one character, `None` if it is not in Windows-1252.
pub fn encode(c: char) -> Option<u8> {
    let u = c as u32;
    if u < 0x80 || (0xA0..=0xFF).contains(&u) {
        return Some(u as u8);
    }
    HIGH.iter()
        .position(|&h| u32::from(h) == u)
        .map(|i| 0x80 + i as u8)
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip_all_bytes() {
        for b in 0..=255u8 {
            assert_eq!(super::encode(super::decode(b)), Some(b));
        }
        assert_eq!(super::encode('ạ'), None);
    }
}
