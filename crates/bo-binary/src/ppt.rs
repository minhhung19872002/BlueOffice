//! PowerPoint 97–2003 presentations ([MS-PPT]).
//!
//! `Current User` (§2.3.2 CurrentUserAtom) points at the newest UserEditAtom
//! (§2.3.3) in the `PowerPoint Document` stream; following the
//! `offsetLastEdit` chain and merging PersistDirectoryAtoms (§2.3.4) yields
//! the persist-object directory. The DocumentContainer (§2.4.1) gives the
//! slide size and the SlideListWithTextContainers (§2.4.14.3) for slides and
//! notes; each SlideContainer (§2.5.1) / NotesContainer (§2.5.6) drawing is
//! walked for TextHeaderAtom + TextCharsAtom / TextBytesAtom pairs, and for
//! shapes and pictures (OfficeArt, [MS-ODRAW]; see `ppt_draw`).

use crate::bytes::{i32_at, latin1, u16_at, u32_at, utf16};
use crate::ppt_draw::{self, DrawCtx, ListText, PptPicture, PptShape};
use crate::{malformed, BinaryError, Cfb, Result};
use std::collections::{HashMap, HashSet};

/// Maximum container nesting walked.
const MAX_DEPTH: usize = 48;
/// Maximum number of UserEditAtoms followed.
const MAX_EDITS: usize = 4096;
/// Maximum number of slides read.
const MAX_SLIDES: usize = 10_000;

// [MS-PPT] §2.13.24 RecordType values.
const RT_DOCUMENT: u16 = 0x03E8;
const RT_DOCUMENT_ATOM: u16 = 0x03E9;
const RT_ENVIRONMENT: u16 = 0x03F2;
const RT_MAIN_MASTER: u16 = 0x03F8;
const RT_PP_DRAWING_GROUP: u16 = 0x040B;
const RT_STYLE_TEXT_PROP_ATOM: u16 = 0x0FA1;
const RT_SLIDE: u16 = 0x03EE;
const RT_NOTES: u16 = 0x03F0;
const RT_NOTES_ATOM: u16 = 0x03F1;
const RT_SLIDE_PERSIST_ATOM: u16 = 0x03F3;
const RT_VBA_INFO: u16 = 0x03FF;
const RT_OUTLINE_TEXT_REF_ATOM: u16 = 0x0F9E;
const RT_TEXT_HEADER_ATOM: u16 = 0x0F9F;
const RT_TEXT_CHARS_ATOM: u16 = 0x0FA0;
const RT_TEXT_BYTES_ATOM: u16 = 0x0FA8;
const RT_SLIDE_LIST_WITH_TEXT: u16 = 0x0FF0;
const RT_USER_EDIT_ATOM: u16 = 0x0FF5;
const RT_PERSIST_DIRECTORY_ATOM: u16 = 0x1772;
const RT_CLIENT_TEXTBOX: u16 = 0xF00D;

/// A presentation's text and slide structure.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PptDeck {
    /// Slide size in EMU (§2.4.2 DocumentAtom.slideSize, 576 master units per inch).
    pub slide_width: i64,
    pub slide_height: i64,
    pub slides: Vec<PptSlide>,
    /// A VBAInfoContainer is present (macros are preserved, never executed).
    pub has_macros: bool,
    /// Pictures of the BLIP store referenced by [`PptShape::picture`].
    pub pictures: Vec<PptPicture>,
}

/// One slide.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PptSlide {
    /// §2.4.14.5 SlidePersistAtom.slideId.
    pub slide_id: u32,
    /// Text blocks in document order (placeholders first, then other shapes).
    pub texts: Vec<PptText>,
    /// Speaker notes paragraphs.
    pub notes: Vec<String>,
    /// Shapes of the slide drawing (geometry, fill, line, pictures, text).
    pub shapes: Vec<PptShape>,
}

/// A text block of a shape or placeholder.
#[derive(Debug, Clone, PartialEq)]
pub struct PptText {
    pub kind: TextKind,
    /// Paragraphs; line breaks inside a paragraph are `\u{2028}`.
    pub paragraphs: Vec<String>,
}

/// §2.13.33 TextTypeEnum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKind {
    Title,
    Body,
    Notes,
    Other,
    CenterBody,
    CenterTitle,
    HalfBody,
    QuarterBody,
}

/// §2.13.33 TextTypeEnum value → [`TextKind`].
pub(crate) fn text_kind(v: u32) -> TextKind {
    TextKind::from_u32(v)
}

impl TextKind {
    fn from_u32(v: u32) -> TextKind {
        match v {
            0 => TextKind::Title,
            1 => TextKind::Body,
            2 => TextKind::Notes,
            5 => TextKind::CenterBody,
            6 => TextKind::CenterTitle,
            7 => TextKind::HalfBody,
            8 => TextKind::QuarterBody,
            _ => TextKind::Other,
        }
    }

    /// Whether this is a title placeholder.
    pub fn is_title(self) -> bool {
        matches!(self, TextKind::Title | TextKind::CenterTitle)
    }
}

impl PptSlide {
    /// The slide title, if any.
    pub fn title(&self) -> Option<String> {
        self.texts
            .iter()
            .find(|t| t.kind.is_title())
            .map(|t| t.paragraphs.join(" "))
    }
}

/// §2.3.1 RecordHeader.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Hdr {
    pub ver: u8,
    pub instance: u16,
    pub typ: u16,
    pub len: usize,
}

pub(crate) fn header(b: &[u8], off: usize) -> Result<Hdr> {
    let vi = u16_at(b, off)?;
    Ok(Hdr {
        ver: (vi & 0x000F) as u8,
        instance: vi >> 4,
        typ: u16_at(b, off + 2)?,
        len: u32_at(b, off + 4)? as usize,
    })
}

/// Children of a container body: (header, body slice).
pub(crate) fn children(body: &[u8]) -> Vec<(Hdr, &[u8])> {
    let mut out = Vec::new();
    let mut p = 0usize;
    while let Ok(h) = header(body, p) {
        let start = p + 8;
        let Some(data) = start
            .checked_add(h.len)
            .and_then(|end| body.get(start..end))
        else {
            // Truncated record: keep what is present.
            if let Some(rest) = body.get(start..) {
                out.push((h, rest));
            }
            break;
        };
        out.push((h, data));
        p = start + h.len;
    }
    out
}

/// The record at `off` of the document stream: (header, body).
fn record_at(doc: &[u8], off: usize) -> Result<(Hdr, &[u8])> {
    let h = header(doc, off)?;
    let body = (off + 8)
        .checked_add(h.len)
        .and_then(|end| doc.get(off + 8..end))
        .ok_or_else(|| malformed("record body out of bounds"))?;
    Ok((h, body))
}

/// Decode a text atom: paragraphs split on CR, VT → line separator.
fn text_paragraphs(h: &Hdr, body: &[u8]) -> Vec<String> {
    let s = if h.typ == RT_TEXT_CHARS_ATOM {
        utf16(body)
    } else {
        latin1(body)
    };
    s.split('\r')
        .map(|p| p.replace('\u{000B}', "\u{2028}"))
        .collect()
}

/// Collect text blocks of a drawing tree. `OutlineTextRefAtom` boxes refer to
/// the slide list text and are skipped.
fn collect_texts(body: &[u8], depth: usize, out: &mut Vec<PptText>) {
    if depth > MAX_DEPTH {
        return;
    }
    let kids = children(body);
    let mut kind: Option<TextKind> = None;
    for (h, data) in &kids {
        match h.typ {
            RT_TEXT_HEADER_ATOM => kind = Some(TextKind::from_u32(u32_at(data, 0).unwrap_or(4))),
            RT_TEXT_CHARS_ATOM | RT_TEXT_BYTES_ATOM => {
                out.push(PptText {
                    kind: kind.take().unwrap_or(TextKind::Other),
                    paragraphs: text_paragraphs(h, data),
                });
            }
            RT_OUTLINE_TEXT_REF_ATOM => kind = None,
            _ if h.ver == 0x0F || h.typ == RT_CLIENT_TEXTBOX => collect_texts(data, depth + 1, out),
            _ => {}
        }
    }
}

/// Read a `.ppt` file.
pub fn read_ppt(bytes: &[u8]) -> Result<PptDeck> {
    let mut cfb = Cfb::open(bytes)?;
    let doc = cfb.read("/PowerPoint Document")?;
    let current = if cfb.has("/Current User") {
        Some(cfb.read("/Current User")?)
    } else {
        None
    };
    let has_vba_storage = cfb.has_storage("/_VBA_PROJECT_CUR");
    // §2.1.2 Pictures stream: the BLIPs of the OfficeArtBStoreContainer.
    let pictures = if cfb.has("/Pictures") {
        cfb.read("/Pictures").ok()
    } else {
        None
    };
    let mut deck = parse_ppt_with(&doc, current.as_deref(), pictures.as_deref())?;
    deck.has_macros |= has_vba_storage;
    Ok(deck)
}

/// Parse the `PowerPoint Document` stream given the `Current User` stream.
#[cfg(test)]
pub(crate) fn parse_ppt(doc: &[u8], current: Option<&[u8]>) -> Result<PptDeck> {
    parse_ppt_with(doc, current, None)
}

/// [`parse_ppt`] with the `Pictures` stream.
pub(crate) fn parse_ppt_with(
    doc: &[u8],
    current: Option<&[u8]>,
    pictures: Option<&[u8]>,
) -> Result<PptDeck> {
    let directory = current.and_then(|c| persist_directory(doc, c).transpose());
    let (persist, doc_ref) = match directory {
        Some(Err(BinaryError::Encrypted)) => return Err(BinaryError::Encrypted),
        Some(Ok(d)) => d,
        _ => (HashMap::new(), None),
    };
    let doc_off = doc_ref
        .and_then(|r| persist.get(&r).copied())
        .filter(|&o| header(doc, o as usize).is_ok_and(|h| h.typ == RT_DOCUMENT))
        .map(|o| o as usize)
        .or_else(|| scan_top_level(doc, RT_DOCUMENT).first().copied())
        .ok_or_else(|| BinaryError::Missing("DocumentContainer".into()))?;
    let (_, doc_body) = record_at(doc, doc_off)?;

    let mut deck = PptDeck::default();
    let mut slide_lists: Vec<Vec<(u32, u32, Vec<PptText>)>> = vec![Vec::new(); 3];
    let mut list_texts: Vec<Vec<ListText<'_>>> = Vec::new();
    let mut ctx = DrawCtx::default();
    for (h, data) in children(doc_body) {
        match h.typ {
            RT_ENVIRONMENT => ctx.fonts = ppt_draw::fonts(data, 0),
            RT_PP_DRAWING_GROUP => {
                ppt_draw::drawing_group(data, pictures, &mut deck.pictures, &mut ctx);
            }
            RT_SLIDE_LIST_WITH_TEXT if h.instance == 0 => list_texts = slide_list_texts(data),
            _ => {}
        }
    }
    // Colour scheme of the first main master (§2.5.3 MainMasterContainer).
    ctx.scheme = scan_top_level(doc, RT_MAIN_MASTER)
        .first()
        .and_then(|&o| record_at(doc, o).ok())
        .and_then(|(_, body)| ppt_draw::color_scheme(body));
    for (h, data) in children(doc_body) {
        match h.typ {
            RT_DOCUMENT_ATOM => {
                // slideSize: PointStruct in master units (576 / inch) → EMU (914400 / inch)
                let x = i64::from(i32_at(data, 0).unwrap_or(5760));
                let y = i64::from(i32_at(data, 4).unwrap_or(4320));
                deck.slide_width = x * 914_400 / 576;
                deck.slide_height = y * 914_400 / 576;
            }
            RT_VBA_INFO => deck.has_macros = true,
            RT_SLIDE_LIST_WITH_TEXT if h.instance <= 2 => {
                slide_lists[h.instance as usize] = slide_list(data);
            }
            _ => {}
        }
    }

    // Notes by the slide id they belong to.
    let mut notes_by_slide: HashMap<u32, Vec<String>> = HashMap::new();
    for (persist_ref, _, _) in &slide_lists[2] {
        let Some(&off) = persist.get(persist_ref) else {
            continue;
        };
        let Ok((h, body)) = record_at(doc, off as usize) else {
            continue;
        };
        if h.typ != RT_NOTES {
            continue;
        }
        let mut slide_id = 0;
        let mut texts = Vec::new();
        for (ch, cd) in children(body) {
            if ch.typ == RT_NOTES_ATOM {
                slide_id = u32_at(cd, 0).unwrap_or(0);
            }
        }
        collect_texts(body, 0, &mut texts);
        let paras: Vec<String> = texts
            .into_iter()
            .filter(|t| matches!(t.kind, TextKind::Notes | TextKind::Body))
            .flat_map(|t| t.paragraphs)
            .collect();
        notes_by_slide.entry(slide_id).or_default().extend(paras);
    }

    let mut slides_seen = HashSet::new();
    for (i, (persist_ref, slide_id, texts)) in slide_lists[0].iter().take(MAX_SLIDES).enumerate() {
        let mut slide = PptSlide {
            slide_id: *slide_id,
            texts: texts.clone(),
            notes: notes_by_slide.remove(slide_id).unwrap_or_default(),
            shapes: Vec::new(),
        };
        if let Some(&off) = persist.get(persist_ref) {
            slides_seen.insert(off);
            if let Ok((h, body)) = record_at(doc, off as usize) {
                if h.typ == RT_SLIDE {
                    collect_texts(body, 0, &mut slide.texts);
                    let list = list_texts.get(i).map_or(&[][..], Vec::as_slice);
                    slide.shapes = ppt_draw::slide_shapes(body, list, &ctx);
                }
            }
        }
        deck.slides.push(slide);
    }
    if deck.slides.is_empty() {
        // No usable slide list: fall back to top-level SlideContainers in stream order.
        for off in scan_top_level(doc, RT_SLIDE).into_iter().take(MAX_SLIDES) {
            if let Ok((_, body)) = record_at(doc, off) {
                let mut slide = PptSlide::default();
                collect_texts(body, 0, &mut slide.texts);
                slide.shapes = ppt_draw::slide_shapes(body, &[], &ctx);
                deck.slides.push(slide);
            }
        }
    }
    Ok(deck)
}

/// §2.4.14.3 SlideListWithTextContainer: (persistIdRef, slideId, texts) per slide.
fn slide_list(body: &[u8]) -> Vec<(u32, u32, Vec<PptText>)> {
    let mut out: Vec<(u32, u32, Vec<PptText>)> = Vec::new();
    let mut kind: Option<TextKind> = None;
    for (h, data) in children(body) {
        match h.typ {
            RT_SLIDE_PERSIST_ATOM => {
                let persist = u32_at(data, 0).unwrap_or(0);
                let id = u32_at(data, 12).unwrap_or(0);
                out.push((persist, id, Vec::new()));
                kind = None;
            }
            RT_TEXT_HEADER_ATOM => kind = Some(TextKind::from_u32(u32_at(data, 0).unwrap_or(4))),
            RT_TEXT_CHARS_ATOM | RT_TEXT_BYTES_ATOM => {
                if let Some(last) = out.last_mut() {
                    last.2.push(PptText {
                        kind: kind.take().unwrap_or(TextKind::Other),
                        paragraphs: text_paragraphs(&h, data),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// Text atoms (with their StyleTextPropAtom) of each slide of the slide list,
/// in the order `OutlineTextRefAtom.index` counts them.
fn slide_list_texts(body: &[u8]) -> Vec<Vec<ListText<'_>>> {
    let mut out: Vec<Vec<ListText<'_>>> = Vec::new();
    let mut kind = TextKind::Other;
    for (h, data) in children(body) {
        match h.typ {
            RT_SLIDE_PERSIST_ATOM => out.push(Vec::new()),
            RT_TEXT_HEADER_ATOM => kind = TextKind::from_u32(u32_at(data, 0).unwrap_or(4)),
            RT_TEXT_CHARS_ATOM | RT_TEXT_BYTES_ATOM => {
                if let Some(last) = out.last_mut() {
                    last.push(ListText {
                        kind,
                        hdr: h,
                        chars: data,
                        style: None,
                    });
                }
            }
            RT_STYLE_TEXT_PROP_ATOM => {
                if let Some(t) = out.last_mut().and_then(|l| l.last_mut()) {
                    t.style = Some(data);
                }
            }
            _ => {}
        }
    }
    out
}

/// Offsets of top-level records of type `typ`.
fn scan_top_level(doc: &[u8], typ: u16) -> Vec<usize> {
    let mut out = Vec::new();
    let mut p = 0usize;
    while let Ok(h) = header(doc, p) {
        if h.typ == typ {
            out.push(p);
        }
        match p.checked_add(8 + h.len) {
            Some(n) if n <= doc.len() => p = n,
            _ => break,
        }
    }
    out
}

type Directory = (HashMap<u32, u32>, Option<u32>);

/// Build the persist directory from the UserEditAtom chain.
fn persist_directory(doc: &[u8], current: &[u8]) -> Result<Option<Directory>> {
    // §2.3.2 CurrentUserAtom: RecordHeader, size, headerToken, offsetToCurrentEdit
    let token = u32_at(current, 12)?;
    if token == 0xF3D1_C4DF {
        return Err(BinaryError::Encrypted);
    }
    if token != 0xE391_C05F {
        return Ok(None);
    }
    let mut off = u32_at(current, 16)? as usize;
    let mut persist: HashMap<u32, u32> = HashMap::new();
    let mut doc_ref = None;
    let mut seen = HashSet::new();
    for _ in 0..MAX_EDITS {
        if !seen.insert(off) {
            break;
        }
        let Ok((h, ue)) = record_at(doc, off) else {
            break;
        };
        if h.typ != RT_USER_EDIT_ATOM {
            break;
        }
        // §2.3.3 UserEditAtom fields after the header.
        let last_edit = u32_at(ue, 8)? as usize;
        let dir_off = u32_at(ue, 12)? as usize;
        if doc_ref.is_none() {
            doc_ref = Some(u32_at(ue, 16)?);
            if ue.len() >= 32 && u32_at(ue, 28).is_ok_and(|v| v != 0) {
                // encryptSessionPersistIdRef present and non-zero: encrypted document
                return Err(BinaryError::Encrypted);
            }
        }
        if let Ok((dh, dir)) = record_at(doc, dir_off) {
            if dh.typ == RT_PERSIST_DIRECTORY_ATOM {
                // §2.3.5 PersistDirectoryEntry: persistId (20 bits), cPersist (12 bits), offsets
                let mut p = 0usize;
                while let Ok(e) = u32_at(dir, p) {
                    let id0 = e & 0x000F_FFFF;
                    let n = (e >> 20) as usize;
                    p += 4;
                    for k in 0..n {
                        let Ok(o) = u32_at(dir, p + 4 * k) else { break };
                        // Newer edits come first: keep the first offset seen.
                        persist.entry(id0 + k as u32).or_insert(o);
                    }
                    p += 4 * n;
                }
            }
        }
        if last_edit == 0 {
            break;
        }
        off = last_edit;
    }
    Ok(Some((persist, doc_ref)))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn rec(ver: u16, instance: u16, typ: u16, body: &[u8]) -> Vec<u8> {
        let mut v = (ver | (instance << 4)).to_le_bytes().to_vec();
        v.extend(typ.to_le_bytes());
        v.extend((body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn atom(typ: u16, body: &[u8]) -> Vec<u8> {
        rec(0, 0, typ, body)
    }

    fn cont(typ: u16, body: &[u8]) -> Vec<u8> {
        rec(0x0F, 0, typ, body)
    }

    fn chars(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    fn slide_persist(persist: u32, id: u32) -> Vec<u8> {
        let mut b = persist.to_le_bytes().to_vec();
        b.extend([0u8; 8]);
        b.extend(id.to_le_bytes());
        b.extend([0u8; 4]);
        atom(RT_SLIDE_PERSIST_ATOM, &b)
    }

    fn textbox(kind: u32, text: &[u8], bytes: bool) -> Vec<u8> {
        let mut tb = atom(RT_TEXT_HEADER_ATOM, &kind.to_le_bytes());
        tb.extend(atom(
            if bytes {
                RT_TEXT_BYTES_ATOM
            } else {
                RT_TEXT_CHARS_ATOM
            },
            text,
        ));
        // OfficeArtSpContainer > OfficeArtClientTextbox
        cont(0xF004, &cont(RT_CLIENT_TEXTBOX, &tb))
    }

    /// Streams (PowerPoint Document, Current User) for a two-slide deck with notes.
    pub fn sample_streams(encrypted: bool) -> (Vec<u8>, Vec<u8>) {
        let mut doc = Vec::new();
        let mut offsets: Vec<u32> = Vec::new();
        // persist 1: DocumentContainer
        let mut d = atom(RT_DOCUMENT_ATOM, &{
            let mut b = 5760i32.to_le_bytes().to_vec();
            b.extend(3240i32.to_le_bytes());
            b.extend([0u8; 32]);
            b
        });
        let mut slwt = slide_persist(2, 256);
        slwt.extend(atom(RT_TEXT_HEADER_ATOM, &0u32.to_le_bytes()));
        slwt.extend(atom(RT_TEXT_CHARS_ATOM, &chars("Báo cáo")));
        slwt.extend(atom(RT_TEXT_HEADER_ATOM, &1u32.to_le_bytes()));
        slwt.extend(atom(RT_TEXT_CHARS_ATOM, &chars("Mục 1\rMục 2\u{b}tiếp")));
        slwt.extend(slide_persist(3, 257));
        d.extend(rec(0x0F, 0, RT_SLIDE_LIST_WITH_TEXT, &slwt));
        d.extend(rec(
            0x0F,
            2,
            RT_SLIDE_LIST_WITH_TEXT,
            &slide_persist(4, 0x100),
        ));
        d.extend(cont(RT_VBA_INFO, &[]));
        offsets.push(doc.len() as u32);
        doc.extend(cont(RT_DOCUMENT, &d));
        // persist 2: slide 1 drawing with a placeholder ref and a free text box
        let mut s1 = atom(0x03EF, &[0u8; 24]);
        let mut drawing = cont(
            0xF004,
            &cont(RT_CLIENT_TEXTBOX, &atom(RT_OUTLINE_TEXT_REF_ATOM, &[0; 4])),
        );
        drawing.extend(textbox(4, b"Ghi chu", true));
        s1.extend(cont(0x040C, &cont(0xF002, &drawing)));
        offsets.push(doc.len() as u32);
        doc.extend(cont(RT_SLIDE, &s1));
        // persist 3: slide 2 (title only in its drawing)
        offsets.push(doc.len() as u32);
        doc.extend(cont(RT_SLIDE, &textbox(6, &chars("Kết thúc"), false)));
        // persist 4: notes for slide 256
        let mut n = atom(RT_NOTES_ATOM, &256u32.to_le_bytes());
        n.extend(textbox(2, &chars("Nói chậm"), false));
        offsets.push(doc.len() as u32);
        doc.extend(cont(RT_NOTES, &n));
        // PersistDirectoryAtom: persistId 1, 4 entries
        let mut dir = (1u32 | (4 << 20)).to_le_bytes().to_vec();
        for o in &offsets {
            dir.extend(o.to_le_bytes());
        }
        let dir_off = doc.len() as u32;
        doc.extend(atom(RT_PERSIST_DIRECTORY_ATOM, &dir));
        let ue_off = doc.len() as u32;
        let mut ue = 256u32.to_le_bytes().to_vec();
        ue.extend([0, 0, 0, 3]);
        ue.extend(0u32.to_le_bytes()); // offsetLastEdit
        ue.extend(dir_off.to_le_bytes());
        ue.extend(1u32.to_le_bytes()); // docPersistIdRef
        ue.extend(5u32.to_le_bytes()); // persistIdSeed
        ue.extend([0, 0, 0, 0]);
        doc.extend(atom(RT_USER_EDIT_ATOM, &ue));
        let token: u32 = if encrypted { 0xF3D1_C4DF } else { 0xE391_C05F };
        let mut cu = 20u32.to_le_bytes().to_vec();
        cu.extend(token.to_le_bytes());
        cu.extend(ue_off.to_le_bytes());
        cu.extend([0u8; 8]);
        let current = atom(0x0FF6, &cu);
        (doc, current)
    }

    /// A 2 × 2 RGB PNG (generated, not a real picture).
    pub fn tiny_png() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 2, 2);
            enc.set_color(png::ColorType::Rgb);
            if let Ok(mut w) = enc.write_header() {
                let _ = w.write_image_data(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]);
            }
        }
        out
    }

    pub fn sample_ppt() -> Vec<u8> {
        let (doc, cu) = sample_streams(false);
        crate::test_util::cfb_file(&[("/PowerPoint Document", doc), ("/Current User", cu)], &[])
    }

    #[test]
    fn reads_sample_deck() {
        let deck = read_ppt(&sample_ppt()).unwrap();
        assert_eq!(deck.slide_width, 9_144_000);
        assert_eq!(deck.slide_height, 5_143_500);
        assert!(deck.has_macros);
        assert_eq!(deck.slides.len(), 2);
        let s1 = &deck.slides[0];
        assert_eq!(s1.slide_id, 256);
        assert_eq!(s1.title().as_deref(), Some("Báo cáo"));
        assert_eq!(s1.texts[1].kind, TextKind::Body);
        assert_eq!(s1.texts[1].paragraphs, vec!["Mục 1", "Mục 2\u{2028}tiếp"]);
        assert_eq!(s1.texts.len(), 3, "outline ref box is not duplicated");
        assert_eq!(s1.texts[2].paragraphs, vec!["Ghi chu"]);
        assert_eq!(s1.notes, vec!["Nói chậm"]);
        assert_eq!(deck.slides[1].title().as_deref(), Some("Kết thúc"));
    }

    #[test]
    fn fallback_without_current_user() {
        let (doc, _) = sample_streams(false);
        let deck = parse_ppt(&doc, None).unwrap();
        // Without the persist directory the slide list cannot be resolved to
        // drawings, but its text is still read.
        assert_eq!(deck.slides.len(), 2);
        assert_eq!(deck.slides[0].title().as_deref(), Some("Báo cáo"));
    }

    #[test]
    fn encrypted_is_reported() {
        let (doc, cu) = sample_streams(true);
        assert!(matches!(
            parse_ppt(&doc, Some(&cu)),
            Err(BinaryError::Encrypted)
        ));
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]
        #[test]
        fn mutated_never_panics(pos in 0usize..4096, val in proptest::num::u8::ANY, cut in 0usize..4096) {
            let (mut doc, cu) = sample_streams(false);
            let i = pos % doc.len();
            doc[i] = val;
            let _ = parse_ppt(&doc, Some(&cu));
            doc.truncate(cut % (doc.len() + 1));
            let _ = parse_ppt(&doc, Some(&cu));
        }

        #[test]
        fn random_never_panics(a in proptest::collection::vec(proptest::num::u8::ANY, 0..1024),
                               b in proptest::collection::vec(proptest::num::u8::ANY, 0..64)) {
            let _ = parse_ppt(&a, Some(&b));
            let _ = parse_ppt(&a, None);
            let _ = read_ppt(&a);
        }
    }
}
