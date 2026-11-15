//! Shapes drawn into page content, image annotations, audio / video and 3D for BlueEditor's
//! Edit tab.
//!
//! - Shapes are path objects appended to the page content (ISO 32000-1 §8.5: `re`, Bézier
//!   ellipses, `m`/`l` lines; `w` `RG` `rg`, `S` / `f` / `B`), editable afterwards with Edit
//!   Object like any other path.
//! - An image annotation is a `Screen` annotation (§12.5.6.18) whose normal appearance draws
//!   the picture.
//! - Audio and video are `Screen` annotations with a rendition action (§12.6.4.13, §13.2.3
//!   media rendition, media clip data with the embedded file).
//! - 3D is a `3D` annotation (§13.6.2) with a U3D or PRC stream; any other file is refused.
use crate::annot::{nums, pdf_date, text_obj};
use crate::objects::matrix;
use crate::PdfDocument;
use lopdf::{Dictionary, Object, ObjectId, Stream, StringFormat};

/// A shape of Add Shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    /// Rectangle.
    Rectangle,
    /// Rectangle with rounded corners.
    RoundedRectangle,
    /// Ellipse.
    Ellipse,
    /// Straight line from the top-left to the bottom-right corner of the box.
    Line,
}

/// Ellipse path through the box (four Bézier quarters).
fn ellipse_path(r: [f32; 4]) -> String {
    let (cx, cy) = (f32::midpoint(r[0], r[2]), f32::midpoint(r[1], r[3]));
    let (rx, ry) = ((r[2] - r[0]) / 2.0, (r[3] - r[1]) / 2.0);
    let k = 0.552_284_8;
    let f = |v: f32| format!("{}", (v * 1000.0).round() / 1000.0);
    format!(
        "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c h ",
        f(cx + rx), f(cy),
        f(cx + rx), f(cy + ry * k), f(cx + rx * k), f(cy + ry), f(cx), f(cy + ry),
        f(cx - rx * k), f(cy + ry), f(cx - rx), f(cy + ry * k), f(cx - rx), f(cy),
        f(cx - rx), f(cy - ry * k), f(cx - rx * k), f(cy - ry), f(cx), f(cy - ry),
        f(cx + rx * k), f(cy - ry), f(cx + rx), f(cy - ry * k), f(cx + rx), f(cy),
    )
}

fn rounded_path(r: [f32; 4], rr: f32) -> String {
    let (x0, y0, x1, y1) = (r[0], r[1], r[2], r[3]);
    let rr = rr.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    let k = rr * 0.447_715_2;
    let f = |v: f32| format!("{}", (v * 1000.0).round() / 1000.0);
    format!(
        "{} {} m {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c h ",
        f(x0 + rr), f(y0), f(x1 - rr), f(y0),
        f(x1 - k), f(y0), f(x1), f(y0 + k), f(x1), f(y0 + rr),
        f(x1), f(y1 - rr),
        f(x1), f(y1 - k), f(x1 - k), f(y1), f(x1 - rr), f(y1),
        f(x0 + rr), f(y1),
        f(x0 + k), f(y1), f(x0), f(y1 - k), f(x0), f(y1 - rr),
        f(x0), f(y0 + rr),
        f(x0), f(y0 + k), f(x0 + k), f(y0), f(x0 + rr), f(y0),
    )
}

fn rgb(c: [u8; 3]) -> String {
    let f = |v: u8| format!("{}", (f32::from(v) / 255.0 * 1000.0).round() / 1000.0);
    format!("{} {} {}", f(c[0]), f(c[1]), f(c[2]))
}

/// Our own play symbol (a triangle in a rounded frame) or a cube outline for 3D, in a box
/// of `w` × `h` points.
fn media_symbol(w: f32, h: f32, cube: bool) -> String {
    let s = w.min(h) * 0.4;
    let (cx, cy) = (w / 2.0, h / 2.0);
    let frame = format!(
        "0.94 g 0 0 {w} {h} re f 0.55 G 1 w 0.5 0.5 {} {} re S ",
        w - 1.0,
        h - 1.0
    );
    if cube {
        let d = s * 0.35;
        format!(
            "{frame}0.3 G 1.2 w {} {} {} {} re S {} {} m {} {} l {} {} l {} {} l S {} {} m {} {} l S ",
            cx - s / 2.0, cy - s / 2.0, s * 0.8, s * 0.8,
            cx - s / 2.0, cy - s / 2.0 + s * 0.8,
            cx - s / 2.0 + d, cy + s / 2.0 + d * 0.5 - s * 0.2 + s * 0.2,
            cx + s / 2.0 + d - s * 0.2, cy + s / 2.0 + d * 0.5,
            cx - s / 2.0 + s * 0.8, cy - s / 2.0 + s * 0.8,
            cx - s / 2.0 + s * 0.8, cy - s / 2.0,
            cx + s / 2.0 + d - s * 0.2, cy - s / 2.0 + d * 0.5 + s * 0.3,
        )
    } else {
        format!(
            "{frame}0.3 g {} {} m {} {} l {} {} l h f ",
            cx - s * 0.35,
            cy - s / 2.0,
            cx + s * 0.5,
            cy,
            cx - s * 0.35,
            cy + s / 2.0
        )
    }
}

impl PdfDocument {
    /// Draw a shape into page `index`'s content inside `rect` (page points): `stroke` and/or
    /// `fill` colours, line `width` (one undo step).
    pub fn add_shape(
        &mut self,
        index: usize,
        kind: ShapeKind,
        rect: [f32; 4],
        stroke: Option<[u8; 3]>,
        fill: Option<[u8; 3]>,
        width: f32,
    ) -> Result<(), String> {
        if stroke.is_none() && fill.is_none() {
            return Err("a shape needs a line or a fill".into());
        }
        self.begin("add shape");
        let r = (|| {
            let (_, inv) = self.bases(index)?;
            // Shape space: page points, y down.
            let (sid, content) = self.page_stream(index)?;
            let r = [
                rect[0].min(rect[2]),
                rect[1].min(rect[3]),
                rect[0].max(rect[2]),
                rect[1].max(rect[3]),
            ];
            let mut s = format!("\nq {} cm ", matrix(inv));
            if let Some(c) = stroke {
                s.push_str(&format!(
                    "{} RG {} w ",
                    rgb(c),
                    (width.max(0.1) * 100.0).round() / 100.0
                ));
            }
            if let Some(c) = fill {
                s.push_str(&format!("{} rg ", rgb(c)));
            }
            let path = match kind {
                ShapeKind::Rectangle => {
                    format!("{} {} {} {} re ", r[0], r[1], r[2] - r[0], r[3] - r[1])
                }
                ShapeKind::RoundedRectangle => {
                    rounded_path(r, (r[2] - r[0]).min(r[3] - r[1]) * 0.15)
                }
                ShapeKind::Ellipse => ellipse_path(r),
                ShapeKind::Line => format!("{} {} m {} {} l ", rect[0], rect[1], rect[2], rect[3]),
            };
            s.push_str(&path);
            let op = match (stroke.is_some(), fill.is_some() && kind != ShapeKind::Line) {
                (true, true) => "B",
                (false, true) => "f",
                _ => "S",
            };
            s.push_str(op);
            s.push_str(" Q\n");
            self.write_page(sid, content, Vec::new(), s.as_bytes());
            Ok(())
        })();
        self.commit();
        r
    }

    /// A `Screen` / `3D` annotation dictionary over `rect` with appearance `ap` drawn in a
    /// `w` × `h` box with `res` resources.
    fn screen_annot(
        &mut self,
        page: usize,
        rect: [f32; 4],
        subtype: &[u8],
        body: String,
        res: Dictionary,
        title: &str,
    ) -> Result<(ObjectId, Dictionary), String> {
        let info = self.pages.get(page).ok_or("no such page")?.clone();
        let (_, inv) = self.bases(page)?;
        let a = crate::interp::apply(inv, rect[0], rect[1]);
        let b = crate::interp::apply(inv, rect[2], rect[3]);
        let ur = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
        let (w, h) = (ur[2] - ur[0], ur[3] - ur[1]);
        let mut ap = Dictionary::new();
        ap.set("Type", Object::Name(b"XObject".to_vec()));
        ap.set("Subtype", Object::Name(b"Form".to_vec()));
        ap.set("BBox", nums(&[0.0, 0.0, w, h]));
        ap.set("Resources", res);
        let ap_id = self.add(Object::Stream(Stream::new(ap, body.into_bytes())));
        let mut n = Dictionary::new();
        n.set("N", Object::Reference(ap_id));
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"Annot".to_vec()));
        d.set("Subtype", Object::Name(subtype.to_vec()));
        d.set("Rect", nums(&ur));
        d.set("F", 4);
        d.set("P", Object::Reference(info.id));
        d.set("AP", n);
        d.set("T", text_obj(title));
        d.set("Contents", text_obj(title));
        d.set("M", text_obj(&pdf_date()));
        let id = self.add(Object::Dictionary(Dictionary::new()));
        Ok((id, d))
    }

    /// Picture `image` (PNG or JPEG) as an image annotation over `rect` (one undo step).
    pub fn add_image_annotation(
        &mut self,
        page: usize,
        rect: [f32; 4],
        image: crate::NewImage<'_>,
        title: &str,
    ) -> Result<(), String> {
        self.begin("image annotation");
        let r = (|| {
            let xid = self.image_xobject(image)?;
            let (_, inv) = self.bases(page)?;
            let a = crate::interp::apply(inv, rect[0], rect[1]);
            let b = crate::interp::apply(inv, rect[2], rect[3]);
            let (w, h) = ((a.0 - b.0).abs(), (a.1 - b.1).abs());
            let mut xo = Dictionary::new();
            xo.set("Im0", Object::Reference(xid));
            let mut res = Dictionary::new();
            res.set("XObject", xo);
            let body = format!("q {w} 0 0 {h} 0 0 cm /Im0 Do Q");
            let (id, d) = self.screen_annot(page, rect, b"Screen", body, res, title)?;
            if let Ok(o) = self.doc.get_object_mut(id) {
                *o = Object::Dictionary(d);
            }
            self.push_annot(page, id)
        })();
        self.commit();
        r
    }

    /// Embed audio or video `bytes` (`name`, MIME type `mime`) over `rect` as a screen
    /// annotation with a rendition action that plays it (one undo step).
    pub fn add_media(
        &mut self,
        page: usize,
        rect: [f32; 4],
        name: &str,
        mime: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.begin("media");
        let r = (|| {
            let (_, inv) = self.bases(page)?;
            let a = crate::interp::apply(inv, rect[0], rect[1]);
            let b = crate::interp::apply(inv, rect[2], rect[3]);
            let (w, h) = ((a.0 - b.0).abs(), (a.1 - b.1).abs());
            let body = media_symbol(w, h, false);
            let (id, mut d) =
                self.screen_annot(page, rect, b"Screen", body, Dictionary::new(), name)?;
            let fs = self.embedded_file(name, bytes);
            // Media clip data (§13.2.4.2) in a media rendition (§13.2.3.2).
            let mut clip = Dictionary::new();
            clip.set("Type", Object::Name(b"MediaClip".to_vec()));
            clip.set("S", Object::Name(b"MCD".to_vec()));
            clip.set("N", text_obj(name));
            clip.set(
                "CT",
                Object::String(mime.as_bytes().to_vec(), StringFormat::Literal),
            );
            clip.set("D", fs);
            let mut perm = Dictionary::new();
            perm.set(
                "TF",
                Object::String(b"TEMPACCESS".to_vec(), StringFormat::Literal),
            );
            clip.set("P", perm);
            let mut rend = Dictionary::new();
            rend.set("Type", Object::Name(b"Rendition".to_vec()));
            rend.set("S", Object::Name(b"MR".to_vec()));
            rend.set("N", text_obj(name));
            rend.set("C", clip);
            let mut act = Dictionary::new();
            act.set("Type", Object::Name(b"Action".to_vec()));
            act.set("S", Object::Name(b"Rendition".to_vec()));
            act.set("OP", 0);
            act.set("AN", Object::Reference(id));
            act.set("R", rend);
            d.set("A", act);
            if let Ok(o) = self.doc.get_object_mut(id) {
                *o = Object::Dictionary(d);
            }
            self.push_annot(page, id)
        })();
        self.commit();
        r
    }

    /// Embed a U3D or PRC model over `rect` as a 3D annotation (one undo step).
    pub fn add_3d(
        &mut self,
        page: usize,
        rect: [f32; 4],
        name: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        let sub: &[u8] = if bytes.starts_with(b"U3D\0") {
            b"U3D"
        } else if bytes.starts_with(b"PRC") {
            b"PRC"
        } else {
            return Err("3D needs a U3D or PRC file".into());
        };
        self.begin("3D");
        let r = (|| {
            let (_, inv) = self.bases(page)?;
            let a = crate::interp::apply(inv, rect[0], rect[1]);
            let b = crate::interp::apply(inv, rect[2], rect[3]);
            let (w, h) = ((a.0 - b.0).abs(), (a.1 - b.1).abs());
            let body = media_symbol(w, h, true);
            let (id, mut d) =
                self.screen_annot(page, rect, b"3D", body, Dictionary::new(), name)?;
            let mut sd = Dictionary::new();
            sd.set("Type", Object::Name(b"3D".to_vec()));
            sd.set("Subtype", Object::Name(sub.to_vec()));
            let sid = self.add(Object::Stream(Stream::new(sd, bytes.to_vec())));
            d.set("3DD", Object::Reference(sid));
            // Activate when the page opens is the viewer's choice; we ask for an explicit click.
            let mut act = Dictionary::new();
            act.set("A", Object::Name(b"XA".to_vec()));
            d.set("3DA", act);
            if let Ok(o) = self.doc.get_object_mut(id) {
                *o = Object::Dictionary(d);
            }
            self.push_annot(page, id)
        })();
        self.commit();
        r
    }

    /// A file specification with `bytes` embedded (§7.11.4).
    fn embedded_file(&mut self, name: &str, bytes: &[u8]) -> Dictionary {
        let mut ef_stream = Dictionary::new();
        ef_stream.set("Type", Object::Name(b"EmbeddedFile".to_vec()));
        let mut params = Dictionary::new();
        params.set("Size", bytes.len() as i64);
        ef_stream.set("Params", params);
        let ef = self.add(Object::Stream(Stream::new(ef_stream, bytes.to_vec())));
        let mut efd = Dictionary::new();
        efd.set("F", Object::Reference(ef));
        efd.set("UF", Object::Reference(ef));
        let mut fs = Dictionary::new();
        fs.set("Type", Object::Name(b"Filespec".to_vec()));
        fs.set(
            "F",
            Object::String(name.as_bytes().to_vec(), StringFormat::Literal),
        );
        fs.set("UF", text_obj(name));
        fs.set("EF", efd);
        fs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{standard_fonts, ObjectKind, RenderOptions};

    fn doc() -> PdfDocument {
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Resources << >> >>",
        ];
        PdfDocument::open(crate::tests::build_pdf(&objs), standard_fonts()).expect("opens")
    }

    fn png() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, 2, 2);
            e.set_color(png::ColorType::Rgb);
            e.set_depth(png::BitDepth::Eight);
            let mut w = e.write_header().expect("header");
            w.write_image_data(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 9, 9, 9])
                .expect("data");
        }
        out
    }

    #[test]
    fn shapes_become_path_objects() {
        let mut d = doc();
        for k in [
            ShapeKind::Rectangle,
            ShapeKind::RoundedRectangle,
            ShapeKind::Ellipse,
            ShapeKind::Line,
        ] {
            d.add_shape(
                0,
                k,
                [20.0, 30.0, 120.0, 90.0],
                Some([200, 0, 0]),
                Some([0, 0, 200]),
                2.0,
            )
            .expect("shape");
        }
        let objs = d.page_objects(0).expect("objects");
        let paths: Vec<&crate::PageObject> =
            objs.iter().filter(|o| o.kind == ObjectKind::Path).collect();
        assert_eq!(paths.len(), 4);
        // The rectangle lands where it was drawn (page points, y down).
        let b = paths[0].bbox;
        assert!(
            (b[0] - 20.0).abs() < 1.5 && (b[1] - 30.0).abs() < 1.5,
            "{b:?}"
        );
        assert!(
            (b[2] - 120.0).abs() < 1.5 && (b[3] - 90.0).abs() < 1.5,
            "{b:?}"
        );
        assert!(d
            .add_shape(0, ShapeKind::Line, [0.0; 4], None, None, 1.0)
            .is_err());
        assert!(d.undo());
        assert_eq!(
            d.page_objects(0)
                .expect("o")
                .iter()
                .filter(|o| o.kind == ObjectKind::Path)
                .count(),
            3
        );
    }

    #[test]
    fn image_media_and_3d_annotations() {
        let mut d = doc();
        d.add_image_annotation(0, [10.0, 10.0, 110.0, 60.0], crate::NewImage(&png()), "Ảnh")
            .expect("image");
        d.add_media(
            0,
            [10.0, 100.0, 170.0, 190.0],
            "clip.mp4",
            "video/mp4",
            b"\0\0\0 ftypmp42",
        )
        .expect("media");
        assert!(d
            .add_3d(0, [10.0, 200.0, 100.0, 280.0], "a.obj", b"v 0 0 0")
            .is_err());
        d.add_3d(0, [10.0, 200.0, 100.0, 280.0], "a.u3d", b"U3D\0rest")
            .expect("3d");
        let saved = d.save().expect("saves");
        let text = String::from_utf8_lossy(&saved);
        assert!(text.contains("/Subtype /Screen") && text.contains("/S /Rendition"));
        assert!(text.contains("/CT (video/mp4)") && text.contains("/Subtype /U3D"));
        let mut again = PdfDocument::open(saved, standard_fonts()).expect("opens");
        assert_eq!(again.annotations().len(), 3);
        let page = again.render(0, RenderOptions::default()).expect("renders");
        assert!(!page.images.is_empty(), "the picture is drawn");
    }
}
