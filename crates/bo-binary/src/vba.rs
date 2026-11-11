//! VBA macro projects: detection, inspection and preservation — never execution.
//!
//! * [`has_macros`] — macro presence in OOXML (`vbaProject.bin` part) and in
//!   the binary formats (`Macros` / `_VBA_PROJECT_CUR` storages, XLM macro
//!   sheets, PowerPoint `VBAInfoContainer`).
//! * [`read_vba_project`] / [`read_vba_from_file`] — module list from the compressed
//!   `VBA/dir` stream ([MS-OVBA] §2.3.4.2) and the source text of every module
//!   stream (§2.3.4.3), using the decompression of [MS-OVBA] §2.4.1. The source is
//!   shown read-only (CORE-VBA-002); it is never compiled or run.
//! * [`compress`] and [`crate::vba_write`] — the reverse, for generated test files.
//! * [`carry_over`] — copy the VBA project parts of a source package into a
//!   package produced by a writer, so macros survive open → save (prompt §6
//!   Phase 5: "giữ nguyên khi lưu, cảnh báo người dùng, KHÔNG thực thi").

use crate::bytes::{u16_at, u32_at, utf16};
use crate::{is_compound_file, malformed, Cfb, Result};
use bo_opc::{rels_part_name, resolve_target, Package, Relationship, TargetMode};
use std::io::{Cursor, Read};

/// Relationship type from the main part to the VBA project ([MS-OFFMACRO2] / [MS-OI29500]).
pub const REL_VBA_PROJECT: &str =
    "http://schemas.microsoft.com/office/2006/relationships/vbaProject";
/// Content type of `vbaProject.bin`.
pub const CT_VBA_PROJECT: &str = "application/vnd.ms-office.vbaProject";
/// Maximum decompressed size of one VBA stream (64 MiB).
const MAX_DECOMPRESSED: usize = 64 << 20;
/// Maximum decompressed size of all module sources of one project (64 MiB).
const MAX_SOURCE_TOTAL: usize = 64 << 20;
/// Maximum number of modules read from a `dir` stream.
const MAX_MODULES: usize = 4096;
/// Maximum size of a `vbaProject.bin` part read from a package (256 MiB).
const MAX_PROJECT_BIN: u64 = 256 << 20;
/// Storages that hold a VBA project: `vbaProject.bin` itself (root), Word binary
/// documents ([MS-DOC] `Macros`), Excel binary workbooks ([MS-XLS] `_VBA_PROJECT_CUR`).
const PROJECT_ROOTS: [&str; 3] = ["", "/Macros", "/_VBA_PROJECT_CUR"];

/// Kind of a VBA module.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VbaModuleKind {
    /// Procedural ("standard") module.
    #[default]
    Standard,
    /// Module of a document object (the document, the workbook, a sheet…).
    Document,
    /// Class module.
    Class,
    /// Designer module (a form).
    Form,
}

impl VbaModuleKind {
    /// Stable identifier: `standard`, `document`, `class`, `form`.
    pub fn as_str(self) -> &'static str {
        match self {
            VbaModuleKind::Standard => "standard",
            VbaModuleKind::Document => "document",
            VbaModuleKind::Class => "class",
            VbaModuleKind::Form => "form",
        }
    }
}

/// A module of a VBA project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VbaModule {
    /// Module name as declared in the `dir` stream.
    pub name: String,
    /// Stream name inside the `VBA` storage.
    pub stream_name: String,
    /// Not a procedural module ([MS-OVBA] MODULETYPE 0x0022: document, class or designer module).
    pub document_module: bool,
    /// Kind of the module (MODULETYPE refined by the `PROJECT` stream).
    pub kind: VbaModuleKind,
    /// Offset of the compressed source in the module stream ([MS-OVBA] MODULEOFFSET).
    pub text_offset: u32,
    /// Source text (line ends as stored, normally CR LF). Empty when [`Self::unreadable`].
    pub source: String,
    /// The module stream is missing, damaged or over the size limits.
    pub unreadable: bool,
}

/// A VBA project: its modules and their source text. Nothing is compiled or run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VbaProject {
    /// Project name (`PROJECTNAME` record of the `dir` stream).
    pub name: String,
    /// Code page of the project's text ([MS-OVBA] PROJECTCODEPAGE); 0 when absent.
    pub code_page: u16,
    /// `bo-encoding` has a table for [`Self::code_page`]; otherwise the text is read
    /// as Windows-1252 and non-ASCII characters may be wrong.
    pub code_page_supported: bool,
    /// Modules of the project.
    pub modules: Vec<VbaModule>,
    /// Modules or sources were left out because of the size limits.
    pub truncated: bool,
}

/// Whether a file (any supported container) carries macros.
pub fn has_macros(bytes: &[u8]) -> bool {
    if is_compound_file(bytes) {
        let Ok(cfb) = Cfb::open(bytes) else {
            return false;
        };
        if cfb.has_storage("/Macros") || cfb.has_storage("/_VBA_PROJECT_CUR") {
            return true;
        }
        if cfb.has("/Workbook") {
            return crate::xls::read_xls(bytes).is_ok_and(|w| w.has_macros);
        }
        if cfb.has("/PowerPoint Document") {
            return crate::ppt::read_ppt(bytes).is_ok_and(|d| d.has_macros);
        }
        return false;
    }
    // ZIP: only the central directory is inspected.
    let Ok(zip) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return false;
    };
    let found = zip
        .file_names()
        .any(|n| n.is_ok_and(|n| n.to_ascii_lowercase().ends_with("vbaproject.bin")));
    found
}

/// Whether an opened OPC package carries a VBA project.
pub fn package_has_macros(pkg: &Package) -> bool {
    pkg.parts()
        .iter()
        .any(|p| p.name.to_ascii_lowercase().ends_with("/vbaproject.bin"))
}

/// [MS-OVBA] §2.4.1.3.1 decompression of a CompressedContainer.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    decompress_limited(data, MAX_DECOMPRESSED)
}

/// [`decompress`] refusing more than `limit` decompressed bytes.
pub fn decompress_limited(data: &[u8], limit: usize) -> Result<Vec<u8>> {
    if data.first() != Some(&0x01) {
        return Err(malformed("VBA: bad compressed container signature"));
    }
    let mut out: Vec<u8> = Vec::with_capacity(data.len().saturating_mul(2).min(limit));
    let mut p = 1usize;
    while p + 2 <= data.len() {
        // §2.4.1.1.5 CompressedChunkHeader
        let hdr = u16_at(data, p)?;
        let size = usize::from(hdr & 0x0FFF) + 3;
        if (hdr >> 12) & 0x7 != 0b011 {
            return Err(malformed("VBA: bad chunk signature"));
        }
        let end = (p + size).min(data.len());
        let chunk_start = out.len();
        let mut q = p + 2;
        if hdr & 0x8000 == 0 {
            // uncompressed chunk: 4096 raw bytes
            out.extend_from_slice(data.get(q..end).unwrap_or_default());
        } else {
            while q < end {
                let flags = data[q];
                q += 1;
                for bit in 0..8 {
                    if q >= end {
                        break;
                    }
                    if flags & (1 << bit) == 0 {
                        out.push(data[q]);
                        q += 1;
                    } else {
                        // §2.4.1.3.19.1 CopyToken help
                        let token = u16_at(data, q)?;
                        q += 2;
                        let diff = out.len() - chunk_start;
                        let mut bit_count = 4u32;
                        while (1usize << bit_count) < diff {
                            bit_count += 1;
                        }
                        let bit_count = bit_count.min(12);
                        let length_mask = 0xFFFFu16 >> bit_count;
                        let len = usize::from(token & length_mask) + 3;
                        let offset = usize::from(token >> (16 - bit_count)) + 1;
                        if offset > diff {
                            return Err(malformed("VBA: copy token before chunk start"));
                        }
                        for _ in 0..len {
                            let b = out[out.len() - offset];
                            out.push(b);
                        }
                    }
                    if out.len() > limit {
                        return Err(malformed("VBA: decompressed stream too large"));
                    }
                }
            }
        }
        if out.len() > limit {
            return Err(malformed("VBA: decompressed stream too large"));
        }
        p = end;
    }
    Ok(out)
}

/// [MS-OVBA] §2.4.1.3.6 compression into a CompressedContainer: 4096-byte chunks
/// of literal and copy tokens; a chunk that does not shrink is stored raw
/// (§2.4.1.3.10, padded with zero bytes to 4096 as the specification requires, so
/// an incompressible last chunk decompresses with trailing zeros).
pub fn compress(data: &[u8]) -> Vec<u8> {
    // §2.4.1.1.1 CompressedContainer: SignatureByte 0x01, then the chunks.
    let mut out = vec![0x01];
    for chunk in data.chunks(4096) {
        if let Some(body) = compress_chunk(chunk) {
            // §2.4.1.1.5 CompressedChunkHeader: size − 3, signature 0b011, flag 1.
            let size = u16::try_from(body.len() + 2 - 3).unwrap_or(0x0FFF) & 0x0FFF;
            out.extend((0xB000 | size).to_le_bytes());
            out.extend(body);
        } else {
            out.extend((0x3000u16 | 4095).to_le_bytes());
            out.extend_from_slice(chunk);
            out.resize(out.len() + 4096 - chunk.len(), 0);
        }
    }
    out
}

/// Number of bits of the offset in a CopyToken when `diff` bytes of the chunk are
/// decompressed ([MS-OVBA] §2.4.1.3.19.1 CopyToken Help).
fn copy_token_bits(diff: usize) -> u32 {
    let mut bit_count = 4u32;
    while (1usize << bit_count) < diff && bit_count < 12 {
        bit_count += 1;
    }
    bit_count
}

/// Token sequences of one chunk (§2.4.1.3.7); `None` when they need more than 4096 bytes.
fn compress_chunk(chunk: &[u8]) -> Option<Vec<u8>> {
    /// How far back matches are searched (the format allows the whole chunk).
    const WINDOW: usize = 1024;
    let mut out = Vec::with_capacity(chunk.len() / 2 + 16);
    let mut pos = 0usize;
    while pos < chunk.len() {
        let flag_at = out.len();
        out.push(0u8);
        for bit in 0..8 {
            if pos >= chunk.len() {
                break;
            }
            let bit_count = copy_token_bits(pos);
            let max_len = usize::from(0xFFFFu16 >> bit_count) + 3;
            let (mut best_len, mut best_off) = (0usize, 0usize);
            for cand in (pos.saturating_sub(WINDOW)..pos).rev() {
                let len = chunk[cand..]
                    .iter()
                    .zip(&chunk[pos..])
                    .take(max_len)
                    .take_while(|(a, b)| a == b)
                    .count();
                if len > best_len {
                    (best_len, best_off) = (len, pos - cand);
                }
            }
            if best_len >= 3 {
                // §2.4.1.3.19.3 Pack CopyToken.
                let token = ((best_off - 1) << (16 - bit_count)) | (best_len - 3);
                out[flag_at] |= 1 << bit;
                out.extend(u16::try_from(token).unwrap_or(0).to_le_bytes());
                pos += best_len;
            } else {
                out.push(chunk[pos]);
                pos += 1;
            }
        }
        if out.len() > 4096 {
            return None;
        }
    }
    Some(out)
}

/// Text of a project in its code page ([MS-OVBA] PROJECTCODEPAGE). Code pages
/// without a table in `bo-encoding` are read as Windows-1252.
fn decode_text(bytes: &[u8], code_page: u16) -> String {
    let charset = bo_encoding::web::Charset::from_code_page(code_page)
        .unwrap_or(bo_encoding::web::Charset::Windows1252);
    bo_encoding::web::decode(bytes, charset)
}

/// Parse a `vbaProject.bin` (or the `Macros` / `_VBA_PROJECT_CUR` storage of
/// a binary file): the modules of the `dir` stream ([MS-OVBA] §2.3.4.2) and the
/// source text of each module stream (§2.3.4.3). Nothing is compiled or executed.
pub fn read_vba_project(bytes: &[u8]) -> Result<VbaProject> {
    let mut cfb = Cfb::open(bytes)?;
    let root = PROJECT_ROOTS
        .into_iter()
        .find(|r| cfb.has(&format!("{r}/VBA/dir")))
        .ok_or_else(|| crate::BinaryError::Missing("VBA/dir".into()))?;
    let dir = decompress(&cfb.read(&format!("{root}/VBA/dir"))?)?;
    let mut project = parse_dir(&dir)?;
    // [MS-OVBA] §2.3.1 PROJECT stream: tells class, designer and document modules apart.
    let project_stream = format!("{root}/PROJECT");
    if cfb.has(&project_stream) {
        if let Ok(text) = cfb.read_capped(&project_stream, 4 << 20) {
            let text = decode_text(&text, project.code_page);
            apply_project_stream(&mut project, &text);
        }
    }
    let mut budget = MAX_SOURCE_TOTAL;
    for m in &mut project.modules {
        match module_source(&mut cfb, root, m, budget) {
            Some(raw) => {
                budget -= raw.len().min(budget);
                // An incompressible last chunk is padded with zero bytes (§2.4.1.3.10).
                let end = raw.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
                m.source = decode_text(&raw[..end], project.code_page);
            }
            None => m.unreadable = true,
        }
    }
    project.truncated |= budget == 0;
    Ok(project)
}

/// [MS-OVBA] §2.3.4.3 Module Stream: PerformanceCache (`text_offset` bytes), then the
/// CompressedSourceCode container. `None` when the stream is missing or damaged, or
/// the source exceeds `budget` bytes.
fn module_source(cfb: &mut Cfb<'_>, root: &str, m: &VbaModule, budget: usize) -> Option<Vec<u8>> {
    if m.stream_name.is_empty() || m.stream_name.contains(['/', '\\']) {
        return None;
    }
    let path = format!("{root}/VBA/{}", m.stream_name);
    if !cfb.has(&path) {
        return None;
    }
    let stream = cfb.read_capped(&path, MAX_DECOMPRESSED as u64).ok()?;
    let compressed = stream.get(m.text_offset as usize..)?;
    decompress_limited(compressed, budget).ok()
}

/// ProjectModule lines of the [MS-OVBA] §2.3.1 `PROJECT` stream:
/// `Document=Name/&H…`, `Module=Name`, `Class=Name`, `BaseClass=Name`. The list ends
/// at the first section header (`[Host Extender Info]`).
fn apply_project_stream(project: &mut VbaProject, text: &str) {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            break;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (kind, name) = match key.trim() {
            "Document" => (
                VbaModuleKind::Document,
                value.split_once('/').map_or(value, |(n, _)| n),
            ),
            "Module" => (VbaModuleKind::Standard, value),
            "Class" => (VbaModuleKind::Class, value),
            "BaseClass" => (VbaModuleKind::Form, value),
            _ => continue,
        };
        let name = name.trim().to_lowercase();
        if let Some(m) = project
            .modules
            .iter_mut()
            .find(|m| m.name.to_lowercase() == name)
        {
            m.kind = kind;
        }
    }
}

/// The VBA project of a file in any supported container: the `vbaProject.bin` part
/// of an OOXML package (`.docm`, `.dotm`, `.xlsm`, `.xltm`, `.xlam`, `.xlsb`, `.pptm`,
/// `.potm`, `.ppsm`; [MS-OFFMACRO2]), the project storage of a binary `.doc` / `.xls`,
/// or the VBA project storage record of a `.ppt`. `Ok(None)`: the file has no VBA
/// project (XLM macro sheets are not VBA).
pub fn read_vba_from_file(bytes: &[u8]) -> Result<Option<VbaProject>> {
    if is_compound_file(bytes) {
        let cfb = Cfb::open(bytes)?;
        if PROJECT_ROOTS
            .into_iter()
            .any(|r| cfb.has(&format!("{r}/VBA/dir")))
        {
            return read_vba_project(bytes).map(Some);
        }
        if cfb.has("/PowerPoint Document") {
            return match crate::ppt::vba_storage(bytes)? {
                Some(storage) => read_vba_project(&storage).map(Some),
                None => Ok(None),
            };
        }
        return Ok(None);
    }
    let Ok(mut zip) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return Ok(None);
    };
    let Some(index) = (0..zip.len()).find(|&i| {
        zip.name_for_index(i)
            .and_then(std::result::Result::ok)
            .is_some_and(|n| n.to_ascii_lowercase().ends_with("vbaproject.bin"))
    }) else {
        return Ok(None);
    };
    let entry = zip
        .by_index(index)
        .map_err(|e| malformed(format!("vbaProject.bin: {e}")))?;
    let mut bin = Vec::new();
    entry.take(MAX_PROJECT_BIN).read_to_end(&mut bin)?;
    read_vba_project(&bin).map(Some)
}

/// [MS-OVBA] §2.3.4.2 dir stream records.
fn parse_dir(dir: &[u8]) -> Result<VbaProject> {
    let mut project = VbaProject::default();
    let mut cur: Option<VbaModule> = None;
    let mut p = 0usize;
    // Text of the MBCS records is in the project code page; Windows-1252 until
    // PROJECTCODEPAGE is read (it precedes every name).
    let mut cp = 1252u16;
    project.code_page_supported = true;
    while p + 6 <= dir.len() {
        let id = u16_at(dir, p)?;
        let size = u32_at(dir, p + 2)? as usize;
        // §2.3.4.2.1.11 PROJECTVERSION: Reserved (4) is followed by 6 data bytes.
        let data_len = if id == 0x0009 { 6 } else { size };
        let data = (p + 6)
            .checked_add(data_len)
            .and_then(|end| dir.get(p + 6..end))
            .ok_or_else(|| malformed("VBA dir record truncated"))?;
        p += 6 + data_len;
        match id {
            // PROJECTCODEPAGE
            0x0003 => {
                cp = u16_at(data, 0)?;
                project.code_page = cp;
                project.code_page_supported =
                    bo_encoding::web::Charset::from_code_page(cp).is_some();
            }
            // PROJECTNAME
            0x0004 => project.name = decode_text(data, cp),
            // MODULENAME starts a module
            0x0019 => {
                if let Some(m) = cur.take() {
                    push_module(&mut project, m);
                }
                cur = Some(VbaModule {
                    name: decode_text(data, cp),
                    ..VbaModule::default()
                });
            }
            // MODULENAMEUNICODE
            0x0047 => {
                if let Some(m) = cur.as_mut() {
                    m.name = utf16(data);
                }
            }
            // MODULESTREAMNAME (MBCS); the Unicode variant follows as record 0x0032
            0x001A => {
                if let Some(m) = cur.as_mut() {
                    m.stream_name = decode_text(data, cp);
                }
            }
            0x0032 => {
                if let Some(m) = cur.as_mut() {
                    m.stream_name = utf16(data);
                }
            }
            // MODULEOFFSET: TextOffset of the compressed source in the module stream
            0x0031 => {
                if let Some(m) = cur.as_mut() {
                    m.text_offset = u32_at(data, 0)?;
                }
            }
            // MODULETYPE: 0x0021 procedural module
            0x0021 => {
                if let Some(m) = cur.as_mut() {
                    m.document_module = false;
                    m.kind = VbaModuleKind::Standard;
                }
            }
            // MODULETYPE: 0x0022 document, class or designer module
            0x0022 => {
                if let Some(m) = cur.as_mut() {
                    m.document_module = true;
                    m.kind = VbaModuleKind::Document;
                }
            }
            // module terminator
            0x002B => {
                if let Some(m) = cur.take() {
                    push_module(&mut project, m);
                }
            }
            // end of dir
            0x0010 => break,
            _ => {}
        }
    }
    if let Some(m) = cur.take() {
        push_module(&mut project, m);
    }
    Ok(project)
}

fn push_module(project: &mut VbaProject, m: VbaModule) {
    if project.modules.len() < MAX_MODULES {
        project.modules.push(m);
    } else {
        project.truncated = true;
    }
}

/// The macro-enabled content type for a main part content type.
pub fn macro_enabled_content_type(ct: &str) -> Option<&'static str> {
    Some(match ct {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml" => {
            "application/vnd.ms-word.document.macroEnabled.main+xml"
        }
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml" => {
            "application/vnd.ms-excel.sheet.macroEnabled.main+xml"
        }
        "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml" => {
            "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml"
        }
        _ => return None,
    })
}

/// Copy the VBA project of `src` into `dst` (both OPC packages whose main
/// part is reached through the package `officeDocument` relationship):
/// `vbaProject.bin`, the parts it references (e.g. `vbaData.xml`), the
/// relationship from the main part and the content types. The main part's
/// content type is switched to its macro-enabled variant. Returns whether
/// anything was copied.
pub fn carry_over(src: &Package, dst: &mut Package) -> Result<bool> {
    let Some(src_main) = src.related_part("/", bo_opc::REL_OFFICE_DOCUMENT)? else {
        return Ok(false);
    };
    let Some(dst_main) = dst.related_part("/", bo_opc::REL_OFFICE_DOCUMENT)? else {
        return Ok(false);
    };
    let src_rels = src.relationships(&src_main)?;
    let Some(vba_rel) = src_rels
        .iter()
        .find(|r| r.rel_type == REL_VBA_PROJECT && r.mode == TargetMode::Internal)
    else {
        return Ok(false);
    };
    let vba_src = resolve_target(&src_main, &vba_rel.target);
    let src_ct = src.content_types()?;
    let mut dst_ct = dst.content_types()?;

    // vbaProject.bin plus the parts it references (one level deep, per [MS-OFFMACRO2]).
    let mut to_copy = vec![vba_src.clone()];
    for r in src.relationships(&vba_src)? {
        if r.mode == TargetMode::Internal {
            to_copy.push(resolve_target(&vba_src, &r.target));
        }
    }
    let rels_name = rels_part_name(&vba_src);
    let main_dir = dst_main.rsplit_once('/').map_or("", |(d, _)| d);
    let file = vba_src
        .rsplit_once('/')
        .map_or(vba_src.as_str(), |(_, f)| f);
    let vba_dst = format!("{main_dir}/{file}");
    for name in &to_copy {
        let Some(part) = src.part(name) else { continue };
        // Keep the project next to the destination main part; referenced parts keep their names.
        let target_name = if name == &vba_src {
            vba_dst.clone()
        } else {
            name.clone()
        };
        dst.put_part(&target_name, part.data.clone());
        if let Some(ct) = src_ct.get(name) {
            dst_ct.set_override(&target_name, ct);
        }
    }
    if let Some(rels) = src.part(&rels_name) {
        dst.put_part(&rels_part_name(&vba_dst), rels.data.clone());
    }
    // Relationship main part → vbaProject.bin with a fresh id.
    let mut rels = dst.relationships(&dst_main)?;
    rels.retain(|r| r.rel_type != REL_VBA_PROJECT);
    let mut n = rels.len() + 1;
    while rels.iter().any(|r| r.id == format!("rId{n}")) {
        n += 1;
    }
    rels.push(Relationship {
        id: format!("rId{n}"),
        rel_type: REL_VBA_PROJECT.to_owned(),
        target: file.to_owned(),
        mode: TargetMode::Internal,
    });
    dst.set_relationships(&dst_main, &rels);
    if let Some(main_ct) = dst_ct.get(&dst_main).and_then(macro_enabled_content_type) {
        dst_ct.set_override(&dst_main, main_ct);
    }
    dst.set_content_types(&dst_ct);
    Ok(true)
}

/// The VBA project of a legacy binary file, rebuilt as a `vbaProject.bin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyProject {
    /// The compound file: the project storage's streams and storages at its root.
    pub bin: Vec<u8>,
    /// The project was signed: its signature streams were left out (a signature
    /// covers the project in its old container and no longer verifies).
    pub signature_dropped: bool,
}

/// Names of the VBA project signature streams kept in the project storage
/// ([MS-OSHARED] digital signature of a VBA project: `\x05DigitalSignature`,
/// `\x05DigitalSignatureEx`, `\x05DigitalSignatureExt`).
fn is_signature_stream(name: &str) -> bool {
    name.starts_with("\u{5}DigitalSignature")
}

/// Build a `vbaProject.bin` from the VBA project of a legacy binary file: the project
/// storage of a `.doc` ([MS-DOC] `Macros`) or `.xls` ([MS-XLS] `_VBA_PROJECT_CUR`), or
/// the project storage record of a `.ppt` ([MS-PPT] VBAInfoContainer). [MS-OVBA]: that
/// storage has the structure of the root of `vbaProject.bin` (`PROJECT`, `VBA/dir`,
/// module streams, designer storages…), so its streams and storages are copied as
/// they are into a new compound file. Signature streams are left out (see
/// [`LegacyProject::signature_dropped`]). `Ok(None)`: no VBA project.
pub fn legacy_project_bin(bytes: &[u8]) -> Result<Option<LegacyProject>> {
    if !is_compound_file(bytes) {
        return Ok(None);
    }
    let probe = Cfb::open(bytes)?;
    let (source, root) = if let Some(root) = ["/Macros", "/_VBA_PROJECT_CUR"]
        .into_iter()
        .find(|r| probe.has(&format!("{r}/VBA/dir")))
    {
        (bytes.to_vec(), root)
    } else if probe.has("/PowerPoint Document") {
        match crate::ppt::vba_storage(bytes)? {
            Some(storage) if is_compound_file(&storage) => (storage, ""),
            _ => return Ok(None),
        }
    } else {
        return Ok(None);
    };
    drop(probe);
    let mut src = cfb::CompoundFile::open(Cursor::new(source))?;
    let prefix = format!("{root}/");
    let entries: Vec<(String, bool)> = src
        .walk()
        .filter(|e| !e.is_root())
        .map(|e| (e.path().to_string_lossy().replace('\\', "/"), e.is_stream()))
        .filter(|(p, _)| root.is_empty() || p.starts_with(&prefix))
        .collect();
    let mut dst = cfb::CompoundFile::create(Cursor::new(Vec::new()))?;
    let mut signature_dropped = false;
    let mut total = 0u64;
    for (path, is_stream) in entries {
        let rel = &path[root.len()..];
        if rel.is_empty() || rel == "/" {
            continue;
        }
        let name = rel.rsplit('/').next().unwrap_or(rel);
        if is_stream && is_signature_stream(name) {
            signature_dropped = true;
            continue;
        }
        if is_stream {
            let mut data = Vec::new();
            src.open_stream(&path)?
                .take(MAX_PROJECT_BIN.saturating_sub(total))
                .read_to_end(&mut data)?;
            total += data.len() as u64;
            if total >= MAX_PROJECT_BIN {
                return Err(malformed("VBA project storage too large"));
            }
            let mut out = dst.create_stream(rel)?;
            std::io::Write::write_all(&mut out, &data)?;
        } else {
            dst.create_storage(rel)?;
        }
    }
    dst.flush()?;
    Ok(Some(LegacyProject {
        bin: dst.into_inner().into_inner(),
        signature_dropped,
    }))
}

/// Add `bin` as the VBA project of `dst` (an OPC package whose main part is reached
/// through the package `officeDocument` relationship, ECMA-376 Part 2 §9.3): the
/// `vbaProject.bin` part next to the main part with its content type override
/// (Part 2 §10.1.2.2.3), the `vbaProject` relationship from the main part
/// ([MS-OFFMACRO2]) and the macro-enabled content type of the main part. A project
/// already there is replaced. Returns whether the package has a main part.
pub fn attach_project(dst: &mut Package, bin: &[u8]) -> Result<bool> {
    let Some(main) = dst.related_part("/", bo_opc::REL_OFFICE_DOCUMENT)? else {
        return Ok(false);
    };
    let dir = main.rsplit_once('/').map_or("", |(d, _)| d);
    let part = format!("{dir}/vbaProject.bin");
    dst.put_part(&part, bin.to_vec());
    let mut types = dst.content_types()?;
    types.set_override(&part, CT_VBA_PROJECT);
    if let Some(enabled) = types.get(&main).and_then(macro_enabled_content_type) {
        types.set_override(&main, enabled);
    }
    dst.set_content_types(&types);
    let mut rels = dst.relationships(&main)?;
    rels.retain(|r| r.rel_type != REL_VBA_PROJECT);
    let mut n = rels.len() + 1;
    while rels.iter().any(|r| r.id == format!("rId{n}")) {
        n += 1;
    }
    rels.push(Relationship {
        id: format!("rId{n}"),
        rel_type: REL_VBA_PROJECT.to_owned(),
        target: "vbaProject.bin".to_owned(),
        mode: TargetMode::Internal,
    });
    dst.set_relationships(&main, &rels);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `host` with `project`'s entries copied under `root` (how the binary formats
    /// keep a VBA project), plus `extra` streams there.
    fn graft(host: &[u8], root: &str, project: &[u8], extra: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut src = cfb::CompoundFile::open(Cursor::new(project)).unwrap();
        let entries: Vec<(String, bool)> = src
            .walk()
            .filter(|e| !e.is_root())
            .map(|e| (e.path().to_string_lossy().replace('\\', "/"), e.is_stream()))
            .collect();
        let mut dst = cfb::CompoundFile::open(Cursor::new(host.to_vec())).unwrap();
        dst.create_storage(root).unwrap();
        for (path, is_stream) in entries {
            let target = format!("{root}{path}");
            if is_stream {
                let mut data = Vec::new();
                src.open_stream(&path)
                    .unwrap()
                    .read_to_end(&mut data)
                    .unwrap();
                dst.create_stream(&target)
                    .unwrap()
                    .write_all(&data)
                    .unwrap();
            } else {
                dst.create_storage(&target).unwrap();
            }
        }
        for (name, data) in extra {
            dst.create_stream(format!("{root}/{name}"))
                .unwrap()
                .write_all(data)
                .unwrap();
        }
        dst.flush().unwrap();
        dst.into_inner().into_inner()
    }

    #[test]
    fn legacy_project_storage_becomes_a_vba_project_bin() {
        let host = crate::test_util::cfb_file(&[("/WordDocument", vec![0; 16])], &[]);
        assert_eq!(legacy_project_bin(&host).unwrap(), None);
        assert_eq!(legacy_project_bin(b"PK\x03\x04").unwrap(), None);
        for (root, kind) in [("/Macros", "doc"), ("/_VBA_PROJECT_CUR", "sheet")] {
            let project = crate::vba_write::sample_project(kind).unwrap();
            let want = read_vba_project(&project).unwrap();
            let file = graft(&host, root, &project, &[]);
            let got = legacy_project_bin(&file).unwrap().unwrap();
            assert!(!got.signature_dropped);
            // The rebuilt project reads as the original, from its root.
            let cf = cfb::CompoundFile::open(Cursor::new(&got.bin)).unwrap();
            assert!(cf.is_stream("/VBA/dir") && cf.is_stream("/PROJECT"));
            assert!(!cf.exists("/WordDocument") && !cf.exists(root));
            assert_eq!(read_vba_project(&got.bin).unwrap(), want);
            // A signed project: the signature streams are left out and reported.
            let signed = graft(
                &host,
                root,
                &project,
                &[
                    ("\u{5}DigitalSignature", b"sig"),
                    ("\u{5}DigitalSignatureEx", b"sig"),
                ],
            );
            let got = legacy_project_bin(&signed).unwrap().unwrap();
            assert!(got.signature_dropped);
            let cf = cfb::CompoundFile::open(Cursor::new(&got.bin)).unwrap();
            assert!(cf.walk().all(|e| !e.name().starts_with('\u{5}')));
            assert_eq!(read_vba_project(&got.bin).unwrap(), want);
        }
    }

    #[test]
    fn attach_project_adds_part_relationship_and_types() {
        let mut dst = package(DOC_CT, false);
        assert!(attach_project(&mut dst, b"bin").unwrap());
        // Attaching again replaces the project (one relationship).
        assert!(attach_project(&mut dst, b"bin2").unwrap());
        let out = Package::read(&dst.write().unwrap(), &bo_opc::Limits::default()).unwrap();
        assert_eq!(out.part("/word/vbaProject.bin").unwrap().data, b"bin2");
        let ct = out.content_types().unwrap();
        assert_eq!(ct.get("/word/vbaProject.bin"), Some(CT_VBA_PROJECT));
        assert_eq!(
            ct.get("/word/document.xml"),
            Some("application/vnd.ms-word.document.macroEnabled.main+xml")
        );
        let rels = out.relationships("/word/document.xml").unwrap();
        assert_eq!(
            rels.iter()
                .filter(|r| r.rel_type == REL_VBA_PROJECT && r.target == "vbaProject.bin")
                .count(),
            1
        );
        assert!(has_macros(&out.write().unwrap()));
        assert!(!attach_project(&mut Package::new(), b"bin").unwrap());
    }

    /// Wrap raw bytes as uncompressed chunks ([MS-OVBA] §2.4.1.3.10 compressing raw chunks).
    fn raw_container(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0x01];
        for chunk in data.chunks(4096) {
            let mut c = chunk.to_vec();
            c.resize(4096, 0);
            out.extend((0x3000u16 | 4095).to_le_bytes());
            out.extend(c);
        }
        out
    }

    fn dir_rec(id: u16, data: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend((data.len() as u32).to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    fn sample_dir() -> Vec<u8> {
        let mut d = dir_rec(0x0001, &1u32.to_le_bytes()); // PROJECTSYSKIND
        d.extend(dir_rec(0x0004, b"VBAProject"));
        // PROJECTVERSION: Reserved 4 then 6 bytes
        d.extend(0x0009u16.to_le_bytes());
        d.extend(4u32.to_le_bytes());
        d.extend([1, 0, 0, 0, 2, 0]);
        d.extend(dir_rec(0x000F, &2u16.to_le_bytes()));
        for (name, doc) in [("ThisWorkbook", true), ("Module1", false)] {
            d.extend(dir_rec(0x0019, name.as_bytes()));
            let u: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
            d.extend(dir_rec(0x0047, &u));
            d.extend(dir_rec(0x001A, name.as_bytes()));
            d.extend(dir_rec(0x0032, &u));
            d.extend(dir_rec(if doc { 0x0022 } else { 0x0021 }, &[]));
            d.extend(dir_rec(0x002B, &[]));
        }
        d.extend(dir_rec(0x0010, &[]));
        d
    }

    #[test]
    fn decompress_spec_example() {
        // [MS-OVBA] §3.2.3 example: "#aaabcdefaaaaghijaaaaaklaaamnopqaaaaaaaaaaaarstuvwxyzaaa"
        let compressed: [u8; 49] = [
            0x01, 0x2F, 0xB0, 0x00, 0x23, 0x61, 0x61, 0x61, 0x62, 0x63, 0x64, 0x65, 0x82, 0x66,
            0x00, 0x70, 0x61, 0x67, 0x68, 0x69, 0x6A, 0x01, 0x38, 0x08, 0x61, 0x6B, 0x6C, 0x00,
            0x30, 0x6D, 0x6E, 0x6F, 0x70, 0x06, 0x71, 0x02, 0x70, 0x04, 0x10, 0x72, 0x73, 0x74,
            0x75, 0x76, 0x10, 0x77, 0x78, 0x79, 0x7A,
        ];
        let out = decompress(&compressed).unwrap();
        assert!(String::from_utf8_lossy(&out)
            .starts_with("#aaabcdefaaaaghijaaaaaklaaamnopqaaaaaaaaaaaarstuvwxyz"));
        // A container may hold fewer bytes than its last chunk header says.
        assert!(decompress_limited(&compressed, 8).is_err());
    }

    #[test]
    fn decompress_raw_chunks_and_errors() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        let out = decompress(&raw_container(&data)).unwrap();
        assert_eq!(&out[..5000], &data[..]);
        assert!(decompress(&[0x02]).is_err());
        // copy token with nothing decompressed yet
        assert!(decompress(&[0x01, 0x03, 0xB0, 0x01, 0x00, 0x00]).is_err());
    }

    #[test]
    fn reads_module_list() {
        let bin = crate::test_util::cfb_file(&[("/VBA/dir", raw_container(&sample_dir()))], &[]);
        let p = read_vba_project(&bin).unwrap();
        assert_eq!(p.name, "VBAProject");
        assert_eq!(p.modules.len(), 2);
        assert_eq!(p.modules[0].name, "ThisWorkbook");
        assert!(p.modules[0].document_module);
        assert_eq!(p.modules[1].stream_name, "Module1");
        assert!(!p.modules[1].document_module);
    }

    fn package(main_ct: &str, with_vba: bool) -> Package {
        let mut pkg = Package::new();
        let mut ct = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="{main_ct}"/>"#
        );
        if with_vba {
            ct.push_str(&format!(
                r#"<Override PartName="/word/vbaProject.bin" ContentType="{CT_VBA_PROJECT}"/><Override PartName="/word/vbaData.xml" ContentType="application/vnd.ms-word.vbaData+xml"/>"#
            ));
        }
        ct.push_str("</Types>");
        pkg.put_part("/[Content_Types].xml", ct.into_bytes());
        pkg.put_part("/_rels/.rels", br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_vec());
        pkg.put_part("/word/document.xml", b"<w:document/>".to_vec());
        if with_vba {
            pkg.put_part("/word/_rels/document.xml.rels", format!(r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId7" Type="{REL_VBA_PROJECT}" Target="vbaProject.bin"/></Relationships>"#).into_bytes());
            pkg.put_part(
                "/word/vbaProject.bin",
                vec![0xD0, 0xCF, 0x11, 0xE0, 1, 2, 3],
            );
            pkg.put_part("/word/_rels/vbaProject.bin.rels", br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.microsoft.com/office/2006/relationships/wordVbaData" Target="vbaData.xml"/></Relationships>"#.to_vec());
            pkg.put_part("/word/vbaData.xml", b"<wne:vbaSuppData/>".to_vec());
        }
        pkg
    }

    const DOC_CT: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

    #[test]
    fn detects_and_preserves_vba() {
        let src = package(
            "application/vnd.ms-word.document.macroEnabled.main+xml",
            true,
        );
        let bytes = src.write().unwrap();
        assert!(has_macros(&bytes));
        assert!(package_has_macros(&src));
        // Untouched round trip keeps the project byte for byte.
        let back = Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        assert_eq!(
            back.part("/word/vbaProject.bin").unwrap().data,
            src.part("/word/vbaProject.bin").unwrap().data
        );
        // A freshly written package gets the project carried over.
        let mut dst = package(DOC_CT, false);
        assert!(!has_macros(&dst.write().unwrap()));
        assert!(carry_over(&back, &mut dst).unwrap());
        let out = Package::read(&dst.write().unwrap(), &bo_opc::Limits::default()).unwrap();
        assert_eq!(
            out.part("/word/vbaProject.bin").unwrap().data,
            vec![0xD0, 0xCF, 0x11, 0xE0, 1, 2, 3]
        );
        assert!(out.part("/word/vbaData.xml").is_some());
        assert!(out.part("/word/_rels/vbaProject.bin.rels").is_some());
        let ct = out.content_types().unwrap();
        assert_eq!(ct.get("/word/vbaProject.bin"), Some(CT_VBA_PROJECT));
        assert_eq!(
            ct.get("/word/document.xml"),
            Some("application/vnd.ms-word.document.macroEnabled.main+xml")
        );
        let rels = out.relationships("/word/document.xml").unwrap();
        assert!(rels
            .iter()
            .any(|r| r.rel_type == REL_VBA_PROJECT && r.target == "vbaProject.bin"));
        // Nothing to carry from a macro-free package.
        let mut dst2 = package(DOC_CT, false);
        assert!(!carry_over(&package(DOC_CT, false), &mut dst2).unwrap());
    }

    #[test]
    fn binary_containers() {
        let doc = crate::test_util::cfb_file(
            &[("/WordDocument", vec![0; 16]), ("/Macros/VBA/dir", vec![1])],
            &[],
        );
        assert!(has_macros(&doc));
        let plain = crate::test_util::cfb_file(&[("/WordDocument", vec![0; 16])], &[]);
        assert!(!has_macros(&plain));
        assert!(!has_macros(b"not a container"));
    }

    /// The example of [MS-OVBA] §3.2.3 (normal compression).
    const SPEC_TEXT: &[u8] = b"#aaabcdefaaaaghijaaaaaklaaamnopqaaaaaaaaaaaarstuvwxyzaaa";
    const SPEC_COMPRESSED: [u8; 49] = [
        0x01, 0x2F, 0xB0, 0x00, 0x23, 0x61, 0x61, 0x61, 0x62, 0x63, 0x64, 0x65, 0x82, 0x66, 0x00,
        0x70, 0x61, 0x67, 0x68, 0x69, 0x6A, 0x01, 0x38, 0x08, 0x61, 0x6B, 0x6C, 0x00, 0x30, 0x6D,
        0x6E, 0x6F, 0x70, 0x06, 0x71, 0x02, 0x70, 0x04, 0x10, 0x72, 0x73, 0x74, 0x75, 0x76, 0x10,
        0x77, 0x78, 0x79, 0x7A,
    ];

    #[test]
    fn compress_matches_spec_examples() {
        // SPEC_COMPRESSED is the published container without its last CopyToken
        // (the final "aaa"): its chunk header announces 50 bytes, two more than given.
        let text = decompress(&SPEC_COMPRESSED).unwrap();
        assert_eq!(text, &SPEC_TEXT[..SPEC_TEXT.len() - 3]);
        // Our compressor finds the same tokens (same sizes, same chunk header); where
        // several matches are equally long it takes the nearest, the published bytes
        // another one, so the offsets of two CopyTokens differ.
        let packed = compress(SPEC_TEXT);
        assert_eq!(packed.len(), SPEC_COMPRESSED.len() + 2);
        assert_eq!(&packed[..3], &SPEC_COMPRESSED[..3]);
        let differing = packed
            .iter()
            .zip(&SPEC_COMPRESSED)
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(differing, 2);
        assert_eq!(decompress(&packed).unwrap(), SPEC_TEXT);
        // [MS-OVBA] §3.2.2 maximum compression: 73 × "a".
        let a73 = [b'a'; 73];
        let packed = [0x01, 0x03, 0xB0, 0x02, 0x61, 0x45, 0x00];
        assert_eq!(compress(&a73), packed);
        assert_eq!(decompress(&packed).unwrap(), a73);
        assert_eq!(compress(b""), [0x01]);
        assert_eq!(decompress(&[0x01]).unwrap(), b"");
    }

    #[test]
    fn compress_round_trips_chunks() {
        // Several chunks of repetitive text: copy tokens with every offset width.
        let text: Vec<u8> = (0..3000u32)
            .flat_map(|i| format!("Line {} of the module\r\n", i % 97).into_bytes())
            .collect();
        let packed = compress(&text);
        assert!(packed.len() < text.len() / 2);
        assert_eq!(decompress(&packed).unwrap(), text);
        // Incompressible data: raw chunks, the last one padded with zeros (§2.4.1.3.10).
        let mut x = 0x2545_F491u32;
        let noise: Vec<u8> = (0..4096 + 4000)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x >> 8) as u8
            })
            .collect();
        let out = decompress(&compress(&noise)).unwrap();
        assert_eq!(out.len(), 8192);
        assert_eq!(&out[..noise.len()], &noise[..]);
        assert!(out[noise.len()..].iter().all(|&b| b == 0));
        assert!(decompress_limited(&compress(&text), 100).is_err());
    }

    use crate::vba_write::{sample_project, write_vba_project, ModuleSpec};

    const VI_SOURCE: &str =
        "Option Explicit\n' Tính tổng doanh thu — đơn vị: ₫\nSub TinhTong()\n    Range(\"B10\").Value = \"Tổng cộng\"\nEnd Sub\n";

    fn four_kinds(code_page: u16) -> Vec<u8> {
        write_vba_project(
            "DuAn",
            code_page,
            &[
                ModuleSpec {
                    name: "ThisWorkbook",
                    kind: VbaModuleKind::Document,
                    source: "",
                },
                ModuleSpec {
                    name: "TinhToán",
                    kind: VbaModuleKind::Standard,
                    source: VI_SOURCE,
                },
                ModuleSpec {
                    name: "KhachHang",
                    kind: VbaModuleKind::Class,
                    source: "Public Ten As String\n",
                },
                ModuleSpec {
                    name: "Form1",
                    kind: VbaModuleKind::Form,
                    source: "Private Sub OK_Click()\nEnd Sub\n",
                },
            ],
        )
        .unwrap()
    }

    fn stored(name: &str, source: &str) -> String {
        format!(
            "Attribute VB_Name = \"{name}\"\r\n{}",
            source.replace('\n', "\r\n")
        )
    }

    #[test]
    fn reads_written_project_with_sources() {
        for cp in [1258u16, 65001] {
            let p = read_vba_project(&four_kinds(cp)).unwrap();
            assert_eq!(p.name, "DuAn");
            assert_eq!(
                (p.code_page, p.code_page_supported, p.truncated),
                (cp, true, false)
            );
            let kinds: Vec<_> = p
                .modules
                .iter()
                .map(|m| (m.name.as_str(), m.kind))
                .collect();
            assert_eq!(
                kinds,
                [
                    ("ThisWorkbook", VbaModuleKind::Document),
                    ("TinhToán", VbaModuleKind::Standard),
                    ("KhachHang", VbaModuleKind::Class),
                    ("Form1", VbaModuleKind::Form),
                ]
            );
            assert!(p.modules.iter().all(|m| !m.unreadable));
            assert!(!p.modules[1].document_module && p.modules[2].document_module);
            assert_eq!(p.modules[1].source, stored("TinhToán", VI_SOURCE));
            assert_eq!(p.modules[0].source, stored("ThisWorkbook", ""));
        }
        // Windows-1252 cannot hold the Vietnamese letters: they are written as `?`.
        let p = read_vba_project(&four_kinds(1252)).unwrap();
        assert!(p.modules[1]
            .source
            .contains("' Tính t?ng doanh thu — ??n v?: ?"));
        assert!(write_vba_project("P", 932, &[]).is_err());
    }

    #[test]
    fn writer_is_deterministic() {
        let a = four_kinds(1258);
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(a, four_kinds(1258));
    }

    #[test]
    fn reads_long_module_over_several_chunks() {
        let mut body = String::new();
        for i in 0..600 {
            body.push_str(&format!("    Cells({i}, 1).Value = \"Dòng {i}\"\n"));
        }
        let source = format!("Sub Fill()\n{body}End Sub\n");
        assert!(source.len() > 3 * 4096);
        let bin = write_vba_project(
            "P",
            1258,
            &[ModuleSpec {
                name: "Module1",
                kind: VbaModuleKind::Standard,
                source: &source,
            }],
        )
        .unwrap();
        let p = read_vba_project(&bin).unwrap();
        assert_eq!(p.modules[0].source, stored("Module1", &source));
    }

    /// Every stream of a compound file, with its path.
    fn streams(bin: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut cf = cfb::CompoundFile::open(Cursor::new(bin)).unwrap();
        let paths: Vec<String> = cf
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|e| e.path().to_string_lossy().replace('\\', "/"))
            .collect();
        paths
            .into_iter()
            .map(|p| {
                let mut data = Vec::new();
                cf.open_stream(&p).unwrap().read_to_end(&mut data).unwrap();
                (p, data)
            })
            .collect()
    }

    /// The project re-rooted under `root`, next to `extra` streams (a binary document).
    fn nested(root: &str, extra: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let moved: Vec<(String, Vec<u8>)> = streams(&sample_project("sheet").unwrap())
            .into_iter()
            .map(|(p, d)| (format!("{root}{p}"), d))
            .collect();
        let mut all: Vec<(&str, Vec<u8>)> = extra.to_vec();
        all.extend(moved.iter().map(|(p, d)| (p.as_str(), d.clone())));
        crate::test_util::cfb_file(&all, &[])
    }

    fn zip_with(name: &str, data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        zw.start_file("[Content_Types].xml", opts).unwrap();
        zw.write_all(b"<Types/>").unwrap();
        zw.start_file(name, opts).unwrap();
        zw.write_all(data).unwrap();
        zw.finish().unwrap().into_inner()
    }

    #[test]
    fn reads_project_from_every_container() {
        let bin = sample_project("sheet").unwrap();
        let direct = read_vba_project(&bin).unwrap();
        assert_eq!(direct.modules.len(), 2);
        assert_eq!(direct.modules[0].kind, VbaModuleKind::Document);
        assert!(direct.modules[1].source.contains("Sub ChaoMung()"));
        assert!(direct.modules[1]
            .source
            .contains("' Ghi lời chào — mã này chỉ được hiển thị"));
        // OOXML packages: the part is found by name, wherever the main part is.
        for part in [
            "xl/vbaProject.bin",
            "word/vbaProject.bin",
            "ppt/vbaProject.bin",
        ] {
            let got = read_vba_from_file(&zip_with(part, &bin)).unwrap();
            assert_eq!(got.as_ref(), Some(&direct), "{part}");
        }
        assert_eq!(
            read_vba_from_file(&zip_with("xl/other.bin", &bin)).unwrap(),
            None
        );
        // Binary .doc ([MS-DOC] Macros storage) and .xls ([MS-XLS] _VBA_PROJECT_CUR).
        let doc = nested("/Macros", &[("/WordDocument", vec![0; 16])]);
        assert_eq!(read_vba_from_file(&doc).unwrap().as_ref(), Some(&direct));
        let xls = nested("/_VBA_PROJECT_CUR", &[("/Workbook", vec![0; 16])]);
        assert_eq!(read_vba_from_file(&xls).unwrap().as_ref(), Some(&direct));
        let plain = crate::test_util::cfb_file(&[("/WordDocument", vec![0; 16])], &[]);
        assert_eq!(read_vba_from_file(&plain).unwrap(), None);
        assert_eq!(read_vba_from_file(b"plain text").unwrap(), None);
    }

    /// Keeps, replaces or drops (`None`) a stream of a project.
    type Edit<'a> = &'a dyn Fn(&str, Vec<u8>) -> Option<Vec<u8>>;

    #[test]
    fn damaged_projects_degrade() {
        let all = streams(&four_kinds(1258));
        let rebuild = |edit: Edit<'_>| {
            let kept: Vec<(String, Vec<u8>)> = all
                .iter()
                .filter_map(|(p, d)| edit(p, d.clone()).map(|d| (p.clone(), d)))
                .collect();
            let refs: Vec<(&str, Vec<u8>)> =
                kept.iter().map(|(p, d)| (p.as_str(), d.clone())).collect();
            crate::test_util::cfb_file(&refs, &[])
        };
        // A missing module stream and a damaged one: the other modules are still read.
        let bin = rebuild(&|p, d| match p {
            "/VBA/KhachHang" => None,
            "/VBA/Form1" => Some(vec![0x01, 0xFF, 0xFF, 0x00]),
            _ => Some(d),
        });
        let p = read_vba_project(&bin).unwrap();
        let state: Vec<_> = p.modules.iter().map(|m| m.unreadable).collect();
        assert_eq!(state, [false, false, true, true]);
        assert_eq!(p.modules[1].source, stored("TinhToán", VI_SOURCE));
        assert_eq!(p.modules[2].source, "");
        // Without the PROJECT stream the kind comes from MODULETYPE alone.
        let p = read_vba_project(&rebuild(&|p, d| (p != "/PROJECT").then_some(d))).unwrap();
        let kinds: Vec<_> = p.modules.iter().map(|m| m.kind).collect();
        assert_eq!(
            kinds,
            [
                VbaModuleKind::Document,
                VbaModuleKind::Standard,
                VbaModuleKind::Document,
                VbaModuleKind::Document
            ]
        );
        // No dir stream: an error, not a panic.
        assert!(read_vba_project(&rebuild(&|p, d| (p != "/VBA/dir").then_some(d))).is_err());
        // A code page without a table is reported.
        let mut d = dir_rec(0x0003, &932u16.to_le_bytes());
        d.extend(sample_dir());
        let p = parse_dir(&d).unwrap();
        assert_eq!((p.code_page, p.code_page_supported), (932, false));
        // A text offset past the end of the stream.
        let mut d = dir_rec(0x0019, b"M");
        d.extend(dir_rec(0x001A, b"M"));
        d.extend(dir_rec(0x0031, &u32::MAX.to_le_bytes()));
        let bin = crate::test_util::cfb_file(
            &[("/VBA/dir", compress(&d)), ("/VBA/M", compress(b"x"))],
            &[],
        );
        assert!(read_vba_project(&bin).unwrap().modules[0].unreadable);
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]
        #[test]
        fn compress_then_decompress_is_identity(
            noise in proptest::collection::vec(proptest::num::u8::ANY, 0..6000),
            alphabet in 1u8..6,
        ) {
            // Low-entropy text (copy tokens) and noise (literal and raw chunks).
            let text: Vec<u8> = noise.iter().map(|b| b'a' + b % alphabet).collect();
            proptest::prop_assert_eq!(&decompress(&compress(&text)).unwrap(), &text);
            let out = decompress(&compress(&noise)).unwrap();
            proptest::prop_assert!(out.len() >= noise.len() && out.len() < noise.len() + 4096);
            proptest::prop_assert_eq!(&out[..noise.len()], &noise[..]);
            // Only a raw last chunk is padded, and only with zeros.
            proptest::prop_assert!(out[noise.len()..].iter().all(|&b| b == 0));
        }

        #[test]
        fn mutated_project_never_panics(
            edits in proptest::collection::vec((0usize..1 << 16, proptest::num::u8::ANY), 1..8),
            cut in 0usize..1 << 16,
        ) {
            let mut bin = four_kinds(1258);
            for (pos, val) in &edits {
                let i = pos % bin.len();
                bin[i] = *val;
            }
            let _ = read_vba_project(&bin);
            let _ = read_vba_from_file(&bin);
            bin.truncate(cut % (bin.len() + 1));
            let _ = read_vba_from_file(&bin);
            // Damage inside the decompressed dir stream and a module stream.
            let all = streams(&four_kinds(1258));
            let mutated: Vec<(String, Vec<u8>)> = all
                .iter()
                .map(|(p, d)| {
                    let mut raw = decompress(d).unwrap_or_else(|_| d.clone());
                    if p.starts_with("/VBA/") && !raw.is_empty() {
                        for (pos, val) in &edits {
                            let i = pos % raw.len();
                            raw[i] = *val;
                        }
                    }
                    (p.clone(), if p == "/VBA/dir" { compress(&raw) } else { d.clone() })
                })
                .collect();
            let refs: Vec<(&str, Vec<u8>)> =
                mutated.iter().map(|(p, d)| (p.as_str(), d.clone())).collect();
            let _ = read_vba_project(&crate::test_util::cfb_file(&refs, &[]));
        }
    }

    proptest::proptest! {
        #[test]
        fn random_never_panics(data in proptest::collection::vec(proptest::num::u8::ANY, 0..2048)) {
            let _ = decompress(&data);
            let mut d = vec![1u8];
            d.extend(&data);
            let _ = decompress(&d);
            let _ = parse_dir(&data);
            let _ = has_macros(&data);
        }
    }
}
