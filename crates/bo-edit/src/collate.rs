//! Vietnamese collation for sorting text (table sort).
//!
//! The Vietnamese alphabet orders the letters with diacritics after their base letter:
//! `a ă â b c d đ e ê g h i k l m n o ô ơ p q r s t u ư v x y`. Words compare first by
//! these letters, then by their tone marks in the order ngang (none), huyền (grave),
//! hỏi (hook above), ngã (tilde), sắc (acute), nặng (dot below), then lower case
//! before upper case.

use std::cmp::Ordering;
use unicode_normalization::UnicodeNormalization;

/// Comparison key of one string: (letters, tones, case flags).
type Key = (Vec<u32>, Vec<u8>, Vec<u8>);

fn key(s: &str) -> Key {
    let mut letters: Vec<u32> = Vec::new();
    let mut tones: Vec<u8> = Vec::new();
    let mut cases: Vec<u8> = Vec::new();
    for c in s.nfd() {
        let u = c as u32;
        match u {
            // Letter modifiers: breve (ă), circumflex (â ê ô), horn (ơ ư).
            0x0306 | 0x0302 | 0x031B => {
                if let Some(l) = letters.last_mut() {
                    *l += match u {
                        0x0306 => 1,
                        0x0302 => 2,
                        _ => 3,
                    };
                }
            }
            // Tone marks.
            0x0300 | 0x0309 | 0x0303 | 0x0301 | 0x0323 => {
                if let Some(t) = tones.last_mut() {
                    *t = match u {
                        0x0300 => 1,
                        0x0309 => 2,
                        0x0303 => 3,
                        0x0301 => 4,
                        _ => 5,
                    };
                }
            }
            // Other combining marks do not change the order.
            0x0300..=0x036F => {}
            _ => {
                let (base, variant, upper) = match c {
                    'đ' => ('d', 1, false),
                    'Đ' => ('d', 1, true),
                    c => {
                        let lower = c.to_lowercase().next().unwrap_or(c);
                        (lower, 0, lower != c)
                    }
                };
                letters.push(base as u32 * 4 + variant);
                tones.push(0);
                cases.push(u8::from(upper));
            }
        }
    }
    (letters, tones, cases)
}

/// Compare two strings in Vietnamese dictionary order.
#[must_use]
pub fn vi_cmp(a: &str, b: &str) -> Ordering {
    let (ka, kb) = (key(a), key(b));
    ka.0.cmp(&kb.0)
        .then_with(|| ka.1.cmp(&kb.1))
        .then_with(|| ka.2.cmp(&kb.2))
        .then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    // source: Vietnamese alphabet order (a ă â … d đ … o ô ơ … u ư) and tone order
    // ngang, huyền, hỏi, ngã, sắc, nặng as used in Vietnamese dictionaries.
    #[test]
    fn vietnamese_order() {
        let mut v = vec![
            "Đào", "An", "Ánh", "Bình", "Âu", "Ăn", "anh", "Dũng", "ơn", "ôm", "om", "ưa", "uy",
        ];
        v.sort_by(|a, b| vi_cmp(a, b));
        assert_eq!(
            v,
            vec![
                "An", "anh", "Ánh", "Ăn", "Âu", "Bình", "Dũng", "Đào", "om", "ôm", "ơn", "uy", "ưa"
            ]
        );
        let mut tones = vec!["mạ", "má", "mã", "mả", "mà", "ma"];
        tones.sort_by(|a, b| vi_cmp(a, b));
        assert_eq!(tones, vec!["ma", "mà", "mả", "mã", "má", "mạ"]);
        // Lower case first for otherwise equal words.
        assert_eq!(vi_cmp("hà", "Hà"), Ordering::Less);
    }
}
