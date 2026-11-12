//! Style management (DOC-STYLE-001): modify / new / delete styles, select every
//! instance, update a style to match the selection, the Style Inspector (style and
//! direct formatting at the caret, each level clearable), Reveal Formatting (the
//! effective formatting with where each value comes from) and the Organizer
//! (copy / delete / rename styles between two documents).
//!
//! ECMA-376 Part 1, §17.7 Styles: §17.7.2 style hierarchy (document defaults →
//! paragraph style chain → character style chain → direct formatting), §17.7.4.17
//! style, §17.7.4.3 basedOn, §17.7.4.10 next, §17.7.4.6 link, §17.7.4.14 qFormat,
//! §17.7.4.2 autoRedefine. Every session edit is one undo step (`Op::Styles` plus
//! the block / numbering ops it needs).

use crate::{body_paragraphs, para, para_at, EditError, Op, Result, Selection, Session};
use bo_model::{
    Alignment, Block, Border, Borders, Color, Document, HalfPoints, LineSpacing, NumRef, ParaProps,
    Paragraph, RunProps, Style, StyleKind, Twips, Underline,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------- spec

/// Style type as hosts name it (§17.18.83 `ST_StyleType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StyleType {
    /// Paragraph style.
    #[default]
    Paragraph,
    /// Character style.
    Character,
    /// Table style.
    Table,
    /// Numbering style.
    Numbering,
}

impl StyleType {
    fn of(k: StyleKind) -> StyleType {
        match k {
            StyleKind::Paragraph => StyleType::Paragraph,
            StyleKind::Character => StyleType::Character,
            StyleKind::Table => StyleType::Table,
            StyleKind::Numbering => StyleType::Numbering,
        }
    }
}

/// A box border of the paragraph (§17.3.1.24 pBdr, all four sides alike).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorderSpec {
    /// `ST_Border` line style (`single`, `double`, `dotted`, …).
    pub style: String,
    /// Width in eighths of a point (`w:sz`).
    pub size: u32,
    /// `RRGGBB`; `None`: automatic.
    #[serde(default)]
    pub color: Option<String>,
}

/// What the Modify / New Style dialog edits. Formatting fields are the style's
/// *effective* values (style chain over the document defaults); `None` = not set.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StyleSpec {
    /// Display name (§17.7.4.9 name).
    pub name: String,
    /// Style type (only used by new styles).
    #[serde(rename = "type")]
    pub kind: StyleType,
    /// §17.7.4.3 basedOn.
    pub based_on: Option<String>,
    /// §17.7.4.10 next (paragraph styles).
    pub next: Option<String>,
    /// "Add to the style gallery" (§17.7.4.14 qFormat).
    pub add_to_gallery: bool,
    /// "Automatically update" (§17.7.4.2 autoRedefine).
    pub auto_update: bool,
    /// Latin font (§17.3.2.26 rFonts).
    pub font: Option<String>,
    /// Size in half-points (§17.3.2.38 sz).
    pub size: Option<u32>,
    /// §17.3.2.1 b.
    pub bold: Option<bool>,
    /// §17.3.2.16 i.
    pub italic: Option<bool>,
    /// Single underline (§17.3.2.40 u).
    pub underline: Option<bool>,
    /// Text colour `RRGGBB` (§17.3.2.6 color).
    pub color: Option<String>,
    /// §17.3.1.13 jc.
    pub align: Option<Alignment>,
    /// Space before, twips (§17.3.1.33 spacing).
    pub space_before: Option<i32>,
    /// Space after, twips.
    pub space_after: Option<i32>,
    /// Line spacing in 240ths of a line (`lineRule="auto"`).
    pub line_spacing: Option<u32>,
    /// Left indent, twips (§17.3.1.12 ind).
    pub indent_left: Option<i32>,
    /// Right indent, twips.
    pub indent_right: Option<i32>,
    /// First-line indent, twips (negative: hanging).
    pub indent_first_line: Option<i32>,
    /// Paragraph box border (§17.3.1.24 pBdr).
    pub border: Option<BorderSpec>,
    /// Paragraph shading fill `RRGGBB` (§17.3.1.31 shd).
    pub shading: Option<String>,
    /// Numbering instance (§17.3.1.19 numPr numId; 0 removes inherited numbering).
    pub num_id: Option<i32>,
    /// Numbering level (§17.3.1.19 numPr ilvl).
    pub num_level: u8,
}

fn twips(v: Option<Twips>) -> Option<i32> {
    v.map(|t| t.0)
}

fn border_spec(b: Option<&Borders>) -> Option<BorderSpec> {
    let b = b?;
    let edge = [&b.top, &b.left, &b.bottom, &b.right]
        .into_iter()
        .flatten()
        .find(|e| e.is_visible())?;
    Some(BorderSpec {
        style: edge.style.clone(),
        size: edge.size,
        color: edge.color.map(Color::to_hex),
    })
}

/// The dialog values of style `id` (`None`: no such style).
pub fn style_spec(doc: &Document, id: &str) -> Option<StyleSpec> {
    let s = doc.style(id)?;
    let (pp, rp) = match s.kind {
        StyleKind::Character => (ParaProps::default(), doc.styles.resolve_character(id)),
        _ => doc.styles.resolve_paragraph(id),
    };
    let pp = pp.inherit(&doc.default_para);
    let rp = rp.inherit(&doc.default_run);
    let line = match pp.line_spacing {
        Some(LineSpacing::Auto(v)) => Some(v),
        _ => None,
    };
    Some(StyleSpec {
        name: display_name(s),
        kind: StyleType::of(s.kind),
        based_on: s.based_on.clone(),
        next: s.next.clone(),
        add_to_gallery: s.quick,
        auto_update: s.auto_redefine,
        font: rp
            .theme_ascii
            .as_deref()
            .and_then(|t| doc.theme_fonts.resolve(t))
            .map(str::to_owned)
            .or(rp.font.clone()),
        size: rp.size.map(|h| h.0),
        bold: rp.bold,
        italic: rp.italic,
        underline: rp.underline.map(|u| u != Underline::None),
        color: rp.color.map(Color::to_hex),
        align: pp.align,
        space_before: twips(pp.space_before),
        space_after: twips(pp.space_after),
        line_spacing: line,
        indent_left: twips(pp.indent_left),
        indent_right: twips(pp.indent_right),
        indent_first_line: twips(pp.indent_first_line),
        border: border_spec(pp.borders.as_ref()),
        shading: pp.shading.map(Color::to_hex),
        num_id: pp.num.map(|n| n.num_id),
        num_level: pp.num.map_or(0, |n| n.ilvl),
    })
}

fn display_name(s: &Style) -> String {
    if s.name.is_empty() {
        s.id.clone()
    } else {
        s.name.clone()
    }
}

/// Write the fields of `spec` that differ from `cur` into the style's own
/// properties (unchanged fields keep whatever the style had, including details the
/// dialog cannot show).
fn apply_spec(st: &mut Style, spec: &StyleSpec, cur: &StyleSpec) {
    macro_rules! changed {
        ($f:ident) => {
            spec.$f != cur.$f
        };
    }
    if changed!(name) && !spec.name.trim().is_empty() {
        st.name = spec.name.trim().to_owned();
    }
    if changed!(based_on) {
        st.based_on = spec.based_on.clone().filter(|b| *b != st.id);
    }
    if changed!(next) {
        st.next.clone_from(&spec.next);
    }
    st.quick = spec.add_to_gallery;
    st.auto_redefine = spec.auto_update;
    let r = &mut st.rpr;
    if changed!(font) {
        r.font.clone_from(&spec.font);
        r.font_h_ansi = None;
        r.font_east_asia = None;
        r.font_cs = None;
        r.theme_ascii = None;
        r.theme_h_ansi = None;
        r.theme_east_asia = None;
        r.theme_cs = None;
    }
    if changed!(size) {
        r.size = spec.size.map(|v| HalfPoints(v.clamp(2, 3276)));
    }
    if changed!(bold) {
        r.bold = spec.bold;
    }
    if changed!(italic) {
        r.italic = spec.italic;
    }
    if changed!(underline) {
        r.underline = spec.underline.map(|u| {
            if u {
                Underline::Single
            } else {
                Underline::None
            }
        });
    }
    if changed!(color) {
        r.color = spec.color.as_deref().and_then(Color::from_hex);
        r.theme_color = None;
    }
    if st.kind == StyleKind::Character {
        return;
    }
    let p = &mut st.ppr;
    if changed!(align) {
        p.align = spec.align;
    }
    if changed!(space_before) {
        p.space_before = spec.space_before.map(Twips);
    }
    if changed!(space_after) {
        p.space_after = spec.space_after.map(Twips);
    }
    if changed!(line_spacing) {
        p.line_spacing = spec.line_spacing.map(LineSpacing::Auto);
    }
    if changed!(indent_left) {
        p.indent_left = spec.indent_left.map(Twips);
    }
    if changed!(indent_right) {
        p.indent_right = spec.indent_right.map(Twips);
    }
    if changed!(indent_first_line) {
        p.indent_first_line = spec.indent_first_line.map(Twips);
    }
    if changed!(border) {
        // `nil` edges switch off inherited borders (§17.18.2).
        let edge = spec.border.as_ref().map_or_else(
            || Border {
                style: "nil".into(),
                size: 0,
                color: None,
                space: 0,
            },
            |b| Border {
                style: b.style.clone(),
                size: b.size,
                color: b.color.as_deref().and_then(Color::from_hex),
                space: 1,
            },
        );
        p.borders = Some(Borders {
            top: Some(edge.clone()),
            left: Some(edge.clone()),
            bottom: Some(edge.clone()),
            right: Some(edge),
            ..Borders::default()
        });
    }
    if changed!(shading) {
        p.shading = spec.shading.as_deref().and_then(Color::from_hex);
    }
    if changed!(num_id) || changed!(num_level) {
        p.num = spec.num_id.map(|num_id| NumRef {
            num_id,
            ilvl: spec.num_level.min(8),
        });
    }
}

/// A style id for `name` unique in `doc` (letters and digits of the name, §17.7.4.17
/// styleId), e.g. `My Heading` → `MyHeading`, `MyHeading1`…
pub fn new_style_id(doc: &Document, name: &str) -> String {
    let base: String = name.chars().filter(|c| c.is_alphanumeric()).collect();
    let base = if base.is_empty() {
        "Style".to_owned()
    } else {
        base
    };
    if doc.style(&base).is_none() {
        return base;
    }
    (1..=u32::MAX)
        .map(|i| format!("{base}{i}"))
        .find(|id| doc.style(id).is_none())
        .unwrap_or(base)
}

// ---------------------------------------------------------------- usage

fn each_block_para(blocks: &[Block], f: &mut dyn FnMut(&Paragraph), tables: &mut dyn FnMut(&str)) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => f(p),
            Block::Table(t) => {
                if let Some(s) = &t.props.style {
                    tables(s);
                }
                for c in t.rows.iter().flat_map(|r| &r.cells) {
                    each_block_para(&c.blocks, f, tables);
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

fn each_story(doc: &Document) -> Vec<&[Block]> {
    let mut out: Vec<&[Block]> = Vec::new();
    for s in &doc.sections {
        out.push(&s.blocks);
        for hf in [&s.headers, &s.footers] {
            for b in [&hf.default, &hf.first, &hf.even].into_iter().flatten() {
                out.push(b);
            }
        }
    }
    for b in doc.footnotes.values().chain(doc.endnotes.values()) {
        out.push(b);
    }
    for c in &doc.comments {
        out.push(&c.blocks);
    }
    out
}

/// Ids of the styles the document's content uses (§17.3.1.27 pStyle, §17.3.2.29
/// rStyle, §17.4.63 tblStyle), plus the default paragraph style when a paragraph
/// names none.
pub fn styles_in_use(doc: &Document) -> BTreeSet<String> {
    let mut used = BTreeSet::new();
    let mut unstyled = false;
    let mut tables = BTreeSet::new();
    for blocks in each_story(doc) {
        each_block_para(
            blocks,
            &mut |p| {
                match &p.props.style {
                    Some(s) => {
                        used.insert(s.clone());
                    }
                    None => unstyled = true,
                }
                for r in &p.runs {
                    if let Some(s) = &r.props.style {
                        used.insert(s.clone());
                    }
                }
            },
            &mut |t| {
                tables.insert(t.to_owned());
            },
        );
    }
    if unstyled {
        if let Some(d) = doc.styles.default_of(StyleKind::Paragraph) {
            used.insert(d.id.clone());
        }
    }
    used.extend(tables);
    used
}

// ---------------------------------------------------------------- document-level edits

fn map_blocks(
    blocks: &mut [Block],
    f: &mut dyn FnMut(&mut Paragraph),
    t: &mut dyn FnMut(&mut Option<String>),
) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => f(p),
            Block::Table(tb) => {
                t(&mut tb.props.style);
                for c in tb.rows.iter_mut().flat_map(|r| &mut r.cells) {
                    map_blocks(&mut c.blocks, f, t);
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

fn map_stories(
    doc: &mut Document,
    f: &mut dyn FnMut(&mut Paragraph),
    t: &mut dyn FnMut(&mut Option<String>),
) {
    for s in &mut doc.sections {
        map_blocks(&mut s.blocks, f, t);
        for hf in [&mut s.headers, &mut s.footers] {
            for b in [&mut hf.default, &mut hf.first, &mut hf.even]
                .into_iter()
                .flatten()
            {
                map_blocks(b, f, t);
            }
        }
    }
    for b in doc.footnotes.values_mut().chain(doc.endnotes.values_mut()) {
        map_blocks(b, f, t);
    }
    // Comment text uses styles too (§17.13.4.2 comment holds block-level content).
    for c in &mut doc.comments {
        map_blocks(&mut c.blocks, f, t);
    }
}

/// Delete style `id` from `doc`: content using it is reassigned to its `basedOn`
/// style (paragraphs naming the default paragraph style drop `pStyle`; runs of a
/// character style without a base drop `rStyle`), styles based on it are rebased on
/// its base with its properties folded in (so they look the same), and `next` /
/// `link` references to it are removed. Errors: `noStyle`, `defaultStyle` (the
/// default style of its type cannot be deleted, §17.7.4.17 default).
pub fn delete_style_in(doc: &mut Document, id: &str) -> std::result::Result<(), String> {
    let Some(st) = doc.style(id).cloned() else {
        return Err("noStyle".into());
    };
    if st.is_default {
        return Err("defaultStyle".into());
    }
    let base = st.based_on.clone().filter(|b| doc.style(b).is_some());
    let default_para = doc
        .styles
        .default_of(StyleKind::Paragraph)
        .map(|s| s.id.clone());
    let to = match st.kind {
        StyleKind::Paragraph => base.clone().filter(|b| Some(b) != default_para.as_ref()),
        _ => base.clone(),
    };
    let sid = id.to_owned();
    let kind = st.kind;
    map_stories(
        doc,
        &mut |p| {
            if kind == StyleKind::Paragraph && p.props.style.as_deref() == Some(&sid) {
                p.props.style.clone_from(&to);
            }
            if kind == StyleKind::Character {
                for r in &mut p.runs {
                    if r.props.style.as_deref() == Some(&sid) {
                        r.props.style.clone_from(&to);
                    }
                }
            }
        },
        &mut |t| {
            if kind == StyleKind::Table && t.as_deref() == Some(&sid) {
                t.clone_from(&to);
            }
        },
    );
    doc.styles.styles.retain(|s| s.id != sid);
    for s in &mut doc.styles.styles {
        if s.based_on.as_deref() == Some(&sid) {
            s.ppr = s.ppr.inherit(&st.ppr);
            s.rpr = s.rpr.inherit(&st.rpr);
            s.ppr.style = None;
            s.rpr.style = None;
            s.based_on.clone_from(&base);
        }
        if s.next.as_deref() == Some(&sid) {
            s.next = None;
        }
        if s.linked.as_deref() == Some(&sid) {
            s.linked = None;
        }
    }
    Ok(())
}

/// Rename style `id` (display name only; the id content refers to stays).
/// Errors: `noStyle`, `badName`, `nameTaken`.
pub fn rename_style_in(
    doc: &mut Document,
    id: &str,
    name: &str,
) -> std::result::Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("badName".into());
    }
    if doc
        .styles
        .styles
        .iter()
        .any(|s| s.id != id && display_name(s).eq_ignore_ascii_case(name))
    {
        return Err("nameTaken".into());
    }
    let s = doc
        .styles
        .styles
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| "noStyle".to_owned())?;
    s.name = name.to_owned();
    Ok(())
}

/// Copy styles `ids` from `from` into `to` (Organizer). A copied style replaces the
/// target's style of the same id; its `basedOn` chain, `link`ed partner and `next`
/// style are copied too when the target lacks them, so it resolves the same way.
/// Numbering a style refers to (§17.3.1.19 numPr in the style's pPr, §17.9.15 num,
/// §17.9.1 abstractNum) is copied under new ids. Returns the ids copied.
pub fn copy_styles(from: &Document, to: &mut Document, ids: &[String]) -> Vec<String> {
    // (id, overwrite)
    let mut work: Vec<(String, bool)> = ids.iter().map(|i| (i.clone(), true)).collect();
    let mut seen = BTreeSet::new();
    let mut copied = Vec::new();
    let mut nums: BTreeMap<i32, i32> = BTreeMap::new();
    while let Some((id, overwrite)) = work.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(src) = from.style(&id) else {
            continue;
        };
        let exists = to.style(&id).is_some();
        if exists && !overwrite {
            continue;
        }
        let mut st = src.clone();
        st.src = None;
        if let Some(n) = st.ppr.num.filter(|n| n.num_id != 0) {
            let new_id = match nums.get(&n.num_id) {
                Some(v) => Some(*v),
                None => copy_num(from, to, n.num_id).inspect(|v| {
                    nums.insert(n.num_id, *v);
                }),
            };
            st.ppr.num = new_id.map(|num_id| NumRef { num_id, ..n });
        }
        // The target keeps its own default of each type.
        if st.is_default && to.styles.default_of(st.kind).is_some_and(|d| d.id != id) {
            st.is_default = false;
        }
        for dep in [&src.based_on, &src.next].into_iter().flatten() {
            work.push((dep.clone(), false));
        }
        if let Some(l) = &src.linked {
            work.push((l.clone(), overwrite));
        }
        match to.styles.styles.iter_mut().find(|s| s.id == id) {
            Some(t) => *t = st,
            None => to.styles.styles.push(st),
        }
        copied.push(id);
    }
    copied
}

/// Copy numbering instance `num_id` of `from` (and its abstract definition) into
/// `to` under fresh ids; the new `numId`.
pub(crate) fn copy_num(from: &Document, to: &mut Document, num_id: i32) -> Option<i32> {
    let num = from.numbering.nums.iter().find(|n| n.id == num_id)?;
    let abs = from
        .numbering
        .abstracts
        .iter()
        .find(|a| a.id == num.abstract_id)?;
    let new_abs = to
        .numbering
        .abstracts
        .iter()
        .map(|a| a.id)
        .max()
        .map_or(0, |m| m + 1);
    let new_num = to
        .numbering
        .nums
        .iter()
        .map(|n| n.id)
        .max()
        .map_or(1, |m| m + 1);
    let mut a = abs.clone();
    a.id = new_abs;
    let mut n = num.clone();
    n.id = new_num;
    n.abstract_id = new_abs;
    to.numbering.abstracts.push(a);
    to.numbering.nums.push(n);
    Some(new_num)
}

// ---------------------------------------------------------------- inspector / reveal

/// Where an effective value comes from (§17.7.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    /// Document defaults (§17.7.5.1 docDefaults) or the application default.
    Default,
    /// A paragraph style of the chain (`style` names it).
    ParagraphStyle,
    /// A character style of the chain.
    CharacterStyle,
    /// Direct formatting.
    Direct,
    /// Section properties (§17.6.17 sectPr).
    Section,
}

/// One effective formatting value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealItem {
    /// `font`, `paragraph` or `section`.
    pub group: String,
    /// Property key (`font`, `size`, `bold`, `align`, `spaceBefore`, `pageWidth`, …).
    pub key: String,
    /// Value, language neutral: `true`/`false`, points as `12pt`, names, `RRGGBB`.
    pub value: String,
    /// Where it comes from.
    pub source: SourceKind,
    /// The style that sets it (style sources).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}

fn pt(t: Twips) -> String {
    fmt_pt(f64::from(t.0) / 20.0)
}

fn fmt_pt(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{r:.0}pt")
    } else {
        format!("{r:.1}pt")
    }
}

type RunGet = fn(&Document, &RunProps) -> Option<String>;
type ParaGet = fn(&ParaProps) -> Option<String>;

fn run_keys() -> Vec<(&'static str, RunGet)> {
    vec![
        ("font", |d, r| {
            r.theme_ascii
                .as_deref()
                .and_then(|t| d.theme_fonts.resolve(t))
                .map(str::to_owned)
                .or(r.font.clone())
        }),
        ("size", |_, r| r.size.map(|h| fmt_pt(f64::from(h.0) / 2.0))),
        ("bold", |_, r| r.bold.map(|v| v.to_string())),
        ("italic", |_, r| r.italic.map(|v| v.to_string())),
        ("underline", |_, r| {
            r.underline.map(|u| u.as_str().to_owned())
        }),
        ("strike", |_, r| r.strike.map(|v| v.to_string())),
        ("doubleStrike", |_, r| {
            r.double_strike.map(|v| v.to_string())
        }),
        ("color", |_, r| r.color.map(Color::to_hex)),
        ("highlight", |_, r| r.highlight.map(Color::to_hex)),
        ("shading", |_, r| r.shading.map(Color::to_hex)),
        ("caps", |_, r| r.caps.map(|v| v.to_string())),
        ("smallCaps", |_, r| r.small_caps.map(|v| v.to_string())),
        ("vertAlign", |_, r| {
            r.vert_align.map(|v| {
                match v {
                    bo_model::VertAlign::Baseline => "baseline",
                    bo_model::VertAlign::Superscript => "superscript",
                    bo_model::VertAlign::Subscript => "subscript",
                }
                .to_owned()
            })
        }),
        ("charSpacing", |_, r| r.spacing.map(pt)),
        ("position", |_, r| {
            r.position.map(|v| fmt_pt(f64::from(v) / 2.0))
        }),
        ("scale", |_, r| r.scale.map(|v| format!("{v}%"))),
        ("hidden", |_, r| r.hidden.map(|v| v.to_string())),
        ("language", |_, r| {
            r.lang.as_ref().and_then(|l| l.val.clone())
        }),
    ]
}

fn para_keys() -> Vec<(&'static str, ParaGet)> {
    vec![
        ("align", |p| {
            p.align.map(|a| {
                match a {
                    Alignment::Left => "left",
                    Alignment::Center => "center",
                    Alignment::Right => "right",
                    Alignment::Justify => "justify",
                }
                .to_owned()
            })
        }),
        ("indentLeft", |p| p.indent_left.map(pt)),
        ("indentRight", |p| p.indent_right.map(pt)),
        ("indentFirstLine", |p| p.indent_first_line.map(pt)),
        ("spaceBefore", |p| p.space_before.map(pt)),
        ("spaceAfter", |p| p.space_after.map(pt)),
        ("lineSpacing", |p| {
            p.line_spacing.map(|l| match l {
                LineSpacing::Auto(v) => format!("{}", f64::from(v) / 240.0),
                LineSpacing::Exact(t) => format!("={}", pt(t)),
                LineSpacing::AtLeast(t) => format!("≥{}", pt(t)),
            })
        }),
        ("keepNext", |p| p.keep_next.map(|v| v.to_string())),
        ("keepLines", |p| p.keep_lines.map(|v| v.to_string())),
        ("pageBreakBefore", |p| {
            p.page_break_before.map(|v| v.to_string())
        }),
        ("widowControl", |p| p.widow_control.map(|v| v.to_string())),
        ("outlineLevel", |p| {
            p.outline_level.map(|v| (v + 1).to_string())
        }),
        ("numbering", |p| {
            p.num
                .filter(|n| n.num_id != 0)
                .map(|n| format!("{}:{}", n.num_id, n.ilvl + 1))
        }),
        ("shading", |p| p.shading.map(Color::to_hex)),
        ("borders", |p| {
            border_spec(p.borders.as_ref()).map(|b| b.style)
        }),
    ]
}

/// Values of the properties set in `rp` alone (direct formatting lists).
fn run_values(doc: &Document, rp: &RunProps) -> Vec<(String, String)> {
    run_keys()
        .into_iter()
        .filter_map(|(k, g)| g(doc, rp).map(|v| (k.to_owned(), v)))
        .collect()
}

fn para_values(pp: &ParaProps) -> Vec<(String, String)> {
    para_keys()
        .into_iter()
        .filter_map(|(k, g)| g(pp).map(|v| (k.to_owned(), v)))
        .collect()
}

/// A style at the caret (inspector).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleRef {
    /// Style id.
    pub id: String,
    /// Display name.
    pub name: String,
}

/// One direct formatting value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectValue {
    /// Property key (as [`RevealItem::key`]).
    pub key: String,
    /// Value.
    pub value: String,
}

/// Style Inspector: the two formatting levels of the paragraph and of the text at
/// the caret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorInfo {
    /// Paragraph style (the default paragraph style when the paragraph names none).
    pub paragraph_style: Option<StyleRef>,
    /// Direct paragraph formatting.
    pub paragraph_direct: Vec<DirectValue>,
    /// Character style of the text at the caret.
    pub character_style: Option<StyleRef>,
    /// Direct character formatting of the text at the caret.
    pub character_direct: Vec<DirectValue>,
}

fn style_ref(doc: &Document, id: &str) -> StyleRef {
    StyleRef {
        id: id.to_owned(),
        name: doc.style(id).map_or_else(|| id.to_owned(), display_name),
    }
}

fn direct(v: Vec<(String, String)>) -> Vec<DirectValue> {
    v.into_iter()
        .map(|(key, value)| DirectValue { key, value })
        .collect()
}

/// Inspector values for a paragraph's properties and the direct run properties at
/// the caret.
pub fn inspect(doc: &Document, pp: &ParaProps, rp: &RunProps) -> InspectorInfo {
    let pstyle = pp.style.clone().or_else(|| {
        doc.styles
            .default_of(StyleKind::Paragraph)
            .map(|s| s.id.clone())
    });
    InspectorInfo {
        paragraph_style: pstyle.map(|s| style_ref(doc, &s)),
        paragraph_direct: direct(para_values(pp)),
        character_style: rp.style.as_deref().map(|s| style_ref(doc, s)),
        character_direct: direct(run_values(
            doc,
            &RunProps {
                style: None,
                ..rp.clone()
            },
        )),
    }
}

/// Reveal Formatting: every effective value at a paragraph (`pp` direct paragraph
/// properties, `rp` direct run properties at the caret, `section` the paragraph's
/// section) with its source, resolved along §17.7.2: document defaults, paragraph
/// style chain (base first), character style chain, direct formatting.
pub fn reveal(
    doc: &Document,
    pp: &ParaProps,
    rp: &RunProps,
    section: Option<&bo_model::Section>,
) -> Vec<RevealItem> {
    let pstyle = pp.style.clone().or_else(|| {
        doc.styles
            .default_of(StyleKind::Paragraph)
            .map(|s| s.id.clone())
    });
    let pchain: Vec<&Style> = pstyle
        .as_deref()
        .map(|s| doc.styles.chain(s))
        .unwrap_or_default();
    let cchain: Vec<&Style> = rp
        .style
        .as_deref()
        .map(|s| doc.styles.chain(s))
        .unwrap_or_default();
    let mut out = Vec::new();
    let item =
        |group: &str, key: &str, value: String, source: SourceKind, style: Option<&Style>| {
            RevealItem {
                group: group.into(),
                key: key.into(),
                value,
                source,
                style: style.map(display_name),
            }
        };
    for (key, get) in run_keys() {
        let mut found = get(doc, &doc.default_run).map(|v| (v, SourceKind::Default, None));
        for s in &pchain {
            if let Some(v) = get(doc, &s.rpr) {
                found = Some((v, SourceKind::ParagraphStyle, Some(*s)));
            }
        }
        for s in &cchain {
            if let Some(v) = get(doc, &s.rpr) {
                found = Some((v, SourceKind::CharacterStyle, Some(*s)));
            }
        }
        if let Some(v) = get(doc, rp) {
            found = Some((v, SourceKind::Direct, None));
        }
        // Application defaults (§17.7.5.1: absent docDefaults).
        let found = found.or_else(|| match key {
            "font" => Some(("Times New Roman".into(), SourceKind::Default, None)),
            "size" => Some(("10pt".into(), SourceKind::Default, None)),
            _ => None,
        });
        if let Some((v, src, st)) = found {
            out.push(item("font", key, v, src, st));
        }
    }
    if let Some(s) = &pstyle {
        let st = doc.style(s);
        out.push(item(
            "paragraph",
            "style",
            st.map_or_else(|| s.clone(), display_name),
            if pp.style.is_some() {
                SourceKind::Direct
            } else {
                SourceKind::Default
            },
            None,
        ));
    }
    for (key, get) in para_keys() {
        let mut found = get(&doc.default_para).map(|v| (v, SourceKind::Default, None));
        for s in &pchain {
            if let Some(v) = get(&s.ppr) {
                found = Some((v, SourceKind::ParagraphStyle, Some(*s)));
            }
        }
        if let Some(v) = get(pp) {
            found = Some((v, SourceKind::Direct, None));
        }
        if let Some((v, src, st)) = found {
            out.push(item("paragraph", key, v, src, st));
        }
    }
    if let Some(sec) = section {
        let pg = &sec.page;
        for (key, v) in [
            ("pageWidth", pt(pg.width)),
            ("pageHeight", pt(pg.height)),
            (
                "orientation",
                if pg.width.0 > pg.height.0 {
                    "landscape".into()
                } else {
                    "portrait".into()
                },
            ),
            ("marginTop", pt(pg.margin_top)),
            ("marginBottom", pt(pg.margin_bottom)),
            ("marginLeft", pt(pg.margin_left)),
            ("marginRight", pt(pg.margin_right)),
            ("columns", sec.columns.count.max(1).to_string()),
        ] {
            out.push(item("section", key, v, SourceKind::Section, None));
        }
    }
    out
}

// ---------------------------------------------------------------- session

/// Formatting level the Style Inspector clears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClearLevel {
    /// The paragraph style (back to the default paragraph style).
    ParagraphStyle,
    /// Direct paragraph formatting.
    ParagraphDirect,
    /// The character style.
    CharacterStyle,
    /// Direct character formatting.
    CharacterDirect,
}

impl Session {
    /// Replace the style sheet, numbering and stories by those of `d` (a modified
    /// clone of the document) in one undo step, with `sel` after it.
    fn adopt(&mut self, d: &Document, sel: Selection) -> Result<Selection> {
        let ops = {
            let doc = &self.doc;
            let mut ops = Vec::new();
            if d.styles != doc.styles {
                ops.push(Op::Styles {
                    old: Box::new(doc.styles.clone()),
                    new: Box::new(d.styles.clone()),
                });
            }
            if d.numbering != doc.numbering {
                ops.push(Op::Numbering {
                    old: Box::new(doc.numbering.clone()),
                    new: Box::new(d.numbering.clone()),
                });
            }
            for (i, (a, b)) in doc.sections.iter().zip(&d.sections).enumerate() {
                if a != b {
                    ops.push(Op::Sections {
                        index: i,
                        remove: vec![a.clone()],
                        insert: vec![b.clone()],
                    });
                }
            }
            for endnote in [false, true] {
                let (a, b) = if endnote {
                    (&doc.endnotes, &d.endnotes)
                } else {
                    (&doc.footnotes, &d.footnotes)
                };
                if a != b {
                    ops.push(Op::Notes {
                        endnote,
                        old: a.clone(),
                        new: b.clone(),
                    });
                }
            }
            if d.comments != doc.comments {
                ops.push(Op::Comments {
                    old: doc.comments.clone(),
                    new: d.comments.clone(),
                });
            }
            ops
        };
        self.edit(move |tx, _| {
            for op in ops {
                tx.push(op)?;
            }
            Ok(sel)
        })
    }

    fn caret_para(&self) -> Option<Paragraph> {
        let at = self.selection.ordered().0;
        let paths = body_paragraphs(&self.doc);
        paths
            .get(at.para)
            .and_then(|p| para_at(&self.doc, p))
            .cloned()
    }

    /// Dialog values of a style (`noStyle` when absent).
    pub fn style_spec(&self, id: &str) -> Result<StyleSpec> {
        if self.doc.style(id).is_none() {
            // A built-in the gallery offers before first use (`builtin_style`).
            let mut d = Document {
                styles: with_builtin(&self.doc.styles, id),
                ..Document::default()
            };
            d.default_run.clone_from(&self.doc.default_run);
            d.default_para.clone_from(&self.doc.default_para);
            return style_spec(&d, id).ok_or_else(|| EditError::Unsupported("noStyle".into()));
        }
        style_spec(&self.doc, id).ok_or_else(|| EditError::Unsupported("noStyle".into()))
    }

    /// Modify style `id` from the dialog (one undo step; content follows).
    pub fn modify_style(&mut self, id: &str, spec: &StyleSpec) -> Result<Selection> {
        let cur = self.style_spec(id)?;
        if !spec.name.trim().is_empty() && spec.name.trim() != cur.name {
            let mut probe = self.doc.clone();
            rename_style_in(&mut probe, id, &spec.name).map_err(EditError::Unsupported)?;
        }
        if let Some(b) = &spec.based_on {
            if self.doc.styles.chain(b).iter().any(|s| s.id == id) {
                return Err(EditError::Unsupported("basedOnLoop".into()));
            }
        }
        let mut sheet = with_builtin(&self.doc.styles, id);
        let Some(st) = sheet.styles.iter_mut().find(|s| s.id == id) else {
            return Err(EditError::Unsupported("noStyle".into()));
        };
        apply_spec(st, spec, &cur);
        self.push_styles(sheet)
    }

    fn push_styles(&mut self, sheet: bo_model::StyleSheet) -> Result<Selection> {
        if sheet == self.doc.styles {
            return Ok(self.selection);
        }
        let old = self.doc.styles.clone();
        self.edit(move |tx, sel| {
            tx.push(Op::Styles {
                old: Box::new(old),
                new: Box::new(sheet),
            })?;
            Ok(sel)
        })
    }

    /// New style from the dialog; `from_selection`: it takes the paragraph formatting
    /// and the character formatting at the caret over the dialog values (Word's "new
    /// style from formatting") and is applied to the selection. Returns the new id.
    pub fn new_style(&mut self, spec: &StyleSpec, from_selection: bool) -> Result<String> {
        let name = spec.name.trim();
        if name.is_empty() {
            return Err(EditError::Unsupported("badName".into()));
        }
        if self
            .doc
            .styles
            .styles
            .iter()
            .any(|s| display_name(s).eq_ignore_ascii_case(name))
        {
            return Err(EditError::Unsupported("nameTaken".into()));
        }
        let kind = match spec.kind {
            StyleType::Character => StyleKind::Character,
            _ => StyleKind::Paragraph,
        };
        let id = new_style_id(&self.doc, name);
        let mut st = Style::new(&id, name, kind);
        st.custom = true;
        st.quick = spec.add_to_gallery;
        st.ui_priority = None;
        st.based_on = spec
            .based_on
            .clone()
            .filter(|b| self.doc.style(b).is_some());
        if kind == StyleKind::Paragraph {
            st.next = spec.next.clone().or_else(|| Some(id.clone()));
        }
        // Defaults the dialog compares against: the base's effective values.
        let base_spec = st
            .based_on
            .as_deref()
            .and_then(|b| style_spec(&self.doc, b))
            .unwrap_or_default();
        let cur = StyleSpec {
            name: spec.name.clone(),
            kind: spec.kind,
            based_on: spec.based_on.clone(),
            next: spec.next.clone(),
            ..base_spec
        };
        apply_spec(&mut st, spec, &cur);
        if from_selection {
            if let Some(p) = self.caret_para() {
                let direct_p = clean_para(&p.props);
                let direct_r = clean_run(&self.caret_direct_props());
                if kind == StyleKind::Paragraph {
                    st.ppr = direct_p.inherit(&st.ppr);
                    st.ppr.style = None;
                }
                st.rpr = direct_r.inherit(&st.rpr);
                st.rpr.style = None;
            }
        }
        let old = self.doc.styles.clone();
        let mut sheet = old.clone();
        sheet.styles.push(st);
        let sid = id.clone();
        let ranges = self.selected_ranges(self.selection);
        let collapsed = self.selection.is_collapsed();
        self.edit(move |tx, sel| {
            tx.push(Op::Styles {
                old: Box::new(old),
                new: Box::new(sheet),
            })?;
            if !from_selection {
                return Ok(sel);
            }
            for (pid, s, e) in ranges {
                let (path, p, mut atoms) = tx.atoms(pid)?;
                let mut q = p.clone();
                if kind == StyleKind::Paragraph {
                    // The paragraph takes the style; its direct formatting moved into it.
                    q.props = ParaProps {
                        style: Some(sid.clone()),
                        num: q.props.num,
                        unknown: std::mem::take(&mut q.props.unknown),
                        mark_ins: q.props.mark_ins.take(),
                        mark_del: q.props.mark_del.take(),
                        change: q.props.change.take(),
                        ..ParaProps::default()
                    };
                    tx.replace_para(&path, q)?;
                } else if !collapsed && s < e {
                    para::format(&mut atoms, s, e, &|r| r.style = Some(sid.clone()));
                    tx.replace_para(&path, para::with_atoms(&p, atoms))?;
                }
            }
            Ok(sel)
        })?;
        Ok(id)
    }

    /// Delete style `id`, reassigning its content to its base (one undo step).
    pub fn delete_style(&mut self, id: &str) -> Result<Selection> {
        self.delete_styles(&[id.to_owned()])
    }

    /// Delete several styles in one undo step.
    pub fn delete_styles(&mut self, ids: &[String]) -> Result<Selection> {
        let mut d = self.doc.clone();
        for id in ids {
            delete_style_in(&mut d, id).map_err(EditError::Unsupported)?;
        }
        let sel = self.selection;
        self.adopt(&d, sel)
    }

    /// Every place style `id` is used in the body (paragraphs for paragraph styles,
    /// text ranges for character styles), in document order.
    pub fn style_instances(&self, id: &str) -> Vec<crate::Match> {
        let Some(st) = self.doc.style(id) else {
            return Vec::new();
        };
        let is_default = st.is_default;
        let kind = st.kind;
        let paths = body_paragraphs(&self.doc);
        let mut out = Vec::new();
        for (pid, path) in paths.iter().enumerate() {
            let Some(p) = para_at(&self.doc, path) else {
                continue;
            };
            let atoms = para::flatten(p);
            match kind {
                StyleKind::Paragraph => {
                    let mine = match &p.props.style {
                        Some(s) => s == id,
                        None => is_default,
                    };
                    if mine {
                        out.push(crate::Match::new(pid, 0, para::len(&atoms)));
                    }
                }
                StyleKind::Character => {
                    let mut off = 0;
                    let mut open: Option<usize> = None;
                    for (rp, c) in &atoms {
                        let l = para::atom_len(c);
                        let on = rp.style.as_deref() == Some(id);
                        match (on, open) {
                            (true, None) => open = Some(off),
                            (false, Some(s)) => {
                                out.push(crate::Match::new(pid, s, off));
                                open = None;
                            }
                            _ => {}
                        }
                        off += l;
                    }
                    if let Some(s) = open {
                        out.push(crate::Match::new(pid, s, off));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Update style `id` to match the selection: the direct paragraph formatting of
    /// the caret paragraph and the direct character formatting at the caret become
    /// part of the style (one undo step).
    pub fn update_style_to_match(&mut self, id: &str) -> Result<Selection> {
        let Some(p) = self.caret_para() else {
            return Err(EditError::BadPosition);
        };
        let mut sheet = self.doc.styles.clone();
        let Some(st) = sheet.styles.iter_mut().find(|s| s.id == id) else {
            return Err(EditError::Unsupported("noStyle".into()));
        };
        let dp = clean_para(&p.props);
        let dr = clean_run(&self.caret_direct_props());
        if st.kind != StyleKind::Character {
            st.ppr = dp.inherit(&st.ppr);
            st.ppr.style = None;
        }
        st.rpr = dr.inherit(&st.rpr);
        st.rpr.style = None;
        self.push_styles(sheet)
    }

    /// "Automatically update" (§17.7.4.2 autoRedefine): after direct formatting of
    /// the selected paragraphs (`para`: paragraph formatting; otherwise character
    /// formatting, which counts only when whole paragraphs are selected), a paragraph
    /// style marked autoRedefine takes that formatting and the paragraphs drop it, so
    /// every paragraph of the style changes. The change joins the formatting's undo
    /// step (recorded after history depth `depth`). Returns whether a style changed.
    pub fn auto_redefine_after(&mut self, depth: usize, para: bool) -> Result<bool> {
        let (a, b) = self.selection.ordered();
        if !para {
            let whole = a.offset == 0
                && self
                    .para_text(b.para)
                    .is_some_and(|t| b.offset >= t.len() && (b.para > a.para || !t.is_empty()));
            if !whole {
                return Ok(false);
            }
        }
        let default_id = self
            .doc
            .styles
            .default_of(StyleKind::Paragraph)
            .map(|s| s.id.clone());
        let paths = body_paragraphs(&self.doc);
        // The first selected paragraph of an autoRedefine style gives the formatting.
        let mut target: Option<(String, ParaProps, RunProps)> = None;
        for id in a.para..=b.para {
            let Some(p) = paths.get(id).and_then(|path| para_at(&self.doc, path)) else {
                continue;
            };
            let sid = p.props.style.clone().or_else(|| default_id.clone());
            let Some(sid) = sid else { continue };
            if !self.doc.style(&sid).is_some_and(|s| s.auto_redefine) {
                continue;
            }
            let atoms = para::flatten(p);
            let run = atoms
                .iter()
                .find(|(_, c)| para::atom_len(c) > 0)
                .map(|(r, _)| clean_run(r))
                .unwrap_or_default();
            target = Some((sid, clean_para(&p.props), run));
            break;
        }
        let Some((sid, dp, dr)) = target else {
            return Ok(false);
        };
        let mut sheet = self.doc.styles.clone();
        let Some(st) = sheet.styles.iter_mut().find(|s| s.id == sid) else {
            return Ok(false);
        };
        if para {
            st.ppr = dp.inherit(&st.ppr);
            st.ppr.style = None;
        } else {
            st.rpr = dr.inherit(&st.rpr);
            st.rpr.style = None;
        }
        let is_default = default_id.as_deref() == Some(sid.as_str());
        let old = self.doc.styles.clone();
        self.end_typing();
        self.edit(move |tx, sel| {
            tx.push(Op::Styles {
                old: Box::new(old),
                new: Box::new(sheet),
            })?;
            // The selected paragraphs of the style drop what the style now holds.
            for id in a.para..=b.para {
                let (path, p, atoms) = tx.atoms(id)?;
                let mine = match &p.props.style {
                    Some(s) => *s == sid,
                    None => is_default,
                };
                if !mine {
                    continue;
                }
                let mut q = if para {
                    let mut q = p.clone();
                    q.props = ParaProps {
                        style: p.props.style.clone(),
                        num: p.props.num,
                        mark_ins: p.props.mark_ins.clone(),
                        mark_del: p.props.mark_del.clone(),
                        mark_lang: p.props.mark_lang.clone(),
                        mark_no_proof: p.props.mark_no_proof,
                        change: p.props.change.clone(),
                        frame: p.props.frame,
                        unknown: p.props.unknown.clone(),
                        ..ParaProps::default()
                    };
                    q
                } else {
                    let atoms = atoms
                        .into_iter()
                        .map(|(r, c)| {
                            if clean_run(&r) == dr {
                                let kept = RunProps {
                                    style: r.style.clone(),
                                    link: r.link.clone(),
                                    ins: r.ins.clone(),
                                    del: r.del.clone(),
                                    change: r.change.clone(),
                                    unknown: r.unknown.clone(),
                                    ..RunProps::default()
                                };
                                (kept, c)
                            } else {
                                (r, c)
                            }
                        })
                        .collect();
                    para::with_atoms(&p, atoms)
                };
                if q != p {
                    q.props.change = p.props.change.clone();
                    tx.replace_para(&path, q)?;
                }
            }
            Ok(sel)
        })?;
        self.merge_history_since(depth);
        Ok(true)
    }

    /// Inspector values at the caret.
    pub fn inspector(&self) -> InspectorInfo {
        let pp = self.caret_para().map(|p| p.props).unwrap_or_default();
        let rp = self
            .pending
            .clone()
            .unwrap_or_else(|| self.caret_direct_props());
        inspect(&self.doc, &pp, &rp)
    }

    /// Reveal Formatting at the caret.
    pub fn reveal_formatting(&self) -> Vec<RevealItem> {
        let at = self.selection.ordered().0;
        let paths = body_paragraphs(&self.doc);
        let pp = self.caret_para().map(|p| p.props).unwrap_or_default();
        let rp = self
            .pending
            .clone()
            .unwrap_or_else(|| self.caret_direct_props());
        let sec = paths
            .get(at.para)
            .and_then(|p| p.first())
            .and_then(|si| self.doc.sections.get(*si));
        reveal(&self.doc, &pp, &rp, sec)
    }

    /// Clear one formatting level of the selection (Style Inspector). Character
    /// levels with a collapsed selection apply to the caret paragraph.
    pub fn clear_level(&mut self, level: ClearLevel) -> Result<Selection> {
        let mut ranges = self.selected_ranges(self.selection);
        if self.selection.is_collapsed() {
            for r in &mut ranges {
                r.1 = 0;
                r.2 = self.para_text(r.0).map_or(0, |t| t.len());
            }
            if matches!(
                level,
                ClearLevel::CharacterDirect | ClearLevel::CharacterStyle
            ) {
                self.pending = None;
            }
        }
        self.edit(move |tx, sel| {
            for (pid, s, e) in ranges {
                let (path, p, mut atoms) = tx.atoms(pid)?;
                let q = match level {
                    ClearLevel::ParagraphStyle => {
                        let mut q = p.clone();
                        q.props.style = None;
                        q
                    }
                    ClearLevel::ParagraphDirect => {
                        let mut q = p.clone();
                        let pr = &mut q.props;
                        *pr = ParaProps {
                            style: pr.style.take(),
                            unknown: std::mem::take(&mut pr.unknown),
                            mark_ins: pr.mark_ins.take(),
                            mark_del: pr.mark_del.take(),
                            mark_lang: pr.mark_lang.take(),
                            change: pr.change.take(),
                            ..ParaProps::default()
                        };
                        q
                    }
                    ClearLevel::CharacterStyle => {
                        para::format(&mut atoms, s, e, &|r| r.style = None);
                        para::with_atoms(&p, atoms)
                    }
                    ClearLevel::CharacterDirect => {
                        para::format(&mut atoms, s, e, &|r| {
                            *r = RunProps {
                                style: r.style.take(),
                                link: r.link.take(),
                                ins: r.ins.take(),
                                del: r.del.take(),
                                ..RunProps::default()
                            };
                        });
                        para::with_atoms(&p, atoms)
                    }
                };
                tx.replace_para(&path, q)?;
            }
            Ok(sel)
        })
    }

    /// Organizer into this document: copy `ids` from `from` (one undo step).
    pub fn organizer_copy_in(&mut self, from: &Document, ids: &[String]) -> Result<Vec<String>> {
        let mut d = self.doc.clone();
        let copied = copy_styles(from, &mut d, ids);
        let sel = self.selection;
        self.adopt(&d, sel)?;
        Ok(copied)
    }

    /// Organizer: rename style `id` of this document (one undo step).
    pub fn rename_style(&mut self, id: &str, name: &str) -> Result<Selection> {
        let mut d = self.doc.clone();
        rename_style_in(&mut d, id, name).map_err(EditError::Unsupported)?;
        self.push_styles(d.styles)
    }
}

/// `sheet` with the built-in definition of `id` (and Normal, its base) added when
/// missing (`crate::builtin_style`).
fn with_builtin(sheet: &bo_model::StyleSheet, id: &str) -> bo_model::StyleSheet {
    let mut out = sheet.clone();
    if out.styles.iter().any(|s| s.id == id) {
        return out;
    }
    let Some(b) = crate::builtin_style(id) else {
        return out;
    };
    if b.based_on.as_deref() == Some("Normal") && !out.styles.iter().any(|s| s.id == "Normal") {
        if let Some(n) = crate::builtin_style("Normal") {
            out.styles.push(n);
        }
    }
    out.styles.push(b);
    out
}

/// Direct paragraph properties that can move into a style (no style, numbering
/// kept by the paragraph, revision marks or unknown children).
fn clean_para(p: &ParaProps) -> ParaProps {
    ParaProps {
        style: None,
        num: None,
        mark_ins: None,
        mark_del: None,
        mark_lang: None,
        mark_no_proof: None,
        change: None,
        frame: None,
        unknown: Vec::new(),
        ..p.clone()
    }
}

/// Direct run properties that can move into a style.
fn clean_run(r: &RunProps) -> RunProps {
    RunProps {
        style: None,
        link: None,
        ins: None,
        del: None,
        change: None,
        unknown: Vec::new(),
        ..r.clone()
    }
}
