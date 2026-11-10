//! Reference styles: BlueOffice's own formatters for in-text citations and bibliography
//! entries. Each style is written from its public style guide (no style files of other
//! products are read or copied):
//!
//! * **APA** — *Publication Manual of the American Psychological Association*, 7th ed. (2020),
//!   ch. 8 (in-text citations), ch. 9–10 (reference list and examples).
//! * **MLA** — *MLA Handbook*, 9th ed. (2021), ch. 5 (works-cited list), ch. 6 (in-text).
//! * **Chicago** — *The Chicago Manual of Style*, 17th ed. (2017), ch. 15 (author-date).
//! * **IEEE** — *IEEE Reference Guide* (IEEE Periodicals, 2018 and later revisions).
//! * **Harvard** — the author-date form taught as "Harvard" in *Cite Them Right* (Pears &
//!   Shields, 11th/12th ed.).
//! * **ISO 690** — ISO 690:2010 *Guidelines for bibliographic references and citations*,
//!   clauses 5–15 (elements), annex A.2 (name and date system) and A.3 (numeric system).
//! * **Vietnamese thesis** — the reference-list rules of the Ministry of Education and
//!   Training's thesis presentation guidance (hướng dẫn trình bày luận văn, luận án của Bộ
//!   GD&ĐT, mục "Tài liệu tham khảo và cách trích dẫn"), as reproduced by Vietnamese
//!   universities: numbered list, sources grouped by language (Vietnamese first), Vietnamese
//!   authors sorted by given name and written in their usual order, foreign authors by
//!   family name; citations by number in square brackets.
//!
//! Titles are kept in the capitalisation the user typed (no automatic sentence or title
//! case). Law texts (source type `Case`: issuing body in `Court`, number in `CaseNumber`)
//! are formatted as a work of the issuing body with its number — an adaptation, since the
//! English-language guides defer to national legal citation practice.

use super::names::{
    bare_doi, edition, en_dash, family_initials, initials, initials_family, inverted, is_range,
    month_number, natural, vietnamese, vietnamese_sort_name, MONTHS, MONTHS_IEEE, MONTHS_MLA,
};
use super::source::{Person, Source};
use crate::collate::vi_cmp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A reference style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StyleId {
    /// APA 7th edition (author-date).
    #[serde(rename = "APA")]
    Apa,
    /// MLA 9th edition (author-page).
    #[serde(rename = "MLA")]
    Mla,
    /// Chicago 17th edition, author-date.
    Chicago,
    /// IEEE (numbers in order of first citation).
    #[serde(rename = "IEEE")]
    Ieee,
    /// Harvard (author-date).
    Harvard,
    /// ISO 690, name and date system.
    #[serde(rename = "ISO690")]
    Iso690,
    /// ISO 690, numeric system.
    #[serde(rename = "ISO690Numeric")]
    Iso690Numeric,
    /// Vietnamese thesis style (numbered, sorted list).
    #[serde(rename = "VNThesis")]
    VnThesis,
}

impl StyleId {
    /// Every style, in the order the picker lists them.
    pub const ALL: [StyleId; 8] = [
        StyleId::Apa,
        StyleId::Mla,
        StyleId::Chicago,
        StyleId::Ieee,
        StyleId::Harvard,
        StyleId::Iso690,
        StyleId::Iso690Numeric,
        StyleId::VnThesis,
    ];

    /// The name stored in `b:Sources/@StyleName`.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            StyleId::Apa => "APA",
            StyleId::Mla => "MLA",
            StyleId::Chicago => "Chicago",
            StyleId::Ieee => "IEEE",
            StyleId::Harvard => "Harvard",
            StyleId::Iso690 => "ISO690",
            StyleId::Iso690Numeric => "ISO690Numeric",
            StyleId::VnThesis => "VNThesis",
        }
    }

    /// The style a `StyleName` means. Names written by other programs are matched loosely
    /// (`ISO 690 - Numerical Reference`, `Harvard - Anglia`, …); an unknown name is APA.
    #[must_use]
    pub fn from_name(name: &str) -> StyleId {
        let n: String = name
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        if n.starts_with("vn") || n.contains("vietnam") {
            StyleId::VnThesis
        } else if n.contains("iso690") {
            if n.contains("num") {
                StyleId::Iso690Numeric
            } else {
                StyleId::Iso690
            }
        } else if n.contains("ieee") {
            StyleId::Ieee
        } else if n.contains("mla") {
            StyleId::Mla
        } else if n.contains("chicago") || n.contains("turabian") {
            StyleId::Chicago
        } else if n.contains("harvard") {
            StyleId::Harvard
        } else {
            StyleId::Apa
        }
    }

    /// Citations are numbers (`[1]`).
    #[must_use]
    pub fn numeric(self) -> bool {
        matches!(
            self,
            StyleId::Ieee | StyleId::Iso690Numeric | StyleId::VnThesis
        )
    }

    /// Citations show the year (a letter tells same-author same-year works apart).
    fn author_date(self) -> bool {
        matches!(
            self,
            StyleId::Apa | StyleId::Chicago | StyleId::Harvard | StyleId::Iso690
        )
    }
}

/// A stretch of formatted text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Span {
    /// The text.
    pub text: String,
    /// Italic.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
}

/// Text with italic stretches, built piece by piece.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rich(pub Vec<Span>);

impl Rich {
    fn push(&mut self, text: &str, italic: bool) {
        if text.is_empty() {
            return;
        }
        match self.0.last_mut() {
            Some(l) if l.italic == italic => l.text.push_str(text),
            _ => self.0.push(Span {
                text: text.to_owned(),
                italic,
            }),
        }
    }

    /// Append plain text.
    fn t(&mut self, text: &str) -> &mut Rich {
        self.push(text, false);
        self
    }

    /// Append italic text.
    fn i(&mut self, text: &str) -> &mut Rich {
        self.push(text, true);
        self
    }

    fn last_char(&self) -> Option<char> {
        self.0.last().and_then(|s| s.text.chars().next_back())
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Append a separator (`". "`, `", "`): nothing at the start; a separator that begins
    /// with the punctuation the text already ends with is not doubled (`Inc.` + `. `).
    fn sep(&mut self, s: &str) -> &mut Rich {
        if self.is_empty() {
            return self;
        }
        let first = s.chars().next();
        let last = self.last_char();
        let doubled = match (last, first) {
            (Some('.' | '?' | '!'), Some('.')) => true,
            (Some(a), Some(b)) => a == b && matches!(a, ',' | ';' | ':'),
            _ => false,
        };
        if doubled {
            self.t(&s[first.map_or(0, char::len_utf8)..])
        } else {
            self.t(s)
        }
    }

    /// End the entry with a full stop (unless it already ends with one).
    fn stop(&mut self) -> &mut Rich {
        if !self.is_empty() && !matches!(self.last_char(), Some('.' | '?' | '!')) {
            self.t(".");
        }
        self
    }

    /// The text without formatting.
    #[must_use]
    pub fn plain(&self) -> String {
        self.0.iter().map(|s| s.text.as_str()).collect()
    }

    /// The text with italic stretches between asterisks (for tests and previews).
    #[must_use]
    pub fn marked(&self) -> String {
        self.0
            .iter()
            .map(|s| {
                if s.italic {
                    format!("*{}*", s.text)
                } else {
                    s.text.clone()
                }
            })
            .collect()
    }
}

/// One source of a citation (ECMA-376 Part 1, §17.16.5.8 CITATION switches).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CiteItem {
    /// Source tag.
    pub tag: String,
    /// `\p`: pages.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub pages: String,
    /// `\v`: volume.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub volume: String,
    /// `\f`: text before the citation.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    /// `\s`: text after the citation.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub suffix: String,
    /// `\n`: leave the author out.
    pub no_author: bool,
    /// `\y`: leave the year out.
    pub no_year: bool,
    /// `\t`: leave the title out.
    pub no_title: bool,
}

/// A citation: one or more sources (`\m` adds a source) and a language (`\l`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Citation {
    /// The cited sources in order.
    pub items: Vec<CiteItem>,
    /// `\l`: language (LCID) of the words the style adds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lcid: Option<String>,
}

fn quote_arg(s: &str) -> String {
    // A quote cannot appear inside a quoted field argument (§17.16.1).
    format!("\"{}\"", s.replace('"', "'"))
}

impl Citation {
    /// Parse a `CITATION` instruction (§17.16.5.8): `CITATION tag [\l lcid] [\p pages]
    /// [\v volume] [\f prefix] [\s suffix] [\n] [\y] [\t] [\m tag …]`. The switches after
    /// `\m tag` belong to that source. `None` for another field.
    #[must_use]
    pub fn parse(instr: &str) -> Option<Citation> {
        let w = bo_model::field::words(instr);
        if !w.first()?.eq_ignore_ascii_case("CITATION") {
            return None;
        }
        let mut c = Citation::default();
        let mut i = 1;
        while i < w.len() {
            let word = &w[i];
            let sw = word.to_ascii_lowercase();
            let arg = if matches!(sw.as_str(), "\\l" | "\\m" | "\\p" | "\\v" | "\\f" | "\\s") {
                i += 1;
                w.get(i).cloned().unwrap_or_default()
            } else {
                String::new()
            };
            match sw.as_str() {
                "\\l" => c.lcid = Some(arg).filter(|a| !a.is_empty()),
                "\\m" => c.items.push(CiteItem {
                    tag: arg,
                    ..CiteItem::default()
                }),
                x if x.starts_with('\\') => {
                    if let Some(it) = c.items.last_mut() {
                        match x {
                            "\\p" => it.pages = arg,
                            "\\v" => it.volume = arg,
                            "\\f" => it.prefix = arg,
                            "\\s" => it.suffix = arg,
                            "\\n" => it.no_author = true,
                            "\\y" => it.no_year = true,
                            "\\t" => it.no_title = true,
                            _ => {}
                        }
                    }
                }
                _ if c.items.is_empty() => c.items.push(CiteItem {
                    tag: word.clone(),
                    ..CiteItem::default()
                }),
                _ => {}
            }
            i += 1;
        }
        c.items.retain(|it| !it.tag.trim().is_empty());
        (!c.items.is_empty()).then_some(c)
    }

    /// The field instruction.
    #[must_use]
    pub fn instr(&self) -> String {
        let mut out = String::from("CITATION");
        for (k, it) in self.items.iter().enumerate() {
            if k > 0 {
                out.push_str(" \\m");
            }
            out.push(' ');
            out.push_str(it.tag.trim());
            if k == 0 {
                if let Some(l) = self.lcid.as_deref().filter(|l| !l.trim().is_empty()) {
                    out.push_str(" \\l ");
                    out.push_str(l.trim());
                }
            }
            for (sw, v) in [
                ("\\p", &it.pages),
                ("\\v", &it.volume),
                ("\\f", &it.prefix),
                ("\\s", &it.suffix),
            ] {
                if !v.is_empty() {
                    out.push(' ');
                    out.push_str(sw);
                    out.push(' ');
                    out.push_str(&quote_arg(v));
                }
            }
            for (sw, on) in [
                ("\\n", it.no_author),
                ("\\y", it.no_year),
                ("\\t", it.no_title),
            ] {
                if on {
                    out.push(' ');
                    out.push_str(sw);
                }
            }
        }
        out
    }
}

/// The words a style adds, in the language of the citation (`\l`).
struct Terms {
    and: &'static str,
    amp: &'static str,
    et_al: &'static str,
    p: &'static str,
    pp: &'static str,
    nd: &'static str,
    no_date: &'static str,
}

const EN: Terms = Terms {
    and: "and",
    amp: "&",
    et_al: "et al.",
    p: "p.",
    pp: "pp.",
    nd: "n.d.",
    no_date: "no date",
};

/// Vietnamese: "và", "và cộng sự", "tr." (trang), "k.n." is not established — "không ngày".
const VI: Terms = Terms {
    and: "và",
    amp: "và",
    et_al: "và cộng sự",
    p: "tr.",
    pp: "tr.",
    nd: "không ngày",
    no_date: "không ngày",
};

fn terms(lcid: Option<&str>) -> &'static Terms {
    match lcid.map(str::trim) {
        Some(l)
            if l == "1066" || l.eq_ignore_ascii_case("vi-VN") || l.eq_ignore_ascii_case("vi") =>
        {
            &VI
        }
        _ => &EN,
    }
}

/// What kind of work a source is for the styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Book,
    Chapter,
    Article,
    Conference,
    Report,
    Thesis,
    Web,
    Legal,
}

fn class(s: &Source) -> Class {
    match s.kind.as_str() {
        "BookSection" => Class::Chapter,
        "JournalArticle" | "ArticleInAPeriodical" => Class::Article,
        "ConferenceProceedings" => Class::Conference,
        // §22.6.2.65 ThesisType: a report that is a thesis.
        "Report" if !s.get("ThesisType").is_empty() => Class::Thesis,
        "Report" => Class::Report,
        "InternetSite" | "DocumentFromInternetSite" => Class::Web,
        "Case" => Class::Legal,
        // Art, film, patent, … : the elements of a book (creator, year, title, publisher).
        _ => Class::Book,
    }
}

/// Who a work is by.
enum By<'a> {
    People(&'a [Person]),
    Body(&'a str),
    Nobody,
}

fn by(s: &Source) -> By<'_> {
    if let Some(a) = s.role("Author") {
        if let Some(c) = a
            .corporate
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            return By::Body(c);
        }
        return By::People(&a.people);
    }
    // A law text is the work of the body that issued it.
    if class(s) == Class::Legal && !s.get("Court").is_empty() {
        return By::Body(s.get("Court"));
    }
    By::Nobody
}

fn editors(s: &Source) -> &[Person] {
    s.role("Editor")
        .or_else(|| s.role("BookAuthor"))
        .map_or(&[], |c| c.people.as_slice())
}

fn container(s: &Source) -> &str {
    for k in [
        "JournalName",
        "PeriodicalTitle",
        "BookTitle",
        "ConferenceName",
        "InternetSiteTitle",
    ] {
        if !s.get(k).is_empty() {
            return s.get(k);
        }
    }
    ""
}

fn publisher(s: &Source) -> &str {
    for k in [
        "Publisher",
        "Institution",
        "ProductionCompany",
        "Distributor",
    ] {
        if !s.get(k).is_empty() {
            return s.get(k);
        }
    }
    ""
}

/// `City: Publisher`, or the one there is.
fn place_publisher(s: &Source) -> String {
    match (s.get("City"), publisher(s)) {
        ("", p) => p.to_owned(),
        (c, "") => c.to_owned(),
        (c, p) => format!("{c}: {p}"),
    }
}

/// Join names: `a`, `a{two}b`, `a, b{last}c`.
fn join(names: &[String], two: &str, last: &str) -> String {
    match names {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a}{two}{b}"),
        [head @ .., z] => format!("{}{last}{z}", head.join(", ")),
    }
}

fn family(p: &Person) -> String {
    if p.last.trim().is_empty() {
        p.given()
    } else {
        p.last.trim().to_owned()
    }
}

/// `pp. 45–67` / `p. 5` (page label of the language).
fn page_label(pages: &str, t: &Terms) -> String {
    let p = en_dash(pages);
    if p.is_empty() {
        String::new()
    } else if is_range(&p) {
        format!("{} {p}", t.pp)
    } else {
        format!("{} {p}", t.p)
    }
}

/// Day, month number and year of a date of the source (`prefix` = `""` or `"Accessed"`…):
/// elements `Year` / `Month` / `Day` or `YearAccessed` / `MonthAccessed` / `DayAccessed`.
fn date_of(s: &Source, accessed: bool) -> (Option<u32>, Option<u32>, String) {
    let (y, m, d) = if accessed {
        ("YearAccessed", "MonthAccessed", "DayAccessed")
    } else {
        ("Year", "Month", "Day")
    };
    let month = month_number(s.get(m));
    let day = s
        .get(d)
        .parse::<u32>()
        .ok()
        .filter(|d| (1..=31).contains(d));
    (day.filter(|_| month.is_some()), month, s.get(y).to_owned())
}

/// `4 May 2023` / `May 2023` / `2023` with the month names of `months`.
fn day_month_year(date: &(Option<u32>, Option<u32>, String), months: &[&str; 12]) -> String {
    let (d, m, y) = date;
    let mut parts: Vec<String> = Vec::new();
    if let Some(d) = d {
        parts.push(d.to_string());
    }
    if let Some(m) = m {
        parts.push(months[(*m as usize - 1).min(11)].to_owned());
    }
    if !y.is_empty() {
        parts.push(y.clone());
    }
    parts.join(" ")
}

/// `May 4, 2023` / `May 2023` / `2023`.
fn month_day_year(date: &(Option<u32>, Option<u32>, String), months: &[&str; 12]) -> String {
    let (d, m, y) = date;
    match (m, d) {
        (Some(m), Some(d)) => format!("{} {d}, {y}", months[(*m as usize - 1).min(11)]),
        (Some(m), None) => format!("{} {y}", months[(*m as usize - 1).min(11)])
            .trim()
            .to_owned(),
        _ => y.clone(),
    }
}

/// `04/05/2023` (Vietnamese numeric date) or what there is of it.
fn vn_date(date: &(Option<u32>, Option<u32>, String)) -> String {
    let (d, m, y) = date;
    match (d, m) {
        (Some(d), Some(m)) => format!("{d:02}/{m:02}/{y}"),
        (None, Some(m)) => format!("{m:02}/{y}"),
        _ => y.clone(),
    }
}

/// The web address of a source: its DOI as a resolver link, else its URL.
fn link(s: &Source) -> String {
    let doi = bare_doi(s.get("DOI"));
    if doi.is_empty() {
        s.get("URL").to_owned()
    } else {
        format!("https://doi.org/{doi}")
    }
}

/// The title without a leading article (for sorting works without an author).
fn sort_title(s: &Source) -> String {
    let t = s.get("Title").to_lowercase();
    for a in ["a ", "an ", "the "] {
        if let Some(rest) = t.strip_prefix(a) {
            return rest.to_owned();
        }
    }
    t
}

/// One bibliography entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// Tag of the source.
    pub tag: String,
    /// The number label of numeric styles (`[1]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The formatted reference.
    pub spans: Vec<Span>,
}

impl Entry {
    /// The entry with italics between asterisks, after its label (`[1] …`).
    #[must_use]
    pub fn marked(&self) -> String {
        let body = Rich(self.spans.clone()).marked();
        match &self.label {
            Some(l) => format!("{l} {body}"),
            None => body,
        }
    }
}

/// Formats the citations and the bibliography of one document in one style.
pub struct Formatter<'a> {
    /// The style.
    pub style: StyleId,
    sources: &'a [Source],
    /// Bibliography order: indexes into `sources`.
    order: Vec<usize>,
    /// Lower-cased tag → index into `sources`.
    by_tag: HashMap<String, usize>,
    /// Source index → number in the bibliography (1-based).
    number: HashMap<usize, usize>,
    /// Source index → letter after the year.
    letter: HashMap<usize, char>,
}

impl<'a> Formatter<'a> {
    /// A formatter for `sources`; `cited` lists the tags of the document's citations in
    /// order of appearance (numeric styles that number by first citation use it).
    #[must_use]
    pub fn new(style: StyleId, sources: &'a [Source], cited: &[String]) -> Formatter<'a> {
        let mut by_tag = HashMap::new();
        for (i, s) in sources.iter().enumerate() {
            by_tag.entry(s.tag.trim().to_lowercase()).or_insert(i);
        }
        let listed: Vec<usize> = (0..sources.len())
            .filter(|&i| !sources[i].is_placeholder())
            .collect();
        let mut order: Vec<usize> = Vec::new();
        match style {
            // IEEE Reference Guide: "references are numbered in the order they are first
            // cited"; ISO 690:2010 A.3: numbered "in the order in which they are first cited".
            // Sources not cited follow in list order.
            StyleId::Ieee | StyleId::Iso690Numeric => {
                for t in cited {
                    if let Some(&i) = by_tag.get(&t.trim().to_lowercase()) {
                        if listed.contains(&i) && !order.contains(&i) {
                            order.push(i);
                        }
                    }
                }
                for &i in &listed {
                    if !order.contains(&i) {
                        order.push(i);
                    }
                }
            }
            // Bộ GD&ĐT: grouped by language, Vietnamese first; Vietnamese authors in
            // alphabetical order of their given name, foreign authors of their family name;
            // works without an author by the first word of the issuing body or title.
            StyleId::VnThesis => {
                order = listed;
                order.sort_by(|&a, &b| {
                    let (sa, sb) = (&sources[a], &sources[b]);
                    sb.is_vietnamese()
                        .cmp(&sa.is_vietnamese())
                        .then_with(|| vi_cmp(&vn_sort_key(sa), &vn_sort_key(sb)))
                        .then_with(|| sa.get("Year").cmp(sb.get("Year")))
                        .then_with(|| vi_cmp(&sort_title(sa), &sort_title(sb)))
                });
            }
            // Alphabetical by author (title when there is none), then year, then title
            // (APA 9.44–9.48, MLA 5.123, Chicago 15.11, ISO 690 A.2).
            _ => {
                order = listed;
                order.sort_by(|&a, &b| {
                    let (sa, sb) = (&sources[a], &sources[b]);
                    vi_cmp(&author_sort_key(sa), &author_sort_key(sb))
                        .then_with(|| sa.get("Year").cmp(sb.get("Year")))
                        .then_with(|| vi_cmp(&sort_title(sa), &sort_title(sb)))
                });
            }
        }
        let number = order.iter().enumerate().map(|(n, &i)| (i, n + 1)).collect();
        // Same author(s) and year: a, b, c … after the year, in bibliography order
        // (APA 8.19, Chicago 15.20, Cite Them Right "same author, same year").
        let mut letter = HashMap::new();
        if style.author_date() {
            let key = |i: usize| {
                let s = &sources[i];
                (author_sort_key(s), s.get("Year").to_owned())
            };
            let mut k = 0;
            while k < order.len() {
                let mut e = k + 1;
                while e < order.len() && key(order[e]) == key(order[k]) {
                    e += 1;
                }
                if e - k > 1 && !sources[order[k]].get("Year").is_empty() {
                    for (n, &i) in order[k..e].iter().enumerate() {
                        letter.insert(i, (b'a' + (n as u8).min(25)) as char);
                    }
                }
                k = e;
            }
        }
        Formatter {
            style,
            sources,
            order,
            by_tag,
            number,
            letter,
        }
    }

    fn find(&self, tag: &str) -> Option<usize> {
        self.by_tag.get(&tag.trim().to_lowercase()).copied()
    }

    fn year(&self, i: usize, none: &str) -> String {
        let y = self.sources[i].get("Year");
        if y.is_empty() {
            none.to_owned()
        } else {
            match self.letter.get(&i) {
                Some(l) => format!("{y}{l}"),
                None => y.to_owned(),
            }
        }
    }

    // ------------------------------------------------------------------ citations

    /// The text of a citation.
    #[must_use]
    pub fn citation(&self, c: &Citation) -> String {
        let t = terms(c.lcid.as_deref());
        if self.style.numeric() {
            return self.numeric_citation(c, t);
        }
        let parts: Vec<String> = c
            .items
            .iter()
            .map(|it| {
                let core = match self.find(&it.tag) {
                    Some(i) if !self.sources[i].is_placeholder() => self.author_date(i, it, t),
                    // A placeholder (or a tag without source) shows its tag.
                    _ => it.tag.trim().to_owned(),
                };
                format!("{}{core}{}", it.prefix, it.suffix)
            })
            .collect();
        format!("({})", parts.join("; "))
    }

    /// `[3]`, `[3, p. 12]`; several sources: `[3], [5]`.
    fn numeric_citation(&self, c: &Citation, t: &Terms) -> String {
        c.items
            .iter()
            .map(|it| {
                let found = self.find(&it.tag);
                let n = found
                    .and_then(|i| self.number.get(&i))
                    .map_or_else(|| it.tag.trim().to_owned(), ToString::to_string);
                let mut loc: Vec<String> = Vec::new();
                if !it.volume.is_empty() {
                    loc.push(format!("vol. {}", it.volume.trim()));
                }
                if !it.pages.is_empty() {
                    loc.push(if self.style == StyleId::VnThesis {
                        // Bộ GD&ĐT: "[15, tr. 314-315]" — "tr." for a Vietnamese source,
                        // "pp." / "p." for a foreign one; the range keeps its hyphen.
                        let vi = found.is_none_or(|i| self.sources[i].is_vietnamese());
                        let p = it.pages.trim();
                        match (vi, is_range(p)) {
                            (true, _) => format!("tr. {p}"),
                            (false, true) => format!("pp. {p}"),
                            (false, false) => format!("p. {p}"),
                        }
                    } else {
                        page_label(&it.pages, t)
                    });
                }
                let inner = std::iter::once(n).chain(loc).collect::<Vec<_>>().join(", ");
                format!("[{}{inner}{}]", it.prefix, it.suffix)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Author and year (author and page for MLA) of one cited source, without brackets.
    fn author_date(&self, i: usize, it: &CiteItem, t: &Terms) -> String {
        let s = &self.sources[i];
        let style = self.style;
        // --- who
        let who = if it.no_author {
            String::new()
        } else {
            match by(s) {
                By::People(p) => {
                    let n: Vec<String> = p.iter().map(family).collect();
                    let et_al = || format!("{} {}", n[0], t.et_al);
                    match style {
                        // APA 8.17: one or two authors are always named; three or more:
                        // the first and "et al."; two are joined by "&" in parentheses.
                        StyleId::Apa => match n.len() {
                            0 => String::new(),
                            1 => n[0].clone(),
                            2 => format!("{} {} {}", n[0], t.amp, n[1]),
                            _ => et_al(),
                        },
                        // MLA 6.5 / ISO 690 A.2: two authors "A and B"; three or more
                        // "A et al.".
                        StyleId::Mla | StyleId::Iso690 => match n.len() {
                            0 => String::new(),
                            1 => n[0].clone(),
                            2 => format!("{} {} {}", n[0], t.and, n[1]),
                            _ => et_al(),
                        },
                        // Chicago 15.29: up to three authors named ("A, B, and C"); four
                        // or more "A et al.".
                        StyleId::Chicago => match n.len() {
                            0 => String::new(),
                            1..=3 => join(&n, &format!(" {} ", t.and), &format!(", {} ", t.and)),
                            _ => et_al(),
                        },
                        // Cite Them Right: up to three authors named ("A, B and C"); four
                        // or more "A et al.".
                        _ => match n.len() {
                            0 => String::new(),
                            1..=3 => {
                                let and = format!(" {} ", t.and);
                                join(&n, &and, &and)
                            }
                            _ => et_al(),
                        },
                    }
                }
                By::Body(b) => b.to_owned(),
                // No author: the title stands in (APA 8.14, MLA 6.9, Chicago 15.34).
                By::Nobody if it.no_title => String::new(),
                By::Nobody => {
                    let short = s.get("ShortTitle");
                    if short.is_empty() {
                        s.get("Title").to_owned()
                    } else {
                        short.to_owned()
                    }
                }
            }
        };
        // --- when
        let when = if it.no_year || style == StyleId::Mla {
            String::new()
        } else {
            self.year(
                i,
                if style == StyleId::Harvard {
                    t.no_date
                } else {
                    t.nd
                },
            )
        };
        // --- where in the work
        let pages = en_dash(&it.pages);
        let vol = it.volume.trim();
        let locator = match style {
            // Chicago 15.23: "(Smith 2020, 5)"; volume and page "2:5". MLA 6.16: "(Smith 5)",
            // "(Smith 2: 5)", a volume alone "vol. 2".
            StyleId::Chicago | StyleId::Mla => match (vol, pages.as_str()) {
                ("", p) => p.to_owned(),
                (v, "") => format!("vol. {v}"),
                (v, p) if style == StyleId::Mla => format!("{v}: {p}"),
                (v, p) => format!("{v}:{p}"),
            },
            // APA 8.13 / Harvard / ISO 690: "p. 5", "pp. 5–7".
            _ => {
                let mut l: Vec<String> = Vec::new();
                if !vol.is_empty() {
                    l.push(format!("Vol. {vol}"));
                }
                if !pages.is_empty() {
                    l.push(page_label(&pages, t));
                }
                l.join(", ")
            }
        };
        let mut out = who;
        let add = |out: &mut String, sep: &str, part: &str| {
            if !part.is_empty() {
                if !out.is_empty() {
                    out.push_str(sep);
                }
                out.push_str(part);
            }
        };
        match style {
            // (Smith, 2020, p. 5)
            StyleId::Apa | StyleId::Harvard => {
                add(&mut out, ", ", &when);
                add(&mut out, ", ", &locator);
            }
            // (Smith 5)
            StyleId::Mla => add(&mut out, " ", &locator),
            // (Smith 2020, 5) / (Smith 2020, p. 5)
            _ => {
                add(&mut out, " ", &when);
                add(&mut out, ", ", &locator);
            }
        }
        if out.is_empty() {
            s.tag.clone()
        } else {
            out
        }
    }

    // ------------------------------------------------------------------ bibliography

    /// The bibliography: every source that is not a placeholder, in the order of the style.
    /// `lcid` is the language of the words the style adds (`BIBLIOGRAPHY \l`).
    #[must_use]
    pub fn bibliography(&self, lcid: Option<&str>) -> Vec<Entry> {
        let t = terms(lcid);
        self.order
            .iter()
            .enumerate()
            .map(|(n, &i)| {
                let s = &self.sources[i];
                let rich = match self.style {
                    StyleId::Apa => self.apa(i, t),
                    StyleId::Mla => mla(s, t),
                    StyleId::Chicago => self.chicago(i, t),
                    StyleId::Ieee => ieee(s, t),
                    StyleId::Harvard => self.harvard(i, t),
                    StyleId::Iso690 => self.iso690(i, t, false),
                    StyleId::Iso690Numeric => self.iso690(i, t, true),
                    StyleId::VnThesis => vn_thesis(s),
                };
                Entry {
                    tag: s.tag.clone(),
                    label: self.style.numeric().then(|| format!("[{}]", n + 1)),
                    spans: rich.0,
                }
            })
            .collect()
    }

    /// The entry of one source (by tag) as the bibliography shows it.
    #[must_use]
    pub fn entry(&self, tag: &str, lcid: Option<&str>) -> Option<Entry> {
        let i = self.find(tag)?;
        let n = self.order.iter().position(|&x| x == i)?;
        self.bibliography(lcid).into_iter().nth(n)
    }

    /// APA 7th edition reference (ch. 9 elements: author, date, title, source; ch. 10 examples).
    fn apa(&self, i: usize, t: &Terms) -> Rich {
        let s = &self.sources[i];
        let c = class(s);
        let mut r = Rich::default();
        // 9.8: family name and initials; up to 20 authors, "&" before the last; 21 or
        // more: the first 19, an ellipsis, the last.
        let who = match by(s) {
            By::People(p) => {
                let n: Vec<String> = p.iter().map(|x| family_initials(x, " ")).collect();
                if n.len() > 20 {
                    format!("{}, . . . {}", n[..19].join(", "), n[n.len() - 1])
                } else {
                    let amp = format!(", {} ", t.amp);
                    join(&n, &amp, &amp)
                }
            }
            By::Body(b) => b.to_owned(),
            By::Nobody => String::new(),
        };
        // 9.14: year in parentheses; a fuller date for works published more often
        // ("2023, May 4"); "n.d." without a date.
        let d = date_of(s, false);
        let mut date = self.year(i, t.nd);
        if matches!(c, Class::Web) || s.kind == "ArticleInAPeriodical" {
            if let (Some(m), false) = (d.1, d.2.is_empty()) {
                date.push_str(", ");
                date.push_str(MONTHS[m as usize - 1]);
                if let Some(day) = d.0 {
                    date.push_str(&format!(" {day}"));
                }
            }
        }
        // 9.19: the title of a work that stands alone is italic; of a part of a greater
        // whole, plain. Bracketed descriptions and edition follow the title.
        let stands_alone = !matches!(c, Class::Chapter | Class::Article | Class::Conference);
        let title = |r: &mut Rich| {
            if stands_alone {
                r.i(s.get("Title"));
            } else {
                r.t(s.get("Title"));
            }
            match c {
                Class::Book | Class::Report => {
                    let e = edition(s.get("Edition"), "ed.");
                    if !e.is_empty() {
                        r.t(&format!(" ({e})"));
                    }
                }
                // 10.6: "[Doctoral dissertation, University]".
                Class::Thesis => {
                    let inst = publisher(s);
                    let ty = s.get("ThesisType");
                    r.t(&match inst {
                        "" => format!(" [{ty}]"),
                        inst => format!(" [{ty}, {inst}]"),
                    });
                }
                Class::Legal if !s.get("CaseNumber").is_empty() => {
                    r.t(&format!(" ({})", s.get("CaseNumber")));
                }
                _ => {}
            }
        };
        // 9.12: without an author the title moves to the author position.
        if who.is_empty() {
            title(&mut r);
            r.stop().t(&format!(" ({date})."));
        } else {
            r.t(&who).stop().t(&format!(" ({date})."));
            r.t(" ");
            title(&mut r);
            r.stop();
        }
        let same_as_author = |x: &str| x.eq_ignore_ascii_case(&who);
        match c {
            // 10.3: In E. Editor (Ed.), *Book title* (pp. xx–xx). Publisher.
            Class::Chapter | Class::Conference => {
                r.t(" In ");
                let eds: Vec<String> = editors(s).iter().map(|x| initials_family(x, " ")).collect();
                if !eds.is_empty() {
                    let two = format!(" {} ", t.amp);
                    let last = format!(", {} ", t.amp);
                    r.t(&join(&eds, &two, &last));
                    r.t(if eds.len() > 1 {
                        " (Eds.), "
                    } else {
                        " (Ed.), "
                    });
                }
                r.i(container(s));
                let p = page_label(s.get("Pages"), t);
                if !p.is_empty() {
                    r.t(&format!(" ({p})"));
                }
                r.stop();
                if !publisher(s).is_empty() {
                    r.t(" ").t(publisher(s)).stop();
                }
            }
            // 10.1: *Journal*, *volume*(issue), pages.
            Class::Article => {
                if !container(s).is_empty() {
                    r.t(" ").i(container(s));
                    let (v, n) = (s.get("Volume"), s.get("Issue"));
                    if !v.is_empty() {
                        r.t(", ").i(v);
                    } else if !n.is_empty() {
                        r.t(", ");
                    }
                    if !n.is_empty() {
                        r.t(&format!("({n})"));
                    }
                    let p = en_dash(s.get("Pages"));
                    if !p.is_empty() {
                        r.t(", ").t(&p);
                    }
                    r.stop();
                }
            }
            // 10.16: the site name follows the title unless it is the author.
            Class::Web => {
                let site = container(s);
                if !site.is_empty() && !same_as_author(site) {
                    r.t(" ").t(site).stop();
                }
            }
            // 9.29: the publisher (no place); left out when it is the author.
            _ => {
                let p = publisher(s);
                if !p.is_empty() && !same_as_author(p) && c != Class::Thesis {
                    r.t(" ").t(p).stop();
                }
            }
        }
        // 9.34–9.35: DOI or URL last, as a link, without a full stop after it.
        let l = link(s);
        if !l.is_empty() {
            r.t(" ").t(&l);
        }
        r
    }

    /// Chicago 17th edition, author-date reference list entry (15.5–15.9 and ch. 14 forms
    /// with the year moved after the author).
    fn chicago(&self, i: usize, t: &Terms) -> Rich {
        let s = &self.sources[i];
        let c = class(s);
        let mut r = Rich::default();
        // 15.9 / 14.76: the first author inverted, the others in normal order; up to ten
        // listed, more than ten: the first seven and "et al.".
        let who = match by(s) {
            By::People(p) => {
                let mut n: Vec<String> = p
                    .iter()
                    .enumerate()
                    .map(|(k, x)| if k == 0 { inverted(x) } else { natural(x) })
                    .collect();
                if n.len() > 10 {
                    n.truncate(7);
                    format!("{}, {}", n.join(", "), t.et_al)
                } else {
                    let and = format!(", {} ", t.and);
                    join(&n, &and, &and)
                }
            }
            By::Body(b) => b.to_owned(),
            By::Nobody => String::new(),
        };
        let quoted = matches!(
            c,
            Class::Chapter | Class::Article | Class::Conference | Class::Web | Class::Thesis
        );
        let title = |r: &mut Rich| {
            if quoted {
                r.t(&quote(s.get("Title"), "."));
            } else {
                r.i(s.get("Title")).stop();
            }
        };
        let year = self.year(i, t.nd);
        if who.is_empty() {
            title(&mut r);
            r.t(&format!(" {year}."));
        } else {
            r.t(&who).stop().t(&format!(" {year}. "));
            title(&mut r);
        }
        let pp = place_publisher(s);
        match c {
            // 14.106: "In *Book*, edited by Ed Itor, 45–67. Place: Publisher."
            Class::Chapter | Class::Conference => {
                r.t(" In ").i(container(s));
                let eds: Vec<String> = editors(s).iter().map(natural).collect();
                if !eds.is_empty() {
                    let two = format!(" {} ", t.and);
                    let last = format!(", {} ", t.and);
                    r.t(", edited by ").t(&join(&eds, &two, &last));
                }
                let p = en_dash(s.get("Pages"));
                if !p.is_empty() {
                    r.t(", ").t(&p);
                }
                r.stop();
                if !pp.is_empty() {
                    r.t(" ").t(&pp).stop();
                }
            }
            // 15.9: *Journal* 12 (3): 100–120.
            Class::Article => {
                if !container(s).is_empty() {
                    r.t(" ").i(container(s));
                    let (v, n) = (s.get("Volume"), s.get("Issue"));
                    if !v.is_empty() {
                        r.t(" ").t(v);
                    }
                    if !n.is_empty() {
                        r.t(&if v.is_empty() {
                            format!(", no. {n}")
                        } else {
                            format!(" ({n})")
                        });
                    }
                    let p = en_dash(s.get("Pages"));
                    if !p.is_empty() {
                        r.t(": ").t(&p);
                    }
                    r.stop();
                }
            }
            // 14.215: "PhD diss., University".
            Class::Thesis => {
                let parts: Vec<&str> = [s.get("ThesisType"), publisher(s)]
                    .into_iter()
                    .filter(|x| !x.is_empty())
                    .collect();
                if !parts.is_empty() {
                    r.t(" ").t(&parts.join(", ")).stop();
                }
            }
            // 14.207: site name, then the date of publication or, lacking one, of access.
            Class::Web => {
                if !container(s).is_empty() {
                    r.t(" ").t(container(s)).stop();
                }
                let d = date_of(s, false);
                let a = date_of(s, true);
                if d.1.is_some() {
                    r.t(" ").t(&month_day_year(&d, &MONTHS)).stop();
                } else if !a.2.is_empty() && d.2.is_empty() {
                    r.t(" Accessed ").t(&month_day_year(&a, &MONTHS)).stop();
                }
            }
            Class::Legal => {
                if !s.get("CaseNumber").is_empty() {
                    r.t(" ").t(s.get("CaseNumber")).stop();
                }
            }
            // 14.100: edition, then "Place: Publisher".
            _ => {
                let e = edition(s.get("Edition"), "ed.");
                if !e.is_empty() {
                    r.t(" ").t(&e).stop();
                }
                if !pp.is_empty() {
                    r.t(" ").t(&pp).stop();
                }
            }
        }
        // 14.6–14.8: a DOI or URL ends the entry, followed by a full stop.
        let l = link(s);
        if !l.is_empty() {
            r.t(" ").t(&l).t(".");
        }
        r
    }

    /// Harvard reference (Cite Them Right forms).
    fn harvard(&self, i: usize, t: &Terms) -> Rich {
        let s = &self.sources[i];
        let c = class(s);
        let mut r = Rich::default();
        let and = format!(" {} ", t.and);
        // Family name, then initials without spaces; "and" before the last author.
        let who = match by(s) {
            By::People(p) => {
                let n: Vec<String> = p.iter().map(|x| family_initials(x, "")).collect();
                join(&n, &and, &and)
            }
            By::Body(b) => b.to_owned(),
            By::Nobody => String::new(),
        };
        let year = self.year(i, t.no_date);
        let part = matches!(c, Class::Chapter | Class::Article | Class::Conference);
        // Without an author the title comes first, in italics.
        if who.is_empty() {
            r.i(s.get("Title")).t(&format!(" ({year})"));
        } else {
            r.t(&who).t(&format!(" ({year}) "));
            if part {
                // The title of a part is in single quotation marks, followed by a comma.
                r.t(&format!("‘{}’", s.get("Title")));
            } else {
                r.i(s.get("Title"));
            }
        }
        let pp = place_publisher(s);
        let pages = page_label(s.get("Pages"), t);
        match c {
            // 'Chapter', in Editor, E. (ed.) *Book*. Place: Publisher, pp. 45–67.
            Class::Chapter => {
                r.sep(", ").t("in ");
                let eds: Vec<String> = editors(s).iter().map(|x| family_initials(x, "")).collect();
                if !eds.is_empty() {
                    r.t(&join(&eds, &and, &and));
                    r.t(if eds.len() > 1 { " (eds.) " } else { " (ed.) " });
                }
                r.i(container(s)).stop();
                if !pp.is_empty() {
                    r.t(" ").t(&pp);
                }
                if !pages.is_empty() {
                    r.sep(", ").t(&pages);
                }
                r.stop();
            }
            // 'Paper', *Conference title*. Place: Publisher, pp. 10–15.
            Class::Conference => {
                r.sep(", ").i(container(s)).stop();
                if !pp.is_empty() {
                    r.t(" ").t(&pp);
                }
                if !pages.is_empty() {
                    r.sep(", ").t(&pages);
                }
                r.stop();
            }
            // 'Article', *Journal*, 12(3), pp. 100–120.
            Class::Article => {
                r.sep(", ").i(container(s));
                let (v, n) = (s.get("Volume"), s.get("Issue"));
                if !v.is_empty() || !n.is_empty() {
                    r.sep(", ").t(v);
                    if !n.is_empty() {
                        r.t(&format!("({n})"));
                    }
                }
                if !pages.is_empty() {
                    r.sep(", ").t(&pages);
                }
                r.stop();
            }
            // *Title*. Level of award. Awarding body.
            Class::Thesis => {
                r.stop();
                for x in [s.get("ThesisType"), publisher(s)] {
                    if !x.is_empty() {
                        r.t(" ").t(x).stop();
                    }
                }
            }
            Class::Legal => {
                if !s.get("CaseNumber").is_empty() {
                    r.t(&format!(" ({})", s.get("CaseNumber")));
                }
                r.stop();
            }
            Class::Web => {
                r.stop();
            }
            // *Title*. 2nd edn. Place: Publisher.
            _ => {
                r.stop();
                let e = edition(s.get("Edition"), "edn.");
                if !e.is_empty() {
                    r.t(" ").t(&e).stop();
                }
                if !pp.is_empty() {
                    r.t(" ").t(&pp).stop();
                }
            }
        }
        // Online sources: "Available at: URL (Accessed: 15 January 2024)."; a DOI needs
        // no access date.
        let l = link(s);
        if !l.is_empty() {
            r.t(" Available at: ").t(&l);
            let a = date_of(s, true);
            if s.get("DOI").is_empty() && !a.2.is_empty() {
                r.t(&format!(" (Accessed: {})", day_month_year(&a, &MONTHS)));
            }
            r.t(".");
        }
        r
    }

    /// ISO 690:2010 reference: name and date system (A.2, the year follows the creator) or
    /// numeric system (A.3, the year stays with the publication details).
    fn iso690(&self, i: usize, t: &Terms, numeric: bool) -> Rich {
        let s = &self.sources[i];
        let c = class(s);
        let mut r = Rich::default();
        let and = format!(" {} ", t.and);
        // 5.4: family name first (the examples of the standard set it in capitals), then
        // the given names; creators joined by commas and "and".
        let iso_name = |p: &Person| {
            let g = p.given();
            let l = p.last.trim().to_uppercase();
            match (l.is_empty(), g.is_empty()) {
                (true, _) => g,
                (false, true) => l,
                (false, false) => format!("{l}, {g}"),
            }
        };
        let who = match by(s) {
            By::People(p) => {
                let n: Vec<String> = p.iter().map(iso_name).collect();
                join(&n, &and, &and)
            }
            By::Body(b) => b.to_owned(),
            By::Nobody => String::new(),
        };
        let year = if numeric {
            s.get("Year").to_owned()
        } else {
            self.year(i, t.nd)
        };
        let part = matches!(c, Class::Chapter | Class::Article | Class::Conference);
        let online = c == Class::Web;
        if !who.is_empty() {
            r.t(&who);
            if numeric {
                r.stop().t(" ");
            } else {
                r.t(&format!(", {year}. "));
            }
        }
        // 6: the title of the item itself is italic unless it is a contribution, whose
        // host's title is italic. 7: "[online]" medium designation after the title.
        if part {
            r.t(s.get("Title"));
        } else {
            r.i(s.get("Title"));
        }
        if online {
            r.t(" [online]");
        }
        r.stop();
        if who.is_empty() && !numeric {
            r.t(&format!(" {year}."));
        }
        // "Place: Publisher" with the year after it in the numeric system.
        let imprint = |with_pages: bool| {
            let mut x = place_publisher(s);
            if numeric && !year.is_empty() {
                x = if x.is_empty() {
                    year.clone()
                } else {
                    format!("{x}, {year}")
                };
            }
            let p = page_label(s.get("Pages"), t);
            if with_pages && !p.is_empty() {
                x = if x.is_empty() { p } else { format!("{x}, {p}") };
            }
            x
        };
        match c {
            // 15.2 contributions: "In: EDITOR (ed.), *Host*. Place: Publisher, pp. 45–67."
            Class::Chapter | Class::Conference => {
                r.t(" In: ");
                let eds: Vec<String> = editors(s).iter().map(iso_name).collect();
                if !eds.is_empty() {
                    r.t(&join(&eds, &and, &and));
                    r.t(if eds.len() > 1 {
                        " (eds.), "
                    } else {
                        " (ed.), "
                    });
                }
                r.i(container(s)).stop();
                let x = imprint(true);
                if !x.is_empty() {
                    r.t(" ").t(&x).stop();
                }
            }
            // 15.3 serial contributions: "*Journal*. Vol. 12, no. 3, pp. 100–120." (numeric:
            // "*Journal*. 2021, vol. 12, no. 3, pp. 100–120.").
            Class::Article => {
                if !container(s).is_empty() {
                    r.t(" ").i(container(s)).stop();
                }
                let mut parts: Vec<String> = Vec::new();
                if numeric && !year.is_empty() {
                    parts.push(year.clone());
                }
                if !s.get("Volume").is_empty() {
                    parts.push(format!("vol. {}", s.get("Volume")));
                }
                if !s.get("Issue").is_empty() {
                    parts.push(format!("no. {}", s.get("Issue")));
                }
                let p = page_label(s.get("Pages"), t);
                if !p.is_empty() {
                    parts.push(p);
                }
                let mut x = parts.join(", ");
                if let Some(f) = x.get_mut(0..1) {
                    f.make_ascii_uppercase();
                }
                if !x.is_empty() {
                    r.t(" ").t(&x).stop();
                }
            }
            Class::Thesis => {
                if !s.get("ThesisType").is_empty() {
                    r.t(" ").t(s.get("ThesisType")).stop();
                }
                let x = imprint(false);
                if !x.is_empty() {
                    r.t(" ").t(&x).stop();
                }
            }
            // 15.7 / 7: site, [Accessed date]. Available from: URL
            Class::Web => {
                let mut x = container(s).to_owned();
                if numeric && !year.is_empty() {
                    x = if x.is_empty() {
                        year.clone()
                    } else {
                        format!("{x}, {year}")
                    };
                }
                if !x.is_empty() {
                    r.t(" ").t(&x).stop();
                }
            }
            Class::Legal => {
                let mut x = s.get("CaseNumber").to_owned();
                if numeric && !year.is_empty() {
                    x = if x.is_empty() {
                        year.clone()
                    } else {
                        format!("{x}, {year}")
                    };
                }
                if !x.is_empty() {
                    r.t(" ").t(&x).stop();
                }
            }
            _ => {
                let e = edition(s.get("Edition"), "ed.");
                if !e.is_empty() {
                    r.t(" ").t(&e).stop();
                }
                let x = imprint(false);
                if !x.is_empty() {
                    r.t(" ").t(&x).stop();
                }
            }
        }
        // 13: standard identifier (ISBN / ISSN) when given. 15.7: DOI, or the date of
        // citation and the location of an online resource.
        if !s.get("StandardNumber").is_empty() {
            r.t(" ").t(s.get("StandardNumber")).stop();
        }
        let doi = bare_doi(s.get("DOI"));
        if !doi.is_empty() {
            r.t(" DOI ").t(&doi).t(".");
        } else if !s.get("URL").is_empty() {
            let a = date_of(s, true);
            if !a.2.is_empty() {
                r.t(&format!(" [Accessed {}].", day_month_year(&a, &MONTHS)));
            }
            r.t(" Available from: ").t(s.get("URL"));
        }
        r
    }
}

/// `“Title.”` — the closing punctuation inside the quotation marks (American usage of
/// MLA, Chicago and IEEE); a title that ends in `?` or `!` keeps only that.
fn quote(title: &str, punct: &str) -> String {
    if title.ends_with(['?', '!']) || title.ends_with(punct) {
        format!("“{title}”")
    } else {
        format!("“{title}{punct}”")
    }
}

/// The key the alphabetical styles sort by: the authors' names (family name first) or,
/// without an author, the title without a leading article.
fn author_sort_key(s: &Source) -> String {
    match by(s) {
        By::People(p) => p
            .iter()
            .map(|x| {
                format!("{} {}", x.last.trim(), x.given())
                    .trim()
                    .to_lowercase()
            })
            .collect::<Vec<_>>()
            .join("; "),
        By::Body(b) => b.to_lowercase(),
        By::Nobody => sort_title(s),
    }
}

/// The key of the Vietnamese thesis list: a Vietnamese first author's given name (then the
/// whole name), a foreign first author's family name, an issuing body's name, or the title.
fn vn_sort_key(s: &Source) -> String {
    match by(s) {
        By::People(p) => match p.first() {
            Some(a) if s.is_vietnamese() => {
                format!("{} {}", vietnamese_sort_name(a), vietnamese(a)).to_lowercase()
            }
            Some(a) => format!("{} {}", a.last.trim(), a.given()).to_lowercase(),
            None => sort_title(s),
        },
        By::Body(b) => b.to_lowercase(),
        By::Nobody => sort_title(s),
    }
}

/// MLA 9th edition works-cited entry (template of core elements, 5.1–5.99: Author. Title of
/// source. Title of container, contributor, version, number, publisher, publication date,
/// location.).
fn mla(s: &Source, t: &Terms) -> Rich {
    // A container element: the first follows a space, the others a comma.
    fn add(r: &mut Rich, first: &mut bool, text: &str, italic: bool) {
        if text.is_empty() {
            return;
        }
        r.t(if *first { " " } else { ", " });
        *first = false;
        if italic {
            r.i(text);
        } else {
            r.t(text);
        }
    }
    let c = class(s);
    let mut r = Rich::default();
    // 5.6–5.8: one author inverted; two: "Family, Given, and Given Family"; three or more:
    // the first and "et al.".
    let who = match by(s) {
        By::People(p) => match p {
            [] => String::new(),
            [a] => inverted(a),
            [a, b] => format!("{}, {} {}", inverted(a), t.and, natural(b)),
            [a, ..] => format!("{}, {}", inverted(a), t.et_al),
        },
        By::Body(b) => b.to_owned(),
        By::Nobody => String::new(),
    };
    // 5.20: when an organisation is both author and publisher, the entry starts with the
    // title and the organisation is given as publisher only.
    let who = if !who.is_empty() && who.eq_ignore_ascii_case(publisher(s)) {
        String::new()
    } else {
        who
    };
    if !who.is_empty() {
        r.t(&who).stop().t(" ");
    }
    // 5.23–5.30: italics for a self-contained work, quotation marks for a part of one.
    let quoted = matches!(
        c,
        Class::Chapter | Class::Article | Class::Conference | Class::Web
    );
    if quoted {
        r.t(&quote(s.get("Title"), "."));
    } else {
        r.i(s.get("Title")).stop();
    }
    // Container elements, separated by commas, closed by a full stop.
    let mut first = true;
    let mut item = |r: &mut Rich, text: &str, italic: bool| add(r, &mut first, text, italic);
    let d = date_of(s, false);
    // 5.68–5.83: day month year, months abbreviated.
    let date = day_month_year(&d, &MONTHS_MLA);
    let pages = page_label(s.get("Pages"), t);
    match c {
        Class::Chapter | Class::Conference => {
            item(&mut r, container(s), true);
            let eds: Vec<String> = editors(s).iter().map(natural).collect();
            if !eds.is_empty() {
                let names = if eds.len() > 2 {
                    format!("{} {}", eds[0], t.et_al)
                } else {
                    eds.join(&format!(" {} ", t.and))
                };
                item(&mut r, &format!("edited by {names}"), false);
            }
            item(&mut r, publisher(s), false);
            item(&mut r, &date, false);
            item(&mut r, &pages, false);
        }
        // 5.51: "vol. 12, no. 3".
        Class::Article => {
            item(&mut r, container(s), true);
            if !s.get("Volume").is_empty() {
                item(&mut r, &format!("vol. {}", s.get("Volume")), false);
            }
            if !s.get("Issue").is_empty() {
                item(&mut r, &format!("no. {}", s.get("Issue")), false);
            }
            item(&mut r, &date, false);
            item(&mut r, &pages, false);
        }
        Class::Web => {
            item(&mut r, container(s), true);
            item(&mut r, &date, false);
        }
        // 5.44 (dissertations): year, then institution and the kind of thesis.
        Class::Thesis => {
            item(&mut r, &d.2, false);
            r.stop();
            let inst: Vec<&str> = [publisher(s), s.get("ThesisType")]
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect();
            if !inst.is_empty() {
                r.t(" ").t(&inst.join(", "));
            }
        }
        Class::Legal => {
            item(&mut r, s.get("CaseNumber"), false);
            item(&mut r, &date, false);
        }
        _ => {
            item(&mut r, &edition(s.get("Edition"), "ed."), false);
            item(&mut r, publisher(s), false);
            item(&mut r, &d.2, false);
        }
    }
    // 5.84–5.97: DOI (as a link) or URL is the location of an online work.
    item(&mut r, &link(s), false);
    r.stop();
    // 5.111: optional access date for online works.
    let a = date_of(s, true);
    if c == Class::Web && !a.2.is_empty() {
        r.t(" Accessed ").t(&day_month_year(&a, &MONTHS_MLA)).stop();
    }
    r
}

/// IEEE reference (IEEE Reference Guide: books, book chapters, periodicals, conference
/// papers, reports, theses, online sources).
fn ieee(s: &Source, t: &Terms) -> Rich {
    let c = class(s);
    let mut r = Rich::default();
    // Initials before the family name; up to six authors named ("A and B", "A, B, and C");
    // seven or more: the first and "et al.".
    let list = |p: &[Person]| {
        let n: Vec<String> = p.iter().map(|x| initials_family(x, " ")).collect();
        if n.len() > 6 {
            format!("{} {}", n[0], t.et_al)
        } else {
            join(&n, &format!(" {} ", t.and), &format!(", {} ", t.and))
        }
    };
    let who = match by(s) {
        By::People(p) => list(p),
        By::Body(b) => b.to_owned(),
        By::Nobody => String::new(),
    };
    let d = date_of(s, false);
    let year = d.2.clone();
    // Online: Author. "Title." Site. Accessed: Mon. Day, Year. [Online]. Available: URL
    if c == Class::Web {
        if !who.is_empty() {
            r.t(&who).stop().t(" ");
        }
        r.t(&quote(s.get("Title"), "."));
        if !container(s).is_empty() {
            r.t(" ").t(container(s)).stop();
        }
        let a = date_of(s, true);
        if !a.2.is_empty() {
            r.t(" Accessed: ")
                .t(&month_day_year(&a, &MONTHS_IEEE))
                .stop();
        }
        if !s.get("URL").is_empty() {
            r.t(" [Online]. Available: ").t(s.get("URL"));
        }
        return r;
    }
    if !who.is_empty() {
        r.t(&who).t(", ");
    }
    // Titles of books in italics; titles of papers, chapters, reports and theses in
    // quotation marks with the comma inside.
    let book_like = c == Class::Book;
    if book_like {
        r.i(s.get("Title"));
    } else {
        r.t(&quote(s.get("Title"), ","));
    }
    // The comma after a quoted title is inside the quotation marks.
    let item = |r: &mut Rich, text: &str| {
        if !text.is_empty() {
            if r.last_char() == Some('”') {
                r.t(" ");
            } else {
                r.sep(", ");
            }
            r.t(text);
        }
    };
    let pages = page_label(s.get("Pages"), t);
    match c {
        // A. Author, "Chapter," in *Book*, E. Editor, Ed. City: Publisher, year, pp. xx–xx.
        Class::Chapter => {
            r.t(" in ").i(container(s));
            let eds = editors(s);
            if !eds.is_empty() {
                r.t(", ").t(&list(eds));
                r.t(if eds.len() > 1 { ", Eds." } else { ", Ed." });
            }
            let pp = place_publisher(s);
            if pp.is_empty() {
                item(&mut r, &year);
            } else {
                r.stop().t(" ").t(&pp);
                item(&mut r, &year);
            }
            item(&mut r, &pages);
        }
        // "Paper," in *Proc. Conference*, City, year, pp. xx–xx.
        Class::Conference => {
            r.t(" in ").i(container(s));
            item(&mut r, s.get("City"));
            item(&mut r, &year);
            item(&mut r, &pages);
        }
        // "Article," *Journal*, vol. x, no. x, pp. xx–xx, Mon. year, doi: ….
        Class::Article => {
            r.t(" ").i(container(s));
            if !s.get("Volume").is_empty() {
                item(&mut r, &format!("vol. {}", s.get("Volume")));
            }
            if !s.get("Issue").is_empty() {
                item(&mut r, &format!("no. {}", s.get("Issue")));
            }
            item(&mut r, &pages);
            let date = match d.1 {
                Some(m) => format!("{} {year}", MONTHS_IEEE[m as usize - 1]),
                None => year.clone(),
            };
            item(&mut r, date.trim());
        }
        // "Title," Company, City, year. / "Title," Ph.D. dissertation, Univ., City, year.
        Class::Report | Class::Thesis => {
            if c == Class::Thesis {
                item(&mut r, s.get("ThesisType"));
            }
            item(&mut r, publisher(s));
            item(&mut r, s.get("City"));
            item(&mut r, &year);
        }
        Class::Legal => {
            item(&mut r, s.get("CaseNumber"));
            item(&mut r, &year);
        }
        // A. Author, *Title*, xth ed. City: Publisher, year.
        _ => {
            item(&mut r, &edition(s.get("Edition"), "ed."));
            let pp = place_publisher(s);
            if pp.is_empty() {
                item(&mut r, &year);
            } else {
                r.stop().t(" ").t(&pp);
                item(&mut r, &year);
            }
        }
    }
    let doi = bare_doi(s.get("DOI"));
    if !doi.is_empty() {
        r.sep(", ").t("doi: ").t(&doi);
        r.stop();
    } else if !s.get("URL").is_empty() {
        r.stop();
        r.t(" [Online]. Available: ").t(s.get("URL"));
    } else {
        r.stop();
    }
    r
}

/// Vietnamese thesis reference (Bộ GD&ĐT guidance). Books, theses, reports: `Tác giả
/// (năm), *Tên sách*, Nhà xuất bản, Nơi xuất bản.` Articles and parts of books: `Tác giả
/// (năm), "Tên bài", *Tên tạp chí hoặc sách*, tập(số), tr. x-y.` Vietnamese names are
/// written in their usual order without commas; foreign names as family name and initials
/// (`Anderson J.E.`); "tr." for Vietnamese sources and "pp." / "p." for foreign ones; the
/// page range keeps its hyphen.
fn vn_thesis(s: &Source) -> Rich {
    let c = class(s);
    let vi = s.is_vietnamese();
    let mut r = Rich::default();
    let who = match by(s) {
        By::People(p) => p
            .iter()
            .map(|x| {
                if vi {
                    vietnamese(x)
                } else {
                    format!("{} {}", x.last.trim(), initials(&x.given(), ""))
                        .trim()
                        .to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
        By::Body(b) => b.to_owned(),
        By::Nobody => String::new(),
    };
    let year = s.get("Year");
    if !who.is_empty() {
        r.t(&who);
        if !year.is_empty() {
            r.t(&format!(" ({year})"));
        }
        r.t(", ");
    }
    let part = matches!(
        c,
        Class::Chapter | Class::Article | Class::Conference | Class::Web
    );
    // "Tên bài báo" in quotation marks, not italic; tên sách, tên tạp chí in italics.
    if part {
        r.t(&format!("“{}”", s.get("Title")));
    } else {
        r.i(s.get("Title"));
    }
    if who.is_empty() && !year.is_empty() {
        r.t(&format!(" ({year})"));
    }
    let item = |r: &mut Rich, text: &str, italic: bool| {
        if !text.is_empty() {
            r.sep(", ");
            if italic {
                r.i(text);
            } else {
                r.t(text);
            }
        }
    };
    let pages = {
        let p = s.get("Pages");
        match (p.is_empty(), vi, is_range(p)) {
            (true, _, _) => String::new(),
            (_, true, _) => format!("tr. {p}"),
            (_, false, true) => format!("pp. {p}"),
            (_, false, false) => format!("p. {p}"),
        }
    };
    match c {
        Class::Article => {
            item(&mut r, container(s), true);
            let (v, n) = (s.get("Volume"), s.get("Issue"));
            let vn = match (v, n) {
                ("", "") => String::new(),
                (v, "") => v.to_owned(),
                (v, n) => format!("{v}({n})"),
            };
            item(&mut r, &vn, false);
            item(&mut r, &pages, false);
        }
        Class::Chapter | Class::Conference => {
            item(&mut r, container(s), true);
            item(&mut r, publisher(s), false);
            item(&mut r, s.get("City"), false);
            item(&mut r, &pages, false);
        }
        Class::Thesis => {
            item(&mut r, s.get("ThesisType"), false);
            item(&mut r, publisher(s), false);
            item(&mut r, s.get("City"), false);
        }
        Class::Web => {
            item(&mut r, container(s), true);
        }
        // Văn bản pháp luật: cơ quan ban hành (năm), *tên văn bản*, số hiệu, ngày ban hành.
        Class::Legal => {
            if !s.get("CaseNumber").is_empty() {
                item(&mut r, &format!("số {}", s.get("CaseNumber")), false);
            }
            let d = date_of(s, false);
            if d.1.is_some() {
                item(&mut r, &format!("ngày {}", vn_date(&d)), false);
            }
        }
        _ => {
            let p = publisher(s);
            // The issuing body is not repeated as publisher.
            if !p.eq_ignore_ascii_case(&who) {
                item(&mut r, p, false);
            }
            item(&mut r, s.get("City"), false);
        }
    }
    // Online: address and the date it was read.
    let l = link(s);
    if !l.is_empty() {
        item(&mut r, &l, false);
        let a = date_of(s, true);
        if s.get("DOI").is_empty() && !a.2.is_empty() {
            let word = if vi { "truy cập ngày" } else { "accessed" };
            item(&mut r, &format!("{word} {}", vn_date(&a)), false);
        }
    }
    r.stop();
    r
}
