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

/// Unicode of the ZapfDingbats font's built-in encoding (ISO 32000-1 Annex D.6 codes, the
/// Unicode values of Adobe's ITC Zapf Dingbats glyph list; 0x80–0x8D as the Unicode
/// parenthesised ornaments U+2768–2775); 0 for no glyph.
const ZAPF_DINGBATS: [u16; 256] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0020, 0x2701, 0x2702, 0x2703,
    0x2704, 0x260E, 0x2706, 0x2707, 0x2708, 0x2709, 0x261B, 0x261E, 0x270C, 0x270D, 0x270E, 0x270F,
    0x2710, 0x2711, 0x2712, 0x2713, 0x2714, 0x2715, 0x2716, 0x2717, 0x2718, 0x2719, 0x271A, 0x271B,
    0x271C, 0x271D, 0x271E, 0x271F, 0x2720, 0x2721, 0x2722, 0x2723, 0x2724, 0x2725, 0x2726, 0x2727,
    0x2605, 0x2729, 0x272A, 0x272B, 0x272C, 0x272D, 0x272E, 0x272F, 0x2730, 0x2731, 0x2732, 0x2733,
    0x2734, 0x2735, 0x2736, 0x2737, 0x2738, 0x2739, 0x273A, 0x273B, 0x273C, 0x273D, 0x273E, 0x273F,
    0x2740, 0x2741, 0x2742, 0x2743, 0x2744, 0x2745, 0x2746, 0x2747, 0x2748, 0x2749, 0x274A, 0x274B,
    0x25CF, 0x274D, 0x25A0, 0x274F, 0x2750, 0x2751, 0x2752, 0x25B2, 0x25BC, 0x25C6, 0x2756, 0x25D7,
    0x2758, 0x2759, 0x275A, 0x275B, 0x275C, 0x275D, 0x275E, 0x0000, 0x2768, 0x2769, 0x276A, 0x276B,
    0x276C, 0x276D, 0x276E, 0x276F, 0x2770, 0x2771, 0x2772, 0x2773, 0x2774, 0x2775, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x2761, 0x2762, 0x2763, 0x2764, 0x2765, 0x2766, 0x2767,
    0x2663, 0x2666, 0x2665, 0x2660, 0x2460, 0x2461, 0x2462, 0x2463, 0x2464, 0x2465, 0x2466, 0x2467,
    0x2468, 0x2469, 0x2776, 0x2777, 0x2778, 0x2779, 0x277A, 0x277B, 0x277C, 0x277D, 0x277E, 0x277F,
    0x2780, 0x2781, 0x2782, 0x2783, 0x2784, 0x2785, 0x2786, 0x2787, 0x2788, 0x2789, 0x278A, 0x278B,
    0x278C, 0x278D, 0x278E, 0x278F, 0x2790, 0x2791, 0x2792, 0x2793, 0x2794, 0x2192, 0x2194, 0x2195,
    0x2798, 0x2799, 0x279A, 0x279B, 0x279C, 0x279D, 0x279E, 0x279F, 0x27A0, 0x27A1, 0x27A2, 0x27A3,
    0x27A4, 0x27A5, 0x27A6, 0x27A7, 0x27A8, 0x27A9, 0x27AA, 0x27AB, 0x27AC, 0x27AD, 0x27AE, 0x27AF,
    0x00F0, 0x27B1, 0x27B2, 0x27B3, 0x27B4, 0x27B5, 0x27B6, 0x27B7, 0x27B8, 0x27B9, 0x27BA, 0x27BB,
    0x27BC, 0x27BD, 0x27BE, 0x0000,
];

/// ZapfDingbats built-in encoding as `uniXXXX` glyph names (`None`: no glyph).
pub(crate) fn zapf_dingbats(code: usize) -> Option<String> {
    ZAPF_DINGBATS
        .get(code)
        .filter(|&&u| u != 0)
        .map(|u| format!("uni{u:04X}"))
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
