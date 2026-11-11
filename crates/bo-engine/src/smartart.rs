//! SmartArt in BlueDoc: the diagram's drawing part ([MS-ODRAWXML] §2.3.2 `dsp:drawing`, shapes in
//! DrawingML, ECMA-376 Part 1 §20.1) — or, without one, its data model laid out by
//! `bo_pptx::diagram_layout` — is drawn with the BlueSlide shape renderer and shown as a picture.
//! The pictures live in [`SmartArtImages`], next to the document: the parts themselves stay in the
//! document's media as XML (kept verbatim when read, written as they are for new / edited
//! diagrams, CORE-SMART-005), so undo and save see the real parts.

use bo_model::diagram;
use bo_model::slide::{CommonSlideData, Presentation, ShapeNode, Slide, SlideSize};
use bo_model::Document;
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

/// One rendered diagram or chart: the part bytes it was drawn from, the PNG and, for a chart, its
/// vector drawing (page of the frame's size).
#[derive(Debug, Clone, Default)]
struct Rendered {
    source: Arc<Vec<u8>>,
    size: (i64, i64),
    png: Vec<u8>,
    vector: Option<bo_render::PageList>,
}

/// Is a media part one this cache draws (a SmartArt diagram part or a chart part)?
fn drawn(name: &str, bytes: &[u8]) -> bool {
    (diagram::split_name(name).is_some() && diagram::is_diagram_xml(bytes))
        || (bo_model::chart_part::is_chart_name(name) && bo_model::chart_part::is_chart_xml(bytes))
}

/// Pictures of the SmartArt diagrams and charts of the open document, by media part name.
#[derive(Debug, Clone, Default)]
pub struct SmartArtImages {
    map: BTreeMap<String, Rendered>,
    /// Diagram media (name, bytes address) the pictures were drawn for.
    key: Vec<(String, usize)>,
}

impl SmartArtImages {
    /// The picture of diagram part `name`.
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.map.get(name).map(|r| r.png.as_slice())
    }

    /// The vector drawing of chart part `name` (a page of the frame's size).
    pub fn vector(&self, name: &str) -> Option<&bo_render::PageList> {
        self.map.get(name).and_then(|r| r.vector.as_ref())
    }

    /// Replace the pictures of charts in `list` by their vector drawing, scaled to the picture box.
    pub fn vectorize(&self, list: &mut bo_render::PageList) {
        use bo_render::Command;
        if !list
            .commands
            .iter()
            .any(|c| matches!(c, Command::Image { image, .. } if self.vector(image).is_some()))
        {
            return;
        }
        let mut out = Vec::with_capacity(list.commands.len());
        for c in std::mem::take(&mut list.commands) {
            match &c {
                Command::Image {
                    x,
                    y,
                    w,
                    h,
                    image,
                    crop: None,
                    rotation,
                } if *rotation == 0.0 => {
                    if let Some(v) = self
                        .vector(image)
                        .filter(|v| v.width > 0.0 && v.height > 0.0)
                    {
                        out.push(Command::PushClip {
                            x: *x,
                            y: *y,
                            w: *w,
                            h: *h,
                        });
                        out.push(Command::PushTransform {
                            m: [w / v.width, 0.0, 0.0, h / v.height, *x, *y],
                        });
                        out.extend(v.commands.iter().cloned());
                        out.push(Command::PopTransform);
                        out.push(Command::PopClip);
                        continue;
                    }
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        list.commands = out;
    }

    /// Forget every picture (another document is opened).
    pub fn clear(&mut self) {
        *self = SmartArtImages::default();
    }

    /// Draw the diagrams of `doc` whose parts changed since the last call.
    pub fn refresh(&mut self, doc: &Document, fonts: &mut FontManager) {
        let key: Vec<(String, usize)> = doc
            .media
            .iter()
            .filter(|(n, b)| drawn(n, b))
            .map(|(n, b)| (n.clone(), Arc::as_ptr(b) as usize))
            .collect();
        if key == self.key {
            return;
        }
        self.key = key;
        let mut next = BTreeMap::new();
        // The document theme gives the chart colours; read once, when a chart is drawn.
        let mut theme: Option<Option<bo_model::slide::Theme>> = None;
        for (name, (w, h)) in crate::media::sizes(doc) {
            let Some(bytes) = doc.media.get(&name) else {
                continue;
            };
            if !drawn(&name, bytes) {
                continue;
            }
            match self.map.remove(&name) {
                Some(r) if Arc::ptr_eq(&r.source, bytes) && r.size == (w, h) => {
                    next.insert(name, r);
                }
                _ if diagram::is_diagram_xml(bytes) => {
                    if let Some(png) = render_png(bytes, w, h, fonts) {
                        next.insert(
                            name,
                            Rendered {
                                source: bytes.clone(),
                                size: (w, h),
                                png,
                                vector: None,
                            },
                        );
                    }
                }
                _ => {
                    let theme = theme.get_or_insert_with(|| crate::doc_chart::doc_theme(doc));
                    let Some(page) = crate::doc_chart::render(theme.as_ref(), bytes, w, h, fonts)
                    else {
                        continue;
                    };
                    let png = bo_raster::render_page_png_with(
                        &page,
                        fonts,
                        2.0,
                        &BTreeMap::<String, Vec<u8>>::new(),
                    )
                    .unwrap_or_default();
                    next.insert(
                        name,
                        Rendered {
                            source: bytes.clone(),
                            size: (w, h),
                            png,
                            vector: Some(page),
                        },
                    );
                }
            }
        }
        self.map = next;
    }
}

impl bo_render::ImageSource for SmartArtImages {
    fn image(&self, id: &str) -> Option<&[u8]> {
        self.get(id)
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
        let doc = bo_docx::read_docx(&std::fs::read(path).unwrap()).unwrap();
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
        let mut images = super::SmartArtImages::default();
        images.refresh(&doc, &mut fonts);
        let png = images.get(&name).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // The document's media keep the part as XML; so does the saved file.
        assert_eq!(doc.media[&name].as_slice(), bytes.as_slice());
        let saved = bo_docx::write_docx(&doc).unwrap();
        let back = bo_docx::read_docx(&saved).unwrap();
        assert_eq!(back.media[&name].as_slice(), bytes.as_slice());
    }
}
