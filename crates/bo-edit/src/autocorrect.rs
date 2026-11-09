//! AutoCorrect as you type (CORE-AUTOCORR-001).
//!
//! After a character is typed, [`correct`] looks at the paragraph's index text
//! before the caret and proposes one [`Correction`]: replacement-list entries,
//! sentence / table-cell capitals, two initial capitals, typographic quotes,
//! dashes, ordinals, fractions, automatic lists and links. The rules are pure
//! functions over the text (byte offsets on char boundaries); [`Session::autocorrect`]
//! applies the result as its own transaction, so undo right after a correction
//! reverts only the correction and keeps the typed character.
//!
//! The default replacement and exception lists are BlueDoc's own; hosts may send
//! any list in [`AutoCorrectOptions`]. Formatting written by the rules uses
//! ECMA-376 Part 1 run properties: §17.3.2.42 vertAlign (ordinals), §17.16.22
//! hyperlink with §17.3.2.6 color / §17.3.2.40 u (links), §17.9 numbering (lists).

use crate::{para, para_clone, path_of, revisions, Result, Session, Tx};
use crate::{Op, Pos, Selection};
use bo_model::index::body_paragraphs;
use bo_model::{Color, NumFormat, NumRef, RunProps, Underline, VertAlign};
use serde::{Deserialize, Serialize};

/// One replacement-list entry: `from` typed as a word (or ending in a symbol) becomes `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replacement {
    /// Text as typed. Without capitals it matches any case; with capitals only that case.
    pub from: String,
    /// Replacement text.
    pub to: String,
}

impl Replacement {
    fn new(from: &str, to: &str) -> Self {
        Replacement {
            from: from.to_owned(),
            to: to.to_owned(),
        }
    }
}

/// What AutoCorrect does as you type. Every flag defaults to on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoCorrectOptions {
    /// Master switch.
    pub enabled: bool,
    /// Apply the replacement list.
    pub replace_text: bool,
    /// Capital at the start of a sentence (and of a paragraph outside tables).
    pub capitalize_sentences: bool,
    /// Capital at the start of a table cell paragraph.
    pub capitalize_cells: bool,
    /// "THe" → "The".
    pub two_initial_caps: bool,
    /// Straight quotes → typographic quotes (“ ” ‘ ’).
    pub smart_quotes: bool,
    /// "a--b" → em dash, "a - b" / "a -- b" → en dash.
    pub dashes: bool,
    /// 1st, 2nd, 3rd, 4th… with the suffix raised.
    pub ordinals: bool,
    /// 1/2 → ½ and the other vulgar fractions.
    pub fractions: bool,
    /// "- ", "* ", "1. ", "1) ", "a. ", "a) " at a paragraph start start a list.
    pub auto_lists: bool,
    /// URLs and e-mail addresses become hyperlinks.
    pub auto_links: bool,
    /// Replacement list.
    pub entries: Vec<Replacement>,
    /// Abbreviations ending in a full stop after which no capital is forced.
    pub exceptions: Vec<String>,
    /// Words that keep two initial capitals.
    pub caps_exceptions: Vec<String>,
}

impl Default for AutoCorrectOptions {
    fn default() -> Self {
        AutoCorrectOptions {
            enabled: true,
            replace_text: true,
            capitalize_sentences: true,
            capitalize_cells: true,
            two_initial_caps: true,
            smart_quotes: true,
            dashes: true,
            ordinals: true,
            fractions: true,
            auto_lists: true,
            auto_links: true,
            entries: default_entries(),
            exceptions: DEFAULT_EXCEPTIONS.iter().map(|s| (*s).to_owned()).collect(),
            caps_exceptions: DEFAULT_CAPS_EXCEPTIONS
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        }
    }
}

/// BlueDoc's default replacement list: symbols typed with ASCII, frequent English
/// typos and frequent Vietnamese misspellings (s/x, ch/tr, d/gi/r confusions).
pub fn default_entries() -> Vec<Replacement> {
    const LIST: &[(&str, &str)] = &[
        // Symbols.
        ("(c)", "©"),
        ("(r)", "®"),
        ("(tm)", "™"),
        ("...", "…"),
        ("-->", "→"),
        ("<--", "←"),
        ("<->", "↔"),
        ("==>", "⇒"),
        ("<==", "⇐"),
        ("<=>", "⇔"),
        ("+/-", "±"),
        ("=/=", "≠"),
        (":)", "☺"),
        (":(", "☹"),
        // English typos.
        ("teh", "the"),
        ("adn", "and"),
        ("taht", "that"),
        ("wiht", "with"),
        ("hte", "the"),
        ("recieve", "receive"),
        ("recieved", "received"),
        ("beleive", "believe"),
        ("seperate", "separate"),
        ("definately", "definitely"),
        ("occured", "occurred"),
        ("occurence", "occurrence"),
        ("untill", "until"),
        ("wich", "which"),
        ("becuase", "because"),
        ("thier", "their"),
        ("alot", "a lot"),
        ("accomodate", "accommodate"),
        ("acheive", "achieve"),
        ("adress", "address"),
        ("begining", "beginning"),
        ("calender", "calendar"),
        ("commitee", "committee"),
        ("enviroment", "environment"),
        ("goverment", "government"),
        ("independant", "independent"),
        ("neccessary", "necessary"),
        ("succesful", "successful"),
        ("tommorow", "tomorrow"),
        ("wierd", "weird"),
        ("doesnt", "doesn't"),
        ("didnt", "didn't"),
        ("isnt", "isn't"),
        ("wasnt", "wasn't"),
        // Vietnamese misspellings.
        ("chuẩn đoán", "chẩn đoán"),
        ("sắc xuất", "xác suất"),
        ("xác xuất", "xác suất"),
        ("suất sắc", "xuất sắc"),
        ("xuất xắc", "xuất sắc"),
        ("suất xứ", "xuất xứ"),
        ("bổ xung", "bổ sung"),
        ("chia xẻ", "chia sẻ"),
        ("sử lý", "xử lý"),
        ("xắp xếp", "sắp xếp"),
        ("sắp sếp", "sắp xếp"),
        ("sát nhập", "sáp nhập"),
        ("dò rỉ", "rò rỉ"),
        ("dành giật", "giành giật"),
        ("trinh phục", "chinh phục"),
        ("vô hình chung", "vô hình trung"),
        ("thăm quan", "tham quan"),
        ("nghành", "ngành"),
        ("ngiêm", "nghiêm"),
        ("ngĩa", "nghĩa"),
    ];
    LIST.iter().map(|(f, t)| Replacement::new(f, t)).collect()
}

/// Abbreviations after which the next word keeps its small letter.
pub const DEFAULT_EXCEPTIONS: &[&str] = &[
    // Vietnamese.
    "v.v.", "TP.", "Tp.", "Q.", "P.", "H.", "X.", "TT.", "GS.", "PGS.", "TS.", "ThS.", "BS.", "KS.",
    "CN.", "Tr.", "tr.", "St.", "Th.", "vd.", "VD.", "Đ.", "K.", "ĐT.", "NXB.", "Nxb.",
    // English.
    "e.g.", "i.e.", "etc.", "vs.", "cf.", "approx.", "Mr.", "Mrs.", "Ms.", "Dr.", "Prof.", "No.",
    "Fig.", "fig.", "p.", "pp.", "vol.", "Inc.", "Ltd.", "Co.", "Jr.", "Sr.", "a.m.", "p.m.",
    "Jan.", "Feb.", "Mar.", "Apr.", "Aug.", "Sep.", "Sept.", "Oct.", "Nov.", "Dec.",
];

/// Words that keep two initial capitals.
pub const DEFAULT_CAPS_EXCEPTIONS: &[&str] =
    &["IDs", "PCs", "CDs", "DVDs", "URLs", "APIs", "GPUs", "CPUs"];

/// The rule a correction comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Rule {
    /// Replacement list.
    Replace,
    /// Capital at a sentence start.
    Capitalize,
    /// Capital at a table-cell start.
    CapitalizeCell,
    /// Two initial capitals.
    TwoCaps,
    /// Typographic quote.
    SmartQuote,
    /// En / em dash.
    Dash,
    /// Raised ordinal suffix.
    Ordinal,
    /// Vulgar fraction.
    Fraction,
    /// Automatic bulleted / numbered list.
    List,
    /// Automatic hyperlink.
    Link,
}

/// One text change: bytes `start..end` of the index text become `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// Start byte.
    pub start: usize,
    /// End byte (exclusive).
    pub end: usize,
    /// New text.
    pub text: String,
    /// The new text is raised (§17.3.2.42 vertAlign superscript).
    pub superscript: bool,
}

/// List started by a typed marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoList {
    /// "- " / "* ".
    Bullet,
    /// "1." (`paren` = "1)").
    Decimal {
        /// Marker ends with ")".
        paren: bool,
    },
    /// "a." (`paren` = "a)").
    Letter {
        /// Marker ends with ")".
        paren: bool,
    },
}

/// What one typed character triggers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    /// Main rule (the first one that applied).
    pub rule: Rule,
    /// Text changes (non-overlapping, before the caret).
    pub edits: Vec<TextEdit>,
    /// Start a list: the marker `0..marker_end` is removed.
    pub list: Option<(AutoList, usize)>,
    /// Make `start..end` a hyperlink to the target.
    pub link: Option<(usize, usize, String)>,
}

impl Correction {
    fn edit(rule: Rule, start: usize, end: usize, text: impl Into<String>) -> Self {
        Correction {
            rule,
            edits: vec![TextEdit {
                start,
                end,
                text: text.into(),
                superscript: false,
            }],
            list: None,
            link: None,
        }
    }
}

/// Where the paragraph is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Context {
    /// Inside a table cell.
    pub in_table: bool,
    /// The paragraph is already a list item.
    pub list: bool,
}

/// Paragraph break pseudo-character for [`correct`] (Enter at the caret).
pub const ENTER: char = '\n';

/// Characters that end a word and trigger the word rules.
fn is_delimiter(c: char) -> bool {
    c.is_whitespace() || matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}')
}

fn is_opener(c: char) -> bool {
    matches!(c, '(' | '[' | '{' | '"' | '\'' | '“' | '‘' | '«' | '<')
}

fn is_closer(c: char) -> bool {
    matches!(
        c,
        ')' | ']' | '}' | '"' | '\'' | '”' | '’' | '»' | '>' | '.' | ',' | ';' | ':' | '!' | '?'
    )
}

/// Object placeholder of the index text (pictures, fields).
const OBJECT: char = '\u{FFFC}';

fn is_boundary(c: char) -> bool {
    c.is_whitespace() || c == OBJECT
}

/// `hay` ends with `needle` (letters compared by their lower case); returns the start byte.
fn ends_with_ci(hay: &str, needle: &str, exact: bool) -> Option<usize> {
    let mut h = hay.char_indices().rev();
    let mut start = hay.len();
    for n in needle.chars().rev() {
        let (i, c) = h.next()?;
        let same = if exact {
            c == n
        } else {
            c == n || c.to_lowercase().eq(n.to_lowercase())
        };
        if !same {
            return None;
        }
        start = i;
    }
    Some(start)
}

fn has_upper(s: &str) -> bool {
    s.chars().any(char::is_uppercase)
}

fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// `to` in the case the user typed `typed` with (for entries written without capitals).
fn adapt_case(typed: &str, to: &str) -> String {
    let letters: Vec<char> = typed.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        to.to_uppercase()
    } else if letters.first().is_some_and(|c| c.is_uppercase()) {
        upper_first(to)
    } else {
        to.to_owned()
    }
}

/// Longest replacement-list entry ending at `end` of `text`; `symbols` selects entries
/// ending in a symbol (applied as soon as their last character is typed) or in a letter
/// or digit (applied when the word ends).
fn find_entry<'a>(
    text: &str,
    end: usize,
    entries: &'a [Replacement],
    symbols: bool,
) -> Option<(usize, &'a Replacement)> {
    let hay = &text[..end];
    let mut best: Option<(usize, &Replacement)> = None;
    for e in entries {
        let Some(last) = e.from.chars().last() else {
            continue;
        };
        if last.is_alphanumeric() == symbols || e.from.chars().count() < 2 && symbols {
            continue;
        }
        let Some(start) = ends_with_ci(hay, &e.from, has_upper(&e.from)) else {
            continue;
        };
        // Entries starting with a letter or digit start a word.
        let first_alnum = e.from.chars().next().is_some_and(char::is_alphanumeric);
        let before = hay[..start].chars().last();
        if first_alnum && before.is_some_and(|c| !is_boundary(c) && !is_opener(c)) {
            continue;
        }
        if best.is_none_or(|(s, _)| start < s) {
            best = Some((start, e));
        }
    }
    best
}

/// Propose the correction for `typed`, just typed before byte `caret` of the paragraph
/// index text `text` ([`ENTER`]: a paragraph break is being made at `caret`).
pub fn correct(
    text: &str,
    caret: usize,
    typed: char,
    ctx: &Context,
    opts: &AutoCorrectOptions,
) -> Option<Correction> {
    if !opts.enabled || caret > text.len() || !text.is_char_boundary(caret) {
        return None;
    }
    let before = if typed == ENTER {
        &text[..caret]
    } else {
        let head = &text[..caret];
        if !head.ends_with(typed) {
            return None;
        }
        &head[..caret - typed.len_utf8()]
    };
    let at = before.len(); // byte where `typed` starts
    if typed == '"' || typed == '\'' {
        return opts.smart_quotes.then(|| smart_quote(before, typed));
    }
    if typed != ENTER && opts.replace_text {
        if let Some((start, e)) = find_entry(text, caret, &opts.entries, true) {
            let to = if has_upper(&e.from) {
                e.to.clone()
            } else {
                adapt_case(&text[start..caret], &e.to)
            };
            return Some(Correction::edit(Rule::Replace, start, caret, to));
        }
    }
    if typed != ENTER && !is_delimiter(typed) {
        return None;
    }
    // The token: non-blank text before the delimiter.
    let ts = before
        .char_indices()
        .rev()
        .find(|(_, c)| is_boundary(*c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let token = &before[ts..];
    if token.is_empty() {
        return None;
    }
    if (typed == ' ' || typed == '\t') && opts.auto_lists && !ctx.list && ts == 0 {
        if let Some(kind) = list_marker(token) {
            return Some(Correction {
                rule: Rule::List,
                edits: Vec::new(),
                list: Some((kind, caret)),
                link: None,
            });
        }
    }
    // The word inside the token, without surrounding punctuation.
    let lead: usize = token
        .chars()
        .take_while(|c| is_opener(*c))
        .map(char::len_utf8)
        .sum();
    let ws = ts + lead;
    let trail: usize = before[ws..]
        .chars()
        .rev()
        .take_while(|c| is_closer(*c))
        .map(char::len_utf8)
        .sum();
    let we = at - trail;
    if (typed.is_whitespace()) && opts.auto_links && we > ws {
        let w = &before[ws..we];
        if let Some(target) = link_target(w) {
            return Some(Correction {
                rule: Rule::Link,
                edits: Vec::new(),
                list: None,
                link: Some((ws, we, target)),
            });
        }
    }
    if we <= ws {
        return None;
    }
    word_rules(text, before, ts, ws, we, typed, *ctx, opts)
}

#[allow(clippy::too_many_arguments)]
fn word_rules(
    text: &str,
    before: &str,
    ts: usize,
    ws: usize,
    we: usize,
    typed: char,
    ctx: Context,
    opts: &AutoCorrectOptions,
) -> Option<Correction> {
    let mut rule = None;
    let mut es = ws;
    let mut word = before[ws..we].to_owned();
    if opts.replace_text {
        if let Some((start, e)) = find_entry(text, we, &opts.entries, false) {
            es = start;
            word = if has_upper(&e.from) {
                e.to.clone()
            } else {
                adapt_case(&text[start..we], &e.to)
            };
            rule = Some(Rule::Replace);
        }
    }
    if opts.fractions && rule.is_none() {
        if let Some(f) = fraction(&word) {
            return Some(Correction::edit(Rule::Fraction, ws, we, f));
        }
    }
    if opts.ordinals && rule.is_none() {
        if let Some(split) = ordinal_suffix(&word) {
            let mut c = Correction::edit(Rule::Ordinal, ws + split, we, &word[split..]);
            c.edits[0].superscript = true;
            return Some(c);
        }
    }
    if opts.two_initial_caps && !opts.caps_exceptions.iter().any(|x| x == &word) {
        if let Some(fixed) = two_caps(&word) {
            word = fixed;
            rule.get_or_insert(Rule::TwoCaps);
        }
    }
    // Sentence capital: the text before the edit is blank, or ends a sentence.
    let mut prefix = &before[..es];
    prefix = prefix.trim_end_matches(is_opener);
    let plain = word.chars().next().is_some_and(char::is_lowercase)
        && !word.chars().skip(1).any(char::is_uppercase)
        && !word.contains(['@', '/', '\\', '_'])
        && !has_inner_dot(&word)
        && !word.chars().any(|c| c.is_ascii_digit());
    // "a." / "a)" may be an enumeration: leave single letters before them alone.
    let marker = matches!(typed, '.' | ')') && word.chars().count() == 1;
    if plain && !marker && typed != '.' && (prefix.is_empty() || prefix.ends_with(is_boundary)) {
        if let Some(r) = sentence_start(prefix, ctx, opts) {
            word = upper_first(&word);
            rule.get_or_insert(r);
        }
    }
    let mut edits = Vec::new();
    if opts.dashes {
        if let Some(e) = dash(before, ts, es, ws, we, &word) {
            if e.start >= ws {
                // "a--b" inside the word.
                word = e.text.clone();
            } else {
                edits.push(e);
            }
            rule.get_or_insert(Rule::Dash);
        }
    }
    let rule = rule?;
    if word != before[es..we] {
        edits.push(TextEdit {
            start: es,
            end: we,
            text: word,
            superscript: false,
        });
    }
    if edits.is_empty() {
        return None;
    }
    Some(Correction {
        rule,
        edits,
        list: None,
        link: None,
    })
}

fn has_inner_dot(w: &str) -> bool {
    let t = w.trim_end_matches('.');
    t.contains('.')
}

/// `Some(rule)` when a word after `prefix` (blank-terminated or empty) starts a sentence.
fn sentence_start(prefix: &str, ctx: Context, opts: &AutoCorrectOptions) -> Option<Rule> {
    let t = prefix.trim_end_matches(is_boundary);
    if t.is_empty() {
        return if ctx.in_table {
            opts.capitalize_cells.then_some(Rule::CapitalizeCell)
        } else {
            opts.capitalize_sentences.then_some(Rule::Capitalize)
        };
    }
    if !opts.capitalize_sentences {
        return None;
    }
    let core = t.trim_end_matches([')', ']', '"', '\'', '”', '’', '»']);
    if core.ends_with("..") || core.ends_with('…') {
        return None;
    }
    if !core.ends_with(['.', '!', '?']) {
        return None;
    }
    if core.ends_with('.') {
        let start = core
            .char_indices()
            .rev()
            .find(|(_, c)| is_boundary(*c))
            .map_or(0, |(i, c)| i + c.len_utf8());
        let prev = core[start..].trim_start_matches(is_opener);
        let excepted = opts
            .exceptions
            .iter()
            .any(|x| x == prev || x.to_lowercase() == prev.to_lowercase() && !has_upper(x));
        if excepted {
            return None;
        }
    }
    Some(Rule::Capitalize)
}

/// "THe" → "The" (two capitals, then small letters only).
fn two_caps(word: &str) -> Option<String> {
    let cs: Vec<char> = word.chars().collect();
    if cs.len() < 3 || !cs.iter().all(|c| c.is_alphabetic()) {
        return None;
    }
    if !(cs[0].is_uppercase() && cs[1].is_uppercase() && cs[2..].iter().all(|c| c.is_lowercase())) {
        return None;
    }
    let mut out = String::new();
    out.push(cs[0]);
    out.extend(cs[1].to_lowercase());
    out.extend(&cs[2..]);
    Some(out)
}

/// Opening or closing typographic quote for `typed` after `before`.
fn smart_quote(before: &str, typed: char) -> Correction {
    let prev = before.chars().last();
    let open = prev.is_none_or(|c| {
        is_boundary(c)
            || matches!(
                c,
                '(' | '[' | '{' | '<' | '“' | '‘' | '«' | '-' | '–' | '—' | '/'
            )
    });
    let q = match (typed, open) {
        ('"', true) => '“',
        ('"', false) => '”',
        (_, true) => '‘',
        (_, false) => '’',
    };
    let at = before.len();
    Correction::edit(Rule::SmartQuote, at, at + typed.len_utf8(), q.to_string())
}

/// Dash rules: "word--word" (no blanks) → em dash; "word - word" or "word -- word" → en dash.
fn dash(before: &str, ts: usize, es: usize, ws: usize, we: usize, word: &str) -> Option<TextEdit> {
    if let Some(i) = word.find("--") {
        let l = word[..i].chars().last();
        let r = word[i + 2..].chars().next();
        if l.is_some_and(char::is_alphanumeric)
            && r.is_some_and(char::is_alphanumeric)
            && !word[i + 2..].contains('-')
        {
            let text = format!("{}—{}", &word[..i], &word[i + 2..]);
            return Some(TextEdit {
                start: ws,
                end: we,
                text,
                superscript: false,
            });
        }
    }
    if es < ts {
        return None; // a multi-word replacement covers the dash
    }
    let head = &before[..ts];
    for pat in [" -- ", " - "] {
        if let Some(stem) = head.strip_suffix(pat) {
            if stem
                .chars()
                .last()
                .is_some_and(|c| !is_boundary(c) && c != '-')
            {
                let start = stem.len() + 1;
                return Some(TextEdit {
                    start,
                    end: start + pat.len() - 2,
                    text: "–".into(),
                    superscript: false,
                });
            }
        }
    }
    None
}

/// Unicode vulgar fraction for "1/2" and its kin.
fn fraction(w: &str) -> Option<&'static str> {
    Some(match w {
        "1/2" => "½",
        "1/3" => "⅓",
        "2/3" => "⅔",
        "1/4" => "¼",
        "3/4" => "¾",
        "1/5" => "⅕",
        "1/6" => "⅙",
        "1/8" => "⅛",
        "3/8" => "⅜",
        "5/8" => "⅝",
        "7/8" => "⅞",
        _ => return None,
    })
}

/// Byte where the English ordinal suffix of "21st" starts, when it matches the number.
fn ordinal_suffix(w: &str) -> Option<usize> {
    let split = w.find(|c: char| !c.is_ascii_digit())?;
    if split == 0 || split > 9 {
        return None;
    }
    let n: u64 = w[..split].parse().ok()?;
    let want = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    (w[split..] == *want).then_some(split)
}

/// List kind for a marker typed at a paragraph start.
fn list_marker(token: &str) -> Option<AutoList> {
    match token {
        "-" | "*" | "•" => return Some(AutoList::Bullet),
        "1." => return Some(AutoList::Decimal { paren: false }),
        "1)" => return Some(AutoList::Decimal { paren: true }),
        "a." => return Some(AutoList::Letter { paren: false }),
        "a)" => return Some(AutoList::Letter { paren: true }),
        _ => {}
    }
    None
}

/// Hyperlink target of a typed URL or e-mail address.
pub(crate) fn link_target(w: &str) -> Option<String> {
    let lower = w.to_lowercase();
    for scheme in ["http://", "https://", "ftp://", "mailto:"] {
        if lower.starts_with(scheme) && w.len() > scheme.len() {
            return Some(w.to_owned());
        }
    }
    if lower.starts_with("www.") && w.len() > 4 && w[4..].contains('.') {
        return Some(format!("http://{w}"));
    }
    // name@domain.tld
    let (user, host) = w.split_once('@')?;
    let ok_user = !user.is_empty()
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'));
    let ok_host = host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
    (ok_user && ok_host).then(|| format!("mailto:{w}"))
}

// ------------------------------------------------------------------ session

impl Session {
    /// AutoCorrect after `typed` was typed just before the caret (one undo step of its
    /// own). Returns the rule that changed the text, if any.
    pub fn autocorrect(&mut self, opts: &AutoCorrectOptions, typed: char) -> Result<Option<Rule>> {
        let sel = self.selection;
        if !sel.is_collapsed() || !opts.enabled {
            return Ok(None);
        }
        self.autocorrect_at(opts, sel.focus, typed, true)
    }

    /// AutoCorrect the end of the paragraph before the caret after Enter split it.
    pub fn autocorrect_break(&mut self, opts: &AutoCorrectOptions) -> Result<Option<Rule>> {
        let sel = self.selection;
        if !sel.is_collapsed() || sel.focus.offset != 0 || sel.focus.para == 0 || !opts.enabled {
            return Ok(None);
        }
        let id = sel.focus.para - 1;
        let len = self.para_text(id).map_or(0, |t| t.len());
        self.autocorrect_at(opts, Pos::new(id, len), ENTER, false)
    }

    fn autocorrect_at(
        &mut self,
        opts: &AutoCorrectOptions,
        at: Pos,
        typed: char,
        move_caret: bool,
    ) -> Result<Option<Rule>> {
        let paths = body_paragraphs(&self.doc);
        let Some(path) = paths.get(at.para) else {
            return Ok(None);
        };
        let p = para_clone(&self.doc, path)?;
        let text = bo_model::index::para_index_text(&p);
        let ctx = Context {
            in_table: path.len() > 2,
            list: p.props.num.is_some_and(|n| n.num_id != 0),
        };
        let Some(c) = correct(&text, at.offset, typed, &ctx, opts) else {
            return Ok(None);
        };
        // A link over text that already is one changes nothing.
        if let Some((s, e, _)) = &c.link {
            let atoms = para::slice(&para::flatten(&p), *s, *e);
            if !atoms.is_empty() && atoms.iter().all(|(r, _)| r.link.is_some()) {
                return Ok(None);
            }
        }
        let rule = c.rule;
        self.edit(|tx, sel| {
            let caret = apply(tx, at, &c)?;
            Ok(if move_caret {
                Selection::caret(Pos::new(at.para, caret))
            } else {
                sel
            })
        })?;
        Ok(Some(rule))
    }
}

/// Apply `c` to paragraph `at.para`; returns the new byte offset of `at`.
fn apply(tx: &mut Tx<'_>, at: Pos, c: &Correction) -> Result<usize> {
    let paths = tx.paths();
    let path = path_of(tx.doc, &paths, at.para)?;
    let mut p = para_clone(tx.doc, &path)?;
    let mut atoms = para::flatten(&p);
    let len_before = para::len(&atoms);
    let tail = len_before.saturating_sub(at.offset);
    let mut edits = c.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse(e.start));
    for e in &edits {
        let mut props = para::props_at(&atoms, if e.end > e.start { e.end } else { e.start });
        if e.superscript {
            props.vert_align = Some(VertAlign::Superscript);
        }
        replace_range(tx, &mut atoms, e.start, e.end, &e.text, &props);
    }
    if let Some((s, e, target)) = &c.link {
        let a = para::split_at(&mut atoms, *s);
        let b = para::split_at(&mut atoms, *e);
        for (r, _) in &mut atoms[a..b] {
            link_props(r, target);
        }
    }
    if let Some((kind, end)) = c.list {
        let props = para::props_at(&atoms, end);
        replace_range(tx, &mut atoms, 0, end, "", &props);
        let old = tx.doc.numbering.clone();
        let mut new = old.clone();
        let id = add_list(&mut new, kind);
        tx.push(Op::Numbering {
            old: Box::new(old),
            new: Box::new(new),
        })?;
        p.props.num = Some(NumRef {
            num_id: id,
            ilvl: 0,
        });
    }
    let caret = para::len(&atoms).saturating_sub(tail);
    tx.replace_para(&path, para::with_atoms(&p, atoms))?;
    Ok(caret)
}

/// Replace `start..end` with `text` (tracked as a deletion + insertion in track changes).
fn replace_range(
    tx: &mut Tx<'_>,
    atoms: &mut Vec<para::Atom>,
    start: usize,
    end: usize,
    text: &str,
    props: &RunProps,
) {
    let at = if let Some(tr) = tx.track.as_mut() {
        let mark = tr.mark();
        let removed = revisions::mark_deleted(atoms, start, end, tr, &mark);
        end - removed
    } else {
        para::delete(atoms, start, end);
        start
    };
    if !text.is_empty() {
        let new = tx.mark_new(atoms, at, para::text_atoms(text, props));
        para::insert(atoms, at, new);
    }
}

/// Hyperlink look: §17.3.2.6 color and §17.3.2.40 u single.
pub(crate) fn link_props(r: &mut RunProps, target: &str) {
    r.link = Some(target.to_owned());
    r.color = Color::from_hex("0563C1");
    r.underline = Some(Underline::Single);
}

/// Add a one-level-style list definition for `kind` (§17.9.6 lvl); returns its `numId`.
fn add_list(n: &mut bo_model::Numbering, kind: AutoList) -> i32 {
    let id = n.add_simple_list(matches!(kind, AutoList::Bullet));
    let suffix = |paren: bool| if paren { ")" } else { "." };
    let abs = n.nums.iter().find(|x| x.id == id).map(|x| x.abstract_id);
    if let Some(a) = n.abstracts.iter_mut().find(|a| Some(a.id) == abs) {
        if let Some(l0) = a.levels.first_mut() {
            match kind {
                AutoList::Bullet => {}
                AutoList::Decimal { paren } => {
                    l0.format = NumFormat::Decimal;
                    l0.text = format!("%1{}", suffix(paren));
                }
                AutoList::Letter { paren } => {
                    l0.format = NumFormat::LowerLetter;
                    l0.text = format!("%1{}", suffix(paren));
                }
            }
        }
    }
    id
}

#[cfg(test)]
mod tests;
