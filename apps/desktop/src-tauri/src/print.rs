//! Printing on the desktop (prompt §3.4 `print`: "desktop = hộp thoại in OS").
//!
//! The engine's PDF is shown in a print window of the app (served by the
//! `boprint` protocol, never written to disk) and the web view's print is
//! started on it: WebView2 (Windows) opens its print dialog — printer, pages,
//! copies, plus "print using the system dialog" — and WKWebView (macOS) the
//! system print panel. The window stays open as a preview. Where a web view
//! cannot show PDFs (WebKitGTK on Linux), or the window cannot be created, the
//! PDF opens in the default viewer, whose print command the user takes.
//! `docs/deploy/desktop-print.md` describes the behaviour per OS.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

/// URI scheme serving the PDFs of open print windows.
pub const SCHEME: &str = "boprint";
/// `BLUEOFFICE_PRINT_DIALOG=0` opens the print window without the dialog (E2E tests).
pub const DIALOG_ENV: &str = "BLUEOFFICE_PRINT_DIALOG";
/// Largest PDF kept for a print window.
const MAX_PDF: usize = 512 * 1024 * 1024;

/// PDFs of the open print windows, by job id.
#[derive(Default)]
pub struct PrintJobs(Mutex<HashMap<u32, Vec<u8>>>);

static NEXT_JOB: AtomicU32 = AtomicU32::new(1);

impl PrintJobs {
    /// Stores a PDF; returns its job id.
    pub fn add(&self, pdf: Vec<u8>) -> Result<u32, String> {
        if pdf.len() > MAX_PDF || !pdf.starts_with(b"%PDF-") {
            return Err("not a PDF".into());
        }
        let id = NEXT_JOB.fetch_add(1, Ordering::Relaxed);
        self.0.lock().map_err(|e| e.to_string())?.insert(id, pdf);
        Ok(id)
    }

    /// PDF of a job (served while its window is open).
    pub fn get(&self, id: u32) -> Option<Vec<u8>> {
        self.0.lock().ok()?.get(&id).cloned()
    }

    pub fn remove(&self, id: u32) {
        if let Ok(mut m) = self.0.lock() {
            m.remove(&id);
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.lock().map_or(0, |m| m.len())
    }
}

/// How a platform prints a PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Print window + the web view's print dialog.
    Window,
    /// Hand the PDF to the default viewer.
    Viewer,
}

/// Strategy of the current OS.
pub fn strategy() -> Strategy {
    if cfg!(any(windows, target_os = "macos")) {
        Strategy::Window
    } else {
        Strategy::Viewer
    }
}

/// Is the dialog opened (`BLUEOFFICE_PRINT_DIALOG` unset, or anything but `0`/`false`/`off`)?
pub fn dialog_enabled(env: Option<&str>) -> bool {
    !matches!(
        env.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("0" | "false" | "off" | "no")
    )
}

/// Window label of a job.
pub fn window_label(id: u32) -> String {
    format!("print-{id}")
}

/// URL of a job's PDF. Windows web views reach custom schemes as
/// `http://<scheme>.localhost/…`, the other platforms as `<scheme>://localhost/…`.
pub fn job_url(id: u32, windows: bool) -> String {
    if windows {
        format!("http://{SCHEME}.localhost/{id}.pdf")
    } else {
        format!("{SCHEME}://localhost/{id}.pdf")
    }
}

/// Job id of a request path (`/12.pdf`).
pub fn job_of_path(path: &str) -> Option<u32> {
    path.trim_start_matches('/')
        .strip_suffix(".pdf")?
        .parse()
        .ok()
}

/// Window title: the document name with a `.pdf` extension.
pub fn window_title(doc: &str) -> String {
    let name: String = doc.chars().filter(|c| !c.is_control()).take(200).collect();
    let name = name.trim();
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    format!("{}.pdf", if stem.is_empty() { "document" } else { stem })
}

/// File name (no path parts) for the PDF handed to the default viewer.
pub fn temp_file_stem(doc: &str) -> String {
    doc.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(100)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jobs_hold_pdfs_until_removed() {
        let jobs = PrintJobs::default();
        assert!(jobs.add(b"<html>".to_vec()).is_err());
        let a = jobs.add(b"%PDF-1.7 a".to_vec()).unwrap();
        let b = jobs.add(b"%PDF-1.7 b".to_vec()).unwrap();
        assert_ne!(a, b);
        assert_eq!(jobs.get(a).as_deref(), Some(&b"%PDF-1.7 a"[..]));
        jobs.remove(a);
        assert!(jobs.get(a).is_none());
        assert_eq!(jobs.len(), 1);
    }

    #[test]
    fn urls_labels_and_paths_round_trip() {
        assert_eq!(job_url(7, true), "http://boprint.localhost/7.pdf");
        assert_eq!(job_url(7, false), "boprint://localhost/7.pdf");
        assert_eq!(job_of_path("/7.pdf"), Some(7));
        assert_eq!(job_of_path("/../7.pdf"), None);
        assert_eq!(job_of_path("/7.html"), None);
        assert_eq!(window_label(42), "print-42");
    }

    #[test]
    fn titles_and_dialog_switch() {
        assert_eq!(window_title("Báo cáo.docx"), "Báo cáo.pdf");
        assert_eq!(window_title("  "), "document.pdf");
        assert_eq!(window_title("a\u{7}b"), "ab.pdf");
        assert_eq!(temp_file_stem("Biên bản 1/2"), "Biên_bản_1_2");
        assert!(dialog_enabled(None));
        assert!(dialog_enabled(Some("1")));
        assert!(!dialog_enabled(Some(" OFF ")));
        assert!(!dialog_enabled(Some("0")));
        let s = strategy();
        if cfg!(windows) {
            assert_eq!(s, Strategy::Window);
        }
    }
}
