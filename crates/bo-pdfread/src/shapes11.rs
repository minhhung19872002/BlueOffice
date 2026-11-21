//! BlueEditor round 11: the shapes of Edit › Add Shapes as the reference editor offers them
//! (Add Path: line, rectangle, rounded rectangle, ellipse, connector, polygon, curve, arrow,
//! corner; Add Shading: the same closed shapes filled with an axial shading), drawn into the
//! page content with stroke and fill colours, line width, dash and opacity (ISO 32000-1 §8.5
//! path construction, §8.4.3.6 dash pattern, §11.6.4.4 constant opacity `/CA` `/ca`, §8.7.4.5.3
//! axial shadings drawn with `sh` inside the path as a clip).
use crate::media_edit::ShapeKind;
use crate::PdfDocument;
use lopdf::{Dictionary, Object};

/// How a shape is painted.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeStyle {
    /// Line colour (`None`: no line).
    pub stroke: Option<[u8; 3]>,
    /// Fill colour (`None`: no fill); with `shading`, the colour the shading starts from.
    pub fill: Option<[u8; 3]>,
    /// Line width in points.
    pub width: f32,
    /// Opacity 0–1 of line and fill.
    pub opacity: f32,
    /// Dash pattern in points (empty: solid).
    pub dash: Vec<f32>,
    /// Fill with an axial shading from `fill` (or grey) on the left to white on the right.
    pub shading: bool,
}

impl Default for ShapeStyle {
    fn default() -> Self {
        Self {
            stroke: Some([0, 0, 0]),
            fill: None,
            width: 1.0,
            opacity: 1.0,
            dash: Vec::new(),
            shading: false,
        }
    }
}

fn f(v: f32) -> String {
    format!("{}", (v * 1000.0).round() / 1000.0)
}

fn rgb(c: [u8; 3]) -> String {
    let g = |v: u8| f(f32::from(v) / 255.0);
    format!("{} {} {}", g(c[0]), g(c[1]), g(c[2]))
}

/// Path of the shapes added in round 11 (page points, y down, inside box `r`); `None` for the
/// kinds `PdfDocument::add_shape` draws itself.
#[must_use]
pub(crate) fn shape_path11(kind: ShapeKind, r: [f32; 4]) -> Option<String> {
    let (x0, y0, x1, y1) = (r[0], r[1], r[2], r[3]);
    let (w, h) = (x1 - x0, y1 - y0);
    let at = |u: f32, v: f32| format!("{} {}", f(x0 + w * u), f(y0 + h * v));
    let poly = |pts: &[(f32, f32)], close: bool| {
        let mut s = String::new();
        for (i, (u, v)) in pts.iter().enumerate() {
            s.push_str(&at(*u, *v));
            s.push_str(if i == 0 { " m " } else { " l " });
        }
        if close {
            s.push_str("h ");
        }
        s
    };
    Some(match kind {
        // A step: across the top, down the middle, across the bottom.
        ShapeKind::Connector => poly(&[(0.0, 0.0), (0.5, 0.0), (0.5, 1.0), (1.0, 1.0)], false),
        ShapeKind::Polygon => {
            // A regular pentagon in the box, point up.
            let pts: Vec<(f32, f32)> = (0..5)
                .map(|k| {
                    let a = std::f32::consts::PI * (-0.5 + 0.4 * k as f32);
                    (0.5 + 0.5 * a.cos(), 0.55 + 0.5 * a.sin())
                })
                .map(|(u, v)| (u, v.clamp(0.0, 1.0)))
                .collect();
            poly(&pts, true)
        }
        ShapeKind::Curve => format!(
            "{} m {} {} {} c {} {} {} c ",
            at(0.0, 0.7),
            at(0.35, -0.2),
            at(0.55, 0.3),
            at(0.5, 0.5),
            at(0.45, 0.75),
            at(0.7, 1.2),
            at(1.0, 0.3)
        ),
        // An arrow pointing up: head over a stem.
        ShapeKind::Arrow => poly(
            &[
                (0.5, 0.0),
                (1.0, 0.5),
                (0.68, 0.5),
                (0.68, 1.0),
                (0.32, 1.0),
                (0.32, 0.5),
                (0.0, 0.5),
            ],
            true,
        ),
        // A corner: an L of two bars.
        ShapeKind::Corner => poly(
            &[
                (0.0, 0.0),
                (0.4, 0.0),
                (0.4, 0.6),
                (1.0, 0.6),
                (1.0, 1.0),
                (0.0, 1.0),
            ],
            true,
        ),
        _ => return None,
    })
}

impl PdfDocument {
    /// Draw shape `kind` in box `rect` of page `index` (page points, y down) painted as
    /// `style` (one undo step).
    ///
    /// # Errors
    /// When the shape has neither line nor fill, or the page cannot be written.
    #[allow(clippy::too_many_lines)]
    pub fn add_shape11(
        &mut self,
        index: usize,
        kind: ShapeKind,
        rect: [f32; 4],
        style: &ShapeStyle,
    ) -> Result<(), String> {
        let open = matches!(
            kind,
            ShapeKind::Line | ShapeKind::Connector | ShapeKind::Curve
        );
        let fills = !open && (style.fill.is_some() || style.shading);
        if style.stroke.is_none() && !fills {
            return Err("a shape needs a line or a fill".into());
        }
        self.begin("add shape");
        let r = (|| {
            let (_, inv) = self.bases(index)?;
            let (sid, content) = self.page_stream(index)?;
            let r = if kind == ShapeKind::Line {
                rect
            } else {
                [
                    rect[0].min(rect[2]),
                    rect[1].min(rect[3]),
                    rect[0].max(rect[2]),
                    rect[1].max(rect[3]),
                ]
            };
            let path = crate::media_edit::base_path(kind, r, rect);
            let mut s = format!("\nq {} cm ", crate::objects::matrix(inv));
            if style.opacity < 0.999 {
                let mut gs = Dictionary::new();
                gs.set("Type", Object::Name(b"ExtGState".to_vec()));
                gs.set("CA", Object::Real(style.opacity.clamp(0.0, 1.0)));
                gs.set("ca", Object::Real(style.opacity.clamp(0.0, 1.0)));
                let id = self.add(Object::Dictionary(gs));
                let name = self.add_resource(index, b"ExtGState", "BoGS", id)?;
                s.push_str(&format!("/{} gs ", String::from_utf8_lossy(&name)));
            }
            if fills && style.shading {
                // The shading inside the path (left to right: the fill colour to white).
                let from = style.fill.unwrap_or([0x60, 0x60, 0x60]);
                let mut func = Dictionary::new();
                func.set("FunctionType", 2);
                func.set("Domain", vec![0.into(), 1.into()]);
                func.set(
                    "C0",
                    from.iter()
                        .map(|v| Object::Real(f32::from(*v) / 255.0))
                        .collect::<Vec<_>>(),
                );
                func.set("C1", vec![1.into(), 1.into(), 1.into()]);
                func.set("N", 1);
                let mut sh = Dictionary::new();
                sh.set("ShadingType", 2);
                sh.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
                sh.set(
                    "Coords",
                    vec![
                        Object::Real(r[0]),
                        Object::Real(r[1]),
                        Object::Real(r[2]),
                        Object::Real(r[1]),
                    ],
                );
                sh.set("Function", func);
                sh.set("Extend", vec![true.into(), true.into()]);
                let id = self.add(Object::Dictionary(sh));
                let name = self.add_resource(index, b"Shading", "BoSh", id)?;
                s.push_str(&format!(
                    "q {path}W n /{} sh Q ",
                    String::from_utf8_lossy(&name)
                ));
            }
            if let Some(c) = style.stroke {
                s.push_str(&format!("{} RG {} w ", rgb(c), f(style.width.max(0.1))));
                if !style.dash.is_empty() {
                    let d: Vec<String> = style.dash.iter().map(|v| f(v.max(0.1))).collect();
                    s.push_str(&format!("[{}] 0 d ", d.join(" ")));
                }
            }
            let solid_fill = fills && !style.shading;
            if solid_fill {
                if let Some(c) = style.fill {
                    s.push_str(&format!("{} rg ", rgb(c)));
                }
            }
            let op = match (style.stroke.is_some(), solid_fill) {
                (true, true) => "B",
                (false, true) => "f",
                (true, false) => "S",
                (false, false) => "n",
            };
            s.push_str(&path);
            s.push_str(op);
            s.push_str(" Q\n");
            self.write_page(sid, content, Vec::new(), s.as_bytes());
            Ok(())
        })();
        self.commit();
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::build_pdf;

    fn page() -> PdfDocument {
        let pdf = build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R >>",
            "<< /Length 0 >>\nstream\n\nendstream",
        ]);
        PdfDocument::open(pdf, crate::standard_fonts()).expect("opens")
    }

    #[test]
    fn every_new_shape_has_a_path() {
        for k in [
            ShapeKind::Connector,
            ShapeKind::Polygon,
            ShapeKind::Curve,
            ShapeKind::Arrow,
            ShapeKind::Corner,
        ] {
            let p = shape_path11(k, [10.0, 10.0, 60.0, 50.0]).expect("path");
            assert!(p.contains(" m "), "{k:?}");
        }
        assert!(shape_path11(ShapeKind::Rectangle, [0.0; 4]).is_none());
    }

    #[test]
    fn styled_shapes_are_drawn_and_undone() {
        let mut doc = page();
        let style = ShapeStyle {
            stroke: Some([200, 0, 0]),
            fill: Some([0, 0, 200]),
            width: 2.0,
            opacity: 0.5,
            dash: vec![3.0, 2.0],
            shading: false,
        };
        doc.add_shape11(0, ShapeKind::Polygon, [20.0, 20.0, 120.0, 120.0], &style)
            .expect("drawn");
        let shaded = ShapeStyle {
            shading: true,
            ..style.clone()
        };
        doc.add_shape11(0, ShapeKind::Arrow, [150.0, 20.0, 250.0, 120.0], &shaded)
            .expect("drawn");
        let objs = doc.page_objects(0).expect("objects");
        assert!(objs.len() >= 2, "{}", objs.len());
        assert!(doc.render(0, crate::RenderOptions::default()).is_ok());
        assert!(doc.undo());
        assert!(doc.undo());
        assert_eq!(doc.page_objects(0).expect("objects").len(), 0);
        // Nothing to paint.
        let none = ShapeStyle {
            stroke: None,
            fill: None,
            ..ShapeStyle::default()
        };
        assert!(doc
            .add_shape11(0, ShapeKind::Rectangle, [0.0, 0.0, 10.0, 10.0], &none)
            .is_err());
    }
}
