//! BlueEditor round 12: the stamps of the editor's Stamp palette (ISO 32000-1 §12.5.6.12
//! rubber stamp annotations, §12.5.5 appearance streams). The drawings are ours: a rounded
//! frame in the stamp's colour (or a filled band with white letters for the "sign here"
//! family) with the title in capitals and, for dynamic stamps, a second smaller line (who and
//! when). The `/Name` is the Table 181 name when there is one, else the title without spaces.
use crate::annot::{nums, pdf_date, text_obj};
use crate::interp::{apply, invert};
use crate::PdfDocument;
use lopdf::{Dictionary, Object, Stream};
use std::fmt::Write as _;

/// How a palette stamp looks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampLook {
    /// Title (drawn in capitals).
    pub title: String,
    /// Second line (dynamic stamps: author and date), empty for none.
    pub sub: String,
    /// Colour.
    pub color: [u8; 3],
    /// Filled band with white letters.
    pub solid: bool,
}

/// Table 181 names.
const STANDARD: [&str; 14] = [
    "Approved",
    "Experimental",
    "NotApproved",
    "AsIs",
    "Expired",
    "NotForPublicRelease",
    "Confidential",
    "Final",
    "Sold",
    "Departmental",
    "ForComment",
    "TopSecret",
    "Draft",
    "ForPublicRelease",
];

fn n(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_owned()
    }
}

/// Outlined text of `s` at `size` with its baseline at (`x`, `y`) (PDF space); its width.
fn outlined(s: &str, size: f32, x: f32, y: f32) -> (String, f32) {
    let Some(data) = crate::fallback::face_data(crate::fallback::SANS) else {
        return (String::new(), 0.0);
    };
    let Ok(face) = ttf_parser::Face::parse(data, 0) else {
        return (String::new(), 0.0);
    };
    let (p, w) = crate::sigappear::text_path(&face, s, size, x, y);
    (p, w)
}

/// A rounded rectangle path (x0, y0)–(x1, y1) with corner radius `r`.
fn rounded(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> String {
    let k = r * 0.4477;
    format!(
        "{} {} m {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c h ",
        n(x0 + r), n(y0), n(x1 - r), n(y0),
        n(x1 - k), n(y0), n(x1), n(y0 + k), n(x1), n(y0 + r),
        n(x1), n(y1 - r),
        n(x1), n(y1 - k), n(x1 - k), n(y1), n(x1 - r), n(y1),
        n(x0 + r), n(y1),
        n(x0 + k), n(y1), n(x0), n(y1 - k), n(x0), n(y1 - r),
        n(x0), n(y0 + r),
        n(x0), n(y0 + k), n(x0 + k), n(y0), n(x0 + r), n(y0),
    )
}

impl PdfDocument {
    /// Place a palette stamp with its top-left corner at `at` (page points, origin top left as
    /// shown) on page `page`, `height` points high; one undo step. Its box (page points, left,
    /// top, right, bottom).
    ///
    /// # Errors
    /// When the page is missing or the document is encrypted without its password.
    pub fn add_stamp12(
        &mut self,
        page: usize,
        at: [f32; 2],
        height: f32,
        look: &StampLook,
        author: &str,
    ) -> Result<[f32; 4], String> {
        if self.doc.is_encrypted() && self.doc.encryption_state.is_none() {
            return Err("comments cannot be added to an encrypted document".into());
        }
        let info = self.pages.get(page).ok_or("no such page")?.clone();
        let title = look.title.to_uppercase();
        let h = height.max(12.0);
        let dynamic = !look.sub.trim().is_empty();
        let size = if dynamic { h * 0.40 } else { h * 0.50 };
        let sub_size = h * 0.20;
        let (_, tw) = outlined(&title, size, 0.0, 0.0);
        let (_, sw) = outlined(&look.sub, sub_size, 0.0, 0.0);
        let pad = h * 0.28;
        let w = tw.max(sw) + 2.0 * pad;
        // Page points (y down) → user space.
        let inv = invert(crate::page_matrix(info.crop, info.rotate)).ok_or("page matrix")?;
        let a = apply(inv, at[0], at[1]);
        let b = apply(inv, at[0] + w, at[1] + h);
        let rect = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
        // The appearance is drawn upright in a w × h box; /Matrix-free, the BBox is the box.
        let [r, g, bl] = look.color.map(|c| f32::from(c) / 255.0);
        let mut s = String::new();
        let rr = h * 0.16;
        if look.solid {
            let _ = write!(
                s,
                "{} {} {} rg {}f ",
                n(r),
                n(g),
                n(bl),
                rounded(0.5, 0.5, w - 0.5, h - 0.5, rr)
            );
            let _ = write!(s, "1 1 1 rg ");
        } else {
            let _ = write!(
                s,
                "1 1 1 rg {}f {} {} {} RG 2 w {}S {} {} {} rg ",
                rounded(1.0, 1.0, w - 1.0, h - 1.0, rr),
                n(r),
                n(g),
                n(bl),
                rounded(1.0, 1.0, w - 1.0, h - 1.0, rr),
                n(r),
                n(g),
                n(bl)
            );
        }
        let base = if dynamic {
            h - pad * 0.55 - size * 0.72
        } else {
            (h - size * 0.7) / 2.0
        };
        let (p, _) = outlined(&title, size, (w - tw) / 2.0, base);
        s.push_str(&p);
        s.push_str("f ");
        if dynamic {
            let (p, _) = outlined(&look.sub, sub_size, (w - sw) / 2.0, pad * 0.6);
            s.push_str(&p);
            s.push_str("f ");
        }
        // Rotated pages: draw the upright stamp turned with the page.
        let (bw, bh, matrix) = match info.rotate {
            90 => (w, h, Some([0.0, 1.0, -1.0, 0.0, h, 0.0])),
            180 => (w, h, Some([-1.0, 0.0, 0.0, -1.0, w, h])),
            270 => (w, h, Some([0.0, -1.0, 1.0, 0.0, 0.0, w])),
            _ => (w, h, None),
        };
        let mut ap = Dictionary::new();
        ap.set("Type", Object::Name(b"XObject".to_vec()));
        ap.set("Subtype", Object::Name(b"Form".to_vec()));
        ap.set("BBox", nums(&[0.0, 0.0, bw, bh]));
        if let Some(m) = matrix {
            ap.set("Matrix", nums(&m));
        }
        ap.set("Resources", Dictionary::new());
        let name: String = STANDARD
            .iter()
            .find(|s| s.eq_ignore_ascii_case(&look.title.replace(' ', "")))
            .map_or_else(
                || {
                    look.title
                        .chars()
                        .filter(char::is_ascii_alphanumeric)
                        .collect()
                },
                |s| (*s).to_owned(),
            );
        self.begin("stamp");
        let ap_id = self.add(Object::Stream(Stream::new(ap, s.into_bytes())));
        let mut apd = Dictionary::new();
        apd.set("N", Object::Reference(ap_id));
        let now = pdf_date();
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"Annot".to_vec()));
        d.set("Subtype", Object::Name(b"Stamp".to_vec()));
        d.set("Rect", nums(&rect));
        d.set("Name", Object::Name(name.into_bytes()));
        d.set("C", nums(&[r, g, bl]));
        d.set("F", 4);
        d.set("P", Object::Reference(info.id));
        d.set("T", text_obj(author));
        let contents = if dynamic {
            format!("{}\n{}", look.title, look.sub)
        } else {
            look.title.clone()
        };
        d.set("Contents", text_obj(&contents));
        d.set("M", text_obj(&now));
        d.set("CreationDate", text_obj(&now));
        d.set("NM", text_obj(&format!("bo-{}-{}", ap_id.0, now)));
        d.set("AP", apd);
        let id = self.add(Object::Dictionary(d));
        let page_d = self
            .doc
            .get_dictionary(info.id)
            .map_err(|e| e.to_string())?;
        if let Ok(Object::Reference(arr)) = page_d.get(b"Annots") {
            let arr = *arr;
            self.touch(arr);
            if let Ok(Object::Array(v)) = self.doc.get_object_mut(arr) {
                v.push(Object::Reference(id));
            }
        } else {
            self.touch(info.id);
            let pd = self
                .doc
                .get_dictionary_mut(info.id)
                .map_err(|e| e.to_string())?;
            match pd.get_mut(b"Annots") {
                Ok(Object::Array(v)) => v.push(Object::Reference(id)),
                _ => pd.set("Annots", vec![Object::Reference(id)]),
            }
        }
        self.commit();
        Ok([at[0], at[1], at[0] + w, at[1] + h])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{standard_fonts, AnnotKind, RenderOptions};

    #[test]
    fn stamps_are_added_drawn_and_named() {
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Resources << >> >>",
        ];
        let mut d =
            PdfDocument::open(crate::tests::build_pdf(&objs), standard_fonts()).expect("opens");
        let look = StampLook {
            title: "Approved".into(),
            sub: "Tester, 2026-11-17 10:00".into(),
            color: [30, 80, 180],
            solid: false,
        };
        let r = d
            .add_stamp12(0, [20.0, 20.0], 30.0, &look, "Tester")
            .expect("adds");
        assert!((r[3] - r[1] - 30.0).abs() < 0.01 && r[2] > r[0] + 40.0);
        let sign = StampLook {
            title: "Sign Here".into(),
            sub: String::new(),
            color: [30, 120, 80],
            solid: true,
        };
        d.add_stamp12(0, [20.0, 80.0], 24.0, &sign, "Tester")
            .expect("adds");
        let list = d.annotations();
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|a| a.kind == AnnotKind::Stamp));
        let saved = d.save().expect("saves");
        let text = String::from_utf8_lossy(&saved);
        assert!(text.contains("/Name /Approved") && text.contains("/Name /SignHere"));
        let mut again = PdfDocument::open(saved, standard_fonts()).expect("opens");
        let page = again.render(0, RenderOptions::default()).expect("renders");
        assert!(page.list.commands.len() > 1);
        assert!(d.undo());
        assert_eq!(d.annotations().len(), 1);
    }
}
