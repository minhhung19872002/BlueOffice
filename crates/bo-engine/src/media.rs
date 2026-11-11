//! Pictures the display list backends cannot draw directly: GIF, BMP and TIFF are
//! re-encoded as PNG, EMF/WMF metafiles ([MS-EMF], [MS-WMF]) are converted to a
//! display list and rasterized at 2 px per point. Only the in-memory media change:
//! the parts are written back unchanged when the document is saved (the writer keeps
//! existing package parts).

use bo_model::{Block, Document, RunContent};
use bo_render::metafile;
use bo_text::FontManager;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Every picture of the document (body, text boxes, headers, footers, notes) that shows a media
/// part, in document order.
pub(crate) fn pictures(doc: &Document) -> Vec<&bo_model::Image> {
    fn visit<'a>(blocks: &'a [Block], out: &mut Vec<&'a bo_model::Image>) {
        for b in blocks {
            match b {
                Block::Paragraph(p) => {
                    for c in p.runs.iter().flat_map(|r| &r.content) {
                        if let RunContent::Image(img) = c {
                            if !img.media.is_empty() {
                                out.push(img);
                            }
                            if let Some(t) = img.shape.as_ref().and_then(|s| s.text.as_ref()) {
                                visit(t, out);
                            }
                        }
                    }
                }
                Block::Table(t) => {
                    for c in t.rows.iter().flat_map(|r| &r.cells) {
                        visit(&c.blocks, out);
                    }
                }
                Block::Opaque(_) => {}
            }
        }
    }
    let mut out = Vec::new();
    for s in &doc.sections {
        visit(&s.blocks, &mut out);
        for set in [&s.headers, &s.footers] {
            for b in [&set.default, &set.first, &set.even].into_iter().flatten() {
                visit(b, &mut out);
            }
        }
    }
    for b in doc.footnotes.values().chain(doc.endnotes.values()) {
        visit(b, &mut out);
    }
    out
}

/// Picture size (EMU) of every media part referenced by a picture.
pub(crate) fn sizes(doc: &Document) -> BTreeMap<String, (i64, i64)> {
    let mut out = BTreeMap::new();
    for img in pictures(doc) {
        out.entry(img.media.clone())
            .or_insert((img.width.0, img.height.0));
    }
    out
}

/// PNG for picture bytes the backends cannot draw (`None`: keep as is).
pub fn to_png(bytes: &[u8], w_emu: i64, h_emu: i64, fonts: &mut FontManager) -> Option<Vec<u8>> {
    if let Some(png) = bo_raster::transcode_to_png(bytes) {
        return Some(png);
    }
    let (w, h) = (
        (w_emu as f32 / 12_700.0).clamp(1.0, 2000.0),
        (h_emu as f32 / 12_700.0).clamp(1.0, 2000.0),
    );
    let conv = if metafile::is_emf(bytes) {
        metafile::emf_to_display_list(bytes, w, h, fonts)?
    } else if metafile::is_wmf(bytes) {
        metafile::wmf_to_display_list(bytes, w, h, fonts)?
    } else {
        return None;
    };
    bo_raster::render_page_png_with(&conv.page, fonts, 2.0, &conv.images).ok()
}

/// Replace GIF/BMP/TIFF/EMF/WMF media of `doc` by PNG renderings.
pub fn convert_media(doc: &mut Document, fonts: &mut FontManager) {
    for (name, (w, h)) in sizes(doc) {
        let Some(bytes) = doc.media.get(&name) else {
            continue;
        };
        if bytes.starts_with(b"\x89PNG") || bytes.starts_with(&[0xFF, 0xD8]) {
            continue;
        }
        if let Some(png) = to_png(bytes, w, h, fonts) {
            doc.media.insert(name, Arc::new(png));
        }
    }
}
