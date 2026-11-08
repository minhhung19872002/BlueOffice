//! BlueOffice desktop shell (Tauri 2).
//!
//! The editor UI is the same React code as the web app; only the platform layer
//! differs. The engine runs natively here (`bo-engine`) and is exposed through
//! the commands below, mirroring the WASM API (`EngineHost`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
// Tauri command handlers must take their arguments by value.
#![allow(clippy::needless_pass_by_value)]

mod collab_cmds;
mod print;
mod sheet_cmds;
mod shell;
mod sign_cmds;
mod slide_cmds;
mod update;

use bo_engine::{DocInfo, Engine, SystemFontIndex};
use print::PrintJobs;
use shell::{percent_decode, LaunchRequest};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::webview::PageLoadEvent;
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::DialogExt;

pub(crate) struct EngineState(pub(crate) Mutex<Engine>);

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
    "Caladea-Regular.ttf",
    "Caladea-Bold.ttf",
    "Caladea-Italic.ttf",
    "Caladea-BoldItalic.ttf",
    "STIXTwoMath-Regular.ttf",
];

pub(crate) fn raw_body(req: &Request<'_>) -> Result<Vec<u8>, String> {
    match req.body() {
        InvokeBody::Raw(b) => Ok(b.clone()),
        InvokeBody::Json(_) => Err("expected a binary request body".into()),
    }
}

pub(crate) fn header_name(req: &Request<'_>) -> String {
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
fn engine_open(
    state: State<'_, EngineState>,
    fonts: State<'_, FontsReady>,
    request: Request<'_>,
) -> Result<DocInfo, String> {
    // The first document waits (briefly) for the installed fonts.
    fonts.wait(FONT_WAIT);
    let bytes = raw_body(&request)?;
    let name = header_name(&request);
    // Password of a protected file ([MS-OFFCRYPTO]), percent-encoded.
    let password = request
        .headers()
        .get("x-password")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode);
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    e.open_with_password(&bytes, &name, password.as_deref())
        .map_err(|e| e.to_string())
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
fn engine_new(state: State<'_, EngineState>, name: String) -> Result<DocInfo, String> {
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    e.new_document(&name).map_err(|e| e.to_string())
}

/// Editing command (JSON `bo_engine::Command`) → `EditResult` JSON.
#[tauri::command]
fn engine_command(state: State<'_, EngineState>, json: String) -> Result<String, String> {
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    e.command_json(&json).map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_insert_image(
    state: State<'_, EngineState>,
    request: Request<'_>,
) -> Result<String, String> {
    let bytes = raw_body(&request)?;
    let name = header_name(&request);
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    let r = e.insert_image(bytes, &name).map_err(|e| e.to_string())?;
    serde_json::to_string(&r).map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_media(state: State<'_, EngineState>, name: String) -> Result<Response, String> {
    let e = state.0.lock().map_err(|e| e.to_string())?;
    Ok(Response::new(
        e.media(&name).map(<[u8]>::to_vec).unwrap_or_default(),
    ))
}

#[tauri::command]
fn engine_save_docx(state: State<'_, EngineState>) -> Result<Response, String> {
    let e = state.0.lock().map_err(|e| e.to_string())?;
    e.save_docx().map(Response::new).map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_slide_notes(state: State<'_, EngineState>, index: usize) -> Result<String, String> {
    let e = state.0.lock().map_err(|e| e.to_string())?;
    e.slide_notes(index).map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_save_pptx(state: State<'_, EngineState>) -> Result<Response, String> {
    let e = state.0.lock().map_err(|e| e.to_string())?;
    e.save_pptx().map(Response::new).map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_mark_saved(state: State<'_, EngineState>) -> Result<(), String> {
    state.0.lock().map_err(|e| e.to_string())?.mark_saved();
    Ok(())
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
    if let Some(path) = std::env::var(shell::E2E_SAVE_DIR_ENV)
        .ok()
        .and_then(|d| shell::e2e_save_path(&d, &name))
    {
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        return Ok(Some(path.to_string_lossy().into_owned()));
    }
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

/// Prints the engine's PDF through the OS print UI (see `print.rs`). Returns the
/// label of the print window, or `None` when the PDF went to the default viewer.
#[tauri::command]
async fn print_pdf(
    app: tauri::AppHandle,
    jobs: State<'_, PrintJobs>,
    request: Request<'_>,
) -> Result<Option<String>, String> {
    let bytes = raw_body(&request)?;
    let name = header_name(&request);
    if print::strategy() == print::Strategy::Window {
        match open_print_window(&app, &jobs, bytes.clone(), &name) {
            Ok(label) => return Ok(Some(label)),
            // No print window (web view error): the default viewer still prints.
            Err(e) => eprintln!("print window: {e}"),
        }
    }
    let stem = print::temp_file_stem(&name);
    let path: PathBuf = std::env::temp_dir().join(format!("blueoffice-print-{stem}.pdf"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    open_with_system(&path)?;
    Ok(None)
}

/// Window showing the PDF (`boprint` protocol) whose web view print dialog opens once loaded.
fn open_print_window(
    app: &tauri::AppHandle,
    jobs: &PrintJobs,
    pdf: Vec<u8>,
    name: &str,
) -> Result<String, String> {
    let id = jobs.add(pdf)?;
    let label = print::window_label(id);
    let url: tauri::Url = match print::job_url(id, cfg!(windows)).parse() {
        Ok(u) => u,
        Err(e) => {
            jobs.remove(id);
            return Err(format!("{e}"));
        }
    };
    // Windows serves custom schemes on `http://<scheme>.localhost`.
    let target = if cfg!(windows) {
        WebviewUrl::External(url)
    } else {
        WebviewUrl::CustomProtocol(url)
    };
    let dialog = print::dialog_enabled(std::env::var(print::DIALOG_ENV).ok().as_deref());
    let started = Arc::new(AtomicBool::new(false));
    let built = WebviewWindowBuilder::new(app, &label, target)
        .title(print::window_title(name))
        .inner_size(900.0, 1000.0)
        .on_page_load(move |w, payload| {
            if dialog
                && payload.event() == PageLoadEvent::Finished
                && !started.swap(true, Ordering::SeqCst)
            {
                // The PDF viewer initialises after the load event.
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1200));
                    if let Err(e) = w.print() {
                        eprintln!("print dialog: {e}");
                    }
                });
            }
        })
        .build();
    match built {
        Ok(w) => {
            let handle = app.clone();
            w.on_window_event(move |e| {
                if matches!(e, WindowEvent::Destroyed) {
                    handle.state::<PrintJobs>().remove(id);
                }
            });
            Ok(label)
        }
        Err(e) => {
            jobs.remove(id);
            Err(e.to_string())
        }
    }
}

/// `boprint://localhost/<job>.pdf`: the PDF of an open print window.
fn serve_print_job(app: &tauri::AppHandle, path: &str) -> tauri::http::Response<Vec<u8>> {
    let pdf = print::job_of_path(path).and_then(|id| app.state::<PrintJobs>().get(id));
    let builder = tauri::http::Response::builder().header("Cache-Control", "no-store");
    match pdf {
        Some(pdf) => builder.header("Content-Type", "application/pdf").body(pdf),
        None => builder.status(404).body(Vec::new()),
    }
    .unwrap_or_default()
}

/// "Check for updates" (`update.rs`).
#[cfg(feature = "updater")]
#[tauri::command]
async fn update_check(app: tauri::AppHandle) -> update::UpdateStatus {
    let current = app.package_info().version.to_string();
    match updater_for(&app) {
        Ok(None) => update::UpdateStatus::NotConfigured,
        Ok(Some(u)) => match u.check().await {
            Ok(found) => update::status_of(
                &current,
                found.map(|r| (r.version, r.body, r.date.map(|d| d.to_string()))),
            ),
            Err(e) => update::UpdateStatus::Failed {
                message: e.to_string(),
            },
        },
        Err(message) => update::UpdateStatus::Failed { message },
    }
}

#[cfg(not(feature = "updater"))]
#[tauri::command]
fn update_check() -> update::UpdateStatus {
    update::UpdateStatus::Disabled
}

/// Downloads, verifies (minisign signature) and installs the update, then restarts.
#[cfg(feature = "updater")]
#[tauri::command]
async fn update_install(app: tauri::AppHandle) -> Result<(), String> {
    let u = updater_for(&app)?.ok_or("updates are not configured")?;
    let Some(found) = u.check().await.map_err(|e| e.to_string())? else {
        return Err("no update available".into());
    };
    found
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart()
}

#[cfg(not(feature = "updater"))]
#[tauri::command]
fn update_install() -> Result<(), String> {
    Err("updates are disabled in this build".into())
}

/// The updater for the configured (or `BLUEOFFICE_UPDATE_URL`) endpoints; `None` if not configured.
#[cfg(feature = "updater")]
fn updater_for(app: &tauri::AppHandle) -> Result<Option<tauri_plugin_updater::Updater>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let plugins = serde_json::to_value(&app.config().plugins).unwrap_or_default();
    let (key, configured) = update::plugin_config(&plugins);
    let env = update::env_endpoints(std::env::var(update::URL_ENV).ok().as_deref())?;
    if !update::configured(&key, configured, env.len()) {
        return Ok(None);
    }
    let mut b = app.updater_builder();
    if !env.is_empty() {
        let urls = env
            .iter()
            .map(|u| u.parse::<tauri::Url>().map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        b = b.endpoints(urls).map_err(|e| e.to_string())?;
    }
    b.build().map(Some).map_err(|e| e.to_string())
}

/// Installed OS fonts for documents (off with `BLUEOFFICE_SYSTEM_FONTS=0`); the
/// index is cached in the app cache folder (`system-fonts.txt`).
fn system_fonts(app: &tauri::AppHandle) -> Option<SystemFontIndex> {
    if !shell::system_fonts_enabled(std::env::var(shell::SYSTEM_FONTS_ENV).ok().as_deref()) {
        return None;
    }
    let cache = app
        .path()
        .app_cache_dir()
        .ok()
        .map(|d| d.join("system-fonts.txt"));
    Some(SystemFontIndex::scan_cached(
        &SystemFontIndex::default_dirs(),
        cache.as_deref(),
    ))
}

/// Longest wait of a document open for the font scan.
pub(crate) const FONT_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// Set once the installed fonts are indexed (scanned off the main thread at startup).
#[derive(Default)]
pub(crate) struct FontsReady(Mutex<bool>, std::sync::Condvar);

impl FontsReady {
    fn done(&self) {
        if let Ok(mut d) = self.0.lock() {
            *d = true;
        }
        self.1.notify_all();
    }

    pub(crate) fn wait(&self, max: std::time::Duration) {
        if let Ok(g) = self.0.lock() {
            let _ = self.1.wait_timeout_while(g, max, |d| !*d);
        }
    }
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
            LaunchRequest::Link(_) | LaunchRequest::Auth { .. } => None,
        })
}

/// Deep link passed on the command line (`bluedoc://open?host=…&file=…`).
#[tauri::command]
fn startup_link() -> Option<shell::DeepLink> {
    shell::classify_args(std::env::args().skip(1))
        .into_iter()
        .find_map(|r| match r {
            LaunchRequest::Link(l) => Some(l),
            LaunchRequest::File { .. } | LaunchRequest::Auth { .. } => None,
        })
}

/// BlueOffice server for host files (`BLUEOFFICE_SERVER`), if configured.
#[tauri::command]
fn server_url() -> Option<String> {
    std::env::var(shell::SERVER_ENV)
        .ok()
        .and_then(|v| shell::normalize_server_url(&v))
}

/// Whether the process was started for a file or a deep link (else the Start screen shows).
#[tauri::command]
fn has_launch_request() -> bool {
    shell::classify_args(std::env::args().skip(1))
        .iter()
        .any(|r| !matches!(r, LaunchRequest::Auth { .. }))
}

/// Opens a web page (host sign-in) in the system browser.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let url = shell::external_url(&url)?;
    // No shell in between: the URL is a single argument (`&` stays literal).
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("rundll32.exe")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// Offline cache folder of the desktop platform (`OfflineCache`, prompt §3.4):
/// `<app local data>/offline/<hex key>.bin`. Holds offline saves of host files,
/// so it is not the OS cache directory (which may be cleaned).
fn cache_path(app: &tauri::AppHandle, key: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("offline");
    Ok(dir.join(shell::cache_file_name(key)?))
}

#[tauri::command]
async fn cache_get(app: tauri::AppHandle, key: String) -> Result<Response, String> {
    match std::fs::read(cache_path(&app, &key)?) {
        Ok(b) => Ok(Response::new(b)),
        // The web view maps this rejection to `null`.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(CACHE_MISSING.into()),
        Err(e) => Err(e.to_string()),
    }
}

/// Error string of a missing cache entry.
const CACHE_MISSING: &str = "cache:missing";

#[tauri::command]
async fn cache_put(app: tauri::AppHandle, request: Request<'_>) -> Result<(), String> {
    let key = request
        .headers()
        .get("x-cache-key")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .ok_or("missing x-cache-key")?;
    let path = cache_path(&app, &key)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // Write then rename: a crash never leaves a half-written save.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, raw_body(&request)?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn cache_delete(app: tauri::AppHandle, key: String) -> Result<(), String> {
    match std::fs::remove_file(cache_path(&app, &key)?) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
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
            LaunchRequest::Auth { url } => app.emit("auth-callback", url),
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
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol(print::SCHEME, |ctx, req| {
            serve_print_job(ctx.app_handle(), req.uri().path())
        });
    #[cfg(feature = "updater")]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    builder
        .setup(|app| {
            app.manage(EngineState(Mutex::new(Engine::with_bundled_fonts())));
            app.manage(sheet_cmds::SheetState::new());
            app.manage(PrintJobs::default());
            app.manage(FontsReady::default());
            // Index the installed fonts without delaying the window.
            let fonts_app = app.handle().clone();
            std::thread::spawn(move || {
                if let Some(fonts) = system_fonts(&fonts_app) {
                    if let Ok(mut e) = fonts_app.state::<EngineState>().0.lock() {
                        e.set_system_fonts(fonts.clone());
                    }
                    if let Ok(mut s) = fonts_app.state::<sheet_cmds::SheetState>().0.lock() {
                        s.set_system_fonts(fonts);
                    }
                }
                fonts_app.state::<FontsReady>().done();
            });
            // Windows/Linux dev builds: register schemes at runtime (installers do it at install time).
            #[cfg(all(debug_assertions, any(windows, target_os = "linux")))]
            let _ = app.deep_link().register_all();
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                let reqs = event
                    .urls()
                    .iter()
                    .filter_map(|u| {
                        shell::parse_auth_callback(u.as_str())
                            .map(|url| LaunchRequest::Auth { url })
                            .or_else(|| {
                                shell::parse_deep_link(u.as_str())
                                    .ok()
                                    .map(LaunchRequest::Link)
                            })
                    })
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
            engine_new,
            engine_command,
            engine_insert_image,
            engine_media,
            engine_save_docx,
            engine_slide_notes,
            engine_save_pptx,
            engine_mark_saved,
            slide_cmds::engine_slide_command,
            slide_cmds::engine_slide_insert_picture,
            collab_cmds::engine_collab_start,
            collab_cmds::engine_collab_apply,
            collab_cmds::engine_collab_peers,
            collab_cmds::engine_collab_stop,
            collab_cmds::engine_slide_collab_start,
            collab_cmds::engine_slide_collab_apply,
            collab_cmds::engine_slide_collab_peers,
            bundled_fonts,
            pick_file,
            read_file,
            save_file,
            print_pdf,
            startup_file,
            startup_link,
            server_url,
            has_launch_request,
            open_url,
            cache_get,
            cache_put,
            cache_delete,
            open_window,
            update_check,
            update_install,
            sheet_cmds::sheet_open,
            sheet_cmds::sheet_call,
            sheet_cmds::sheet_binary,
            sign_cmds::sign_document,
            sign_cmds::verify_document,
            sign_cmds::sign_defaults,
            sign_cmds::sign_token_certificates,
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
