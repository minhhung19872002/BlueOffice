//! The open document a headless run works on, and its engine [`Bridge`].

use crate::{App, Bridge};
use bo_engine::{Engine, SheetEngine};
use serde_json::Value;

/// A document opened with the native engine.
pub enum Document {
    /// BlueDoc (`Engine`, document mode).
    Doc(Box<Engine>),
    /// BlueSlide (`Engine`, presentation mode).
    Slide(Box<Engine>),
    /// BlueSheet.
    Sheet(Box<SheetEngine>),
}

/// The file could not be opened or saved.
#[derive(Debug)]
pub struct OpenError(pub String);

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for OpenError {}

fn err(e: impl std::fmt::Display) -> OpenError {
    OpenError(e.to_string())
}

/// Workbook file name extensions (everything else opens in `Engine`).
const SHEET_EXTS: &[&str] = &[
    "xlsx", "xlsm", "xltx", "xltm", "xls", "xlsb", "ods", "csv", "tsv",
];

impl Document {
    /// Opens `bytes`; the application follows the name's extension and the content.
    /// `locale`: BlueSheet input locale (`vi-VN` or `en-US`).
    ///
    /// # Errors
    /// Unreadable or unsupported file.
    pub fn open(bytes: &[u8], name: &str, locale: &str) -> Result<Self, OpenError> {
        let ext = std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if SHEET_EXTS.contains(&ext.as_str()) {
            let mut e = SheetEngine::with_bundled_fonts();
            e.set_locale(locale);
            e.open(bytes, name).map_err(err)?;
            return Ok(Document::Sheet(Box::new(e)));
        }
        let mut e = Engine::with_bundled_fonts();
        let info = e.open(bytes, name).map_err(err)?;
        Ok(if info.kind == "slide" {
            Document::Slide(Box::new(e))
        } else {
            Document::Doc(Box::new(e))
        })
    }

    /// The object API scripts get for this document.
    #[must_use]
    pub fn app(&self) -> App {
        match self {
            Document::Doc(_) => App::Doc,
            Document::Slide(_) => App::Slide,
            Document::Sheet(_) => App::Sheet,
        }
    }

    /// Saves as `ext`: `docx` / `pptx` / `xlsx` (the document's own package), `csv`
    /// (first sheet) or `pdf`.
    ///
    /// # Errors
    /// The format does not fit the document, or saving failed.
    pub fn save(&mut self, ext: &str) -> Result<Vec<u8>, OpenError> {
        let ext = ext.to_ascii_lowercase();
        match (self, ext.as_str()) {
            (Document::Doc(e) | Document::Slide(e), "pdf") => e.export_pdf().map_err(err),
            (Document::Doc(e), "docx" | "docm") => e.save_docx().map_err(err),
            (Document::Slide(e), "pptx" | "pptm") => e.save_pptx().map_err(err),
            (Document::Sheet(e), "pdf") => e.export_pdf().map_err(err),
            (Document::Sheet(e), "xlsx" | "xlsm" | "csv") => e
                .save(if ext == "csv" { "csv" } else { "xlsx" }, 0)
                .map_err(err),
            (d, other) => Err(OpenError(format!(
                "cannot save a {} file as .{other}",
                d.app().as_str()
            ))),
        }
    }
}

fn arg_usize(args: &[Value], i: usize, what: &str) -> Result<usize, String> {
    args.get(i)
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| format!("{what}: expected an integer ≥ 0"))
}

fn arg_u32(args: &[Value], i: usize, what: &str) -> Result<u32, String> {
    args.get(i)
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| format!("{what}: expected an integer ≥ 0"))
}

fn json<T: serde::Serialize>(v: &T) -> Result<String, String> {
    serde_json::to_string(v).map_err(|e| e.to_string())
}

impl Bridge for Document {
    fn call(&mut self, op: &str, args: &[Value]) -> Result<String, String> {
        let first = || args.first().map(Value::to_string).unwrap_or_default();
        match (self, op) {
            // ADR-0008: the editor's own JSON command bridges.
            (Document::Doc(e), "command") => e.command_json(&first()).map_err(|e| e.to_string()),
            (Document::Slide(e), "slideCommand") => {
                e.slide_command_json(&first()).map_err(|e| e.to_string())
            }
            (Document::Sheet(e), "exec") => e.exec_json(&first()).map_err(|e| e.to_string()),
            (Document::Sheet(e), "info") => json(&e.info().map_err(|e| e.to_string())?),
            (Document::Sheet(e), "cell") => {
                let d = e
                    .cell(
                        arg_usize(args, 0, "sheet")?,
                        arg_u32(args, 1, "row")?,
                        arg_u32(args, 2, "col")?,
                    )
                    .map_err(|e| e.to_string())?;
                json(&d)
            }
            (Document::Sheet(e), "addSheet") => {
                let name = args.first().and_then(Value::as_str);
                json(&e.add_sheet(name).map_err(|e| e.to_string())?)
            }
            (Document::Sheet(e), "renameSheet") => {
                let name = args
                    .get(1)
                    .and_then(Value::as_str)
                    .ok_or("name: expected a string")?;
                e.rename_sheet(arg_usize(args, 0, "sheet")?, name)
                    .map_err(|e| e.to_string())?;
                Ok(String::new())
            }
            (_, other) => Err(format!("unknown engine operation {other}")),
        }
    }
}
