//! Names, dates and page ranges as reference styles write them.

use super::source::Person;
use unicode_normalization::UnicodeNormalization;

/// `s` without diacritics, `đ` → `d` (for tags and BibTeX keys).
#[must_use]
pub fn fold_ascii(s: &str) -> String {
    s.nfd()
        .filter(|c| !('\u{0300}'..='\u{036F}').contains(c))
        .map(|c| match c {
            'đ' => 'd',
            'Đ' => 'D',
            c => c,
        })
        .collect()
}

/// Initials of given names: `John Kevin` → `J. K.` (`sep` = `" "`) or `J.K.` (`sep` = `""`);
/// a hyphenated name keeps its hyphen (`Jean-Paul` → `J.-P.`).
#[must_use]
pub fn initials(given: &str, sep: &str) -> String {
    given
        .split_whitespace()
        .map(|w| {
            w.split('-')
                .filter_map(|part| part.chars().find(|c| c.is_alphabetic()))
                .map(|c| format!("{}.", c.to_uppercase()))
                .collect::<Vec<_>>()
                .join("-")
        })
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(sep)
}

/// `Smith, J. K.` (family name, initials).
#[must_use]
pub fn family_initials(p: &Person, sep: &str) -> String {
    let i = initials(&p.given(), sep);
    match (p.last.trim().is_empty(), i.is_empty()) {
        (true, _) => p.given(),
        (false, true) => p.last.trim().to_owned(),
        (false, false) => format!("{}, {i}", p.last.trim()),
    }
}

/// `J. K. Smith` (initials, family name).
#[must_use]
pub fn initials_family(p: &Person, sep: &str) -> String {
    let i = initials(&p.given(), sep);
    match (p.last.trim().is_empty(), i.is_empty()) {
        (true, _) => p.given(),
        (false, true) => p.last.trim().to_owned(),
        (false, false) => format!("{i} {}", p.last.trim()),
    }
}

/// `Smith, John K.` (family name first, given names in full).
#[must_use]
pub fn inverted(p: &Person) -> String {
    let g = p.given();
    match (p.last.trim().is_empty(), g.is_empty()) {
        (true, _) => g,
        (false, true) => p.last.trim().to_owned(),
        (false, false) => format!("{}, {g}", p.last.trim()),
    }
}

/// `John K. Smith` (given names, family name).
#[must_use]
pub fn natural(p: &Person) -> String {
    format!("{} {}", p.given(), p.last.trim()).trim().to_owned()
}

/// `Nguyễn Văn An`: a Vietnamese name in its usual order (family name, then the rest).
#[must_use]
pub fn vietnamese(p: &Person) -> String {
    format!("{} {}", p.last.trim(), p.given()).trim().to_owned()
}

/// The name a Vietnamese person is sorted by: the last word of the full name.
#[must_use]
pub fn vietnamese_sort_name(p: &Person) -> String {
    let full = vietnamese(p);
    full.split_whitespace()
        .next_back()
        .unwrap_or_default()
        .to_owned()
}

/// A name typed as `Family, Given`, or as `Given Family` (the last word is the family name;
/// a lower-case particle before it — `van`, `de`, `von` … — belongs to it). `{Braced}` text
/// is one corporate-like family name.
#[must_use]
pub fn parse_name(text: &str) -> Person {
    let t = text.trim();
    if let Some((last, first)) = t.split_once(',') {
        // "Family, Jr, Given" (BibTeX): the last part is the given name.
        let first = first.rsplit(',').next().unwrap_or(first);
        return Person::new(last, first);
    }
    let words: Vec<&str> = t.split_whitespace().collect();
    if words.len() <= 1 {
        return Person::new(t, "");
    }
    let mut start = words.len() - 1;
    while start > 1
        && words[start - 1]
            .chars()
            .next()
            .is_some_and(char::is_lowercase)
    {
        start -= 1;
    }
    Person::new(&words[start..].join(" "), &words[..start].join(" "))
}

/// Month number of `1`–`12`, an English month name or abbreviation, or `Tháng n`.
#[must_use]
pub fn month_number(m: &str) -> Option<u32> {
    let m = m.trim().trim_end_matches('.');
    let digits: String = m.chars().filter(char::is_ascii_digit).collect();
    if !digits.is_empty() {
        return digits.parse().ok().filter(|n| (1..=12).contains(n));
    }
    let l = m.to_ascii_lowercase();
    if l.len() < 3 {
        return None;
    }
    MONTHS
        .iter()
        .position(|n| n.to_ascii_lowercase().starts_with(&l))
        .map(|i| i as u32 + 1)
}

/// English month names.
pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Month abbreviations of MLA (MLA Handbook, 9th ed.: in the works-cited list, months of more
/// than four letters are abbreviated — Jan., Feb., Mar., Apr., Aug., Sept., Oct., Nov., Dec.).
pub const MONTHS_MLA: [&str; 12] = [
    "Jan.", "Feb.", "Mar.", "Apr.", "May", "June", "July", "Aug.", "Sept.", "Oct.", "Nov.", "Dec.",
];

/// Month abbreviations of the IEEE Reference Guide.
pub const MONTHS_IEEE: [&str; 12] = [
    "Jan.", "Feb.", "Mar.", "Apr.", "May", "Jun.", "Jul.", "Aug.", "Sep.", "Oct.", "Nov.", "Dec.",
];

/// A page range with an en dash between numbers (`45-67`, `45--67` → `45–67`).
#[must_use]
pub fn en_dash(pages: &str) -> String {
    let chars: Vec<char> = pages.trim().chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '-' || chars[i] == '–' || chars[i] == '—' {
            let mut j = i;
            while j < chars.len() && matches!(chars[j], '-' | '–' | '—') {
                j += 1;
            }
            let before = out.trim_end().chars().next_back();
            let after = chars[j..].iter().find(|c| !c.is_whitespace());
            if before.is_some_and(char::is_alphanumeric)
                && after.is_some_and(|c| c.is_alphanumeric())
            {
                out.truncate(out.trim_end().len());
                out.push('–');
                i = j;
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Whether a locator names more than one page (`45-67`, `3, 9`).
#[must_use]
pub fn is_range(pages: &str) -> bool {
    pages.contains(['-', '–', '—', ',']) || pages.contains(" and ")
}

/// `2` → `2nd` (English ordinal).
#[must_use]
pub fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// An edition statement: `2` → `2nd {word}`; text that is not a number is kept (with `word`
/// added when it does not already name the edition).
#[must_use]
pub fn edition(value: &str, word: &str) -> String {
    let v = value.trim();
    if v.is_empty() {
        return String::new();
    }
    if let Ok(n) = v.parse::<u32>() {
        return format!("{} {word}", ordinal(n));
    }
    // "3rd ed.", "Revised edition", "2. edn." already name the edition.
    if v.to_lowercase()
        .split_whitespace()
        .any(|w| matches!(w.trim_end_matches('.'), "ed" | "edn" | "edition" | "eds"))
    {
        v.to_owned()
    } else {
        format!("{v} {word}")
    }
}

/// A DOI without resolver prefix (`https://doi.org/10.1/x`, `doi:10.1/x` → `10.1/x`).
#[must_use]
pub fn bare_doi(doi: &str) -> String {
    let d = doi.trim();
    let l = d.to_ascii_lowercase();
    for p in [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
        "doi:",
    ] {
        if l.starts_with(p) {
            return d[p.len()..].trim().to_owned();
        }
    }
    d.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let p = Person::new("Smith", "John Kevin");
        assert_eq!(family_initials(&p, " "), "Smith, J. K.");
        assert_eq!(family_initials(&p, ""), "Smith, J.K.");
        assert_eq!(initials_family(&p, " "), "J. K. Smith");
        assert_eq!(inverted(&p), "Smith, John Kevin");
        assert_eq!(natural(&p), "John Kevin Smith");
        assert_eq!(initials("Jean-Paul", " "), "J.-P.");
        let v = Person::new("Nguyễn", "Văn An");
        assert_eq!(vietnamese(&v), "Nguyễn Văn An");
        assert_eq!(vietnamese_sort_name(&v), "An");
        assert_eq!(fold_ascii("Đặng Thị Hồng"), "Dang Thi Hong");
    }

    #[test]
    fn typed_names() {
        assert_eq!(
            parse_name("Smith, John K."),
            Person::new("Smith", "John K.")
        );
        assert_eq!(parse_name("John K. Smith"), Person::new("Smith", "John K."));
        assert_eq!(
            parse_name("Ludwig van Beethoven"),
            Person::new("van Beethoven", "Ludwig")
        );
        assert_eq!(parse_name("Plato"), Person::new("Plato", ""));
        assert_eq!(
            parse_name("King, Jr, Martin"),
            Person::new("King", "Martin")
        );
    }

    #[test]
    fn pages_months_editions() {
        assert_eq!(en_dash("45-67"), "45–67");
        assert_eq!(en_dash("45 -- 67"), "45–67");
        assert_eq!(en_dash("12"), "12");
        assert!(is_range("3, 9") && !is_range("12"));
        assert_eq!(month_number("5"), Some(5));
        assert_eq!(month_number("Sept."), Some(9));
        assert_eq!(month_number("Tháng 11"), Some(11));
        assert_eq!(month_number("x"), None);
        assert_eq!(edition("2", "ed."), "2nd ed.");
        assert_eq!(edition("23", "edn."), "23rd edn.");
        assert_eq!(edition("Revised", "ed."), "Revised ed.");
        assert_eq!(edition("3rd ed.", "ed."), "3rd ed.");
        assert_eq!(bare_doi("https://doi.org/10.1/x"), "10.1/x");
        assert_eq!(bare_doi("doi:10.1/x"), "10.1/x");
    }
}
