//! `bo-cli edit`: fill a template from a JSON list of edits (CORE-API-001,
//! `bo_engine::fill`) and save it in its format or as PDF.

use bo_engine::fill::{parse_edits, FillOutcome};
use std::path::Path;

type Error = Box<dyn std::error::Error>;

/// Apply the edits of `edits_json` to `input`; returns the output bytes and what each
/// edit changed. `to`: `docx` / `xlsx` (the input's own format) or `pdf`.
pub fn run(input: &Path, edits_json: &str, to: &str) -> Result<(Vec<u8>, Vec<FillOutcome>), Error> {
    let edits = parse_edits(edits_json)?;
    let bytes = std::fs::read(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let name = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document");
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "xlsx" | "xlsm" => {
            let mut engine = bo_engine::SheetEngine::with_bundled_fonts();
            engine.open(&bytes, name)?;
            let report = engine.fill(&edits)?;
            let out = match to {
                "pdf" => engine.export_pdf()?,
                "xlsx" => engine.save("xlsx", 0)?,
                other => return Err(format!("cannot save a workbook as '{other}'").into()),
            };
            Ok((out, report))
        }
        "docx" | "docm" => {
            let mut engine = bo_engine::Engine::with_bundled_fonts();
            engine.open(&bytes, name)?;
            let report = engine.fill(&edits)?;
            let out = match to {
                "pdf" => engine.export_pdf()?,
                "docx" => engine.save_docx()?,
                other => return Err(format!("cannot save a document as '{other}'").into()),
            };
            Ok((out, report))
        }
        other => Err(format!("edit: unsupported template type '.{other}' (docx, xlsx)").into()),
    }
}
