//! Export options: PDF/A conformance, tagged PDF (logical structure), bookmarks
//! (document outline) and associated files.

use serde::{Deserialize, Serialize};

/// PDF/A part, conformance level B (ISO 19005-1/-2/-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PdfA {
    /// ISO 19005-1:2005, level B (PDF 1.4 base, no transparency).
    #[serde(rename = "1b")]
    A1b,
    /// ISO 19005-2:2011, level B (PDF 1.7 base).
    #[serde(rename = "2b")]
    A2b,
    /// ISO 19005-3:2012, level B (as -2, plus embedded files of any type).
    #[serde(rename = "3b")]
    A3b,
}

impl PdfA {
    /// Parse `1b` / `2b` / `3b` (case-insensitive; `1`, `2`, `3` too).
    #[must_use]
    pub fn parse(s: &str) -> Option<PdfA> {
        match s.trim().to_ascii_lowercase().as_str() {
            "1b" | "1" | "a-1b" | "pdfa-1b" => Some(PdfA::A1b),
            "2b" | "2" | "a-2b" | "pdfa-2b" => Some(PdfA::A2b),
            "3b" | "3" | "a-3b" | "pdfa-3b" => Some(PdfA::A3b),
            _ => None,
        }
    }

    /// `pdfaid:part` value.
    #[must_use]
    pub fn part(self) -> u8 {
        match self {
            PdfA::A1b => 1,
            PdfA::A2b => 2,
            PdfA::A3b => 3,
        }
    }

    /// Embedded files are allowed (ISO 19005-3 §6.8; -1 forbids them, -2 only
    /// allows PDF/A files).
    #[must_use]
    pub fn allows_attachments(self) -> bool {
        self == PdfA::A3b
    }
}

/// Standard structure types used by the exporters (ISO 32000-1 §14.8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// §14.8.4.2 Sect — a section (one sheet, one slide).
    Sect,
    /// §14.8.4.2 Div — generic block grouping.
    Div,
    /// §14.8.4.3.2 H1…H6 — heading of level 1 to 6.
    H(u8),
    /// §14.8.4.3.2 P — paragraph.
    P,
    /// §14.8.4.3.3 L — list.
    L,
    /// §14.8.4.3.3 LI — list item.
    LI,
    /// §14.8.4.3.3 LBody — list item body.
    LBody,
    /// §14.8.4.3.4 Table.
    Table,
    /// §14.8.4.3.4 TR — table row.
    TR,
    /// §14.8.4.3.4 TH — table header cell.
    TH,
    /// §14.8.4.3.4 TD — table data cell.
    TD,
    /// §14.8.4.5 Figure — picture (with `/Alt`).
    Figure,
    /// §14.8.4.2 Note — footnote / endnote.
    Note,
    /// §14.8.4.4.1 Span — inline text.
    Span,
}

impl Role {
    pub(crate) fn pdf(self) -> pdf_writer::types::StructRole {
        use pdf_writer::types::StructRole as S;
        match self {
            Role::Sect => S::Sect,
            Role::Div => S::Div,
            Role::H(l) => match l {
                0 | 1 => S::H1,
                2 => S::H2,
                3 => S::H3,
                4 => S::H4,
                5 => S::H5,
                _ => S::H6,
            },
            Role::P => S::P,
            Role::L => S::L,
            Role::LI => S::LI,
            Role::LBody => S::LBody,
            Role::Table => S::Table,
            Role::TR => S::TR,
            Role::TH => S::TH,
            Role::TD => S::TD,
            Role::Figure => S::Figure,
            Role::Note => S::Note,
            Role::Span => S::Span,
        }
    }

    /// Tag of the marked-content sequences owned by an element of this role.
    pub(crate) fn tag(self) -> &'static str {
        match self {
            Role::H(l) => match l {
                0 | 1 => "H1",
                2 => "H2",
                3 => "H3",
                4 => "H4",
                5 => "H5",
                _ => "H6",
            },
            Role::Figure => "Figure",
            Role::TH => "TH",
            Role::TD => "TD",
            Role::Span => "Span",
            Role::LBody => "LBody",
            _ => "P",
        }
    }
}

/// A child of a structure element: another element, or the display-list
/// commands `start..end` of page `page`, drawn as one marked-content sequence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StructKid {
    /// Nested structure element.
    Node(StructNode),
    /// Commands `start..end` (indices into the page's command list).
    Content {
        /// 0-based page index.
        page: usize,
        /// First command.
        start: usize,
        /// End (exclusive).
        end: usize,
    },
}

/// A structure element (ISO 32000-1 §14.7.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructNode {
    /// Structure type.
    pub role: Role,
    /// Alternate description (`/Alt`, §14.9.3), e.g. a picture's alt text.
    pub alt: Option<String>,
    /// Language of the element (`/Lang`, §14.9.2) when it differs from the document.
    pub lang: Option<String>,
    /// Children in reading order.
    pub kids: Vec<StructKid>,
}

impl StructNode {
    /// An element without children.
    #[must_use]
    pub fn new(role: Role) -> StructNode {
        StructNode {
            role,
            alt: None,
            lang: None,
            kids: Vec::new(),
        }
    }

    /// Append the commands `start..end` of `page`, extending the previous content
    /// kid when it ends exactly at `start` on the same page.
    pub fn push_content(&mut self, page: usize, start: usize, end: usize) {
        if let Some(StructKid::Content {
            page: p, end: e, ..
        }) = self.kids.last_mut()
        {
            if *p == page && *e == start {
                *e = end;
                return;
            }
        }
        self.kids.push(StructKid::Content { page, start, end });
    }

    /// `true` when neither the element nor its descendants own any content.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kids.iter().all(|k| match k {
            StructKid::Node(n) => n.is_empty(),
            StructKid::Content { .. } => false,
        })
    }
}

/// One bookmark (ISO 32000-1 §12.3.3 outline item) of a flat list; `level` 1 is
/// top level, deeper levels nest under the closest previous shallower entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    /// Shown title.
    pub title: String,
    /// Nesting level (1 = top level).
    pub level: u8,
    /// 0-based target page.
    pub page: usize,
    /// Target top edge on that page, in points from the page top (y-down).
    pub y: f32,
}

/// A file embedded as an associated file of the document (ISO 32000-2 §14.13;
/// ISO 19005-3 §6.8), e.g. the source `.docx` of a PDF/A-3 export.
#[derive(Debug, Clone, PartialEq)]
pub struct Attachment {
    /// File name (`/F` and `/UF`).
    pub name: String,
    /// MIME type (`/Subtype` of the embedded file stream).
    pub mime: String,
    /// Description (`/Desc`).
    pub description: String,
    /// File bytes.
    pub data: Vec<u8>,
    /// `/AFRelationship` is `/Source` (otherwise `/Data`).
    pub source: bool,
}

/// Calendar date and time in UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfDate {
    /// Year (e.g. 2026).
    pub year: u16,
    /// Month 1–12.
    pub month: u8,
    /// Day 1–31.
    pub day: u8,
    /// Hour 0–23.
    pub hour: u8,
    /// Minute 0–59.
    pub minute: u8,
    /// Second 0–59.
    pub second: u8,
}

impl PdfDate {
    /// UTC date of a Unix time in seconds (H. Hinnant's days-to-civil algorithm).
    #[must_use]
    pub fn from_unix(secs: i64) -> PdfDate {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        PdfDate {
            year: y.clamp(0, 9999) as u16,
            month: m as u8,
            day: d as u8,
            hour: (rem / 3600) as u8,
            minute: (rem / 60 % 60) as u8,
            second: (rem % 60) as u8,
        }
    }

    /// ISO 8601 form used in XMP (`2026-11-10T08:30:00Z`).
    #[must_use]
    pub fn iso(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    pub(crate) fn pdf(self) -> pdf_writer::Date {
        pdf_writer::Date::new(self.year)
            .month(self.month)
            .day(self.day)
            .hour(self.hour)
            .minute(self.minute)
            .second(self.second)
            .utc_offset_hour(0)
            .utc_offset_minute(0)
    }
}

/// Where a link annotation leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// A URI action (ISO 32000-1 §12.6.4.7). Non-ASCII characters are percent-encoded.
    Uri(String),
    /// A go-to action (§12.6.4.2) to the top of page `page` (0-based) of this document.
    Page(usize),
}

/// A link annotation (ISO 32000-1 §12.5.6.5) over a rectangle of a page.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfLink {
    /// 0-based page the link is on.
    pub page: usize,
    /// `[x, y, w, h]` in points from the page's top-left corner.
    pub rect: [f32; 4],
    /// Where it leads.
    pub target: LinkTarget,
}

/// Everything beyond the plain page content that an export may add.
#[derive(Debug, Clone, Default)]
pub struct PdfOptions {
    /// PDF/A conformance to meet; `None` = plain PDF 1.7.
    pub pdfa: Option<PdfA>,
    /// Logical structure (tagged PDF, ISO 32000-1 §14.7–14.8): the children of
    /// the `/Document` element. `None` = untagged.
    pub structure: Option<Vec<StructNode>>,
    /// Document language (`/Lang`, BCP 47 such as `vi-VN`).
    pub lang: Option<String>,
    /// Bookmarks in document order.
    pub bookmarks: Vec<Bookmark>,
    /// Associated files (only written for plain PDF and PDF/A-3).
    pub attachments: Vec<Attachment>,
    /// Link annotations (URLs and jumps to pages). Links to pages that do not
    /// exist and links on pages that do not exist are left out.
    pub links: Vec<PdfLink>,
    /// `/Author` and `dc:creator`.
    pub author: Option<String>,
    /// `/Creator` and `xmp:CreatorTool` (the application that made the content).
    pub creator: Option<String>,
    /// Creation and modification date; omitted when `None` (deterministic output).
    pub date: Option<PdfDate>,
}

/// Export choices a user makes in the PDF dialogs (the JSON the hosts send).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExportChoices {
    /// PDF/A part (`"1b"`, `"2b"`, `"3b"`) or none.
    pub pdfa: Option<PdfA>,
    /// Write the logical structure (tagged PDF).
    pub tagged: bool,
    /// Write bookmarks.
    pub bookmarks: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_dates() {
        // 2000-03-01T00:00:00Z = 951 868 800 s (leap-year February handled).
        assert_eq!(
            PdfDate::from_unix(951_868_800).iso(),
            "2000-03-01T00:00:00Z"
        );
        assert_eq!(PdfDate::from_unix(0).iso(), "1970-01-01T00:00:00Z");
        // 86 399 s = 23:59:59 of the first day.
        assert_eq!(PdfDate::from_unix(86_399).iso(), "1970-01-01T23:59:59Z");
    }

    #[test]
    fn parse_parts() {
        assert_eq!(PdfA::parse("2B"), Some(PdfA::A2b));
        assert_eq!(PdfA::parse("x"), None);
        let c: ExportChoices = serde_json::from_str(r#"{"pdfa":"3b","tagged":true}"#).unwrap();
        assert_eq!(c.pdfa, Some(PdfA::A3b));
        assert!(c.tagged && !c.bookmarks);
    }
}
