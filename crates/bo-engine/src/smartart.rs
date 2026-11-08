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

/// Render a diagram drawing (`w`×`h` EMU) to PNG at 2 px per point.
pub fn render_png(xml: &[u8], w: i64, h: i64, fonts: &mut FontManager) -> Option<Vec<u8>> {
    let shapes = drawing_shapes(xml)?;
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
    s.contains("<dsp:drawing") || s.contains(":drawing ") && s.contains("diagram")
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
