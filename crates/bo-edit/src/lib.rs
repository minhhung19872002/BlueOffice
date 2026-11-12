//! Editing for BlueDoc: positions and selection, transactions of invertible
//! [`Op`]s with unlimited undo/redo, text input and deletion, paragraph
//! split/merge, character/paragraph formatting, styles, lists, insertion of
//! tables, pictures, page and section breaks, find/replace and clipboard
//! fragments (internal JSON, HTML, plain text).
//!
//! Positions are `(body paragraph id, byte offset into its index text)`; the id
//! is the paragraph's index in [`bo_model::index::body_paragraphs`] (document
//! order, table cells included) and matches `bo_layout::LineSource::Body`.
#![forbid(unsafe_code)]

pub mod autocorrect;
pub mod blocks;
pub mod charfx;
pub mod cite;
mod collate;
pub mod comments;
pub mod compare;
pub mod controls;
pub mod custom_xml;
pub mod design;
pub mod fields;
pub mod find;
pub mod forms;
pub mod html;
mod inspect;
pub mod lists;
pub mod merge;
mod moves;
pub mod ole;
mod ops;
pub mod outline;
pub mod page_layout;
pub mod para;
mod paste;
pub mod picture;
pub mod picture_tools;
pub mod proofing;
pub mod refs;
pub mod revisions;
mod row_controls;
pub mod section;
pub mod shapes;
pub mod sigline;
pub mod smartart;
pub mod styles;
pub mod table;
pub mod table_tools;
pub mod translate;

pub use autocorrect::{AutoCorrectOptions, Replacement};
pub use comments::CommentInfo;
pub use fields::RefTarget;
pub use find::{find, FindOptions, Match};
pub use lists::{ListLevelSpec, ListNumbering};
pub use moves::MovePaste;
pub use ops::{Op, Pos, Selection, Transaction};
pub use refs::{CitationMark, IndexMark, NoteConversion};
use revisions::Tracker;
pub use revisions::{RevKind, RevisionItem, StoryRef};
pub use section::{HfVariant, PageSetupSpec, PasteMode, SetupScope};
pub use table::{BorderPreset, TableEdit};

use bo_model::change::BlockChanges;
use bo_model::index::{blocks_at, body_paragraphs, para_at, ParaPath};
use bo_model::{
    Alignment, Block, Color, ContentControl, ControlKind, Document, HalfPoints, Image, LineSpacing,
    NumRef, ParaProps, Paragraph, RunContent, RunProps, Section, SectionBreak, StyleKind, TabAlign,
    TabLeader, TabStop, Table, Twips, Underline, VertAlign,
};
use para::Atom;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

/// Editing errors.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    /// The position's block path does not address a paragraph.
    #[error("position does not address a paragraph")]
    BadPath,
    /// The operation no longer matches the document (stale edit).
    #[error("operation does not match the document (stale edit)")]
    Conflict,
    /// The position (paragraph id or offset) is invalid.
    #[error("invalid position")]
    BadPosition,
    /// The command is not supported for this content.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// The pasted clipboard payload could not be parsed.
    #[error("invalid clipboard data: {0}")]
    Clipboard(String),
    /// The find/replace pattern is not a valid regular expression.
    #[error("invalid regular expression: {0}")]
    Regex(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, EditError>;

/// Character formatting commands (ECMA-376 Part 1, §17.3.2 run properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CharFormat {
    /// Toggle bold (`w:b`).
    Bold,
    /// Toggle italic (`w:i`).
    Italic,
    /// Toggle single underline (`w:u`).
    Underline,
    /// Toggle strikethrough (`w:strike`).
    Strike,
    /// Toggle superscript (`w:vertAlign="superscript"`).
    Superscript,
    /// Toggle subscript (`w:vertAlign="subscript"`).
    Subscript,
    /// Set the font family (`w:rFonts`).
    Font {
        /// Font family name.
        name: String,
    },
    /// Size in half-points (§17.3.2.38 sz).
    Size {
        /// Size in half-points.
        #[serde(alias = "half_points")]
        half_points: u32,
    },
    /// `RRGGBB` or `None` for automatic.
    Color {
        /// Colour as `RRGGBB`; `None` = automatic.
        hex: Option<String>,
    },
    /// `RRGGBB` or `None` to remove (§17.3.2.15 highlight).
    Highlight {
        /// Highlight colour as `RRGGBB`; `None` removes it.
        hex: Option<String>,
    },
    /// Proofing language of the text (§17.3.2.20 lang `@w:val`, a BCP 47 tag such as `vi-VN`);
    /// `None` removes the direct language (the style's applies again).
    Language {
        /// Language tag; `None` = inherit.
        lang: Option<String>,
    },
    /// Do not check spelling or grammar (§17.3.2.21 noProof): `on` sets it, `false` removes it.
    NoProof {
        /// Skip proofing.
        on: bool,
    },
    /// Advanced character formatting from the Font dialog: effects, spacing, position,
    /// scale, kerning, border, shading, two lines in one, w14 text effects and
    /// typography ([`charfx::CharAdvancedPatch`]).
    Advanced {
        /// The changes.
        patch: Box<charfx::CharAdvancedPatch>,
    },
    /// Remove direct character formatting.
    Clear,
}

/// How [`Session::change_case`] changes the letters of the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaseMode {
    /// First letter of each sentence in capitals, the rest lower case.
    Sentence,
    /// All lower case.
    Lower,
    /// All capitals.
    Upper,
    /// First letter of each word in capitals.
    Title,
    /// Swap capitals and lower case.
    Toggle,
    /// UPPERCASE → lowercase → Capitalize Each Word → UPPERCASE, by what the text is now.
    Cycle,
}

/// One character in `mode` (a character that maps to several keeps the first; the text keeps
/// its length in characters so its formatting stays in place).
fn case_char(ch: char, mode: CaseMode, word_start: bool, sentence_start: bool) -> char {
    let up = |c: char| c.to_uppercase().next().unwrap_or(c);
    let low = |c: char| c.to_lowercase().next().unwrap_or(c);
    let upper = match mode {
        CaseMode::Upper => true,
        CaseMode::Lower | CaseMode::Cycle => false,
        CaseMode::Title => word_start,
        CaseMode::Sentence => sentence_start && ch.is_alphanumeric(),
        CaseMode::Toggle => !ch.is_uppercase(),
    };
    if upper {
        up(ch)
    } else {
        low(ch)
    }
}

/// Paragraph formatting commands (ECMA-376 Part 1, §17.3.1 paragraph properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ParaFormat {
    /// Paragraph alignment (`w:jc`).
    Align {
        /// New alignment.
        align: Alignment,
    },
    /// Left indent (`w:ind @w:left`).
    IndentLeft {
        /// Indent in twips.
        twips: i32,
    },
    /// First-line indent (`w:ind @w:firstLine`; negative = hanging).
    IndentFirstLine {
        /// Indent in twips; negative = hanging indent.
        twips: i32,
    },
    /// Increase the left indent by one step.
    IncreaseIndent,
    /// Decrease the left indent by one step.
    DecreaseIndent,
    /// Line spacing in 240ths of a line (240 = single).
    LineSpacing {
        /// Spacing in 240ths of a line.
        value: u32,
    },
    /// Space before the paragraph (`w:spacing @w:before`).
    SpaceBefore {
        /// Space in twips.
        twips: i32,
    },
    /// Space after the paragraph (`w:spacing @w:after`).
    SpaceAfter {
        /// Space in twips.
        twips: i32,
    },
    /// Toggle a bulleted list (`w:numPr`).
    Bullets,
    /// Toggle a numbered list (`w:numPr`).
    Numbering,
    /// Apply a paragraph style (`w:pStyle`).
    Style {
        /// Style id (`w:styleId`).
        id: String,
    },
    /// Remove direct paragraph formatting and the paragraph style.
    Clear,
    /// Remove direct paragraph formatting, keeping the paragraph style (Ctrl+Q).
    Reset,
    /// Right indent (§17.3.1.12 ind @w:right).
    IndentRight {
        /// Indent in twips.
        twips: i32,
    },
    /// Left, right and first-line indents at once (ruler); `first_line` < 0 is a
    /// hanging indent (§17.3.1.12 ind @w:firstLine / @w:hanging).
    Indents {
        /// Left indent in twips.
        left: i32,
        /// Right indent in twips.
        right: i32,
        /// First-line indent in twips; negative = hanging indent.
        first_line: i32,
    },
    /// Line spacing with its rule (§17.3.1.33 spacing @w:line/@w:lineRule): `auto`
    /// in 240ths of a line, `exact` / `atLeast` in twips.
    LineSpacingRule {
        /// Spacing rule.
        rule: LineRule,
        /// 240ths of a line for `auto`, twips otherwise.
        value: u32,
    },
    /// Make the effective tab stops exactly `tabs` (§17.3.1.37 tabs): stops of the
    /// paragraph style that are not wanted are cleared with `w:val="clear"`.
    Tabs {
        /// Wanted tab stops.
        tabs: Vec<TabSpec>,
    },
    /// Paragraph shading fill (§17.3.1.31 shd) or `None` to remove it.
    Shading {
        /// Fill colour as `RRGGBB`; `None` removes the shading.
        hex: Option<String>,
    },
    /// Leave the paragraphs out of line numbering (§17.3.1.35 suppressLineNumbers).
    SuppressLineNumbers {
        /// Suppress (`true`) or number again.
        on: bool,
    },
    /// Do not hyphenate the paragraphs (§17.3.1.34 suppressAutoHyphens).
    SuppressAutoHyphens {
        /// Suppress (`true`) or hyphenate again.
        on: bool,
    },
}

/// Line spacing rule (§17.18.48 `ST_LineSpacingRule`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineRule {
    /// Proportional spacing in 240ths of a line.
    #[default]
    Auto,
    /// Exact line height in twips.
    Exact,
    /// Minimum line height in twips.
    AtLeast,
}

/// A tab stop as seen by hosts (§17.3.1.37 tab; `pos` in twips from the left indent origin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabSpec {
    /// Position in twips from the left indent origin.
    pub pos: i32,
    /// Alignment of text at the stop.
    #[serde(default)]
    pub align: TabKind,
    /// Leader characters filling the tab.
    #[serde(default)]
    pub leader: LeaderKind,
}

/// Tab stop alignment offered by the ruler (§17.18.84 `ST_TabJc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TabKind {
    /// Text starts at the stop.
    #[default]
    Left,
    /// Text is centred on the stop.
    Center,
    /// Text ends at the stop.
    Right,
    /// The decimal separator aligns on the stop.
    Decimal,
    /// A vertical bar is drawn at the stop.
    Bar,
}

/// Tab leader (§17.18.85 `ST_TabTlc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LeaderKind {
    /// No leader.
    #[default]
    None,
    /// Dotted leader.
    Dot,
    /// Hyphen leader.
    Hyphen,
    /// Underscore leader.
    Underscore,
}

impl TabSpec {
    fn to_model(self) -> TabStop {
        TabStop {
            pos: Twips(self.pos),
            align: match self.align {
                TabKind::Left => TabAlign::Left,
                TabKind::Center => TabAlign::Center,
                TabKind::Right => TabAlign::Right,
                TabKind::Decimal => TabAlign::Decimal,
                TabKind::Bar => TabAlign::Bar,
            },
            leader: match self.leader {
                LeaderKind::None => TabLeader::None,
                LeaderKind::Dot => TabLeader::Dot,
                LeaderKind::Hyphen => TabLeader::Hyphen,
                LeaderKind::Underscore => TabLeader::Underscore,
            },
        }
    }

    fn from_model(t: TabStop) -> Option<TabSpec> {
        Some(TabSpec {
            pos: t.pos.0,
            align: match t.align {
                TabAlign::Left => TabKind::Left,
                TabAlign::Center => TabKind::Center,
                TabAlign::Right => TabKind::Right,
                TabAlign::Decimal => TabKind::Decimal,
                TabAlign::Bar => TabKind::Bar,
                TabAlign::Clear => return None,
            },
            leader: match t.leader {
                TabLeader::None => LeaderKind::None,
                TabLeader::Dot => LeaderKind::Dot,
                TabLeader::Hyphen => LeaderKind::Hyphen,
                TabLeader::Underscore => LeaderKind::Underscore,
            },
        })
    }
}

/// Line spacing at the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineSpacingState {
    /// Spacing rule.
    pub rule: LineRule,
    /// 240ths of a line for `auto`, twips otherwise.
    pub value: u32,
}

/// Page setup of the section holding the caret (twips; ECMA-376 Part 1, §17.6.13 pgSz,
/// §17.6.11 pgMar, §17.6.4 cols).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionState {
    /// 0-based index of the section.
    pub index: usize,
    /// Number of sections in the document.
    pub count: usize,
    /// Page width.
    pub width: i32,
    /// Page height.
    pub height: i32,
    /// Top margin.
    pub margin_top: i32,
    /// Bottom margin.
    pub margin_bottom: i32,
    /// Left margin.
    pub margin_left: i32,
    /// Right margin.
    pub margin_right: i32,
    /// Distance from the page top to the header.
    pub header: i32,
    /// Distance from the page bottom to the footer.
    pub footer: i32,
    /// Number of text columns.
    pub columns: u16,
    /// Space between columns.
    pub column_space: i32,
    /// §17.10.6 titlePg: different first page header/footer.
    pub title_page: bool,
    /// §17.15.1.35 evenAndOddHeaders (document setting).
    pub even_and_odd: bool,
    /// §17.11.19 numRestart / §17.11.21 pos of the section's footnotes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footnote_restart: Option<bo_model::NoteRestart>,
    /// Footnote placement of the section, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footnote_pos: Option<bo_model::NotePos>,
}

/// The table holding the caret.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableState {
    /// Number of rows.
    pub rows: usize,
    /// Grid columns (§17.4.48 tblGrid).
    pub cols: usize,
    /// 0-based row of the caret's cell.
    pub row: usize,
    /// Grid column of the caret's cell.
    pub col: usize,
    /// The caret's row repeats as a header row (§17.4.49 tblHeader).
    pub header_row: bool,
    /// Fill of the caret's cell (§17.4.33 shd).
    pub shading: Option<String>,
    /// Table alignment on the page (§17.4.28 jc).
    pub align: Alignment,
    /// Preferred width: `pct` in percent, else `dxa` twips (0 = auto) (§17.4.63 tblW).
    pub width_pct: Option<u32>,
    /// Preferred width in twips when not a percentage (0 = auto).
    pub width: i32,
    /// The selection covers more than one cell (merge is possible).
    pub multi_cell: bool,
    /// The caret's cell is merged (split is possible).
    pub merged: bool,
    /// Text direction of the caret's cell (§17.4.72 textDirection; `None` = horizontal).
    pub direction: Option<bo_model::TextDirection>,
    /// Text wraps around the table (§17.4.58 tblpPr).
    pub wrap: bool,
    /// Fixed column widths (§17.4.53 tblLayout `fixed`).
    pub fixed_layout: bool,
}

/// Formatting at the selection start (drives ribbon toggle states).
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatState {
    /// Bold is on.
    pub bold: bool,
    /// Italic is on.
    pub italic: bool,
    /// Underline is on.
    pub underline: bool,
    /// Strikethrough is on.
    pub strike: bool,
    /// Superscript is on.
    pub superscript: bool,
    /// Subscript is on.
    pub subscript: bool,
    /// Font family, `None` when mixed or unknown.
    pub font: Option<String>,
    /// Font size in half-points, `None` when mixed or unknown.
    pub size_half_points: Option<u32>,
    /// Text colour `RRGGBB`, `None` = automatic.
    pub color: Option<String>,
    /// Highlight colour `RRGGBB`, if any.
    pub highlight: Option<String>,
    /// Proofing language of Latin text (§17.3.2.20 lang `@w:val`), `None` when not set anywhere.
    pub lang: Option<String>,
    /// Spelling and grammar are not checked (§17.3.2.21 noProof).
    pub no_proof: bool,
    /// Paragraph alignment.
    pub align: Alignment,
    /// Paragraph style id, if any.
    pub style: Option<String>,
    /// The paragraph is in a bulleted list.
    pub bullets: bool,
    /// The paragraph is in a numbered list.
    pub numbering: bool,
    /// There is an edit to undo.
    pub can_undo: bool,
    /// There is an edit to redo.
    pub can_redo: bool,
    /// Effective paragraph indents and spacing (twips).
    pub indent_left: i32,
    /// Right indent (twips).
    pub indent_right: i32,
    /// Negative = hanging indent.
    pub first_line: i32,
    /// Space before the paragraph (twips).
    pub space_before: i32,
    /// Space after the paragraph (twips).
    pub space_after: i32,
    /// Effective line spacing.
    pub line_spacing: LineSpacingState,
    /// Effective custom tab stops of the paragraph.
    pub tabs: Vec<TabSpec>,
    /// Paragraph shading.
    pub shading: Option<String>,
    /// Page setup of the caret's section.
    pub section: SectionState,
    /// The table holding the caret, if any.
    pub table: Option<TableState>,
    /// Floating picture at the selection (§20.4.2.3 anchor).
    pub picture: Option<picture::PictureState>,
    /// A SmartArt is at the selection (CORE-SMART-005).
    pub smart_art: bool,
    /// A chart is at the selection (CORE-CHART-007 in BlueDoc).
    pub chart: bool,
    /// A picture the picture tools act on is at the selection (CORE-PIC-001).
    pub picture_tools: bool,
    /// An embedded / linked object (§17.3.3.19 object) is at the selection (CORE-UI-008).
    pub ole: bool,
    /// Advanced character formatting (Font dialog).
    pub advanced: charfx::CharAdvanced,
    /// Target of the hyperlink at the caret (§17.16.22 hyperlink), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

/// Content copied to the clipboard.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// Plain-text rendering.
    pub text: String,
    /// HTML rendering for other applications.
    pub html: String,
    /// Lossless internal format (JSON of model blocks).
    pub internal: String,
}

/// Media parts used by pictures of the body.
fn media_in_body(doc: &Document) -> std::collections::BTreeSet<String> {
    fn walk(blocks: &[Block], out: &mut std::collections::BTreeSet<String>) {
        for b in blocks {
            match b {
                Block::Paragraph(p) => {
                    for r in &p.runs {
                        for c in &r.content {
                            if let RunContent::Image(img) = c {
                                out.insert(img.media.clone());
                            }
                        }
                    }
                }
                Block::Table(t) => {
                    for row in &t.rows {
                        for c in &row.cells {
                            walk(&c.blocks, out);
                        }
                    }
                }
                Block::Opaque(_) => {}
            }
        }
    }
    let mut out = std::collections::BTreeSet::new();
    for s in &doc.sections {
        walk(&s.blocks, &mut out);
    }
    out
}

/// A picture of the body, a header/footer or a note shows media part `name`.
fn doc_uses_media(doc: &Document, name: &str) -> bool {
    media_in_body(doc).contains(name)
        || doc.sections.iter().any(|s| {
            [&s.headers, &s.footers].iter().any(|h| {
                [&h.default, &h.first, &h.even]
                    .iter()
                    .any(|b| b.as_ref().is_some_and(|b| blocks_use_media(b, name)))
            })
        })
        || doc
            .footnotes
            .values()
            .chain(doc.endnotes.values())
            .any(|b| blocks_use_media(b, name))
}

/// `blocks` show a picture of media part `name`.
fn blocks_use_media(blocks: &[Block], name: &str) -> bool {
    let doc = Document {
        sections: vec![bo_model::Section::new(blocks.to_vec())],
        ..Document::default()
    };
    media_in_body(&doc).contains(name)
}

/// Internal clipboard content with the footnotes/endnotes its blocks reference.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ClipBlocks {
    blocks: Vec<Block>,
    #[serde(default)]
    footnotes: std::collections::BTreeMap<i32, Vec<Block>>,
    #[serde(default)]
    endnotes: std::collections::BTreeMap<i32, Vec<Block>>,
}

/// Mime type used for the internal clipboard format.
pub const INTERNAL_MIME: &str = "application/x-blueoffice-blocks+json";

// ------------------------------------------------------------------ helpers

fn path_of(doc: &Document, paths: &[ParaPath], id: usize) -> Result<ParaPath> {
    let p = paths.get(id).ok_or(EditError::BadPosition)?;
    para_at(doc, p).ok_or(EditError::BadPath)?;
    Ok(p.clone())
}

fn para_clone(doc: &Document, path: &[usize]) -> Result<Paragraph> {
    para_at(doc, path).cloned().ok_or(EditError::BadPath)
}

fn split_path(path: &[usize]) -> (Vec<usize>, usize) {
    let (last, prefix) = path.split_last().map_or((0, &[][..]), |(l, p)| (*l, p));
    (prefix.to_vec(), last)
}

fn id_of(doc: &Document, path: &[usize]) -> Option<usize> {
    body_paragraphs(doc).iter().position(|p| p == path)
}

/// Index-text length of a paragraph.
pub fn para_len(p: &Paragraph) -> usize {
    para::len(&para::flatten(p))
}

/// Previous grapheme boundary before `off` in `text` (0 when none).
fn prev_boundary(text: &str, off: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .take_while(|&i| i < off)
        .last()
        .unwrap_or(0)
}

/// Next grapheme boundary after `off`.
fn next_boundary(text: &str, off: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(i, g)| i + g.len())
        .find(|&e| e > off)
        .unwrap_or(text.len())
}

/// Effective run properties: defaults → paragraph style → character style → direct (§17.7.2).
pub fn effective_run(doc: &Document, p: &Paragraph, direct: &RunProps) -> RunProps {
    let mut r = doc.default_run.clone();
    let style = p.props.style.clone().or_else(|| {
        doc.styles
            .default_of(StyleKind::Paragraph)
            .map(|s| s.id.clone())
    });
    if let Some(id) = style {
        r = doc.styles.resolve_paragraph(&id).1.inherit(&r);
    }
    if let Some(id) = &direct.style {
        r = doc.styles.resolve_character(id).inherit(&r);
    }
    direct.inherit(&r)
}

/// Paragraph properties from the document defaults and the paragraph style (without
/// direct formatting, §17.7.2).
pub fn style_para_props(doc: &Document, direct: &ParaProps) -> ParaProps {
    let style = direct.style.clone().or_else(|| {
        doc.styles
            .default_of(StyleKind::Paragraph)
            .map(|s| s.id.clone())
    });
    style
        .map(|id| doc.styles.resolve_paragraph(&id).0)
        .unwrap_or_default()
        .inherit(&doc.default_para)
}

/// Effective paragraph properties: defaults → style → direct.
pub fn effective_para(doc: &Document, direct: &ParaProps) -> ParaProps {
    direct.inherit(&style_para_props(doc, direct))
}

/// Direct tab stops that turn the inherited stops `base` into exactly `wanted`
/// (§17.3.1.37: a `clear` stop removes an inherited one).
fn direct_tabs(base: &[TabStop], wanted: &[TabSpec]) -> Option<Vec<TabStop>> {
    let wanted: Vec<TabStop> = wanted.iter().map(|t| t.to_model()).collect();
    let mut out: Vec<TabStop> = base
        .iter()
        .filter(|b| b.align != TabAlign::Clear && !wanted.contains(b))
        .map(|b| TabStop {
            align: TabAlign::Clear,
            ..*b
        })
        .collect();
    out.extend(wanted.iter().filter(|w| !base.contains(w)).copied());
    out.sort_by_key(|t| t.pos.0);
    (!out.is_empty()).then_some(out)
}

/// Transaction builder: ops are applied as they are pushed.
struct Tx<'a> {
    doc: &'a mut Document,
    ops: Vec<Op>,
    /// Set in track-changes mode: edits are recorded as revisions (§17.13.5).
    track: Option<Tracker>,
}

impl Tx<'_> {
    fn push(&mut self, op: Op) -> Result<()> {
        op.apply(self.doc)?;
        self.ops.push(op);
        Ok(())
    }

    /// Record an op already applied to the document.
    fn record_applied(&mut self, op: Op) {
        self.ops.push(op);
    }

    /// Remove the notes whose last reference this transaction removed and, in
    /// track-changes mode, the media of pictures the author inserted and removed again
    /// (their insertion is undone, so the part goes too).
    fn drop_orphan_notes(&mut self) -> Result<()> {
        if self.track.is_some() {
            let mut gone = std::collections::BTreeSet::new();
            for op in &self.ops {
                if let Op::Blocks { remove, .. } = op {
                    let d = Document {
                        sections: vec![bo_model::Section::new(remove.clone())],
                        ..Document::default()
                    };
                    gone.extend(media_in_body(&d));
                }
            }
            for name in gone {
                if !doc_uses_media(self.doc, &name) {
                    if let Some(data) = self.doc.media.get(&name).cloned() {
                        self.push(Op::Media {
                            name,
                            old: Some(data),
                            new: None,
                        })?;
                    }
                }
            }
        }
        let mut removed = Vec::new();
        for op in &self.ops {
            if let Op::Blocks { remove, .. } = op {
                section::note_refs(remove, &mut removed);
            }
        }
        if removed.is_empty() {
            return Ok(());
        }
        let mut present = Vec::new();
        for s in &self.doc.sections {
            section::note_refs(&s.blocks, &mut present);
        }
        for endnote in [false, true] {
            let map = if endnote {
                &self.doc.endnotes
            } else {
                &self.doc.footnotes
            };
            let gone: Vec<i32> = removed
                .iter()
                .filter(|k| k.0 == endnote && !present.contains(k) && map.contains_key(&k.1))
                .map(|k| k.1)
                .collect();
            if gone.is_empty() {
                continue;
            }
            let old = map.clone();
            let mut new = old.clone();
            for id in gone {
                new.remove(&id);
            }
            self.push(Op::Notes { endnote, old, new })?;
        }
        Ok(())
    }

    fn paths(&self) -> Vec<ParaPath> {
        body_paragraphs(self.doc)
    }

    fn replace_para(&mut self, path: &[usize], new: Paragraph) -> Result<()> {
        let old = para_clone(self.doc, path)?;
        if old == new {
            return Ok(());
        }
        let (prefix, index) = split_path(path);
        self.push(Op::Blocks {
            path: prefix,
            index,
            remove: vec![Block::Paragraph(old)],
            insert: vec![Block::Paragraph(new)],
        })
    }

    fn atoms(&self, id: usize) -> Result<(ParaPath, Paragraph, Vec<Atom>)> {
        let paths = self.paths();
        let path = path_of(self.doc, &paths, id)?;
        let p = para_clone(self.doc, &path)?;
        let a = para::flatten(&p);
        Ok((path, p, a))
    }

    /// Delete `a..b` (document order). Returns where new content goes: `a`, or the
    /// end of the (kept) deleted text in track-changes mode.
    fn delete_range(&mut self, a: Pos, b: Pos) -> Result<Pos> {
        Ok(self.delete_span(a, b)?.1)
    }

    /// Delete `a..b`; returns the positions before and after what remains of the range
    /// (equal unless deletions are tracked).
    fn delete_span(&mut self, a: Pos, b: Pos) -> Result<(Pos, Pos)> {
        if a >= b {
            let p = a.min(b);
            return Ok((p, p));
        }
        if self.track.is_some() {
            return self.tracked_delete(a, b);
        }
        let p = self.untracked_delete(a, b)?;
        Ok((p, p))
    }

    /// Track-changes deletion: text is marked deleted (§17.13.5.14), paragraph marks in
    /// the range too (§17.13.5.15); the author's own insertions are really removed.
    fn tracked_delete(&mut self, a: Pos, b: Pos) -> Result<(Pos, Pos)> {
        let Some(mut tr) = self.track.take() else {
            return Err(EditError::BadPosition);
        };
        let mark = tr.mark();
        let res = (|| {
            let paths = self.paths();
            let mut end = b;
            for id in a.para..=b.para {
                let path = path_of(self.doc, &paths, id)?;
                let p = para_clone(self.doc, &path)?;
                let mut atoms = para::flatten(&p);
                let l = para::len(&atoms);
                let s = if id == a.para { a.offset } else { 0 };
                let e = if id == b.para { b.offset } else { l };
                let removed = revisions::mark_deleted(&mut atoms, s, e, &tr, &mark);
                if id == b.para {
                    end = Pos::new(id, e - removed);
                }
                let mut q = para::with_atoms(&p, atoms);
                if id < b.para && q.props.mark_del.is_none() {
                    q.props.mark_del = Some(mark.clone());
                }
                self.replace_para(&path, q)?;
            }
            Ok((a, end))
        })();
        self.track = Some(tr);
        res
    }

    fn untracked_delete(&mut self, a: Pos, b: Pos) -> Result<Pos> {
        let paths = self.paths();
        let pa = path_of(self.doc, &paths, a.para)?;
        let pb = path_of(self.doc, &paths, b.para)?;
        if a.para == b.para {
            let p = para_clone(self.doc, &pa)?;
            let mut atoms = para::flatten(&p);
            para::delete(&mut atoms, a.offset, b.offset);
            self.replace_para(&pa, para::with_atoms(&p, atoms))?;
            return Ok(a);
        }
        let (ca, ia) = split_path(&pa);
        let (cb, ib) = split_path(&pb);
        if ca == cb && ia < ib {
            let p = para_clone(self.doc, &pa)?;
            let q = para_clone(self.doc, &pb)?;
            let mut left = para::flatten(&p);
            let l = para::len(&left);
            para::delete(&mut left, a.offset, l);
            let mut right = para::flatten(&q);
            para::delete(&mut right, 0, b.offset);
            left.extend(right);
            let merged = para::with_atoms(&p, left);
            let blocks = blocks_at(self.doc, &ca).ok_or(EditError::BadPath)?;
            let remove = blocks[ia..=ib].to_vec();
            self.push(Op::Blocks {
                path: ca,
                index: ia,
                remove,
                insert: vec![Block::Paragraph(merged)],
            })?;
            return Ok(a);
        }
        // Range across containers (e.g. table cells): clear the text, keep the structure.
        for id in a.para..=b.para {
            let path = path_of(self.doc, &paths, id)?;
            let p = para_clone(self.doc, &path)?;
            let mut atoms = para::flatten(&p);
            let l = para::len(&atoms);
            let s = if id == a.para { a.offset } else { 0 };
            let e = if id == b.para { b.offset } else { l };
            para::delete(&mut atoms, s, e);
            self.replace_para(&path, para::with_atoms(&p, atoms))?;
        }
        Ok(a)
    }

    /// New content never inherits revision marks from its neighbours; in
    /// track-changes mode it becomes an insertion (§17.13.5.18).
    fn mark_new(&mut self, existing: &[Atom], off: usize, atoms: Vec<Atom>) -> Vec<Atom> {
        let mark = self
            .track
            .as_mut()
            .map(|t| revisions::insertion_mark(existing, off, t));
        atoms
            .into_iter()
            .map(|(p, c)| {
                let mut p = p.without_revisions();
                if para::atom_len(&c) > 0 {
                    p.ins.clone_from(&mark);
                }
                (p, c)
            })
            .collect()
    }

    /// A new paragraph mark (tracked as inserted in track-changes mode, §17.13.5.16).
    fn new_mark(&mut self, props: &mut ParaProps) {
        props.mark_ins = self.track.as_mut().map(Tracker::mark);
        props.mark_del = None;
    }

    /// Replace the blocks of section `si` (one op for the changed middle part).
    fn set_section_blocks(&mut self, si: usize, new: &[Block]) -> Result<()> {
        let old = self
            .doc
            .sections
            .get(si)
            .map(|s| s.blocks.clone())
            .ok_or(EditError::BadPath)?;
        if old == new {
            return Ok(());
        }
        let pre = old.iter().zip(new).take_while(|(x, y)| x == y).count();
        let suf = old[pre..]
            .iter()
            .rev()
            .zip(new[pre..].iter().rev())
            .take_while(|(x, y)| x == y)
            .count();
        self.push(Op::Blocks {
            path: vec![si],
            index: pre,
            remove: old[pre..old.len() - suf].to_vec(),
            insert: new[pre..new.len() - suf].to_vec(),
        })
    }

    /// Recompute fields until page numbers settle (at most three layouts).
    fn update_fields(
        &mut self,
        pages_of: &mut dyn FnMut(&Document) -> Vec<u32>,
        env: &fields::FieldEnv,
    ) -> Result<usize> {
        self.update_fields_in(pages_of, env, None)
    }

    /// [`Tx::update_fields`] of the fields in `scope` only (`None`: all).
    fn update_fields_in(
        &mut self,
        pages_of: &mut dyn FnMut(&Document) -> Vec<u32>,
        env: &fields::FieldEnv,
        scope: Option<&fields::Scope>,
    ) -> Result<usize> {
        let mut total = 0;
        for round in 0..3 {
            let pages = pages_of(self.doc);
            let up = fields::update_in(self.doc, &pages, env, scope);
            let unchanged = up.styles.is_none()
                && up
                    .sections
                    .iter()
                    .zip(&self.doc.sections)
                    .all(|(b, s)| *b == s.blocks);
            if round == 0 {
                total = up.count;
            }
            if unchanged {
                break;
            }
            if let Some(st) = up.styles {
                self.push(Op::Styles {
                    old: Box::new(self.doc.styles.clone()),
                    new: Box::new(st),
                })?;
            }
            for (si, blocks) in up.sections.into_iter().enumerate() {
                self.set_section_blocks(si, &blocks)?;
            }
        }
        Ok(total)
    }

    /// Insert zero-width marks (comment ranges and references) at `at`.
    fn insert_marks(&mut self, at: Pos, marks: Vec<RunContent>) -> Result<()> {
        let (path, p, mut a) = self.atoms(at.para)?;
        let props = para::props_at(&a, at.offset).without_revisions();
        let new: Vec<Atom> = marks
            .into_iter()
            .map(|m| {
                let pr = match m {
                    RunContent::CommentRef(_) => RunProps {
                        style: Some("CommentReference".into()),
                        ..RunProps::default()
                    },
                    _ => RunProps {
                        link: props.link.clone(),
                        ..RunProps::default()
                    },
                };
                (pr, m)
            })
            .collect();
        para::insert(&mut a, at.offset, new);
        self.replace_para(&path, para::with_atoms(&p, a))
    }

    fn insert_atoms(&mut self, at: Pos, atoms: Vec<Atom>) -> Result<Pos> {
        let (path, p, mut a) = self.atoms(at.para)?;
        let atoms = self.mark_new(&a, at.offset, atoms);
        let n = para::len(&atoms);
        para::insert(&mut a, at.offset, atoms);
        self.replace_para(&path, para::with_atoms(&p, a))?;
        Ok(Pos::new(at.para, at.offset + n))
    }

    /// Split the paragraph at `at`; returns the start of the new (second) paragraph.
    fn split(&mut self, at: Pos) -> Result<Pos> {
        let (path, p, mut left) = self.atoms(at.para)?;
        let total = para::len(&left);
        let i = para::split_at(&mut left, at.offset);
        // A form field's end character stays with its field (§17.16.18).
        let i = para::past_field_end(&left, i);
        let right = left.split_off(i);
        let mut right_props = p.props.clone();
        // §17.7.4.10 next: a new paragraph after a heading gets the style's "next" style.
        if at.offset >= total {
            if let Some(next) = p
                .props
                .style
                .as_ref()
                .and_then(|id| self.doc.style(id))
                .and_then(|s| s.next.clone())
            {
                right_props.style = Some(next);
            }
        }
        right_props.unknown.clear();
        let mut first = para::with_atoms(&p, left);
        // The original mark stays with the second part; the first one gets a new mark.
        self.new_mark(&mut first.props);
        let second = Paragraph {
            props: right_props,
            runs: para::rebuild(right),
            src: None,
        };
        let (prefix, index) = split_path(&path);
        self.push(Op::Blocks {
            path: prefix,
            index,
            remove: vec![Block::Paragraph(p)],
            insert: vec![Block::Paragraph(first), Block::Paragraph(second)],
        })?;
        Ok(Pos::new(at.para + 1, 0))
    }

    /// Replace the match `m` (which may span paragraph marks) by `replacement`, whose
    /// `\n` characters split paragraphs; `text` and `atoms` are the start paragraph's.
    fn replace_span(
        &mut self,
        m: &Match,
        text: &str,
        atoms: &[Atom],
        replacement: &str,
    ) -> Result<Selection> {
        let end_para = m.last_para();
        let (_, q, _) = self.atoms(end_para)?;
        let end_text = bo_model::index::para_index_text(&q);
        if end_para < m.para
            || m.end > end_text.len()
            || !end_text.is_char_boundary(m.end)
            || (end_para == m.para && m.end < m.start)
        {
            return Err(EditError::Conflict);
        }
        let probe = if m.start < text.len() {
            m.start + 1
        } else {
            m.start
        };
        let props = para::props_at(atoms, probe);
        let (_, at) = self.delete_span(Pos::new(m.para, m.start), Pos::new(end_para, m.end))?;
        let mut caret = at;
        for (i, line) in replacement.split('\n').enumerate() {
            if i > 0 {
                caret = self.split(caret)?;
            }
            caret = self.insert_atoms(caret, para::text_atoms(line, &props))?;
        }
        Ok(Selection {
            anchor: at,
            focus: caret,
        })
    }

    fn insert_blocks(
        &mut self,
        container: Vec<usize>,
        index: usize,
        blocks: Vec<Block>,
    ) -> Result<()> {
        self.push(Op::Blocks {
            path: container,
            index,
            remove: Vec::new(),
            insert: blocks,
        })
    }
}

/// Built-in paragraph styles offered by the style gallery (BlueDoc's own definitions;
/// ids follow the conventional `Heading1`… so other applications map them to outline levels).
pub fn builtin_style(id: &str) -> Option<bo_model::Style> {
    use bo_model::Style;
    let mut s = match id {
        "Normal" => {
            let mut s = Style::new("Normal", "Normal", StyleKind::Paragraph);
            s.is_default = true;
            return Some(s);
        }
        "Title" => Style::new("Title", "Title", StyleKind::Paragraph),
        "Heading1" => Style::new("Heading1", "heading 1", StyleKind::Paragraph),
        "Heading2" => Style::new("Heading2", "heading 2", StyleKind::Paragraph),
        "Heading3" => Style::new("Heading3", "heading 3", StyleKind::Paragraph),
        // Deeper headings, e.g. for a multilevel list linked to heading styles (§17.9.23).
        "Heading4" | "Heading5" | "Heading6" | "Heading7" | "Heading8" | "Heading9" => {
            let n = id.trim_start_matches("Heading");
            Style::new(id, &format!("heading {n}"), StyleKind::Paragraph)
        }
        _ => return None,
    };
    s.based_on = Some("Normal".into());
    s.next = Some("Normal".into());
    s.rpr.bold = Some(true);
    s.ppr.keep_next = Some(true);
    s.ppr.space_before = Some(Twips(240));
    s.ppr.space_after = Some(Twips(120));
    let (size, level) = match id {
        "Title" => (40, None),
        "Heading1" => (32, Some(0)),
        "Heading2" => (28, Some(1)),
        "Heading3" => (26, Some(2)),
        // §17.3.1.20 outlineLvl: Heading4 … Heading9 → 3 … 8.
        _ => (
            24,
            id.trim_start_matches("Heading")
                .parse::<u8>()
                .ok()
                .map(|n| n.saturating_sub(1)),
        ),
    };
    s.rpr.size = Some(HalfPoints(size));
    s.ppr.outline_level = level;
    if id == "Title" {
        s.ppr.align = Some(Alignment::Center);
    }
    if id == "Heading3" {
        s.rpr.italic = Some(true);
    }
    Some(s)
}

/// Largest content-control id of the body (§17.5.2.18 id).
fn max_control_id(doc: &Document) -> i64 {
    body_paragraphs(doc)
        .iter()
        .filter_map(|path| para_at(doc, path))
        .flat_map(|p| p.runs.iter().flat_map(|r| &r.content))
        .filter_map(|c| match c {
            RunContent::ControlStart(cc) => cc.id,
            _ => None,
        })
        .max()
        .unwrap_or(0)
        .max(0)
        .max(
            doc.sections
                .iter()
                .map(|s| row_controls::max_row_control_id(&s.blocks))
                .max()
                .unwrap_or(0),
        )
}

// ------------------------------------------------------------------ session

/// An editing session: the document, its selection and the undo history.
#[derive(Debug, Clone)]
pub struct Session {
    doc: Document,
    selection: Selection,
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    /// Formatting applied to the next typed text (collapsed-selection formatting).
    pending: Option<RunProps>,
    typing: bool,
    /// The equation (its start) last edited in place by typing ([`Session::replace_equation`]
    /// with `group`): the next such edit of it joins the same undo step.
    math_group: Option<Pos>,
    version: u64,
    /// Track-changes mode (§17.13.5): new edits are recorded as revisions.
    track: bool,
    /// Author of new revisions and comments.
    author: String,
    /// `ST_DateTime` stamped on new revisions and comments.
    clock: Option<String>,
    /// Blocks changed since the last [`Session::take_changes`].
    changes: BlockChanges,
    /// Formatting picked by the Format Painter.
    painter: Option<section::PickedFormat>,
    /// New revision ids are above this (a header/footer or note edited in its own
    /// session takes ids unique in the whole document, §17.13.5).
    id_floor: i32,
    /// Collaboration: new comment / note ids are ≡ this (mod 65 536), so that two
    /// replicas adding one at the same moment do not pick the same id.
    id_seed: Option<u16>,
    /// What fields read from outside the text (clock, properties, user, variables).
    field_env: fields::FieldEnv,
    /// The last cut made while tracking (cut and paste is recorded as a move).
    last_cut: Option<moves::CutRecord>,
}

/// Smallest id ≥ `candidate` that is ≡ `seed` (mod 65 536); `candidate` itself
/// without a seed (or when that would overflow `i32`).
pub(crate) fn seeded_id(candidate: i32, seed: Option<u16>) -> i32 {
    let Some(seed) = seed else {
        return candidate;
    };
    let c = i64::from(candidate);
    let s = i64::from(seed);
    let id = c + (s - c).rem_euclid(65_536);
    i32::try_from(id).unwrap_or(candidate)
}

/// Author used when the host does not know the user.
pub const DEFAULT_AUTHOR: &str = "BlueOffice";

/// Error of accepting / rejecting revisions or turning tracking off while tracking is
/// locked (§17.15.1.29 documentProtection `w:edit="trackedChanges"`).
pub const TRACKING_LOCKED: &str = "review:locked";

impl Session {
    /// Collaboration (ECMA-376 comment §17.13.4.2 / footnote §17.11.10 ids are
    /// per document): new comment and note ids are taken ≡ `seed` (mod 65 536),
    /// derived from the replica's client id, instead of max + 1, so concurrent
    /// additions on two replicas get different ids. `None` (default): max + 1.
    pub fn set_id_seed(&mut self, seed: Option<u16>) {
        self.id_seed = seed;
    }

    /// First number to try for a new part name (`image<n>.png`, `data<n>.xml`, …):
    /// `n` alone, or — while collaborating ([`Session::set_id_seed`]) — in a band of
    /// the replica's own (seed × 100 000 + `n`), so parts added at once on two
    /// replicas get different names (ECMA-376 Part 2, §9.1.1 part names) instead of
    /// one replacing the other.
    #[must_use]
    pub fn part_number_start(&self, n: usize) -> usize {
        match self.id_seed {
            Some(s) => usize::from(s) * 100_000 + n,
            None => n,
        }
    }

    /// Start editing `doc` with the caret at the beginning.
    pub fn new(doc: Document) -> Session {
        // §17.15.1.89 trackRevisions, or §17.18.37 trackedChanges protection.
        let track = doc.track_revisions
            || doc
                .protection
                .as_ref()
                .is_some_and(bo_model::form::DocProtection::tracked_changes_enforced);
        Session {
            doc,
            selection: Selection::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            pending: None,
            typing: false,
            math_group: None,
            version: 0,
            track,
            author: DEFAULT_AUTHOR.to_owned(),
            clock: None,
            changes: BlockChanges::everything(),
            painter: None,
            id_floor: 0,
            id_seed: None,
            field_env: fields::FieldEnv {
                vi: true,
                ..fields::FieldEnv::default()
            },
            last_cut: None,
        }
    }

    /// New revisions get ids above `floor` (see [`revisions::max_id`]).
    pub fn set_revision_floor(&mut self, floor: i32) {
        self.id_floor = floor;
    }

    /// Convert text typed in legacy Vietnamese fonts (TCVN3 `.Vn*`, `VNI-*`) to
    /// Unicode and switch those runs and styles to a Unicode font
    /// ([`bo_encoding::convert_document_runs`]). One undoable transaction (not
    /// tracked as a revision); returns the number of runs converted, 0 when
    /// there is nothing to convert (no transaction then). The caret moves to the
    /// start: converted text has other byte lengths.
    pub fn convert_legacy_text(&mut self) -> Result<usize> {
        let mut conv = self.doc.clone();
        let n = bo_encoding::convert_document_runs(&mut conv);
        if n == 0 {
            return Ok(0);
        }
        let track = std::mem::replace(&mut self.track, false);
        let res = self.edit(|tx, _| {
            let old = tx.doc.sections.clone();
            tx.push(Op::Sections {
                index: 0,
                remove: old,
                insert: conv.sections,
            })?;
            for (endnote, new) in [(false, conv.footnotes), (true, conv.endnotes)] {
                let old = if endnote {
                    &tx.doc.endnotes
                } else {
                    &tx.doc.footnotes
                };
                if *old != new {
                    let old = old.clone();
                    tx.push(Op::Notes { endnote, old, new })?;
                }
            }
            if tx.doc.styles != conv.styles {
                tx.push(Op::Styles {
                    old: Box::new(tx.doc.styles.clone()),
                    new: Box::new(conv.styles),
                })?;
            }
            Ok(Selection::caret(Pos::new(0, 0)))
        });
        self.track = track;
        res.map(|_| n)
    }

    /// Turn track-changes mode on or off (hosts save it as §17.15.1.89 trackRevisions,
    /// see [`Session::doc_with_track_setting`]). While tracking is locked ([`Session::tracking_locked`]) it cannot be turned off.
    pub fn set_track_changes(&mut self, on: bool) {
        if !on && self.tracking_locked() {
            return;
        }
        self.track = on;
        self.typing = false;
    }

    /// The document with its §17.15.1.89 trackRevisions setting matching the session's
    /// track-changes mode (borrowed when it already does), for saving.
    #[must_use]
    pub fn doc_with_track_setting(&self) -> std::borrow::Cow<'_, Document> {
        if self.doc.track_revisions == self.track {
            std::borrow::Cow::Borrowed(&self.doc)
        } else {
            let mut d = self.doc.clone();
            d.track_revisions = self.track;
            std::borrow::Cow::Owned(d)
        }
    }

    /// Tracking is locked: the document allows only tracked changes (§17.15.1.29
    /// documentProtection `w:edit="trackedChanges"`, enforced), so track changes stays on
    /// and revisions cannot be accepted or rejected until the protection is stopped.
    #[must_use]
    pub fn tracking_locked(&self) -> bool {
        self.doc
            .protection
            .as_ref()
            .is_some_and(bo_model::form::DocProtection::tracked_changes_enforced)
    }

    /// Whether new edits are recorded as revisions.
    pub fn track_changes(&self) -> bool {
        self.track
    }

    /// Author of new revisions and comments (empty → [`DEFAULT_AUTHOR`]).
    pub fn set_author(&mut self, author: &str) {
        let a = author.trim();
        self.author = if a.is_empty() {
            DEFAULT_AUTHOR.to_owned()
        } else {
            a.to_owned()
        };
    }

    /// Author name stamped on new revisions and comments.
    pub fn author(&self) -> &str {
        &self.author
    }

    /// Date stamped on new revisions and comments (`ST_DateTime`, e.g. `2026-11-08T09:30:00Z`).
    pub fn set_clock(&mut self, date: Option<String>) {
        self.clock = date;
    }

    /// Date stamped on new revisions and comments, if set.
    pub fn clock(&self) -> Option<&str> {
        self.clock.as_deref()
    }

    /// Revisions of the main text (review pane).
    pub fn revisions(&self) -> Vec<RevisionItem> {
        revisions::list(&self.doc)
    }

    /// Ids of the revisions touching the selection (or containing the caret).
    pub fn revisions_at_selection(&self) -> Vec<i32> {
        let (a, b) = self.selection.ordered();
        let mut ids: Vec<i32> = self
            .revisions()
            .into_iter()
            .filter(|r| r.story.is_none())
            .filter(|r| {
                if a == b {
                    r.start <= a && a <= r.end
                } else {
                    r.start < b && a < r.end
                }
            })
            .map(|r| r.id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// Compare with `original` (prompt §7.1): the body becomes this document with its differences
    /// from the original as tracked changes by the session's author (one undo step). Returns the
    /// number of revisions made.
    pub fn compare_with_original(&mut self, original: &Document) -> Result<usize> {
        let first = revisions::max_id(&self.doc).max(revisions::max_id(original));
        let out = compare::compare(
            original,
            &self.doc,
            &self.author.clone(),
            self.clock.clone(),
            first,
        );
        let made = (revisions::max_id(&out) - first).max(0) as usize;
        if made == 0 {
            return Ok(0);
        }
        let caret = self.selection.ordered().0;
        self.edit(|tx, _| {
            for (si, sec) in out.sections.iter().enumerate() {
                if si < tx.doc.sections.len() && tx.doc.sections[si].blocks != sec.blocks {
                    tx.set_section_blocks(si, &sec.blocks)?;
                }
            }
            Ok(Selection::caret(caret))
        })?;
        let c = self.clamp(caret);
        self.selection = Selection::caret(c);
        if let Some(t) = self.undo.last_mut() {
            t.after = self.selection;
        }
        Ok(made)
    }

    /// Combine with another reviewer's copy (prompt §7.1 "Kết hợp"): the original is this
    /// document with its revisions rejected; the result holds this document's changes (by their
    /// author) and those of `other` (by its revisions' author, else `other_author`), all as
    /// tracked changes (one undo step). Returns the number of revisions.
    pub fn combine_with(&mut self, other: &Document, other_author: &str) -> Result<usize> {
        let resolved = |d: &Document, accept: bool| {
            let mut d = d.clone();
            for s in &mut d.sections {
                s.blocks = revisions::resolve_blocks(&s.blocks, &|_| true, accept);
            }
            d
        };
        // The most frequent revision author of a document.
        let author_of = |d: &Document| {
            let mut n: std::collections::BTreeMap<String, usize> =
                std::collections::BTreeMap::new();
            for r in revisions::list(d) {
                *n.entry(r.author).or_default() += 1;
            }
            n.into_iter().max_by_key(|(_, c)| *c).map(|(a, _)| a)
        };
        let base = resolved(&self.doc, false);
        let mine = resolved(&self.doc, true);
        let theirs = resolved(other, true);
        let author_a = author_of(&self.doc).unwrap_or_else(|| self.author.clone());
        let author_b = author_of(other).unwrap_or_else(|| other_author.to_owned());
        let first = revisions::max_id(&self.doc).max(revisions::max_id(other));
        let out = compare::combine(
            &base,
            (&mine, &author_a),
            (&theirs, &author_b),
            self.clock.as_deref(),
            first,
        );
        let made = revisions::list(&out).len();
        let caret = self.selection.ordered().0;
        self.edit(|tx, _| {
            for (si, sec) in out.sections.iter().enumerate() {
                if si < tx.doc.sections.len() && tx.doc.sections[si].blocks != sec.blocks {
                    tx.set_section_blocks(si, &sec.blocks)?;
                }
            }
            Ok(Selection::caret(caret))
        })?;
        let c = self.clamp(caret);
        self.selection = Selection::caret(c);
        if let Some(t) = self.undo.last_mut() {
            t.after = self.selection;
        }
        Ok(made)
    }

    /// What fields read from outside the text (clock, document properties, user, variables).
    pub fn set_field_env(&mut self, env: fields::FieldEnv) {
        self.field_env = env;
    }

    /// The field environment with the session clock as "now" when it has none.
    fn fields_now(&self) -> fields::FieldEnv {
        let mut env = self.field_env.clone();
        if env.now.is_none() {
            env.now = self
                .clock
                .as_deref()
                .and_then(bo_model::field::DateTime::parse_iso);
        }
        if env.user_name.is_empty() {
            env.user_name.clone_from(&self.author);
        }
        env
    }

    /// The environment of field updates.
    pub fn field_env(&self) -> &fields::FieldEnv {
        &self.field_env
    }

    /// Every field of the main text in document order, as (paragraph, from, to) index-text spans:
    /// simple and complex fields (§17.16.19 fldSimple, §17.16.18 fldChar) and legacy form fields
    /// (§17.16.17).
    fn field_spans(&self) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        for (pid, path) in body_paragraphs(&self.doc).iter().enumerate() {
            let Some(p) = para_at(&self.doc, path) else {
                continue;
            };
            let atoms = para::flatten(p);
            let mut starts = Vec::with_capacity(atoms.len() + 1);
            let mut pos = 0;
            for a in &atoms {
                starts.push(pos);
                pos += para::atom_len(&a.1);
            }
            starts.push(pos);
            let mut spans: Vec<(usize, usize, usize)> = fields::para_fields(&atoms)
                .into_iter()
                .map(|(_, a, b)| (pid, starts[a], starts[b + 1]))
                .collect();
            let mut open = None;
            for (i, a) in atoms.iter().enumerate() {
                match a.1 {
                    RunContent::FormField(_) => open = Some(i),
                    RunContent::FormEnd => {
                        if let Some(s) = open.take() {
                            spans.push((pid, starts[s], starts[i + 1]));
                        }
                    }
                    _ => {}
                }
            }
            spans.sort_unstable();
            out.extend(spans);
        }
        out
    }

    /// Select the next field after the selection (`forward`, F11) or the one before it
    /// (Shift+F11), wrapping around the document. Returns the new selection, `None` when the
    /// document has no field.
    pub fn next_field(&mut self, forward: bool) -> Option<Selection> {
        let all = self.field_spans();
        let (a, b) = self.selection.ordered();
        let is_current =
            |s: &(usize, usize, usize)| a == Pos::new(s.0, s.1) && b == Pos::new(s.0, s.2);
        let next = if forward {
            all.iter()
                .find(|s| (s.0, s.1) >= (b.para, b.offset) && !is_current(s))
                .or_else(|| all.first())
        } else {
            all.iter()
                .rev()
                .find(|s| (s.0, s.2) <= (a.para, a.offset) && !is_current(s))
                .or_else(|| all.last())
        }?;
        self.selection = Selection {
            anchor: Pos::new(next.0, next.1),
            focus: Pos::new(next.0, next.2),
        };
        Some(self.selection)
    }

    /// The ASK / FILLIN questions an update of the fields asks (§17.16.5.4, §17.16.5.20).
    #[must_use]
    pub fn field_prompts(&self) -> Vec<merge::rules::Prompt> {
        fields::prompts(&self.doc)
    }

    /// Update the fields with the answers typed for their ASK / FILLIN questions (by the keys
    /// of [`Session::field_prompts`]; a missing one: the default answer).
    pub fn update_fields_answered(
        &mut self,
        pages_of: &mut dyn FnMut(&Document) -> Vec<u32>,
        answers: std::collections::BTreeMap<String, String>,
    ) -> Result<usize> {
        self.field_env.answers = answers;
        let r = self.update_fields(pages_of);
        self.field_env.answers.clear();
        r
    }

    /// The fields F9 updates: those the selection covers or the caret touches (all of them
    /// when everything is selected).
    #[must_use]
    pub fn selection_scope(&self) -> fields::Scope {
        fields::Scope {
            ranges: self.selected_ranges(self.selection),
        }
    }

    /// The ASK / FILLIN questions an update of the fields in the selection asks.
    #[must_use]
    pub fn selected_field_prompts(&self) -> Vec<merge::rules::Prompt> {
        fields::prompts_in(&self.doc, &self.selection_scope())
    }

    /// F9: update the fields in the selection (see [`Session::selection_scope`]) with the
    /// answers typed for their ASK / FILLIN questions, in one undo step; the updated fields show
    /// their results again (a field showing its code after Ctrl+F9 / Shift+F9). Returns the
    /// number of fields whose result changed.
    pub fn update_selected_fields(
        &mut self,
        pages_of: &mut dyn FnMut(&Document) -> Vec<u32>,
        answers: std::collections::BTreeMap<String, String>,
    ) -> Result<usize> {
        let scope = self.selection_scope();
        let mut env = self.fields_now();
        env.answers = answers;
        let sel = self.selection;
        let mut n = 0;
        self.edit(|tx, _| {
            n = tx.update_fields_in(pages_of, &env, Some(&scope))?;
            Ok(sel)
        })?;
        self.fix_selection();
        Ok(n)
    }

    /// The fields in the selection (the one the caret touches): (paragraph id, atom index,
    /// code, showing its code).
    fn selected_fields(&self) -> Vec<(usize, usize, String, bool)> {
        let scope = self.selection_scope();
        let paths = body_paragraphs(&self.doc);
        let mut out = Vec::new();
        for &(pid, _, _) in &scope.ranges {
            let Some(p) = paths.get(pid).and_then(|path| para_at(&self.doc, path)) else {
                continue;
            };
            let mut pos = 0;
            for (ai, (_, c)) in para::flatten(p).iter().enumerate() {
                let l = para::atom_len(c);
                if let RunContent::Field { instr, codes, .. } = c {
                    if scope.hits(pid, pos, pos + l) {
                        out.push((pid, ai, instr.clone(), *codes));
                    }
                }
                pos += l;
            }
        }
        out
    }

    /// Shift+F9: the fields in the selection (the one the caret touches) show their codes
    /// instead of their results, or their results again — all the opposite of the first one.
    /// Returns the number of fields toggled (one undo step).
    pub fn toggle_field_codes(&mut self) -> Result<usize> {
        let targets = self.selected_fields();
        let Some(show) = targets.first().map(|t| !t.3) else {
            return Ok(0);
        };
        let sel = self.selection;
        self.edit(|tx, _| {
            let mut i = 0;
            while i < targets.len() {
                let pid = targets[i].0;
                let (path, p, mut atoms) = tx.atoms(pid)?;
                while i < targets.len() && targets[i].0 == pid {
                    if let Some((_, RunContent::Field { codes, .. })) = atoms.get_mut(targets[i].1)
                    {
                        *codes = show;
                    }
                    i += 1;
                }
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            }
            Ok(sel)
        })?;
        Ok(targets.len())
    }

    /// Ctrl+F9 with nothing selected: an empty field `{ }` at the caret showing its code, the
    /// field selected so its code is typed in place ([`Session::set_field_code`]).
    pub fn insert_empty_field(&mut self) -> Result<Selection> {
        let field = RunContent::Field {
            instr: String::new(),
            cached: String::new(),
            locked: false,
            codes: true,
        };
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let end = tx.insert_atoms(at, vec![(props, field)])?;
            Ok(Selection {
                anchor: at,
                focus: end,
            })
        })
    }

    /// The code of the field the selection covers or the caret touches, and whether it shows
    /// its code.
    #[must_use]
    pub fn field_at(&self) -> Option<(String, bool)> {
        self.selected_fields()
            .into_iter()
            .next()
            .map(|t| (t.2, t.3))
    }

    /// Replace the code of the field the selection covers or the caret touches (typed in place
    /// after Ctrl+F9, or in a field showing its code). Returns whether there is one (one undo
    /// step when the code changes).
    pub fn set_field_code(&mut self, instr: &str) -> Result<bool> {
        let Some((pid, ai, old, _)) = self.selected_fields().into_iter().next() else {
            return Ok(false);
        };
        let new = instr.trim();
        if old == new {
            return Ok(true);
        }
        let sel = self.selection;
        self.edit(|tx, _| {
            let (path, p, mut atoms) = tx.atoms(pid)?;
            if let Some((_, RunContent::Field { instr, .. })) = atoms.get_mut(ai) {
                new.clone_into(instr);
            }
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            Ok(sel)
        })?;
        Ok(true)
    }

    /// Lock (`Some(true)`), unlock (`Some(false)`) or unlink (`None`: the result becomes plain
    /// text) the fields in the selection — a caret acts on the field it touches (§17.16.18
    /// fldLock). Returns the number of fields (one undo step).
    pub fn set_fields_locked(&mut self, lock: Option<bool>) -> Result<usize> {
        let ranges = self.selected_ranges(self.selection);
        let (a, b) = self.selection.ordered();
        let collapsed = a == b;
        let mut n = 0;
        self.edit(|tx, sel| {
            for (id, s, e) in ranges {
                let (path, p, mut atoms) = tx.atoms(id)?;
                let mut pos = 0;
                let mut changed = false;
                for at in &mut atoms {
                    let l = para::atom_len(&at.1);
                    let touched = if collapsed {
                        pos == s || pos + l == s
                    } else {
                        pos < e && pos + l > s
                    };
                    pos += l;
                    if !touched {
                        continue;
                    }
                    if let RunContent::Field { cached, locked, .. } = &mut at.1 {
                        if let Some(v) = lock {
                            *locked = v;
                        } else {
                            let text = std::mem::take(cached);
                            at.1 = RunContent::Text(text);
                        }
                        changed = true;
                        n += 1;
                    }
                }
                if changed {
                    tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                }
            }
            Ok(sel)
        })?;
        self.fix_selection();
        Ok(n)
    }

    /// Accept (`accept`) or reject revisions: `ids = None` resolves all of them.
    /// Returns the number of revisions resolved (one undo step).
    pub fn resolve_revisions(&mut self, ids: Option<&[i32]>, accept: bool) -> Result<usize> {
        if self.tracking_locked() {
            return Err(EditError::Unsupported(TRACKING_LOCKED.into()));
        }
        // Both halves of a tracked move are resolved together (§17.13.5.22–27).
        let moves = moves::move_groups(&self.doc);
        // Both halves of an edited equation too (ADR-0015 addendum).
        let set: Option<std::collections::HashSet<i32>> = ids.map(|v| {
            let mut s = moves.expand(v);
            revisions::with_equation_halves(&self.doc, &mut s);
            s
        });
        let dead = moves.dead_markers(set.as_ref());
        let all = self.revisions();
        let n = match &set {
            None => all.len(),
            Some(ids) => all.iter().filter(|r| ids.contains(&r.id)).count(),
        };
        if n == 0 {
            return Ok(0);
        }
        let filter = |id: i32| set.as_ref().is_none_or(|s| s.contains(&id));
        let resolve = |b: &[Block]| {
            moves::strip_markers(&revisions::resolve_blocks(b, &filter, accept), &dead)
        };
        let caret = self.selection.ordered().0;
        self.edit(|tx, _| {
            let before = media_in_body(tx.doc);
            for si in 0..tx.doc.sections.len() {
                let new = resolve(&tx.doc.sections[si].blocks);
                tx.set_section_blocks(si, &new)?;
                let mut sec = revisions::resolve_section(&tx.doc.sections[si], &filter, accept);
                // Revisions inside headers and footers (§17.10).
                let cur = sec.as_ref().unwrap_or(&tx.doc.sections[si]).clone();
                let mut hf = cur.clone();
                for v in [HfVariant::Default, HfVariant::First, HfVariant::Even] {
                    for set in [&mut hf.headers, &mut hf.footers] {
                        let slot = section::hf_slot_mut(set, v);
                        if let Some(b) = slot.as_ref() {
                            *slot = Some(resolve(b));
                        }
                    }
                }
                if hf != cur {
                    sec = Some(hf);
                }
                if let Some(sec) = sec {
                    let old = tx.doc.sections[si].clone();
                    tx.push(Op::Sections {
                        index: si,
                        remove: vec![old],
                        insert: vec![sec],
                    })?;
                }
            }
            // Revisions inside footnotes and endnotes (§17.11).
            for endnote in [false, true] {
                let old = if endnote {
                    tx.doc.endnotes.clone()
                } else {
                    tx.doc.footnotes.clone()
                };
                let new: std::collections::BTreeMap<i32, Vec<Block>> =
                    old.iter().map(|(id, b)| (*id, resolve(b))).collect();
                if new != old {
                    tx.push(Op::Notes { endnote, old, new })?;
                }
            }
            // Pictures removed by resolving (rejected insertions) take their media part along.
            let after = media_in_body(tx.doc);
            for name in before.difference(&after) {
                if !doc_uses_media(tx.doc, name) {
                    if let Some(data) = tx.doc.media.get(name).cloned() {
                        tx.push(Op::Media {
                            name: name.clone(),
                            old: Some(data),
                            new: None,
                        })?;
                    }
                }
            }
            Ok(Selection::caret(caret))
        })?;
        let c = self.clamp(caret);
        self.selection = Selection::caret(c);
        if let Some(t) = self.undo.last_mut() {
            t.after = self.selection;
        }
        Ok(n)
    }

    // -------------------------------------------------------------- fields

    /// Update every field (TOC, SEQ, REF, PAGEREF; ECMA-376 Part 1, §17.16) in one
    /// undo step. `pages_of(doc)` lays a document out and returns the displayed page
    /// number of each body paragraph; it is called again until page numbers settle.
    /// Returns the number of fields whose result changed.
    pub fn update_fields(
        &mut self,
        pages_of: &mut dyn FnMut(&Document) -> Vec<u32>,
    ) -> Result<usize> {
        // Computed form fields first: REF fields may show their results (§17.16.17).
        let mut n = self.recalc_form_fields()?;
        let sel = self.selection;
        let env = self.fields_now();
        self.edit(|tx, _| {
            n += tx.update_fields(pages_of, &env)?;
            Ok(sel)
        })?;
        self.fix_selection();
        Ok(n)
    }

    /// Insert a table of contents (`TOC \o "1-3" \h \z \u`, §17.16.5.68) at the caret
    /// and fill it.
    pub fn insert_toc(&mut self, pages_of: &mut dyn FnMut(&Document) -> Vec<u32>) -> Result<usize> {
        let mut n = 0;
        let env = self.fields_now();
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let paths = tx.paths();
            let path = path_of(tx.doc, &paths, at.para)?;
            if path.len() != 2 {
                return Err(EditError::Unsupported(
                    "table of contents inside a table".into(),
                ));
            }
            let p = para_clone(tx.doc, &path)?;
            let len = para_len(&p);
            let (prefix, index) = split_path(&path);
            let insert_at = if at.offset == 0 {
                index
            } else {
                if at.offset < len {
                    tx.split(at)?;
                }
                index + 1
            };
            let toc = fields::new_toc_paragraph("TOC \\o \"1-3\" \\h \\z \\u");
            tx.insert_blocks(prefix, insert_at, vec![Block::Paragraph(toc)])?;
            n = tx.update_fields(pages_of, &env)?;
            let caret = Pos::new(at.para.min(tx.paths().len().saturating_sub(1)), 0);
            Ok(Selection::caret(caret))
        })?;
        self.fix_selection();
        Ok(n)
    }

    /// Insert a caption paragraph after the caret's paragraph: `label n: text` with a
    /// `SEQ label` field (§17.16.5.56).
    pub fn insert_caption(&mut self, label: &str, text: &str) -> Result<Selection> {
        let label = label.trim();
        if label.is_empty() || label.contains(char::is_whitespace) {
            return Err(EditError::Unsupported("caption label".into()));
        }
        let at = self.selection.ordered().1;
        let n = fields::next_seq(&self.doc, label, at.para);
        let cap = fields::caption_paragraph(label, n, text);
        let need = fields::ensure_styles(&self.doc.styles, &["Caption".to_owned()]);
        self.edit(|tx, _| {
            if let Some(new) = need {
                tx.push(Op::Styles {
                    old: Box::new(tx.doc.styles.clone()),
                    new: Box::new(new),
                })?;
            }
            let paths = tx.paths();
            let path = path_of(tx.doc, &paths, at.para)?;
            let (prefix, index) = split_path(&path);
            let mut cap = cap;
            tx.new_mark(&mut cap.props);
            cap.runs = para::rebuild(tx.mark_new(&[], 0, para::flatten(&cap)));
            let len = para_len(&cap);
            tx.insert_blocks(prefix, index + 1, vec![Block::Paragraph(cap)])?;
            Ok(Selection::caret(Pos::new(at.para + 1, len)))
        })
    }

    /// Insert a numbered equation as a new paragraph after the caret's: the equation centred
    /// (centre tab stop) and its number `(SEQ label)` at the right margin (right tab stop),
    /// numbered like captions (§17.16.5.56 SEQ).
    pub fn insert_numbered_equation(&mut self, math_xml: &str, label: &str) -> Result<Selection> {
        let label = label.trim();
        if label.is_empty() || label.contains(char::is_whitespace) {
            return Err(EditError::Unsupported("equation label".into()));
        }
        let at = self.selection.ordered().1;
        let n = fields::next_seq(&self.doc, label, at.para);
        self.edit(|tx, _| {
            let paths = tx.paths();
            let path = path_of(tx.doc, &paths, at.para)?;
            let section = path.first().copied().unwrap_or(0);
            let width = tx.doc.sections.get(section).map_or(9355, |s| {
                s.page.width.0 - s.page.margin_left.0 - s.page.margin_right.0
            });
            let (prefix, index) = split_path(&path);
            let tab = |pos: i32, align: TabAlign| bo_model::TabStop {
                pos: Twips(pos),
                align,
                leader: bo_model::TabLeader::None,
            };
            let props = bo_model::ParaProps {
                tabs: Some(vec![
                    tab(width / 2, TabAlign::Center),
                    tab(width, TabAlign::Right),
                ]),
                ..bo_model::ParaProps::default()
            };
            let rp = RunProps::default();
            let atoms = vec![
                (rp.clone(), RunContent::Tab),
                (
                    rp.clone(),
                    RunContent::Markup(bo_model::Raw::new(math_xml, 0)),
                ),
                (rp.clone(), RunContent::Tab),
                (rp.clone(), RunContent::Text("(".into())),
                (
                    rp.clone(),
                    RunContent::Field {
                        instr: format!("SEQ {label} \\* ARABIC"),
                        cached: n.to_string(),
                        locked: false,
                        codes: false,
                    },
                ),
                (rp, RunContent::Text(")".into())),
            ];
            let mut p = bo_model::Paragraph::new(props, para::rebuild(atoms));
            tx.new_mark(&mut p.props);
            p.runs = para::rebuild(tx.mark_new(&[], 0, para::flatten(&p)));
            let len = para_len(&p);
            tx.insert_blocks(prefix, index + 1, vec![Block::Paragraph(p)])?;
            Ok(Selection::caret(Pos::new(at.para + 1, len)))
        })
    }

    /// The selected text (index text; paragraphs joined by spaces), e.g. the code Ctrl+F9 puts
    /// between field braces.
    #[must_use]
    pub fn selected_text(&self) -> String {
        let (a, b) = self.selection.ordered();
        let mut parts = Vec::new();
        for id in a.para..=b.para {
            let t = self.para_text(id).unwrap_or_default();
            let from = if id == a.para {
                a.offset.min(t.len())
            } else {
                0
            };
            let to = if id == b.para {
                b.offset.min(t.len())
            } else {
                t.len()
            };
            parts.push(t.get(from..to.max(from)).unwrap_or_default().to_owned());
        }
        parts.join(" ")
    }

    /// Insert a simple field (§17.16.19 fldSimple) with its current result at the caret.
    pub fn insert_field(&mut self, instr: &str, cached: &str) -> Result<Selection> {
        let field = RunContent::Field {
            instr: instr.trim().to_owned(),
            cached: cached.to_owned(),
            locked: false,
            codes: false,
        };
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            Ok(Selection::caret(tx.insert_atoms(at, vec![(props, field)])?))
        })
    }

    /// Insert a mail-merge rule as a complex field (§17.16.18) whose instruction holds
    /// `parts` (nested MERGEFIELDs), showing `shown` until merged.
    pub fn insert_merge_rule(
        &mut self,
        parts: &[merge::rules::RulePart],
        shown: &str,
    ) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let field = merge::rules::complex_field(parts, shown, &props);
            Ok(Selection::caret(tx.insert_atoms(at, field)?))
        })
    }

    /// Insert an inline content control (§17.5.2.31 sdt) of `kind` (`text`, `richText`,
    /// `checkbox`, `dropDown`, `comboBox`, `date`, `picture`) at the caret, showing `placeholder`
    /// (a checkbox shows ☐); the content is selected. List controls offer `items`. A
    /// `repeatingSection` wraps the selected paragraphs instead (`placeholder`: its title).
    pub fn insert_control(
        &mut self,
        kind: &str,
        placeholder: &str,
        items: &[String],
    ) -> Result<Selection> {
        let list: Vec<(String, String)> = items.iter().map(|i| (i.clone(), i.clone())).collect();
        let ck = match kind {
            "text" => ControlKind::Text { multi_line: false },
            "richText" => ControlKind::RichText,
            "checkbox" => ControlKind::Checkbox {
                checked: false,
                on: '\u{2612}',
                off: '\u{2610}',
            },
            "dropDown" => ControlKind::DropDown(list),
            "comboBox" => ControlKind::ComboBox(list),
            "date" => ControlKind::Date {
                format: "dd/MM/yyyy".into(),
                full_date: None,
                store_as: Some("dateTime".into()),
                lid: None,
            },
            // §17.5.2.26 picture: the placeholder until a picture replaces it.
            "picture" => ControlKind::Picture,
            // §17.5.2 docPartList: the Quick Parts gallery, every category.
            "buildingBlockGallery" => ControlKind::Gallery {
                gallery: "Quick Parts".into(),
                category: None,
            },
            "repeatingSection" => return self.insert_repeating_section(placeholder),
            _ => return Err(EditError::Unsupported(format!("control {kind}"))),
        };
        let checkbox = matches!(ck, ControlKind::Checkbox { .. });
        let id = 1 + max_control_id(&self.doc);
        let cc = ContentControl {
            id: Some(id),
            alias: None,
            tag: None,
            kind: ck,
            lock: None,
            placeholder: !checkbox,
            block: false,
            props_xml: None,
            data_binding: None,
            appearance: None,
            color: None,
            temporary: false,
        };
        let text = if checkbox {
            "\u{2610}".to_owned()
        } else {
            placeholder.to_owned()
        };
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            // One insertion, so the placeholder text stays marked as such.
            let n = text.len();
            tx.insert_atoms(
                at,
                vec![
                    (RunProps::default(), RunContent::ControlStart(Box::new(cc))),
                    (props, RunContent::Text(text)),
                    (RunProps::default(), RunContent::ControlEnd(false)),
                ],
            )?;
            Ok(Selection {
                anchor: at,
                focus: Pos::new(at.para, at.offset + n),
            })
        })
    }

    /// Insert a picture content control (§17.5.2.26 picture) at the caret showing `png` (a
    /// placeholder picture `size` EMU square) as its placeholder (§17.5.2.39 showingPlcHdr); a
    /// picture chosen for it replaces it. The placeholder is selected.
    pub fn insert_picture_control(&mut self, png: Vec<u8>, size: i64) -> Result<Selection> {
        let start = self.selection.ordered().0;
        let id = 1 + max_control_id(&self.doc);
        let cc = ContentControl {
            id: Some(id),
            alias: None,
            tag: None,
            kind: ControlKind::Picture,
            lock: None,
            placeholder: true,
            block: false,
            props_xml: None,
            data_binding: None,
            appearance: None,
            color: None,
            temporary: false,
        };
        self.insert_image(png, "png", size, size)?;
        // Wrap the new picture in the control (one undo step with the picture).
        let pic_end = self.selection.focus;
        let before = self.undo.len();
        self.edit(|tx, _| {
            let (path, p, mut atoms) = tx.atoms(pic_end.para)?;
            let mut pos = 0;
            let mut k = atoms.len();
            for (i, a) in atoms.iter().enumerate() {
                pos += para::atom_len(&a.1);
                if pos == pic_end.offset && matches!(a.1, RunContent::Image(_)) {
                    k = i;
                    break;
                }
            }
            if k == atoms.len() {
                return Err(EditError::BadPosition);
            }
            atoms.insert(k + 1, (RunProps::default(), RunContent::ControlEnd(false)));
            atoms.insert(
                k,
                (RunProps::default(), RunContent::ControlStart(Box::new(cc))),
            );
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            Ok(Selection {
                anchor: start,
                focus: pic_end,
            })
        })?;
        if self.undo.len() == before + 1 && before >= 1 {
            // Join the two steps (picture, control) into one.
            if let (Some(last), Some(prev)) = (self.undo.pop(), self.undo.last_mut()) {
                prev.ops.extend(last.ops);
                prev.after = last.after;
            }
        }
        Ok(self.selection)
    }

    /// The inline content control around the caret: its properties, the paragraph and the atom
    /// indices of its start and end markers.
    fn control_span(&self) -> Option<(ContentControl, usize, usize, usize)> {
        let caret = self.selection.focus;
        let path = body_paragraphs(&self.doc).get(caret.para)?.clone();
        let p = para_at(&self.doc, &path)?;
        let atoms = para::flatten(p);
        let mut pos = 0;
        let mut open: Vec<(usize, usize)> = Vec::new(); // (atom index, offset)
        let mut found = None;
        for (i, (_, c)) in atoms.iter().enumerate() {
            match c {
                RunContent::ControlStart(cc) if !cc.block => open.push((i, pos)),
                RunContent::ControlEnd(false) => {
                    if let Some((s, so)) = open.pop() {
                        if so <= caret.offset && caret.offset <= pos {
                            if let RunContent::ControlStart(cc) = &atoms[s].1 {
                                // The innermost control around the caret.
                                if found.is_none() {
                                    found = Some(((**cc).clone(), caret.para, s, i));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            pos += para::atom_len(c);
        }
        found
    }

    /// The content control around the caret, for the host.
    #[must_use]
    pub fn control_at_caret(&self) -> Option<ContentControl> {
        self.control_span().map(|c| c.0)
    }

    /// Set the content of the control around the caret: toggle a checkbox (`value` ignored), pick
    /// a list item or type a combo-box value, set a date (ISO 8601; shown in the control's format),
    /// or put text in a text control. Returns whether a control was changed (one undo step).
    pub fn set_control_value(&mut self, value: &str, vi: bool) -> Result<bool> {
        let Some((mut cc, para_id, s, e)) = self.control_span() else {
            return Ok(false);
        };
        if matches!(
            cc.lock.as_deref(),
            Some("contentLocked" | "sdtContentLocked")
        ) {
            return Ok(false);
        }
        let text = match &mut cc.kind {
            ControlKind::Checkbox { checked, on, off } => {
                *checked = !*checked;
                if *checked {
                    on.to_string()
                } else {
                    off.to_string()
                }
            }
            ControlKind::DropDown(items) => {
                match items.iter().find(|(d, v)| d == value || v == value) {
                    Some((d, _)) => d.clone(),
                    None => return Ok(false),
                }
            }
            ControlKind::Date {
                format, full_date, ..
            } => {
                let Some(d) = bo_model::field::DateTime::parse_iso(value) else {
                    return Ok(false);
                };
                *full_date = Some(format!(
                    "{:04}-{:02}-{:02}T00:00:00Z",
                    d.year, d.month, d.day
                ));
                bo_model::field::format_date(&d, format, vi)
            }
            ControlKind::Group
            | ControlKind::RepeatingSection
            | ControlKind::RepeatingSectionItem
            | ControlKind::Picture => return Ok(false),
            _ => value.to_owned(),
        };
        cc.placeholder = false;
        let caret = self.selection;
        self.edit(|tx, _| {
            let (path, p, atoms) = tx.atoms(para_id)?;
            let props = atoms
                .get(s + 1..e)
                .and_then(|x| x.iter().find(|a| matches!(a.1, RunContent::Text(_))))
                .map_or_else(|| atoms[s].0.clone(), |a| a.0.clone());
            let mut out: Vec<para::Atom> = atoms[..s].to_vec();
            out.push((atoms[s].0.clone(), RunContent::ControlStart(Box::new(cc))));
            if !text.is_empty() {
                out.push((props, RunContent::Text(text)));
            }
            out.extend_from_slice(&atoms[e..]);
            tx.replace_para(&path, para::with_atoms(&p, out))?;
            Ok(caret)
        })?;
        self.fix_selection();
        Ok(true)
    }

    /// Insert an Office Math zone in place of the selection (`xml`: an `m:oMath` or `m:oMathPara`
    /// element, ECMA-376 Part 1, §22.1.2.77 / §22.1.2.78); the new equation is selected.
    pub fn insert_equation(&mut self, xml: &str) -> Result<Selection> {
        let math = RunContent::Markup(bo_model::Raw::new(xml, 0));
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let end = tx.insert_atoms(at, vec![(props, math.clone())])?;
            Ok(Selection {
                anchor: at,
                focus: end,
            })
        })
    }

    /// Replace the Office Math zone that starts at `at` with `xml` (an equation edited in place,
    /// CORE-MATH-010; ECMA-376 Part 1, §22.1.2.77 oMath): its run formatting is kept and the caret
    /// is left just after it. With `group`, the change joins the previous one when that was a grouped
    /// edit of the same equation with nothing in between (one undo step per typed group).
    ///
    /// In track-changes mode the change is a revision of the whole equation (ADR-0015 addendum):
    /// the original zone is marked deleted (§17.13.5.14 del) and the edited one is inserted just
    /// after it (§17.13.5.18 ins); an equation this author inserted (the pending one of an edit
    /// session) is changed in place, so one session leaves one revision. Returns where the edited
    /// equation now starts (after the deleted original when one was marked).
    pub fn replace_equation(&mut self, at: Pos, xml: &str, group: bool) -> Result<Pos> {
        let mergeable = group && self.math_group == Some(at);
        let prev_after = self.undo.last().map(|t| t.after);
        let math = RunContent::Markup(bo_model::Raw::new(xml, 0));
        let mut new_at = at;
        self.edit(|tx, _| {
            let (path, p, mut atoms) = tx.atoms(at.para)?;
            let mut off = 0;
            let mut found = None;
            for (i, a) in atoms.iter().enumerate() {
                if off == at.offset
                    && matches!(&a.1, RunContent::Markup(r) if bo_model::index::is_math_xml(&r.xml))
                {
                    found = Some(i);
                    break;
                }
                off += para::atom_len(&a.1);
            }
            let i = found.ok_or(EditError::BadPosition)?;
            // A deleted equation is not edited.
            if atoms[i].0.del.is_some() {
                return Err(EditError::BadPosition);
            }
            let len = para::atom_len(&math);
            match tx.track.as_mut() {
                Some(t) if !t.owns(&atoms[i].0) => {
                    let del = t.mark();
                    let ins = t.mark();
                    let mut props = atoms[i].0.clone();
                    props.ins = Some(ins);
                    atoms[i].0.del = Some(del);
                    new_at = Pos::new(at.para, at.offset + para::atom_len(&atoms[i].1));
                    atoms.insert(i + 1, (props, math.clone()));
                }
                _ => atoms[i].1 = math.clone(),
            }
            tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            Ok(Selection::caret(Pos::new(new_at.para, new_at.offset + len)))
        })?;
        if mergeable
            && self.undo.len() >= 2
            && prev_after == Some(self.undo[self.undo.len() - 1].before)
        {
            let last = self.undo.pop();
            if let (Some(last), Some(prev)) = (last, self.undo.last_mut()) {
                prev.ops.extend(last.ops);
                prev.after = last.after;
            }
        }
        self.math_group = group.then_some(new_at);
        Ok(new_at)
    }

    /// The next in-place equation edit starts a new undo step.
    pub fn end_math_group(&mut self) {
        self.math_group = None;
    }

    /// The OMML of the equation that starts at `at`, if there is one.
    #[must_use]
    pub fn equation_at(&self, at: Pos) -> Option<String> {
        self.equation_atom(at).map(|(_, xml)| xml)
    }

    /// The equation at `at` is a tracked deletion (§17.13.5.14 del, §17.13.5.22 moveFrom).
    #[must_use]
    pub fn equation_deleted(&self, at: Pos) -> bool {
        self.equation_atom(at).is_some_and(|(p, _)| p.del.is_some())
    }

    fn equation_atom(&self, at: Pos) -> Option<(RunProps, String)> {
        let paths = body_paragraphs(&self.doc);
        let p = para_at(&self.doc, paths.get(at.para)?)?;
        let mut off = 0;
        for (props, c) in para::flatten(p) {
            if off == at.offset {
                if let RunContent::Markup(raw) = &c {
                    if bo_model::index::is_math_xml(&raw.xml) {
                        return Some((props, raw.xml.to_string()));
                    }
                }
            }
            if off > at.offset {
                break;
            }
            off += para::atom_len(&c);
        }
        None
    }

    /// The equation the selection covers exactly (or the one just before a collapsed caret): its range
    /// and its OMML.
    #[must_use]
    pub fn selected_equation(&self) -> Option<(Selection, String)> {
        let (a, b) = self.selection.ordered();
        if a.para != b.para {
            return None;
        }
        let paths = body_paragraphs(&self.doc);
        let p = para_at(&self.doc, paths.get(a.para)?)?;
        let mut off = 0;
        for (_, c) in para::flatten(p) {
            let n = para::atom_len(&c);
            if let RunContent::Markup(raw) = &c {
                if bo_model::index::is_math_xml(&raw.xml) {
                    let covers = a.offset == off && b.offset == off + n;
                    let after = a == b && a.offset == off + n;
                    if covers || after {
                        return Some((
                            Selection {
                                anchor: Pos::new(a.para, off),
                                focus: Pos::new(a.para, off + n),
                            },
                            raw.xml.to_string(),
                        ));
                    }
                }
            }
            off += n;
        }
        None
    }

    /// Bookmarks and captions that can be cross-referenced.
    pub fn ref_targets(&self) -> Vec<RefTarget> {
        fields::ref_targets(&self.doc)
    }

    /// Insert a cross-reference to `target` (a bookmark name or `caption:<id>` from
    /// [`Session::ref_targets`]): its text (`REF`) or page number (`PAGEREF`).
    pub fn insert_cross_reference(
        &mut self,
        target: &str,
        page: bool,
        pages_of: &mut dyn FnMut(&Document) -> Vec<u32>,
    ) -> Result<Selection> {
        let target = target.to_owned();
        let env = self.fields_now();
        self.edit(|tx, sel| {
            let name = if let Some(pid) = target.strip_prefix("caption:") {
                let pid: usize = pid.parse().map_err(|_| EditError::BadPosition)?;
                let paths = tx.paths();
                let path = path_of(tx.doc, &paths, pid)?;
                let p = para_clone(tx.doc, &path)?;
                let (atoms, name) = fields::bookmark_caption(tx.doc, &p, pid);
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                name
            } else {
                target.clone()
            };
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let instr = if page {
                format!("PAGEREF {name} \\h")
            } else {
                format!("REF {name} \\h")
            };
            let caret = tx.insert_atoms(
                at,
                vec![(
                    props,
                    RunContent::Field {
                        instr,
                        cached: String::new(),
                        locked: false,
                        codes: false,
                    },
                )],
            )?;
            tx.update_fields(pages_of, &env)?;
            Ok(Selection::caret(caret))
        })
    }

    fn fix_selection(&mut self) {
        let s = Selection {
            anchor: self.clamp(self.selection.anchor),
            focus: self.clamp(self.selection.focus),
        };
        self.selection = s;
        if let Some(t) = self.undo.last_mut() {
            t.after = s;
        }
    }

    // -------------------------------------------------------------- comments

    /// Comments with their anchors (comments pane).
    pub fn comments(&self) -> Vec<CommentInfo> {
        comments::list(&self.doc)
    }

    /// Comment on the selection (the word at the caret when collapsed); returns its id.
    pub fn add_comment(&mut self, text: &str) -> Result<i32> {
        self.add_comment_with(text, &[], None)
    }

    /// [`Self::add_comment`] with @mentions (UTF-16 ranges of `text`) and an
    /// assigned task (CORE-COMMENT-001).
    pub fn add_comment_with(
        &mut self,
        text: &str,
        mentions: &[bo_model::mention::Mention],
        task: Option<bo_model::mention::CommentTask>,
    ) -> Result<i32> {
        let sel = if self.selection.is_collapsed() {
            self.word_at(self.selection.focus)
        } else {
            self.selection
        };
        let (a, b) = sel.ordered();
        let id = seeded_id(comments::next_id(&self.doc), self.id_seed);
        let c = comments::make(
            &self.doc,
            id,
            &self.author,
            self.clock.as_deref(),
            text,
            None,
            mentions,
            task,
        );
        self.edit(|tx, sel| {
            let mut list = tx.doc.comments.clone();
            list.push(c);
            tx.push(Op::Comments {
                old: tx.doc.comments.clone(),
                new: list,
            })?;
            tx.insert_marks(
                b,
                vec![RunContent::CommentEnd(id), RunContent::CommentRef(id)],
            )?;
            tx.insert_marks(a, vec![RunContent::CommentStart(id)])?;
            Ok(sel)
        })?;
        Ok(id)
    }

    /// Reply to a comment (anchored next to it); returns the reply's id.
    pub fn reply_comment(&mut self, parent: i32, text: &str) -> Result<i32> {
        self.reply_comment_with(parent, text, &[], None)
    }

    /// [`Self::reply_comment`] with @mentions and an assigned task.
    pub fn reply_comment_with(
        &mut self,
        parent: i32,
        text: &str,
        mentions: &[bo_model::mention::Mention],
        task: Option<bo_model::mention::CommentTask>,
    ) -> Result<i32> {
        let Some(pi) = self.doc.comments.iter().position(|c| c.id == parent) else {
            return Err(EditError::Unsupported(format!("no comment {parent}")));
        };
        let anchors = comments::anchors(&self.doc);
        let [start, end, reference] = anchors.get(&parent).copied().unwrap_or_default();
        let id = seeded_id(comments::next_id(&self.doc), self.id_seed);
        let mut list = self.doc.comments.clone();
        if list[pi].para_id.is_none() {
            list[pi].para_id = Some(comments::new_para_id(&self.doc, &(parent, "parent")));
        }
        let reply = comments::make(
            &self.doc,
            id,
            &self.author,
            self.clock.as_deref(),
            text,
            Some(&list[pi]),
            mentions,
            task,
        );
        // Replies follow the thread they belong to.
        let thread = comments::thread_ids(&self.doc, parent);
        let at = list
            .iter()
            .rposition(|c| thread.contains(&c.id))
            .map_or(list.len(), |i| i + 1);
        list.insert(at, reply);
        self.edit(|tx, sel| {
            tx.push(Op::Comments {
                old: tx.doc.comments.clone(),
                new: list,
            })?;
            if let Some(e) = end.or(reference) {
                tx.insert_marks(
                    e,
                    vec![RunContent::CommentEnd(id), RunContent::CommentRef(id)],
                )?;
            }
            if let Some(s) = start.or(reference) {
                tx.insert_marks(s, vec![RunContent::CommentStart(id)])?;
            }
            Ok(sel)
        })?;
        Ok(id)
    }

    /// Mark a comment thread resolved (`done`) or open again ([MS-DOCX] `w15:done`).
    pub fn resolve_comment(&mut self, id: i32, done: bool) -> Result<()> {
        let mut list = self.doc.comments.clone();
        let Some(c) = list.iter_mut().find(|c| c.id == id) else {
            return Err(EditError::Unsupported(format!("no comment {id}")));
        };
        if c.done == done {
            return Ok(());
        }
        c.done = done;
        if c.para_id.is_none() {
            c.para_id = Some(comments::new_para_id(&self.doc, &(id, "done")));
        }
        self.edit(|tx, sel| {
            tx.push(Op::Comments {
                old: tx.doc.comments.clone(),
                new: list,
            })?;
            Ok(sel)
        })?;
        Ok(())
    }

    /// Delete a comment with its replies and their marks in the text.
    pub fn delete_comment(&mut self, id: i32) -> Result<()> {
        if !self.doc.comments.iter().any(|c| c.id == id) {
            return Err(EditError::Unsupported(format!("no comment {id}")));
        }
        let ids = comments::thread_ids(&self.doc, id);
        let list: Vec<bo_model::Comment> = self
            .doc
            .comments
            .iter()
            .filter(|c| !ids.contains(&c.id))
            .cloned()
            .collect();
        self.edit(|tx, sel| {
            tx.push(Op::Comments {
                old: tx.doc.comments.clone(),
                new: list,
            })?;
            let paths = tx.paths();
            for path in paths {
                let p = para_clone(tx.doc, &path)?;
                let all = para::flatten(&p);
                let n = all.len();
                let atoms: Vec<Atom> = all
                    .into_iter()
                    .filter(|(_, c)| {
                        !matches!(c, RunContent::CommentStart(i) | RunContent::CommentEnd(i)
                            | RunContent::CommentRef(i) if ids.contains(i))
                    })
                    .collect();
                if atoms.len() != n {
                    tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                }
            }
            Ok(sel)
        })?;
        Ok(())
    }

    /// Select the text a comment is anchored to.
    pub fn select_comment(&mut self, id: i32) -> bool {
        let Some(c) = self.comments().into_iter().find(|c| c.id == id) else {
            return false;
        };
        match (c.start, c.end) {
            (Some(s), Some(e)) => {
                let (s, e) = (self.clamp(s), self.clamp(e));
                self.set_selection(Selection {
                    anchor: s,
                    focus: e,
                });
                true
            }
            _ => false,
        }
    }

    /// Select the next (`forward`) or previous top-level comment; returns its id.
    pub fn next_comment(&mut self, forward: bool) -> Option<i32> {
        let (a, _) = self.selection.ordered();
        let list: Vec<CommentInfo> = self
            .comments()
            .into_iter()
            .filter(|c| c.parent.is_none() && c.start.is_some())
            .collect();
        let c = if forward {
            list.iter()
                .filter(|c| c.start > Some(a))
                .min_by_key(|c| c.start)
        } else {
            list.iter()
                .filter(|c| c.start < Some(a))
                .max_by_key(|c| c.start)
        }?;
        let id = c.id;
        self.select_comment(id);
        Some(id)
    }

    /// Select the next (or previous) revision relative to the selection; returns it.
    pub fn select_revision(&mut self, forward: bool) -> Option<RevisionItem> {
        let (a, _) = self.selection.ordered();
        let all: Vec<RevisionItem> = self
            .revisions()
            .into_iter()
            .filter(|r| r.story.is_none())
            .collect();
        let r = if forward {
            all.into_iter().find(|r| r.start > a)
        } else {
            all.into_iter().rev().find(|r| r.start < a)
        }?;
        let start = self.clamp(r.start);
        let end = self.clamp(r.end);
        self.set_selection(Selection {
            anchor: start,
            focus: end,
        });
        Some(r)
    }

    /// The document being edited.
    pub fn doc(&self) -> &Document {
        &self.doc
    }

    /// Consume the session and return the document.
    pub fn into_doc(self) -> Document {
        self.doc
    }

    /// The current selection.
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Incremented by every change of the document (edit, undo, redo).
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Blocks changed since the previous call (everything right after [`Session::new`]);
    /// hosts feed them to the incremental layout.
    pub fn take_changes(&mut self) -> BlockChanges {
        std::mem::take(&mut self.changes)
    }

    /// Blocks changed since the last [`Session::take_changes`].
    pub fn pending_changes(&self) -> &BlockChanges {
        &self.changes
    }

    /// There is an edit to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// There is an edit to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Number of body paragraphs.
    pub fn paragraph_count(&self) -> usize {
        body_paragraphs(&self.doc).len()
    }

    /// Clamp a position to an existing paragraph and a character boundary.
    pub fn clamp(&self, p: Pos) -> Pos {
        let paths = body_paragraphs(&self.doc);
        if paths.is_empty() {
            return Pos::default();
        }
        let para = p.para.min(paths.len() - 1);
        let text = para_at(&self.doc, &paths[para])
            .map(bo_model::index::para_index_text)
            .unwrap_or_default();
        let mut off = p.offset.min(text.len());
        while off > 0 && !text.is_char_boundary(off) {
            off -= 1;
        }
        Pos::new(para, off)
    }

    /// Set the selection (clamped).
    pub fn set_selection(&mut self, sel: Selection) {
        let s = Selection {
            anchor: self.clamp(sel.anchor),
            focus: self.clamp(sel.focus),
        };
        if s != self.selection {
            self.pending = None;
            self.typing = false;
        }
        self.selection = s;
    }

    /// Index text of a body paragraph.
    pub fn para_text(&self, id: usize) -> Option<String> {
        let paths = body_paragraphs(&self.doc);
        para_at(&self.doc, paths.get(id)?).map(bo_model::index::para_index_text)
    }

    fn edit(
        &mut self,
        f: impl FnOnce(&mut Tx<'_>, Selection) -> Result<Selection>,
    ) -> Result<Selection> {
        let before = self.selection;
        let track = self.track.then(|| Tracker {
            author: self.author.clone(),
            date: self.clock.clone(),
            next_id: revisions::max_id(&self.doc).max(self.id_floor),
        });
        let mut tx = Tx {
            doc: &mut self.doc,
            ops: Vec::new(),
            track,
        };
        let mut res = f(&mut tx, before);
        if res.is_ok() {
            // Deleting a note reference deletes its note (§17.11.14, §17.11.7).
            if let Err(e) = tx.drop_orphan_notes() {
                res = Err(e);
            }
        }
        if res.is_ok() && !tx.doc.custom_xml.is_empty() {
            // Bound content controls edited here update their nodes (§17.5.2.6), in this step.
            custom_xml::sync_in(
                &mut tx,
                bo_model::custom_xml::binding::SyncMode::FromControls,
            );
        }
        crate::ops::record_ops(&tx.ops, tx.doc, &mut self.changes);
        match res {
            Ok(after) => {
                let ops = tx.ops;
                if !ops.is_empty() {
                    self.version += 1;
                    self.undo.push(Transaction { ops, before, after });
                    self.redo.clear();
                }
                self.selection = after;
                self.typing = false;
                self.math_group = None;
                Ok(after)
            }
            Err(e) => {
                for op in tx.ops.iter().rev() {
                    let _ = op.inverse().apply(tx.doc);
                }
                Err(e)
            }
        }
    }

    /// Apply ops made by someone else (a collaborator's edit): not recorded in
    /// this user's history. The local history is dropped because its ops no
    /// longer match the document (collaborative undo goes through the replica).
    pub fn apply_external(&mut self, ops: Vec<Op>, selection: Selection) -> Result<()> {
        if ops.is_empty() {
            self.set_selection(selection);
            return Ok(());
        }
        let t = Transaction {
            ops,
            before: self.selection,
            after: selection,
        };
        t.apply(&mut self.doc)?;
        crate::ops::record_ops(&t.ops, &self.doc, &mut self.changes);
        self.version += 1;
        self.undo.clear();
        self.redo.clear();
        self.typing = false;
        let s = Selection {
            anchor: self.clamp(selection.anchor),
            focus: self.clamp(selection.focus),
        };
        self.selection = s;
        Ok(())
    }

    /// Undo the last transaction.
    pub fn undo(&mut self) -> bool {
        let Some(t) = self.undo.pop() else {
            return false;
        };
        let inv = t.inverse();
        if inv.apply(&mut self.doc).is_err() {
            return false;
        }
        crate::ops::record_ops(&inv.ops, &self.doc, &mut self.changes);
        self.selection = t.before;
        self.version += 1;
        self.redo.push(t);
        self.typing = false;
        self.math_group = None;
        true
    }

    /// Redo the last undone transaction.
    pub fn redo(&mut self) -> bool {
        let Some(t) = self.redo.pop() else {
            return false;
        };
        if t.apply(&mut self.doc).is_err() {
            return false;
        }
        crate::ops::record_ops(&t.ops, &self.doc, &mut self.changes);
        self.selection = t.after;
        self.version += 1;
        self.undo.push(t);
        self.typing = false;
        self.math_group = None;
        true
    }

    /// Revert the last transaction without keeping it for redo (an edit refused
    /// after it was applied, e.g. by document protection). `false` when none.
    pub fn discard_last(&mut self) -> bool {
        let Some(t) = self.undo.pop() else {
            return false;
        };
        let inv = t.inverse();
        if inv.apply(&mut self.doc).is_err() {
            self.undo.push(t);
            return false;
        }
        crate::ops::record_ops(&inv.ops, &self.doc, &mut self.changes);
        self.selection = t.before;
        self.version += 1;
        self.typing = false;
        true
    }

    /// Number of transactions that can be undone.
    pub fn history_len(&self) -> usize {
        self.undo.len()
    }

    /// Join every transaction recorded after the first `depth` into one undo step
    /// (a BlueScript run, CORE-SCRIPT-001). Returns the history length afterwards.
    pub fn merge_history_since(&mut self, depth: usize) -> usize {
        if self.undo.len() > depth + 1 {
            let mut tail = self.undo.split_off(depth);
            let mut first = tail.remove(0);
            for t in tail {
                first.ops.extend(t.ops);
                first.after = t.after;
            }
            self.undo.push(first);
            self.typing = false;
        }
        self.undo.len()
    }

    /// The next typed character starts a new undo step (no merging into the last one).
    pub fn end_typing(&mut self) {
        self.typing = false;
    }

    /// Insert zero-width markup around the selection (`start` at its start,
    /// `end` at its end), e.g. a `w:permStart` / `w:permEnd` pair (ECMA-376
    /// Part 1, §17.13.7). One undo step; the selection is kept.
    pub fn wrap_selection(
        &mut self,
        start: bo_model::Raw,
        end: bo_model::Raw,
    ) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            // End first: in the same paragraph the start offset stays valid.
            for (pos, raw, after) in [(b, end, true), (a, start, false)] {
                let (path, p, mut atoms) = tx.atoms(pos.para)?;
                let mut i = para::split_at(&mut atoms, pos.offset);
                if after {
                    // The end goes after other zero-width markup at the point.
                    while atoms.get(i).is_some_and(|x| para::atom_len(&x.1) == 0) {
                        i += 1;
                    }
                }
                atoms.insert(i, (RunProps::default(), RunContent::Markup(raw)));
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            }
            Ok(sel)
        })
    }

    /// Remove the zero-width markup items `f` selects (by their XML) from every
    /// body paragraph; one undo step. Returns how many were removed.
    pub fn remove_markup(&mut self, f: &dyn Fn(&str) -> bool) -> Result<usize> {
        let mut n = 0;
        self.edit(|tx, sel| {
            for path in tx.paths() {
                let p = para_clone(tx.doc, &path)?;
                let atoms = para::flatten(&p);
                let before = atoms.len();
                let kept: Vec<_> = atoms
                    .into_iter()
                    .filter(|(_, c)| !matches!(c, RunContent::Markup(r) if f(&r.xml)))
                    .collect();
                if kept.len() != before {
                    n += before - kept.len();
                    tx.replace_para(&path, para::with_atoms(&p, kept))?;
                }
            }
            Ok(sel)
        })?;
        Ok(n)
    }

    // -------------------------------------------------------------- text

    /// Type text at the selection. `\n` splits paragraphs; consecutive typing is one undo step per word.
    pub fn insert_text(&mut self, text: &str) -> Result<Selection> {
        if text.is_empty() {
            return Ok(self.selection);
        }
        let pending = self.pending.take();
        let mergeable = self.typing
            && self.selection.is_collapsed()
            && !text.contains('\n')
            && text.chars().count() == 1;
        let prev_after = self.undo.last().map(|t| t.after);
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let result = self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = pending.unwrap_or_else(|| para::props_at(&atoms, at.offset));
            let mut caret = at;
            for (i, line) in text.split('\n').enumerate() {
                if i > 0 {
                    caret = tx.split(caret)?;
                }
                caret = tx.insert_atoms(caret, para::text_atoms(line, &props))?;
            }
            Ok(Selection::caret(caret))
        })?;
        // Group single-character typing into one undo step per word.
        if mergeable
            && self.undo.len() >= 2
            && prev_after == Some(self.undo[self.undo.len() - 1].before)
        {
            let last = self.undo.pop();
            if let (Some(last), Some(prev)) = (last, self.undo.last_mut()) {
                prev.ops.extend(last.ops);
                prev.after = last.after;
            }
        }
        self.typing = !text.ends_with(char::is_whitespace);
        Ok(result)
    }

    /// Backspace.
    pub fn delete_backward(&mut self) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            if a != b {
                return Ok(Selection::caret(tx.delete_span(a, b)?.0));
            }
            if a.offset > 0 {
                let (_, p, _) = tx.atoms(a.para)?;
                let text = bo_model::index::para_index_text(&p);
                let s = prev_boundary(&text, a.offset);
                return Ok(Selection::caret(tx.delete_span(Pos::new(a.para, s), a)?.0));
            }
            let (path, p, atoms) = tx.atoms(a.para)?;
            // Backspace at the start of a list item removes the numbering first.
            if p.props.num.is_some_and(|n| n.num_id != 0) {
                let mut q = p.clone();
                q.props.num = None;
                tx.replace_para(&path, q)?;
                return Ok(sel);
            }
            // Then a first-line indent (set with Tab at the paragraph start) goes before the
            // paragraphs are joined (§17.3.1.12 ind firstLine).
            if p.props.indent_first_line.is_some_and(|t| t.0 > 0) {
                let mut q = p.clone();
                q.props.indent_first_line = None;
                tx.replace_para(&path, q)?;
                return Ok(sel);
            }
            let (prefix, index) = split_path(&path);
            if index == 0 {
                return Ok(sel);
            }
            let blocks = blocks_at(tx.doc, &prefix).ok_or(EditError::BadPath)?;
            let Some(Block::Paragraph(prev)) = blocks.get(index - 1).cloned() else {
                return Ok(sel);
            };
            if tx.track.is_some() {
                // §17.13.5.15: the previous paragraph mark is marked deleted.
                let caret = Pos::new(a.para - 1, para_len(&prev));
                if prev.props.mark_del.is_none() {
                    let mut path_prev = prefix;
                    path_prev.push(index - 1);
                    let mut q = prev.clone();
                    q.props.mark_del = tx.track.as_mut().map(Tracker::mark);
                    tx.replace_para(&path_prev, q)?;
                }
                return Ok(Selection::caret(caret));
            }
            let mut merged = para::flatten(&prev);
            let caret = para::len(&merged);
            merged.extend(atoms);
            tx.push(Op::Blocks {
                path: prefix,
                index: index - 1,
                remove: vec![Block::Paragraph(prev.clone()), Block::Paragraph(p)],
                insert: vec![Block::Paragraph(para::with_atoms(&prev, merged))],
            })?;
            Ok(Selection::caret(Pos::new(a.para - 1, caret)))
        })
    }

    /// Delete (forward).
    pub fn delete_forward(&mut self) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            if a != b {
                return Ok(Selection::caret(tx.delete_span(a, b)?.1));
            }
            let (path, p, atoms) = tx.atoms(a.para)?;
            let len = para::len(&atoms);
            if a.offset < len {
                let text = bo_model::index::para_index_text(&p);
                let e = next_boundary(&text, a.offset);
                return Ok(Selection::caret(tx.delete_span(a, Pos::new(a.para, e))?.1));
            }
            let (prefix, index) = split_path(&path);
            let blocks = blocks_at(tx.doc, &prefix).ok_or(EditError::BadPath)?;
            let Some(Block::Paragraph(next)) = blocks.get(index + 1).cloned() else {
                return Ok(sel);
            };
            if tx.track.is_some() {
                // §17.13.5.15: this paragraph's mark is marked deleted.
                if p.props.mark_del.is_none() {
                    let mut q = p.clone();
                    q.props.mark_del = tx.track.as_mut().map(Tracker::mark);
                    tx.replace_para(&path, q)?;
                }
                return Ok(Selection::caret(Pos::new(a.para + 1, 0)));
            }
            let mut merged = atoms;
            merged.extend(para::flatten(&next));
            tx.push(Op::Blocks {
                path: prefix,
                index,
                remove: vec![Block::Paragraph(p.clone()), Block::Paragraph(next)],
                insert: vec![Block::Paragraph(para::with_atoms(&p, merged))],
            })?;
            Ok(sel)
        })
    }

    /// Enter: split the paragraph (an empty list item leaves the list instead).
    pub fn split_paragraph(&mut self) -> Result<Selection> {
        // What is typed in the new empty paragraph keeps the formatting at the break (bold
        // text, Enter, more bold text), as word processors carry the paragraph mark's run
        // properties (§17.3.1.29 rPr of the paragraph mark) into the new paragraph.
        let carry = self
            .pending
            .clone()
            .unwrap_or_else(|| self.caret_direct_props());
        let r = self.split_paragraph_inner()?;
        if self.para_text(r.focus.para).is_some_and(|t| t.is_empty()) {
            let mut c = carry;
            c.link = None;
            c.ins = None;
            c.del = None;
            c.change = None;
            if c != RunProps::default() {
                self.pending = Some(c);
            }
        }
        Ok(r)
    }

    fn split_paragraph_inner(&mut self) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (path, p, atoms) = tx.atoms(at.para)?;
            if para::len(&atoms) == 0 && p.props.num.is_some_and(|n| n.num_id != 0) {
                let mut q = p.clone();
                q.props.num = None;
                tx.replace_para(&path, q)?;
                return Ok(Selection::caret(at));
            }
            Ok(Selection::caret(tx.split(at)?))
        })
    }

    // -------------------------------------------------------------- formatting

    fn selected_ranges(&self, sel: Selection) -> Vec<(usize, usize, usize)> {
        let (a, b) = sel.ordered();
        (a.para..=b.para)
            .map(|id| {
                let len = self.para_text(id).map_or(0, |t| t.len());
                let s = if id == a.para { a.offset } else { 0 };
                let e = if id == b.para { b.offset } else { len };
                (id, s, e)
            })
            .collect()
    }

    fn atoms_in_selection(&self) -> Vec<(Paragraph, RunProps)> {
        let mut out = Vec::new();
        let paths = body_paragraphs(&self.doc);
        for (id, s, e) in self.selected_ranges(self.selection) {
            let Some(p) = paths.get(id).and_then(|path| para_at(&self.doc, path)) else {
                continue;
            };
            let atoms = para::flatten(p);
            for (props, _) in para::slice(&atoms, s, e) {
                out.push((p.clone(), props));
            }
        }
        out
    }

    fn all_effective(&self, f: impl Fn(&RunProps) -> bool) -> bool {
        let atoms = self.atoms_in_selection();
        !atoms.is_empty()
            && atoms
                .iter()
                .all(|(p, r)| f(&effective_run(&self.doc, p, r)))
    }

    /// Apply a character format to the selection (or to the next typed text when collapsed).
    pub fn format_text(&mut self, f: &CharFormat) -> Result<Selection> {
        let sel = self.selection;
        let caret_props = self.caret_props();
        let on = |get: fn(&RunProps) -> bool| {
            if sel.is_collapsed() {
                get(&caret_props)
            } else {
                self.all_effective(get)
            }
        };
        let change: Box<dyn Fn(&mut RunProps)> = match f {
            CharFormat::Bold => {
                let v = !on(|r| r.bold.unwrap_or(false));
                Box::new(move |r| r.bold = Some(v))
            }
            CharFormat::Italic => {
                let v = !on(|r| r.italic.unwrap_or(false));
                Box::new(move |r| r.italic = Some(v))
            }
            CharFormat::Underline => {
                let v = !on(|r| r.underline.is_some_and(|u| u != Underline::None));
                Box::new(move |r| {
                    r.underline = Some(if v {
                        Underline::Single
                    } else {
                        Underline::None
                    });
                })
            }
            CharFormat::Strike => {
                let v = !on(|r| r.strike.unwrap_or(false));
                Box::new(move |r| r.strike = Some(v))
            }
            CharFormat::Superscript | CharFormat::Subscript => {
                let want = if *f == CharFormat::Superscript {
                    VertAlign::Superscript
                } else {
                    VertAlign::Subscript
                };
                let is = if sel.is_collapsed() {
                    caret_props.vert_align == Some(want)
                } else {
                    self.all_effective(|r| r.vert_align == Some(want))
                };
                Box::new(move |r| {
                    r.vert_align = Some(if is { VertAlign::Baseline } else { want });
                })
            }
            CharFormat::Font { name } => {
                let n = name.clone();
                Box::new(move |r| {
                    r.font = Some(n.clone());
                    r.font_east_asia = None;
                    r.font_cs = None;
                })
            }
            CharFormat::Size { half_points } => {
                let v = (*half_points).clamp(2, 3276);
                Box::new(move |r| r.size = Some(HalfPoints(v)))
            }
            CharFormat::Color { hex } => {
                let c = hex.as_deref().and_then(Color::from_hex);
                // A colour chosen replaces the theme colour too (§17.3.2.6).
                Box::new(move |r| {
                    r.color = c;
                    r.theme_color = None;
                })
            }
            CharFormat::Highlight { hex } => {
                let c = hex.as_deref().and_then(Color::from_hex);
                Box::new(move |r| r.highlight = c)
            }
            CharFormat::Language { lang } => {
                let tag = lang.clone().filter(|t| !t.trim().is_empty());
                Box::new(move |r| {
                    let mut l = r.lang.take().unwrap_or_default();
                    l.val.clone_from(&tag);
                    r.lang = (!l.is_empty()).then_some(l);
                })
            }
            CharFormat::NoProof { on } => {
                let on = *on;
                Box::new(move |r| r.no_proof = on.then_some(true))
            }
            CharFormat::Advanced { patch } => {
                let patch = patch.clone();
                Box::new(move |r| patch.apply(r))
            }
            CharFormat::Clear => Box::new(|r| {
                let link = r.link.take();
                *r = RunProps {
                    link,
                    ..RunProps::default()
                };
            }),
        };
        if sel.is_collapsed() {
            let mut p = self
                .pending
                .clone()
                .unwrap_or_else(|| self.caret_direct_props());
            change(&mut p);
            self.pending = Some(p);
            return Ok(sel);
        }
        let ranges = self.selected_ranges(sel);
        self.edit(|tx, sel| {
            for (id, s, e) in ranges {
                if s >= e {
                    continue;
                }
                let (path, p, mut atoms) = tx.atoms(id)?;
                if let Some(tr) = tx.track.as_mut() {
                    // §17.13.5.31 rPrChange keeps the properties before the change.
                    let i = para::split_at(&mut atoms, s);
                    let j = para::split_at(&mut atoms, e);
                    let mut info = None;
                    for at in &mut atoms[i..j] {
                        let before = at.0.clone();
                        change(&mut at.0);
                        if at.0 == before || tr.owns(&before) || before.change.is_some() {
                            continue;
                        }
                        let info = info.get_or_insert_with(|| tr.mark().info).clone();
                        at.0.change = Some(Box::new(bo_model::RunPropsChange {
                            info,
                            old: before.without_revisions(),
                        }));
                    }
                } else {
                    para::format(&mut atoms, s, e, &*change);
                }
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            }
            Ok(sel)
        })
    }

    fn caret_direct_props(&self) -> RunProps {
        let at = self.selection.ordered().0;
        let paths = body_paragraphs(&self.doc);
        paths
            .get(at.para)
            .and_then(|p| para_at(&self.doc, p))
            .map(|p| para::props_at(&para::flatten(p), at.offset))
            .unwrap_or_default()
    }

    fn caret_props(&self) -> RunProps {
        let at = self.selection.ordered().0;
        let paths = body_paragraphs(&self.doc);
        let Some(p) = paths.get(at.para).and_then(|p| para_at(&self.doc, p)) else {
            return self.doc.default_run.clone();
        };
        // With a selection, report the formatting of its first character.
        let probe = if self.selection.is_collapsed() {
            at.offset
        } else {
            at.offset + 1
        };
        let direct = self
            .pending
            .clone()
            .unwrap_or_else(|| para::props_at(&para::flatten(p), probe));
        effective_run(&self.doc, p, &direct)
    }

    /// Apply a paragraph format to every paragraph touched by the selection.
    pub fn format_paragraphs(&mut self, f: &ParaFormat) -> Result<Selection> {
        let (a, b) = self.selection.ordered();
        let ids: Vec<usize> = (a.para..=b.para).collect();
        let first = self.para_props(a.para);
        let list_kind = |bullet: bool, doc: &Document, props: &ParaProps| {
            props.num.is_some_and(|n| {
                doc.numbering
                    .level(n)
                    .is_some_and(|l| (l.format == bo_model::NumFormat::Bullet) == bullet)
            })
        };
        let f = f.clone();
        self.edit(move |tx, sel| {
            // Lists need a numbering definition first (§17.9 numbering part).
            let mut list_num: Option<Option<NumRef>> = None;
            if let ParaFormat::Bullets | ParaFormat::Numbering = f {
                let bullet = f == ParaFormat::Bullets;
                if list_kind(bullet, tx.doc, &first) {
                    list_num = Some(None);
                } else {
                    let old = tx.doc.numbering.clone();
                    let mut new = old.clone();
                    let id = new.add_simple_list(bullet);
                    tx.push(Op::Numbering {
                        old: Box::new(old),
                        new: Box::new(new),
                    })?;
                    list_num = Some(Some(NumRef {
                        num_id: id,
                        ilvl: 0,
                    }));
                }
            }
            for id in ids {
                let (path, p, _) = tx.atoms(id)?;
                let mut q = p.clone();
                let pr = &mut q.props;
                match &f {
                    ParaFormat::Align { align } => pr.align = Some(*align),
                    ParaFormat::IndentLeft { twips } => pr.indent_left = Some(Twips(*twips)),
                    ParaFormat::IndentFirstLine { twips } => {
                        pr.indent_first_line = Some(Twips(*twips));
                    }
                    ParaFormat::IncreaseIndent | ParaFormat::DecreaseIndent => {
                        let up = f == ParaFormat::IncreaseIndent;
                        if let Some(n) = pr.num.as_mut().filter(|n| n.num_id != 0) {
                            n.ilvl = if up {
                                (n.ilvl + 1).min(8)
                            } else {
                                n.ilvl.saturating_sub(1)
                            };
                        } else {
                            let cur = pr.indent_left.map_or(0, |t| t.0);
                            let next = if up { cur + 720 } else { (cur - 720).max(0) };
                            pr.indent_left = Some(Twips(next));
                        }
                    }
                    ParaFormat::LineSpacing { value } => {
                        pr.line_spacing = Some(LineSpacing::Auto((*value).clamp(60, 2400)));
                    }
                    ParaFormat::SpaceBefore { twips } => {
                        pr.space_before = Some(Twips((*twips).max(0)));
                    }
                    ParaFormat::SpaceAfter { twips } => {
                        pr.space_after = Some(Twips((*twips).max(0)));
                    }
                    ParaFormat::Bullets | ParaFormat::Numbering => {
                        pr.num = list_num.flatten();
                    }
                    ParaFormat::Style { id } => pr.style = Some(id.clone()),
                    ParaFormat::IndentRight { twips } => pr.indent_right = Some(Twips(*twips)),
                    ParaFormat::Indents {
                        left,
                        right,
                        first_line,
                    } => {
                        // §17.3.1.12 ind: the hanging indent may not move the first line
                        // left of the page margin.
                        let left = (*left).max(0);
                        pr.indent_left = Some(Twips(left));
                        pr.indent_right = Some(Twips(*right));
                        pr.indent_first_line = Some(Twips((*first_line).max(-left)));
                    }
                    ParaFormat::LineSpacingRule { rule, value } => {
                        pr.line_spacing = Some(match rule {
                            LineRule::Auto => LineSpacing::Auto((*value).clamp(60, 2400)),
                            LineRule::Exact => LineSpacing::Exact(Twips((*value).max(20) as i32)),
                            LineRule::AtLeast => {
                                LineSpacing::AtLeast(Twips((*value).max(20) as i32))
                            }
                        });
                    }
                    ParaFormat::Tabs { tabs } => {
                        let base = style_para_props(tx.doc, &p.props).tabs.unwrap_or_default();
                        pr.tabs = direct_tabs(&base, tabs);
                    }
                    ParaFormat::Shading { hex } => {
                        pr.shading = hex.as_deref().and_then(Color::from_hex);
                    }
                    ParaFormat::SuppressLineNumbers { on } => {
                        pr.suppress_line_numbers = on.then_some(true);
                    }
                    ParaFormat::SuppressAutoHyphens { on } => {
                        pr.suppress_auto_hyphens = on.then_some(true);
                    }
                    ParaFormat::Clear | ParaFormat::Reset => {
                        let unknown = std::mem::take(&mut pr.unknown);
                        let style = (f == ParaFormat::Reset).then(|| pr.style.take()).flatten();
                        *pr = ParaProps {
                            unknown,
                            // §17.3.1.27 pStyle: Reset keeps the paragraph's style.
                            style,
                            mark_ins: pr.mark_ins.take(),
                            mark_del: pr.mark_del.take(),
                            change: pr.change.take(),
                            ..ParaProps::default()
                        };
                    }
                }
                if q.props != p.props && p.props.change.is_none() {
                    // §17.13.5.29 pPrChange keeps the properties before the change.
                    if let Some(info) = tx.track.as_mut().map(|t| t.mark().info) {
                        q.props.change = Some(Box::new(bo_model::ParaPropsChange {
                            info,
                            old: ParaProps {
                                mark_ins: None,
                                mark_del: None,
                                change: None,
                                unknown: Vec::new(),
                                ..p.props.clone()
                            },
                        }));
                    }
                }
                tx.replace_para(&path, q)?;
            }
            Ok(sel)
        })
    }

    fn para_props(&self, id: usize) -> ParaProps {
        let paths = body_paragraphs(&self.doc);
        paths
            .get(id)
            .and_then(|p| para_at(&self.doc, p))
            .map(|p| p.props.clone())
            .unwrap_or_default()
    }

    /// Apply a style: paragraph styles to the touched paragraphs, character styles to the selection.
    pub fn apply_style(&mut self, id: &str) -> Result<Selection> {
        if self.doc.style(id).is_none() {
            let Some(builtin) = builtin_style(id) else {
                return Err(EditError::Unsupported(format!("unknown style {id}")));
            };
            // Add the built-in definition first (same undo step as applying it).
            let old = self.doc.styles.clone();
            let mut new = old.clone();
            if builtin.based_on.as_deref() == Some("Normal") && self.doc.style("Normal").is_none() {
                if let Some(n) = builtin_style("Normal") {
                    new.styles.push(n);
                }
            }
            new.styles.push(builtin);
            let sid = id.to_owned();
            let ids: Vec<usize> = {
                let (a, b) = self.selection.ordered();
                (a.para..=b.para).collect()
            };
            return self.edit(move |tx, sel| {
                tx.push(Op::Styles {
                    old: Box::new(old),
                    new: Box::new(new),
                })?;
                for pid in ids {
                    let (path, p, _) = tx.atoms(pid)?;
                    let mut q = p.clone();
                    q.props.style = Some(sid.clone());
                    tx.replace_para(&path, q)?;
                }
                Ok(sel)
            });
        }
        let Some(style) = self.doc.style(id).cloned() else {
            return Err(EditError::Unsupported(format!("unknown style {id}")));
        };
        match style.kind {
            StyleKind::Character => {
                let sid = id.to_owned();
                if self.selection.is_collapsed() {
                    let mut p = self
                        .pending
                        .clone()
                        .unwrap_or_else(|| self.caret_direct_props());
                    p.style = Some(sid);
                    self.pending = Some(p);
                    return Ok(self.selection);
                }
                let ranges = self.selected_ranges(self.selection);
                self.edit(|tx, sel| {
                    for (pid, s, e) in ranges {
                        let (path, p, mut atoms) = tx.atoms(pid)?;
                        para::format(&mut atoms, s, e, &|r| r.style = Some(sid.clone()));
                        tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                    }
                    Ok(sel)
                })
            }
            _ => self.format_paragraphs(&ParaFormat::Style { id: id.to_owned() }),
        }
    }

    /// Formatting at the selection start.
    pub fn format_state(&self) -> FormatState {
        let r = self.caret_props();
        let at = self.selection.ordered().0;
        let pp = self.para_props(at.para);
        let doc = &self.doc;
        let list = |bullet: bool| {
            pp.num.is_some_and(|n| {
                n.num_id != 0
                    && doc
                        .numbering
                        .level(n)
                        .is_some_and(|l| (l.format == bo_model::NumFormat::Bullet) == bullet)
            })
        };
        let style = pp.style.clone().or_else(|| {
            doc.styles
                .default_of(StyleKind::Paragraph)
                .map(|s| s.id.clone())
        });
        let align = style
            .as_ref()
            .map(|s| doc.styles.resolve_paragraph(s).0)
            .unwrap_or_default()
            .inherit(&doc.default_para)
            .align;
        let eff = effective_para(doc, &pp);
        FormatState {
            bold: r.bold.unwrap_or(false),
            italic: r.italic.unwrap_or(false),
            underline: r.underline.is_some_and(|u| u != Underline::None),
            strike: r.strike.unwrap_or(false),
            superscript: r.vert_align == Some(VertAlign::Superscript),
            subscript: r.vert_align == Some(VertAlign::Subscript),
            font: r.font.clone(),
            size_half_points: r.size.map(|s| s.0),
            color: r.color.map(Color::to_hex),
            highlight: r.highlight.map(Color::to_hex),
            lang: r.lang.as_ref().and_then(|l| l.val.clone()),
            no_proof: r.no_proof.unwrap_or(false),
            align: pp.align.or(align).unwrap_or_default(),
            style,
            bullets: list(true),
            numbering: list(false),
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
            indent_left: eff.indent_left.map_or(0, |t| t.0),
            indent_right: eff.indent_right.map_or(0, |t| t.0),
            first_line: eff.indent_first_line.map_or(0, |t| t.0),
            space_before: eff.space_before.map_or(0, |t| t.0),
            space_after: eff.space_after.map_or(0, |t| t.0),
            line_spacing: match eff.line_spacing.unwrap_or_default() {
                LineSpacing::Auto(v) => LineSpacingState {
                    rule: LineRule::Auto,
                    value: v,
                },
                LineSpacing::Exact(t) => LineSpacingState {
                    rule: LineRule::Exact,
                    value: t.0.max(0) as u32,
                },
                LineSpacing::AtLeast(t) => LineSpacingState {
                    rule: LineRule::AtLeast,
                    value: t.0.max(0) as u32,
                },
            },
            tabs: eff
                .tabs
                .iter()
                .flatten()
                .filter_map(|t| TabSpec::from_model(*t))
                .collect(),
            shading: eff.shading.map(Color::to_hex),
            section: self.section_state(),
            table: self.table_state(),
            picture: self.picture_state(),
            picture_tools: self.tools_target().is_some(),
            smart_art: self.smartart_target().is_some(),
            chart: self.chart_target().is_some(),
            ole: self.ole_at_selection().is_some(),
            advanced: charfx::CharAdvanced::of(&r),
            link: self.link_at_selection(),
        }
    }

    // -------------------------------------------------------------- insertion

    /// Insert a `rows × cols` table at the caret (§17.4.38 tbl).
    pub fn insert_table(&mut self, rows: usize, cols: usize) -> Result<Selection> {
        let rows = rows.clamp(1, 500);
        let cols = cols.clamp(1, 63);
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (path, _, atoms) = tx.atoms(at.para)?;
            let section = path.first().copied().unwrap_or(0);
            let width = tx.doc.sections.get(section).map_or(Twips(9355), |s| {
                Twips(s.page.width.0 - s.page.margin_left.0 - s.page.margin_right.0)
            });
            let mut table = Table::simple(rows, cols, width);
            if let Some(tr) = tx.track.as_mut() {
                // §17.13.5.19: a tracked new table is a set of inserted rows.
                let m = tr.mark();
                for r in &mut table.rows {
                    r.props.ins = Some(m.clone());
                }
            }
            let table = Block::Table(table);
            let len = para::len(&atoms);
            let (prefix, index) = split_path(&path);
            let insert_at = if at.offset == 0 {
                index
            } else {
                if at.offset < len {
                    tx.split(at)?;
                }
                index + 1
            };
            let mut blocks = vec![table];
            let container_len = blocks_at(tx.doc, &prefix).map_or(0, Vec::len);
            if insert_at >= container_len {
                // A table is always followed by a paragraph (§17.4.38 note on cell/body end).
                blocks.push(Block::Paragraph(Paragraph::default()));
            }
            tx.insert_blocks(prefix.clone(), insert_at, blocks)?;
            let mut cell = prefix;
            cell.extend([insert_at, 0, 0, 0]);
            let id = id_of(tx.doc, &cell).ok_or(EditError::BadPath)?;
            Ok(Selection::caret(Pos::new(id, 0)))
        })
    }

    /// Insert an inline picture. `width`/`height` in EMU; `ext` is the file extension (png, jpeg…).
    pub fn insert_image(
        &mut self,
        bytes: Vec<u8>,
        ext: &str,
        width: i64,
        height: i64,
    ) -> Result<Selection> {
        self.insert_picture(bytes, ext, None, width, height)
    }

    /// Insert an SVG picture (CORE-PIC-001): `png` is its fallback picture (the blip),
    /// `svg` the SVG version ([MS-ODRAWXML] `asvg:svgBlip`), both new media parts.
    pub fn insert_svg_image(
        &mut self,
        png: Vec<u8>,
        svg: Vec<u8>,
        width: i64,
        height: i64,
    ) -> Result<Selection> {
        self.insert_picture(png, "png", Some(svg), width, height)
    }

    fn insert_picture(
        &mut self,
        bytes: Vec<u8>,
        ext: &str,
        svg: Option<Vec<u8>>,
        width: i64,
        height: i64,
    ) -> Result<Selection> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        let mut n = self.part_number_start(self.doc.media.len() + 1);
        let name = loop {
            let cand = format!("/word/media/image{n}.{ext}");
            if !self.doc.media.contains_key(&cand) {
                break cand;
            }
            n += 1;
        };
        let svg = svg.map(|data| {
            let mut n = n + 1;
            loop {
                let cand = format!("/word/media/image{n}.svg");
                if !self.doc.media.contains_key(&cand) {
                    break (cand, Arc::new(data));
                }
                n += 1;
            }
        });
        let bytes = Arc::new(bytes);
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            tx.push(Op::Media {
                name: name.clone(),
                old: None,
                new: Some(bytes),
            })?;
            let format = match svg {
                Some((svg_name, data)) => {
                    tx.push(Op::Media {
                        name: svg_name.clone(),
                        old: None,
                        new: Some(data),
                    })?;
                    Some(Box::new(bo_model::picture::PictureFormat {
                        svg: Some(svg_name),
                        ..Default::default()
                    }))
                }
                None => None,
            };
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let img = Image {
                media: name,
                width: bo_model::Emu(width.max(1)),
                height: bo_model::Emu(height.max(1)),
                anchor: None,
                crop: None,
                rotation: 0,
                description: String::new(),
                shape: None,
                format,
                src: None,
                ole: None,
            };
            let caret = tx.insert_atoms(at, vec![(props, RunContent::Image(Box::new(img)))])?;
            Ok(Selection::caret(caret))
        })
    }

    /// Insert a page break (§17.3.3.1 br type="page") and continue in a new paragraph.
    pub fn insert_page_break(&mut self) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let after = tx.insert_atoms(at, vec![(props, RunContent::PageBreak)])?;
            Ok(Selection::caret(tx.split(after)?))
        })
    }

    /// Insert a section break of `kind` at the caret (§17.6.22 type). Only in the main text, not in tables.
    pub fn insert_section_break(&mut self, kind: SectionBreak) -> Result<Selection> {
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let paths = tx.paths();
            let path = path_of(tx.doc, &paths, at.para)?;
            if path.len() != 2 {
                return Err(EditError::Unsupported(
                    "section break inside a table".into(),
                ));
            }
            let next = tx.split(at)?;
            let si = path[0];
            let bi = path[1];
            let sec = tx.doc.sections.get(si).cloned().ok_or(EditError::BadPath)?;
            let mut first = sec.clone();
            first.blocks = sec.blocks[..=bi].to_vec();
            first.src = None;
            let mut second: Section = sec.clone();
            second.blocks = sec.blocks[bi + 1..].to_vec();
            second.break_type = kind;
            tx.push(Op::Sections {
                index: si,
                remove: vec![sec],
                insert: vec![first, second],
            })?;
            Ok(Selection::caret(next))
        })
    }

    // -------------------------------------------------------------- find / replace

    /// Find all matches in the main text.
    pub fn find(&self, query: &str, opts: &FindOptions) -> Result<Vec<Match>> {
        find::find(&self.doc, query, opts)
    }

    /// Replace one match (it must still be valid); returns the selection after the
    /// replacement. Special characters of `with` are expanded (`^p` splits the
    /// paragraph, `^&` is the matched text).
    pub fn replace(&mut self, m: Match, with: &str) -> Result<Selection> {
        self.replace_match(m, None, with)
    }

    /// Replace one match of `query` (groups of `with` are expanded against the match
    /// found again in context: `$1` in regex mode, `\1` with wildcards).
    pub fn replace_match(
        &mut self,
        m: Match,
        query: Option<(&str, &FindOptions)>,
        with: &str,
    ) -> Result<Selection> {
        let replacement = find::replacement_for(&self.doc, &m, query, with)?;
        self.replace_matches(vec![(m, replacement)])
    }

    /// Find matches of `query` that have the format `fmt` (an empty query finds every
    /// stretch of text with that format).
    pub fn find_formatted(
        &self,
        query: &str,
        opts: &FindOptions,
        fmt: &find::FindFormat,
    ) -> Result<Vec<Match>> {
        find::find_formatted(&self.doc, query, opts, fmt)
    }

    /// Replace every match of `query` with the format `fmt` (one undo step).
    pub fn replace_all_formatted(
        &mut self,
        query: &str,
        opts: &FindOptions,
        fmt: &find::FindFormat,
        with: &str,
    ) -> Result<usize> {
        let items = if query.is_empty() {
            find::find_formatted(&self.doc, query, opts, fmt)?
                .into_iter()
                .map(|m| (m, find::expand_replacement(with, None, false)))
                .collect()
        } else {
            find::replacements(&self.doc, query, opts, Some(with), Some(fmt))?
        };
        let n = items.len();
        if n > 0 {
            self.replace_matches(items)?;
        }
        Ok(n)
    }

    /// Replace every match of `query`; returns the number of replacements (one undo step).
    pub fn replace_all(&mut self, query: &str, opts: &FindOptions, with: &str) -> Result<usize> {
        let items = find::replacements(&self.doc, query, opts, Some(with), None)?;
        let n = items.len();
        if n > 0 {
            self.replace_matches(items)?;
        }
        Ok(n)
    }

    /// Replace matches by their replacement texts (`\n` = paragraph mark), last first,
    /// in one transaction.
    fn replace_matches(&mut self, mut items: Vec<(Match, String)>) -> Result<Selection> {
        items.sort_by_key(|(m, _)| std::cmp::Reverse((m.para, m.start)));
        self.edit(|tx, sel| {
            let mut last = sel;
            for (m, replacement) in items {
                let (path, p, mut atoms) = tx.atoms(m.para)?;
                let text = bo_model::index::para_index_text(&p);
                if m.start > text.len() || !text.is_char_boundary(m.start) {
                    return Err(EditError::Conflict);
                }
                if m.is_multi_para() || replacement.contains('\n') {
                    last = tx.replace_span(&m, &text, &atoms, &replacement)?;
                    continue;
                }
                if m.end > text.len() || m.end < m.start || !text.is_char_boundary(m.end) {
                    return Err(EditError::Conflict);
                }
                let props = para::props_at(&atoms, (m.start + 1).min(m.end.max(m.start)));
                let at = if let Some(tr) = tx.track.as_mut() {
                    let mark = tr.mark();
                    let removed = revisions::mark_deleted(&mut atoms, m.start, m.end, tr, &mark);
                    m.end - removed
                } else {
                    para::delete(&mut atoms, m.start, m.end);
                    m.start
                };
                let new = tx.mark_new(&atoms, at, para::text_atoms(&replacement, &props));
                para::insert(&mut atoms, at, new);
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                let start = if tx.track.is_some() { at } else { m.start };
                last = Selection {
                    anchor: Pos::new(m.para, start),
                    focus: Pos::new(
                        m.para,
                        start + para::len(&para::text_atoms(&replacement, &props)),
                    ),
                };
            }
            Ok(last)
        })
    }

    // -------------------------------------------------------------- clipboard

    /// Paragraphs of the selection (sliced at its ends).
    pub fn selected_blocks(&self) -> Vec<Block> {
        let paths = body_paragraphs(&self.doc);
        let mut out = Vec::new();
        for (id, s, e) in self.selected_ranges(self.selection) {
            let Some(p) = paths.get(id).and_then(|path| para_at(&self.doc, path)) else {
                continue;
            };
            let atoms: Vec<Atom> = para::slice(&para::flatten(p), s, e)
                .into_iter()
                .filter(|a| {
                    !matches!(
                        a.1,
                        RunContent::Markup(_)
                            | RunContent::CommentStart(_)
                            | RunContent::CommentEnd(_)
                            | RunContent::CommentRef(_)
                    )
                })
                .collect();
            out.push(Block::Paragraph(Paragraph {
                props: ParaProps {
                    unknown: Vec::new(),
                    ..p.props.clone()
                },
                runs: para::rebuild(atoms),
                src: None,
            }));
        }
        out
    }

    /// Copy the selection.
    pub fn copy(&self) -> Clip {
        let blocks = self.selected_blocks();
        let text = blocks
            .iter()
            .filter_map(Block::as_paragraph)
            .map(Paragraph::plain_text)
            .collect::<Vec<_>>()
            .join("\n")
            .replace('\u{FFFC}', "");
        let mut refs = Vec::new();
        section::note_refs(&blocks, &mut refs);
        let internal = if refs.is_empty() {
            serde_json::to_string(&blocks).unwrap_or_default()
        } else {
            // Blocks with the notes they reference (pasted as new notes).
            let pick = |endnote: bool| -> std::collections::BTreeMap<i32, Vec<Block>> {
                let map = if endnote {
                    &self.doc.endnotes
                } else {
                    &self.doc.footnotes
                };
                refs.iter()
                    .filter(|k| k.0 == endnote)
                    .filter_map(|k| map.get(&k.1).map(|b| (k.1, b.clone())))
                    .collect()
            };
            serde_json::to_string(&ClipBlocks {
                blocks: blocks.clone(),
                footnotes: pick(false),
                endnotes: pick(true),
            })
            .unwrap_or_default()
        };
        Clip {
            html: html::to_html(&self.doc, &blocks),
            internal,
            text,
        }
    }

    /// Cut = copy + delete.
    pub fn cut(&mut self) -> Result<Clip> {
        let c = self.copy();
        if !self.selection.is_collapsed() {
            let before = self.cut_start();
            self.delete_backward()?;
            self.cut_end(before, &c.internal);
        }
        Ok(c)
    }

    /// Paste blocks produced by [`Session::copy`] (internal format).
    pub fn paste_internal(&mut self, json: &str) -> Result<Selection> {
        let clip: ClipBlocks = match serde_json::from_str::<Vec<Block>>(json) {
            Ok(blocks) => ClipBlocks {
                blocks,
                ..ClipBlocks::default()
            },
            Err(_) => {
                serde_json::from_str(json).map_err(|e| EditError::Clipboard(e.to_string()))?
            }
        };
        if clip.footnotes.is_empty() && clip.endnotes.is_empty() {
            return self.paste_blocks(clip.blocks);
        }
        // Copied notes become new notes with fresh ids, then the blocks are pasted;
        // both are one undo step.
        let mut blocks = clip.blocks;
        let mut maps = [self.doc.footnotes.clone(), self.doc.endnotes.clone()];
        for (endnote, src) in [(false, &clip.footnotes), (true, &clip.endnotes)] {
            let map = &mut maps[usize::from(endnote)];
            for (old_id, content) in src {
                let id = seeded_id(
                    map.keys().copied().max().unwrap_or(0).max(1) + 1,
                    self.id_seed,
                );
                map.insert(id, content.clone());
                section::renumber_note_ref(&mut blocks, endnote, *old_id, id);
            }
        }
        let [foot, end] = maps;
        let before = self.undo.len();
        self.edit(|tx, sel| {
            for (endnote, new) in [(false, foot), (true, end)] {
                let old = if endnote {
                    tx.doc.endnotes.clone()
                } else {
                    tx.doc.footnotes.clone()
                };
                if old != new {
                    tx.push(Op::Notes { endnote, old, new })?;
                }
            }
            Ok(sel)
        })?;
        let res = self.paste_blocks(blocks);
        if res.is_err() {
            if self.undo.len() > before {
                self.undo();
                self.redo.clear();
            }
            return res;
        }
        if self.undo.len() >= before + 2 {
            if let (Some(last), Some(prev)) = (self.undo.pop(), self.undo.last_mut()) {
                prev.ops.extend(last.ops);
                prev.after = last.after;
            }
        }
        res
    }

    /// Paste HTML (from browsers and other applications).
    pub fn paste_html(&mut self, html: &str) -> Result<Selection> {
        self.paste_blocks(html::from_html(html))
    }

    /// Paste plain text (one paragraph per line).
    pub fn paste_text(&mut self, text: &str) -> Result<Selection> {
        let blocks = text
            .replace("\r\n", "\n")
            .split('\n')
            .map(|l| {
                Block::Paragraph(Paragraph::new(
                    ParaProps::default(),
                    para::rebuild(para::text_atoms(l, &RunProps::default())),
                ))
            })
            .collect();
        self.paste_plain(blocks)
    }

    fn paste_plain(&mut self, blocks: Vec<Block>) -> Result<Selection> {
        // Plain text takes the formatting at the caret.
        let props = self.caret_direct_props();
        let blocks = blocks
            .into_iter()
            .map(|b| match b {
                Block::Paragraph(p) => {
                    let atoms = para::flatten(&p)
                        .into_iter()
                        .map(|(_, c)| (props.clone(), c))
                        .collect();
                    Block::Paragraph(Paragraph::new(ParaProps::default(), para::rebuild(atoms)))
                }
                other => other,
            })
            .collect();
        self.paste_blocks_inner(blocks, true)
    }

    fn paste_blocks(&mut self, blocks: Vec<Block>) -> Result<Selection> {
        self.paste_blocks_inner(blocks, false)
    }

    pub(crate) fn paste_blocks_inner(
        &mut self,
        blocks: Vec<Block>,
        keep_para_props: bool,
    ) -> Result<Selection> {
        let paras: Vec<Paragraph> = blocks
            .into_iter()
            .filter_map(|b| match b {
                Block::Paragraph(p) => Some(p),
                _ => None,
            })
            .collect();
        if paras.is_empty() {
            return Ok(self.selection);
        }
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            if paras.len() == 1 {
                let atoms = para::flatten(&paras[0]);
                return Ok(Selection::caret(tx.insert_atoms(at, atoms)?));
            }
            let (path, p, mut atoms) = tx.atoms(at.para)?;
            let i = para::split_at(&mut atoms, at.offset);
            let right = atoms.split_off(i);
            let n = paras.len();
            let mut out = Vec::with_capacity(n);
            let mut first = atoms;
            let pasted = tx.mark_new(&first, at.offset, para::flatten(&paras[0]));
            first.extend(pasted);
            let mut first = para::with_atoms(&p, first);
            tx.new_mark(&mut first.props);
            out.push(Block::Paragraph(first));
            for q in &paras[1..n - 1] {
                let mut q = q.clone();
                if keep_para_props {
                    q.props = ParaProps {
                        unknown: Vec::new(),
                        num: q.props.num.or(p.props.num),
                        ..p.props.clone()
                    };
                }
                q.runs = para::rebuild(tx.mark_new(&[], 0, para::flatten(&q)));
                tx.new_mark(&mut q.props);
                out.push(Block::Paragraph(q));
            }
            let last_src = &paras[n - 1];
            let mut last = tx.mark_new(&[], 0, para::flatten(last_src));
            let caret = para::len(&last);
            last.extend(right);
            let last_props = if keep_para_props {
                ParaProps {
                    unknown: Vec::new(),
                    num: last_src.props.num.or(p.props.num),
                    ..p.props.clone()
                }
            } else {
                last_src.props.clone()
            };
            out.push(Block::Paragraph(Paragraph {
                props: last_props,
                runs: para::rebuild(last),
                src: None,
            }));
            let (prefix, index) = split_path(&path);
            tx.push(Op::Blocks {
                path: prefix,
                index,
                remove: vec![Block::Paragraph(p)],
                insert: out,
            })?;
            Ok(Selection::caret(Pos::new(at.para + n - 1, caret)))
        })
    }

    // -------------------------------------------------------------- word / paragraph ranges

    /// Selection of the word around `p` (double click).
    pub fn word_at(&self, p: Pos) -> Selection {
        let p = self.clamp(p);
        let text = self.para_text(p.para).unwrap_or_default();
        for (i, w) in text.split_word_bound_indices() {
            if p.offset >= i && p.offset < i + w.len()
                || p.offset == text.len() && i + w.len() == text.len()
            {
                return Selection {
                    anchor: Pos::new(p.para, i),
                    focus: Pos::new(p.para, i + w.len()),
                };
            }
        }
        Selection::caret(p)
    }

    /// Change the case of the selected text (the word at the caret when nothing is selected),
    /// keeping its formatting; the selection covers the text afterwards (one undo step).
    /// `Cycle` goes UPPERCASE → lowercase → Capitalize Each Word → UPPERCASE (Shift+F3).
    pub fn change_case(&mut self, mode: CaseMode) -> Result<Selection> {
        if self.selection.is_collapsed() {
            let w = self.word_at(self.selection.focus);
            self.set_selection(w);
        }
        let (a, b) = self.selection.ordered();
        let mut text = String::new();
        for id in a.para..=b.para {
            let t = self.para_text(id).unwrap_or_default();
            let s = if id == a.para {
                a.offset.min(t.len())
            } else {
                0
            };
            let e = if id == b.para {
                b.offset.min(t.len())
            } else {
                t.len()
            };
            text.push_str(t.get(s..e).unwrap_or_default());
            text.push('\n');
        }
        let mode = match mode {
            CaseMode::Cycle => {
                let has_upper = text.chars().any(char::is_uppercase);
                let has_lower = text.chars().any(char::is_lowercase);
                if has_upper && !has_lower {
                    CaseMode::Lower
                } else if !has_upper {
                    CaseMode::Title
                } else {
                    CaseMode::Upper
                }
            }
            m => m,
        };
        let anchor_first = self.selection.anchor <= self.selection.focus;
        self.edit(move |tx, _| {
            let mut end = b;
            // Word / sentence starts carry over paragraph by paragraph.
            let (mut word_start, mut sentence_start) = (true, true);
            for id in a.para..=b.para {
                let paths = tx.paths();
                let path = path_of(tx.doc, &paths, id)?;
                let p = para_clone(tx.doc, &path)?;
                let mut atoms = para::flatten(&p);
                let len = para::len(&atoms);
                let s = if id == a.para { a.offset.min(len) } else { 0 };
                let e = if id == b.para { b.offset.min(len) } else { len };
                let i = para::split_at(&mut atoms, s);
                let j = para::split_at(&mut atoms, e);
                let mut grew = 0isize;
                for (_, c) in &mut atoms[i..j] {
                    if let RunContent::Text(t) = c {
                        let new: String = t
                            .chars()
                            .map(|ch| {
                                let out = case_char(ch, mode, word_start, sentence_start);
                                word_start = !ch.is_alphanumeric();
                                if matches!(ch, '.' | '!' | '?') {
                                    sentence_start = true;
                                } else if ch.is_alphanumeric() {
                                    sentence_start = false;
                                }
                                out
                            })
                            .collect();
                        grew += new.len() as isize - t.len() as isize;
                        *t = new;
                    } else {
                        word_start = true;
                    }
                }
                word_start = true;
                if id == b.para {
                    end = Pos::new(id, e.saturating_add_signed(grew));
                }
                tx.replace_para(&path, para::with_atoms(&p, atoms))?;
            }
            Ok(if anchor_first {
                Selection {
                    anchor: a,
                    focus: end,
                }
            } else {
                Selection {
                    anchor: end,
                    focus: a,
                }
            })
        })
    }

    /// Selection of a double-clicked word: the word and the spaces after it, the way word
    /// processors select a word with the mouse (deleting it leaves no double space).
    pub fn word_select_at(&self, p: Pos) -> Selection {
        let w = self.word_at(p);
        let text = self.para_text(w.focus.para).unwrap_or_default();
        let (a, b) = w.ordered();
        let word = text.get(a.offset..b.offset).unwrap_or_default();
        if word.trim().is_empty() {
            return w;
        }
        let rest = text.get(b.offset..).unwrap_or_default();
        let spaces = rest.len() - rest.trim_start_matches(' ').len();
        Selection {
            anchor: a,
            focus: Pos::new(b.para, b.offset + spaces),
        }
    }

    /// Selection of the sentence around `p` with the spaces after it (Ctrl+click).
    pub fn sentence_at(&self, p: Pos) -> Selection {
        let p = self.clamp(p);
        let text = self.para_text(p.para).unwrap_or_default();
        for (i, s) in text.split_sentence_bound_indices() {
            if p.offset >= i && p.offset < i + s.len()
                || p.offset == text.len() && i + s.len() == text.len()
            {
                return Selection {
                    anchor: Pos::new(p.para, i),
                    focus: Pos::new(p.para, i + s.len()),
                };
            }
        }
        Selection::caret(p)
    }

    /// Start of the paragraph (backward: of this one when inside it, else of the previous one)
    /// or of the next paragraph (forward; the end of the last one) — Ctrl+Up / Ctrl+Down.
    pub fn paragraph_boundary(&self, p: Pos, forward: bool) -> Pos {
        let p = self.clamp(p);
        if forward {
            if p.para + 1 < self.paragraph_count() {
                Pos::new(p.para + 1, 0)
            } else {
                Pos::new(p.para, self.para_text(p.para).map_or(0, |t| t.len()))
            }
        } else if p.offset > 0 || p.para == 0 {
            Pos::new(p.para, 0)
        } else {
            Pos::new(p.para - 1, 0)
        }
    }

    /// Ctrl+Backspace / Ctrl+Delete: delete the selection, else from the caret to the
    /// previous / next word boundary (one undo step).
    pub fn delete_word(&mut self, forward: bool) -> Result<Selection> {
        if self.selection.is_collapsed() {
            let at = self.selection.focus;
            let to = self.word_boundary(at, forward);
            // At a paragraph edge the word step crosses the paragraph mark: delete just it.
            if to.para == at.para && to != at {
                self.set_selection(Selection {
                    anchor: at,
                    focus: to,
                });
            }
        }
        if forward {
            self.delete_forward()
        } else {
            self.delete_backward()
        }
    }

    /// The Tab key outside a table: at the start of a list item (or over several paragraphs
    /// of a list) it demotes / promotes the items one level; at the start of a paragraph that
    /// has text it indents the first line by half an inch, then the whole paragraph (the
    /// "set indents with Tab and Backspace" behaviour of word processors); otherwise it types
    /// a tab character (Shift+Tab then does nothing).
    pub fn tab_key(&mut self, back: bool) -> Result<Selection> {
        let (a, b) = self.selection.ordered();
        let props = self.para_props(a.para);
        let in_list = props.num.is_some_and(|n| n.num_id != 0);
        if in_list && (a.offset == 0 || a.para != b.para) {
            return self.format_paragraphs(&if back {
                ParaFormat::DecreaseIndent
            } else {
                ParaFormat::IncreaseIndent
            });
        }
        let at_start = self.selection.is_collapsed() && a.offset == 0;
        let has_text = self.para_text(a.para).is_some_and(|t| !t.is_empty());
        if at_start && has_text && !in_list {
            // ECMA-376 Part 1, §17.3.1.12 ind: firstLine / left, in twentieths of a point.
            let first = props.indent_first_line.map_or(0, |t| t.0);
            let left = props.indent_left.map_or(0, |t| t.0);
            return match (back, first > 0) {
                (false, false) => {
                    self.format_paragraphs(&ParaFormat::IndentFirstLine { twips: 720 })
                }
                (false, true) => {
                    self.format_paragraphs(&ParaFormat::IndentLeft { twips: left + 720 })
                }
                (true, true) => self.format_paragraphs(&ParaFormat::IndentFirstLine { twips: 0 }),
                (true, false) if left > 0 => self.format_paragraphs(&ParaFormat::IndentLeft {
                    twips: (left - 720).max(0),
                }),
                (true, false) => Ok(self.selection),
            };
        }
        if back {
            return Ok(self.selection);
        }
        self.insert_text("\t")
    }

    /// Selection of the whole paragraph (triple click).
    pub fn paragraph_at(&self, p: Pos) -> Selection {
        let p = self.clamp(p);
        let len = self.para_text(p.para).map_or(0, |t| t.len());
        Selection {
            anchor: Pos::new(p.para, 0),
            focus: Pos::new(p.para, len),
        }
    }

    /// Selection of the whole main text (Ctrl+A).
    pub fn select_all(&mut self) -> Selection {
        let n = self.paragraph_count();
        let last = n.saturating_sub(1);
        let len = self.para_text(last).map_or(0, |t| t.len());
        let s = Selection {
            anchor: Pos::new(0, 0),
            focus: Pos::new(last, len),
        };
        self.set_selection(s);
        s
    }

    /// Previous/next word boundary from `p` (Ctrl+arrows).
    pub fn word_boundary(&self, p: Pos, forward: bool) -> Pos {
        let p = self.clamp(p);
        let text = self.para_text(p.para).unwrap_or_default();
        let bounds: Vec<usize> = text
            .split_word_bound_indices()
            .filter(|(_, w)| !w.trim().is_empty())
            .map(|(i, _)| i)
            .collect();
        if forward {
            let next = bounds.iter().copied().find(|&i| i > p.offset);
            match next {
                Some(i) => Pos::new(p.para, i),
                None if p.offset < text.len() => Pos::new(p.para, text.len()),
                None if p.para + 1 < self.paragraph_count() => Pos::new(p.para + 1, 0),
                None => p,
            }
        } else {
            let prev = bounds.iter().copied().rev().find(|&i| i < p.offset);
            match prev {
                Some(i) => Pos::new(p.para, i),
                None if p.offset > 0 => Pos::new(p.para, 0),
                None if p.para > 0 => {
                    let l = self.para_text(p.para - 1).map_or(0, |t| t.len());
                    Pos::new(p.para - 1, l)
                }
                None => p,
            }
        }
    }

    /// Previous/next grapheme position (arrow keys).
    pub fn char_step(&self, p: Pos, forward: bool) -> Pos {
        let p = self.clamp(p);
        let text = self.para_text(p.para).unwrap_or_default();
        if forward {
            if p.offset < text.len() {
                Pos::new(p.para, next_boundary(&text, p.offset))
            } else if p.para + 1 < self.paragraph_count() {
                Pos::new(p.para + 1, 0)
            } else {
                p
            }
        } else if p.offset > 0 {
            Pos::new(p.para, prev_boundary(&text, p.offset))
        } else if p.para > 0 {
            let l = self.para_text(p.para - 1).map_or(0, |t| t.len());
            Pos::new(p.para - 1, l)
        } else {
            p
        }
    }
}

#[cfg(test)]
mod feature_tests;
#[cfg(test)]
mod forms_tests;
#[cfg(test)]
mod move_tests;
#[cfg(test)]
mod paste_tests;
#[cfg(test)]
mod table_tools_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod track_tests;

#[cfg(test)]
mod id_seed_tests {
    use super::seeded_id;

    #[test]
    fn seeded_ids() {
        assert_eq!(seeded_id(7, None), 7);
        assert_eq!(seeded_id(7, Some(9)), 9);
        assert_eq!(seeded_id(9, Some(9)), 9);
        assert_eq!(seeded_id(10, Some(9)), 65_536 + 9);
        // Two replicas from the same maximum never collide.
        assert_ne!(seeded_id(5, Some(100)), seeded_id(5, Some(200)));
        // Near the end of i32: max + 1 rather than overflowing.
        assert_eq!(seeded_id(i32::MAX - 1, Some(3)), i32::MAX - 1);
    }
}
