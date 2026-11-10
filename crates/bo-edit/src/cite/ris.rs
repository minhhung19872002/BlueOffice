//! RIS import of sources: the tagged text format of reference managers (`TY  - JOUR` …
//! `ER  - `; the public RIS format specification, tag list "RIS Format Specifications").

use super::names::{month_number, parse_name};
use super::source::{new_tag, unique_tag, Contributor, Person, Source};

/// `2020/05/04/…` (RIS date `YYYY/MM/DD/other`) → year, month, day.
fn split_date(v: &str) -> (String, String, String) {
    let mut it = v.split(['/', '-']).map(str::trim);
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

fn person(v: &str) -> Person {
    // "Family, Given, Suffix" — the suffix is dropped.
    let mut parts = v.splitn(3, ',').map(str::trim);
    match (parts.next(), parts.next()) {
        (Some(l), Some(f)) => Person::new(l, f),
        _ => parse_name(v),
    }
}

fn add_person(s: &mut Source, role: &str, v: &str) {
    let p = person(v);
    match s.contributors.iter_mut().find(|c| c.role == role) {
        Some(c) => c.people.push(p),
        None => s.contributors.push(Contributor {
            role: role.to_owned(),
            corporate: None,
            people: vec![p],
        }),
    }
}

/// Characters a tag may hold.
fn tag_of(id: &str) -> String {
    super::names::fold_ascii(id)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "_-:.".contains(*c))
        .collect()
}

/// The sources of a RIS text; `ID` becomes the tag when there is one (made unique among
/// the records and `taken`).
#[must_use]
pub fn import(text: &str, taken: &[String]) -> Vec<Source> {
    let mut used: Vec<String> = taken.to_vec();
    let mut out: Vec<Source> = Vec::new();
    let mut cur: Option<Source> = None;
    let mut id = String::new();
    let mut ty = String::new();
    let (mut sp, mut ep) = (String::new(), String::new());
    let mut secondary = String::new();
    let mut last: Option<String> = None;
    let finish = |s: Option<Source>,
                  id: &mut String,
                  ty: &mut String,
                  sp: &mut String,
                  ep: &mut String,
                  secondary: &mut String,
                  used: &mut Vec<String>,
                  out: &mut Vec<Source>| {
        let Some(mut s) = s else {
            return;
        };
        if !sp.is_empty() {
            let pages = if ep.is_empty() || ep == sp || sp.contains('-') {
                sp.clone()
            } else {
                format!("{sp}-{ep}")
            };
            s.set("Pages", &pages);
        }
        // T2 is the journal of an article, the book of a chapter, the conference of a paper.
        if !secondary.is_empty() {
            let field = match s.kind.as_str() {
                "BookSection" => "BookTitle",
                "ConferenceProceedings" => "ConferenceName",
                "JournalArticle" => "JournalName",
                "ArticleInAPeriodical" => "PeriodicalTitle",
                "InternetSite" => "InternetSiteTitle",
                _ => "",
            };
            if !field.is_empty() && s.get(field).is_empty() {
                s.set(field, secondary);
            }
        }
        if ty == "THES" && s.get("ThesisType").is_empty() {
            s.set("ThesisType", "Thesis");
        }
        // A thesis names its school as publisher.
        if ty == "THES" && s.get("Institution").is_empty() {
            let p = s.get("Publisher").to_owned();
            s.set("Institution", &p);
            s.set("Publisher", "");
        }
        let key = tag_of(id);
        s.tag = if key.is_empty() {
            new_tag(&s, used)
        } else {
            unique_tag(&key, used)
        };
        used.push(s.tag.clone());
        out.push(s);
        id.clear();
        ty.clear();
        sp.clear();
        ep.clear();
        secondary.clear();
    };
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let line = line.strip_prefix('\u{FEFF}').unwrap_or(line);
        // "TY  - JOUR": two characters, two spaces, a hyphen, an optional space.
        let tagged = line.len() >= 5
            && line.is_char_boundary(2)
            && line[..2]
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && line[2..].trim_start().starts_with('-');
        if !tagged {
            // A continuation line of the previous field (long abstracts, titles).
            if let (Some(s), Some(field)) = (cur.as_mut(), last.as_deref()) {
                if !line.trim().is_empty() {
                    let joined = format!("{} {}", s.get(field), line.trim());
                    s.set(field, &joined);
                }
            }
            continue;
        }
        let tag = &line[..2];
        let value = line[2..].trim_start().trim_start_matches('-').trim();
        last = None;
        if tag == "TY" {
            finish(
                cur.take(),
                &mut id,
                &mut ty,
                &mut sp,
                &mut ep,
                &mut secondary,
                &mut used,
                &mut out,
            );
            ty = value.to_ascii_uppercase();
            let kind = match ty.as_str() {
                "BOOK" | "EDBOOK" | "EBOOK" => "Book",
                "CHAP" | "ECHAP" => "BookSection",
                "JOUR" | "EJOUR" | "JFULL" | "ABST" | "INPR" => "JournalArticle",
                "MGZN" | "NEWS" => "ArticleInAPeriodical",
                "CONF" | "CPAPER" => "ConferenceProceedings",
                "RPRT" | "THES" | "STAND" => "Report",
                "ELEC" | "WEB" | "BLOG" => "InternetSite",
                "CASE" | "STAT" | "LEGAL" | "BILL" | "GOVDOC" => "Case",
                "PAT" => "Patent",
                "ART" => "Art",
                "MPCT" | "VIDEO" | "ADVS" => "Film",
                "SOUND" | "MUSIC" => "SoundRecording",
                _ => "Misc",
            };
            cur = Some(Source {
                kind: kind.to_owned(),
                ..Source::default()
            });
            continue;
        }
        if tag == "ER" {
            finish(
                cur.take(),
                &mut id,
                &mut ty,
                &mut sp,
                &mut ep,
                &mut secondary,
                &mut used,
                &mut out,
            );
            continue;
        }
        let Some(s) = cur.as_mut() else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        let mut field = |s: &mut Source, name: &str, first_wins: bool| {
            if !first_wins || s.get(name).is_empty() {
                s.set(name, value);
            }
            last = Some(name.to_owned());
        };
        match tag {
            "AU" | "A1" => add_person(s, "Author", value),
            "A2" | "ED" => add_person(s, "Editor", value),
            "A4" => add_person(s, "Translator", value),
            "TI" | "T1" => field(s, "Title", true),
            "ST" => field(s, "ShortTitle", true),
            "T2" | "BT" if s.kind == "Book" && s.get("Title").is_empty() && tag == "BT" => {
                field(s, "Title", true);
            }
            "T2" | "BT" => secondary = value.to_owned(),
            "JO" | "JF" | "J2" | "JA" if s.kind == "ArticleInAPeriodical" => {
                field(s, "PeriodicalTitle", true);
            }
            "JO" | "JF" => field(s, "JournalName", false),
            "J2" | "JA" => field(s, "JournalName", true),
            "PY" | "Y1" => {
                let (y, m, d) = split_date(value);
                s.set("Year", &y);
                if s.get("Month").is_empty() {
                    s.set("Month", &m);
                    s.set("Day", &d);
                }
            }
            "DA" => {
                let (y, m, d) = split_date(value);
                if s.get("Year").is_empty() {
                    s.set("Year", &y);
                }
                s.set("Month", &m);
                s.set("Day", &d);
            }
            "Y2" => {
                let (y, m, d) = split_date(value);
                s.set("YearAccessed", &y);
                s.set("MonthAccessed", &m);
                s.set("DayAccessed", &d);
            }
            "VL" => field(s, "Volume", true),
            "IS" if s.kind == "Case" => field(s, "CaseNumber", true),
            "IS" => field(s, "Issue", true),
            "SP" => sp = value.to_owned(),
            "EP" => ep = value.to_owned(),
            "PB" => field(s, "Publisher", true),
            "CY" | "PP" => field(s, "City", true),
            "ET" => field(s, "Edition", true),
            "SN" => field(s, "StandardNumber", true),
            "DO" => field(s, "DOI", true),
            "UR" | "L2" => field(s, "URL", true),
            "M3" if ty == "THES" => field(s, "ThesisType", true),
            "N1" => field(s, "Comments", true),
            "LA" => {
                let l = value.to_lowercase();
                if l.starts_with("vi") {
                    s.set("LCID", "1066");
                } else if l.starts_with("en") {
                    s.set("LCID", "1033");
                }
            }
            "ID" => id = value.to_owned(),
            _ => {}
        }
    }
    finish(
        cur.take(),
        &mut id,
        &mut ty,
        &mut sp,
        &mut ep,
        &mut secondary,
        &mut used,
        &mut out,
    );
    out
}
