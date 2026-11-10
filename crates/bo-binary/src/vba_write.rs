//! A small [MS-OVBA] project writer for generated test and corpus files
//! (CORE-VBA-002).
//!
//! It builds a `vbaProject.bin` compound file from source text only: the `PROJECT`
//! stream ([MS-OVBA] §2.3.1), `VBA/_VBA_PROJECT` (§2.3.4.1) with the version
//! 0xFFFF that tells a reader the performance caches are absent, the compressed
//! `VBA/dir` stream (§2.3.4.2) and one module stream per module (§2.3.4.3) holding
//! nothing but the compressed source (`TextOffset` 0). There is no compiled code,
//! no references and no protection data, so this is what BlueOffice's reader and
//! other readers of the source need — it was not checked against the application
//! the format comes from. BlueOffice never writes or changes the VBA project of a
//! user's file: an existing project is carried over byte for byte.

use crate::vba::{compress, VbaModuleKind};
use crate::{malformed, Result};
use bo_encoding::web::{encode, Charset};
use std::io::{Cursor, Write};

/// A module to write.
#[derive(Debug, Clone, Copy)]
pub struct ModuleSpec<'a> {
    /// Module name (also its stream name).
    pub name: &'a str,
    /// Kind of the module.
    pub kind: VbaModuleKind,
    /// Source text; line ends are written as CR LF. The `Attribute VB_Name` line is added.
    pub source: &'a str,
}

/// One `dir` record: Id, Size, data ([MS-OVBA] §2.3.4.2).
fn rec(out: &mut Vec<u8>, id: u16, data: &[u8]) {
    out.extend(id.to_le_bytes());
    out.extend(u32::try_from(data.len()).unwrap_or(0).to_le_bytes());
    out.extend_from_slice(data);
}

fn utf16le(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// The uncompressed `dir` stream of a project.
fn dir_stream(name: &str, code_page: u16, charset: Charset, modules: &[ModuleSpec<'_>]) -> Vec<u8> {
    let mut d = Vec::new();
    // §2.3.4.2.1 PROJECTINFORMATION
    rec(&mut d, 0x0001, &1u32.to_le_bytes()); // PROJECTSYSKIND: 32-bit Windows
    rec(&mut d, 0x0002, &0x0409u32.to_le_bytes()); // PROJECTLCID
    rec(&mut d, 0x0014, &0x0409u32.to_le_bytes()); // PROJECTLCIDINVOKE
    rec(&mut d, 0x0003, &code_page.to_le_bytes()); // PROJECTCODEPAGE
    rec(&mut d, 0x0004, &encode(name, charset)); // PROJECTNAME
    rec(&mut d, 0x0005, &[]); // PROJECTDOCSTRING
    rec(&mut d, 0x0040, &[]); //   … Unicode
    rec(&mut d, 0x0006, &[]); // PROJECTHELPFILEPATH
    rec(&mut d, 0x003D, &[]); //   … second path
    rec(&mut d, 0x0007, &0u32.to_le_bytes()); // PROJECTHELPCONTEXT
    rec(&mut d, 0x0008, &0u32.to_le_bytes()); // PROJECTLIBFLAGS
                                              // PROJECTVERSION: Reserved (4), VersionMajor (u32), VersionMinor (u16).
    d.extend(0x0009u16.to_le_bytes());
    d.extend(4u32.to_le_bytes());
    d.extend(1u32.to_le_bytes());
    d.extend(0u16.to_le_bytes());
    rec(&mut d, 0x000C, &[]); // PROJECTCONSTANTS
    rec(&mut d, 0x003C, &[]); //   … Unicode
                              // §2.3.4.2.3 PROJECTMODULES (no PROJECTREFERENCES)
    rec(
        &mut d,
        0x000F,
        &u16::try_from(modules.len())
            .unwrap_or(u16::MAX)
            .to_le_bytes(),
    );
    rec(&mut d, 0x0013, &0xFFFFu16.to_le_bytes()); // PROJECTCOOKIE
    for m in modules {
        // §2.3.4.2.3.2 MODULE
        rec(&mut d, 0x0019, &encode(m.name, charset)); // MODULENAME
        rec(&mut d, 0x0047, &utf16le(m.name)); // MODULENAMEUNICODE
        rec(&mut d, 0x001A, &encode(m.name, charset)); // MODULESTREAMNAME
        rec(&mut d, 0x0032, &utf16le(m.name)); //   … Unicode
        rec(&mut d, 0x001C, &[]); // MODULEDOCSTRING
        rec(&mut d, 0x0048, &[]); //   … Unicode
        rec(&mut d, 0x0031, &0u32.to_le_bytes()); // MODULEOFFSET: source at offset 0
        rec(&mut d, 0x001E, &0u32.to_le_bytes()); // MODULEHELPCONTEXT
        rec(&mut d, 0x002C, &0xFFFFu16.to_le_bytes()); // MODULECOOKIE
        let ty = if m.kind == VbaModuleKind::Standard {
            0x0021
        } else {
            0x0022
        };
        rec(&mut d, ty, &[]); // MODULETYPE
        rec(&mut d, 0x002B, &[]); // Terminator
    }
    rec(&mut d, 0x0010, &[]); // dir Terminator
    d
}

/// The text of the `PROJECT` stream ([MS-OVBA] §2.3.1): the module list and the name.
fn project_stream(name: &str, modules: &[ModuleSpec<'_>]) -> String {
    let mut s = String::from("ID=\"{00000000-0000-0000-0000-000000000000}\"\r\n");
    for m in modules {
        match m.kind {
            VbaModuleKind::Document => {
                s.push_str(&format!("Document={}/&H00000000\r\n", m.name));
            }
            VbaModuleKind::Standard => s.push_str(&format!("Module={}\r\n", m.name)),
            VbaModuleKind::Class => s.push_str(&format!("Class={}\r\n", m.name)),
            VbaModuleKind::Form => s.push_str(&format!("BaseClass={}\r\n", m.name)),
        }
    }
    s.push_str(&format!(
        "Name=\"{name}\"\r\nHelpContextID=\"0\"\r\nVersionCompatible32=\"393222000\"\r\n"
    ));
    s
}

/// The source of a module as stored: the `Attribute VB_Name` line, then the text
/// with CR LF line ends.
fn stored_source(m: &ModuleSpec<'_>) -> String {
    let mut s = format!("Attribute VB_Name = \"{}\"\r\n", m.name);
    for line in m.source.lines() {
        s.push_str(line);
        s.push_str("\r\n");
    }
    s
}

/// Build a `vbaProject.bin` holding `modules`. `code_page` must be one `bo-encoding`
/// can write (1252, 1258, 65001); characters outside it become `?`. Module names
/// must be unique compound-file stream names (no `/`).
pub fn write_vba_project(
    name: &str,
    code_page: u16,
    modules: &[ModuleSpec<'_>],
) -> Result<Vec<u8>> {
    let charset = Charset::from_code_page(code_page)
        .filter(|c| !matches!(c, Charset::Utf16Le | Charset::Utf16Be))
        .ok_or_else(|| crate::BinaryError::Unsupported(format!("code page {code_page}")))?;
    if modules
        .iter()
        .any(|m| m.name.is_empty() || m.name.contains(['/', '\\', ':', '!']))
    {
        return Err(malformed("VBA module name is not a stream name"));
    }
    let mut cf = cfb::CompoundFile::create(Cursor::new(Vec::new()))?;
    cf.create_storage("/VBA")?;
    let mut put = |path: &str, data: &[u8]| -> Result<()> {
        let mut st = cf.create_stream(path)?;
        st.write_all(data)?;
        Ok(())
    };
    put("/PROJECT", &encode(&project_stream(name, modules), charset))?;
    // §2.3.4.1 _VBA_PROJECT: Reserved1 0x61CC, Version 0xFFFF, Reserved2 0x00, Reserved3.
    put(
        "/VBA/_VBA_PROJECT",
        &[0xCC, 0x61, 0xFF, 0xFF, 0x00, 0x00, 0x00],
    )?;
    put(
        "/VBA/dir",
        &compress(&dir_stream(name, code_page, charset, modules)),
    )?;
    for m in modules {
        put(
            &format!("/VBA/{}", m.name),
            &compress(&encode(&stored_source(m), charset)),
        )?;
    }
    // Storage timestamps are fixed so that the same input gives the same bytes.
    for storage in ["/", "/VBA"] {
        cf.set_created_time(storage, std::time::UNIX_EPOCH)?;
        cf.set_modified_time(storage, std::time::UNIX_EPOCH)?;
    }
    cf.flush()?;
    Ok(cf.into_inner().into_inner())
}

/// The project of the generated corpus files (`macro-sample.docm` / `.xlsm` /
/// `.pptm`): a document module and a standard module with Vietnamese comments, in
/// Windows-1258. `host` is `doc`, `sheet` or `slide`.
pub fn sample_project(host: &str) -> Result<Vec<u8>> {
    let (doc_name, body) = match host {
        "sheet" => (
            "ThisWorkbook",
            "    Range(\"A1\").Value = \"Xin chào\"\n    Range(\"A2\").Value = 42\n",
        ),
        "slide" => ("Slide1", "    MsgBox \"Xin chào\"\n"),
        _ => ("ThisDocument", "    Selection.TypeText \"Xin chào\"\n"),
    };
    let module = format!(
        "Option Explicit\n\n' Ghi lời chào — mã này chỉ được hiển thị, không bao giờ chạy.\nSub ChaoMung()\n{body}End Sub\n"
    );
    write_vba_project(
        "VBAProject",
        1258,
        &[
            ModuleSpec {
                name: doc_name,
                kind: VbaModuleKind::Document,
                source: "Option Explicit\n",
            },
            ModuleSpec {
                name: "Module1",
                kind: VbaModuleKind::Standard,
                source: &module,
            },
        ],
    )
}
