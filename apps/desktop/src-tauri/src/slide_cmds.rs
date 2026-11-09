//! BlueSlide editing commands (native `bo_engine::slide_edit`), mirroring
//! the WASM `slideCommand` / `slideInsertPicture` methods used by `EngineHost`.

use crate::{header_name, raw_body, EngineState};
use tauri::ipc::{Request, Response};
use tauri::State;

/// Slide editing command (JSON `bo_engine::SlideCommand`) → `SlideEditResult` JSON.
#[tauri::command]
pub fn engine_slide_command(state: State<'_, EngineState>, json: String) -> Result<String, String> {
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    e.slide_command_json(&json).map_err(|e| e.to_string())
}

/// Insert a picture (raw body, `x-file-name` header) on the current slide.
#[tauri::command]
pub fn engine_slide_insert_picture(
    state: State<'_, EngineState>,
    request: Request<'_>,
) -> Result<String, String> {
    let bytes = raw_body(&request)?;
    let name = header_name(&request);
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    let r = e
        .slide_insert_picture(bytes, &name)
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&r).map_err(|e| e.to_string())
}

/// Handout / notes PDF, slide pictures or presentation package
/// (JSON `bo_engine::slide_export::SlideExport`) → file bytes.
#[tauri::command]
pub fn engine_slide_export(
    state: State<'_, EngineState>,
    json: String,
) -> Result<Response, String> {
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    e.export_slides_json(&json)
        .map(Response::new)
        .map_err(|e| e.to_string())
}
