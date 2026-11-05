//! BlueOffice desktop shell (Tauri 2).
//!
//! The editor UI is the same React code as the web app; only the platform layer
//! differs. The engine runs natively here (`bo-engine`) and is exposed through
//! the commands below, mirroring the WASM API (`EngineHost`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
// Tauri command handlers must take their arguments by value.
#![allow(clippy::needless_pass_by_value)]

use bo_engine::{DocInfo, Engine};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

struct EngineState(Mutex<Engine>);

/// Fonts compiled into the engine, in face-id order (same as the web manifest).
const FONT_FILES: &[&str] = &[
    "Tinos-Regular.ttf",
    "Tinos-Bold.ttf",
    "Tinos-Italic.ttf",
    "Tinos-BoldItalic.ttf",
    "LiberationSans-Regular.ttf",
    "LiberationSans-Bold.ttf",
    "LiberationSans-Italic.ttf",
    "LiberationSans-BoldItalic.ttf",
    "Carlito-Regular.ttf",
    "Carlito-Bold.ttf",
    "Carlito-Italic.ttf",
    "Carlito-BoldItalic.ttf",
];

fn raw_body(req: &Request<'_>) -> Result<Vec<u8>, String> {
    match req.body() {
        InvokeBody::Raw(b) => Ok(b.clone()),
        InvokeBody::Json(_) => Err("expected a binary request body".into()),
    }
}

fn header_name(req: &Request<'_>) -> String {
    req.headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map_or_else(|| "document".into(), percent_decode)
}

/// Decode `encodeURIComponent` output.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[tauri::command]
fn engine_init(state: State<'_, EngineState>) -> usize {
    state.0.lock().map_or(0, |e| e.face_count())
}

#[tauri::command]
fn engine_open(state: State<'_, EngineState>, request: Request<'_>) -> Result<DocInfo, String> {
    let bytes = raw_body(&request)?;
    let name = header_name(&request);
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    e.open(&bytes, &name).map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_page(state: State<'_, EngineState>, index: usize) -> Result<String, String> {
    let e = state.0.lock().map_err(|e| e.to_string())?;
    e.page_json(index).map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_glyph_outlines(
    state: State<'_, EngineState>,
    face: u32,
    glyphs: Vec<u16>,
) -> Result<String, String> {
    let e = state.0.lock().map_err(|e| e.to_string())?;
    Ok(e.glyph_outlines_json(face, &glyphs))
}

#[tauri::command]
fn engine_export_pdf(state: State<'_, EngineState>) -> Result<Response, String> {
    let e = state.0.lock().map_err(|e| e.to_string())?;
    e.export_pdf().map(Response::new).map_err(|e| e.to_string())
}

#[tauri::command]
fn bundled_fonts() -> Vec<&'static str> {
    FONT_FILES.to_vec()
}

#[derive(serde::Serialize)]
struct Picked {
    name: String,
    path: String,
}

#[tauri::command]
async fn pick_file(
    app: tauri::AppHandle,
    extensions: Vec<String>,
) -> Result<Option<Picked>, String> {
    let exts: Vec<&str> = extensions.iter().map(String::as_str).collect();
    let picked = app
        .dialog()
        .file()
        .add_filter("Documents", &exts)
        .blocking_pick_file();
    let Some(fp) = picked else { return Ok(None) };
    let path = fp.into_path().map_err(|e| e.to_string())?;
    Ok(Some(Picked {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: path.to_string_lossy().into_owned(),
    }))
}

#[tauri::command]
async fn read_file(path: String) -> Result<Response, String> {
    std::fs::read(&path)
        .map(Response::new)
        .map_err(|e| format!("{path}: {e}"))
}

#[tauri::command]
async fn save_file(app: tauri::AppHandle, request: Request<'_>) -> Result<Option<String>, String> {
    let bytes = raw_body(&request)?;
    let name = header_name(&request);
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_owned())
        .unwrap_or_default();
    let mut dlg = app.dialog().file().set_file_name(&name);
    if !ext.is_empty() {
        dlg = dlg.add_filter(ext.to_uppercase(), &[ext.as_str()]);
    }
    let Some(fp) = dlg.blocking_save_file() else {
        return Ok(None);
    };
    let path = fp.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Phase 1 printing: hand the PDF to the system viewer (native print dialog in Phase 2).
#[tauri::command]
async fn print_pdf(request: Request<'_>) -> Result<(), String> {
    let bytes = raw_body(&request)?;
    let stem: String = header_name(&request)
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path: PathBuf = std::env::temp_dir().join(format!("blueoffice-print-{stem}.pdf"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    open_with_system(&path)
}

fn open_with_system(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(path)
        .spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(path).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// File passed on the command line (file association: `blueoffice.exe file.docx`).
#[tauri::command]
fn startup_file() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-') && std::path::Path::new(a).is_file())
}

#[tauri::command]
fn set_title(window: tauri::WebviewWindow, title: String) -> Result<(), String> {
    window.set_title(&title).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(EngineState(Mutex::new(Engine::with_bundled_fonts())));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_init,
            engine_open,
            engine_page,
            engine_glyph_outlines,
            engine_export_pdf,
            bundled_fonts,
            pick_file,
            read_file,
            save_file,
            print_pdf,
            startup_file,
            set_title
        ])
        .run(tauri::generate_context!())
        .expect("error while running BlueOffice");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_uri_components() {
        assert_eq!(
            percent_decode("T%C3%A0i%20li%E1%BB%87u.docx"),
            "Tài liệu.docx"
        );
        assert_eq!(percent_decode("a%2"), "a%2");
    }

    #[test]
    fn font_list_matches_web_manifest() {
        let manifest = include_str!("../../../../assets/fonts/manifest.json");
        let v: serde_json::Value = serde_json::from_str(manifest).unwrap();
        let fonts: Vec<&str> = v["fonts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_str().unwrap())
            .collect();
        assert_eq!(fonts, FONT_FILES);
    }
}
