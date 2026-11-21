//! Sticky-note styles (BlueReader round 16): the icon name of a text annotation (ISO 32000-1,
//! §12.5.6.4 Table 172 `/Name`: Comment, Key, Note, Help, NewParagraph, Paragraph, Insert) and
//! its opacity (`/CA`, §12.5.2 Table 168), with an appearance stream drawn for each icon. The
//! icon drawings are our own.
use crate::annot::{nums, pdf_date, text_obj, Annot, AnnotKind};
use crate::PdfDocument;
use lopdf::{Dictionary, Object};
use std::fmt::Write as _;

/// The note icons of the Format pane, in its order (six a row): the standard names of
/// §12.5.6.4 and the extra names viewers commonly write.
pub const NOTE_ICONS: [&str; 15] = [
    "Comment",
    "Cross",
    "Help",
    "Insert",
    "Check",
    "RightArrow",
    "NewParagraph",
    "Star",
    "Circle",
    "Key",
    "RightPointer",
    "UpArrow",
    "Note",
    "Paragraph",
    "UpLeftArrow",
];

/// Every icon name drawn: [`NOTE_ICONS`] and CrossHairs (written by other viewers).
pub const NOTE_ICONS_DRAWN: [&str; 16] = [
    "Comment",
    "Cross",
    "Help",
    "Insert",
    "Check",
    "RightArrow",
    "NewParagraph",
    "Star",
    "Circle",
    "Key",
    "RightPointer",
    "UpArrow",
    "Note",
    "Paragraph",
    "UpLeftArrow",
    "CrossHairs",
];

/// Appearance content of a note icon in `rect` (user space, y up): filled in `rgb`, outlined
/// dark grey, at `alpha` through `/GS0` (the caller adds the ExtGState).
#[must_use]
pub fn note_icon_content(name: &str, rect: [f32; 4], rgb: [f32; 3], alpha: f32) -> String {
    let (x, y) = (rect[0], rect[1]);
    let (w, h) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    // Grid of 20 units, v up from the bottom.
    let p = |u: f32, v: f32| (x + u * w / 20.0, y + v * h / 20.0);
    let [r, g, b] = rgb;
    let mut s = String::new();
    if alpha < 0.999 {
        s.push_str("/GS0 gs\n");
    }
    let _ = writeln!(s, "{r} {g} {b} rg 0.25 0.25 0.25 RG 0.8 w 1 j 1 J");
    let poly = |s: &mut String, pts: &[(f32, f32)], op: &str| {
        for (i, (u, v)) in pts.iter().enumerate() {
            let (px, py) = p(*u, *v);
            let _ = write!(s, "{px:.2} {py:.2} {} ", if i == 0 { "m" } else { "l" });
        }
        let _ = writeln!(s, "h {op}");
    };
    let line = |s: &mut String, a: (f32, f32), b2: (f32, f32)| {
        let (ax, ay) = p(a.0, a.1);
        let (bx, by) = p(b2.0, b2.1);
        let _ = write!(s, "{ax:.2} {ay:.2} m {bx:.2} {by:.2} l ");
    };
    // A circle by four Béziers (k = 0.5523).
    let circle = |s: &mut String, cu: f32, cv: f32, ru: f32, op: &str| {
        let k = 0.5523 * ru;
        let pts = [
            (
                (cu + ru, cv),
                (cu + ru, cv + k),
                (cu + k, cv + ru),
                (cu, cv + ru),
            ),
            (
                (cu, cv + ru),
                (cu - k, cv + ru),
                (cu - ru, cv + k),
                (cu - ru, cv),
            ),
            (
                (cu - ru, cv),
                (cu - ru, cv - k),
                (cu - k, cv - ru),
                (cu, cv - ru),
            ),
            (
                (cu, cv - ru),
                (cu + k, cv - ru),
                (cu + ru, cv - k),
                (cu + ru, cv),
            ),
        ];
        let (sx, sy) = p(pts[0].0 .0, pts[0].0 .1);
        let _ = write!(s, "{sx:.2} {sy:.2} m ");
        for (_, c1, c2, e) in pts {
            let (ax, ay) = p(c1.0, c1.1);
            let (bx, by) = p(c2.0, c2.1);
            let (ex, ey) = p(e.0, e.1);
            let _ = write!(s, "{ax:.2} {ay:.2} {bx:.2} {by:.2} {ex:.2} {ey:.2} c ");
        }
        let _ = writeln!(s, "h {op}");
    };
    match name {
        // A key: round bow with a hole, a shaft and two bits.
        "Key" => {
            circle(&mut s, 6.0, 13.0, 5.0, "B");
            poly(
                &mut s,
                &[
                    (10.0, 11.0),
                    (19.0, 2.5),
                    (19.0, 0.8),
                    (16.6, 0.8),
                    (16.6, 2.6),
                    (14.6, 2.6),
                    (14.6, 4.6),
                    (8.4, 9.4),
                ],
                "B",
            );
            circle(&mut s, 5.2, 14.0, 1.4, "S");
        }
        // A sheet with a folded corner and three lines.
        "Note" => {
            poly(
                &mut s,
                &[
                    (3.0, 1.0),
                    (3.0, 19.0),
                    (17.0, 19.0),
                    (17.0, 6.0),
                    (12.0, 1.0),
                ],
                "B",
            );
            poly(&mut s, &[(12.0, 1.0), (12.0, 6.0), (17.0, 6.0)], "S");
            for v in [15.0, 12.0, 9.0] {
                line(&mut s, (6.0, v), (14.0, v));
            }
            s.push_str("S\n");
        }
        // A disc with a question hook and a dot.
        "Help" => {
            circle(&mut s, 10.0, 10.0, 9.0, "B");
            let (ax, ay) = p(7.0, 12.5);
            let (c1x, c1y) = p(7.0, 16.5);
            let (c2x, c2y) = p(13.0, 16.5);
            let (ex, ey) = p(13.0, 12.5);
            let (c3x, c3y) = p(13.0, 10.0);
            let (fx, fy) = p(10.0, 10.5);
            let (gx, gy) = p(10.0, 8.0);
            let _ = writeln!(
                s,
                "1.6 w {ax:.2} {ay:.2} m {c1x:.2} {c1y:.2} {c2x:.2} {c2y:.2} {ex:.2} {ey:.2} c \
                 {c3x:.2} {c3y:.2} {fx:.2} {fy:.2} {gx:.2} {gy:.2} c S"
            );
            s.push_str("0.25 0.25 0.25 rg\n");
            circle(&mut s, 10.0, 4.6, 1.1, "f");
        }
        // A roof (triangle) over a paragraph bar: a new paragraph starts here.
        "NewParagraph" => {
            poly(&mut s, &[(10.0, 19.0), (18.5, 9.0), (1.5, 9.0)], "B");
            poly(
                &mut s,
                &[(3.0, 1.0), (3.0, 6.5), (17.0, 6.5), (17.0, 1.0)],
                "B",
            );
        }
        // A pilcrow: a round bowl with two stems.
        "Paragraph" => {
            circle(&mut s, 8.0, 13.5, 5.0, "B");
            poly(
                &mut s,
                &[
                    (8.0, 18.5),
                    (16.0, 18.5),
                    (16.0, 1.0),
                    (13.5, 1.0),
                    (13.5, 18.5),
                ],
                "B",
            );
            poly(
                &mut s,
                &[(8.0, 8.5), (8.0, 1.0), (10.5, 1.0), (10.5, 18.5)],
                "B",
            );
        }
        // An insertion caret.
        "Insert" => {
            poly(
                &mut s,
                &[
                    (10.0, 17.0),
                    (19.0, 2.0),
                    (15.0, 2.0),
                    (10.0, 10.5),
                    (5.0, 2.0),
                    (1.0, 2.0),
                ],
                "B",
            );
        }
        // A thick diagonal cross.
        "Cross" => poly(
            &mut s,
            &[
                (3.0, 1.0),
                (10.0, 8.0),
                (17.0, 1.0),
                (19.0, 3.0),
                (12.0, 10.0),
                (19.0, 17.0),
                (17.0, 19.0),
                (10.0, 12.0),
                (3.0, 19.0),
                (1.0, 17.0),
                (8.0, 10.0),
                (1.0, 3.0),
            ],
            "B",
        ),
        // A tick.
        "Check" => poly(
            &mut s,
            &[
                (1.0, 10.0),
                (4.0, 13.0),
                (8.0, 8.0),
                (17.0, 19.0),
                (19.0, 17.0),
                (8.0, 1.5),
            ],
            "B",
        ),
        // A ring.
        "Circle" => {
            circle(&mut s, 10.0, 10.0, 9.0, "B");
            circle(&mut s, 10.0, 10.0, 4.5, "S");
        }
        // A five-pointed star.
        "Star" => {
            let pts: Vec<(f32, f32)> = (0..10)
                .map(|k| {
                    let r = if k % 2 == 0 { 9.5 } else { 4.0 };
                    let a = std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
                    (10.0 + r * a.cos(), 10.0 + r * a.sin())
                })
                .collect();
            poly(&mut s, &pts, "B");
        }
        // A block arrow to the right.
        "RightArrow" => poly(
            &mut s,
            &[
                (1.0, 7.0),
                (11.0, 7.0),
                (11.0, 2.0),
                (19.0, 10.0),
                (11.0, 18.0),
                (11.0, 13.0),
                (1.0, 13.0),
            ],
            "B",
        ),
        // A notched pointer to the right.
        "RightPointer" => poly(
            &mut s,
            &[(1.0, 2.0), (19.0, 10.0), (1.0, 18.0), (6.0, 10.0)],
            "B",
        ),
        // A block arrow up.
        "UpArrow" => poly(
            &mut s,
            &[
                (7.0, 1.0),
                (13.0, 1.0),
                (13.0, 11.0),
                (18.0, 11.0),
                (10.0, 19.0),
                (2.0, 11.0),
                (7.0, 11.0),
            ],
            "B",
        ),
        // A pointer to the upper left.
        "UpLeftArrow" => poly(
            &mut s,
            &[
                (1.0, 19.0),
                (13.0, 15.5),
                (9.5, 12.0),
                (18.5, 3.0),
                (17.0, 1.5),
                (8.0, 10.5),
                (4.5, 7.0),
            ],
            "B",
        ),
        // A ring with cross hairs.
        "CrossHairs" => {
            circle(&mut s, 10.0, 10.0, 7.0, "B");
            line(&mut s, (10.0, 0.5), (10.0, 19.5));
            line(&mut s, (0.5, 10.0), (19.5, 10.0));
            s.push_str("S\n");
        }
        // Comment (and any other name): a speech bubble with lines.
        _ => {
            poly(
                &mut s,
                &[
                    (1.0, 6.0),
                    (1.0, 19.0),
                    (19.0, 19.0),
                    (19.0, 6.0),
                    (9.0, 6.0),
                    (4.5, 1.0),
                    (5.0, 6.0),
                ],
                "B",
            );
            for v in [15.0, 12.0, 9.0] {
                line(&mut s, (4.0, v), (16.0, v));
            }
            s.push_str("S\n");
        }
    }
    s
}

impl PdfDocument {
    /// The icon name (`/Name`) and opacity (`/CA`) of a note; `None` for other comments.
    #[must_use]
    pub fn note_style(&self, a: &Annot) -> Option<(String, f32)> {
        if a.kind != AnnotKind::Note {
            return None;
        }
        let d = self.doc.get_dictionary(a.id?).ok()?;
        let name = d.get(b"Name").and_then(Object::as_name).map_or_else(
            |_| "Note".to_owned(),
            |n| String::from_utf8_lossy(n).into_owned(),
        );
        let ca = d
            .get(b"CA")
            .and_then(Object::as_float)
            .map_or(1.0, |v| v.clamp(0.0, 1.0));
        Some((name, ca))
    }

    /// Give note `a` icon `icon` (one of [`NOTE_ICONS`]) and opacity `alpha` (0–1): `/Name`,
    /// `/CA` and a new appearance (one undo step).
    pub fn set_note_style(&mut self, a: &Annot, icon: &str, alpha: f32) -> Result<(), String> {
        if a.kind != AnnotKind::Note {
            return Err("not a note".into());
        }
        if !NOTE_ICONS_DRAWN.contains(&icon) {
            return Err("unknown note icon".into());
        }
        let id = a.id.ok_or("the comment cannot be changed")?;
        let alpha = if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            1.0
        };
        let rect: Vec<f32> = {
            let d = self.doc.get_dictionary(id).map_err(|e| e.to_string())?;
            d.get(b"Rect")
                .ok()
                .and_then(|o| o.as_array().ok())
                .map(|v| v.iter().filter_map(|o| o.as_float().ok()).collect())
                .unwrap_or_default()
        };
        let rect = <[f32; 4]>::try_from(rect.as_slice()).map_err(|_| "no rectangle")?;
        let rect = [
            rect[0].min(rect[2]),
            rect[1].min(rect[3]),
            rect[0].max(rect[2]),
            rect[1].max(rect[3]),
        ];
        let rgb = a.color.map(|c| f32::from(c) / 255.0);
        let content = note_icon_content(icon, rect, rgb, alpha);
        self.begin("note style");
        let mut res = Dictionary::new();
        if alpha < 0.999 {
            let mut gs = Dictionary::new();
            gs.set("Type", Object::Name(b"ExtGState".to_vec()));
            gs.set("ca", Object::Real(alpha));
            gs.set("CA", Object::Real(alpha));
            let mut ext = Dictionary::new();
            ext.set("GS0", gs);
            res.set("ExtGState", ext);
        }
        let mut ap_dict = Dictionary::new();
        ap_dict.set("Type", Object::Name(b"XObject".to_vec()));
        ap_dict.set("Subtype", Object::Name(b"Form".to_vec()));
        ap_dict.set("BBox", nums(&rect));
        ap_dict.set("Resources", res);
        let ap_id = self.add(Object::Stream(lopdf::Stream::new(
            ap_dict,
            content.into_bytes(),
        )));
        self.touch(id);
        if let Ok(d) = self.doc.get_dictionary_mut(id) {
            d.set("Name", Object::Name(icon.as_bytes().to_vec()));
            if alpha < 0.999 {
                d.set("CA", Object::Real(alpha));
            } else {
                d.remove(b"CA");
            }
            d.set("M", text_obj(&pdf_date()));
            let mut ap = Dictionary::new();
            ap.set("N", Object::Reference(ap_id));
            d.set("AP", ap);
        }
        self.commit();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_draws_a_closed_outline() {
        for name in NOTE_ICONS_DRAWN {
            let c = note_icon_content(name, [10.0, 10.0, 30.0, 30.0], [1.0, 0.9, 0.0], 0.5);
            assert!(c.starts_with("/GS0 gs"), "{name}");
            assert!(c.contains(" B") || c.contains(" B\n"), "{name}: {c}");
            // Every coordinate stays inside the rectangle.
            for v in c.split_whitespace().filter_map(|t| t.parse::<f32>().ok()) {
                assert!(v <= 30.01, "{name}: {v}");
            }
        }
        let opaque = note_icon_content("Key", [0.0, 0.0, 20.0, 20.0], [0.0; 3], 1.0);
        assert!(!opaque.contains("/GS0"));
    }
}
