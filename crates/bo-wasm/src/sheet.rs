//! BlueSheet engine for JavaScript (`bo_engine::SheetEngine`). JSON in/out,
//! viewport coordinates and sizes in points.

use bo_engine::sheet::{SheetEngine, ViewRequest};
use wasm_bindgen::prelude::*;

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

/// Spreadsheet engine handle exported to JavaScript.
#[wasm_bindgen]
pub struct WasmSheet {
    inner: SheetEngine,
}

#[wasm_bindgen]
impl WasmSheet {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmSheet {
        WasmSheet {
            inner: SheetEngine::new(),
        }
    }

    #[wasm_bindgen(js_name = addFont)]
    pub fn add_font(&mut self, bytes: Vec<u8>) -> usize {
        self.inner.add_font(bytes)
    }

    /// Clock for `NOW()`/`TODAY()` (`Date.now()`).
    #[wasm_bindgen(js_name = setNow)]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn set_now(&mut self, unix_ms: f64) {
        self.inner.set_now_ms(unix_ms.max(0.0) as u64);
    }

    #[wasm_bindgen(js_name = setLocale)]
    pub fn set_locale(&mut self, tag: &str) {
        self.inner.set_locale(tag);
    }

    /// Opens `.xlsx`/`.csv`; returns `WorkbookInfo` JSON.
    pub fn open(&mut self, bytes: &[u8], name: &str) -> Result<String, JsError> {
        Ok(json(&self.inner.open(bytes, name).map_err(err)?))
    }

    #[wasm_bindgen(js_name = newWorkbook)]
    pub fn new_workbook(&mut self, name: &str) -> Result<String, JsError> {
        Ok(json(&self.inner.new_workbook(name).map_err(err)?))
    }

    pub fn info(&self) -> Result<String, JsError> {
        Ok(json(&self.inner.info().map_err(err)?))
    }

    /// Display list (page JSON) of a viewport.
    pub fn render(
        &mut self,
        sheet: usize,
        scroll_x: f64,
        scroll_y: f64,
        width: f64,
        height: f64,
    ) -> Result<String, JsError> {
        let req = ViewRequest {
            sheet,
            scroll_x,
            scroll_y,
            width,
            height,
        };
        self.inner.render_json(&req).map_err(err)
    }

    #[wasm_bindgen(js_name = hitTest)]
    #[allow(clippy::too_many_arguments)]
    pub fn hit_test(
        &mut self,
        sheet: usize,
        scroll_x: f64,
        scroll_y: f64,
        width: f64,
        height: f64,
        x: f64,
        y: f64,
    ) -> Result<String, JsError> {
        let req = ViewRequest {
            sheet,
            scroll_x,
            scroll_y,
            width,
            height,
        };
        Ok(json(&self.inner.hit_test(&req, x, y).map_err(err)?))
    }

    #[wasm_bindgen(js_name = rangeRect)]
    #[allow(clippy::too_many_arguments)]
    pub fn range_rect(
        &mut self,
        sheet: usize,
        scroll_x: f64,
        scroll_y: f64,
        width: f64,
        height: f64,
        r0: u32,
        c0: u32,
        r1: u32,
        c1: u32,
    ) -> Result<String, JsError> {
        let req = ViewRequest {
            sheet,
            scroll_x,
            scroll_y,
            width,
            height,
        };
        Ok(json(
            &self
                .inner
                .range_rect(&req, (r0, c0), (r1, c1))
                .map_err(err)?,
        ))
    }

    #[wasm_bindgen(js_name = contentSize)]
    pub fn content_size(&mut self, sheet: usize) -> Result<Vec<f64>, JsError> {
        let (w, h) = self.inner.content_size(sheet).map_err(err)?;
        Ok(vec![w, h])
    }

    pub fn cell(&self, sheet: usize, row: u32, col: u32) -> Result<String, JsError> {
        Ok(json(&self.inner.cell(sheet, row, col).map_err(err)?))
    }

    /// `cells` = JSON `[[row, col, "input"], …]`, applied as one undo step.
    #[wasm_bindgen(js_name = setInputs)]
    pub fn set_inputs(&mut self, sheet: usize, cells: &str) -> Result<(), JsError> {
        let v: Vec<(u32, u32, String)> = serde_json::from_str(cells).map_err(err)?;
        self.inner.set_inputs(sheet, &v).map_err(err)
    }

    pub fn structural(
        &mut self,
        sheet: usize,
        rows: bool,
        at: u32,
        count: u32,
        delete: bool,
    ) -> Result<(), JsError> {
        self.inner
            .structural(sheet, rows, at, count, delete)
            .map_err(err)
    }

    pub fn undo(&mut self) -> Result<bool, JsError> {
        self.inner.undo().map_err(err)
    }

    pub fn redo(&mut self) -> Result<bool, JsError> {
        self.inner.redo().map_err(err)
    }

    #[wasm_bindgen(js_name = addSheet)]
    #[allow(clippy::needless_pass_by_value)] // wasm-bindgen needs owned Option<String>
    pub fn add_sheet(&mut self, name: Option<String>) -> Result<usize, JsError> {
        self.inner.add_sheet(name.as_deref()).map_err(err)
    }

    #[wasm_bindgen(js_name = renameSheet)]
    pub fn rename_sheet(&mut self, sheet: usize, name: &str) -> Result<(), JsError> {
        self.inner.rename_sheet(sheet, name).map_err(err)
    }

    #[wasm_bindgen(js_name = deleteSheet)]
    pub fn delete_sheet(&mut self, sheet: usize) -> Result<(), JsError> {
        self.inner.delete_sheet(sheet).map_err(err)
    }

    /// Binds the workbook to a collaboration session; returns the change to
    /// publish when seeding (JSON `CollabChange`).
    #[wasm_bindgen(js_name = collabStart)]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn collab_start(&mut self, seed: bool, client_id: f64) -> Result<String, JsError> {
        self.inner
            .collab_start_json(seed, client_id.max(0.0) as u64)
            .map_err(err)
    }

    /// Local changes since the last call (JSON `CollabChange`).
    #[wasm_bindgen(js_name = collabLocal)]
    pub fn collab_local(&mut self) -> Result<String, JsError> {
        self.inner.collab_local_json().map_err(err)
    }

    /// Applies a replica change; returns the workbook info JSON.
    #[wasm_bindgen(js_name = collabApply)]
    pub fn collab_apply(&mut self, json: &str) -> Result<String, JsError> {
        self.inner.collab_apply_json(json).map_err(err)
    }

    #[wasm_bindgen(js_name = collabStop)]
    pub fn collab_stop(&mut self) {
        self.inner.collab_stop();
    }

    /// Editing command (`bo_engine::sheet::SheetCommand` JSON) → JSON result.
    pub fn exec(&mut self, command: &str) -> Result<String, JsError> {
        self.inner.exec_json(command).map_err(err)
    }

    /// `format` = `xlsx` | `csv`.
    pub fn save(&mut self, format: &str, sheet: usize) -> Result<Vec<u8>, JsError> {
        self.inner.save(format, sheet).map_err(err)
    }

    #[wasm_bindgen(js_name = exportPdf)]
    pub fn export_pdf(&mut self) -> Result<Vec<u8>, JsError> {
        self.inner.export_pdf().map_err(err)
    }

    #[wasm_bindgen(js_name = glyphOutlines)]
    pub fn glyph_outlines(&self, face: u32, glyphs: &[u16]) -> String {
        self.inner.glyph_outlines_json(face, glyphs)
    }
}

impl Default for WasmSheet {
    fn default() -> Self {
        Self::new()
    }
}
