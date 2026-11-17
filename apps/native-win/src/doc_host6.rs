//! BlueDoc host parts of parity round 6: application preferences and the recent-documents
//! list (files under `%LOCALAPPDATA%\BlueOffice`, overridable for scripted captures with
//! `BLUEOFFICE_PREFS` / `BLUEOFFICE_RECENT`), printing a document through the Windows GDI
//! print path shared with BlueReader, and saving a copy as another file type.
use bo_engine::Engine;
use std::path::{Path, PathBuf};

// GDI printing (Win32 calls confined to this child module, like win32.rs).
#[path = "reader_print.rs"]
#[allow(unsafe_code, dead_code)]
pub mod print;

fn app_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("BlueOffice"))
}

/// Where the preferences live.
pub fn prefs_path() -> Option<PathBuf> {
    std::env::var_os("BLUEOFFICE_PREFS")
        .map(PathBuf::from)
        .or_else(|| app_dir().map(|d| d.join("doc-prefs.txt")))
}

/// Where the recent list lives.
fn recent_path() -> Option<PathBuf> {
    std::env::var_os("BLUEOFFICE_RECENT")
        .map(PathBuf::from)
        .or_else(|| app_dir().map(|d| d.join("doc-recent.txt")))
}

/// The stored preferences (empty when none).
pub fn read_prefs() -> String {
    prefs_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
}

/// Store the preferences.
pub fn write_prefs(text: &str) {
    if let Some(p) = prefs_path() {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, text);
    }
}

/// `locale=` of the stored preferences.
pub fn prefs_locale(text: &str) -> Option<String> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == "locale")
        .map(|(_, v)| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

/// The recent list: (path, when) most recent first.
pub fn read_recent() -> Vec<(String, String)> {
    recent_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (p, w) = l.split_once('\t').unwrap_or((l, ""));
            (!p.trim().is_empty()).then(|| (p.to_owned(), w.to_owned()))
        })
        .collect()
}

/// Put `path` first in the recent list (at most 50 entries) and return the list.
pub fn touch_recent(path: &Path) -> Vec<(String, String)> {
    let full = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let long = full.to_string_lossy();
    let s = long.strip_prefix(r"\\?\").unwrap_or(&long).to_owned();
    let mut list = read_recent();
    list.retain(|(p, _)| !p.eq_ignore_ascii_case(&s));
    list.insert(0, (s, today()));
    list.truncate(50);
    if let Some(p) = recent_path() {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let text = list.iter().fold(String::new(), |mut t, (p, w)| {
            t.push_str(p);
            t.push('\t');
            t.push_str(w);
            t.push('\n');
            t
        });
        let _ = std::fs::write(p, text);
    }
    list
}

/// Today's local date as `yyyy-mm-dd`.
fn today() -> String {
    let offset = i64::from(crate::win32::utc_offset_minutes()) * 60;
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
        + offset;
    civil(secs.div_euclid(86_400))
}

/// `yyyy-mm-dd` of a day count since 1970-01-01.
fn civil(days: i64) -> String {
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{}-{m:02}-{d:02}", if m <= 2 { y + 1 } else { y })
}

/// File › Info facts the engine does not keep: total editing time (`docProps/app.xml`
/// TotalTime, minutes), whether the file is in compatibility mode (a `.doc`, or a `.docx`
/// whose `w:compatSetting compatibilityMode` is below 15 or missing — [MS-DOCX] 2.6.1), and
/// the file system's created / modified dates (local `yyyy-mm-dd hh:mm`) used when
/// `docProps/core.xml` has none.
pub fn file_details(path: &Path) -> bo_ui::FileDetails {
    let mut d = bo_ui::FileDetails::default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if let Ok(meta) = std::fs::metadata(path) {
        d.created = meta.created().ok().map(local_stamp).unwrap_or_default();
        d.modified = meta.modified().ok().map(local_stamp).unwrap_or_default();
    }
    if ext == "doc" {
        d.compat = true;
        return d;
    }
    let Ok(bytes) = std::fs::read(path) else {
        return d;
    };
    let Ok(pkg) = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()) else {
        return d;
    };
    let text = |part: &str| {
        pkg.part(part)
            .map(|p| String::from_utf8_lossy(&p.data).into_owned())
            .unwrap_or_default()
    };
    let app = text("/docProps/app.xml");
    d.edit_minutes = app
        .split("<TotalTime>")
        .nth(1)
        .and_then(|r| r.split('<').next())
        .and_then(|v| v.trim().parse().ok());
    if matches!(ext.as_str(), "docx" | "docm" | "dotx" | "dotm") {
        let settings = text("/word/settings.xml");
        let mode = settings
            .find("compatibilityMode")
            .and_then(|i| settings[i..].split("w:val=\"").nth(1))
            .and_then(|r| r.split('"').next())
            .and_then(|v| v.parse::<u32>().ok());
        d.compat = mode.is_none_or(|m| m < 15);
    }
    d
}

/// A file time as local `yyyy-mm-dd hh:mm`.
fn local_stamp(t: std::time::SystemTime) -> String {
    let offset = i64::from(crate::win32::utc_offset_minutes()) * 60;
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
        + offset;
    let (date, rem) = (civil(secs.div_euclid(86_400)), secs.rem_euclid(86_400));
    format!("{date} {:02}:{:02}", rem / 3600, rem % 3600 / 60)
}

/// File size in bytes.
pub fn file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}

/// Installed printers and the default one's index.
pub fn printers() -> (Vec<String>, usize) {
    let (names, default) = print::printers();
    let i = default
        .and_then(|d| names.iter().position(|n| *n == d))
        .unwrap_or(0);
    (names, i)
}

/// Decode a PNG to opaque RGBA.
#[allow(clippy::many_single_char_names)]
fn decode_png(bytes: &[u8]) -> Option<print::PrintPage> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let n = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return None,
    };
    let mut pixels = Vec::with_capacity(buf.len() / n * 4);
    for px in buf.chunks_exact(n) {
        let (r, g, b, a) = match n {
            1 => (px[0], px[0], px[0], 255),
            2 => (px[0], px[0], px[0], px[1]),
            3 => (px[0], px[1], px[2], 255),
            _ => (px[0], px[1], px[2], px[3]),
        };
        // On white paper.
        let mix = |c: u8| ((u16::from(c) * u16::from(a) + 255 * (255 - u16::from(a))) / 255) as u8;
        pixels.extend_from_slice(&[mix(r), mix(g), mix(b), 255]);
    }
    Some(print::PrintPage {
        width: info.width,
        height: info.height,
        pixels,
    })
}

/// Print `pages` of the document open in `engine` (File › Print, `--print`): fitted to the
/// printable area, several per sheet when asked, copies collated or not, one or both sides;
/// `output` writes the job to a file (print-to-file printers). Returns sheets printed.
pub fn print_document(
    engine: &Engine,
    sizes: &[(f32, f32)],
    job: &bo_ui::PrintJob,
    title: &str,
    output: Option<String>,
) -> Result<usize, String> {
    let size_of = |i: usize| sizes.get(i).copied().unwrap_or((612.0, 792.0));
    let pages: Vec<usize> = job
        .pages
        .iter()
        .copied()
        .filter(|&i| i < sizes.len())
        .collect();
    if pages.is_empty() {
        return Err("no pages to print".into());
    }
    let opts = bo_ui::PlanOptions {
        scaling: bo_ui::PrintScaling::Fit,
        auto_rotate: true,
        auto_center: true,
        layout: if job.per_sheet > 1 {
            bo_ui::PrintLayout::Multiple {
                per_sheet: job.per_sheet,
                order: bo_ui::PageOrder::Horizontal,
                border: false,
                margin: 0.0,
                custom: None,
            }
        } else {
            bo_ui::PrintLayout::Size
        },
        copies: job.copies.max(1),
        collate: job.collate,
    };
    let plan = |area: (f32, f32)| bo_ui::print_plan(&pages, &size_of, None, area, &opts);
    let mut render = |i: usize, scale: f32| -> Option<print::PrintPage> {
        decode_png(&engine.render_png(i, scale).ok()?)
    };
    let landscape = pages.first().is_some_and(|&i| {
        let (w, h) = size_of(i);
        w > h
    });
    let options = print::Options {
        grayscale: false,
        duplex: job.duplex.min(2),
        output,
        landscape,
        max_dpi: 300.0,
        devmode: None,
        paper: pages.first().map(|&i| {
            let (w, h) = size_of(i);
            (w.min(h), w.max(h))
        }),
    };
    print::print(&job.printer, title, &plan, &size_of, &options, &mut render)
}

/// Save a copy of the document as `ext` (File › Export › Change File Type): templates as a
/// package variant, the rest through the engine's exports. Returns the bytes to write.
pub fn export_as(engine: &mut Engine, ext: &str) -> Result<Vec<u8>, String> {
    match ext {
        "pdf" => engine.export_pdf().map_err(|e| e.to_string()),
        "dotx" | "docx" => {
            engine.set_save_format(ext).map_err(|e| e.to_string())?;
            let bytes = engine.save_docx().map_err(|e| e.to_string());
            // The open document keeps its own type.
            let _ = engine.set_save_format("docx");
            bytes
        }
        other => engine.export_as(other).map_err(|e| e.to_string()),
    }
}

/// File dialog filter of an export type.
pub fn filter_of(ext: &str) -> (&'static str, &'static str) {
    match ext {
        "dotx" => ("BlueDoc template (*.dotx)", "*.dotx"),
        "pdf" => ("PDF (*.pdf)", "*.pdf"),
        "html" => ("Web Page (*.html)", "*.html"),
        "mht" => ("Single File Web Page (*.mht)", "*.mht"),
        "xml" => ("XML Document (*.xml)", "*.xml"),
        "epub" => ("EPUB (*.epub)", "*.epub"),
        _ => ("Document (*.docx)", "*.docx"),
    }
}

/// A new message in the mail program (`mailto:`), naming the document and where it is.
pub fn email(subject: &str, body: &str) {
    let enc = |s: &str| {
        s.bytes().fold(String::new(), |mut o, b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                o.push(char::from(b));
            } else {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                o.push('%');
                o.push(char::from(HEX[usize::from(b >> 4)]));
                o.push(char::from(HEX[usize::from(b & 15)]));
            }
            o
        })
    };
    let url = format!("mailto:?subject={}&body={}", enc(subject), enc(body));
    let _ = std::process::Command::new("rundll32.exe")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .spawn();
}

/// Show `path` selected in the system file manager.
pub fn open_location(path: &str) {
    let _ = std::process::Command::new("explorer.exe")
        .arg(format!("/select,{path}"))
        .spawn();
}

#[cfg(test)]
mod tests {
    #[test]
    fn locale_from_prefs() {
        assert_eq!(
            super::prefs_locale("a=1\nlocale=vi-VN\n").as_deref(),
            Some("vi-VN")
        );
        assert_eq!(super::prefs_locale("locale=\n"), None);
    }

    #[test]
    fn today_is_a_date() {
        let d = super::today();
        assert_eq!(d.len(), 10);
        assert!(d.starts_with("20"));
    }
}
