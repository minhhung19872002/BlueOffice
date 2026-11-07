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
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };
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
    pub numbering_fingerprint: u64,
    /// Fingerprint of [`Document::comments`] as read (unchanged ⇒ keep the comment parts).
    pub comments_fingerprint: u64,
    /// Root start tag of the source `comments.xml` (its namespace declarations).
    pub comments_root: Option<String>,
    /// The source has a `commentsIds` part ([MS-DOCX] durable ids).
    pub comments_ids: bool,
}

/// A whole word-processing document.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Document {
    pub sections: Vec<Section>,
    /// Document-wide default run properties (`w:docDefaults/w:rPrDefault`).
    pub default_run: RunProps,
    /// Document-wide default paragraph properties (`w:docDefaults/w:pPrDefault`).
    pub default_para: ParaProps,
    pub styles: StyleSheet,
    pub numbering: Numbering,
    /// Footnotes / endnotes by id (ECMA-376 Part 1, §17.11).
    pub footnotes: BTreeMap<i32, Vec<Block>>,
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
    pub source: Option<Arc<SourcePackage>>,
}

/// Attribution of a tracked change (ECMA-376 Part 1, §17.13.5 `CT_TrackChange`:
/// `w:id`, `w:author`, `w:date`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RevisionInfo {
    pub id: i32,
    pub author: String,
    /// `ST_DateTime` (ISO 8601, e.g. `2026-11-08T09:30:00Z`).
    pub date: Option<String>,
}

/// Insertion or deletion of content (§17.13.5.18 ins, §17.13.5.14 del) or one side of
/// a move (§17.13.5.25 moveTo, §17.13.5.22 moveFrom) when `moved`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RevMark {
    pub info: RevisionInfo,
    pub moved: bool,
}

/// Previous run properties of a formatting revision (§17.13.5.31 rPrChange).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RunPropsChange {
    pub info: RevisionInfo,
    pub old: RunProps,
}

/// Previous paragraph properties of a formatting revision (§17.13.5.29 pPrChange).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ParaPropsChange {
    pub info: RevisionInfo,
    pub old: ParaProps,
}

/// A comment (ECMA-376 Part 1, §17.13.4.2 comment) with the threading extension of
/// [MS-DOCX] §2.5.2 (`w15:commentEx`: `paraId`, `paraIdParent`, `done`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Comment {
    pub id: i32,
    pub author: String,
    pub initials: Option<String>,
    pub date: Option<String>,
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
    #[default]
    NextPage,
    Continuous,
    EvenPage,
    OddPage,
    NextColumn,
}

/// Text columns (§17.6.4 cols).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Columns {
    pub count: u16,
    pub space: Twips,
}

impl Default for Columns {
    fn default() -> Self {
        Columns {
            count: 1,
            space: Twips(720),
        }
    }
}

/// Header or footer variants (§17.10.5 headerReference).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct HeaderFooter {
    pub default: Option<Vec<Block>>,
    pub first: Option<Vec<Block>>,
    pub even: Option<Vec<Block>>,
}

/// A section (ECMA-376 Part 1, §17.6 Sections).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Section {
    pub page: PageSetup,
    pub blocks: Vec<Block>,
    pub columns: Columns,
    pub headers: HeaderFooter,
    pub footers: HeaderFooter,
    /// `w:titlePg` — distinct first page header/footer.
    pub title_page: bool,
    pub break_type: SectionBreak,
    /// `w:pgNumType/@w:start`.
    pub page_number_start: Option<u32>,
    /// Original `w:sectPr` XML.
    #[serde(skip)]
    pub src: Option<Raw>,
}

impl Section {
    /// Section with default page setup and the given blocks.
    pub fn new(blocks: Vec<Block>) -> Section {
        Section {
            blocks,
            ..Section::default()
        }
    }

    /// Fingerprint of the modelled section properties (excluding blocks).
    pub fn props_fingerprint(&self) -> u64 {
        fingerprint(&(
            self.page,
            self.columns,
            &self.headers,
            &self.footers,
            self.title_page,
            self.break_type,
            self.page_number_start,
        ))
    }
}

/// Page size and margins (ECMA-376 Part 1, §17.6.13 pgSz, §17.6.11 pgMar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageSetup {
    pub width: Twips,
    pub height: Twips,
    pub margin_top: Twips,
    pub margin_bottom: Twips,
    pub margin_left: Twips,
    pub margin_right: Twips,
    pub header: Twips,
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
    Paragraph(Paragraph),
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
    pub props: ParaProps,
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
    #[default]
    Left,
    Center,
    Right,
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
    Left,
    Center,
    Right,
    Decimal,
    Bar,
    Clear,
}

/// Tab leader (§17.18.85 `ST_TabTlc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TabLeader {
    None,
    Dot,
    Hyphen,
    Underscore,
}

/// Custom tab stop (§17.3.1.37 tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabStop {
    pub pos: Twips,
    pub align: TabAlign,
    pub leader: TabLeader,
}

/// Border line (§17.3.4 / §17.18.2 `ST_Border`). `size` in eighths of a point.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Border {
    pub style: String,
    pub size: u32,
    pub color: Option<Color>,
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

/// Box borders (paragraph `pBdr` §17.3.1.24, table `tblBorders` §17.4.39, cell `tcBorders` §17.4.67).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Borders {
    pub top: Option<Border>,
    pub left: Option<Border>,
    pub bottom: Option<Border>,
    pub right: Option<Border>,
    pub inside_h: Option<Border>,
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
    pub num_id: i32,
    pub ilvl: u8,
}

/// Paragraph properties. `None` means "inherit".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ParaProps {
    /// `w:pStyle` (§17.3.1.27).
    pub style: Option<String>,
    pub align: Option<Alignment>,
    pub indent_left: Option<Twips>,
    pub indent_right: Option<Twips>,
    /// Positive = first-line indent, negative = hanging indent.
    pub indent_first_line: Option<Twips>,
    pub space_before: Option<Twips>,
    pub space_after: Option<Twips>,
    pub line_spacing: Option<LineSpacing>,
    pub page_break_before: Option<bool>,
    pub keep_next: Option<bool>,
    pub keep_lines: Option<bool>,
    pub widow_control: Option<bool>,
    pub contextual_spacing: Option<bool>,
    pub num: Option<NumRef>,
    pub tabs: Option<Vec<TabStop>>,
    pub shading: Option<Color>,
    pub borders: Option<Borders>,
    pub outline_level: Option<u8>,
    /// Tracked insertion / deletion of the paragraph mark (§17.13.5.16 / §17.13.5.15,
    /// children of `w:pPr/w:rPr`). Never inherited.
    pub mark_ins: Option<RevMark>,
    pub mark_del: Option<RevMark>,
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
            mark_ins: self.mark_ins.clone(),
            mark_del: self.mark_del.clone(),
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

/// Underline style (ECMA-376 Part 1, §17.18.99 `ST_Underline`) — subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Underline {
    None,
    Single,
    Double,
    Thick,
    Dotted,
    Dash,
    Wave,
    Words,
}

/// Vertical alignment of a run (§17.3.2.42 vertAlign).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VertAlign {
    Baseline,
    Superscript,
    Subscript,
}

/// Run properties. `None` means "inherit".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RunProps {
    /// `w:rStyle` (§17.3.2.29).
    pub style: Option<String>,
    /// `w:rFonts` ascii/hAnsi.
    pub font: Option<String>,
    pub font_east_asia: Option<String>,
    pub font_cs: Option<String>,
    pub size: Option<HalfPoints>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<Underline>,
    pub strike: Option<bool>,
    pub double_strike: Option<bool>,
    pub color: Option<Color>,
    pub highlight: Option<Color>,
    pub shading: Option<Color>,
    pub vert_align: Option<VertAlign>,
    pub caps: Option<bool>,
    pub small_caps: Option<bool>,
    /// Character spacing (§17.3.2.35 spacing), twips.
    pub spacing: Option<Twips>,
    /// Kerning threshold (§17.3.2.19 kern).
    pub kern: Option<HalfPoints>,
    pub hidden: Option<bool>,
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
        RunProps {
            style: self.style.clone().or_else(|| base.style.clone()),
            font: self.font.clone().or_else(|| base.font.clone()),
            font_east_asia: self
                .font_east_asia
                .clone()
                .or_else(|| base.font_east_asia.clone()),
            font_cs: self.font_cs.clone().or_else(|| base.font_cs.clone()),
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
    pub props: RunProps,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Wrap {
    None,
    Square,
    Tight,
    TopAndBottom,
}

/// Position of a floating object (§20.4.2.3 anchor).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Anchor {
    /// `positionH/@relativeFrom` (page, margin, column, character).
    pub h_relative: String,
    /// `positionV/@relativeFrom` (page, margin, paragraph, line).
    pub v_relative: String,
    pub x: Emu,
    pub y: Emu,
    pub wrap: Wrap,
    /// `behindDoc`.
    pub behind: bool,
}

/// A picture (DrawingML picture inside `w:drawing`, §20.4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Image {
    /// Media part name (key into [`Document::media`]).
    pub media: String,
    pub width: Emu,
    pub height: Emu,
    pub anchor: Option<Anchor>,
    /// `a:srcRect` l,t,r,b in 1/1000 of a percent.
    pub crop: Option<[i32; 4]>,
    /// Rotation in 60 000ths of a degree.
    pub rotation: i32,
    pub description: String,
    /// Original `w:drawing` XML.
    #[serde(skip)]
    pub src: Option<Raw>,
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
    Field { instr: String, cached: String },
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
            RunContent::PageBreak
            | RunContent::ColumnBreak
            | RunContent::SoftHyphen
            | RunContent::FootnoteRef(_)
            | RunContent::EndnoteRef(_)
            | RunContent::NoteMark
            | RunContent::Markup(_)
            | RunContent::CommentStart(_)
            | RunContent::CommentEnd(_)
            | RunContent::CommentRef(_) => {}
        }
    }
}

// ---------------------------------------------------------------- tables

/// Preferred width (§17.18.90 `ST_TblWidth`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TableWidth {
    Auto,
    Dxa(Twips),
    /// Fiftieths of a percent.
    Pct(u32),
}

/// Table properties (§17.4.60 tblPr) — subset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct TableProps {
    pub style: Option<String>,
    pub width: Option<TableWidth>,
    pub align: Option<Alignment>,
    pub indent: Option<Twips>,
    pub borders: Option<Borders>,
    /// `tblLayout type="fixed"`.
    pub fixed_layout: bool,
    /// Default cell margins left/right (`tblCellMar`).
    pub cell_margin: Option<(Twips, Twips)>,
    pub unknown: Vec<Raw>,
}

/// Vertical merge (§17.4.85 vMerge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VMerge {
    Restart,
    Continue,
}

/// Cell vertical alignment (§17.4.84 vAlign).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CellVAlign {
    Top,
    Center,
    Bottom,
}

/// Cell properties (§17.4.70 tcPr) — subset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellProps {
    pub width: Option<TableWidth>,
    pub grid_span: u32,
    pub v_merge: Option<VMerge>,
    pub shading: Option<Color>,
    pub borders: Option<Borders>,
    pub v_align: Option<CellVAlign>,
    pub unknown: Vec<Raw>,
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
            unknown: Vec::new(),
        }
    }
}

/// Row height rule (§17.18.37 `ST_HeightRule`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeightRule {
    AtLeast,
    Exact,
}

/// Row properties (§17.4.82 trPr) — subset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct RowProps {
    /// `tblHeader` — repeat on each page.
    pub header: bool,
    pub cant_split: bool,
    pub height: Option<(Twips, HeightRule)>,
    pub unknown: Vec<Raw>,
}

/// A table cell.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Cell {
    pub props: CellProps,
    pub blocks: Vec<Block>,
}

/// A table row.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Row {
    pub props: RowProps,
    pub cells: Vec<Cell>,
}

/// A table (§17.4.38 tbl).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Table {
    pub props: TableProps,
    /// `tblGrid` column widths.
    pub grid: Vec<Twips>,
    pub rows: Vec<Row>,
    #[serde(skip)]
    pub src: Option<Raw>,
}

impl Table {
    /// Fingerprint of the modelled content.
    pub fn content_fingerprint(&self) -> u64 {
        fingerprint(&(&self.props, &self.grid, &self.rows))
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
    Paragraph,
    Character,
    Table,
    Numbering,
}

/// A style definition (§17.7.4.17 style).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Style {
    pub id: String,
    pub name: String,
    pub kind: StyleKind,
    pub based_on: Option<String>,
    pub next: Option<String>,
    pub linked: Option<String>,
    pub is_default: bool,
    /// Shown in the style gallery (`qFormat`).
    pub quick: bool,
    pub ppr: ParaProps,
    pub rpr: RunProps,
    pub tblpr: Option<TableProps>,
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
    Decimal,
    DecimalZero,
    LowerLetter,
    UpperLetter,
    LowerRoman,
    UpperRoman,
    Bullet,
    None,
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
    pub ilvl: u8,
    pub start: i32,
    pub format: NumFormat,
    /// `lvlText`, e.g. `%1.` or `•`.
    pub text: String,
    pub align: Alignment,
    pub ppr: ParaProps,
    pub rpr: RunProps,
    /// Restart after a higher level (`lvlRestart`); `None` = default behaviour.
    pub restart: Option<u8>,
    /// `suff`: tab (default), space or nothing.
    pub suffix: String,
}

/// Abstract numbering definition (§17.9.1 abstractNum).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AbstractNum {
    pub id: i32,
    pub levels: Vec<Level>,
}

/// Numbering instance (§17.9.15 num) with level overrides (§17.9.8 lvlOverride).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Num {
    pub id: i32,
    pub abstract_id: i32,
    /// (ilvl, startOverride)
    pub start_overrides: Vec<(u8, i32)>,
}

/// `numbering.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Numbering {
    pub abstracts: Vec<AbstractNum>,
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
