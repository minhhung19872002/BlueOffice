//! Drawing comments (BlueReader round 3): pencil (Ink, ISO 32000-1 §12.5.6.13), line and
//! arrow (Line, §12.5.6.7), polyline and polygon (§12.5.6.9), oval and rectangle
//! (Square / Circle, §12.5.6.8), callout and typewriter (FreeText with `/IT
//! FreeTextCallout` / `FreeTextTypeWriter`, §12.5.6.6), stamps of our own design (Stamp,
//! §12.5.6.12, standard `/Name` values of Table 181) and measurements (distance, perimeter
//! and area: `/IT LineDimension` / `PolyLineDimension` / `PolygonDimension` with a
//! `/Measure` dictionary, §12.9).
//!
//! Each comment gets an appearance stream (§12.5.5) built from its geometry and [`Style`]
//! (colour `/C`, opacity `/CA`, thickness `/BS /W`, font size in `/DA`). The geometry is
//! read back from the standard keys (`/InkList`, `/L`, `/Vertices`, `/CL`, `/Rect`), so a
//! comment can be resized or restyled by building its appearance again.
use crate::annot::{nums, pdf_date, text_obj, Annot};
use crate::interp::{apply, invert, M};
use crate::{resolve, PdfDocument};
use lopdf::{Dictionary, Object, Stream};
use std::fmt::Write as _;

/// Stamps of Table 181 (the drawings are ours: a rounded frame and the name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StampKind {
    /// `/Approved`.
    Approved,
    /// `/NotApproved`.
    NotApproved,
    /// `/Draft`.
    Draft,
    /// `/Final`.
    Final,
    /// `/Confidential`.
    Confidential,
    /// `/ForComment`.
    ForComment,
    /// `/Expired`.
    Expired,
    /// `/Experimental`.
    Experimental,
}

impl StampKind {
    /// All stamps, in menu order.
    pub const ALL: [StampKind; 8] = [
        Self::Approved,
        Self::NotApproved,
        Self::Draft,
        Self::Final,
        Self::Confidential,
        Self::ForComment,
        Self::Expired,
        Self::Experimental,
    ];

    /// The `/Name` value.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Approved => "Approved",
            Self::NotApproved => "NotApproved",
            Self::Draft => "Draft",
            Self::Final => "Final",
            Self::Confidential => "Confidential",
            Self::ForComment => "ForComment",
            Self::Expired => "Expired",
            Self::Experimental => "Experimental",
        }
    }

    /// Text drawn in the stamp.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Approved => "APPROVED",
            Self::NotApproved => "NOT APPROVED",
            Self::Draft => "DRAFT",
            Self::Final => "FINAL",
            Self::Confidential => "CONFIDENTIAL",
            Self::ForComment => "FOR COMMENT",
            Self::Expired => "EXPIRED",
            Self::Experimental => "EXPERIMENTAL",
        }
    }

    /// Colour of the stamp (green approvals, red refusals, blue otherwise).
    #[must_use]
    pub fn color(self) -> [u8; 3] {
        match self {
            Self::Approved | Self::Final => [34, 139, 34],
            Self::NotApproved | Self::Confidential | Self::Expired => [200, 30, 30],
            Self::Draft | Self::ForComment | Self::Experimental => [30, 80, 180],
        }
    }

    fn from_name(n: &[u8]) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.name().as_bytes() == n)
            .unwrap_or(Self::Draft)
    }
}

/// Geometry of a drawn comment, in page points (y down, as [`Annot::rect`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// Freehand strokes (pencil).
    Ink(Vec<Vec<[f32; 2]>>),
    /// A line, with an arrow head at `b` when `arrow`.
    Line {
        /// Start.
        a: [f32; 2],
        /// End.
        b: [f32; 2],
        /// Arrow head at the end.
        arrow: bool,
    },
    /// Open polyline.
    Polyline(Vec<[f32; 2]>),
    /// Closed polygon.
    Polygon(Vec<[f32; 2]>),
    /// Ellipse in a box (left, top, right, bottom).
    Oval([f32; 4]),
    /// Rectangle (left, top, right, bottom).
    Rect([f32; 4]),
    /// Text box with a leader line to `tip`.
    Callout {
        /// The text box.
        rect: [f32; 4],
        /// Where the leader line points.
        tip: [f32; 2],
    },
    /// Text without a frame (typewriter).
    Typewriter([f32; 4]),
    /// A stamp in a box.
    Stamp([f32; 4], StampKind),
    /// Measured distance between two points.
    Distance([f32; 2], [f32; 2]),
    /// Measured length of a polyline.
    Perimeter(Vec<[f32; 2]>),
    /// Measured area of a polygon.
    Area(Vec<[f32; 2]>),
}

/// Look of a drawn comment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Stroke and text colour.
    pub color: [u8; 3],
    /// Opacity 0–1 (`/CA`).
    pub opacity: f32,
    /// Line thickness in points (`/BS /W`).
    pub width: f32,
    /// Text size in points.
    pub font_size: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            color: [229, 57, 53],
            opacity: 1.0,
            width: 1.0,
            font_size: 12.0,
        }
    }
}

/// Unit of measurements: its label and page points per unit (§12.9 number format: `/U`, and
/// `/C` = 1 / points). Inches by default.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureUnit {
    /// Label after the value (`in`, `mm`, `cm`, `pt`, `pc`).
    pub label: String,
    /// Page points (default user space units) per unit.
    pub points: f32,
}

impl Default for MeasureUnit {
    fn default() -> Self {
        Self {
            label: "in".to_owned(),
            points: 72.0,
        }
    }
}

impl MeasureUnit {
    /// A unit of `points` page points labelled `label` (non-positive sizes fall back to inches).
    #[must_use]
    pub fn new(label: &str, points: f32) -> Self {
        if points.is_finite() && points > 0.0 && !label.is_empty() {
            Self {
                label: label.to_owned(),
                points,
            }
        } else {
            Self::default()
        }
    }

    /// The unit of a `/Measure` dictionary: its first `/X` (or `/D`) number format's `/U` and
    /// `/C` (§12.9 Table 261 / 263).
    fn of_measure(doc: &crate::store::Document, m: &Object) -> Option<Self> {
        let m = resolve(doc, m).as_dict().ok()?;
        let fmt = [&b"X"[..], b"D"].iter().find_map(|k| {
            let a = resolve(doc, m.get(k).ok()?).as_array().ok()?;
            resolve(doc, a.first()?).as_dict().ok()
        })?;
        let u = fmt.get(b"U").ok().map(|o| resolve(doc, o))?;
        let label = crate::text_string(u.as_str().ok()?);
        let c = fmt
            .get(b"C")
            .ok()
            .and_then(|o| resolve(doc, o).as_float().ok())?;
        (c > 0.0).then(|| Self::new(label.trim(), 1.0 / c))
    }
}

/// Length of a polyline in page points.
#[must_use]
pub fn path_length(p: &[[f32; 2]]) -> f32 {
    p.windows(2)
        .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
        .sum()
}

/// Area of a polygon in square page points (shoelace formula).
#[must_use]
pub fn polygon_area(p: &[[f32; 2]]) -> f32 {
    let n = p.len();
    if n < 3 {
        return 0.0;
    }
    let twice: f32 = (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    twice.abs() / 2.0
}

/// The label of a measurement.
#[must_use]
pub fn measure_label(shape: &Shape) -> Option<String> {
    measure_label_in(shape, &MeasureUnit::default())
}

/// [`measure_label`] in `unit`.
#[must_use]
pub fn measure_label_in(shape: &Shape, unit: &MeasureUnit) -> Option<String> {
    let (k, u) = (unit.points, unit.label.as_str());
    Some(match shape {
        Shape::Distance(a, b) => format!("{:.2} {u}", path_length(&[*a, *b]) / k),
        Shape::Perimeter(p) => format!("{:.2} {u}", path_length(p) / k),
        Shape::Area(p) => format!("{:.2} sq {u}", polygon_area(p) / (k * k)),
        _ => return None,
    })
}

fn pdf_text(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '(' | ')' | '\\' => format!("\\{c}"),
            c if (c as u32) < 256 && !c.is_control() => c.to_string(),
            _ => "?".to_owned(),
        })
        .collect()
}

/// Helvetica width estimate of `s` at `size` (half an em per character, a little more for
/// capitals).
fn text_width(s: &str, size: f32) -> f32 {
    s.chars()
        .map(|c| if c.is_uppercase() { 0.68 } else { 0.52 })
        .sum::<f32>()
        * size
}

fn bounds(points: &[[f32; 2]]) -> [f32; 4] {
    points.iter().fold(
        [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ],
        |r, p| {
            [
                r[0].min(p[0]),
                r[1].min(p[1]),
                r[2].max(p[0]),
                r[3].max(p[1]),
            ]
        },
    )
}

fn grow(r: [f32; 4], d: f32) -> [f32; 4] {
    [r[0] - d, r[1] - d, r[2] + d, r[3] + d]
}

impl Shape {
    /// Page-point box the comment covers (its `/Rect` before turning to user space).
    #[must_use]
    pub fn bounds(&self, style: &Style) -> [f32; 4] {
        self.bounds_in(style, &MeasureUnit::default())
    }

    /// [`Shape::bounds`] with measurement labels in `unit`.
    #[must_use]
    pub fn bounds_in(&self, style: &Style, unit: &MeasureUnit) -> [f32; 4] {
        let w = style.width.max(0.5);
        match self {
            Shape::Ink(strokes) => {
                let all: Vec<[f32; 2]> = strokes.iter().flatten().copied().collect();
                grow(bounds(&all), w)
            }
            Shape::Line { a, b, arrow } => {
                grow(bounds(&[*a, *b]), w * if *arrow { 5.0 } else { 1.0 })
            }
            Shape::Polyline(p) | Shape::Polygon(p) => grow(bounds(p), w),
            Shape::Oval(r) | Shape::Rect(r) | Shape::Typewriter(r) | Shape::Stamp(r, _) => *r,
            Shape::Callout { rect, tip } => {
                let b = bounds(&[[rect[0], rect[1]], [rect[2], rect[3]], *tip]);
                grow(b, w * 5.0)
            }
            Shape::Distance(a, b) => {
                let label = measure_label_in(self, unit).unwrap_or_default();
                let mid = [f32::midpoint(a[0], b[0]), f32::midpoint(a[1], b[1])];
                let tw = text_width(&label, style.font_size) / 2.0 + 2.0;
                let r = bounds(&[
                    *a,
                    *b,
                    [mid[0] - tw, mid[1] - style.font_size * 1.6],
                    [mid[0] + tw, mid[1]],
                ]);
                grow(r, w * 4.0)
            }
            Shape::Perimeter(p) | Shape::Area(p) => {
                let label = measure_label_in(self, unit).unwrap_or_default();
                let last = p.last().copied().unwrap_or([0.0, 0.0]);
                let mut pts = p.clone();
                pts.push([
                    last[0] + text_width(&label, style.font_size) + 6.0,
                    last[1] - style.font_size * 1.6,
                ]);
                grow(bounds(&pts), w * 2.0)
            }
        }
    }

    /// The same shape mapped linearly from the box `from` onto the box `to` (resizing).
    #[must_use]
    pub fn mapped(&self, from: [f32; 4], to: [f32; 4]) -> Shape {
        let sx = (to[2] - to[0]) / (from[2] - from[0]).max(1e-3);
        let sy = (to[3] - to[1]) / (from[3] - from[1]).max(1e-3);
        let p = |q: [f32; 2]| [to[0] + (q[0] - from[0]) * sx, to[1] + (q[1] - from[1]) * sy];
        let r = |q: [f32; 4]| {
            let a = p([q[0], q[1]]);
            let b = p([q[2], q[3]]);
            [a[0], a[1], b[0], b[1]]
        };
        let ps = |v: &Vec<[f32; 2]>| v.iter().map(|q| p(*q)).collect::<Vec<_>>();
        match self {
            Shape::Ink(s) => Shape::Ink(s.iter().map(ps).collect()),
            Shape::Line { a, b, arrow } => Shape::Line {
                a: p(*a),
                b: p(*b),
                arrow: *arrow,
            },
            Shape::Polyline(v) => Shape::Polyline(ps(v)),
            Shape::Polygon(v) => Shape::Polygon(ps(v)),
            Shape::Oval(q) => Shape::Oval(r(*q)),
            Shape::Rect(q) => Shape::Rect(r(*q)),
            Shape::Callout { rect, tip } => Shape::Callout {
                rect: r(*rect),
                tip: p(*tip),
            },
            Shape::Typewriter(q) => Shape::Typewriter(r(*q)),
            Shape::Stamp(q, k) => Shape::Stamp(r(*q), *k),
            Shape::Distance(a, b) => Shape::Distance(p(*a), p(*b)),
            Shape::Perimeter(v) => Shape::Perimeter(ps(v)),
            Shape::Area(v) => Shape::Area(ps(v)),
        }
    }

    fn subtype(&self) -> &'static str {
        match self {
            Shape::Ink(_) => "Ink",
            Shape::Line { .. } | Shape::Distance(..) => "Line",
            Shape::Polyline(_) | Shape::Perimeter(_) => "PolyLine",
            Shape::Polygon(_) | Shape::Area(_) => "Polygon",
            Shape::Oval(_) => "Circle",
            Shape::Rect(_) => "Square",
            Shape::Callout { .. } | Shape::Typewriter(_) => "FreeText",
            Shape::Stamp(..) => "Stamp",
        }
    }
}

/// Content-stream writer in user space.
struct Pen {
    m: M,
    s: String,
}

impl Pen {
    fn pt(&self, p: [f32; 2]) -> (f32, f32) {
        let (x, y) = apply(self.m, p[0], p[1]);
        ((x * 100.0).round() / 100.0, (y * 100.0).round() / 100.0)
    }
    fn mv(&mut self, p: [f32; 2]) {
        let (x, y) = self.pt(p);
        let _ = write!(self.s, "{x} {y} m ");
    }
    fn ln(&mut self, p: [f32; 2]) {
        let (x, y) = self.pt(p);
        let _ = write!(self.s, "{x} {y} l ");
    }
    fn cv(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2]) {
        let (x1, y1) = self.pt(a);
        let (x2, y2) = self.pt(b);
        let (x3, y3) = self.pt(c);
        let _ = write!(self.s, "{x1} {y1} {x2} {y2} {x3} {y3} c ");
    }
    fn poly(&mut self, p: &[[f32; 2]]) {
        for (i, q) in p.iter().enumerate() {
            if i == 0 {
                self.mv(*q);
            } else {
                self.ln(*q);
            }
        }
    }
    fn rect(&mut self, r: [f32; 4]) {
        self.poly(&[[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]]);
        self.s.push_str("h ");
    }
    fn oval(&mut self, r: [f32; 4]) {
        let (cx, cy) = (f32::midpoint(r[0], r[2]), f32::midpoint(r[1], r[3]));
        let (rx, ry) = ((r[2] - r[0]) / 2.0, (r[3] - r[1]) / 2.0);
        let k = 0.552_284_8;
        self.mv([cx + rx, cy]);
        self.cv(
            [cx + rx, cy + ry * k],
            [cx + rx * k, cy + ry],
            [cx, cy + ry],
        );
        self.cv(
            [cx - rx * k, cy + ry],
            [cx - rx, cy + ry * k],
            [cx - rx, cy],
        );
        self.cv(
            [cx - rx, cy - ry * k],
            [cx - rx * k, cy - ry],
            [cx, cy - ry],
        );
        self.cv(
            [cx + rx * k, cy - ry],
            [cx + rx, cy - ry * k],
            [cx + rx, cy],
        );
        self.s.push_str("h ");
    }
    /// A filled arrow head at `b` pointing away from `a`.
    fn arrow(&mut self, a: [f32; 2], b: [f32; 2], size: f32) {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt().max(1e-3);
        let (ux, uy) = (dx / len, dy / len);
        let back = [b[0] - ux * size, b[1] - uy * size];
        let (nx, ny) = (-uy * size * 0.45, ux * size * 0.45);
        self.poly(&[
            b,
            [back[0] + nx, back[1] + ny],
            [back[0] - nx, back[1] - ny],
        ]);
        self.s.push_str("h f\n");
    }
    /// Text lines from page point `at` (top-left of the first line).
    fn text(&mut self, lines: &[&str], at: [f32; 2], size: f32, font: &str) {
        let lead = size * 1.2;
        for (i, line) in lines.iter().enumerate() {
            let (x, y) = self.pt([at[0], at[1] + size * 0.9 + lead * i as f32]);
            let _ = writeln!(
                self.s,
                "BT /{font} {size} Tf {x} {y} Td ({}) Tj ET",
                pdf_text(line)
            );
        }
    }
}

/// Wrap `text` into lines of at most `width` points at `size`.
fn wrap_text(text: &str, width: f32, size: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.lines() {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let cand = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if text_width(&cand, size) > width && !line.is_empty() {
                out.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = cand;
            }
        }
        out.push(line);
    }
    out
}

/// Appearance content and resources of `shape`; `m` maps page points to user space.
#[allow(clippy::too_many_lines)]
fn appearance(
    shape: &Shape,
    style: &Style,
    text: &str,
    m: M,
    unit: &MeasureUnit,
) -> (String, Dictionary) {
    let [r, g, b] = style.color.map(|c| f32::from(c) / 255.0);
    let w = style.width.max(0.1);
    let mut pen = Pen {
        m,
        s: format!("/GS0 gs {r} {g} {b} RG {r} {g} {b} rg {w} w 1 J 1 j\n"),
    };
    let mut res = Dictionary::new();
    let mut gs = Dictionary::new();
    gs.set("Type", Object::Name(b"ExtGState".to_vec()));
    gs.set("CA", Object::Real(style.opacity.clamp(0.0, 1.0)));
    gs.set("ca", Object::Real(style.opacity.clamp(0.0, 1.0)));
    let mut ext = Dictionary::new();
    ext.set("GS0", gs);
    res.set("ExtGState", ext);
    let mut fonts = Dictionary::new();
    for (key, base) in [("Helv", "Helvetica"), ("HeBo", "Helvetica-Bold")] {
        let mut f = Dictionary::new();
        f.set("Type", Object::Name(b"Font".to_vec()));
        f.set("Subtype", Object::Name(b"Type1".to_vec()));
        f.set("BaseFont", Object::Name(base.as_bytes().to_vec()));
        f.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        fonts.set(key, f);
    }
    res.set("Font", fonts);
    let fs = style.font_size.max(4.0);
    match shape {
        Shape::Ink(strokes) => {
            for s in strokes {
                if s.len() == 1 {
                    pen.mv(s[0]);
                    pen.ln([s[0][0] + 0.01, s[0][1]]);
                } else {
                    pen.poly(s);
                }
                pen.s.push_str("S\n");
            }
        }
        Shape::Line { a, b, arrow } => {
            pen.mv(*a);
            pen.ln(*b);
            pen.s.push_str("S\n");
            if *arrow {
                pen.arrow(*a, *b, (w * 4.0).max(6.0));
            }
        }
        Shape::Polyline(p) => {
            pen.poly(p);
            pen.s.push_str("S\n");
        }
        Shape::Polygon(p) => {
            pen.poly(p);
            pen.s.push_str("s\n");
        }
        Shape::Oval(q) => {
            pen.oval(grow(*q, -w / 2.0));
            pen.s.push_str("S\n");
        }
        Shape::Rect(q) => {
            pen.rect(grow(*q, -w / 2.0));
            pen.s.push_str("S\n");
        }
        Shape::Callout { rect, tip } => {
            pen.s.push_str("q 1 1 1 rg ");
            pen.rect(*rect);
            pen.s.push_str("f Q\n");
            pen.rect(grow(*rect, -w / 2.0));
            pen.s.push_str("S\n");
            // Leader from the middle of the side facing the tip.
            let cx = f32::midpoint(rect[0], rect[2]);
            let cy = f32::midpoint(rect[1], rect[3]);
            let from = if tip[0] < rect[0] {
                [rect[0], cy]
            } else if tip[0] > rect[2] {
                [rect[2], cy]
            } else if tip[1] < rect[1] {
                [cx, rect[1]]
            } else {
                [cx, rect[3]]
            };
            pen.mv(from);
            pen.ln(*tip);
            pen.s.push_str("S\n");
            pen.arrow(from, *tip, (w * 4.0).max(6.0));
            let lines = wrap_text(text, rect[2] - rect[0] - 8.0, fs);
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            pen.text(&refs, [rect[0] + 4.0, rect[1] + 3.0], fs, "Helv");
        }
        Shape::Typewriter(q) => {
            let lines: Vec<&str> = text.lines().collect();
            pen.text(&lines, [q[0] + 1.0, q[1] + 1.0], fs, "Helv");
        }
        Shape::Stamp(q, kind) => {
            let rad = ((q[3] - q[1]) * 0.18).min(8.0);
            let sw = ((q[3] - q[1]) * 0.07).clamp(1.5, 4.0);
            let i = grow(*q, -sw / 2.0);
            let k = rad * 0.45;
            let _ = write!(pen.s, "{sw} w ");
            pen.mv([i[0] + rad, i[1]]);
            pen.ln([i[2] - rad, i[1]]);
            pen.cv([i[2] - k, i[1]], [i[2], i[1] + k], [i[2], i[1] + rad]);
            pen.ln([i[2], i[3] - rad]);
            pen.cv([i[2], i[3] - k], [i[2] - k, i[3]], [i[2] - rad, i[3]]);
            pen.ln([i[0] + rad, i[3]]);
            pen.cv([i[0] + k, i[3]], [i[0], i[3] - k], [i[0], i[3] - rad]);
            pen.ln([i[0], i[1] + rad]);
            pen.cv([i[0], i[1] + k], [i[0] + k, i[1]], [i[0] + rad, i[1]]);
            pen.s.push_str("h S\n");
            let label = kind.label();
            let size = ((q[3] - q[1]) * 0.5)
                .min((q[2] - q[0] - 2.0 * sw - 6.0) / (text_width(label, 1.0) * 1.05));
            let size = size.max(4.0);
            let tw = text_width(label, size) * 1.05;
            let x = (q[0] + q[2] - tw) / 2.0;
            let y = f32::midpoint(q[1], q[3]) - size * 0.62;
            pen.text(&[label], [x, y], size, "HeBo");
        }
        Shape::Distance(a, b) => {
            pen.mv(*a);
            pen.ln(*b);
            pen.s.push_str("S\n");
            // End ticks across the line.
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = (dx * dx + dy * dy).sqrt().max(1e-3);
            let (nx, ny) = (-dy / len * w * 4.0, dx / len * w * 4.0);
            for p in [a, b] {
                pen.mv([p[0] + nx, p[1] + ny]);
                pen.ln([p[0] - nx, p[1] - ny]);
            }
            pen.s.push_str("S\n");
            pen.arrow(*b, *a, (w * 4.0).max(5.0));
            pen.arrow(*a, *b, (w * 4.0).max(5.0));
            let label = measure_label_in(shape, unit).unwrap_or_default();
            let mid = [f32::midpoint(a[0], b[0]), f32::midpoint(a[1], b[1])];
            let tw = text_width(&label, fs);
            pen.text(
                &[&label],
                [mid[0] - tw / 2.0, mid[1] - fs * 1.4],
                fs,
                "Helv",
            );
        }
        Shape::Perimeter(p) | Shape::Area(p) => {
            pen.poly(p);
            pen.s.push_str(if matches!(shape, Shape::Area(_)) {
                "s\n"
            } else {
                "S\n"
            });
            let label = measure_label_in(shape, unit).unwrap_or_default();
            let last = p.last().copied().unwrap_or([0.0, 0.0]);
            pen.text(&[&label], [last[0] + 4.0, last[1] - fs * 1.4], fs, "Helv");
        }
    }
    (pen.s, res)
}

impl PdfDocument {
    fn user_matrix(&self, page: usize) -> Result<(M, M), String> {
        let info = self.pages.get(page).ok_or("no such page")?;
        let base = crate::page_matrix(info.crop, info.rotate);
        let inv = invert(base).ok_or("page matrix")?;
        Ok((base, inv))
    }

    /// Add a drawn comment on `page`; `text` is its comment (or the text of a callout /
    /// typewriter). It shows at once and is written by [`PdfDocument::save`].
    pub fn add_drawing(
        &mut self,
        page: usize,
        shape: &Shape,
        style: &Style,
        author: &str,
        text: &str,
    ) -> Result<(), String> {
        self.add_drawing_in(page, shape, style, author, text, &MeasureUnit::default())
    }

    /// [`PdfDocument::add_drawing`] with measurements (distance, perimeter, area) in `unit`:
    /// their label and `/Measure` dictionary (§12.9) use it.
    pub fn add_drawing_in(
        &mut self,
        page: usize,
        shape: &Shape,
        style: &Style,
        author: &str,
        text: &str,
        unit: &MeasureUnit,
    ) -> Result<(), String> {
        if self.doc.is_encrypted() && self.doc.encryption_state.is_none() {
            return Err("comments cannot be added to an encrypted document".into());
        }
        let info = self.pages.get(page).ok_or("no such page")?.clone();
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"Annot".to_vec()));
        d.set("Subtype", Object::Name(shape.subtype().as_bytes().to_vec()));
        d.set("F", 4);
        d.set("P", Object::Reference(info.id));
        d.set("T", text_obj(author));
        let now = pdf_date();
        d.set("CreationDate", text_obj(&now));
        d.set(
            "NM",
            text_obj(&format!("bo-draw-{now}-{}", self.doc.max_id + 1)),
        );
        self.begin("drawing");
        let id = self.add(Object::Dictionary(Dictionary::new()));
        let r = self.write_drawing(id, page, shape, style, text, d, Some(unit));
        if r.is_ok() {
            // Link it from the page's /Annots.
            if let Ok(Object::Reference(arr)) = self
                .doc
                .get_dictionary(info.id)
                .map_err(|e| e.to_string())?
                .get(b"Annots")
            {
                let arr = *arr;
                self.touch(arr);
                if let Ok(Object::Array(v)) = self.doc.get_object_mut(arr) {
                    v.push(Object::Reference(id));
                }
            } else {
                self.touch(info.id);
                let p = self
                    .doc
                    .get_dictionary_mut(info.id)
                    .map_err(|e| e.to_string())?;
                match p.get_mut(b"Annots") {
                    Ok(Object::Array(v)) => v.push(Object::Reference(id)),
                    _ => p.set("Annots", vec![Object::Reference(id)]),
                }
            }
        }
        self.commit();
        r
    }

    /// Fill annotation `id` (dictionary `d` with the fixed keys) with `shape`'s geometry keys,
    /// style and a new appearance stream; measurements in `unit`, else in the unit of the
    /// comment's own `/Measure` (inches when it has none).
    #[allow(clippy::too_many_arguments)]
    fn write_drawing(
        &mut self,
        id: lopdf::ObjectId,
        page: usize,
        shape: &Shape,
        style: &Style,
        text: &str,
        mut d: Dictionary,
        unit: Option<&MeasureUnit>,
    ) -> Result<(), String> {
        let unit = unit.cloned().unwrap_or_else(|| self.measure_unit_of(&d));
        let unit = &unit;
        let (_, inv) = self.user_matrix(page)?;
        let u = |p: [f32; 2]| {
            let (x, y) = apply(inv, p[0], p[1]);
            [x, y]
        };
        let flat = |v: &[[f32; 2]]| -> Vec<f32> { v.iter().flat_map(|p| u(*p)).collect() };
        let b = shape.bounds_in(style, unit);
        let corners = [
            u([b[0], b[1]]),
            u([b[2], b[3]]),
            u([b[0], b[3]]),
            u([b[2], b[1]]),
        ];
        let rect = bounds(&corners);
        let [cr, cg, cb] = style.color.map(|c| f32::from(c) / 255.0);
        d.set("Rect", nums(&rect));
        d.set("C", nums(&[cr, cg, cb]));
        d.set("CA", Object::Real(style.opacity.clamp(0.0, 1.0)));
        let mut bs = Dictionary::new();
        bs.set("W", Object::Real(style.width));
        d.set("BS", bs);
        d.set("M", text_obj(&pdf_date()));
        d.set("Contents", text_obj(text));
        for k in [
            &b"InkList"[..],
            b"L",
            b"LE",
            b"Vertices",
            b"CL",
            b"IT",
            b"Measure",
            b"DA",
            b"Name",
        ] {
            d.remove(k);
        }
        let measure = || {
            // §12.9 Table 261: a rectilinear measure, 1 unit = 1 unit; Table 263: /C turns
            // default user space units (points) into the unit.
            let mut x = Dictionary::new();
            x.set("Type", Object::Name(b"NumberFormat".to_vec()));
            x.set("U", text_obj(&unit.label));
            x.set("C", Object::Real(1.0 / unit.points));
            x.set("D", 100);
            let mut a = Dictionary::new();
            a.set("Type", Object::Name(b"NumberFormat".to_vec()));
            a.set("U", text_obj(&format!("sq {}", unit.label)));
            a.set("C", Object::Real(1.0));
            a.set("D", 100);
            let mut m = Dictionary::new();
            m.set("Type", Object::Name(b"Measure".to_vec()));
            m.set("Subtype", Object::Name(b"RL".to_vec()));
            m.set("R", text_obj(&format!("1 {u} = 1 {u}", u = unit.label)));
            m.set("X", vec![Object::Dictionary(x.clone())]);
            m.set("D", vec![Object::Dictionary(x)]);
            m.set("A", vec![Object::Dictionary(a)]);
            m
        };
        let name = |s: &str| Object::Name(s.as_bytes().to_vec());
        match shape {
            Shape::Ink(strokes) => {
                let list: Vec<Object> = strokes.iter().map(|s| nums(&flat(s))).collect();
                d.set("InkList", list);
            }
            Shape::Line { a, b, arrow } => {
                d.set("L", nums(&flat(&[*a, *b])));
                if *arrow {
                    d.set("LE", vec![name("None"), name("ClosedArrow")]);
                }
            }
            Shape::Distance(a, b) => {
                d.set("L", nums(&flat(&[*a, *b])));
                d.set("LE", vec![name("ClosedArrow"), name("ClosedArrow")]);
                d.set("IT", name("LineDimension"));
                d.set("Measure", measure());
                d.set(
                    "Contents",
                    text_obj(&measure_label_in(shape, unit).unwrap_or_default()),
                );
            }
            Shape::Polyline(p) => d.set("Vertices", nums(&flat(p))),
            Shape::Polygon(p) => d.set("Vertices", nums(&flat(p))),
            Shape::Perimeter(p) | Shape::Area(p) => {
                d.set("Vertices", nums(&flat(p)));
                d.set(
                    "IT",
                    name(if matches!(shape, Shape::Area(_)) {
                        "PolygonDimension"
                    } else {
                        "PolyLineDimension"
                    }),
                );
                d.set("Measure", measure());
                d.set(
                    "Contents",
                    text_obj(&measure_label_in(shape, unit).unwrap_or_default()),
                );
            }
            Shape::Oval(_) | Shape::Rect(_) => {}
            Shape::Callout { tip, rect } => {
                d.set("IT", name("FreeTextCallout"));
                let cy = f32::midpoint(rect[1], rect[3]);
                let edge = if tip[0] < rect[0] { rect[0] } else { rect[2] };
                d.set("CL", nums(&flat(&[*tip, [edge, cy]])));
                // §12.5.6.6: the text box inside /Rect.
                let rd = [
                    (rect[0] - b[0]).max(0.0),
                    (rect[1] - b[1]).max(0.0),
                    (b[2] - rect[2]).max(0.0),
                    (b[3] - rect[3]).max(0.0),
                ];
                d.set("RD", nums(&rd));
                d.set(
                    "DA",
                    text_obj(&format!("{cr} {cg} {cb} rg /Helv {} Tf", style.font_size)),
                );
            }
            Shape::Typewriter(_) => {
                d.set("IT", name("FreeTextTypeWriter"));
                d.set(
                    "DA",
                    text_obj(&format!("{cr} {cg} {cb} rg /Helv {} Tf", style.font_size)),
                );
            }
            Shape::Stamp(_, k) => d.set("Name", name(k.name())),
        }
        let (content, res) = appearance(shape, style, text, inv, unit);
        let mut ap_dict = Dictionary::new();
        ap_dict.set("Type", Object::Name(b"XObject".to_vec()));
        ap_dict.set("Subtype", Object::Name(b"Form".to_vec()));
        ap_dict.set("BBox", nums(&rect));
        ap_dict.set("Resources", res);
        let ap_id = self.add(Object::Stream(Stream::new(ap_dict, content.into_bytes())));
        let mut ap = Dictionary::new();
        ap.set("N", Object::Reference(ap_id));
        d.set("AP", ap);
        self.touch(id);
        *self.doc.get_object_mut(id).map_err(|e| e.to_string())? = Object::Dictionary(d);
        Ok(())
    }

    /// The geometry, style and text of a drawn comment (read back from its keys); `None` for
    /// comments without a geometry this reader draws.
    #[must_use]
    pub fn drawing(&self, a: &Annot) -> Option<(Shape, Style, String)> {
        let id = a.id?;
        let d = self.doc.get_dictionary(id).ok()?;
        let (base, _) = self.user_matrix(a.page).ok()?;
        let doc = &self.doc;
        let nums_of = |o: &Object| -> Vec<f32> {
            resolve(doc, o)
                .as_array()
                .map(|v| {
                    v.iter()
                        .filter_map(|x| resolve(doc, x).as_float().ok())
                        .collect()
                })
                .unwrap_or_default()
        };
        let pts = |v: Vec<f32>| -> Vec<[f32; 2]> {
            v.chunks_exact(2)
                .map(|c| {
                    let (x, y) = apply(base, c[0], c[1]);
                    [x, y]
                })
                .collect()
        };
        let key = |k: &[u8]| d.get(k).ok().map(nums_of).unwrap_or_default();
        let name = |k: &[u8]| -> Vec<u8> {
            d.get(k)
                .ok()
                .map(|o| resolve(doc, o))
                .and_then(|o| o.as_name().ok())
                .map(<[u8]>::to_vec)
                .unwrap_or_default()
        };
        let it = name(b"IT");
        let subtype = name(b"Subtype");
        let rect = a.rect;
        let width = d
            .get(b"BS")
            .ok()
            .map(|o| resolve(doc, o))
            .and_then(|o| o.as_dict().ok())
            .and_then(|bs| bs.get(b"W").ok())
            .and_then(|w| resolve(doc, w).as_float().ok())
            .unwrap_or(1.0);
        let opacity = d
            .get(b"CA")
            .ok()
            .and_then(|o| resolve(doc, o).as_float().ok())
            .unwrap_or(1.0);
        let font_size = d
            .get(b"DA")
            .ok()
            .and_then(|o| resolve(doc, o).as_str().ok())
            .and_then(|s| {
                let s = String::from_utf8_lossy(s).into_owned();
                let parts: Vec<&str> = s.split_whitespace().collect();
                parts
                    .iter()
                    .position(|p| *p == "Tf")
                    .and_then(|i| parts.get(i.wrapping_sub(1)))
                    .and_then(|v| v.parse::<f32>().ok())
            })
            .unwrap_or(12.0);
        let style = Style {
            color: a.color,
            opacity,
            width,
            font_size,
        };
        let shape = match subtype.as_slice() {
            b"Ink" => {
                let list = d.get(b"InkList").ok().map(|o| resolve(doc, o))?;
                let strokes = list
                    .as_array()
                    .ok()?
                    .iter()
                    .map(|s| pts(nums_of(s)))
                    .collect();
                Shape::Ink(strokes)
            }
            b"Line" => {
                let p = pts(key(b"L"));
                let (pa, pb) = (*p.first()?, *p.get(1)?);
                if it == b"LineDimension" {
                    Shape::Distance(pa, pb)
                } else {
                    let le = d
                        .get(b"LE")
                        .ok()
                        .and_then(|o| o.as_array().ok())
                        .and_then(|v| v.get(1))
                        .and_then(|o| o.as_name().ok())
                        .is_some_and(|n| n != b"None");
                    Shape::Line {
                        a: pa,
                        b: pb,
                        arrow: le,
                    }
                }
            }
            b"PolyLine" => {
                let p = pts(key(b"Vertices"));
                if it == b"PolyLineDimension" {
                    Shape::Perimeter(p)
                } else {
                    Shape::Polyline(p)
                }
            }
            b"Polygon" => {
                let p = pts(key(b"Vertices"));
                if it == b"PolygonDimension" {
                    Shape::Area(p)
                } else {
                    Shape::Polygon(p)
                }
            }
            b"Circle" => Shape::Oval(rect),
            b"Square" => Shape::Rect(rect),
            b"FreeText" if it == b"FreeTextTypeWriter" => Shape::Typewriter(rect),
            b"FreeText" if it == b"FreeTextCallout" => {
                let cl = pts(key(b"CL"));
                let tip = *cl.first()?;
                let rd = key(b"RD");
                let inner = match rd.as_slice() {
                    [l, t, r, b] => [rect[0] + l, rect[1] + t, rect[2] - r, rect[3] - b],
                    _ => rect,
                };
                Shape::Callout { rect: inner, tip }
            }
            b"Stamp" => Shape::Stamp(rect, StampKind::from_name(&name(b"Name"))),
            _ => return None,
        };
        Some((shape, style, a.contents.clone()))
    }

    /// Unit of a measurement comment (its `/Measure`; inches when it has none).
    #[must_use]
    pub fn measure_unit(&self, a: &Annot) -> MeasureUnit {
        a.id.and_then(|id| self.doc.get_dictionary(id).ok())
            .map(|d| self.measure_unit_of(d))
            .unwrap_or_default()
    }

    fn measure_unit_of(&self, d: &Dictionary) -> MeasureUnit {
        d.get(b"Measure")
            .ok()
            .and_then(|m| MeasureUnit::of_measure(&self.doc, m))
            .unwrap_or_default()
    }

    /// Give a drawn comment another geometry, style or text (resize, properties): its keys and
    /// appearance are written again. Comments without a geometry of ours get only their
    /// `/Rect` (their appearance scales to it).
    pub fn update_drawing(
        &mut self,
        a: &Annot,
        shape: &Shape,
        style: &Style,
        text: &str,
    ) -> Result<(), String> {
        let id = a.id.ok_or("the comment cannot be changed")?;
        let d = self
            .doc
            .get_dictionary(id)
            .map_err(|e| e.to_string())?
            .clone();
        self.begin("change drawing");
        let r = self.write_drawing(id, a.page, shape, style, text, d, None);
        self.commit();
        r
    }

    /// Resize a comment to `rect` (page points, left top right bottom).
    pub fn resize_annotation(&mut self, a: &Annot, rect: [f32; 4]) -> Result<(), String> {
        if rect[2] - rect[0] < 2.0 || rect[3] - rect[1] < 2.0 {
            return Err("too small".into());
        }
        if let Some((shape, style, text)) = self.drawing(a) {
            let from = shape.bounds_in(&style, &self.measure_unit(a));
            // Map the shape's own box so the new bounds land on `rect`.
            let shape = shape.mapped(from, rect);
            return self.update_drawing(a, &shape, &style, &text);
        }
        let id = a.id.ok_or("the comment cannot be changed")?;
        let (_, inv) = self.user_matrix(a.page)?;
        let u = |x: f32, y: f32| {
            let (x, y) = apply(inv, x, y);
            [x, y]
        };
        let r = bounds(&[u(rect[0], rect[1]), u(rect[2], rect[3])]);
        self.begin("resize comment");
        self.touch(id);
        if let Ok(d) = self.doc.get_dictionary_mut(id) {
            d.set("Rect", nums(&r));
            d.set("M", text_obj(&pdf_date()));
        }
        self.commit();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AnnotKind;

    fn blank() -> PdfDocument {
        let pdf = crate::tests::build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>",
        ]);
        PdfDocument::open(pdf, crate::standard_fonts()).expect("pdf")
    }

    #[test]
    fn measurements() {
        assert!((path_length(&[[0.0, 0.0], [72.0, 0.0], [72.0, 72.0]]) - 144.0).abs() < 1e-3);
        assert!(
            (polygon_area(&[[0.0, 0.0], [72.0, 0.0], [72.0, 72.0], [0.0, 72.0]]) - 5184.0).abs()
                < 1e-2
        );
        assert_eq!(
            measure_label(&Shape::Distance([0.0, 0.0], [108.0, 0.0])).as_deref(),
            Some("1.50 in")
        );
        assert_eq!(
            measure_label(&Shape::Area(vec![
                [0.0, 0.0],
                [144.0, 0.0],
                [144.0, 72.0],
                [0.0, 72.0]
            ]))
            .as_deref(),
            Some("2.00 sq in")
        );
    }

    #[test]
    fn measurements_saved_in_the_chosen_unit() {
        let mut doc = blank();
        let mm = MeasureUnit::new("mm", 72.0 / 25.4);
        let style = Style::default();
        // 144 pt = 50.8 mm.
        let d = Shape::Distance([60.0, 600.0], [204.0, 600.0]);
        doc.add_drawing_in(0, &d, &style, "me", "", &mm)
            .expect("add");
        let a = doc.annotations().pop().expect("annot");
        assert_eq!(a.contents, "50.80 mm");
        let unit = doc.measure_unit(&a);
        assert_eq!(unit.label, "mm");
        assert!((unit.points - 72.0 / 25.4).abs() < 1e-3);
        // §12.9: /Measure /RL with /R, /X number format /U mm /C = 1 / points per mm.
        let dict = doc.doc.get_dictionary(a.id.expect("id")).expect("dict");
        let m = dict
            .get(b"Measure")
            .and_then(Object::as_dict)
            .expect("measure");
        assert_eq!(
            m.get(b"Subtype").and_then(Object::as_name).ok(),
            Some(&b"RL"[..])
        );
        assert_eq!(
            m.get(b"R").and_then(Object::as_str).ok(),
            Some(&b"1 mm = 1 mm"[..])
        );
        let x = m.get(b"X").and_then(Object::as_array).expect("x")[0]
            .as_dict()
            .expect("fmt");
        let c = x.get(b"C").and_then(Object::as_float).expect("c");
        assert!((c - 25.4 / 72.0).abs() < 1e-4);
        // Restyling keeps the comment's own unit.
        let (shape, mut st, text) = doc.drawing(&a).expect("drawing");
        st.width = 3.0;
        doc.update_drawing(&a, &shape, &st, &text).expect("update");
        let a = doc.annotations().pop().expect("annot");
        assert_eq!(a.contents, "50.80 mm");
        // Area in square centimetres: 144 × 72 pt = 5.08 × 2.54 cm = 12.90 sq cm.
        let cm = MeasureUnit::new("cm", 72.0 / 2.54);
        let area = Shape::Area(vec![
            [400.0, 600.0],
            [544.0, 600.0],
            [544.0, 672.0],
            [400.0, 672.0],
        ]);
        assert_eq!(measure_label_in(&area, &cm).as_deref(), Some("12.90 sq cm"));
        // Bad units fall back to inches.
        assert_eq!(MeasureUnit::new("", 0.0), MeasureUnit::default());
    }

    #[test]
    fn every_shape_round_trips_through_its_keys() {
        let mut doc = blank();
        let style = Style {
            color: [10, 20, 200],
            opacity: 0.5,
            width: 2.0,
            font_size: 14.0,
        };
        let shapes = vec![
            Shape::Ink(vec![vec![[100.0, 100.0], [120.0, 110.0], [140.0, 100.0]]]),
            Shape::Line {
                a: [100.0, 200.0],
                b: [300.0, 220.0],
                arrow: true,
            },
            Shape::Polyline(vec![[50.0, 300.0], [80.0, 320.0], [120.0, 300.0]]),
            Shape::Polygon(vec![[200.0, 300.0], [260.0, 300.0], [230.0, 350.0]]),
            Shape::Oval([300.0, 300.0, 380.0, 360.0]),
            Shape::Rect([400.0, 300.0, 480.0, 360.0]),
            Shape::Callout {
                rect: [300.0, 400.0, 450.0, 440.0],
                tip: [250.0, 480.0],
            },
            Shape::Typewriter([60.0, 500.0, 200.0, 520.0]),
            Shape::Stamp([300.0, 500.0, 450.0, 550.0], StampKind::Approved),
            Shape::Distance([60.0, 600.0], [204.0, 600.0]),
            Shape::Perimeter(vec![[300.0, 600.0], [372.0, 600.0], [372.0, 672.0]]),
            Shape::Area(vec![
                [400.0, 600.0],
                [472.0, 600.0],
                [472.0, 672.0],
                [400.0, 672.0],
            ]),
        ];
        for s in &shapes {
            doc.add_drawing(0, s, &style, "me", "hello").expect("add");
        }
        let annots = doc.annotations();
        assert_eq!(annots.len(), shapes.len());
        for (a, s) in annots.iter().zip(&shapes) {
            let (back, st, text) = doc.drawing(a).expect("drawing");
            assert_eq!(st.color, [10, 20, 200]);
            assert!((st.opacity - 0.5).abs() < 1e-3 && (st.width - 2.0).abs() < 1e-3);
            match (s, &back) {
                (Shape::Oval(_), Shape::Oval(_))
                | (Shape::Rect(_), Shape::Rect(_))
                | (Shape::Typewriter(_), Shape::Typewriter(_))
                | (Shape::Stamp(..), Shape::Stamp(..)) => {}
                (Shape::Callout { tip: t1, rect: r1 }, Shape::Callout { tip: t2, rect: r2 }) => {
                    assert!((t1[0] - t2[0]).abs() < 0.1 && (t1[1] - t2[1]).abs() < 0.1);
                    assert!(
                        r1.iter().zip(r2).all(|(x, y)| (x - y).abs() < 0.1),
                        "{r1:?} {r2:?}"
                    );
                    assert!((st.font_size - 14.0).abs() < 1e-3);
                }
                _ => {
                    let (fa, fb) = (format!("{s:?}"), format!("{back:?}"));
                    // Same kind and points (to rounding).
                    assert_eq!(fa.split('(').next(), fb.split('(').next());
                    assert_eq!(fa.split('{').next(), fb.split('{').next());
                }
            }
            if matches!(s, Shape::Distance(..)) {
                assert_eq!(text, "2.00 in");
            }
        }
        assert!(annots.iter().any(|a| a.kind == AnnotKind::Stamp));
        // The page renders with every appearance.
        let page = doc
            .render(0, crate::RenderOptions::default())
            .expect("render");
        assert!(page.list.commands.len() > 20);
        // Typewriter / callout text in Helvetica, the stamp in Helvetica-Bold: two faces
        // (direct font dictionaries of the appearance resources are told apart).
        let faces: std::collections::HashSet<u32> = page
            .list
            .commands
            .iter()
            .filter_map(|c| match c {
                bo_render::Command::Glyphs { face, .. } => Some(*face),
                _ => None,
            })
            .collect();
        assert!(faces.len() >= 2, "{faces:?}");
        // Save and read back.
        let bytes = doc.save().expect("save");
        let again = PdfDocument::open(bytes, crate::standard_fonts()).expect("reopen");
        assert_eq!(again.annotations().len(), shapes.len());
    }

    #[test]
    fn resize_and_restyle() {
        let mut doc = blank();
        doc.add_drawing(
            0,
            &Shape::Rect([100.0, 100.0, 200.0, 150.0]),
            &Style::default(),
            "me",
            "",
        )
        .expect("add");
        let a = doc.annotations().remove(0);
        doc.resize_annotation(&a, [100.0, 100.0, 300.0, 250.0])
            .expect("resize");
        let a = doc.annotations().remove(0);
        assert!(
            a.rect
                .iter()
                .zip([100.0, 100.0, 300.0, 250.0])
                .all(|(x, y)| (x - y).abs() < 0.1),
            "{:?}",
            a.rect
        );
        let (shape, mut style, text) = doc.drawing(&a).expect("drawing");
        style.color = [0, 128, 0];
        style.width = 4.0;
        doc.update_drawing(&a, &shape, &style, &text)
            .expect("style");
        let a = doc.annotations().remove(0);
        assert_eq!(a.color, [0, 128, 0]);
        assert!((doc.drawing(&a).expect("d").1.width - 4.0).abs() < 1e-3);
        // Undo goes back one step at a time.
        assert!(doc.undo());
        assert_eq!(doc.annotations()[0].color, Style::default().color);
    }
}
