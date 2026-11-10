//! BibTeX import and export of sources. The format is the one described in the BibTeX
//! documentation (O. Patashnik, *BibTeXing*, 1988): `@type{key, field = value, …}` entries,
//! values in braces or quotes joined by `#`, `@string` abbreviations, names as `Family,
//! Given` or `Given Family` separated by `and`, and LaTeX accent commands, the common ones
//! of which become Unicode letters here.

use super::names::{fold_ascii, month_number, parse_name};
use super::source::{new_tag, unique_tag, Contributor, Person, Source};
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

/// Largest text accepted (characters).
pub const MAX_LEN: usize = 8 * 1024 * 1024;

struct Parser<'a> {
    s: &'a [char],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }

    fn ident(&mut self) -> String {
        let start = self.i;
        while self
            .peek()
            .is_some_and(|c| !c.is_whitespace() && !"{}(),=#\"".contains(c))
        {
            self.i += 1;
        }
        self.s[start..self.i].iter().collect()
    }

    /// `{…}` with nested braces; the outer braces are dropped.
    fn braced(&mut self) -> String {
        let mut depth = 0;
        let mut out = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            match c {
                '\\' => {
                    out.push(c);
                    if let Some(n) = self.peek() {
                        out.push(n);
                        self.i += 1;
                    }
                }
                '{' => {
                    depth += 1;
                    if depth > 1 {
                        out.push(c);
                    }
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(c);
                }
                c => out.push(c),
            }
        }
        out
    }

    /// `"…"`; braces inside protect quotes.
    fn quoted(&mut self) -> String {
        let mut depth = 0;
        let mut out = String::new();
        self.i += 1;
        while let Some(c) = self.peek() {
            self.i += 1;
            match c {
                '\\' => {
                    out.push(c);
                    if let Some(n) = self.peek() {
                        out.push(n);
                        self.i += 1;
                    }
                }
                '{' => {
                    depth += 1;
                    out.push(c);
                }
                '}' => {
                    depth -= 1;
                    out.push(c);
                }
                '"' if depth <= 0 => break,
                c => out.push(c),
            }
        }
        out
    }

    /// A field value: pieces joined by `#`; a bare word is a number or an abbreviation.
    fn value(&mut self, strings: &HashMap<String, String>) -> String {
        let mut out = String::new();
        loop {
            self.ws();
            match self.peek() {
                // The braces of a braced value are kept (they mark a literal name).
                Some('{') => {
                    let v = self.braced();
                    out.push('{');
                    out.push_str(&v);
                    out.push('}');
                }
                Some('"') => out.push_str(&self.quoted()),
                Some(_) => {
                    let w = self.ident();
                    if w.is_empty() {
                        break;
                    }
                    match strings.get(&w.to_lowercase()) {
                        Some(v) => out.push_str(v),
                        None => out.push_str(&w),
                    }
                }
                None => break,
            }
            self.ws();
            if self.peek() == Some('#') {
                self.i += 1;
            } else {
                break;
            }
        }
        out
    }
}

/// A raw entry: (type, key, fields in order; field names lower-cased).
type RawEntry = (String, String, Vec<(String, String)>);

/// The raw entries of a `.bib` text.
fn entries(text: &str) -> Vec<RawEntry> {
    let chars: Vec<char> = text.chars().take(MAX_LEN).collect();
    let mut p = Parser { s: &chars, i: 0 };
    let mut strings: HashMap<String, String> = HashMap::new();
    let mut out = Vec::new();
    while p.i < chars.len() {
        // Everything outside an entry is a comment.
        if chars[p.i] != '@' {
            p.i += 1;
            continue;
        }
        p.i += 1;
        let kind = p.ident().to_lowercase();
        p.ws();
        let Some(open) = p.peek().filter(|c| *c == '{' || *c == '(') else {
            continue;
        };
        let close = if open == '{' { '}' } else { ')' };
        if kind == "comment" || kind == "preamble" {
            if open == '{' {
                p.braced();
            } else {
                while p.peek().is_some_and(|c| c != ')') {
                    p.i += 1;
                }
            }
            continue;
        }
        p.i += 1;
        p.ws();
        let key = if kind == "string" {
            String::new()
        } else {
            let k = p.ident();
            p.ws();
            if p.peek() == Some(',') {
                p.i += 1;
            }
            k
        };
        let mut fields = Vec::new();
        loop {
            p.ws();
            match p.peek() {
                None => break,
                Some(c) if c == close => {
                    p.i += 1;
                    break;
                }
                Some(',') => {
                    p.i += 1;
                    continue;
                }
                _ => {}
            }
            let name = p.ident().to_lowercase();
            p.ws();
            if p.peek() != Some('=') {
                // Not a field: skip a character so the scan always advances.
                if name.is_empty() {
                    p.i += 1;
                }
                continue;
            }
            p.i += 1;
            let v = p.value(&strings);
            if kind == "string" {
                strings.insert(name, strip_outer(&v).to_owned());
            } else {
                fields.push((name, v));
            }
        }
        if kind != "string" {
            out.push((kind, key, fields));
        }
    }
    out
}

/// `{text}` → `text` when the braces wrap the whole value.
fn strip_outer(v: &str) -> &str {
    let t = v.trim();
    if wrapped(t) {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

/// Whether `t` is one brace group (`{…}` closing only at the end).
fn wrapped(t: &str) -> bool {
    if !t.starts_with('{') || !t.ends_with('}') {
        return false;
    }
    let mut depth = 0;
    let mut prev = ' ';
    for (i, c) in t.char_indices() {
        if prev != '\\' {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 && i + 1 < t.len() {
                        return false;
                    }
                }
                _ => {}
            }
        }
        prev = c;
    }
    depth == 0
}

/// The combining mark of a LaTeX accent command (`\'` → U+0301 …).
fn accent(c: char) -> Option<char> {
    Some(match c {
        '\'' => '\u{0301}',
        '`' => '\u{0300}',
        '^' => '\u{0302}',
        '"' => '\u{0308}',
        '~' => '\u{0303}',
        '=' => '\u{0304}',
        '.' => '\u{0307}',
        'c' => '\u{0327}',
        'v' => '\u{030C}',
        'u' => '\u{0306}',
        'H' => '\u{030B}',
        'k' => '\u{0328}',
        'd' => '\u{0323}',
        'r' => '\u{030A}',
        'b' => '\u{0331}',
        'h' => '\u{0309}',
        _ => return None,
    })
}

/// A LaTeX letter command (`\ss`, `\o`, `\ae`, `\dj` …).
fn letter(name: &str) -> Option<&'static str> {
    Some(match name {
        "ss" => "ß",
        "o" => "ø",
        "O" => "Ø",
        "ae" => "æ",
        "AE" => "Æ",
        "oe" => "œ",
        "OE" => "Œ",
        "aa" => "å",
        "AA" => "Å",
        "l" => "ł",
        "L" => "Ł",
        "i" => "ı",
        "j" => "ȷ",
        "dj" => "đ",
        "DJ" => "Đ",
        "dh" => "ð",
        "th" => "þ",
        "textendash" => "–",
        "textemdash" => "—",
        "textquoteright" => "’",
        "textquoteleft" => "‘",
        "dots" | "ldots" => "…",
        _ => return None,
    })
}

/// LaTeX text as plain Unicode: accent commands (`\'e`, `\"{o}`, `{\v s}`, `\c{c}`), letter
/// commands (`\ss`, `\o`, `\dj`), escaped characters (`\&`), `--` / `---` dashes, `~` as a
/// space, TeX quotes; formatting commands (`\emph{…}`, `\textit{…}`) keep their content;
/// the remaining braces are dropped.
#[must_use]
pub fn delatex(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    // The base letter an accent applies to: `e`, `{e}`, `\i`, `{\i}`.
    let base = |i: &mut usize| -> Option<String> {
        while c.get(*i) == Some(&' ') {
            *i += 1;
        }
        let mut braced = false;
        if c.get(*i) == Some(&'{') {
            braced = true;
            *i += 1;
        }
        let mut b = String::new();
        if c.get(*i) == Some(&'\\') {
            let mut j = *i + 1;
            let mut name = String::new();
            while c.get(j).is_some_and(char::is_ascii_alphabetic) {
                name.push(c[j]);
                j += 1;
            }
            // Dotless i and j take the accent as plain letters.
            b.push_str(match name.as_str() {
                "i" => "i",
                "j" => "j",
                n => letter(n)?,
            });
            *i = j;
        } else {
            b.push(*c.get(*i).filter(|x| x.is_alphabetic())?);
            *i += 1;
        }
        if braced {
            while c.get(*i).is_some_and(|x| *x != '}') {
                *i += 1;
            }
            *i += 1;
        }
        Some(b)
    };
    while i < c.len() {
        match c[i] {
            '\\' => {
                let Some(&n) = c.get(i + 1) else {
                    break;
                };
                if n.is_ascii_alphabetic() {
                    let mut j = i + 1;
                    let mut name = String::new();
                    while c.get(j).is_some_and(char::is_ascii_alphabetic) {
                        name.push(c[j]);
                        j += 1;
                    }
                    if let Some(l) = letter(&name) {
                        out.push_str(l);
                        i = j;
                        // A command swallows the space after it.
                        if c.get(i) == Some(&' ') {
                            i += 1;
                        }
                        continue;
                    }
                    let mark = if name.chars().count() == 1 {
                        name.chars().next().and_then(accent)
                    } else {
                        None
                    };
                    if let Some(mark) = mark {
                        let mut k = j;
                        if let Some(b) = base(&mut k) {
                            out.push_str(&b);
                            out.push(mark);
                            i = k;
                            continue;
                        }
                    }
                    // Another command (\emph, \textit, \url …): dropped, its argument stays.
                    i = j;
                    if c.get(i) == Some(&' ') {
                        i += 1;
                    }
                } else if let Some(mark) = accent(n) {
                    let mut k = i + 2;
                    if let Some(b) = base(&mut k) {
                        out.push_str(&b);
                        out.push(mark);
                        i = k;
                    } else {
                        out.push(n);
                        i += 2;
                    }
                } else {
                    // \& \% \_ \$ \# \{ \}
                    out.push(n);
                    i += 2;
                }
            }
            '{' | '}' => i += 1,
            '~' => {
                out.push(' ');
                i += 1;
            }
            '-' if c.get(i + 1) == Some(&'-') => {
                if c.get(i + 2) == Some(&'-') {
                    out.push('—');
                    i += 3;
                } else {
                    out.push('–');
                    i += 2;
                }
            }
            '`' if c.get(i + 1) == Some(&'`') => {
                out.push('“');
                i += 2;
            }
            '\'' if c.get(i + 1) == Some(&'\'') => {
                out.push('”');
                i += 2;
            }
            x => {
                out.push(x);
                i += 1;
            }
        }
    }
    let composed: String = out.nfc().collect();
    composed.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Split a name list on `and` outside braces.
fn split_names(v: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    let words: Vec<&str> = v.split_whitespace().collect();
    for w in words {
        if depth == 0 && w.eq_ignore_ascii_case("and") {
            out.push(std::mem::take(&mut cur));
            continue;
        }
        for ch in w.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    out.push(cur);
    out.into_iter().filter(|n| !n.trim().is_empty()).collect()
}

/// A contributor list: people, or one corporate name when the only name is in braces
/// (`author = {{World Health Organization}}`).
fn contributor(role: &str, raw: &str) -> Contributor {
    let names = split_names(strip_outer(raw));
    if let [only] = names.as_slice() {
        if wrapped(only.trim()) {
            return Contributor {
                role: role.to_owned(),
                corporate: Some(delatex(only)),
                people: Vec::new(),
            };
        }
    }
    Contributor {
        role: role.to_owned(),
        corporate: None,
        people: names
            .iter()
            .filter(|n| !n.eq_ignore_ascii_case("others"))
            .map(|n| {
                if wrapped(n.trim()) {
                    Person::new(&delatex(n), "")
                } else {
                    parse_name(&delatex(n))
                }
            })
            .collect(),
    }
}

/// Characters a tag may hold: it is written unquoted in the field instruction.
fn tag_of(key: &str) -> String {
    fold_ascii(key)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "_-:.".contains(*c))
        .collect()
}

/// `2021-05-04` / `2021/05/04` / `2021` → year, month, day.
fn split_date(v: &str) -> (String, String, String) {
    let mut it = v
        .split(['-', '/', ' '])
        .map(str::trim)
        .filter(|x| !x.is_empty());
    let y = it.next().unwrap_or_default().to_owned();
    let m = it
        .next()
        .and_then(month_number)
        .map(|m| m.to_string())
        .unwrap_or_default();
    let d = it
        .next()
        .and_then(|d| d.parse::<u32>().ok())
        .map(|d| d.to_string())
        .unwrap_or_default();
    (y, m, d)
}

/// The sources of a BibTeX text. Entry types map to the source types of §22.6.2.63;
/// keys become tags (made unique among themselves and `taken`).
#[must_use]
pub fn import(text: &str, taken: &[String]) -> Vec<Source> {
    let mut used: Vec<String> = taken.to_vec();
    let mut out = Vec::new();
    for (kind, key, fields) in entries(text) {
        let get = |n: &str| {
            fields
                .iter()
                .find(|(k, _)| k == n)
                .map(|(_, v)| delatex(v))
                .unwrap_or_default()
        };
        let raw = |n: &str| fields.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str());
        let mut s = Source::default();
        let has_url = !get("url").is_empty();
        let mut thesis = "";
        s.kind = match kind.as_str() {
            "book" | "booklet" | "mvbook" | "collection" => "Book",
            "inbook" | "incollection" | "bookinbook" => "BookSection",
            "article" => "JournalArticle",
            "inproceedings" | "conference" | "proceedings" => "ConferenceProceedings",
            "techreport" | "report" | "manual" => "Report",
            "phdthesis" => {
                thesis = "PhD thesis";
                "Report"
            }
            "mastersthesis" => {
                thesis = "Master's thesis";
                "Report"
            }
            "thesis" => {
                thesis = "Thesis";
                "Report"
            }
            "online" | "electronic" | "www" | "webpage" => "InternetSite",
            "patent" => "Patent",
            "legislation" | "legal" | "jurisdiction" => "Case",
            _ if has_url => "InternetSite",
            _ => "Misc",
        }
        .to_owned();
        for (role, field) in [
            ("Author", "author"),
            ("Editor", "editor"),
            ("Translator", "translator"),
        ] {
            if let Some(v) = raw(field) {
                let c = contributor(role, v);
                if c.corporate.is_some() || !c.people.is_empty() {
                    s.contributors.push(c);
                }
            }
        }
        s.set("Title", &get("title"));
        s.set("ShortTitle", &get("shorttitle"));
        let journal = [get("journal"), get("journaltitle")]
            .into_iter()
            .find(|x| !x.is_empty())
            .unwrap_or_default();
        s.set("JournalName", &journal);
        match s.kind.as_str() {
            "ConferenceProceedings" => s.set("ConferenceName", &get("booktitle")),
            _ => s.set("BookTitle", &get("booktitle")),
        }
        // `date` (biblatex) or `year` + `month`.
        let (mut y, mut m, d) = split_date(&get("date"));
        if !get("year").is_empty() {
            y = get("year");
        }
        if let Some(n) = month_number(&get("month")) {
            m = n.to_string();
        }
        s.set("Year", &y);
        s.set("Month", &m);
        s.set("Day", &d);
        s.set("Volume", &get("volume"));
        let number = [get("number"), get("issue")]
            .into_iter()
            .find(|x| !x.is_empty())
            .unwrap_or_default();
        s.set("Issue", &number);
        // BibTeX writes a range with two hyphens.
        s.set("Pages", &get("pages").replace('–', "-"));
        let publisher = [get("publisher"), get("organization")]
            .into_iter()
            .find(|x| !x.is_empty())
            .unwrap_or_default();
        s.set("Publisher", &publisher);
        let inst = [get("school"), get("institution")]
            .into_iter()
            .find(|x| !x.is_empty())
            .unwrap_or_default();
        s.set("Institution", &inst);
        let city = [get("address"), get("location")]
            .into_iter()
            .find(|x| !x.is_empty())
            .unwrap_or_default();
        s.set("City", &city);
        s.set("Edition", &get("edition"));
        s.set("URL", &get("url"));
        s.set("DOI", &get("doi"));
        let isn = [get("isbn"), get("issn")]
            .into_iter()
            .find(|x| !x.is_empty())
            .unwrap_or_default();
        s.set("StandardNumber", &isn);
        s.set("Comments", &get("note"));
        if !thesis.is_empty() || !get("type").is_empty() && kind.contains("thesis") {
            let ty = get("type");
            s.set("ThesisType", if ty.is_empty() { thesis } else { &ty });
        }
        if s.kind == "Case" {
            s.set("CaseNumber", &get("number"));
            s.set("Issue", "");
        }
        let (ay, am, ad) = split_date(&get("urldate"));
        s.set("YearAccessed", &ay);
        s.set("MonthAccessed", &am);
        s.set("DayAccessed", &ad);
        let lang = [get("langid"), get("language")]
            .into_iter()
            .find(|x| !x.is_empty())
            .unwrap_or_default()
            .to_lowercase();
        if lang.starts_with("vi") || lang.contains("vietnam") {
            s.set("LCID", "1066");
        } else if lang.starts_with("en") {
            s.set("LCID", "1033");
        }
        if s.kind == "InternetSite" && s.get("InternetSiteTitle").is_empty() {
            s.set("InternetSiteTitle", &get("organization"));
            if !get("organization").is_empty() && get("publisher").is_empty() {
                s.set("Publisher", "");
            }
        }
        let key = tag_of(&key);
        s.tag = if key.is_empty() {
            new_tag(&s, &used)
        } else {
            unique_tag(&key, &used)
        };
        used.push(s.tag.clone());
        out.push(s);
    }
    out
}

/// Escape the characters TeX treats specially.
fn escape(v: &str) -> String {
    let mut out = String::new();
    for c in v.chars() {
        match c {
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '\\' => out.push_str("\\textbackslash{}"),
            c => out.push(c),
        }
    }
    out
}

fn names(c: &Contributor) -> String {
    if let Some(corp) = c.corporate.as_deref().filter(|x| !x.trim().is_empty()) {
        return format!("{{{}}}", escape(corp.trim()));
    }
    c.people
        .iter()
        .map(|p| {
            let g = p.given();
            if g.is_empty() {
                format!("{{{}}}", escape(p.last.trim()))
            } else {
                format!("{}, {}", escape(p.last.trim()), escape(&g))
            }
        })
        .collect::<Vec<_>>()
        .join(" and ")
}

/// The BibTeX text of `sources` (UTF-8; placeholders are left out).
#[must_use]
pub fn export(sources: &[Source]) -> String {
    let mut out = String::new();
    for s in sources.iter().filter(|s| !s.is_placeholder()) {
        let thesis = s.get("ThesisType").to_lowercase();
        let kind = match s.kind.as_str() {
            "Book" => "book",
            "BookSection" => "incollection",
            "JournalArticle" | "ArticleInAPeriodical" => "article",
            "ConferenceProceedings" => "inproceedings",
            "Report"
                if thesis.contains("phd")
                    || thesis.contains("doctor")
                    || thesis.contains("tiến sĩ") =>
            {
                "phdthesis"
            }
            "Report" if !thesis.is_empty() => "mastersthesis",
            "Report" => "techreport",
            "InternetSite" | "DocumentFromInternetSite" => "online",
            "Patent" => "patent",
            _ => "misc",
        };
        let mut f: Vec<(&str, String)> = Vec::new();
        for (role, field) in [
            ("Author", "author"),
            ("Editor", "editor"),
            ("Translator", "translator"),
        ] {
            if let Some(c) = s.role(role) {
                f.push((field, names(c)));
            }
        }
        if s.role("Author").is_none() && s.kind == "Case" && !s.get("Court").is_empty() {
            f.push(("author", format!("{{{}}}", escape(s.get("Court")))));
        }
        let put = |f: &mut Vec<(&str, String)>, field: &'static str, v: &str| {
            if !v.is_empty() {
                f.push((field, escape(v)));
            }
        };
        put(&mut f, "title", s.get("Title"));
        put(&mut f, "journal", s.get("JournalName"));
        if s.get("JournalName").is_empty() {
            put(&mut f, "journal", s.get("PeriodicalTitle"));
        }
        put(&mut f, "booktitle", s.get("BookTitle"));
        put(&mut f, "booktitle", s.get("ConferenceName"));
        put(&mut f, "year", s.get("Year"));
        put(&mut f, "month", s.get("Month"));
        put(&mut f, "volume", s.get("Volume"));
        put(&mut f, "number", s.get("Issue"));
        put(&mut f, "number", s.get("CaseNumber"));
        put(
            &mut f,
            "pages",
            &s.get("Pages").replace(['–', '—'], "-").replace('-', "--"),
        );
        put(&mut f, "publisher", s.get("Publisher"));
        put(
            &mut f,
            if kind.ends_with("thesis") {
                "school"
            } else {
                "institution"
            },
            s.get("Institution"),
        );
        put(&mut f, "address", s.get("City"));
        put(&mut f, "edition", s.get("Edition"));
        if kind == "mastersthesis" || kind == "phdthesis" {
            put(&mut f, "type", s.get("ThesisType"));
        }
        put(&mut f, "organization", s.get("InternetSiteTitle"));
        put(&mut f, "doi", s.get("DOI"));
        if !s.get("URL").is_empty() {
            f.push(("url", s.get("URL").to_owned()));
        }
        let y = s.get("YearAccessed");
        if !y.is_empty() {
            let m = month_number(s.get("MonthAccessed"));
            let d = s.get("DayAccessed").parse::<u32>().ok();
            f.push((
                "urldate",
                match (m, d) {
                    (Some(m), Some(d)) => format!("{y}-{m:02}-{d:02}"),
                    (Some(m), None) => format!("{y}-{m:02}"),
                    _ => y.to_owned(),
                },
            ));
        }
        put(&mut f, "isbn", s.get("StandardNumber"));
        put(&mut f, "note", s.get("Comments"));
        if s.get("LCID") == "1066" {
            f.push(("langid", "vietnamese".into()));
        }
        let key = tag_of(&s.tag);
        out.push_str(&format!("@{kind}{{{key},\n"));
        for (k, v) in f {
            out.push_str(&format!("  {k} = {{{v}}},\n"));
        }
        out.push_str("}\n\n");
    }
    out
}
