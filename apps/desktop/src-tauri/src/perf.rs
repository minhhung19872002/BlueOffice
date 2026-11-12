//! Performance log of the desktop app (`BLUEOFFICE_PERF_LOG=<file>`, docs/perf/BASELINE.md).
//!
//! Off unless the variable is set: then every event is one JSON line `{"t": <ms since the Unix
//! epoch>, "event": …}` appended to the file — process start, the web view's marks (shell
//! visible, first page painted, editor ready; sent by the probe script below through
//! [`perf_mark`]) and the time the native engine spent in each command. `scripts/perf-desktop.mjs`
//! starts the app with it and reads the file back. The probe script is only injected when the
//! log is on, so a normal start pays one environment lookup.

use std::fs::File;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Probe injected into every web view when the log is on (marks, IPC and paint timings).
const PROBE_JS: &str = include_str!("perf-probe.js");

fn log() -> Option<&'static Mutex<File>> {
    static LOG: OnceLock<Option<Mutex<File>>> = OnceLock::new();
    LOG.get_or_init(|| {
        let path = std::env::var_os("BLUEOFFICE_PERF_LOG").filter(|p| !p.is_empty())?;
        File::options()
            .create(true)
            .append(true)
            .open(path)
            .ok()
            .map(Mutex::new)
    })
    .as_ref()
}

/// Whether the log is on.
pub fn enabled() -> bool {
    log().is_some()
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// Appends `event` (with the fields of `data`, a JSON object) to the log.
pub fn mark(event: &str, data: Option<serde_json::Value>) {
    let Some(file) = log() else { return };
    let mut line = serde_json::json!({ "t": now_ms(), "event": event });
    if let (Some(serde_json::Value::Object(extra)), Some(obj)) = (data, line.as_object_mut()) {
        for (k, v) in extra {
            obj.entry(k).or_insert(v);
        }
    }
    if let Ok(mut f) = file.lock() {
        let _ = writeln!(f, "{line}");
    }
}

/// Runs `f`, logging how long it took as an `engine` event of command `cmd`.
pub fn timed<T>(cmd: &str, f: impl FnOnce() -> T) -> T {
    if !enabled() {
        return f();
    }
    let start = Instant::now();
    let out = f();
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    mark("engine", Some(serde_json::json!({ "cmd": cmd, "ms": ms })));
    out
}

/// A mark from the web view's probe script.
#[tauri::command]
pub fn perf_mark(event: String, data: Option<serde_json::Value>) {
    mark(&format!("web:{event}"), data);
}

/// The plugin injecting the probe script, when the log is on.
pub fn plugin<R: tauri::Runtime>() -> Option<tauri::plugin::TauriPlugin<R>> {
    enabled().then(|| {
        // BLUEOFFICE_PERF_PROBE=<file.js>: another probe without rebuilding (harness work).
        let probe = std::env::var_os("BLUEOFFICE_PERF_PROBE")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_else(|| PROBE_JS.to_owned());
        tauri::plugin::Builder::new("bo-perf")
            .js_init_script(probe)
            .build()
    })
}
