//! VBA macro projects: detection, inspection and preservation — never execution.
//!
//! * [`has_macros`] — macro presence in OOXML (`vbaProject.bin` part) and in
//!   the binary formats (`Macros` / `_VBA_PROJECT_CUR` storages, XLM macro
//!   sheets, PowerPoint `VBAInfoContainer`).
//! * [`read_vba_project`] — module list from the compressed `VBA/dir` stream
//!   ([MS-OVBA] §2.3.4.2), using the decompression of [MS-OVBA] §2.4.1.
//! * [`carry_over`] — copy the VBA project parts of a source package into a
//!   package produced by a writer, so macros survive open → save (prompt §6
//!   Phase 5: "giữ nguyên khi lưu, cảnh báo người dùng, KHÔNG thực thi").

use crate::bytes::{latin1, u16_at, u32_at, utf16};
use crate::{is_compound_file, malformed, Cfb, Result};
use bo_opc::{rels_part_name, resolve_target, Package, Relationship, TargetMode};
use std::io::Cursor;

/// Relationship type from the main part to the VBA project ([MS-OFFMACRO2] / [MS-OI29500]).
pub const REL_VBA_PROJECT: &str =
    "http://schemas.microsoft.com/office/2006/relationships/vbaProject";
/// Content type of `vbaProject.bin`.
pub const CT_VBA_PROJECT: &str = "application/vnd.ms-office.vbaProject";
/// Maximum decompressed size of one VBA stream (64 MiB).
const MAX_DECOMPRESSED: usize = 64 << 20;

/// A module of a VBA project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbaModule {
    pub name: String,
    /// Stream name inside the `VBA` storage.
    pub stream_name: String,
    /// Document module (sheet / ThisWorkbook / ThisDocument) rather than a standard module.
    pub document_module: bool,
}

/// What we know about a VBA project (for the macro warning UI).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VbaProject {
    pub name: String,
    pub modules: Vec<VbaModule>,
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
    if data.first() != Some(&0x01) {
        return Err(malformed("VBA: bad compressed container signature"));
    }
    let mut out: Vec<u8> = Vec::with_capacity(data.len() * 2);
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
                    if out.len() > MAX_DECOMPRESSED {
                        return Err(malformed("VBA: decompressed stream too large"));
                    }
                }
            }
        }
        if out.len() > MAX_DECOMPRESSED {
            return Err(malformed("VBA: decompressed stream too large"));
        }
        p = end;
    }
    Ok(out)
}

/// Parse a `vbaProject.bin` (or the `Macros` / `_VBA_PROJECT_CUR` storage of
/// a binary file) and list its modules. Nothing is compiled or executed.
pub fn read_vba_project(bytes: &[u8]) -> Result<VbaProject> {
    let mut cfb = Cfb::open(bytes)?;
    let dir_name = ["/VBA/dir", "/Macros/VBA/dir", "/_VBA_PROJECT_CUR/VBA/dir"]
        .into_iter()
        .find(|n| cfb.has(n))
        .ok_or_else(|| crate::BinaryError::Missing("VBA/dir".into()))?;
    let dir = decompress(&cfb.read(dir_name)?)?;
    parse_dir(&dir)
}

/// [MS-OVBA] §2.3.4.2 dir stream records.
fn parse_dir(dir: &[u8]) -> Result<VbaProject> {
    let mut project = VbaProject::default();
    let mut cur: Option<VbaModule> = None;
    let mut p = 0usize;
    while p + 6 <= dir.len() {
        let id = u16_at(dir, p)?;
        let size = u32_at(dir, p + 2)? as usize;
        // §2.3.4.2.1.11 PROJECTVERSION: Reserved (4) is followed by 6 data bytes.
        let data_len = if id == 0x0009 { 6 } else { size };
        let data = dir
            .get(p + 6..p + 6 + data_len)
            .ok_or_else(|| malformed("VBA dir record truncated"))?;
        p += 6 + data_len;
        match id {
            // PROJECTNAME
            0x0004 => project.name = latin1(data),
            // MODULENAME starts a module
            0x0019 => {
                if let Some(m) = cur.take() {
                    project.modules.push(m);
                }
                cur = Some(VbaModule {
                    name: latin1(data),
                    stream_name: String::new(),
                    document_module: false,
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
                    m.stream_name = latin1(data);
                }
            }
            0x0032 => {
                if let Some(m) = cur.as_mut() {
                    m.stream_name = utf16(data);
                }
            }
            // MODULETYPE document
            0x0022 => {
                if let Some(m) = cur.as_mut() {
                    m.document_module = true;
                }
            }
            // module terminator
            0x002B => {
                if let Some(m) = cur.take() {
                    project.modules.push(m);
                }
            }
            // end of dir
            0x0010 => break,
            _ => {}
        }
    }
    if let Some(m) = cur.take() {
        project.modules.push(m);
    }
    Ok(project)
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

#[cfg(test)]
mod tests {
    use super::*;

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
