//! Simple font encodings (ISO 32000-1, §9.6.6): base encodings, `/Differences`, glyph names
//! to Unicode (Adobe Glyph List and the `uniXXXX` / `uXXXX[XX]` conventions).
use crate::encoding_data::{AGL, MAC_EXPERT, MAC_ROMAN, STANDARD, SYMBOL, WIN_ANSI};

/// A base encoding by its PDF name.
pub(crate) fn base(name: &[u8]) -> Option<&'static [&'static str; 256]> {
    match name {
        b"WinAnsiEncoding" => Some(&WIN_ANSI),
        b"MacRomanEncoding" => Some(&MAC_ROMAN),
        b"StandardEncoding" => Some(&STANDARD),
        b"MacExpertEncoding" => Some(&MAC_EXPERT),
        b"SymbolEncoding" => Some(&SYMBOL),
        _ => None,
    }
}

/// The standard encoding (default of Type 1 fonts without their own).
pub(crate) fn standard() -> &'static [&'static str; 256] {
    &STANDARD
}

/// WinAnsiEncoding.
pub(crate) fn win_ansi() -> &'static [&'static str; 256] {
    &WIN_ANSI
}

/// Symbol font built-in encoding.
pub(crate) fn symbol() -> &'static [&'static str; 256] {
    &SYMBOL
}

/// MacRoman code of a glyph name (TrueType (1,0) cmaps).
pub(crate) fn mac_roman_code(name: &str) -> Option<u8> {
    MAC_ROMAN
        .iter()
        .position(|n| *n == name)
        .and_then(|i| u8::try_from(i).ok())
}

/// Unicode text of a glyph name.
pub(crate) fn unicode(name: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }
    if let Ok(i) = AGL.binary_search_by(|(n, _)| (*n).cmp(name)) {
        return char::from_u32(u32::from(AGL[i].1)).map(String::from);
    }
    // uniXXXX[XXXX…] (one or more BMP code units) and uXXXX[XX].
    let base = name.split('.').next().unwrap_or(name);
    if let Some(hex) = base.strip_prefix("uni") {
        if hex.len() >= 4 && hex.len() % 4 == 0 && hex.bytes().all(|c| c.is_ascii_hexdigit()) {
            let units: Vec<u16> = (0..hex.len() / 4)
                .filter_map(|k| u16::from_str_radix(&hex[k * 4..k * 4 + 4], 16).ok())
                .collect();
            return String::from_utf16(&units).ok();
        }
    }
    if let Some(hex) = base.strip_prefix('u') {
        if (4..=6).contains(&hex.len()) && hex.bytes().all(|c| c.is_ascii_hexdigit()) {
            return u32::from_str_radix(hex, 16)
                .ok()
                .and_then(char::from_u32)
                .map(String::from);
        }
    }
    if base != name {
        return unicode(base);
    }
    // "g123" / "cid123" style names and ligatures like "f_f_i".
    if name.contains('_') {
        let parts: Option<String> = name.split('_').map(unicode).collect();
        return parts;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_to_unicode() {
        assert_eq!(unicode("A").as_deref(), Some("A"));
        assert_eq!(unicode("eacute").as_deref(), Some("é"));
        assert_eq!(unicode("uni1EA0").as_deref(), Some("Ạ"));
        assert_eq!(unicode("u1F600").as_deref(), Some("😀"));
        assert_eq!(unicode("a_b").as_deref(), Some("ab"));
        assert_eq!(unicode("a.sc").as_deref(), Some("a"));
        assert_eq!(unicode("nosuchglyph"), None);
        assert_eq!(WIN_ANSI[0x41], "A");
        assert_eq!(base(b"WinAnsiEncoding").map(|e| e[0xe9]), Some("eacute"));
        assert_eq!(mac_roman_code("A"), Some(0x41));
    }
}
