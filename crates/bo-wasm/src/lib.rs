//! WebAssembly bindings for the BlueOffice engine.
//!
//! The JS side (`packages/platform`, `WebEngineHost`) runs this module inside a
//! Web Worker. Structured results cross the boundary as JSON strings; binary
//! results (PDF, PNG) as `Uint8Array`.
#![forbid(unsafe_code)]

use bo_engine::Engine;
use wasm_bindgen::prelude::*;

/// Engine handle exported to JavaScript.
#[wasm_bindgen]
pub struct WasmEngine {
    inner: Engine,
}

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

#[wasm_bindgen]
impl WasmEngine {
    /// Create an engine without fonts.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmEngine {
        WasmEngine {
            inner: Engine::new(),
        }
    }

    /// Register a font file; returns the number of faces added.
    #[wasm_bindgen(js_name = addFont)]
    pub fn add_font(&mut self, bytes: Vec<u8>) -> usize {
        self.inner.add_font(bytes)
    }

    /// Number of loaded faces.
    #[wasm_bindgen(js_name = faceCount)]
    pub fn face_count(&self) -> usize {
        self.inner.face_count()
    }

    /// Open a document; returns `DocInfo` as JSON.
    pub fn open(&mut self, bytes: &[u8], name: &str) -> Result<String, JsError> {
        let info = self.inner.open(bytes, name).map_err(err)?;
        Ok(info.to_json())
    }

    /// `DocInfo` of the open document as JSON.
    pub fn info(&self) -> Result<String, JsError> {
        Ok(self.inner.info().map_err(err)?.to_json())
    }

    /// Display list of a page as JSON.
    #[wasm_bindgen(js_name = pageJson)]
    pub fn page_json(&self, index: usize) -> Result<String, JsError> {
        self.inner.page_json(index).map_err(err)
    }

    /// Glyph outlines as JSON (`GlyphOutline[]`).
    #[wasm_bindgen(js_name = glyphOutlines)]
    pub fn glyph_outlines(&self, face: u32, glyphs: &[u16]) -> String {
        self.inner.glyph_outlines_json(face, glyphs)
    }

    /// Export the document as PDF bytes.
    #[wasm_bindgen(js_name = exportPdf)]
    pub fn export_pdf(&self) -> Result<Vec<u8>, JsError> {
        self.inner.export_pdf().map_err(err)
    }

    /// Start a blank document; returns `DocInfo` as JSON.
    #[wasm_bindgen(js_name = newDocument)]
    pub fn new_document(&mut self, name: &str) -> Result<String, JsError> {
        Ok(self.inner.new_document(name).map_err(err)?.to_json())
    }

    /// Run an editing command (JSON `Command`); returns `EditResult` as JSON.
    pub fn command(&mut self, json: &str) -> Result<String, JsError> {
        self.inner.command_json(json).map_err(err)
    }

    /// Insert a PNG/JPEG picture at the selection; returns `EditResult` as JSON.
    #[wasm_bindgen(js_name = insertImage)]
    pub fn insert_image(&mut self, bytes: Vec<u8>, name: &str) -> Result<String, JsError> {
        let r = self.inner.insert_image(bytes, name).map_err(err)?;
        Ok(serde_json::to_string(&r).unwrap_or_default())
    }

    /// Bytes of a media part referenced by an `image` command (empty when unknown).
    pub fn media(&self, name: &str) -> Vec<u8> {
        self.inner
            .media(name)
            .map(<[u8]>::to_vec)
            .unwrap_or_default()
    }

    /// Serialize the document as `.docx`.
    #[wasm_bindgen(js_name = saveDocx)]
    pub fn save_docx(&self) -> Result<Vec<u8>, JsError> {
        self.inner.save_docx().map_err(err)
    }

    /// Mark the current state as saved.
    #[wasm_bindgen(js_name = markSaved)]
    pub fn mark_saved(&mut self) {
        self.inner.mark_saved();
    }

    /// Render a page to PNG bytes.
    #[wasm_bindgen(js_name = renderPng)]
    pub fn render_png(&self, index: usize, scale: f32) -> Result<Vec<u8>, JsError> {
        self.inner.render_png(index, scale).map_err(err)
    }
}

impl Default for WasmEngine {
    fn default() -> Self {
        Self::new()
    }
}
