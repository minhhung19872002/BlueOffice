//! BlueEditor: the PDF editor of BlueOffice with the self-drawn Rust interface on Windows
//! (ADR-0018, plan N6). It is BlueReader's window and host with editing: page organisation,
//! text and object editing, forms, redaction, signing, protection and conversion.
//!
//! `blueeditor-native.exe [file.pdf…]` opens PDFs in tabs of one Win32 window.
//! `blueeditor-native.exe --render in.pdf <page> out.png [zoom%]` and `--bench in.pdf` work as
//! for BlueReader. `BLUEOFFICE_NATIVE_PERF=<file>` appends one JSON line per handled input.
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
    if args
        .first()
        .is_some_and(|a| a == "--render" || a == "--bench")
    {
        if let Err(e) = reader_app::tool(&args) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    // The client area reaches the window edge: the reference editor's frame has no invisible
    // resize borders, like the reference reader's (PrintWindow: title colour from x = 0).
    win32::enable_full_client();
    let ui = std::thread::Builder::new()
        .name("ui".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            if let Err(e) = reader_app::run(true) {
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
    eprintln!("blueeditor-native runs on Windows only");
}
