//! Live collaboration for BlueDoc (prompt §3.2, §5.2, §6 Phase 6): the
//! document body ↔ the `body` Y.Text of `bo-collab`.
//!
//! The body is flattened into *lines*, one per top-level block of every
//! section in order (the same model as rich-text Yjs bindings):
//! * a paragraph is its text followed by `\n`; run properties are text
//!   attributes (`bold`, `italic`, `underline`, `strike`, `size` in
//!   half-points, `color`, `font`, `highlight`, plus `r` = JSON of the
//!   remaining [`RunProps`]); paragraph properties are attributes of its `\n`
//!   (`style`, `align`, `list`, plus `p` = JSON of the remaining
//!   [`ParaProps`]); the last block of a section carries `sect` (JSON of the
//!   section properties, ECMA-376 Part 1 §17.6.17 sectPr);
//! * non-text run content (tabs, breaks, pictures, fields, bookmarks …) is a
//!   one-position embed `{"c": JSON}`; a top-level table is the embed
//!   `{"t": key}` + `\n` with its rows, columns and cells shared cell by cell
//!   ([`parts`]); a nested table or an unknown block is one embed
//!   `{"b": JSON}` followed by `\n` (last writer wins for that block).
//!
//! Styles, numbering, comments, foot/endnotes, headers/footers travel in the
//! `meta` map and nested texts ([`parts`]).
//!
//! Positions are UTF-16 offsets into that text. The engine keeps a *shadow*
//! of the shared text (the lines, with the model block each line stands for).
//! After a local edit the changed blocks are found by comparing the model
//! with the shadow and turned into a minimal delta (common prefix/suffix per
//! character, formatting as `retain` with attributes). A remote delta is
//! applied to the shadow first; the changed lines are then rebuilt into model
//! blocks and applied as one transaction that is not part of the local
//! history ([`bo_edit::Session::apply_external`]).

pub(crate) mod order;
mod parts;

use crate::edit::EditResult;
use crate::{Engine, EngineError};
use bo_edit::para::{self, atom_len};
use bo_edit::{Op, Pos, Selection};
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{
    Block, Color, Document, HalfPoints, NumFormat, NumRef, Numbering, ParaProps, Paragraph,
    RunContent, RunProps, Section, StyleSheet,
};
use parts::{Seen, Story, TableShadow};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Text attributes of one position.
pub type Attrs = BTreeMap<String, Value>;
type Rc = Arc<Attrs>;

/// One Quill-style delta op over the body text (UTF-16 lengths; a `null`
/// attribute value removes the attribute).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TextOp {
    /// Insert text (or an embed).
    Insert {
        /// Text, or an embed object.
        insert: Value,
        /// Formatting of the inserted text.
        #[serde(default, skip_serializing_if = "Map::is_empty")]
        attributes: Map<String, Value>,
    },
    /// Delete characters.
    Delete {
        /// Number of UTF-16 code units to delete.
        delete: u32,
    },
    /// Skip characters, optionally changing their formatting.
    Retain {
        /// Number of UTF-16 code units to keep.
        retain: u32,
        /// Formatting changes of the retained text.
        #[serde(default, skip_serializing_if = "Map::is_empty")]
        attributes: Map<String, Value>,
    },
}

/// A change of the shared state (the `bo_collab::Change` envelope).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CollabChange {
    /// Body deltas, applied in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<Vec<TextOp>>,
    /// BlueSlide slide properties by slide key (`null` = removed).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slides: BTreeMap<String, Option<String>>,
    /// BlueSlide objects by `"<slide>/<shape>"` (`null` = removed).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shapes: BTreeMap<String, Option<String>>,
    /// Binary parts added during the session (`{"ct","b64"}` JSON).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub blobs: BTreeMap<String, Option<String>>,
    /// Other shared objects (JSON strings, last writer wins per key).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, Option<String>>,
    /// BlueSheet cells by `"<sheet>!<row>,<col>"` ids (`null` = removed).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cells: BTreeMap<String, Option<String>>,
    /// Nested texts by key (`null` = removed).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub texts: BTreeMap<String, Option<TextDelta>>,
}

/// Change of one nested text (`bo_collab::TextChange`): `reset` = recreated
/// with the content the deltas build from empty.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TextDelta {
    /// The text was recreated: apply `deltas` to an empty text.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reset: bool,
    /// Deltas applied in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deltas: Vec<Vec<TextOp>>,
}

impl CollabChange {
    /// Nothing changed.
    pub fn is_empty(&self) -> bool {
        self.body.iter().all(Vec::is_empty)
            && self.slides.is_empty()
            && self.shapes.is_empty()
            && self.blobs.is_empty()
            && self.meta.is_empty()
            && self.cells.is_empty()
            && self.texts.is_empty()
    }
}

/// Collaboration output of a BlueDoc command (part of [`EditResult`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CollabOut {
    /// What to apply to the replica (`WasmCollab.applyLocal`).
    pub change: CollabChange,
    /// The selection as UTF-16 offsets (anchor, focus) for presence.
    pub selection: (u32, u32),
}

/// A binary part shared through the `blobs` map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Blob {
    /// MIME content type of the part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ct: Option<String>,
    /// Part bytes, base64.
    pub b64: String,
}

impl Blob {
    /// JSON encoding of a part (content type and base64 bytes).
    pub fn encode(ct: Option<&str>, bytes: &[u8]) -> String {
        use base64::Engine as _;
        serde_json::to_string(&Blob {
            ct: ct.map(str::to_owned),
            b64: base64::engine::general_purpose::STANDARD.encode(bytes),
        })
        .unwrap_or_default()
    }

    /// `(content type, bytes)`; `None` for malformed input.
    pub fn decode(json: &str) -> Option<(Option<String>, Vec<u8>)> {
        use base64::Engine as _;
        let b: Blob = serde_json::from_str(json).ok()?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b.b64.as_bytes())
            .ok()?;
        Some((b.ct, bytes))
    }
}

/// Peer caret / selection geometry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PeerRects {
    /// Caret of the peer's selection focus, if laid out.
    pub caret: Option<crate::nav::CaretRect>,
    /// Highlight rectangles of the peer's range selection.
    pub rects: Vec<crate::nav::SelRect>,
}

// ---------------------------------------------------------------------------
// Lines
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Unit {
    Ch(char),
    Embed(Value),
    /// The paragraph's `\n`.
    End,
}

impl Unit {
    fn len16(&self) -> u32 {
        match self {
            Unit::Ch(c) => c.len_utf16() as u32,
            Unit::Embed(_) | Unit::End => 1,
        }
    }
}

type Cell = (Unit, Rc);

#[derive(Debug, Clone, PartialEq)]
enum Seg {
    Text(String, Rc),
    Embed(Value, Rc),
}

/// One line of the shared text: content then (normally) the paragraph end.
#[derive(Debug, Clone, PartialEq, Default)]
struct Line {
    segs: Vec<Seg>,
    /// Attributes of the `\n`; `None` for an unterminated last line.
    end: Option<Rc>,
}

impl Line {
    fn len16(&self) -> u32 {
        let content: u32 = self
            .segs
            .iter()
            .map(|s| match s {
                Seg::Text(t, _) => t.encode_utf16().count() as u32,
                Seg::Embed(..) => 1,
            })
            .sum();
        content + u32::from(self.end.is_some())
    }

    fn explode(&self, out: &mut Vec<Cell>) {
        for s in &self.segs {
            match s {
                Seg::Text(t, a) => out.extend(t.chars().map(|c| (Unit::Ch(c), a.clone()))),
                Seg::Embed(v, a) => out.push((Unit::Embed(v.clone()), a.clone())),
            }
        }
        if let Some(e) = &self.end {
            out.push((Unit::End, e.clone()));
        }
    }

    fn sect(&self) -> Option<String> {
        self.end
            .as_ref()
            .and_then(|e| e.get("sect"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    }
}

fn implode(cells: Vec<Cell>) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut cur = Line::default();
    for (u, a) in cells {
        match u {
            Unit::Ch(c) => {
                if let Some(Seg::Text(t, ta)) = cur.segs.last_mut() {
                    if *ta == a {
                        t.push(c);
                        continue;
                    }
                }
                cur.segs.push(Seg::Text(c.to_string(), a));
            }
            Unit::Embed(v) => cur.segs.push(Seg::Embed(v, a)),
            Unit::End => {
                cur.end = Some(a);
                lines.push(std::mem::take(&mut cur));
            }
        }
    }
    if !cur.segs.is_empty() {
        lines.push(cur);
    }
    lines
}

/// One line with the model block it stands for.
#[derive(Debug, Clone)]
struct Entry {
    block: Block,
    sect: Option<String>,
    line: Line,
    len: u32,
}

/// JSON numbers from Yrs are floats: integral ones become integers so that
/// attribute values compare equal on every replica.
fn norm(v: &Value) -> Value {
    match v {
        Value::Number(n) if n.as_i64().is_none() && n.as_u64().is_none() => n
            .as_f64()
            .filter(|f| f.fract() == 0.0 && f.abs() < 9.0e15)
            .map_or_else(|| v.clone(), |f| Value::from(f as i64)),
        _ => v.clone(),
    }
}

fn attrs_of(map: &Map<String, Value>) -> Attrs {
    map.iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| (k.clone(), norm(v)))
        .collect()
}

fn to_map(a: &Attrs) -> Map<String, Value> {
    a.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn attr_diff(old: &Attrs, new: &Attrs) -> Map<String, Value> {
    let mut d = Map::new();
    for (k, v) in new {
        if old.get(k) != Some(v) {
            d.insert(k.clone(), v.clone());
        }
    }
    for k in old.keys() {
        if !new.contains_key(k) {
            d.insert(k.clone(), Value::Null);
        }
    }
    d
}

// ------------------------------------------------------------- properties

/// Text attributes of run properties (§17.3.2 rPr).
fn run_attrs(p: &RunProps) -> Attrs {
    let mut a = Attrs::new();
    let mut rest = p.clone();
    if let Some(v) = rest.bold.take() {
        a.insert("bold".into(), Value::Bool(v));
    }
    if let Some(v) = rest.italic.take() {
        a.insert("italic".into(), Value::Bool(v));
    }
    if let Some(v) = rest.strike.take() {
        a.insert("strike".into(), Value::Bool(v));
    }
    if let Some(u) = rest.underline.take() {
        if let Ok(v) = serde_json::to_value(u) {
            a.insert("underline".into(), v);
        }
    }
    if let Some(h) = rest.size.take() {
        a.insert("size".into(), Value::from(h.0));
    }
    if let Some(c) = rest.color.take() {
        a.insert("color".into(), Value::String(c.to_hex()));
    }
    if let Some(f) = rest.font.take() {
        a.insert("font".into(), Value::String(f));
    }
    if let Some(c) = rest.highlight.take() {
        a.insert("highlight".into(), Value::String(c.to_hex()));
    }
    if rest != RunProps::default() {
        if let Ok(j) = serde_json::to_string(&rest) {
            a.insert("r".into(), Value::String(j));
        }
    }
    a
}

fn run_props(a: &Attrs) -> RunProps {
    let mut p: RunProps = a
        .get("r")
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    p.bold = a.get("bold").and_then(Value::as_bool);
    p.italic = a.get("italic").and_then(Value::as_bool);
    p.strike = a.get("strike").and_then(Value::as_bool);
    p.underline = a
        .get("underline")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    p.size = a
        .get("size")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .map(HalfPoints);
    p.color = a
        .get("color")
        .and_then(Value::as_str)
        .and_then(Color::from_hex);
    p.font = a.get("font").and_then(Value::as_str).map(str::to_owned);
    p.highlight = a
        .get("highlight")
        .and_then(Value::as_str)
        .and_then(Color::from_hex);
    p
}

fn is_bullet(n: &Numbering, num_id: i32) -> bool {
    n.level(NumRef { num_id, ilvl: 0 })
        .is_some_and(|l| l.format == NumFormat::Bullet)
}

/// Context for turning model blocks into lines.
struct LineCtx<'a> {
    numbering: &'a Numbering,
    base_nums: &'a BTreeSet<i32>,
}

/// Paragraph attributes of paragraph properties (§17.3.1 pPr). Lists from
/// the opened file keep their `numId`; lists created during the session are
/// shared by kind (`bullet:<ilvl>` / `number:<ilvl>`) because every replica
/// numbers its own new definitions.
fn para_attrs(p: &ParaProps, ctx: &LineCtx<'_>) -> Attrs {
    let mut a = Attrs::new();
    let mut rest = p.clone();
    if let Some(s) = rest.style.take() {
        a.insert("style".into(), Value::String(s));
    }
    if let Some(al) = rest.align.take() {
        if let Ok(v) = serde_json::to_value(al) {
            a.insert("align".into(), v);
        }
    }
    if let Some(n) = rest.num.take() {
        let key = if n.num_id <= 0 || ctx.base_nums.contains(&n.num_id) {
            format!("{}:{}", n.num_id, n.ilvl)
        } else if is_bullet(ctx.numbering, n.num_id) {
            format!("bullet:{}", n.ilvl)
        } else {
            format!("number:{}", n.ilvl)
        };
        a.insert("list".into(), Value::String(key));
    }
    if rest != ParaProps::default() {
        if let Ok(j) = serde_json::to_string(&rest) {
            a.insert("p".into(), Value::String(j));
        }
    }
    a
}

/// Working copies of the document parts remote paragraphs may need.
struct BuildCtx<'a> {
    numbering: Numbering,
    styles: StyleSheet,
    base_nums: &'a BTreeSet<i32>,
    numbering_changed: bool,
    styles_changed: bool,
    /// Shared tables and stories for `{"t"}` lines (body only).
    tables: Option<&'a BTreeMap<String, TableShadow>>,
    stories: Option<&'a BTreeMap<String, Story>>,
}

impl BuildCtx<'_> {
    fn list(&mut self, key: &str) -> Option<NumRef> {
        let (kind, lvl) = key.split_once(':')?;
        let ilvl: u8 = lvl.parse().ok()?;
        let num_id = match kind {
            "bullet" | "number" => {
                let bullet = kind == "bullet";
                let found = self.numbering.nums.iter().map(|n| n.id).find(|id| {
                    !self.base_nums.contains(id) && is_bullet(&self.numbering, *id) == bullet
                });
                if let Some(id) = found {
                    id
                } else {
                    self.numbering_changed = true;
                    self.numbering.add_simple_list(bullet)
                }
            }
            n => n.parse().ok()?,
        };
        Some(NumRef { num_id, ilvl })
    }

    fn para_props(&mut self, a: &Attrs) -> ParaProps {
        let mut p: ParaProps = a
            .get("p")
            .and_then(Value::as_str)
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        p.style = a.get("style").and_then(Value::as_str).map(str::to_owned);
        if let Some(id) = &p.style {
            if !self.styles.styles.iter().any(|s| &s.id == id) {
                if let Some(s) = bo_edit::builtin_style(id) {
                    self.styles.styles.push(s);
                    self.styles_changed = true;
                }
            }
        }
        p.align = a
            .get("align")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        p.num = a
            .get("list")
            .and_then(Value::as_str)
            .and_then(|k| self.list(k));
        p
    }

    /// The model block a line stands for.
    fn block(&mut self, line: &Line) -> Block {
        if let Some(key) = parts::table_key(line) {
            let empty = BTreeMap::new();
            let stories = self.stories.unwrap_or(&empty);
            return match self.tables.and_then(|t| t.get(key)) {
                Some(t) => t.build(key, stories),
                None => Block::Paragraph(Paragraph::default()),
            };
        }
        for s in &line.segs {
            if let Seg::Embed(v, _) = s {
                if let Some(b) = v
                    .get("b")
                    .and_then(Value::as_str)
                    .and_then(|j| serde_json::from_str::<Block>(j).ok())
                {
                    // A block embed stands for the whole line; stray content
                    // next to it (concurrent edits) is dropped on every replica.
                    return b;
                }
            }
        }
        let mut atoms = Vec::with_capacity(line.segs.len());
        for s in &line.segs {
            match s {
                Seg::Text(t, a) => atoms.push((run_props(a), RunContent::Text(t.clone()))),
                Seg::Embed(v, a) => {
                    if let Some(c) = v
                        .get("c")
                        .and_then(Value::as_str)
                        .and_then(|j| serde_json::from_str::<RunContent>(j).ok())
                    {
                        atoms.push((run_props(a), c));
                    }
                }
            }
        }
        let props = line
            .end
            .as_ref()
            .map(|e| self.para_props(e))
            .unwrap_or_default();
        Block::Paragraph(Paragraph::new(props, para::rebuild(atoms)))
    }
}

fn content_embed(c: &RunContent) -> Value {
    let mut o = Map::new();
    o.insert(
        "c".into(),
        Value::String(serde_json::to_string(c).unwrap_or_default()),
    );
    Value::Object(o)
}

/// The line of a model block (`table` = key of a top-level table shared cell by cell).
fn line_of(block: &Block, sect: Option<&str>, table: Option<&str>, ctx: &LineCtx<'_>) -> Line {
    let mut segs = Vec::new();
    let mut end = match block {
        Block::Table(_) if table.is_some() => {
            let mut o = Map::new();
            o.insert(
                "t".into(),
                Value::String(table.unwrap_or_default().to_owned()),
            );
            segs.push(Seg::Embed(Value::Object(o), Arc::new(Attrs::new())));
            Attrs::new()
        }
        Block::Paragraph(p) => {
            for (props, c) in para::flatten(p) {
                let a: Rc = Arc::new(run_attrs(&props));
                match c {
                    RunContent::Text(t) => {
                        // `\n` is the paragraph end in the shared text: escape it.
                        for (i, piece) in t.split('\n').enumerate() {
                            if i > 0 {
                                segs.push(Seg::Embed(
                                    content_embed(&RunContent::Text("\n".into())),
                                    a.clone(),
                                ));
                            }
                            if !piece.is_empty() {
                                match segs.last_mut() {
                                    Some(Seg::Text(prev, pa)) if *pa == a => prev.push_str(piece),
                                    _ => segs.push(Seg::Text(piece.to_owned(), a.clone())),
                                }
                            }
                        }
                    }
                    other => segs.push(Seg::Embed(content_embed(&other), a)),
                }
            }
            para_attrs(&p.props, ctx)
        }
        other => {
            let mut o = Map::new();
            o.insert(
                "b".into(),
                Value::String(serde_json::to_string(other).unwrap_or_default()),
            );
            segs.push(Seg::Embed(Value::Object(o), Arc::new(Attrs::new())));
            Attrs::new()
        }
    };
    if let Some(s) = sect {
        end.insert("sect".into(), Value::String(s.to_owned()));
    }
    Line {
        segs,
        end: Some(Arc::new(end)),
    }
}

/// Section properties without the blocks.
fn section_props(s: &Section) -> Section {
    Section {
        page: s.page,
        blocks: Vec::new(),
        columns: s.columns,
        headers: s.headers.clone(),
        footers: s.footers.clone(),
        title_page: s.title_page,
        break_type: s.break_type,
        page_number_start: s.page_number_start,
        footnote_pr: s.footnote_pr.clone(),
        endnote_pr: s.endnote_pr.clone(),
        change: s.change.clone(),
        src: None,
    }
}

/// Section properties shared on the section's last `\n`; header/footer
/// contents travel as stories (`hf:<section>:<kind>`).
fn section_json(s: &Section) -> String {
    let mut p = section_props(s);
    parts::strip_headers(&mut p);
    serde_json::to_string(&p).unwrap_or_default()
}

/// Model blocks in document order with the section JSON of each section's last block.
fn model_items(doc: &Document) -> Vec<(&Block, Option<String>)> {
    let mut v = Vec::new();
    for s in &doc.sections {
        let n = s.blocks.len();
        for (i, b) in s.blocks.iter().enumerate() {
            v.push((b, (i + 1 == n).then(|| section_json(s))));
        }
    }
    v
}

/// Global block index → (section, block in section).
fn locate(doc: &Document, mut g: usize) -> Option<(usize, usize)> {
    for (si, s) in doc.sections.iter().enumerate() {
        if g < s.blocks.len() {
            return Some((si, g));
        }
        g -= s.blocks.len();
    }
    None
}

fn global_of(doc: &Document, section: usize, block: usize) -> usize {
    doc.sections
        .iter()
        .take(section)
        .map(|s| s.blocks.len())
        .sum::<usize>()
        + block
}

// ---------------------------------------------------------------------------
// Delta building
// ---------------------------------------------------------------------------

#[derive(Default)]
struct DeltaBuilder {
    ops: Vec<TextOp>,
    retain: u32,
}

impl DeltaBuilder {
    fn flush(&mut self) {
        if self.retain > 0 {
            self.ops.push(TextOp::Retain {
                retain: self.retain,
                attributes: Map::new(),
            });
            self.retain = 0;
        }
    }

    fn retain(&mut self, n: u32, attributes: Map<String, Value>) {
        if n == 0 {
            return;
        }
        if attributes.is_empty() {
            self.retain += n;
            return;
        }
        self.flush();
        if let Some(TextOp::Retain {
            retain,
            attributes: a,
        }) = self.ops.last_mut()
        {
            if *a == attributes {
                *retain += n;
                return;
            }
        }
        self.ops.push(TextOp::Retain {
            retain: n,
            attributes,
        });
    }

    fn insert(&mut self, u: &Unit, a: &Attrs) {
        self.flush();
        let attributes = to_map(a);
        let text = match u {
            Unit::Ch(c) => Some(*c),
            Unit::End => Some('\n'),
            Unit::Embed(_) => None,
        };
        if let Some(c) = text {
            if let Some(TextOp::Insert {
                insert: Value::String(s),
                attributes: pa,
            }) = self.ops.last_mut()
            {
                if *pa == attributes {
                    s.push(c);
                    return;
                }
            }
            self.ops.push(TextOp::Insert {
                insert: Value::String(c.to_string()),
                attributes,
            });
        } else if let Unit::Embed(v) = u {
            self.ops.push(TextOp::Insert {
                insert: v.clone(),
                attributes,
            });
        }
    }

    fn delete(&mut self, n: u32) {
        if n == 0 {
            return;
        }
        self.flush();
        if let Some(TextOp::Delete { delete }) = self.ops.last_mut() {
            *delete += n;
            return;
        }
        self.ops.push(TextOp::Delete { delete: n });
    }

    fn finish(self) -> Vec<TextOp> {
        // A trailing plain retain is implicit.
        self.ops
    }
}

fn format_run(b: &mut DeltaBuilder, old: &[Cell], new: &[Cell]) {
    for (o, n) in old.iter().zip(new) {
        let d = if Arc::ptr_eq(&o.1, &n.1) {
            Map::new()
        } else {
            attr_diff(&o.1, &n.1)
        };
        b.retain(o.0.len16(), d);
    }
}

/// Minimal delta turning `old` into `new` (common prefix and suffix).
fn diff_cells(b: &mut DeltaBuilder, old: &[Cell], new: &[Cell]) {
    let n = old.len().min(new.len());
    let mut p = 0;
    while p < n && old[p].0 == new[p].0 {
        p += 1;
    }
    let mut s = 0;
    while s < n - p && old[old.len() - 1 - s].0 == new[new.len() - 1 - s].0 {
        s += 1;
    }
    format_run(b, &old[..p], &new[..p]);
    for (u, a) in &new[p..new.len() - s] {
        b.insert(u, a);
    }
    b.delete(old[p..old.len() - s].iter().map(|c| c.0.len16()).sum());
    format_run(b, &old[old.len() - s..], &new[new.len() - s..]);
}

/// Where a UTF-16 position goes after a delta (inserts at the position push it).
fn transform_index(mut idx: u32, delta: &[TextOp]) -> u32 {
    let mut cur = 0u32;
    for op in delta {
        match op {
            TextOp::Retain { retain, .. } => cur = cur.saturating_add(*retain),
            TextOp::Insert { insert, .. } => {
                let l = insert_len(insert);
                if cur <= idx {
                    idx = idx.saturating_add(l);
                }
                cur = cur.saturating_add(l);
            }
            TextOp::Delete { delete } => {
                if cur < idx {
                    idx -= (*delete).min(idx - cur);
                }
            }
        }
        if cur > idx {
            break;
        }
    }
    idx
}

fn insert_len(v: &Value) -> u32 {
    match v {
        Value::String(s) => s.encode_utf16().count() as u32,
        _ => 1,
    }
}

fn insert_cells(v: &Value, a: &Rc) -> Vec<Cell> {
    match v {
        Value::String(s) => s
            .chars()
            .map(|c| {
                if c == '\n' {
                    (Unit::End, a.clone())
                } else {
                    (Unit::Ch(c), a.clone())
                }
            })
            .collect(),
        other => vec![(Unit::Embed(other.clone()), a.clone())],
    }
}

/// Index of the cell at UTF-16 offset `off` (a split surrogate pair rounds down).
fn cell_at(cells: &[Cell], off: u32) -> usize {
    let mut acc = 0;
    for (i, c) in cells.iter().enumerate() {
        if acc >= off {
            return i;
        }
        acc += c.0.len16();
        if acc > off {
            return i;
        }
    }
    cells.len()
}

/// Number of cells from `i` covering `n` UTF-16 units.
fn cells_for(cells: &[Cell], i: usize, n: u32) -> usize {
    let mut acc = 0;
    let mut k = i;
    while k < cells.len() && acc < n {
        acc += cells[k].0.len16();
        k += 1;
    }
    k - i
}

// ---------------------------------------------------------------------------
// The binding
// ---------------------------------------------------------------------------

/// Shadow of the shared document: body lines, tables, stories and `meta`.
#[derive(Debug, Default)]
pub(crate) struct DocCollab {
    entries: Vec<Entry>,
    /// Late joiner waiting for the shared text (local edits are not shared).
    joining: bool,
    /// Numbering definitions of the opened file.
    base_nums: BTreeSet<i32>,
    /// Media parts every replica has (opened file + shared blobs).
    known_media: BTreeSet<String>,
    /// Document version the shadow was last compared at.
    version: u64,
    /// Replica client id and counter for ids of new tables, rows and columns.
    client: u64,
    counter: u64,
    tables: BTreeMap<String, TableShadow>,
    stories: BTreeMap<String, Story>,
    /// Shared `meta` entries of the document parts (styles, numbering, comments …).
    meta: BTreeMap<String, String>,
    seen: Seen,
}

/// Prefix/suffix of unchanged blocks: `None` when nothing changed.
fn region(entries: &[Entry], items: &[(&Block, Option<String>)]) -> Option<(usize, usize)> {
    let (no, nn) = (entries.len(), items.len());
    let same = |e: &Entry, it: &(&Block, Option<String>)| e.sect == it.1 && e.block == *it.0;
    let n = no.min(nn);
    let mut p = 0;
    while p < n && same(&entries[p], &items[p]) {
        p += 1;
    }
    let mut s = 0;
    while s < n - p && same(&entries[no - 1 - s], &items[nn - 1 - s]) {
        s += 1;
    }
    (p != no || p != nn).then_some((p, s))
}

impl DocCollab {
    fn new(doc: &Document, seed: bool, client: u64) -> DocCollab {
        DocCollab {
            joining: !seed,
            base_nums: doc.numbering.nums.iter().map(|n| n.id).collect(),
            known_media: doc.media.keys().cloned().collect(),
            client,
            ..DocCollab::default()
        }
    }

    /// Shadow of `doc` as the seeder publishes it (positions of a late joiner's caret).
    fn of(doc: &Document) -> DocCollab {
        let mut c = DocCollab::new(doc, true, 0);
        let _ = c.local_change(doc);
        c
    }

    fn total(&self) -> u32 {
        self.entries.iter().map(|e| e.len).sum()
    }

    /// Changes of the document since the last call (model vs shadow); from
    /// an empty shadow this is the whole document (the seeder's content).
    fn local_change(&mut self, doc: &Document) -> CollabChange {
        let mut out = CollabChange::default();
        if self.joining {
            return out;
        }
        let ctx = LineCtx {
            numbering: &doc.numbering,
            base_nums: &self.base_nums,
        };
        let (client, counter) = (self.client, &mut self.counter);
        let mut fresh = || {
            *counter += 1;
            format!("{client:x}.{counter}")
        };
        let items = model_items(doc);
        if let Some((p, s)) = region(&self.entries, &items) {
            let (no, nn) = (self.entries.len(), items.len());
            // Tables of the changed region keep their key in order.
            let mut old_tables: Vec<(String, Option<bo_model::Table>)> = self.entries[p..no - s]
                .iter()
                .filter_map(|e| {
                    let k = parts::table_key(&e.line)?;
                    let t = match &e.block {
                        Block::Table(t) => Some(t.clone()),
                        _ => None,
                    };
                    Some((k.to_owned(), t))
                })
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let mut fresh_entries = Vec::with_capacity(nn - s - p);
            for (b, sect) in &items[p..nn - s] {
                let key = if let Block::Table(t) = b {
                    let (key, old) = old_tables.pop().unwrap_or_else(|| (fresh(), None));
                    parts::TableDiff {
                        key: &key,
                        ctx: &ctx,
                    }
                    .run(
                        &mut self.tables,
                        &mut self.stories,
                        old.as_ref(),
                        t,
                        &mut fresh,
                        &mut out,
                    );
                    Some(key)
                } else {
                    None
                };
                let line = line_of(b, sect.as_deref(), key.as_deref(), &ctx);
                fresh_entries.push(Entry {
                    block: (*b).clone(),
                    len: line.len16(),
                    sect: sect.clone(),
                    line,
                });
            }
            for (key, _) in old_tables {
                parts::drop_table(&mut self.tables, &mut self.stories, &key, &mut out);
            }
            let mut old = Vec::new();
            for e in &self.entries[p..no - s] {
                e.line.explode(&mut old);
            }
            let mut new = Vec::new();
            for e in &fresh_entries {
                e.line.explode(&mut new);
            }
            let mut b = DeltaBuilder::default();
            b.retain(self.entries[..p].iter().map(|e| e.len).sum(), Map::new());
            diff_cells(&mut b, &old, &new);
            self.entries.splice(p..no - s, fresh_entries);
            let d = b.finish();
            if !d.is_empty() {
                out.body.push(d);
            }
        }
        parts::share_parts(
            doc,
            &self.base_nums,
            &mut self.seen,
            &mut self.meta,
            &mut self.tables,
            &mut self.stories,
            &ctx,
            &mut fresh,
            &mut out,
        );
        out
    }
}

/// Applies one remote delta to body or story lines; changed lines get blocks from `ctx`.
fn apply_entries(
    entries: &mut Vec<Entry>,
    delta: &[TextOp],
    ctx: &mut BuildCtx<'_>,
) -> Result<(), String> {
    let mut ops: &[TextOp] = delta;
    while let Some(TextOp::Retain { attributes, .. }) = ops.last() {
        if !attributes.is_empty() {
            break;
        }
        ops = &ops[..ops.len() - 1];
    }
    if ops.is_empty() {
        return Ok(());
    }
    let (start, rest) = match ops.first() {
        Some(TextOp::Retain { retain, attributes }) if attributes.is_empty() => {
            (*retain, &ops[1..])
        }
        _ => (0, ops),
    };
    let span: u32 = rest
        .iter()
        .map(|o| match o {
            TextOp::Retain { retain, .. } => *retain,
            TextOp::Delete { delete } => *delete,
            TextOp::Insert { .. } => 0,
        })
        .sum();
    let end = start.saturating_add(span);
    let total: u32 = entries.iter().map(|e| e.len).sum();
    if end > total {
        return Err(format!("delta covers {end} of {total}"));
    }
    let n = entries.len();
    let mut starts = Vec::with_capacity(n + 1);
    let mut acc = 0u32;
    for e in entries.iter() {
        starts.push(acc);
        acc += e.len;
    }
    starts.push(acc);
    // The line holding `start` (a boundary belongs to the next line; the
    // end of the text to an unterminated last line).
    let mut la = (0..n).find(|&k| starts[k + 1] > start).unwrap_or(n);
    if la == n && n > 0 && entries[n - 1].line.end.is_none() {
        la = n - 1;
    }
    let mut lb = la;
    while lb < n && (lb == la || starts[lb] < end) {
        lb += 1;
    }
    let mut cells = Vec::new();
    for e in &entries[la..lb] {
        e.line.explode(&mut cells);
    }
    let mut i = cell_at(&cells, start - starts[la]);
    for op in rest {
        match op {
            TextOp::Retain { retain, attributes } => {
                let k = cells_for(&cells, i, *retain);
                if !attributes.is_empty() {
                    for c in &mut cells[i..i + k] {
                        let mut a = (*c.1).clone();
                        for (key, v) in attributes {
                            if v.is_null() {
                                a.remove(key);
                            } else {
                                a.insert(key.clone(), norm(v));
                            }
                        }
                        c.1 = Arc::new(a);
                    }
                }
                i += k;
            }
            TextOp::Delete { delete } => {
                let k = cells_for(&cells, i, *delete);
                cells.drain(i..i + k);
            }
            TextOp::Insert { insert, attributes } => {
                let new = insert_cells(insert, &Arc::new(attrs_of(attributes)));
                let k = new.len();
                cells.splice(i..i, new);
                i += k;
            }
        }
    }
    // A removed paragraph end joins the next line.
    while lb < n && !matches!(cells.last(), Some((Unit::End, _))) {
        entries[lb].line.explode(&mut cells);
        lb += 1;
    }
    let fresh: Vec<Entry> = implode(cells)
        .into_iter()
        .map(|line| Entry {
            block: ctx.block(&line),
            sect: line.sect(),
            len: line.len16(),
            line,
        })
        .collect();
    entries.splice(la..lb, fresh);
    Ok(())
}

/// A position inside a cell of a shared table, by ids.
#[derive(Debug, Clone)]
struct CellPos {
    table: String,
    row: String,
    col: String,
    para: usize,
    offset: usize,
}

impl DocCollab {
    /// The cell (by table/row/column id) of a position in a top-level table cell.
    fn cell_pos(&self, doc: &Document, pos: Pos, paths: &[Vec<usize>]) -> Option<CellPos> {
        let path = paths.get(pos.para)?;
        let [sec, blk, row, cell, para] = path.as_slice() else {
            return None;
        };
        let e = self.entries.get(global_of(doc, *sec, *blk))?;
        let key = parts::table_key(&e.line)?;
        let sh = self.tables.get(key)?;
        let Block::Table(t) = &e.block else {
            return None;
        };
        let rid = parts::sorted(&sh.rows).get(*row)?.clone();
        let start = *parts::starts(t.rows.get(*row)?).get(*cell)?;
        let cid = parts::sorted(&sh.cols).get(start)?.clone();
        Some(CellPos {
            table: key.to_owned(),
            row: rid,
            col: cid,
            para: *para,
            offset: pos.offset,
        })
    }

    /// The model position of a [`CellPos`] (the cell may have moved or gone).
    fn pos_of_cell(&self, doc: &Document, cp: &CellPos, paths: &[Vec<usize>]) -> Option<Pos> {
        let g = self
            .entries
            .iter()
            .position(|e| parts::table_key(&e.line) == Some(cp.table.as_str()))?;
        let (sec, blk) = locate(doc, g)?;
        let Some(Block::Table(t)) = doc.sections.get(sec)?.blocks.get(blk) else {
            return None;
        };
        let sh = self.tables.get(&cp.table)?;
        let ri = parts::sorted(&sh.rows).iter().position(|r| *r == cp.row)?;
        let cols = parts::sorted(&sh.cols);
        let row = t.rows.get(ri)?;
        let ci = parts::starts(row)
            .iter()
            .position(|&g| cols.get(g) == Some(&cp.col))?;
        let n = row.cells.get(ci)?.blocks.len();
        let path = vec![sec, blk, ri, ci, cp.para.min(n.saturating_sub(1))];
        let id = paths.iter().position(|p| *p == path)?;
        let len = para_at(doc, &path).map_or(0, |p| {
            para::flatten(p).iter().map(|(_, c)| atom_len(c)).sum()
        });
        Some(Pos::new(id, cp.offset.min(len)))
    }

    /// Model ops making the document match the shadow.
    fn model_ops(&mut self, doc: &Document) -> Vec<Op> {
        if self.entries.is_empty() {
            return Vec::new();
        }
        let items = model_items(doc);
        let (ne, nm) = (self.entries.len(), items.len());
        let same = |e: &Entry, it: &(&Block, Option<String>)| e.sect == it.1 && e.block == *it.0;
        let n = ne.min(nm);
        let mut p = 0;
        while p < n && same(&self.entries[p], &items[p]) {
            p += 1;
        }
        let mut s = 0;
        while s < n - p && same(&self.entries[ne - 1 - s], &items[nm - 1 - s]) {
            s += 1;
        }
        if p == ne && p == nm {
            return Vec::new();
        }
        let old_marks: Vec<&Option<String>> = items[p..nm - s]
            .iter()
            .map(|i| &i.1)
            .filter(|x| x.is_some())
            .collect();
        let new_marks: Vec<&Option<String>> = self.entries[p..ne - s]
            .iter()
            .map(|e| &e.sect)
            .filter(|x| x.is_some())
            .collect();
        let marks_at_end = old_marks.len() == 1
            && items[nm - s - 1].1.is_some()
            && self.entries[ne - s - 1].sect.is_some();
        let within = old_marks == new_marks && (old_marks.is_empty() || marks_at_end) && p < nm;
        if within {
            if let Some((sec, bi)) = locate(doc, p) {
                let remove: Vec<Block> = items[p..nm - s].iter().map(|i| i.0.clone()).collect();
                let insert: Vec<Block> = self.entries[p..ne - s]
                    .iter()
                    .map(|e| e.block.clone())
                    .collect();
                return vec![Op::Blocks {
                    path: vec![sec],
                    index: bi,
                    remove,
                    insert,
                }];
            }
        }
        // The section structure changed: rebuild every section from the shadow.
        let mut sections: Vec<Section> = Vec::new();
        let mut blocks = Vec::new();
        let mut ordinal = 0;
        for e in &self.entries {
            blocks.push(e.block.clone());
            if let Some(js) = &e.sect {
                let mut sec = match doc.sections.get(ordinal) {
                    Some(old) if section_json(old) == *js => {
                        let mut s = old.clone();
                        s.blocks.clear();
                        s
                    }
                    _ => serde_json::from_str::<Section>(js).unwrap_or_else(|_| {
                        doc.sections.last().map(section_props).unwrap_or_default()
                    }),
                };
                sec.blocks = std::mem::take(&mut blocks);
                sections.push(sec);
                ordinal += 1;
            }
        }
        if !blocks.is_empty() {
            if let Some(last) = sections.last_mut() {
                last.blocks.append(&mut blocks);
            } else {
                let mut sec = doc.sections.last().map(section_props).unwrap_or_default();
                sec.blocks = blocks;
                sections.push(sec);
            }
        }
        vec![Op::Sections {
            index: 0,
            remove: doc.sections.clone(),
            insert: sections,
        }]
    }

    /// UTF-16 offset of a model position.
    fn index_of(&self, doc: &Document, pos: Pos, paths: &[Vec<usize>]) -> u32 {
        let Some(path) = paths.get(pos.para) else {
            return self.total();
        };
        let (Some(&sec), Some(&blk)) = (path.first(), path.get(1)) else {
            return 0;
        };
        let g = global_of(doc, sec, blk);
        let base: u32 = self.entries.iter().take(g).map(|e| e.len).sum();
        if path.len() > 2 {
            return base;
        }
        let Some(p) = para_at(doc, path) else {
            return base;
        };
        let mut u = 0u32;
        let mut rem = pos.offset;
        for (_, c) in para::flatten(p) {
            match c {
                RunContent::Text(t) => {
                    if rem <= t.len() {
                        let mut k = rem;
                        while k > 0 && !t.is_char_boundary(k) {
                            k -= 1;
                        }
                        return base + t[..k].encode_utf16().count() as u32;
                    }
                    u += t.encode_utf16().count() as u32;
                    rem -= t.len();
                }
                other => {
                    let l = atom_len(&other);
                    if rem == 0 || rem < l {
                        return base + u;
                    }
                    u += 1;
                    rem -= l;
                }
            }
        }
        base + u
    }

    /// Model position of a UTF-16 offset.
    fn pos_of(&self, doc: &Document, idx: u32, paths: &[Vec<usize>]) -> Pos {
        let mut acc = 0u32;
        let mut g = self.entries.len().saturating_sub(1);
        for (k, e) in self.entries.iter().enumerate() {
            if idx < acc + e.len {
                g = k;
                break;
            }
            acc += e.len;
        }
        if g < self.entries.len() && idx >= acc + self.entries[g].len {
            // Past the end: the last entry.
            acc = self.total() - self.entries[g].len;
        }
        let local = idx.saturating_sub(acc);
        let Some((sec, blk)) = locate(doc, g) else {
            return Pos::default();
        };
        let head = [sec, blk];
        let Some(id) = paths
            .iter()
            .position(|p| p.len() >= 2 && p[..2] >= head[..])
        else {
            return Pos::new(paths.len().saturating_sub(1), 0);
        };
        if paths[id].len() != 2 || paths[id][..2] != head[..] {
            return Pos::new(id, 0);
        }
        let Some(p) = para_at(doc, &paths[id]) else {
            return Pos::new(id, 0);
        };
        let mut u = 0u32;
        let mut bytes = 0usize;
        for (_, c) in para::flatten(p) {
            if u >= local {
                break;
            }
            match c {
                RunContent::Text(t) => {
                    for ch in t.chars() {
                        if u >= local {
                            break;
                        }
                        u += ch.len_utf16() as u32;
                        bytes += ch.len_utf8();
                    }
                }
                other => {
                    u += 1;
                    bytes += atom_len(&other);
                }
            }
        }
        Pos::new(id, bytes)
    }

    fn selection_of(&self, doc: &Document, sel: Selection) -> (u32, u32) {
        let paths = body_paragraphs(doc);
        (
            self.index_of(doc, sel.anchor, &paths),
            self.index_of(doc, sel.focus, &paths),
        )
    }
}

// ---------------------------------------------------------------------------
// Engine API
// ---------------------------------------------------------------------------

fn bad(e: impl std::fmt::Display) -> EngineError {
    EngineError::BadCommand(e.to_string())
}

impl Engine {
    /// Bind the open document to a collaboration session. The seeder
    /// (`seed`, first editor of the room) gets the whole document (body,
    /// tables, stories, definitions) as the change to publish; a late joiner
    /// gets nothing and replaces its document with the shared state on the
    /// first [`Engine::collab_apply`] (edits made before that are not shared).
    /// `client` is the replica's client id (ids of new tables, rows, columns).
    pub fn collab_start(&mut self, seed: bool, client: u64) -> crate::Result<CollabChange> {
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        // New comment / note ids differ between replicas (see Session::set_id_seed);
        // 2.. keeps clear of the note separator ids (§17.11.23).
        s.set_id_seed(Some(u16::try_from(client % 65_534).unwrap_or(0) + 2));
        let s = &*s;
        let mut c = DocCollab::new(s.doc(), seed, client);
        let mut change = c.local_change(s.doc());
        change.body.retain(|d| !d.is_empty());
        c.version = s.version();
        self.collab = Some(c);
        Ok(change)
    }

    /// [`Self::collab_start`] with the change as JSON.
    pub fn collab_start_json(&mut self, seed: bool, client: u64) -> crate::Result<String> {
        Ok(serde_json::to_string(&self.collab_start(seed, client)?).unwrap_or_default())
    }

    /// Leave the collaboration session.
    pub fn collab_stop(&mut self) {
        self.collab = None;
        if let Some(s) = self.session.as_mut() {
            s.set_id_seed(None);
        }
    }

    /// A collaboration session is running.
    pub fn collab_active(&self) -> bool {
        self.collab.is_some()
    }

    /// After a local command: the change to share and the selection.
    pub(crate) fn collab_after_edit(&mut self) -> Option<CollabOut> {
        let s = self.session.as_ref()?;
        let c = self.collab.as_mut()?;
        let doc = s.doc();
        let mut change = CollabChange::default();
        if s.version() != c.version {
            c.version = s.version();
            change = c.local_change(doc);
            if !c.joining {
                for (name, bytes) in &doc.media {
                    if c.known_media.insert(name.clone()) {
                        change
                            .blobs
                            .insert(name.clone(), Some(Blob::encode(None, bytes)));
                    }
                }
            }
        }
        Some(CollabOut {
            change,
            selection: c.selection_of(doc, s.selection()),
        })
    }

    /// Apply changes from the replica (`WasmCollab.takeRemote`): collaborators'
    /// edits and collaborative undo/redo. Not part of the local history.
    #[allow(clippy::too_many_lines)]
    pub fn collab_apply(&mut self, change: &CollabChange) -> crate::Result<EditResult> {
        let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        let c = self
            .collab
            .as_mut()
            .ok_or_else(|| bad("collaboration not started"))?;
        let doc = s.doc();
        let paths = body_paragraphs(doc);
        let sel = s.selection();
        // A late joiner's caret is measured in its own copy of the file (the
        // shared text replaces it) and must not be pushed by that first insert.
        let was_joining = c.joining;
        let (mut anchor, mut focus) = if was_joining {
            let own = DocCollab::of(doc);
            (
                own.index_of(doc, sel.anchor, &paths),
                own.index_of(doc, sel.focus, &paths),
            )
        } else {
            (
                c.index_of(doc, sel.anchor, &paths),
                c.index_of(doc, sel.focus, &paths),
            )
        };
        let (cell_anchor, cell_focus) = if was_joining {
            (None, None)
        } else {
            (
                c.cell_pos(doc, sel.anchor, &paths),
                c.cell_pos(doc, sel.focus, &paths),
            )
        };
        // Shared definitions and table structure.
        let mut touched: BTreeSet<String> = BTreeSet::new();
        let mut tables_hit: BTreeSet<String> = BTreeSet::new();
        for (k, v) in &change.meta {
            match v {
                Some(j) => c.meta.insert(k.clone(), j.clone()),
                None => c.meta.remove(k),
            };
            if let Some((kind, t, rest)) = parts::split_table_key(k) {
                c.tables
                    .entry(t.to_owned())
                    .or_default()
                    .set(kind, rest, v.as_ref());
                tables_hit.insert(t.to_owned());
            }
            touched.insert(k.clone());
        }
        let defs = parts::rebuild_parts(doc, &c.meta, &c.stories, &touched, was_joining);
        let mut ctx = BuildCtx {
            numbering: defs.numbering.unwrap_or_else(|| doc.numbering.clone()),
            styles: defs.styles.unwrap_or_else(|| doc.styles.clone()),
            base_nums: &c.base_nums,
            numbering_changed: false,
            styles_changed: false,
            tables: None,
            stories: None,
        };
        ctx.numbering_changed = ctx.numbering != doc.numbering;
        ctx.styles_changed = ctx.styles != doc.styles;
        let mut error = None;
        // Nested texts (cells, notes, comments, headers).
        for (k, v) in &change.texts {
            if let Err(e) = parts::apply_text(&mut c.stories, k, v.as_ref(), &mut ctx) {
                error = Some(e);
            }
            if let Some(t) = parts::story_table(k) {
                tables_hit.insert(t.to_owned());
            }
            touched.insert(k.clone());
        }
        c.tables.retain(|_, t| !t.is_empty());
        // Tables nested in cells: their blocks, and the tables around changed ones.
        parts::rebuild_nested(&c.tables, &mut c.stories, &mut tables_hit, &mut touched);
        ctx.tables = Some(&c.tables);
        ctx.stories = Some(&c.stories);
        // The body.
        let mut entries = std::mem::take(&mut c.entries);
        for d in &change.body {
            if let Err(e) = apply_entries(&mut entries, d, &mut ctx) {
                error = Some(e);
                break;
            }
            if !was_joining {
                anchor = transform_index(anchor, d);
                focus = transform_index(focus, d);
            }
        }
        // Tables whose structure or cells changed are rebuilt.
        for e in &mut entries {
            if let Some(k) = parts::table_key(&e.line) {
                if was_joining || tables_hit.contains(k) {
                    e.block = ctx.block(&e.line);
                }
            }
        }
        if was_joining {
            let total: u32 = entries.iter().map(|e| e.len).sum();
            anchor = anchor.min(total);
            focus = focus.min(total);
        }
        let joined = !entries.is_empty();
        if c.joining && !joined {
            c.entries = entries;
            return self.collab_result(None, error);
        }
        if entries.is_empty() {
            let line = Line {
                segs: Vec::new(),
                end: None,
            };
            entries.push(Entry {
                block: ctx.block(&line),
                sect: None,
                len: 0,
                line,
            });
        }
        let content = parts::rebuild_parts(doc, &c.meta, &c.stories, &touched, was_joining);
        let mut ops = Vec::new();
        if ctx.numbering_changed {
            ops.push(Op::Numbering {
                old: Box::new(doc.numbering.clone()),
                new: Box::new(ctx.numbering),
            });
        }
        if ctx.styles_changed {
            ops.push(Op::Styles {
                old: Box::new(doc.styles.clone()),
                new: Box::new(ctx.styles),
            });
        }
        if let Some(cm) = content.comments.filter(|cm| *cm != doc.comments) {
            ops.push(Op::Comments {
                old: doc.comments.clone(),
                new: cm,
            });
        }
        for (endnote, notes) in [(false, content.footnotes), (true, content.endnotes)] {
            let cur = if endnote {
                &doc.endnotes
            } else {
                &doc.footnotes
            };
            if let Some(n) = notes.filter(|n| n != cur) {
                ops.push(Op::Notes {
                    endnote,
                    old: cur.clone(),
                    new: n,
                });
            }
        }
        if let Some(eo) = content.eo.filter(|eo| *eo != doc.even_and_odd_headers) {
            ops.push(Op::EvenAndOddHeaders {
                old: doc.even_and_odd_headers,
                new: eo,
            });
        }
        if let Some(cn) = content.notice.filter(|cn| *cn != doc.continuation_notice) {
            ops.push(Op::ContinuationNotice {
                old: doc.continuation_notice.clone(),
                new: cn,
            });
        }
        for (name, v) in &change.blobs {
            let Some((_, bytes)) = v.as_deref().and_then(Blob::decode) else {
                continue;
            };
            c.known_media.insert(name.clone());
            if !doc.media.contains_key(name) {
                ops.push(Op::Media {
                    name: name.clone(),
                    old: None,
                    new: Some(Arc::new(bytes)),
                });
            }
        }
        let mut tmp = DocCollab {
            entries,
            ..DocCollab::default()
        };
        ops.extend(tmp.model_ops(doc));
        c.entries = tmp.entries;
        c.joining = false;
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let keep = s.selection();
        if let Err(e) = s.apply_external(ops, keep) {
            // Cannot happen while the shadow matches the model; the document is unchanged.
            error = Some(e.to_string());
        }
        // Header/footer contents (after the sections are in place).
        if let Some(sections) = parts::rebuilt_headers(s.doc(), &c.stories, &touched, true) {
            let op = Op::Sections {
                index: 0,
                remove: s.doc().sections.clone(),
                insert: sections,
            };
            let keep = s.selection();
            if let Err(e) = s.apply_external(vec![op], keep) {
                error = Some(e.to_string());
            }
        }
        // Positions in the new text.
        let c = self.collab.as_mut().ok_or(EngineError::NoDocument)?;
        let doc = s.doc();
        c.seen.update(doc);
        let paths = body_paragraphs(doc);
        // A caret in a table cell stays in that cell (found again by row/column id).
        let place = |cp: &Option<CellPos>, idx: u32| {
            cp.as_ref()
                .and_then(|cp| c.pos_of_cell(doc, cp, &paths))
                .unwrap_or_else(|| c.pos_of(doc, idx, &paths))
        };
        let selection = Selection {
            anchor: place(&cell_anchor, anchor),
            focus: place(&cell_focus, focus),
        };
        s.set_selection(selection);
        c.version = s.version();
        self.goal_x = None;
        let dirty = self.relayout()?;
        self.collab_result(Some(dirty), error)
    }
    fn collab_result(
        &mut self,
        dirty: Option<Vec<usize>>,
        error: Option<String>,
    ) -> crate::Result<EditResult> {
        let changed = dirty.is_some();
        let mut r = self.edit_result(dirty.unwrap_or_default(), (None, None, None), error)?;
        if changed {
            if let Some(s) = self.session.as_ref() {
                r.review = Some(crate::review::review_state(s, self.revision_view()));
            }
        }
        r.collab = self.collab_after_edit();
        Ok(r)
    }

    /// [`Self::collab_apply`] with JSON in and out.
    pub fn collab_apply_json(&mut self, json: &str) -> crate::Result<String> {
        let change: CollabChange = serde_json::from_str(json).map_err(bad)?;
        let r = self.collab_apply(&change)?;
        Ok(serde_json::to_string(&r).unwrap_or_default())
    }

    /// Geometry of collaborators' selections given as UTF-16 `(anchor, head)`.
    pub fn collab_peers(&self, sels: &[(u32, u32)]) -> crate::Result<Vec<PeerRects>> {
        let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        let layout = self.layout.document();
        let c = self
            .collab
            .as_ref()
            .ok_or_else(|| bad("collaboration not started"))?;
        let doc = s.doc();
        let paths = body_paragraphs(doc);
        let total = c.total();
        Ok(sels
            .iter()
            .map(|&(a, h)| {
                let sel = Selection {
                    anchor: s.clamp(c.pos_of(doc, a.min(total), &paths)),
                    focus: s.clamp(c.pos_of(doc, h.min(total), &paths)),
                };
                PeerRects {
                    caret: crate::nav::caret_rect(layout, sel.focus),
                    rects: if sel.is_collapsed() {
                        Vec::new()
                    } else {
                        crate::nav::selection_rects(layout, sel)
                    },
                }
            })
            .collect())
    }

    /// [`Self::collab_peers`] with JSON `[[anchor, head], …]` in and out.
    pub fn collab_peers_json(&self, json: &str) -> crate::Result<String> {
        let sels: Vec<(u32, u32)> = serde_json::from_str(json).map_err(bad)?;
        Ok(serde_json::to_string(&self.collab_peers(&sels)?).unwrap_or_default())
    }

    /// The shared text as the engine sees it: `(text, attributes)` runs with
    /// embeds as `"\u{FFFC}"` (tests and diagnostics).
    pub fn collab_runs(&self) -> Vec<(String, Attrs)> {
        let mut out: Vec<(String, Attrs)> = Vec::new();
        let Some(c) = self.collab.as_ref() else {
            return out;
        };
        let mut cells = Vec::new();
        for e in &c.entries {
            e.line.explode(&mut cells);
        }
        for (u, a) in cells {
            let ch = match u {
                Unit::Ch(c) => c,
                Unit::Embed(_) => '\u{FFFC}',
                Unit::End => '\n',
            };
            match out.last_mut() {
                Some((t, pa)) if *pa == *a => t.push(ch),
                _ => out.push((ch.to_string(), (*a).clone())),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(s: &str) -> Vec<Cell> {
        let a = Arc::new(Attrs::new());
        s.chars()
            .map(|c| {
                if c == '\n' {
                    (Unit::End, a.clone())
                } else {
                    (Unit::Ch(c), a.clone())
                }
            })
            .collect()
    }

    #[test]
    fn minimal_text_delta() {
        let mut b = DeltaBuilder::default();
        diff_cells(&mut b, &cells("abc\n"), &cells("abXc\n"));
        let ops = serde_json::to_value(b.finish()).unwrap();
        assert_eq!(ops, serde_json::json!([{"retain": 2}, {"insert": "X"}]));
        let mut b = DeltaBuilder::default();
        diff_cells(&mut b, &cells("ab\ncd\n"), &cells("abcd\n"));
        let ops = serde_json::to_value(b.finish()).unwrap();
        assert_eq!(ops, serde_json::json!([{"retain": 2}, {"delete": 1}]));
    }

    #[test]
    fn positions_follow_remote_edits() {
        let d = vec![
            TextOp::Retain {
                retain: 2,
                attributes: Map::new(),
            },
            TextOp::Insert {
                insert: Value::String("xyz".into()),
                attributes: Map::new(),
            },
            TextOp::Delete { delete: 1 },
        ];
        assert_eq!(transform_index(1, &d), 1);
        assert_eq!(transform_index(2, &d), 5);
        assert_eq!(transform_index(4, &d), 6);
    }

    #[test]
    fn run_attributes_round_trip() {
        let p = RunProps {
            bold: Some(true),
            italic: Some(false),
            size: Some(HalfPoints(28)),
            color: Color::from_hex("C00000"),
            font: Some("Times New Roman".into()),
            caps: Some(true),
            ..RunProps::default()
        };
        let a = run_attrs(&p);
        assert_eq!(a["size"], Value::from(28));
        assert!(a.contains_key("r"));
        assert_eq!(run_props(&a), p);
        // Floats from Yrs normalise to the same attributes.
        let mut m = to_map(&a);
        m.insert("size".into(), serde_json::json!(28.0));
        assert_eq!(attrs_of(&m), a);
    }
}
