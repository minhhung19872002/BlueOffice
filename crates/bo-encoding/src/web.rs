//! Text encodings of web pages and e-mail parts: UTF-8, UTF-16, Windows-1252
//! (with its ISO-8859-1 / US-ASCII labels, as the WHATWG Encoding Standard maps
//! them) and Windows-1258 (Vietnamese).
//!
//! The encoding of an HTML file is found the way the HTML Living Standard
//! describes (§13.2.3 "Determining the character encoding"), reduced to what the
//! importer needs: a byte order mark, then a `charset` given by the transport (a
//! MIME part), then a `<meta charset>` / `<meta http-equiv="Content-Type">` in the
//! first bytes of the file, then UTF-8 when the bytes are valid UTF-8 and
//! Windows-1252 otherwise.

use crate::cp1252;
use unicode_normalization::UnicodeNormalization;

/// A supported text encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Charset {
    /// UTF-8.
    Utf8,
    /// UTF-16, little endian.
    Utf16Le,
    /// UTF-16, big endian.
    Utf16Be,
    /// Windows-1252 (also the labels `iso-8859-1`, `latin1`, `us-ascii`).
    Windows1252,
    /// Windows-1258 (Vietnamese; tone marks are combining characters).
    Windows1258,
}

impl Charset {
    /// The encoding of a label (`charset=` value), ASCII case-insensitive.
    pub fn from_label(label: &str) -> Option<Charset> {
        let l = label
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .to_ascii_lowercase();
        Some(match l.as_str() {
            "utf-8" | "utf8" | "unicode-1-1-utf-8" => Charset::Utf8,
            "utf-16" | "utf-16le" | "unicode" | "ucs-2" => Charset::Utf16Le,
            "utf-16be" => Charset::Utf16Be,
            "windows-1252" | "cp1252" | "x-cp1252" | "iso-8859-1" | "iso8859-1" | "latin1"
            | "l1" | "us-ascii" | "ascii" | "ansi_x3.4-1968" | "iso_8859-1" | "cp819"
            | "ibm819" => Charset::Windows1252,
            "windows-1258" | "cp1258" | "x-cp1258" => Charset::Windows1258,
            _ => return None,
        })
    }
}

/// Windows-1258 bytes that differ from Windows-1252 (Unicode.org `CP1258.TXT`).
fn cp1258(b: u8) -> char {
    match b {
        0x8A | 0x8E | 0x9A | 0x9E => char::from(b),
        0xC3 => '\u{0102}',
        0xCC => '\u{0300}',
        0xD0 => '\u{0110}',
        0xD2 => '\u{0309}',
        0xD5 => '\u{01A0}',
        0xDD => '\u{01AF}',
        0xDE => '\u{0303}',
        0xE3 => '\u{0103}',
        0xEC => '\u{0301}',
        0xF0 => '\u{0111}',
        0xF2 => '\u{0323}',
        0xF5 => '\u{01A1}',
        0xFD => '\u{01B0}',
        0xFE => '\u{20AB}',
        _ => cp1252::decode(b),
    }
}

fn utf16(bytes: &[u8], be: bool) -> String {
    let units = bytes.chunks_exact(2).map(|c| {
        if be {
            u16::from_be_bytes([c[0], c[1]])
        } else {
            u16::from_le_bytes([c[0], c[1]])
        }
    });
    char::decode_utf16(units)
        .map(|r| r.unwrap_or('\u{FFFD}'))
        .collect()
}

/// Decode `bytes` in `charset` (invalid sequences become U+FFFD).
pub fn decode(bytes: &[u8], charset: Charset) -> String {
    match charset {
        Charset::Utf8 => {
            String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes))
                .into_owned()
        }
        Charset::Utf16Le => utf16(bytes.strip_prefix(&[0xFF, 0xFE]).unwrap_or(bytes), false),
        Charset::Utf16Be => utf16(bytes.strip_prefix(&[0xFE, 0xFF]).unwrap_or(bytes), true),
        Charset::Windows1252 => bytes.iter().map(|&b| cp1252::decode(b)).collect(),
        // Base letter + combining tone mark → precomposed letter (NFC).
        Charset::Windows1258 => bytes.iter().map(|&b| cp1258(b)).nfc().collect(),
    }
}

/// The encoding declared by a byte order mark.
pub fn bom_charset(bytes: &[u8]) -> Option<Charset> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        Some(Charset::Utf8)
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        Some(Charset::Utf16Le)
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        Some(Charset::Utf16Be)
    } else {
        None
    }
}

/// The `charset` named by a `<meta>` element in the first 4 KiB of an HTML file
/// (`<meta charset="…">` or `<meta http-equiv="Content-Type" content="…; charset=…">`).
pub fn meta_charset(bytes: &[u8]) -> Option<Charset> {
    let head: String = bytes[..bytes.len().min(4096)]
        .iter()
        .map(|&b| char::from(b).to_ascii_lowercase())
        .collect();
    let mut rest = head.as_str();
    while let Some(at) = rest.find("<meta") {
        rest = &rest[at + 5..];
        let tag = &rest[..rest.find('>').unwrap_or(rest.len())];
        if let Some(k) = tag.find("charset") {
            let v = tag[k + 7..].trim_start();
            if let Some(v) = v.strip_prefix('=') {
                let v = v.trim_start().trim_start_matches(['"', '\'']);
                let end = v
                    .find(|c: char| {
                        c == '"' || c == '\'' || c == ';' || c == '/' || c.is_whitespace()
                    })
                    .unwrap_or(v.len());
                if let Some(c) = Charset::from_label(&v[..end]) {
                    // A meta element cannot be read as UTF-16 unless the file is
                    // not UTF-16: the label then means UTF-8 (HTML §13.2.3.2).
                    return Some(match c {
                        Charset::Utf16Le | Charset::Utf16Be => Charset::Utf8,
                        other => other,
                    });
                }
            }
        }
    }
    None
}

/// Decode an HTML (or other web text) file: byte order mark, then the transport
/// `label`, then a `<meta>` declaration, then UTF-8 when valid, else Windows-1252.
pub fn decode_html(bytes: &[u8], label: Option<&str>) -> String {
    let charset = bom_charset(bytes)
        .or_else(|| label.and_then(Charset::from_label))
        .or_else(|| meta_charset(bytes))
        .unwrap_or_else(|| {
            if std::str::from_utf8(bytes).is_ok() {
                Charset::Utf8
            } else {
                Charset::Windows1252
            }
        });
    decode(bytes, charset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_boms() {
        assert_eq!(Charset::from_label(" UTF-8 "), Some(Charset::Utf8));
        assert_eq!(
            Charset::from_label("\"ISO-8859-1\""),
            Some(Charset::Windows1252)
        );
        assert_eq!(
            Charset::from_label("windows-1258"),
            Some(Charset::Windows1258)
        );
        assert_eq!(Charset::from_label("shift_jis"), None);
        assert_eq!(decode_html(b"\xEF\xBB\xBFa", Some("windows-1252")), "a");
        let mut le = vec![0xFF, 0xFE];
        le.extend("Việt".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(decode_html(&le, None), "Việt");
        let mut be = vec![0xFE, 0xFF];
        be.extend("Việt".encode_utf16().flat_map(u16::to_be_bytes));
        assert_eq!(decode_html(&be, None), "Việt");
    }

    #[test]
    fn meta_declarations() {
        let a = b"<html><head><META CHARSET='windows-1258'></head><body>Vi\xEA\xF2t Nam \xD0\xF4\xCCng</body>";
        assert_eq!(meta_charset(a), Some(Charset::Windows1258));
        assert!(decode_html(a, None).contains("Việt Nam Đồng"));
        let b = b"<meta http-equiv=\"Content-Type\" content=\"text/html; charset=iso-8859-1\">caf\xE9 \x93x\x94";
        assert!(decode_html(b, None).ends_with("café \u{201C}x\u{201D}"));
        // The transport label wins over the meta element.
        assert!(decode_html(b"<meta charset=utf-8>caf\xE9", Some("latin1")).ends_with("café"));
        // No declaration: UTF-8 when valid, Windows-1252 otherwise.
        assert_eq!(decode_html("Tiếng Việt".as_bytes(), None), "Tiếng Việt");
        assert_eq!(decode_html(b"caf\xE9", None), "café");
        assert_eq!(meta_charset(b"<meta name=x content=y><p>charset"), None);
        assert_eq!(meta_charset(b"<meta charset=utf-16>"), Some(Charset::Utf8));
    }

    #[test]
    fn any_bytes_decode() {
        let all: Vec<u8> = (0..=255).collect();
        for c in [
            Charset::Utf8,
            Charset::Utf16Le,
            Charset::Utf16Be,
            Charset::Windows1252,
            Charset::Windows1258,
        ] {
            assert!(decode(&all[..255], c).len() > 100);
        }
    }
}
