//! CORE-VBA-002: the VBA macro source of an opened file is shown read-only through
//! the `vbaProject` command of the three JSON bridges; the project is never run and
//! is kept byte for byte when the file is saved.
//!
//! The macro-enabled files are generated (`bo-cli sample-macros tests/corpus/macro`,
//! project written by `bo_binary::vba_write` from source text).
#![allow(clippy::unwrap_used)] // test helpers

use bo_engine::{Engine, SheetEngine};
use bo_opc::{Limits, Package};
use serde_json::{json, Value};
use std::io::{Cursor, Read, Write};

fn corpus(rel: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn part(bytes: &[u8], name: &str) -> Vec<u8> {
    Package::read(bytes, &Limits::default())
        .unwrap()
        .part(name)
        .unwrap_or_else(|| panic!("no part {name}"))
        .data
        .clone()
}

fn parse(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

const VBA_CMD: &str = r#"{"cmd":"vbaProject"}"#;

/// Checks the answer for a generated corpus file.
fn assert_sample(v: &Value, doc_module: &str, code: &str) {
    assert_eq!(v["name"], "VBAProject");
    assert_eq!(v["codePage"], 1258);
    assert_eq!(v["codePageSupported"], true);
    assert!(v.get("error").is_none() && v.get("truncated").is_none());
    let modules = v["modules"].as_array().unwrap();
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0]["name"], doc_module);
    assert_eq!(modules[0]["kind"], "document");
    assert_eq!(modules[0]["source"], "Option Explicit\n");
    assert_eq!(modules[1]["name"], "Module1");
    assert_eq!(modules[1]["kind"], "standard");
    let src = modules[1]["source"].as_str().unwrap();
    assert!(
        src.starts_with(
            "Option Explicit\n\n' Ghi lời chào — mã này chỉ được hiển thị, không bao giờ chạy.\nSub ChaoMung()\n"
        ),
        "{src}"
    );
    assert!(src.contains(code), "{src}");
    assert!(src.ends_with("End Sub\n") && !src.contains('\r') && !src.contains("Attribute VB_"));
}

#[test]
fn document_shows_macro_source_and_keeps_the_project() {
    let docm = corpus("macro/macro-vba.docm");
    let vba = part(&docm, "/word/vbaProject.bin");
    let mut e = Engine::with_bundled_fonts();
    assert!(e.command_json(VBA_CMD).is_err(), "no document open");
    let info = e.open(&docm, "macro-vba.docm").unwrap();
    assert!(info.macros);
    let v = parse(&e.command_json(VBA_CMD).unwrap());
    assert_sample(&v, "ThisDocument", "Selection.TypeText \"Xin chào\"");
    // Reading the source does not touch the document.
    let state = parse(&e.command_json(r#"{"cmd":"state"}"#).unwrap());
    assert_eq!(state["modified"], false);

    // Edit, save: the project is carried over byte for byte and is still shown.
    e.command_json(r#"{"cmd":"insertText","text":"Thêm chữ. "}"#)
        .unwrap();
    let saved = e.save_docx().unwrap();
    assert_eq!(part(&saved, "/word/vbaProject.bin"), vba);
    let mut again = Engine::with_bundled_fonts();
    assert!(again.open(&saved, "saved.docm").unwrap().macros);
    assert_eq!(parse(&again.command_json(VBA_CMD).unwrap()), v);

    // Read-only documents still show the source (the command changes nothing).
    e.command_json(r#"{"cmd":"setReadOnly","on":true}"#)
        .unwrap();
    assert_eq!(parse(&e.command_json(VBA_CMD).unwrap()), v);

    // A new document has no project.
    e.new_document("moi.docx").unwrap();
    let none = parse(&e.command_json(VBA_CMD).unwrap());
    assert_eq!(none["modules"], json!([]));
    assert_eq!(none["name"], "");
}

#[test]
fn workbook_shows_macro_source_and_keeps_the_project() {
    let xlsm = corpus("macro/macro-vba.xlsm");
    let vba = part(&xlsm, "/xl/vbaProject.bin");
    let mut e = SheetEngine::new();
    assert!(e.exec_json(VBA_CMD).is_err(), "no workbook open");
    assert!(e.open(&xlsm, "macro-vba.xlsm").unwrap().macros);
    let v = parse(&e.exec_json(VBA_CMD).unwrap());
    assert_sample(&v, "ThisWorkbook", "Range(\"A1\").Value = \"Xin chào\"");
    // The macro was not run: A1 still holds the text of the file.
    assert_eq!(e.cell(0, 0, 0).unwrap().input, "Tệp mẫu có macro VBA");
    assert!(!e.can_undo());

    e.set_input(0, 4, 0, "7").unwrap();
    let saved = e.save("xlsx", 0).unwrap();
    assert_eq!(part(&saved, "/xl/vbaProject.bin"), vba);
    let mut again = SheetEngine::new();
    assert!(again.open(&saved, "saved.xlsm").unwrap().macros);
    assert_eq!(parse(&again.exec_json(VBA_CMD).unwrap()), v);

    let plain = corpus("doc/merge-data.xlsx");
    let mut e = SheetEngine::new();
    assert!(!e.open(&plain, "merge-data.xlsx").unwrap().macros);
    assert_eq!(parse(&e.exec_json(VBA_CMD).unwrap())["modules"], json!([]));
}

#[test]
fn presentation_shows_macro_source_and_keeps_the_project() {
    let pptm = corpus("macro/macro-vba.pptm");
    let vba = part(&pptm, "/ppt/vbaProject.bin");
    let mut e = Engine::with_bundled_fonts();
    assert!(e.open(&pptm, "macro-vba.pptm").unwrap().macros);
    let v = parse(&e.slide_command_json(VBA_CMD).unwrap());
    assert_sample(&v, "Slide1", "MsgBox \"Xin chào\"");
    let saved = e.save_pptx().unwrap();
    assert_eq!(part(&saved, "/ppt/vbaProject.bin"), vba);
    let mut again = Engine::with_bundled_fonts();
    assert!(again.open(&saved, "saved.pptm").unwrap().macros);
    assert_eq!(parse(&again.slide_command_json(VBA_CMD).unwrap()), v);
}

/// `host` (a binary `.doc` / `.xls` of the corpus) with the generated project stored
/// under `root`, as the binary formats keep it.
fn graft(host: &[u8], root: &str, project: &[u8]) -> Vec<u8> {
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
    dst.flush().unwrap();
    dst.into_inner().into_inner()
}

#[test]
fn binary_documents_show_their_macro_source() {
    // The project of the generated .docm, moved into the storages of [MS-DOC] / [MS-XLS].
    let project = part(&corpus("macro/macro-vba.docm"), "/word/vbaProject.bin");
    let doc = graft(&corpus("doc/binary-doc-01-sample.doc"), "/Macros", &project);
    let mut e = Engine::with_bundled_fonts();
    assert!(e.open(&doc, "macro.doc").unwrap().macros);
    let v = parse(&e.command_json(VBA_CMD).unwrap());
    assert_sample(&v, "ThisDocument", "Selection.TypeText \"Xin chào\"");

    let xls = graft(
        &corpus("sheet/binary-xls-01-features.xls"),
        "/_VBA_PROJECT_CUR",
        &project,
    );
    let mut s = SheetEngine::new();
    assert!(s.open(&xls, "macro.xls").unwrap().macros);
    let v = parse(&s.exec_json(VBA_CMD).unwrap());
    assert_sample(&v, "ThisDocument", "Selection.TypeText \"Xin chào\"");
}

#[test]
fn unreadable_project_is_reported_not_hidden() {
    // A package whose vbaProject.bin is not a compound file at all.
    let docm = corpus("macro/macro-vba.docm");
    let mut pkg = Package::read(&docm, &Limits::default()).unwrap();
    pkg.put_part("/word/vbaProject.bin", b"not a project".to_vec());
    let broken = pkg.write().unwrap();
    let mut e = Engine::with_bundled_fonts();
    assert!(e.open(&broken, "broken.docm").unwrap().macros);
    let v = parse(&e.command_json(VBA_CMD).unwrap());
    assert_eq!(v["modules"], json!([]));
    assert!(v["error"].as_str().is_some_and(|s| !s.is_empty()), "{v}");
    // The damaged part is still preserved as it was.
    assert_eq!(
        part(&e.save_docx().unwrap(), "/word/vbaProject.bin"),
        b"not a project"
    );
}
