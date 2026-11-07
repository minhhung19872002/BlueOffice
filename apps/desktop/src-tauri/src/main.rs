//! BlueOffice desktop shell (Tauri 2).
//!
//! The editor UI is the same React code as the web app; only the platform layer
//! differs. The engine runs natively here (`bo-engine`) and is exposed through
//! the commands below, mirroring the WASM API (`EngineHost`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
// Tauri command handlers must take their arguments by value.
#![allow(clippy::needless_pass_by_value)]

mod shell;

use bo_engine::{DocInfo, Engine};
use shell::{percent_decode, LaunchRequest};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_deep_link::DeepLinkExt;
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

/// File passed on the command line (file association: `bluedoc.exe file.docx`).
#[tauri::command]
fn startup_file() -> Option<String> {
    shell::classify_args(std::env::args().skip(1))
        .into_iter()
        .find_map(|r| match r {
            LaunchRequest::File { path } => Some(path),
            LaunchRequest::Link(_) => None,
        })
}

/// Deep link passed on the command line (`bluedoc://open?host=…&file=…`).
#[tauri::command]
fn startup_link() -> Option<shell::DeepLink> {
    shell::classify_args(std::env::args().skip(1))
        .into_iter()
        .find_map(|r| match r {
            LaunchRequest::Link(l) => Some(l),
            LaunchRequest::File { .. } => None,
        })
}

/// Opens another native document window (`query` e.g. `app=sheet`).
#[tauri::command]
async fn open_window(app: tauri::AppHandle, query: String) -> Result<String, String> {
    let url = shell::window_url(&query)?;
    let label = shell::next_window_label();
    let title = app
        .config()
        .product_name
        .clone()
        .unwrap_or_else(|| "BlueOffice".into());
    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(1280.0, 860.0)
        .min_inner_size(720.0, 480.0)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(label)
}

/// Forwards OS open requests (second instance, deep link) to the focused UI.
fn dispatch_requests(app: &tauri::AppHandle, requests: Vec<LaunchRequest>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    for r in requests {
        let _ = match r {
            LaunchRequest::File { path } => app.emit("open-file", path),
            LaunchRequest::Link(link) => app.emit("open-link", link),
        };
    }
}

#[tauri::command]
fn set_title(window: tauri::WebviewWindow, title: String) -> Result<(), String> {
    window.set_title(&title).map_err(|e| e.to_string())
}

fn main() {
    let builder = tauri::Builder::default()
        // Must be the first plugin: a second launch forwards its args here.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            dispatch_requests(app, shell::classify_args(argv.iter().skip(1)));
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init());
    #[cfg(feature = "updater")]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    builder
        .setup(|app| {
            app.manage(EngineState(Mutex::new(Engine::with_bundled_fonts())));
            // Windows/Linux dev builds: register schemes at runtime (installers do it at install time).
            #[cfg(all(debug_assertions, any(windows, target_os = "linux")))]
            let _ = app.deep_link().register_all();
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                let reqs = event
                    .urls()
                    .iter()
                    .filter_map(|u| shell::parse_deep_link(u.as_str()).ok())
                    .map(LaunchRequest::Link)
                    .collect();
                dispatch_requests(&handle, reqs);
            });
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
            startup_link,
            open_window,
            set_title
        ])
        .run(tauri::generate_context!())
        .expect("error while running BlueOffice");
}

#[cfg(test)]
mod tests {
    use super::*;

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
