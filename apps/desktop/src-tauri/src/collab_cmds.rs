//! Live collaboration binding of the native engine (`bo_engine::collab`,
//! `bo_engine::slide_edit::collab`), mirroring the WASM `collab*` methods of
//! `EngineHost`. The replica itself (`bo-collab`) runs in the web view.

use crate::EngineState;
use tauri::State;

fn lock<'a>(
    state: &'a State<'_, EngineState>,
) -> Result<std::sync::MutexGuard<'a, bo_engine::Engine>, String> {
    state.0.lock().map_err(|e| e.to_string())
}

/// Bind the open document; returns the change to publish (JSON `CollabChange`).
#[tauri::command]
pub fn engine_collab_start(
    state: State<'_, EngineState>,
    seed: bool,
    client: Option<u64>,
) -> Result<String, String> {
    lock(&state)?
        .collab_start_json(seed, client.unwrap_or(0))
        .map_err(|e| e.to_string())
}

/// Apply a replica change → `EditResult` JSON.
#[tauri::command]
pub fn engine_collab_apply(state: State<'_, EngineState>, json: String) -> Result<String, String> {
    lock(&state)?
        .collab_apply_json(&json)
        .map_err(|e| e.to_string())
}

/// Collaborators' carets (`[[anchor, head], …]`) → geometry JSON.
#[tauri::command]
pub fn engine_collab_peers(state: State<'_, EngineState>, json: String) -> Result<String, String> {
    lock(&state)?
        .collab_peers_json(&json)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn engine_collab_stop(state: State<'_, EngineState>) -> Result<(), String> {
    let mut e = lock(&state)?;
    e.collab_stop();
    e.slide_collab_stop();
    Ok(())
}

/// Bind the open presentation; returns the change to publish.
#[tauri::command]
pub fn engine_slide_collab_start(
    state: State<'_, EngineState>,
    seed: bool,
    client: u64,
) -> Result<String, String> {
    lock(&state)?
        .slide_collab_start_json(seed, client)
        .map_err(|e| e.to_string())
}

/// Apply a replica change to the presentation → `SlideEditResult` JSON.
#[tauri::command]
pub fn engine_slide_collab_apply(
    state: State<'_, EngineState>,
    json: String,
) -> Result<String, String> {
    lock(&state)?
        .slide_collab_apply_json(&json)
        .map_err(|e| e.to_string())
}

/// Collaborators' selected objects (`["slide/shape", …]`) → boxes JSON.
#[tauri::command]
pub fn engine_slide_collab_peers(
    state: State<'_, EngineState>,
    json: String,
) -> Result<String, String> {
    lock(&state)?
        .slide_collab_peers_json(&json)
        .map_err(|e| e.to_string())
}
