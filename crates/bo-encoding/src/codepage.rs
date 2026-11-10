//! Text of legacy data files by Windows / DOS code page number: dBASE tables say
//! their code page in the header, DOS-era text files use the OEM code page of the
//! machine that wrote them.
//!
//! Supported: the single-byte code pages 437, 737, 850, 852, 857, 860, 861, 863,
//! 865, 866, 874 and 1250–1258 (not the double-byte East Asian pages 932 / 936 /
//! 949 / 950), and 65001 (UTF-8).

use crate::web::{decode as decode_web, Charset};
use crate::{cp1252, cp_tables};

/// Whether [`decode`] knows code page `cp`.
pub fn is_supported(cp: u16) -> bool {
    matches!(cp, 1252 | 1258 | 65001) || cp_tables::upper_half(cp).is_some()
}

/// Decode `bytes` of code page `cp`. `None` when the code page is not supported
/// (the caller picks a fallback); undefined bytes become U+FFFD.
pub fn decode(bytes: &[u8], cp: u16) -> Option<String> {
    match cp {
        1252 => Some(bytes.iter().map(|&b| cp1252::decode(b)).collect()),
        1258 => Some(decode_web(bytes, Charset::Windows1258)),
        65001 => Some(String::from_utf8_lossy(bytes).into_owned()),
        _ => {
            let upper: Vec<char> = cp_tables::upper_half(cp)?.chars().collect();
            Some(
                bytes
                    .iter()
                    .map(|&b| {
                        if b < 0x80 {
                            char::from(b)
                        } else {
                            upper
                                .get(usize::from(b - 0x80))
                                .copied()
                                .unwrap_or('\u{FFFD}')
                        }
                    })
                    .collect(),
            )
        }
    }
}

/// Text of a file with no declared encoding: UTF-8 (with or without a byte order
/// mark) when the bytes are valid UTF-8, UTF-16 with a byte order mark, and
/// Windows-1252 otherwise.
pub fn decode_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_web(bytes, Charset::Utf16Le);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_web(bytes, Charset::Utf16Be);
    }
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(body) {
        Ok(s) => s.to_owned(),
        Err(_) => body.iter().map(|&b| cp1252::decode(b)).collect(),
    }
}

/// Bytes of `text` for a file with no declared encoding: Windows-1252 when every
/// character has a byte there (what legacy readers expect), UTF-8 otherwise.
/// [`decode_text`] reads both back, except Windows-1252 text that happens to be
/// valid UTF-8 — only possible with at least two adjacent non-ASCII characters
/// such as `Ã©`.
pub fn encode_text(text: &str) -> Vec<u8> {
    if text.is_ascii() {
        return text.as_bytes().to_vec();
    }
    let ansi: Option<Vec<u8>> = text.chars().map(cp1252::encode).collect();
    match ansi {
        // Keep the round trip exact: use the single-byte form only when it cannot
        // be mistaken for UTF-8.
        Some(bytes) if std::str::from_utf8(&bytes).is_err() => bytes,
        _ => text.as_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_and_windows_code_pages() {
        // Unicode.org CP437.TXT: 0x82 é, 0x9B ¢, 0xB3 │, 0xE1 ß, 0xFD ².
        assert_eq!(
            decode(&[0x82, 0x9B, 0xB3, 0xE1, 0xFD], 437).as_deref(),
            Some("é¢│ß²")
        );
        // CP850.TXT: 0x9B ø, 0x9D Ø, 0xB5 Á, 0xD5 ı, 0xF3 ¾.
        assert_eq!(
            decode(&[0x9B, 0x9D, 0xB5, 0xD5, 0xF3], 850).as_deref(),
            Some("øØÁı¾")
        );
        // CP866.TXT: 0x80 А, 0xA0 а, 0xE0 р, 0xF1 ё.
        assert_eq!(
            decode(&[0x80, 0xA0, 0xE0, 0xF1], 866).as_deref(),
            Some("Аарё")
        );
        // CP852.TXT: 0x88 ł, 0x9F č, 0xD0 đ.
        assert_eq!(decode(&[0x88, 0x9F, 0xD0], 852).as_deref(), Some("łčđ"));
        // CP1250.TXT: 0x8A Š, 0xE8 č; CP1251.TXT: 0xC0 А, 0xFF я.
        assert_eq!(decode(&[0x8A, 0xE8], 1250).as_deref(), Some("Šč"));
        assert_eq!(decode(&[0xC0, 0xFF], 1251).as_deref(), Some("Ая"));
        // CP874.TXT: 0xA1 ก; 0xDB is undefined.
        assert_eq!(decode(&[0xA1, 0xDB], 874).as_deref(), Some("ก\u{FFFD}"));
        assert_eq!(decode(b"abc\x80", 1252).as_deref(), Some("abc€"));
        // Windows-1258: base letter + combining tone mark → precomposed (NFC).
        assert_eq!(decode(&[0x61, 0xEC], 1258).as_deref(), Some("á"));
        assert_eq!(decode("Việt".as_bytes(), 65001).as_deref(), Some("Việt"));
        assert_eq!(decode(b"x", 932), None);
        assert!(is_supported(437) && is_supported(1252) && !is_supported(950));
        // ASCII is the same in every code page.
        for cp in [437, 737, 850, 852, 857, 860, 861, 863, 865, 866, 874, 1257] {
            assert_eq!(decode(b"A~ 0", cp).as_deref(), Some("A~ 0"), "cp{cp}");
            assert_eq!(decode(&[0xFF; 3], cp).map(|s| s.chars().count()), Some(3));
        }
    }

    #[test]
    fn undeclared_text_round_trips() {
        for s in ["plain", "café 10 €", "Việt Nam", "Ã©", "é", "日本"] {
            assert_eq!(decode_text(&encode_text(s)), s, "{s}");
        }
        // Single-byte where legacy readers expect it.
        assert_eq!(encode_text("café"), [b'c', b'a', b'f', 0xE9]);
        assert_eq!(decode_text(&[0xEF, 0xBB, 0xBF, b'a']), "a");
        assert_eq!(decode_text(&[0xFF, 0xFE, b'a', 0]), "a");
        assert_eq!(decode_text(&[0xFE, 0xFF, 0, b'a']), "a");
    }
}
