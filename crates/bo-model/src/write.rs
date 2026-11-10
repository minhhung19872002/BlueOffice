//! Word-processing document model (ECMA-376 Part 1, §17 WordprocessingML).
//!
//! Lossless round-trip (ADR-0005): blocks, section properties and unknown
//! property children carry their original XML ([`Raw`]). A block whose modelled
//! content is unchanged since reading is written back byte-for-byte.

use crate::units::{Emu, HalfPoints, Twips};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    /// Red channel (0–255).
    pub r: u8,
    /// Green channel (0–255).
    pub g: u8,
    /// Blue channel (0–255).
    pub b: u8,
}

impl Color {
    /// Pure black (`000000`).
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };
    /// Pure white (`FFFFFF`).
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
    };

    /// Parse `RRGGBB` (ECMA-376 Part 1, §17.18.38 `ST_HexColorRGB`). Returns `None` for `auto` or junk.
    pub fn from_hex(s: &str) -> Option<Color> {
        if s.len() != 6 || !s.is_ascii() {
            return None;
        }
        let p = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
        Some(Color {
            r: p(0)?,
            g: p(2)?,
            b: p(4)?,
        })
    }

    /// `RRGGBB` upper-case hex.
    pub fn to_hex(self) -> String {
        format!("{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Highlight colour names (ECMA-376 Part 1, §17.18.40 `ST_HighlightColor`).
    pub fn from_highlight(name: &str) -> Option<Color> {
        let (r, g, b) = match name {
            "black" => (0, 0, 0),
            "blue" => (0, 0, 255),
            "cyan" => (0, 255, 255),
            "green" => (0, 255, 0),
            "magenta" => (255, 0, 255),
            "red" => (255, 0, 0),
            "yellow" => (255, 255, 0),
            "white" => (255, 255, 255),
            "darkBlue" => (0, 0, 128),
            "darkCyan" => (0, 128, 128),
            "darkGreen" => (0, 128, 0),
            "darkMagenta" => (128, 0, 128),
            "darkRed" => (128, 0, 0),
            "darkYellow" => (128, 128, 0),
            "darkGray" => (128, 128, 128),
            "lightGray" => (192, 192, 192),
            _ => return None,
        };
        Some(Color { r, g, b })
    }

    /// Nearest highlight name for writing (`w:highlight`).
    pub fn highlight_name(self) -> &'static str {
        const NAMES: [&str; 16] = [
            "black",
            "blue",
            "cyan",
            "green",
            "magenta",
            "red",
            "yellow",
            "white",
            "darkBlue",
            "darkCyan",
            "darkGreen",
            "darkMagenta",
            "darkRed",
            "darkYellow",
            "darkGray",
            "lightGray",
        ];
        let mut best = ("yellow", u32::MAX);
        for n in NAMES {
            if let Some(c) = Color::from_highlight(n) {
                let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).unsigned_abs();
                let dist = d(c.r, self.r).pow(2) + d(c.g, self.g).pow(2) + d(c.b, self.b).pow(2);
                if dist < best.1 {
                    best = (n, dist);
                }
            }
        }
        best.0
    }
}

/// Original XML of an element, kept for lossless writing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Raw {
    /// The element's exact source text (namespace prefixes of the source part).
    pub xml: Arc<str>,
    /// Fingerprint of the modelled content when it was read (0 = not applicable).
    pub fingerprint: u64,
}

impl Raw {
    /// Wrap source XML with the fingerprint of its modelled content.
    pub fn new(xml: impl Into<Arc<str>>, fingerprint: u64) -> Raw {
        Raw {
            xml: xml.into(),
            fingerprint,
        }
    }
}

/// Stable hash used for fingerprints (FNV-1a over a `Hash` impl; deterministic across runs and targets).
pub fn fingerprint<T: std::hash::Hash + ?Sized>(v: &T) -> u64 {
    struct Fnv(u64);
    impl std::hash::Hasher for Fnv {
        fn finish(&self) -> u64 {
            self.0
        }
        fn write(&mut self, bytes: &[u8]) {
            for b in bytes {
                self.0 ^= u64::from(*b);
                self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    v.hash(&mut h);
    std::hash::Hasher::finish(&h)
}

/// Where the document was read from (enables lossless saving).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePackage {
    /// Original package bytes.
    #[serde(skip)]
    pub bytes: Arc<Vec<u8>>,
    /// Main document part name (e.g. `/word/document.xml`).
    pub main_part: String,
    /// Text of the main part before the first body child (prolog, root element, `<w:body>`).
    pub body_prefix: String,
    /// Text after the last body child (`</w:body>…</w:document>`).
    pub body_suffix: String,
    /// Fingerprint of styles / numbering as read (unchanged ⇒ keep the parts).
    pub styles_fingerprint: u64,
    /// Fingerprint of the numbering definitions as read (unchanged ⇒ keep `numbering.xml`).
    pub numbering_fingerprint: u64,
    /// Fingerprint of [`Document::comments`] as read (unchanged ⇒ keep the comment parts).
    pub comments_fingerprint: u64,
    /// Root start tag of the source `comments.xml` (its namespace declarations).
    pub comments_root: Option<String>,
    /// The source has a `commentsIds` part ([MS-DOCX] durable ids).
    pub comments_ids: bool,
    /// Fingerprint of the footnotes and endnotes as read (changed ⇒ note parts rewritten).
    #[serde(default)]
    pub notes_fingerprint: u64,
    /// `evenAndOddHeaders` as read (changed ⇒ the settings part is patched).
    #[serde(default)]
    pub even_and_odd_headers: bool,
    /// Page colour as read (changed ⇒ `document.xml`'s §17.2.1 background is patched).
    #[serde(default)]
    pub background: Option<Color>,
    /// §17.15.1.32 displayBackgroundShape as read (changed ⇒ settings patched).
    #[serde(default)]
    pub display_background_shape: bool,
    /// `documentProtection` as read (changed ⇒ the settings part is patched).
    #[serde(default)]
    pub protection: Option<crate::form::DocProtection>,
    /// Hyphenation settings as read (changed ⇒ settings patched).
    #[serde(default)]
    pub hyphenation: Hyphenation,
    /// §17.15.1.89 trackRevisions as read (changed ⇒ settings patched).
    #[serde(default)]
    pub track_revisions: bool,
    /// Custom XML parts as read (ECMA-376 Part 1, §15.2.4; changed ⇒ parts rewritten).
    #[serde(default)]
    pub custom_xml: Vec<crate::custom_xml::CustomXmlPart>,
}

/// A whole word-processing document.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Document {
    /// Sections in document order; each holds its blocks and `w:sectPr` properties.
    pub sections: Vec<Section>,
    /// Document-wide default run properties (`w:docDefaults/w:rPrDefault`).
    pub default_run: RunProps,
    /// Document-wide default paragraph properties (`w:docDefaults/w:pPrDefault`).
    pub default_para: ParaProps,
    /// Style definitions (`styles.xml`).
    pub styles: StyleSheet,
    /// Numbering definitions (`numbering.xml`).
    pub numbering: Numbering,
    /// Footnotes / endnotes by id (ECMA-376 Part 1, §17.11).
    pub footnotes: BTreeMap<i32, Vec<Block>>,
    /// Endnotes by id (ECMA-376 Part 1, §17.11).
    pub endnotes: BTreeMap<i32, Vec<Block>>,
    /// Binary parts referenced by images, keyed by part name (`/word/media/image1.png`).
    #[serde(skip)]
    pub media: BTreeMap<String, Arc<Vec<u8>>>,
    /// `w:evenAndOddHeaders` (§17.10.1).
    pub even_and_odd_headers: bool,
    /// `w:defaultTabStop` (§17.15.1.25), default 720.
    pub default_tab_stop: Option<Twips>,
    /// Comments (ECMA-376 Part 1, §17.13.4 `comments.xml`), in part order.
    pub comments: Vec<Comment>,
    /// Font scheme of the document theme (`word/theme/theme1.xml`, ECMA-376 Part 1
    /// §20.1.4.1.18 fontScheme) used to resolve theme fonts of runs.
    #[serde(default)]
    pub theme_fonts: ThemeFonts,
    /// Automatic hyphenation settings (§17.15.1.10 autoHyphenation, …).
    #[serde(default)]
    pub hyphenation: Hyphenation,
    /// Document-wide footnote properties (ECMA-376 Part 1, §17.11.11 footnotePr in
    /// `settings.xml`); a section's own footnotePr overrides them.
    #[serde(default)]
    pub footnote_pr: NoteProps,
    /// Document-wide endnote properties (§17.11.4 endnotePr in `settings.xml`).
    #[serde(default)]
    pub endnote_pr: NoteProps,
    /// Content of the footnote continuation notice (a `w:footnote` of `w:type`
    /// `continuationNotice`, §17.18.33 `ST_FtnEdn`): shown below a footnote that
    /// continues on the next page. Written back when the footnotes part is
    /// rewritten; shared in collaboration as a document setting.
    #[serde(skip)]
    pub continuation_notice: Option<Vec<Block>>,
    /// Page colour, theme part and related settings (Design tab).
    #[serde(default)]
    pub design: DocDesign,
    /// Editing restrictions (ECMA-376 Part 1, §17.15.1.29 documentProtection).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protection: Option<crate::form::DocProtection>,
    /// Track changes is on (ECMA-376 Part 1, §17.15.1.89 trackRevisions in settings).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub track_revisions: bool,
    /// Custom XML data storage parts (ECMA-376 Part 1, §15.2.4) with their properties
    /// (§15.2.5), in relationship order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_xml: Vec<crate::custom_xml::CustomXmlPart>,
    /// Source package the document was read from (`None` for a new document).
    pub source: Option<Arc<SourcePackage>>,
}

/// Document-wide page background and theme (Design tab).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocDesign {
    /// Page colour (ECMA-376 Part 1, §17.2.1 background `w:color`); `None` = none.
    pub background: Option<Color>,
    /// §17.15.1.32 displayBackgroundShape: show the background in print layout.
    #[serde(default)]
    pub display_background_shape: bool,
    /// Theme part authored by BlueOffice (`word/theme/theme1.xml`, DrawingML
    /// §20.1.6.9 theme) replacing the one of the source package; `None` keeps it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_xml: Option<Arc<str>>,
    /// Id of the BlueOffice theme applied (`theme_xml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_id: Option<String>,
    /// Id of the paragraph-spacing style set applied last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_set: Option<String>,
}

/// Major/minor typefaces of a theme font scheme (§20.1.4.1.24 majorFont,
/// §20.1.4.1.25 minorFont: `a:latin`, `a:ea`, `a:cs`). Empty = not given.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ThemeFonts {
    /// Major (heading) Latin typeface (`a:majorFont/a:latin/@typeface`).
    pub major_latin: String,
    /// Major East Asian typeface (`a:majorFont/a:ea`).
    pub major_east_asia: String,
    /// Major complex-script typeface (`a:majorFont/a:cs`).
    pub major_cs: String,
    /// Minor (body) Latin typeface (`a:minorFont/a:latin/@typeface`).
    pub minor_latin: String,
    /// Minor East Asian typeface (`a:minorFont/a:ea`).
    pub minor_east_asia: String,
    /// Minor complex-script typeface (`a:minorFont/a:cs`).
    pub minor_cs: String,
}

impl ThemeFonts {
    /// Typeface of an `ST_Theme` value (§17.18.96: `majorAscii`, `minorHAnsi`,
    /// `majorEastAsia`, `minorBidi`, …); `None` when unknown or empty.
    pub fn resolve(&self, theme: &str) -> Option<&str> {
        let (major, rest) = if let Some(r) = theme.strip_prefix("major") {
            (true, r)
        } else {
            (false, theme.strip_prefix("minor")?)
        };
        let f = match (major, rest) {
            (true, "Ascii" | "HAnsi") => &self.major_latin,
            (true, "EastAsia") => &self.major_east_asia,
            (true, "Bidi") => &self.major_cs,
            (false, "Ascii" | "HAnsi") => &self.minor_latin,
            (false, "EastAsia") => &self.minor_east_asia,
            (false, "Bidi") => &self.minor_cs,
            _ => return None,
        };
        (!f.is_empty()).then_some(f.as_str())
    }
}

/// Attribution of a tracked change (ECMA-376 Part 1, §17.13.5 `CT_TrackChange`:
/// `w:id`, `w:author`, `w:date`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RevisionInfo {
    /// Revision id (`w:id`).
    pub id: i32,
    /// Author name (`w:author`).
    pub author: String,
    /// `ST_DateTime` (ISO 8601, e.g. `2026-11-08T09:30:00Z`).
    pub date: Option<String>,
}

/// Insertion or deletion of content (§17.13.5.18 ins, §17.13.5.14 del) or one side of
/// a move (§17.13.5.25 moveTo, §17.13.5.22 moveFrom) when `moved`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RevMark {
    /// Who made the change and when.
    pub info: RevisionInfo,
    /// The mark is one side of a move (`w:moveFrom` / `w:moveTo`) rather than a plain `w:del` / `w:ins`.
    pub moved: bool,
}

/// Previous run properties of a formatting revision (§17.13.5.31 rPrChange).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RunPropsChange {
    /// Who made the change and when.
    pub info: RevisionInfo,
    /// Run properties before the change.
    pub old: RunProps,
}

/// Previous paragraph properties of a formatting revision (§17.13.5.29 pPrChange).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ParaPropsChange {
    /// Who made the change and when.
    pub info: RevisionInfo,
    /// Paragraph properties before the change.
    pub old: ParaProps,
}

/// A comment (ECMA-376 Part 1, §17.13.4.2 comment) with the threading extension of
/// [MS-DOCX] §2.5.2 (`w15:commentEx`: `paraId`, `paraIdParent`, `done`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Comment {
    /// Comment id (`w:id`), referenced by the comment range and reference marks.
    pub id: i32,
    /// Author name (`w:author`).
    pub author: String,
    /// Author initials (`w:initials`).
    pub initials: Option<String>,
    /// Creation date (`w:date`, ISO 8601).
    pub date: Option<String>,
    /// Comment body content.
    pub blocks: Vec<Block>,
    /// `w14:paraId` of the comment's last paragraph (key of `w15:commentEx`).
    pub para_id: Option<String>,
    /// `w15:paraIdParent`: the comment this one replies to.
    pub parent_para_id: Option<String>,
    /// `w15:done`: the thread is resolved.
    pub done: bool,
    /// `w16cid:durableId` ([MS-DOCX] §2.6.x commentsIds), kept when present.
    pub durable_id: Option<String>,
}

impl Comment {
    /// Plain text of the comment (paragraphs joined with `\n`; the annotation mark and
    /// other verbatim objects are left out).
    pub fn text(&self) -> String {
        let mut v = Vec::new();
        blocks_text(&self.blocks, &mut v);
        v.join("\n").replace('\u{FFFC}', "")
    }
}

impl Document {
    /// Iterate over top-level paragraphs in document order (tables are skipped).
    pub fn paragraphs(&self) -> impl Iterator<Item = &Paragraph> {
        self.sections.iter().flat_map(|s| {
            s.blocks.iter().filter_map(|b| match b {
                Block::Paragraph(p) => Some(p),
                _ => None,
            })
        })
    }

    /// Plain text of the document (tables included, cells separated by tabs), paragraphs separated by `\n`.
    pub fn plain_text(&self) -> String {
        let mut out = Vec::new();
        for s in &self.sections {
            blocks_text(&s.blocks, &mut out);
        }
        out.join("\n")
    }

    /// Look up a style by id.
    pub fn style(&self, id: &str) -> Option<&Style> {
        self.styles.styles.iter().find(|s| s.id == id)
    }

    /// Effective footnote properties of section `si`: the section's own footnotePr
    /// over the document-wide one (§17.11.11).
    pub fn footnote_props(&self, si: usize) -> NoteProps {
        match self.sections.get(si) {
            Some(s) => s.footnote_pr.over(&self.footnote_pr),
            None => self.footnote_pr.clone(),
        }
    }

    /// Effective endnote properties of section `si` (section endnotePr over §17.11.4).
    pub fn endnote_props(&self, si: usize) -> NoteProps {
        match self.sections.get(si) {
            Some(s) => s.endnote_pr.over(&self.endnote_pr),
            None => self.endnote_pr.clone(),
        }
    }
}

/// Words (whitespace-separated) of `blocks` as [`Document::plain_text`] counts them.
pub fn blocks_word_count(blocks: &[Block]) -> usize {
    let mut v = Vec::new();
    blocks_text(blocks, &mut v);
    v.iter().map(|t| t.split_whitespace().count()).sum()
}

fn blocks_text(blocks: &[Block], out: &mut Vec<String>) {
    // Text of paragraphs whose mark is a tracked deletion (§17.13.5.15) joins the next one.
    let mut carry = String::new();
    for (i, b) in blocks.iter().enumerate() {
        match b {
            Block::Paragraph(p) => {
                carry.push_str(&p.plain_text());
                let joins = p.props.mark_del.is_some()
                    && matches!(blocks.get(i + 1), Some(Block::Paragraph(_)));
                if !joins {
                    out.push(std::mem::take(&mut carry));
                }
            }
            Block::Table(t) => {
                for r in &t.rows {
                    let cells: Vec<String> = r
                        .cells
                        .iter()
                        .map(|c| {
                            let mut v = Vec::new();
                            blocks_text(&c.blocks, &mut v);
                            v.join(" ")
                        })
                        .collect();
                    out.push(cells.join("\t"));
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Section break kind (ECMA-376 Part 1, §17.18.77 `ST_SectionMark`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum SectionBreak {
    /// Start on the next page (`nextPage`, default).
    #[default]
    NextPage,
    /// Start on the same page (`continuous`).
    Continuous,
    /// Start on the next even-numbered page (`evenPage`).
    EvenPage,
    /// Start on the next odd-numbered page (`oddPage`).
    OddPage,
    /// Start in the next column (`nextColumn`).
    NextColumn,
}

/// Text columns (ECMA-376 Part 1, §17.6.4 cols).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Columns {
    /// Number of columns (`w:num`, at most 45).
    pub count: u16,
    /// Gap between columns (`w:space`), in twips (equal-width columns).
    pub space: Twips,
    /// §17.6.4 `w:sep`: a vertical line is drawn between the columns.
    #[serde(default)]
    pub separator: bool,
    /// Columns of unequal width (`w:equalWidth="0"` with one §17.6.3 `w:col` per
    /// column, in order); empty = `count` equal columns `space` apart.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cols: Vec<ColumnDef>,
}

/// One text column of unequal-width columns (ECMA-376 Part 1, §17.6.3 col).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColumnDef {
    /// Column width (`w:w`), in twips.
    pub width: Twips,
    /// Space after the column (`w:space`), in twips.
    pub space: Twips,
}

impl Default for Columns {
    fn default() -> Self {
        Columns {
            count: 1,
            space: Twips(720),
            separator: false,
            cols: Vec::new(),
        }
    }
}

impl Columns {
    /// `count` equal columns `space` apart.
    #[must_use]
    pub fn equal(count: u16, space: Twips) -> Columns {
        Columns {
            count: count.clamp(1, 45),
            space,
            ..Columns::default()
        }
    }

    /// The column widths are given per column (§17.6.4 `w:equalWidth` false).
    #[must_use]
    pub fn unequal(&self) -> bool {
        !self.cols.is_empty()
    }

    /// Number of columns in use (1 to 45).
    #[must_use]
    pub fn len(&self) -> usize {
        if self.cols.is_empty() {
            usize::from(self.count.clamp(1, 45))
        } else {
            self.cols.len().min(45)
        }
    }

    /// Never empty: a section has at least one column.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }
}

/// Restart rule of line numbers (ECMA-376 Part 1, §17.18.47 `ST_LineNumberRestart`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineNumberRestart {
    /// Restart on each page (`newPage`, the default).
    #[default]
    NewPage,
    /// Restart at the start of each section (`newSection`).
    NewSection,
    /// Continue from the previous section (`continuous`).
    Continuous,
}

impl LineNumberRestart {
    /// The `ST_LineNumberRestart` value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            LineNumberRestart::NewPage => "newPage",
            LineNumberRestart::NewSection => "newSection",
            LineNumberRestart::Continuous => "continuous",
        }
    }

    /// Parse an `ST_LineNumberRestart` value (unknown = `newPage`).
    #[must_use]
    pub fn parse(s: &str) -> LineNumberRestart {
        match s {
            "newSection" => LineNumberRestart::NewSection,
            "continuous" => LineNumberRestart::Continuous,
            _ => LineNumberRestart::NewPage,
        }
    }
}

/// Line numbering of a section (ECMA-376 Part 1, §17.6.8 lnNumType).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineNumbering {
    /// `w:countBy`: numbers shown on multiples of this value (1 = every line).
    pub count_by: u16,
    /// `w:start` as stored: the first number is `start + 1` (files written by
    /// common word processors store the starting number less one; an absent
    /// attribute starts at 1).
    pub start: u32,
    /// `w:distance`: gap between the numbers and the text; `None` = automatic.
    pub distance: Option<Twips>,
    /// `w:restart`.
    pub restart: LineNumberRestart,
}

impl Default for LineNumbering {
    fn default() -> Self {
        LineNumbering {
            count_by: 1,
            start: 0,
            distance: None,
            restart: LineNumberRestart::NewPage,
        }
    }
}

/// Automatic hyphenation settings (ECMA-376 Part 1, §17.15.1.10 autoHyphenation,
/// §17.15.1.52 hyphenationZone, §17.15.1.21 consecutiveHyphenLimit, §17.15.1.40
/// doNotHyphenateCaps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hyphenation {
    /// Hyphenate automatically.
    pub auto: bool,
    /// Hyphenation zone: the line-end gap below which words are not hyphenated
    /// (`None` = the default, 0.25 inch).
    pub zone: Option<Twips>,
    /// Most consecutive hyphenated lines (`None` or 0 = no limit).
    pub consecutive_limit: Option<u16>,
    /// Do not hyphenate words in capitals.
    pub no_caps: bool,
}

impl Hyphenation {
    /// Default hyphenation zone (0.25 inch).
    pub const DEFAULT_ZONE: Twips = Twips(360);
}

/// Position of a drop cap (ECMA-376 Part 1, §17.18.20 `ST_DropCap`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DropCap {
    /// In the text: the following lines are indented beside it (`drop`).
    Drop,
    /// In the left margin (`margin`).
    Margin,
}

impl DropCap {
    /// The `ST_DropCap` value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            DropCap::Drop => "drop",
            DropCap::Margin => "margin",
        }
    }
}

/// Text frame of a drop cap paragraph (ECMA-376 Part 1, §17.3.1.11 framePr with
/// `w:dropCap`). Other text frames stay verbatim in [`ParaProps::unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameProps {
    /// `w:dropCap`.
    pub drop_cap: DropCap,
    /// `w:lines`: height of the drop cap in lines of the following paragraph.
    pub lines: u8,
    /// `w:hSpace`: distance from the text.
    pub h_space: Twips,
}

/// Header or footer variants (§17.10.5 headerReference).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct HeaderFooter {
    /// Header/footer used on all pages without a specific variant (`w:type="default"`).
    pub default: Option<Vec<Block>>,
    /// First-page header/footer (`w:type="first"`, used with `w:titlePg`).
    pub first: Option<Vec<Block>>,
    /// Even-page header/footer (`w:type="even"`, used with `w:evenAndOddHeaders`).
    pub even: Option<Vec<Block>>,
}

/// A section (ECMA-376 Part 1, §17.6 Sections).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Section {
    /// Page size and margins.
    pub page: PageSetup,
    /// Block-level content of the section.
    pub blocks: Vec<Block>,
    /// Text columns (`w:cols`).
    pub columns: Columns,
    /// Header variants (`w:headerReference`).
    pub headers: HeaderFooter,
    /// Footer variants (`w:footerReference`).
    pub footers: HeaderFooter,
    /// `w:titlePg` — distinct first page header/footer.
    pub title_page: bool,
    /// Kind of break that starts this section (`w:type`).
    pub break_type: SectionBreak,
    /// `w:pgNumType/@w:start`.
    pub page_number_start: Option<u32>,
    /// §17.11.11 footnotePr of the section.
    #[serde(default)]
    pub footnote_pr: NoteProps,
    /// §17.11.4 endnotePr of the section.
    #[serde(default)]
    pub endnote_pr: NoteProps,
    /// Tracked change of the section properties (§17.13.5.32 sectPrChange).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<Box<SectionChange>>,
    /// Page borders (ECMA-376 Part 1, §17.6.10 pgBorders).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_borders: Option<PageBorders>,
    /// Line numbering (ECMA-376 Part 1, §17.6.8 lnNumType); `None` = off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_numbers: Option<LineNumbering>,
    /// Direction of the text of the section's pages (ECMA-376 Part 1, §17.6.20
    /// textDirection); `None` = horizontal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_direction: Option<TextDirection>,
    /// Source parts of the header/footer variants as read: (header?, `ST_HdrFtr`
    /// kind, part name). Rewritten header parts start from their relationships so
    /// verbatim content keeps resolving its `r:id`s.
    #[serde(skip)]
    pub hf_parts: HfParts,
    /// §17.6.6 formProt `w:val="0"`: the section stays editable while filling in forms is
    /// enforced (omitted ⇒ protected).
    #[serde(default)]
    pub form_unprotected: bool,
    /// Original `w:sectPr` XML.
    #[serde(skip)]
    pub src: Option<Raw>,
}

/// Source parts of a section's headers and footers: (header?, `ST_HdrFtr` kind, part
/// name). Provenance only — two sections compare equal whatever parts they came from.
#[derive(Debug, Clone, Default)]
pub struct HfParts(pub Vec<(bool, String, String)>);

impl PartialEq for HfParts {
    fn eq(&self, _: &HfParts) -> bool {
        true
    }
}

impl HfParts {
    /// Part name of the header (`header`) or footer of `kind` as read.
    pub fn part(&self, header: bool, kind: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(h, k, _)| *h == header && k == kind)
            .map(|(_, _, p)| p.as_str())
    }
}

impl Section {
    /// Section with default page setup and the given blocks.
    pub fn new(blocks: Vec<Block>) -> Section {
        Section {
            blocks,
            ..Section::default()
        }
    }

    /// `other` equals `self` except for the headers, footers and §17.10.6 titlePg
    /// (what header/footer edits change; the body pagination does not depend on them
    /// beyond their heights).
    pub fn same_except_headers(&self, other: &Section) -> bool {
        // Destructured so that a new field must be classified here.
        let Section {
            page,
            blocks,
            columns,
            headers: _,
            footers: _,
            title_page: _,
            break_type,
            page_number_start,
            footnote_pr,
            endnote_pr,
            change,
            page_borders,
            line_numbers,
            text_direction,
            hf_parts: _,
            form_unprotected,
            src,
        } = self;
        *page == other.page
            && *page_borders == other.page_borders
            && *line_numbers == other.line_numbers
            && *text_direction == other.text_direction
            && *columns == other.columns
            && *break_type == other.break_type
            && *page_number_start == other.page_number_start
            && *footnote_pr == other.footnote_pr
            && *endnote_pr == other.endnote_pr
            && *change == other.change
            && *form_unprotected == other.form_unprotected
            && *src == other.src
            && *blocks == other.blocks
    }

    /// Fingerprint of the modelled section properties (excluding blocks).
    pub fn props_fingerprint(&self) -> u64 {
        fingerprint(&(
            self.page,
            &self.columns,
            &self.headers,
            &self.footers,
            self.title_page,
            self.break_type,
            self.page_number_start,
            &self.footnote_pr,
            &self.endnote_pr,
            &self.change,
            &self.page_borders,
            (
                self.form_unprotected,
                self.line_numbers,
                self.text_direction,
            ),
        ))
    }

    /// Snapshot of the properties a section property change records (§17.13.5.32).
    pub fn change_snapshot(&self, info: RevisionInfo) -> SectionChange {
        SectionChange {
            info,
            page: self.page,
            columns: self.columns.clone(),
            title_page: self.title_page,
            break_type: self.break_type,
            page_number_start: self.page_number_start,
        }
    }
}

/// Footnote or endnote numbering and placement (ECMA-376 Part 1, §17.11.11 footnotePr,
/// §17.11.4 endnotePr); `None` = inherited from the document settings / default.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct NoteProps {
    /// §17.11.21 pos (footnotes, `ST_FtnPos`) / §17.11.22 pos (endnotes, `ST_EdnPos`).
    pub pos: Option<NotePos>,
    /// §17.11.17 numFmt.
    pub fmt: Option<NumFormat>,
    /// §17.11.20 numStart.
    pub start: Option<u32>,
    /// §17.11.19 numRestart (`ST_RestartNumber`).
    pub restart: Option<NoteRestart>,
}

impl NoteProps {
    /// Nothing set.
    pub fn is_empty(&self) -> bool {
        *self == NoteProps::default()
    }

    /// `self` with the unset properties taken from `base`.
    #[must_use]
    pub fn over(&self, base: &NoteProps) -> NoteProps {
        NoteProps {
            pos: self.pos.or(base.pos),
            fmt: self.fmt.clone().or_else(|| base.fmt.clone()),
            start: self.start.or(base.start),
            restart: self.restart.or(base.restart),
        }
    }
}

/// Note placement (§17.18.30 `ST_FtnPos`, §17.18.21 `ST_EdnPos`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotePos {
    /// Footnotes at the bottom of the page (default).
    PageBottom,
    /// Footnotes right below the last text line of the page.
    BeneathText,
    /// Endnotes at the end of the section.
    SectEnd,
    /// Endnotes at the end of the document (default).
    DocEnd,
}

impl NotePos {
    /// Parse an `ST_FtnPos` / `ST_EdnPos` value; `None` if unknown.
    pub fn parse(s: &str) -> Option<NotePos> {
        Some(match s {
            "pageBottom" => NotePos::PageBottom,
            "beneathText" => NotePos::BeneathText,
            "sectEnd" => NotePos::SectEnd,
            "docEnd" => NotePos::DocEnd,
            _ => return None,
        })
    }
    /// Serialized attribute value.
    pub fn as_str(self) -> &'static str {
        match self {
            NotePos::PageBottom => "pageBottom",
            NotePos::BeneathText => "beneathText",
            NotePos::SectEnd => "sectEnd",
            NotePos::DocEnd => "docEnd",
        }
    }
}

/// Note numbering restart (§17.18.74 `ST_RestartNumber`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteRestart {
    /// Numbering continues across sections (`continuous`).
    Continuous,
    /// Numbering restarts in each section (`eachSect`).
    EachSect,
    /// Numbering restarts on each page (`eachPage`).
    EachPage,
}

impl NoteRestart {
    /// Parse an `ST_RestartNumber` value; `None` if unknown.
    pub fn parse(s: &str) -> Option<NoteRestart> {
        Some(match s {
            "continuous" => NoteRestart::Continuous,
            "eachSect" => NoteRestart::EachSect,
            "eachPage" => NoteRestart::EachPage,
            _ => return None,
        })
    }
    /// Serialized attribute value.
    pub fn as_str(self) -> &'static str {
        match self {
            NoteRestart::Continuous => "continuous",
            NoteRestart::EachSect => "eachSect",
            NoteRestart::EachPage => "eachPage",
        }
    }
}

/// Page size and margins (ECMA-376 Part 1, §17.6.13 pgSz, §17.6.11 pgMar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageSetup {
    /// Page width (`w:pgSz/@w:w`), in twips.
    pub width: Twips,
    /// Page height (`w:pgSz/@w:h`), in twips.
    pub height: Twips,
    /// Top margin (`w:pgMar/@w:top`), in twips.
    pub margin_top: Twips,
    /// Bottom margin (`w:pgMar/@w:bottom`), in twips.
    pub margin_bottom: Twips,
    /// Left margin (`w:pgMar/@w:left`), in twips.
    pub margin_left: Twips,
    /// Right margin (`w:pgMar/@w:right`), in twips.
    pub margin_right: Twips,
    /// Distance from the page top to the header (`w:pgMar/@w:header`), in twips.
    pub header: Twips,
    /// Distance from the page bottom to the footer (`w:pgMar/@w:footer`), in twips.
    pub footer: Twips,
}

impl Default for PageSetup {
    /// A4 portrait with the margins recommended by Nghị định 30/2020/NĐ-CP
    /// (top/bottom 20 mm, left 30 mm, right 15 mm).
    fn default() -> Self {
        PageSetup {
            width: Twips(11_906),
            height: Twips(16_838),
            margin_top: Twips(1134),
            margin_bottom: Twips(1134),
            margin_left: Twips(1701),
            margin_right: Twips(850),
            header: Twips(720),
            footer: Twips(720),
        }
    }
}

impl PageSetup {
    /// Landscape when wider than tall.
    pub fn is_landscape(&self) -> bool {
        self.width > self.height
    }
}

/// Block-level content.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Block {
    /// A paragraph (`w:p`).
    Paragraph(Paragraph),
    /// A table (`w:tbl`).
    Table(Table),
    /// Unknown block-level element kept verbatim (`w:sdt`, `w:customXml`, …).
    Opaque(Raw),
}

impl Block {
    /// The paragraph, if this block is one.
    pub fn as_paragraph(&self) -> Option<&Paragraph> {
        match self {
            Block::Paragraph(p) => Some(p),
            _ => None,
        }
    }
}

/// A paragraph (ECMA-376 Part 1, §17.3.1.22 p).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Paragraph {
    /// Paragraph properties (`w:pPr`).
    pub props: ParaProps,
    /// Runs and inline content in order.
    pub runs: Vec<Run>,
    /// Original `w:p` XML.
    #[serde(skip)]
    pub src: Option<Raw>,
}

impl Paragraph {
    /// Paragraph with the given properties and runs.
    pub fn new(props: ParaProps, runs: Vec<Run>) -> Paragraph {
        Paragraph {
            props,
            runs,
            src: None,
        }
    }

    /// Fingerprint of the modelled content.
    pub fn content_fingerprint(&self) -> u64 {
        fingerprint(&(&self.props, &self.runs))
    }

    /// Concatenated text; tabs as `\t`, line breaks as `\u{2028}`. Tracked deletions
    /// (§17.13.5.14) are left out, i.e. this is the text with all changes accepted.
    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        for r in self.runs.iter().filter(|r| r.props.del.is_none()) {
            for c in &r.content {
                c.push_text(&mut s);
            }
        }
        s
    }
}

/// Horizontal alignment (ECMA-376 Part 1, §17.18.44 `ST_Jc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Alignment {
    /// Left-aligned (`left` / `start`).
    #[default]
    Left,
    /// Centred (`center`).
    Center,
    /// Right-aligned (`right` / `end`).
    Right,
    /// Justified (`both`).
    Justify,
}

/// Line spacing rule (ECMA-376 Part 1, §17.3.1.33 spacing, §17.18.48 `ST_LineSpacingRule`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineSpacing {
    /// Multiple of single spacing in 240ths (240 = single).
    Auto(u32),
    /// Exact line height.
    Exact(Twips),
    /// Minimum line height.
    AtLeast(Twips),
}

impl Default for LineSpacing {
    fn default() -> Self {
        LineSpacing::Auto(240)
    }
}

/// Tab stop alignment (§17.18.84 `ST_TabJc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TabAlign {
    /// Left-aligned tab.
    Left,
    /// Centred tab.
    Center,
    /// Right-aligned tab.
    Right,
    /// Decimal-aligned tab.
    Decimal,
    /// Bar tab (vertical line at the position).
    Bar,
    /// Clears an inherited tab stop at this position.
    Clear,
}

/// Tab leader (§17.18.85 `ST_TabTlc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TabLeader {
    /// No leader.
    None,
    /// Dotted leader.
    Dot,
    /// Hyphen leader.
    Hyphen,
    /// Underscore leader.
    Underscore,
}

/// Custom tab stop (§17.3.1.37 tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabStop {
    /// Tab position (`w:pos`), in twips.
    pub pos: Twips,
    /// Tab alignment (`w:val`).
    pub align: TabAlign,
    /// Leader character (`w:leader`).
    pub leader: TabLeader,
}

/// Border line (§17.3.4 / §17.18.2 `ST_Border`). `size` in eighths of a point.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Border {
    /// Line style (`w:val`, e.g. `single`, `double`, `nil`).
    pub style: String,
    /// Line width (`w:sz`), in eighths of a point.
    pub size: u32,
    /// Line colour (`w:color`); `None` = auto.
    pub color: Option<Color>,
    /// Spacing between the border and the content (`w:space`), in points.
    pub space: u32,
}

impl Border {
    /// Single line of `size` eighths of a point.
    pub fn single(size: u32) -> Border {
        Border {
            style: "single".into(),
            size,
            color: None,
            space: 0,
        }
    }
    /// Whether the border draws anything.
    pub fn is_visible(&self) -> bool {
        !matches!(self.style.as_str(), "nil" | "none") && self.size > 0
    }
}

/// Line styles of `ST_Border` (ECMA-376 Part 1, §17.18.2); every other value names
/// an art border (a repeated picture motif).
const LINE_BORDERS: &[&str] = &[
    "nil",
    "none",
    "single",
    "thick",
    "double",
    "dotted",
    "dashed",
    "dotDash",
    "dotDotDash",
    "triple",
    "thinThickSmallGap",
    "thickThinSmallGap",
    "thinThickThinSmallGap",
    "thinThickMediumGap",
    "thickThinMediumGap",
    "thinThickThinMediumGap",
    "thinThickLargeGap",
    "thickThinLargeGap",
    "thinThickThinLargeGap",
    "wave",
    "doubleWave",
    "dashSmallGap",
    "dashDotStroked",
    "threeDEmboss",
    "threeDEngrave",
    "outset",
    "inset",
];

impl Border {
    /// The style is an art border (§17.18.2 `ST_Border` values such as
    /// `basicBlackDots`), whose `w:sz` is in points rather than eighths of a point.
    pub fn is_art(&self) -> bool {
        !LINE_BORDERS.contains(&self.style.as_str())
    }
}

/// One edge of the page borders (ECMA-376 Part 1, §17.6.21 top, §17.6.2 bottom,
/// §17.6.7 left, §17.6.15 right: `CT_Border` with `w:shadow` and `w:frame`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageBorder {
    /// Style, width (eighths of a point; points for art borders), colour and
    /// spacing (points) of the edge.
    pub line: Border,
    /// `w:shadow`: the edge is drawn with a shadow.
    #[serde(default)]
    pub shadow: bool,
    /// `w:frame`: the edge looks three-dimensional (frame effect).
    #[serde(default)]
    pub frame: bool,
}

/// Pages of a section showing its page borders (§17.18.63 `ST_PageBorderDisplay`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PageBorderDisplay {
    /// Every page (`allPages`, default).
    #[default]
    AllPages,
    /// Only the first page of the section (`firstPage`).
    FirstPage,
    /// Every page but the first (`notFirstPage`).
    NotFirstPage,
}

impl PageBorderDisplay {
    /// Parse an `ST_PageBorderDisplay` value (unknown ⇒ all pages).
    pub fn parse(s: &str) -> PageBorderDisplay {
        match s {
            "firstPage" => PageBorderDisplay::FirstPage,
            "notFirstPage" => PageBorderDisplay::NotFirstPage,
            _ => PageBorderDisplay::AllPages,
        }
    }
    /// Serialized attribute value.
    pub fn as_str(self) -> &'static str {
        match self {
            PageBorderDisplay::AllPages => "allPages",
            PageBorderDisplay::FirstPage => "firstPage",
            PageBorderDisplay::NotFirstPage => "notFirstPage",
        }
    }
}

/// Page borders of a section (ECMA-376 Part 1, §17.6.10 pgBorders).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageBorders {
    /// Top edge (§17.6.21 top).
    pub top: Option<PageBorder>,
    /// Left edge (§17.6.7 left).
    pub left: Option<PageBorder>,
    /// Bottom edge (§17.6.2 bottom).
    pub bottom: Option<PageBorder>,
    /// Right edge (§17.6.15 right).
    pub right: Option<PageBorder>,
    /// `w:offsetFrom="page"` (§17.18.64 `ST_PageBorderOffset`): `w:space` is
    /// measured from the page edge; otherwise (`text`, default) from the text.
    #[serde(default)]
    pub offset_from_page: bool,
    /// `w:display`: which pages show the borders.
    #[serde(default)]
    pub display: PageBorderDisplay,
    /// `w:zOrder="back"` (§17.18.65 `ST_PageBorderZOrder`): drawn behind the text.
    #[serde(default)]
    pub behind: bool,
}

impl PageBorders {
    /// The four edges in the order top, left, bottom, right.
    pub fn edges(&self) -> [Option<&PageBorder>; 4] {
        [
            self.top.as_ref(),
            self.left.as_ref(),
            self.bottom.as_ref(),
            self.right.as_ref(),
        ]
    }
    /// No edge draws anything.
    pub fn is_empty(&self) -> bool {
        self.edges()
            .iter()
            .all(|e| e.is_none_or(|b| !b.line.is_visible()))
    }
}

/// Box borders (paragraph `pBdr` §17.3.1.24, table `tblBorders` §17.4.39, cell `tcBorders` §17.4.67).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Borders {
    /// Top edge.
    pub top: Option<Border>,
    /// Left edge.
    pub left: Option<Border>,
    /// Bottom edge.
    pub bottom: Option<Border>,
    /// Right edge.
    pub right: Option<Border>,
    /// Horizontal lines between rows / paragraphs (`insideH`).
    pub inside_h: Option<Border>,
    /// Vertical lines between cells (`insideV`).
    pub inside_v: Option<Border>,
}

impl Borders {
    /// All four outer edges plus inside lines.
    pub fn all(b: &Border) -> Borders {
        Borders {
            top: Some(b.clone()),
            left: Some(b.clone()),
            bottom: Some(b.clone()),
            right: Some(b.clone()),
            inside_h: Some(b.clone()),
            inside_v: Some(b.clone()),
        }
    }
    /// True when no edge is set.
    pub fn is_empty(&self) -> bool {
        *self == Borders::default()
    }
}

/// Numbering reference of a paragraph (§17.3.1.19 numPr).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NumRef {
    /// Numbering instance id (`w:numId`).
    pub num_id: i32,
    /// List level, 0-based (`w:ilvl`).
    pub ilvl: u8,
}

/// Paragraph properties. `None` means "inherit".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ParaProps {
    /// `w:pStyle` (§17.3.1.27).
    pub style: Option<String>,
    /// Paragraph justification (`w:jc`).
    pub align: Option<Alignment>,
    /// Left (start) indent (`w:ind/@w:left`), in twips.
    pub indent_left: Option<Twips>,
    /// Right (end) indent (`w:ind/@w:right`), in twips.
    pub indent_right: Option<Twips>,
    /// Positive = first-line indent, negative = hanging indent.
    pub indent_first_line: Option<Twips>,
    /// Space before the paragraph (`w:spacing/@w:before`), in twips.
    pub space_before: Option<Twips>,
    /// Space after the paragraph (`w:spacing/@w:after`), in twips.
    pub space_after: Option<Twips>,
    /// Line spacing (`w:spacing/@w:line` with `@w:lineRule`).
    pub line_spacing: Option<LineSpacing>,
    /// Start the paragraph on a new page (`w:pageBreakBefore`).
    pub page_break_before: Option<bool>,
    /// Keep with the next paragraph (`w:keepNext`).
    pub keep_next: Option<bool>,
    /// Keep all lines on one page (`w:keepLines`).
    pub keep_lines: Option<bool>,
    /// Widow/orphan control (`w:widowControl`).
    pub widow_control: Option<bool>,
    /// Ignore spacing between paragraphs of the same style (`w:contextualSpacing`).
    pub contextual_spacing: Option<bool>,
    /// List numbering (`w:numPr`).
    pub num: Option<NumRef>,
    /// Custom tab stops (`w:tabs`); merged with inherited stops.
    pub tabs: Option<Vec<TabStop>>,
    /// Background fill colour (`w:shd/@w:fill`).
    pub shading: Option<Color>,
    /// Paragraph borders (`w:pBdr`).
    pub borders: Option<Borders>,
    /// Outline level, 0-based (`w:outlineLvl`).
    pub outline_level: Option<u8>,
    /// `w:bidi` (§17.3.1.6): right-to-left paragraph (base direction for bidi reordering).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidi: Option<bool>,
    /// Drop cap text frame (§17.3.1.11 framePr with `w:dropCap`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameProps>,
    /// §17.3.1.35 suppressLineNumbers: the paragraph's lines are not numbered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_line_numbers: Option<bool>,
    /// §17.3.1.34 suppressAutoHyphens: no automatic hyphenation in the paragraph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_auto_hyphens: Option<bool>,
    /// Tracked insertion / deletion of the paragraph mark (§17.13.5.16 / §17.13.5.15,
    /// children of `w:pPr/w:rPr`). Never inherited.
    pub mark_ins: Option<RevMark>,
    /// Tracked deletion of the paragraph mark.
    pub mark_del: Option<RevMark>,
    /// Language of the paragraph mark (§17.3.2.20 lang in `w:pPr/w:rPr`, §17.3.1.29): text
    /// typed at the end of the paragraph takes it. Never inherited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_lang: Option<Lang>,
    /// `w:noProof` of the paragraph mark (§17.3.2.21 in `w:pPr/w:rPr`). Never inherited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_no_proof: Option<bool>,
    /// Tracked paragraph formatting change (§17.13.5.29 pPrChange). Never inherited.
    pub change: Option<Box<ParaPropsChange>>,
    /// Unknown `w:pPr` children kept verbatim.
    pub unknown: Vec<Raw>,
}

impl ParaProps {
    /// Overlay `self` on top of `base`.
    #[must_use]
    pub fn inherit(&self, base: &ParaProps) -> ParaProps {
        ParaProps {
            style: self.style.clone().or_else(|| base.style.clone()),
            align: self.align.or(base.align),
            indent_left: self.indent_left.or(base.indent_left),
            indent_right: self.indent_right.or(base.indent_right),
            indent_first_line: self.indent_first_line.or(base.indent_first_line),
            space_before: self.space_before.or(base.space_before),
            space_after: self.space_after.or(base.space_after),
            line_spacing: self.line_spacing.or(base.line_spacing),
            page_break_before: self.page_break_before.or(base.page_break_before),
            keep_next: self.keep_next.or(base.keep_next),
            keep_lines: self.keep_lines.or(base.keep_lines),
            widow_control: self.widow_control.or(base.widow_control),
            contextual_spacing: self.contextual_spacing.or(base.contextual_spacing),
            num: self.num.or(base.num),
            tabs: merge_tabs(base.tabs.as_ref(), self.tabs.as_ref()),
            shading: self.shading.or(base.shading),
            borders: self.borders.clone().or_else(|| base.borders.clone()),
            outline_level: self.outline_level.or(base.outline_level),
            bidi: self.bidi.or(base.bidi),
            frame: self.frame.or(base.frame),
            suppress_line_numbers: self.suppress_line_numbers.or(base.suppress_line_numbers),
            suppress_auto_hyphens: self.suppress_auto_hyphens.or(base.suppress_auto_hyphens),
            mark_ins: self.mark_ins.clone(),
            mark_del: self.mark_del.clone(),
            mark_lang: self.mark_lang.clone(),
            mark_no_proof: self.mark_no_proof,
            change: self.change.clone(),
            unknown: if self.unknown.is_empty() {
                base.unknown.clone()
            } else {
                self.unknown.clone()
            },
        }
    }
}

/// Tabs merge: derived stops add to base stops; `Clear` removes a base stop (§17.3.1.37).
fn merge_tabs(base: Option<&Vec<TabStop>>, own: Option<&Vec<TabStop>>) -> Option<Vec<TabStop>> {
    match (base, own) {
        (None, None) => None,
        (Some(b), None) => Some(b.clone()),
        (b, Some(o)) => {
            let mut v: Vec<TabStop> = b.cloned().unwrap_or_default();
            for t in o {
                v.retain(|x| x.pos != t.pos);
                if t.align != TabAlign::Clear {
                    v.push(*t);
                }
            }
            v.sort_by_key(|t| t.pos);
            Some(v)
        }
    }
}

/// Underline style (ECMA-376 Part 1, §17.18.99 `ST_Underline`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Underline {
    /// No underline (`none`).
    None,
    /// Single line (`single`).
    Single,
    /// Double line (`double`).
    Double,
    /// Thick line (`thick`).
    Thick,
    /// Dotted line (`dotted`).
    Dotted,
    /// Dashed line (`dash`).
    Dash,
    /// Wavy line (`wave`).
    Wave,
    /// Underline words only, not spaces (`words`).
    Words,
    /// Heavy dotted line (`dottedHeavy`).
    DottedHeavy,
    /// Heavy dashed line (`dashedHeavy`).
    DashedHeavy,
    /// Long dashes (`dashLong`).
    DashLong,
    /// Heavy long dashes (`dashLongHeavy`).
    DashLongHeavy,
    /// Dot-dash line (`dotDash`).
    DotDash,
    /// Heavy dot-dash line (`dashDotHeavy`).
    DashDotHeavy,
    /// Dot-dot-dash line (`dotDotDash`).
    DotDotDash,
    /// Heavy dot-dot-dash line (`dashDotDotHeavy`).
    DashDotDotHeavy,
    /// Heavy wavy line (`wavyHeavy`).
    WavyHeavy,
    /// Double wavy line (`wavyDouble`).
    WavyDouble,
}

impl Underline {
    /// Every value with its `ST_Underline` name (§17.18.99).
    pub const ALL: [(Underline, &'static str); 18] = [
        (Underline::None, "none"),
        (Underline::Single, "single"),
        (Underline::Double, "double"),
        (Underline::Thick, "thick"),
        (Underline::Dotted, "dotted"),
        (Underline::Dash, "dash"),
        (Underline::Wave, "wave"),
        (Underline::Words, "words"),
        (Underline::DottedHeavy, "dottedHeavy"),
        (Underline::DashedHeavy, "dashedHeavy"),
        (Underline::DashLong, "dashLong"),
        (Underline::DashLongHeavy, "dashLongHeavy"),
        (Underline::DotDash, "dotDash"),
        (Underline::DashDotHeavy, "dashDotHeavy"),
        (Underline::DotDotDash, "dotDotDash"),
        (Underline::DashDotDotHeavy, "dashDotDotHeavy"),
        (Underline::WavyHeavy, "wavyHeavy"),
        (Underline::WavyDouble, "wavyDouble"),
    ];

    /// Parse an `ST_Underline` value; unknown values read as `single`.
    pub fn parse(s: &str) -> Underline {
        Underline::ALL
            .iter()
            .find(|(_, n)| *n == s)
            .map_or(Underline::Single, |(u, _)| *u)
    }

    /// The `ST_Underline` name.
    pub fn as_str(self) -> &'static str {
        Underline::ALL
            .iter()
            .find(|(u, _)| *u == self)
            .map_or("single", |(_, n)| n)
    }
}

/// Vertical alignment of a run (§17.3.2.42 vertAlign).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VertAlign {
    /// Normal position (`baseline`).
    Baseline,
    /// Raised and smaller (`superscript`).
    Superscript,
    /// Lowered and smaller (`subscript`).
    Subscript,
}

/// Languages of a run (ECMA-376 Part 1, §17.3.2.20 lang): one BCP 47 tag per script class
/// (§17.18.48 `ST_Lang`). Each attribute is inherited on its own through the style hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lang {
    /// `@w:val`: language of Latin (and other non East Asian, non complex script) text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub val: Option<String>,
    /// `@w:eastAsia`: language of East Asian text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub east_asia: Option<String>,
    /// `@w:bidi`: language of complex script (right-to-left) text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidi: Option<String>,
}

impl Lang {
    /// A language for Latin text only.
    pub fn latin(tag: &str) -> Lang {
        Lang {
            val: Some(tag.to_owned()),
            ..Lang::default()
        }
    }

    /// No attribute is set.
    pub fn is_empty(&self) -> bool {
        self.val.is_none() && self.east_asia.is_none() && self.bidi.is_none()
    }

    /// Overlay `self` on top of `base`, attribute by attribute.
    #[must_use]
    pub fn inherit(&self, base: &Lang) -> Lang {
        Lang {
            val: self.val.clone().or_else(|| base.val.clone()),
            east_asia: self.east_asia.clone().or_else(|| base.east_asia.clone()),
            bidi: self.bidi.clone().or_else(|| base.bidi.clone()),
        }
    }
}

fn inherit_lang(own: Option<&Lang>, base: Option<&Lang>) -> Option<Lang> {
    match (own, base) {
        (Some(o), Some(b)) => Some(o.inherit(b)),
        (Some(l), None) | (None, Some(l)) => Some(l.clone()),
        (None, None) => None,
    }
}

/// Run properties. `None` means "inherit".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RunProps {
    /// `w:rStyle` (§17.3.2.29).
    pub style: Option<String>,
    /// `w:rFonts/@w:ascii` (§17.3.2.26): font of U+0000–U+007F. Unless
    /// [`RunProps::font_h_ansi`] (or its theme) is set at the same level, it is also
    /// the hAnsi font (BlueOffice writes both attributes together).
    pub font: Option<String>,
    /// `w:rFonts/@w:eastAsia`.
    pub font_east_asia: Option<String>,
    /// `w:rFonts/@w:cs` (complex script).
    pub font_cs: Option<String>,
    /// `w:rFonts/@w:hAnsi`: characters outside ASCII that are neither East Asian nor
    /// complex script (Latin Extended, so Vietnamese letters with diacritics).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_h_ansi: Option<String>,
    /// `w:rFonts/@w:asciiTheme`, `@w:hAnsiTheme`, `@w:eastAsiaTheme`, `@w:cstheme`
    /// (§17.18.96 `ST_Theme`: `minorHAnsi`, `majorEastAsia`, …), resolved against the
    /// theme's font scheme; a theme attribute wins over the explicit font of its slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_ascii: Option<String>,
    /// Theme font of the hAnsi slot (`@w:hAnsiTheme`), see `theme_ascii`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_h_ansi: Option<String>,
    /// Theme font of the East Asian slot (`@w:eastAsiaTheme`), see `theme_ascii`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_east_asia: Option<String>,
    /// Theme font of the complex-script slot (`@w:cstheme`), see `theme_ascii`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_cs: Option<String>,
    /// `w:rFonts/@w:hint` (§17.18.42 `ST_Hint`: `default`, `eastAsia`, `cs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_hint: Option<String>,
    /// `w:rtl` (§17.3.2.30): right-to-left run, shown with the complex script font.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
    /// `w:cs` (§17.3.2.7): complex script run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complex: Option<bool>,
    /// Font size (`w:sz`), in half-points.
    pub size: Option<HalfPoints>,
    /// Bold (`w:b`).
    pub bold: Option<bool>,
    /// Italic (`w:i`).
    pub italic: Option<bool>,
    /// Underline style (`w:u`).
    pub underline: Option<Underline>,
    /// Single strikethrough (`w:strike`).
    pub strike: Option<bool>,
    /// Double strikethrough (`w:dstrike`).
    pub double_strike: Option<bool>,
    /// Text colour (`w:color`).
    pub color: Option<Color>,
    /// Highlight colour (`w:highlight`), stored as RGB.
    pub highlight: Option<Color>,
    /// Background shading fill (`w:shd/@w:fill`).
    pub shading: Option<Color>,
    /// Superscript / subscript (`w:vertAlign`).
    pub vert_align: Option<VertAlign>,
    /// All capitals (`w:caps`).
    pub caps: Option<bool>,
    /// Small capitals (`w:smallCaps`).
    pub small_caps: Option<bool>,
    /// Character spacing (§17.3.2.35 spacing), twips.
    pub spacing: Option<Twips>,
    /// Kerning threshold (§17.3.2.19 kern).
    pub kern: Option<HalfPoints>,
    /// Hidden text (`w:vanish`).
    pub hidden: Option<bool>,
    /// Hollow outline glyphs (§17.3.2.23 outline).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<bool>,
    /// Shadowed glyphs (§17.3.2.31 shadow).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<bool>,
    /// Raised (embossed) glyphs (§17.3.2.13 emboss).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emboss: Option<bool>,
    /// Engraved (imprinted) glyphs (§17.3.2.18 imprint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imprint: Option<bool>,
    /// Raised (> 0) or lowered (< 0) baseline, half-points (§17.3.2.24 position).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,
    /// Horizontal scale of the glyphs in percent (§17.3.2.43 w, 1–600).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    /// Border around the run (§17.3.2.4 bdr).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    /// Shading pattern and pattern colour (§17.3.2.32 shd `w:val`, `w:color`); the
    /// fill is [`RunProps::shading`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shading_pattern: Option<crate::run_fx::ShadingPattern>,
    /// East Asian layout: two lines in one, horizontal in vertical (§17.3.2.10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub east_asian: Option<crate::run_fx::EastAsianLayout>,
    /// [MS-DOCX] w14 text effects and OpenType features.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<Box<crate::run_fx::TextEffects>>,
    /// Languages of the run (§17.3.2.20 lang), used to pick the proofing dictionary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<Lang>,
    /// Do not check spelling or grammar (§17.3.2.21 noProof).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_proof: Option<bool>,
    /// Hyperlink target (URL or `#bookmark`) when the run is inside `w:hyperlink`.
    pub link: Option<String>,
    /// The run is inside `w:ins` / `w:moveTo` (§17.13.5.18, §17.13.5.25). Never inherited.
    pub ins: Option<RevMark>,
    /// The run is inside `w:del` / `w:moveFrom` (§17.13.5.14, §17.13.5.22). Never inherited.
    pub del: Option<RevMark>,
    /// Tracked formatting change (§17.13.5.31 rPrChange). Never inherited.
    pub change: Option<Box<RunPropsChange>>,
    /// Unknown `w:rPr` children kept verbatim.
    pub unknown: Vec<Raw>,
}

impl RunProps {
    /// True when the run carries any tracked change.
    pub fn has_revision(&self) -> bool {
        self.ins.is_some() || self.del.is_some() || self.change.is_some()
    }

    /// Copy without revision marks.
    #[must_use]
    pub fn without_revisions(&self) -> RunProps {
        RunProps {
            ins: None,
            del: None,
            change: None,
            ..self.clone()
        }
    }

    /// Overlay `self` on top of `base`.
    #[must_use]
    pub fn inherit(&self, base: &RunProps) -> RunProps {
        fn slot(
            own: (&Option<String>, &Option<String>),
            base: (&Option<String>, &Option<String>),
        ) -> (Option<String>, Option<String>) {
            if own.0.is_some() || own.1.is_some() {
                (own.0.clone(), own.1.clone())
            } else {
                (base.0.clone(), base.1.clone())
            }
        }
        // §17.3.2.26: each font slot (explicit name or theme) is inherited on its own.
        let ascii = slot(
            (&self.font, &self.theme_ascii),
            (&base.font, &base.theme_ascii),
        );
        // An ascii font given without an hAnsi font stands for both (see `font`).
        let h_ansi = if self.font_h_ansi.is_none()
            && self.theme_h_ansi.is_none()
            && (self.font.is_some() || self.theme_ascii.is_some())
        {
            (None, None)
        } else {
            slot(
                (&self.font_h_ansi, &self.theme_h_ansi),
                (&base.font_h_ansi, &base.theme_h_ansi),
            )
        };
        let east = slot(
            (&self.font_east_asia, &self.theme_east_asia),
            (&base.font_east_asia, &base.theme_east_asia),
        );
        let cs = slot(
            (&self.font_cs, &self.theme_cs),
            (&base.font_cs, &base.theme_cs),
        );
        RunProps {
            style: self.style.clone().or_else(|| base.style.clone()),
            font: ascii.0,
            theme_ascii: ascii.1,
            font_h_ansi: h_ansi.0,
            theme_h_ansi: h_ansi.1,
            font_east_asia: east.0,
            theme_east_asia: east.1,
            font_cs: cs.0,
            theme_cs: cs.1,
            font_hint: self.font_hint.clone().or_else(|| base.font_hint.clone()),
            rtl: self.rtl.or(base.rtl),
            complex: self.complex.or(base.complex),
            size: self.size.or(base.size),
            bold: self.bold.or(base.bold),
            italic: self.italic.or(base.italic),
            underline: self.underline.or(base.underline),
            strike: self.strike.or(base.strike),
            double_strike: self.double_strike.or(base.double_strike),
            color: self.color.or(base.color),
            highlight: self.highlight.or(base.highlight),
            shading: self.shading.or(base.shading),
            vert_align: self.vert_align.or(base.vert_align),
            caps: self.caps.or(base.caps),
            small_caps: self.small_caps.or(base.small_caps),
            spacing: self.spacing.or(base.spacing),
            kern: self.kern.or(base.kern),
            hidden: self.hidden.or(base.hidden),
            outline: self.outline.or(base.outline),
            shadow: self.shadow.or(base.shadow),
            emboss: self.emboss.or(base.emboss),
            imprint: self.imprint.or(base.imprint),
            position: self.position.or(base.position),
            scale: self.scale.or(base.scale),
            border: self.border.clone().or_else(|| base.border.clone()),
            shading_pattern: self
                .shading_pattern
                .clone()
                .or_else(|| base.shading_pattern.clone()),
            east_asian: self.east_asian.clone().or_else(|| base.east_asian.clone()),
            effects: match (&self.effects, &base.effects) {
                (Some(o), Some(b)) => Some(Box::new(o.inherit(b))),
                (Some(e), None) | (None, Some(e)) => Some(e.clone()),
                (None, None) => None,
            },
            lang: inherit_lang(self.lang.as_ref(), base.lang.as_ref()),
            no_proof: self.no_proof.or(base.no_proof),
            link: self.link.clone().or_else(|| base.link.clone()),
            ins: self.ins.clone(),
            del: self.del.clone(),
            change: self.change.clone(),
            unknown: if self.unknown.is_empty() {
                base.unknown.clone()
            } else {
                self.unknown.clone()
            },
        }
    }
}

/// A run of uniformly formatted content (ECMA-376 Part 1, §17.3.2.25 r).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Run {
    /// Run properties (`w:rPr`).
    pub props: RunProps,
    /// Content items of the run in order.
    pub content: Vec<RunContent>,
}

impl Run {
    /// Convenience constructor for a plain text run.
    pub fn text(text: impl Into<String>, props: RunProps) -> Run {
        Run {
            props,
            content: vec![RunContent::Text(text.into())],
        }
    }

    /// Text of the run (see [`RunContent::push_text`]).
    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        for c in &self.content {
            c.push_text(&mut s);
        }
        s
    }
}

/// How an anchored object wraps text (§20.4.2.15–20.4.2.19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Wrap {
    /// In line with text (no wrapping element).
    #[default]
    None,
    /// Square wrap around the bounding box (`wrapSquare`).
    Square,
    /// Tight wrap around the object outline (`wrapTight`).
    Tight,
    /// Text only above and below (`wrapTopAndBottom`).
    TopAndBottom,
}

/// Sides of a square/tight-wrapped object that text flows on (§20.4.3.7 `ST_WrapText`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WrapText {
    /// Text on both sides (`bothSides`).
    #[default]
    BothSides,
    /// Text on the left side only (`left`).
    Left,
    /// Text on the right side only (`right`).
    Right,
    /// Text on the larger side only (`largest`).
    Largest,
}

/// Position of a floating object (§20.4.2.3 anchor).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Anchor {
    /// `positionH/@relativeFrom` (page, margin, column, character).
    pub h_relative: String,
    /// `positionV/@relativeFrom` (page, margin, paragraph, line).
    pub v_relative: String,
    /// Horizontal offset (`positionH/posOffset`), in EMU.
    pub x: Emu,
    /// Vertical offset (`positionV/posOffset`), in EMU.
    pub y: Emu,
    /// Text wrapping mode.
    pub wrap: Wrap,
    /// `behindDoc`.
    pub behind: bool,
    /// `wrapText` of `wrapSquare` (§20.4.2.17) / `wrapTight` (§20.4.2.20).
    #[serde(default)]
    pub wrap_text: WrapText,
    /// Distance kept from text: top, bottom, left, right (`distT` `distB` `distL`
    /// `distR` of `anchor` §20.4.2.3, overridden by those of the wrap element).
    #[serde(default)]
    pub dist: [Emu; 4],
    /// Bounding box `[x0, y0, x1, y1]` of the tight wrap polygon (`wp:wrapPolygon`,
    /// §20.4.2.16) in the 21 600-unit square over the extent that Word uses.
    #[serde(default)]
    pub polygon: Option<[i64; 4]>,
    /// Vertices of the tight wrap polygon in the same 21 600-unit square
    /// (§20.4.2.16: §20.4.2.14 `start` then §20.4.2.9 `lineTo`, in order); empty
    /// when only the bounding box `polygon` is known.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub polygon_points: Vec<(i64, i64)>,
    /// `positionH/align` (§20.4.2.1 align, `ST_AlignH`: left, center, right, inside,
    /// outside); replaces `x` when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_align: Option<String>,
    /// `positionV/align` (§20.4.2.2 align, `ST_AlignV`: top, center, bottom, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_align: Option<String>,
}

/// Outline of a shape (ECMA-376 Part 1, §20.1.2.2.24 ln) resolved to a colour.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShapeLine {
    /// Line width (`a:ln/@w`), in EMU.
    pub width: Emu,
    /// Line colour (`a:solidFill`), resolved.
    pub color: Color,
    /// §20.1.10.48 `ST_PresetLineDashVal` (`dash`, `sysDot`, …); `None` = solid.
    pub dash: Option<String>,
}

/// Vertical anchoring of text in a shape (§20.1.10.59 `ST_TextAnchoringType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextAnchor {
    /// Text at the top (`t`).
    #[default]
    Top,
    /// Text centred vertically (`ctr`).
    Center,
    /// Text at the bottom (`b`).
    Bottom,
}

/// A WordprocessingML shape or text box (`wps:wsp`, [MS-ODRAWXML] §2.3.10 /
/// ECMA-376 Part 1 §20.1.2.2 shape properties) or its VML fallback (`v:shape`,
/// `v:rect`, … with `v:textbox`, ECMA-376 Part 4 §19.1), carried by an [`Image`]
/// whose `media` is empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Shape {
    /// §20.1.9.18 prstGeom `prst` (`rect`, `ellipse`, `roundRect`, …).
    pub preset: String,
    /// Adjust values (`a:avLst/a:gd`, `fmla="val N"`): (name, value).
    #[serde(default)]
    pub adjust: Vec<(String, i64)>,
    /// Solid fill; `None` = no fill (§20.1.8.44 noFill).
    pub fill: Option<Color>,
    /// Outline; `None` = no line.
    pub line: Option<ShapeLine>,
    /// `a:xfrm/@flipH`, `@flipV` (§20.1.7.6).
    #[serde(default)]
    pub flip_h: bool,
    /// Flipped vertically (`a:xfrm/@flipV`).
    #[serde(default)]
    pub flip_v: bool,
    /// Text box content (§17.17.1 txbxContent); `None` = no text box.
    pub text: Option<Vec<Block>>,
    /// Text insets left, top, right, bottom (`wps:bodyPr` lIns/tIns/rIns/bIns,
    /// §21.1.2.1.1; defaults 91 440 / 45 720 EMU).
    pub insets: [Emu; 4],
    /// `wps:bodyPr/@anchor`.
    #[serde(default)]
    pub anchor: TextAnchor,
    /// Read from the VML fallback (no DrawingML shape in the source).
    #[serde(default)]
    pub vml: bool,
    /// WordArt text path, picture fill and opacity (VML watermarks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub art: Option<Box<ShapeArt>>,
}

/// Text drawn along a shape (ECMA-376 Part 4, §19.1.2.23 textpath), stretched to
/// fill the shape box (`fitshape`) as text watermarks are.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPath {
    /// `string`: the text.
    pub text: String,
    /// `font-family` of the text path style.
    pub font: String,
    /// `font-weight:bold`.
    #[serde(default)]
    pub bold: bool,
    /// `font-style:italic`.
    #[serde(default)]
    pub italic: bool,
}

/// Decorative content of a shape: a text path or a picture, with an opacity; the
/// shape may be a watermark (shown behind the text of every page).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeArt {
    /// Text path (Part 4, §19.1.2.23 textpath).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextPath>,
    /// Picture media part (§19.1.2.11 imagedata `r:id`), key into [`Document::media`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// The picture is washed out (imagedata `gain` / `blacklevel`).
    #[serde(default)]
    pub washout: bool,
    /// Fill opacity in 1/1000 (§19.1.2.5 fill `opacity`; 1000 = opaque).
    pub opacity: u16,
    /// The shape is a watermark (Design › Watermark replaces and removes it).
    #[serde(default)]
    pub watermark: bool,
    /// The picture is a signature line (Part 4, Office VML `o:signatureline`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_line: Option<Box<crate::SignatureLine>>,
}

impl Default for ShapeArt {
    fn default() -> Self {
        ShapeArt {
            text: None,
            picture: None,
            washout: false,
            opacity: 1000,
            watermark: false,
            signature_line: None,
        }
    }
}

impl Shape {
    /// Default text insets (§21.1.2.1.1 bodyPr: 0.1" left/right, 0.05" top/bottom).
    pub const INSETS: [Emu; 4] = [Emu(91_440), Emu(45_720), Emu(91_440), Emu(45_720)];
}

/// A drawing object (`w:drawing`, §20.4): a picture, or — when [`Image::shape`] is
/// set — a shape or text box.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Image {
    /// Media part name (key into [`Document::media`]); empty for shapes.
    pub media: String,
    /// Displayed width (`wp:extent/@cx`), in EMU.
    pub width: Emu,
    /// Displayed height (`wp:extent/@cy`), in EMU.
    pub height: Emu,
    /// Floating position; `None` for an inline picture (`wp:inline`).
    pub anchor: Option<Anchor>,
    /// `a:srcRect` l,t,r,b in 1/1000 of a percent.
    pub crop: Option<[i32; 4]>,
    /// Rotation in 60 000ths of a degree.
    pub rotation: i32,
    /// Alternative text (`wp:docPr/@descr`).
    pub description: String,
    /// Shape / text box drawn instead of a picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<Box<Shape>>,
    /// Picture adjustments and crop shape (CORE-PIC-001).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<Box<crate::picture::PictureFormat>>,
    /// Embedded or linked object this picture previews (`w:object`, CORE-EMBED-001).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ole: Option<Box<crate::ole::OleObject>>,
    /// Original `w:drawing` XML.
    #[serde(skip)]
    pub src: Option<Raw>,
}

impl Image {
    /// Image id the renderers draw: the media part, or a derived id carrying the
    /// picture's adjustments and crop shape ([`crate::picture::display_id`]).
    pub fn display_id(&self) -> String {
        match &self.format {
            Some(f) => crate::picture::display_id(&self.media, f, self.crop),
            None => self.media.clone(),
        }
    }
}

/// Run content items (ECMA-376 Part 1, §17.3.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunContent {
    /// §17.3.3.31 t
    Text(String),
    /// §17.3.3.32 tab
    Tab,
    /// §17.3.3.1 br (textWrapping)
    LineBreak,
    /// §17.3.3.1 br type="page"
    PageBreak,
    /// §17.3.3.1 br type="column"
    ColumnBreak,
    /// §17.3.3.11 noBreakHyphen
    NoBreakHyphen,
    /// §17.3.3.29 softHyphen
    SoftHyphen,
    /// Picture (§17.3.3.9 drawing).
    Image(Box<Image>),
    /// Field with its instruction and cached result text (§17.16).
    Field {
        /// Field instruction (`w:instrText` / `w:fldSimple/@w:instr`), e.g. `PAGE`.
        instr: String,
        /// Result text cached in the document, shown until the field is updated.
        cached: String,
        /// Locked (§17.16.18 fldLock): updates leave the result as it is.
        #[serde(default)]
        locked: bool,
    },
    /// §17.11.14 footnoteReference
    FootnoteRef(i32),
    /// §17.11.7 endnoteReference
    EndnoteRef(i32),
    /// Footnote/endnote number mark inside the note itself (§17.11.13 footnoteRef).
    NoteMark,
    /// Unknown run child kept verbatim (e.g. `w:object`, `mc:AlternateContent`, `w:fldChar`).
    Opaque(Raw),
    /// Paragraph-level markup kept verbatim and written *outside* `w:r`
    /// (bookmarks, permission marks, `w:proofErr`).
    Markup(Raw),
    /// Start of a commented range (§17.13.4.4 commentRangeStart), written outside `w:r`.
    CommentStart(i32),
    /// End of a commented range (§17.13.4.3 commentRangeEnd), written outside `w:r`.
    CommentEnd(i32),
    /// Comment reference mark inside a run (§17.13.4.5 commentReference).
    CommentRef(i32),
    /// Start of a content control (§17.5.2.29 sdt); written outside runs. A block-level control
    /// (`block`) starts at the beginning of its first paragraph and wraps whole paragraphs.
    ControlStart(Box<ContentControl>),
    /// End of the content control started before (`true`: a block-level one).
    ControlEnd(bool),
    /// Legacy form field (§17.16.17 ffData): its begin character, instruction (`FORMTEXT`,
    /// `FORMCHECKBOX`, `FORMDROPDOWN`) and separator. The result content follows up to
    /// [`RunContent::FormEnd`]; a check box or drop-down list shows its state instead
    /// (one object character).
    FormField(Box<crate::form::FormField>),
    /// End character of the form field started before (§17.16.18 fldChar `end`).
    FormEnd,
}

/// The kind of a content control (§17.5.2.38 sdtPr children; [MS-DOCX] w14 / w15).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ControlKind {
    /// §17.5.2.44 text (plain text; `multi_line`).
    Text {
        /// Line breaks allowed.
        multi_line: bool,
    },
    /// Rich text (no type element).
    RichText,
    /// §17.5.2.26 picture.
    Picture,
    /// §17.5.2.5 comboBox: items (display text, value); free text allowed.
    ComboBox(Vec<(String, String)>),
    /// §17.5.2.15 dropDownList: items (display text, value).
    DropDown(Vec<(String, String)>),
    /// §17.5.2.7 date: display format (`dd/MM/yyyy`) and the full date (ISO 8601).
    Date {
        /// §17.5.2.18 dateFormat.
        format: String,
        /// `w:fullDate`.
        full_date: Option<String>,
        /// §17.5.2.40 storeMappedDataAs (`text`, `date`, `dateTime`): how a bound date is
        /// stored in its custom XML node.
        #[serde(default)]
        store_as: Option<String>,
    },
    /// [MS-DOCX] w14:checkbox: checked, and the checked / unchecked characters.
    Checkbox {
        /// w14:checked.
        checked: bool,
        /// Shown when checked (default ☒).
        on: char,
        /// Shown when unchecked (default ☐).
        off: char,
    },
    /// [MS-DOCX] w15:repeatingSection.
    RepeatingSection,
    /// §17.5.2.15 group.
    Group,
    /// Another kind (citation, bibliography, equation, doc-part gallery…).
    Other,
}

/// A content control's properties (§17.5.2.38 sdtPr).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentControl {
    /// §17.5.2.18 id.
    pub id: Option<i64>,
    /// §17.5.2.1 alias (the title shown).
    pub alias: Option<String>,
    /// §17.5.2.42 tag.
    pub tag: Option<String>,
    /// What the control holds.
    pub kind: ControlKind,
    /// §17.5.2.23 lock: `sdtLocked`, `contentLocked`, `sdtContentLocked`.
    pub lock: Option<String>,
    /// The content shows the placeholder (§17.5.2.39 showingPlcHdr).
    pub placeholder: bool,
    /// A block-level control (around paragraphs).
    pub block: bool,
    /// `w:sdtPr` as read (written back while the parsed fields are unchanged).
    pub props_xml: Option<String>,
    /// §17.5.2.6 dataBinding: the custom XML node the content mirrors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_binding: Option<crate::custom_xml::DataBinding>,
}

impl RunContent {
    /// Append the text this item contributes to plain text.
    pub fn push_text(&self, s: &mut String) {
        match self {
            RunContent::Text(t) => s.push_str(t),
            RunContent::Tab => s.push('\t'),
            RunContent::LineBreak => s.push('\u{2028}'),
            RunContent::NoBreakHyphen => s.push('\u{2011}'),
            RunContent::Field { cached, .. } => s.push_str(cached),
            // Complex-field structure (§17.16.18 fldChar, §17.16.23 instrText) is not text.
            RunContent::Opaque(r) if r.xml.contains("fldChar") || r.xml.contains("instrText") => {}
            RunContent::Image(_) | RunContent::Opaque(_) => s.push('\u{FFFC}'),
            RunContent::Markup(r) if crate::index::is_math_xml(&r.xml) => s.push('\u{FFFC}'),
            RunContent::PageBreak
            | RunContent::ColumnBreak
            | RunContent::SoftHyphen
            | RunContent::FootnoteRef(_)
            | RunContent::EndnoteRef(_)
            | RunContent::NoteMark
            | RunContent::Markup(_)
            | RunContent::CommentStart(_)
            | RunContent::CommentEnd(_)
            | RunContent::CommentRef(_)
            | RunContent::ControlStart(_)
            | RunContent::ControlEnd(_)
            | RunContent::FormEnd => {}
            RunContent::FormField(f) => {
                if let Some(d) = f.display() {
                    s.push_str(&d);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- tables

/// Preferred width (§17.18.90 `ST_TblWidth`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TableWidth {
    /// Automatic width (`auto`).
    Auto,
    /// Absolute width (`dxa`), in twips.
    Dxa(Twips),
    /// Fiftieths of a percent.
    Pct(u32),
}

/// Table properties (§17.4.60 tblPr) — subset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct TableProps {
    /// Table style id (`w:tblStyle`).
    pub style: Option<String>,
    /// Preferred table width (`w:tblW`).
    pub width: Option<TableWidth>,
    /// Table alignment (`w:jc`).
    pub align: Option<Alignment>,
    /// Indent from the leading margin (`w:tblInd`), in twips.
    pub indent: Option<Twips>,
    /// Table borders (`w:tblBorders`).
    pub borders: Option<Borders>,
    /// `tblLayout type="fixed"`.
    pub fixed_layout: bool,
    /// Default cell margins left/right (`tblCellMar`).
    pub cell_margin: Option<(Twips, Twips)>,
    /// Floating table position (§17.4.58 tblpPr); `None` = the table is inline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub float: Option<TableFloat>,
    /// Unknown `w:tblPr` children kept verbatim.
    pub unknown: Vec<Raw>,
    /// Tracked change of the table properties (§17.13.5.34 tblPrChange).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<Box<TablePropsChange>>,
}

/// Floating table positioning (ECMA-376 Part 1, §17.4.58 tblpPr). Distances are in
/// twips; anchors keep their `ST_HAnchor` / `ST_VAnchor` value (§17.18.35, §17.18.100:
/// `text`, `margin`, `page`) and the alignments their `ST_XAlign` / `ST_YAlign` value
/// (§17.18.106, §17.18.109), which replace the offsets when present.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct TableFloat {
    /// Distance from the text on the left (`w:leftFromText`).
    pub left_from_text: Twips,
    /// Distance from the text on the right (`w:rightFromText`).
    pub right_from_text: Twips,
    /// Distance from the text above (`w:topFromText`).
    pub top_from_text: Twips,
    /// Distance from the text below (`w:bottomFromText`).
    pub bottom_from_text: Twips,
    /// Horizontal anchor (`w:horzAnchor`), `None` = attribute omitted.
    pub horz_anchor: Option<String>,
    /// Vertical anchor (`w:vertAnchor`), `None` = attribute omitted.
    pub vert_anchor: Option<String>,
    /// Horizontal offset from the anchor (`w:tblpX`).
    pub x: Twips,
    /// Horizontal alignment relative to the anchor (`w:tblpXSpec`).
    pub x_align: Option<String>,
    /// Vertical offset from the anchor (`w:tblpY`).
    pub y: Twips,
    /// Vertical alignment relative to the anchor (`w:tblpYSpec`).
    pub y_align: Option<String>,
}

/// Direction of the text flow in a cell (ECMA-376 Part 1, §17.18.93 `ST_TextDirection`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextDirection {
    /// Horizontal lines, top to bottom (`lrTb`, strict `tb`).
    LrTb,
    /// Vertical lines read top to bottom, stacked right to left (`tbRl`, strict `rl`).
    TbRl,
    /// Vertical lines read bottom to top, stacked left to right (`btLr`, strict `lr`).
    BtLr,
    /// Horizontal, East Asian characters rotated (`lrTbV`, strict `tbV`).
    LrTbV,
    /// Vertical top to bottom, East Asian characters upright (`tbRlV`, strict `rlV`).
    TbRlV,
    /// Vertical top to bottom, stacked left to right (`tbLrV`, strict `lrV`).
    TbLrV,
}

impl TextDirection {
    /// Parse a transitional or strict `ST_TextDirection` value.
    #[must_use]
    pub fn parse(s: &str) -> Option<TextDirection> {
        Some(match s {
            "lrTb" | "tb" => TextDirection::LrTb,
            "tbRl" | "rl" => TextDirection::TbRl,
            "btLr" | "lr" => TextDirection::BtLr,
            "lrTbV" | "tbV" => TextDirection::LrTbV,
            "tbRlV" | "rlV" => TextDirection::TbRlV,
            "tbLrV" | "lrV" => TextDirection::TbLrV,
            _ => return None,
        })
    }

    /// The transitional `ST_TextDirection` value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            TextDirection::LrTb => "lrTb",
            TextDirection::TbRl => "tbRl",
            TextDirection::BtLr => "btLr",
            TextDirection::LrTbV => "lrTbV",
            TextDirection::TbRlV => "tbRlV",
            TextDirection::TbLrV => "tbLrV",
        }
    }

    /// Clockwise rotation of the lines in degrees: 90 for top-to-bottom text, -90 for
    /// bottom-to-top text, 0 for horizontal text.
    #[must_use]
    pub fn rotation(self) -> i32 {
        match self {
            TextDirection::TbRl | TextDirection::TbRlV | TextDirection::TbLrV => 90,
            TextDirection::BtLr => -90,
            TextDirection::LrTb | TextDirection::LrTbV => 0,
        }
    }
}

/// Previous table properties of a tracked change (§17.13.5.34 tblPrChange).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct TablePropsChange {
    /// Who made the change and when.
    pub info: RevisionInfo,
    /// Table properties before the change.
    pub old: TableProps,
}

/// Previous cell properties of a tracked change (§17.13.5.36 tcPrChange).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct CellPropsChange {
    /// Who made the change and when.
    pub info: RevisionInfo,
    /// Cell properties before the change.
    pub old: CellProps,
}

/// Previous section properties of a tracked change (§17.13.5.32 sectPrChange; the
/// properties of `CT_SectPrBase`, without header/footer references).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct SectionChange {
    /// Who made the change and when.
    pub info: RevisionInfo,
    /// Page size and margins before the change.
    pub page: PageSetup,
    /// Text columns before the change.
    pub columns: Columns,
    /// `w:titlePg` before the change.
    pub title_page: bool,
    /// Section break kind before the change.
    pub break_type: SectionBreak,
    /// Page number start before the change.
    pub page_number_start: Option<u32>,
}

/// Vertical merge (§17.4.85 vMerge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VMerge {
    /// First cell of a vertically merged range (`restart`).
    Restart,
    /// Cell continues the merged range above (`continue`).
    Continue,
}

/// Cell vertical alignment (§17.4.84 vAlign).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CellVAlign {
    /// Top-aligned (`top`).
    Top,
    /// Centred vertically (`center`).
    Center,
    /// Bottom-aligned (`bottom`).
    Bottom,
}

/// Cell properties (§17.4.70 tcPr) — subset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellProps {
    /// Preferred cell width (`w:tcW`).
    pub width: Option<TableWidth>,
    /// Number of grid columns the cell spans (`w:gridSpan`), default 1.
    pub grid_span: u32,
    /// Vertical merge state (`w:vMerge`).
    pub v_merge: Option<VMerge>,
    /// Background fill colour (`w:shd/@w:fill`).
    pub shading: Option<Color>,
    /// Cell borders (`w:tcBorders`).
    pub borders: Option<Borders>,
    /// Vertical alignment of the content (`w:vAlign`).
    pub v_align: Option<CellVAlign>,
    /// Text flow direction (`w:textDirection`, §17.4.72).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_direction: Option<TextDirection>,
    /// Unknown `w:tcPr` children kept verbatim.
    pub unknown: Vec<Raw>,
    /// Tracked change of the cell properties (§17.13.5.36 tcPrChange).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<Box<CellPropsChange>>,
    /// Tracked cell insertion (§17.13.5.4 cellIns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ins: Option<RevMark>,
    /// Tracked cell deletion (§17.13.5.3 cellDel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub del: Option<RevMark>,
    /// Tracked change of the vertical merge (§17.13.5.6 cellMerge).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<Box<CellMerge>>,
}

impl Default for CellProps {
    fn default() -> Self {
        CellProps {
            width: None,
            grid_span: 1,
            v_merge: None,
            shading: None,
            borders: None,
            v_align: None,
            text_direction: None,
            unknown: Vec::new(),
            change: None,
            ins: None,
            del: None,
            merge: None,
        }
    }
}

impl CellProps {
    /// Copy without tracked changes (what a §17.13.5.36 tcPrChange records).
    #[must_use]
    pub fn bare(&self) -> CellProps {
        CellProps {
            change: None,
            ins: None,
            del: None,
            merge: None,
            ..self.clone()
        }
    }

    /// True when the cell carries a tracked change.
    pub fn has_revision(&self) -> bool {
        self.change.is_some() || self.ins.is_some() || self.del.is_some() || self.merge.is_some()
    }
}

/// Tracked vertical merge change of a cell (ECMA-376 Part 1, §17.13.5.6 cellMerge).
/// `v_merge` is the revised setting and `v_merge_orig` the setting before the
/// revision (§17.18.1 `ST_AnnotationVMerge`: `rest`, `cont`; `None` = not merged).
/// The cell's own `tcPr/vMerge` holds the revised setting.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct CellMerge {
    /// Author, date and id of the revision.
    pub info: RevisionInfo,
    /// Vertical merge after the revision (`@w:vMerge`).
    pub v_merge: Option<VMerge>,
    /// Vertical merge before the revision (`@w:vMergeOrig`).
    pub v_merge_orig: Option<VMerge>,
}

/// Row height rule (§17.18.37 `ST_HeightRule`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeightRule {
    /// Height is a minimum (`atLeast`).
    AtLeast,
    /// Height is exact (`exact`).
    Exact,
}

/// Row properties (§17.4.82 trPr) — subset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RowProps {
    /// `tblHeader` — repeat on each page.
    pub header: bool,
    /// Row may not break across pages (`w:cantSplit`).
    pub cant_split: bool,
    /// Row height (`w:trHeight`), in twips, with its rule.
    pub height: Option<(Twips, HeightRule)>,
    /// Unknown `w:trPr` children kept verbatim.
    pub unknown: Vec<Raw>,
    /// Tracked row insertion (§17.13.5.19 ins in trPr).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ins: Option<RevMark>,
    /// Tracked row deletion (§17.13.5.12 del in trPr).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub del: Option<RevMark>,
    /// Tracked change of the row properties (§17.13.5.37 trPrChange).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<Box<RowPropsChange>>,
}

/// Previous row properties of a tracked change (ECMA-376 Part 1, §17.13.5.37
/// trPrChange, `CT_TrPrBase`): `tblHeader`, `cantSplit` and `trHeight`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RowPropsChange {
    /// Author, date and id of the revision.
    pub info: RevisionInfo,
    /// Previous `tblHeader` (repeat as header row).
    pub header: bool,
    /// Previous `cantSplit` (row may not break across pages).
    pub cant_split: bool,
    /// Previous row height (`trHeight`), in twips, with its rule.
    pub height: Option<(Twips, HeightRule)>,
}

impl RowProps {
    /// The base properties a §17.13.5.37 trPrChange records.
    pub fn snapshot(&self, info: RevisionInfo) -> RowPropsChange {
        RowPropsChange {
            info,
            header: self.header,
            cant_split: self.cant_split,
            height: self.height,
        }
    }

    /// Same base properties (tracked changes and unknown children ignored).
    pub fn same_base(&self, other: &RowProps) -> bool {
        self.header == other.header
            && self.cant_split == other.cant_split
            && self.height == other.height
    }
}

/// A table cell.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Cell {
    /// Cell properties (`w:tcPr`).
    pub props: CellProps,
    /// Block-level content of the cell.
    pub blocks: Vec<Block>,
}

/// A table row.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Row {
    /// Row properties (`w:trPr`).
    pub props: RowProps,
    /// Cells of the row (`w:tc`).
    pub cells: Vec<Cell>,
}

/// A table (§17.4.38 tbl).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Table {
    /// Table properties (`w:tblPr`).
    pub props: TableProps,
    /// `tblGrid` column widths.
    pub grid: Vec<Twips>,
    /// Tracked change of the grid (§17.13.5.33 tblGridChange): the previous columns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_change: Option<Box<GridChange>>,
    /// Rows of the table (`w:tr`).
    pub rows: Vec<Row>,
    /// Original `w:tbl` XML.
    #[serde(skip)]
    pub src: Option<Raw>,
}

/// Previous grid of a tracked change (§17.13.5.33 tblGridChange). `CT_TblGridChange`
/// only carries an id; author and date are those of the revision sharing that id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct GridChange {
    /// Revision id (author and date come from the revision sharing it).
    pub info: RevisionInfo,
    /// Previous `tblGrid` column widths, in twips.
    pub old: Vec<Twips>,
}

/// Which state of a table with tracked structural changes (§17.13.5) to describe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableView {
    /// Every cell present: inserted and deleted cells side by side.
    Markup,
    /// As if every change were accepted.
    Final,
    /// As if every change were rejected.
    Original,
}

/// Grid placement of a cell in a [`TableView`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPlace {
    /// First grid column.
    pub start: usize,
    /// Grid columns covered (0 for a hidden cell).
    pub span: usize,
    /// The cell is not part of this view (deleted in the final view, inserted in the
    /// original view).
    pub hidden: bool,
    /// Vertical merge of the cell in this view (`tcPr/vMerge`).
    pub v_merge: Option<VMerge>,
}

/// Grid and cell placements of a table in one [`TableView`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableGeometry {
    /// Grid column widths of this view, in twips.
    pub grid: Vec<Twips>,
    /// Placement of every cell, row by row (source cell order).
    pub rows: Vec<Vec<CellPlace>>,
}

impl Table {
    /// Fingerprint of the modelled content.
    pub fn content_fingerprint(&self) -> u64 {
        fingerprint(&(&self.props, &self.grid, &self.grid_change, &self.rows))
    }

    /// `(start, span)` of every cell with all cells present (markup). Spans are the
    /// current ones (§17.4.17 gridSpan), except in a row wider than the grid: a
    /// tracked horizontal merge keeps the merged-away cells as deleted cells
    /// (§17.13.5.3 cellDel), so cells whose span grew by a §17.13.5.36 tcPrChange
    /// use their previous span there.
    pub fn markup_spans(&self) -> Vec<Vec<(usize, usize)>> {
        let cols = self.grid.len();
        self.rows
            .iter()
            .map(|r| {
                let total: usize = r
                    .cells
                    .iter()
                    .map(|c| c.props.grid_span.max(1) as usize)
                    .sum();
                let over = cols > 0 && total > cols;
                let mut g = 0;
                r.cells
                    .iter()
                    .map(|c| {
                        let mut n = c.props.grid_span.max(1) as usize;
                        if over {
                            if let Some(ch) = &c.props.change {
                                n = n.min(ch.old.grid_span.max(1) as usize);
                            }
                        }
                        let s = g;
                        g += n;
                        (s, n)
                    })
                    .collect()
            })
            .collect()
    }

    /// Grid columns that disappear when the cells selected by `gone` are removed: every
    /// row covers the column (in markup) with a removed cell, a cell spanning more
    /// columns (it shrinks) or nothing, and at least one row with a removed cell.
    pub fn removed_columns(&self, gone: &dyn Fn(&Cell) -> bool) -> Vec<bool> {
        let spans = self.markup_spans();
        let n = spans
            .iter()
            .map(|r| r.last().map_or(0, |(s, k)| s + k))
            .max()
            .unwrap_or(0)
            .max(self.grid.len());
        let mut removed = vec![false; n];
        for (g, slot) in removed.iter_mut().enumerate() {
            let mut any = false;
            let mut ok = true;
            for (row, sp) in self.rows.iter().zip(&spans) {
                let Some(ci) = sp.iter().position(|&(s, k)| s <= g && g < s + k) else {
                    continue;
                };
                let c = &row.cells[ci];
                if gone(c) {
                    any = true;
                } else if c.props.grid_span <= 1 {
                    ok = false;
                }
            }
            *slot = any && ok;
        }
        // A kept cell keeps at least one of its columns.
        for (row, sp) in self.rows.iter().zip(&spans) {
            for (c, &(s, _)) in row.cells.iter().zip(sp) {
                if gone(c) {
                    continue;
                }
                let k = c.props.grid_span.max(1) as usize;
                let end = (s + k).min(n);
                if s < end && removed[s..end].iter().all(|r| *r) {
                    removed[end - 1] = false;
                }
            }
        }
        removed
    }

    /// Grid and cell placements of the table in `view` (tracked cell insertions and
    /// deletions §17.13.5.4 / §17.13.5.3, merges §17.13.5.6, cell property changes
    /// §17.13.5.36 and grid changes §17.13.5.33). The final view is what accepting
    /// every change produces.
    pub fn geometry(&self, view: TableView) -> TableGeometry {
        let spans = self.markup_spans();
        match view {
            TableView::Markup => TableGeometry {
                grid: self.grid.clone(),
                rows: self
                    .rows
                    .iter()
                    .zip(&spans)
                    .map(|(r, sp)| {
                        r.cells
                            .iter()
                            .zip(sp)
                            .map(|(c, &(start, span))| CellPlace {
                                start,
                                span,
                                hidden: false,
                                v_merge: c.props.v_merge,
                            })
                            .collect()
                    })
                    .collect(),
            },
            TableView::Final => {
                let removed = self.removed_columns(&|c| c.props.del.is_some());
                let grid = self
                    .grid
                    .iter()
                    .enumerate()
                    .filter(|(g, _)| !removed.get(*g).copied().unwrap_or(false))
                    .map(|(_, w)| *w)
                    .collect();
                let rows = self
                    .rows
                    .iter()
                    .zip(&spans)
                    .map(|(r, sp)| {
                        let mut g = 0;
                        r.cells
                            .iter()
                            .zip(sp)
                            .map(|(c, &(s, _))| {
                                if c.props.del.is_some() {
                                    return CellPlace {
                                        start: g,
                                        span: 0,
                                        hidden: true,
                                        v_merge: c.props.v_merge,
                                    };
                                }
                                let k = c.props.grid_span.max(1) as usize;
                                let lost = (s..s + k)
                                    .filter(|x| removed.get(*x).copied().unwrap_or(false))
                                    .count();
                                let span = k.saturating_sub(lost).max(1);
                                let p = CellPlace {
                                    start: g,
                                    span,
                                    hidden: false,
                                    v_merge: c.props.v_merge,
                                };
                                g += span;
                                p
                            })
                            .collect()
                    })
                    .collect();
                TableGeometry { grid, rows }
            }
            TableView::Original => {
                let grid = self
                    .grid_change
                    .as_ref()
                    .map_or_else(|| self.grid.clone(), |g| g.old.clone());
                let rows = self
                    .rows
                    .iter()
                    .map(|r| {
                        let mut g = 0;
                        r.cells
                            .iter()
                            .map(|c| {
                                let old = c.props.change.as_ref().map(|ch| &ch.old);
                                let v_merge = match (&c.props.merge, old) {
                                    (Some(m), _) => m.v_merge_orig,
                                    (None, Some(o)) => o.v_merge,
                                    (None, None) => c.props.v_merge,
                                };
                                if c.props.ins.is_some() {
                                    return CellPlace {
                                        start: g,
                                        span: 0,
                                        hidden: true,
                                        v_merge,
                                    };
                                }
                                let span =
                                    old.map_or(c.props.grid_span, |o| o.grid_span).max(1) as usize;
                                let p = CellPlace {
                                    start: g,
                                    span,
                                    hidden: false,
                                    v_merge,
                                };
                                g += span;
                                p
                            })
                            .collect()
                    })
                    .collect();
                TableGeometry { grid, rows }
            }
        }
    }

    /// A `rows × cols` table with equal columns spanning `width`, single borders, empty paragraphs.
    pub fn simple(rows: usize, cols: usize, width: Twips) -> Table {
        let cols = cols.max(1);
        let w = Twips(width.0 / cols as i32);
        Table {
            props: TableProps {
                width: Some(TableWidth::Dxa(width)),
                borders: Some(Borders::all(&Border::single(4))),
                ..TableProps::default()
            },
            grid: vec![w; cols],
            grid_change: None,
            rows: (0..rows.max(1))
                .map(|_| Row {
                    props: RowProps::default(),
                    cells: (0..cols)
                        .map(|_| Cell {
                            props: CellProps {
                                width: Some(TableWidth::Dxa(w)),
                                ..CellProps::default()
                            },
                            blocks: vec![Block::Paragraph(Paragraph::default())],
                        })
                        .collect(),
                })
                .collect(),
            src: None,
        }
    }
}

// ---------------------------------------------------------------- styles

/// Style type (§17.18.83 `ST_StyleType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StyleKind {
    /// Paragraph style.
    Paragraph,
    /// Character (run) style.
    Character,
    /// Table style.
    Table,
    /// Numbering style.
    Numbering,
}

/// A style definition (§17.7.4.17 style).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Style {
    /// Style id (`w:styleId`), referenced by `pStyle` / `rStyle` / `tblStyle`.
    pub id: String,
    /// Display name (`w:name`).
    pub name: String,
    /// Style type (`w:type`).
    pub kind: StyleKind,
    /// Parent style id (`w:basedOn`).
    pub based_on: Option<String>,
    /// Style for the following paragraph (`w:next`).
    pub next: Option<String>,
    /// Linked paragraph/character style id (`w:link`).
    pub linked: Option<String>,
    /// Default style of its kind (`w:default`).
    pub is_default: bool,
    /// Shown in the style gallery (`qFormat`).
    pub quick: bool,
    /// Paragraph properties (`w:pPr`).
    pub ppr: ParaProps,
    /// Run properties (`w:rPr`).
    pub rpr: RunProps,
    /// Table properties (`w:tblPr`) of a table style.
    pub tblpr: Option<TableProps>,
    /// Original `w:style` XML.
    #[serde(skip)]
    pub src: Option<Raw>,
}

impl Style {
    /// New style of a kind with empty properties.
    pub fn new(id: &str, name: &str, kind: StyleKind) -> Style {
        Style {
            id: id.into(),
            name: name.into(),
            kind,
            based_on: None,
            next: None,
            linked: None,
            is_default: false,
            quick: true,
            ppr: ParaProps::default(),
            rpr: RunProps::default(),
            tblpr: None,
            src: None,
        }
    }
}

/// Style definitions (`styles.xml`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct StyleSheet {
    /// Style definitions in part order.
    pub styles: Vec<Style>,
    /// `w:latentStyles` kept verbatim.
    #[serde(skip)]
    pub latent: Option<Raw>,
}

impl StyleSheet {
    /// Default style of a kind (`w:default="1"`).
    pub fn default_of(&self, kind: StyleKind) -> Option<&Style> {
        self.styles.iter().find(|s| s.kind == kind && s.is_default)
    }

    /// Resolve a style chain (base first) for `id`, guarding against cycles.
    pub fn chain(&self, id: &str) -> Vec<&Style> {
        let mut out: Vec<&Style> = Vec::new();
        let mut cur = Some(id.to_owned());
        while let Some(i) = cur {
            if out.len() > 32 || out.iter().any(|s| s.id == i) {
                break;
            }
            let Some(s) = self.styles.iter().find(|s| s.id == i) else {
                break;
            };
            out.push(s);
            cur = s.based_on.clone();
        }
        out.reverse();
        out
    }

    /// Effective paragraph + run properties contributed by a paragraph style chain.
    pub fn resolve_paragraph(&self, id: &str) -> (ParaProps, RunProps) {
        let mut p = ParaProps::default();
        let mut r = RunProps::default();
        for s in self.chain(id) {
            p = s.ppr.inherit(&p);
            r = s.rpr.inherit(&r);
        }
        (p, r)
    }

    /// Effective run properties of a character style chain.
    pub fn resolve_character(&self, id: &str) -> RunProps {
        let mut r = RunProps::default();
        for s in self.chain(id) {
            r = s.rpr.inherit(&r);
        }
        r
    }
}

// ---------------------------------------------------------------- numbering

/// Number format (§17.18.59 `ST_NumberFormat`) — subset; unknown kept by name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NumFormat {
    /// Arabic numerals (`decimal`).
    Decimal,
    /// Zero-padded arabic numerals (`decimalZero`).
    DecimalZero,
    /// Lower-case letters (`lowerLetter`).
    LowerLetter,
    /// Upper-case letters (`upperLetter`).
    UpperLetter,
    /// Lower-case roman numerals (`lowerRoman`).
    LowerRoman,
    /// Upper-case roman numerals (`upperRoman`).
    UpperRoman,
    /// Bullet character from `lvlText` (`bullet`).
    Bullet,
    /// No number (`none`).
    None,
    /// Any other format, kept by its attribute value.
    Other(String),
}

impl NumFormat {
    /// Parse `w:numFmt/@w:val`.
    pub fn parse(s: &str) -> NumFormat {
        match s {
            "decimal" => NumFormat::Decimal,
            "decimalZero" => NumFormat::DecimalZero,
            "lowerLetter" => NumFormat::LowerLetter,
            "upperLetter" => NumFormat::UpperLetter,
            "lowerRoman" => NumFormat::LowerRoman,
            "upperRoman" => NumFormat::UpperRoman,
            "bullet" => NumFormat::Bullet,
            "none" => NumFormat::None,
            o => NumFormat::Other(o.to_owned()),
        }
    }
    /// Serialize to `w:numFmt/@w:val`.
    pub fn as_str(&self) -> &str {
        match self {
            NumFormat::Decimal => "decimal",
            NumFormat::DecimalZero => "decimalZero",
            NumFormat::LowerLetter => "lowerLetter",
            NumFormat::UpperLetter => "upperLetter",
            NumFormat::LowerRoman => "lowerRoman",
            NumFormat::UpperRoman => "upperRoman",
            NumFormat::Bullet => "bullet",
            NumFormat::None => "none",
            NumFormat::Other(s) => s,
        }
    }
}

/// One level of an abstract numbering (§17.9.6 lvl).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Level {
    /// Level index, 0-based (`w:ilvl`).
    pub ilvl: u8,
    /// Start value (`w:start`).
    pub start: i32,
    /// Number format (`w:numFmt`).
    pub format: NumFormat,
    /// `lvlText`, e.g. `%1.` or `•`.
    pub text: String,
    /// Number alignment (`w:lvlJc`).
    pub align: Alignment,
    /// Paragraph properties of the level (`w:pPr`), e.g. indents.
    pub ppr: ParaProps,
    /// Run properties of the number text (`w:rPr`).
    pub rpr: RunProps,
    /// Restart after a higher level (`lvlRestart`); `None` = default behaviour.
    pub restart: Option<u8>,
    /// `suff`: tab (default), space or nothing.
    pub suffix: String,
    /// Paragraph style linked to this level (§17.9.23 pStyle): paragraphs of that style
    /// numbered through the style's `numPr` take this level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pstyle: Option<String>,
}

/// Abstract numbering definition (§17.9.1 abstractNum).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AbstractNum {
    /// Abstract numbering id (`w:abstractNumId`).
    pub id: i32,
    /// Level definitions.
    pub levels: Vec<Level>,
}

/// Numbering instance (§17.9.15 num) with level overrides (§17.9.8 lvlOverride).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Num {
    /// Numbering instance id (`w:numId`), referenced by `numPr`.
    pub id: i32,
    /// Abstract definition used (`w:abstractNumId`).
    pub abstract_id: i32,
    /// (ilvl, startOverride)
    pub start_overrides: Vec<(u8, i32)>,
}

/// `numbering.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Numbering {
    /// Abstract numbering definitions.
    pub abstracts: Vec<AbstractNum>,
    /// Numbering instances.
    pub nums: Vec<Num>,
}

impl Numbering {
    /// Level definition for a numbering reference.
    pub fn level(&self, r: NumRef) -> Option<&Level> {
        let num = self.nums.iter().find(|n| n.id == r.num_id)?;
        let abs = self.abstracts.iter().find(|a| a.id == num.abstract_id)?;
        abs.levels.iter().find(|l| l.ilvl == r.ilvl)
    }

    /// Start value of a level considering overrides.
    pub fn start(&self, r: NumRef) -> i32 {
        let ov = self
            .nums
            .iter()
            .find(|n| n.id == r.num_id)
            .and_then(|n| n.start_overrides.iter().find(|(l, _)| *l == r.ilvl))
            .map(|(_, s)| *s);
        ov.or_else(|| self.level(r).map(|l| l.start)).unwrap_or(1)
    }

    /// The level a style-numbered paragraph takes: a style's `numPr` names the list and
    /// the level whose `pStyle` is that style gives `ilvl` (ECMA-376 Part 1, §17.9.23
    /// pStyle, §17.7.4.17 with §17.3.1.19 numPr); otherwise `r` unchanged.
    pub fn level_for_style(&self, r: NumRef, style: &str) -> NumRef {
        let ilvl = self
            .abstract_of(r.num_id)
            .and_then(|a| self.abstracts.iter().find(|x| x.id == a))
            .and_then(|a| a.levels.iter().find(|l| l.pstyle.as_deref() == Some(style)))
            .map_or(r.ilvl, |l| l.ilvl);
        NumRef { ilvl, ..r }
    }

    /// Next free abstract numbering id (`w:abstractNumId`).
    pub fn next_abstract_id(&self) -> i32 {
        self.abstracts.iter().map(|a| a.id).max().unwrap_or(-1) + 1
    }

    /// Next free numbering instance id (`w:numId`; 0 means "no numbering").
    pub fn next_num_id(&self) -> i32 {
        self.nums.iter().map(|n| n.id).max().unwrap_or(0).max(0) + 1
    }

    /// Abstract id of a numbering instance.
    pub fn abstract_of(&self, num_id: i32) -> Option<i32> {
        self.nums
            .iter()
            .find(|n| n.id == num_id)
            .map(|n| n.abstract_id)
    }

    /// Add a simple 9-level list (bullet or decimal) and return its `numId`.
    pub fn add_simple_list(&mut self, bullet: bool) -> i32 {
        let abs_id = self.abstracts.iter().map(|a| a.id).max().unwrap_or(-1) + 1;
        let num_id = self.nums.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let bullets = ["•", "◦", "▪"];
        let levels = (0u8..9)
            .map(|i| Level {
                ilvl: i,
                start: 1,
                format: if bullet {
                    NumFormat::Bullet
                } else {
                    [
                        NumFormat::Decimal,
                        NumFormat::LowerLetter,
                        NumFormat::LowerRoman,
                    ][usize::from(i % 3)]
                    .clone()
                },
                text: if bullet {
                    bullets[usize::from(i % 3)].to_owned()
                } else {
                    format!("%{}.", i + 1)
                },
                align: Alignment::Left,
                ppr: ParaProps {
                    indent_left: Some(Twips(720 * (i32::from(i) + 1))),
                    indent_first_line: Some(Twips(-360)),
                    ..ParaProps::default()
                },
                rpr: RunProps::default(),
                restart: None,
                suffix: "tab".into(),
                pstyle: None,
            })
            .collect();
        self.abstracts.push(AbstractNum { id: abs_id, levels });
        self.nums.push(Num {
            id: num_id,
            abstract_id: abs_id,
            start_overrides: Vec::new(),
        });
        num_id
    }
}

/// Format a list number (ECMA-376 Part 1, §17.18.59).
pub fn format_number(n: i32, f: &NumFormat) -> String {
    match f {
        NumFormat::Decimal | NumFormat::Other(_) => n.to_string(),
        NumFormat::DecimalZero => format!("{n:02}"),
        NumFormat::LowerLetter => letters(n).to_lowercase(),
        NumFormat::UpperLetter => letters(n),
        NumFormat::LowerRoman => roman(n).to_lowercase(),
        NumFormat::UpperRoman => roman(n),
        NumFormat::Bullet | NumFormat::None => String::new(),
    }
}

/// A, B, … Z, AA, BB, … (word-processor letter numbering repeats the letter).
fn letters(n: i32) -> String {
    if n <= 0 {
        return n.to_string();
    }
    let idx = ((n - 1) % 26) as u8;
    let reps = ((n - 1) / 26 + 1) as usize;
    std::iter::repeat_n((b'A' + idx) as char, reps).collect()
}

fn roman(n: i32) -> String {
    if !(1..4000).contains(&n) {
        return n.to_string();
    }
    let table = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut n = n;
    let mut s = String::new();
    for (v, r) in table {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_color() {
        assert_eq!(
            Color::from_hex("FF0080"),
            Some(Color {
                r: 255,
                g: 0,
                b: 128
            })
        );
        assert_eq!(Color::from_hex("auto"), None);
        assert_eq!(Color::from_hex("éé"), None);
        assert_eq!(Color::from_hex("1F4E99").unwrap().to_hex(), "1F4E99");
        assert_eq!(
            Color::from_highlight("yellow").unwrap().highlight_name(),
            "yellow"
        );
    }

    #[test]
    fn inheritance() {
        let base = RunProps {
            size: Some(HalfPoints(24)),
            bold: Some(false),
            ..Default::default()
        };
        let own = RunProps {
            bold: Some(true),
            ..Default::default()
        };
        let r = own.inherit(&base);
        assert_eq!(r.size, Some(HalfPoints(24)));
        assert_eq!(r.bold, Some(true));
    }

    #[test]
    fn plain_text() {
        let p = Paragraph::new(
            ParaProps::default(),
            vec![Run {
                props: RunProps::default(),
                content: vec![
                    RunContent::Text("a".into()),
                    RunContent::Tab,
                    RunContent::Text("b".into()),
                ],
            }],
        );
        assert_eq!(p.plain_text(), "a\tb");
    }

    #[test]
    fn number_formats() {
        // source: ECMA-376 Part 1 §17.18.59 ST_NumberFormat value descriptions.
        assert_eq!(format_number(4, &NumFormat::UpperRoman), "IV");
        assert_eq!(format_number(1994, &NumFormat::LowerRoman), "mcmxciv");
        assert_eq!(format_number(3, &NumFormat::LowerLetter), "c");
        assert_eq!(format_number(28, &NumFormat::UpperLetter), "BB");
        assert_eq!(format_number(7, &NumFormat::DecimalZero), "07");
        assert_eq!(NumFormat::parse("upperRoman").as_str(), "upperRoman");
    }

    #[test]
    fn style_chain_and_cycles() {
        let mk = |id: &str, base: Option<&str>, size: u32| {
            let mut s = Style::new(id, id, StyleKind::Paragraph);
            s.based_on = base.map(Into::into);
            s.rpr.size = Some(HalfPoints(size));
            s
        };
        let ss = StyleSheet {
            styles: vec![
                mk("Normal", None, 24),
                mk("H1", Some("Normal"), 32),
                mk("A", Some("B"), 1),
                mk("B", Some("A"), 2),
            ],
            latent: None,
        };
        assert_eq!(ss.resolve_paragraph("H1").1.size, Some(HalfPoints(32)));
        assert_eq!(ss.chain("H1").len(), 2);
        assert_eq!(ss.chain("A").len(), 2); // cycle stops
    }

    #[test]
    fn tabs_merge() {
        let t = |p: i32, a| TabStop {
            pos: Twips(p),
            align: a,
            leader: TabLeader::None,
        };
        let base = vec![t(720, TabAlign::Left), t(1440, TabAlign::Left)];
        let own = vec![t(720, TabAlign::Clear), t(2000, TabAlign::Right)];
        let m = merge_tabs(Some(&base), Some(&own)).unwrap();
        assert_eq!(m, vec![t(1440, TabAlign::Left), t(2000, TabAlign::Right)]);
    }

    #[test]
    fn numbering_lists() {
        let mut n = Numbering::default();
        let id = n.add_simple_list(false);
        let r = NumRef {
            num_id: id,
            ilvl: 1,
        };
        assert_eq!(n.level(r).unwrap().text, "%2.");
        assert_eq!(n.start(r), 1);
    }

    #[test]
    fn fingerprint_is_stable() {
        assert_eq!(fingerprint("abc"), fingerprint("abc"));
        assert_ne!(fingerprint("abc"), fingerprint("abd"));
    }
}
