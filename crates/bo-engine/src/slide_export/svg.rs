//! Display list → SVG 1.1 (W3C Scalable Vector Graphics 1.1, Second Edition).
//!
//! The view box is the page in points; `width` / `height` give the pixel size.
//! Glyphs are drawn as outline paths defined once in `<defs>` and placed with
//! `<use>` (§5.6), so the file needs no fonts; the run text is kept in an
//! invisible `<text>` element for search and copy. Pictures are embedded as
//! `data:` URIs (RFC 2397). Clips (§14.3), transforms (§7.6), group opacity
//! (§14.5), gradients (§13.2) and blur filters (§15.17 feGaussianBlur) map the
//! display list commands one to one.

use bo_render::{Command, ImageSource, PageList, Paint, PathSeg, Rgba, Stroke};
use bo_text::{FaceId, FontManager, PathCmd};
use std::collections::BTreeMap;
use std::fmt::Write as _;

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '&' => o.push_str("&amp;"),
            '"' => o.push_str("&quot;"),
            // XML 1.0 §2.2: control characters other than tab / LF / CR are not allowed.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

/// Shortest decimal form of a coordinate (3 decimals).
fn n(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_owned()
    }
}

fn color(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

/// `fill` / `stroke` opacity attribute when the colour is not opaque.
fn alpha(attr: &str, c: Rgba) -> String {
    if c.3 == 255 {
        String::new()
    } else {
        format!(r#" {attr}="{}""#, n(f32::from(c.3) / 255.0))
    }
}

fn path_data(segs: &[PathSeg]) -> String {
    let mut d = String::new();
    for s in segs {
        let _ = match *s {
            PathSeg::M(x, y) => write!(d, "M{} {}", n(x), n(y)),
            PathSeg::L(x, y) => write!(d, "L{} {}", n(x), n(y)),
            PathSeg::Q(a, b, x, y) => write!(d, "Q{} {} {} {}", n(a), n(b), n(x), n(y)),
            PathSeg::C(a, b, c, e, x, y) => {
                write!(d, "C{} {} {} {} {} {}", n(a), n(b), n(c), n(e), n(x), n(y))
            }
            PathSeg::Z => write!(d, "Z"),
        };
    }
    d
}

fn glyph_data(fonts: &FontManager, face: u32, gid: u16) -> String {
    let mut d = String::new();
    for c in fonts.outline(FaceId(face), gid) {
        let _ = match c {
            PathCmd::MoveTo(x, y) => write!(d, "M{} {}", n(x), n(y)),
            PathCmd::LineTo(x, y) => write!(d, "L{} {}", n(x), n(y)),
            PathCmd::QuadTo(a, b, x, y) => write!(d, "Q{} {} {} {}", n(a), n(b), n(x), n(y)),
            PathCmd::CubicTo(a, b, c2, e, x, y) => {
                write!(d, "C{} {} {} {} {} {}", n(a), n(b), n(c2), n(e), n(x), n(y))
            }
            PathCmd::Close => write!(d, "Z"),
        };
    }
    d
}

fn stroke_attrs(s: &Stroke) -> String {
    let mut a = format!(
        r#" stroke="{}" stroke-width="{}""#,
        color(s.color),
        n(s.width)
    );
    a.push_str(&alpha("stroke-opacity", s.color));
    if matches!(s.cap.as_str(), "round" | "square") {
        let _ = write!(a, r#" stroke-linecap="{}""#, s.cap);
    }
    if matches!(s.join.as_str(), "round" | "bevel") {
        let _ = write!(a, r#" stroke-linejoin="{}""#, s.join);
    }
    if !s.dash.is_empty() {
        let d: Vec<String> = s.dash.iter().map(|v| n(*v)).collect();
        let _ = write!(a, r#" stroke-dasharray="{}""#, d.join(" "));
    }
    a
}

/// `data:` URI of a picture the SVG viewers can show (PNG / JPEG; other
/// formats are re-encoded as PNG).
fn data_uri(bytes: &[u8]) -> Option<String> {
    use base64::Engine as _;
    let (mime, data) = match bo_render::image::sniff(bytes) {
        bo_render::image::Format::Png => ("image/png", bytes.to_vec()),
        bo_render::image::Format::Jpeg => ("image/jpeg", bytes.to_vec()),
        bo_render::image::Format::Other => ("image/png", bo_raster::transcode_to_png(bytes)?),
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(data)
    ))
}

struct Writer<'a> {
    fonts: &'a FontManager,
    images: &'a dyn ImageSource,
    defs: String,
    body: String,
    /// Closing tags of open groups (`PushClip`, `PushTransform`, `PushOpacity`).
    open: Vec<&'static str>,
    next_id: usize,
    glyphs: BTreeMap<(u32, u16), Option<String>>,
    pictures: BTreeMap<String, Option<String>>,
}

impl Writer<'_> {
    fn id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}{}", self.next_id)
    }

    fn paint(&mut self, p: &Paint) -> String {
        match p {
            Paint::Solid { color: c } => {
                format!(r#" fill="{}"{}"#, color(*c), alpha("fill-opacity", *c))
            }
            Paint::Linear {
                x1,
                y1,
                x2,
                y2,
                stops,
            } => {
                let id = self.id("lg");
                let _ = write!(
                    self.defs,
                    r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}">"#,
                    n(*x1),
                    n(*y1),
                    n(*x2),
                    n(*y2)
                );
                self.stops(stops);
                self.defs.push_str("</linearGradient>");
                format!(r#" fill="url(#{id})""#)
            }
            Paint::Radial { cx, cy, r, stops } => {
                let id = self.id("rg");
                let _ = write!(
                    self.defs,
                    r#"<radialGradient id="{id}" gradientUnits="userSpaceOnUse" cx="{}" cy="{}" r="{}">"#,
                    n(*cx),
                    n(*cy),
                    n(*r)
                );
                self.stops(stops);
                self.defs.push_str("</radialGradient>");
                format!(r#" fill="url(#{id})""#)
            }
        }
    }

    fn stops(&mut self, stops: &[bo_render::GradientStop]) {
        for s in stops {
            let _ = write!(
                self.defs,
                r#"<stop offset="{}" stop-color="{}"{}/>"#,
                n(s.offset.clamp(0.0, 1.0)),
                color(s.color),
                alpha("stop-opacity", s.color)
            );
        }
    }

    fn glyph(&mut self, face: u32, gid: u16) -> Option<String> {
        if let Some(id) = self.glyphs.get(&(face, gid)) {
            return id.clone();
        }
        let d = glyph_data(self.fonts, face, gid);
        let id = (!d.is_empty()).then(|| format!("g{face}-{gid}"));
        if let Some(id) = &id {
            let _ = write!(self.defs, r#"<path id="{id}" d="{d}"/>"#);
        }
        self.glyphs.insert((face, gid), id.clone());
        id
    }

    fn picture(&mut self, id: &str) -> Option<String> {
        if let Some(p) = self.pictures.get(id) {
            return p.clone();
        }
        let uri = self.images.image(id).and_then(data_uri);
        self.pictures.insert(id.to_owned(), uri.clone());
        uri
    }

    fn command(&mut self, cmd: &Command) {
        match cmd {
            Command::FillRect {
                x,
                y,
                w,
                h,
                color: c,
            } => {
                let _ = write!(
                    self.body,
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"{}/>"#,
                    n(*x),
                    n(*y),
                    n(*w),
                    n(*h),
                    color(*c),
                    alpha("fill-opacity", *c)
                );
            }
            Command::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                color: c,
                dash,
            } => {
                let dash = dash.map_or_else(String::new, |[on, off]| {
                    format!(r#" stroke-dasharray="{} {}""#, n(on), n(off))
                });
                let _ = write!(
                    self.body,
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"{}{dash}/>"#,
                    n(*x1),
                    n(*y1),
                    n(*x2),
                    n(*y2),
                    color(*c),
                    n(*width),
                    alpha("stroke-opacity", *c)
                );
            }
            Command::Glyphs {
                face,
                size,
                x,
                y,
                color: c,
                text,
                glyphs,
            } => {
                if (*face as usize) >= self.fonts.len() {
                    return;
                }
                let upem = f32::from(self.fonts.info(FaceId(*face)).units_per_em.max(1));
                let s = size / upem;
                let _ = write!(
                    self.body,
                    r#"<g fill="{}"{}>"#,
                    color(*c),
                    alpha("fill-opacity", *c)
                );
                for g in glyphs {
                    if let Some(id) = self.glyph(*face, g.0) {
                        // Font units are y-up: flip with a negative y scale.
                        let _ = write!(
                            self.body,
                            r##"<use xlink:href="#{id}" transform="translate({} {}) scale({} {})"/>"##,
                            n(x + g.1),
                            n(y + g.2),
                            n(s),
                            n(-s)
                        );
                    }
                }
                if !text.trim().is_empty() {
                    let _ = write!(
                        self.body,
                        r#"<text x="{}" y="{}" font-size="{}" fill-opacity="0">{}</text>"#,
                        n(*x),
                        n(*y),
                        n(*size),
                        esc(text)
                    );
                }
                self.body.push_str("</g>");
            }
            Command::PushClip { x, y, w, h } => {
                let id = self.id("c");
                let _ = write!(
                    self.defs,
                    r#"<clipPath id="{id}"><rect x="{}" y="{}" width="{}" height="{}"/></clipPath>"#,
                    n(*x),
                    n(*y),
                    n(*w),
                    n(*h)
                );
                let _ = write!(self.body, r#"<g clip-path="url(#{id})">"#);
                self.open.push("</g>");
            }
            Command::PushClipPath { segs } => {
                let id = self.id("c");
                let _ = write!(
                    self.defs,
                    r#"<clipPath id="{id}"><path d="{}"/></clipPath>"#,
                    path_data(segs)
                );
                let _ = write!(self.body, r#"<g clip-path="url(#{id})">"#);
                self.open.push("</g>");
            }
            Command::PushTransform { m } => {
                let _ = write!(
                    self.body,
                    r#"<g transform="matrix({} {} {} {} {} {})">"#,
                    n(m[0]),
                    n(m[1]),
                    n(m[2]),
                    n(m[3]),
                    n(m[4]),
                    n(m[5])
                );
                self.open.push("</g>");
            }
            Command::PushOpacity { alpha: a } => {
                let _ = write!(self.body, r#"<g opacity="{}">"#, n(a.clamp(0.0, 1.0)));
                self.open.push("</g>");
            }
            Command::PopClip | Command::PopTransform | Command::PopOpacity => {
                if let Some(close) = self.open.pop() {
                    self.body.push_str(close);
                }
            }
            Command::Image {
                x,
                y,
                w,
                h,
                image,
                crop,
                rotation,
            } => {
                let Some(uri) = self.picture(image) else {
                    return;
                };
                let rot = if *rotation == 0.0 {
                    String::new()
                } else {
                    format!(
                        r#" transform="rotate({} {} {})""#,
                        n(*rotation),
                        n(x + w / 2.0),
                        n(y + h / 2.0)
                    )
                };
                let [cl, ct, cr, cb] = crop.unwrap_or([0.0; 4]);
                if cl == 0.0 && ct == 0.0 && cr == 0.0 && cb == 0.0 {
                    let _ = write!(
                        self.body,
                        r#"<image x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none" xlink:href="{uri}"{rot}/>"#,
                        n(*x),
                        n(*y),
                        n(*w),
                        n(*h)
                    );
                } else {
                    // The whole picture scaled so the kept part fills the box, clipped to the box.
                    let fw = w / (1.0 - cl - cr).max(1e-3);
                    let fh = h / (1.0 - ct - cb).max(1e-3);
                    let id = self.id("c");
                    let _ = write!(
                        self.defs,
                        r#"<clipPath id="{id}"><rect x="{}" y="{}" width="{}" height="{}"/></clipPath>"#,
                        n(*x),
                        n(*y),
                        n(*w),
                        n(*h)
                    );
                    let _ = write!(
                        self.body,
                        r#"<g{rot}><g clip-path="url(#{id})"><image x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none" xlink:href="{uri}"/></g></g>"#,
                        n(x - cl * fw),
                        n(y - ct * fh),
                        n(fw),
                        n(fh)
                    );
                }
            }
            Command::Path { segs, fill, stroke } => {
                let fill = fill
                    .as_ref()
                    .map_or_else(|| r#" fill="none""#.to_owned(), |p| self.paint(p));
                let stroke = stroke.as_ref().map(stroke_attrs).unwrap_or_default();
                let _ = write!(
                    self.body,
                    r#"<path d="{}"{fill}{stroke}/>"#,
                    path_data(segs)
                );
            }
            Command::BlurPath {
                segs,
                color: c,
                radius,
                spread,
            } => {
                let id = self.id("b");
                let _ = write!(
                    self.defs,
                    r#"<filter id="{id}" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="{}"/></filter>"#,
                    n(radius / 2.0)
                );
                let grow = if *spread > 0.0 {
                    format!(
                        r#" stroke="{}" stroke-width="{}" stroke-linejoin="round""#,
                        color(*c),
                        n(spread * 2.0)
                    )
                } else {
                    String::new()
                };
                let _ = write!(
                    self.body,
                    r#"<path d="{}" fill="{}"{}{grow} filter="url(#{id})"/>"#,
                    path_data(segs),
                    color(*c),
                    alpha("opacity", *c)
                );
            }
        }
    }
}

/// SVG document of a page, `px_width` pixels wide.
pub fn page_svg(
    page: &PageList,
    fonts: &FontManager,
    images: &dyn ImageSource,
    px_width: u32,
) -> String {
    let mut w = Writer {
        fonts,
        images,
        defs: String::new(),
        body: String::new(),
        open: Vec::new(),
        next_id: 0,
        glyphs: BTreeMap::new(),
        pictures: BTreeMap::new(),
    };
    for c in &page.commands {
        w.command(c);
    }
    while let Some(close) = w.open.pop() {
        w.body.push_str(close);
    }
    let px_h = (px_width as f32 * page.height / page.width.max(1.0)).round();
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            "\n",
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.1" width="{}" height="{}" viewBox="0 0 {} {}">"#,
            r##"<defs>{}</defs><rect width="100%" height="100%" fill="#ffffff"/>{}</svg>"##
        ),
        px_width,
        n(px_h),
        n(page.width),
        n(page.height),
        w.defs,
        w.body
    )
}
