//! BlueSlide with the self-drawn Rust interface on Windows (ADR-0018, plan N6).
//!
//! `blueslide-native.exe [file.pptx]` opens a presentation in a Win32 window. Slides and
//! thumbnails are drawn by Direct2D/DirectWrite into GPU bitmaps and composed 1:1 into a
//! flip-model swap chain; input is handled in-process by `bo_ui::SlideWindow` (typed engine
//! slide commands, no JSON, no WebView). `BLUEOFFICE_NATIVE_PERF=<file>` appends one JSON
//! line per handled input with its timings. F5 runs the slide show full screen.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod slide_app;
#[cfg(windows)]
#[allow(unsafe_code, dead_code)]
mod win32;

#[cfg(windows)]
fn main() {
    // Layout and parsing recurse deeply on large decks, beyond the main thread's stack.
    let ui = std::thread::Builder::new()
        .name("ui".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            if let Err(e) = slide_app::run() {
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
    eprintln!("blueslide-native runs on Windows only");
}
