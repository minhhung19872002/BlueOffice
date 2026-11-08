//! SmartArt in BlueDoc: the diagram's pre-rendered drawing part ([MS-ODRAWXML]
//! §2.3.2 `dsp:drawing`, shapes in DrawingML, ECMA-376 Part 1 §20.1) is drawn with
//! the BlueSlide shape renderer and shown as a picture. The part itself is kept
//! verbatim in the package (the image only lives in memory).

use bo_model::slide::{CommonSlideData, Presentation, ShapeNode, Slide, SlideSize};
use bo_model::{Block, Document, RunContent};
use bo_text::FontManager;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Shapes of a diagram drawing part.
pub fn drawing_shapes(xml: &[u8]) -> Option<Vec<ShapeNode>> {
    bo_pptx::diagram::drawing_shapes(xml, "/word/diagrams/drawing.xml")
}

/// Render a diagram drawing (`w`×`h` EMU) to PNG at 2 px per point; a data model part (SmartArt saved
/// without its drawing) is laid out first (`bo_pptx::diagram_layout`).
pub fn render_png(xml: &[u8], w: i64, h: i64, fonts: &mut FontManager) -> Option<Vec<u8>> {
    let shapes = bo_pptx::diagram::diagram_shapes(xml, "/word/diagrams/drawing.xml", w, h)?;
    if shapes.is_empty() || w <= 0 || h <= 0 {
        return None;
    }
    let pres = Presentation {
        slide_size: SlideSize {
            cx: w,
            cy: h,
            kind: None,
        },
        slides: vec![Slide {
            data: CommonSlideData {
                shapes,
                ..CommonSlideData::default()
            },
            ..Slide::default()
        }],
        ..Presentation::default()
    };
    let mut images = BTreeMap::new();
    let page = bo_pptx::render_slide(&pres, 0, fonts, &mut images).ok()?;
    bo_raster::render_page_png_with(&page, fonts, 2.0, &images).ok()
}

fn is_drawing_part(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(512)];
    let s = String::from_utf8_lossy(head);
    s.contains("<dsp:drawing")
        || s.contains(":drawing ") && s.contains("diagram")
        || s.contains(":dataModel ") && s.contains("drawingml/2006/diagram")
}

/// Replace the media of SmartArt pictures by a rendering of their drawing part.
pub fn render_smartart(doc: &mut Document, fonts: &mut FontManager) {
    fn visit(blocks: &[Block], sizes: &mut BTreeMap<String, (i64, i64)>) {
        for b in blocks {
            match b {
                Block::Paragraph(p) => {
                    for r in &p.runs {
                        for c in &r.content {
                            if let RunContent::Image(img) = c {
                                sizes
                                    .entry(img.media.clone())
                                    .or_insert((img.width.0, img.height.0));
                            }
                        }
                    }
                }
                Block::Table(t) => {
                    for row in &t.rows {
                        for cell in &row.cells {
                            visit(&cell.blocks, sizes);
                        }
                    }
                }
                Block::Opaque(_) => {}
            }
        }
    }
    let mut sizes: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for s in &doc.sections {
        visit(&s.blocks, &mut sizes);
    }
    for (name, (w, h)) in sizes {
        let Some(bytes) = doc.media.get(&name) else {
            continue;
        };
        if !is_drawing_part(bytes) {
            continue;
        }
        if let Some(png) = render_png(bytes, w, h, fonts) {
            doc.media.insert(name, Arc::new(png));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    // source: corpus smartart-04-datamodel.docx (data model only, ECMA-376 Part 1 §21.4.3) — CORE-SMART-002.
    #[test]
    fn data_model_smartart_is_laid_out_and_drawn() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/corpus/doc/smartart-04-datamodel.docx"
        );
        let mut doc = bo_docx::read_docx(&std::fs::read(path).unwrap()).unwrap();
        // The picture of the frame is the data model part, kept verbatim.
        let (name, bytes) = doc
            .media
            .iter()
            .find(|(n, _)| n.ends_with("data1.xml"))
            .map(|(n, b)| (n.clone(), b.clone()))
            .unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("dgm:dataModel"));
        let mut fonts = bo_text::FontManager::new();
        for f in crate::bundled::FONTS {
            fonts.add_font(f.to_vec());
        }
        super::render_smartart(&mut doc, &mut fonts);
        let png = &doc.media[&name];
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // The saved file keeps the data part as it was.
        let saved =
            bo_docx::write_docx(&bo_docx::read_docx(&std::fs::read(path).unwrap()).unwrap())
                .unwrap();
        let back = bo_docx::read_docx(&saved).unwrap();
        assert_eq!(back.media[&name].as_slice(), bytes.as_slice());
    }
}
