//! Automatic hyphenation (ECMA-376 Part 1, §17.15.1.10 autoHyphenation).
//!
//! BlueOffice's own rules for English words (ADR-0011): no third-party pattern
//! file is used. A word is split before common suffixes, after a few long
//! prefixes and, inside the stem, between syllables found from its vowel groups
//! (one consonant goes with the next syllable, two are split unless they form a
//! digraph, a longer cluster keeps a valid onset with the next syllable).
//!
//! Only words made of ASCII letters are hyphenated: Vietnamese is written in
//! monosyllables (with diacritics), so it only ever breaks at spaces.

/// Letters kept before a hyphen at least (as in common typesetting practice).
pub(crate) const LEFT_MIN: usize = 2;
/// Letters carried to the next line at least.
pub(crate) const RIGHT_MIN: usize = 3;
/// Shorter words are never hyphenated.
pub(crate) const MIN_WORD: usize = 6;

/// Suffixes split off before hyphenating the stem, longest first.
const SUFFIXES: &[&str] = &[
    "ments", "ment", "nesses", "ness", "lessly", "less", "ships", "ship", "tions", "tion", "sions",
    "sion", "able", "ible", "fully", "ful", "ings", "ing",
];

/// Prefixes split off when the rest is long enough.
const PREFIXES: &[&str] = &[
    "counter", "inter", "multi", "over", "semi", "super", "trans", "under",
];

/// Consonant pairs that sound as one consonant and are never split.
const DIGRAPHS: &[&str] = &["ch", "ck", "gh", "ph", "sh", "th", "wh"];

/// Consonant pairs that may start a syllable.
const ONSETS: &[&str] = &[
    "bl", "br", "ch", "cl", "cr", "dr", "fl", "fr", "gl", "gr", "ph", "pl", "pr", "sc", "sh", "sk",
    "sl", "sm", "sn", "sp", "st", "sw", "th", "tr", "tw", "wh", "wr",
];

/// Consonant triples that may start a syllable.
const ONSETS3: &[&str] = &["chr", "sch", "scr", "shr", "spl", "spr", "str", "thr"];

fn is_vowel(w: &[u8], i: usize) -> bool {
    match w[i] {
        b'a' | b'e' | b'i' | b'o' => true,
        // "qu": the u is part of the consonant.
        b'u' => !(i > 0 && w[i - 1] == b'q'),
        // y is a vowel after a consonant ("hyphen"), a consonant at the start ("yellow").
        b'y' => i > 0 && !is_vowel(w, i - 1),
        _ => false,
    }
}

/// Hyphenation points of `word`: byte offsets (ascending) where it may be split
/// with a hyphen; empty for short words and words that are not plain ASCII
/// letters.
pub(crate) fn points(word: &str) -> Vec<usize> {
    let n = word.len();
    if n < MIN_WORD || !word.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Vec::new();
    }
    let w = word.to_ascii_lowercase().into_bytes();
    let mut out: Vec<usize> = Vec::new();
    // 1. Suffix: break before it (between doubled consonants before "-ing":
    //    run-ning, but fall-ing / miss-ing).
    let mut end = n;
    for suf in SUFFIXES {
        let s = suf.len();
        if n >= s + 3 && w.ends_with(suf.as_bytes()) {
            let mut at = n - s;
            if suf.starts_with("ing")
                && at >= 2
                && w[at - 1] == w[at - 2]
                && !is_vowel(&w, at - 1)
                && !matches!(w[at - 1], b'l' | b's' | b'f' | b'z')
            {
                at -= 1;
            }
            out.push(at);
            end = at;
            break;
        }
    }
    // 2. Prefix.
    let mut start = 0;
    for pre in PREFIXES {
        let p = pre.len();
        if end >= p + 4 && w.starts_with(pre.as_bytes()) {
            out.push(p);
            start = p;
            break;
        }
    }
    // 3. Syllables of the stem w[start..end].
    let stem = &w[start..end];
    let m = stem.len();
    // Vowel groups (start, end) of the stem; a final silent "e" is not one
    // ("hope-less"), except in a final consonant + "le" syllable ("ta-ble").
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < m {
        if is_vowel(stem, i) {
            let g0 = i;
            while i < m && is_vowel(stem, i) {
                i += 1;
            }
            groups.push((g0, i));
        } else {
            i += 1;
        }
    }
    let le = m >= 3 && stem.ends_with(b"le") && !is_vowel(stem, m - 3);
    if let Some(&(g0, g1)) = groups.last() {
        if g1 == m && g1 - g0 == 1 && stem[g0] == b'e' && !le && groups.len() > 1 {
            groups.pop();
        }
    }
    for pair in groups.windows(2) {
        let (a, b) = (pair[0].1, pair[1].0);
        let c = &stem[a..b];
        let at = match c.len() {
            0 => continue,
            // V-CV, but x stays with the first syllable (ex-am-ple).
            1 => {
                if c[0] == b'x' {
                    b
                } else {
                    a
                }
            }
            2 => {
                if c == b"ck" {
                    b
                } else if DIGRAPHS.contains(&std::str::from_utf8(c).unwrap_or("")) {
                    a
                } else {
                    a + 1
                }
            }
            k => {
                let last3 = std::str::from_utf8(&c[k - 3..]).unwrap_or("");
                let last2 = std::str::from_utf8(&c[k - 2..]).unwrap_or("");
                if k >= 4 && ONSETS3.contains(&last3) {
                    b - 3
                } else if ONSETS.contains(&last2) {
                    b - 2
                } else {
                    b - 1
                }
            }
        };
        out.push(start + at);
    }
    // Both parts keep a vowel and enough letters.
    let has_vowel = |r: std::ops::Range<usize>| r.into_iter().any(|i| is_vowel(&w, i));
    out.sort_unstable();
    out.dedup();
    out.retain(|&p| p >= LEFT_MIN && n - p >= RIGHT_MIN && has_vowel(0..p) && has_vowel(p..n));
    out
}

#[cfg(test)]
mod tests {
    use super::points;

    /// The word with a hyphen at every point.
    fn show(word: &str) -> String {
        let mut s = String::new();
        let pts = points(word);
        for (i, c) in word.char_indices() {
            if pts.contains(&i) {
                s.push('-');
            }
            s.push(c);
        }
        s
    }

    #[test]
    fn dictionary_words() {
        // Syllable divisions as given by English dictionaries.
        assert_eq!(show("information"), "in-for-ma-tion");
        assert_eq!(show("establishment"), "es-tab-lish-ment");
        assert_eq!(show("important"), "im-por-tant");
        assert_eq!(show("wonderful"), "won-der-ful");
        assert_eq!(show("children"), "chil-dren");
        assert_eq!(show("beautiful"), "beau-ti-ful");
        assert_eq!(show("hopeless"), "hope-less");
        assert_eq!(show("running"), "run-ning");
        assert_eq!(show("question"), "ques-tion");
        assert_eq!(show("Hundred"), "Hun-dred");
        assert_eq!(show("pumpkin"), "pump-kin");
        assert_eq!(show("monster"), "mon-ster");
    }

    #[test]
    fn not_hyphenated() {
        // Too short, or not plain ASCII letters (Vietnamese syllables, numbers).
        for w in ["table", "nghiêng", "thuyền", "2026abc"] {
            assert_eq!(points(w), Vec::<usize>::new(), "{w}");
        }
        assert!(points("rhythm").iter().all(|&p| p >= 2 && 6 - p >= 3));
    }
}
