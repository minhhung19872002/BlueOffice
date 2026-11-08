//! Windows metafiles → display list, written from the public specifications
//! [MS-EMF] (Enhanced Metafile Format) and [MS-WMF] (Windows Metafile Format).
//!
//! The records used by common clip art are drawn: polygons, polylines and Béziers
//! (16- and 32-bit), poly-polygons, rectangles, rounded rectangles, ellipses,
//! move/line, paths (begin/end/fill/stroke), pens, brushes and fonts (with the stock
//! objects), the window/viewport mapping and the world transform (EMF), save/restore
//! of the device context, text output (`EMR_EXTTEXTOUTW`, `META_TEXTOUT`,
//! `META_EXTTEXTOUT`) and device-independent bitmaps (`EMR_STRETCHDIBITS`,
//! `EMR_BITBLT`, `EMR_STRETCHBLT`, `META_STRETCHDIB`, `META_DIBSTRETCHBLT`). Other
//! records (EMF+ comments, clipping, raster operations, palettes) are skipped.
//!
//! The picture is fitted into a `width × height` point box. Bitmaps are returned as
//! BMP files keyed by the `image` ids of their commands.

use crate::{Command, Glyph, PageList, Paint, PathSeg, Rgba, Stroke};
use bo_text::FontManager;
use std::collections::BTreeMap;

/// A converted metafile.
pub struct Converted {
    /// The picture as a display list (`width` × `height`, drawing commands).
    pub page: PageList,
    /// BMP bytes of the bitmaps referenced by `Image` commands.
    pub images: BTreeMap<String, Vec<u8>>,
}

/// Is `bytes` an EMF ([MS-EMF] §2.3.4.2 EMR_HEADER, signature " EMF")?
pub fn is_emf(bytes: &[u8]) -> bool {
    u32le(bytes, 0) == Some(1) && u32le(bytes, 40) == Some(0x464D_4520)
}

/// Is `bytes` a WMF (placeable header [MS-WMF] §2.3.2.3 or META_HEADER §2.3.2.2)?
pub fn is_wmf(bytes: &[u8]) -> bool {
    u32le(bytes, 0) == Some(0x9AC6_CDD7)
        || (matches!(u16le(bytes, 0), Some(1 | 2))
            && u16le(bytes, 2) == Some(9)
            && matches!(u16le(bytes, 4), Some(0x0100 | 0x0300)))
}

fn u16le(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}
fn i16le(b: &[u8], i: usize) -> Option<i16> {
    u16le(b, i).map(|v| v as i16)
}
fn u32le(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}
fn i32le(b: &[u8], i: usize) -> Option<i32> {
    u32le(b, i).map(|v| v as i32)
}
fn f32le(b: &[u8], i: usize) -> Option<f32> {
    u32le(b, i).map(f32::from_bits)
}

/// COLORREF (0x00BBGGRR, [MS-WMF] §2.2.2.8).
fn colorref(v: u32) -> Rgba {
    Rgba(v as u8, (v >> 8) as u8, (v >> 16) as u8, 255)
}

#[derive(Debug, Clone)]
enum Obj {
    Pen {
        color: Rgba,
        width: f64,
        null: bool,
    },
    Brush(Option<Rgba>),
    Font {
        height: f64,
        bold: bool,
        italic: bool,
        face: String,
    },
    Other,
}

#[derive(Debug, Clone)]
struct Dc {
    pen: (Rgba, f64, bool),
    brush: Option<Rgba>,
    font: (f64, bool, bool, String),
    text_color: Rgba,
    text_align: u32,
    win_org: (f64, f64),
    win_ext: Option<(f64, f64)>,
    vp_org: (f64, f64),
    vp_ext: Option<(f64, f64)>,
    /// World transform `[m11, m12, m21, m22, dx, dy]` (EMF).
    world: [f64; 6],
    pos: (f64, f64),
}

impl Default for Dc {
    fn default() -> Self {
        Dc {
            pen: (Rgba(0, 0, 0, 255), 0.0, false),
            brush: Some(Rgba(255, 255, 255, 255)),
            font: (12.0, false, false, "Arial".into()),
            text_color: Rgba(0, 0, 0, 255),
            text_align: 0,
            win_org: (0.0, 0.0),
            win_ext: None,
            vp_org: (0.0, 0.0),
            vp_ext: None,
            world: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            pos: (0.0, 0.0),
        }
    }
}

/// Drawing state shared by the EMF and WMF players.
struct Player<'f> {
    fonts: &'f mut FontManager,
    dc: Dc,
    stack: Vec<Dc>,
    objects: Vec<Option<Obj>>,
    /// Device → output: (sx, sy, dx, dy).
    out: (f64, f64, f64, f64),
    cmds: Vec<Command>,
    images: BTreeMap<String, Vec<u8>>,
    path: Option<Vec<PathSeg>>,
}

impl Player<'_> {
    /// Logical → output point.
    fn pt(&self, x: f64, y: f64) -> (f32, f32) {
        let w = &self.dc.world;
        let (x, y) = (w[0] * x + w[2] * y + w[4], w[1] * x + w[3] * y + w[5]);
        let (sx, sy) = match (self.dc.win_ext, self.dc.vp_ext) {
            (Some(we), Some(ve)) if we.0 != 0.0 && we.1 != 0.0 => (ve.0 / we.0, ve.1 / we.1),
            _ => (1.0, 1.0),
        };
        let dx = (x - self.dc.win_org.0) * sx + self.dc.vp_org.0;
        let dy = (y - self.dc.win_org.1) * sy + self.dc.vp_org.1;
        let (ox, oy, tx, ty) = self.out;
        ((dx * ox + tx) as f32, (dy * oy + ty) as f32)
    }

    /// Length scale logical → output (for pen widths and font sizes).
    fn scale(&self) -> f64 {
        let w = &self.dc.world;
        let (sx, sy) = match (self.dc.win_ext, self.dc.vp_ext) {
            (Some(we), Some(ve)) if we.0 != 0.0 && we.1 != 0.0 => (ve.0 / we.0, ve.1 / we.1),
            _ => (1.0, 1.0),
        };
        let k = ((w[0] * w[3] - w[1] * w[2]).abs()).sqrt();
        k * ((sx * self.out.0).abs() + (sy * self.out.1).abs()) / 2.0
    }

    fn stroke(&self) -> Option<Stroke> {
        let (color, width, null) = self.dc.pen;
        (!null).then(|| Stroke {
            width: ((width * self.scale()) as f32).max(0.5),
            color,
            dash: Vec::new(),
            cap: "round".into(),
            join: "round".into(),
        })
    }

    fn fill(&self) -> Option<Paint> {
        self.dc.brush.map(|color| Paint::Solid { color })
    }

    /// Emit (or add to the open path) a shape.
    fn shape(&mut self, segs: Vec<PathSeg>, fill: bool, stroke: bool) {
        if segs.is_empty() {
            return;
        }
        if let Some(p) = self.path.as_mut() {
            p.extend(segs);
            return;
        }
        let fill = if fill { self.fill() } else { None };
        let stroke = if stroke { self.stroke() } else { None };
        if fill.is_some() || stroke.is_some() {
            self.cmds.push(Command::Path { segs, fill, stroke });
        }
    }

    fn poly(&mut self, pts: &[(f64, f64)], close: bool) {
        let mut segs = Vec::with_capacity(pts.len() + 1);
        for (i, &(x, y)) in pts.iter().enumerate() {
            let (x, y) = self.pt(x, y);
            segs.push(if i == 0 {
                PathSeg::M(x, y)
            } else {
                PathSeg::L(x, y)
            });
        }
        if close {
            segs.push(PathSeg::Z);
        }
        self.shape(segs, close, true);
    }

    fn bezier(&mut self, pts: &[(f64, f64)], from_current: bool) {
        let mut segs = Vec::new();
        let mut rest = pts;
        if from_current {
            let (x, y) = self.pt(self.dc.pos.0, self.dc.pos.1);
            segs.push(PathSeg::M(x, y));
        } else if let Some((&(x, y), r)) = pts.split_first() {
            let (x, y) = self.pt(x, y);
            segs.push(PathSeg::M(x, y));
            rest = r;
        }
        for c in rest.chunks_exact(3) {
            let (a, b) = self.pt(c[0].0, c[0].1);
            let (cc, d) = self.pt(c[1].0, c[1].1);
            let (e, f) = self.pt(c[2].0, c[2].1);
            segs.push(PathSeg::C(a, b, cc, d, e, f));
        }
        if let Some(&p) = pts.last() {
            self.dc.pos = p;
        }
        self.shape(segs, false, true);
    }

    fn rect(&mut self, l: f64, t: f64, r: f64, b: f64) {
        self.poly(&[(l, t), (r, t), (r, b), (l, b)], true);
    }

    fn ellipse(&mut self, l: f64, t: f64, r: f64, b: f64) {
        // Four cubic arcs (κ = 0.5523).
        let (cx, cy, rx, ry) = (f64::midpoint(l, r), f64::midpoint(t, b), (r - l) / 2.0, (b - t) / 2.0);
        let k = 0.552_284_8;
        let p = |x: f64, y: f64| self.pt(x, y);
        let (x0, y0) = p(cx + rx, cy);
        let mut segs = vec![PathSeg::M(x0, y0)];
        let quads = [
            (
                (cx + rx, cy + ry * k),
                (cx + rx * k, cy + ry),
                (cx, cy + ry),
            ),
            (
                (cx - rx * k, cy + ry),
                (cx - rx, cy + ry * k),
                (cx - rx, cy),
            ),
            (
                (cx - rx, cy - ry * k),
                (cx - rx * k, cy - ry),
                (cx, cy - ry),
            ),
            (
                (cx + rx * k, cy - ry),
                (cx + rx, cy - ry * k),
                (cx + rx, cy),
            ),
        ];
        for (a, b, c) in quads {
            let (a, b, c) = (p(a.0, a.1), p(b.0, b.1), p(c.0, c.1));
            segs.push(PathSeg::C(a.0, a.1, b.0, b.1, c.0, c.1));
        }
        segs.push(PathSeg::Z);
        self.shape(segs, true, true);
    }

    fn round_rect(&mut self, l: f64, t: f64, r: f64, b: f64, w: f64, h: f64) {
        let (rx, ry) = (
            (w / 2.0).min((r - l).abs() / 2.0),
            (h / 2.0).min((b - t).abs() / 2.0),
        );
        let k = 1.0 - 0.552_284_8;
        let p = |x: f64, y: f64| self.pt(x, y);
        let pts = [
            p(l + rx, t),
            p(r - rx, t),
            p(r - rx * k, t),
            p(r, t + ry * k),
            p(r, t + ry),
            p(r, b - ry),
            p(r, b - ry * k),
            p(r - rx * k, b),
            p(r - rx, b),
            p(l + rx, b),
            p(l + rx * k, b),
            p(l, b - ry * k),
            p(l, b - ry),
            p(l, t + ry),
            p(l, t + ry * k),
            p(l + rx * k, t),
            p(l + rx, t),
        ];
        let mut segs = vec![
            PathSeg::M(pts[0].0, pts[0].1),
            PathSeg::L(pts[1].0, pts[1].1),
        ];
        for i in [2, 7, 12] {
            let (a, b, c) = (pts[i], pts[i + 1], pts[i + 2]);
            segs.push(PathSeg::C(a.0, a.1, b.0, b.1, c.0, c.1));
            let n = pts[i + 3];
            segs.push(PathSeg::L(n.0, n.1));
        }
        let (a, b, c) = (pts[14], pts[15], pts[16]);
        segs.push(PathSeg::C(a.0, a.1, b.0, b.1, c.0, c.1));
        segs.push(PathSeg::Z);
        self.shape(segs, true, true);
    }

    fn line_to(&mut self, x: f64, y: f64) {
        let (a, b) = self.pt(self.dc.pos.0, self.dc.pos.1);
        let (c, d) = self.pt(x, y);
        self.dc.pos = (x, y);
        if let Some(p) = self.path.as_mut() {
            if p.is_empty() {
                p.push(PathSeg::M(a, b));
            }
            p.push(PathSeg::L(c, d));
            return;
        }
        self.shape(vec![PathSeg::M(a, b), PathSeg::L(c, d)], false, true);
    }

    fn move_to(&mut self, x: f64, y: f64) {
        self.dc.pos = (x, y);
        let (a, b) = self.pt(x, y);
        if let Some(p) = self.path.as_mut() {
            p.push(PathSeg::M(a, b));
        }
    }

    fn select(&mut self, idx: u32) {
        // Stock objects ([MS-EMF] §2.1.31 StockObject).
        if idx & 0x8000_0000 != 0 {
            match idx & 0x7FFF_FFFF {
                0 => self.dc.brush = Some(Rgba(255, 255, 255, 255)),
                1 => self.dc.brush = Some(Rgba(192, 192, 192, 255)),
                2 => self.dc.brush = Some(Rgba(128, 128, 128, 255)),
                3 => self.dc.brush = Some(Rgba(64, 64, 64, 255)),
                4 => self.dc.brush = Some(Rgba(0, 0, 0, 255)),
                5 => self.dc.brush = None,
                6 => self.dc.pen = (Rgba(255, 255, 255, 255), 0.0, false),
                7 => self.dc.pen = (Rgba(0, 0, 0, 255), 0.0, false),
                8 => self.dc.pen.2 = true,
                _ => {}
            }
            return;
        }
        match self.objects.get(idx as usize).cloned().flatten() {
            Some(Obj::Pen { color, width, null }) => self.dc.pen = (color, width, null),
            Some(Obj::Brush(b)) => self.dc.brush = b,
            Some(Obj::Font {
                height,
                bold,
                italic,
                face,
            }) => self.dc.font = (height, bold, italic, face),
            _ => {}
        }
    }

    fn put_object(&mut self, idx: Option<usize>, o: Obj) {
        let i = idx.unwrap_or_else(|| {
            self.objects
                .iter()
                .position(Option::is_none)
                .unwrap_or(self.objects.len())
        });
        if i > 65_535 {
            return;
        }
        if self.objects.len() <= i {
            self.objects.resize(i + 1, None);
        }
        self.objects[i] = Some(o);
    }

    fn text(&mut self, x: f64, y: f64, s: &str) {
        let s: String = s.chars().filter(|c| !c.is_control()).collect();
        if s.trim().is_empty() {
            return;
        }
        let (h, bold, italic, face) = self.dc.font.clone();
        let size = (h.abs() * self.scale()) as f32 * if h > 0.0 { 0.8 } else { 1.0 };
        if size <= 0.1 {
            return;
        }
        let Some(fid) = self.fonts.resolve(&face, bold, italic) else {
            return;
        };
        let info = self.fonts.info(fid).clone();
        let upem = f32::from(info.units_per_em);
        let k = size / upem;
        let shaped = self.fonts.shape(fid, &s);
        let width: f32 = shaped.iter().map(|g| g.x_advance as f32 * k).sum();
        let (mut ox, mut oy) = self.pt(x, y);
        // TA_RIGHT 2, TA_CENTER 6, TA_BOTTOM 8, TA_BASELINE 24 ([MS-WMF] §2.1.2.3).
        match self.dc.text_align & 6 {
            6 => ox -= width / 2.0,
            2 => ox -= width,
            _ => {}
        }
        match self.dc.text_align & 0x18 {
            0x18 => {}
            0x08 => oy += f32::from(info.descender) * k,
            _ => oy += f32::from(info.ascender) * k,
        }
        let mut gx = 0.0;
        let glyphs = shaped
            .iter()
            .map(|g| {
                let r = Glyph(
                    g.glyph_id,
                    gx + g.x_offset as f32 * k,
                    -(g.y_offset as f32) * k,
                    g.cluster,
                );
                gx += g.x_advance as f32 * k;
                r
            })
            .collect();
        self.cmds.push(Command::Glyphs {
            face: fid.0,
            size,
            x: ox,
            y: oy,
            color: self.dc.text_color,
            text: s,
            glyphs,
        });
    }

    /// A packed DIB drawn into the logical rectangle (`x`, `y`, `w`, `h`).
    fn dib(&mut self, dib: &[u8], x: f64, y: f64, w: f64, h: f64) {
        if dib.len() < 12 {
            return;
        }
        // Wrap in a BITMAPFILEHEADER so that the raster backend can decode it.
        let hs = u32le(dib, 0).unwrap_or(40) as usize;
        let bpp = u16le(dib, if hs == 12 { 10 } else { 14 }).unwrap_or(24);
        let used = if hs == 12 {
            0
        } else {
            u32le(dib, 32).unwrap_or(0) as usize
        };
        let comp = if hs == 12 {
            0
        } else {
            u32le(dib, 16).unwrap_or(0)
        };
        let colors = if bpp <= 8 {
            if used > 0 {
                used
            } else {
                1 << bpp
            }
        } else {
            used
        };
        let masks = if comp == 3 && hs == 40 { 12 } else { 0 };
        let off = 14 + hs + masks + colors * if hs == 12 { 3 } else { 4 };
        let mut bmp = b"BM".to_vec();
        bmp.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
        bmp.extend_from_slice(&[0; 4]);
        bmp.extend_from_slice(&(off as u32).to_le_bytes());
        bmp.extend_from_slice(dib);
        let id = format!("metafile-dib-{}", self.images.len() + 1);
        self.images.insert(id.clone(), bmp);
        let (x0, y0) = self.pt(x, y);
        let (x1, y1) = self.pt(x + w, y + h);
        self.cmds.push(Command::Image {
            x: x0.min(x1),
            y: y0.min(y1),
            w: (x1 - x0).abs(),
            h: (y1 - y0).abs(),
            image: id,
            crop: None,
            rotation: 0.0,
        });
    }

    fn end_path(&mut self, fill: bool, stroke: bool) {
        let Some(segs) = self.path.take() else { return };
        self.shape(segs, fill, stroke);
    }
}

fn finish(p: Player<'_>, width: f32, height: f32) -> Converted {
    Converted {
        page: PageList {
            width,
            height,
            commands: p.cmds,
        },
        images: p.images,
    }
}

/// Convert an EMF into a display list fitted in `width × height` points.
pub fn emf_to_display_list(
    b: &[u8],
    width: f32,
    height: f32,
    fonts: &mut FontManager,
) -> Option<Converted> {
    if !is_emf(b) {
        return None;
    }
    // §2.3.4.2 EMR_HEADER: Bounds (device units, inclusive).
    let (l, t, r, bt) = (
        f64::from(i32le(b, 8)?),
        f64::from(i32le(b, 12)?),
        f64::from(i32le(b, 16)?),
        f64::from(i32le(b, 20)?),
    );
    let (bw, bh) = ((r - l + 1.0).max(1.0), (bt - t + 1.0).max(1.0));
    let (sx, sy) = (f64::from(width) / bw, f64::from(height) / bh);
    let mut p = Player {
        fonts,
        dc: Dc::default(),
        stack: Vec::new(),
        objects: Vec::new(),
        out: (sx, sy, -l * sx, -t * sy),
        cmds: Vec::new(),
        images: BTreeMap::new(),
        path: None,
    };
    let mut i = 0usize;
    let mut guard = 0u32;
    while i + 8 <= b.len() && guard < 1_000_000 {
        guard += 1;
        let ty = u32le(b, i)?;
        let size = u32le(b, i + 8 - 4)? as usize;
        if size < 8 || i + size > b.len() {
            break;
        }
        let rec = &b[i..i + size];
        let g = |o: usize| f64::from(i32le(rec, o).unwrap_or(0));
        let pts32 = |at: usize, n: usize| -> Vec<(f64, f64)> {
            (0..n).map(|k| (g(at + k * 8), g(at + k * 8 + 4))).collect()
        };
        let pts16 = |at: usize, n: usize| -> Vec<(f64, f64)> {
            (0..n)
                .map(|k| {
                    (
                        f64::from(i16le(rec, at + k * 4).unwrap_or(0)),
                        f64::from(i16le(rec, at + k * 4 + 2).unwrap_or(0)),
                    )
                })
                .collect()
        };
        let count = |at: usize, each: usize| -> usize {
            (u32le(rec, at).unwrap_or(0) as usize).min(size.saturating_sub(at + 4) / each)
        };
        match ty {
            14 => break, // EMR_EOF
            // EMR_POLYBEZIER / POLYGON / POLYLINE / POLYBEZIERTO / POLYLINETO
            2..=6 => {
                let pts = pts32(28, count(24, 8));
                match ty {
                    2 => p.bezier(&pts, false),
                    3 => p.poly(&pts, true),
                    4 => p.poly(&pts, false),
                    5 => p.bezier(&pts, true),
                    _ => {
                        for (x, y) in pts {
                            p.line_to(x, y);
                        }
                    }
                }
            }
            // 16-bit variants
            85..=89 => {
                let pts = pts16(28, count(24, 4));
                match ty {
                    85 => p.bezier(&pts, false),
                    86 => p.poly(&pts, true),
                    87 => p.poly(&pts, false),
                    88 => p.bezier(&pts, true),
                    _ => {
                        for (x, y) in pts {
                            p.line_to(x, y);
                        }
                    }
                }
            }
            // EMR_POLYPOLYLINE(16) / EMR_POLYPOLYGON(16)
            7 | 8 | 90 | 91 => {
                let n = (u32le(rec, 24).unwrap_or(0) as usize).min(size / 4);
                let mut at = 32 + n * 4;
                let each = if ty >= 90 { 4 } else { 8 };
                let close = matches!(ty, 8 | 91);
                let mut segs = Vec::new();
                for k in 0..n {
                    let c = (u32le(rec, 32 + k * 4).unwrap_or(0) as usize).min(size / each);
                    let pts = if each == 4 {
                        pts16(at, c)
                    } else {
                        pts32(at, c)
                    };
                    at += c * each;
                    for (j, (x, y)) in pts.into_iter().enumerate() {
                        let (x, y) = p.pt(x, y);
                        segs.push(if j == 0 {
                            PathSeg::M(x, y)
                        } else {
                            PathSeg::L(x, y)
                        });
                    }
                    if close {
                        segs.push(PathSeg::Z);
                    }
                }
                p.shape(segs, close, true);
            }
            9 => p.dc.win_ext = Some((g(8), g(12))),
            10 => p.dc.win_org = (g(8), g(12)),
            11 => p.dc.vp_ext = Some((g(8), g(12))),
            12 => p.dc.vp_org = (g(8), g(12)),
            22 => p.dc.text_align = u32le(rec, 8).unwrap_or(0),
            24 => p.dc.text_color = colorref(u32le(rec, 8).unwrap_or(0)),
            27 => p.move_to(g(8), g(12)),
            54 => p.line_to(g(8), g(12)),
            33 => p.stack.push(p.dc.clone()),
            34 => {
                let rel = i32le(rec, 8).unwrap_or(-1);
                let n = rel.unsigned_abs() as usize;
                if n > 0 && n <= p.stack.len() {
                    let keep = p.stack.len() - n;
                    p.dc = p.stack[keep].clone();
                    p.stack.truncate(keep);
                }
            }
            // EMR_SETWORLDTRANSFORM / EMR_MODIFYWORLDTRANSFORM (§2.2.28 XFORM)
            35 | 36 => {
                let f = |o: usize| f64::from(f32le(rec, o).unwrap_or(0.0));
                let m = [f(8), f(12), f(16), f(20), f(24), f(28)];
                let w = p.dc.world;
                let mul = |a: [f64; 6], b: [f64; 6]| {
                    [
                        a[0] * b[0] + a[1] * b[2],
                        a[0] * b[1] + a[1] * b[3],
                        a[2] * b[0] + a[3] * b[2],
                        a[2] * b[1] + a[3] * b[3],
                        a[4] * b[0] + a[5] * b[2] + b[4],
                        a[4] * b[1] + a[5] * b[3] + b[5],
                    ]
                };
                p.dc.world = if ty == 35 {
                    m
                } else {
                    match u32le(rec, 32).unwrap_or(0) {
                        1 => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                        2 => mul(m, w),
                        3 => mul(w, m),
                        4 => m,
                        _ => w,
                    }
                };
            }
            37 => p.select(u32le(rec, 8).unwrap_or(0)),
            40 => {
                if let Some(o) = p.objects.get_mut(u32le(rec, 8).unwrap_or(0) as usize) {
                    *o = None;
                }
            }
            // EMR_CREATEPEN (§2.2.19 LogPen)
            38 => {
                let style = u32le(rec, 12).unwrap_or(0);
                p.put_object(
                    Some(u32le(rec, 8).unwrap_or(0) as usize),
                    Obj::Pen {
                        color: colorref(u32le(rec, 24).unwrap_or(0)),
                        width: g(16),
                        null: style & 0xF == 5,
                    },
                );
            }
            // EMR_EXTCREATEPEN (§2.2.20 LogPenEx)
            95 => {
                let style = u32le(rec, 28).unwrap_or(0);
                p.put_object(
                    Some(u32le(rec, 8).unwrap_or(0) as usize),
                    Obj::Pen {
                        color: colorref(u32le(rec, 40).unwrap_or(0)),
                        width: f64::from(u32le(rec, 32).unwrap_or(0)),
                        null: style & 0xF == 5,
                    },
                );
            }
            // EMR_CREATEBRUSHINDIRECT (§2.2.12 LogBrushEx)
            39 => {
                let style = u32le(rec, 12).unwrap_or(0);
                p.put_object(
                    Some(u32le(rec, 8).unwrap_or(0) as usize),
                    Obj::Brush((style != 1).then(|| colorref(u32le(rec, 16).unwrap_or(0)))),
                );
            }
            // EMR_EXTCREATEFONTINDIRECTW (§2.2.13 LogFont)
            82 => {
                let face: String = char::decode_utf16(
                    (0..32).map_while(|k| u16le(rec, 40 + k * 2).filter(|c| *c != 0)),
                )
                .filter_map(Result::ok)
                .collect();
                p.put_object(
                    Some(u32le(rec, 8).unwrap_or(0) as usize),
                    Obj::Font {
                        height: g(12),
                        bold: i32le(rec, 28).unwrap_or(400) >= 600,
                        italic: rec.get(32).is_some_and(|v| *v != 0),
                        face: if face.is_empty() {
                            "Arial".into()
                        } else {
                            face
                        },
                    },
                );
            }
            42 => p.ellipse(g(8), g(12), g(16), g(20)),
            43 => p.rect(g(8), g(12), g(16), g(20)),
            44 => p.round_rect(g(8), g(12), g(16), g(20), g(24), g(28)),
            59 => p.path = Some(Vec::new()),
            // 60 EMR_ENDPATH: nothing to do, the path is used by the next fill/stroke record.
            61 => {
                if let Some(path) = p.path.as_mut() {
                    path.push(PathSeg::Z);
                }
            }
            62 => p.end_path(true, false),
            63 => p.end_path(true, true),
            64 => p.end_path(false, true),
            // EMR_EXTTEXTOUTW (§2.3.5.8, EmrText §2.2.5)
            84 => {
                let n = (u32le(rec, 44).unwrap_or(0) as usize).min(4096);
                let off = u32le(rec, 48).unwrap_or(0) as usize;
                let s: String = char::decode_utf16((0..n).filter_map(|k| u16le(rec, off + k * 2)))
                    .filter_map(Result::ok)
                    .collect();
                p.text(g(36), g(40), &s);
            }
            // EMR_BITBLT (76), EMR_STRETCHBLT (77), EMR_STRETCHDIBITS (81)
            76 | 77 | 81 => {
                let (x, y, w, h, bmi, cbmi, bits, cbits) = if ty == 81 {
                    (g(24), g(28), g(72), g(76), 48, 52, 56, 60)
                } else {
                    (g(24), g(28), g(32), g(36), 84, 88, 92, 96)
                };
                let r32 = |o: usize| u32le(rec, o).unwrap_or(0) as usize;
                let (ob, cb, ox, cx) = (r32(bmi), r32(cbmi), r32(bits), r32(cbits));
                if cb > 0 && cx > 0 {
                    if let (Some(hdr), Some(px)) = (rec.get(ob..ob + cb), rec.get(ox..ox + cx)) {
                        let mut dib = hdr.to_vec();
                        dib.extend_from_slice(px);
                        p.dib(&dib, x, y, w, h);
                    }
                }
            }
            _ => {}
        }
        i += size;
    }
    Some(finish(p, width, height))
}

/// Convert a WMF into a display list fitted in `width × height` points.
pub fn wmf_to_display_list(
    b: &[u8],
    width: f32,
    height: f32,
    fonts: &mut FontManager,
) -> Option<Converted> {
    if !is_wmf(b) {
        return None;
    }
    // §2.3.2.3 placeable header: bounding box in logical units.
    let (mut i, bbox) = if u32le(b, 0) == Some(0x9AC6_CDD7) {
        let r = |o: usize| f64::from(i16le(b, o).unwrap_or(0));
        (22usize, Some((r(6), r(8), r(10), r(12))))
    } else {
        (0, None)
    };
    let header_words = usize::from(u16le(b, i + 2)?);
    i += header_words * 2;
    // Logical box: the placeable bounds, else the first SETWINDOWORG/EXT pair.
    let mut org = bbox.map(|(l, t, _, _)| (l, t));
    let mut ext = bbox.map(|(l, t, r, bb)| (r - l, bb - t));
    {
        let mut j = i;
        while j + 6 <= b.len() && (org.is_none() || ext.is_none()) {
            let size = u32le(b, j)? as usize * 2;
            let f = u16le(b, j + 4)?;
            if size < 6 {
                break;
            }
            let y = f64::from(i16le(b, j + 6).unwrap_or(0));
            let x = f64::from(i16le(b, j + 8).unwrap_or(0));
            match f {
                0x020B if org.is_none() => org = Some((x, y)),
                0x020C if ext.is_none() => ext = Some((x, y)),
                0 => break,
                _ => {}
            }
            j += size;
        }
    }
    let (ox, oy) = org.unwrap_or((0.0, 0.0));
    let (ew, eh) = ext.unwrap_or((f64::from(width), f64::from(height)));
    let (sx, sy) = (
        f64::from(width) / if ew == 0.0 { 1.0 } else { ew },
        f64::from(height) / if eh == 0.0 { 1.0 } else { eh },
    );
    let mut p = Player {
        fonts,
        dc: Dc::default(),
        stack: Vec::new(),
        objects: Vec::new(),
        // Logical units map straight to the output box.
        out: (sx, sy, -ox * sx, -oy * sy),
        cmds: Vec::new(),
        images: BTreeMap::new(),
        path: None,
    };
    let mut guard = 0u32;
    while i + 6 <= b.len() && guard < 1_000_000 {
        guard += 1;
        let size = u32le(b, i)? as usize * 2;
        let f = u16le(b, i + 4)?;
        if size < 6 || i + size > b.len() {
            break;
        }
        let rec = &b[i..i + size];
        let w = |k: usize| f64::from(i16le(rec, 6 + k * 2).unwrap_or(0));
        let pts = |at: usize, n: usize| -> Vec<(f64, f64)> {
            (0..n)
                .map(|k| {
                    (
                        f64::from(i16le(rec, at + k * 4).unwrap_or(0)),
                        f64::from(i16le(rec, at + k * 4 + 2).unwrap_or(0)),
                    )
                })
                .collect()
        };
        let n_at = |at: usize| usize::from(u16le(rec, at).unwrap_or(0)).min(size / 4);
        match f {
            0x0000 => break,
            // 0x020B / 0x020C (window org/ext) fall through to the default arm: they are
            // already folded into the output mapping.
            0x012E => p.dc.text_align = u32::from(u16le(rec, 6).unwrap_or(0)),
            0x0209 => p.dc.text_color = colorref(u32le(rec, 6).unwrap_or(0)),
            0x0214 => p.move_to(w(1), w(0)),
            0x0213 => p.line_to(w(1), w(0)),
            0x041B => p.rect(w(3), w(2), w(1), w(0)),
            0x0418 => p.ellipse(w(3), w(2), w(1), w(0)),
            0x061C => p.round_rect(w(5), w(4), w(3), w(2), w(1), w(0)),
            0x0324 => p.poly(&pts(8, n_at(6)), true),
            0x0325 => p.poly(&pts(8, n_at(6)), false),
            // META_POLYPOLYGON (§2.3.3.16)
            0x0538 => {
                let n = n_at(6);
                let mut at = 8 + n * 2;
                let mut segs = Vec::new();
                for k in 0..n {
                    let c = n_at(8 + k * 2);
                    for (j, (x, y)) in pts(at, c).into_iter().enumerate() {
                        let (x, y) = p.pt(x, y);
                        segs.push(if j == 0 {
                            PathSeg::M(x, y)
                        } else {
                            PathSeg::L(x, y)
                        });
                    }
                    segs.push(PathSeg::Z);
                    at += c * 4;
                }
                p.shape(segs, true, true);
            }
            0x001E => p.stack.push(p.dc.clone()),
            0x0127 => {
                if let Some(d) = p.stack.pop() {
                    p.dc = d;
                }
            }
            // META_CREATEPENINDIRECT (§2.2.1.4 Pen)
            0x02FA => p.put_object(
                None,
                Obj::Pen {
                    color: colorref(u32le(rec, 12).unwrap_or(0)),
                    width: w(1),
                    null: u16le(rec, 6).unwrap_or(0) & 0xF == 5,
                },
            ),
            // META_CREATEBRUSHINDIRECT (§2.2.1.1 LogBrush)
            0x02FC => {
                let style = u16le(rec, 6).unwrap_or(0);
                p.put_object(
                    None,
                    Obj::Brush((style != 1).then(|| colorref(u32le(rec, 8).unwrap_or(0)))),
                );
            }
            // META_CREATEFONTINDIRECT (§2.2.1.2 Font)
            0x02FB => {
                let face: String = (0..32)
                    .map_while(|k| rec.get(24 + k).copied().filter(|c| *c != 0))
                    .map(char::from)
                    .collect();
                p.put_object(
                    None,
                    Obj::Font {
                        height: w(0),
                        bold: i16le(rec, 14).unwrap_or(400) >= 600,
                        italic: rec.get(16).is_some_and(|v| *v != 0),
                        face: if face.is_empty() {
                            "Arial".into()
                        } else {
                            face
                        },
                    },
                );
            }
            // Other object-creating records take a slot of the object table (§3.1.4.1).
            0x00F7 | 0x01F9 | 0x06FF | 0x0142 => p.put_object(None, Obj::Other),
            0x012D => {
                let k = u16le(rec, 6).unwrap_or(0);
                p.select(u32::from(k));
            }
            0x01F0 => {
                if let Some(o) = p.objects.get_mut(usize::from(u16le(rec, 6).unwrap_or(0))) {
                    *o = None;
                }
            }
            // META_TEXTOUT (§2.3.3.20)
            0x0521 => {
                let n = usize::from(u16le(rec, 6).unwrap_or(0)).min(size);
                let s: String = rec
                    .get(8..8 + n)
                    .unwrap_or(&[])
                    .iter()
                    .map(|c| char::from(*c))
                    .collect();
                let at = 8 + n + n % 2;
                let y = f64::from(i16le(rec, at).unwrap_or(0));
                let x = f64::from(i16le(rec, at + 2).unwrap_or(0));
                p.text(x, y, &s);
            }
            // META_EXTTEXTOUT (§2.3.3.5)
            0x0A32 => {
                let n = usize::from(u16le(rec, 10).unwrap_or(0)).min(size);
                let opts = u16le(rec, 12).unwrap_or(0);
                let at = if opts & 6 != 0 { 22 } else { 14 };
                let s: String = rec
                    .get(at..at + n)
                    .unwrap_or(&[])
                    .iter()
                    .map(|c| char::from(*c))
                    .collect();
                p.text(w(1), w(0), &s);
            }
            // META_STRETCHDIB (§2.3.1.6) / META_DIBSTRETCHBLT (§2.3.1.3)
            0x0F43 => {
                let dib = rec.get(28..).unwrap_or(&[]);
                p.dib(dib, w(10), w(9), w(8), w(7));
            }
            0x0B41 if size > 26 => {
                let dib = rec.get(26..).unwrap_or(&[]);
                p.dib(dib, w(9), w(8), w(7), w(6));
            }
            _ => {}
        }
        i += size;
    }
    Some(finish(p, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(ty: u32, body: &[u8]) -> Vec<u8> {
        let mut v = ty.to_le_bytes().to_vec();
        v.extend_from_slice(&((body.len() + 8) as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn le32(vals: &[i32]) -> Vec<u8> {
        vals.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    pub(crate) fn sample_emf() -> Vec<u8> {
        let mut header = le32(&[0, 0, 99, 99, 0, 0, 2000, 2000]);
        header.extend_from_slice(&0x464D_4520u32.to_le_bytes());
        header.extend_from_slice(&[0; 60]);
        let mut b = rec(1, &header);
        b.extend(rec(39, &le32(&[1, 0, 0x0000_00FF, 0])));
        b.extend(rec(37, &le32(&[1])));
        b.extend(rec(43, &le32(&[10, 10, 50, 50])));
        b.extend(rec(42, &le32(&[50, 50, 90, 90])));
        b.extend(rec(14, &le32(&[0, 0, 0])));
        b
    }

    #[test]
    fn emf_rectangle_and_ellipse() {
        let b = sample_emf();
        assert!(is_emf(&b));
        let mut fm = FontManager::new();
        let c = emf_to_display_list(&b, 100.0, 100.0, &mut fm).unwrap();
        let paths: Vec<_> = c
            .page
            .commands
            .iter()
            .filter(|c| matches!(c, Command::Path { .. }))
            .collect();
        assert_eq!(paths.len(), 2);
        let Command::Path { segs, fill, .. } = paths[0] else {
            unreachable!()
        };
        assert_eq!(segs[0], PathSeg::M(10.0, 10.0));
        assert_eq!(
            fill,
            &Some(Paint::Solid {
                color: Rgba(255, 0, 0, 255)
            })
        );
        assert!(emf_to_display_list(&b[..20], 1.0, 1.0, &mut fm).is_none());
    }

    #[test]
    fn wmf_polygon_with_placeable_header() {
        let mut b = 0x9AC6_CDD7u32.to_le_bytes().to_vec();
        b.extend_from_slice(&[0, 0]);
        for v in [0i16, 0, 200, 100, 1440] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&[0; 6]);
        // META_HEADER: type 1, size 9 words, version 0x0300
        for v in [1u16, 9, 0x0300, 0, 0, 0, 0, 0, 0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        // META_POLYGON with 3 points
        let mut r = Vec::new();
        r.extend_from_slice(&10u32.to_le_bytes());
        r.extend_from_slice(&0x0324u16.to_le_bytes());
        for v in [3i16, 0, 0, 200, 0, 100, 100] {
            r.extend_from_slice(&v.to_le_bytes());
        }
        b.extend(r);
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        assert!(is_wmf(&b));
        let mut fm = FontManager::new();
        let c = wmf_to_display_list(&b, 20.0, 10.0, &mut fm).unwrap();
        let Some(Command::Path { segs, .. }) = c.page.commands.first() else {
            panic!("no path")
        };
        assert_eq!(segs[1], PathSeg::L(20.0, 0.0));
        assert_eq!(segs[2], PathSeg::L(10.0, 10.0));
    }
}
