//! Macro-enabled corpus files (`tests/corpus/macro`, CORE-VBA-002): a document, a
//! workbook and a presentation that carry a generated VBA project
//! (`bo_binary::vba_write::sample_project` — written from source text per
//! [MS-OVBA], no bytes from any other product). They exercise the read-only macro
//! viewer and the preservation of the project on save; the macros are never run.
//! `macro-legacy.doc` / `.xls` carry the projects in the storages of the binary formats.

use bo_binary::vba::{CT_VBA_PROJECT, REL_VBA_PROJECT};
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// `package` with `vba` as its VBA project: the `vbaProject.bin` part next to the main
/// part, the relationship from the main part ([MS-OFFMACRO2]) and the macro-enabled
/// content type of the main part.
pub fn attach_vba_project(package: &[u8], vba: &[u8]) -> Res<Vec<u8>> {
    let mut pkg = Package::read(package, &Limits::default())?;
    let main = pkg
        .related_part("/", REL_OFFICE_DOCUMENT)?
        .ok_or("no officeDocument relationship")?;
    let dir = main.rsplit_once('/').map_or("", |(d, _)| d);
    let part = format!("{dir}/vbaProject.bin");
    pkg.put_part(&part, vba.to_vec());
    let mut types = pkg.content_types()?;
    types.set_override(&part, CT_VBA_PROJECT);
    let enabled = types
        .get(&main)
        .and_then(bo_binary::vba::macro_enabled_content_type)
        .ok_or("the main part has no macro-enabled variant")?;
    types.set_override(&main, enabled);
    pkg.set_content_types(&types);
    let mut rels = pkg.relationships(&main)?;
    rels.push(Relationship {
        id: "rIdVba".into(),
        rel_type: REL_VBA_PROJECT.into(),
        target: "vbaProject.bin".into(),
        mode: TargetMode::Internal,
    });
    pkg.set_relationships(&main, &rels);
    Ok(pkg.write()?)
}

/// The corpus files: (file name, bytes).
pub fn files() -> Res<Vec<(&'static str, Vec<u8>)>> {
    let docx = crate::corpus::macro_document()?;

    let mut wb = bo_model::sheet::Workbook::new();
    for (r, text) in ["Tệp mẫu có macro VBA", "Mã chỉ được hiển thị"]
        .into_iter()
        .enumerate()
    {
        wb.sheets[0].cells.insert(
            (r as u32, 0),
            bo_model::sheet::Cell {
                value: bo_model::sheet::CellValue::Text(text.to_owned()),
                ..Default::default()
            },
        );
    }
    let xlsx = bo_xlsx::write_xlsx(&wb)?;
    let pptx = crate::slide_samples::macro_base_deck();

    let vba = bo_binary::vba_write::sample_project;
    Ok(vec![
        ("macro-vba.docm", attach_vba_project(&docx, &vba("doc")?)?),
        ("macro-vba.xlsm", attach_vba_project(&xlsx, &vba("sheet")?)?),
        ("macro-vba.pptm", attach_vba_project(&pptx, &vba("slide")?)?),
        // Legacy binary files with the same projects in their project storages: they
        // open with the macros kept and save as `.docm` / `.xlsm`.
        (
            "macro-legacy.doc",
            bo_hostile::legacy_with_macros("doc").map_err(|e| e.to_string())?,
        ),
        (
            "macro-legacy.xls",
            bo_hostile::legacy_with_macros("sheet").map_err(|e| e.to_string())?,
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_engine::variant::{detect, Variant};

    fn corpus(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/macro")
            .join(name)
    }

    /// The committed files are what the generator writes (`bo-cli sample-macros`).
    #[test]
    fn committed_corpus_is_up_to_date() {
        for (name, bytes) in files().unwrap() {
            let on_disk = std::fs::read(corpus(name)).unwrap_or_default();
            assert!(
                on_disk == bytes,
                "{name} is stale: run `cargo run -p bo-cli -- sample-macros tests/corpus/macro`"
            );
        }
    }

    #[test]
    fn files_are_macro_enabled_packages_with_readable_source() {
        let expect = [
            (
                "macro-vba.docm",
                Variant::Docm,
                "ThisDocument",
                "Selection.TypeText",
            ),
            (
                "macro-vba.xlsm",
                Variant::Xlsm,
                "ThisWorkbook",
                "Range(\"A1\").Value",
            ),
            ("macro-vba.pptm", Variant::Pptm, "Slide1", "MsgBox"),
        ];
        for ((name, bytes), (n, variant, doc_module, code)) in files().unwrap().iter().zip(expect) {
            assert_eq!(*name, n);
            assert_eq!(detect(bytes), Some(variant));
            assert!(bo_binary::has_macros(bytes));
            let p = bo_engine::vba_view::read(bytes).unwrap();
            assert_eq!(p.error, None);
            assert_eq!((p.code_page, p.code_page_supported), (1258, true));
            let got: Vec<_> = p
                .modules
                .iter()
                .map(|m| (m.name.as_str(), m.kind))
                .collect();
            assert_eq!(got, [(doc_module, "document"), ("Module1", "standard")]);
            assert_eq!(p.modules[0].source, "Option Explicit\n");
            let src = &p.modules[1].source;
            assert!(src.starts_with("Option Explicit\n\n' Ghi lời chào — mã này chỉ được hiển thị"));
            assert!(src.contains(code) && src.contains("\"Xin chào\""), "{src}");
        }
    }
}
