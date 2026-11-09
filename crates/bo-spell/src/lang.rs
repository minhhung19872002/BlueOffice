//! Language guessing and readability statistics for proofing (`docs/parity/BACKLOG.md`
//! CORE-SPELL-005).
//!
//! [`detect`] is a small heuristic written for this project: Vietnamese is told apart from English
//! by its letters with diacritics (`ă â đ ê ô ơ ư` and the five tone marks), English by plain ASCII
//! words and the letters `f j w z`, which native Vietnamese words never use.

use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// Lower-case letters only Vietnamese uses among the languages BlueOffice proofs (the quality
/// marks and every toned vowel).
const VI_LETTERS: &str = "àáảãạăằắẳẵặâầấẩẫậđèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵ";

/// A word written with a Vietnamese letter (`chào`, `Đà`).
#[must_use]
pub fn has_vietnamese_letter(word: &str) -> bool {
    word.chars()
        .flat_map(char::to_lowercase)
        .any(|c| VI_LETTERS.contains(c))
}

/// The language of a text, guessed from its letters: `vi-VN`, `en-US` or `None` when the text is
/// too short or mixed to tell.
///
/// * Vietnamese when at least a quarter of the words carry a Vietnamese letter.
/// * English when no word does, there are at least three words, and either a word uses
///   `f j w z` or the words are all ASCII with an average length above four letters (Vietnamese
///   syllables are short).
#[must_use]
pub fn detect(text: &str) -> Option<&'static str> {
    let words: Vec<&str> = crate::words(text)
        .into_iter()
        .map(|r| &text[r])
        .filter(|w| w.chars().any(char::is_alphabetic))
        .collect();
    if words.is_empty() {
        return None;
    }
    let vi = words.iter().filter(|w| has_vietnamese_letter(w)).count();
    if vi * 4 >= words.len() && vi > 0 {
        return Some("vi-VN");
    }
    if vi > 0 || words.len() < 3 || !words.iter().all(|w| w.is_ascii()) {
        return None;
    }
    let foreign = words.iter().any(|w| {
        w.chars()
            .any(|c| matches!(c.to_ascii_lowercase(), 'f' | 'j' | 'w' | 'z'))
    });
    let letters: usize = words.iter().map(|w| w.chars().count()).sum();
    (foreign || letters > 4 * words.len()).then_some("en-US")
}

/// Readability statistics of a text (the main text of a document).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Readability {
    /// Words (UAX #29 words with a letter or digit).
    pub words: usize,
    /// Sentences (UAX #29 sentences with a word).
    pub sentences: usize,
    /// Non-empty paragraphs.
    pub paragraphs: usize,
    /// Characters of the words.
    pub characters: usize,
}

impl Readability {
    /// Average words per sentence (0 without sentences).
    #[must_use]
    pub fn words_per_sentence(&self) -> f64 {
        ratio(self.words, self.sentences)
    }

    /// Average sentences per paragraph (0 without paragraphs).
    #[must_use]
    pub fn sentences_per_paragraph(&self) -> f64 {
        ratio(self.sentences, self.paragraphs)
    }

    /// Average characters per word (0 without words).
    #[must_use]
    pub fn characters_per_word(&self) -> f64 {
        ratio(self.characters, self.words)
    }
}

#[allow(clippy::cast_precision_loss)]
fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        a as f64 / b as f64
    }
}

/// Readability statistics of paragraphs.
#[must_use]
pub fn readability<'a>(paragraphs: impl IntoIterator<Item = &'a str>) -> Readability {
    let mut r = Readability::default();
    for p in paragraphs {
        let words = crate::words(p);
        if words.is_empty() {
            continue;
        }
        r.paragraphs += 1;
        r.words += words.len();
        r.characters += words
            .iter()
            .map(|w| p[w.clone()].chars().count())
            .sum::<usize>();
        r.sentences += p
            .unicode_sentences()
            .filter(|s| s.chars().any(char::is_alphanumeric))
            .count();
    }
    r
}

/// The sentence of `text` around byte range `at` (UAX #29 sentence boundaries), trimmed.
#[must_use]
pub fn sentence_around(text: &str, at: &Range<usize>) -> Range<usize> {
    let mut out = 0..text.len();
    for (i, s) in text.split_sentence_bound_indices() {
        let end = i + s.len();
        if i <= at.start && at.start < end.max(i + 1) {
            out = i..end.max(at.end);
            break;
        }
    }
    // Trim trailing spaces of the sentence.
    let trimmed = text[out.clone()].trim_end().len();
    out.start..(out.start + trimmed).max(at.end)
}

#[cfg(test)]
mod tests {
    use super::*;

    // source: everyday Vietnamese and English sentences written for these tests.
    #[test]
    fn detects_vietnamese_and_english() {
        assert_eq!(detect("Hôm nay trời đẹp quá"), Some("vi-VN"));
        assert_eq!(detect("The weather is lovely today"), Some("en-US"));
        // Toneless syllables alone cannot be told apart.
        assert_eq!(detect("xin cho ban"), None);
        assert_eq!(detect("ok"), None);
        assert_eq!(detect(""), None);
        assert!(has_vietnamese_letter("Đà") && !has_vietnamese_letter("Da"));
    }

    #[test]
    fn readability_counts() {
        // 2 + 3 words in the first paragraph (two sentences), 4 in the second (one sentence).
        let r = readability(["Xin chào. Bạn khỏe không?", "", "This is a test"]);
        assert_eq!(r.paragraphs, 2);
        assert_eq!(r.words, 9);
        assert_eq!(r.sentences, 3);
        assert!((r.words_per_sentence() - 3.0).abs() < 1e-9);
        assert!((r.sentences_per_paragraph() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn sentence_context() {
        let t = "Một câu. Câu thứ hai có lỗii ở đây. Câu ba.";
        let at = t.find("lỗii").unwrap();
        let r = sentence_around(t, &(at..at + "lỗii".len()));
        assert_eq!(&t[r], "Câu thứ hai có lỗii ở đây.");
    }
}
