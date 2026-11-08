//! Shapes, pictures and text formatting of `.ppt` slides.
//!
//! * Drawing: SlideContainer → PPDrawing (0x040C, [MS-PPT] §2.5.1) →
//!   OfficeArtDgContainer → OfficeArtSpgrContainer / OfficeArtSpContainer
//!   ([MS-ODRAW] §2.2.13–§2.2.17). Each shape has an OfficeArtFSP (§2.2.40:
//!   shape type = record instance, flip flags), an OfficeArtFOPT property table
//!   (§2.2.9: fill §2.3.7, line §2.3.8, rotation §2.3.18.5, `pib` §2.3.23.2),
//!   an anchor (PptOfficeArtClientAnchor [MS-PPT] §2.7.1, OfficeArtChildAnchor
//!   [MS-ODRAW] §2.2.39 inside groups, mapped through OfficeArtFSPGR §2.2.38),
//!   an OfficeArtClientData with a PlaceholderAtom ([MS-PPT] §2.13.21) and an
//!   OfficeArtClientTextbox with the text and its StyleTextPropAtom (§2.9.44).
//! * Pictures: DocumentContainer → PPDrawingGroup (0x040B) →
//!   OfficeArtDggContainer → OfficeArtBStoreContainer → OfficeArtFBSE
//!   ([MS-ODRAW] §2.2.32), whose BLIP (§2.2.23) is embedded or stored in the
//!   `Pictures` stream at `foDelay`. PNG and JPEG are kept; other BLIP types
//!   (EMF/WMF/PICT/DIB/TIFF) become a grey placeholder rectangle.
//! * Colours: OfficeArtCOLORREF (§2.2.2) RGB or index into the slide's
//!   colour scheme (SlideSchemeColorSchemeAtom, [MS-PPT] §2.4.17).

use crate::bytes::{i32_at, u16_at, u32_at, utf16};
use std::collections::HashMap;

use crate::ppt::{children, Hdr, TextKind};

const RT_PP_DRAWING: u16 = 0x040C;
const RT_COLOR_SCHEME: u16 = 0x07F0;
const RT_PLACEHOLDER_ATOM: u16 = 0x0BC3;
const RT_OUTLINE_TEXT_REF_ATOM: u16 = 0x0F9E;
const RT_TEXT_HEADER_ATOM: u16 = 0x0F9F;
const RT_TEXT_CHARS_ATOM: u16 = 0x0FA0;
const RT_STYLE_TEXT_PROP_ATOM: u16 = 0x0FA1;
const RT_TEXT_BYTES_ATOM: u16 = 0x0FA8;
const RT_FONT_ENTITY_ATOM: u16 = 0x0FB7;

const OA_DGG: u16 = 0xF000;
const OA_BSTORE: u16 = 0xF001;
const OA_DG: u16 = 0xF002;
const OA_SPGR: u16 = 0xF003;
const OA_SP: u16 = 0xF004;
const OA_FBSE: u16 = 0xF007;
const OA_FSPGR: u16 = 0xF009;
const OA_FSP: u16 = 0xF00A;
const OA_FOPT: u16 = 0xF00B;
const OA_CLIENT_TEXTBOX: u16 = 0xF00D;
const OA_CHILD_ANCHOR: u16 = 0xF00F;
const OA_CLIENT_ANCHOR: u16 = 0xF010;
const OA_CLIENT_DATA: u16 = 0xF011;

/// Shapes walked per slide (nesting and count limits, §5 of the prompt).
const MAX_SHAPES: usize = 4096;
const MAX_GROUP_DEPTH: usize = 16;
/// Default colour scheme (background, text, shadow, title text, fill,
/// accent, accent and hyperlink, accent and followed hyperlink).
pub const DEFAULT_SCHEME: [u32; 8] = [
    0xFF_FFFF, 0x00_0000, 0x80_8080, 0x00_0000, 0xBB_E0E3, 0x33_3399, 0x00_9999, 0x99_CC00,
];

/// A picture of the BLIP store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PptPicture {
    /// Picture bytes as stored (PNG or JPEG).
    pub data: Vec<u8>,
    /// `png` or `jpeg`.
    pub ext: &'static str,
}

/// Placeholder role ([MS-PPT] §2.13.21 PlaceholderEnum, grouped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PptPlaceholder {
    /// Slide title.
    Title,
    /// Centered title of a title slide.
    CenterTitle,
    /// Subtitle of a title slide.
    SubTitle,
    /// Body text.
    Body,
    /// Date, footer, slide number, header.
    Footer,
    /// Any other placeholder (object, chart, picture, …).
    Other,
}

/// A run of formatted text.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PptRun {
    /// Text of the run.
    pub text: String,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underlined.
    pub underline: bool,
    /// Size in points.
    pub size: Option<u16>,
    /// `0xRRGGBB`.
    pub color: Option<u32>,
    /// Typeface name; `None` = the default font.
    pub font: Option<String>,
}

/// A paragraph of a text box.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PptParagraph {
    /// Indent level 0–4.
    pub level: u8,
    /// `l`, `ctr`, `r`, `just`, `dist`.
    pub align: Option<&'static str>,
    /// Bullet on (`Some(true)`) or off; `None` = inherited.
    pub bullet: Option<bool>,
    /// Formatted runs of the paragraph.
    pub runs: Vec<PptRun>,
}

/// Text of a shape.
#[derive(Debug, Clone, PartialEq)]
pub struct PptRichText {
    /// Text type of the block (title, body, notes, …).
    pub kind: TextKind,
    /// Paragraphs of the text.
    pub paragraphs: Vec<PptParagraph>,
}

/// A shape of a slide, in slide coordinates (EMU).
#[derive(Debug, Clone, PartialEq)]
pub struct PptShape {
    /// MSOSPT shape type ([MS-ODRAW] §2.4.24).
    pub shape_type: u16,
    /// DrawingML preset geometry for the shape type (ECMA-376 §20.1.10.56).
    pub geometry: &'static str,
    /// Left edge in EMU.
    pub x: i64,
    /// Top edge in EMU.
    pub y: i64,
    /// Width in EMU.
    pub cx: i64,
    /// Height in EMU.
    pub cy: i64,
    /// Clockwise rotation in 60 000ths of a degree.
    pub rotation: i32,
    /// Flipped horizontally.
    pub flip_h: bool,
    /// Flipped vertically.
    pub flip_v: bool,
    /// `0xRRGGBB`; `None` = not filled.
    pub fill: Option<u32>,
    /// (`0xRRGGBB`, width in EMU); `None` = no line.
    pub line: Option<(u32, i64)>,
    /// Index into [`crate::PptDeck::pictures`].
    pub picture: Option<usize>,
    /// A picture whose format is not kept (EMF, WMF, …).
    pub missing_picture: bool,
    /// Placeholder role, when the shape is a placeholder.
    pub placeholder: Option<PptPlaceholder>,
    /// Text of the shape, if any.
    pub text: Option<PptRichText>,
    /// `t`, `ctr`, `b` (anchorText §2.3.21.6, simplified).
    pub anchor: Option<&'static str>,
}

/// A text block of the slide list (`OutlineTextRefAtom` targets).
#[derive(Debug, Clone)]
pub(crate) struct ListText<'a> {
    pub kind: TextKind,
    pub hdr: Hdr,
    pub chars: &'a [u8],
    pub style: Option<&'a [u8]>,
}

/// Document-wide drawing data.
#[derive(Debug, Clone, Default)]
pub(crate) struct DrawCtx {
    pub fonts: Vec<String>,
    pub scheme: Option<[u32; 8]>,
    /// Default shape properties (`drawingPrimaryOptions` of the DggContainer).
    pub defaults: HashMap<u16, u32>,
    /// BLIP store: index → picture (`None` when the format is not kept).
    pub blips: Vec<Option<usize>>,
}

/// MSOSPT → preset geometry ([MS-ODRAW] §2.4.24; unknown types are drawn as
/// rectangles).
pub fn preset_for(spt: u16) -> &'static str {
    match spt {
        2 => "roundRect",
        3 => "ellipse",
        4 => "diamond",
        5 => "triangle",
        6 => "rtTriangle",
        7 => "parallelogram",
        8 => "trapezoid",
        9 => "hexagon",
        10 => "octagon",
        11 => "plus",
        12 => "star5",
        13 => "rightArrow",
        15 => "homePlate",
        16 => "cube",
        19 => "arc",
        20 | 32 => "line",
        21 => "plaque",
        22 => "can",
        23 => "donut",
        55 => "chevron",
        56 => "pentagon",
        57 => "noSmoking",
        58 => "star8",
        59 => "star16",
        60 => "star32",
        61 => "wedgeRectCallout",
        62 => "wedgeRoundRectCallout",
        63 => "wedgeEllipseCallout",
        65 => "foldedCorner",
        66 => "leftArrow",
        67 => "downArrow",
        68 => "upArrow",
        69 => "leftRightArrow",
        70 => "upDownArrow",
        73 => "lightningBolt",
        74 => "heart",
        84 => "bevel",
        92 => "star24",
        95 => "blockArc",
        96 => "smileyFace",
        106 => "cloudCallout",
        109 => "flowChartProcess",
        110 => "flowChartDecision",
        111 => "flowChartInputOutput",
        116 => "flowChartTerminator",
        176 => "flowChartAlternateProcess",
        183 => "sun",
        184 => "moon",
        187 => "star4",
        _ => "rect",
    }
}

/// Simple (non-complex) properties of an OfficeArtFOPT (§2.2.9).
fn fopt(h: &Hdr, body: &[u8], out: &mut HashMap<u16, u32>) {
    for k in 0..usize::from(h.instance) {
        let (Ok(id), Ok(v)) = (u16_at(body, k * 6), u32_at(body, k * 6 + 2)) else {
            break;
        };
        if id & 0x8000 == 0 {
            out.insert(id & 0x3FFF, v);
        }
    }
}

/// Font names of the FontCollection (Environment §2.4.10, FontEntityAtom §2.9.16).
pub(crate) fn fonts(environment: &[u8], depth: usize) -> Vec<String> {
    let mut out = Vec::new();
    if depth > 4 {
        return out;
    }
    for (h, data) in children(environment) {
        if h.typ == RT_FONT_ENTITY_ATOM {
            let name = utf16(data.get(..64).unwrap_or(data));
            out.push(name.split('\0').next().unwrap_or("").to_owned());
        } else if h.ver == 0x0F {
            out.extend(fonts(data, depth + 1));
        }
    }
    out
}

/// First SlideSchemeColorSchemeAtom inside `body`.
pub(crate) fn color_scheme(body: &[u8]) -> Option<[u32; 8]> {
    for (h, data) in children(body) {
        if h.typ == RT_COLOR_SCHEME {
            let mut s = [0u32; 8];
            for (i, c) in s.iter_mut().enumerate() {
                let r = u32::from(*data.get(i * 4)?);
                let g = u32::from(*data.get(i * 4 + 1)?);
                let b = u32::from(*data.get(i * 4 + 2)?);
                *c = (r << 16) | (g << 8) | b;
            }
            return Some(s);
        }
    }
    None
}

/// OfficeArtCOLORREF (§2.2.2) → `0xRRGGBB`.
fn colorref(v: u32, scheme: &[u32; 8]) -> u32 {
    let (r, g, b, flags) = (v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF, v >> 24);
    if flags & 0x08 != 0 {
        return scheme.get(r as usize).copied().unwrap_or(0);
    }
    if flags & 0x10 != 0 {
        // System colour index: not resolvable here.
        return 0;
    }
    (r << 16) | (g << 8) | b
}

/// The BLIP store and default properties of the PPDrawingGroup.
pub(crate) fn drawing_group(
    body: &[u8],
    pictures_stream: Option<&[u8]>,
    pictures: &mut Vec<PptPicture>,
    ctx: &mut DrawCtx,
) {
    for (h, dgg) in children(body) {
        if h.typ != OA_DGG {
            continue;
        }
        for (ch, cd) in children(dgg) {
            match ch.typ {
                OA_FOPT => fopt(&ch, cd, &mut ctx.defaults),
                OA_BSTORE => {
                    for (fh, fbse) in children(cd) {
                        if fh.typ != OA_FBSE || ctx.blips.len() >= MAX_SHAPES {
                            continue;
                        }
                        let pic = fbse_picture(fbse, pictures_stream);
                        ctx.blips.push(pic.map(|p| {
                            pictures.push(p);
                            pictures.len() - 1
                        }));
                    }
                }
                _ => {}
            }
        }
    }
}

/// The picture of an OfficeArtFBSE: embedded BLIP or `Pictures` stream at `foDelay`.
fn fbse_picture(fbse: &[u8], pictures_stream: Option<&[u8]>) -> Option<PptPicture> {
    let size = u32_at(fbse, 20).ok()? as usize;
    let fo_delay = u32_at(fbse, 28).ok()? as usize;
    let cb_name = usize::from(*fbse.get(33)?);
    let embedded = fbse.get(36 + cb_name..).filter(|b| b.len() >= 8);
    let blip = match (embedded, pictures_stream) {
        (Some(b), _) => b,
        (None, Some(p)) => p.get(fo_delay..fo_delay.checked_add(size)?)?,
        (None, None) => return None,
    };
    blip_picture(blip)
}

/// OfficeArtBlip (§2.2.23): PNG and JPEG data.
fn blip_picture(rec: &[u8]) -> Option<PptPicture> {
    let h = crate::ppt::header(rec, 0).ok()?;
    let body = rec.get(8..8usize.checked_add(h.len)?)?;
    let (ext, two_uids): (&'static str, bool) = match (h.typ, h.instance) {
        // OfficeArtBlipJPEG §2.2.27 (RGB / CMYK), second UID for the odd instances.
        (0xF01D | 0xF02A, i) => ("jpeg", matches!(i, 0x46B | 0x6E3)),
        // OfficeArtBlipPNG §2.2.28.
        (0xF01E, i) => ("png", i == 0x6E1),
        _ => return None,
    };
    let start = 16 + if two_uids { 16 } else { 0 } + 1;
    let data = body.get(start..)?;
    let ok = match ext {
        "png" => data.starts_with(&[0x89, b'P', b'N', b'G']),
        _ => data.starts_with(&[0xFF, 0xD8]),
    };
    ok.then(|| PptPicture {
        data: data.to_vec(),
        ext,
    })
}

/// Coordinate mapping of a group: slide = origin + (child − from) × scale.
#[derive(Debug, Clone, Copy)]
struct Map {
    from: (f64, f64),
    origin: (f64, f64),
    scale: (f64, f64),
}

impl Map {
    const IDENTITY: Map = Map {
        from: (0.0, 0.0),
        origin: (0.0, 0.0),
        scale: (1.0, 1.0),
    };

    fn apply(&self, r: [f64; 4]) -> [f64; 4] {
        let x = self.origin.0 + (r[0] - self.from.0) * self.scale.0;
        let y = self.origin.1 + (r[1] - self.from.1) * self.scale.1;
        [x, y, r[2] * self.scale.0, r[3] * self.scale.1]
    }
}

/// Anchor rectangle `[x, y, w, h]` in the parent coordinate space.
fn anchor_rect(h: &Hdr, data: &[u8]) -> Option<[f64; 4]> {
    let (l, t, r, b) = if h.typ == OA_CHILD_ANCHOR {
        // OfficeArtChildAnchor: xLeft, yTop, xRight, yBottom.
        (
            i32_at(data, 0).ok()?,
            i32_at(data, 4).ok()?,
            i32_at(data, 8).ok()?,
            i32_at(data, 12).ok()?,
        )
    } else if data.len() >= 16 {
        // RectStruct: top, left, right, bottom.
        (
            i32_at(data, 4).ok()?,
            i32_at(data, 0).ok()?,
            i32_at(data, 8).ok()?,
            i32_at(data, 12).ok()?,
        )
    } else {
        // SmallRectStruct: top, left, right, bottom (i16).
        let s = |o: usize| u16_at(data, o).map(|v| i32::from(v.cast_signed()));
        (s(2).ok()?, s(0).ok()?, s(4).ok()?, s(6).ok()?)
    };
    Some([
        f64::from(l),
        f64::from(t),
        f64::from(r) - f64::from(l),
        f64::from(b) - f64::from(t),
    ])
}

/// Text of a client textbox (own atoms, or the slide list via OutlineTextRefAtom).
fn textbox(
    body: &[u8],
    list: &[ListText<'_>],
    ctx: &DrawCtx,
    scheme: &[u32; 8],
) -> Option<PptRichText> {
    let mut kind = TextKind::Other;
    let mut chars: Option<(Hdr, &[u8])> = None;
    let mut style: Option<&[u8]> = None;
    for (h, data) in children(body) {
        match h.typ {
            RT_TEXT_HEADER_ATOM => {
                kind = crate::ppt::text_kind(u32_at(data, 0).unwrap_or(4));
            }
            RT_TEXT_CHARS_ATOM | RT_TEXT_BYTES_ATOM => chars = Some((h, data)),
            RT_STYLE_TEXT_PROP_ATOM => style = Some(data),
            RT_OUTLINE_TEXT_REF_ATOM => {
                let i = usize::try_from(i32_at(data, 0).unwrap_or(-1)).ok()?;
                let t = list.get(i)?;
                return Some(rich_text(t.kind, &t.hdr, t.chars, t.style, ctx, scheme));
            }
            _ => {}
        }
    }
    let (h, data) = chars?;
    Some(rich_text(kind, &h, data, style, ctx, scheme))
}

/// Character / paragraph runs of a StyleTextPropAtom ([MS-PPT] §2.9.44).
#[derive(Default, Clone)]
struct CharStyle {
    bold: bool,
    italic: bool,
    underline: bool,
    size: Option<u16>,
    color: Option<u32>,
    font: Option<u16>,
}

#[derive(Default, Clone)]
struct ParaStyle {
    level: u8,
    align: Option<&'static str>,
    bullet: Option<bool>,
}

/// Bytes `n` at `p` (advancing), `None` when out of bounds.
fn take<'a>(b: &'a [u8], p: &mut usize, n: usize) -> Option<&'a [u8]> {
    let s = b.get(*p..p.checked_add(n)?)?;
    *p += n;
    Some(s)
}

/// TextPFException (§2.9.18).
fn pf_exception(b: &[u8], p: &mut usize) -> Option<ParaStyle> {
    let masks = u32::from_le_bytes(take(b, p, 4)?.try_into().ok()?);
    let bit = |n: u32| masks & (1 << n) != 0;
    let mut st = ParaStyle::default();
    if masks & 0xF != 0 {
        let flags = u16::from_le_bytes(take(b, p, 2)?.try_into().ok()?);
        if bit(0) {
            st.bullet = Some(flags & 1 != 0);
        }
    }
    for n in [7, 4, 6] {
        if bit(n) {
            take(b, p, 2)?;
        }
    }
    if bit(5) {
        take(b, p, 4)?;
    }
    if bit(11) {
        let a = u16::from_le_bytes(take(b, p, 2)?.try_into().ok()?);
        st.align = Some(match a {
            1 => "ctr",
            2 => "r",
            3 => "just",
            4 => "dist",
            _ => "l",
        });
    }
    for n in [12, 13, 14, 8, 10, 15] {
        if bit(n) {
            take(b, p, 2)?;
        }
    }
    if bit(20) {
        let count = usize::from(u16::from_le_bytes(take(b, p, 2)?.try_into().ok()?));
        take(b, p, count.checked_mul(4)?)?;
    }
    if bit(16) {
        take(b, p, 2)?;
    }
    if masks & (0b111 << 17) != 0 {
        take(b, p, 2)?;
    }
    if bit(21) {
        take(b, p, 2)?;
    }
    Some(st)
}

/// TextCFException (§2.9.20).
fn cf_exception(b: &[u8], p: &mut usize, scheme: &[u32; 8]) -> Option<CharStyle> {
    let masks = u32::from_le_bytes(take(b, p, 4)?.try_into().ok()?);
    let bit = |n: u32| masks & (1 << n) != 0;
    let mut st = CharStyle::default();
    // fontStyle: bold, italic, underline, shadow, fehint, kumi, emboss, fHasStyle.
    if masks & 0x3EB7 != 0 {
        let fs = u16::from_le_bytes(take(b, p, 2)?.try_into().ok()?);
        st.bold = bit(0) && fs & 1 != 0;
        st.italic = bit(1) && fs & 2 != 0;
        st.underline = bit(2) && fs & 4 != 0;
    }
    if bit(16) {
        st.font = Some(u16::from_le_bytes(take(b, p, 2)?.try_into().ok()?));
    }
    for n in [21, 22, 23] {
        if bit(n) {
            take(b, p, 2)?;
        }
    }
    if bit(17) {
        st.size = Some(u16::from_le_bytes(take(b, p, 2)?.try_into().ok()?));
    }
    if bit(18) {
        let c = take(b, p, 4)?;
        let (r, g, bl, idx) = (u32::from(c[0]), u32::from(c[1]), u32::from(c[2]), c[3]);
        st.color = match idx {
            0xFE => Some((r << 16) | (g << 8) | bl),
            i if i < 8 => scheme.get(usize::from(i)).copied(),
            _ => None,
        };
    }
    Some(st)
}

/// Builds paragraphs and runs from the characters and their style runs.
fn rich_text(
    kind: TextKind,
    h: &Hdr,
    data: &[u8],
    style: Option<&[u8]>,
    ctx: &DrawCtx,
    scheme: &[u32; 8],
) -> PptRichText {
    let text: Vec<char> = if h.typ == RT_TEXT_CHARS_ATOM {
        utf16(data).chars().collect()
    } else {
        crate::bytes::latin1(data).chars().collect()
    };
    let n = text.len() + 1;
    let mut pruns: Vec<(usize, ParaStyle)> = Vec::new();
    let mut cruns: Vec<(usize, CharStyle)> = Vec::new();
    if let Some(b) = style {
        let mut p = 0usize;
        let mut covered = 0usize;
        while covered < n && pruns.len() < MAX_SHAPES {
            let Some(count) = take(b, &mut p, 4)
                .and_then(|c| c.try_into().ok())
                .map(u32::from_le_bytes)
            else {
                break;
            };
            let Some(level) = take(b, &mut p, 2)
                .and_then(|c| c.try_into().ok())
                .map(u16::from_le_bytes)
            else {
                break;
            };
            let Some(mut st) = pf_exception(b, &mut p) else {
                break;
            };
            st.level = u8::try_from(level.min(8)).unwrap_or(0);
            let count = (count as usize).max(1);
            pruns.push((count, st));
            covered += count;
        }
        covered = 0;
        while covered < n && cruns.len() < MAX_SHAPES {
            let Some(count) = take(b, &mut p, 4)
                .and_then(|c| c.try_into().ok())
                .map(u32::from_le_bytes)
            else {
                break;
            };
            let Some(st) = cf_exception(b, &mut p, scheme) else {
                break;
            };
            let count = (count as usize).max(1);
            cruns.push((count, st));
            covered += count;
        }
    }
    let style_at = |runs: &[(usize, CharStyle)], i: usize| -> CharStyle {
        let mut at = 0;
        for (c, s) in runs {
            if i < at + c {
                return s.clone();
            }
            at += c;
        }
        CharStyle::default()
    };
    let para_at = |i: usize| -> ParaStyle {
        let mut at = 0;
        for (c, s) in &pruns {
            if i < at + c {
                return s.clone();
            }
            at += c;
        }
        ParaStyle::default()
    };
    let mut paragraphs = Vec::new();
    let mut start = 0usize;
    for (i, ch) in text.iter().chain(std::iter::once(&'\r')).enumerate() {
        if *ch != '\r' {
            continue;
        }
        let ps = para_at(start);
        let mut para = PptParagraph {
            level: ps.level,
            align: ps.align,
            bullet: ps.bullet,
            runs: Vec::new(),
        };
        let mut k = start;
        while k < i {
            let cs = style_at(&cruns, k);
            let mut e = k + 1;
            while e < i && same(&style_at(&cruns, e), &cs) {
                e += 1;
            }
            let s: String = text[k..e]
                .iter()
                .map(|c| if *c == '\u{b}' { '\u{2028}' } else { *c })
                .collect();
            para.runs.push(PptRun {
                text: s,
                bold: cs.bold,
                italic: cs.italic,
                underline: cs.underline,
                size: cs.size,
                color: cs.color,
                font: cs
                    .font
                    .and_then(|f| ctx.fonts.get(usize::from(f)))
                    .filter(|f| !f.is_empty())
                    .cloned(),
            });
            k = e;
        }
        paragraphs.push(para);
        start = i + 1;
    }
    PptRichText { kind, paragraphs }
}

fn same(a: &CharStyle, b: &CharStyle) -> bool {
    a.bold == b.bold
        && a.italic == b.italic
        && a.underline == b.underline
        && a.size == b.size
        && a.color == b.color
        && a.font == b.font
}

/// Shapes of a SlideContainer body.
pub(crate) fn slide_shapes(slide: &[u8], list: &[ListText<'_>], ctx: &DrawCtx) -> Vec<PptShape> {
    let scheme = color_scheme(slide).or(ctx.scheme).unwrap_or(DEFAULT_SCHEME);
    let mut out = Vec::new();
    for (h, data) in children(slide) {
        if h.typ != RT_PP_DRAWING {
            continue;
        }
        for (dh, dg) in children(data) {
            if dh.typ != OA_DG {
                continue;
            }
            for (gh, spgr) in children(dg) {
                if gh.typ == OA_SPGR {
                    group(spgr, Map::IDENTITY, true, 0, list, ctx, &scheme, &mut out);
                }
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn group(
    body: &[u8],
    map: Map,
    top: bool,
    depth: usize,
    list: &[ListText<'_>],
    ctx: &DrawCtx,
    scheme: &[u32; 8],
    out: &mut Vec<PptShape>,
) {
    if depth > MAX_GROUP_DEPTH {
        return;
    }
    let mut inner = map;
    for (i, (h, data)) in children(body).into_iter().enumerate() {
        if out.len() >= MAX_SHAPES {
            return;
        }
        match h.typ {
            OA_SP if i == 0 => {
                // The group's own shape: its child coordinate space (FSPGR)
                // mapped onto its anchor in the parent.
                if top {
                    continue;
                }
                let mut child: Option<[f64; 4]> = None;
                let mut anchor: Option<[f64; 4]> = None;
                for (ch, cd) in children(data) {
                    match ch.typ {
                        OA_FSPGR => {
                            child = anchor_rect(
                                &Hdr {
                                    typ: OA_CHILD_ANCHOR,
                                    ..ch
                                },
                                cd,
                            );
                        }
                        OA_CLIENT_ANCHOR | OA_CHILD_ANCHOR => anchor = anchor_rect(&ch, cd),
                        _ => {}
                    }
                }
                if let (Some(c), Some(a)) = (child, anchor) {
                    let a = map.apply(a);
                    let sx = if c[2].abs() > f64::EPSILON {
                        a[2] / c[2]
                    } else {
                        1.0
                    };
                    let sy = if c[3].abs() > f64::EPSILON {
                        a[3] / c[3]
                    } else {
                        1.0
                    };
                    inner = Map {
                        from: (c[0], c[1]),
                        origin: (a[0], a[1]),
                        scale: (sx, sy),
                    };
                }
            }
            OA_SP => {
                if let Some(s) = shape(data, &inner, list, ctx, scheme) {
                    out.push(s);
                }
            }
            OA_SPGR => group(data, inner, false, depth + 1, list, ctx, scheme, out),
            _ => {}
        }
    }
}

/// One OfficeArtSpContainer.
fn shape(
    body: &[u8],
    map: &Map,
    list: &[ListText<'_>],
    ctx: &DrawCtx,
    scheme: &[u32; 8],
) -> Option<PptShape> {
    let mut spt = 1u16;
    let mut flags = 0u32;
    let mut props = ctx.defaults.clone();
    let mut rect: Option<[f64; 4]> = None;
    let mut placeholder = None;
    let mut text = None;
    for (h, data) in children(body) {
        match h.typ {
            OA_FSP => {
                spt = h.instance;
                flags = u32_at(data, 4).unwrap_or(0);
            }
            OA_FOPT => fopt(&h, data, &mut props),
            OA_CLIENT_ANCHOR | OA_CHILD_ANCHOR => rect = anchor_rect(&h, data),
            OA_CLIENT_DATA => {
                for (ch, cd) in children(data) {
                    if ch.typ == RT_PLACEHOLDER_ATOM {
                        placeholder = cd.get(4).map(|p| match p {
                            0x0D | 0x11 => PptPlaceholder::Title,
                            0x0F => PptPlaceholder::CenterTitle,
                            0x10 => PptPlaceholder::SubTitle,
                            0x0E | 0x12 | 0x13 => PptPlaceholder::Body,
                            0x07..=0x0A => PptPlaceholder::Footer,
                            _ => PptPlaceholder::Other,
                        });
                    }
                }
            }
            OA_CLIENT_TEXTBOX => text = textbox(data, list, ctx, scheme),
            _ => {}
        }
    }
    // fDeleted / fBackground shapes and placeholders for date, footer and slide number.
    if flags & (0x0008 | 0x0400) != 0 || placeholder == Some(PptPlaceholder::Footer) {
        return None;
    }
    let r = map.apply(rect?);
    // §2.3.18.5 rotation: 16.16 fixed-point degrees.
    let rot = props
        .get(&0x0004)
        .map_or(0.0, |v| f64::from(v.cast_signed()) / 65_536.0);
    let rot = rot.rem_euclid(360.0);
    // Shapes rotated by 45°–135° / 225°–315° store the bounds of the rotated
    // shape: swap width and height about the centre.
    let mut b = r;
    if (45.0..135.0).contains(&rot) || (225.0..315.0).contains(&rot) {
        let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        b = [cx - r[3] / 2.0, cy - r[2] / 2.0, r[3], r[2]];
    }
    // 576 master units per inch → EMU.
    #[allow(clippy::cast_possible_truncation)]
    let emu = |v: f64| (v * 914_400.0 / 576.0).round() as i64;
    let fill_bools = props.get(&0x01BF).copied().unwrap_or(0x0010_0010);
    let line_bools = props.get(&0x01FF).copied().unwrap_or(0x0008_0008);
    let is_ph = placeholder.is_some();
    let filled = if fill_bools & (1 << 20) != 0 {
        fill_bools & (1 << 4) != 0
    } else {
        !is_ph
    };
    let lined = if line_bools & (1 << 19) != 0 {
        line_bools & (1 << 3) != 0
    } else {
        !is_ph
    };
    // Text boxes (msosptTextBox) without explicit fill / line properties.
    let textbox_shape = spt == 202;
    let pib = props.get(&0x0104).copied();
    let blip = pib.and_then(|i| ctx.blips.get((i as usize).checked_sub(1)?).copied());
    let fill = (filled && (!textbox_shape || props.contains_key(&0x0181)))
        .then(|| colorref(props.get(&0x0181).copied().unwrap_or(0x00FF_FFFF), scheme));
    let line = (lined && (!textbox_shape || props.contains_key(&0x01C0))).then(|| {
        (
            colorref(props.get(&0x01C0).copied().unwrap_or(0), scheme),
            i64::from(props.get(&0x01CB).copied().unwrap_or(9525)),
        )
    });
    #[allow(clippy::cast_possible_truncation)]
    Some(PptShape {
        shape_type: spt,
        geometry: preset_for(spt),
        x: emu(b[0]),
        y: emu(b[1]),
        cx: emu(b[2]).max(0),
        cy: emu(b[3]).max(0),
        rotation: (rot * 60_000.0).round() as i32,
        flip_h: flags & 0x40 != 0,
        flip_v: flags & 0x80 != 0,
        fill: if blip.is_some() { None } else { fill },
        line,
        picture: blip.flatten(),
        missing_picture: blip.is_some_and(|b| b.is_none()),
        placeholder,
        text,
        anchor: props.get(&0x0087).map(|a| match a {
            1 | 4 | 6 | 8 => "ctr",
            2 | 5 | 7 | 9 => "b",
            _ => "t",
        }),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::ppt::tests::rec;

    fn atom(typ: u16, inst: u16, body: &[u8]) -> Vec<u8> {
        rec(0, inst, typ, body)
    }

    fn cont(typ: u16, body: &[u8]) -> Vec<u8> {
        rec(0x0F, 0, typ, body)
    }

    /// OfficeArtFOPT with simple properties.
    pub fn opt(props: &[(u16, u32)]) -> Vec<u8> {
        let mut b = Vec::new();
        for (id, v) in props {
            b.extend(id.to_le_bytes());
            b.extend(v.to_le_bytes());
        }
        rec(3, u16::try_from(props.len()).unwrap_or(0), OA_FOPT, &b)
    }

    /// A shape container: type, anchor (master units l, t, r, b), properties, extra records.
    pub fn sp(spt: u16, id: u32, rect: [i32; 4], props: &[(u16, u32)], extra: &[u8]) -> Vec<u8> {
        let mut fsp = id.to_le_bytes().to_vec();
        fsp.extend(0x0A00u32.to_le_bytes()); // fHaveAnchor | fHaveSpt
        let mut b = rec(2, spt, OA_FSP, &fsp);
        b.extend(opt(props));
        let mut a = Vec::new();
        for v in [rect[1], rect[0], rect[2], rect[3]] {
            a.extend(v.to_le_bytes());
        }
        b.extend(atom(OA_CLIENT_ANCHOR, 0, &a));
        b.extend_from_slice(extra);
        cont(OA_SP, &b)
    }

    /// PPDrawing with the given shape containers (patriarch first).
    pub fn drawing(shapes: &[Vec<u8>]) -> Vec<u8> {
        let mut fsp = 1024u32.to_le_bytes().to_vec();
        fsp.extend(0x0005u32.to_le_bytes()); // fGroup | fPatriarch
        let mut spgr = cont(OA_SP, &rec(2, 0, OA_FSP, &fsp));
        for s in shapes {
            spgr.extend_from_slice(s);
        }
        cont(RT_PP_DRAWING, &cont(OA_DG, &cont(OA_SPGR, &spgr)))
    }

    /// A text box with one style run of `n` characters (bold, 24 pt, RGB colour).
    pub fn styled_text(text: &str, color: u32) -> Vec<u8> {
        let chars: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let n = u32::try_from(text.encode_utf16().count() + 1).unwrap_or(0);
        let mut st = Vec::new();
        // One paragraph run: centred.
        st.extend(n.to_le_bytes());
        st.extend(0u16.to_le_bytes());
        st.extend((1u32 << 11).to_le_bytes());
        st.extend(1u16.to_le_bytes());
        // One character run: bold + size + colour.
        st.extend(n.to_le_bytes());
        st.extend((1u32 | (1 << 17) | (1 << 18)).to_le_bytes());
        st.extend(1u16.to_le_bytes());
        st.extend(24u16.to_le_bytes());
        st.extend([(color >> 16) as u8, (color >> 8) as u8, color as u8, 0xFE]);
        let mut tb = atom(RT_TEXT_HEADER_ATOM, 0, &4u32.to_le_bytes());
        tb.extend(atom(RT_TEXT_CHARS_ATOM, 0, &chars));
        tb.extend(atom(RT_STYLE_TEXT_PROP_ATOM, 0, &st));
        cont(OA_CLIENT_TEXTBOX, &tb)
    }

    /// A PNG BLIP record (OfficeArtBlipPNG, one UID).
    pub fn png_blip(png: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; 16];
        b.push(0xFF);
        b.extend_from_slice(png);
        rec(0, 0x6E0, 0xF01E, &b)
    }

    /// PPDrawingGroup with an embedded BLIP store.
    pub fn drawing_group_rec(blips: &[Vec<u8>]) -> Vec<u8> {
        let mut store = Vec::new();
        for b in blips {
            let mut f = vec![6u8, 6];
            f.extend([0u8; 16]);
            f.extend(0u16.to_le_bytes());
            f.extend(u32::try_from(b.len()).unwrap_or(0).to_le_bytes());
            f.extend(1u32.to_le_bytes());
            f.extend(0u32.to_le_bytes());
            f.extend([0u8, 0, 0, 0]);
            f.extend_from_slice(b);
            store.extend(rec(2, 6, OA_FBSE, &f));
        }
        let n = u16::try_from(blips.len()).unwrap_or(0);
        cont(0x040B, &cont(OA_DGG, &rec(0x0F, n, OA_BSTORE, &store)))
    }

    #[test]
    fn shapes_with_fill_line_rotation_and_text() {
        let red = 0x0000_00C0u32; // OfficeArtCOLORREF r=0xC0
        let shapes = [
            sp(
                3,
                1025,
                [576, 576, 1152, 864],
                &[
                    (0x0181, red),
                    (0x01C0, 0x0800_0001), // scheme index 1 (text colour)
                    (0x01CB, 25_400),
                    (0x0004, 30 << 16),
                ],
                &styled_text("Xin chào", 0x00_70C0),
            ),
            // Not filled, no line.
            sp(
                1,
                1026,
                [0, 0, 288, 288],
                &[(0x01BF, 0x0010_0000), (0x01FF, 0x0008_0000)],
                &[],
            ),
        ];
        let d = drawing(&shapes);
        let ctx = DrawCtx::default();
        let out = slide_shapes(&d, &[], &ctx);
        assert_eq!(out.len(), 2);
        let e = &out[0];
        assert_eq!(e.geometry, "ellipse");
        assert_eq!((e.x, e.y, e.cx, e.cy), (914_400, 914_400, 914_400, 457_200));
        assert_eq!(e.rotation, 30 * 60_000);
        assert_eq!(e.fill, Some(0xC0_0000));
        assert_eq!(e.line, Some((0, 25_400)));
        let t = e.text.as_ref().unwrap();
        assert_eq!(t.paragraphs[0].align, Some("ctr"));
        let r = &t.paragraphs[0].runs[0];
        assert_eq!(r.text, "Xin chào");
        assert!(r.bold);
        assert_eq!(r.size, Some(24));
        assert_eq!(r.color, Some(0x00_70C0));
        assert_eq!(out[1].fill, None);
        assert_eq!(out[1].line, None);
    }

    #[test]
    fn rotated_bounds_and_groups_are_mapped() {
        // 90° rotation: stored bounds are those of the rotated shape.
        let s = sp(1, 1025, [0, 0, 100, 200], &[(0x0004, 90 << 16)], &[]);
        // A group whose child space 0..1000 maps onto slide 576..1152.
        let mut fspgr = Vec::new();
        for v in [0i32, 0, 1000, 1000] {
            fspgr.extend(v.to_le_bytes());
        }
        let mut gfsp = 2000u32.to_le_bytes().to_vec();
        gfsp.extend(0x0201u32.to_le_bytes());
        let mut gsp = rec(1, 0, OA_FSPGR, &fspgr);
        gsp.extend(rec(2, 0, OA_FSP, &gfsp));
        let mut a = Vec::new();
        for v in [576i32, 576, 1152, 1152] {
            a.extend(v.to_le_bytes());
        }
        gsp.extend(atom(OA_CLIENT_ANCHOR, 0, &a));
        let mut child_fsp = 2001u32.to_le_bytes().to_vec();
        child_fsp.extend(0x0A02u32.to_le_bytes());
        let mut child = rec(2, 1, OA_FSP, &child_fsp);
        let mut ca = Vec::new();
        for v in [500i32, 500, 1000, 1000] {
            ca.extend(v.to_le_bytes());
        }
        child.extend(atom(OA_CHILD_ANCHOR, 0, &ca));
        let mut grp = cont(OA_SP, &gsp);
        grp.extend(cont(OA_SP, &child));
        let group = cont(OA_SPGR, &grp);
        let out = slide_shapes(&drawing(&[s, group]), &[], &DrawCtx::default());
        assert_eq!(out.len(), 2);
        // 100 × 200 bounds rotated 90°: 200 × 100 shape about the same centre.
        let emu = |v: i64| v * 914_400 / 576;
        assert_eq!((out[0].cx, out[0].cy), (emu(200), emu(100)));
        assert_eq!(out[0].x, emu(-50));
        // Child at 500..1000 of 0..1000 → 864..1152 master units.
        assert_eq!((out[1].x, out[1].cx), (emu(864), emu(288)));
    }

    #[test]
    fn blip_store_pictures() {
        let png = crate::ppt::tests::tiny_png();
        let dg = drawing_group_rec(&[png_blip(&png), rec(0, 0x3D4, 0xF01A, &[0; 60])]);
        let mut pics = Vec::new();
        let mut ctx = DrawCtx::default();
        drawing_group(&dg[8..], None, &mut pics, &mut ctx);
        assert_eq!(pics.len(), 1);
        assert_eq!(pics[0].ext, "png");
        assert_eq!(pics[0].data, png);
        assert_eq!(ctx.blips, vec![Some(0), None]);
        let shapes = [
            sp(75, 1025, [0, 0, 576, 576], &[(0x0104, 1)], &[]),
            sp(75, 1026, [0, 0, 576, 576], &[(0x0104, 2)], &[]),
        ];
        let out = slide_shapes(&drawing(&shapes), &[], &ctx);
        assert_eq!(out[0].picture, Some(0));
        assert!(out[1].missing_picture);
    }
}
