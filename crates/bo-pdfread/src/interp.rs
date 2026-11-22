//! Content stream interpreter (ISO 32000-1, §8 graphics, §9.4 text objects): turns page
//! content into display list commands in page points (origin top-left, y down), and records
//! every shown glyph with its box for search, selection and copy.
use crate::color::ColorSpace;
use crate::filters;
use crate::font::{Font, FontCache, GlyphRef};
use crate::function::Function;
use crate::image::{self, Image};
use crate::lexer::{Lexer, Op};
use crate::store::Document;
use crate::{RenderOptions, TextChar};
use bo_render::{BlendMode, Command, Glyph, GradientStop, Paint, PathSeg, Rgba, SoftMask, Stroke};
use bo_text::FontManager;
use lopdf::{Dictionary, Object, ObjectId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

/// Affine matrix `[a b c d e f]`: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
pub(crate) type M = [f32; 6];

pub(crate) const IDENTITY: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `a` then `b` (PDF order: `a × b`).
pub(crate) fn mul(a: M, b: M) -> M {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

pub(crate) fn apply(m: M, x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

pub(crate) fn invert(m: M) -> Option<M> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-12 || !det.is_finite() {
        return None;
    }
    let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
    Some([a, b, c, d, -(m[4] * a + m[5] * c), -(m[4] * b + m[5] * d)])
}

fn scale_of(m: M) -> f32 {
    (m[0] * m[3] - m[1] * m[2]).abs().sqrt()
}

/// Largest number of operators run for one page (hostile files).
const MAX_OPS: u64 = 4_000_000;
/// Time budget for one page.
const MAX_TIME_MS: u128 = 6_000;

/// Decoded pictures shared by the pages of a document.
#[derive(Default)]
pub(crate) struct ImageCache {
    map: HashMap<String, Option<Arc<Image>>>,
    bytes: usize,
}

/// Decoded images kept for pages drawn again (a 1 202-page manual read to the end or searched
/// decoded every image into 384 MB before; the reader keeps the pages around the view in its
/// own cache).
const IMAGE_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// The budget, or `BO_PDF_IMAGE_CACHE_MB` (BlueReader round 8 memory measurements).
fn image_cache_bytes() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("BO_PDF_IMAGE_CACHE_MB")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map_or(IMAGE_CACHE_BYTES, |mb| mb << 20)
    })
}

impl ImageCache {
    fn get_or(&mut self, key: &str, f: impl FnOnce() -> Option<Image>) -> Option<Arc<Image>> {
        if let Some(v) = self.map.get(key) {
            return v.clone();
        }
        let v = f().map(Arc::new);
        let n = v.as_ref().map_or(0, |i| i.pixels.len());
        if self.bytes + n > image_cache_bytes() {
            self.map.clear();
            self.bytes = 0;
        }
        self.bytes += n;
        self.map.insert(key.to_owned(), v.clone());
        v
    }
}

/// Document-wide state the interpreter reads and fills.
pub(crate) struct Shared<'a> {
    pub doc: &'a Document,
    pub fonts: &'a mut FontManager,
    pub font_cache: &'a mut FontCache,
    pub images: &'a mut ImageCache,
    /// Optional content groups that are off.
    pub hidden: &'a HashSet<ObjectId>,
    pub opts: RenderOptions,
}

/// What a page produced.
#[derive(Default)]
pub(crate) struct Out {
    pub commands: Vec<Command>,
    pub images: HashMap<String, Arc<Image>>,
    pub chars: Vec<TextChar>,
    /// Page objects with their content stream spans (BlueEditor), when recording.
    pub objects: Option<Vec<crate::objects::Recorded>>,
}

/// Text state at a text-showing operator (to write the run again elsewhere).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextState {
    pub font: Vec<u8>,
    pub size: f32,
    pub tm: M,
    pub tm_after: M,
    pub tc: f32,
    pub tw: f32,
    pub th: f32,
    pub tl: f32,
    pub rise: f32,
    pub mode: u8,
    pub fill: Rgba,
    pub stroke: Rgba,
}

#[derive(Clone)]
enum Res<'a> {
    Dict(&'a Dictionary),
    Owned(Rc<Font>),
}

#[derive(Clone)]
struct Gs {
    ctm: M,
    fill_cs: Arc<ColorSpace>,
    fill: Vec<f32>,
    fill_pattern: Option<Vec<u8>>,
    stroke_cs: Arc<ColorSpace>,
    stroke: Vec<f32>,
    stroke_pattern: Option<Vec<u8>>,
    lw: f32,
    cap: u8,
    join: u8,
    dash: Vec<f32>,
    fill_alpha: f32,
    stroke_alpha: f32,
    font: Option<Rc<Font>>,
    /// Resource name of the font.
    font_name: Vec<u8>,
    size: f32,
    tc: f32,
    tw: f32,
    th: f32,
    tl: f32,
    rise: f32,
    mode: u8,
    /// Blend mode (§11.3.5, ExtGState `/BM`); `None` = Normal.
    blend: Option<BlendMode>,
    /// Soft mask (§11.6.5, ExtGState `/SMask`) in page space.
    smask: Option<Rc<SoftMask>>,
    /// Clips pushed while this state was current.
    clips: u32,
    /// Overprint of fills and strokes (§8.7.4.5, ExtGState `/op` and `/OP`).
    op_fill: bool,
    op_stroke: bool,
}

impl Gs {
    fn new(ctm: M) -> Self {
        Self {
            ctm,
            fill_cs: Arc::new(ColorSpace::Gray),
            fill: vec![0.0],
            fill_pattern: None,
            stroke_cs: Arc::new(ColorSpace::Gray),
            stroke: vec![0.0],
            stroke_pattern: None,
            lw: 1.0,
            cap: 0,
            join: 0,
            dash: Vec::new(),
            fill_alpha: 1.0,
            stroke_alpha: 1.0,
            font: None,
            font_name: Vec::new(),
            size: 0.0,
            tc: 0.0,
            tw: 0.0,
            th: 1.0,
            tl: 0.0,
            rise: 0.0,
            mode: 0,
            blend: None,
            smask: None,
            clips: 0,
            op_fill: false,
            op_stroke: false,
        }
    }
}

fn rgba(cs: &ColorSpace, v: &[f32], alpha: f32) -> Rgba {
    let c = cs.rgb(v);
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba(b(c[0]), b(c[1]), b(c[2]), b(alpha))
}

fn f(o: &Object) -> f32 {
    o.as_float().unwrap_or(0.0)
}

fn transform_seg(s: PathSeg, m: M) -> PathSeg {
    let p = |x: f32, y: f32| apply(m, x, y);
    match s {
        PathSeg::M(x, y) => {
            let (a, b) = p(x, y);
            PathSeg::M(a, b)
        }
        PathSeg::L(x, y) => {
            let (a, b) = p(x, y);
            PathSeg::L(a, b)
        }
        PathSeg::Q(x1, y1, x, y) => {
            let (a, b) = p(x1, y1);
            let (c, d) = p(x, y);
            PathSeg::Q(a, b, c, d)
        }
        PathSeg::C(x1, y1, x2, y2, x, y) => {
            let (a, b) = p(x1, y1);
            let (c, d) = p(x2, y2);
            let (e, g) = p(x, y);
            PathSeg::C(a, b, c, d, e, g)
        }
        PathSeg::Z => PathSeg::Z,
    }
}

/// The axis-aligned rectangle a path is, if it is one.
fn as_rect(segs: &[PathSeg]) -> Option<[f32; 4]> {
    let pts: Vec<(f32, f32)> = match segs {
        [PathSeg::M(a, b), PathSeg::L(c, d), PathSeg::L(e, g), PathSeg::L(h, i), PathSeg::Z]
        | [PathSeg::M(a, b), PathSeg::L(c, d), PathSeg::L(e, g), PathSeg::L(h, i), PathSeg::L(_, _), PathSeg::Z] =>
        {
            vec![(*a, *b), (*c, *d), (*e, *g), (*h, *i)]
        }
        _ => return None,
    };
    let eq = |x: f32, y: f32| (x - y).abs() < 0.01;
    let horizontal_first = eq(pts[0].1, pts[1].1)
        && eq(pts[1].0, pts[2].0)
        && eq(pts[2].1, pts[3].1)
        && eq(pts[3].0, pts[0].0);
    let vertical_first = eq(pts[0].0, pts[1].0)
        && eq(pts[1].1, pts[2].1)
        && eq(pts[2].0, pts[3].0)
        && eq(pts[3].1, pts[0].1);
    if !horizontal_first && !vertical_first {
        return None;
    }
    let x0 = pts.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
    let x1 = pts.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
    let y0 = pts.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
    let y1 = pts.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
    Some([x0, y0, x1 - x0, y1 - y0])
}

/// One content stream run (page, form, Type 3 glyph, annotation appearance).
pub(crate) struct Interp<'s, 'a> {
    sh: &'s mut Shared<'a>,
    pub out: Out,
    gs: Gs,
    stack: Vec<Gs>,
    res: Vec<Res<'a>>,
    path: Vec<PathSeg>,
    /// Current point and subpath start in user space.
    cur: (f32, f32),
    start: (f32, f32),
    clip: Option<bool>,
    /// Knockout transparency groups being run (§11.4.6.2).
    knockout_depth: u32,
    /// Glyph outlines of text shown with a clipping render mode (4–7) since BT (§9.3.6).
    text_clip: Option<Vec<PathSeg>>,
    tm: M,
    tlm: M,
    depth: u32,
    ops: u64,
    started: Instant,
    /// Marked content nesting: whether each level hides its content.
    marked: Vec<bool>,
    /// `/BOLine <</N n>> BDC` sequences open (BlueEditor's split text objects), innermost last.
    bo_marks: Vec<Option<i64>>,
    hidden_depth: u32,
    /// Form XObjects being run (cycle guard).
    forms: Vec<ObjectId>,
    inline_count: u32,
    /// Page size in points (shadings fill the clip of the page).
    page: (f32, f32),
    /// Type 3 glyph with `d1`: colours come from the text state.
    uncolored: bool,
    image_prefix: String,
    /// Where the clip pushed by the last path painting operator starts.
    clip_mark: Option<usize>,
    /// Colour of all shown text, fill and stroke (BlueReader Night Mode); alpha is kept.
    pub(crate) text_color: Option<Rgba>,
    /// Simulate overprinting ([`crate::RenderExtra::overprint`]).
    pub(crate) overprint: bool,
}

impl<'s, 'a> Interp<'s, 'a> {
    pub(crate) fn new(
        sh: &'s mut Shared<'a>,
        base: M,
        resources: Option<&'a Dictionary>,
        page: (f32, f32),
        image_prefix: String,
    ) -> Self {
        Self {
            sh,
            out: Out::default(),
            gs: Gs::new(base),
            stack: Vec::new(),
            res: resources.map(Res::Dict).into_iter().collect(),
            path: Vec::new(),
            cur: (0.0, 0.0),
            start: (0.0, 0.0),
            clip: None,
            knockout_depth: 0,
            text_clip: None,
            tm: IDENTITY,
            tlm: IDENTITY,
            depth: 0,
            ops: 0,
            started: Instant::now(),
            marked: Vec::new(),
            bo_marks: Vec::new(),
            hidden_depth: 0,
            forms: Vec::new(),
            inline_count: 0,
            page,
            uncolored: false,
            image_prefix,
            clip_mark: None,
            text_color: None,
            overprint: false,
        }
    }

    fn doc(&self) -> &'a Document {
        self.sh.doc
    }

    /// Look a resource up from the innermost resource dictionary outwards.
    fn resource(&self, category: &[u8], name: &[u8]) -> Option<Object> {
        let doc = self.doc();
        for r in self.res.iter().rev() {
            let dict: Option<&Dictionary> = match r {
                Res::Dict(d) => Some(d),
                Res::Owned(font) => font.type3.as_ref().and_then(|t| t.resources.as_ref()),
            };
            let Some(d) = dict else { continue };
            let Ok(cat) = d.get(category) else { continue };
            let cat = doc.dereference(cat).map_or(cat, |(_, o)| o);
            if let Ok(cd) = cat.as_dict() {
                if let Ok(v) = cd.get(name) {
                    return Some(v.clone());
                }
            }
        }
        None
    }

    fn color_space(&self, o: &Object) -> Option<ColorSpace> {
        let lookup = |n: &[u8]| self.resource(b"ColorSpace", n);
        ColorSpace::parse(self.doc(), o, &lookup, 0)
    }

    /// Whether `o` (or the named `/ColorSpace` resource) is a Separation / DeviceN array.
    fn is_tint_space(&self, o: &Object) -> bool {
        let doc = self.doc();
        let o = match o {
            Object::Name(n) => match self.resource(b"ColorSpace", n) {
                Some(v) => v,
                None => return false,
            },
            other => other.clone(),
        };
        let o = doc.dereference(&o).map_or(o.clone(), |(_, v)| v.clone());
        o.as_array().ok().and_then(|a| a.first()).is_some_and(|k| {
            let k = doc.dereference(k).map_or(k, |(_, v)| v);
            matches!(k.as_name(), Ok(b"Separation" | b"DeviceN"))
        })
    }

    fn hidden(&self) -> bool {
        self.hidden_depth > 0
    }

    /// Run a content stream.
    pub(crate) fn run(&mut self, content: &[u8]) {
        let mut lexer = Lexer::new(content);
        let mut path_start: Option<usize> = None;
        while let Some(op) = lexer.next() {
            self.ops += 1;
            if self.ops > MAX_OPS
                || (self.ops.is_multiple_of(4096)
                    && self.started.elapsed().as_millis() > MAX_TIME_MS)
            {
                break;
            }
            if self.out.objects.is_some() && self.depth == 0 {
                self.recorded_op(&op, lexer.span(), &mut path_start);
            } else {
                self.op(&op);
            }
        }
    }

    /// Run one top-level operator and note the page object it drew (BlueEditor).
    fn recorded_op(&mut self, op: &Op, span: (usize, usize), path_start: &mut Option<usize>) {
        use crate::objects::{ObjectKind, Recorded};
        let name = op.name.as_slice();
        let (n_cmds, n_chars) = (self.out.commands.len(), self.out.chars.len());
        let show = matches!(name, b"Tj" | b"TJ" | b"'" | b"\"");
        let before = show.then(|| (self.tm, self.gs.clone()));
        let clipping = self.clip.is_some();
        if matches!(name, b"m" | b"re") && path_start.is_none() {
            *path_start = Some(span.0);
        }
        let ctm = self.gs.ctm;
        self.op(op);
        let mut start = span.0;
        let kind = match name {
            b"Tj" | b"TJ" | b"'" | b"\"" => Some(ObjectKind::Text),
            b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" | b"n" => {
                let first = path_start.take();
                // Clipping paths and `n` belong to the content around them.
                match first {
                    Some(p) if name != b"n" && !clipping => {
                        start = p;
                        Some(ObjectKind::Path)
                    }
                    _ => None,
                }
            }
            b"Do" => Some(
                if self.out.commands[n_cmds..]
                    .iter()
                    .any(|c| matches!(c, Command::Image { .. }))
                {
                    ObjectKind::Image
                } else {
                    ObjectKind::Form
                },
            ),
            b"BI" => Some(ObjectKind::Image),
            b"sh" => Some(ObjectKind::Shading),
            _ => None,
        };
        let Some(kind) = kind else { return };

        let text_state = before.map(|(tm, gs)| TextState {
            font: gs.font_name.clone(),
            size: gs.size,
            tm,
            tm_after: self.tm,
            tc: gs.tc,
            tw: gs.tw,
            th: gs.th,
            tl: gs.tl,
            rise: gs.rise,
            mode: gs.mode,
            fill: rgba(&gs.fill_cs, &gs.fill, 1.0),
            stroke: rgba(&gs.stroke_cs, &gs.stroke, 1.0),
        });
        let rec = Recorded {
            kind,
            span: (start, span.1),
            commands: (n_cmds, self.out.commands.len()),
            chars: (n_chars, self.out.chars.len()),
            ctm,
            op: op.name.clone(),
            args: op.args.clone(),
            text: text_state,
            mark: self.bo_marks.iter().rev().find_map(|m| *m),
        };
        if let Some(list) = self.out.objects.as_mut() {
            list.push(rec);
        }
    }

    /// Close every clip and state still open.
    /// Whether the run left the graphics state as it started from `base`: nothing saved,
    /// clipped or marked is still open (BlueEditor's kept page drawing, `typing_fast`).
    pub(crate) fn at_rest(&self, base: M) -> bool {
        self.stack.is_empty()
            && self.gs.clips == 0
            && self.gs.ctm == base
            && self.marked.is_empty()
            && self.bo_marks.is_empty()
            && self.hidden_depth == 0
    }

    pub(crate) fn finish(&mut self) {
        while !self.stack.is_empty() {
            self.restore();
        }
        for _ in 0..self.gs.clips {
            self.out.commands.push(Command::PopClip);
        }
        self.gs.clips = 0;
    }

    fn save(&mut self) {
        if self.stack.len() < 256 {
            let mut copy = self.gs.clone();
            copy.clips = 0;
            let parent = std::mem::replace(&mut self.gs, copy);
            self.stack.push(parent);
        }
    }

    fn restore(&mut self) {
        let Some(prev) = self.stack.pop() else {
            return;
        };
        for _ in 0..self.gs.clips {
            self.out.commands.push(Command::PopClip);
        }
        self.gs = prev;
    }

    fn move_to(&mut self, x: f32, y: f32) {
        self.cur = (x, y);
        self.start = (x, y);
        let (a, b) = apply(self.gs.ctm, x, y);
        self.path.push(PathSeg::M(a, b));
    }

    /// No open subpath: the path is empty or its last subpath was closed (`h`), after which
    /// the current point is the closed subpath's start (ISO 32000-1 §8.5.2.1).
    fn needs_move(&self) -> bool {
        matches!(self.path.last(), None | Some(PathSeg::Z))
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if self.needs_move() {
            self.move_to(self.cur.0, self.cur.1);
        }
        self.cur = (x, y);
        let (a, b) = apply(self.gs.ctm, x, y);
        self.path.push(PathSeg::L(a, b));
    }

    fn curve_to(&mut self, p: [f32; 6]) {
        if self.needs_move() {
            self.move_to(self.cur.0, self.cur.1);
        }
        self.cur = (p[4], p[5]);
        let m = self.gs.ctm;
        let (a, b) = apply(m, p[0], p[1]);
        let (c, d) = apply(m, p[2], p[3]);
        let (e, g) = apply(m, p[4], p[5]);
        self.path.push(PathSeg::C(a, b, c, d, e, g));
    }

    fn close(&mut self) {
        if !self.needs_move() {
            self.path.push(PathSeg::Z);
            self.cur = self.start;
        }
    }

    fn fill_paint(&mut self) -> Option<Paint> {
        if self.gs.fill_cs.paints_nothing() {
            return None;
        }
        if let Some(name) = self.gs.fill_pattern.clone() {
            return self.pattern_paint(&name);
        }
        Some(Paint::Solid {
            color: rgba(&self.gs.fill_cs, &self.gs.fill, self.gs.fill_alpha),
        })
    }

    fn fill_rgba(&self) -> Rgba {
        rgba(&self.gs.fill_cs, &self.gs.fill, self.gs.fill_alpha)
    }

    fn stroke_style(&self) -> Option<Stroke> {
        if self.gs.stroke_cs.paints_nothing() {
            return None;
        }
        let s = scale_of(self.gs.ctm);
        let width = (self.gs.lw * s).max(self.sh.opts.min_line);
        // Round 14: a line thinner than the one device pixel it is widened to keeps its ink
        // (the pixel's coverage), as the reference reader draws fine grids at low zoom. A
        // zero-width line (§8.4.3.2, the thinnest line) stays solid.
        // Measured: at 60 % a 0.25 pt line is still solid there; only pages drawn very small
        // (under 1 px for 4 pt, a 200-inch page fitted) fade their fine lines.
        let thin = if self.gs.lw > 0.0
            && self.sh.opts.min_line > 4.0
            && self.gs.lw * s < self.sh.opts.min_line
        {
            (self.gs.lw * s / self.sh.opts.min_line.max(1e-6)).max(0.15)
        } else {
            1.0
        };
        let color = if self.gs.stroke_pattern.is_some() {
            Rgba(128, 128, 128, (self.gs.stroke_alpha * thin * 255.0) as u8)
        } else {
            rgba(
                &self.gs.stroke_cs,
                &self.gs.stroke,
                self.gs.stroke_alpha * thin,
            )
        };
        let dash: Vec<f32> = if self.gs.dash.iter().all(|d| *d <= 0.0) {
            Vec::new()
        } else {
            self.gs.dash.iter().map(|d| (d * s).max(0.01)).collect()
        };
        Some(Stroke {
            width,
            color,
            dash,
            cap: ["butt", "round", "square"][usize::from(self.gs.cap.min(2))].to_owned(),
            join: ["miter", "round", "bevel"][usize::from(self.gs.join.min(2))].to_owned(),
        })
    }

    /// Paint the current path, then apply a pending clip and start a new path.
    fn paint(&mut self, fill: bool, stroke: bool) {
        // A stroke painted with a pattern (§8.7.3): its outline filled with the pattern
        // (before: a grey line).
        if stroke
            && self.gs.stroke_pattern.is_some()
            && !self.hidden()
            && !self.path.is_empty()
            && !self.gs.stroke_cs.paints_nothing()
        {
            let s = scale_of(self.gs.ctm);
            let width = (self.gs.lw * s).max(self.sh.opts.min_line);
            let dash: Vec<f32> = if self.gs.dash.iter().all(|d| *d <= 0.0) {
                Vec::new()
            } else {
                self.gs.dash.iter().map(|d| (d * s).max(0.01)).collect()
            };
            let outline =
                crate::stroke_outline::outline(&self.path, width, self.gs.cap, self.gs.join, &dash);
            let clip = self.clip.take();
            let segs = self.path.clone();
            if fill {
                self.paint(true, false);
            }
            let swap = |gs: &mut Gs| {
                std::mem::swap(&mut gs.fill_cs, &mut gs.stroke_cs);
                std::mem::swap(&mut gs.fill, &mut gs.stroke);
                std::mem::swap(&mut gs.fill_pattern, &mut gs.stroke_pattern);
                std::mem::swap(&mut gs.fill_alpha, &mut gs.stroke_alpha);
            };
            swap(&mut self.gs);
            self.path = outline;
            self.paint(true, false);
            swap(&mut self.gs);
            if clip.is_some() {
                self.clip = clip;
                self.path = segs;
                self.paint(false, false);
            }
            return;
        }
        let segs = std::mem::take(&mut self.path);
        if !self.hidden() && !segs.is_empty() && (fill || stroke) {
            let tiling = fill
                && self
                    .gs
                    .fill_pattern
                    .clone()
                    .is_some_and(|n| self.is_tiling(&n));
            let mesh = if fill && !tiling {
                self.gs
                    .fill_pattern
                    .clone()
                    .and_then(|n| self.mesh_pattern(&n))
            } else {
                None
            };
            if let Some((shading, m)) = mesh {
                // A mesh or function shading pattern: its triangles clipped to the path.
                self.push_clip(&segs);
                self.mesh_fill(&shading, m);
                self.out.commands.push(Command::PopClip);
                self.gs.clips = self.gs.clips.saturating_sub(1);
                if stroke {
                    let st = self.stroke_style();
                    self.out.commands.push(Command::Path {
                        segs: segs.clone(),
                        fill: None,
                        stroke: st,
                    });
                }
            } else if tiling {
                if let Some(n) = self.gs.fill_pattern.clone() {
                    self.tiling_fill(&n, &segs);
                }
                if stroke {
                    let st = self.stroke_style();
                    self.out.commands.push(Command::Path {
                        segs: segs.clone(),
                        fill: None,
                        stroke: st,
                    });
                }
            } else {
                let fill = if fill { self.fill_paint() } else { None };
                let stroke = if stroke { self.stroke_style() } else { None };
                if fill.is_some() || stroke.is_some() {
                    self.out.commands.push(Command::Path {
                        segs: segs.clone(),
                        fill,
                        stroke,
                    });
                }
            }
        }
        if let Some(even_odd) = self.clip.take() {
            self.clip_mark = Some(self.out.commands.len());
            if even_odd {
                // `W*` (§8.5.4): the even-odd clip as a nonzero one (`evenodd`).
                self.push_clip(&crate::evenodd::nonzero_equivalent(&segs));
            } else {
                self.push_clip(&segs);
            }
        }
    }

    fn push_clip(&mut self, segs: &[PathSeg]) {
        let cmd = match as_rect(segs) {
            Some([x, y, w, h]) => Command::PushClip { x, y, w, h },
            None if segs
                .iter()
                .any(|s| matches!(s, PathSeg::L(..) | PathSeg::C(..) | PathSeg::Q(..))) =>
            {
                Command::PushClipPath {
                    segs: segs.to_vec(),
                }
            }
            None => Command::PushClip {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
        };
        self.out.commands.push(cmd);
        self.gs.clips += 1;
    }

    /// Run one operator; painting operators under a blend mode or soft mask are wrapped in a
    /// group (§11.3.5 / §11.6.5: each object is composited on its own). Form XObjects are
    /// treated as transparency groups: their content starts with Normal and no soft mask.
    fn op(&mut self, op: &Op) {
        let painting = matches!(
            op.name.as_slice(),
            b"S" | b"s"
                | b"f"
                | b"F"
                | b"f*"
                | b"B"
                | b"B*"
                | b"b"
                | b"b*"
                | b"sh"
                | b"Do"
                | b"BI"
                | b"Tj"
                | b"'"
                | b"\""
                | b"TJ"
        );
        // Overprint simulation: an overprinting CMYK / spot colour multiplies with the
        // backdrop (subtractive inks add up), the usual on-screen approximation.
        let simulate = painting && self.overprint && self.gs.blend.is_none() && {
            let stroke_only = matches!(op.name.as_slice(), b"S" | b"s");
            let (on, cs) = if stroke_only {
                (self.gs.op_stroke, &self.gs.stroke_cs)
            } else {
                (self.gs.op_fill, &self.gs.fill_cs)
            };
            on && cs.is_subtractive()
        };
        if simulate {
            self.gs.blend = Some(BlendMode::Multiply);
        }
        // Inside a knockout group each object composites with the group's initial backdrop,
        // not with the objects before it: for a blend mode over the (mostly white or
        // transparent) start this is close to Normal, and the knockout layer (primitive blend
        // copy) cannot read a backdrop for a blended object, so blending is left out there
        // (measured against the reference: `/K true` groups with `/BM /Multiply` inside).
        let blend_here = self.gs.blend.is_some() && self.knockout_depth == 0;
        if !painting || (!blend_here && self.gs.smask.is_none()) {
            self.op_inner(op);
            return;
        }
        let blend = self.gs.blend.take();
        let group_blend = blend.filter(|_| self.knockout_depth == 0);
        let mask = self.gs.smask.take();
        let start = self.out.commands.len();
        self.clip_mark = None;
        self.op_inner(op);
        let end = self.clip_mark.take().unwrap_or(self.out.commands.len());
        if end > start {
            self.out.commands.insert(end, Command::PopGroup);
            self.out.commands.insert(
                start,
                Command::PushGroup {
                    blend: group_blend,
                    mask: mask.as_ref().map(|m| Box::new(SoftMask::clone(m))),
                    knockout: false,
                    isolated: false,
                },
            );
        }
        self.gs.blend = if simulate { None } else { blend };
        self.gs.smask = mask;
    }

    #[allow(clippy::too_many_lines)]
    fn op_inner(&mut self, op: &Op) {
        let a = &op.args;
        let n = |i: usize| a.get(i).map_or(0.0, f);
        match op.name.as_slice() {
            b"q" => self.save(),
            b"Q" => self.restore(),
            b"cm" if a.len() >= 6 => {
                let m = [n(0), n(1), n(2), n(3), n(4), n(5)];
                if m.iter().all(|v| v.is_finite()) {
                    self.gs.ctm = mul(m, self.gs.ctm);
                }
            }
            b"w" => self.gs.lw = n(0).abs(),
            b"J" => self.gs.cap = n(0).clamp(0.0, 2.0) as u8,
            b"j" => self.gs.join = n(0).clamp(0.0, 2.0) as u8,
            b"d" => {
                self.gs.dash = a
                    .first()
                    .and_then(|o| o.as_array().ok())
                    .map(|v| v.iter().map(f).collect())
                    .unwrap_or_default();
            }
            b"gs" => {
                if let Some(Object::Name(name)) = a.first() {
                    if let Some(o) = self.resource(b"ExtGState", name) {
                        self.ext_gstate(&o);
                    }
                }
            }
            // Path construction.
            b"m" => self.move_to(n(0), n(1)),
            b"l" => self.line_to(n(0), n(1)),
            b"c" => self.curve_to([n(0), n(1), n(2), n(3), n(4), n(5)]),
            b"v" => {
                let (x, y) = self.cur;
                self.curve_to([x, y, n(0), n(1), n(2), n(3)]);
            }
            b"y" => self.curve_to([n(0), n(1), n(2), n(3), n(2), n(3)]),
            b"h" => self.close(),
            b"re" => {
                let (x, y, w, h) = (n(0), n(1), n(2), n(3));
                self.move_to(x, y);
                self.line_to(x + w, y);
                self.line_to(x + w, y + h);
                self.line_to(x, y + h);
                self.close();
            }
            // Painting.
            b"S" => self.paint(false, true),
            b"s" => {
                self.close();
                self.paint(false, true);
            }
            b"f" | b"F" => self.paint(true, false),
            b"B" => self.paint(true, true),
            b"b" => {
                self.close();
                self.paint(true, true);
            }
            // Even-odd fills (§8.5.3.3.3): re-oriented for the nonzero rule (`evenodd`).
            b"f*" | b"B*" | b"b*" => {
                if op.name == b"b*" {
                    self.close();
                }
                let stroke = op.name != b"f*";
                // A dashed outline keeps its direction (the dash phase starts at the start).
                if !stroke || self.gs.dash.is_empty() {
                    self.path = crate::evenodd::nonzero_equivalent(&self.path);
                }
                self.paint(true, stroke);
            }
            b"n" => self.paint(false, false),
            b"W" | b"W*" => self.clip = Some(op.name == b"W*"),
            // Colour.
            b"CS" | b"cs" => {
                let parsed = a.first().and_then(|o| self.color_space(o));
                // A Separation / DeviceN space whose tint transform is unusable is ignored:
                // the previous space stays current (measured on the reference reader).
                if parsed.is_none() && a.first().is_some_and(|o| self.is_tint_space(o)) {
                    return;
                }
                let cs = parsed.unwrap_or(ColorSpace::Gray);
                let init = cs.initial();
                if op.name == b"CS" {
                    self.gs.stroke_cs = Arc::new(cs);
                    self.gs.stroke = init;
                    self.gs.stroke_pattern = None;
                } else {
                    self.gs.fill_cs = Arc::new(cs);
                    self.gs.fill = init;
                    self.gs.fill_pattern = None;
                }
            }
            b"SC" | b"SCN" | b"sc" | b"scn" => {
                let stroke = op.name[0] == b'S';
                let vals: Vec<f32> = a.iter().filter_map(|o| o.as_float().ok()).collect();
                let pattern = match a.last() {
                    Some(Object::Name(p)) => Some(p.clone()),
                    _ => None,
                };
                if stroke {
                    self.gs.stroke = vals;
                    self.gs.stroke_pattern = pattern;
                } else {
                    self.gs.fill = vals;
                    self.gs.fill_pattern = pattern;
                }
            }
            b"G" | b"g" | b"RG" | b"rg" | b"K" | b"k" => {
                let cs = match op.name[0] {
                    b'G' | b'g' => ColorSpace::Gray,
                    b'R' | b'r' => ColorSpace::Rgb,
                    _ => ColorSpace::Cmyk,
                };
                let vals: Vec<f32> = a.iter().filter_map(|o| o.as_float().ok()).collect();
                if op.name[0].is_ascii_uppercase() {
                    self.gs.stroke_cs = Arc::new(cs);
                    self.gs.stroke = vals;
                    self.gs.stroke_pattern = None;
                } else {
                    self.gs.fill_cs = Arc::new(cs);
                    self.gs.fill = vals;
                    self.gs.fill_pattern = None;
                }
            }
            // Text.
            b"BT" => {
                self.tm = IDENTITY;
                self.tlm = IDENTITY;
                self.text_clip = None;
            }
            // Text shown with a clipping render mode adds its outlines to the clip at ET.
            b"ET" => {
                if let Some(segs) = self.text_clip.take() {
                    self.push_clip(&segs);
                }
            }
            b"Tc" => self.gs.tc = n(0),
            b"Tw" => self.gs.tw = n(0),
            b"Tz" => self.gs.th = n(0) / 100.0,
            b"TL" => self.gs.tl = n(0),
            b"Ts" => self.gs.rise = n(0),
            b"Tr" => self.gs.mode = n(0).clamp(0.0, 7.0) as u8,
            b"Tf" => {
                self.gs.size = n(1);
                if let Some(Object::Name(name)) = a.first() {
                    self.gs.font = self.load_font(name);
                    self.gs.font_name.clone_from(name);
                }
            }
            b"Td" => self.next_line(n(0), n(1)),
            b"TD" => {
                self.gs.tl = -n(1);
                self.next_line(n(0), n(1));
            }
            b"Tm" if a.len() >= 6 => {
                self.tm = [n(0), n(1), n(2), n(3), n(4), n(5)];
                self.tlm = self.tm;
            }
            b"T*" => self.next_line(0.0, -self.gs.tl),
            b"Tj" => {
                if let Some(Object::String(s, _)) = a.first() {
                    self.show(s);
                }
            }
            b"'" => {
                self.next_line(0.0, -self.gs.tl);
                if let Some(Object::String(s, _)) = a.first() {
                    self.show(s);
                }
            }
            b"\"" => {
                self.gs.tw = n(0);
                self.gs.tc = n(1);
                self.next_line(0.0, -self.gs.tl);
                if let Some(Object::String(s, _)) = a.get(2) {
                    self.show(s);
                }
            }
            b"TJ" => {
                if let Some(Object::Array(items)) = a.first() {
                    for it in items {
                        match it {
                            Object::String(s, _) => self.show(s),
                            o => {
                                let v = o.as_float().unwrap_or(0.0);
                                let fs = self.gs.size;
                                let vertical = self.gs.font.as_ref().is_some_and(|f| f.vertical);
                                let adj = -v / 1000.0 * fs;
                                if vertical {
                                    self.tm = mul([1.0, 0.0, 0.0, 1.0, 0.0, adj], self.tm);
                                } else {
                                    self.tm =
                                        mul([1.0, 0.0, 0.0, 1.0, adj * self.gs.th, 0.0], self.tm);
                                }
                            }
                        }
                    }
                }
            }
            b"d0" => {}
            b"d1" => self.uncolored = true,
            // Shading, XObjects, inline images.
            b"sh" => {
                if let Some(Object::Name(name)) = a.first() {
                    self.shade(name);
                }
            }
            b"Do" => {
                if let Some(Object::Name(name)) = a.first() {
                    self.do_xobject(name);
                }
            }
            b"BI" => {
                if let (Some(Object::Dictionary(d)), Some(data)) = (a.first(), op.data.as_ref()) {
                    self.inline_image(d, data);
                }
            }
            // Optional content.
            b"BDC" | b"BMC" => {
                let hide = op.name == b"BDC"
                    && matches!(a.first(), Some(Object::Name(t)) if t == b"OC")
                    && self.oc_hidden(a.get(1));
                self.marked.push(hide);
                if hide {
                    self.hidden_depth += 1;
                }
                // BlueEditor's split text objects: `/BOLine <</N n>> BDC … EMC`.
                let line = match (op.name.as_slice(), a.first(), a.get(1)) {
                    (b"BDC", Some(Object::Name(t)), Some(Object::Dictionary(d)))
                        if t == b"BOLine" =>
                    {
                        d.get(b"N").ok().and_then(|n| n.as_i64().ok())
                    }
                    _ => None,
                };
                self.bo_marks.push(line);
            }
            b"EMC" => {
                self.bo_marks.pop();
                if self.marked.pop() == Some(true) {
                    self.hidden_depth = self.hidden_depth.saturating_sub(1);
                }
            }
            _ => {}
        }
    }

    fn oc_hidden(&self, prop: Option<&Object>) -> bool {
        let obj = match prop {
            Some(Object::Name(n)) => match self.resource(b"Properties", n) {
                Some(o) => o,
                None => return false,
            },
            Some(o) => o.clone(),
            None => return false,
        };
        self.ocg_off(&obj)
    }

    /// Whether an optional content group (or membership dictionary) is off.
    fn ocg_off(&self, o: &Object) -> bool {
        let doc = self.doc();
        if let Object::Reference(id) = o {
            if self.sh.hidden.contains(id) {
                return true;
            }
        }
        let Ok((_, Object::Dictionary(d))) = doc.dereference(o) else {
            return false;
        };
        if d.get(b"Type").and_then(Object::as_name).ok() == Some(b"OCMD") {
            let groups: Vec<ObjectId> = match d.get(b"OCGs") {
                Ok(Object::Reference(r)) => vec![*r],
                Ok(Object::Array(a)) => a.iter().filter_map(|o| o.as_reference().ok()).collect(),
                _ => return false,
            };
            let policy = d.get(b"P").and_then(Object::as_name).unwrap_or(b"AnyOn");
            let off = |g: &ObjectId| self.sh.hidden.contains(g);
            return match policy {
                b"AllOn" => groups.iter().any(off),
                b"AnyOff" => !groups.iter().any(off),
                b"AllOff" => !groups.iter().all(off),
                _ => !groups.is_empty() && groups.iter().all(off),
            };
        }
        false
    }

    fn ext_gstate(&mut self, o: &Object) {
        let doc = self.doc();
        let Ok((_, Object::Dictionary(d))) = doc.dereference(o) else {
            return;
        };
        for (k, v) in d {
            let v = doc.dereference(v).map_or(v, |(_, o)| o);
            match k.as_slice() {
                b"LW" => self.gs.lw = f(v).abs(),
                b"LC" => self.gs.cap = f(v).clamp(0.0, 2.0) as u8,
                b"LJ" => self.gs.join = f(v).clamp(0.0, 2.0) as u8,
                b"D" => {
                    if let Ok(arr) = v.as_array() {
                        self.gs.dash = arr
                            .first()
                            .and_then(|o| o.as_array().ok())
                            .map(|v| v.iter().map(f).collect())
                            .unwrap_or_default();
                    }
                }
                // `/OP` sets both unless `/op` is given too (§8.4.5 Table 58).
                b"OP" => {
                    let on = v.as_bool().unwrap_or(false);
                    self.gs.op_stroke = on;
                    if d.get(b"op").is_err() {
                        self.gs.op_fill = on;
                    }
                }
                b"op" => self.gs.op_fill = v.as_bool().unwrap_or(false),
                b"CA" => self.gs.stroke_alpha = f(v).clamp(0.0, 1.0),
                b"ca" => self.gs.fill_alpha = f(v).clamp(0.0, 1.0),
                b"BM" => {
                    self.gs.blend = match v {
                        Object::Name(n) => BlendMode::from_pdf_name(n),
                        Object::Array(a) => a
                            .iter()
                            .filter_map(|o| o.as_name().ok())
                            .find_map(BlendMode::from_pdf_name),
                        _ => None,
                    };
                }
                b"SMask" => {
                    self.gs.smask = match v {
                        Object::Dictionary(m) => self.soft_mask(m).map(Rc::new),
                        _ => None,
                    };
                }
                b"Font" => {
                    if let Ok(arr) = v.as_array() {
                        if let (Some(fo), Some(size)) = (arr.first(), arr.get(1)) {
                            self.gs.size = f(size);
                            self.gs.font = self.sh.font_cache.get(doc, fo, self.sh.fonts);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Draw a soft mask's group (§11.6.5.2) with the current matrix into its own commands.
    fn soft_mask(&mut self, d: &Dictionary) -> Option<SoftMask> {
        let doc = self.doc();
        let luminosity = d.get(b"S").and_then(Object::as_name).ok() != Some(b"Alpha");
        let id = d.get(b"G").ok()?.as_reference().ok()?;
        if self.depth > 12 || self.forms.contains(&id) {
            return None;
        }
        let Object::Stream(s) = doc.get_object(id).ok()? else {
            return None;
        };
        let content = filters::stream_bytes(doc, s).ok()?;
        let num_array = |o: &Object| -> Vec<f32> {
            doc.dereference(o)
                .map_or(o, |(_, o)| o)
                .as_array()
                .map(|a| a.iter().map(f).collect())
                .unwrap_or_default()
        };
        let matrix = s
            .dict
            .get(b"Matrix")
            .ok()
            .map(num_array)
            .and_then(|v| <[f32; 6]>::try_from(v.as_slice()).ok())
            .unwrap_or(IDENTITY);
        let bbox = s.dict.get(b"BBox").ok().map(num_array);
        let res = s
            .dict
            .get(b"Resources")
            .ok()
            .map(|o| doc.dereference(o).map_or(o, |(_, o)| o))
            .and_then(|o| o.as_dict().ok());
        // Backdrop colour in the group's colour space (default black: nothing shows outside).
        let space = s
            .dict
            .get(b"Group")
            .ok()
            .map(|o| doc.dereference(o).map_or(o, |(_, o)| o))
            .and_then(|o| o.as_dict().ok())
            .and_then(|g| g.get(b"CS").ok())
            .and_then(|o| self.color_space(o));
        let bc = d.get(b"BC").ok().map(num_array).unwrap_or_default();
        let backdrop = match (&space, bc.is_empty()) {
            (Some(cs), false) => rgba(cs, &bc, 1.0),
            (None, false) if bc.len() == 1 => rgba(&ColorSpace::Gray, &bc, 1.0),
            (None, false) if bc.len() == 3 => rgba(&ColorSpace::Rgb, &bc, 1.0),
            (None, false) if bc.len() == 4 => rgba(&ColorSpace::Cmyk, &bc, 1.0),
            _ => Rgba(0, 0, 0, 255),
        };
        let saved = std::mem::take(&mut self.out.commands);
        let chars = self.out.chars.len();
        let path = std::mem::take(&mut self.path);
        self.save();
        let ctm = self.gs.ctm;
        self.gs = Gs::new(ctm);
        self.forms.push(id);
        self.run_form(&content, matrix, bbox.as_deref(), res);
        self.forms.pop();
        self.restore();
        self.path = path;
        self.out.chars.truncate(chars);
        let commands = std::mem::replace(&mut self.out.commands, saved);
        Some(SoftMask {
            luminosity,
            backdrop,
            commands,
        })
    }

    fn load_font(&mut self, name: &[u8]) -> Option<Rc<Font>> {
        let o = self.resource(b"Font", name)?;
        let doc = self.doc();
        self.sh.font_cache.get(doc, &o, self.sh.fonts)
    }

    fn next_line(&mut self, tx: f32, ty: f32) {
        self.tlm = mul([1.0, 0.0, 0.0, 1.0, tx, ty], self.tlm);
        self.tm = self.tlm;
    }

    /// Show a string with the current font (§9.4.4).
    #[allow(clippy::too_many_lines)]
    fn show(&mut self, s: &[u8]) {
        let Some(font) = self.gs.font.clone() else {
            return;
        };
        let fs = self.gs.size;
        let th = self.gs.th;
        let mode = self.gs.mode;
        let visible = !self.hidden() && !matches!(mode, 3 | 7) && fs != 0.0;
        let fill = !matches!(mode, 1 | 5);
        let stroke = matches!(mode, 1 | 2 | 5 | 6);
        let clip_mode = mode >= 4;
        let mut color = if fill {
            self.fill_rgba()
        } else {
            rgba(&self.gs.stroke_cs, &self.gs.stroke, self.gs.stroke_alpha)
        };
        if let Some(t) = self.text_color {
            color = Rgba(t.0, t.1, t.2, color.3);
        }
        // Text filled with a pattern (§8.7.3: shading or tiling pattern as the fill colour):
        // its outlines filled like a path (before: the glyphs in black).
        let pattern_fill = fill && self.gs.fill_pattern.is_some() && self.text_color.is_none();
        // Outlines for stroked and clipping text too (glyph runs only fill).
        let paths = self.sh.opts.text_as_paths
            || font.face.is_none()
            || font.is_type1_program()
            || stroke
            || clip_mode
            || pattern_fill;
        let mut run: Vec<Glyph> = Vec::new();
        let mut run_text = String::new();
        let mut run_origin = (0.0, 0.0);
        let mut run_lin = [0.0_f32; 4];
        let mut run_size = 0.0_f32;
        let mut glyph_path: Vec<PathSeg> = Vec::new();
        let sub_data = font.substitute_data();
        let sub_face = sub_data
            .as_ref()
            .and_then(|(d, i)| ttf_parser::Face::parse(d, *i).ok());
        for (code, len) in font.codes(s) {
            let text = font.text(code);
            let space = code == 32 && len == 1;
            let (w0, advance) = if let Some(_t3) = &font.type3 {
                let w = font.width(code) * font.matrix[0];
                (w, w * fs)
            } else {
                let w = font.width(code) / 1000.0;
                (w, w * fs)
            };
            let tsm = [fs * th, 0.0, 0.0, fs, 0.0, self.gs.rise];
            let mut trm = mul(tsm, mul(self.tm, self.gs.ctm));
            if font.vertical {
                let (vx, vy) = font.vertical_metrics(code);
                trm = mul([1.0, 0.0, 0.0, 1.0, -vx / 1000.0, -vy / 1000.0], trm);
            }
            let (ox, oy) = apply(trm, 0.0, 0.0);
            // Text box for search and selection.
            if !text.is_empty() {
                let (asc, desc) = (font.ascent / 1000.0, font.descent / 1000.0);
                let w = if font.vertical { 1.0 } else { w0 };
                let corners = [
                    apply(trm, 0.0, desc),
                    apply(trm, w, desc),
                    apply(trm, w, asc),
                    apply(trm, 0.0, asc),
                ];
                let xs = corners.iter().map(|c| c.0);
                let ys = corners.iter().map(|c| c.1);
                let x0 = xs.clone().fold(f32::INFINITY, f32::min);
                let x1 = xs.fold(f32::NEG_INFINITY, f32::max);
                let y0 = ys.clone().fold(f32::INFINITY, f32::min);
                let y1 = ys.fold(f32::NEG_INFINITY, f32::max);
                let (bx, by) = apply(trm, 1.0, 0.0);
                if x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite() {
                    self.out.chars.push(TextChar {
                        text: text.clone(),
                        rect: [x0, y0, x1, y1],
                        origin: [ox, oy],
                        dir: [bx - ox, by - oy],
                        size: scale_of(trm),
                    });
                }
            }
            if visible || clip_mode && fs != 0.0 {
                if let Some(t3) = font.type3.as_ref().filter(|_| visible) {
                    let name = font.glyph_name(code);
                    if let Some(proc_) = name.and_then(|n| t3.procs.get(n.as_bytes())).cloned() {
                        let m = mul(font.matrix, trm);
                        self.type3_glyph(&font, &proc_, m);
                    }
                } else if let Some(g) = font.glyph(code).or_else(|| {
                    sub_face
                        .as_ref()
                        .and_then(|f| font.substitute_glyph(code, f))
                        .map(GlyphRef::Gid)
                }) {
                    if paths {
                        if let Some(o) = font.outline(&g, self.sh.fonts) {
                            let m = mul(font.matrix, trm);
                            glyph_path.extend(o.iter().map(|s| transform_seg(*s, m)));
                        }
                    } else if let (GlyphRef::Gid(gid), Some(_)) = (&g, font.face) {
                        // Glyph run: em of the face maps through `trm` (text space y up,
                        // page y down).
                        let lin = [trm[0], trm[1], -trm[2], -trm[3]];
                        let size = scale_of(trm);
                        let same = !run.is_empty()
                            && (size - run_size).abs() < 1e-4
                            && lin
                                .iter()
                                .zip(run_lin.iter())
                                .all(|(a, b)| (a - b).abs() < 1e-4);
                        if !same {
                            self.flush_run(
                                &font,
                                &mut run,
                                &mut run_text,
                                run_origin,
                                run_lin,
                                run_size,
                                color,
                            );
                            run_origin = (ox, oy);
                            run_lin = lin;
                            run_size = size;
                        }
                        // Position in run space: inverse of the run's linear map / size.
                        let (dx, dy) = (ox - run_origin.0, oy - run_origin.1);
                        let l = [lin[0] / size, lin[1] / size, lin[2] / size, lin[3] / size];
                        let det = l[0] * l[3] - l[1] * l[2];
                        let (rx, ry) = if det.abs() > 1e-9 {
                            (
                                (l[3] * dx - l[2] * dy) / det,
                                (-l[1] * dx + l[0] * dy) / det,
                            )
                        } else {
                            (dx, dy)
                        };
                        let cluster = run_text.len() as u32;
                        run.push(Glyph(*gid, rx, ry, cluster));
                        run_text.push_str(&text);
                    }
                }
            }
            // Advance (§9.4.4).
            let spacing = self.gs.tc + if space { self.gs.tw } else { 0.0 };
            if font.vertical {
                self.tm = mul([1.0, 0.0, 0.0, 1.0, 0.0, -(advance) + spacing], self.tm);
            } else {
                self.tm = mul([1.0, 0.0, 0.0, 1.0, (advance + spacing) * th, 0.0], self.tm);
            }
        }
        self.flush_run(
            &font,
            &mut run,
            &mut run_text,
            run_origin,
            run_lin,
            run_size,
            color,
        );
        if clip_mode {
            self.text_clip
                .get_or_insert_with(Vec::new)
                .extend_from_slice(&glyph_path);
        }
        if !glyph_path.is_empty() && visible && pattern_fill {
            let saved = std::mem::replace(&mut self.path, glyph_path);
            let clip = self.clip.take();
            self.paint(true, stroke);
            self.clip = clip;
            self.path = saved;
        } else if !glyph_path.is_empty() && visible {
            let fill_paint = fill.then_some(Paint::Solid { color });
            let mut stroke_style = if stroke { self.stroke_style() } else { None };
            if let (Some(t), Some(st)) = (self.text_color, stroke_style.as_mut()) {
                st.color = Rgba(t.0, t.1, t.2, st.color.3);
            }
            self.out.commands.push(Command::Path {
                segs: glyph_path,
                fill: fill_paint,
                stroke: stroke_style,
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn flush_run(
        &mut self,
        font: &Font,
        run: &mut Vec<Glyph>,
        text: &mut String,
        origin: (f32, f32),
        lin: [f32; 4],
        size: f32,
        color: Rgba,
    ) {
        if run.is_empty() {
            return;
        }
        let Some(face) = font.face else {
            run.clear();
            text.clear();
            return;
        };
        let glyphs = std::mem::take(run);
        let text = std::mem::take(text);
        let l = [lin[0] / size, lin[1] / size, lin[2] / size, lin[3] / size];
        let upright = (l[0] - 1.0).abs() < 1e-3
            && l[1].abs() < 1e-3
            && l[2].abs() < 1e-3
            && (l[3] - 1.0).abs() < 1e-3;
        if upright {
            self.out.commands.push(Command::Glyphs {
                face: face.0,
                size,
                x: origin.0,
                y: origin.1,
                color,
                text,
                glyphs,
            });
        } else {
            self.out.commands.push(Command::PushTransform {
                m: [l[0], l[1], l[2], l[3], origin.0, origin.1],
            });
            self.out.commands.push(Command::Glyphs {
                face: face.0,
                size,
                x: 0.0,
                y: 0.0,
                color,
                text,
                glyphs,
            });
            self.out.commands.push(Command::PopTransform);
        }
    }

    fn type3_glyph(&mut self, font: &Rc<Font>, proc_: &Object, m: M) {
        if self.depth > 6 {
            return;
        }
        let doc = self.doc();
        let Ok((_, Object::Stream(s))) = doc.dereference(proc_) else {
            return;
        };
        let Ok(content) = filters::stream_bytes(doc, s) else {
            return;
        };
        self.save();
        self.gs.ctm = m;
        let saved_tm = (self.tm, self.tlm);
        self.res.push(Res::Owned(font.clone()));
        self.depth += 1;
        let path = std::mem::take(&mut self.path);
        let stack = self.stack.len();
        self.run(&content);
        while self.stack.len() > stack {
            self.restore();
        }
        self.path = path;
        self.depth -= 1;
        self.res.pop();
        (self.tm, self.tlm) = saved_tm;
        self.restore();
        self.uncolored = false;
    }

    /// Fill opacity of the current state (annotation stand-ins).
    pub(crate) fn set_fill_alpha(&mut self, alpha: f32) {
        self.gs.fill_alpha = alpha.clamp(0.0, 1.0);
    }

    fn do_xobject(&mut self, name: &[u8]) {
        let Some(o) = self.resource(b"XObject", name) else {
            return;
        };
        let doc = self.doc();
        let id = o.as_reference().ok();
        let Some(Object::Stream(s)) = id.and_then(|i| doc.get_object(i).ok()) else {
            return;
        };
        if let Ok(oc) = s.dict.get(b"OC") {
            if self.ocg_off(oc) {
                return;
            }
        }
        match s.dict.get(b"Subtype").and_then(Object::as_name) {
            Ok(b"Image") => {
                if self.hidden() {
                    return;
                }
                let stencil = s
                    .dict
                    .get(b"ImageMask")
                    .is_ok_and(|o| o.as_bool().unwrap_or(false));
                let fill = self.gs.fill_cs.rgb(&self.gs.fill);
                let key = match (id, stencil) {
                    (Some(id), false) => format!("x{}_{}", id.0, id.1),
                    (Some(id), true) => format!(
                        "m{}_{}_{:02x}{:02x}{:02x}",
                        id.0,
                        id.1,
                        (fill[0] * 255.0) as u8,
                        (fill[1] * 255.0) as u8,
                        (fill[2] * 255.0) as u8
                    ),
                    (None, _) => {
                        self.inline_count += 1;
                        format!("{}d{}", self.image_prefix, self.inline_count)
                    }
                };
                let space = s
                    .dict
                    .get(b"ColorSpace")
                    .ok()
                    .and_then(|o| self.color_space(o));
                let images = &mut *self.sh.images;
                let img = images.get_or(&key, || {
                    image::decode(&image::Params {
                        doc,
                        dict: &s.dict,
                        data: &s.content,
                        space,
                        fill,
                    })
                });
                if let Some(img) = img {
                    let smooth = interpolate(doc, &s.dict);
                    self.place_image(key, img, smooth);
                }
            }
            Ok(b"Form") => {
                if self.depth > 12 || id.is_some_and(|i| self.forms.contains(&i)) {
                    return;
                }
                let Ok(content) = filters::stream_bytes(doc, s) else {
                    return;
                };
                let matrix = s
                    .dict
                    .get(b"Matrix")
                    .ok()
                    .and_then(|o| o.as_array().ok())
                    .map(|a| a.iter().map(f).collect::<Vec<_>>())
                    .and_then(|v| <[f32; 6]>::try_from(v.as_slice()).ok())
                    .unwrap_or(IDENTITY);
                let bbox = s
                    .dict
                    .get(b"BBox")
                    .ok()
                    .and_then(|o| o.as_array().ok())
                    .map(|a| a.iter().map(f).collect::<Vec<_>>());
                let res = s
                    .dict
                    .get(b"Resources")
                    .ok()
                    .map(|o| doc.dereference(o).map_or(o, |(_, o)| o))
                    .and_then(|o| o.as_dict().ok());
                let group = s
                    .dict
                    .get(b"Group")
                    .ok()
                    .map(|o| doc.dereference(o).map_or(o, |(_, o)| o))
                    .and_then(|o| o.as_dict().ok())
                    .filter(|g| {
                        g.get(b"S")
                            .ok()
                            .and_then(|v| v.as_name().ok())
                            .is_none_or(|n| n == b"Transparency")
                    });
                // §11.4.6.2 / §11.6.6: a transparency group with `/K true` is a knockout group.
                let knockout = group
                    .and_then(|g| g.get(b"K").ok())
                    .and_then(|k| k.as_bool().ok())
                    .unwrap_or(false);
                // §11.6.4.4 / §11.6.6: the alpha constant at `Do` applies to the group as a
                // whole and its content starts at alpha 1 (an `/ca` inside replaced it before,
                // so a group drawn at 0.8 with objects at 0.6 inside showed at 0.6).
                let group_alpha = if group.is_some() {
                    self.gs.fill_alpha
                } else {
                    1.0
                };
                let outer_alpha = (self.gs.fill_alpha, self.gs.stroke_alpha);
                if group.is_some() {
                    self.gs.fill_alpha = 1.0;
                    self.gs.stroke_alpha = 1.0;
                }
                if group_alpha < 0.999 {
                    self.out
                        .commands
                        .push(Command::PushOpacity { alpha: group_alpha });
                }
                if knockout {
                    self.out.commands.push(Command::PushGroup {
                        blend: None,
                        mask: None,
                        knockout: true,
                        isolated: false,
                    });
                }
                // §11.4.5 `/I true`: blended objects inside compose on a transparent start
                // (only marked when something inside blends: a plain isolated group draws the
                // same either way and a layer costs time).
                let isolated = !knockout
                    && group
                        .and_then(|g| g.get(b"I").ok())
                        .and_then(|v| v.as_bool().ok())
                        .unwrap_or(false);
                let inner_start = self.out.commands.len();
                if let Some(i) = id {
                    self.forms.push(i);
                }
                self.knockout_depth += u32::from(knockout);
                self.run_form(&content, matrix, bbox.as_deref(), res);
                self.knockout_depth -= u32::from(knockout);
                if id.is_some() {
                    self.forms.pop();
                }
                if knockout {
                    self.out.commands.push(Command::PopGroup);
                }
                if isolated
                    && self.out.commands[inner_start..]
                        .iter()
                        .any(|c| matches!(c, Command::PushGroup { blend: Some(_), .. }))
                {
                    self.out.commands.insert(
                        inner_start,
                        Command::PushGroup {
                            blend: None,
                            mask: None,
                            knockout: false,
                            isolated: true,
                        },
                    );
                    self.out.commands.push(Command::PopGroup);
                }
                if group_alpha < 0.999 {
                    self.out.commands.push(Command::PopOpacity);
                }
                (self.gs.fill_alpha, self.gs.stroke_alpha) = outer_alpha;
            }
            _ => {}
        }
    }

    /// Run a form's content with its matrix and bounding box clip.
    pub(crate) fn run_form(
        &mut self,
        content: &[u8],
        matrix: M,
        bbox: Option<&[f32]>,
        res: Option<&'a Dictionary>,
    ) {
        self.save();
        self.gs.ctm = mul(matrix, self.gs.ctm);
        if let Some([x0, y0, x1, y1]) = bbox.and_then(|b| <[f32; 4]>::try_from(b).ok()) {
            self.path.clear();
            self.move_to(x0, y0);
            self.line_to(x1, y0);
            self.line_to(x1, y1);
            self.line_to(x0, y1);
            self.close();
            let segs = std::mem::take(&mut self.path);
            self.push_clip(&segs);
        }
        let saved_path = std::mem::take(&mut self.path);
        let saved_tm = (self.tm, self.tlm);
        if let Some(r) = res {
            self.res.push(Res::Dict(r));
        }
        self.depth += 1;
        let marked = self.marked.len();
        let stack = self.stack.len();
        self.run(content);
        // Unbalanced q/Q or marked content inside the form stays inside it.
        while self.stack.len() > stack {
            self.restore();
        }
        while self.marked.len() > marked {
            if self.marked.pop() == Some(true) {
                self.hidden_depth = self.hidden_depth.saturating_sub(1);
            }
        }
        self.depth -= 1;
        if res.is_some() {
            self.res.pop();
        }
        (self.tm, self.tlm) = saved_tm;
        self.path = saved_path;
        self.restore();
    }

    fn inline_image(&mut self, d: &Dictionary, data: &[u8]) {
        if self.hidden() {
            return;
        }
        let space = d
            .get(b"ColorSpace")
            .or_else(|_| d.get(b"CS"))
            .ok()
            .and_then(|o| self.color_space(o));
        let fill = self.gs.fill_cs.rgb(&self.gs.fill);
        self.inline_count += 1;
        let key = format!("{}i{}", self.image_prefix, self.inline_count);
        let img = image::decode(&image::Params {
            doc: self.doc(),
            dict: d,
            data,
            space,
            fill,
        });
        if let Some(img) = img {
            let smooth = interpolate(self.doc(), d);
            self.place_image(key, Arc::new(img), smooth);
        }
    }

    /// Draw a picture into the unit square of the current matrix.
    /// Without `/Interpolate true` (§8.9.5.3) the image id ends in [`PIXELATED`]: backends
    /// enlarge it pixel by pixel, as other readers do.
    fn place_image(&mut self, key: String, img: Arc<Image>, smooth: bool) {
        let t = mul([1.0, 0.0, 0.0, -1.0, 0.0, 1.0], self.gs.ctm);
        if !t.iter().all(|v| v.is_finite()) {
            return;
        }
        let key = if smooth {
            key
        } else {
            format!("{key}{PIXELATED}")
        };
        self.out.images.insert(key.clone(), img);
        let alpha = self.gs.fill_alpha;
        if alpha < 0.999 {
            self.out.commands.push(Command::PushOpacity { alpha });
        }
        if t[1].abs() < 1e-4 && t[2].abs() < 1e-4 && t[0] > 0.0 && t[3] > 0.0 {
            self.out.commands.push(Command::Image {
                x: t[4],
                y: t[5],
                w: t[0],
                h: t[3],
                image: key,
                crop: None,
                rotation: 0.0,
            });
        } else {
            self.out.commands.push(Command::PushTransform { m: t });
            self.out.commands.push(Command::Image {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                image: key,
                crop: None,
                rotation: 0.0,
            });
            self.out.commands.push(Command::PopTransform);
        }
        if alpha < 0.999 {
            self.out.commands.push(Command::PopOpacity);
        }
    }

    fn page_rect(&self) -> Vec<PathSeg> {
        let (w, h) = self.page;
        vec![
            PathSeg::M(-1.0, -1.0),
            PathSeg::L(w + 1.0, -1.0),
            PathSeg::L(w + 1.0, h + 1.0),
            PathSeg::L(-1.0, h + 1.0),
            PathSeg::Z,
        ]
    }

    /// Area an axial shading paints when one of its ends is not extended (§8.7.4.5.3
    /// `/Extend`, default `[false false]`): the band between the lines through the axis ends
    /// perpendicular to it (in shading space), open on an extended side. `None` when both
    /// ends extend (the whole page) or the shading is not axial.
    fn axial_band(&self, o: &Object, m: M) -> Option<Vec<PathSeg>> {
        let doc = self.doc();
        let dict = match doc.dereference(o).ok()?.1 {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => return None,
        };
        if dict.get(b"ShadingType").ok()?.as_i64().ok()? != 2 {
            return None;
        }
        let c = shading_coords(doc, dict);
        let [x0, y0, x1, y1] = <[f32; 4]>::try_from(c.get(..4)?).ok()?;
        let ext: Vec<bool> = dict
            .get(b"Extend")
            .ok()
            .map(|o| doc.dereference(o).map_or(o, |(_, o)| o))
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().map(|b| b.as_bool().unwrap_or(false)).collect())
            .unwrap_or_default();
        let (e0, e1) = (
            ext.first().copied().unwrap_or(false),
            ext.get(1).copied().unwrap_or(false),
        );
        if e0 && e1 {
            return None;
        }
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = dx.hypot(dy);
        if len < 1e-6 {
            return Some(Vec::new());
        }
        // How far the band must reach: the page corners in shading space.
        let inv = invert(m)?;
        let reach = self
            .page_rect()
            .iter()
            .filter_map(|s| match s {
                PathSeg::M(x, y) | PathSeg::L(x, y) => Some(apply(inv, *x, *y)),
                _ => None,
            })
            .map(|(x, y)| (x - x0).hypot(y - y0).max((x - x1).hypot(y - y1)))
            .fold(0.0_f32, f32::max)
            + 1.0;
        let (ux, uy) = (dx / len, dy / len);
        let (nx, ny) = (-uy * reach, ux * reach);
        let from = if e0 { -reach } else { 0.0 };
        let to = if e1 { len + reach } else { len };
        let at = |s: f32, k: f32| apply(m, x0 + ux * s + nx * k, y0 + uy * s + ny * k);
        let corners = [at(from, -1.0), at(to, -1.0), at(to, 1.0), at(from, 1.0)];
        if !corners.iter().all(|p| p.0.is_finite() && p.1.is_finite()) {
            return None;
        }
        Some(vec![
            PathSeg::M(corners[0].0, corners[0].1),
            PathSeg::L(corners[1].0, corners[1].1),
            PathSeg::L(corners[2].0, corners[2].1),
            PathSeg::L(corners[3].0, corners[3].1),
            PathSeg::Z,
        ])
    }

    fn shade(&mut self, name: &[u8]) {
        if self.hidden() {
            return;
        }
        let Some(o) = self.resource(b"Shading", name) else {
            return;
        };
        if let Some(p) = self.shading_paint(&o, self.gs.ctm) {
            let segs = self
                .axial_band(&o, self.gs.ctm)
                .unwrap_or_else(|| self.page_rect());
            self.out.commands.push(Command::Path {
                segs,
                fill: Some(p),
                stroke: None,
            });
        } else {
            let m = self.gs.ctm;
            self.mesh_fill(&o, m);
        }
    }

    /// Draw a function-based or mesh shading (types 1, 4–7) as small triangles in pattern
    /// space `m`; false when it is not one.
    fn mesh_fill(&mut self, o: &Object, m: M) -> bool {
        let doc = self.doc();
        let Ok((_, obj)) = doc.dereference(o) else {
            return false;
        };
        let (dict, data) = match obj {
            Object::Stream(s) => (&s.dict, filters::stream_bytes(doc, s).ok()),
            Object::Dictionary(d) => (d, None),
            _ => return false,
        };
        let kind = dict
            .get(b"ShadingType")
            .ok()
            .and_then(|v| v.as_i64().ok())
            .unwrap_or(0);
        if !matches!(kind, 1 | 3..=7) {
            return false;
        }
        let Some(cs) = dict
            .get(b"ColorSpace")
            .ok()
            .and_then(|c| self.color_space(c))
        else {
            return false;
        };
        let func = dict
            .get(b"Function")
            .ok()
            .and_then(|o| Function::parse(doc, o, 0));
        let tris = match (kind, &func, &data) {
            (1, Some(f), _) => crate::mesh::function_based(doc, dict, f),
            (3, _, _) => {
                let coords = shading_coords(doc, dict);
                let Ok(c) = <[f32; 6]>::try_from(coords.as_slice()) else {
                    return false;
                };
                let nums = |k: &[u8]| -> Vec<f32> {
                    dict.get(k)
                        .ok()
                        .and_then(|o| o.as_array().ok())
                        .map(|a| a.iter().map(f).collect())
                        .unwrap_or_default()
                };
                let domain = nums(b"Domain");
                let t = (
                    domain.first().copied().unwrap_or(0.0),
                    domain.get(1).copied().unwrap_or(1.0),
                );
                let ext: Vec<bool> = dict
                    .get(b"Extend")
                    .ok()
                    .and_then(|o| o.as_array().ok())
                    .map(|a| a.iter().map(|b| b.as_bool().unwrap_or(false)).collect())
                    .unwrap_or_default();
                let extend = (
                    ext.first().copied().unwrap_or(false),
                    ext.get(1).copied().unwrap_or(false),
                );
                // How far the extension must reach: the page corners in shading space.
                let reach = invert(m).map_or(1e4, |inv| {
                    let page = self.page_rect();
                    page.iter()
                        .filter_map(|s| match s {
                            PathSeg::M(x, y) | PathSeg::L(x, y) => Some(apply(inv, *x, *y)),
                            _ => None,
                        })
                        .map(|(x, y)| ((x - c[0]).hypot(y - c[1])).max((x - c[3]).hypot(y - c[4])))
                        .fold(0.0_f32, f32::max)
                });
                crate::mesh::radial(c, t, extend, reach)
            }
            (4..=7, _, Some(d)) => {
                crate::mesh::mesh(kind, dict, d, cs.components(), func.is_some())
            }
            _ => return false,
        };
        let alpha = self.gs.fill_alpha;
        // A hairline in each triangle's colour hides the seams between neighbours.
        let seam = self.sh.opts.min_line * 0.8;
        for t in tris {
            let vals = match &func {
                Some(f) if kind != 1 => f.eval(&t.color),
                _ => t.color,
            };
            let color = rgba(&cs, &vals, alpha);
            let p: Vec<(f32, f32)> = t.pts.iter().map(|&(x, y)| apply(m, x, y)).collect();
            if !p.iter().all(|q| q.0.is_finite() && q.1.is_finite()) {
                continue;
            }
            self.out.commands.push(Command::Path {
                segs: vec![
                    PathSeg::M(p[0].0, p[0].1),
                    PathSeg::L(p[1].0, p[1].1),
                    PathSeg::L(p[2].0, p[2].1),
                    PathSeg::Z,
                ],
                fill: Some(Paint::Solid { color }),
                stroke: (alpha >= 0.999 && seam > 0.0).then(|| Stroke {
                    width: seam,
                    color,
                    dash: Vec::new(),
                    cap: "butt".to_owned(),
                    join: "round".to_owned(),
                }),
            });
        }
        true
    }

    /// Gradient paint of an axial or radial shading in pattern space `m`.
    fn shading_paint(&self, o: &Object, m: M) -> Option<Paint> {
        let doc = self.doc();
        let dict = match doc.dereference(o).ok()?.1 {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => return None,
        };
        let kind = dict.get(b"ShadingType").ok()?.as_i64().ok()?;
        let cs = self.color_space(dict.get(b"ColorSpace").ok()?)?;
        let coords: Vec<f32> = dict
            .get(b"Coords")
            .ok()
            .map(|o| doc.dereference(o).map_or(o, |(_, o)| o))
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().map(f).collect())
            .unwrap_or_default();
        let func = dict
            .get(b"Function")
            .ok()
            .and_then(|o| Function::parse(doc, o, 0));
        let domain: Vec<f32> = dict
            .get(b"Domain")
            .ok()
            .and_then(|o| o.as_array().ok())
            .map_or_else(|| vec![0.0, 1.0], |a| a.iter().map(f).collect());
        let (t0, t1) = (
            domain.first().copied().unwrap_or(0.0),
            domain.get(1).copied().unwrap_or(1.0),
        );
        let color_at = |t: f32| -> Rgba {
            let vals = func.as_ref().map_or_else(|| vec![t], |f| f.eval(&[t]));
            rgba(&cs, &vals, self.gs.fill_alpha)
        };
        let stops = |map: &dyn Fn(f32) -> f32| -> Vec<GradientStop> {
            (0..=16)
                .map(|k| {
                    let u = k as f32 / 16.0;
                    GradientStop {
                        offset: map(u).clamp(0.0, 1.0),
                        color: color_at(t0 + u * (t1 - t0)),
                    }
                })
                .collect()
        };
        match kind {
            2 if coords.len() >= 4 => {
                let (x1, y1) = apply(m, coords[0], coords[1]);
                let (x2, y2) = apply(m, coords[2], coords[3]);
                Some(Paint::Linear {
                    x1,
                    y1,
                    x2,
                    y2,
                    stops: stops(&|u| u),
                })
            }
            3 if coords.len() >= 6 && !radial_needs_mesh(&coords, m) => {
                let (r0, r1) = (coords[2], coords[5]);
                let (cx, cy) = apply(m, coords[3], coords[4]);
                let r = r1.max(r0).max(0.01) * scale_of(m);
                let outer = r1.max(r0).max(0.01);
                Some(Paint::Radial {
                    cx,
                    cy,
                    r,
                    stops: stops(&|u| (r0 + u * (r1 - r0)) / outer),
                })
            }
            1 | 4..=7 => None,
            _ => {
                // Other shadings: their background.
                let bg: Vec<f32> = dict
                    .get(b"Background")
                    .ok()
                    .and_then(|o| o.as_array().ok())
                    .map(|a| a.iter().map(f).collect())
                    .unwrap_or_default();
                if bg.is_empty() {
                    None
                } else {
                    Some(Paint::Solid {
                        color: rgba(&cs, &bg, self.gs.fill_alpha),
                    })
                }
            }
        }
    }

    /// The shading and pattern matrix of a shading pattern whose shading is a mesh or
    /// function-based one (or a radial one a circular gradient cannot draw).
    fn mesh_pattern(&self, name: &[u8]) -> Option<(Object, M)> {
        let doc = self.doc();
        let o = self.pattern_object(name)?;
        let d = match doc.dereference(&o).ok()?.1 {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => return None,
        };
        let sh = d.get(b"Shading").ok()?;
        let kind = match doc.dereference(sh).ok()?.1 {
            Object::Dictionary(s) => s.get(b"ShadingType").ok()?.as_i64().ok()?,
            Object::Stream(s) => s.dict.get(b"ShadingType").ok()?.as_i64().ok()?,
            _ => return None,
        };
        let m = self.pattern_matrix(d);
        let radial_mesh = kind == 3 && {
            let coords = match doc.dereference(sh).ok()?.1 {
                Object::Dictionary(s) => shading_coords(doc, s),
                Object::Stream(s) => shading_coords(doc, &s.dict),
                _ => Vec::new(),
            };
            radial_needs_mesh(&coords, m)
        };
        (matches!(kind, 1 | 4..=7) || radial_mesh).then(|| (sh.clone(), m))
    }

    fn pattern_object(&self, name: &[u8]) -> Option<Object> {
        self.resource(b"Pattern", name)
    }

    fn is_tiling(&self, name: &[u8]) -> bool {
        let doc = self.doc();
        self.pattern_object(name)
            .and_then(|o| doc.dereference(&o).ok().map(|(_, o)| o.clone()))
            .and_then(|o| match o {
                Object::Stream(s) => s
                    .dict
                    .get(b"PatternType")
                    .ok()
                    .and_then(|v| v.as_i64().ok()),
                Object::Dictionary(d) => d.get(b"PatternType").ok().and_then(|v| v.as_i64().ok()),
                _ => None,
            })
            == Some(1)
    }

    fn pattern_matrix(&self, d: &Dictionary) -> M {
        let m = d
            .get(b"Matrix")
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().map(f).collect::<Vec<_>>())
            .and_then(|v| <[f32; 6]>::try_from(v.as_slice()).ok())
            .unwrap_or(IDENTITY);
        // Pattern space is the default space of the page (or of the form being run).
        let base = self.stack.first().map_or(self.gs.ctm, |g| g.ctm);
        mul(m, base)
    }

    fn pattern_paint(&self, name: &[u8]) -> Option<Paint> {
        let doc = self.doc();
        let o = self.pattern_object(name)?;
        let d = match doc.dereference(&o).ok()?.1 {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => return None,
        };
        let m = self.pattern_matrix(d);
        let sh = d.get(b"Shading").ok()?;
        self.shading_paint(sh, m)
    }

    /// Average colour (alpha-weighted, over white) of the only picture among a
    /// pattern cell's `/XObject` resources, if the cell draws exactly one picture.
    fn cell_image_average(&self, res: &Dictionary) -> Option<Rgba> {
        let doc = self.doc();
        let xo = res.get(b"XObject").ok()?;
        let xo = doc.dereference(xo).map_or(xo, |(_, o)| o).as_dict().ok()?;
        let mut it = xo.iter();
        let (_, first) = it.next()?;
        if it.next().is_some() {
            return None;
        }
        let Object::Stream(s) = doc.dereference(first).ok()?.1 else {
            return None;
        };
        if s.dict.get(b"Subtype").and_then(Object::as_name).ok()? != b"Image" {
            return None;
        }
        let lookup = |n: &[u8]| {
            res.get(b"ColorSpace")
                .ok()
                .and_then(|c| doc.dereference(c).map_or(Some(c), |(_, o)| Some(o)))
                .and_then(|c| c.as_dict().ok())
                .and_then(|c| c.get(n).ok().cloned())
        };
        let space = s
            .dict
            .get(b"ColorSpace")
            .ok()
            .and_then(|o| ColorSpace::parse(doc, o, &lookup, 0));
        let img = image::decode(&image::Params {
            doc,
            dict: &s.dict,
            data: &s.content,
            space,
            fill: self.gs.fill_cs.rgb(&self.gs.fill),
        })?;
        let n = (img.width as usize) * (img.height as usize);
        if n == 0 {
            return None;
        }
        // Premultiplied pixels over white.
        let mut sum = [0_u64; 3];
        for px in img.pixels.chunks_exact(4) {
            let a = u64::from(px[3]);
            for k in 0..3 {
                sum[k] += u64::from(px[k]) + (255 - a);
            }
        }
        let avg = |k: usize| (sum[k] as f64 / n as f64).round().clamp(0.0, 255.0) as u8;
        Some(Rgba(avg(0), avg(1), avg(2), 255))
    }

    /// Fill `segs` with a tiling pattern by drawing its cell over the path's bounds.
    fn tiling_fill(&mut self, name: &[u8], segs: &[PathSeg]) {
        let doc = self.doc();
        let Some(Object::Stream(s)) = self
            .pattern_object(name)
            .and_then(|o| o.as_reference().ok())
            .and_then(|i| doc.get_object(i).ok())
        else {
            return;
        };
        let Ok(content) = filters::stream_bytes(doc, s) else {
            return;
        };
        let m = self.pattern_matrix(&s.dict);
        let num = |k: &[u8]| s.dict.get(k).ok().map_or(0.0, f);
        let (xs, ys) = (num(b"XStep").abs(), num(b"YStep").abs());
        let bbox: Vec<f32> = s
            .dict
            .get(b"BBox")
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().map(f).collect())
            .unwrap_or_default();
        let res = s
            .dict
            .get(b"Resources")
            .ok()
            .map(|o| doc.dereference(o).map_or(o, |(_, o)| o))
            .and_then(|o| o.as_dict().ok());
        let Some(inv) = invert(m) else { return };
        if xs < 1e-3 || ys < 1e-3 || bbox.len() < 4 {
            return;
        }
        // Path bounds in pattern space.
        let mut lo = (f32::INFINITY, f32::INFINITY);
        let mut hi = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for sg in segs {
            let pts: &[(f32, f32)] = &match *sg {
                PathSeg::M(x, y) | PathSeg::L(x, y) => vec![(x, y)],
                PathSeg::Q(a, b, x, y) => vec![(a, b), (x, y)],
                PathSeg::C(a, b, c, d, x, y) => vec![(a, b), (c, d), (x, y)],
                PathSeg::Z => vec![],
            };
            for &(x, y) in pts {
                let (u, v) = apply(inv, x, y);
                lo = (lo.0.min(u), lo.1.min(v));
                hi = (hi.0.max(u), hi.1.max(v));
            }
        }
        let i0 = ((lo.0 - bbox[2]) / xs).floor() as i64;
        let i1 = ((hi.0 - bbox[0]) / xs).ceil() as i64;
        let j0 = ((lo.1 - bbox[3]) / ys).floor() as i64;
        let j1 = ((hi.1 - bbox[1]) / ys).ceil() as i64;
        if (i1 - i0).saturating_mul(j1 - j0) > 4000 || self.depth > 8 {
            // Too many cells to draw one by one: a cell that is one picture (the 1/600"
            // hatch tiles of word-processor shading patterns, §8.7.3.1) paints its
            // average colour — what its fine texture reads as — else an even grey keeps
            // the area visible.
            let color = res
                .and_then(|r| self.cell_image_average(r))
                .unwrap_or(Rgba(200, 200, 200, 255));
            self.out.commands.push(Command::Path {
                segs: segs.to_vec(),
                fill: Some(Paint::Solid { color }),
                stroke: None,
            });
            return;
        }
        self.push_clip(segs);
        let ctm = self.gs.ctm;
        let saved_ctm_base = self.gs.ctm;
        // The cell paints in colours, not with this pattern again (§8.7.3.3): an uncoloured
        // pattern (`/PaintType 2`) in the colour given with `scn` in the underlying space,
        // a coloured one from black. Before, the cell's own `f` filled with the pattern
        // again, recursing until the depth limit and leaving the rest of the page undrawn.
        let saved_colours = (
            Arc::clone(&self.gs.fill_cs),
            self.gs.fill.clone(),
            self.gs.fill_pattern.take(),
            Arc::clone(&self.gs.stroke_cs),
            self.gs.stroke.clone(),
            self.gs.stroke_pattern.take(),
        );
        let uncoloured = num(b"PaintType") as i64 == 2;
        let (cs, vals) = match &*saved_colours.0 {
            ColorSpace::Pattern(Some(base)) if uncoloured => {
                (Arc::clone(base), saved_colours.1.clone())
            }
            _ => (Arc::new(ColorSpace::Gray), vec![0.0]),
        };
        self.gs.fill_cs = Arc::clone(&cs);
        self.gs.fill.clone_from(&vals);
        self.gs.stroke_cs = cs;
        self.gs.stroke = vals;
        for j in j0..j1 {
            for i in i0..i1 {
                let cell = mul([1.0, 0.0, 0.0, 1.0, i as f32 * xs, j as f32 * ys], m);
                // run_form multiplies onto the current matrix: start from identity.
                self.gs.ctm = IDENTITY;
                self.run_form(&content, cell, Some(&bbox), res);
                self.gs.ctm = ctm;
            }
        }
        self.gs.ctm = saved_ctm_base;
        (
            self.gs.fill_cs,
            self.gs.fill,
            self.gs.fill_pattern,
            self.gs.stroke_cs,
            self.gs.stroke,
            self.gs.stroke_pattern,
        ) = saved_colours;
        self.out.commands.push(Command::PopClip);
        self.gs.clips = self.gs.clips.saturating_sub(1);
    }
}

/// Suffix of the id of an image that is enlarged without smoothing (no `/Interpolate true`).
pub const PIXELATED: &str = "#px";

/// `/Interpolate` (or `/I` inline) of an image dictionary.
fn interpolate(doc: &Document, d: &Dictionary) -> bool {
    d.get(b"Interpolate")
        .or_else(|_| d.get(b"I"))
        .ok()
        .and_then(|o| doc.dereference(o).ok().map(|(_, o)| o))
        .and_then(|o| o.as_bool().ok())
        .unwrap_or(false)
}

/// `/Coords` of a shading dictionary.
fn shading_coords(doc: &Document, d: &Dictionary) -> Vec<f32> {
    d.get(b"Coords")
        .ok()
        .map(|o| doc.dereference(o).map_or(o, |(_, o)| o))
        .and_then(|o| o.as_array().ok())
        .map(|a| a.iter().filter_map(|o| o.as_float().ok()).collect())
        .unwrap_or_default()
}

/// Whether a radial shading (§8.7.4.5.4) needs triangles: its circles have different centres,
/// or the matrix to the page is not a uniform scale with rotation (circles become ellipses).
/// A device radial gradient draws concentric circles only.
fn radial_needs_mesh(coords: &[f32], m: M) -> bool {
    let Ok([x0, y0, _, x1, y1, _]) = <[f32; 6]>::try_from(coords) else {
        return false;
    };
    let [a, b, c, d, _, _] = m;
    let scale = a.hypot(b).max(c.hypot(d)).max(1e-6);
    let similar = ((a - d).abs() < 1e-3 * scale && (b + c).abs() < 1e-3 * scale)
        || ((a + d).abs() < 1e-3 * scale && (b - c).abs() < 1e-3 * scale);
    let r = coords[2].abs().max(coords[5].abs()).max(1e-6);
    !similar || (x0 - x1).hypot(y0 - y1) > 1e-3 * r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrices() {
        let m = mul(
            [2.0, 0.0, 0.0, 2.0, 1.0, 1.0],
            [1.0, 0.0, 0.0, -1.0, 0.0, 100.0],
        );
        assert_eq!(apply(m, 1.0, 1.0), (3.0, 97.0));
        let inv = invert(m).expect("invertible");
        let (x, y) = apply(inv, 3.0, 97.0);
        assert!((x - 1.0).abs() < 1e-5 && (y - 1.0).abs() < 1e-5);
        assert_eq!(
            as_rect(&[
                PathSeg::M(0.0, 0.0),
                PathSeg::L(5.0, 0.0),
                PathSeg::L(5.0, 3.0),
                PathSeg::L(0.0, 3.0),
                PathSeg::Z
            ]),
            Some([0.0, 0.0, 5.0, 3.0])
        );
    }
}
