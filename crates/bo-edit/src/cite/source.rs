//! Bibliography sources (ECMA-376 Part 1, §22.6 Bibliography): the `b:Sources` custom XML
//! part of a document — every source type (§22.6.2.63 ST_SourceType) with its contributors
//! (§22.6.2.3 Author … §22.6.2.72 Writer; person names §22.6.2.52, corporate §22.6.2.17) and
//! its other elements (title, year, publisher, journal, volume, issue, pages, URL, access
//! date, LCID, …) kept by element name, so nothing a file holds is lost when it is written back.

use bo_model::custom_xml::dom::{self, XElement, XNode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Namespace of the bibliography part (§22.6).
pub const NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/bibliography";

/// The source types of §22.6.2.63 ST_SourceType.
pub const SOURCE_TYPES: [&str; 17] = [
    "Book",
    "BookSection",
    "JournalArticle",
    "ArticleInAPeriodical",
    "ConferenceProceedings",
    "Report",
    "InternetSite",
    "DocumentFromInternetSite",
    "ElectronicSource",
    "Art",
    "SoundRecording",
    "Performance",
    "Film",
    "Interview",
    "Patent",
    "Case",
    "Misc",
];

/// A person's name (§22.6.2.52 Person).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Person {
    /// Family name (`b:Last`).
    pub last: String,
    /// Given name (`b:First`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub first: String,
    /// Middle name (`b:Middle`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub middle: String,
}

impl Person {
    /// A person from family and given names.
    #[must_use]
    pub fn new(last: &str, first: &str) -> Person {
        Person {
            last: last.trim().to_owned(),
            first: first.trim().to_owned(),
            middle: String::new(),
        }
    }

    /// Given names in order (first, then middle).
    #[must_use]
    pub fn given(&self) -> String {
        [self.first.trim(), self.middle.trim()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// The contributors of one role (`b:Author/b:Author`, `b:Author/b:Editor`, …): people or
/// one corporate name (§22.6.2.17 Corporate).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Contributor {
    /// Role element name: `Author`, `Editor`, `Translator`, `BookAuthor`, `Compiler`, …
    pub role: String,
    /// Corporate author.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corporate: Option<String>,
    /// People in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub people: Vec<Person>,
}

/// One source (§22.6.2.59 Source).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Source {
    /// `b:Tag`: the name citations refer to.
    pub tag: String,
    /// `b:SourceType` (§22.6.2.63).
    pub kind: String,
    /// Contributors by role.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contributors: Vec<Contributor>,
    /// The other elements by local name (`Title`, `Year`, `Publisher`, `JournalName`,
    /// `Volume`, `Issue`, `Pages`, `URL`, `DOI`, `YearAccessed`, `LCID`, `Guid`, …).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
}

impl Source {
    /// The value of element `name` (trimmed; `""` when absent).
    #[must_use]
    pub fn get(&self, name: &str) -> &str {
        self.fields.get(name).map_or("", |s| s.trim())
    }

    /// Set (or, with an empty value, remove) element `name`.
    pub fn set(&mut self, name: &str, value: &str) {
        if value.trim().is_empty() {
            self.fields.remove(name);
        } else {
            self.fields.insert(name.to_owned(), value.trim().to_owned());
        }
    }

    /// The contributors of `role` (when it names somebody).
    #[must_use]
    pub fn role(&self, role: &str) -> Option<&Contributor> {
        self.contributors.iter().find(|c| {
            c.role == role
                && (c.corporate.as_deref().is_some_and(|s| !s.trim().is_empty())
                    || !c.people.is_empty())
        })
    }

    /// A placeholder: a tag to cite now and fill in later (no title, no contributors).
    #[must_use]
    pub fn is_placeholder(&self) -> bool {
        self.contributors.iter().all(|c| {
            c.people.is_empty() && c.corporate.as_deref().is_none_or(|s| s.trim().is_empty())
        }) && self
            .fields
            .keys()
            .all(|k| matches!(k.as_str(), "Guid" | "LCID" | "RefOrder"))
    }

    /// The source is written in Vietnamese: `b:LCID` 1066, or (no LCID) its title or first
    /// author uses letters only Vietnamese has.
    #[must_use]
    pub fn is_vietnamese(&self) -> bool {
        let lcid = self.get("LCID");
        if !lcid.is_empty() && lcid != "0" {
            return lcid == "1066" || lcid.eq_ignore_ascii_case("vi-VN");
        }
        let vn = |s: &str| {
            s.chars().any(|c| {
                matches!(c, 'ă' | 'Ă' | 'đ' | 'Đ' | 'ơ' | 'Ơ' | 'ư' | 'Ư')
                    || ('\u{1EA0}'..='\u{1EF9}').contains(&c)
            })
        };
        vn(self.get("Title"))
            || self.role("Author").is_some_and(|a| {
                a.corporate.as_deref().is_some_and(vn)
                    || a.people.iter().any(|p| vn(&p.last) || vn(&p.first))
            })
    }
}

/// The sources of a document (§22.6.2.60 Sources) and the selected style.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SourceList {
    /// `b:Sources/@StyleName`.
    pub style_name: String,
    /// `b:Sources/@SelectedStyle` as read (a style file reference of the writing program;
    /// dropped when the style is changed here).
    pub selected_style: String,
    /// The sources in order.
    pub sources: Vec<Source>,
}

fn text_of(e: &XElement) -> String {
    e.string_value().trim().to_owned()
}

fn read_people(e: &XElement) -> Vec<Person> {
    e.elements()
        .filter(|p| p.local() == "Person")
        .map(|p| {
            let mut out = Person::default();
            for f in p.elements() {
                match f.local() {
                    "Last" => out.last = text_of(f),
                    "First" => out.first = text_of(f),
                    "Middle" => out.middle = text_of(f),
                    _ => {}
                }
            }
            out
        })
        .filter(|p| !(p.last.is_empty() && p.first.is_empty() && p.middle.is_empty()))
        .collect()
}

fn read_source(e: &XElement) -> Source {
    let mut s = Source::default();
    for f in e.elements() {
        match f.local() {
            "Tag" => s.tag = text_of(f),
            "SourceType" => s.kind = text_of(f),
            // §22.6.2.4 Author (the list of contributor roles).
            "Author" => {
                for role in f.elements() {
                    let mut c = Contributor {
                        role: role.local().to_owned(),
                        ..Contributor::default()
                    };
                    for x in role.elements() {
                        match x.local() {
                            "Corporate" => c.corporate = Some(text_of(x)),
                            "NameList" => c.people = read_people(x),
                            _ => {}
                        }
                    }
                    s.contributors.push(c);
                }
            }
            name => {
                let v = text_of(f);
                if !v.is_empty() {
                    s.fields.insert(name.to_owned(), v);
                }
            }
        }
    }
    if s.kind.is_empty() {
        s.kind = "Misc".into();
    }
    s
}

/// Parse a sources part. `None` when `xml` is not one (document element `Sources` in [`NS`]).
#[must_use]
pub fn parse_sources(xml: &str) -> Option<SourceList> {
    // Cheap test first: every custom XML part is asked.
    if !xml.contains(NS) {
        return None;
    }
    let d = dom::parse(xml).ok()?;
    if d.root.local() != "Sources" {
        return None;
    }
    let attr = |n: &str| {
        d.root
            .attrs
            .iter()
            .find(|(k, _)| k == n)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    Some(SourceList {
        style_name: attr("StyleName"),
        selected_style: attr("SelectedStyle"),
        sources: d
            .root
            .elements()
            .filter(|e| e.local() == "Source")
            .map(read_source)
            .collect(),
    })
}

fn el(name: &str, text: &str) -> XNode {
    XNode::Element(XElement {
        name: format!("b:{name}"),
        attrs: Vec::new(),
        children: vec![XNode::Text(text.to_owned())],
    })
}

fn node(name: &str, children: Vec<XNode>) -> XNode {
    XNode::Element(XElement {
        name: format!("b:{name}"),
        attrs: Vec::new(),
        children,
    })
}

fn valid_name(n: &str) -> bool {
    let mut c = n.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic())
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_')
}

fn write_source(s: &Source) -> XNode {
    let mut kids = vec![el("Tag", &s.tag), el("SourceType", &s.kind)];
    let roles: Vec<XNode> = s
        .contributors
        .iter()
        .filter(|c| valid_name(&c.role))
        .filter_map(|c| {
            let corp = c.corporate.as_deref().map(str::trim).unwrap_or_default();
            // §22.6.2.17: a corporate name stands instead of the name list.
            if !corp.is_empty() {
                return Some(node(&c.role, vec![el("Corporate", corp)]));
            }
            if c.people.is_empty() {
                return None;
            }
            let people = c
                .people
                .iter()
                .map(|p| {
                    let mut f = vec![el("Last", p.last.trim())];
                    if !p.first.trim().is_empty() {
                        f.push(el("First", p.first.trim()));
                    }
                    if !p.middle.trim().is_empty() {
                        f.push(el("Middle", p.middle.trim()));
                    }
                    node("Person", f)
                })
                .collect();
            Some(node(&c.role, vec![node("NameList", people)]))
        })
        .collect();
    if !roles.is_empty() {
        kids.push(node("Author", roles));
    }
    for (k, v) in &s.fields {
        if valid_name(k) && !v.trim().is_empty() && !matches!(k.as_str(), "Tag" | "SourceType") {
            kids.push(el(k, v.trim()));
        }
    }
    node("Source", kids)
}

/// The XML of a sources part (§22.6.2.60).
#[must_use]
pub fn write_sources(list: &SourceList) -> String {
    let mut attrs = Vec::new();
    if !list.selected_style.is_empty() {
        attrs.push(("SelectedStyle".to_owned(), list.selected_style.clone()));
    }
    if !list.style_name.is_empty() {
        attrs.push(("StyleName".to_owned(), list.style_name.clone()));
    }
    attrs.push(("xmlns:b".to_owned(), NS.to_owned()));
    attrs.push(("xmlns".to_owned(), NS.to_owned()));
    let root = XElement {
        name: "b:Sources".into(),
        attrs,
        children: list.sources.iter().map(write_source).collect(),
    };
    dom::write(&dom::XDocument {
        prolog: "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>".into(),
        root,
        epilog: String::new(),
    })
}

/// A tag for a new source in the usual shape — the first three letters of the first author's
/// family name (or of the title) and the last two digits of the year (`Smi20`) — made unique
/// among `taken` with a numeric suffix. Only ASCII letters and digits are used.
#[must_use]
pub fn new_tag(s: &Source, taken: &[String]) -> String {
    let seed = s
        .role("Author")
        .map(|a| {
            a.corporate
                .clone()
                .filter(|c| !c.trim().is_empty())
                .or_else(|| a.people.first().map(|p| p.last.clone()))
                .unwrap_or_default()
        })
        .filter(|x| !x.trim().is_empty())
        .unwrap_or_else(|| s.get("Title").to_owned());
    let mut stem: String = super::names::fold_ascii(&seed)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(3)
        .collect();
    if stem.is_empty() {
        stem = "Src".into();
    }
    if let Some(f) = stem.get_mut(0..1) {
        f.make_ascii_uppercase();
    }
    let digits: Vec<char> = s.get("Year").chars().filter(char::is_ascii_digit).collect();
    let yy: String = digits[digits.len().saturating_sub(2)..].iter().collect();
    unique_tag(&format!("{stem}{yy}"), taken)
}

/// `base` or, when taken, `base` with the first free number appended.
#[must_use]
pub fn unique_tag(base: &str, taken: &[String]) -> String {
    let used = |t: &str| taken.iter().any(|x| x.eq_ignore_ascii_case(t));
    if !base.is_empty() && !used(base) {
        return base.to_owned();
    }
    let mut n = 1;
    loop {
        let t = format!("{base}{n}");
        if !used(&t) {
            return t;
        }
        n += 1;
    }
}
