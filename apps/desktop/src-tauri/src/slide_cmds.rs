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

/// Start a slide show video / GIF export (JSON `VideoOptions`) → `VideoInfo` JSON.
#[tauri::command]
pub fn engine_video_begin(state: State<'_, EngineState>, json: String) -> Result<String, String> {
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    e.video_begin_json(&json).map_err(|e| e.to_string())
}

/// Frame `index` of the export: straight RGBA, or PNG when `png`.
#[tauri::command]
pub fn engine_video_frame(
    state: State<'_, EngineState>,
    index: u32,
    png: bool,
) -> Result<Response, String> {
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    let bytes = if png {
        e.video_frame_png(index)
    } else {
        e.video_frame(index)
    };
    bytes.map(Response::new).map_err(|e| e.to_string())
}

/// Render frame `index` into the GIF of the export.
#[tauri::command]
pub fn engine_video_gif_add(state: State<'_, EngineState>, index: u32) -> Result<(), String> {
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    e.video_gif_add(index).map_err(|e| e.to_string())
}

/// Finish the GIF of the export.
#[tauri::command]
pub fn engine_video_gif_finish(state: State<'_, EngineState>) -> Result<Response, String> {
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    e.video_gif_finish()
        .map(Response::new)
        .map_err(|e| e.to_string())
}

/// End the export.
#[tauri::command]
pub fn engine_video_end(state: State<'_, EngineState>) -> Result<(), String> {
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    e.video_end();
    Ok(())
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
