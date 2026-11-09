//! Spell checking for BlueOffice (`docs/parity/BACKLOG.md` CORE-SPELL).
//!
//! * [`Dictionary`]: a Hunspell-compatible dictionary (`.aff` + `.dic`, format after the public hunspell(5)
//!   description, implemented from scratch): affix stripping with conditions and cross products,
//!   continuation classes (two suffixes), compounding, case rules and the special flags; suggestions from
//!   the replacement table, keyboard neighbours, the try characters, deletions, swaps and word splits.
//! * [`Checker`]: anything that checks one word (dictionaries, rule-based checkers such as Vietnamese
//!   syllables).
//! * [`Speller`]: several checkers for one language plus a user dictionary and an ignore list, and
//!   [`words`] to find the words of a text (UAX #29).
#![forbid(unsafe_code)]

pub mod aff;
pub mod grammar;
pub mod lang;
pub mod vi;

use aff::{AffixFile, Flag};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// Checks single words.
pub trait Checker: Send + Sync {
    /// The word is correctly spelt.
    fn check(&self, word: &str) -> bool;
    /// Correction candidates, best first.
    fn suggest(&self, word: &str) -> Vec<String>;
}

/// A Hunspell-compatible dictionary.
#[derive(Debug, Clone)]
pub struct Dictionary {
    aff: AffixFile,
    words: HashMap<String, Vec<Vec<Flag>>>,
}

/// Capitalisation of a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Case {
    Lower,
    Initial,
    All,
    Mixed,
}

fn case_of(w: &str) -> Case {
    let letters: Vec<char> = w.chars().filter(|c| c.is_alphabetic()).collect();
    let upper = letters.iter().filter(|c| c.is_uppercase()).count();
    if upper == 0 {
        Case::Lower
    } else if upper == letters.len() && letters.len() > 1 {
        Case::All
    } else if upper == letters.len()
        || (upper == 1 && letters.first().is_some_and(|c| c.is_uppercase()))
    {
        Case::Initial
    } else {
        Case::Mixed
    }
}

fn capitalize(w: &str) -> String {
    let mut cs = w.chars();
    match cs.next() {
        Some(c) => c.to_uppercase().chain(cs).collect(),
        None => String::new(),
    }
}

fn with_case(w: &str, case: Case) -> String {
    match case {
        Case::Initial => capitalize(w),
        Case::All => w.to_uppercase(),
        Case::Lower | Case::Mixed => w.to_owned(),
    }
}

impl Dictionary {
    /// Loads a dictionary from the bytes of its `.aff` and `.dic` files (UTF-8 or ISO 8859-1, per `SET`).
    pub fn from_aff_dic(aff: &[u8], dic: &[u8]) -> Result<Dictionary, String> {
        let aff_text = aff::decode(aff)?;
        let affix = AffixFile::parse(&aff_text)?;
        // The .dic file uses the encoding the .aff declares.
        let set_line = aff_text
            .lines()
            .find(|l| l.trim().starts_with("SET "))
            .unwrap_or("SET UTF-8");
        let mut tagged = format!("{set_line}\n").into_bytes();
        let dic_text = {
            tagged.extend_from_slice(dic);
            let t = aff::decode(&tagged)?;
            t.split_once('\n')
                .map(|(_, rest)| rest.to_owned())
                .unwrap_or_default()
        };
        let words = affix.parse_dic(&dic_text);
        Ok(Dictionary { aff: affix, words })
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// No entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    fn has(flags: &[Flag], f: Option<Flag>) -> bool {
        f.is_some_and(|f| flags.contains(&f))
    }

    /// Flag sets of a root, without forbidden ones.
    fn roots(&self, w: &str) -> impl Iterator<Item = &Vec<Flag>> {
        self.words
            .get(w)
            .into_iter()
            .flatten()
            .filter(|f| !Self::has(f, self.aff.forbidden))
    }

    fn forbidden(&self, w: &str) -> bool {
        self.words
            .get(w)
            .is_some_and(|sets| sets.iter().any(|f| Self::has(f, self.aff.forbidden)))
    }

    /// `w` is a root usable on its own (not NEEDAFFIX / ONLYINCOMPOUND).
    fn plain(&self, w: &str) -> bool {
        self.roots(w)
            .any(|f| !Self::has(f, self.aff.needaffix) && !Self::has(f, self.aff.onlyincompound))
    }

    /// A root of `w` carrying `flag` (and, when `with`, also accepting a further affix).
    fn root_with(&self, root: &str, flag: Flag) -> bool {
        self.roots(root).any(|f| f.contains(&flag))
    }

    /// Words obtained by removing one suffix: (root, suffix).
    fn strip_suffixes<'a>(
        &'a self,
        w: &'a str,
    ) -> impl Iterator<Item = (String, &'a aff::Affix)> + 'a {
        self.aff.suffixes.iter().filter_map(move |s| {
            let stem = w.strip_suffix(s.add.as_str())?;
            if stem.is_empty() && s.strip.is_empty() {
                return None;
            }
            let root = format!("{stem}{}", s.strip);
            s.cond.matches_end(&root).then_some((root, s))
        })
    }

    fn strip_prefixes<'a>(
        &'a self,
        w: &'a str,
    ) -> impl Iterator<Item = (String, &'a aff::Affix)> + 'a {
        self.aff.prefixes.iter().filter_map(move |p| {
            let stem = w.strip_prefix(p.add.as_str())?;
            if stem.is_empty() && p.strip.is_empty() {
                return None;
            }
            let root = format!("{}{stem}", p.strip);
            p.cond.matches_start(&root).then_some((root, p))
        })
    }

    /// `w` derives from a root through affixes (one prefix, up to two suffixes).
    fn affixed(&self, w: &str) -> bool {
        for (root, s) in self.strip_suffixes(w) {
            if self.root_with(&root, s.flag) {
                return true;
            }
            // Cross product: a prefix on the suffixed word.
            if s.cross {
                for (r2, p) in self.strip_prefixes(&root) {
                    if p.cross
                        && self
                            .roots(&r2)
                            .any(|f| f.contains(&s.flag) && f.contains(&p.flag))
                    {
                        return true;
                    }
                }
            }
            // Twofold suffixes: an inner suffix whose continuation allows this one.
            for (r2, s2) in self.strip_suffixes(&root) {
                if s2.cont.contains(&s.flag) && self.root_with(&r2, s2.flag) {
                    return true;
                }
            }
            // The suffix's own continuation classes on the root (affix with a NEEDAFFIX root).
        }
        for (root, p) in self.strip_prefixes(w) {
            if self.root_with(&root, p.flag) {
                return true;
            }
        }
        false
    }

    fn compound_part(&self, w: &str, pos: usize, last: bool) -> bool {
        let role = if pos == 0 {
            self.aff.compound_begin
        } else if last {
            self.aff.compound_end
        } else {
            self.aff.compound_middle
        };
        self.roots(w)
            .any(|f| Self::has(f, self.aff.compound_flag) || Self::has(f, role))
    }

    /// `w` is a compound of dictionary words (COMPOUNDFLAG / BEGIN / MIDDLE / END, COMPOUNDMIN).
    fn compound(&self, w: &str, pos: usize, depth: usize) -> bool {
        if depth > 4 || (self.aff.compound_flag.is_none() && self.aff.compound_begin.is_none()) {
            return false;
        }
        let min = self.aff.compound_min;
        let idx: Vec<usize> = w.char_indices().map(|(i, _)| i).chain([w.len()]).collect();
        let n = idx.len() - 1;
        for &cut in idx.iter().take(n.saturating_sub(min) + 1).skip(min) {
            let (head, tail) = w.split_at(cut);
            if !self.compound_part(head, pos, false) {
                continue;
            }
            if self.compound_part(tail, pos + 1, true) || self.compound(tail, pos + 1, depth + 1) {
                return true;
            }
        }
        false
    }

    fn check_exact(&self, w: &str) -> bool {
        if self.forbidden(w) {
            return false;
        }
        self.plain(w) || self.affixed(w) || self.compound(w, 0, 0)
    }

    fn keepcase(&self, w: &str) -> bool {
        self.words
            .get(w)
            .is_some_and(|sets| sets.iter().any(|f| Self::has(f, self.aff.keepcase)))
    }

    fn nosuggest(&self, w: &str) -> bool {
        self.words
            .get(w)
            .is_some_and(|sets| sets.iter().any(|f| Self::has(f, self.aff.nosuggest)))
    }

    fn check_case(&self, w: &str) -> bool {
        if self.check_exact(w) {
            return true;
        }
        match case_of(w) {
            Case::Initial => {
                let lower = w.to_lowercase();
                !self.keepcase(&lower) && self.check_exact(&lower)
            }
            Case::All => {
                let lower = w.to_lowercase();
                let cap = capitalize(&lower);
                (!self.keepcase(&lower) && self.check_exact(&lower))
                    || (!self.keepcase(&cap) && self.check_exact(&cap))
            }
            Case::Lower | Case::Mixed => false,
        }
    }

    fn candidates(&self, w: &str) -> Vec<(String, u32)> {
        let mut out: Vec<(String, u32)> = Vec::new();
        let chars: Vec<char> = w.chars().collect();
        let push = |s: String, score: u32, out: &mut Vec<(String, u32)>| {
            if s != w && !s.is_empty() {
                out.push((s, score));
            }
        };
        // REP table (likely misspellings first).
        for (from, to) in &self.aff.rep {
            let mut start = 0;
            while let Some(i) = w[start..].find(from.as_str()) {
                let at = start + i;
                push(
                    format!("{}{to}{}", &w[..at], &w[at + from.len()..]),
                    0,
                    &mut out,
                );
                start = at + from.len().max(1);
            }
        }
        // Keyboard neighbours (KEY groups).
        for (i, &c) in chars.iter().enumerate() {
            for group in &self.aff.key {
                let g: Vec<char> = group.chars().collect();
                if let Some(p) = g.iter().position(|&x| x == c) {
                    for q in [p.wrapping_sub(1), p + 1] {
                        if let Some(&n) = g.get(q) {
                            let mut v = chars.clone();
                            v[i] = n;
                            push(v.into_iter().collect(), 1, &mut out);
                        }
                    }
                }
            }
        }
        // Swaps of neighbours.
        for i in 0..chars.len().saturating_sub(1) {
            let mut v = chars.clone();
            v.swap(i, i + 1);
            push(v.into_iter().collect(), 1, &mut out);
        }
        // Deletions.
        for i in 0..chars.len() {
            let mut v = chars.clone();
            v.remove(i);
            push(v.into_iter().collect(), 2, &mut out);
        }
        // TRY characters: replace and insert.
        let try_chars: Vec<char> = self.aff.try_chars.chars().collect();
        for i in 0..=chars.len() {
            for &t in &try_chars {
                if i < chars.len() && chars[i] != t {
                    let mut v = chars.clone();
                    v[i] = t;
                    push(v.into_iter().collect(), 2, &mut out);
                }
                let mut v = chars.clone();
                v.insert(i, t);
                push(v.into_iter().collect(), 2, &mut out);
            }
        }
        // Two words.
        for i in 1..chars.len() {
            let (a, b): (String, String) =
                (chars[..i].iter().collect(), chars[i..].iter().collect());
            if self.check_case(&a) && self.check_case(&b) {
                push(format!("{a} {b}"), 3, &mut out);
            }
        }
        out
    }
}

/// Bigram similarity (n-gram suggestion fallback).
fn ngram_score(a: &str, b: &str) -> usize {
    let grams = |s: &str| -> HashSet<(char, char)> {
        let cs: Vec<char> = s.chars().collect();
        cs.windows(2).map(|w| (w[0], w[1])).collect()
    };
    grams(a).intersection(&grams(b)).count()
}

impl Checker for Dictionary {
    fn check(&self, word: &str) -> bool {
        let w = word.trim_matches(|c: char| c == '.' || c == '\'' || c == '’');
        if w.is_empty() || !w.chars().any(char::is_alphabetic) {
            return true;
        }
        // Numbers with letters attached (e.g. 2nd, 10km) are not checked.
        if w.chars().any(|c| c.is_ascii_digit()) {
            return true;
        }
        self.check_case(w) || self.check_case(&w.replace('’', "'"))
    }

    fn suggest(&self, word: &str) -> Vec<String> {
        let case = case_of(word);
        let base = match case {
            Case::Lower | Case::Mixed => word.to_owned(),
            _ => word.to_lowercase(),
        };
        let mut seen = HashSet::new();
        let mut scored: Vec<(String, u32)> = Vec::new();
        for (c, s) in self.candidates(&base) {
            if !seen.insert(c.clone()) {
                continue;
            }
            let ok = if c.contains(' ') {
                true
            } else {
                self.check_case(&c) && !self.nosuggest(&c)
            };
            if ok {
                scored.push((c, s));
            }
        }
        if scored.is_empty() && self.words.len() <= 500_000 {
            // n-gram search over roots of similar length.
            let n = base.chars().count();
            let mut best: Vec<(usize, &String)> = self
                .words
                .keys()
                .filter(|k| {
                    k.chars().count().abs_diff(n) <= 2 && self.plain(k) && !self.nosuggest(k)
                })
                .map(|k| (ngram_score(&base, k), k))
                .filter(|(s, _)| *s > 0)
                .collect();
            best.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
            scored = best
                .into_iter()
                .take(5)
                .map(|(s, k)| (k.clone(), 10 - s.min(9) as u32))
                .collect();
        }
        scored.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        scored
            .into_iter()
            .map(|(s, _)| with_case(&s, case))
            .take(8)
            .collect()
    }
}

/// Checkers tagged by language plus a user dictionary and an ignore list.
///
/// A checker added with [`Speller::add`] serves every language; one added with
/// [`Speller::add_language`] serves the texts of its primary language subtag (`vi` for `vi-VN`,
/// `en` for `en-US` and `en-GB`).
pub struct Speller {
    checkers: Vec<(Option<String>, Box<dyn Checker>)>,
    user: HashSet<String>,
    ignored: HashSet<String>,
    /// Words written entirely in capitals (abbreviations such as `UBND`, `NĐ`) are not checked (default on).
    pub ignore_all_caps: bool,
    /// Words with digits (`A4`, `mp3`, `2nd`) are not checked (default on).
    pub ignore_digits: bool,
}

impl Default for Speller {
    fn default() -> Speller {
        Speller {
            checkers: Vec::new(),
            user: HashSet::new(),
            ignored: HashSet::new(),
            ignore_all_caps: true,
            ignore_digits: true,
        }
    }
}

/// Primary language subtag of a BCP 47 tag, lower case (`vi-VN` → `vi`).
#[must_use]
pub fn primary_language(tag: &str) -> String {
    tag.split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

impl Speller {
    /// An empty speller (every word is accepted until a checker is added).
    #[must_use]
    pub fn new() -> Speller {
        Speller::default()
    }

    /// Adds a checker for every language; a word is correct when any checker accepts it.
    pub fn add(&mut self, c: Box<dyn Checker>) {
        self.checkers.push((None, c));
    }

    /// Adds a checker for one language (BCP 47 tag; only the primary subtag counts).
    pub fn add_language(&mut self, lang: &str, c: Box<dyn Checker>) {
        self.checkers.push((Some(primary_language(lang)), c));
    }

    /// A checker serves `lang` (`None`: any language).
    #[must_use]
    pub fn has_language(&self, lang: Option<&str>) -> bool {
        self.serving(lang).next().is_some()
    }

    fn serving<'a>(&'a self, lang: Option<&str>) -> impl Iterator<Item = &'a dyn Checker> + 'a {
        let want = lang.map(primary_language);
        self.checkers
            .iter()
            .filter(move |(tag, _)| match (&want, tag) {
                (None, _) | (Some(_), None) => true,
                (Some(w), Some(t)) => w == t,
            })
            .map(|(_, c)| c.as_ref())
    }

    /// Adds a word to the user dictionary.
    pub fn add_user_word(&mut self, w: &str) {
        self.user.insert(w.to_owned());
    }

    /// Ignores a word for this session ("ignore all").
    pub fn ignore(&mut self, w: &str) {
        self.ignored.insert(w.to_owned());
    }

    /// The word needs no dictionary: user word, ignored, abbreviation in capitals, with digits or
    /// without letters.
    fn exempt(&self, w: &str) -> bool {
        (self.ignore_all_caps && case_of(w) == Case::All)
            || (self.ignore_digits && w.chars().any(char::is_numeric))
            || !w.chars().any(char::is_alphabetic)
            || self.user.contains(w)
            || self.ignored.contains(w)
            || self.user.contains(&w.to_lowercase())
    }

    /// The word is correct in any language.
    #[must_use]
    pub fn check(&self, w: &str) -> bool {
        self.check_in(w, None).unwrap_or(true)
    }

    /// The word is correct in `lang` (`None`: any language); `None` when no checker serves the
    /// language (the word is then not checked).
    #[must_use]
    pub fn check_in(&self, w: &str, lang: Option<&str>) -> Option<bool> {
        if self.checkers.is_empty() || self.exempt(w) {
            return Some(true);
        }
        // Checkers accept words with digits; asked to check them, they are wrong unless the user
        // dictionary has them.
        if w.chars().any(char::is_numeric) {
            return self.has_language(lang).then_some(false);
        }
        let mut any = false;
        for c in self.serving(lang) {
            any = true;
            if c.check(w) {
                return Some(true);
            }
        }
        any.then_some(false)
    }

    /// Suggestions from every checker, merged in order.
    #[must_use]
    pub fn suggest(&self, w: &str) -> Vec<String> {
        self.suggest_in(w, None)
    }

    /// Suggestions from the checkers of `lang` (`None`: every checker), merged in order.
    #[must_use]
    pub fn suggest_in(&self, w: &str, lang: Option<&str>) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for c in self.serving(lang) {
            for s in c.suggest(w) {
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        }
        out.truncate(8);
        out
    }

    /// Byte ranges of the misspelt words of `text` (URLs, e-mail addresses and numbers are
    /// skipped, see [`checkable_words`]).
    #[must_use]
    pub fn misspelt(&self, text: &str) -> Vec<Range<usize>> {
        checkable_words(text)
            .into_iter()
            .filter(|r| !self.check(&text[r.clone()]))
            .collect()
    }
}

/// Byte ranges of the words of `text` (UAX #29 word boundaries; apostrophes inside words kept).
#[must_use]
pub fn words(text: &str) -> Vec<Range<usize>> {
    text.split_word_bound_indices()
        .filter(|(_, w)| w.chars().any(char::is_alphanumeric))
        .map(|(i, w)| i..i + w.len())
        .collect()
}

/// The words of `text` worth checking: [`words`] without the ones inside a URL (`http://…`,
/// `www.…`, `mailto:…`) or an e-mail address, and without numbers (no letter at all).
#[must_use]
pub fn checkable_words(text: &str) -> Vec<Range<usize>> {
    let skip = skipped_tokens(text);
    words(text)
        .into_iter()
        .filter(|r| text[r.clone()].chars().any(char::is_alphabetic))
        .filter(|r| !skip.iter().any(|s| s.start <= r.start && r.end <= s.end))
        .collect()
}

/// Byte ranges of the whitespace-separated tokens that are URLs or e-mail addresses.
fn skipped_tokens(text: &str) -> Vec<Range<usize>> {
    fn special(t: &str) -> bool {
        let t = t.trim_matches(|c: char| "()[]{}<>\"'“”‘’,;".contains(c));
        let lower = t.to_lowercase();
        let url =
            lower.contains("://") || lower.starts_with("www.") || lower.starts_with("mailto:");
        let mail = t.split_once('@').is_some_and(|(user, host)| {
            !user.is_empty() && host.contains('.') && !host.starts_with('.')
        });
        url || mail
    }
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                if special(&text[s..i]) {
                    out.push(s..i);
                }
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        if special(&text[s..]) {
            out.push(s..text.len());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Dictionary written for these tests after the hunspell(5) format description.
    const AFF: &str = "SET UTF-8
TRY esianrtolcdugmphbyfvkwz
KEY qwertyuiop|asdfghjkl|zxcvbnm
REP 2
REP f ph
REP ph f
NOSUGGEST !
FORBIDDENWORD *
KEEPCASE K
COMPOUNDFLAG C
COMPOUNDMIN 3
ONLYINCOMPOUND O
NEEDAFFIX N

PFX U Y 1
PFX U 0 un .

SFX S Y 3
SFX S y ies [^aeiou]y
SFX S 0 s [aeiou]y
SFX S 0 s [^y]

SFX D Y 4
SFX D 0 d e
SFX D y ied [^aeiou]y
SFX D 0 ed [^ey]
SFX D 0 ed [aeiou]y

SFX L N 1
SFX L 0 ly/S .
";
    const DIC: &str = "13
walk/DS
carry/SD
play/SD
do/U
like/DU
graph/S
quick/L
damn/!
colour/*
iPhone/K
foot/C
ball/C
happi/N
";

    fn dict() -> Dictionary {
        Dictionary::from_aff_dic(AFF.as_bytes(), DIC.as_bytes()).unwrap()
    }

    #[test]
    fn roots_suffixes_prefixes_and_conditions() {
        let d = dict();
        for w in [
            "walk", "walks", "walked", "carries", "carried", "plays", "played", "undo", "liked",
            "unlike", "graphs",
        ] {
            assert!(d.check(w), "{w}");
        }
        for w in ["carrys", "playies", "walkes", "unwalk", "graphed"] {
            assert!(!d.check(w), "{w}");
        }
    }

    #[test]
    fn cross_product_and_continuation() {
        let d = dict();
        // like + un- + -ed (both cross).
        assert!(d.check("unliked"));
        // quick + -ly (continuation S) + -s.
        assert!(d.check("quickly"));
    }

    #[test]
    fn case_rules_and_special_flags() {
        let d = dict();
        assert!(d.check("Walk") && d.check("WALKED"));
        assert!(!d.check("wAlk"));
        // KEEPCASE: only as written.
        assert!(d.check("iPhone") && !d.check("Iphone"));
        // FORBIDDENWORD.
        assert!(!d.check("colour"));
        // NEEDAFFIX: not alone.
        assert!(!d.check("happi"));
        // Numbers and punctuation are not checked.
        assert!(d.check("2nd") && d.check("--") && d.check("walk."));
    }

    #[test]
    fn compounds() {
        let d = dict();
        assert!(d.check("football"));
        assert!(d.check("footballfoot"));
        assert!(!d.check("footwalk"));
    }

    #[test]
    fn suggestions() {
        let d = dict();
        assert_eq!(d.suggest("wlak").first().map(String::as_str), Some("walk"));
        assert!(d.suggest("grafs").contains(&"graphs".to_owned()));
        assert!(d.suggest("Walkd").contains(&"Walked".to_owned()));
        assert!(d.suggest("walkplay").contains(&"walk play".to_owned()));
        // NOSUGGEST words are accepted but never proposed.
        assert!(d.check("damn"));
        assert!(!d.suggest("damm").contains(&"damn".to_owned()));
    }

    #[test]
    fn speller_with_user_words_and_ignore_list() {
        let mut s = Speller::new();
        s.add(Box::new(dict()));
        let text = "We walkd the BlueOffice way.";
        let bad: Vec<&str> = s.misspelt(text).into_iter().map(|r| &text[r]).collect();
        assert_eq!(bad, vec!["We", "walkd", "the", "BlueOffice", "way"]);
        s.add_user_word("BlueOffice");
        s.ignore("We");
        let bad: Vec<&str> = s.misspelt(text).into_iter().map(|r| &text[r]).collect();
        assert_eq!(bad, vec!["walkd", "the", "way"]);
        // Abbreviations in capitals are skipped unless asked.
        assert!(s.check("UBND"));
        s.ignore_all_caps = false;
        assert!(!s.check("UBND"));
    }

    #[test]
    fn checkers_by_language() {
        let mut s = Speller::new();
        s.add_language("en-US", Box::new(dict()));
        s.add_language("vi-VN", Box::new(vi::Vietnamese));
        // English words are English, Vietnamese syllables Vietnamese; en-GB shares the en checker.
        assert_eq!(s.check_in("walked", Some("en-GB")), Some(true));
        assert_eq!(s.check_in("walked", Some("vi-VN")), Some(false));
        assert_eq!(s.check_in("chào", Some("vi-VN")), Some(true));
        assert_eq!(s.check_in("chào", Some("en-US")), Some(false));
        // No checker for French: not checked.
        assert_eq!(s.check_in("bonjour", Some("fr-FR")), None);
        assert!(!s.has_language(Some("fr-FR")) && s.has_language(Some("en-AU")));
        // Without a language any checker may accept the word.
        assert_eq!(s.check_in("chào", None), Some(true));
        // Suggestions come from the language's checker only.
        assert!(s
            .suggest_in("wlak", Some("vi-VN"))
            .iter()
            .all(|w| w != "walk"));
        assert!(s
            .suggest_in("wlak", Some("en-US"))
            .contains(&"walk".to_owned()));
        // Words with digits are skipped unless asked.
        assert!(s.check("mp3x"));
        s.ignore_digits = false;
        assert!(!s.check("mp3x"));
    }

    #[test]
    fn urls_mails_and_numbers_are_not_checked() {
        let t = "See www.exampel.org, mail tom@exampel.com or (https://exampel.net/pagee) 12,5 now";
        let w: Vec<&str> = checkable_words(t).into_iter().map(|r| &t[r]).collect();
        assert_eq!(w, vec!["See", "mail", "or", "now"]);
    }

    #[test]
    fn words_of_a_text() {
        let t = "Xin chào, Việt Nam! don't 3.5";
        let w: Vec<&str> = words(t).into_iter().map(|r| &t[r]).collect();
        assert_eq!(w, vec!["Xin", "chào", "Việt", "Nam", "don't", "3.5"]);
    }

    #[test]
    fn iso_8859_1_dictionaries() {
        let aff = b"SET ISO8859-1\nTRY e\n";
        let dic = b"1\ncaf\xe9\n";
        let d = Dictionary::from_aff_dic(aff, dic).unwrap();
        assert!(d.check("café"));
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "\\PC{0,24}") {
            let d = dict();
            let _ = d.check(&s);
            let _ = d.suggest(&s);
        }
    }
}
