//! Collaboration replica for JavaScript (`bo_collab::Session`). Binary frames
//! cross the boundary packed as `[u32 LE length][bytes]…` because one call can
//! produce several y-sync messages.

use bo_collab::{Change, Presence, Session};
use wasm_bindgen::prelude::*;

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Packs frames as length-prefixed chunks.
fn pack(frames: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(frames.iter().map(|f| f.len() + 4).sum());
    for f in frames {
        let len = u32::try_from(f.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(f);
    }
    out
}

/// CRDT replica of one shared file.
#[wasm_bindgen]
pub struct WasmCollab {
    inner: Session,
}

#[wasm_bindgen]
impl WasmCollab {
    /// `client_id` should be random per tab (JS `crypto.getRandomValues`).
    #[wasm_bindgen(constructor)]
    pub fn new(client_id: u32) -> WasmCollab {
        WasmCollab {
            inner: Session::with_client_id(u64::from(client_id)),
        }
    }

    /// Clock for awareness timeouts / undo grouping (`Date.now()`).
    #[wasm_bindgen(js_name = setTime)]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn set_time(&self, unix_ms: f64) {
        self.inner.set_time(unix_ms.max(0.0) as u64);
    }

    /// Sync step 1 + awareness (one frame stream).
    pub fn start(&self) -> Result<Vec<u8>, JsError> {
        self.inner.start().map_err(err)
    }

    /// Handles one frame; returns packed replies.
    pub fn receive(&mut self, frame: &[u8]) -> Result<Vec<u8>, JsError> {
        Ok(pack(&self.inner.receive(frame).map_err(err)?))
    }

    /// Packed pending broadcasts.
    #[wasm_bindgen(js_name = drainOutbox)]
    pub fn drain_outbox(&mut self) -> Vec<u8> {
        pack(&self.inner.drain_outbox())
    }

    #[wasm_bindgen(js_name = setCell)]
    pub fn set_cell(&mut self, sheet: &str, cell: &str, input: &str) -> Result<(), JsError> {
        if input.is_empty() {
            self.inner.clear_cell(sheet, cell).map_err(err)
        } else {
            self.inner.set_cell(sheet, cell, input).map_err(err)
        }
    }

    /// All cells as JSON `{"Sheet1!A1": "input", …}`.
    #[wasm_bindgen(js_name = cellsJson)]
    pub fn cells_json(&self) -> String {
        serde_json::to_string(&self.inner.cells()).unwrap_or_else(|_| "{}".into())
    }

    /// Publishes presence (JSON `Presence`).
    #[wasm_bindgen(js_name = setPresence)]
    pub fn set_presence(&mut self, json: &str) -> Result<(), JsError> {
        let p: Presence = serde_json::from_str(json).map_err(err)?;
        self.inner.set_presence(&p).map_err(err)
    }

    /// Other users as JSON `[[clientId, Presence], …]`.
    #[wasm_bindgen(js_name = peersJson)]
    pub fn peers_json(&self) -> String {
        serde_json::to_string(&self.inner.peers()).unwrap_or_else(|_| "[]".into())
    }

    /// Client id of this replica.
    #[wasm_bindgen(js_name = clientId)]
    pub fn client_id(&self) -> f64 {
        self.inner.client_id() as f64
    }

    /// Applies a change made by this replica's editor (JSON `Change`; the
    /// `collab.change` of an engine result).
    #[wasm_bindgen(js_name = applyLocal)]
    pub fn apply_local(&mut self, json: &str) -> Result<(), JsError> {
        let c: Change = serde_json::from_str(json).map_err(err)?;
        self.inner.apply_local(&c).map_err(err)
    }

    /// Publishes the seeder's initial content (not undoable).
    pub fn seed(&mut self, json: &str) -> Result<(), JsError> {
        let c: Change = serde_json::from_str(json).map_err(err)?;
        self.inner.seed(&c).map_err(err)
    }

    /// Remote changes and undo/redo results to apply to the engine (JSON `Change`).
    #[wasm_bindgen(js_name = takeRemote)]
    pub fn take_remote(&mut self) -> String {
        let c = self.inner.take_remote();
        if c.is_empty() {
            return String::new();
        }
        serde_json::to_string(&c).unwrap_or_default()
    }

    /// The whole shared state as one change (late joiners).
    #[wasm_bindgen(js_name = fullState)]
    pub fn full_state(&mut self) -> String {
        serde_json::to_string(&self.inner.full_state()).unwrap_or_default()
    }

    /// Undoes this user's last change; the result comes from `takeRemote`.
    pub fn undo(&mut self) -> bool {
        self.inner.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.inner.redo()
    }

    #[wasm_bindgen(js_name = canUndo)]
    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    #[wasm_bindgen(js_name = canRedo)]
    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    /// Starts a new undo step.
    #[wasm_bindgen(js_name = stopCapturing)]
    pub fn stop_capturing(&mut self) {
        self.inner.stop_capturing();
    }

    /// Full state (server snapshot).
    pub fn snapshot(&self) -> Vec<u8> {
        self.inner.snapshot()
    }

    #[wasm_bindgen(js_name = loadSnapshot)]
    pub fn load_snapshot(&mut self, update: &[u8]) -> Result<(), JsError> {
        self.inner.load_snapshot(update).map_err(err)
    }
}

#[cfg(test)]
mod tests {
    use super::pack;

    #[test]
    fn packs_length_prefixed() {
        assert_eq!(
            pack(&[vec![1, 2], vec![], vec![3]]),
            vec![2, 0, 0, 0, 1, 2, 0, 0, 0, 0, 1, 0, 0, 0, 3]
        );
    }
}
