//! Check document (CORE-INSPECT-001): a document inspector that finds and removes
//! hidden or personal content, an accessibility checker and a compatibility
//! checker, for BlueDoc, BlueSheet and BlueSlide.
//!
//! The engine reports stable ids (categories, rules, features, formats) and
//! the UI words them. Content of the document model (hidden text, comments,
//! hidden rows, hidden slides, notes, …) is removed as one undoable edit;
//! package-level parts (document properties — ECMA-376 Part 2, §11 core
//! properties —, custom XML parts, the VBA project, comment parts of a deck)
//! are queued and dropped when the file is saved ([`package::scrub`]).

pub mod a11y;
mod bridge;
pub mod compat;
pub mod doc;
pub mod package;
pub mod sheet;
pub mod slide;

use serde::{Deserialize, Serialize};

/// At most this many accessibility issues are listed (the count is still exact).
pub const MAX_ISSUES: usize = 500;
/// At most this many detail strings per inspector category.
pub const MAX_DETAILS: usize = 20;

/// What the document inspector looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// Comments and their authors.
    Comments,
    /// Tracked changes (reported; accepting them all is a separate command).
    Revisions,
    /// Core and extended document properties with personal information
    /// (author, last modified by, company, manager, title, …).
    Properties,
    /// Custom document properties (`docProps/custom.xml`).
    CustomProperties,
    /// Custom XML data parts (`customXml/item*.xml`).
    CustomXml,
    /// Document server properties and content type schemas kept as custom XML.
    ServerProperties,
    /// A VBA project (`vbaProject.bin`).
    Macros,
    /// Text formatted as hidden (`w:vanish`, ECMA-376 Part 1, §17.3.2.41).
    HiddenText,
    /// Hidden worksheet rows (§18.3.1.73 `row/@hidden`).
    HiddenRows,
    /// Hidden worksheet columns (§18.3.1.13 `col/@hidden`).
    HiddenColumns,
    /// Hidden worksheets (§18.18.68 `ST_SheetState`).
    HiddenSheets,
    /// Slides left out of the slide show (§19.3.1.38 `sld/@show`).
    HiddenSlides,
    /// Objects placed entirely outside the slide.
    OffSlide,
    /// Speaker notes.
    Notes,
    /// Headers and footers (and slide footer placeholders).
    HeadersFooters,
    /// Watermarks in headers.
    Watermarks,
    /// Objects marked invisible (`cNvPr/@hidden`, §20.1.2.2.8).
    InvisibleObjects,
    /// Embedded documents (OLE objects, packages).
    Embedded,
}

/// How a finding is dealt with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// Removed from the document model now (one undo step).
    Remove,
    /// Removed from the package when the file is saved.
    OnSave,
    /// Tracked changes: accept them all (a separate review command).
    AcceptAll,
    /// Reported only.
    None,
}

/// One category of the document inspector.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// What was looked for.
    pub category: Category,
    /// How many were found.
    pub count: usize,
    /// A few names / values found (author names, sheet names, …).
    pub details: Vec<String>,
    /// What the remove button does.
    pub action: Action,
    /// Removal is queued for the next save.
    pub pending: bool,
}

impl Finding {
    pub(crate) fn new(category: Category, count: usize, action: Action) -> Self {
        Finding {
            category,
            count,
            details: Vec::new(),
            action: if count == 0 { Action::None } else { action },
            pending: false,
        }
    }

    pub(crate) fn with_details(mut self, details: impl IntoIterator<Item = String>) -> Self {
        for d in details {
            if self.details.len() >= MAX_DETAILS {
                break;
            }
            if !d.is_empty() && !self.details.contains(&d) {
                self.details.push(d);
            }
        }
        self
    }
}

/// Severity of an accessibility issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// Content that people using assistive technology cannot read.
    Error,
    /// Content that is hard to read or navigate.
    Warning,
    /// Something worth improving.
    Tip,
}

/// Accessibility rule (BlueOffice's own rule set, after WCAG 2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Rule {
    /// Picture, shape, chart or diagram without alternative text (WCAG 1.1.1).
    AltText,
    /// Table without a marked header row (WCAG 1.3.1).
    TableHeader,
    /// Merged cells in a worksheet (WCAG 1.3.1).
    MergedCells,
    /// Heading levels skipped, e.g. a level 3 after a level 1 (WCAG 1.3.1).
    HeadingOrder,
    /// Long document without headings (WCAG 2.4.6).
    NoHeadings,
    /// Text colour too close to its background (WCAG 1.4.3).
    Contrast,
    /// Link without text, or whose text is the bare address (WCAG 2.4.4).
    LinkText,
    /// Slide without a title (WCAG 2.4.2).
    SlideTitle,
    /// Several slides with the same title (WCAG 2.4.2).
    DuplicateTitle,
    /// The title is not the first object read on the slide (WCAG 1.3.2).
    ReadingOrder,
    /// Worksheet tab still has its default name (WCAG 2.4.6).
    SheetName,
    /// Table with an entirely blank row or column, likely used for layout (WCAG 1.3.1).
    LayoutTable,
    /// The document has no title property (WCAG 2.4.2).
    DocumentTitle,
}

/// Where an issue is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Target {
    /// The document as a whole.
    Document,
    /// Body paragraph `para` (BlueDoc paragraph id).
    Paragraph {
        /// Paragraph id.
        para: usize,
    },
    /// Drawing object at index offset `at` of body paragraph `para` (BlueDoc).
    Drawing {
        /// Paragraph id.
        para: usize,
        /// Byte offset of the object in the paragraph's index text.
        at: usize,
    },
    /// Table whose first body paragraph is `para` (BlueDoc).
    Table {
        /// Paragraph id of the table's first cell paragraph.
        para: usize,
    },
    /// A slide (BlueSlide, 0-based).
    Slide {
        /// Slide index.
        slide: usize,
    },
    /// A top-level object of a slide (`cNvPr/@id`).
    Shape {
        /// Slide index.
        slide: usize,
        /// Shape id.
        id: u32,
    },
    /// A worksheet (BlueSheet, 0-based).
    Sheet {
        /// Sheet index.
        sheet: usize,
    },
    /// A cell range of a worksheet (0-based, inclusive).
    Range {
        /// Sheet index.
        sheet: usize,
        /// First row.
        r0: u32,
        /// First column.
        c0: u32,
        /// Last row.
        r1: u32,
        /// Last column.
        c1: u32,
    },
}

/// A quick fix offered with an issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FixKind {
    /// Ask for alternative text.
    AltText,
    /// Mark the first row as the header row.
    HeaderRow,
}

/// One accessibility issue.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// The rule.
    pub rule: Rule,
    /// How serious it is.
    pub severity: Severity,
    /// Where it is.
    pub target: Target,
    /// Object name, title or text sample shown with the issue.
    pub detail: String,
    /// Contrast ratio (`Contrast` issues), e.g. 2.8.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
    /// Quick fix the UI offers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixKind>,
}

impl Issue {
    pub(crate) fn new(
        rule: Rule,
        severity: Severity,
        target: Target,
        detail: impl Into<String>,
    ) -> Self {
        Issue {
            rule,
            severity,
            target,
            detail: detail.into(),
            ratio: None,
            fix: None,
        }
    }

    #[must_use]
    pub(crate) fn fix(mut self, fix: FixKind) -> Self {
        self.fix = Some(fix);
        self
    }
}

/// A quick fix request (`inspectFix`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Fix {
    /// Set the alternative text of the object at `target`.
    AltText {
        /// A `Drawing` (BlueDoc) or `Shape` (BlueSlide) target.
        target: Target,
        /// The alternative text.
        text: String,
    },
    /// Mark the first row of the table at `target` as its header row.
    HeaderRow {
        /// A `Table` (BlueDoc), `Shape` (BlueSlide) or `Range` (BlueSheet table) target.
        target: Target,
    },
}

/// Formats the compatibility checker knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    /// The binary formats of the 97–2003 generation (`.doc`, `.xls`, `.ppt`):
    /// limits of the format itself.
    Legacy,
    /// OpenDocument (`.odt`, `.ods`, `.odp`) as written by BlueOffice's converters.
    Odf,
}

/// A feature that does not survive saving in some formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Feature {
    /// Content controls (§17.5.2).
    ContentControls,
    /// Equations (Office Math, §22.1).
    Equations,
    /// SmartArt diagrams.
    SmartArt,
    /// Charts.
    Charts,
    /// Newer chart types (waterfall, treemap, … — chartex parts).
    ModernCharts,
    /// Tables.
    Tables,
    /// Pictures, shapes and text boxes.
    Drawings,
    /// Comments.
    Comments,
    /// Comment replies and resolved threads.
    CommentThreads,
    /// Tracked changes.
    TrackedChanges,
    /// Headers and footers.
    HeadersFooters,
    /// Footnotes and endnotes.
    Notes,
    /// Text effects (glow, outline, reflection — w14).
    TextEffects,
    /// Bookmark names longer than 40 characters.
    LongBookmarks,
    /// Tables wider than 63 columns.
    WideTables,
    /// Fields (converted to their result text).
    Fields,
    /// Cells beyond 65 536 rows or 256 columns.
    BeyondGrid,
    /// Functions newer than the 97–2003 function set.
    NewFunctions,
    /// Dynamic array formulas (spilled results).
    DynamicArrays,
    /// Slicers.
    Slicers,
    /// Timelines.
    Timelines,
    /// Sparklines.
    Sparklines,
    /// Conditional formatting.
    ConditionalFormats,
    /// Data bars, colour scales, icon sets or more than three rules per range.
    AdvancedConditionalFormats,
    /// Data validation.
    DataValidation,
    /// PivotTables.
    PivotTables,
    /// Sheet names longer than 31 characters.
    LongSheetNames,
    /// More than 4 000 cell formats.
    ManyCellFormats,
    /// Audio and video.
    Media,
    /// Slide transitions.
    Transitions,
    /// Morph transitions.
    Morph,
    /// Animations.
    Animations,
    /// Slide sections.
    Sections,
    /// 3D models.
    Models3d,
}

/// One row of the compatibility checker.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatItem {
    /// The feature.
    pub feature: Feature,
    /// How many times it occurs.
    pub count: usize,
    /// Formats in which it is lost or changed.
    pub formats: Vec<Format>,
}

/// The whole "Check document" report.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Document inspector, one entry per category the app supports.
    pub inspector: Vec<Finding>,
    /// Accessibility issues (at most [`MAX_ISSUES`]).
    pub accessibility: Vec<Issue>,
    /// Total number of accessibility issues.
    pub issue_count: usize,
    /// Compatibility checker (features present in the document only).
    pub compatibility: Vec<CompatItem>,
}

impl Report {
    pub(crate) fn new(
        mut inspector: Vec<Finding>,
        mut accessibility: Vec<Issue>,
        compatibility: Vec<CompatItem>,
        pending: &package::Scrub,
    ) -> Self {
        for f in &mut inspector {
            f.pending = pending.has(f.category);
        }
        accessibility.sort_by_key(|i| i.severity);
        let issue_count = accessibility.len();
        accessibility.truncate(MAX_ISSUES);
        Report {
            inspector,
            accessibility,
            issue_count,
            compatibility: compatibility.into_iter().filter(|c| c.count > 0).collect(),
        }
    }
}

/// Text shortened for a detail line.
pub(crate) fn sample(text: &str) -> String {
    const MAX: usize = 60;
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= MAX {
        t
    } else {
        let mut s: String = t.chars().take(MAX).collect();
        s.push('…');
        s
    }
}

/// The text looks like a bare web or e-mail address.
pub(crate) fn looks_like_url(text: &str) -> bool {
    let t = text.trim().to_ascii_lowercase();
    !t.contains(' ')
        && (t.starts_with("http://")
            || t.starts_with("https://")
            || t.starts_with("www.")
            || t.starts_with("mailto:")
            || t.starts_with("ftp://"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_text() {
        assert!(looks_like_url("https://example.com/a"));
        assert!(looks_like_url(" www.example.com "));
        assert!(!looks_like_url("Example site"));
    }

    #[test]
    fn samples_are_short() {
        assert_eq!(sample("  a \n b  "), "a b");
        assert_eq!(sample(&"x".repeat(100)).chars().count(), 61);
    }

    #[test]
    fn report_serializes_ids() {
        let r = Report::new(
            vec![Finding::new(Category::HiddenText, 2, Action::Remove)],
            vec![Issue::new(
                Rule::AltText,
                Severity::Error,
                Target::Drawing { para: 1, at: 0 },
                "",
            )
            .fix(FixKind::AltText)],
            vec![CompatItem {
                feature: Feature::Equations,
                count: 1,
                formats: vec![Format::Legacy],
            }],
            &package::Scrub::default(),
        );
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["inspector"][0]["category"], "hiddenText");
        assert_eq!(j["inspector"][0]["action"], "remove");
        assert_eq!(j["accessibility"][0]["target"]["kind"], "drawing");
        assert_eq!(j["accessibility"][0]["fix"], "altText");
        assert_eq!(j["compatibility"][0]["formats"][0], "legacy");
    }
}
