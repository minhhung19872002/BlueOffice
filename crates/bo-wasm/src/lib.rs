//! WebAssembly bindings for the BlueOffice engine.
//!
//! The JS side (`packages/platform`, `WebEngineHost`) runs this module inside a
//! Web Worker. Structured results cross the boundary as JSON strings; binary
//! results (PDF, PNG) as `Uint8Array`.
#![forbid(unsafe_code)]

mod collab;
mod sheet;
mod sign;
pub use collab::WasmCollab;
pub use sheet::WasmSheet;
pub use sign::WasmPreparedSignature;

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

    /// Open a document (decrypting it with `password` when it is protected); returns
    /// `DocInfo` as JSON. Errors `password required` / `wrong password` ask for one.
    // wasm-bindgen passes optional strings by value.
    #[allow(clippy::needless_pass_by_value)]
    pub fn open(
        &mut self,
        bytes: &[u8],
        name: &str,
        password: Option<String>,
    ) -> Result<String, JsError> {
        let info = self
            .inner
            .open_with_password(bytes, name, password.as_deref())
            .map_err(err)?;
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

    /// Run a slide editing command (JSON `SlideCommand`); returns `SlideEditResult` as JSON.
    #[wasm_bindgen(js_name = slideCommand)]
    pub fn slide_command(&mut self, json: &str) -> Result<String, JsError> {
        self.inner.slide_command_json(json).map_err(err)
    }

    /// Insert a PNG/JPEG picture on the current slide; returns `SlideEditResult` as JSON.
    #[wasm_bindgen(js_name = slideInsertPicture)]
    pub fn slide_insert_picture(&mut self, bytes: Vec<u8>, name: &str) -> Result<String, JsError> {
        let r = self.inner.slide_insert_picture(bytes, name).map_err(err)?;
        Ok(serde_json::to_string(&r).unwrap_or_default())
    }

    /// Bind the open document to a collaboration session; returns the change
    /// to publish (`seed`) as JSON (`CollabChange`).
    /// `client_id` = the replica's client id (ids of new tables, rows, columns).
    #[wasm_bindgen(js_name = collabStart)]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn collab_start(&mut self, seed: bool, client_id: Option<f64>) -> Result<String, JsError> {
        self.inner
            .collab_start_json(seed, client_id.unwrap_or(0.0).max(0.0) as u64)
            .map_err(err)
    }

    /// Apply a replica change (`WasmCollab.takeRemote`); returns `EditResult` as JSON.
    #[wasm_bindgen(js_name = collabApply)]
    pub fn collab_apply(&mut self, json: &str) -> Result<String, JsError> {
        self.inner.collab_apply_json(json).map_err(err)
    }

    /// Caret/selection geometry of collaborators (`[[anchor, head], …]` UTF-16).
    #[wasm_bindgen(js_name = collabPeers)]
    pub fn collab_peers(&self, json: &str) -> Result<String, JsError> {
        self.inner.collab_peers_json(json).map_err(err)
    }

    /// The shared body text as the engine sees it (embeds as U+FFFC; diagnostics, tests).
    #[wasm_bindgen(js_name = collabText)]
    pub fn collab_text(&self) -> String {
        self.inner
            .collab_runs()
            .into_iter()
            .map(|(t, _)| t)
            .collect()
    }

    #[wasm_bindgen(js_name = collabStop)]
    pub fn collab_stop(&mut self) {
        self.inner.collab_stop();
        self.inner.slide_collab_stop();
    }

    /// Bind the open presentation to a collaboration session.
    #[wasm_bindgen(js_name = slideCollabStart)]
    pub fn slide_collab_start(&mut self, seed: bool, client_id: f64) -> Result<String, JsError> {
        self.inner
            .slide_collab_start_json(seed, client_id.max(0.0) as u64)
            .map_err(err)
    }

    /// Apply a replica change to the presentation; returns `SlideEditResult` as JSON.
    #[wasm_bindgen(js_name = slideCollabApply)]
    pub fn slide_collab_apply(&mut self, json: &str) -> Result<String, JsError> {
        self.inner.slide_collab_apply_json(json).map_err(err)
    }

    /// Boxes of collaborators' selected objects (`["slide/shape", …]`).
    #[wasm_bindgen(js_name = slideCollabPeers)]
    pub fn slide_collab_peers(&self, json: &str) -> Result<String, JsError> {
        self.inner.slide_collab_peers_json(json).map_err(err)
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
