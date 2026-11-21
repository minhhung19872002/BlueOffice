//! BlueDoc with the self-drawn Rust interface on Windows (ADR-0018, plan N1).
//!
//! `bluedoc-native.exe [file.docx]` opens a document in a Win32 window. Pages are drawn by
//! Direct2D/DirectWrite into GPU bitmaps and composed 1:1 into a flip-model swap chain; each
//! input is handled in-process by `bo_ui::DocView` and presented at once (no WebView, no IPC).
//! `BLUEOFFICE_NATIVE_PERF=<file>` appends one JSON line per handled input with its timings.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
#[cfg(windows)]
#[allow(unsafe_code)]
mod win32;

#[cfg(windows)]
fn main() {
    // The window, its message loop and the engine share one UI thread. Layout and parsing
    // recurse deeply on large documents, beyond the 1 MiB default stack of the main thread.
    let ui = std::thread::Builder::new()
        .name("ui".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            win32::enable_doc_keys();
            if let Err(e) = app::run() {
                win32::fatal(&e);
            }
        });
    match ui {
        Ok(thread) => {
            let _ = thread.join();
        }
        Err(e) => win32::fatal(&e.to_string()),
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("bluedoc-native runs on Windows only");
}
