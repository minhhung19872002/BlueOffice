//! `.odp` slide text (ODF 1.3 Part 3, §10.1 `draw:page`, §10.4 frames).

use crate::dom::{self, Element};
use crate::ns::{DRAW, OFFICE, PRESENTATION, TEXT};
use crate::ods::paragraph_text;
use crate::package::OdfPackage;
use crate::{OdfError, Result, MIME_ODP};

/// Text content of a presentation (intermediate type).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Presentation {
    pub slides: Vec<Slide>,
}

/// One `draw:page`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Slide {
    /// `draw:name`.
    pub name: String,
    /// Shapes that carry text, in document order.
    pub frames: Vec<TextFrame>,
    /// Paragraphs of the speaker notes (`presentation:notes`).
    pub notes: Vec<String>,
}

/// A text-bearing shape.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextFrame {
    /// `presentation:class` (`title`, `outline`, `subtitle`, …) if the shape is a placeholder.
    pub class: Option<String>,
    /// One entry per paragraph (list items included).
    pub paragraphs: Vec<String>,
}

/// Collect `text:p`/`text:h` in document order, descending into lists.
fn collect_paragraphs(el: &Element, out: &mut Vec<String>, depth: usize) {
    if depth > dom::MAX_DEPTH {
        return;
    }
    for e in el.elements() {
        if e.is(TEXT, "p") || e.is(TEXT, "h") {
            out.push(paragraph_text(e));
        } else if e.ns == TEXT
            && matches!(
                e.name.as_str(),
                "list" | "list-item" | "list-header" | "section"
            )
        {
            collect_paragraphs(e, out, depth + 1);
        }
    }
}

fn shapes(el: &Element, frames: &mut Vec<TextFrame>, depth: usize) {
    if depth > dom::MAX_DEPTH {
        return;
    }
    for e in el.elements() {
        if e.ns != DRAW {
            continue;
        }
        match e.name.as_str() {
            // §10.3.15 draw:g groups
            "g" => shapes(e, frames, depth + 1),
            // §10.4.2 draw:frame → draw:text-box (§10.4.3)
            "frame" => {
                let mut paragraphs = Vec::new();
                for tb in e.children_named(DRAW, "text-box") {
                    collect_paragraphs(tb, &mut paragraphs, depth + 1);
                }
                if !paragraphs.is_empty() {
                    frames.push(TextFrame {
                        class: e.attr(PRESENTATION, "class").map(str::to_owned),
                        paragraphs,
                    });
                }
            }
            // Other shapes (custom-shape, rect, ellipse, …) hold text:p directly.
            _ => {
                let mut paragraphs = Vec::new();
                collect_paragraphs(e, &mut paragraphs, depth + 1);
                if !paragraphs.is_empty() {
                    frames.push(TextFrame {
                        class: e.attr(PRESENTATION, "class").map(str::to_owned),
                        paragraphs,
                    });
                }
            }
        }
    }
}

/// Read slide text from an `.odp` package.
pub fn read_odp(bytes: &[u8]) -> Result<Presentation> {
    let pkg = OdfPackage::read(bytes)?;
    pkg.expect_mimetype(MIME_ODP)?;
    let content = dom::parse(pkg.require("content.xml")?, "content.xml")?;
    let pres = content
        .child(OFFICE, "body")
        .and_then(|b| b.child(OFFICE, "presentation"))
        .ok_or_else(|| OdfError::Missing("office:body/office:presentation".into()))?;
    let mut out = Presentation::default();
    for page in pres.children_named(DRAW, "page") {
        let mut slide = Slide {
            name: page.attr(DRAW, "name").unwrap_or_default().to_owned(),
            ..Slide::default()
        };
        shapes(page, &mut slide.frames, 0);
        if let Some(notes) = page.child(PRESENTATION, "notes") {
            let mut frames = Vec::new();
            shapes(notes, &mut frames, 0);
            slide.notes = frames
                .into_iter()
                .filter(|f| f.class.as_deref() != Some("page"))
                .flat_map(|f| f.paragraphs)
                .collect();
        }
        out.slides.push(slide);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample_bytes() -> Vec<u8> {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {} xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0"><office:body><office:presentation>
<draw:page draw:name="Trang 1">
  <draw:frame presentation:class="title"><draw:text-box><text:p>Báo cáo   quý III</text:p></draw:text-box></draw:frame>
  <draw:frame presentation:class="outline"><draw:text-box><text:list><text:list-item><text:p>Doanh thu</text:p></text:list-item><text:list-item><text:p>Chi <text:span>phí</text:span></text:p></text:list-item></text:list></draw:text-box></draw:frame>
  <draw:frame><draw:image/></draw:frame>
  <draw:g><draw:custom-shape><text:p>Hình</text:p></draw:custom-shape></draw:g>
  <presentation:notes><draw:page-thumbnail/><draw:frame presentation:class="notes"><draw:text-box><text:p>Ghi chú</text:p></draw:text-box></draw:frame></presentation:notes>
</draw:page>
<draw:page draw:name="Trang 2"/>
</office:presentation></office:body></office:document-content>"#,
            crate::odt_write::XMLNS
        );
        OdfPackage::write(MIME_ODP, &[("content.xml", "text/xml", xml.into_bytes())]).unwrap()
    }

    #[test]
    fn reads_frames() {
        let p = read_odp(&sample_bytes()).unwrap();
        assert_eq!(p.slides.len(), 2);
        let s = &p.slides[0];
        assert_eq!(s.name, "Trang 1");
        assert_eq!(s.frames.len(), 3);
        assert_eq!(s.frames[0].class.as_deref(), Some("title"));
        assert_eq!(s.frames[0].paragraphs, vec!["Báo cáo quý III"]);
        assert_eq!(s.frames[1].paragraphs, vec!["Doanh thu", "Chi phí"]);
        assert_eq!(s.frames[2].paragraphs, vec!["Hình"]);
        assert_eq!(s.notes, vec!["Ghi chú"]);
        assert_eq!(p.slides[1].frames, vec![]);
    }

    #[test]
    fn wrong_type_rejected() {
        let odt = crate::write_odt(&bo_model::Document::default()).unwrap();
        assert!(matches!(read_odp(&odt), Err(OdfError::WrongType(_))));
    }
}
