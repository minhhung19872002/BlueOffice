//! Legacy Vietnamese 8-bit encodings → Unicode (NFC).
//!
//! Before Unicode became the norm (TCVN 6909:2001), Vietnamese documents were
//! typed with special fonts that re-purpose the upper half of an 8-bit code page:
//!
//! * **TCVN3 / ABC** — TCVN 5712:1993 "VN3" table, fonts `.VnTime`, `.VnArial`, …
//!   One byte per Vietnamese letter. Lower-case only; the matching upper-case
//!   glyphs live at the *same* code points in the `…H` fonts (`.VnTimeH`).
//! * **VNI-Windows** — fonts `VNI-Times`, `VNI-Helve`, … A base letter followed by
//!   a combining "mark" byte (e.g. `a` + `ù` = `á`), plus a few single-byte letters
//!   (`ñ` = `đ`, `ô` = `ơ`, `ö` = `ư`, `í` = `í`, `ò` = `ị`, …).
//!
//! When such a document is opened, the bytes reach us already decoded as
//! Windows-1252 (that is what the font's code page claims), e.g. TCVN3
//! `"ViÖt Nam"` or VNI `"Vieät Nam"`. The functions here take that mis-decoded
//! text and return proper precomposed Unicode, which is already in NFC because
//! every Vietnamese letter has a precomposed code point.
//!
//! Tables are transcribed from the public TCVN 5712:1993 and VNI-Windows code
//! charts; no font or proprietary converter was consulted.
#![forbid(unsafe_code)]

pub mod cp1252;
mod tables;

use bo_model::{Document, RunContent};

pub use tables::{VOWELS, VOWELS_UPPER};

/// A legacy Vietnamese encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Legacy {
    /// TCVN 5712:1993 VN3 ("ABC"), fonts `.Vn*`.
    Tcvn3,
    /// VNI-Windows, fonts `VNI-*`.
    Vni,
}

/// Font that converted runs are switched to.
pub const REPLACEMENT_FONT: &str = "Times New Roman";

/// Recover the original 8-bit value of a character that was decoded with
/// Windows-1252. Symbol-font private-use characters `U+F000..=U+F0FF` (how Word
/// stores some legacy-font text) map to their low byte as well.
pub fn char_to_byte(c: char) -> Option<u8> {
    let u = c as u32;
    if (0xF000..=0xF0FF).contains(&u) {
        return Some((u & 0xFF) as u8);
    }
    cp1252::encode(c)
}

/// Decode raw TCVN3 bytes. ASCII passes through; bytes outside the table are
/// decoded as Windows-1252.
pub fn tcvn3_bytes_to_unicode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| tables::tcvn3_decode(b).unwrap_or_else(|| cp1252::decode(b)))
        .collect()
}

/// Convert TCVN3 text that was decoded as Windows-1252 to Unicode.
///
/// `upper` selects the upper-case companion fonts (`.VnTimeH`, `.VnArialH`):
/// those fonts draw capitals at the lower-case code points, so the result is
/// upper-cased.
pub fn tcvn3_to_unicode(s: &str, upper: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let mapped = char_to_byte(c).and_then(tables::tcvn3_decode);
        let ch = mapped.unwrap_or(c);
        if upper {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Encode Unicode text to TCVN3 bytes (used by tests and by "save as legacy"
/// tooling). Characters that TCVN3 cannot represent become `?`. Upper-case
/// toned vowels are not encodable in TCVN3 (they need the `…H` fonts) and are
/// lower-cased.
pub fn unicode_to_tcvn3(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| {
            tables::tcvn3_encode(c)
                .or_else(|| {
                    let lower = c.to_lowercase().next().unwrap_or(c);
                    tables::tcvn3_encode(lower)
                })
                .or_else(|| cp1252::encode(c))
                .unwrap_or(b'?')
        })
        .collect()
}

/// Decode raw VNI-Windows bytes.
pub fn vni_bytes_to_unicode(bytes: &[u8]) -> String {
    let s: String = bytes.iter().map(|&b| cp1252::decode(b)).collect();
    vni_to_unicode(&s)
}

/// Convert VNI-Windows text that was decoded as Windows-1252 to Unicode.
pub fn vni_to_unicode(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some(&next) = chars.get(i + 1) {
            if let Some(v) = tables::vni_pair(c, next) {
                out.push(v);
                i += 2;
                continue;
            }
        }
        out.push(tables::vni_single(c).unwrap_or(c));
        i += 1;
    }
    out
}

/// Encode Unicode text as VNI-Windows (returned as the Windows-1252 decoded
/// string, i.e. what a legacy document would contain).
pub fn unicode_to_vni(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match tables::vni_encode(c) {
            Some((a, Some(b))) => {
                out.push(a);
                out.push(b);
            }
            Some((a, None)) => out.push(a),
            None => out.push(c),
        }
    }
    out
}

/// Convert `s` from the given legacy encoding (`upper` only matters for TCVN3).
pub fn convert(s: &str, legacy: Legacy, upper: bool) -> String {
    match legacy {
        Legacy::Tcvn3 => tcvn3_to_unicode(s, upper),
        Legacy::Vni => vni_to_unicode(s),
    }
}

/// Classify a font name: `.Vn*` → TCVN3, `VNI-*` → VNI. Returns the encoding and
/// whether it is a TCVN3 upper-case (`…H`) font.
pub fn legacy_from_font(name: &str) -> Option<(Legacy, bool)> {
    let n = name.trim();
    let lower = n.to_ascii_lowercase();
    if lower.starts_with(".vn") {
        // `.VnTimeH`, `.VnArialH`, `.VnAvantH` … — the trailing `H` marks capitals.
        let upper = n.len() > 4 && n.ends_with('H');
        Some((Legacy::Tcvn3, upper))
    } else if lower.starts_with("vni-") || lower.starts_with("vni ") {
        Some((Legacy::Vni, false))
    } else {
        None
    }
}

/// Heuristically detect text that is TCVN3 or VNI decoded as Windows-1252.
///
/// Returns `None` for plain ASCII, for text that already contains Unicode
/// Vietnamese (or any character outside Windows-1252), and when the evidence is
/// too weak. The heuristic counts:
/// * TCVN3 "strong" bytes `0xA1..=0xBF` (punctuation-like symbols such as `µ ¸ ¹`
///   that never occur inside words in Western text), and upper-case Latin-1
///   letters squeezed between lower-case letters (`ViÖt`);
/// * VNI base-vowel + mark pairs (`oä`, `aø`) and orphan marks.
pub fn detect_legacy_vietnamese(s: &str) -> Option<Legacy> {
    let chars: Vec<char> = s.chars().collect();
    let mut non_ascii = 0usize;
    let mut tcvn_strong = 0usize;
    let mut tcvn_hits = 0usize;
    let mut vni_pairs = 0usize;
    let mut vni_singles = 0usize;
    let mut vni_orphans = 0usize;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii() {
            i += 1;
            continue;
        }
        non_ascii += 1;
        let b = char_to_byte(c)?;
        if tables::tcvn3_decode(b).is_some() {
            tcvn_hits += 1;
            if (0xA1..=0xBF).contains(&b) {
                tcvn_strong += 1;
            } else if c.is_uppercase() {
                let prev_lower = i > 0 && chars[i - 1].is_ascii_lowercase();
                let next_lower = chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
                if prev_lower && next_lower {
                    tcvn_strong += 1;
                }
            }
        }
        // VNI: is this char a mark following a valid base?
        if i > 0 && tables::vni_pair(chars[i - 1], c).is_some() {
            vni_pairs += 1;
        } else if tables::vni_single(c).is_some() {
            vni_singles += 1;
        } else if tables::is_vni_mark(c) {
            vni_orphans += 1;
        }
        i += 1;
    }
    if non_ascii == 0 {
        return None;
    }
    if vni_pairs > 0 && vni_pairs >= tcvn_strong && vni_orphans * 2 <= vni_pairs + vni_singles {
        return Some(Legacy::Vni);
    }
    if tcvn_strong > 0 && tcvn_hits * 2 >= non_ascii {
        return Some(Legacy::Tcvn3);
    }
    None
}

/// Convert every run of `doc` whose font (own or inherited from the document
/// defaults) is a legacy Vietnamese font: the text is converted and the font is
/// replaced by [`REPLACEMENT_FONT`]. Returns the number of runs converted.
pub fn convert_document(doc: &mut Document) -> usize {
    let default_font = doc.default_run.font.clone();
    let default_legacy = default_font.as_deref().and_then(legacy_from_font);
    let mut count = 0;
    for section in &mut doc.sections {
        for block in &mut section.blocks {
            let bo_model::Block::Paragraph(p) = block else {
                // Tables and opaque blocks: legacy fonts there are left unchanged in this version.
                continue;
            };
            for run in &mut p.runs {
                let legacy = match run.props.font.as_deref() {
                    Some(f) => legacy_from_font(f),
                    None => default_legacy,
                };
                let Some((enc, upper)) = legacy else { continue };
                for item in &mut run.content {
                    if let RunContent::Text(t) = item {
                        *t = convert(t, enc, upper);
                    }
                }
                if run.props.font.is_some() {
                    run.props.font = Some(REPLACEMENT_FONT.to_owned());
                }
                count += 1;
            }
        }
    }
    if default_legacy.is_some() {
        doc.default_run.font = Some(REPLACEMENT_FONT.to_owned());
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{Block, Paragraph, Run, RunProps, Section};
    use unicode_normalization::UnicodeNormalization;

    const MOTTO: &str = "Cộng hoà xã hội chủ nghĩa Việt Nam — Độc lập - Tự do - Hạnh phúc";

    #[test]
    fn tcvn3_known_strings() {
        assert_eq!(
            tcvn3_to_unicode("Céng hoµ x· héi chñ nghÜa ViÖt Nam", false),
            "Cộng hoà xã hội chủ nghĩa Việt Nam"
        );
        assert_eq!(
            tcvn3_to_unicode("§éc lËp - Tù do - H¹nh phóc", false),
            "Độc lập - Tự do - Hạnh phúc"
        );
        // .VnTimeH: capitals at lower-case code points.
        assert_eq!(tcvn3_to_unicode("céng hoµ", true), "CỘNG HOÀ");
    }

    #[test]
    fn vni_known_strings() {
        assert_eq!(
            vni_to_unicode("Coäng hoøa xaõ hoäi chuû nghóa Vieät Nam"),
            "Cộng hòa xã hội chủ nghĩa Việt Nam"
        );
        assert_eq!(
            vni_to_unicode("Ñoäc laäp - Töï do - Haïnh phuùc"),
            "Độc lập - Tự do - Hạnh phúc"
        );
        assert_eq!(vni_to_unicode("COÄNG HOØA"), "CỘNG HÒA");
        assert_eq!(vni_to_unicode("Ngöôøi"), "Người");
    }

    /// Every letter of the Vietnamese alphabet (all 134 vowel forms + đ/Đ)
    /// survives Unicode → legacy → Unicode for both encodings.
    #[test]
    fn exhaustive_round_trip() {
        let mut all = String::new();
        for row in VOWELS.iter().chain(VOWELS_UPPER.iter()) {
            all.extend(row.iter());
        }
        all.push_str("đĐ");
        assert_eq!(all.chars().count(), 12 * 6 * 2 + 2);

        // VNI covers upper and lower case.
        let vni = unicode_to_vni(&all);
        assert_eq!(vni_to_unicode(&vni), all);
        for c in all.chars() {
            let enc = unicode_to_vni(&c.to_string());
            assert!(
                enc.chars().all(|e| char_to_byte(e).is_some()),
                "{c} not 8-bit"
            );
            assert_eq!(vni_to_unicode(&enc), c.to_string(), "VNI {c}");
        }

        // TCVN3 encodes every lower-case letter plus Ă Â Ê Ô Ơ Ư Đ.
        let mut lower = String::new();
        for row in &VOWELS {
            lower.extend(row.iter());
        }
        lower.push('đ');
        let bytes = unicode_to_tcvn3(&lower);
        assert_eq!(bytes.len(), lower.chars().count());
        assert!(bytes.iter().all(|&b| b != b'?'));
        assert_eq!(tcvn3_bytes_to_unicode(&bytes), lower);
        for c in "ĂÂÊÔƠƯĐ".chars() {
            let b = unicode_to_tcvn3(&c.to_string());
            assert_eq!(tcvn3_bytes_to_unicode(&b), c.to_string());
        }
        // Upper-case rows via the H fonts.
        let mut upper = String::new();
        for row in &VOWELS_UPPER {
            upper.extend(row.iter());
        }
        let enc = unicode_to_tcvn3(&upper.to_lowercase());
        let as1252: String = enc.iter().map(|&b| cp1252::decode(b)).collect();
        assert_eq!(tcvn3_to_unicode(&as1252, true), upper);
    }

    #[test]
    fn tcvn3_table_is_injective_and_nfc() {
        let mut seen = std::collections::HashMap::new();
        for b in 0x80..=0xFFu8 {
            if let Some(c) = tables::tcvn3_decode(b) {
                assert!(seen.insert(c, b).is_none(), "duplicate {c}");
                assert_eq!(c.to_string().nfc().collect::<String>(), c.to_string());
                assert_eq!(tables::tcvn3_encode(c), Some(b));
            }
        }
        // 60 toned lower-case vowels + 6 lower-case base letters ă â ê ô ơ ư +
        // đ + 7 capitals Ă Â Ê Ô Ơ Ư Đ.
        assert_eq!(seen.len(), 60 + 6 + 1 + 7);
    }

    #[test]
    fn outputs_are_nfc() {
        let s = vni_to_unicode(&unicode_to_vni(MOTTO));
        assert_eq!(s, s.nfc().collect::<String>());
        assert_eq!(s, MOTTO);
    }

    #[test]
    fn ascii_unchanged() {
        let s = "Hello, world! 123 <>&";
        assert_eq!(tcvn3_to_unicode(s, false), s);
        assert_eq!(vni_to_unicode(s), s);
    }

    #[test]
    fn detection() {
        assert_eq!(
            detect_legacy_vietnamese("Céng hoµ x· héi chñ nghÜa ViÖt Nam"),
            Some(Legacy::Tcvn3)
        );
        assert_eq!(detect_legacy_vietnamese("ViÖt Nam"), Some(Legacy::Tcvn3));
        assert_eq!(
            detect_legacy_vietnamese("Coäng hoøa xaõ hoäi chuû nghóa Vieät Nam"),
            Some(Legacy::Vni)
        );
        assert_eq!(detect_legacy_vietnamese("plain ascii"), None);
        assert_eq!(detect_legacy_vietnamese(MOTTO), None);
        assert_eq!(detect_legacy_vietnamese("café crème"), None);
        assert_eq!(detect_legacy_vietnamese(""), None);
    }

    #[test]
    fn font_classification() {
        assert_eq!(legacy_from_font(".VnTime"), Some((Legacy::Tcvn3, false)));
        assert_eq!(legacy_from_font(".VnTimeH"), Some((Legacy::Tcvn3, true)));
        assert_eq!(legacy_from_font(".VnArialH"), Some((Legacy::Tcvn3, true)));
        assert_eq!(legacy_from_font("VNI-Times"), Some((Legacy::Vni, false)));
        assert_eq!(legacy_from_font("Times New Roman"), None);
    }

    #[test]
    fn document_conversion() {
        let mut doc = Document::default();
        doc.default_run.font = Some(".VnTime".into());
        doc.sections.push(Section {
            blocks: vec![Block::Paragraph(Paragraph {
                runs: vec![
                    Run::text("ViÖt Nam", RunProps::default()),
                    Run::text(
                        "Vieät Nam",
                        RunProps {
                            font: Some("VNI-Times".into()),
                            ..Default::default()
                        },
                    ),
                    Run::text(
                        "céng hoµ",
                        RunProps {
                            font: Some(".VnTimeH".into()),
                            ..Default::default()
                        },
                    ),
                    Run::text(
                        "Arial ViÖt",
                        RunProps {
                            font: Some("Arial".into()),
                            ..Default::default()
                        },
                    ),
                ],
                ..Default::default()
            })],
            ..Default::default()
        });
        assert_eq!(convert_document(&mut doc), 3);
        assert_eq!(doc.plain_text(), "Việt NamViệt NamCỘNG HOÀArial ViÖt");
        assert_eq!(doc.default_run.font.as_deref(), Some(REPLACEMENT_FONT));
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.runs[1].props.font.as_deref(), Some(REPLACEMENT_FONT));
        assert_eq!(p.runs[3].props.font.as_deref(), Some("Arial"));
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "\\PC*") {
            let _ = tcvn3_to_unicode(&s, true);
            let _ = vni_to_unicode(&s);
            let _ = detect_legacy_vietnamese(&s);
            let _ = unicode_to_tcvn3(&s);
        }

        #[test]
        fn bytes_never_panic(b in proptest::collection::vec(proptest::num::u8::ANY, 0..256)) {
            let _ = tcvn3_bytes_to_unicode(&b);
            let _ = vni_bytes_to_unicode(&b);
        }
    }
}
