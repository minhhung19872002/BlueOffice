//! Best-effort opening of a PDF file as a BlueDoc document (CORE-FMT-004).
//!
//! PDF is a page-description format: it stores positioned glyphs, not paragraphs.
//! Only the text is recovered, the way ISO 32000-1 §9.10 "Extraction of Text
//! Content" describes: the text-showing operators of each page content stream are
//! decoded through the font's `ToUnicode` CMap (§9.10.3) or its simple encoding,
//! and the text positions (§9.4.2 text-positioning operators) decide where a line
//! ends and where a word gap is. One paragraph per line; every PDF page starts on
//! a new page. Layout, pictures, tables, fonts and text inside form XObjects are not
//! reconstructed. The file structure (objects, cross references, streams, content
//! operators) is parsed by `lopdf` (MIT); a page or font that cannot be decoded is
//! skipped instead of failing the whole file.

use crate::{EngineError, Result};
use bo_model::{Block, Document, ParaProps, Paragraph, Run, RunProps, Section};
use lopdf::{Dictionary, Object};
use std::collections::{BTreeMap, HashMap};

/// Whether `bytes` look like a PDF file: the `%PDF-` header (ISO 32000-1, §7.5.2
/// "File Header"), which readers accept within the first 1024 bytes. A ZIP package
/// or compound file is never one, whatever it holds.
pub fn is_pdf(bytes: &[u8]) -> bool {
    if bytes.starts_with(b"PK") || bytes.starts_with(crate::CFB_SIGNATURE) {
        return false;
    }
    let head = &bytes[..bytes.len().min(1024)];
    head.windows(5).any(|w| w == b"%PDF-")
}

/// Upper bounds, so a hostile file cannot make opening unbounded.
const MAX_PAGES: usize = 10_000;
const MAX_PAGE_CHARS: usize = 1 << 20;
const MAX_CMAP_ENTRIES: usize = 1 << 18;
const MAX_WIDTHS: usize = 1 << 16;

/// The text of every page of a PDF file as a document. Errors only when the file
/// structure itself cannot be read (or it is encrypted with a password).
pub fn read_pdf(bytes: &[u8]) -> Result<Document> {
    let pdf =
        lopdf::Document::load_mem(bytes).map_err(|e| EngineError::Format(format!("PDF: {e}")))?;
    if pdf.is_encrypted() {
        // ISO 32000-1, §7.6 Encryption: lopdf already tried the empty user password.
        return Err(EngineError::Format(
            "PDF: the file is password protected".into(),
        ));
    }
    let texts: Vec<String> = pdf
        .get_pages()
        .values()
        .take(MAX_PAGES)
        .map(|&id| page_text(&pdf, id).unwrap_or_default())
        .collect();
    if texts.is_empty() {
        return Err(EngineError::Format("PDF: the file has no pages".into()));
    }
    Ok(document_of_pages(&texts))
}

/// One paragraph per non-blank line; every page after the first starts on a new
/// page (`w:pageBreakBefore`, ECMA-376 Part 1, §17.3.1.23).
fn document_of_pages(pages: &[String]) -> Document {
    let mut blocks = Vec::new();
    for (i, page) in pages.iter().enumerate() {
        let mut first = true;
        for line in page.lines() {
            let line: String = line.chars().filter(|c| !c.is_control()).collect();
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let props = ParaProps {
                page_break_before: (first && i > 0).then_some(true),
                ..ParaProps::default()
            };
            blocks.push(Block::Paragraph(Paragraph::new(
                props,
                vec![Run::text(line, RunProps::default())],
            )));
            first = false;
        }
    }
    let mut doc = crate::plain_text_document("");
    if !blocks.is_empty() {
        doc.sections = vec![Section::new(blocks)];
    }
    doc
}

// ---------------------------------------------------------------------------
// Fonts

/// What text extraction needs of a font resource (ISO 32000-1, §9.6 simple fonts,
/// §9.7 composite fonts).
struct FontInfo<'a> {
    /// Composite (`Type0`) font: two-byte codes (Identity-H / Identity-V and the
    /// usual CJK CMaps; §9.7.5).
    two_byte: bool,
    /// `ToUnicode` CMap: character code → text (§9.10.3).
    to_unicode: HashMap<u32, String>,
    /// Simple-font encoding, when there is no usable `ToUnicode` (§9.6.6).
    encoding: Option<lopdf::Encoding<'a>>,
    /// Glyph widths in thousandths of an em: simple fonts `FirstChar` + `Widths`
    /// (§9.6.2), composite fonts the descendant's `W` array (§9.7.4.3).
    widths: HashMap<u32, f32>,
    default_width: f32,
}

impl FontInfo<'_> {
    fn width(&self, code: u32) -> f32 {
        self.widths
            .get(&code)
            .copied()
            .unwrap_or(self.default_width)
    }

    fn text(&self, code: u32) -> String {
        if let Some(s) = self.to_unicode.get(&code) {
            return s.clone();
        }
        if !self.two_byte {
            if let (Some(enc), Ok(b)) = (&self.encoding, u8::try_from(code)) {
                if let Ok(s) = lopdf::Document::decode_text(enc, &[b]) {
                    return s;
                }
            }
            // No encoding information: the byte as Latin-1 (best effort).
            if let Some(c) = char::from_u32(code).filter(|c| !c.is_control()) {
                return c.to_string();
            }
        }
        String::new()
    }
}

fn deref<'a>(pdf: &'a lopdf::Document, obj: &'a Object) -> Option<&'a Object> {
    pdf.dereference(obj).ok().map(|(_, o)| o)
}

fn get<'a>(pdf: &'a lopdf::Document, dict: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    deref(pdf, dict.get(key).ok()?)
}

fn number(o: &Object) -> Option<f32> {
    o.as_float().ok().filter(|v| v.is_finite())
}

fn font_info<'a>(pdf: &'a lopdf::Document, font: &'a Dictionary) -> FontInfo<'a> {
    let two_byte = font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0");
    let to_unicode = get(pdf, font, b"ToUnicode")
        .and_then(|o| o.as_stream().ok())
        .and_then(|s| {
            s.decompressed_content()
                .ok()
                .or_else(|| Some(s.content.clone()))
        })
        .map(|data| parse_to_unicode(&data))
        .unwrap_or_default();
    let encoding = if two_byte || !to_unicode.is_empty() {
        None
    } else {
        font.get_font_encoding(pdf).ok()
    };
    let mut widths = HashMap::new();
    let mut default_width = 500.0;
    if two_byte {
        // §9.7.4.3: DW defaults to 1000; W is `c [w1 w2 …]` or `cfirst clast w`.
        default_width = 1000.0;
        let desc = get(pdf, font, b"DescendantFonts")
            .and_then(|o| o.as_array().ok())
            .and_then(|a| a.first())
            .and_then(|o| deref(pdf, o))
            .and_then(|o| o.as_dict().ok());
        if let Some(desc) = desc {
            if let Some(dw) = get(pdf, desc, b"DW").and_then(number) {
                default_width = dw;
            }
            if let Some(w) = get(pdf, desc, b"W").and_then(|o| o.as_array().ok()) {
                cid_widths(pdf, w, &mut widths);
            }
        }
    } else if let Some(w) = get(pdf, font, b"Widths").and_then(|o| o.as_array().ok()) {
        let first = get(pdf, font, b"FirstChar")
            .and_then(|o| o.as_i64().ok())
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0);
        for (i, o) in w.iter().take(MAX_WIDTHS).enumerate() {
            if let (Some(v), Ok(i)) = (deref(pdf, o).and_then(number), u32::try_from(i)) {
                widths.insert(first.saturating_add(i), v);
            }
        }
    }
    FontInfo {
        two_byte,
        to_unicode,
        encoding,
        widths,
        default_width,
    }
}

fn cid_widths(pdf: &lopdf::Document, w: &[Object], out: &mut HashMap<u32, f32>) {
    let code = |o: &Object| {
        deref(pdf, o)
            .and_then(|o| o.as_i64().ok())
            .and_then(|v| u32::try_from(v).ok())
    };
    let mut i = 0;
    while i < w.len() && out.len() < MAX_WIDTHS {
        let Some(first) = code(&w[i]) else { break };
        match w.get(i + 1).and_then(|o| deref(pdf, o)) {
            Some(Object::Array(list)) => {
                for (k, o) in list.iter().enumerate() {
                    if let (Some(v), Ok(k)) = (deref(pdf, o).and_then(number), u32::try_from(k)) {
                        out.insert(first.saturating_add(k), v);
                    }
                }
                i += 2;
            }
            Some(o) => {
                let (Some(last), Some(v)) = (
                    code(o),
                    w.get(i + 2).and_then(|o| deref(pdf, o)).and_then(number),
                ) else {
                    break;
                };
                let last = last.min(first.saturating_add(MAX_WIDTHS as u32));
                for c in first..=last {
                    out.insert(c, v);
                }
                i += 3;
            }
            None => break,
        }
    }
}

// ---------------------------------------------------------------------------
// ToUnicode CMaps

#[derive(Debug, PartialEq)]
enum Token {
    Hex(Vec<u8>),
    Open,
    Close,
    Word(Vec<u8>),
}

/// Tokens of a CMap file (PostScript syntax subset: hexadecimal strings, arrays,
/// names / operators; comments, dictionaries and literal strings are skipped).
fn tokens(data: &[u8]) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        match b {
            b'%' => {
                while i < data.len() && data[i] != b'\n' && data[i] != b'\r' {
                    i += 1;
                }
            }
            b'<' if data.get(i + 1) == Some(&b'<') => i += 2,
            b'>' if data.get(i + 1) == Some(&b'>') => i += 2,
            b'<' => {
                let mut digits = Vec::new();
                i += 1;
                while i < data.len() && data[i] != b'>' {
                    if let Some(d) = char::from(data[i]).to_digit(16) {
                        digits.push(d as u8);
                    }
                    i += 1;
                }
                i += 1;
                if digits.len() % 2 == 1 {
                    digits.push(0);
                }
                out.push(Token::Hex(
                    digits.chunks(2).map(|p| p[0] << 4 | p[1]).collect(),
                ));
            }
            b'(' => {
                let mut depth = 0usize;
                while i < data.len() {
                    match data[i] {
                        b'\\' => i += 1,
                        b'(' => depth += 1,
                        b')' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                i += 1;
            }
            b'[' => {
                out.push(Token::Open);
                i += 1;
            }
            b']' => {
                out.push(Token::Close);
                i += 1;
            }
            _ if b.is_ascii_whitespace() || b == 0 => i += 1,
            _ => {
                let start = i;
                while i < data.len()
                    && !data[i].is_ascii_whitespace()
                    && !b"<>[]()%/".contains(&data[i])
                {
                    i += 1;
                }
                if i == start {
                    i += 1; // a lone `/` or other delimiter
                } else {
                    out.push(Token::Word(data[start..i].to_vec()));
                }
            }
        }
    }
    out
}

fn code_of(bytes: &[u8]) -> Option<u32> {
    (bytes.len() <= 4).then(|| bytes.iter().fold(0u32, |a, &b| a << 8 | u32::from(b)))
}

/// Destination text of a CMap mapping: UTF-16BE (§9.10.3).
fn utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|p| u16::from_be_bytes([p[0], p.get(1).copied().unwrap_or(0)]))
        .collect();
    String::from_utf16_lossy(&units)
}

/// `bfchar` and `bfrange` mappings of a `ToUnicode` CMap (ISO 32000-1, §9.10.3;
/// CMap syntax of Adobe Technical Note #5014 "CMap and CIDFont Files Specification").
fn parse_to_unicode(data: &[u8]) -> HashMap<u32, String> {
    let toks = tokens(data);
    let mut map = HashMap::new();
    let mut i = 0;
    while i < toks.len() && map.len() < MAX_CMAP_ENTRIES {
        match &toks[i] {
            Token::Word(w) if w == b"beginbfchar" => {
                i += 1;
                while let (Some(Token::Hex(src)), Some(Token::Hex(dst))) =
                    (toks.get(i), toks.get(i + 1))
                {
                    if let Some(c) = code_of(src) {
                        map.insert(c, utf16(dst));
                    }
                    i += 2;
                }
            }
            Token::Word(w) if w == b"beginbfrange" => {
                i += 1;
                while let (Some(Token::Hex(lo)), Some(Token::Hex(hi))) =
                    (toks.get(i), toks.get(i + 1))
                {
                    let (Some(lo), Some(hi)) = (code_of(lo), code_of(hi)) else {
                        break;
                    };
                    let hi = hi.min(lo.saturating_add(0xFFFF));
                    i += 2;
                    match toks.get(i) {
                        Some(Token::Hex(dst)) => {
                            // Consecutive codes map to consecutive values: the last
                            // UTF-16 unit is incremented.
                            let mut d = dst.clone();
                            for c in lo..=hi {
                                map.insert(c, utf16(&d));
                                if map.len() >= MAX_CMAP_ENTRIES {
                                    break;
                                }
                                let n = d.len();
                                if n >= 2 {
                                    let next =
                                        u16::from_be_bytes([d[n - 2], d[n - 1]]).wrapping_add(1);
                                    d[n - 2..].copy_from_slice(&next.to_be_bytes());
                                }
                            }
                            i += 1;
                        }
                        Some(Token::Open) => {
                            i += 1;
                            let mut c = lo;
                            while let Some(Token::Hex(dst)) = toks.get(i) {
                                if c <= hi {
                                    map.insert(c, utf16(dst));
                                }
                                c = c.saturating_add(1);
                                i += 1;
                            }
                            if toks.get(i) == Some(&Token::Close) {
                                i += 1;
                            }
                        }
                        _ => break,
                    }
                }
            }
            _ => i += 1,
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Page content

/// Text state of the content walk (ISO 32000-1, §9.3 text state parameters,
/// §9.4.2 text matrices).
struct Walk<'f, 'a> {
    fonts: &'f BTreeMap<Vec<u8>, FontInfo<'a>>,
    font: Option<&'f FontInfo<'a>>,
    size: f32,
    char_spacing: f32,
    word_spacing: f32,
    h_scale: f32,
    leading: f32,
    /// Text matrix `[a b c d e f]` and text line matrix.
    tm: [f32; 6],
    tlm: [f32; 6],
    out: String,
    /// Where the last shown text ended (x) and its baseline (y), text space.
    last: Option<(f32, f32)>,
    gap: bool,
}

const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

impl Walk<'_, '_> {
    fn em(&self) -> f32 {
        let [_, _, c, d, _, _] = self.tm;
        (self.size * (c * c + d * d).sqrt()).abs().max(0.1)
    }

    fn move_line(&mut self, tx: f32, ty: f32) {
        let [a, b, c, d, e, f] = self.tlm;
        self.tlm = [a, b, c, d, e + tx * a + ty * c, f + tx * b + ty * d];
        self.tm = self.tlm;
    }

    fn advance(&mut self, w: f32) {
        let [a, b, ..] = self.tm;
        self.tm[4] += w * a;
        self.tm[5] += w * b;
    }

    fn show(&mut self, bytes: &[u8]) {
        let Some(font) = self.font else { return };
        let (x, y) = (self.tm[4], self.tm[5]);
        let em = self.em();
        if let Some((end, base)) = self.last {
            if (y - base).abs() > em * 0.5 {
                self.out.push('\n');
            } else if (self.gap || x > end + em * 0.15 || x < end - em * 4.0)
                && !self.out.ends_with([' ', '\n'])
            {
                self.out.push(' ');
            }
        }
        self.gap = false;
        let step = if font.two_byte { 2 } else { 1 };
        for unit in bytes.chunks(step) {
            let Some(code) = code_of(unit) else { continue };
            if self.out.len() < MAX_PAGE_CHARS {
                self.out.push_str(&font.text(code));
            }
            let mut w = font.width(code) / 1000.0 * self.size + self.char_spacing;
            if step == 1 && code == 32 {
                w += self.word_spacing;
            }
            self.advance(w * self.h_scale);
        }
        self.last = Some((self.tm[4], y));
    }

    fn op(&mut self, op: &str, args: &[Object]) {
        let n = |i: usize| args.get(i).and_then(number).unwrap_or(0.0);
        match op {
            "BT" => {
                self.tm = IDENTITY;
                self.tlm = IDENTITY;
            }
            "Tf" => {
                self.font = args
                    .first()
                    .and_then(|o| o.as_name().ok())
                    .and_then(|name| self.fonts.get(name));
                self.size = n(1);
            }
            "Tc" => self.char_spacing = n(0),
            "Tw" => self.word_spacing = n(0),
            "Tz" => self.h_scale = n(0) / 100.0,
            "TL" => self.leading = n(0),
            "Tm" if args.len() >= 6 => {
                self.tlm = [n(0), n(1), n(2), n(3), n(4), n(5)];
                self.tm = self.tlm;
            }
            "Td" => self.move_line(n(0), n(1)),
            "TD" => {
                self.leading = -n(1);
                self.move_line(n(0), n(1));
            }
            "T*" => self.move_line(0.0, -self.leading),
            "Tj" => {
                if let Some(Ok(s)) = args.first().map(Object::as_str) {
                    self.show(s);
                }
            }
            "'" => {
                self.move_line(0.0, -self.leading);
                if let Some(Ok(s)) = args.first().map(Object::as_str) {
                    self.show(s);
                }
            }
            "\"" => {
                self.word_spacing = n(0);
                self.char_spacing = n(1);
                self.move_line(0.0, -self.leading);
                if let Some(Ok(s)) = args.get(2).map(Object::as_str) {
                    self.show(s);
                }
            }
            "TJ" => {
                let Some(Ok(items)) = args.first().map(Object::as_array) else {
                    return;
                };
                for item in items {
                    match item {
                        Object::String(s, _) => self.show(s),
                        other => {
                            // §9.4.3: a number moves the next glyph left by
                            // thousandths of an em; a large negative one is a gap.
                            let k = number(other).unwrap_or(0.0);
                            self.advance(-k / 1000.0 * self.size * self.h_scale);
                            if k < -250.0 {
                                self.gap = true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn page_text(pdf: &lopdf::Document, page: lopdf::ObjectId) -> Option<String> {
    let fonts: BTreeMap<Vec<u8>, FontInfo> = pdf
        .get_page_fonts(page)
        .ok()?
        .into_iter()
        .map(|(name, dict)| (name, font_info(pdf, dict)))
        .collect();
    let content = pdf.get_page_content(page).ok()?;
    let content = lopdf::content::Content::decode(&content).ok()?;
    let mut walk = Walk {
        fonts: &fonts,
        font: None,
        size: 0.0,
        char_spacing: 0.0,
        word_spacing: 0.0,
        h_scale: 1.0,
        leading: 0.0,
        tm: IDENTITY,
        tlm: IDENTITY,
        out: String::new(),
        last: None,
        gap: false,
    };
    for op in &content.operations {
        walk.op(&op.operator, &op.operands);
    }
    Some(walk.out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_the_header() {
        assert!(is_pdf(b"%PDF-1.7\n"));
        assert!(is_pdf(b"\xEF\xBB\xBF junk %PDF-1.4"));
        assert!(!is_pdf(b"PK\x03\x04 stored %PDF-1.4"));
        assert!(!is_pdf(b""));
    }

    #[test]
    fn pages_become_paragraphs_with_page_breaks() {
        let doc = document_of_pages(&["Một\n\n  \nHai\n".into(), String::new(), "Ba\n".into()]);
        let paras: Vec<_> = doc.sections[0]
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Paragraph(p) => Some((p.plain_text(), p.props.page_break_before)),
                _ => None,
            })
            .collect();
        assert_eq!(
            paras,
            vec![
                ("Một".to_owned(), None),
                ("Hai".to_owned(), None),
                ("Ba".to_owned(), Some(true)),
            ]
        );
    }

    #[test]
    fn to_unicode_cmaps_bfchar_and_bfrange() {
        // bfchar pairs, a bfrange with a starting value and one with an array.
        let cmap = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
            /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
            1 begincodespacerange <0000> <FFFF> endcodespacerange\n\
            2 beginbfchar <0003> <0020> <0011> <1EA1> endbfchar % comment <0012> <0041>\n\
            2 beginbfrange <0020> <0022> <0041> <0030> <0031> [<00660069> <00E9>] endbfrange\n\
            endcmap CMapName currentdict /CMap defineresource pop end end";
        let m = parse_to_unicode(cmap);
        assert_eq!(m.get(&3).map(String::as_str), Some(" "));
        assert_eq!(m.get(&0x11).map(String::as_str), Some("ạ"));
        assert_eq!(m.get(&0x12), None);
        assert_eq!(m.get(&0x20).map(String::as_str), Some("A"));
        assert_eq!(m.get(&0x22).map(String::as_str), Some("C"));
        assert_eq!(m.get(&0x30).map(String::as_str), Some("fi"));
        assert_eq!(m.get(&0x31).map(String::as_str), Some("é"));
        assert_eq!(m.len(), 7);
        // Damaged CMaps never panic.
        for cut in 0..cmap.len() {
            let _ = parse_to_unicode(&cmap[..cut]);
        }
    }

    #[test]
    fn text_positions_make_lines_and_word_gaps() {
        let fonts = BTreeMap::from([(
            b"F1".to_vec(),
            FontInfo {
                two_byte: false,
                to_unicode: HashMap::new(),
                encoding: None,
                widths: HashMap::new(),
                default_width: 500.0,
            },
        )]);
        let content = lopdf::content::Content::decode(
            b"BT /F1 10 Tf 72 700 Td (Ab) Tj (c) Tj 20 0 Td (de) Tj \
              [(f) -400 (g)] TJ 0 -14 TD (Line) Tj T* (Three) Tj ET",
        )
        .unwrap();
        let mut walk = Walk {
            fonts: &fonts,
            font: None,
            size: 0.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            h_scale: 1.0,
            leading: 0.0,
            tm: IDENTITY,
            tlm: IDENTITY,
            out: String::new(),
            last: None,
            gap: false,
        };
        for op in &content.operations {
            walk.op(&op.operator, &op.operands);
        }
        // `Abc` continues; the Td jump of 20 (beyond the 15 advanced) is a gap.
        assert_eq!(walk.out, "Abc def g\nLine\nThree");
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(read_pdf(b"%PDF-1.4\n garbage").is_err());
    }
}
