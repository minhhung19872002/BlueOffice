//! Display list → PDF.
//!
//! Fonts are embedded as `CIDFontType2` (TrueType outlines) with `Identity-H`
//! encoding, subsetted with `subsetter`. A `ToUnicode` CMap built from the
//! glyph clusters keeps the text searchable and copyable.
//! Reference: ISO 32000-1:2008 §9.7 (Composite fonts), §9.10.3 (ToUnicode).
#![forbid(unsafe_code)]

use bo_render::{Command, DisplayList, Rgba};
use bo_text::{FaceId, FontManager};
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str};
use std::collections::{BTreeMap, HashMap};

/// PDF export errors.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("font subsetting failed for face {0}: {1}")]
    Subset(u32, String),
}

/// Metadata written to the document information dictionary.
#[derive(Debug, Clone, Default)]
pub struct PdfMeta {
    pub title: Option<String>,
}

struct FontUse {
    remapper: subsetter::GlyphRemapper,
    /// new gid → unicode text
    to_unicode: BTreeMap<u16, String>,
    font_ref: Ref,
}

fn compress(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

/// Write the display list as a PDF document.
pub fn write_pdf(list: &DisplayList, fonts: &FontManager, meta: &PdfMeta) -> Result<Vec<u8>, PdfError> {
    let mut pdf = Pdf::new();
    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog_id = alloc();
    let pages_id = alloc();
    let info_id = alloc();

    // Pass 1: collect glyph usage per face.
    let mut uses: HashMap<u32, FontUse> = HashMap::new();
    let mut face_order: Vec<u32> = Vec::new();
    for page in &list.pages {
        for cmd in &page.commands {
            if let Command::Glyphs { face, text, glyphs, .. } = cmd {
                let u = uses.entry(*face).or_insert_with(|| {
                    face_order.push(*face);
                    let mut remapper = subsetter::GlyphRemapper::new();
                    remapper.remap(0); // .notdef must stay glyph 0
                    FontUse { remapper, to_unicode: BTreeMap::new(), font_ref: Ref::new(1) }
                });
                for (i, g) in glyphs.iter().enumerate() {
                    let new = u.remapper.remap(g.0);
                    let start = g.3 as usize;
                    let end = glyphs
                        .iter()
                        .skip(i + 1)
                        .map(|n| n.3 as usize)
                        .find(|&c| c > start)
                        .unwrap_or(text.len());
                    if let Some(s) = text.get(start..end.max(start)) {
                        // Ligature/cluster: only the first glyph of a cluster carries text.
                        let first_of_cluster = i == 0 || glyphs[i - 1].3 != g.3;
                        if first_of_cluster && !s.is_empty() {
                            u.to_unicode.entry(new).or_insert_with(|| s.to_owned());
                        }
                    }
                }
            }
        }
    }
    for f in &face_order {
        uses.get_mut(f).expect("face registered").font_ref = alloc();
    }

    // Pages.
    let mut page_ids = Vec::new();
    for page in &list.pages {
        let page_id = alloc();
        let content_id = alloc();
        page_ids.push(page_id);
        let h = page.height;
        let mut c = Content::new();
        let mut used_fonts: Vec<u32> = Vec::new();
        for cmd in &page.commands {
            match cmd {
                Command::FillRect { x, y, w, h: rh, color } => {
                    set_fill(&mut c, *color);
                    c.rect(*x, h - y - rh, *w, *rh);
                    c.fill_nonzero();
                }
                Command::Line { x1, y1, x2, y2, width, color } => {
                    c.set_stroke_rgb(f32::from(color.0) / 255.0, f32::from(color.1) / 255.0, f32::from(color.2) / 255.0);
                    c.set_line_width(*width);
                    c.move_to(*x1, h - y1);
                    c.line_to(*x2, h - y2);
                    c.stroke();
                }
                Command::Glyphs { face, size, x, y, color, glyphs, .. } => {
                    let u = &uses[face];
                    if !used_fonts.contains(face) {
                        used_fonts.push(*face);
                    }
                    set_fill(&mut c, *color);
                    c.begin_text();
                    let name = format!("F{face}");
                    c.set_font(Name(name.as_bytes()), *size);
                    for g in glyphs {
                        let new = u.remapper.get(g.0).unwrap_or(0);
                        c.set_text_matrix([1.0, 0.0, 0.0, 1.0, x + g.1, h - (y + g.2)]);
                        c.show(Str(&new.to_be_bytes()));
                    }
                    c.end_text();
                }
                Command::PushClip { x, y, w, h: rh } => {
                    c.save_state();
                    c.rect(*x, h - y - rh, *w, *rh);
                    c.clip_nonzero();
                    c.end_path();
                }
                Command::PopClip => {
                    c.restore_state();
                }
            }
        }
        let data = compress(&c.finish());
        pdf.stream(content_id, &data).filter(Filter::FlateDecode);
        let mut p = pdf.page(page_id);
        p.media_box(Rect::new(0.0, 0.0, page.width, page.height));
        p.parent(pages_id);
        p.contents(content_id);
        let mut res = p.resources();
        let mut fdict = res.fonts();
        for f in &used_fonts {
            fdict.pair(Name(format!("F{f}").as_bytes()), uses[f].font_ref);
        }
        fdict.finish();
        res.finish();
        p.finish();
    }

    // Fonts.
    for face in &face_order {
        let u = &uses[face];
        let id = FaceId(*face);
        let info = fonts.info(id);
        let (data, index) = fonts.data(id);
        let subset = subsetter::subset(data, index, &u.remapper)
            .map_err(|e| PdfError::Subset(*face, format!("{e:?}")))?;
        let upem = f32::from(info.units_per_em);
        let to_pdf = |v: f32| v * 1000.0 / upem;
        let base = format!("{}+{}", subset_tag(*face), info.family.replace(' ', ""));
        let cid_id = alloc();
        let desc_id = alloc();
        let file_id = alloc();
        let cmap_id = alloc();

        pdf.type0_font(u.font_ref)
            .base_font(Name(base.as_bytes()))
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(cid_id)
            .to_unicode(cmap_id);

        // Widths of the subset glyphs (new gid order).
        let face_parsed = fonts.face(id);
        let mut widths = Vec::new();
        for old in u.remapper.remapped_gids() {
            let adv = face_parsed
                .as_ref()
                .and_then(|f| f.glyph_hor_advance(bo_text::ttf::GlyphId(old)))
                .unwrap_or(0);
            widths.push(to_pdf(f32::from(adv)));
        }
        let system_info = SystemInfo {
            registry: Str(b"Adobe"),
            ordering: Str(b"Identity"),
            supplement: 0,
        };
        let mut cid = pdf.cid_font(cid_id);
        cid.subtype(CidFontType::Type2)
            .base_font(Name(base.as_bytes()))
            .system_info(system_info)
            .font_descriptor(desc_id)
            .default_width(0.0)
            .cid_to_gid_map_predefined(Name(b"Identity"));
        cid.widths().consecutive(0, widths.iter().copied());
        cid.finish();

        let bbox = face_parsed.as_ref().map_or(Rect::new(0.0, -200.0, 1000.0, 900.0), |f| {
            let b = f.global_bounding_box();
            Rect::new(
                to_pdf(f32::from(b.x_min)),
                to_pdf(f32::from(b.y_min)),
                to_pdf(f32::from(b.x_max)),
                to_pdf(f32::from(b.y_max)),
            )
        });
        let mut flags = FontFlags::NON_SYMBOLIC;
        if info.italic {
            flags |= FontFlags::ITALIC;
        }
        pdf.font_descriptor(desc_id)
            .name(Name(base.as_bytes()))
            .flags(flags)
            .bbox(bbox)
            .italic_angle(if info.italic { -12.0 } else { 0.0 })
            .ascent(to_pdf(f32::from(info.ascender)))
            .descent(to_pdf(f32::from(info.descender)))
            .cap_height(to_pdf(f32::from(info.ascender)) * 0.7)
            .stem_v(if info.bold { 140.0 } else { 80.0 })
            .font_file2(file_id);
        let compressed = compress(&subset);
        pdf.stream(file_id, &compressed)
            .filter(Filter::FlateDecode)
            .pair(Name(b"Length1"), subset.len() as i32);

        let mut cmap = UnicodeCmap::new(Name(b"Custom"), system_info);
        for (gid, s) in &u.to_unicode {
            cmap.pair_with_multiple(*gid, s.chars());
        }
        pdf.stream(cmap_id, &cmap.finish());
    }

    pdf.pages(pages_id).kids(page_ids.iter().copied()).count(page_ids.len() as i32);
    pdf.catalog(catalog_id).pages(pages_id);
    let mut info = pdf.document_info(info_id);
    info.producer(pdf_writer::TextStr("BlueOffice"));
    if let Some(t) = &meta.title {
        info.title(pdf_writer::TextStr(t));
    }
    info.finish();
    Ok(pdf.finish())
}

fn set_fill(c: &mut Content, color: Rgba) {
    c.set_fill_rgb(f32::from(color.0) / 255.0, f32::from(color.1) / 255.0, f32::from(color.2) / 255.0);
}

/// Six-letter subset tag (ISO 32000-1 §9.6.4).
fn subset_tag(face: u32) -> String {
    let mut n = face.wrapping_mul(2_654_435_761);
    (0..6)
        .map(|_| {
            let c = (b'A' + (n % 26) as u8) as char;
            n /= 26;
            c
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_render::PageList;

    #[test]
    fn empty_document_is_valid_pdf() {
        let list = DisplayList {
            pages: vec![PageList { width: 595.0, height: 842.0, commands: vec![] }],
        };
        let bytes = write_pdf(&list, &FontManager::new(), &PdfMeta::default()).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("/MediaBox [0 0 595 842]"));
        assert!(s.trim_end().ends_with("%%EOF"));
    }

    #[test]
    fn subset_tags_are_uppercase() {
        let t = subset_tag(3);
        assert_eq!(t.len(), 6);
        assert!(t.chars().all(|c| c.is_ascii_uppercase()));
    }
}
