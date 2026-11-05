//! Fonts and text shaping.
//!
//! * [`FontManager`] owns font binaries (bundled with the app, served by the
//!   server, or — on native targets — loaded from the operating system).
//! * Family resolution applies the metric-compatible fallback table from the
//!   product brief (Times New Roman → Liberation Serif/Tinos, Arial → Liberation
//!   Sans/Arimo, Calibri → Carlito, Cambria → Caladea) so documents keep their
//!   line breaks when the original font is missing.
//! * Shaping uses `rustybuzz`; results are cached per (face, text) in font units,
//!   so the cache is independent of font size and zoom.
#![forbid(unsafe_code)]

mod manager;
mod outline;

pub use manager::{FaceId, FaceInfo, FontManager, ShapedGlyph};
pub use outline::{outline_svg_path, PathCmd};
/// Re-export of the font parser used by this crate.
pub use ttf_parser as ttf;

/// Normalize text to Unicode NFC (Vietnamese text often arrives decomposed).
pub fn nfc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    if unicode_normalization::is_nfc_quick(s.chars()) == unicode_normalization::IsNormalized::Yes {
        s.to_owned()
    } else {
        s.nfc().collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn nfc_vietnamese() {
        // "ệ" decomposed: e + U+0323 (dot below) + U+0302 (circumflex)
        let decomposed = "Vi\u{0065}\u{0323}\u{0302}t";
        assert_eq!(super::nfc(decomposed), "Việt");
        assert_eq!(super::nfc("Việt").chars().count(), 4);
    }
}
