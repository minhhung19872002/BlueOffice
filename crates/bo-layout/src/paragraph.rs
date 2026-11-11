//! Paragraph layout: itemization, shaping, line breaking, tabs, alignment, line height.
//! Output is position-independent (relative to the content box) and cacheable.

use crate::blocks::Ctx;
use crate::drawing::{shape_drawing, Drawing, Float};
use crate::style::{para_style_run, revision_style, run_props, run_style, slot_of, RunStyle};
use crate::types::{Fragment, GlyphPos, NoteKey};
use crate::LayoutError;
use crate::RevisionView;
use bo_model::index::para_index;
use bo_model::{
    Alignment, Borders, Color, Document, Image, LineSpacing, ParaProps, Paragraph, RunContent,
    RunProps, TabAlign, TabLeader, TabStop, Twips, WrapText,
};
use bo_text::{FaceId, FontManager, ShapedGlyph};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// A [`Fragment`] being built (its text and glyphs still growing).
struct Draft {
    face: FaceId,
    size: f32,
    x: f32,
    y: f32,
    width: f32,
    color: Color,
    underline: bo_model::Underline,
    strike: bool,
    double_strike: bool,
    text: String,
    glyphs: Vec<GlyphPos>,
    link: Option<String>,
    fx: Option<Arc<crate::charfx::TextFx>>,
}

impl Draft {
    fn finish(self) -> Fragment {
        Fragment {
            face: self.face,
            size: self.size,
            x: self.x,
            y: self.y,
            width: self.width,
            color: self.color,
            underline: self.underline,
            strike: self.strike,
            double_strike: self.double_strike,
            text: self.text.into(),
            glyphs: self.glyphs.into(),
            link: self.link,
            fx: self.fx,
        }
    }
}

/// Area kept free of text by a square/tight-wrapped picture (ECMA-376 Part 1,
/// §20.4.2.17 wrapSquare, §20.4.2.20 wrapTight), relative to the paragraph: y from
/// the top of its first line, x from the left of its content box. Distances from
/// text (`distT`…) are included.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WrapZone {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
    /// Sides text may flow on (§20.4.3.7 `ST_WrapText`).
    pub side: WrapText,
    /// Tight wrap polygon in the same coordinates: the area kept clear on a line
    /// is the polygon's extent at that line's height (§20.4.2.20).
    pub outline: Option<crate::flow::Outline>,
}

/// Narrowest text segment beside a wrapped picture (narrower gaps stay empty).
pub(crate) const MIN_SEGMENT: f32 = 18.0;

/// Text segments `(start x, width)` of a line spanning `y..y + h` between `x0` and
/// `x1`, left to right, around the zones it overlaps.
fn segments(zones: &[WrapZone], x0: f32, x1: f32, y: f32, h: f32) -> Vec<(f32, f32)> {
    let mut segs = vec![(x0, x1)];
    for z in zones {
        if y >= z.bottom || y + h <= z.top {
            continue;
        }
        let (zl, zr) = match &z.outline {
            Some(o) => match o.extent(y, y + h) {
                Some(e) => e,
                None => continue,
            },
            None => (z.left, z.right),
        };
        let mut next = Vec::with_capacity(segs.len() + 1);
        for &(a, b) in &segs {
            if zr <= a || zl >= b {
                next.push((a, b));
                continue;
            }
            let left = (a, zl.min(b));
            let right = (zr.max(a), b);
            let wl = left.1 - left.0;
            let wr = right.1 - right.0;
            match z.side {
                WrapText::BothSides => {
                    next.push(left);
                    next.push(right);
                }
                WrapText::Left => next.push(left),
                WrapText::Right => next.push(right),
                WrapText::Largest => next.push(if wr > wl { right } else { left }),
            }
        }
        segs = next;
    }
    let out: Vec<(f32, f32)> = segs
        .into_iter()
        .filter(|(a, b)| b - a >= MIN_SEGMENT)
        .map(|(a, b)| (a, (b - a).max(1.0)))
        .collect();
    if out.is_empty() {
        // No room beside the picture: the flow moves such lines below it.
        vec![(x0, (x1 - x0).max(1.0))]
    } else {
        out
    }
}

/// Height of a line made of `pieces` (same rules as the positioning pass, §17.3.1.33).
fn row_height(
    pieces: &[Piece],
    styles: &[RunStyle],
    fonts: &FontManager,
    mark: &RunStyle,
    spacing: LineSpacing,
) -> f32 {
    let (mut ascent, mut descent, mut gap) = (0.0f32, 0.0f32, 0.0f32);
    let mut metric = |face: FaceId, size: f32, shift: f32| {
        let info = fonts.info(face);
        let scale = size / f32::from(info.units_per_em);
        ascent = ascent.max(f32::from(info.ascender) * scale - shift.min(0.0));
        descent = descent.max(-f32::from(info.descender) * scale + shift.max(0.0));
        gap = gap.max(f32::from(info.line_gap) * scale);
    };
    let mut obj = 0.0f32;
    let mut obj_depth = 0.0f32;
    for p in pieces {
        if p.kind == Kind::Object && p.image.is_some() {
            obj = obj.max(p.obj_h);
            obj_depth = obj_depth.max(p.obj_d);
        } else {
            let st = &styles[p.style];
            metric(p.face, st.size, st.shift);
        }
    }
    if pieces
        .iter()
        .all(|p| !matches!(p.kind, Kind::Text | Kind::Space | Kind::Object))
    {
        metric(mark.face, mark.size, 0.0);
    }
    let natural = ascent.max(obj) + descent.max(obj_depth) + gap;
    match spacing {
        LineSpacing::Auto(v) => natural * v as f32 / 240.0,
        LineSpacing::Exact(t) => t.to_pt(),
        LineSpacing::AtLeast(t) => natural.max(t.to_pt()),
    }
}

const LINE_SEP: char = '\u{2028}';
const PAGE_SEP: char = '\u{000C}';
const COL_SEP: char = '\u{000B}';

/// Values of page-dependent fields.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FieldContext {
    /// 1-based number of the current page (PAGE).
    pub page: u32,
    /// Total number of pages (NUMPAGES).
    pub pages: u32,
    /// Number of pages in the current section (SECTIONPAGES).
    pub section_pages: u32,
    /// Show field codes instead of results.
    pub codes: bool,
}

/// Inputs of a paragraph layout.
pub(crate) struct ParaInput<'a> {
    pub doc: &'a Document,
    pub para: &'a Paragraph,
    /// Effective paragraph properties.
    pub props: ParaProps,
    /// List label (text, level run props, suffix).
    pub label: Option<(String, RunProps, String)>,
    pub width: f32,
    pub fields: FieldContext,
    pub default_tab: f32,
    /// Note numbering: (endnote?, id) → displayed number.
    pub note_numbers: &'a HashMap<NoteKey, String>,
    /// Number shown by a `footnoteRef` mark inside a note's own text.
    pub note_mark: Option<&'a str>,
    /// Square/tight-wrapped pictures the lines flow around (empty: none).
    pub wrap: &'a [WrapZone],
    /// How tracked changes are shown.
    pub view: RevisionView,
    /// Markup filter of the all-markup view.
    pub marks: crate::MarkFilter,
}

/// Cache of laid-out paragraphs (see crate docs, "Incremental layout").
#[derive(Default)]
pub struct LayoutCache {
    map: HashMap<u64, Arc<ParaLayout>>,
    hits: u64,
    misses: u64,
}

impl LayoutCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }
    /// (hits, misses) since creation.
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
    /// Drop everything (e.g. after fonts change, or when the document closes).
    pub fn clear(&mut self) {
        self.map = HashMap::new();
    }
    /// Number of cached paragraph layouts (memory diagnostics).
    pub fn len(&self) -> usize {
        self.map.len()
    }
    /// True when nothing is cached.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// An inline picture positioned in a line (bottom on the baseline).
#[derive(Debug, Clone)]
pub(crate) struct RelObject {
    pub x: f32,
    pub w: f32,
    pub h: f32,
    pub image: Image,
    /// Shape / text box drawn in the object box.
    pub drawing: Option<Arc<Drawing>>,
    /// Tracked insertion/deletion of the picture (§17.13.5.18 / §17.13.5.14 on its run).
    pub revision: Option<(Color, bool)>,
}

/// Lines of one paragraph, positioned relative to the content box.
#[derive(Debug)]
pub(crate) struct ParaLayout {
    pub lines: Vec<RelLine>,
    pub space_before: f32,
    pub space_after: f32,
    pub page_break_before: bool,
    pub keep_next: bool,
    pub keep_lines: bool,
    pub widow_control: bool,
    pub contextual_spacing: bool,
    pub style: Option<String>,
    pub shading: Option<Color>,
    pub borders: Option<Borders>,
    pub indent_left: f32,
    pub indent_right: f32,
    /// Anchored (floating) pictures of this paragraph.
    pub floats: Vec<Float>,
    /// Drop cap frame (§17.3.1.11 framePr `w:dropCap`).
    pub frame: Option<bo_model::FrameProps>,
    /// The lines are counted by line numbering (§17.3.1.35 suppressLineNumbers off).
    pub numbered: bool,
}

/// Automatic hyphenation of a paragraph (ECMA-376 Part 1, §17.15.1.10).
struct Hyph {
    /// §17.15.1.52 hyphenationZone (points).
    zone: f32,
    /// §17.15.1.21 consecutiveHyphenLimit (0 = none).
    limit: usize,
    /// §17.15.1.40 doNotHyphenateCaps.
    no_caps: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RelLine {
    pub height: f32,
    /// Baseline offset from line top.
    pub baseline: f32,
    /// x relative to content left, y = 0 (baseline-relative glyph offsets).
    pub fragments: Vec<Fragment>,
    pub carets: crate::types::Carets,
    pub objects: Vec<RelObject>,
    /// Highlight rectangles: (x, w, color) spanning ascent..descent.
    pub highlights: Vec<(f32, f32, Color)>,
    /// Character borders (§17.3.2.4 bdr): (x, w, border) spanning ascent..descent.
    pub borders: Vec<(f32, f32, bo_model::Border)>,
    pub ascent: f32,
    pub descent: f32,
    pub start: u32,
    pub end: u32,
    pub page_break_after: bool,
    pub column_break_after: bool,
    /// Notes referenced on this line.
    pub notes: Vec<NoteKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Space,
    Tab,
    LineBreak,
    PageBreak,
    ColumnBreak,
    Object,
}

#[derive(Debug, Clone)]
struct Piece {
    style: usize,
    face: FaceId,
    kind: Kind,
    /// Display text (after caps transform).
    text: String,
    glyphs: Arc<[ShapedGlyph]>,
    width: f32,
    /// Original byte offsets of each display char start, plus the end offset.
    offsets: Vec<u32>,
    /// Label pieces have no caret positions.
    virtual_: bool,
    /// Picture (boxed: pieces are kept per line, most of them text).
    image: Option<Box<Image>>,
    drawing: Option<Arc<Drawing>>,
    /// Object height above baseline (pictures).
    obj_h: f32,
    /// Object depth below the baseline (equations).
    obj_d: f32,
    note: Option<NoteKey>,
    /// Bidirectional embedding level (UAX #9; odd = right to left).
    level: u8,
}

impl Piece {
    fn start(&self) -> u32 {
        self.offsets.first().copied().unwrap_or(0)
    }
    fn end(&self) -> u32 {
        self.offsets.last().copied().unwrap_or(0)
    }
    /// The other fields, without the text, glyphs and offsets (copied separately).
    fn clone_shallow(&self) -> Self {
        Self {
            style: self.style,
            face: self.face,
            kind: self.kind,
            text: String::new(),
            glyphs: Arc::from(Vec::new()),
            width: self.width,
            offsets: Vec::new(),
            virtual_: self.virtual_,
            image: self.image.clone(),
            drawing: self.drawing.clone(),
            obj_h: self.obj_h,
            obj_d: self.obj_d,
            note: self.note,
            level: self.level,
        }
    }
}

/// Longest run of characters without a break opportunity laid out as one word. Longer
/// runs (no natural language has such words; hostile or generated files do) are cut
/// into chunks of [`CHUNK_CHARS`] characters at grapheme boundaries, and an overflowing
/// chunk is cut where the line ends instead of being split into one word per grapheme:
/// the memory and time of laying out such a run stay linear and small.
const MAX_WORD_CHARS: usize = 4096;
/// Characters per chunk of an over-long run (a few lines' worth: cutting a chunk at a
/// line end copies the rest of it).
const CHUNK_CHARS: usize = 256;

/// Whether a word is a chunk of an over-long run (see [`MAX_WORD_CHARS`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chunk {
    /// An ordinary word.
    No,
    /// The first chunk of a run: placed like a word.
    First,
    /// A following chunk: continues the line the previous chunk ended on.
    Next,
}

#[derive(Debug, Clone)]
struct Word {
    pieces: Vec<Piece>,
    chunk: Chunk,
    /// A chunk not shaped yet: its byte range of the index text (shaped when the line
    /// breaker reaches it, so that one chunk at a time holds glyphs).
    lazy: Option<(usize, usize)>,
}

impl Word {
    fn new(pieces: Vec<Piece>) -> Self {
        Self {
            pieces,
            chunk: Chunk::No,
            lazy: None,
        }
    }
    fn brk(&self) -> Option<Kind> {
        self.pieces
            .iter()
            .rev()
            .find(|p| {
                matches!(
                    p.kind,
                    Kind::LineBreak | Kind::PageBreak | Kind::ColumnBreak
                )
            })
            .map(|p| p.kind)
    }
    fn char_count(&self) -> usize {
        self.pieces
            .iter()
            .filter(|p| p.kind == Kind::Text)
            .map(|p| p.text.chars().count())
            .sum()
    }
}

fn hash_inputs(inp: &ParaInput<'_>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    inp.para.props.hash(&mut h);
    inp.para.runs.hash(&mut h);
    // Signature lines leave their signed state out of their hash (display only);
    // it changes their drawing all the same.
    for c in inp.para.runs.iter().flat_map(|r| &r.content) {
        if let RunContent::Image(img) = c {
            if let Some(l) = img
                .shape
                .as_ref()
                .and_then(|s| s.art.as_ref())
                .and_then(|a| a.signature_line.as_ref())
            {
                l.signed.hash(&mut h);
            }
        }
    }
    inp.props.hash(&mut h);
    inp.label.hash(&mut h);
    inp.width.to_bits().hash(&mut h);
    inp.default_tab.to_bits().hash(&mut h);
    inp.doc.default_run.hash(&mut h);
    inp.doc.theme_fonts.hash(&mut h);
    inp.doc.hyphenation.hash(&mut h);
    // Run properties coming from the style definitions (ECMA-376 Part 1,
    // §17.7.2 style hierarchy): the paragraph style chain and the character
    // styles of the runs. The paragraph and runs above only name the styles, so
    // a changed definition (another document, undo, a collaborator's edit) must
    // not be served from the cache.
    crate::style::para_style_run(inp.doc, &inp.props).hash(&mut h);
    let mut char_styles: Vec<&str> = inp
        .para
        .runs
        .iter()
        .filter_map(|r| r.props.style.as_deref())
        .collect();
    char_styles.sort_unstable();
    char_styles.dedup();
    for id in char_styles {
        inp.doc.styles.resolve_character(id).hash(&mut h);
    }
    inp.note_mark.hash(&mut h);
    inp.view.hash(&mut h);
    inp.marks.hash(&mut h);
    for z in inp.wrap {
        for v in [z.top, z.bottom, z.left, z.right] {
            v.to_bits().hash(&mut h);
        }
        z.side.hash(&mut h);
        if let Some(o) = &z.outline {
            for &(x, y) in o.points.iter() {
                x.to_bits().hash(&mut h);
                y.to_bits().hash(&mut h);
            }
            o.pad.0.to_bits().hash(&mut h);
            o.pad.1.to_bits().hash(&mut h);
        }
    }
    // Page-dependent fields and note numbers only matter when present: the numbers
    // of the notes this paragraph references (hashing the whole note map made the
    // layout quadratic in the number of notes). Drawings (text boxes) may reference
    // notes of their own, so they hash every number.
    let mut fields = false;
    let mut drawings = false;
    for c in inp.para.runs.iter().flat_map(|r| &r.content) {
        match c {
            RunContent::Field { .. } => fields = true,
            RunContent::FootnoteRef(id) | RunContent::EndnoteRef(id) => {
                fields = true;
                let key = (matches!(c, RunContent::EndnoteRef(_)), *id);
                inp.note_numbers.get(&key).hash(&mut h);
            }
            RunContent::Image(img) if img.shape.is_some() => drawings = true,
            _ => {}
        }
    }
    if fields || drawings {
        inp.fields.hash(&mut h);
    }
    if drawings {
        let mut notes: Vec<_> = inp.note_numbers.iter().collect();
        notes.sort();
        notes.hash(&mut h);
    }
    h.finish()
}

pub(crate) fn layout_paragraph(
    inp: &ParaInput<'_>,
    fonts: &mut FontManager,
    cache: &mut LayoutCache,
) -> Result<Arc<ParaLayout>, LayoutError> {
    let key = hash_inputs(inp);
    if let Some(l) = cache.map.get(&key) {
        cache.hits += 1;
        return Ok(l.clone());
    }
    cache.misses += 1;
    let l = Arc::new(layout_uncached(inp, fonts, cache)?);
    if cache.map.len() > 100_000 {
        cache.map.clear();
    }
    cache.map.insert(key, l.clone());
    Ok(l)
}

/// Shading of form field results (light grey, as the classic format shows them unless
/// the doNotShadeFormData setting of §17.15.1 is on).
const FORM_SHADING: Color = Color {
    r: 0xD9,
    g: 0xD9,
    b: 0xD9,
};

/// Colour of the placeholder text of a content control (§17.5.2.39 showingPlcHdr).
const PLACEHOLDER_GREY: Color = Color {
    r: 0x80,
    g: 0x80,
    b: 0x80,
};

fn field_text(instr: &str, cached: &str, f: FieldContext) -> String {
    if f.codes {
        return format!("{{ {} }}", instr.trim());
    }
    let kind = instr
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    // The general switches apply (§17.16.4: `PAGE \* ROMAN`, `\# "00"`).
    let n = |v: u32| {
        bo_model::field::format(
            &bo_model::field::FieldValue::Number(f64::from(v)),
            instr,
            true,
        )
    };
    match kind.as_str() {
        "PAGE" if f.page > 0 => n(f.page),
        "NUMPAGES" if f.pages > 0 => n(f.pages),
        "SECTIONPAGES" if f.section_pages > 0 => n(f.section_pages),
        _ => cached.to_owned(),
    }
}

/// Embedding level of every byte of the paragraph text (UAX #9 via `unicode-bidi`),
/// base level 1 for a `w:bidi` paragraph (§17.3.1.6); `None` when the paragraph is
/// left to right with no right-to-left characters (nothing to reorder).
fn bidi_levels(text: &str, rtl: bool) -> Option<Vec<u8>> {
    use unicode_bidi::{BidiClass, BidiInfo, Level};
    if !rtl
        && !text.chars().any(|c| {
            matches!(
                unicode_bidi::bidi_class(c),
                BidiClass::R | BidiClass::AL | BidiClass::RLE | BidiClass::RLO | BidiClass::RLI
            )
        })
    {
        return None;
    }
    let base = if rtl { Level::rtl() } else { Level::ltr() };
    let info = BidiInfo::new(text, Some(base));
    let mut out: Vec<u8> = info
        .levels
        .iter()
        .map(unicode_bidi::Level::number)
        .collect();
    // L1: trailing whitespace goes back to the paragraph level.
    for (i, c) in text.char_indices().rev() {
        if !c.is_whitespace() {
            break;
        }
        for v in &mut out[i..i + c.len_utf8()] {
            *v = base.number();
        }
    }
    Some(out)
}

/// Split pieces whose characters have different embedding levels (re-shaped per
/// level run) and record each piece's level.
fn split_by_level(
    pieces: Vec<Piece>,
    levels: &[u8],
    styles: &[RunStyle],
    fonts: &mut FontManager,
) -> Vec<Piece> {
    let lv = |o: u32| levels.get(o as usize).copied().unwrap_or(0);
    let mut out = Vec::with_capacity(pieces.len());
    for mut p in pieces {
        let shaped_text = matches!(p.kind, Kind::Text | Kind::Space)
            && !p.virtual_
            && p.offsets.len() == p.text.chars().count() + 1;
        if !shaped_text {
            p.level = lv(p.start());
            out.push(p);
            continue;
        }
        let chars: Vec<(usize, char)> = p.text.char_indices().collect();
        let mut k = 0;
        while k < chars.len() {
            let l = lv(p.offsets[k]);
            let mut j = k + 1;
            while j < chars.len() && lv(p.offsets[j]) == l {
                j += 1;
            }
            if k == 0 && j == chars.len() {
                p.level = l;
                out.push(p);
                break;
            }
            let b0 = chars[k].0;
            let b1 = chars.get(j).map_or(p.text.len(), |c| c.0);
            let sub = &p.text[b0..b1];
            let st = &styles[p.style];
            let glyphs = shape_run(fonts, p.face, sub, st);
            let upem = f32::from(fonts.info(p.face).units_per_em);
            let width = glyphs.iter().map(|g| i64::from(g.x_advance)).sum::<i64>() as f32 * st.size
                / upem
                + st.spacing * glyphs.len() as f32;
            out.push(Piece {
                text: sub.to_owned(),
                glyphs,
                width,
                offsets: p.offsets[k..=j].to_vec(),
                level: l,
                ..p.clone()
            });
            k = j;
        }
    }
    out
}

/// Pieces of one line in visual order (UAX #9 rule L2: from the highest level down
/// to the lowest odd level, reverse every run at that level or higher). Trailing
/// spaces and breaks keep the paragraph level (rule L1).
fn visual_order(pieces: &[Piece], base: u8) -> Vec<Piece> {
    let mut v: Vec<Piece> = pieces.to_vec();
    let mut end = v.len();
    while end > 0 && !matches!(v[end - 1].kind, Kind::Text | Kind::Object) {
        v[end - 1].level = base;
        end -= 1;
    }
    let max = v.iter().map(|p| p.level).max().unwrap_or(0);
    let min_odd = v
        .iter()
        .map(|p| p.level)
        .filter(|l| l % 2 == 1)
        .min()
        .unwrap_or(max + 1);
    let mut level = max;
    while level >= min_odd && level > 0 {
        let mut i = 0;
        while i < end {
            if v[i].level >= level {
                let mut j = i;
                while j < end && v[j].level >= level {
                    j += 1;
                }
                v[i..j].reverse();
                i = j;
            } else {
                i += 1;
            }
        }
        level -= 1;
    }
    v
}

/// Combining marks (they keep the font of the character they attach to).
fn is_combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

fn kind_of(c: char) -> Kind {
    match c {
        ' ' | '\u{3000}' | '\u{00A0}' => Kind::Space,
        '\t' => Kind::Tab,
        LINE_SEP | '\n' | '\r' => Kind::LineBreak,
        PAGE_SEP => Kind::PageBreak,
        COL_SEP => Kind::ColumnBreak,
        _ => Kind::Text,
    }
}

/// An atom of paragraph content between index offsets.
struct Atom {
    start: usize,
    end: usize,
    style: usize,
    /// Display text (text items, field results, note numbers).
    text: String,
    /// Atomic object (picture, field, note reference): one caret unit.
    object: bool,
    image: Option<Image>,
    /// Laid-out shape / text box of an inline drawing object.
    drawing: Option<Arc<Drawing>>,
    note: Option<NoteKey>,
    /// Depth below the baseline (equations).
    obj_d: f32,
}

/// An Office Math zone (ECMA-376 Part 1, §22.1) as an inline object: the placeholder picture (box above
/// the baseline), its drawing (glyph runs + rules, origin at the box top) and its depth below the baseline.
fn math_object(
    xml: &str,
    st: &RunStyle,
    fonts: &mut FontManager,
) -> Option<(Image, Arc<Drawing>, f32)> {
    let b = math_box(xml, st, fonts)?;
    let top = b.asc;
    let fragments = b
        .runs
        .into_iter()
        .map(|r| Fragment {
            face: r.face,
            size: r.size,
            x: r.x,
            y: top + r.y,
            width: r.width,
            color: st.color,
            underline: bo_model::Underline::None,
            strike: false,
            double_strike: false,
            text: r.text.into(),
            glyphs: r
                .glyphs
                .iter()
                .map(|&(id, x, y, cluster)| GlyphPos { id, x, y, cluster })
                .collect::<Vec<_>>()
                .into(),
            link: None,
            fx: None,
        })
        .collect();
    let items = b
        .rules
        .iter()
        .map(|&(x1, y1, x2, y2, width)| crate::types::PageItem::Rule {
            x1,
            y1: top + y1,
            x2,
            y2: top + y2,
            width,
            color: st.color,
            style: "single".into(),
        })
        .collect();
    let line = crate::types::Line {
        top: 0.0,
        height: 0.0,
        baseline: top,
        fragments,
        source: crate::types::LineSource::Other,
        start: 0,
        end: 0,
        carets: crate::types::Carets::default(),
        rotation: None,
    };
    let emu = |pt: f32| bo_model::Emu((pt * 12_700.0).round() as i64);
    let image = Image {
        media: String::new(),
        width: emu(b.w),
        height: emu(top),
        anchor: None,
        crop: None,
        rotation: 0,
        description: "equation".into(),
        shape: None,
        format: None,
        src: None,
        ole: None,
    };
    Some((
        image,
        Arc::new(Drawing {
            items,
            lines: vec![line],
        }),
        b.desc,
    ))
}

/// An Office Math zone laid out in the style of its run: upright and italic faces of the run's family
/// and the document's math font (OMML default Cambria Math, §22.1.2.61 mathFont) or a substitute with
/// a MATH table (bundled STIX Two Math); the text font when none has one.
fn math_box(xml: &str, st: &RunStyle, fonts: &mut FontManager) -> Option<crate::math::MathBox> {
    let family = fonts.info(st.face).family.clone();
    let bold = fonts.info(st.face).bold;
    let upright = fonts.resolve(&family, bold, false).unwrap_or(st.face);
    let italic = fonts.resolve(&family, bold, true).unwrap_or(upright);
    let math = fonts
        .resolve("cambria math", false, false)
        .filter(|f| fonts.math_constants(*f).is_some());
    crate::math::layout_math(xml, st.size, fonts, upright, italic, math)
}

/// Slot geometry (CORE-MATH-010) of the equation at index offset `offset` of `para`, laid out from
/// `xml` (the zone as written from its tree) in the style of the equation's run, as the paragraph lays
/// it out: coordinates from the equation's left edge (its caret stop) and the line's baseline.
pub fn equation_geometry(
    doc: &bo_model::Document,
    fonts: &mut FontManager,
    para: &Paragraph,
    offset: usize,
    xml: &str,
) -> Option<crate::math::EquationGeometry> {
    let (_, spans) = para_index(para);
    let sp = spans.iter().find(|sp| {
        sp.start == offset
            && matches!(
                para.runs.get(sp.run).and_then(|r| r.content.get(sp.item)),
                Some(RunContent::Markup(raw)) if bo_model::index::is_math_xml(&raw.xml)
            )
    })?;
    let props = crate::style::para_props(doc, para);
    let base = para_style_run(doc, &props);
    let rp = run_props(doc, &base, &para.runs.get(sp.run)?.props);
    let st = run_style(doc, fonts, &rp)?;
    let b = math_box(xml, &st, fonts)?;
    let n = b.slots.iter().map(|s| s.0 + 1).max().unwrap_or(0);
    let mut slots = vec![crate::math::MathSlotBox::default(); n];
    for (id, g) in b.slots {
        slots[id] = g;
    }
    Some(crate::math::EquationGeometry {
        w: b.w,
        asc: b.asc,
        desc: b.desc,
        slots,
    })
}

/// Lay out the shape / text box of drawing object `img` at index offset `at`.
fn drawing_of(
    inp: &ParaInput<'_>,
    fonts: &mut FontManager,
    cache: &mut LayoutCache,
    img: &Image,
    shape: &bo_model::Shape,
    at: usize,
) -> Result<Drawing, LayoutError> {
    let mut ctx = Ctx {
        doc: inp.doc,
        fonts,
        cache,
        lists: crate::style::ListCounters::default(),
        other_lists: crate::style::ListCounters::default(),
        notes: inp.note_numbers.clone(),
        fields: inp.fields,
        default_tab: inp.default_tab,
        next_id: 0,
        id_origin: 0,
        endnote_order: Vec::new(),
        view: inp.view,
        marks: inp.marks,
        note: None,
        textbox: None,
    };
    shape_drawing(&mut ctx, img, shape, at as u32)
}

fn layout_uncached(
    inp: &ParaInput<'_>,
    fonts: &mut FontManager,
    cache: &mut LayoutCache,
) -> Result<ParaLayout, LayoutError> {
    let props = &inp.props;
    let doc = inp.doc;
    let indent_left = props.indent_left.unwrap_or(Twips(0)).to_pt();
    let indent_right = props.indent_right.unwrap_or(Twips(0)).to_pt();
    let first = props.indent_first_line.unwrap_or(Twips(0)).to_pt();
    // §17.3.1.6 bidi: in a right-to-left paragraph `left`/`right` are the logical
    // start/end edges (start = right); no alignment means start.
    let align = match (props.bidi.unwrap_or(false), props.align) {
        (true, None | Some(Alignment::Left)) => Alignment::Right,
        (true, Some(Alignment::Right)) => Alignment::Left,
        (_, a) => a.unwrap_or_default(),
    };
    let spacing = props.line_spacing.unwrap_or_default();
    let base_run = para_style_run(doc, props);

    // 1. Styles per run, atoms over the index text.
    let (index_text, spans) = para_index(inp.para);
    let mut styles: Vec<RunStyle> = Vec::new();
    let mut run_style_idx: Vec<usize> = Vec::new();
    // Effective run properties (phonetic guides inherit them).
    let mut run_rps: Vec<RunProps> = Vec::new();
    for r in &inp.para.runs {
        // The original view shows the formatting before a tracked change (§17.13.5.31).
        let direct = match (&r.props.change, inp.view) {
            (Some(c), RevisionView::Original) => RunProps {
                ins: r.props.ins.clone(),
                del: r.props.del.clone(),
                link: r.props.link.clone(),
                ..c.old.clone()
            },
            _ => r.props.clone(),
        };
        let rp = run_props(doc, &base_run, &direct);
        run_rps.push(rp.clone());
        styles.push(revision_style(
            run_style(doc, fonts, &rp).ok_or(LayoutError::NoFonts)?,
            &rp,
            inp.view,
            &inp.marks,
        ));
        run_style_idx.push(styles.len() - 1);
    }
    let mark_style = match styles.last() {
        Some(s) => s.clone(),
        None => run_style(doc, fonts, &base_run).ok_or(LayoutError::NoFonts)?,
    };
    if styles.is_empty() {
        styles.push(mark_style.clone());
    }
    let mut atoms: Vec<Atom> = Vec::new();
    let mut floats = Vec::new();
    // Display equations (m:oMathPara) by start offset: alignment of their own line.
    let mut display_at: HashMap<u32, Alignment> = HashMap::new();
    // Legacy form fields (§17.16.17): open fields (text field?) and the shaded / resized
    // variants of the run styles.
    let mut forms: Vec<bool> = Vec::new();
    let mut form_styles: HashMap<(usize, Option<u32>), usize> = HashMap::new();
    // Inline content controls open here: whether each shows its placeholder (§17.5.2.39
    // showingPlcHdr), and the grey variants of the run styles placeholder text is shown in.
    let mut controls: Vec<bool> = Vec::new();
    let mut placeholder_styles: HashMap<usize, usize> = HashMap::new();
    for sp in &spans {
        let run = &inp.para.runs[sp.run];
        let mut style = run_style_idx[sp.run];
        let item = &run.content[sp.item];
        match item {
            RunContent::ControlStart(cc) if !cc.block => controls.push(cc.placeholder),
            RunContent::ControlEnd(false) => {
                controls.pop();
            }
            _ => {}
        }
        // Placeholder text in an automatic colour shows grey, as editors show placeholders.
        if controls.last() == Some(&true) && run_rps[sp.run].color.is_none() {
            style = *placeholder_styles.entry(style).or_insert_with(|| {
                let mut st = styles[style].clone();
                st.color = PLACEHOLDER_GREY;
                styles.push(st);
                styles.len() - 1
            });
        }
        match item {
            RunContent::FormField(f) => forms.push(f.is_text()),
            RunContent::FormEnd => {
                forms.pop();
                continue;
            }
            // The result of a check box / drop-down field is not shown, its state is.
            _ if forms.last() == Some(&false) => continue,
            _ => {}
        }
        if !forms.is_empty() {
            // Form field results are shaded; a check box of a fixed size (§17.16.29) has it.
            let size = match item {
                RunContent::FormField(f) => match &f.kind {
                    bo_model::FormFieldKind::CheckBox(c) => c.size,
                    _ => None,
                },
                _ => None,
            };
            style = *form_styles.entry((style, size)).or_insert_with(|| {
                let mut st = styles[style].clone();
                if st.highlight.is_none() {
                    st.highlight = Some(FORM_SHADING);
                }
                if let Some(hp) = size {
                    st.size = hp.max(2) as f32 / 2.0;
                }
                styles.push(st);
                styles.len() - 1
            });
        }
        let math = match item {
            RunContent::Markup(raw) if bo_model::index::is_math_xml(&raw.xml) => {
                if let Some(a) = crate::math::display_alignment(&raw.xml) {
                    display_at.insert(sp.start as u32, a);
                }
                math_object(&raw.xml, &styles[style], fonts)
            }
            // §17.3.3.25 ruby; §17.3.2.10 eastAsianLayout combine.
            RunContent::Opaque(raw) if bo_model::index::is_ruby_xml(&raw.xml) => {
                crate::ruby::ruby_object(&raw.xml, &run_rps[sp.run], doc, fonts)
            }
            RunContent::Text(t) if styles[style].combine && !styles[style].hidden => {
                crate::ruby::combine_object(t, &styles[style], fonts)
            }
            _ => None,
        };
        let drawing = match item {
            RunContent::Image(img) => match &img.shape {
                Some(s) => Some(Arc::new(drawing_of(inp, fonts, cache, img, s, sp.start)?)),
                None => None,
            },
            _ => math.as_ref().map(|m| m.1.clone()),
        };
        if let RunContent::Image(img) = item {
            if img.anchor.is_some() {
                // A floating picture deleted (final view) or inserted (original view)
                // by a tracked change is not shown (§17.13.5.14, §17.13.5.18).
                if !styles[style].hidden {
                    floats.push(Float {
                        img: (**img).clone(),
                        drawing,
                        para: None,
                    });
                }
                continue;
            }
        }
        if styles[style].hidden || sp.start == sp.end {
            continue;
        }
        let (text, object, image, note) = match item {
            RunContent::Image(img) => (String::new(), true, Some((**img).clone()), None),
            RunContent::Markup(_) | RunContent::Opaque(_) | RunContent::Text(_)
                if math.is_some() =>
            {
                (
                    String::new(),
                    true,
                    math.as_ref().map(|m| m.0.clone()),
                    None,
                )
            }
            RunContent::Field { instr, cached, .. } => {
                (field_text(instr, cached, inp.fields), true, None, None)
            }
            RunContent::FormField(f) => (
                if inp.fields.codes {
                    format!("{{ {} }}", f.kind.keyword())
                } else {
                    f.display().unwrap_or_default()
                },
                true,
                None,
                None,
            ),
            RunContent::FootnoteRef(id) | RunContent::EndnoteRef(id) => {
                let key = (matches!(item, RunContent::EndnoteRef(_)), *id);
                (
                    inp.note_numbers
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| "*".to_owned()),
                    true,
                    None,
                    Some(key),
                )
            }
            RunContent::NoteMark => (
                inp.note_mark.unwrap_or_default().to_owned(),
                true,
                None,
                None,
            ),
            _ => (index_text[sp.start..sp.end].to_owned(), false, None, None),
        };
        atoms.push(Atom {
            start: sp.start,
            end: sp.end,
            style,
            text,
            object,
            image,
            drawing,
            note,
            obj_d: math.as_ref().map_or(0.0, |m| m.2),
        });
    }

    // 2. Words at UAX #14 break opportunities, then pieces.
    let mut words: Vec<Word> = Vec::new();
    if let Some((label, lrp, suffix)) = &inp.label {
        let lstyle = run_style(doc, fonts, &lrp.inherit(&base_run)).ok_or(LayoutError::NoFonts)?;
        styles.push(lstyle);
        let si = styles.len() - 1;
        let mut pieces = Vec::new();
        if !label.is_empty() {
            pieces.extend(text_pieces(label, &[], si, &styles, fonts, true));
        }
        match suffix.as_str() {
            "space" => pieces.extend(text_pieces(" ", &[], si, &styles, fonts, true)),
            "nothing" => {}
            _ => pieces.push(special_piece(Kind::Tab, si, &styles, 0, true)),
        }
        if !pieces.is_empty() {
            words.push(Word::new(pieces));
        }
    }
    let mut prev = 0;
    let breaks: Vec<usize> = unicode_linebreak::linebreaks(&index_text)
        .map(|(i, _)| i)
        .collect();
    let base_level = u8::from(props.bidi.unwrap_or(false));
    let levels = bidi_levels(&index_text, base_level == 1);
    for idx in breaks {
        if idx > prev {
            let ranges = chunk_ranges(&index_text, prev, idx);
            if ranges.len() > 1 {
                // Chunks are shaped when reached (see `Word::lazy`).
                for (k, range) in ranges.into_iter().enumerate() {
                    words.push(Word {
                        pieces: Vec::new(),
                        chunk: if k == 0 { Chunk::First } else { Chunk::Next },
                        lazy: Some(range),
                    });
                }
            } else {
                let mut pieces = word_pieces(&atoms, prev, idx, &styles, fonts);
                if let Some(lv) = &levels {
                    pieces = split_by_level(pieces, lv, &styles, fonts);
                }
                if !pieces.is_empty() {
                    words.push(Word::new(pieces));
                }
            }
        }
        prev = idx;
    }

    // A display equation takes a line of its own: virtual line breaks end the text before it and the
    // equation itself (unless it ends the paragraph).
    if !display_at.is_empty() {
        let mut out: Vec<Word> = Vec::with_capacity(words.len() + 2);
        for mut w in words {
            shape_lazy(&mut w, &atoms, &styles, fonts, levels.as_deref());
            let mut rest: Vec<Piece> = Vec::new();
            for p in w.pieces {
                let display = p.kind == Kind::Object && display_at.contains_key(&p.start());
                if !display {
                    // Spaces right after an equation stay on its line (before its break).
                    let after_eq = rest.is_empty()
                        && p.kind == Kind::Space
                        && out.last().is_some_and(|w: &Word| {
                            w.pieces
                                .last()
                                .is_some_and(|b| b.kind == Kind::LineBreak && b.virtual_)
                        });
                    if after_eq {
                        if let Some(w) = out.last_mut() {
                            let at = w.pieces.len() - 1;
                            w.pieces.insert(at, p);
                        }
                    } else {
                        rest.push(p);
                    }
                    continue;
                }
                let ps = p.style;
                let brk = |at: u32| special_piece(Kind::LineBreak, ps, &styles, at, true);
                if !rest.is_empty() {
                    rest.push(brk(p.start()));
                    out.push(Word {
                        pieces: std::mem::take(&mut rest),
                        chunk: w.chunk,
                        lazy: None,
                    });
                } else if let Some(prev) = out.last_mut() {
                    if prev.brk().is_none() {
                        prev.pieces.push(brk(p.start()));
                    }
                }
                let end = p.end();
                let mut obj = vec![p];
                obj.push(brk(end));
                out.push(Word::new(obj));
            }
            if !rest.is_empty() {
                out.push(Word {
                    pieces: rest,
                    chunk: w.chunk,
                    lazy: None,
                });
            }
        }
        // No empty line after an equation that ends the paragraph.
        if let Some(last) = out.last_mut() {
            let only_trailing = last
                .pieces
                .last()
                .is_some_and(|p| p.kind == Kind::LineBreak && p.virtual_);
            if only_trailing {
                last.pieces.pop();
            }
        }
        words = out;
    }

    // 3. Greedy line filling.
    let tabs: Vec<TabStop> = props.tabs.clone().unwrap_or_default();
    let tab_ctx = TabCtx {
        stops: &tabs,
        default: inp.default_tab.max(1.0),
        hanging_at: if first < 0.0 { Some(indent_left) } else { None },
    };
    let avail = |first_line: bool| {
        (inp.width - indent_left - indent_right - if first_line { first } else { 0.0 }).max(1.0)
    };
    let mut lines: Vec<Vec<Piece>> = Vec::new();
    let mut line_breaks: Vec<Option<Kind>> = Vec::new();
    let mut cur: Vec<Piece> = Vec::new();
    let mut queue: std::collections::VecDeque<Word> = words.into();
    // §17.15.1.10 autoHyphenation unless §17.3.1.34 suppressAutoHyphens.
    let hyph = (doc.hyphenation.auto && !props.suppress_auto_hyphens.unwrap_or(false)).then(|| {
        let h = &doc.hyphenation;
        Hyph {
            zone: h
                .zone
                .unwrap_or(bo_model::Hyphenation::DEFAULT_ZONE)
                .to_pt()
                .max(0.0),
            limit: usize::from(h.consecutive_limit.unwrap_or(0)),
            no_caps: h.no_caps,
        }
    });
    // Lines ending in an automatic hyphen, and how many in a row end so.
    let mut hyph_lines: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut in_row = 0usize;
    // Segments of each line: (first piece, start x, width). One per line unless the
    // line flows around a wrapped picture.
    let mut segs: Vec<Vec<(usize, f32, f32)>> = Vec::new();
    if !inp.wrap.is_empty() {
        let x1 = inp.width - indent_right;
        let mut y = 0.0f32;
        let mut prev_h: Option<f32> = None;
        let mut row: Vec<(f32, f32)> = Vec::new();
        let mut cur_segs: Vec<(usize, f32, f32)> = Vec::new();
        while let Some(mut word) = queue.pop_front() {
            shape_lazy(&mut word, &atoms, &styles, fonts, levels.as_deref());
            if word.pieces.is_empty() {
                continue;
            }
            let is_first = lines.is_empty();
            if row.is_empty() {
                let h = prev_h.unwrap_or_else(|| {
                    row_height(&word.pieces, &styles, fonts, &mark_style, spacing)
                });
                let x0 = indent_left + if is_first { first } else { 0.0 };
                row = segments(inp.wrap, x0, x1, y, h);
                cur_segs = vec![(0, row[0].0, row[0].1)];
            }
            let seg = cur_segs.len() - 1;
            let (seg_start, start_x, max) = cur_segs[seg];
            let mut trial = cur[seg_start..].to_vec();
            trial.extend(word.pieces.iter().cloned());
            let (_, ink) = position(&trial, start_x, &tab_ctx);
            let overflow = ink > max + 0.01;
            if overflow && seg + 1 < row.len() {
                // Continue on the other side of the picture.
                cur_segs.push((cur.len(), row[seg + 1].0, row[seg + 1].1));
                queue.push_front(word);
                continue;
            }
            // A chunk of an over-long run is cut where the segment ends.
            let cut = if overflow
                && (word.chunk == Chunk::Next || (word.chunk == Chunk::First && cur.is_empty()))
            {
                let (_, used) = position(&cur[seg_start..], start_x, &tab_ctx);
                fit_split(&word, max - used, cur.is_empty())
            } else {
                None
            };
            if cut.is_none() && overflow && cur.is_empty() && word.char_count() > 1 {
                // One part back means the word cannot be divided (a single grapheme of
                // several characters, e.g. a letter with combining marks): it overflows
                // the line as it is. Queuing it again would never end.
                let parts = explode(&word, fonts);
                if parts.len() > 1 {
                    for w2 in parts.into_iter().rev() {
                        queue.push_front(w2);
                    }
                    continue;
                }
            }
            let end = if let Some((head, tail)) = cut {
                cur.extend(head.pieces);
                queue.push_front(tail);
                in_row = 0;
                Some(None)
            } else {
                None
            };
            let split = if end.is_some() {
                None
            } else if overflow && !cur.is_empty() && word.chunk == Chunk::No {
                hyph.as_ref()
                    .filter(|h| h.limit == 0 || in_row < h.limit)
                    .and_then(|h| {
                        hyphenate(
                            &word,
                            &cur[seg_start..],
                            start_x,
                            max,
                            &tab_ctx,
                            &styles,
                            fonts,
                            h,
                        )
                    })
            } else {
                None
            };
            let end = if end.is_some() {
                end
            } else if let Some((head, tail)) = split {
                cur.extend(head.pieces);
                queue.push_front(tail);
                hyph_lines.insert(lines.len());
                in_row += 1;
                Some(None)
            } else if overflow && !cur.is_empty() {
                queue.push_front(word);
                in_row = 0;
                Some(None)
            } else {
                let brk = word.brk();
                cur.extend(word.pieces);
                brk.map(Some)
            };
            if let Some(brk) = end {
                let h = row_height(&cur, &styles, fonts, &mark_style, spacing);
                lines.push(std::mem::take(&mut cur));
                line_breaks.push(brk);
                segs.push(std::mem::take(&mut cur_segs));
                row.clear();
                y += h;
                prev_h = Some(h);
            }
        }
        if !cur_segs.is_empty() {
            segs.push(cur_segs);
        }
    }
    while let Some(mut word) = queue.pop_front() {
        shape_lazy(&mut word, &atoms, &styles, fonts, levels.as_deref());
        if word.pieces.is_empty() {
            continue;
        }
        let is_first = lines.is_empty();
        let start_x = indent_left + if is_first { first } else { 0.0 };
        let max = avail(is_first);
        // Measure the line with the word appended, then take the word back: moving the
        // pieces avoids cloning the whole line for every word (CORE-PERF-001).
        let base = cur.len();
        cur.append(&mut word.pieces);
        let (_, ink) = position(&cur, start_x, &tab_ctx);
        word.pieces = cur.split_off(base);
        // A chunk of an over-long run continues the line and is cut where it ends.
        if ink > max + 0.01
            && (word.chunk == Chunk::Next || (word.chunk == Chunk::First && cur.is_empty()))
        {
            let (_, used) = position(&cur, start_x, &tab_ctx);
            if let Some((head, tail)) = fit_split(&word, max - used, cur.is_empty()) {
                cur.extend(head.pieces);
                lines.push(std::mem::take(&mut cur));
                line_breaks.push(None);
                queue.push_front(tail);
                in_row = 0;
                continue;
            }
        }
        if ink > max + 0.01 && word.chunk == Chunk::No {
            // §17.15.1.10 autoHyphenation: the first part of the word and a hyphen.
            let split = hyph
                .as_ref()
                .filter(|h| h.limit == 0 || in_row < h.limit)
                .and_then(|h| hyphenate(&word, &cur, start_x, max, &tab_ctx, &styles, fonts, h));
            if let Some((head, tail)) = split {
                cur.extend(head.pieces);
                hyph_lines.insert(lines.len());
                lines.push(std::mem::take(&mut cur));
                line_breaks.push(None);
                queue.push_front(tail);
                in_row += 1;
                continue;
            }
        }
        if ink > max + 0.01 && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            line_breaks.push(None);
            queue.push_front(word);
            in_row = 0;
            continue;
        }
        if cur.is_empty() && ink > max + 0.01 && word.char_count() > 1 {
            // See the wrapped case above: an indivisible word is placed as it is.
            let parts = explode(&word, fonts);
            if parts.len() > 1 {
                for w2 in parts.into_iter().rev() {
                    queue.push_front(w2);
                }
                continue;
            }
        }
        let brk = word.brk();
        cur.extend(word.pieces);
        if brk.is_some() {
            lines.push(std::mem::take(&mut cur));
            line_breaks.push(brk);
        }
    }
    let ended_with_line_break = line_breaks.last().copied().flatten() == Some(Kind::LineBreak);
    if !cur.is_empty() || lines.is_empty() || ended_with_line_break {
        lines.push(cur);
        line_breaks.push(None);
    }
    let n_lines = lines.len();
    segs.truncate(n_lines);
    while segs.len() < n_lines {
        let is_first = segs.is_empty();
        segs.push(vec![(
            0,
            indent_left + if is_first { first } else { 0.0 },
            avail(is_first),
        )]);
    }

    // 4. Position pieces → fragments, carets, alignment and line height.
    let mut out: Vec<RelLine> = Vec::with_capacity(n_lines);
    let para_end = index_text.len() as u32;
    for (li, line_pieces) in lines.into_iter().enumerate() {
        let is_last = li + 1 == n_lines;
        let brk = line_breaks[li];
        // A display equation's line follows its m:jc instead of the paragraph alignment.
        let align = line_pieces
            .iter()
            .find_map(|p| {
                (p.kind == Kind::Object)
                    .then(|| display_at.get(&p.start()).copied())
                    .flatten()
            })
            .unwrap_or(align);
        let mut ascent = 0.0f32;
        let mut descent = 0.0f32;
        let mut gap = 0.0f32;
        let mut frags: Vec<Draft> = Vec::new();
        let mut carets: Vec<(u32, f32)> = Vec::new();
        let mut objects = Vec::new();
        let mut highlights = Vec::new();
        let mut borders: Vec<(f32, f32, bo_model::Border)> = Vec::new();
        let mut notes = Vec::new();
        let mut first_caret = None;
        let line_segs = &segs[li];
        for (sk, &(s0, start_x, max)) in line_segs.iter().enumerate() {
            let s1 = line_segs.get(sk + 1).map_or(line_pieces.len(), |s| s.0);
            let reordered;
            let pieces = match &levels {
                Some(_) => {
                    reordered = visual_order(&line_pieces[s0.min(s1)..s1], base_level);
                    &reordered[..]
                }
                None => &line_pieces[s0.min(s1)..s1],
            };
            let last_seg = sk + 1 == line_segs.len();
            let (xs, ink_end) = position(pieces, start_x, &tab_ctx);
            let slack = (max - ink_end).max(0.0);
            let justify = align == Alignment::Justify && (!last_seg || (!is_last && brk.is_none()));
            let last_tab = pieces.iter().rposition(|p| p.kind == Kind::Tab);
            let last_ink = pieces
                .iter()
                .rposition(|p| matches!(p.kind, Kind::Text | Kind::Object));
            let stretchable = |i: usize, p: &Piece| {
                p.kind == Kind::Space
                    && last_tab.is_none_or(|t| i > t)
                    && last_ink.is_some_and(|l| i < l)
            };
            let space_count: usize = if justify {
                pieces
                    .iter()
                    .enumerate()
                    .filter(|(i, p)| stretchable(*i, p))
                    .map(|(_, p)| p.text.chars().count())
                    .sum()
            } else {
                0
            };
            let per_space = if space_count > 0 {
                slack / space_count as f32
            } else {
                0.0
            };
            let offset = if last_tab.is_some() {
                0.0
            } else {
                match align {
                    Alignment::Center => slack / 2.0,
                    Alignment::Right => slack,
                    _ => 0.0,
                }
            };
            first_caret.get_or_insert(start_x + offset);

            let mut extra = 0.0f32;
            for (i, p) in pieces.iter().enumerate() {
                let st = &styles[p.style];
                let info = fonts.info(p.face);
                let scale = st.size / f32::from(info.units_per_em);
                if p.kind == Kind::Object && p.image.is_some() {
                    ascent = ascent.max(p.obj_h);
                    descent = descent.max(p.obj_d);
                } else {
                    ascent = ascent.max(f32::from(info.ascender) * scale - st.shift.min(0.0));
                    descent = descent.max(-f32::from(info.descender) * scale + st.shift.max(0.0));
                    gap = gap.max(f32::from(info.line_gap) * scale);
                }
                let x0 = offset + xs[i] + extra;
                let x_next = offset + xs[i + 1] + extra;
                if p.kind == Kind::Tab {
                    if let Some(leader) = leader_at(&tab_ctx, xs[i + 1]) {
                        if let Some(f) = leader_fragment(fonts, st, p.face, leader, x0, x_next) {
                            frags.push(f);
                        }
                    }
                }
                if !p.virtual_ {
                    let w = if p.kind == Kind::Tab {
                        x_next - x0
                    } else {
                        p.width
                    };
                    push_carets(&mut carets, p, x0, w, scale, st.spacing);
                }
                if let Some(n) = p.note {
                    notes.push(n);
                }
                match p.kind {
                    Kind::Object if p.image.is_some() => {
                        if let Some(img) = &p.image {
                            objects.push(RelObject {
                                x: x0,
                                w: p.width,
                                h: p.obj_h,
                                image: (**img).clone(),
                                drawing: p.drawing.clone(),
                                revision: st.revision,
                            });
                        }
                        continue;
                    }
                    Kind::LineBreak | Kind::PageBreak | Kind::ColumnBreak | Kind::Tab => continue,
                    _ => {}
                }
                let stretch_here = per_space > 0.0 && stretchable(i, p);
                let mut glyphs = Vec::with_capacity(p.glyphs.len());
                let mut gx = 0.0f32;
                for g in p.glyphs.iter() {
                    glyphs.push(GlyphPos {
                        id: g.glyph_id,
                        x: gx + g.x_offset as f32 * scale,
                        y: -(g.y_offset as f32) * scale + st.shift,
                        cluster: g.cluster,
                    });
                    gx += g.x_advance as f32 * scale + st.spacing;
                    if stretch_here {
                        gx += per_space;
                    }
                }
                if stretch_here {
                    extra += per_space * p.text.chars().count() as f32;
                }
                if let Some(h) = st.highlight {
                    highlights.push((x0, gx, h));
                }
                // §17.3.2.4 bdr: adjacent runs with the same border share one box.
                if let Some(b) = &st.border {
                    match borders.last_mut() {
                        Some(l) if l.2 == *b && (l.0 + l.1 - x0).abs() < 0.01 => l.1 += gx,
                        _ => borders.push((x0, gx, b.clone())),
                    }
                }
                if let Some(last) = frags.last_mut() {
                    let same = last.face == p.face
                        && last.size == st.size
                        && last.color == st.color
                        && last.underline == st.underline
                        && last.strike == st.strike
                        && last.double_strike == st.double_strike
                        && last.link == st.link
                        && last.fx == st.fx
                        && (last.x + last.width - x0).abs() < 0.001
                        && last.glyphs.iter().all(|g| (g.y - st.shift).abs() < 1e-4)
                        && glyphs.iter().all(|g| (g.y - st.shift).abs() < 1e-4);
                    if same {
                        let dx = x0 - last.x;
                        let base = last.text.len() as u32;
                        last.glyphs.extend(glyphs.into_iter().map(|g| GlyphPos {
                            x: g.x + dx,
                            cluster: g.cluster + base,
                            ..g
                        }));
                        last.width = dx + gx;
                        last.text.push_str(&p.text);
                        continue;
                    }
                }
                frags.push(Draft {
                    face: p.face,
                    size: st.size,
                    x: x0,
                    y: 0.0,
                    width: gx,
                    color: st.color,
                    underline: st.underline,
                    strike: st.strike,
                    double_strike: st.double_strike,
                    text: p.text.clone(),
                    glyphs,
                    link: st.link.clone(),
                    fx: st.fx.clone(),
                });
            }
        }
        let pieces = &line_pieces;
        if pieces
            .iter()
            .all(|p| !matches!(p.kind, Kind::Text | Kind::Space | Kind::Object))
        {
            let info = fonts.info(mark_style.face);
            let scale = mark_style.size / f32::from(info.units_per_em);
            ascent = ascent.max(f32::from(info.ascender) * scale);
            descent = descent.max(-f32::from(info.descender) * scale);
            gap = gap.max(f32::from(info.line_gap) * scale);
        }
        let natural = ascent + descent + gap;
        // Line height (ECMA-376 Part 1, §17.3.1.33 spacing/@line, @lineRule).
        let (height, baseline) = match spacing {
            LineSpacing::Auto(v) => {
                let h = natural * v as f32 / 240.0;
                (h, (h - natural).max(0.0) + ascent)
            }
            LineSpacing::Exact(t) => {
                let h = t.to_pt();
                (h, h - descent)
            }
            LineSpacing::AtLeast(t) => {
                let h = natural.max(t.to_pt());
                (h, h - descent - gap)
            }
        };
        let real: Vec<&Piece> = pieces.iter().filter(|p| !p.virtual_).collect();
        let (start, end) = if real.is_empty() {
            let at = out.last().map_or(0, |l| l.end).min(para_end);
            (at, at)
        } else {
            (
                real.iter().map(|p| p.start()).min().unwrap_or(0),
                real.iter().map(|p| p.end()).max().unwrap_or(0),
            )
        };
        // §17.3.3.29 softHyphen: a line broken at an optional hyphen shows a hyphen;
        // so does a line ended by automatic hyphenation (§17.15.1.10).
        if !is_last
            && brk.is_none()
            && (hyph_lines.contains(&li)
                || index_text
                    .get(..end as usize)
                    .is_some_and(|t| t.ends_with('\u{00AD}')))
        {
            if let Some(last) = frags.iter().max_by(|a, b| a.x.total_cmp(&b.x)) {
                let (face, size, color, x) =
                    (last.face, last.size, last.color, last.x + last.width);
                let shaped = fonts.shape_kern(face, "-", false);
                let k = size / f32::from(fonts.info(face).units_per_em);
                let w: f32 = shaped.iter().map(|g| g.x_advance as f32 * k).sum();
                frags.push(Draft {
                    face,
                    size,
                    x,
                    y: 0.0,
                    width: w,
                    color,
                    underline: bo_model::Underline::None,
                    strike: false,
                    double_strike: false,
                    text: "-".into(),
                    glyphs: shaped
                        .iter()
                        .map(|g| GlyphPos {
                            id: g.glyph_id,
                            x: 0.0,
                            y: 0.0,
                            cluster: 0,
                        })
                        .collect(),
                    link: None,
                    fx: None,
                });
            }
        }
        if carets.is_empty() {
            carets.push((start, first_caret.unwrap_or(indent_left)));
        }
        carets.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        carets.dedup_by_key(|c| c.0);
        out.push(RelLine {
            height,
            baseline,
            fragments: frags.into_iter().map(Draft::finish).collect(),
            carets: carets.into(),
            objects,
            highlights,
            borders,
            ascent,
            descent,
            start,
            end,
            page_break_after: brk == Some(Kind::PageBreak),
            column_break_after: brk == Some(Kind::ColumnBreak),
            notes,
        });
    }

    Ok(ParaLayout {
        lines: out,
        space_before: props.space_before.unwrap_or(Twips(0)).to_pt(),
        space_after: props.space_after.unwrap_or(Twips(0)).to_pt(),
        page_break_before: props.page_break_before.unwrap_or(false),
        keep_next: props.keep_next.unwrap_or(false),
        keep_lines: props.keep_lines.unwrap_or(false),
        widow_control: props.widow_control.unwrap_or(true),
        contextual_spacing: props.contextual_spacing.unwrap_or(false),
        style: props.style.clone(),
        shading: props.shading,
        borders: props.borders.clone(),
        indent_left,
        indent_right,
        floats,
        frame: props.frame,
        numbered: !props.suppress_line_numbers.unwrap_or(false),
    })
}

/// Split `word` at an automatic hyphenation point (ECMA-376 Part 1, §17.15.1.10)
/// so that its first part and a hyphen fit after `cur` in a line `max` wide: the
/// last point that fits, when the gap left without hyphenating is at least the
/// hyphenation zone (§17.15.1.52) and the word is not in capitals with
/// §17.15.1.40 doNotHyphenateCaps.
#[allow(clippy::too_many_arguments)]
fn hyphenate(
    word: &Word,
    cur: &[Piece],
    start_x: f32,
    max: f32,
    tab: &TabCtx<'_>,
    styles: &[RunStyle],
    fonts: &mut FontManager,
    h: &Hyph,
) -> Option<(Word, Word)> {
    let (_, ink0) = position(cur, start_x, tab);
    if !cur.is_empty() && max - ink0 < h.zone {
        return None;
    }
    // The leading text pieces (real text with one offset per character).
    let n_text = word
        .pieces
        .iter()
        .take_while(|p| {
            p.kind == Kind::Text
                && !p.virtual_
                && p.image.is_none()
                && p.note.is_none()
                && p.offsets.len() == p.text.chars().count() + 1
        })
        .count();
    if n_text == 0 {
        return None;
    }
    let text: String = word.pieces[..n_text]
        .iter()
        .map(|p| p.text.as_str())
        .collect();
    let letters_len = text.bytes().take_while(u8::is_ascii_alphabetic).count();
    let letters = &text[..letters_len];
    if h.no_caps && letters.bytes().all(|b| b.is_ascii_uppercase()) {
        return None;
    }
    for &at in crate::hyphen::points(letters).iter().rev() {
        let (head, tail) = split_word(word, n_text, at, styles, fonts)?;
        let last = head.pieces.last()?;
        let st = &styles[last.style];
        let upem = f32::from(fonts.info(last.face).units_per_em);
        let hw = fonts
            .shape_kern(last.face, "-", false)
            .iter()
            .map(|g| i64::from(g.x_advance))
            .sum::<i64>() as f32
            * st.size
            / upem;
        let mut trial = cur.to_vec();
        trial.extend(head.pieces.iter().cloned());
        let (_, ink) = position(&trial, start_x, tab);
        if ink + hw <= max + 0.01 {
            return Some((head, tail));
        }
    }
    None
}

/// Split `word` at byte `at` of the text of its first `n_text` pieces (the piece
/// holding that offset is shaped again in two).
fn split_word(
    word: &Word,
    n_text: usize,
    at: usize,
    styles: &[RunStyle],
    fonts: &mut FontManager,
) -> Option<(Word, Word)> {
    let mut head = Vec::new();
    let mut tail = Vec::new();
    let mut acc = 0usize;
    for (i, p) in word.pieces.iter().enumerate() {
        let len = p.text.len();
        if i >= n_text || acc >= at {
            tail.push(p.clone());
        } else if acc + len <= at {
            head.push(p.clone());
        } else {
            let k = at - acc;
            if !p.text.is_char_boundary(k) {
                return None;
            }
            let ci = p.text[..k].chars().count();
            let st = &styles[p.style];
            let upem = f32::from(fonts.info(p.face).units_per_em);
            let mut part = |t: &str, offs: &[u32]| {
                let glyphs = shape_run(fonts, p.face, t, st);
                let width = glyphs.iter().map(|g| i64::from(g.x_advance)).sum::<i64>() as f32
                    * st.size
                    / upem
                    + st.spacing * glyphs.len() as f32;
                Piece {
                    text: t.to_owned(),
                    glyphs,
                    width,
                    offsets: offs.to_vec(),
                    ..p.clone()
                }
            };
            head.push(part(&p.text[..k], &p.offsets[..=ci]));
            tail.push(part(&p.text[k..], &p.offsets[ci..]));
        }
        acc += len;
    }
    if head.is_empty() || tail.is_empty() {
        return None;
    }
    Some((Word::new(head), Word::new(tail)))
}

/// Shape a lazy chunk (see [`Word::lazy`]); its glyphs stay out of the shaping cache
/// (they are shaped once). Other words are left as they are.
fn shape_lazy(
    word: &mut Word,
    atoms: &[Atom],
    styles: &[RunStyle],
    fonts: &mut FontManager,
    levels: Option<&[u8]>,
) {
    let Some((a, b)) = word.lazy.take() else {
        return;
    };
    fonts.set_shape_caching(false);
    let mut pieces = word_pieces(atoms, a, b, styles, fonts);
    if let Some(lv) = levels {
        pieces = split_by_level(pieces, lv, styles, fonts);
    }
    fonts.set_shape_caching(true);
    word.pieces = pieces;
}

/// Byte ranges of the run `text[a..b]` (no break opportunity inside): the run itself, or
/// chunks of [`CHUNK_CHARS`] characters cut at grapheme boundaries (UAX #29) when it is
/// longer than [`MAX_WORD_CHARS`].
fn chunk_ranges(text: &str, a: usize, b: usize) -> Vec<(usize, usize)> {
    use unicode_segmentation::UnicodeSegmentation;
    let Some(run) = text.get(a..b) else {
        return vec![(a, b)];
    };
    // Bytes ≥ characters: a short run in bytes is short in characters.
    if run.len() <= MAX_WORD_CHARS || run.chars().count() <= MAX_WORD_CHARS {
        return vec![(a, b)];
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut chars = 0usize;
    for (i, g) in run.grapheme_indices(true) {
        let n = g.chars().count();
        if chars > 0 && chars + n > CHUNK_CHARS {
            out.push((a + start, a + i));
            start = i;
            chars = 0;
        }
        chars += n;
    }
    out.push((a + start, b));
    out
}

/// Cut a chunk of an over-long run (see [`MAX_WORD_CHARS`]) where `room` points of ink
/// end: the head holds the graphemes that fit (at least one when `force`), the tail the
/// rest. Both reuse the shaped glyphs — the cut is at a cluster start that is also a
/// grapheme boundary, so no glyph is split and nothing is shaped again. `None` when
/// nothing fits (and not `force`) or the word cannot be cut there (a tab, an atom).
fn fit_split(word: &Word, room: f32, force: bool) -> Option<(Word, Word)> {
    use unicode_segmentation::UnicodeSegmentation;
    let mut x = 0.0f32;
    for (i, p) in word.pieces.iter().enumerate() {
        let plain = p.kind == Kind::Text
            && !p.glyphs.is_empty()
            && p.image.is_none()
            && p.note.is_none()
            && !p.virtual_
            && p.offsets.len() == p.text.chars().count() + 1;
        match p.kind {
            Kind::Tab => return None,
            Kind::Space | Kind::LineBreak | Kind::PageBreak | Kind::ColumnBreak => {
                x += p.width;
                continue;
            }
            Kind::Text | Kind::Object => {}
        }
        if x + p.width <= room + 0.01 {
            x += p.width;
            continue;
        }
        let at_piece = |i: usize| {
            (i > 0).then(|| {
                (
                    Word {
                        pieces: word.pieces[..i].to_vec(),
                        chunk: word.chunk,
                        lazy: None,
                    },
                    Word {
                        pieces: word.pieces[i..].to_vec(),
                        chunk: Chunk::Next,
                        lazy: None,
                    },
                )
            })
        };
        if !plain {
            return at_piece(i);
        }
        let len = p.text.len();
        // Advance of the glyphs of each cluster, by byte; cluster starts.
        let mut adv = vec![0i64; len + 1];
        let mut starts = vec![false; len + 1];
        for g in p.glyphs.iter() {
            let c = (g.cluster as usize).min(len);
            adv[c] += i64::from(g.x_advance);
            starts[c] = true;
        }
        let total: i64 = adv.iter().sum();
        let per_unit = p.width / total.max(1) as f32;
        // The last cut that fits, else (forced, first piece) the first possible cut.
        let mut best = None;
        let mut first_cut = None;
        let mut acc = 0i64;
        let mut done = 0usize;
        for (gi, g) in p.text.grapheme_indices(true) {
            let k = gi + g.len();
            acc += adv[done..k].iter().sum::<i64>();
            done = k;
            if k >= len || !starts[k] {
                continue;
            }
            if x + acc as f32 * per_unit <= room + 0.01 {
                best = Some(k);
            } else {
                first_cut.get_or_insert(k);
                break;
            }
        }
        let k = match best {
            Some(k) => k,
            None if i > 0 => return at_piece(i),
            None if force => first_cut?,
            None => return None,
        };
        let ci = p.text[..k].chars().count();
        let head_adv: i64 = adv[..k].iter().sum();
        let head_w = head_adv as f32 * per_unit;
        let kc = u32::try_from(k).ok()?;
        let mut head = word.pieces[..i].to_vec();
        head.push(Piece {
            text: p.text[..k].to_owned(),
            glyphs: p
                .glyphs
                .iter()
                .filter(|g| g.cluster < kc)
                .copied()
                .collect(),
            width: head_w,
            offsets: p.offsets[..=ci].to_vec(),
            ..p.clone_shallow()
        });
        let mut tail = vec![Piece {
            text: p.text[k..].to_owned(),
            glyphs: p
                .glyphs
                .iter()
                .filter(|g| g.cluster >= kc)
                .map(|g| ShapedGlyph {
                    cluster: g.cluster - kc,
                    ..*g
                })
                .collect(),
            width: p.width - head_w,
            offsets: p.offsets[ci..].to_vec(),
            ..p.clone_shallow()
        }];
        tail.extend(word.pieces[i + 1..].iter().cloned());
        return Some((
            Word {
                pieces: head,
                chunk: word.chunk,
                lazy: None,
            },
            Word {
                pieces: tail,
                chunk: Chunk::Next,
                lazy: None,
            },
        ));
    }
    None
}

/// Caret stops for a piece: one per original char boundary.
fn push_carets(
    carets: &mut Vec<(u32, f32)>,
    p: &Piece,
    x0: f32,
    width: f32,
    scale: f32,
    spacing: f32,
) {
    if p.offsets.len() < 2 {
        return;
    }
    let shaped_text = matches!(p.kind, Kind::Text | Kind::Space) && !p.glyphs.is_empty();
    let char_count = p.text.chars().count();
    if !shaped_text || p.offsets.len() != char_count + 1 {
        // Atomic: start and end.
        carets.push((p.start(), x0));
        carets.push((p.end(), x0 + width));
        return;
    }
    if p.level % 2 == 1 {
        // Right to left: the caret before a character is right of the glyphs of the
        // characters before it (glyphs come in visual order).
        for (k, (cb, _)) in p.text.char_indices().enumerate() {
            let before: f32 = p
                .glyphs
                .iter()
                .filter(|g| (g.cluster as usize) < cb)
                .map(|g| g.x_advance as f32 * scale + spacing)
                .sum();
            carets.push((p.offsets[k], x0 + (width - before).max(0.0)));
        }
        carets.push((p.end(), x0));
        return;
    }
    let mut adv_before: Vec<f32> = Vec::with_capacity(p.glyphs.len());
    let mut x = 0.0;
    for g in p.glyphs.iter() {
        adv_before.push(x);
        x += g.x_advance as f32 * scale + spacing;
    }
    // The first glyph of a cluster at or after each character: with clusters in
    // increasing order (left-to-right text) one forward scan finds them all.
    let monotonic = p.glyphs.windows(2).all(|w| w[0].cluster <= w[1].cluster);
    let mut gi = 0usize;
    for (k, (cb, _)) in p.text.char_indices().enumerate() {
        let found = if monotonic {
            while gi < p.glyphs.len() && (p.glyphs[gi].cluster as usize) < cb {
                gi += 1;
            }
            (gi < p.glyphs.len()).then_some(gi)
        } else {
            p.glyphs.iter().position(|g| g.cluster as usize >= cb)
        };
        let cx = found.map_or(x, |i| adv_before[i]);
        carets.push((p.offsets[k], x0 + cx.min(width)));
    }
    carets.push((p.end(), x0 + width));
}

struct TabCtx<'a> {
    stops: &'a [TabStop],
    default: f32,
    hanging_at: Option<f32>,
}

/// Next tab stop after `x` (content-relative): custom stops, the hanging-indent stop, then defaults
/// (ECMA-376 Part 1, §17.3.1.37 tab; §17.15.1.25 defaultTabStop).
fn next_stop(t: &TabCtx<'_>, x: f32) -> (f32, TabAlign) {
    let mut best: Option<(f32, TabAlign)> = None;
    for s in t.stops {
        if s.align == TabAlign::Clear || s.align == TabAlign::Bar {
            continue;
        }
        let p = s.pos.to_pt();
        if p > x + 0.01 && best.is_none_or(|b| p < b.0) {
            best = Some((p, s.align));
        }
    }
    if let Some(h) = t.hanging_at {
        if h > x + 0.01 && best.is_none_or(|b| h < b.0) {
            best = Some((h, TabAlign::Left));
        }
    }
    best.unwrap_or_else(|| (((x / t.default).floor() + 1.0) * t.default, TabAlign::Left))
}

/// Leader of the custom stop located at `stop_x`.
fn leader_at(t: &TabCtx<'_>, stop_x: f32) -> Option<TabLeader> {
    t.stops
        .iter()
        .filter(|s| s.leader != TabLeader::None)
        .find(|s| (s.pos.to_pt() - stop_x).abs() < 0.5 || s.pos.to_pt() >= stop_x - 0.5)
        .map(|s| s.leader)
}

fn leader_fragment(
    fonts: &mut FontManager,
    st: &RunStyle,
    face: FaceId,
    leader: TabLeader,
    x0: f32,
    x1: f32,
) -> Option<Draft> {
    let ch = match leader {
        TabLeader::Dot => ".",
        TabLeader::Hyphen => "-",
        TabLeader::Underscore => "_",
        TabLeader::None => return None,
    };
    let shaped = fonts.shape(face, ch);
    let g = shaped.first()?;
    let scale = st.size / f32::from(fonts.info(face).units_per_em);
    let adv = g.x_advance as f32 * scale;
    if adv <= 0.1 || x1 - x0 < adv * 2.0 {
        return None;
    }
    // Align leader glyphs to a grid so consecutive leaders line up.
    let start = (x0 / adv).ceil() * adv;
    let mut glyphs = Vec::new();
    let mut x = start;
    let mut text = String::new();
    while x + adv <= x1 - adv * 0.5 {
        glyphs.push(GlyphPos {
            id: g.glyph_id,
            x: x - start,
            y: 0.0,
            cluster: text.len() as u32,
        });
        text.push_str(ch);
        x += adv;
    }
    Some(Draft {
        face,
        size: st.size,
        x: start,
        y: 0.0,
        width: x - start,
        color: st.color,
        underline: bo_model::Underline::None,
        strike: false,
        double_strike: false,
        text,
        glyphs,
        link: None,
        fx: None,
    })
}

/// Content-relative x of each piece start (plus the end) and the ink end relative to `start_x`
/// (trailing spaces and breaks excluded). Right/center/decimal tabs align the following segment.
fn position(pieces: &[Piece], start_x: f32, t: &TabCtx<'_>) -> (Vec<f32>, f32) {
    let mut xs = Vec::with_capacity(pieces.len() + 1);
    let mut x = start_x;
    let mut ink = start_x;
    for (i, p) in pieces.iter().enumerate() {
        xs.push(x);
        match p.kind {
            Kind::Tab => {
                let (stop, al) = next_stop(t, x);
                let mut seg = 0.0;
                let mut dec = None;
                for q in &pieces[i + 1..] {
                    if matches!(
                        q.kind,
                        Kind::Tab | Kind::LineBreak | Kind::PageBreak | Kind::ColumnBreak
                    ) {
                        break;
                    }
                    if dec.is_none() && al == TabAlign::Decimal {
                        if let Some(pos) = q.text.find(['.', ',']) {
                            let frac = pos as f32 / q.text.len().max(1) as f32;
                            dec = Some(seg + q.width * frac);
                        }
                    }
                    seg += q.width;
                }
                x = match al {
                    TabAlign::Right => (stop - seg).max(x),
                    TabAlign::Center => (stop - seg / 2.0).max(x),
                    TabAlign::Decimal => (stop - dec.unwrap_or(seg)).max(x),
                    _ => stop,
                };
                ink = ink.max(x);
            }
            Kind::LineBreak | Kind::PageBreak | Kind::ColumnBreak => {}
            Kind::Space => x += p.width,
            Kind::Text | Kind::Object => {
                x += p.width;
                ink = ink.max(x);
            }
        }
    }
    xs.push(x);
    (xs, ink - start_x)
}

fn special_piece(kind: Kind, style: usize, styles: &[RunStyle], at: u32, virtual_: bool) -> Piece {
    Piece {
        style,
        face: styles[style].face,
        kind,
        text: String::new(),
        glyphs: Arc::from(Vec::new()),
        width: 0.0,
        offsets: vec![at, at + u32::from(!virtual_)],
        virtual_,
        image: None,
        drawing: None,
        level: 0,
        obj_h: 0.0,
        obj_d: 0.0,
        note: None,
    }
}

/// Shape `text` with a run's kerning and OpenType features ([MS-DOCX] w14 typography),
/// advances scaled by its horizontal scale (§17.3.2.43 w).
pub(crate) fn shape_run(
    fonts: &mut FontManager,
    face: FaceId,
    text: &str,
    st: &RunStyle,
) -> Arc<[ShapedGlyph]> {
    let g = fonts.shape_features(face, text, st.kern, &st.features);
    if (st.scale_x - 1.0).abs() < 1e-6 {
        return g;
    }
    g.iter()
        .map(|g| ShapedGlyph {
            x_advance: (g.x_advance as f32 * st.scale_x).round() as i32,
            x_offset: (g.x_offset as f32 * st.scale_x).round() as i32,
            ..*g
        })
        .collect()
}

/// Shape `text` (one style) into Text/Space pieces. `offsets` gives the original byte
/// offset of each char plus the end (empty → all zero, used for virtual pieces).
fn text_pieces(
    text: &str,
    offsets: &[u32],
    style: usize,
    styles: &[RunStyle],
    fonts: &mut FontManager,
    virtual_: bool,
) -> Vec<Piece> {
    let st = &styles[style];
    let display: String = if st.caps {
        text.chars()
            .map(|c| c.to_uppercase().next().unwrap_or(c))
            .collect()
    } else {
        text.to_owned()
    };
    let n_chars = display.chars().count();
    let offs: Vec<u32> = if offsets.len() == n_chars + 1 {
        offsets.to_vec()
    } else {
        vec![offsets.first().copied().unwrap_or(0); n_chars + 1]
    };
    let class = |c: char| {
        if kind_of(c) == Kind::Space {
            Kind::Space
        } else {
            Kind::Text
        }
    };
    let chars: Vec<(usize, char)> = display.char_indices().collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < chars.len() {
        let kind = class(chars[k].1);
        let mut j = k + 1;
        while j < chars.len() && class(chars[j].1) == kind {
            j += 1;
        }
        let b0 = chars[k].0;
        let b1 = if j < chars.len() {
            chars[j].0
        } else {
            display.len()
        };
        let group = &display[b0..b1];
        let mut cursor = k;
        // §17.3.2.26: split by font slot (marks stay with their base character), then
        // by glyph coverage.
        let mut slots: Vec<(std::ops::Range<usize>, FaceId)> = Vec::new();
        if st.faces.iter().all(|f| *f == st.face) {
            slots.push((0..group.len(), st.face));
        } else {
            for (i, c) in group.char_indices() {
                let face = if is_combining(c) {
                    slots.last().map_or(st.face, |s| s.1)
                } else {
                    st.faces[slot_of(c, st.hint_east_asia, st.complex) as usize]
                };
                let end = i + c.len_utf8();
                match slots.last_mut() {
                    Some(s) if s.1 == face => s.0.end = end,
                    _ => slots.push((i..end, face)),
                }
            }
        }
        let items: Vec<(std::ops::Range<usize>, FaceId)> = slots
            .into_iter()
            .flat_map(|(r, f)| {
                let base = r.start;
                fonts
                    .itemize(f, &group[r])
                    .into_iter()
                    .map(move |(rr, ff)| (rr.start + base..rr.end + base, ff))
                    .collect::<Vec<_>>()
            })
            .collect();
        for (r, face) in items {
            let t = &group[r];
            let nc = t.chars().count();
            let glyphs = shape_run(fonts, face, t, st);
            let upem = f32::from(fonts.info(face).units_per_em);
            let width = glyphs.iter().map(|g| i64::from(g.x_advance)).sum::<i64>() as f32 * st.size
                / upem
                + st.spacing * glyphs.len() as f32;
            out.push(Piece {
                style,
                face,
                kind,
                text: t.to_owned(),
                glyphs,
                width,
                offsets: offs[cursor..=cursor + nc].to_vec(),
                virtual_,
                image: None,
                drawing: None,
                level: 0,
                obj_h: 0.0,
                obj_d: 0.0,
                note: None,
            });
            cursor += nc;
        }
        k = j;
    }
    out
}

fn word_pieces(
    atoms: &[Atom],
    a: usize,
    b: usize,
    styles: &[RunStyle],
    fonts: &mut FontManager,
) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for at in atoms {
        let s = at.start.max(a);
        let e = at.end.min(b);
        if s >= e {
            continue;
        }
        if at.object {
            // A multi-character object (two lines in one) belongs to its first word.
            if at.start < a {
                continue;
            }
            if let Some(img) = &at.image {
                pieces.push(Piece {
                    style: at.style,
                    face: styles[at.style].face,
                    kind: Kind::Object,
                    text: "\u{FFFC}".into(),
                    glyphs: Arc::from(Vec::new()),
                    width: img.width.to_pt(),
                    offsets: vec![at.start as u32, at.end as u32],
                    virtual_: false,
                    image: Some(Box::new(img.clone())),
                    drawing: at.drawing.clone(),
                    level: 0,
                    obj_h: img.height.to_pt(),
                    obj_d: at.obj_d,
                    note: None,
                });
                continue;
            }
            // Field result / note number: shaped text mapped to one caret unit.
            let mut sub = text_pieces(&at.text, &[], at.style, styles, fonts, false);
            if sub.is_empty() {
                let mut p = special_piece(Kind::Object, at.style, styles, at.start as u32, false);
                p.offsets = vec![at.start as u32, at.end as u32];
                sub.push(p);
            }
            let n = sub.len();
            for (i, p) in sub.iter_mut().enumerate() {
                let o0 = if i == 0 {
                    at.start as u32
                } else {
                    at.end as u32
                };
                p.offsets = vec![o0, at.end as u32];
                p.note = if i + 1 == n { at.note } else { None };
            }
            pieces.extend(sub);
            continue;
        }
        let sub = &at.text[s - at.start..e - at.start];
        let mut buf = String::new();
        let mut buf_offs: Vec<u32> = Vec::new();
        for (bi, c) in sub.char_indices() {
            let off = (s + bi) as u32;
            let k = kind_of(c);
            match k {
                Kind::Tab | Kind::LineBreak | Kind::PageBreak | Kind::ColumnBreak => {
                    if !buf.is_empty() {
                        buf_offs.push(off);
                        pieces.extend(text_pieces(&buf, &buf_offs, at.style, styles, fonts, false));
                        buf.clear();
                        buf_offs.clear();
                    }
                    let mut sp = special_piece(k, at.style, styles, off, false);
                    sp.offsets = vec![off, off + c.len_utf8() as u32];
                    pieces.push(sp);
                }
                // Soft hyphen (§17.3.3.29): invisible; a hyphen is drawn when a line breaks there.
                _ if c == '\u{00AD}' => {}
                _ => {
                    buf.push(c);
                    buf_offs.push(off);
                }
            }
        }
        if !buf.is_empty() {
            buf_offs.push(e as u32);
            pieces.extend(text_pieces(&buf, &buf_offs, at.style, styles, fonts, false));
        }
    }
    pieces
}

/// Split a word into one word per grapheme cluster (emergency breaking).
fn explode(word: &Word, fonts: &mut FontManager) -> Vec<Word> {
    use unicode_segmentation::UnicodeSegmentation;
    let mut out = Vec::new();
    for p in &word.pieces {
        if p.kind != Kind::Text
            || p.glyphs.is_empty()
            || p.offsets.len() != p.text.chars().count() + 1
        {
            match out.last_mut() {
                Some(Word { pieces, .. }) => pieces.push(p.clone()),
                None => out.push(Word::new(vec![p.clone()])),
            }
            continue;
        }
        let per_unit = p.width
            / p.glyphs
                .iter()
                .map(|g| i64::from(g.x_advance))
                .sum::<i64>()
                .max(1) as f32;
        // The other fields of every grapheme piece, without the word's text and offsets:
        // cloning the whole piece per grapheme made a long unbroken word quadratic.
        let template = Piece {
            text: String::new(),
            offsets: Vec::new(),
            ..p.clone()
        };
        let mut ci = 0usize;
        for g in p.text.graphemes(true) {
            let nc = g.chars().count();
            let glyphs = fonts.shape(p.face, g);
            let width =
                glyphs.iter().map(|x| i64::from(x.x_advance)).sum::<i64>() as f32 * per_unit;
            out.push(Word::new(vec![Piece {
                text: g.to_owned(),
                glyphs,
                width,
                offsets: p.offsets[ci..=ci + nc].to_vec(),
                ..template.clone()
            }]));
            ci += nc;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_runs_are_cut_into_chunks_at_grapheme_boundaries() {
        // Short runs stay whole.
        let short = "a".repeat(MAX_WORD_CHARS);
        assert_eq!(chunk_ranges(&short, 0, short.len()), vec![(0, short.len())]);
        // A long run: chunks of CHUNK_CHARS characters, contiguous, never splitting
        // "e" from its combining acute accent.
        let long: String = (0..MAX_WORD_CHARS + 500)
            .map(|i| if i % 3 == 0 { "e\u{301}" } else { "x" })
            .collect();
        let r = chunk_ranges(&long, 0, long.len());
        assert!(r.len() > 1);
        assert_eq!(r.first().map(|c| c.0), Some(0));
        assert_eq!(r.last().map(|c| c.1), Some(long.len()));
        for w in r.windows(2) {
            assert_eq!(w[0].1, w[1].0);
        }
        for &(a, b) in &r {
            let chunk = &long[a..b];
            assert!(chunk.chars().count() <= CHUNK_CHARS);
            assert!(!chunk.starts_with('\u{301}'));
        }
    }

    #[test]
    fn tab_stops() {
        let t = TabCtx {
            stops: &[],
            default: 36.0,
            hanging_at: None,
        };
        assert_eq!(next_stop(&t, 0.0).0, 36.0);
        assert_eq!(next_stop(&t, 36.0).0, 72.0);
        let stops = [TabStop {
            pos: Twips(2000),
            align: TabAlign::Right,
            leader: TabLeader::Dot,
        }];
        let t = TabCtx {
            stops: &stops,
            default: 36.0,
            hanging_at: Some(18.0),
        };
        assert_eq!(next_stop(&t, 0.0), (18.0, TabAlign::Left));
        assert_eq!(next_stop(&t, 20.0), (100.0, TabAlign::Right));
        assert_eq!(next_stop(&t, 120.0).0, 144.0);
        assert_eq!(leader_at(&t, 100.0), Some(TabLeader::Dot));
    }

    #[test]
    fn fields() {
        let f = FieldContext {
            page: 3,
            pages: 9,
            section_pages: 2,
            codes: false,
        };
        // Field codes shown instead of results (Alt+F9).
        assert_eq!(
            field_text("PAGE", "1", FieldContext { codes: true, ..f }),
            "{ PAGE }"
        );
        assert_eq!(field_text("PAGE \\* MERGEFORMAT", "1", f), "3");
        assert_eq!(field_text("NUMPAGES", "1", f), "9");
        // §17.16.4.3 general formats on page numbers.
        assert_eq!(field_text("PAGE \\* ROMAN", "1", f), "III");
        assert_eq!(field_text("PAGE \\# \"00\"", "1", f), "03");
        assert_eq!(
            field_text("DATE \\@ dd/MM/yyyy", "05/11/2026", f),
            "05/11/2026"
        );
    }
}
