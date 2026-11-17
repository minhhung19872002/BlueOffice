//! English meanings and synonyms for BlueSlide's Spelling pane (round 17): the thesaurus data
//! the web editors use (`assets/thesaurus/en/en.json.gz`, Open English WordNet 2025, CC BY 4.0
//! — `assets/thesaurus/en/README.md`, format `bo-thesaurus/1` of
//! `packages/platform/src/thesaurus.ts`), loaded on the first lookup.
use std::collections::HashMap;
use std::sync::OnceLock;

static GZ: &[u8] = include_bytes!("../../../assets/thesaurus/en/en.json.gz");

/// One meaning: part of speech (`n`, `v`, `a`, `r`), short definition, synonyms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meaning {
    pub pos: String,
    pub gloss: String,
    pub synonyms: Vec<String>,
    /// Antonyms of the word in this sense.
    pub antonyms: Vec<String>,
}

struct Data {
    words: Vec<String>,
    synsets: Vec<(String, String, Vec<usize>)>,
    /// Senses of each word: synset and antonym word ids.
    entries: HashMap<String, Vec<(usize, Vec<usize>)>>,
    forms: HashMap<String, String>,
}

/// The deflate stream of a gzip member (RFC 1952 §2.3: header, optional fields, data, CRC).
fn gunzip(b: &[u8]) -> Option<Vec<u8>> {
    if b.len() < 18 || b[0] != 0x1f || b[1] != 0x8b || b[2] != 8 {
        return None;
    }
    let flags = b[3];
    let mut i = 10;
    if flags & 4 != 0 {
        let n = usize::from(*b.get(i)?) | usize::from(*b.get(i + 1)?) << 8;
        i += 2 + n;
    }
    for bit in [8u8, 16] {
        if flags & bit != 0 {
            i += b.get(i..)?.iter().position(|c| *c == 0)? + 1;
        }
    }
    if flags & 2 != 0 {
        i += 2;
    }
    let body = b.get(i..b.len().checked_sub(8)?)?;
    miniz_oxide::inflate::decompress_to_vec(body).ok()
}

fn parse(json: &[u8]) -> Option<Data> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    let words = v
        .get("words")?
        .as_array()?
        .iter()
        .map(|w| w.as_str().unwrap_or_default().to_owned())
        .collect();
    let synsets = v
        .get("synsets")?
        .as_array()?
        .iter()
        .map(|s| {
            let pos = s.get(0).and_then(|p| p.as_str()).unwrap_or("n").to_owned();
            let gloss = s.get(1).and_then(|g| g.as_str()).unwrap_or("").to_owned();
            let ws = s
                .get(2)
                .and_then(|w| w.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(serde_json::Value::as_u64)
                        .map(|n| n as usize)
                        .collect()
                })
                .unwrap_or_default();
            (pos, gloss, ws)
        })
        .collect();
    let entries = v
        .get("entries")?
        .as_object()?
        .iter()
        .map(|(k, senses)| {
            let ids = senses
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| {
                            let id = s.get(0).and_then(serde_json::Value::as_u64)? as usize;
                            let ants = s
                                .get(1)
                                .and_then(serde_json::Value::as_array)
                                .map(|x| {
                                    x.iter()
                                        .filter_map(serde_json::Value::as_u64)
                                        .map(|n| n as usize)
                                        .collect()
                                })
                                .unwrap_or_default();
                            Some((id, ants))
                        })
                        .collect()
                })
                .unwrap_or_default();
            (k.clone(), ids)
        })
        .collect();
    let forms = v
        .get("forms")?
        .as_object()?
        .iter()
        .map(|(k, f)| (k.clone(), f.as_str().unwrap_or_default().to_owned()))
        .collect();
    Some(Data {
        words,
        synsets,
        entries,
        forms,
    })
}

fn data() -> Option<&'static Data> {
    static DATA: OnceLock<Option<Data>> = OnceLock::new();
    DATA.get_or_init(|| gunzip(GZ).as_deref().and_then(parse))
        .as_ref()
}

/// Base-form candidates of an English word (the word first): plural / third person `-s`,
/// `-es`, `-ies`, past `-ed`, `-ing`, a dropped `e` (a reduced form of the web's rules).
fn bases(w: &str) -> Vec<String> {
    let mut out = vec![w.to_owned()];
    let mut add = |s: String| {
        if s.len() >= 2 && !out.contains(&s) {
            out.push(s);
        }
    };
    if let Some(s) = w.strip_suffix("ies") {
        add(format!("{s}y"));
    }
    if let Some(s) = w.strip_suffix("es") {
        add(s.to_owned());
    }
    if let Some(s) = w.strip_suffix('s') {
        if !w.ends_with("ss") {
            add(s.to_owned());
        }
    }
    for suf in ["ed", "ing"] {
        if let Some(s) = w.strip_suffix(suf) {
            add(s.to_owned());
            add(format!("{s}e"));
        }
    }
    out
}

/// Meanings of `word` (at most `max`), each with its synonyms other than the word itself.
#[must_use]
pub fn meanings(word: &str, max: usize) -> Vec<Meaning> {
    let Some(d) = data() else {
        return Vec::new();
    };
    let w = word.trim().to_lowercase();
    let mut lemmas: Vec<String> = Vec::new();
    for b in bases(&w) {
        if let Some(f) = d.forms.get(&b) {
            lemmas.extend(f.split(' ').map(str::to_owned));
        }
        lemmas.push(b);
    }
    let mut out = Vec::new();
    for l in lemmas {
        let Some(ids) = d.entries.get(&l) else {
            continue;
        };
        for (id, ants) in ids {
            let Some((pos, gloss, ws)) = d.synsets.get(*id) else {
                continue;
            };
            let synonyms = ws
                .iter()
                .filter_map(|i| d.words.get(*i))
                .filter(|s| s.to_lowercase() != l)
                .cloned()
                .collect();
            out.push(Meaning {
                pos: pos.clone(),
                gloss: gloss.clone(),
                synonyms,
                antonyms: ants
                    .iter()
                    .filter_map(|i| d.words.get(*i))
                    .cloned()
                    .collect(),
            });
            if out.len() >= max {
                return out;
            }
        }
        if !out.is_empty() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_words_up() {
        let m = meanings("documents", 3);
        assert!(!m.is_empty(), "document has meanings");
        assert!(m.iter().all(|x| !x.gloss.is_empty()));
        assert_eq!(meanings("zzqxv", 3).len(), 0);
        assert_eq!(bases("cities")[1], "city");
    }
}
