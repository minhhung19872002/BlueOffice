//! `.odp` slide text (ODF 1.3 Part 3, §10.1 `draw:page`, §10.4 frames): read and write.

use crate::dom::{self, Element};
use crate::ns::{DRAW, OFFICE, PRESENTATION, TEXT};
use crate::ods::{paragraph_text, write_text};
use crate::package::{meta_xml, OdfPackage};
use crate::{OdfError, Result, MIME_ODP};

/// Text content of a presentation (intermediate type).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Presentation {
    /// Slides in document order.
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

const NS_PRESENTATION: &str =
    r#"xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0""#;

/// Page size written by [`write_odp`]: 16:9, 28 cm × 15.75 cm.
const PAGE_W_CM: f64 = 28.0;
const PAGE_H_CM: f64 = 15.75;

/// Paragraphs as `text:p` elements; `\n` / U+2028 become `text:line-break` (§6.1.5).
fn write_paragraphs(out: &mut String, paragraphs: &[String]) {
    for p in paragraphs {
        out.push_str("<text:p>");
        for (i, line) in p.split(['\n', '\u{2028}']).enumerate() {
            if i > 0 {
                out.push_str("<text:line-break/>");
            }
            write_text(out, line);
        }
        out.push_str("</text:p>");
    }
}

/// One `draw:frame` with a `draw:text-box` (§10.4.2, §10.4.3) at a vertical slot.
fn write_frame(out: &mut String, frame: &TextFrame, y_cm: f64, h_cm: f64) {
    out.push_str("<draw:frame");
    if let Some(c) = &frame.class {
        // §19.392 presentation:class
        out.push_str(&format!(r#" presentation:class="{}""#, dom::escape(c)));
    }
    out.push_str(&format!(
        r#" svg:x="1cm" svg:y="{y_cm:.2}cm" svg:width="{:.2}cm" svg:height="{h_cm:.2}cm"><draw:text-box>"#,
        PAGE_W_CM - 2.0
    ));
    write_paragraphs(out, &frame.paragraphs);
    out.push_str("</draw:text-box></draw:frame>");
}

/// Serialize slide text as an `.odp` package (ODF 1.3): one `draw:page` per
/// slide on a single master page, frames stacked top to bottom, notes in
/// `presentation:notes`.
pub fn write_odp(pres: &Presentation) -> Result<Vec<u8>> {
    let mut content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-content {} {NS_PRESENTATION}><office:body><office:presentation>",
        crate::odt_write::XMLNS
    );
    for (i, slide) in pres.slides.iter().enumerate() {
        let name = if slide.name.is_empty() {
            format!("page{}", i + 1)
        } else {
            slide.name.clone()
        };
        // §10.1 draw:page requires draw:master-page-name
        content.push_str(&format!(
            r#"<draw:page draw:name="{}" draw:master-page-name="Default">"#,
            dom::escape(&name)
        ));
        let n = slide.frames.len().max(1) as f64;
        let slot = (PAGE_H_CM - 1.0) / n;
        for (k, f) in slide.frames.iter().enumerate() {
            write_frame(&mut content, f, 0.5 + slot * k as f64, slot);
        }
        if !slide.notes.is_empty() {
            // §16.33 presentation:notes
            content.push_str(r#"<presentation:notes><draw:page-thumbnail presentation:class="page" svg:x="2cm" svg:y="1cm" svg:width="16cm" svg:height="9cm"/>"#);
            write_frame(
                &mut content,
                &TextFrame {
                    class: Some("notes".into()),
                    paragraphs: slide.notes.clone(),
                },
                11.0,
                12.0,
            );
            content.push_str("</presentation:notes>");
        }
        content.push_str("</draw:page>");
    }
    content.push_str("</office:presentation></office:body></office:document-content>");

    // styles.xml: page layout (§16.5) and the master page (§16.9).
    let styles = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-styles {} {NS_PRESENTATION}><office:styles/><office:automatic-styles><style:page-layout style:name=\"PM1\"><style:page-layout-properties fo:page-width=\"{PAGE_W_CM}cm\" fo:page-height=\"{PAGE_H_CM}cm\" style:print-orientation=\"landscape\"/></style:page-layout></office:automatic-styles><office:master-styles><style:master-page style:name=\"Default\" style:page-layout-name=\"PM1\"/></office:master-styles></office:document-styles>",
        crate::odt_write::XMLNS
    );
    OdfPackage::write(
        MIME_ODP,
        &[
            ("content.xml", "text/xml", content.into_bytes()),
            ("styles.xml", "text/xml", styles.into_bytes()),
            ("meta.xml", "text/xml", meta_xml()),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_read_round_trip() {
        let pres = Presentation {
            slides: vec![
                Slide {
                    name: "Mở đầu".into(),
                    frames: vec![
                        TextFrame {
                            class: Some("title".into()),
                            paragraphs: vec!["Báo cáo <năm> & \"kế hoạch\"".into()],
                        },
                        TextFrame {
                            class: Some("outline".into()),
                            paragraphs: vec![
                                "Một".into(),
                                "Hai  cách\tnhau".into(),
                                "dòng 1\ndòng 2".into(),
                            ],
                        },
                    ],
                    notes: vec!["Ghi chú cho người trình bày".into()],
                },
                Slide {
                    name: String::new(),
                    frames: vec![TextFrame {
                        class: None,
                        paragraphs: vec!["Hộp chữ".into()],
                    }],
                    notes: Vec::new(),
                },
            ],
        };
        let bytes = write_odp(&pres).unwrap();
        assert_eq!(crate::detect_mimetype(&bytes).as_deref(), Some(MIME_ODP));
        let back = read_odp(&bytes).unwrap();
        let mut expected = pres.clone();
        expected.slides[1].name = "page2".into();
        assert_eq!(back, expected);
        // A second round trip is a fixed point.
        assert_eq!(read_odp(&write_odp(&back).unwrap()).unwrap(), back);
    }

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
