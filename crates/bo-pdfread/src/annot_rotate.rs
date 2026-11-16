//! Turning a drawn comment (BlueReader round 10, the rotate handle of a selected comment): its
//! points turn about its centre and its keys and appearance are written again. Rectangles and
//! ellipses become polygons (`/Subtype /Polygon`, ISO 32000-1 §12.5.6.9: PDF rectangles and
//! ellipses stay upright); lines, polylines, polygons, ink and measurements keep their kind.
use crate::annot::Annot;
use crate::annot_draw::Shape;
use crate::PdfDocument;
use lopdf::Object;

/// Points of an ellipse drawn as a polygon.
const OVAL_POINTS: usize = 48;

fn turn(p: [f32; 2], c: [f32; 2], (s, k): (f32, f32)) -> [f32; 2] {
    let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
    [c[0] + dx * k - dy * s, c[1] + dx * s + dy * k]
}

/// `shape` turned by `angle` radians (clockwise on the page, y down) about `c`; `None` for
/// shapes that do not turn (text boxes, callouts, stamps).
#[must_use]
pub fn rotated_shape(shape: &Shape, c: [f32; 2], angle: f32) -> Option<Shape> {
    let sc = angle.sin_cos();
    let all = |v: &[[f32; 2]]| v.iter().map(|p| turn(*p, c, sc)).collect::<Vec<_>>();
    Some(match shape {
        Shape::Ink(strokes) => Shape::Ink(strokes.iter().map(|s| all(s)).collect()),
        Shape::Line { a, b, arrow } => Shape::Line {
            a: turn(*a, c, sc),
            b: turn(*b, c, sc),
            arrow: *arrow,
        },
        Shape::Polyline(v) => Shape::Polyline(all(v)),
        Shape::Polygon(v) => Shape::Polygon(all(v)),
        Shape::Perimeter(v) => Shape::Perimeter(all(v)),
        Shape::Area(v) => Shape::Area(all(v)),
        Shape::Distance(a, b) => Shape::Distance(turn(*a, c, sc), turn(*b, c, sc)),
        Shape::Rect(r) => Shape::Polygon(all(&[
            [r[0], r[1]],
            [r[2], r[1]],
            [r[2], r[3]],
            [r[0], r[3]],
        ])),
        Shape::Oval(r) => {
            let (cx, cy) = (r[0].midpoint(r[2]), r[1].midpoint(r[3]));
            let (rx, ry) = ((r[2] - r[0]) / 2.0, (r[3] - r[1]) / 2.0);
            let pts: Vec<[f32; 2]> = (0..OVAL_POINTS)
                .map(|i| {
                    let t = i as f32 / OVAL_POINTS as f32 * std::f32::consts::TAU;
                    [cx + rx * t.cos(), cy + ry * t.sin()]
                })
                .collect();
            Shape::Polygon(all(&pts))
        }
        Shape::Callout { .. } | Shape::Typewriter(_) | Shape::Stamp(..) => return None,
    })
}

impl PdfDocument {
    /// Whether comment `a` can be turned ([`PdfDocument::rotate_drawing`]).
    pub fn can_rotate(&self, a: &Annot) -> bool {
        self.drawing(a)
            .is_some_and(|(s, _, _)| rotated_shape(&s, [0.0, 0.0], 0.1).is_some())
    }

    /// Turn drawn comment `a` by `angle` radians (clockwise on the page) about the middle of
    /// its rectangle, as one undo step.
    pub fn rotate_drawing(&mut self, a: &Annot, angle: f32) -> Result<(), String> {
        let (shape, style, text) = self.drawing(a).ok_or("the comment cannot be turned")?;
        let c = [a.rect[0].midpoint(a.rect[2]), a.rect[1].midpoint(a.rect[3])];
        let turned = rotated_shape(&shape, c, angle).ok_or("the comment cannot be turned")?;
        let id = a.id.ok_or("the comment cannot be changed")?;
        self.begin("rotate comment");
        if matches!(shape, Shape::Rect(_) | Shape::Oval(_)) {
            self.touch(id);
            if let Ok(d) = self.doc.get_dictionary_mut(id) {
                d.set("Subtype", Object::Name(b"Polygon".to_vec()));
                d.remove(b"RD");
            }
        }
        // update_drawing joins the open step and closes it.
        self.update_drawing(a, &turned, &style, &text)
    }
}

#[cfg(test)]
mod tests {
    use crate::{standard_fonts, AnnotKind, PdfDocument, Shape};

    #[test]
    fn rectangles_turn_into_polygons_and_lines_keep_their_kind() {
        let pdf = crate::tests::build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>",
        ]);
        let mut d = PdfDocument::open(pdf, standard_fonts()).expect("opens");
        let style = crate::DrawStyle::default();
        d.add_drawing(
            0,
            &Shape::Rect([100.0, 100.0, 200.0, 150.0]),
            &style,
            "me",
            "",
        )
        .expect("rect");
        let a = d.annotations().remove(0);
        assert!(d.can_rotate(&a));
        d.rotate_drawing(&a, std::f32::consts::FRAC_PI_2)
            .expect("turns");
        let a = d.annotations().remove(0);
        assert_eq!(a.kind, AnnotKind::Line);
        // A 100 × 50 box turned a quarter is 50 × 100 about the same centre.
        let (w, h) = (a.rect[2] - a.rect[0], a.rect[3] - a.rect[1]);
        assert!((w - h / 2.0).abs() < 4.0, "{:?}", a.rect);
        let Some((Shape::Polygon(v), _, _)) = d.drawing(&a) else {
            panic!("polygon");
        };
        assert_eq!(v.len(), 4);
        assert!(
            (v[0][0] - 175.0).abs() < 0.01 && (v[0][1] - 75.0).abs() < 0.01,
            "{v:?}"
        );
        // One undo step.
        assert!(d.undo());
        assert_eq!(d.annotations()[0].kind, AnnotKind::Square);
        // A typewriter does not turn.
        d.add_drawing(
            0,
            &Shape::Typewriter([10.0, 10.0, 90.0, 30.0]),
            &style,
            "me",
            "Hi",
        )
        .expect("typewriter");
        let t = d
            .annotations()
            .into_iter()
            .find(|a| a.kind == AnnotKind::TextBox)
            .expect("text");
        assert!(!d.can_rotate(&t));
        let saved = d.save().expect("saves");
        assert!(PdfDocument::open(saved, standard_fonts()).is_ok());
    }
}
