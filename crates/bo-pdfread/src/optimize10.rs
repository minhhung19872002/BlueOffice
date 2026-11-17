//! PDF Optimizer, round 10 (BlueEditor File › PDF Optimizer › Advanced Optimization, laid out as
//! the reference editor's): every panel of the dialog does its work here.
//!
//! - Compress Images: colour / greyscale pictures downsampled (average, bicubic or subsampling)
//!   above a resolution and written as JPEG or Flate (ZIP); monochrome pictures (1 bit, image
//!   masks) downsampled and written as JBIG2 (lossless MMR region), CCITT Group 3 / Group 4,
//!   Flate or run length (ISO 32000-1 §7.4.4–§7.4.7, `ccitt_enc`).
//! - Unembed Fonts: the list of embedded fonts, the chosen ones unembedded (`/FontFile*` taken
//!   out of the descriptor, §9.8), and every other TrueType program subset to the glyphs the
//!   pages show (`font_subset`).
//! - Discard Objects / Discard User Data: form actions, JavaScript, alternate images,
//!   thumbnails, tags, print settings, bookmarks, comments / forms / media, external
//!   references, metadata, attachments, private application data, hidden layers.
//! - Clean Up: object streams or not, LZW streams written with Flate, links to pages that do
//!   not exist removed, content streams rewritten compactly.
//! - Audit space usage: bytes of the file by kind of object, as the reference's dialog.
use crate::color::ColorSpace;
use crate::sanitize::HiddenData;
use crate::{inherited, resolve, PdfDocument, SaveOptions};
use lopdf::{Dictionary, Object, ObjectId, Stream};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Downsampling method of the Compress Images panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Downsample {
    /// Keep the resolution.
    Off,
    /// Average of the covered pixels (box filter).
    Average,
    /// Bicubic interpolation (Catmull-Rom) after averaging.
    #[default]
    Bicubic,
    /// The centre pixel of each covered block.
    Subsample,
}

/// Compression of colour / greyscale pictures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorCompression {
    /// JPEG for photos that were JPEG, Flate for the rest.
    Retain,
    /// Baseline JPEG.
    #[default]
    Jpeg,
    /// Flate (lossless).
    Zip,
}

/// Compression of monochrome pictures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MonoCompression {
    /// The picture's own filter when it is rewritten, else untouched.
    Retain,
    /// JBIG2 (lossless MMR generic region).
    #[default]
    Jbig2,
    /// CCITT Group 3 (one-dimensional).
    Ccitt3,
    /// CCITT Group 4.
    Ccitt4,
    /// Flate.
    Zip,
    /// Run length.
    RunLength,
}

/// One resolution rule: downsample to `ppi` the pictures above `above` ppi.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resample {
    /// How pixels are combined.
    pub method: Downsample,
    /// Target resolution.
    pub ppi: f32,
    /// Only pictures above this resolution are downsampled.
    pub above: f32,
}

/// Objects and data to discard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Discard {
    /// Submit / import / reset form actions.
    pub form_actions: bool,
    /// Form fields drawn into the page.
    pub flatten_fields: bool,
    /// JavaScript actions.
    pub javascript: bool,
    /// `/Alternates` of pictures.
    pub alternate_images: bool,
    /// Page thumbnails.
    pub thumbnails: bool,
    /// Structure tree and tags.
    pub tags: bool,
    /// Print settings of the viewer preferences.
    pub print_settings: bool,
    /// Bookmarks.
    pub bookmarks: bool,
    /// Comments, form fields and media.
    pub comments_forms_media: bool,
    /// Actions to other files (`GoToR`, `Launch`).
    pub external_refs: bool,
    /// Document information and metadata.
    pub metadata: bool,
    /// File attachments.
    pub attachments: bool,
    /// `/PieceInfo` data of other applications.
    pub private_data: bool,
    /// Content of hidden layers, and the layers.
    pub hidden_layers: bool,
}

/// Clean Up panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct CleanUp {
    /// Pack objects into object streams ("Compress entire file").
    pub object_streams: bool,
    /// Flate-compress streams without a filter.
    pub flate_unencoded: bool,
    /// Rewrite LZW streams with Flate.
    pub lzw_to_flate: bool,
    /// Remove links whose destination page does not exist.
    pub invalid_links: bool,
    /// Rewrite content streams compactly.
    pub optimize_content: bool,
}

impl Default for CleanUp {
    fn default() -> Self {
        Self {
            object_streams: true,
            flate_unencoded: true,
            lzw_to_flate: true,
            invalid_links: true,
            optimize_content: true,
        }
    }
}

/// Everything the PDF Optimizer dialog sets.
#[derive(Debug, Clone, PartialEq)]
pub struct Optimize10 {
    /// Compress Images panel on.
    pub images: bool,
    /// Colour / greyscale resolution rule.
    pub color: Resample,
    /// Colour / greyscale compression.
    pub color_compression: ColorCompression,
    /// JPEG quality 1–100.
    pub jpeg_quality: u8,
    /// Monochrome resolution rule.
    pub mono: Resample,
    /// Monochrome compression.
    pub mono_compression: MonoCompression,
    /// Keep a picture's old data when the new one is not smaller.
    pub only_smaller: bool,
    /// Unembed Fonts panel on: `unembed` are taken out, `subset` subsets the others.
    pub fonts: bool,
    /// Fonts to unembed (names as `embedded_fonts` lists them).
    pub unembed: Vec<String>,
    /// Subset the other embedded TrueType fonts.
    pub subset: bool,
    /// Discard Objects and Discard User Data panels.
    pub discard: Discard,
    /// Clean Up panel.
    pub clean: CleanUp,
    /// Make compatible with this PDF version ("1.3" … "1.7"); `None` keeps the file's.
    pub version: Option<String>,
}

impl Default for Optimize10 {
    fn default() -> Self {
        Self {
            images: true,
            color: Resample {
                method: Downsample::Bicubic,
                ppi: 150.0,
                above: 225.0,
            },
            color_compression: ColorCompression::Jpeg,
            jpeg_quality: 50,
            mono: Resample {
                method: Downsample::Bicubic,
                ppi: 300.0,
                above: 450.0,
            },
            mono_compression: MonoCompression::Jbig2,
            only_smaller: true,
            fonts: true,
            unembed: Vec::new(),
            subset: true,
            discard: Discard::default(),
            clean: CleanUp::default(),
            version: None,
        }
    }
}

/// What the optimizer did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report10 {
    /// File size before.
    pub before: usize,
    /// File size after.
    pub after: usize,
    /// Colour / greyscale pictures rewritten.
    pub images: usize,
    /// Monochrome pictures rewritten.
    pub mono: usize,
    /// Fonts subset.
    pub subset: usize,
    /// Fonts unembedded.
    pub unembedded: usize,
}

/// An embedded font of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedFont {
    /// `/BaseFont` without a subset tag.
    pub name: String,
    /// Already a subset (`ABCDEF+Name`).
    pub subset: bool,
    /// Bytes of the font program.
    pub bytes: usize,
}

/// Kinds of the Audit Space Usage dialog, in its order.
pub const AUDIT_KINDS: [&str; 12] = [
    "Images",
    "Bookmarks",
    "Thumbnails",
    "Comments",
    "Forms",
    "Fonts",
    "Content Streams",
    "Structure Info",
    "Named Destinations",
    "Document Overhead",
    "Color Spaces",
    "Cross Reference Table",
];

const IMAGES: usize = 0;
const BOOKMARKS: usize = 1;
const THUMBNAILS: usize = 2;
const COMMENTS: usize = 3;
const FORMS: usize = 4;
const FONTS: usize = 5;
const CONTENT: usize = 6;
const STRUCTURE: usize = 7;
const DESTS: usize = 8;
const OVERHEAD: usize = 9;
const COLORSPACES: usize = 10;
const XREF: usize = 11;

/// Separable Catmull-Rom resampling of RGBA `src` (`w` × `h`) to `nw` × `nh`; a box filter
/// first brings large reductions within two pixels a step.
fn bicubic(src: &[u8], w: usize, h: usize, nw: usize, nh: usize) -> Vec<u8> {
    let (mut cur, mut cw, mut ch) = (src.to_vec(), w, h);
    let (bw, bh) = ((nw * 2).min(w), (nh * 2).min(h));
    if bw < w || bh < h {
        cur = crate::optimize::downsample(&cur, w, h, bw, bh);
        cw = bw;
        ch = bh;
    }
    let kernel = |t: f32| {
        let t = t.abs();
        if t < 1.0 {
            1.5 * t * t * t - 2.5 * t * t + 1.0
        } else if t < 2.0 {
            -0.5 * t * t * t + 2.5 * t * t - 4.0 * t + 2.0
        } else {
            0.0
        }
    };
    let pass = |data: &[u8], sw: usize, sh: usize, dw: usize, dh: usize, horiz: bool| {
        let mut out = vec![0_u8; dw * dh * 4];
        let (n_src, n_dst) = if horiz { (sw, dw) } else { (sh, dh) };
        let scale = n_src as f32 / n_dst as f32;
        for d in 0..n_dst {
            let c = (d as f32 + 0.5) * scale - 0.5;
            let base = c.floor() as isize;
            let mut taps = [(0_usize, 0.0_f32); 4];
            let mut sum = 0.0;
            for (k, tap) in taps.iter_mut().enumerate() {
                let i = base - 1 + k as isize;
                let wgt = kernel(c - i as f32);
                *tap = (i.clamp(0, n_src as isize - 1) as usize, wgt);
                sum += wgt;
            }
            let others = if horiz { dh } else { dw };
            for o in 0..others {
                let mut acc = [0.0_f32; 4];
                for (i, wgt) in taps {
                    let at = if horiz { o * sw + i } else { i * sw + o };
                    for (ch_i, a) in acc.iter_mut().enumerate() {
                        *a += f32::from(data[at * 4 + ch_i]) * wgt;
                    }
                }
                let to = if horiz { o * dw + d } else { d * dw + o };
                for (ch_i, a) in acc.iter().enumerate() {
                    out[to * 4 + ch_i] = (a / sum).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        out
    };
    let hpass = pass(&cur, cw, ch, nw, ch, true);
    pass(&hpass, nw, ch, nw, nh, false)
}

/// Nearest-pixel resampling (the centre of each block).
fn subsample(src: &[u8], w: usize, h: usize, nw: usize, nh: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(nw * nh * 4);
    for y in 0..nh {
        let sy = ((y * 2 + 1) * h / (nh * 2)).min(h - 1);
        for x in 0..nw {
            let sx = ((x * 2 + 1) * w / (nw * 2)).min(w - 1);
            out.extend_from_slice(&src[(sy * w + sx) * 4..(sy * w + sx) * 4 + 4]);
        }
    }
    out
}

fn resample(m: Downsample, px: &[u8], w: usize, h: usize, nw: usize, nh: usize) -> Vec<u8> {
    match m {
        Downsample::Off => px.to_vec(),
        Downsample::Average => crate::optimize::downsample(px, w, h, nw, nh),
        Downsample::Bicubic => bicubic(px, w, h, nw, nh),
        Downsample::Subsample => subsample(px, w, h, nw, nh),
    }
}

/// Run-length encoding (§7.4.5).
pub(crate) fn run_length(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 2 + 2);
    let mut i = 0;
    while i < data.len() {
        let mut j = i + 1;
        while j < data.len() && j - i < 128 && data[j] == data[i] {
            j += 1;
        }
        if j - i >= 2 {
            out.push((257 - (j - i)) as u8);
            out.push(data[i]);
            i = j;
            continue;
        }
        // Literal run until two equal bytes start.
        let mut k = i + 1;
        while k < data.len() && k - i < 128 && !(k + 1 < data.len() && data[k] == data[k + 1]) {
            k += 1;
        }
        out.push((k - i - 1) as u8);
        out.extend_from_slice(&data[i..k]);
        i = k;
    }
    out.push(128);
    out
}

/// Rows of 1-bit samples (1 = white), packed.
fn pack_bits(black: &[u8], w: usize, h: usize) -> Vec<u8> {
    let stride = w.div_ceil(8);
    let mut out = vec![0_u8; stride * h];
    for y in 0..h {
        for x in 0..w {
            if black[y * w + x] == 0 {
                out[y * stride + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    out
}

/// CCITT Group 3 one-dimensional coding (Modified Huffman rows, no EOLs).
fn g3(black: &[u8], w: usize, h: usize) -> Vec<u8> {
    crate::ccitt_enc::g3_1d(black, w, h)
}

fn filter_is(d: &Dictionary, n: &[u8]) -> bool {
    match d.get(b"Filter") {
        Ok(Object::Name(f)) => f == n,
        Ok(Object::Array(a)) => a.iter().any(|f| f.as_name().ok() == Some(n)),
        _ => false,
    }
}

impl PdfDocument {
    /// Compress the pictures as the Compress Images panel says (inside the open step).
    fn compress_images10(
        &mut self,
        o: &Optimize10,
        jpeg: crate::JpegEncoder<'_>,
        rep: &mut Report10,
    ) {
        let sizes = self.drawn_sizes();
        for (id, (pw, ph)) in sizes {
            if pw < 1.0 || ph < 1.0 {
                continue;
            }
            let Ok(Object::Stream(s)) = self.doc.get_object(id) else {
                continue;
            };
            let d = &s.dict;
            if d.has(b"Mask") || d.has(b"Decode") || d.has(b"JBIG2Globals") {
                continue;
            }
            let mask = d
                .get(b"ImageMask")
                .and_then(Object::as_bool)
                .unwrap_or(false);
            let bpc = d
                .get(b"BitsPerComponent")
                .and_then(Object::as_i64)
                .unwrap_or(8);
            let w = d.get(b"Width").and_then(Object::as_i64).unwrap_or(0);
            let h = d.get(b"Height").and_then(Object::as_i64).unwrap_or(0);
            let (Ok(w), Ok(h)) = (usize::try_from(w), usize::try_from(h)) else {
                continue;
            };
            if w == 0 || h == 0 {
                continue;
            }
            let none = |_: &[u8]| None;
            let space = if mask {
                None
            } else {
                d.get(b"ColorSpace")
                    .ok()
                    .and_then(|c| ColorSpace::parse(&self.doc, c, &none, 0))
            };
            let mono = mask || (bpc == 1 && matches!(space, Some(ColorSpace::Gray)));
            let rule = if mono { o.mono } else { o.color };
            let tw = (pw / 72.0 * rule.ppi).ceil().max(1.0) as usize;
            let th = (ph / 72.0 * rule.ppi).ceil().max(1.0) as usize;
            let above = rule.above / rule.ppi.max(1.0);
            let shrink = rule.method != Downsample::Off
                && rule.ppi > 0.0
                && ((w as f32) > tw as f32 * above || (h as f32) > th as f32 * above);
            let k = if shrink {
                (tw as f32 / w as f32).max(th as f32 / h as f32).min(1.0)
            } else {
                1.0
            };
            let (nw, nh) = (
                ((w as f32 * k).round() as usize).max(1),
                ((h as f32 * k).round() as usize).max(1),
            );
            let was_jpeg = filter_is(d, b"DCTDecode");
            let old_len = s.content.len();
            let mut nd = Dictionary::new();
            nd.set("Type", Object::Name(b"XObject".to_vec()));
            nd.set("Subtype", Object::Name(b"Image".to_vec()));
            nd.set("Width", nw as i64);
            nd.set("Height", nh as i64);
            for key in [b"Interpolate".as_slice(), b"SMask", b"Intent", b"OC"] {
                if let Ok(v) = d.get(key) {
                    nd.set(key.to_vec(), v.clone());
                }
            }
            let data = if mono {
                let comp = match o.mono_compression {
                    MonoCompression::Retain if !shrink => continue,
                    MonoCompression::Retain if filter_is(d, b"JBIG2Decode") => {
                        MonoCompression::Jbig2
                    }
                    MonoCompression::Retain if filter_is(d, b"CCITTFaxDecode") => {
                        MonoCompression::Ccitt4
                    }
                    MonoCompression::Retain if filter_is(d, b"RunLengthDecode") => {
                        MonoCompression::RunLength
                    }
                    MonoCompression::Retain => MonoCompression::Zip,
                    c => c,
                };
                let Some(img) = crate::image::decode(&crate::image::Params {
                    doc: &self.doc,
                    dict: d,
                    data: &s.content,
                    space: space.clone(),
                    fill: [0.0; 3],
                }) else {
                    continue;
                };
                if img.width as usize != w || img.height as usize != h {
                    continue;
                }
                // 0 / 255 grey in RGBA, resampled, then thresholded.
                let grey: Vec<u8> = img
                    .pixels
                    .chunks_exact(4)
                    .flat_map(|p| {
                        let black = if mask {
                            p[3] > 127
                        } else {
                            u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]) < 384
                        };
                        let v = if black { 0 } else { 255 };
                        [v, v, v, 255]
                    })
                    .collect();
                let small = if shrink {
                    resample(rule.method, &grey, w, h, nw, nh)
                } else {
                    grey
                };
                // Downsampled strokes keep their weight: a pixel a third black or more stays
                // black (a 50 % cut thins text).
                let cut = if shrink { 170 } else { 128 };
                let black: Vec<u8> = small
                    .chunks_exact(4)
                    .map(|p| u8::from(p[0] < cut))
                    .collect();
                nd.set("BitsPerComponent", 1);
                if mask {
                    nd.set("ImageMask", true);
                } else {
                    nd.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
                }
                match comp {
                    MonoCompression::Jbig2 => {
                        nd.set("Filter", Object::Name(b"JBIG2Decode".to_vec()));
                        crate::ccitt_enc::jbig2_mmr(&black, nw, nh)
                    }
                    MonoCompression::Ccitt3 | MonoCompression::Ccitt4 => {
                        let g4 = comp == MonoCompression::Ccitt4;
                        nd.set("Filter", Object::Name(b"CCITTFaxDecode".to_vec()));
                        let mut p = Dictionary::new();
                        p.set("K", if g4 { -1 } else { 0 });
                        p.set("Columns", nw as i64);
                        p.set("Rows", nh as i64);
                        if !g4 {
                            p.set("EndOfBlock", false);
                        }
                        nd.set("DecodeParms", p);
                        if g4 {
                            crate::ccitt_enc::g4(&black, nw, nh)
                        } else {
                            g3(&black, nw, nh)
                        }
                    }
                    MonoCompression::RunLength => {
                        nd.set("Filter", Object::Name(b"RunLengthDecode".to_vec()));
                        run_length(&pack_bits(&black, nw, nh))
                    }
                    _ => {
                        nd.set("Filter", Object::Name(b"FlateDecode".to_vec()));
                        miniz_oxide::deflate::compress_to_vec_zlib(&pack_bits(&black, nw, nh), 9)
                    }
                }
            } else {
                let simple = matches!(
                    space,
                    Some(ColorSpace::Gray | ColorSpace::Rgb | ColorSpace::Indexed { .. })
                );
                if !simple && !was_jpeg {
                    continue;
                }
                let gray = matches!(space, Some(ColorSpace::Gray));
                let as_jpeg = match o.color_compression {
                    ColorCompression::Retain => was_jpeg && !gray,
                    ColorCompression::Jpeg => true,
                    ColorCompression::Zip => false,
                };
                // Nothing to do: same size and already in the asked form.
                if !shrink
                    && (as_jpeg == was_jpeg || o.color_compression == ColorCompression::Retain)
                {
                    continue;
                }
                let Some(img) = crate::image::decode(&crate::image::Params {
                    doc: &self.doc,
                    dict: d,
                    data: &s.content,
                    space: space.clone(),
                    fill: [0.0; 3],
                }) else {
                    continue;
                };
                if img.width as usize != w || img.height as usize != h {
                    continue;
                }
                let small = if shrink {
                    resample(rule.method, &img.pixels, w, h, nw, nh)
                } else {
                    img.pixels
                };
                nd.set("BitsPerComponent", 8);
                if as_jpeg {
                    let Some(j) = jpeg(&small, nw as u32, nh as u32, o.jpeg_quality) else {
                        continue;
                    };
                    nd.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
                    nd.set("Filter", Object::Name(b"DCTDecode".to_vec()));
                    j
                } else {
                    let samples: Vec<u8> = if gray {
                        small.chunks_exact(4).map(|p| p[0]).collect()
                    } else {
                        small
                            .chunks_exact(4)
                            .flat_map(|p| [p[0], p[1], p[2]])
                            .collect()
                    };
                    nd.set(
                        "ColorSpace",
                        Object::Name(if gray {
                            b"DeviceGray".to_vec()
                        } else {
                            b"DeviceRGB".to_vec()
                        }),
                    );
                    nd.set("Filter", Object::Name(b"FlateDecode".to_vec()));
                    miniz_oxide::deflate::compress_to_vec_zlib(&samples, 9)
                }
            };
            if o.only_smaller && data.len() >= old_len {
                continue;
            }
            self.touch(id);
            if let Some(obj) = self.doc.objects.get_mut(&id) {
                *obj = Object::Stream(Stream::new(nd, data));
                if mono {
                    rep.mono += 1;
                } else {
                    rep.images += 1;
                }
            }
        }
    }

    /// Font dictionaries (by id) with their descriptor id and font program key / id.
    fn font_programs(&self) -> Vec<(ObjectId, String, ObjectId, Vec<u8>, ObjectId)> {
        let mut out = Vec::new();
        for (id, o) in &self.doc.objects {
            let Ok(d) = o.as_dict() else { continue };
            if d.get(b"Type").and_then(Object::as_name).ok() != Some(b"Font") {
                continue;
            }
            let sub = d.get(b"Subtype").and_then(Object::as_name).unwrap_or(b"");
            if sub == b"CIDFontType0" || sub == b"CIDFontType2" {
                // Reached through their Type0 parent.
                continue;
            }
            let name = d
                .get(b"BaseFont")
                .and_then(Object::as_name)
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_default();
            let leaf = if sub == b"Type0" {
                d.get(b"DescendantFonts")
                    .map(|o| resolve(&self.doc, o))
                    .and_then(Object::as_array)
                    .ok()
                    .and_then(|a| a.first())
                    .map(|o| resolve(&self.doc, o))
                    .and_then(|o| o.as_dict().ok())
            } else {
                Some(d)
            };
            let Some(desc_ref) = leaf.and_then(|l| l.get(b"FontDescriptor").ok()) else {
                continue;
            };
            let Ok(desc_id) = desc_ref.as_reference() else {
                continue;
            };
            let Ok(desc) = self.doc.get_dictionary(desc_id) else {
                continue;
            };
            for key in [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"] {
                if let Ok(fid) = desc.get(key).and_then(Object::as_reference) {
                    out.push((*id, name.clone(), desc_id, key.to_vec(), fid));
                }
            }
        }
        out
    }

    /// The embedded fonts (Unembed Fonts panel), by name.
    pub fn embedded_fonts(&self) -> Vec<EmbeddedFont> {
        let mut out: Vec<EmbeddedFont> = Vec::new();
        for (_, name, _, _, fid) in self.font_programs() {
            let bytes = match self.doc.get_object(fid) {
                Ok(Object::Stream(s)) => s.content.len(),
                _ => 0,
            };
            let (subset, bare) = match name.split_once('+') {
                Some((tag, rest))
                    if tag.len() == 6 && tag.bytes().all(|b| b.is_ascii_uppercase()) =>
                {
                    (true, rest.to_owned())
                }
                _ => (false, name.clone()),
            };
            if let Some(e) = out.iter_mut().find(|e| e.name == bare) {
                e.bytes += bytes;
            } else {
                out.push(EmbeddedFont {
                    name: bare,
                    subset,
                    bytes,
                });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// Codes shown with each font (font dictionary id → codes), and fonts that cannot be
    /// subset safely (form field defaults, unreadable content).
    fn shown_codes(&self) -> (HashMap<ObjectId, BTreeSet<u32>>, HashSet<ObjectId>) {
        let mut codes: HashMap<ObjectId, BTreeSet<u32>> = HashMap::new();
        let mut blocked: HashSet<ObjectId> = HashSet::new();
        let mut seen: HashSet<ObjectId> = HashSet::new();
        // (content bytes, resources) to read.
        let mut todo: Vec<(Vec<u8>, Option<Dictionary>)> = Vec::new();
        let doc = &self.doc;
        for p in &self.pages {
            let Ok(page) = doc.get_dictionary(p.id) else {
                continue;
            };
            let res = inherited(doc, page, b"Resources")
                .and_then(|r| r.as_dict().ok())
                .cloned();
            todo.push((crate::page_content(doc, page), res));
            // Appearance streams of the annotations.
            if let Ok(annots) = page
                .get(b"Annots")
                .map(|a| resolve(doc, a))
                .and_then(Object::as_array)
            {
                for a in annots {
                    let Ok(ad) = resolve(doc, a).as_dict() else {
                        continue;
                    };
                    let Ok(ap) = ad
                        .get(b"AP")
                        .map(|o| resolve(doc, o))
                        .and_then(Object::as_dict)
                    else {
                        continue;
                    };
                    for (_, v) in ap {
                        let v = resolve(doc, v);
                        let mut streams = Vec::new();
                        match v {
                            Object::Stream(s) => streams.push(s),
                            Object::Dictionary(states) => {
                                for (_, sv) in states {
                                    if let Object::Stream(s) = resolve(doc, sv) {
                                        streams.push(s);
                                    }
                                }
                            }
                            _ => {}
                        }
                        for s in streams {
                            let r = s
                                .dict
                                .get(b"Resources")
                                .map(|r| resolve(doc, r))
                                .and_then(Object::as_dict)
                                .ok()
                                .cloned();
                            if let Ok(b) = crate::filters::stream_bytes(doc, s) {
                                todo.push((b, r));
                            }
                        }
                    }
                }
            }
        }
        // Fonts of the form defaults stay whole (new values are typed with them).
        if let Some(dr) = doc
            .catalog()
            .ok()
            .and_then(|c| c.get(b"AcroForm").ok())
            .map(|a| resolve(doc, a))
            .and_then(|a| a.as_dict().ok())
            .and_then(|a| a.get(b"DR").ok())
            .map(|r| resolve(doc, r))
            .and_then(|r| r.as_dict().ok())
            .and_then(|r| r.get(b"Font").ok())
            .map(|f| resolve(doc, f))
            .and_then(|f| f.as_dict().ok())
        {
            for (_, f) in dr {
                if let Ok(id) = f.as_reference() {
                    blocked.insert(id);
                }
            }
        }
        while let Some((bytes, res)) = todo.pop() {
            let fonts: Vec<(Vec<u8>, ObjectId)> = res
                .as_ref()
                .and_then(|r| r.get(b"Font").ok())
                .map(|f| resolve(doc, f))
                .and_then(|f| f.as_dict().ok())
                .map(|f| {
                    f.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_reference().ok()?)))
                        .collect()
                })
                .unwrap_or_default();
            let Ok(content) = lopdf::content::Content::decode(&bytes) else {
                blocked.extend(fonts.iter().map(|f| f.1));
                continue;
            };
            // Form XObjects drawn here.
            if let Some(xo) = res
                .as_ref()
                .and_then(|r| r.get(b"XObject").ok())
                .map(|x| resolve(doc, x))
                .and_then(|x| x.as_dict().ok())
            {
                for (_, v) in xo {
                    let Ok(id) = v.as_reference() else { continue };
                    if !seen.insert(id) {
                        continue;
                    }
                    if let Ok(Object::Stream(s)) = doc.get_object(id) {
                        if s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Form") {
                            let r = s
                                .dict
                                .get(b"Resources")
                                .map(|r| resolve(doc, r))
                                .and_then(Object::as_dict)
                                .ok()
                                .cloned()
                                .or_else(|| res.clone());
                            if let Ok(b) = crate::filters::stream_bytes(doc, s) {
                                todo.push((b, r));
                            }
                        }
                    }
                }
            }
            let mut cur: Option<ObjectId> = None;
            let mut two = false;
            for op in &content.operations {
                let strings: Vec<&[u8]> = match op.operator.as_str() {
                    "Tf" => {
                        let name = op.operands.first().and_then(|o| o.as_name().ok());
                        cur = name.and_then(|n| fonts.iter().find(|f| f.0 == n).map(|f| f.1));
                        two = cur
                            .and_then(|id| doc.get_dictionary(id).ok())
                            .and_then(|d| d.get(b"Subtype").and_then(Object::as_name).ok())
                            == Some(b"Type0");
                        continue;
                    }
                    "Tj" | "'" => op
                        .operands
                        .last()
                        .and_then(|o| o.as_str().ok())
                        .into_iter()
                        .collect(),
                    "\"" => op
                        .operands
                        .get(2)
                        .and_then(|o| o.as_str().ok())
                        .into_iter()
                        .collect(),
                    "TJ" => op
                        .operands
                        .first()
                        .and_then(|o| o.as_array().ok())
                        .map(|a| a.iter().filter_map(|o| o.as_str().ok()).collect())
                        .unwrap_or_default(),
                    _ => continue,
                };
                let Some(f) = cur else { continue };
                let set = codes.entry(f).or_default();
                for s in strings {
                    if two {
                        for c in s.chunks(2) {
                            let hi = u32::from(c[0]);
                            let lo = u32::from(*c.get(1).unwrap_or(&0));
                            set.insert(hi << 8 | lo);
                        }
                    } else {
                        set.extend(s.iter().map(|b| u32::from(*b)));
                    }
                }
            }
        }
        (codes, blocked)
    }

    /// Glyph ids of TrueType program `font` that codes `shown` of font dictionary `fd` use;
    /// `None` when the mapping is not known.
    fn glyphs_of(
        &self,
        fd: &Dictionary,
        font: &[u8],
        shown: &BTreeSet<u32>,
    ) -> Option<BTreeSet<u16>> {
        let doc = &self.doc;
        let sub = fd.get(b"Subtype").and_then(Object::as_name).ok()?;
        let mut out = BTreeSet::new();
        if sub == b"Type0" {
            let enc = fd.get(b"Encoding").and_then(Object::as_name).ok()?;
            if enc != b"Identity-H" && enc != b"Identity-V" {
                return None;
            }
            let leaf = resolve(doc, fd.get(b"DescendantFonts").ok()?)
                .as_array()
                .ok()?
                .first()
                .map(|o| resolve(doc, o))?
                .as_dict()
                .ok()?;
            let map = match leaf.get(b"CIDToGIDMap").map(|m| resolve(doc, m)) {
                Ok(Object::Stream(s)) => Some(crate::filters::stream_bytes(doc, s).ok()?),
                Ok(Object::Name(n)) if n == b"Identity" => None,
                Err(_) => None,
                _ => return None,
            };
            for c in shown {
                let gid = match &map {
                    Some(m) => {
                        let i = *c as usize * 2;
                        u16::from_be_bytes([*m.get(i)?, *m.get(i + 1)?])
                    }
                    None => u16::try_from(*c).ok()?,
                };
                out.insert(gid);
            }
            return Some(out);
        }
        if sub != b"TrueType" {
            return None;
        }
        let face = ttf_parser::Face::parse(font, 0).ok()?;
        let cmap = face.tables().cmap?;
        // Names from /Encoding (base encoding + /Differences), WinAnsi by default.
        let mut names: Vec<Option<String>> = vec![None; 256];
        let enc = fd.get(b"Encoding").map(|e| resolve(doc, e));
        let base_name = match &enc {
            Ok(Object::Name(n)) => Some(n.clone()),
            Ok(Object::Dictionary(d)) => d
                .get(b"BaseEncoding")
                .and_then(Object::as_name)
                .ok()
                .map(<[u8]>::to_vec),
            _ => None,
        };
        let base = base_name
            .as_deref()
            .and_then(crate::encoding::base)
            .unwrap_or_else(crate::encoding::win_ansi);
        for (i, n) in base.iter().enumerate() {
            if !n.is_empty() {
                names[i] = Some((*n).to_owned());
            }
        }
        if let Ok(Object::Dictionary(d)) = &enc {
            if let Ok(diff) = d
                .get(b"Differences")
                .map(|o| resolve(doc, o))
                .and_then(Object::as_array)
            {
                let mut code = 0_usize;
                for o in diff {
                    match o {
                        Object::Integer(i) => code = usize::try_from(*i).unwrap_or(0),
                        Object::Name(n) => {
                            if let Some(slot) = names.get_mut(code) {
                                *slot = Some(String::from_utf8_lossy(n).into_owned());
                            }
                            code += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        for c in shown {
            let code = u8::try_from(*c).ok()?;
            let mut found = false;
            for t in cmap.subtables {
                let p = t.platform_id;
                let candidates: Vec<u32> = match (p, t.encoding_id) {
                    (ttf_parser::PlatformId::Windows, 0) => {
                        vec![u32::from(code), 0xF000 | u32::from(code)]
                    }
                    (ttf_parser::PlatformId::Macintosh, 0) => vec![u32::from(code)],
                    _ if t.is_unicode() => names[usize::from(code)]
                        .as_deref()
                        .and_then(crate::encoding::unicode)
                        .and_then(|s| s.chars().next())
                        .map(|ch| vec![ch as u32])
                        .unwrap_or_default(),
                    _ => Vec::new(),
                };
                for cp in candidates {
                    if let Some(g) = t.glyph_index(cp) {
                        out.insert(g.0);
                        found = true;
                    }
                }
            }
            if let Some(g) = names[usize::from(code)]
                .as_deref()
                .and_then(|n| face.glyph_index_by_name(n))
            {
                out.insert(g.0);
                found = true;
            }
            if !found && *c != 32 {
                return None;
            }
        }
        Some(out)
    }

    /// Unembed the fonts named `names` and subset the other TrueType programs (inside the
    /// open step); returns (subset, unembedded).
    fn fonts10(&mut self, names: &[String], subset: bool) -> (usize, usize) {
        let programs = self.font_programs();
        let (codes, blocked) = if subset {
            self.shown_codes()
        } else {
            (HashMap::new(), HashSet::new())
        };
        let mut done_desc: HashSet<ObjectId> = HashSet::new();
        let (mut subsets, mut unembedded) = (0, 0);
        // Each font program may serve several font dictionaries: the union of their glyphs.
        let mut keep: BTreeMap<ObjectId, Option<BTreeSet<u16>>> = BTreeMap::new();
        for (fdid, name, desc, key, fid) in &programs {
            let bare = name.split_once('+').map_or(name.as_str(), |(_, r)| r);
            if names.iter().any(|n| n == bare || n == name) {
                if done_desc.insert(*desc) {
                    self.touch(*desc);
                    if let Ok(d) = self.doc.get_dictionary_mut(*desc) {
                        d.remove(key);
                        d.remove(b"CIDSet");
                        unembedded += 1;
                    }
                }
                continue;
            }
            if !subset || key != b"FontFile2" || name.contains('+') {
                continue;
            }
            let glyphs = if blocked.contains(fdid) {
                None
            } else {
                let shown = codes.get(fdid).cloned().unwrap_or_default();
                let font = match self.doc.get_object(*fid) {
                    Ok(Object::Stream(s)) => crate::filters::stream_bytes(&self.doc, s).ok(),
                    _ => None,
                };
                match (font, self.doc.get_dictionary(*fdid)) {
                    (Some(f), Ok(fd)) => self.glyphs_of(fd, &f, &shown),
                    _ => None,
                }
            };
            let e = keep.entry(*fid).or_insert_with(|| Some(BTreeSet::new()));
            match (e.as_mut(), glyphs) {
                (Some(set), Some(g)) => set.extend(g),
                _ => *e = None,
            }
        }
        for (fid, glyphs) in keep {
            let Some(glyphs) = glyphs else { continue };
            let Ok(Object::Stream(s)) = self.doc.get_object(fid) else {
                continue;
            };
            let Ok(font) = crate::filters::stream_bytes(&self.doc, s) else {
                continue;
            };
            let Some(small) = crate::font_subset::subset_truetype(&font, &glyphs) else {
                continue;
            };
            if small.len() >= font.len() {
                continue;
            }
            let mut nd = Dictionary::new();
            nd.set("Length1", small.len() as i64);
            nd.set("Filter", Object::Name(b"FlateDecode".to_vec()));
            let z = miniz_oxide::deflate::compress_to_vec_zlib(&small, 9);
            self.touch(fid);
            if let Some(o) = self.doc.objects.get_mut(&fid) {
                *o = Object::Stream(Stream::new(nd, z));
                subsets += 1;
            }
        }
        (subsets, unembedded)
    }

    /// Discard what `d` asks for (inside the open step).
    fn discard10(&mut self, d: &Discard) -> Result<(), String> {
        let hidden = HiddenData {
            metadata: d.metadata,
            attachments: d.attachments,
            comments: d.comments_forms_media,
            forms: d.comments_forms_media,
            hidden_layers: d.hidden_layers,
            bookmarks: d.bookmarks,
            links: false,
            scripts: d.javascript,
        };
        if hidden != HiddenData::default() {
            self.remove_hidden_data(hidden)?;
        }
        if d.flatten_fields && !d.comments_forms_media {
            self.flatten_form()?;
        }
        let root = self.catalog_root()?;
        let ids: Vec<ObjectId> = self.doc.objects.keys().copied().collect();
        for id in ids {
            let mut change: Vec<&[u8]> = Vec::new();
            let mut drop_annot_action = false;
            {
                let Some(o) = self.doc.objects.get(&id) else {
                    continue;
                };
                let dict = match o {
                    Object::Dictionary(x) => x,
                    Object::Stream(s) => &s.dict,
                    _ => continue,
                };
                let ty = dict.get(b"Type").and_then(Object::as_name).unwrap_or(b"");
                if d.thumbnails && ty == b"Page" && dict.has(b"Thumb") {
                    change.push(b"Thumb");
                }
                if d.tags && dict.has(b"StructParents") {
                    change.push(b"StructParents");
                }
                if d.tags && dict.has(b"StructParent") {
                    change.push(b"StructParent");
                }
                if d.alternate_images && dict.has(b"Alternates") {
                    change.push(b"Alternates");
                }
                if d.private_data && dict.has(b"PieceInfo") {
                    change.push(b"PieceInfo");
                }
                if d.form_actions || d.external_refs {
                    if let Ok(a) = dict
                        .get(b"A")
                        .map(|a| resolve(&self.doc, a))
                        .and_then(Object::as_dict)
                    {
                        let s = a.get(b"S").and_then(Object::as_name).unwrap_or(b"");
                        drop_annot_action = (d.form_actions
                            && matches!(s, b"SubmitForm" | b"ImportData" | b"ResetForm"))
                            || (d.external_refs && matches!(s, b"GoToR" | b"Launch"));
                    }
                }
            }
            if id == root {
                for (on, key) in [
                    (d.tags, b"StructTreeRoot".as_slice()),
                    (d.tags, b"MarkInfo"),
                    (d.private_data, b"PieceInfo"),
                ] {
                    if on && self.doc.get_dictionary(root).is_ok_and(|c| c.has(key)) {
                        change.push(key);
                    }
                }
            }
            if drop_annot_action {
                change.push(b"A");
            }
            if change.is_empty() {
                continue;
            }
            self.touch(id);
            if let Some(o) = self.doc.objects.get_mut(&id) {
                let dict = match o {
                    Object::Dictionary(x) => x,
                    Object::Stream(s) => &mut s.dict,
                    _ => continue,
                };
                for k in change {
                    dict.remove(k);
                }
            }
        }
        if d.print_settings {
            let prefs = self
                .doc
                .get_dictionary(root)
                .ok()
                .and_then(|c| c.get(b"ViewerPreferences").ok())
                .cloned();
            match prefs {
                Some(Object::Reference(pid)) => {
                    self.touch(pid);
                    if let Ok(p) = self.doc.get_dictionary_mut(pid) {
                        for k in [
                            b"PrintScaling".as_slice(),
                            b"Duplex",
                            b"PickTrayByPDFSize",
                            b"PrintPageRange",
                            b"NumCopies",
                        ] {
                            p.remove(k);
                        }
                    }
                }
                Some(Object::Dictionary(_)) => {
                    self.touch(root);
                    if let Ok(Object::Dictionary(p)) = self
                        .doc
                        .get_dictionary_mut(root)
                        .and_then(|c| c.get_mut(b"ViewerPreferences"))
                    {
                        for k in [
                            b"PrintScaling".as_slice(),
                            b"Duplex",
                            b"PickTrayByPDFSize",
                            b"PrintPageRange",
                            b"NumCopies",
                        ] {
                            p.remove(k);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Clean Up: LZW → Flate, links to missing pages, compact content (inside the open step).
    fn clean10(&mut self, c: CleanUp) {
        let pages: HashSet<ObjectId> = self.pages.iter().map(|p| p.id).collect();
        let ids: Vec<ObjectId> = self.doc.objects.keys().copied().collect();
        let contents: HashSet<ObjectId> = self
            .pages
            .iter()
            .filter_map(|p| self.doc.get_dictionary(p.id).ok())
            .filter_map(|d| d.get(b"Contents").ok())
            .flat_map(|o| match o {
                Object::Reference(r) => vec![*r],
                Object::Array(a) => a.iter().filter_map(|x| x.as_reference().ok()).collect(),
                _ => Vec::new(),
            })
            .collect();
        for id in ids {
            let Some(Object::Stream(s)) = self.doc.objects.get(&id) else {
                if c.invalid_links {
                    self.drop_dead_links(id, &pages);
                }
                continue;
            };
            let lzw = c.lzw_to_flate && filter_is(&s.dict, b"LZWDecode");
            let content = c.optimize_content && contents.contains(&id);
            if !lzw && !content {
                continue;
            }
            let Ok(mut raw) = crate::filters::stream_bytes(&self.doc, s) else {
                continue;
            };
            if content {
                if let Ok(parsed) = lopdf::content::Content::decode(&raw) {
                    // Inline images or anything the parser cannot write back stay as they were.
                    let unsafe_ops = parsed
                        .operations
                        .iter()
                        .any(|o| o.operator == "BI" || o.operator == "ID");
                    if !unsafe_ops {
                        if let Ok(enc) = parsed.encode() {
                            if enc.len() < raw.len() {
                                raw = enc;
                            }
                        }
                    }
                }
            }
            let mut nd = s.dict.clone();
            nd.remove(b"DecodeParms");
            nd.set("Filter", Object::Name(b"FlateDecode".to_vec()));
            let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 9);
            if !lzw && z.len() >= s.content.len() {
                continue;
            }
            self.touch(id);
            if let Some(o) = self.doc.objects.get_mut(&id) {
                *o = Object::Stream(Stream::new(nd, z));
            }
        }
    }

    /// Remove link annotations of page dictionary `id` whose destination page is gone.
    fn drop_dead_links(&mut self, id: ObjectId, pages: &HashSet<ObjectId>) {
        let Ok(page) = self.doc.get_dictionary(id) else {
            return;
        };
        if page.get(b"Type").and_then(Object::as_name).ok() != Some(b"Page") {
            return;
        }
        let Ok(Object::Array(annots)) = page.get(b"Annots").map(|a| resolve(&self.doc, a)).cloned()
        else {
            return;
        };
        let dead = |o: &Object| -> bool {
            let Ok(a) = resolve(&self.doc, o).as_dict() else {
                return false;
            };
            if a.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Link") {
                return false;
            }
            let dest = a.get(b"Dest").ok().or_else(|| {
                a.get(b"A")
                    .map(|x| resolve(&self.doc, x))
                    .ok()
                    .and_then(|x| x.as_dict().ok())
                    .filter(|x| x.get(b"S").and_then(Object::as_name).ok() == Some(b"GoTo"))
                    .and_then(|x| x.get(b"D").ok())
            });
            match dest.map(|d| resolve(&self.doc, d)) {
                Some(Object::Array(arr)) => match arr.first() {
                    Some(Object::Reference(p)) => !pages.contains(p),
                    _ => false,
                },
                _ => false,
            }
        };
        let keep: Vec<Object> = annots.iter().filter(|a| !dead(a)).cloned().collect();
        if keep.len() == annots.len() {
            return;
        }
        self.touch(id);
        if let Ok(p) = self.doc.get_dictionary_mut(id) {
            p.set("Annots", keep);
        }
    }

    /// Optimize with every panel of the dialog (one undo step) and write the file.
    pub fn optimize10(
        &mut self,
        o: &Optimize10,
        jpeg: crate::JpegEncoder<'_>,
    ) -> Result<(Vec<u8>, Report10), String> {
        let mut rep = Report10 {
            before: self.bytes().len(),
            ..Report10::default()
        };
        self.begin_group("optimize");
        let r = (|| {
            if o.images {
                self.compress_images10(o, jpeg, &mut rep);
            }
            if o.fonts {
                let (s, u) = self.fonts10(&o.unembed, o.subset);
                rep.subset = s;
                rep.unembedded = u;
            }
            self.discard10(&o.discard)?;
            self.clean10(o.clean);
            Ok::<(), String>(())
        })();
        self.end_group();
        r?;
        if let Some(v) = &o.version {
            self.doc.version.clone_from(v);
        }
        // Object streams are PDF 1.5 (§7.5.7).
        let streams_ok = self.doc.version.as_str() >= "1.5";
        let bytes = self.save_full(SaveOptions {
            compress: o.clean.flate_unencoded,
            object_streams: o.clean.object_streams && streams_ok,
            prune: true,
        })?;
        rep.after = bytes.len();
        Ok((bytes, rep))
    }

    /// Pages (0-based) whose own resources use transparency: constant alpha below 1, a soft
    /// mask or a blend mode other than Normal (§11.6), pictures with a soft mask, or groups.
    pub fn transparency_pages(&self) -> Vec<usize> {
        let doc = &self.doc;
        let mut out = Vec::new();
        for (i, p) in self.pages.iter().enumerate() {
            let Ok(page) = doc.get_dictionary(p.id) else {
                continue;
            };
            let Some(res) = inherited(doc, page, b"Resources").and_then(|r| r.as_dict().ok())
            else {
                continue;
            };
            let sub = |key: &[u8]| {
                res.get(key)
                    .ok()
                    .map(|o| resolve(doc, o))
                    .and_then(|o| o.as_dict().ok())
            };
            let gs = sub(b"ExtGState").is_some_and(|g| {
                g.iter().any(|(_, v)| {
                    let Ok(d) = resolve(doc, v).as_dict() else {
                        return false;
                    };
                    let alpha = |k: &[u8]| {
                        d.get(k)
                            .ok()
                            .and_then(|a| {
                                a.as_float()
                                    .ok()
                                    .or_else(|| a.as_i64().ok().map(|i| i as f32))
                            })
                            .is_some_and(|a| a < 1.0)
                    };
                    let smask = d
                        .get(b"SMask")
                        .is_ok_and(|m| m.as_name().ok() != Some(b"None"));
                    let blend = d
                        .get(b"BM")
                        .ok()
                        .and_then(|b| b.as_name().ok())
                        .is_some_and(|b| b != b"Normal" && b != b"Compatible");
                    alpha(b"CA") || alpha(b"ca") || smask || blend
                })
            });
            let xo = sub(b"XObject").is_some_and(|x| {
                x.iter().any(|(_, v)| match resolve(doc, v) {
                    Object::Stream(s) => {
                        s.dict.has(b"SMask")
                            || s.dict
                                .get(b"Group")
                                .ok()
                                .map(|g| resolve(doc, g))
                                .and_then(|g| g.as_dict().ok())
                                .is_some_and(|g| {
                                    g.get(b"S").and_then(Object::as_name).ok()
                                        == Some(b"Transparency")
                                })
                    }
                    _ => false,
                })
            });
            if gs || xo {
                out.push(i);
            }
        }
        out
    }

    /// Flatten transparency (one undo step): each page of `transparency_pages` with a
    /// rendering in `pages` (index, width, height, RGBA of the whole page) is drawn as that
    /// picture; its own content stays under an empty clip (§8.5.4) so its text can still be
    /// searched and copied. Rotated pages are left as they are. Returns the pages flattened.
    pub fn flatten_transparency(
        &mut self,
        pages: &[(usize, u32, u32, Vec<u8>)],
        ppi: f32,
    ) -> Result<usize, String> {
        let wanted = self.transparency_pages();
        let mut n = 0;
        self.begin("flatten transparency");
        for (index, w, h, rgba) in pages {
            if !wanted.contains(index) {
                continue;
            }
            let Some(info) = self.pages.get(*index).cloned() else {
                continue;
            };
            if info.rotate % 360 != 0 || rgba.len() < (*w as usize) * (*h as usize) * 4 {
                continue;
            }
            let Ok(page) = self.doc.get_dictionary(info.id).cloned() else {
                continue;
            };
            let bbox = crate::rect_of(&self.doc, inherited(&self.doc, &page, b"CropBox"))
                .or_else(|| crate::rect_of(&self.doc, inherited(&self.doc, &page, b"MediaBox")))
                .unwrap_or([0.0, 0.0, 612.0, 792.0]);
            let mut res = inherited(&self.doc, &page, b"Resources")
                .and_then(|r| r.as_dict().ok())
                .cloned()
                .unwrap_or_default();
            let mut xo = res
                .get(b"XObject")
                .ok()
                .map(|x| resolve(&self.doc, x))
                .and_then(|x| x.as_dict().ok())
                .cloned()
                .unwrap_or_default();
            let rgb: Vec<u8> = rgba
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            let mut d = Dictionary::new();
            d.set("Type", Object::Name(b"XObject".to_vec()));
            d.set("Subtype", Object::Name(b"Image".to_vec()));
            d.set("Width", i64::from(*w));
            d.set("Height", i64::from(*h));
            d.set("BitsPerComponent", 8);
            d.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
            d.set("Filter", Object::Name(b"FlateDecode".to_vec()));
            let img = self.add(Object::Stream(Stream::new(
                d,
                miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6),
            )));
            let name = format!("BOFlat{}", img.0);
            let pre = self.add(Object::Stream(Stream::new(
                Dictionary::new(),
                b"q 0 0 0 0 re W n\n".to_vec(),
            )));
            let post = self.add(Object::Stream(Stream::new(
                Dictionary::new(),
                format!(
                    "\nQ\nq {} 0 0 {} {} {} cm /{name} Do Q\n",
                    bbox[2] - bbox[0],
                    bbox[3] - bbox[1],
                    bbox[0],
                    bbox[1]
                )
                .into_bytes(),
            )));
            xo.set(name.into_bytes(), img);
            res.set("XObject", xo);
            let mut contents = vec![Object::Reference(pre)];
            match page.get(b"Contents") {
                Ok(Object::Array(a)) => contents.extend(a.iter().cloned()),
                Ok(o) => contents.push(o.clone()),
                Err(_) => {}
            }
            contents.push(Object::Reference(post));
            self.touch(info.id);
            if let Ok(pd) = self.doc.get_dictionary_mut(info.id) {
                pd.set("Resources", res);
                pd.set("Contents", contents);
                n += 1;
            }
            let _ = ppi;
        }
        self.commit();
        Ok(n)
    }

    /// Bytes of the file by kind (`AUDIT_KINDS` order) for Audit Space Usage, from the bytes
    /// the document was opened or last saved with.
    pub fn audit_space(&self) -> Vec<usize> {
        audit(self.bytes())
    }
}

fn lres<'a>(doc: &'a lopdf::Document, o: &'a Object) -> &'a Object {
    doc.dereference(o).map_or(o, |(_, o)| o)
}

fn linherited<'a>(
    doc: &'a lopdf::Document,
    page: &'a Dictionary,
    key: &[u8],
) -> Option<&'a Object> {
    let mut d = page;
    for _ in 0..32 {
        if let Ok(v) = d.get(key) {
            return Some(lres(doc, v));
        }
        d = lres(doc, d.get(b"Parent").ok()?).as_dict().ok()?;
    }
    None
}

/// Byte span of every object in `bytes` (uncompressed objects by their offsets, objects in
/// object streams by their share of the stream), and the bytes of cross-reference sections.
fn spans(doc: &lopdf::Document, bytes: &[u8]) -> (HashMap<ObjectId, usize>, usize) {
    let mut offsets: Vec<(usize, ObjectId)> = Vec::new();
    let mut packed: BTreeMap<u32, Vec<(u16, ObjectId)>> = BTreeMap::new();
    for (num, e) in &doc.reference_table.entries {
        match e {
            lopdf::xref::XrefEntry::Normal { offset, generation } => {
                offsets.push((*offset as usize, (*num, *generation)));
            }
            lopdf::xref::XrefEntry::Compressed { container, index } => {
                packed
                    .entry(*container)
                    .or_default()
                    .push((*index, (*num, 0)));
            }
            _ => {}
        }
    }
    offsets.sort_unstable();
    // Cross-reference tables: "xref" ... "trailer".
    let mut xref = 0;
    let mut marks: Vec<usize> = Vec::new();
    let mut i = 0;
    while let Some(at) = bytes.get(i..).and_then(|s| find(s, b"xref")) {
        let p = i + at;
        let line_start = p == 0 || matches!(bytes[p - 1], b'\n' | b'\r');
        let next = bytes.get(p + 4).copied().unwrap_or(b' ');
        if line_start && matches!(next, b'\n' | b'\r' | b' ') {
            let end = bytes
                .get(p..)
                .and_then(|s| find(s, b"trailer"))
                .map_or(bytes.len(), |t| p + t);
            xref += end - p;
            marks.push(p);
            i = end;
        } else {
            i = p + 4;
        }
    }
    let mut ends: Vec<usize> = offsets.iter().map(|o| o.0).collect();
    ends.extend(marks);
    if let Some(t) = rfind(bytes, b"startxref") {
        ends.push(t);
    }
    ends.push(bytes.len());
    ends.sort_unstable();
    let mut out: HashMap<ObjectId, usize> = HashMap::new();
    for (off, id) in &offsets {
        let end = ends
            .iter()
            .find(|e| **e > *off)
            .copied()
            .unwrap_or(bytes.len());
        out.insert(*id, end - off);
    }
    // XRef streams count as the cross-reference table.
    for (id, o) in &doc.objects {
        if let Object::Stream(s) = o {
            if s.dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"XRef") {
                if let Some(n) = out.remove(id) {
                    xref += n;
                }
            }
        }
    }
    // Objects in object streams: the stream's bytes shared by their uncompressed lengths.
    for (container, members) in packed {
        let Some(total) = out.remove(&(container, 0)) else {
            continue;
        };
        let lens: Vec<usize> = members
            .iter()
            .map(|(_, id)| doc.objects.get(id).map_or(1, |o| obj_len(o).max(1)))
            .collect();
        let sum: usize = lens.iter().sum::<usize>().max(1);
        for ((_, id), l) in members.iter().zip(lens) {
            out.insert(*id, total * l / sum);
        }
    }
    (out, xref)
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

fn rfind(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).rposition(|w| w == n)
}

/// Rough written length of an object (for sharing an object stream).
fn obj_len(o: &Object) -> usize {
    match o {
        Object::Dictionary(d) => {
            4 + d
                .iter()
                .map(|(k, v)| k.len() + 2 + obj_len(v))
                .sum::<usize>()
        }
        Object::Array(a) => 2 + a.iter().map(|v| obj_len(v) + 1).sum::<usize>(),
        Object::String(s, _) => s.len() + 2,
        Object::Name(n) => n.len() + 1,
        Object::Stream(s) => s.content.len() + obj_len(&Object::Dictionary(s.dict.clone())),
        Object::Reference(_) => 8,
        _ => 6,
    }
}

/// Kind of each object, from where it is used.
fn classify(doc: &lopdf::Document) -> HashMap<ObjectId, usize> {
    let mut kind: HashMap<ObjectId, usize> = HashMap::new();
    let mut todo: Vec<(Object, usize)> = Vec::new();
    let push = |todo: &mut Vec<(Object, usize)>, o: &Object, k: usize| todo.push((o.clone(), k));
    // Pages: content, resources, annotations, thumbnails.
    for (_, pid) in doc.get_pages() {
        let Ok(p) = doc.get_dictionary(pid) else {
            continue;
        };
        if let Ok(c) = p.get(b"Contents") {
            push(&mut todo, c, CONTENT);
        }
        if let Ok(t) = p.get(b"Thumb") {
            push(&mut todo, t, THUMBNAILS);
        }
        if let Ok(a) = p.get(b"Annots") {
            for x in lres(doc, a).as_array().map_or(&[][..], Vec::as_slice) {
                let widget = lres(doc, x)
                    .as_dict()
                    .ok()
                    .and_then(|d| d.get(b"Subtype").and_then(Object::as_name).ok())
                    == Some(b"Widget");
                push(&mut todo, x, if widget { FORMS } else { COMMENTS });
            }
        }
        if let Some(r) = linherited(doc, p, b"Resources") {
            resources(doc, r, &mut todo);
        }
    }
    if let Ok(c) = doc.catalog() {
        for (key, k) in [
            (b"Outlines".as_slice(), BOOKMARKS),
            (b"AcroForm", FORMS),
            (b"StructTreeRoot", STRUCTURE),
            (b"Dests", DESTS),
        ] {
            if let Ok(o) = c.get(key) {
                push(&mut todo, o, k);
            }
        }
        if let Ok(n) = c
            .get(b"Names")
            .map(|n| lres(doc, n))
            .and_then(Object::as_dict)
        {
            if let Ok(o) = n.get(b"Dests") {
                push(&mut todo, o, DESTS);
            }
        }
    }
    while let Some((o, k)) = todo.pop() {
        let o = match o {
            Object::Reference(id) => {
                if kind.contains_key(&id) {
                    continue;
                }
                let Ok(t) = doc.get_object(id) else { continue };
                let ty = match t {
                    Object::Dictionary(d) => d.get(b"Type").and_then(Object::as_name).ok(),
                    _ => None,
                };
                if matches!(ty, Some(b"Page" | b"Pages" | b"Catalog")) {
                    continue;
                }
                // Images and fonts met inside other things keep their own kind.
                let own = match t {
                    Object::Stream(s) => {
                        match s.dict.get(b"Subtype").and_then(Object::as_name).ok() {
                            Some(b"Image") => Some(IMAGES),
                            _ => None,
                        }
                    }
                    Object::Dictionary(d) if ty == Some(b"Font") && d.has(b"Subtype") => {
                        Some(FONTS)
                    }
                    _ => None,
                };
                let k = own.unwrap_or(k);
                kind.insert(id, k);
                if let Object::Stream(s) = t {
                    if let Ok(r) = s.dict.get(b"Resources") {
                        resources(doc, r, &mut todo);
                    }
                }
                (t.clone(), k)
            }
            other => (other, k),
        };
        let (o, k) = o;
        match &o {
            Object::Dictionary(d) => {
                for (key, v) in d {
                    if matches!(key.as_slice(), b"Parent" | b"P" | b"Resources") {
                        continue;
                    }
                    let vk = if key.as_slice() == b"ColorSpace" && k != COLORSPACES {
                        COLORSPACES
                    } else {
                        k
                    };
                    todo.push((v.clone(), vk));
                }
            }
            Object::Stream(s) => {
                for (key, v) in &s.dict {
                    if matches!(key.as_slice(), b"Parent" | b"P" | b"Resources") {
                        continue;
                    }
                    let vk = if key.as_slice() == b"ColorSpace" {
                        COLORSPACES
                    } else {
                        k
                    };
                    todo.push((v.clone(), vk));
                }
            }
            Object::Array(a) => {
                for v in a {
                    todo.push((v.clone(), k));
                }
            }
            _ => {}
        }
    }
    kind
}

/// Push the parts of a resource dictionary with their kinds.
fn resources(doc: &lopdf::Document, r: &Object, todo: &mut Vec<(Object, usize)>) {
    let Ok(r) = lres(doc, r).as_dict() else {
        return;
    };
    for (key, v) in r {
        let k = match key.as_slice() {
            b"Font" => FONTS,
            b"ColorSpace" => COLORSPACES,
            b"XObject" => CONTENT,
            _ => OVERHEAD,
        };
        if key.as_slice() == b"XObject" {
            // Form XObjects are content, images are images.
            if let Ok(x) = lres(doc, v).as_dict() {
                for (_, xv) in x {
                    todo.push((xv.clone(), CONTENT));
                }
            }
            continue;
        }
        todo.push((v.clone(), k));
    }
}

/// Audit Space Usage of a PDF file: bytes by `AUDIT_KINDS`.
pub(crate) fn audit(bytes: &[u8]) -> Vec<usize> {
    let mut out = vec![0_usize; AUDIT_KINDS.len()];
    let Ok(doc) = lopdf::Document::load_mem(bytes) else {
        out[OVERHEAD] = bytes.len();
        return out;
    };
    let (spans, xref) = spans(&doc, bytes);
    let kinds = classify(&doc);
    for (id, n) in spans {
        out[kinds.get(&id).copied().unwrap_or(OVERHEAD)] += n;
    }
    out[XREF] += xref;
    let counted: usize = out.iter().sum();
    out[OVERHEAD] += bytes.len().saturating_sub(counted);
    if counted > bytes.len() {
        out[OVERHEAD] = out[OVERHEAD].saturating_sub(counted - bytes.len());
    }
    out
}

#[cfg(test)]
#[path = "optimize10_tests.rs"]
mod tests;
