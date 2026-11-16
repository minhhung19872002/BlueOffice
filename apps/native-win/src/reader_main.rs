//! BlueReader: the PDF reader of BlueOffice with the self-drawn Rust interface on Windows
//! (ADR-0018, plan N6).
//!
//! `bluereader-native.exe [file.pdf…]` opens PDFs in tabs of one Win32 window. Pages become
//! display lists in-process (`bo-pdfread`) drawn by Direct2D/DirectWrite into GPU page images.
//! `bluereader-native.exe --render in.pdf <page> out.png [zoom%]` writes one page as PNG and
//! `--bench in.pdf` prints open and render timings (no window); `--text in.pdf` prints each
//! page's text as Select All + Copy gives it.
//! `BLUEOFFICE_NATIVE_PERF=<file>` appends one JSON line per handled input with its timings.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod reader_app;
#[cfg(windows)]
// Shared with BlueDoc; some of its helpers are not used here.
#[allow(unsafe_code, dead_code)]
mod win32;

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| {
        a == "--render" || a == "--bench" || a == "--print" || a == "--text" || a == "--search"
    }) {
        if let Err(e) = reader_app::tool(&args) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    // Dates of the comment list ("today" shows the time).
    bo_ui::ReaderWindow::set_local_offset(win32::utc_offset_minutes());
    // Alt / F10 show the ribbon's key tips.
    win32::enable_ribbon_keys();
    // Client area to the window edge, like the reference reader.
    win32::enable_full_client();
    let ui = std::thread::Builder::new()
        .name("ui".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            if let Err(e) = reader_app::run(false) {
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
    eprintln!("bluereader-native runs on Windows only");
}
