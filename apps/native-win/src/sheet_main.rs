//! BlueSheet with the self-drawn Rust interface on Windows (ADR-0018, plan N6).
//!
//! `bluesheet-native.exe [file.xlsx]` opens a workbook in a Win32 window. The grid is drawn by
//! Direct2D/DirectWrite into one GPU bitmap from the in-process `SheetEngine`'s display list
//! and redrawn only when cells, scroll or size change; headers, selection and the cell editor
//! are drawn over it each frame (no WebView, no IPC).
//! `BLUEOFFICE_NATIVE_PERF=<file>` appends one JSON line per handled input with its timings.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod sheet_app;
#[cfg(windows)]
// Shared with BlueDoc; some of its helpers are not used here.
#[allow(unsafe_code, dead_code)]
mod win32;

#[cfg(windows)]
fn main() {
    // Same arrangement as BlueDoc: one UI thread with a large stack.
    let ui = std::thread::Builder::new()
        .name("ui".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            win32::enable_sheet_keys();
            bo_ui::SheetWindow::set_utc_offset(win32::utc_offset_minutes());
            if let Err(e) = sheet_app::run() {
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
    eprintln!("bluesheet-native runs on Windows only");
}
