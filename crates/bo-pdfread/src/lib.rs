//! PDF reading for BlueReader (and later BlueEditor): pages become `bo_render::PageList`
//! display lists, drawn by the same GPU page bitmaps as the other BlueOffice apps.
//!
//! Coverage (ISO 32000-1): cross-reference tables and streams, object streams and files
//! encrypted with an empty user password (through `lopdf`); the page tree with inherited
//! attributes, MediaBox / CropBox / Rotate, page labels, outlines and link annotations;
//! content streams with paths, clipping, colour spaces, images, shadings, Form XObjects,
//! optional content and constant transparency; simple, composite and Type 3 fonts with
//! embedded TrueType / OpenType / CFF / Type 1 programs; annotation appearance streams;
//! text with positions for search, selection and copy; markup annotations written back as an
//! incremental update.
#![forbid(unsafe_code)]
// Nested helpers next to their only caller; PostScript and PDF operator tables.
#![allow(clippy::items_after_statements, clippy::match_same_arms)]

mod annot;
mod cmap;
mod codecs;
mod color;
mod crypt;
mod edit;
mod encoding;
#[allow(clippy::unreadable_literal, clippy::large_const_arrays)]
mod encoding_data;
mod fallback;
mod filters;
mod font;
mod forms;
mod function;
mod image;
mod interp;
mod lexer;
mod objects;
mod sfnt;
mod text;
mod type1;
mod write;

pub use annot::{Annot, AnnotKind, NewAnnot};
pub use edit::SaveOptions;
pub use forms::{FieldKind, FormField};
pub use image::Image;
pub use objects::{image_size, text_lines, NewImage, ObjectKind, PageObject, TextLine};
pub use text::TextLayer;

use bo_render::PageList;
use bo_text::FontManager;
use interp::{mul, Interp, Shared, M};
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Options of one page rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderOptions {
    /// Draw every glyph as an outline path (when the text engine refused a font).
    pub text_as_paths: bool,
    /// Thinnest stroke in points (one device pixel at the zoom the page is drawn at).
    pub min_line: f32,
    /// Draw annotations (comments, form fields).
    pub annotations: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            text_as_paths: false,
            min_line: 0.5,
            annotations: true,
        }
    }
}

/// A shown character with its box in page points (origin top-left, y down).
#[derive(Debug, Clone, PartialEq)]
pub struct TextChar {
    /// Unicode text (may be several characters for ligatures, or empty).
    pub text: String,
    /// Box: left, top, right, bottom.
    pub rect: [f32; 4],
    /// Baseline origin.
    pub origin: [f32; 2],
    /// Text direction (one em along the baseline).
    pub dir: [f32; 2],
    /// Font size on the page.
    pub size: f32,
}

/// Where a link goes.
#[derive(Debug, Clone, PartialEq)]
pub enum LinkTarget {
    /// A page of this document, optionally a vertical position on it (points from the top).
    Page {
        /// Page index.
        index: usize,
        /// Top of the destination in page points.
        top: Option<f32>,
    },
    /// An external address.
    Uri(String),
}

/// A link area on a page.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    /// Left, top, right, bottom in page points.
    pub rect: [f32; 4],
    /// Destination.
    pub target: LinkTarget,
}

/// A rendered page.
pub struct Page {
    /// Display list in page points.
    pub list: PageList,
    /// Pictures referenced by the list's `Image` commands.
    pub images: HashMap<String, Arc<Image>>,
    /// Shown characters in content order.
    pub chars: Vec<TextChar>,
    /// Link areas.
    pub links: Vec<Link>,
}

/// Size and labels of a page.
#[derive(Debug, Clone, PartialEq)]
pub struct PageInfo {
    /// Width in points after rotation.
    pub width: f32,
    /// Height in points after rotation.
    pub height: f32,
    /// Page rotation in degrees (0, 90, 180, 270).
    pub rotate: u16,
    /// Page label (`/PageLabels`), or the page number.
    pub label: String,
    id: ObjectId,
    /// Crop box in user space: x0, y0, x1, y1.
    crop: [f32; 4],
}

/// An outline (bookmark) entry.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineItem {
    /// Title.
    pub title: String,
    /// Destination page and position.
    pub target: Option<LinkTarget>,
    /// Shown expanded.
    pub open: bool,
    /// Child entries.
    pub children: Vec<OutlineItem>,
}

/// An open PDF document.
pub struct PdfDocument {
    doc: Document,
    bytes: Arc<Vec<u8>>,
    pages: Vec<PageInfo>,
    fonts: FontManager,
    font_cache: font::FontCache,
    images: interp::ImageCache,
    hidden: HashSet<ObjectId>,
    /// Objects added or changed since opening (written by an incremental save).
    changed: Vec<ObjectId>,
    /// The document has `/PageLabels`.
    labelled: bool,
    /// Undo / redo and page tree edits (BlueEditor).
    edit: edit::EditState,
}

fn resolve<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    doc.dereference(o).map_or(o, |(_, o)| o)
}

/// An attribute of a page, inherited from its ancestors (§7.7.3.4).
fn inherited<'a>(doc: &'a Document, page: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    let mut d = page;
    for _ in 0..32 {
        if let Ok(v) = d.get(key) {
            return Some(resolve(doc, v));
        }
        d = resolve(doc, d.get(b"Parent").ok()?).as_dict().ok()?;
    }
    None
}

fn rect_of(doc: &Document, o: Option<&Object>) -> Option<[f32; 4]> {
    let a = o?.as_array().ok()?;
    let v: Vec<f32> = a
        .iter()
        .filter_map(|o| resolve(doc, o).as_float().ok())
        .collect();
    let [x0, y0, x1, y1] = <[f32; 4]>::try_from(v.as_slice()).ok()?;
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return None;
    }
    let r = [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)];
    (r[2] - r[0] >= 1.0 && r[3] - r[1] >= 1.0).then_some(r)
}

/// Decode a PDF text string (§7.9.2.2): UTF-16BE with BOM, UTF-8 with BOM or PDFDocEncoding.
pub(crate) fn text_string(b: &[u8]) -> String {
    if let Some(rest) = b.strip_prefix(&[0xfe, 0xff]) {
        let units: Vec<u16> = rest
            .chunks(2)
            .map(|c| u16::from(c[0]) << 8 | u16::from(c.get(1).copied().unwrap_or(0)))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(rest) = b.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    b.iter()
        .map(|&c| match c {
            0x80..=0x9f | 0xad => {
                // PDFDocEncoding specials (Annex D.3) that differ from Latin-1.
                const SPECIAL: [char; 32] = [
                    '•', '†', '‡', '…', '—', '–', 'ƒ', '⁄', '‹', '›', '−', '‰', '„', '“', '”', '‘',
                    '’', '‚', '™', 'ﬁ', 'ﬂ', 'Ł', 'Œ', 'Š', 'Ÿ', 'Ž', 'ı', 'ł', 'œ', 'š', 'ž',
                    '\u{fffd}',
                ];
                if c == 0xad {
                    '\u{ad}'
                } else {
                    SPECIAL[usize::from(c - 0x80)]
                }
            }
            c => char::from(c),
        })
        .collect()
}

fn roman(mut n: u32, upper: bool) -> String {
    const T: [(u32, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut s = String::new();
    for (v, r) in T {
        while n >= v && s.len() < 64 {
            s.push_str(r);
            n -= v;
        }
    }
    if upper {
        s.to_uppercase()
    } else {
        s
    }
}

fn letters(n: u32, upper: bool) -> String {
    if n == 0 {
        return String::new();
    }
    let c = char::from(b'a' + ((n - 1) % 26) as u8);
    let s: String = std::iter::repeat_n(c, ((n - 1) / 26 + 1).min(64) as usize).collect();
    if upper {
        s.to_uppercase()
    } else {
        s
    }
}

/// Page labels from the `/PageLabels` number tree (§12.4.2).
fn page_labels(doc: &Document, count: usize) -> Vec<String> {
    let mut ranges: Vec<(usize, Dictionary)> = Vec::new();
    fn walk(doc: &Document, node: &Dictionary, out: &mut Vec<(usize, Dictionary)>, depth: u32) {
        if depth > 16 {
            return;
        }
        if let Some(nums) = node
            .get(b"Nums")
            .ok()
            .and_then(|o| resolve(doc, o).as_array().ok())
        {
            for pair in nums.chunks(2) {
                if let [k, v] = pair {
                    if let (Ok(k), Ok(d)) = (resolve(doc, k).as_i64(), resolve(doc, v).as_dict()) {
                        out.push((k.max(0) as usize, d.clone()));
                    }
                }
            }
        }
        if let Some(kids) = node
            .get(b"Kids")
            .ok()
            .and_then(|o| resolve(doc, o).as_array().ok())
        {
            for k in kids {
                if let Ok(d) = resolve(doc, k).as_dict() {
                    walk(doc, d, out, depth + 1);
                }
            }
        }
    }
    if let Some(root) = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"PageLabels").ok())
        .and_then(|o| resolve(doc, o).as_dict().ok())
    {
        walk(doc, root, &mut ranges, 0);
    }
    ranges.sort_by_key(|r| r.0);
    (0..count)
        .map(|i| {
            let Some((start, d)) = ranges.iter().rev().find(|r| r.0 <= i) else {
                return (i + 1).to_string();
            };
            let first = d
                .get(b"St")
                .ok()
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(1)
                .max(1) as u32;
            let n = first + (i - start) as u32;
            let prefix = d
                .get(b"P")
                .ok()
                .and_then(|o| resolve(doc, o).as_str().ok())
                .map(text_string)
                .unwrap_or_default();
            let num = match d.get(b"S").ok().and_then(|o| o.as_name().ok()) {
                Some(b"D") => n.to_string(),
                Some(b"R") => roman(n, true),
                Some(b"r") => roman(n, false),
                Some(b"A") => letters(n, true),
                Some(b"a") => letters(n, false),
                _ => String::new(),
            };
            format!("{prefix}{num}")
        })
        .collect()
}

/// Base matrix of a page: user space to page points (y down, rotation applied).
fn page_matrix(crop: [f32; 4], rotate: u16) -> M {
    let (w, h) = (crop[2] - crop[0], crop[3] - crop[1]);
    let flip = [1.0, 0.0, 0.0, -1.0, -crop[0], crop[3]];
    let rot = match rotate {
        90 => [0.0, 1.0, -1.0, 0.0, h, 0.0],
        180 => [-1.0, 0.0, 0.0, -1.0, w, h],
        270 => [0.0, -1.0, 1.0, 0.0, 0.0, w],
        _ => interp::IDENTITY,
    };
    mul(flip, rot)
}

#[cfg(feature = "bundled-fonts")]
mod bundled {
    macro_rules! font {
        ($f:literal) => {
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/fonts/",
                $f
            ))
            .as_slice()
        };
    }
    /// Substitutes of Times and Helvetica (metric-compatible, OFL / Apache-2.0).
    pub(crate) const FONTS: &[&[u8]] = &[
        font!("Tinos-Regular.ttf"),
        font!("Tinos-Bold.ttf"),
        font!("Tinos-Italic.ttf"),
        font!("Tinos-BoldItalic.ttf"),
        font!("LiberationSans-Regular.ttf"),
        font!("LiberationSans-Bold.ttf"),
        font!("LiberationSans-Italic.ttf"),
        font!("LiberationSans-BoldItalic.ttf"),
    ];
}

/// A font manager with the bundled substitutes for the standard fonts.
#[cfg(feature = "bundled-fonts")]
pub fn standard_fonts() -> FontManager {
    let mut m = FontManager::new();
    for f in bundled::FONTS {
        m.add_font(f.to_vec());
    }
    m
}

impl PdfDocument {
    /// Open a PDF from its bytes. `fonts` holds the faces that stand in for fonts that are not
    /// embedded (see [`standard_fonts`]); embedded fonts are added to it as pages render.
    pub fn open(bytes: Vec<u8>, fonts: FontManager) -> Result<Self, String> {
        Self::open_with_password(bytes, fonts, "")
    }

    /// Whether `bytes` are a PDF that needs a password to open.
    pub fn needs_password(bytes: &[u8]) -> bool {
        let start = bytes.windows(5).take(1024).position(|w| w == b"%PDF-");
        start
            .and_then(|s| Document::load_mem(&bytes[s..]).ok())
            .is_some_and(|d| d.is_encrypted() && d.encryption_state.is_none())
    }

    /// Open a PDF encrypted with a user or owner `password` (the standard security handler,
    /// ISO 32000-1 §7.6.3–7.6.4; an empty password opens unprotected files). The document keeps
    /// its security handler: it is encrypted again with the same key when saved.
    pub fn open_with_password(
        bytes: Vec<u8>,
        fonts: FontManager,
        password: &str,
    ) -> Result<Self, String> {
        let start = bytes
            .windows(5)
            .take(1024)
            .position(|w| w == b"%PDF-")
            .ok_or("not a PDF file")?;
        let mut doc = Document::load_mem(&bytes[start..]).map_err(|e| format!("PDF: {e}"))?;
        if doc.is_encrypted() && doc.encryption_state.is_none() {
            if password.is_empty() {
                return Err("the document is protected by a password".into());
            }
            doc = crypt::load(&bytes[start..], password)?;
        }
        let (pages, labelled) = scan_pages(&doc);
        if pages.is_empty() {
            return Err("the document has no pages".into());
        }
        let hidden = hidden_groups(&doc);
        Ok(Self {
            doc,
            bytes: Arc::new(bytes),
            pages,
            fonts,
            font_cache: font::FontCache::default(),
            images: interp::ImageCache::default(),
            hidden,
            changed: Vec::new(),
            labelled,
            edit: edit::EditState::default(),
        })
    }

    /// Whether the document defines page labels (`/PageLabels`).
    pub fn has_page_labels(&self) -> bool {
        self.labelled
    }

    /// Number of pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Size and label of page `index`.
    pub fn page_info(&self, index: usize) -> Option<&PageInfo> {
        self.pages.get(index)
    }

    /// Fonts of the document (substitutes and embedded faces), for drawing glyph runs.
    pub fn fonts(&self) -> &FontManager {
        &self.fonts
    }

    /// Document title from the information dictionary.
    pub fn title(&self) -> Option<String> {
        let info = self.doc.trailer.get(b"Info").ok()?;
        let d = resolve(&self.doc, info).as_dict().ok()?;
        let t = resolve(&self.doc, d.get(b"Title").ok()?).as_str().ok()?;
        let s = text_string(t).trim().to_owned();
        (!s.is_empty()).then_some(s)
    }

    /// The bytes the document was opened from, with any saved updates appended.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Render page `index` into a display list.
    pub fn render(&mut self, index: usize, opts: RenderOptions) -> Result<Page, String> {
        let info = self.pages.get(index).ok_or("no such page")?.clone();
        let base = page_matrix(info.crop, info.rotate);
        let doc = &self.doc;
        let page = doc.get_dictionary(info.id).map_err(|e| e.to_string())?;
        let resources = inherited(doc, page, b"Resources").and_then(|o| o.as_dict().ok());
        let content = page_content(doc, page);
        let mut shared = Shared {
            doc,
            fonts: &mut self.fonts,
            font_cache: &mut self.font_cache,
            images: &mut self.images,
            hidden: &self.hidden,
            opts,
        };
        let mut it = Interp::new(
            &mut shared,
            base,
            resources,
            (info.width, info.height),
            format!("p{index}"),
        );
        it.run(&content);
        it.finish();
        let mut links = Vec::new();
        if let Some(annots) = page
            .get(b"Annots")
            .ok()
            .and_then(|o| resolve(doc, o).as_array().ok())
        {
            for a in annots {
                let Ok(d) = resolve(doc, a).as_dict() else {
                    continue;
                };
                let subtype = d.get(b"Subtype").and_then(Object::as_name).unwrap_or(b"");
                let Some(rect) = annot_rect(doc, d) else {
                    continue;
                };
                if subtype == b"Link" {
                    if let Some(target) = link_target(doc, d, &self.pages) {
                        links.push(Link {
                            rect: device_rect(base, rect),
                            target,
                        });
                    }
                }
                if opts.annotations {
                    draw_annotation(&mut it, doc, d, rect);
                }
            }
        }
        it.finish();
        let out = std::mem::take(&mut it.out);
        Ok(Page {
            list: PageList {
                width: info.width,
                height: info.height,
                commands: out.commands,
            },
            images: out.images,
            chars: out.chars,
            links,
        })
    }

    /// Bookmarks.
    pub fn outline(&self) -> Vec<OutlineItem> {
        let doc = &self.doc;
        let Some(root) = doc
            .catalog()
            .ok()
            .and_then(|c| c.get(b"Outlines").ok())
            .and_then(|o| resolve(doc, o).as_dict().ok())
        else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        outline_children(doc, root, &self.pages, &mut seen, 0)
    }

    /// Page index of a page object.
    fn page_index(pages: &[PageInfo], id: ObjectId) -> Option<usize> {
        pages.iter().position(|p| p.id == id)
    }
}

/// Sizes, rotations and labels of every page in page tree order, and whether the document
/// has `/PageLabels` (§7.7.3, §12.4.2).
pub(crate) fn scan_pages(doc: &Document) -> (Vec<PageInfo>, bool) {
    let ids: Vec<ObjectId> = doc.get_pages().into_values().collect();
    let labels = page_labels(doc, ids.len());
    let labelled = doc.catalog().is_ok_and(|c| c.has(b"PageLabels"));
    let pages = ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            let d = doc.get_dictionary(id).ok();
            let media = d
                .and_then(|d| rect_of(doc, inherited(doc, d, b"MediaBox")))
                .unwrap_or([0.0, 0.0, 612.0, 792.0]);
            let crop = d
                .and_then(|d| rect_of(doc, inherited(doc, d, b"CropBox")))
                .map_or(media, |c| {
                    // The crop box is clipped to the media box.
                    let r = [
                        c[0].max(media[0]),
                        c[1].max(media[1]),
                        c[2].min(media[2]),
                        c[3].min(media[3]),
                    ];
                    if r[2] - r[0] >= 1.0 && r[3] - r[1] >= 1.0 {
                        r
                    } else {
                        media
                    }
                });
            let rotate = d
                .and_then(|d| inherited(doc, d, b"Rotate"))
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(0)
                .rem_euclid(360);
            let rotate = (rotate / 90 * 90) as u16;
            let (w, h) = (crop[2] - crop[0], crop[3] - crop[1]);
            let (width, height) = if rotate % 180 == 90 { (h, w) } else { (w, h) };
            PageInfo {
                width: width.min(14_400.0),
                height: height.min(14_400.0),
                rotate,
                label: labels
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| (i + 1).to_string()),
                id,
                crop,
            }
        })
        .collect();
    (pages, labelled)
}

fn page_content(doc: &Document, page: &Dictionary) -> Vec<u8> {
    let mut out = Vec::new();
    let Ok(c) = page.get(b"Contents") else {
        return out;
    };
    let streams: Vec<&Object> = match resolve(doc, c) {
        Object::Array(a) => a.iter().collect(),
        o => vec![o],
    };
    for s in streams {
        if let Ok(st) = resolve(doc, s).as_stream() {
            if let Ok(b) = filters::stream_bytes(doc, st) {
                out.extend_from_slice(&b);
                out.push(b'\n');
            }
        }
    }
    out
}

fn annot_rect(doc: &Document, d: &Dictionary) -> Option<[f32; 4]> {
    let o = d.get(b"Rect").ok().map(|o| resolve(doc, o))?;
    let a = o.as_array().ok()?;
    let v: Vec<f32> = a
        .iter()
        .filter_map(|o| resolve(doc, o).as_float().ok())
        .collect();
    let [x0, y0, x1, y1] = <[f32; 4]>::try_from(v.as_slice()).ok()?;
    Some([x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)])
}

/// A user-space rectangle on the page in page points (left, top, right, bottom).
fn device_rect(base: M, r: [f32; 4]) -> [f32; 4] {
    let pts = [
        interp::apply(base, r[0], r[1]),
        interp::apply(base, r[2], r[3]),
        interp::apply(base, r[0], r[3]),
        interp::apply(base, r[2], r[1]),
    ];
    let xs = pts.iter().map(|p| p.0);
    let ys = pts.iter().map(|p| p.1);
    [
        xs.clone().fold(f32::INFINITY, f32::min),
        ys.clone().fold(f32::INFINITY, f32::min),
        xs.fold(f32::NEG_INFINITY, f32::max),
        ys.fold(f32::NEG_INFINITY, f32::max),
    ]
}

/// Draw an annotation's normal appearance (§12.5.5) or a stand-in for markup without one.
fn draw_annotation<'a>(
    it: &mut Interp<'_, 'a>,
    doc: &'a Document,
    d: &'a Dictionary,
    rect: [f32; 4],
) {
    let flags = d.get(b"F").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
    let subtype = d.get(b"Subtype").and_then(Object::as_name).unwrap_or(b"");
    if flags & (2 | 32) != 0 || subtype == b"Popup" {
        return;
    }
    let ap = d
        .get(b"AP")
        .ok()
        .map(|o| resolve(doc, o))
        .and_then(|o| o.as_dict().ok())
        .and_then(|ap| ap.get(b"N").ok())
        .map(|n| resolve(doc, n));
    let stream = match ap {
        Some(Object::Stream(s)) => Some(s),
        Some(Object::Dictionary(states)) => {
            let state = d.get(b"AS").and_then(Object::as_name).ok();
            state
                .and_then(|s| states.get(s).ok())
                .map(|o| resolve(doc, o))
                .and_then(|o| o.as_stream().ok())
        }
        _ => None,
    };
    if let Some(s) = stream {
        let Ok(content) = filters::stream_bytes(doc, s) else {
            return;
        };
        let nums = |k: &[u8]| -> Vec<f32> {
            s.dict
                .get(k)
                .ok()
                .map(|o| resolve(doc, o))
                .and_then(|o| o.as_array().ok())
                .map(|a| {
                    a.iter()
                        .filter_map(|o| resolve(doc, o).as_float().ok())
                        .collect()
                })
                .unwrap_or_default()
        };
        let matrix = <[f32; 6]>::try_from(nums(b"Matrix").as_slice()).unwrap_or(interp::IDENTITY);
        let Ok(bbox) = <[f32; 4]>::try_from(nums(b"BBox").as_slice()) else {
            return;
        };
        // Transformed bounding box, then the map from it onto Rect (Algorithm 8.1).
        let corners = [
            interp::apply(matrix, bbox[0], bbox[1]),
            interp::apply(matrix, bbox[2], bbox[3]),
            interp::apply(matrix, bbox[0], bbox[3]),
            interp::apply(matrix, bbox[2], bbox[1]),
        ];
        let xs = corners.iter().map(|c| c.0);
        let ys = corners.iter().map(|c| c.1);
        let (x0, x1) = (
            xs.clone().fold(f32::INFINITY, f32::min),
            xs.fold(f32::NEG_INFINITY, f32::max),
        );
        let (y0, y1) = (
            ys.clone().fold(f32::INFINITY, f32::min),
            ys.fold(f32::NEG_INFINITY, f32::max),
        );
        if !(x1 - x0 > 1e-3 && y1 - y0 > 1e-3) {
            return;
        }
        let sx = (rect[2] - rect[0]) / (x1 - x0);
        let sy = (rect[3] - rect[1]) / (y1 - y0);
        let a = [sx, 0.0, 0.0, sy, rect[0] - x0 * sx, rect[1] - y0 * sy];
        let res = s
            .dict
            .get(b"Resources")
            .ok()
            .map(|o| resolve(doc, o))
            .and_then(|o| o.as_dict().ok());
        it.run_form(&content, mul(matrix, a), Some(&bbox), res);
    } else if let Some((content, alpha)) = annot::fallback_appearance(doc, d, rect) {
        it.set_fill_alpha(alpha);
        it.run_form(&content, interp::IDENTITY, None, None);
        it.set_fill_alpha(1.0);
    }
}

fn dest_target(
    doc: &Document,
    dest: &Object,
    pages: &[PageInfo],
    depth: u32,
) -> Option<LinkTarget> {
    if depth > 8 {
        return None;
    }
    match resolve(doc, dest) {
        Object::Array(a) => {
            let index = match a.first()? {
                Object::Reference(id) => PdfDocument::page_index(pages, *id)?,
                Object::Integer(i) => usize::try_from(*i).ok()?,
                _ => return None,
            };
            let info = pages.get(index)?;
            let kind = a.get(1).and_then(|o| o.as_name().ok()).unwrap_or(b"Fit");
            let top_user = match kind {
                b"XYZ" => a.get(3).and_then(|o| resolve(doc, o).as_float().ok()),
                b"FitH" | b"FitBH" => a.get(2).and_then(|o| resolve(doc, o).as_float().ok()),
                b"FitR" => a.get(5).and_then(|o| resolve(doc, o).as_float().ok()),
                _ => None,
            };
            let top = top_user.map(|y| {
                let base = page_matrix(info.crop, info.rotate);
                interp::apply(base, info.crop[0], y)
                    .1
                    .clamp(0.0, info.height)
            });
            Some(LinkTarget::Page { index, top })
        }
        Object::Dictionary(d) => dest_target(doc, d.get(b"D").ok()?, pages, depth + 1),
        Object::String(name, _) | Object::Name(name) => {
            let d = named_dest(doc, name)?;
            dest_target(doc, &d, pages, depth + 1)
        }
        _ => None,
    }
}

/// Look up a named destination (`/Dests` dictionary or the `/Names /Dests` name tree).
fn named_dest(doc: &Document, name: &[u8]) -> Option<Object> {
    let cat = doc.catalog().ok()?;
    if let Some(d) = cat
        .get(b"Dests")
        .ok()
        .and_then(|o| resolve(doc, o).as_dict().ok())
    {
        if let Ok(v) = d.get(name) {
            return Some(resolve(doc, v).clone());
        }
    }
    let names = cat
        .get(b"Names")
        .ok()
        .and_then(|o| resolve(doc, o).as_dict().ok())?;
    let tree = names
        .get(b"Dests")
        .ok()
        .and_then(|o| resolve(doc, o).as_dict().ok())?;
    fn find(doc: &Document, node: &Dictionary, name: &[u8], depth: u32) -> Option<Object> {
        if depth > 32 {
            return None;
        }
        if let Some(n) = node
            .get(b"Names")
            .ok()
            .and_then(|o| resolve(doc, o).as_array().ok())
        {
            for pair in n.chunks(2) {
                if let [k, v] = pair {
                    if resolve(doc, k).as_str().ok() == Some(name) {
                        return Some(resolve(doc, v).clone());
                    }
                }
            }
        }
        let kids = node
            .get(b"Kids")
            .ok()
            .and_then(|o| resolve(doc, o).as_array().ok())?;
        for k in kids {
            let Ok(kd) = resolve(doc, k).as_dict() else {
                continue;
            };
            if let Some(lim) = kd
                .get(b"Limits")
                .ok()
                .and_then(|o| resolve(doc, o).as_array().ok())
            {
                let lo = lim.first().and_then(|o| resolve(doc, o).as_str().ok());
                let hi = lim.get(1).and_then(|o| resolve(doc, o).as_str().ok());
                if let (Some(lo), Some(hi)) = (lo, hi) {
                    if name < lo || name > hi {
                        continue;
                    }
                }
            }
            if let Some(v) = find(doc, kd, name, depth + 1) {
                return Some(v);
            }
        }
        None
    }
    find(doc, tree, name, 0)
}

fn action_target(doc: &Document, a: &Dictionary, pages: &[PageInfo]) -> Option<LinkTarget> {
    match a.get(b"S").and_then(Object::as_name).ok()? {
        b"GoTo" => dest_target(doc, a.get(b"D").ok()?, pages, 0),
        b"URI" => {
            let u = resolve(doc, a.get(b"URI").ok()?).as_str().ok()?;
            Some(LinkTarget::Uri(String::from_utf8_lossy(u).into_owned()))
        }
        _ => None,
    }
}

fn link_target(doc: &Document, d: &Dictionary, pages: &[PageInfo]) -> Option<LinkTarget> {
    if let Ok(dest) = d.get(b"Dest") {
        return dest_target(doc, dest, pages, 0);
    }
    let a = resolve(doc, d.get(b"A").ok()?).as_dict().ok()?;
    action_target(doc, a, pages)
}

fn outline_children(
    doc: &Document,
    parent: &Dictionary,
    pages: &[PageInfo],
    seen: &mut HashSet<ObjectId>,
    depth: u32,
) -> Vec<OutlineItem> {
    let mut out = Vec::new();
    if depth > 32 {
        return out;
    }
    let mut next = parent
        .get(b"First")
        .ok()
        .and_then(|o| o.as_reference().ok());
    while let Some(id) = next {
        if !seen.insert(id) || seen.len() > 100_000 {
            break;
        }
        let Ok(d) = doc.get_dictionary(id) else { break };
        let title = d
            .get(b"Title")
            .ok()
            .and_then(|o| resolve(doc, o).as_str().ok())
            .map(text_string)
            .unwrap_or_default();
        let target = d
            .get(b"Dest")
            .ok()
            .and_then(|o| dest_target(doc, o, pages, 0))
            .or_else(|| {
                let a = resolve(doc, d.get(b"A").ok()?).as_dict().ok()?;
                action_target(doc, a, pages)
            });
        let open = d
            .get(b"Count")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .is_some_and(|c| c > 0);
        let children = outline_children(doc, d, pages, seen, depth + 1);
        out.push(OutlineItem {
            title: title.replace(['\r', '\n'], " "),
            target,
            open,
            children,
        });
        next = d.get(b"Next").ok().and_then(|o| o.as_reference().ok());
    }
    out
}

/// Optional content groups that are off in the default configuration (§8.11.4.3).
fn hidden_groups(doc: &Document) -> HashSet<ObjectId> {
    let mut out = HashSet::new();
    let Some(props) = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"OCProperties").ok())
        .and_then(|o| resolve(doc, o).as_dict().ok())
    else {
        return out;
    };
    let refs = |o: Option<&Object>| -> Vec<ObjectId> {
        o.map(|o| resolve(doc, o))
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
            .unwrap_or_default()
    };
    let Some(d) = props
        .get(b"D")
        .ok()
        .and_then(|o| resolve(doc, o).as_dict().ok())
    else {
        return out;
    };
    if d.get(b"BaseState").and_then(Object::as_name).ok() == Some(b"OFF") {
        let on: HashSet<ObjectId> = refs(d.get(b"ON").ok()).into_iter().collect();
        for g in refs(props.get(b"OCGs").ok()) {
            if !on.contains(&g) {
                out.insert(g);
            }
        }
    }
    out.extend(refs(d.get(b"OFF").ok()));
    out
}

#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod forms_tests;
#[cfg(test)]
mod objects_tests;
#[cfg(test)]
mod tests;
