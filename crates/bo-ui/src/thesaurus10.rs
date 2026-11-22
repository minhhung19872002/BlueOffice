//! English synonyms for BlueDoc's context menu (round 10, Synonyms ▸): the thesaurus data of
//! CORE-THES-001 (`assets/thesaurus/en/en.json.gz`, derived from Open English WordNet, CC BY 4.0,
//! see `assets/thesaurus/en/README.md`), decoded on the first lookup. English inflections are
//! looked up like the web thesaurus: the word, its irregular forms, then plain suffix rules.
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const DATA: &[u8] = include_bytes!("../../../assets/thesaurus/en/en.json.gz");

/// A sense reference: `[synset]` or `[synset, [antonym words]]`.
#[derive(Deserialize)]
#[serde(untagged)]
enum Sense {
    Plain((usize,)),
    WithAntonyms((usize, Vec<usize>)),
}

impl Sense {
    fn synset(&self) -> usize {
        match self {
            Sense::Plain((s,)) | Sense::WithAntonyms((s, _)) => *s,
        }
    }
}

#[derive(Deserialize)]
struct Data {
    words: Vec<String>,
    synsets: Vec<(String, String, Vec<usize>)>,
    entries: HashMap<String, Vec<Sense>>,
    forms: HashMap<String, String>,
}

fn data() -> Option<&'static Data> {
    static D: OnceLock<Option<Data>> = OnceLock::new();
    D.get_or_init(|| {
        let raw = gunzip(DATA)?;
        serde_json::from_slice(&raw).ok()
    })
    .as_ref()
}

/// The deflate stream of a gzip member (RFC 1952: header, optional fields, data).
fn gunzip(gz: &[u8]) -> Option<Vec<u8>> {
    if gz.len() < 18 || gz[0] != 0x1f || gz[1] != 0x8b || gz[2] != 8 {
        return None;
    }
    let flags = gz[3];
    let mut at = 10usize;
    if flags & 4 != 0 {
        let n = usize::from(u16::from_le_bytes([*gz.get(at)?, *gz.get(at + 1)?]));
        at += 2 + n;
    }
    for bit in [8u8, 16] {
        if flags & bit != 0 {
            at += gz.get(at..)?.iter().position(|b| *b == 0)? + 1;
        }
    }
    if flags & 2 != 0 {
        at += 2;
    }
    miniz_oxide::inflate::decompress_to_vec(gz.get(at..gz.len() - 8)?).ok()
}

/// Headwords a word may be a form of (itself, irregular forms, suffix rules).
fn lemmas(d: &Data, word: &str) -> Vec<String> {
    let w = word.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if d.entries.contains_key(&s) && !out.contains(&s) {
            out.push(s);
        }
    };
    push(w.clone());
    if let Some(f) = d.forms.get(&w) {
        for l in f.split(' ') {
            push(l.to_owned());
        }
    }
    for (suffix, repl) in [
        ("ies", "y"),
        ("ied", "y"),
        ("es", ""),
        ("s", ""),
        ("ed", ""),
        ("ed", "e"),
        ("ing", ""),
        ("ing", "e"),
        ("er", ""),
        ("est", ""),
    ] {
        if let Some(stem) = w.strip_suffix(suffix) {
            push(format!("{stem}{repl}"));
            let c: Vec<char> = stem.chars().collect();
            if c.len() >= 2 && c[c.len() - 1] == c[c.len() - 2] {
                push(c[..c.len() - 1].iter().collect());
            }
        }
    }
    out
}

/// Up to `max` synonyms of `word`, sense by sense (the word itself left out).
pub(crate) fn synonyms(word: &str, max: usize) -> Vec<String> {
    let Some(d) = data() else {
        return Vec::new();
    };
    let w = word.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    for lemma in lemmas(d, word) {
        for sense in d.entries.get(&lemma).into_iter().flatten() {
            let Some((_, _, words)) = d.synsets.get(sense.synset()) else {
                continue;
            };
            for &i in words {
                if let Some(s) = d.words.get(i) {
                    if s.to_lowercase() != w && s.to_lowercase() != lemma && !out.contains(s) {
                        out.push(s.clone());
                    }
                }
                if out.len() >= max {
                    return out;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn synonyms_of_plain_and_inflected_words() {
        let s = super::synonyms("quick", 8);
        assert!((1..=8).contains(&s.len()), "{s:?}");
        assert!(
            s.iter()
                .any(|w| w == "speedy" || w == "fast" || w == "agile"),
            "{s:?}"
        );
        assert_ne!(super::synonyms("running", 8), Vec::<String>::new());
        assert_eq!(super::synonyms("qwzxv", 8), Vec::<String>::new());
    }
}
