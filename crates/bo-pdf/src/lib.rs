//! Display list → PDF.
//!
//! Fonts are embedded as `CIDFontType2` (TrueType outlines) with `Identity-H`
//! encoding, subsetted with `subsetter`. A `ToUnicode` CMap built from the
//! glyph clusters keeps the text searchable and copyable.
//! Reference: ISO 32000-1:2008 §9.7 (Composite fonts), §9.10.3 (ToUnicode).
#![forbid(unsafe_code)]

use bo_render::{Command, DisplayList, GradientStop, ImageSource, Paint, PathSeg, Rgba, Stroke};
use bo_text::{FaceId, FontManager};
use pdf_writer::types::{
    CidFontType, FontFlags, FunctionShadingType, LineCapStyle, LineJoinStyle, SystemInfo,
    UnicodeCmap,
};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str};
use std::collections::{BTreeMap, HashMap};

/// PDF export errors.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    /// Subsetting the font with the given face id failed.
    #[error("font subsetting failed for face {0}: {1}")]
    Subset(u32, String),
}

/// Metadata written to the document information dictionary.
#[derive(Debug, Clone, Default)]
pub struct PdfMeta {
    /// `/Title` entry.
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
pub fn write_pdf(
    list: &DisplayList,
    fonts: &FontManager,
    meta: &PdfMeta,
) -> Result<Vec<u8>, PdfError> {
    write_pdf_with_images(list, fonts, meta, &BTreeMap::<String, Vec<u8>>::new())
}

/// Write the display list as PDF, embedding the PNG/JPEG bytes that `Image`
/// commands reference by id.
pub fn write_pdf_with_images(
    list: &DisplayList,
    fonts: &FontManager,
    meta: &PdfMeta,
    images: &dyn ImageSource,
) -> Result<Vec<u8>, PdfError> {
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
            if let Command::Glyphs {
                face, text, glyphs, ..
            } = cmd
            {
                let u = uses.entry(*face).or_insert_with(|| {
                    face_order.push(*face);
                    let mut remapper = subsetter::GlyphRemapper::new();
                    remapper.remap(0); // .notdef must stay glyph 0
                    FontUse {
                        remapper,
                        to_unicode: BTreeMap::new(),
                        font_ref: Ref::new(1),
                    }
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
    let mut image_refs: HashMap<String, Option<Ref>> = HashMap::new();
    let mut alpha_refs: BTreeMap<u8, Ref> = BTreeMap::new();
    for page in &list.pages {
        let page_id = alloc();
        let content_id = alloc();
        page_ids.push(page_id);
        let h = page.height;
        let mut c = Content::new();
        let mut used_fonts: Vec<u32> = Vec::new();
        let mut res = PageRes::default();
        let mut pending: Vec<PendingShading> = Vec::new();
        for cmd in &page.commands {
            match cmd {
                Command::FillRect {
                    x,
                    y,
                    w,
                    h: rh,
                    color,
                } => {
                    set_fill(&mut c, *color);
                    c.rect(*x, h - y - rh, *w, *rh);
                    c.fill_nonzero();
                }
                Command::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    color,
                    dash,
                } => {
                    c.set_stroke_rgb(
                        f32::from(color.0) / 255.0,
                        f32::from(color.1) / 255.0,
                        f32::from(color.2) / 255.0,
                    );
                    c.set_line_width(*width);
                    if let Some([on, off]) = dash {
                        c.set_dash_pattern([*on, *off], 0.0);
                    }
                    c.move_to(*x1, h - y1);
                    c.line_to(*x2, h - y2);
                    c.stroke();
                    if dash.is_some() {
                        c.set_dash_pattern([], 0.0);
                    }
                }
                Command::Glyphs {
                    face,
                    size,
                    x,
                    y,
                    color,
                    glyphs,
                    ..
                } => {
                    let u = &uses[face];
                    if !used_fonts.contains(face) {
                        used_fonts.push(*face);
                    }
                    c.save_state();
                    use_alpha(&mut c, &mut res, color.3);
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
                    c.restore_state();
                }
                Command::PushClip { x, y, w, h: rh } => {
                    c.save_state();
                    c.rect(*x, h - y - rh, *w, *rh);
                    c.clip_nonzero();
                    c.end_path();
                }
                Command::PopClip | Command::PopTransform | Command::PopOpacity => {
                    c.restore_state();
                }
                Command::PushTransform { m } => {
                    c.save_state();
                    c.transform(flip_matrix(*m, h));
                }
                Command::PushClipPath { segs } => {
                    c.save_state();
                    emit_path(&mut c, segs, h);
                    c.clip_nonzero();
                    c.end_path();
                }
                Command::PushOpacity { alpha } => {
                    c.save_state();
                    use_alpha(
                        &mut c,
                        &mut res,
                        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
                    );
                }
                Command::BlurPath {
                    segs,
                    color,
                    radius,
                    spread,
                } => {
                    // PDF has no blur filter: the blurred shape is rasterised as a soft
                    // mask (ISO 32000-1, §11.6.5.3 SMask) of a 1 × 1 image of its colour.
                    let Some(m) = bo_raster::blur_mask(segs, *radius, *spread, BLUR_PX_PER_PT)
                    else {
                        continue;
                    };
                    let id = alloc();
                    write_soft_image(&mut pdf, &mut alloc, id, *color, &m);
                    let name = format!("Im{}", id.get());
                    res.images.push((name.clone(), id));
                    emit_image(&mut c, &name, [m.x, m.y, m.w, m.h], None, 0.0, h);
                }
                Command::Path { segs, fill, stroke } => {
                    emit_vector_path(
                        &mut c,
                        &mut res,
                        &mut pending,
                        &mut alloc,
                        segs,
                        fill.as_ref(),
                        stroke.as_ref(),
                        h,
                    );
                }
                Command::Image {
                    x,
                    y,
                    w,
                    h: ih,
                    image,
                    crop,
                    rotation,
                } => {
                    let r = if let Some(r) = image_refs.get(image) {
                        *r
                    } else {
                        let r = images.image(image).and_then(load_image).map(|img| {
                            let id = alloc();
                            write_image(&mut pdf, &mut alloc, id, &img);
                            id
                        });
                        image_refs.insert(image.clone(), r);
                        r
                    };
                    let Some(r) = r else {
                        // Missing image: light grey placeholder.
                        set_fill(&mut c, Rgba(220, 220, 220, 255));
                        c.rect(*x, h - y - ih, *w, *ih);
                        c.fill_nonzero();
                        continue;
                    };
                    let name = format!("Im{}", r.get());
                    if !res.images.iter().any(|(n, _)| *n == name) {
                        res.images.push((name.clone(), r));
                    }
                    emit_image(&mut c, &name, [*x, *y, *w, *ih], *crop, *rotation, h);
                }
            }
        }
        for s in &pending {
            write_shading(&mut pdf, &mut alloc, s);
        }
        for a in &res.alphas {
            if !alpha_refs.contains_key(a) {
                let id = alloc();
                let v = f32::from(*a) / 255.0;
                pdf.ext_graphics(id).non_stroking_alpha(v).stroking_alpha(v);
                alpha_refs.insert(*a, id);
            }
        }
        let data = compress(&c.finish());
        pdf.stream(content_id, &data).filter(Filter::FlateDecode);
        let mut p = pdf.page(page_id);
        p.media_box(Rect::new(0.0, 0.0, page.width, page.height));
        p.parent(pages_id);
        p.contents(content_id);
        let mut resources = p.resources();
        let mut fdict = resources.fonts();
        for f in &used_fonts {
            fdict.pair(Name(format!("F{f}").as_bytes()), uses[f].font_ref);
        }
        fdict.finish();
        if !res.shadings.is_empty() {
            let mut d = resources.shadings();
            for (n, r) in &res.shadings {
                d.pair(Name(n.as_bytes()), *r);
            }
        }
        if !res.images.is_empty() {
            let mut d = resources.x_objects();
            for (n, r) in &res.images {
                d.pair(Name(n.as_bytes()), *r);
            }
        }
        if !res.alphas.is_empty() {
            let mut d = resources.ext_g_states();
            for a in &res.alphas {
                if let Some(r) = alpha_refs.get(a) {
                    d.pair(Name(alpha_name(*a).as_bytes()), *r);
                }
            }
        }
        resources.finish();
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

        let bbox = face_parsed
            .as_ref()
            .map_or(Rect::new(0.0, -200.0, 1000.0, 900.0), |f| {
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

    pdf.pages(pages_id)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    pdf.catalog(catalog_id).pages(pages_id);
    let mut info = pdf.document_info(info_id);
    info.producer(pdf_writer::TextStr("BlueOffice"));
    if let Some(t) = &meta.title {
        info.title(pdf_writer::TextStr(t));
    }
    info.finish();
    Ok(pdf.finish())
}

// ---------------------------------------------------------------------------
// Vector graphics, transforms and images (ISO 32000-1 §8.4 graphics state,
// §8.5 path construction, §8.7.4.5 shadings, §8.9 images, §11.6.4.3 soft masks).
//
// The display list is y-down with origin top-left; PDF user space is y-up.
// Every command flips its own coordinates (`y → h − y`). A display-list
// transform M is therefore emitted as F·M·F (F = the flip), which composes
// correctly with the flipped coordinates of the commands inside it.
// ---------------------------------------------------------------------------

/// Resources referenced by one page's content stream.
#[derive(Default)]
struct PageRes {
    alphas: Vec<u8>,
    shadings: Vec<(String, Ref)>,
    images: Vec<(String, Ref)>,
}

/// A shading to be written after the page content.
struct PendingShading {
    id: Ref,
    radial: bool,
    coords: [f32; 6],
    stops: Vec<GradientStop>,
}

/// Image XObject ready to be written (decoded once per document).
enum ImageData {
    /// Baseline JPEG passed through with `DCTDecode`.
    Jpeg {
        data: Vec<u8>,
        width: u32,
        height: u32,
        components: u8,
    },
    /// Decoded samples (8 bit) plus an optional alpha channel for the soft mask.
    Raw {
        rgb: Vec<u8>,
        gray: bool,
        alpha: Option<Vec<u8>>,
        width: u32,
        height: u32,
    },
}

/// Conjugate a y-down transform by the page flip (see module note above).
fn flip_matrix(m: [f32; 6], h: f32) -> [f32; 6] {
    let [a, b, c, d, e, f] = m;
    [a, -b, -c, d, c * h + e, h - d * h - f]
}

fn alpha_name(a: u8) -> String {
    format!("A{a}")
}

/// Append path segments (flipped) to the content stream.
fn emit_path(c: &mut Content, segs: &[PathSeg], h: f32) {
    let mut cur = (0.0_f32, 0.0_f32);
    let mut start = cur;
    for s in segs {
        match *s {
            PathSeg::M(x, y) => {
                c.move_to(x, h - y);
                cur = (x, y);
                start = cur;
            }
            PathSeg::L(x, y) => {
                c.line_to(x, h - y);
                cur = (x, y);
            }
            // PDF has no quadratic segments: elevate to cubic.
            PathSeg::Q(qx, qy, x, y) => {
                let c1 = (
                    cur.0 + 2.0 / 3.0 * (qx - cur.0),
                    cur.1 + 2.0 / 3.0 * (qy - cur.1),
                );
                let c2 = (x + 2.0 / 3.0 * (qx - x), y + 2.0 / 3.0 * (qy - y));
                c.cubic_to(c1.0, h - c1.1, c2.0, h - c2.1, x, h - y);
                cur = (x, y);
            }
            PathSeg::C(x1, y1, x2, y2, x, y) => {
                c.cubic_to(x1, h - y1, x2, h - y2, x, h - y);
                cur = (x, y);
            }
            PathSeg::Z => {
                c.close_path();
                cur = start;
            }
        }
    }
}

fn set_stroke(c: &mut Content, s: &Stroke) {
    c.set_stroke_rgb(
        f32::from(s.color.0) / 255.0,
        f32::from(s.color.1) / 255.0,
        f32::from(s.color.2) / 255.0,
    );
    c.set_line_width(s.width);
    c.set_line_cap(match s.cap.as_str() {
        "round" => LineCapStyle::RoundCap,
        "square" => LineCapStyle::ProjectingSquareCap,
        _ => LineCapStyle::ButtCap,
    });
    c.set_line_join(match s.join.as_str() {
        "miter" => LineJoinStyle::MiterJoin,
        "bevel" => LineJoinStyle::BevelJoin,
        _ => LineJoinStyle::RoundJoin,
    });
    if s.dash.len() >= 2 {
        c.set_dash_pattern(s.dash.iter().copied(), 0.0);
    }
}

/// Use an alpha ExtGState inside the current `q … Q` when `a < 255`.
fn use_alpha(c: &mut Content, res: &mut PageRes, a: u8) {
    if a == 255 {
        return;
    }
    if !res.alphas.contains(&a) {
        res.alphas.push(a);
    }
    c.set_parameters(Name(alpha_name(a).as_bytes()));
}

#[allow(clippy::too_many_arguments)]
fn emit_vector_path(
    c: &mut Content,
    res: &mut PageRes,
    pending: &mut Vec<PendingShading>,
    alloc: &mut dyn FnMut() -> Ref,
    segs: &[PathSeg],
    fill: Option<&Paint>,
    stroke: Option<&Stroke>,
    h: f32,
) {
    match fill {
        Some(Paint::Solid { color }) => {
            c.save_state();
            use_alpha(c, res, color.3);
            set_fill(c, *color);
            emit_path(c, segs, h);
            c.fill_nonzero();
            c.restore_state();
        }
        Some(Paint::Linear {
            x1,
            y1,
            x2,
            y2,
            stops,
        }) => {
            shade(
                c,
                res,
                pending,
                alloc,
                segs,
                h,
                false,
                [*x1, h - y1, *x2, h - y2, 0.0, 0.0],
                stops,
            );
        }
        Some(Paint::Radial { cx, cy, r, stops }) => {
            shade(
                c,
                res,
                pending,
                alloc,
                segs,
                h,
                true,
                [*cx, h - cy, 0.0, *cx, h - cy, *r],
                stops,
            );
        }
        None => {}
    }
    if let Some(s) = stroke {
        c.save_state();
        use_alpha(c, res, s.color.3);
        set_stroke(c, s);
        emit_path(c, segs, h);
        c.stroke();
        c.restore_state();
    }
}

#[allow(clippy::too_many_arguments)]
fn shade(
    c: &mut Content,
    res: &mut PageRes,
    pending: &mut Vec<PendingShading>,
    alloc: &mut dyn FnMut() -> Ref,
    segs: &[PathSeg],
    h: f32,
    radial: bool,
    coords: [f32; 6],
    stops: &[GradientStop],
) {
    if stops.is_empty() {
        return;
    }
    if stops.len() == 1 {
        emit_vector_path(
            c,
            res,
            pending,
            alloc,
            segs,
            Some(&Paint::Solid {
                color: stops[0].color,
            }),
            None,
            h,
        );
        return;
    }
    let id = alloc();
    let name = format!("Sh{}", res.shadings.len());
    res.shadings.push((name.clone(), id));
    pending.push(PendingShading {
        id,
        radial,
        coords,
        stops: stops.to_vec(),
    });
    c.save_state();
    emit_path(c, segs, h);
    c.clip_nonzero();
    c.end_path();
    c.shading(Name(name.as_bytes()));
    c.restore_state();
}

fn rgb(c: Rgba) -> [f32; 3] {
    [
        f32::from(c.0) / 255.0,
        f32::from(c.1) / 255.0,
        f32::from(c.2) / 255.0,
    ]
}

/// Write an axial/radial shading with an exponential (2 stops) or stitching
/// (n stops) function, ISO 32000-1 §7.10.3 / §7.10.4.
fn write_shading(pdf: &mut Pdf, alloc: &mut dyn FnMut() -> Ref, s: &PendingShading) {
    let mut stops = s.stops.clone();
    stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    if let Some(first) = stops.first().copied() {
        if first.offset > 0.0 {
            stops.insert(
                0,
                GradientStop {
                    offset: 0.0,
                    ..first
                },
            );
        }
    }
    if let Some(last) = stops.last().copied() {
        if last.offset < 1.0 {
            stops.push(GradientStop {
                offset: 1.0,
                ..last
            });
        }
    }
    let mut fns = Vec::new();
    for w in stops.windows(2) {
        let id = alloc();
        pdf.exponential_function(id)
            .domain([0.0, 1.0])
            .c0(rgb(w[0].color))
            .c1(rgb(w[1].color))
            .n(1.0);
        fns.push(id);
    }
    let func = if fns.len() == 1 {
        fns[0]
    } else {
        let id = alloc();
        let bounds: Vec<f32> = stops[1..stops.len() - 1].iter().map(|s| s.offset).collect();
        pdf.stitching_function(id)
            .domain([0.0, 1.0])
            .functions(fns.iter().copied())
            .bounds(bounds)
            .encode(fns.iter().flat_map(|_| [0.0, 1.0]));
        id
    };
    let mut sh = pdf.function_shading(s.id);
    sh.shading_type(if s.radial {
        FunctionShadingType::Radial
    } else {
        FunctionShadingType::Axial
    });
    sh.color_space().device_rgb();
    sh.function(func);
    if s.radial {
        sh.coords(s.coords);
    } else {
        sh.coords(s.coords[..4].iter().copied());
    }
    sh.extend([true, true]);
}

/// Size and component count of a baseline/progressive JPEG (SOFn marker, ITU-T T.81 §B.2.2).
fn jpeg_info(data: &[u8]) -> Option<(u32, u32, u8)> {
    let mut i = 2;
    while i + 9 < data.len() {
        if data[i] != 0xFF {
            return None;
        }
        let marker = data[i + 1];
        let len = usize::from(u16::from_be_bytes([data[i + 2], data[i + 3]]));
        if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            let h = u32::from(u16::from_be_bytes([data[i + 5], data[i + 6]]));
            let w = u32::from(u16::from_be_bytes([data[i + 7], data[i + 8]]));
            return Some((w, h, data[i + 9]));
        }
        i += 2 + len;
    }
    None
}

fn decode_png(data: &[u8]) -> Option<ImageData> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(data));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let (n, gray, has_alpha) = match info.color_type {
        png::ColorType::Grayscale => (1, true, false),
        png::ColorType::GrayscaleAlpha => (2, true, true),
        png::ColorType::Rgb => (3, false, false),
        png::ColorType::Rgba => (4, false, true),
        png::ColorType::Indexed => return None,
    };
    let color_n = if gray { 1 } else { 3 };
    let mut rgb = Vec::with_capacity(buf.len() / n * color_n);
    let mut alpha = has_alpha.then(|| Vec::with_capacity(buf.len() / n));
    for px in buf.chunks_exact(n) {
        rgb.extend_from_slice(&px[..color_n]);
        if let Some(a) = alpha.as_mut() {
            a.push(px[n - 1]);
        }
    }
    Some(ImageData::Raw {
        rgb,
        gray,
        alpha,
        width: info.width,
        height: info.height,
    })
}

fn load_image(data: &[u8]) -> Option<ImageData> {
    if data.starts_with(&[0xFF, 0xD8]) {
        let (width, height, components) = jpeg_info(data)?;
        return Some(ImageData::Jpeg {
            data: data.to_vec(),
            width,
            height,
            components,
        });
    }
    if data.starts_with(b"\x89PNG") {
        return decode_png(data);
    }
    None
}

fn write_image(pdf: &mut Pdf, alloc: &mut dyn FnMut() -> Ref, id: Ref, img: &ImageData) {
    match img {
        ImageData::Jpeg {
            data,
            width,
            height,
            components,
        } => {
            let mut x = pdf.image_xobject(id, data);
            x.filter(Filter::DctDecode);
            x.width(*width as i32).height(*height as i32);
            match components {
                1 => x.color_space().device_gray(),
                4 => x.color_space().device_cmyk(),
                _ => x.color_space().device_rgb(),
            }
            x.bits_per_component(8);
        }
        ImageData::Raw {
            rgb,
            gray,
            alpha,
            width,
            height,
        } => {
            let mask = alpha.as_ref().map(|a| {
                let mid = alloc();
                let data = compress(a);
                let mut m = pdf.image_xobject(mid, &data);
                m.filter(Filter::FlateDecode);
                m.width(*width as i32).height(*height as i32);
                m.color_space().device_gray();
                m.bits_per_component(8);
                mid
            });
            let data = compress(rgb);
            let mut x = pdf.image_xobject(id, &data);
            x.filter(Filter::FlateDecode);
            x.width(*width as i32).height(*height as i32);
            if *gray {
                x.color_space().device_gray();
            } else {
                x.color_space().device_rgb();
            }
            x.bits_per_component(8);
            if let Some(m) = mask {
                x.s_mask(m);
            }
        }
    }
}

/// Resolution of rasterised soft shadows / glows (pixels per point).
const BLUR_PX_PER_PT: f32 = 2.0;

/// A 1 × 1 image of `color` whose soft mask (ISO 32000-1, §11.6.5.3; it may have
/// other dimensions than its image, §11.6.5.2) is the blurred coverage `m`
/// scaled by the colour's alpha.
fn write_soft_image(
    pdf: &mut Pdf,
    alloc: &mut dyn FnMut() -> Ref,
    id: Ref,
    color: Rgba,
    m: &bo_raster::BlurMask,
) {
    let a = u32::from(color.3);
    let alpha: Vec<u8> = m
        .alpha
        .iter()
        .map(|&v| ((u32::from(v) * a + 127) / 255) as u8)
        .collect();
    let mid = alloc();
    let data = compress(&alpha);
    let mut s = pdf.image_xobject(mid, &data);
    s.filter(Filter::FlateDecode);
    s.width(m.width as i32).height(m.height as i32);
    s.color_space().device_gray();
    s.bits_per_component(8);
    s.finish();
    let rgb = [color.0, color.1, color.2];
    let mut x = pdf.image_xobject(id, &rgb);
    x.width(1).height(1);
    x.color_space().device_rgb();
    x.bits_per_component(8);
    x.s_mask(mid);
}

/// Draw an image XObject into the destination rect (crop = l,t,r,b fractions).
fn emit_image(
    c: &mut Content,
    name: &str,
    [x, y, w, hh]: [f32; 4],
    crop: Option<[f32; 4]>,
    rotation: f32,
    h: f32,
) {
    let [cl, ct, cr, cb] = crop.unwrap_or([0.0; 4]);
    let fw = w / (1.0 - cl - cr).max(1e-3);
    let fh = hh / (1.0 - ct - cb).max(1e-3);
    let x0 = x - cl * fw;
    let y0 = y - ct * fh;
    c.save_state();
    if rotation != 0.0 {
        let (s, co) = rotation.to_radians().sin_cos();
        let (cx, cy) = (x + w / 2.0, y + hh / 2.0);
        // Rotation about the centre in y-down space, conjugated by the flip.
        let m = [co, s, -s, co, cx - co * cx + s * cy, cy - s * cx - co * cy];
        c.transform(flip_matrix(m, h));
    }
    c.rect(x, h - y - hh, w, hh);
    c.clip_nonzero();
    c.end_path();
    c.transform([fw, 0.0, 0.0, fh, x0, h - y0 - fh]);
    c.x_object(Name(name.as_bytes()));
    c.restore_state();
}

fn set_fill(c: &mut Content, color: Rgba) {
    c.set_fill_rgb(
        f32::from(color.0) / 255.0,
        f32::from(color.1) / 255.0,
        f32::from(color.2) / 255.0,
    );
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
            pages: vec![PageList {
                width: 595.0,
                height: 842.0,
                commands: vec![],
            }],
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
    fn tiny_png() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 2, 1);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[255, 0, 0, 255, 0, 0, 255, 128])
                .unwrap();
        }
        out
    }

    #[test]
    fn vector_commands_and_images() {
        let tri = vec![
            PathSeg::M(0.0, 0.0),
            PathSeg::Q(50.0, 0.0, 50.0, 50.0),
            PathSeg::L(0.0, 50.0),
            PathSeg::Z,
        ];
        let stop = |offset, c| GradientStop {
            offset,
            color: Rgba(c, c, c, 255),
        };
        let list = DisplayList {
            pages: vec![PageList {
                width: 200.0,
                height: 100.0,
                commands: vec![
                    Command::PushTransform {
                        m: [1.0, 0.0, 0.0, 1.0, 10.0, 10.0],
                    },
                    Command::Path {
                        segs: tri.clone(),
                        fill: Some(Paint::Linear {
                            x1: 0.0,
                            y1: 0.0,
                            x2: 50.0,
                            y2: 0.0,
                            stops: vec![stop(0.0, 0), stop(0.5, 128), stop(1.0, 255)],
                        }),
                        stroke: Some(Stroke {
                            width: 1.0,
                            color: Rgba(0, 0, 0, 128),
                            dash: vec![3.0, 1.0],
                            cap: "round".into(),
                            join: "miter".into(),
                        }),
                    },
                    Command::PopTransform,
                    Command::PushClipPath { segs: tri },
                    Command::Image {
                        x: 0.0,
                        y: 0.0,
                        w: 40.0,
                        h: 20.0,
                        image: "a.png".into(),
                        crop: Some([0.1, 0.0, 0.0, 0.0]),
                        rotation: 0.0,
                    },
                    Command::PopClip,
                ],
            }],
        };
        let mut images = BTreeMap::new();
        images.insert("a.png".to_owned(), tiny_png());
        let bytes = write_pdf_with_images(&list, &FontManager::new(), &PdfMeta::default(), &images)
            .unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("/ShadingType 2"));
        assert!(s.contains("/FunctionType 3"));
        assert!(s.contains("/Subtype /Image"));
        assert!(s.contains("/SMask"));
        assert!(s.contains("/ca 0.5"));
        assert!(s.trim_end().ends_with("%%EOF"));
    }

    #[test]
    fn flip_matrix_conjugates() {
        // Translation by (10, 20) in y-down space is (10, −20) in PDF space.
        assert_eq!(
            flip_matrix([1.0, 0.0, 0.0, 1.0, 10.0, 20.0], 100.0),
            [1.0, 0.0, 0.0, 1.0, 10.0, -20.0]
        );
        assert_eq!(
            jpeg_info(&[0xFF, 0xD8, 0xFF, 0xC0, 0, 17, 8, 0, 4, 0, 3, 3]),
            Some((3, 4, 3))
        );
    }
}
