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

    /// Number of slides of the open presentation (0 for documents).
    #[wasm_bindgen(js_name = slideCount)]
    pub fn slide_count(&self) -> usize {
        self.inner.slide_count()
    }

    /// Display list of a slide as JSON.
    #[wasm_bindgen(js_name = slideJson)]
    pub fn slide_json(&self, index: usize) -> Result<String, JsError> {
        self.inner.slide_json(index).map_err(err)
    }

    /// Thumbnail display list of a slide (scaled to `width` points) as JSON.
    #[wasm_bindgen(js_name = slideThumbnailJson)]
    pub fn slide_thumbnail_json(&mut self, index: usize, width: f32) -> Result<String, JsError> {
        self.inner.slide_thumbnail_json(index, width).map_err(err)
    }

    /// Speaker notes of a slide.
    #[wasm_bindgen(js_name = slideNotes)]
    pub fn slide_notes(&self, index: usize) -> Result<String, JsError> {
        self.inner.slide_notes(index).map_err(err)
    }

    /// Bytes of an image referenced by an `image` display-list command.
    #[wasm_bindgen(js_name = imageBytes)]
    pub fn image_bytes(&self, id: &str) -> Option<Vec<u8>> {
        self.inner.image_bytes(id).map(<[u8]>::to_vec)
    }

    /// Save the open presentation as `.pptx` bytes.
    #[wasm_bindgen(js_name = savePptx)]
    pub fn save_pptx(&self) -> Result<Vec<u8>, JsError> {
        self.inner.save_pptx().map_err(err)
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
