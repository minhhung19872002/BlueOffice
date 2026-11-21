//! Paragraphs for BlueEditor's Edit Text (ISO 32000-1 §9.4 text objects, §9.3 text state).
//!
//! Text lines of a page ([`crate::text_lines`]) are grouped into paragraph blocks: lines on one
//! baseline become a visual line, and visual lines that follow each other at a steady pitch,
//! in one font size, whose left edges, centres or right edges line up form a block with that
//! alignment.
//!
//! A block is edited as a session ([`BlockEdit`]): the page is analysed once when editing
//! starts and the block's runs are neutralised in a copy of the page content (the prefix; a
//! neutralised run only moves the text position, so later text stays put). Every change lays
//! the block out again — words wrapped at the block's width, alignment and line pitch kept,
//! each character in its own style — and writes the prefix plus one text object for the block
//! into the page's stream, uncompressed. Character advances are measured once per style and
//! character by running a small text object through the interpreter. The stream is
//! compressed when the session ends ([`PdfDocument::end_block_edit`]).
#![allow(clippy::single_match_else)]
use crate::interp::{apply, invert, mul, TextState, M};
use crate::objects::{deleted_run, hex, num, rgb, ObjectKind, PageObject, Recorded};
use crate::{fallback, PdfDocument};
use bo_render::Rgba;
use lopdf::{Object, ObjectId};
use std::collections::HashMap;

/// How the lines of a block line up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Left edges line up.
    Left,
    /// Centres line up.
    Center,
    /// Right edges line up.
    Right,
    /// Both edges line up (word gaps stretched; the last line is left aligned).
    Justify,
}

/// A paragraph of a page: the unit Edit Text edits.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBlock {
    /// Page object indices of its runs.
    pub ids: Vec<usize>,
    /// Left, top, right, bottom in page points.
    pub bbox: [f32; 4],
    /// Its text; lines are joined with a space.
    pub text: String,
    /// One box per character of `text`.
    pub char_boxes: Vec<[f32; 4]>,
    /// Page object index of each character's run.
    pub runs: Vec<usize>,
    /// How its lines line up.
    pub align: Align,
    /// Number of lines.
    pub lines: usize,
    /// Font size of its first run on the page.
    pub font_size: f32,
    /// Fill colour of its first run.
    pub color: [u8; 3],
}

/// Text to page points of a run (one text space unit, before the font size) and its baseline
/// origin; `None` when it is not a text run.
fn run_frame(rec: &Recorded) -> Option<(M, (f32, f32))> {
    let ts = rec.text.as_ref()?;
    let full = mul(ts.tm, rec.ctm);
    Some((full, (full[4], full[5])))
}

/// Upright text: x to the right, y up (page y grows downwards).
fn upright(m: M) -> bool {
    m[0] > 0.0 && m[3] < 0.0 && m[1].abs() < 1e-3 * m[0] && m[2].abs() < 1e-3 * m[0].abs()
}

/// A visual line: text lines on one baseline.
struct VLine {
    ids: Vec<usize>,
    text: String,
    boxes: Vec<[f32; 4]>,
    runs: Vec<usize>,
    bbox: [f32; 4],
    x0: f32,
    baseline: f32,
    size: f32,
    upright: bool,
    color: [u8; 3],
    /// Split text mark of its first run (lines of different marks never join).
    mark: Option<i64>,
}

impl VLine {
    fn width(&self) -> f32 {
        self.bbox[2] - self.x0
    }
}

fn visual_lines(objs: &[PageObject]) -> Vec<VLine> {
    let mut out: Vec<VLine> = Vec::new();
    for l in crate::text_lines(objs) {
        let Some((full, origin)) = l.ids.first().and_then(|&i| run_frame(&objs[i].rec)) else {
            continue;
        };
        let up = upright(full);
        let mark = l.ids.first().and_then(|&i| objs[i].rec.mark);
        if let Some(v) = out.last_mut() {
            let em = v.size.max(l.font_size).max(1.0);
            let gap = l.bbox[0] - v.bbox[2];
            if up
                && v.upright
                && v.mark == mark
                && (origin.1 - v.baseline).abs() < 0.2 * em
                && gap > -0.3 * em
                && gap < 1.5 * em
            {
                if gap > 0.2 * em && !v.text.ends_with(' ') && !l.text.starts_with(' ') {
                    v.text.push(' ');
                    v.boxes.push([v.bbox[2], l.bbox[1], l.bbox[0], l.bbox[3]]);
                    let last = v.runs.last().copied().unwrap_or(l.ids[0]);
                    v.runs.push(last);
                }
                v.ids.extend(&l.ids);
                v.text.push_str(&l.text);
                v.boxes.extend(&l.char_boxes);
                v.runs.extend(&l.runs);
                v.bbox = [
                    v.bbox[0].min(l.bbox[0]),
                    v.bbox[1].min(l.bbox[1]),
                    v.bbox[2].max(l.bbox[2]),
                    v.bbox[3].max(l.bbox[3]),
                ];
                v.size = v.size.max(l.font_size);
                continue;
            }
        }
        out.push(VLine {
            ids: l.ids,
            text: l.text,
            boxes: l.char_boxes,
            runs: l.runs,
            bbox: l.bbox,
            x0: origin.0.min(l.bbox[0]),
            baseline: origin.1,
            size: l.font_size,
            upright: up,
            color: l.color,
            mark,
        });
    }
    out
}

/// Whether `n` continues the block of `lines`.
fn continues(lines: &[VLine], n: &VLine) -> bool {
    let (Some(first), Some(last)) = (lines.first(), lines.last()) else {
        return false;
    };
    if !first.upright || !n.upright || n.mark != first.mark {
        return false;
    }
    let size = first.size.max(1.0);
    if (n.size - first.size).abs() > 0.15 * size {
        return false;
    }
    let d = n.baseline - last.baseline;
    if d < 0.8 * size || d > 2.0 * size {
        return false;
    }
    if lines.len() >= 2 {
        let pitch = lines[1].baseline - lines[0].baseline;
        if (d - pitch).abs() > 0.15 * pitch + 0.5 {
            return false;
        }
    }
    let left = lines.iter().map(|l| l.x0).fold(f32::INFINITY, f32::min);
    let right = lines
        .iter()
        .map(|l| l.bbox[2])
        .fold(f32::NEG_INFINITY, f32::max);
    let em = size;
    let width = (right - left).max(n.width());
    let indent = lines.len() == 1 && n.x0 < first.x0 && first.x0 - n.x0 <= 4.0 * em;
    let lefts = (n.x0 - left).abs() <= 0.6 * em || indent;
    // A left-aligned paragraph ends at a short line.
    if lefts && last.width() >= 0.7 * width {
        return true;
    }
    let centre = |l: &VLine| f32::midpoint(l.x0, l.bbox[2]);
    let centres = lines
        .iter()
        .all(|l| (centre(l) - centre(n)).abs() <= 0.6 * em);
    let rights = lines
        .iter()
        .all(|l| (l.bbox[2] - n.bbox[2]).abs() <= 0.6 * em);
    (centres || rights) && !lefts
}

fn alignment(lines: &[VLine]) -> Align {
    if lines.len() < 2 {
        return Align::Left;
    }
    let em = lines[0].size.max(1.0);
    let left = lines
        .iter()
        .skip(1)
        .map(|l| l.x0)
        .fold(f32::INFINITY, f32::min);
    let rest_left = lines
        .iter()
        .skip(1)
        .all(|l| (l.x0 - left).abs() <= 0.6 * em);
    let all_left = rest_left && (lines[0].x0 - left).abs() <= 0.6 * em;
    // A first-line indent: the rest line up, the first starts further right.
    let indented = rest_left && lines[0].x0 > left;
    let body = &lines[..lines.len() - 1];
    let right = body
        .iter()
        .map(|l| l.bbox[2])
        .fold(f32::NEG_INFINITY, f32::max);
    let rights = body.iter().all(|l| (l.bbox[2] - right).abs() <= 0.3 * em);
    let lefts_kind = if rights && body.len() >= 2 {
        Align::Justify
    } else {
        Align::Left
    };
    if all_left {
        return lefts_kind;
    }
    let centre = |l: &VLine| f32::midpoint(l.x0, l.bbox[2]);
    let c0 = centre(&lines[0]);
    if lines.iter().all(|l| (centre(l) - c0).abs() <= 0.6 * em) {
        return Align::Center;
    }
    let r0 = lines[0].bbox[2];
    if lines.iter().all(|l| (l.bbox[2] - r0).abs() <= 0.6 * em) {
        return Align::Right;
    }
    if indented {
        return lefts_kind;
    }
    Align::Left
}

/// Upper-case hexadecimal digits of a PDF hex string.
const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";

/// Group the text of a page into paragraph blocks (content order).
pub fn text_blocks(objs: &[PageObject]) -> Vec<TextBlock> {
    let mut groups: Vec<Vec<VLine>> = Vec::new();
    for v in visual_lines(objs) {
        match groups.last_mut() {
            Some(g) if continues(g, &v) => g.push(v),
            _ => groups.push(vec![v]),
        }
    }
    groups
        .into_iter()
        .map(|g| {
            let align = alignment(&g);
            let mut b = TextBlock {
                ids: Vec::new(),
                bbox: g[0].bbox,
                text: String::new(),
                char_boxes: Vec::new(),
                runs: Vec::new(),
                align,
                lines: g.len(),
                font_size: g[0].size,
                color: g[0].color,
            };
            for (k, l) in g.into_iter().enumerate() {
                if k > 0 && !b.text.ends_with(' ') {
                    b.text.push(' ');
                    let r = b.char_boxes.last().copied().unwrap_or(l.bbox);
                    b.char_boxes.push([r[2], r[1], r[2], r[3]]);
                    let last = b.runs.last().copied().unwrap_or(0);
                    b.runs.push(last);
                }
                b.ids.extend(l.ids);
                b.text.push_str(&l.text);
                b.char_boxes.extend(l.boxes);
                b.runs.extend(l.runs);
                b.bbox = [
                    b.bbox[0].min(l.bbox[0]),
                    b.bbox[1].min(l.bbox[1]),
                    b.bbox[2].max(l.bbox[2]),
                    b.bbox[3].max(l.bbox[3]),
                ];
            }
            let n = b.text.chars().count();
            b.char_boxes
                .resize(n, [b.bbox[2], b.bbox[1], b.bbox[2], b.bbox[3]]);
            let last = b.runs.last().copied().unwrap_or(0);
            b.runs.resize(n, last);
            b
        })
        .collect()
}

/// Character style in a block being edited.
#[derive(Debug, Clone, PartialEq)]
pub struct RunStyle {
    /// Font family shown to the user (the font's name without a subset tag).
    pub family: String,
    /// Size on the page in points.
    pub size: f32,
    /// Fill colour.
    pub color: [u8; 3],
    /// Bold face.
    pub bold: bool,
    /// Italic face.
    pub italic: bool,
    /// Underlined (a rule drawn under the text).
    pub underline: bool,
    /// Struck through.
    pub strike: bool,
    /// Superscript (1), subscript (-1) or neither (0).
    pub script: i8,
    /// Text state written for it (font resource, size, spacing, colours).
    pub(crate) ts: TextState,
    /// Text space (one unit, before the font size) to page points; no translation.
    pub(crate) lin: M,
    /// Bundled face embedded instead of the run's font.
    pub(crate) face: Option<usize>,
}

/// A style change for [`PdfDocument::block_edit_style`].
#[derive(Debug, Clone, PartialEq)]
pub enum StyleChange {
    /// Bold on or off.
    Bold(bool),
    /// Italic on or off.
    Italic(bool),
    /// Size in page points.
    Size(f32),
    /// Fill colour.
    Color([u8; 3]),
    /// A bundled family: serif (true) or sans serif.
    Serif(bool),
    /// Underline on or off.
    Underline(bool),
    /// Strikethrough on or off.
    Strike(bool),
    /// Superscript (1), subscript (-1) or back to the baseline (0).
    Script(i8),
    /// Horizontal scale in percent (`Tz`).
    Scale(f32),
    /// Extra space between characters in points (`Tc`).
    CharSpacing(f32),
    /// Extra space between words in points (`Tw`).
    WordSpacing(f32),
}

impl RunStyle {
    /// Page points per unscaled text space unit along the baseline.
    fn along_k(&self) -> f32 {
        (self.lin[0] * self.lin[0] + self.lin[1] * self.lin[1])
            .sqrt()
            .max(1e-6)
    }

    /// Horizontal scale in percent.
    pub fn scale(&self) -> f32 {
        self.ts.th * 100.0
    }

    /// Character spacing in points.
    pub fn char_spacing(&self) -> f32 {
        self.ts.tc * self.along_k()
    }

    /// Word spacing in points.
    pub fn word_spacing(&self) -> f32 {
        self.ts.tw * self.along_k()
    }
}

/// Advance along the baseline and box relative to the origin (page points), and the codes.
#[derive(Debug, Clone)]
struct Metric {
    adv: f32,
    rel: [f32; 4],
    codes: Vec<u8>,
}

/// A paragraph being edited (see the module documentation).
#[derive(Debug, Clone)]
pub struct BlockEdit {
    /// Page index.
    pub page: usize,
    /// Its text (`'\n'` is a line break typed with Enter).
    pub text: Vec<char>,
    /// Style index (into `styles`) of each character.
    pub style_of: Vec<usize>,
    /// Styles of the block.
    pub styles: Vec<RunStyle>,
    /// Alignment kept on every layout.
    pub align: Align,
    /// Character boxes after the last layout (page points), one per character.
    pub char_boxes: Vec<[f32; 4]>,
    /// Line of each character after the last layout.
    pub line_of: Vec<usize>,
    /// Box of the laid out block (page points).
    pub bbox: [f32; 4],
    /// Baseline-to-baseline distance (page points).
    pub pitch: f32,
    /// Number of lines after the last layout.
    pub lines: usize,
    /// Top-left of the first line's baseline (page points).
    origin: (f32, f32),
    /// Width the text wraps at.
    pub(crate) width: f32,
    /// First-line indent of each paragraph (along the baseline).
    indent: f32,
    /// Unit vectors along the baseline and across lines (downwards) in page points.
    u: (f32, f32),
    v: (f32, f32),
    /// CTM of the first run (user space to page points).
    ctm: M,
    pub(crate) prefix: Vec<u8>,
    sid: Option<ObjectId>,
    metrics: HashMap<(usize, char), Metric>,
    /// The page content was written by this session.
    pub dirty: bool,
    /// What moves with the paragraph when it gains or loses lines (`block_edit_push`).
    pub(crate) push: Option<crate::paragraph_push::Push>,
}

fn metric(e: &BlockEdit, i: usize) -> Option<&Metric> {
    let k = e.style_of[i].min(e.styles.len() - 1);
    e.metrics.get(&(k, e.text[i]))
}

impl BlockEdit {
    /// Caret before character `c` (`c` = length: after the last): x, top, x, bottom in page
    /// points.
    pub fn caret_box(&self, c: usize) -> [f32; 4] {
        let n = self.text.len().min(self.char_boxes.len());
        if n == 0 {
            return [self.bbox[0], self.bbox[1], self.bbox[0], self.bbox[3]];
        }
        if c < n {
            let b = self.char_boxes[c];
            return [b[0], b[1], b[0], b[3]];
        }
        let b = self.char_boxes[n - 1];
        if self.text[n - 1] == '\n' {
            // Start of the empty line after a typed break.
            let (dx, dy) = (self.pitch * self.v.0, self.pitch * self.v.1);
            let x = self.origin.0 + self.indent * self.u.0 + dx;
            return [x, b[1] + dy, x, b[3] + dy];
        }
        [b[2], b[1], b[2], b[3]]
    }

    /// Line of caret position `c`.
    fn caret_line(&self, c: usize) -> usize {
        let n = self.text.len().min(self.line_of.len());
        match (c < n, n) {
            (_, 0) => 0,
            (true, _) => self.line_of[c],
            (false, _) => self.line_of[n - 1] + usize::from(self.text[n - 1] == '\n'),
        }
    }

    /// Caret positions (first, last) of line `line`.
    pub fn line_span(&self, line: usize) -> (usize, usize) {
        let n = self.text.len().min(self.line_of.len());
        let first = (0..n).find(|&i| self.line_of[i] == line).unwrap_or(n);
        let mut last = (0..n)
            .rev()
            .find(|&i| self.line_of[i] == line)
            .map_or(n, |i| i + 1);
        // Before a typed break, or before the space a soft wrap hangs.
        let ends_break = last > first && last <= n && self.text[last - 1] == '\n';
        let hangs = last < n && last > first && self.text[last - 1] == ' ';
        if ends_break || hangs {
            last -= 1;
        }
        (first, last)
    }

    /// Caret position nearest to a page point.
    pub fn hit(&self, x: f32, y: f32) -> usize {
        let n = self.text.len().min(self.char_boxes.len());
        if n == 0 {
            return 0;
        }
        let lines = self.line_of[..n].iter().max().copied().unwrap_or(0) + 1;
        let mut best = (0, f32::INFINITY);
        for line in 0..lines {
            let (top, bottom) = (0..n)
                .filter(|&i| self.line_of[i] == line)
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(t, b), i| {
                    (t.min(self.char_boxes[i][1]), b.max(self.char_boxes[i][3]))
                });
            let d = if y < top {
                top - y
            } else if y > bottom {
                y - bottom
            } else {
                0.0
            };
            if d < best.1 {
                best = (line, d);
            }
        }
        self.hit_in_line(best.0, x)
    }

    fn hit_in_line(&self, line: usize, x: f32) -> usize {
        let (first, last) = self.line_span(line);
        (first..last)
            .find(|&i| x < f32::midpoint(self.char_boxes[i][0], self.char_boxes[i][2]))
            .unwrap_or(last)
    }

    /// Caret one line up or down from `c` at the same x (unchanged at the first / last line).
    pub fn vertical(&self, c: usize, down: bool) -> usize {
        let line = self.caret_line(c);
        let n = self.text.len().min(self.line_of.len());
        let lines = if n == 0 {
            1
        } else {
            self.line_of[n - 1] + 1 + usize::from(self.text[n - 1] == '\n')
        };
        let target = if down {
            if line + 1 >= lines {
                return n;
            }
            line + 1
        } else {
            if line == 0 {
                return 0;
            }
            line - 1
        };
        let x = self.caret_box(c)[0];
        if (0..n).all(|i| self.line_of[i] != target) {
            return n;
        }
        self.hit_in_line(target, x)
    }

    /// Start and end of the line holding caret `c` (Home / End).
    pub fn home_end(&self, c: usize) -> (usize, usize) {
        let line = self.caret_line(c);
        let n = self.text.len().min(self.line_of.len());
        if (0..n).all(|i| self.line_of[i] != line) {
            return (n, n);
        }
        self.line_span(line)
    }

    /// The word around caret `c` (double click).
    pub fn word(&self, c: usize) -> (usize, usize) {
        let n = self.text.len();
        let w = |i: usize| self.text[i].is_alphanumeric();
        let mut a = c.min(n);
        while a > 0 && w(a - 1) {
            a -= 1;
        }
        let mut b = c.min(n);
        while b < n && w(b) {
            b += 1;
        }
        (a, b)
    }

    /// Boxes covering characters `a..b`, one per line.
    pub fn selection_rects(&self, a: usize, b: usize) -> Vec<[f32; 4]> {
        let mut out: Vec<(usize, [f32; 4])> = Vec::new();
        for i in a..b.min(self.char_boxes.len()).min(self.line_of.len()) {
            let r = self.char_boxes[i];
            let line = self.line_of[i];
            match out.last_mut() {
                Some((l, u)) if *l == line => {
                    *u = [
                        u[0].min(r[0]),
                        u[1].min(r[1]),
                        u[2].max(r[2]),
                        u[3].max(r[3]),
                    ];
                }
                _ => out.push((line, r)),
            }
        }
        out.into_iter().map(|o| o.1).collect()
    }

    /// Insert `s` at `at` in the style of the character before it (after it at the start).
    pub fn insert(&mut self, at: usize, s: &str) -> usize {
        let at = at.min(self.text.len());
        let style = if at > 0 {
            self.style_of.get(at - 1)
        } else {
            self.style_of.first()
        }
        .copied()
        .unwrap_or(0);
        let mut k = at;
        for c in s.chars().filter(|c| *c == '\n' || !c.is_control()) {
            self.text.insert(k, c);
            self.style_of.insert(k.min(self.style_of.len()), style);
            k += 1;
        }
        k
    }

    /// Remove characters `a..b`.
    pub fn remove(&mut self, a: usize, b: usize) {
        let n = self.text.len();
        let (a, b) = (a.min(n), b.min(n));
        if a < b {
            self.text.drain(a..b);
            let m = self.style_of.len();
            self.style_of.drain(a.min(m)..b.min(m));
        }
    }

    /// Unit vector across lines (downwards) in page points.
    pub(crate) fn across(&self) -> (f32, f32) {
        self.v
    }

    /// Space beyond the line height between lines (points): the pitch less the height of the
    /// first character's box (ascent to descent of its font).
    pub fn line_gap(&self) -> f32 {
        let h = self
            .char_boxes
            .iter()
            .zip(&self.text)
            .find(|(_, c)| !c.is_whitespace())
            .map_or(self.pitch, |(b, _)| b[3] - b[1]);
        (self.pitch - h).max(0.0)
    }

    /// Move the paragraph's left edge by `delta` points (indent / outdent; the right edge
    /// stays).
    pub fn shift_left(&mut self, delta: f32) {
        let d = if self.width.is_finite() {
            delta.min(self.width - 20.0)
        } else {
            delta
        };
        let d = d.max(-(self.origin.0 * self.u.0 + self.origin.1 * self.u.1).max(0.0));
        self.origin = (self.origin.0 + d * self.u.0, self.origin.1 + d * self.u.1);
        if self.width.is_finite() {
            self.width -= d;
        }
    }

    /// Style of the character before caret `c` (the first at the start).
    pub fn style_at(&self, c: usize) -> Option<&RunStyle> {
        let k = if c > 0 {
            self.style_of.get(c - 1)
        } else {
            self.style_of.first()
        }?;
        self.styles.get(*k)
    }
}

/// Family of a font resource: its base font name without a subset tag (§9.6.4).
fn family_of(font: Option<&lopdf::Dictionary>) -> String {
    let name = font
        .and_then(|f| f.get(b"BaseFont").ok())
        .and_then(|b| b.as_name().ok())
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default();
    let name = match name.split_once('+') {
        Some((tag, rest)) if tag.len() == 6 && tag.bytes().all(|b| b.is_ascii_uppercase()) => {
            rest.to_owned()
        }
        _ => name,
    };
    let base = name.split([',', '-']).next().unwrap_or("").to_owned();
    if base.is_empty() {
        "Sans".into()
    } else {
        base
    }
}

const FAMILY_SERIF: &str = "Tinos";
const FAMILY_SANS: &str = "Liberation Sans";

fn mat_str(m: M) -> String {
    m.iter().map(|v| num(*v)).collect::<Vec<_>>().join(" ")
}

impl PdfDocument {
    /// Page font dictionary named `name` on page `index`.
    fn page_font(&self, index: usize, name: &[u8]) -> Option<&lopdf::Dictionary> {
        let info = self.pages.get(index)?;
        let doc = &self.doc;
        let page = doc.get_dictionary(info.id).ok()?;
        let res = crate::inherited(doc, page, b"Resources")?.as_dict().ok()?;
        let fonts = crate::resolve(doc, res.get(b"Font").ok()?).as_dict().ok()?;
        crate::resolve(doc, fonts.get(name).ok()?).as_dict().ok()
    }

    /// The page content an edit starts from: the editor's own stream, or the content
    /// wrapped in `q … Q` exactly as [`PdfDocument::page_stream`] will make it.
    pub(crate) fn virtual_content(&self, index: usize) -> Result<Vec<u8>, String> {
        let info = self.pages.get(index).ok_or("no such page")?;
        let page = self
            .doc
            .get_dictionary(info.id)
            .map_err(|e| e.to_string())?;
        if let Ok(Object::Reference(id)) = page.get(b"Contents") {
            if self.edit.own_streams.contains(id) {
                let s = self.doc.get_object(*id).and_then(Object::as_stream);
                return s
                    .ok()
                    .and_then(|s| crate::filters::stream_bytes(&self.doc, s).ok())
                    .ok_or_else(|| "page content".to_owned());
            }
        }
        let mut content = b"q\n".to_vec();
        content.extend(crate::page_content(&self.doc, page));
        content.extend_from_slice(b"\nQ\n");
        Ok(content)
    }

    pub(crate) fn style_of_run(&self, index: usize, rec: &Recorded) -> Option<RunStyle> {
        let ts = rec.text.clone()?;
        let (full, _) = run_frame(rec)?;
        let lin = [full[0], full[1], full[2], full[3], 0.0, 0.0];
        let size = (lin[2] * lin[2] + lin[3] * lin[3]).sqrt() * ts.size;
        let face = self.fallback_face(index, &ts.font);
        Some(RunStyle {
            family: family_of(self.page_font(index, &ts.font)),
            size,
            color: [ts.fill.0, ts.fill.1, ts.fill.2],
            bold: face & 1 != 0,
            italic: face & 2 != 0,
            underline: false,
            strike: false,
            script: 0,
            ts,
            lin,
            face: None,
        })
    }

    /// Start editing `block` of page `index` (from [`text_blocks`] of this page's objects);
    /// opens an undo group that [`PdfDocument::end_block_edit`] closes.
    pub fn begin_block_edit(
        &mut self,
        index: usize,
        block: &TextBlock,
    ) -> Result<BlockEdit, String> {
        let content = self.virtual_content(index)?;
        let objs = self.objects_in(index, Some(&content))?;
        let first = *block.ids.first().ok_or("no text")?;
        if block
            .ids
            .iter()
            .any(|&i| objs.get(i).is_none_or(|o| o.kind != ObjectKind::Text))
        {
            return Err("the page changed".into());
        }
        let rec0 = objs[first].rec.clone();
        let (full0, o0) = run_frame(&rec0).ok_or("not a text run")?;
        let ulen = (full0[0] * full0[0] + full0[1] * full0[1]).sqrt().max(1e-6);
        let u = (full0[0] / ulen, full0[1] / ulen);
        let v = (-u.1, u.0);
        // Styles: one per run, equal ones shared.
        let mut styles: Vec<RunStyle> = Vec::new();
        let mut by_run: HashMap<usize, usize> = HashMap::new();
        for &i in &block.ids {
            let Some(st) = self.style_of_run(index, &objs[i].rec) else {
                continue;
            };
            let k = styles.iter().position(|s| *s == st).unwrap_or_else(|| {
                styles.push(st);
                styles.len() - 1
            });
            by_run.insert(i, k);
        }
        if styles.is_empty() {
            return Err("not a text run".into());
        }
        let text: Vec<char> = block.text.chars().collect();
        let style_of: Vec<usize> = block
            .runs
            .iter()
            .map(|r| by_run.get(r).copied().unwrap_or(0))
            .collect();
        // Geometry: lines of the block along u from its left edge.
        let along = |p: (f32, f32)| p.0 * u.0 + p.1 * u.1;
        let across = |p: (f32, f32)| p.0 * v.0 + p.1 * v.1;
        let mut line_starts: Vec<(f32, f32)> = Vec::new();
        for &i in &block.ids {
            if let Some((_, o)) = run_frame(&objs[i].rec) {
                if line_starts
                    .last()
                    .is_none_or(|l| (across(*l) - across(o)).abs() > 0.2 * block.font_size.max(1.0))
                {
                    line_starts.push(o);
                }
            }
        }
        let size = block.font_size.max(1.0);
        let pitch = match line_starts.as_slice() {
            [a, b, ..] => across(*b) - across(*a),
            _ => {
                let tl = rec0.text.as_ref().map_or(0.0, |t| t.tl);
                if tl > 0.0 {
                    tl * (full0[2] * full0[2] + full0[3] * full0[3]).sqrt()
                } else {
                    size * 1.2
                }
            }
        };
        let left = if block.lines > 1 {
            line_starts
                .iter()
                .map(|p| along(*p))
                .fold(f32::INFINITY, f32::min)
                .min(along((block.bbox[0], 0.0)))
        } else {
            along(o0)
        };
        let right = block
            .char_boxes
            .iter()
            .map(|b| along((b[2], b[1])))
            .fold(along(o0), f32::max);
        let info = self.pages.get(index).ok_or("no such page")?;
        let mut indent = 0.0;
        let (width, origin) = if block.lines > 1 {
            // Back to a page point on the first baseline at the left edge.
            let a0 = across(o0);
            indent = (along(o0) - left).max(0.0);
            (
                (right - left) + 0.5,
                (left * u.0 + a0 * v.0, left * u.1 + a0 * v.1),
            )
        } else if upright(full0) {
            // One line grows to the right margin before it wraps.
            let margin = o0.0.clamp(0.0, 36.0);
            ((info.width - o0.0 - margin).max(right - left + 0.5), o0)
        } else {
            (f32::INFINITY, o0)
        };
        let mut prefix = content;
        let mut edits: Vec<((usize, usize), Vec<u8>)> = block
            .ids
            .iter()
            .map(|&i| (objs[i].rec.span, deleted_run(&objs[i].rec).into_bytes()))
            .collect();
        edits.sort_by_key(|e| std::cmp::Reverse(e.0 .0));
        for ((a, b), with) in edits {
            if a <= b && b <= prefix.len() {
                prefix.splice(a..b, with);
            }
        }
        self.begin_group("edit text");
        // Lines of the original layout: a new line where the baseline moves down.
        let mut line_of = Vec::with_capacity(text.len());
        let mut line = 0;
        let mut prev: Option<f32> = None;
        for b in &block.char_boxes {
            let y = across((b[0], b[3]));
            if prev.is_some_and(|p| y - p > 0.5 * size) {
                line += 1;
            }
            prev = Some(y);
            line_of.push(line);
        }
        let mut e = BlockEdit {
            page: index,
            text,
            style_of,
            styles,
            align: block.align,
            char_boxes: block.char_boxes.clone(),
            line_of,
            bbox: block.bbox,
            pitch,
            lines: block.lines,
            origin,
            width,
            indent,
            u,
            v,
            ctm: rec0.ctm,
            prefix,
            sid: None,
            metrics: HashMap::new(),
            dirty: false,
            push: None,
        };
        // Measure the paragraph's characters now: the first keystroke only lays out.
        if let Err(err) = self.measure(&mut e) {
            self.end_group();
            return Err(err);
        }
        Ok(e)
    }

    /// Encode `c` in style `k`; switches the style to an embedded bundled face when its own
    /// font has no glyph for it (returns false then: measure again).
    fn style_code(
        &mut self,
        e: &mut BlockEdit,
        k: usize,
        c: char,
    ) -> Result<Option<Vec<u8>>, String> {
        let st = &e.styles[k];
        if let Some(face) = st.face {
            let (_, bytes) = self.embed_text(face, &c.to_string())?;
            return Ok(Some(bytes));
        }
        let font = st.ts.font.clone();
        if let Some(b) = self.encode_in_font(e.page, &font, &c.to_string()) {
            return Ok(Some(b));
        }
        if c == ' ' {
            // Subset fonts often have no space glyph (words placed one by one): the space
            // only moves the next word (no codes).
            return Ok(Some(Vec::new()));
        }
        let face = self.fallback_face(e.page, &font);
        self.set_style_face(e, k, face)?;
        Ok(None)
    }

    fn set_style_face(&mut self, e: &mut BlockEdit, k: usize, face: usize) -> Result<(), String> {
        let (fid, _) = self.embed_text(face, "")?;
        let name = self.add_resource(e.page, b"Font", "BoF", fid)?;
        let st = &mut e.styles[k];
        st.face = Some(face);
        st.ts.font = name;
        st.bold = face & 1 != 0;
        st.italic = face & 2 != 0;
        st.family = if face < fallback::SANS {
            FAMILY_SERIF
        } else {
            FAMILY_SANS
        }
        .into();
        // Codes and widths differ in the new font.
        e.metrics.retain(|(s, _), _| *s != k);
        Ok(())
    }

    /// Measure the characters of `e` that have no metrics yet.
    fn measure(&mut self, e: &mut BlockEdit) -> Result<(), String> {
        loop {
            let mut missing: Vec<(usize, char)> = Vec::new();
            for (i, &c) in e.text.iter().enumerate() {
                let k = e
                    .style_of
                    .get(i)
                    .copied()
                    .unwrap_or(0)
                    .min(e.styles.len() - 1);
                if c != '\n' && !e.metrics.contains_key(&(k, c)) && !missing.contains(&(k, c)) {
                    missing.push((k, c));
                }
            }
            if missing.is_empty() {
                return Ok(());
            }
            let mut codes = Vec::with_capacity(missing.len());
            let mut restart = false;
            for &(k, c) in &missing {
                if let Some(b) = self.style_code(e, k, c)? {
                    codes.push(b);
                } else {
                    restart = true;
                    break;
                }
            }
            if restart {
                continue;
            }
            // One text object; each character its own `Tj` at the same origin.
            let inv_ctm = invert(e.ctm).ok_or("singular matrix")?;
            let base = self.page_base(e.page)?;
            let ctm_u = mul(e.ctm, invert(base).ok_or("page matrix")?);
            let mut s = format!("q {} cm BT ", mat_str(ctm_u)).into_bytes();
            let mut spans = Vec::with_capacity(missing.len());
            let mut cur: Option<usize> = None;
            for (&(k, _), code) in missing.iter().zip(&codes) {
                if code.is_empty() {
                    spans.push(usize::MAX);
                    continue;
                }
                if cur != Some(k) {
                    s.extend(style_ops(&e.styles[k]));
                    cur = Some(k);
                }
                let tm = mul(
                    [
                        e.styles[k].lin[0],
                        e.styles[k].lin[1],
                        e.styles[k].lin[2],
                        e.styles[k].lin[3],
                        e.origin.0,
                        e.origin.1,
                    ],
                    inv_ctm,
                );
                s.extend(format!("{} Tm ", mat_str(tm)).into_bytes());
                let start = s.len();
                s.extend(hex(code).into_bytes());
                s.extend_from_slice(b" Tj");
                spans.push(start);
                s.push(b' ');
            }
            s.extend_from_slice(b"ET Q\n");
            let objs = self.objects_in(e.page, Some(&s))?;
            for (n, &(k, c)) in missing.iter().enumerate() {
                let o = objs.iter().find(|o| o.rec.span.0 == spans[n]);
                let size = e.styles[k].size.max(1.0);
                let m = match o
                    .filter(|_| spans[n] != usize::MAX)
                    .and_then(|o| Some((o, o.rec.text.as_ref()?)))
                {
                    Some((o, ts)) => {
                        let a = apply(mul(ts.tm, o.rec.ctm), 0.0, 0.0);
                        let b = apply(mul(ts.tm_after, o.rec.ctm), 0.0, 0.0);
                        let adv = (b.0 - a.0) * e.u.0 + (b.1 - a.1) * e.u.1;
                        let rel = o
                            .char_boxes
                            .first()
                            .map_or([0.0, -0.8 * size, adv, 0.2 * size], |r| {
                                [r[0] - a.0, r[1] - a.1, r[2] - a.0, r[3] - a.1]
                            });
                        Metric {
                            adv,
                            rel,
                            codes: codes[n].clone(),
                        }
                    }
                    // A space without a glyph: a quarter em (a typical word space).
                    None => Metric {
                        adv: 0.25 * size,
                        rel: [0.0, -0.8 * size, 0.25 * size, 0.2 * size],
                        codes: codes[n].clone(),
                    },
                };
                e.metrics.insert((k, c), m);
            }
            return Ok(());
        }
    }

    fn page_base(&self, index: usize) -> Result<M, String> {
        let info = self.pages.get(index).ok_or("no such page")?;
        Ok(crate::page_matrix(info.crop, info.rotate))
    }

    /// Lay `e` out again and write it into the page (after its text or styles changed).
    pub fn block_edit_update(&mut self, e: &mut BlockEdit) -> Result<(), String> {
        e.style_of
            .resize(e.text.len(), e.style_of.last().copied().unwrap_or(0));
        self.measure(e)?;
        let n = e.text.len();
        // Advances looked up once (a 500-line paragraph has ~20 000 characters per key).
        let advs: Vec<f32> = (0..n)
            .map(|i| metric(e, i).map_or(0.0, |m| m.adv))
            .collect();
        // Lines: (start, end, hard end) by greedy word wrap.
        let mut lines: Vec<(usize, usize, bool)> = Vec::new();
        let mut start = 0;
        let mut x = e.indent;
        let mut brk: Option<usize> = None;
        let mut i = 0;
        while i < n {
            let c = e.text[i];
            if c == '\n' {
                lines.push((start, i + 1, true));
                start = i + 1;
                x = e.indent;
                brk = None;
                i += 1;
                continue;
            }
            let adv = advs[i];
            if c != ' ' && x + adv > e.width && i > start {
                let end = brk.filter(|&b| b > start).unwrap_or(i);
                lines.push((start, end, false));
                start = end;
                brk = None;
                x = (start..i).map(|j| advs[j]).sum();
                continue;
            }
            x += adv;
            if c == ' ' {
                brk = Some(i + 1);
            }
            i += 1;
        }
        lines.push((start, n, true));
        // Positions.
        let mut xs = vec![0.0_f32; n];
        let mut line_of = vec![0_usize; n];
        for (k, &(a, b, hard)) in lines.iter().enumerate() {
            // The first line of a paragraph starts at the indent.
            let first = k == 0 || lines[k - 1].2;
            let ind = if first { e.indent } else { 0.0 };
            let shown_end = (a..b)
                .rev()
                .find(|&j| !matches!(e.text[j], ' ' | '\n'))
                .map_or(a, |j| j + 1);
            let w: f32 = (a..shown_end).map(|j| advs[j]).sum();
            let spaces = (a..shown_end).filter(|&j| e.text[j] == ' ').count();
            let room = if e.width.is_finite() {
                e.width - 0.5 - ind - w
            } else {
                0.0
            };
            let (mut x, extra) = match e.align {
                Align::Left => (ind, 0.0),
                Align::Center => (ind + room.max(0.0) / 2.0, 0.0),
                Align::Right => (ind + room.max(0.0), 0.0),
                Align::Justify if !hard && spaces > 0 && room > 0.0 => (ind, room / spaces as f32),
                Align::Justify => (ind, 0.0),
            };
            for j in a..b {
                xs[j] = x;
                line_of[j] = k;
                if e.text[j] != '\n' {
                    x += advs[j];
                    if e.text[j] == ' ' && j < shown_end {
                        x += extra;
                    }
                }
            }
        }
        // Boxes.
        let at = |e: &BlockEdit, x: f32, line: usize| {
            let d = line as f32 * e.pitch;
            (
                e.origin.0 + x * e.u.0 + d * e.v.0,
                e.origin.1 + x * e.u.1 + d * e.v.1,
            )
        };
        let size = e.styles.first().map_or(10.0, |s| s.size);
        let mut boxes = Vec::with_capacity(n);
        let mut bbox: Option<[f32; 4]> = None;
        for j in 0..n {
            let p = at(e, xs[j], line_of[j]);
            let r = match metric(e, j) {
                Some(m) if e.text[j] != '\n' => [
                    p.0 + m.rel[0],
                    p.1 + m.rel[1],
                    p.0 + m.rel[2],
                    p.1 + m.rel[3],
                ],
                _ => [p.0, p.1 - 0.8 * size, p.0, p.1 + 0.2 * size],
            };
            bbox = Some(bbox.map_or(r, |b| {
                [
                    b[0].min(r[0]),
                    b[1].min(r[1]),
                    b[2].max(r[2]),
                    b[3].max(r[3]),
                ]
            }));
            boxes.push(r);
        }
        let p = at(e, 0.0, 0);
        e.bbox = bbox.unwrap_or([p.0, p.1 - 0.8 * size, p.0 + 1.0, p.1 + 0.2 * size]);
        e.char_boxes = boxes;
        e.line_of.clone_from(&line_of);
        e.lines = lines.len();
        // The block's text object: one `Tm` and `Tj` per piece of one style (each word
        // when justified).
        let inv_ctm = invert(e.ctm).ok_or("singular matrix")?;
        let base = self.page_base(e.page)?;
        let ctm_u = mul(e.ctm, invert(base).ok_or("page matrix")?);
        let open = format!("q {} cm BT ", mat_str(ctm_u)).into_bytes();
        let mut tail = b"\n".to_vec();
        // Each line is its own balanced chunk (`q … BT … ET Q`, full style), so the page
        // drawing keeps the lines a key did not change (`typing_fast` chunks).
        let mut starts = vec![tail.len()];
        tail.extend_from_slice(&open);
        let mut chunk_line: Option<usize> = None;
        let mut cur: Option<usize> = None;
        // Underline and strikethrough rules: style, start and end x, line.
        let mut rules: Vec<(usize, f32, f32, usize)> = Vec::new();
        let mut j = 0;
        while j < n {
            if e.text[j] == '\n' || metric(e, j).is_none_or(|m| m.codes.is_empty()) {
                j += 1;
                continue;
            }
            let k = e.style_of[j].min(e.styles.len() - 1);
            let mut end = j + 1;
            while end < n
                && e.text[end] != '\n'
                && line_of[end] == line_of[j]
                && e.style_of[end].min(e.styles.len() - 1) == k
                && metric(e, end).is_some_and(|m| !m.codes.is_empty())
                && !(e.align == Align::Justify && e.text[end - 1] == ' ')
            {
                end += 1;
            }
            if chunk_line.is_some_and(|l| l != line_of[j]) {
                tail.extend_from_slice(b"ET Q\n");
                starts.push(tail.len());
                tail.extend_from_slice(&open);
                cur = None;
            }
            chunk_line = Some(line_of[j]);
            if cur != Some(k) {
                tail.extend(style_ops(&e.styles[k]));
                cur = Some(k);
            }
            let p = at(e, xs[j], line_of[j]);
            let l = e.styles[k].lin;
            let tm = mul([l[0], l[1], l[2], l[3], p.0, p.1], inv_ctm);
            tail.extend(format!("{} Tm <", mat_str(tm)).into_bytes());
            for q in j..end {
                if let Some(m) = metric(e, q) {
                    // Hex string bytes without a format call per byte (a 500-line
                    // paragraph writes ~20 000 of them per key).
                    for b in &m.codes {
                        tail.push(HEX_DIGITS[usize::from(b >> 4)]);
                        tail.push(HEX_DIGITS[usize::from(b & 15)]);
                    }
                }
            }
            tail.extend_from_slice(b"> Tj ");
            if e.styles[k].underline || e.styles[k].strike {
                let x1 = xs[end - 1] + advs[end - 1];
                rules.push((k, xs[j], x1, line_of[j]));
            }
            j = end;
        }
        tail.extend_from_slice(b"ET ");
        for (k, x0, x1, line) in rules {
            let st = &e.styles[k];
            let size = st.size;
            let t = (size * 0.06).max(0.4);
            let mut ys = Vec::new();
            if st.underline {
                ys.push(size * 0.12);
            }
            if st.strike {
                ys.push(-size * 0.28);
            }
            for dy in ys {
                // A thin band across the line (page points), into the text object's space.
                let p0 = at(e, x0, line);
                let p1 = at(e, x1, line);
                let (vx, vy) = (e.v.0, e.v.1);
                let corners = [
                    (p0.0 + vx * (dy - t / 2.0), p0.1 + vy * (dy - t / 2.0)),
                    (p1.0 + vx * (dy - t / 2.0), p1.1 + vy * (dy - t / 2.0)),
                    (p1.0 + vx * (dy + t / 2.0), p1.1 + vy * (dy + t / 2.0)),
                    (p0.0 + vx * (dy + t / 2.0), p0.1 + vy * (dy + t / 2.0)),
                ];
                let c = st.color;
                tail.extend(
                    format!(
                        "{} {} {} rg ",
                        num(f32::from(c[0]) / 255.0),
                        num(f32::from(c[1]) / 255.0),
                        num(f32::from(c[2]) / 255.0)
                    )
                    .into_bytes(),
                );
                for (i, q) in corners.iter().enumerate() {
                    let u = apply(inv_ctm, q.0, q.1);
                    tail.extend(
                        format!(
                            "{} {} {} ",
                            num(u.0),
                            num(u.1),
                            if i == 0 { "m" } else { "l" }
                        )
                        .into_bytes(),
                    );
                }
                tail.extend_from_slice(b"h f ");
            }
        }
        tail.extend_from_slice(b"Q\n");
        let chunks: Vec<std::ops::Range<usize>> = starts
            .iter()
            .zip(starts.iter().skip(1).chain(std::iter::once(&tail.len())))
            .map(|(&a, &b)| a..b)
            .collect();
        crate::paragraph_push::update(e);
        let sid = match e.sid {
            Some(s) => s,
            None => {
                let (s, _) = self.page_stream(e.page)?;
                e.sid = Some(s);
                s
            }
        };
        let mut content = Vec::with_capacity(e.prefix.len() + tail.len());
        content.extend_from_slice(&e.prefix);
        content.extend_from_slice(&tail);
        self.set_stream_raw(sid, content);
        self.typing.note(e.page, &e.prefix);
        self.typing.note_tail(tail.len(), chunks);
        e.dirty = true;
        Ok(())
    }

    /// Change the style of characters `range` of `e` (all of them when it is empty) and lay
    /// the block out again.
    pub fn block_edit_style(
        &mut self,
        e: &mut BlockEdit,
        range: std::ops::Range<usize>,
        change: &StyleChange,
    ) -> Result<(), String> {
        let n = e.text.len();
        let range = if range.is_empty() {
            0..n
        } else {
            range.start.min(n)..range.end.min(n)
        };
        let used: Vec<usize> = {
            let mut u: Vec<usize> = range.clone().map(|i| e.style_of[i]).collect();
            u.sort_unstable();
            u.dedup();
            u
        };
        let mut map: HashMap<usize, usize> = HashMap::new();
        for k in used {
            let mut st = e.styles[k].clone();
            let mut face = st.face;
            let bundled = |serif: bool, bold: bool, italic: bool| {
                (if serif { 0 } else { fallback::SANS })
                    + usize::from(bold)
                    + 2 * usize::from(italic)
            };
            let serif_now = match st.face {
                Some(f) => f < fallback::SANS,
                None => self.fallback_face(e.page, &st.ts.font) < fallback::SANS,
            };
            match change {
                StyleChange::Bold(b) => face = Some(bundled(serif_now, *b, st.italic)),
                StyleChange::Italic(i) => face = Some(bundled(serif_now, st.bold, *i)),
                StyleChange::Serif(s) => face = Some(bundled(*s, st.bold, st.italic)),
                StyleChange::Size(s) => {
                    if st.size > 0.0 && *s > 0.0 {
                        st.ts.size *= s / st.size;
                        st.size = *s;
                    }
                }
                StyleChange::Color(c) => {
                    st.color = *c;
                    st.ts.fill = Rgba(c[0], c[1], c[2], 255);
                    st.ts.stroke = st.ts.fill;
                }
                StyleChange::Underline(on) => st.underline = *on,
                StyleChange::Strike(on) => st.strike = *on,
                StyleChange::Script(s) => {
                    // Raised or lowered by a third of the size at 65 % size (§9.3.7 Ts).
                    let full = match st.script {
                        0 => st.ts.size,
                        _ => st.ts.size / 0.65,
                    };
                    st.script = (*s).clamp(-1, 1);
                    st.ts.size = if st.script == 0 { full } else { full * 0.65 };
                    st.ts.rise = f32::from(st.script) * full * 0.33;
                    let k = (st.lin[2] * st.lin[2] + st.lin[3] * st.lin[3]).sqrt();
                    st.size = st.ts.size * k;
                }
                StyleChange::Scale(pct) => st.ts.th = (pct / 100.0).clamp(0.1, 10.0),
                StyleChange::CharSpacing(v) => st.ts.tc = v / st.along_k(),
                StyleChange::WordSpacing(v) => st.ts.tw = v / st.along_k(),
            }
            let new_face = face != st.face;
            let idx = if let Some(p) = e.styles.iter().position(|s| {
                if new_face {
                    s.face == face
                        && s.size == st.size
                        && s.color == st.color
                        && s.lin == st.lin
                        && s.underline == st.underline
                        && s.strike == st.strike
                        && s.script == st.script
                        && s.ts.tc == st.ts.tc
                        && s.ts.tw == st.ts.tw
                        && s.ts.th == st.ts.th
                        && s.ts.rise == st.ts.rise
                } else {
                    *s == st
                }
            }) {
                p
            } else {
                e.styles.push(st);
                let p = e.styles.len() - 1;
                if let (true, Some(f)) = (new_face, face) {
                    self.set_style_face(e, p, f)?;
                }
                p
            };
            map.insert(k, idx);
        }
        for i in range {
            if let Some(&t) = map.get(&e.style_of[i]) {
                e.style_of[i] = t;
            }
        }
        self.block_edit_update(e)
    }

    /// Finish editing: compress the page stream and close the undo group.
    pub fn end_block_edit(&mut self, e: &BlockEdit) {
        self.typing.clear();
        if let Some(sid) = e.sid {
            let bytes = match self.doc.get_object(sid) {
                Ok(Object::Stream(s)) if s.dict.get(b"Filter").is_err() => Some(s.content.clone()),
                _ => None,
            };
            if let Some(b) = bytes {
                self.set_stream(sid, b);
            }
        }
        self.end_group();
    }

    /// Store `content` uncompressed (while typing; compressed when editing ends).
    pub(crate) fn set_stream_raw(&mut self, id: ObjectId, content: Vec<u8>) {
        self.touch(id);
        if let Ok(Object::Stream(s)) = self.doc.get_object_mut(id) {
            s.dict.remove(b"DecodeParms");
            s.dict.remove(b"Filter");
            s.set_content(content);
        }
    }
}

/// Text state operators of a style.
fn style_ops(st: &RunStyle) -> Vec<u8> {
    let mut s = Vec::new();
    crate::write::object(&mut s, &Object::Name(st.ts.font.clone()));
    let ts = &st.ts;
    s.extend(
        format!(
            " {} Tf {} Tc {} Tw {} Tz {} Ts {} Tr {} rg {} RG ",
            num(ts.size),
            num(ts.tc),
            num(ts.tw),
            num(ts.th * 100.0),
            num(ts.rise),
            ts.mode,
            rgb(ts.fill),
            rgb(ts.stroke),
        )
        .into_bytes(),
    );
    s
}
