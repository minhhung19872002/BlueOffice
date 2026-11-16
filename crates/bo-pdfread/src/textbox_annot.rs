//! A text box comment as BlueEditor types it in place (ISO 32000-1 §12.5.6.6 FreeText without an
//! intent): a white box with a coloured border and the typed lines in Helvetica. The defaults
//! the editor uses come from a file the reference editor saved (`docs/ui/REPORT-editor.md`
//! round 7): 9 pt text `#0000FF`, 1 pt border `#FF0000`, white fill, a 100 pt wide text area,
//! lines `1.116 em` apart with the first baseline `0.905 em` below the top.
use crate::annot::{nums, pdf_date, text_obj};
use crate::interp::{apply, invert};
use crate::PdfDocument;
use lopdf::{Dictionary, Object, Stream};
use std::fmt::Write as _;

/// Look of a text box comment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextBoxLook {
    /// Border colour.
    pub border: [u8; 3],
    /// Text colour.
    pub text: [u8; 3],
    /// Text size in points.
    pub size: f32,
    /// Border width in points (drawn outside the text area).
    pub width: f32,
}

impl Default for TextBoxLook {
    fn default() -> Self {
        Self {
            border: [255, 0, 0],
            text: [0, 0, 255],
            size: 9.0,
            width: 1.0,
        }
    }
}

impl TextBoxLook {
    /// Distance between baselines (points).
    #[must_use]
    pub fn line_height(&self) -> f32 {
        self.size * 1.116
    }

    /// First baseline below the top of the text area (points).
    #[must_use]
    pub fn first_baseline(&self) -> f32 {
        self.size * 0.905
    }
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn rgb(c: [u8; 3]) -> String {
    let [r, g, b] = c.map(|v| f32::from(v) / 255.0);
    format!("{r} {g} {b}")
}

/// A string in WinAnsi for a `Tj` (characters outside it become `?`).
fn literal(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '(' | ')' | '\\' => format!("\\{c}"),
            c if (c as u32) < 256 && !c.is_control() => c.to_string(),
            _ => "?".to_owned(),
        })
        .collect()
}

/// XML text with markup characters escaped.
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl PdfDocument {
    /// Add a text box comment on `page`: the text area's top-left `at` and width `width` (page
    /// points), `lines` as wrapped for it (one undo step).
    pub fn add_text_box(
        &mut self,
        page: usize,
        at: [f32; 2],
        width: f32,
        lines: &[String],
        look: &TextBoxLook,
        author: &str,
    ) -> Result<(), String> {
        if self.doc.is_encrypted() && self.doc.encryption_state.is_none() {
            return Err("comments cannot be added to an encrypted document".into());
        }
        let info = self.pages.get(page).ok_or("no such page")?.clone();
        let inv = invert(crate::page_matrix(info.crop, info.rotate)).ok_or("page matrix")?;
        let n = lines.len().max(1);
        #[allow(clippy::cast_precision_loss)]
        let h = look.line_height() * n as f32;
        let w = width.max(1.0);
        // Text area in user space (y up).
        let a = apply(inv, at[0], at[1]);
        let b = apply(inv, at[0] + w, at[1] + h);
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        let bw = look.width.max(0.0);
        let rect = [x0 - bw, y0 - bw, x1 + bw, y1 + bw];
        let mut s = format!("q 1 1 1 rg {} {} {} {} re f Q\n", x0, y0, x1 - x0, y1 - y0);
        if bw > 0.0 {
            let _ = writeln!(
                s,
                "q {} RG {bw} w {} {} {} {} re S Q",
                rgb(look.border),
                x0 - bw / 2.0,
                y0 - bw / 2.0,
                x1 - x0 + bw,
                y1 - y0 + bw
            );
        }
        let _ = writeln!(
            s,
            "q {x0} {y0} {} {} re W n BT {} rg /Helv {} Tf",
            x1 - x0,
            y1 - y0,
            rgb(look.text),
            look.size
        );
        for (i, line) in lines.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let y = y1 - look.first_baseline() - look.line_height() * i as f32;
            let _ = writeln!(s, "1 0 0 1 {x0} {y} Tm ({}) Tj", literal(line));
        }
        s.push_str("ET Q\n");
        let mut font = Dictionary::new();
        font.set("Type", Object::Name(b"Font".to_vec()));
        font.set("Subtype", Object::Name(b"Type1".to_vec()));
        font.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
        font.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        let mut fonts = Dictionary::new();
        fonts.set("Helv", font);
        let mut res = Dictionary::new();
        res.set("Font", fonts);
        let mut ap_dict = Dictionary::new();
        ap_dict.set("Type", Object::Name(b"XObject".to_vec()));
        ap_dict.set("Subtype", Object::Name(b"Form".to_vec()));
        ap_dict.set("BBox", nums(&rect));
        ap_dict.set("Resources", res);
        let text = lines.join("\n");
        self.begin("text box");
        let ap_id = self.add(Object::Stream(Stream::new(ap_dict, s.into_bytes())));
        let mut ap = Dictionary::new();
        ap.set("N", Object::Reference(ap_id));
        let now = pdf_date();
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"Annot".to_vec()));
        d.set("Subtype", Object::Name(b"FreeText".to_vec()));
        d.set("Rect", nums(&rect));
        d.set("F", 4);
        d.set("P", Object::Reference(info.id));
        d.set("T", text_obj(author));
        d.set("Subj", text_obj("Textbox"));
        d.set("Contents", text_obj(&text));
        d.set("M", text_obj(&now));
        d.set("CreationDate", text_obj(&now));
        d.set("NM", text_obj(&format!("bo-tb-{}-{now}", ap_id.0)));
        // Background (§12.5.6.6 /C for a free text annotation), border in /DA as the
        // reference writes it, text colour and font in the rich text (§12.7.3.4).
        d.set("C", nums(&[1.0, 1.0, 1.0]));
        d.set(
            "DA",
            text_obj(&format!("{} rg /Helv {} Tf", rgb(look.border), look.size)),
        );
        d.set(
            "DS",
            text_obj(&format!(
                "font: Helvetica,sans-serif {:.2}pt;color:{}",
                look.size,
                hex(look.border)
            )),
        );
        let mut body = String::new();
        for l in lines {
            let _ = write!(
                body,
                "<p dir=\"ltr\"><span style=\"font-size:{}pt;color:{};font-family:Helvetica\">{}</span></p>",
                look.size,
                hex(look.text),
                xml(l)
            );
        }
        d.set(
            "RC",
            text_obj(&format!(
                "<?xml version=\"1.0\"?><body xmlns=\"http://www.w3.org/1999/xhtml\" \
                 xmlns:xfa=\"http://www.xfa.org/schema/xfa-data/1.0/\" xfa:spec=\"2.0.2\">{body}</body>"
            )),
        );
        let mut bs = Dictionary::new();
        bs.set("S", Object::Name(b"S".to_vec()));
        bs.set("W", Object::Real(bw));
        d.set("BS", bs);
        d.set("Q", 0);
        d.set("AP", ap);
        let id = self.add(Object::Dictionary(d));
        let r = self.link_text_box(info.id, id);
        self.commit();
        r
    }

    /// Append annotation `id` to the `/Annots` of page object `page`.
    fn link_text_box(&mut self, page: lopdf::ObjectId, id: lopdf::ObjectId) -> Result<(), String> {
        if let Ok(Object::Reference(arr)) = self
            .doc
            .get_dictionary(page)
            .map_err(|e| e.to_string())?
            .get(b"Annots")
        {
            let arr = *arr;
            self.touch(arr);
            if let Ok(Object::Array(v)) = self.doc.get_object_mut(arr) {
                v.push(Object::Reference(id));
            }
            return Ok(());
        }
        self.touch(page);
        let p = self
            .doc
            .get_dictionary_mut(page)
            .map_err(|e| e.to_string())?;
        match p.get_mut(b"Annots") {
            Ok(Object::Array(v)) => v.push(Object::Reference(id)),
            _ => p.set("Annots", vec![Object::Reference(id)]),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{standard_fonts, AnnotKind, PdfDocument, TextBoxLook};

    #[test]
    fn a_text_box_is_a_free_text_comment_with_the_reference_defaults() {
        let bytes = crate::tests::build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] >>",
        ]);
        let mut doc = PdfDocument::open(bytes, standard_fonts()).expect("opens");
        let look = TextBoxLook::default();
        let lines = vec!["Text box".to_owned(), "two (lines)".to_owned()];
        doc.add_text_box(0, [50.0, 40.0], 100.0, &lines, &look, "me")
            .expect("adds");
        let a = doc.annotations();
        let a = a.last().expect("comment");
        assert_eq!(a.kind, AnnotKind::TextBox);
        assert_eq!(a.contents, "Text box\ntwo (lines)");
        // 100 pt text area, two 10.044 pt lines, 1 pt border outside: 102 x 22.088.
        let r = a.rect;
        assert!((r[2] - r[0] - 102.0).abs() < 0.01, "{r:?}");
        assert!((r[3] - r[1] - 22.088).abs() < 0.01, "{r:?}");
        assert!(
            (r[0] - 49.0).abs() < 0.01 && (r[1] - 39.0).abs() < 0.01,
            "{r:?}"
        );
        let saved = doc.save().expect("saves");
        let s = String::from_utf8_lossy(&saved);
        assert!(s.contains("/Subj (Textbox)"), "{s}");
        assert!(s.contains("color:#0000FF"));
        // Shown on the page with its own appearance (one undo step).
        assert!(doc.undo());
        assert_eq!(doc.annotations().len(), 0);
    }
}
