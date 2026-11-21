//! Printing for BlueReader through the Windows GDI print path: the installed printers, and a
//! job that draws each sheet of the plan (`bo_ui::print_plan`: one page fitted / shrunk / at a
//! scale, several pages per sheet, booklet, current view) with every page as a bitmap rendered
//! at the printer's resolution (capped), on one or both sides of the paper.
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    CreateDCW, CreateICW, DeleteDC, GetDeviceCaps, PatBlt, SetBkMode, SetStretchBltMode,
    StretchDIBits, TextOutW, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLACKNESS, DEVMODEW,
    DIB_RGB_COLORS, DMDUP_HORIZONTAL, DMDUP_VERTICAL, DMORIENT_LANDSCAPE, DMORIENT_PORTRAIT,
    DM_DUPLEX, DM_IN_BUFFER, DM_IN_PROMPT, DM_ORIENTATION, DM_OUT_BUFFER, DM_PAPERLENGTH,
    DM_PAPERSIZE, DM_PAPERWIDTH, HALFTONE, HDC, HORZRES, LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT,
    PHYSICALOFFSETX, PHYSICALOFFSETY, PHYSICALWIDTH, SRCCOPY, TRANSPARENT, VERTRES,
};
use windows::Win32::Graphics::Printing::{
    ClosePrinter, DocumentPropertiesW, EnumPrintersW, GetDefaultPrinterW, OpenPrinterW,
    PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_HANDLE, PRINTER_INFO_4W,
};
use windows::Win32::Storage::Xps::{EndDoc, EndPage, StartDocW, StartPage, DOCINFOW};

/// IDOK of the driver's settings dialog.
const IDOK: i32 = 1;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Installed printers and the default one's name.
pub fn printers() -> (Vec<String>, Option<String>) {
    let mut names = Vec::new();
    // SAFETY: the first call asks for the size; the buffer then holds `count` PRINTER_INFO_4W
    // records followed by the strings they point into, read while the buffer lives.
    unsafe {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let (mut needed, mut count) = (0_u32, 0_u32);
        let _ = EnumPrintersW(
            flags,
            PCWSTR::null(),
            4,
            None,
            &raw mut needed,
            &raw mut count,
        );
        if needed > 0 {
            // u64 storage keeps the records aligned.
            let mut buf = vec![0_u64; (needed as usize).div_ceil(8)];
            let bytes =
                std::slice::from_raw_parts_mut(buf.as_mut_ptr().cast::<u8>(), needed as usize);
            if EnumPrintersW(
                flags,
                PCWSTR::null(),
                4,
                Some(bytes),
                &raw mut needed,
                &raw mut count,
            )
            .is_ok()
            {
                let infos = std::slice::from_raw_parts(
                    buf.as_ptr().cast::<PRINTER_INFO_4W>(),
                    count as usize,
                );
                for info in infos {
                    if !info.pPrinterName.is_null() {
                        if let Ok(n) = info.pPrinterName.to_string() {
                            names.push(n);
                        }
                    }
                }
            }
        }
        let mut len = 0_u32;
        let _ = GetDefaultPrinterW(None, &raw mut len);
        let default = if len > 0 {
            let mut buf = vec![0_u16; len as usize];
            if GetDefaultPrinterW(Some(PWSTR(buf.as_mut_ptr())), &raw mut len).as_bool() {
                let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
                Some(String::from_utf16_lossy(&buf[..end]))
            } else {
                None
            }
        } else {
            None
        };
        (names, default)
    }
}

/// One page to print: premultiplied RGBA pixels and the page size in points.
pub struct PrintPage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` RGBA bytes (opaque).
    pub pixels: Vec<u8>,
}

/// How a job prints (where pages go comes from the sheet plan).
pub struct Options {
    /// Shades of grey.
    pub grayscale: bool,
    /// Both sides: 0 one side, 1 flip on the long edge, 2 on the short edge.
    pub duplex: u8,
    /// Write the job to this file instead of the printer port (print-to-file printers such
    /// as the system's PDF printer).
    pub output: Option<String>,
    /// Paper turned landscape.
    pub landscape: bool,
    /// Highest resolution pages are rendered at (dots per inch); the printer scales up.
    pub max_dpi: f32,
    /// Driver settings chosen with Properties (a DEVMODEW buffer), the base of the job's.
    pub devmode: Option<Vec<u64>>,
    /// Paper of this size (points, portrait), when the PDF page size chooses the paper.
    pub paper: Option<(f32, f32)>,
}

/// The driver settings of `printer`: `base` merged in when given; DEVMODEW in a u64 buffer.
/// `prompt` shows the driver's dialog owned by `hwnd` and returns `None` when it is cancelled.
fn driver_devmode(
    printer: &[u16],
    base: Option<&[u64]>,
    prompt: Option<isize>,
) -> Option<Vec<u64>> {
    // SAFETY: the printer handle is closed on every path; the buffer is as large as the
    // driver asked and u64-aligned for DEVMODEW; `base` is a DEVMODEW the driver gave earlier.
    unsafe {
        let mut h = PRINTER_HANDLE::default();
        OpenPrinterW(PCWSTR(printer.as_ptr()), &raw mut h, None).ok()?;
        let owner = prompt.map(|w| HWND(w as *mut _));
        let size = DocumentPropertiesW(owner, h, PCWSTR(printer.as_ptr()), None, None, 0);
        let mut out = None;
        if size > 0 {
            let mut buf = vec![0_u64; (size as usize).div_ceil(8)];
            let dm = buf.as_mut_ptr().cast::<DEVMODEW>();
            let input = base.map(|b| b.as_ptr().cast::<DEVMODEW>());
            let mut mode = DM_OUT_BUFFER.0;
            if input.is_some() {
                mode |= DM_IN_BUFFER.0;
            }
            if prompt.is_some() {
                mode |= DM_IN_PROMPT.0;
            }
            let r = DocumentPropertiesW(owner, h, PCWSTR(printer.as_ptr()), Some(dm), input, mode);
            if (prompt.is_some() && r == IDOK) || (prompt.is_none() && r >= 0) {
                out = Some(buf);
            }
        }
        let _ = ClosePrinter(h);
        out
    }
}

/// Show `printer`'s own settings dialog (the Print dialog's Properties) over window `hwnd`,
/// starting from `base`; the chosen settings, or `None` when cancelled.
pub fn properties(hwnd: isize, printer: &str, base: Option<&[u64]>) -> Option<Vec<u64>> {
    driver_devmode(&wide(printer), base, Some(hwnd))
}

/// Paper size and printable area of `printer` (with its chosen settings), in points.
pub fn paper(printer: &str, devmode: Option<&[u64]>) -> Option<bo_ui::Paper> {
    let name = wide(printer);
    let driver = wide("WINSPOOL");
    // SAFETY: an information context only answers GetDeviceCaps; it is deleted here.
    unsafe {
        let ic = CreateICW(
            PCWSTR(driver.as_ptr()),
            PCWSTR(name.as_ptr()),
            PCWSTR::null(),
            devmode.map(|b| b.as_ptr().cast::<DEVMODEW>()),
        );
        if ic.is_invalid() {
            return None;
        }
        let cap = |i| GetDeviceCaps(Some(ic), i) as f32;
        let (dx, dy) = (cap(LOGPIXELSX).max(72.0), cap(LOGPIXELSY).max(72.0));
        let pt = |v: f32, d: f32| v / d * 72.0;
        let p = bo_ui::Paper {
            size: (pt(cap(PHYSICALWIDTH), dx), pt(cap(PHYSICALHEIGHT), dy)),
            area: [
                pt(cap(PHYSICALOFFSETX), dx),
                pt(cap(PHYSICALOFFSETY), dy),
                pt(cap(HORZRES), dx),
                pt(cap(VERTRES), dy),
            ],
        };
        let _ = DeleteDC(ic);
        (p.size.0 > 0.0 && p.size.1 > 0.0).then_some(p)
    }
}

/// The driver's default paper (`DMPAPER_*` code, 0 when none is named) of `printer` and
/// whether it can print on both sides (`DC_DUPLEX`).
#[allow(dead_code)] // BlueSheet's File › Print.
pub fn defaults(printer: &str) -> Option<(u32, bool)> {
    let buf = driver_devmode(&wide(printer), None, None)?;
    // SAFETY: `buf` holds a DEVMODEW written by the driver (u64-aligned); only read here.
    unsafe {
        let dm = buf.as_ptr().cast::<DEVMODEW>();
        let fields = (*dm).dmFields;
        let paper = if (fields & DM_PAPERSIZE).0 != 0 {
            u32::try_from((*dm).Anonymous1.Anonymous1.dmPaperSize).unwrap_or(0)
        } else {
            0
        };
        // Both sides: what the driver reports as a capability (DC_DUPLEX), not only that its
        // device mode has the field.
        let name = wide(printer);
        let duplex = windows::Win32::Storage::Xps::DeviceCapabilitiesW(
            PCWSTR(name.as_ptr()),
            PCWSTR::null(),
            windows::Win32::Storage::Xps::DC_DUPLEX,
            None,
            None,
        ) == 1;
        Some((paper, duplex))
    }
}

/// The job's device mode of `printer`: the chosen driver settings with the dialog's
/// orientation and both-sides choice, when the driver gives one.
fn devmode(printer: &[u16], opts: &Options) -> Option<Vec<u64>> {
    let mut buf = driver_devmode(printer, opts.devmode.as_deref(), None)?;
    // SAFETY: `buf` holds a DEVMODEW written by the driver (u64-aligned).
    unsafe {
        let dm = buf.as_mut_ptr().cast::<DEVMODEW>();
        (*dm).dmFields |= DM_ORIENTATION;
        (*dm).Anonymous1.Anonymous1.dmOrientation = if opts.landscape {
            DMORIENT_LANDSCAPE as i16
        } else {
            DMORIENT_PORTRAIT as i16
        };
        if let Some((w, h)) = opts.paper {
            // Tenths of a millimetre; no named size.
            let tenth = |v: f32| (v / 72.0 * 254.0).round() as i16;
            (*dm).dmFields |= DM_PAPERSIZE | DM_PAPERWIDTH | DM_PAPERLENGTH;
            (*dm).Anonymous1.Anonymous1.dmPaperSize = 0;
            (*dm).Anonymous1.Anonymous1.dmPaperWidth = tenth(w.min(h));
            (*dm).Anonymous1.Anonymous1.dmPaperLength = tenth(w.max(h));
        }
        if opts.duplex != 0 {
            (*dm).dmFields |= DM_DUPLEX;
            (*dm).dmDuplex = if opts.duplex == 1 {
                DMDUP_VERTICAL
            } else {
                DMDUP_HORIZONTAL
            };
        }
    }
    Some(buf)
}

/// Print the sheets `plan` gives for the printable area (points) on `printer`. `size_of` is a
/// page's size in points; `render` draws page `index` at a scale in device pixels per point.
/// Returns sheets printed.
#[allow(clippy::chunks_exact_to_as_chunks, clippy::too_many_lines)] // `as_chunks_mut`: MSRV.
pub fn print(
    printer: &str,
    title: &str,
    plan: &dyn Fn((f32, f32)) -> Vec<bo_ui::Sheet>,
    size_of: &dyn Fn(usize) -> (f32, f32),
    opts: &Options,
    render: &mut dyn FnMut(usize, f32) -> Option<PrintPage>,
) -> Result<usize, String> {
    let name = wide(printer);
    let driver = wide("WINSPOOL");
    let doc_name = wide(title);
    let output = opts.output.as_deref().map(wide);
    let dm = devmode(&name, opts);
    // SAFETY: the device context is created and deleted here; every buffer passed to GDI
    // lives for the duration of the call; the job is ended (or aborted by EndDoc) on all paths.
    unsafe {
        let hdc: HDC = CreateDCW(
            PCWSTR(driver.as_ptr()),
            PCWSTR(name.as_ptr()),
            PCWSTR::null(),
            dm.as_ref().map(|b| b.as_ptr().cast::<DEVMODEW>()),
        );
        if hdc.is_invalid() {
            return Err(format!("cannot open printer {printer}"));
        }
        let dpi_x = GetDeviceCaps(Some(hdc), LOGPIXELSX).max(72) as f32;
        let dpi_y = GetDeviceCaps(Some(hdc), LOGPIXELSY).max(72) as f32;
        let area = (
            GetDeviceCaps(Some(hdc), HORZRES).max(1) as f32 / dpi_x * 72.0,
            GetDeviceCaps(Some(hdc), VERTRES).max(1) as f32 / dpi_y * 72.0,
        );
        let sheets = plan(area);
        let info = DOCINFOW {
            cbSize: std::mem::size_of::<DOCINFOW>() as i32,
            lpszDocName: PCWSTR(doc_name.as_ptr()),
            lpszOutput: output
                .as_ref()
                .map_or_else(PCWSTR::null, |o| PCWSTR(o.as_ptr())),
            ..Default::default()
        };
        if StartDocW(hdc, &raw const info) <= 0 {
            let _ = DeleteDC(hdc);
            return Err("the printer refused the job".into());
        }
        SetStretchBltMode(hdc, HALFTONE);
        let mut printed = 0;
        let result = (|| -> Result<(), String> {
            for sheet in &sheets {
                if StartPage(hdc) <= 0 {
                    return Err("the printer refused a page".into());
                }
                for pl in &sheet.items {
                    // Destination in printer pixels.
                    let d = [
                        (pl.dest[0] / 72.0 * dpi_x).round() as i32,
                        (pl.dest[1] / 72.0 * dpi_y).round() as i32,
                        (pl.dest[2] / 72.0 * dpi_x).round() as i32,
                        (pl.dest[3] / 72.0 * dpi_y).round() as i32,
                    ];
                    // Points of the page across the destination (before turning).
                    let across = if pl.turn { pl.dest[3] } else { pl.dest[2] };
                    let page_w = pl.clip.map_or_else(|| size_of(pl.page).0, |c| c[2]);
                    // Render resolution: the printed size, at most MAX_DPI.
                    let printed_dpi =
                        (across / page_w.max(1.0) * dpi_x).clamp(36.0, opts.max_dpi.max(36.0));
                    let scale = printed_dpi / 72.0;
                    if let Some(img) = render(pl.page, scale) {
                        let img = match pl.clip {
                            Some(c) => crop(&img, c, scale),
                            None => img,
                        };
                        let img = if pl.turn { rotate_cw(&img) } else { img };
                        let mut bgra = img.pixels;
                        for px in bgra.chunks_exact_mut(4) {
                            if opts.grayscale {
                                // Rec. 601 luma.
                                let y = ((u32::from(px[0]) * 299
                                    + u32::from(px[1]) * 587
                                    + u32::from(px[2]) * 114)
                                    / 1000) as u8;
                                px[0] = y;
                                px[1] = y;
                                px[2] = y;
                            } else {
                                px.swap(0, 2);
                            }
                        }
                        let bmi = BITMAPINFO {
                            bmiHeader: BITMAPINFOHEADER {
                                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                                biWidth: img.width as i32,
                                biHeight: -(img.height as i32),
                                biPlanes: 1,
                                biBitCount: 32,
                                biCompression: BI_RGB.0,
                                ..Default::default()
                            },
                            ..Default::default()
                        };
                        StretchDIBits(
                            hdc,
                            d[0],
                            d[1],
                            d[2],
                            d[3],
                            0,
                            0,
                            img.width as i32,
                            img.height as i32,
                            Some(bgra.as_ptr().cast()),
                            &raw const bmi,
                            DIB_RGB_COLORS,
                            SRCCOPY,
                        );
                    }
                    if pl.cut_marks {
                        // Corner marks a quarter inch long, about 0.5 pt thick.
                        let t = (dpi_x / 144.0).round().max(1.0) as i32;
                        let m = (dpi_x / 4.0) as i32;
                        for (x, y, sx, sy) in [
                            (d[0], d[1], 1, 1),
                            (d[0] + d[2], d[1], -1, 1),
                            (d[0], d[1] + d[3], 1, -1),
                            (d[0] + d[2], d[1] + d[3], -1, -1),
                        ] {
                            let hx = if sx > 0 { x } else { x - m };
                            let vy = if sy > 0 { y } else { y - m };
                            let _ = PatBlt(hdc, hx, y - t / 2, m, t, BLACKNESS);
                            let _ = PatBlt(hdc, x - t / 2, vy, t, m, BLACKNESS);
                        }
                    }
                    if let Some((row, col)) = pl.tile {
                        let label = wide(&format!("{} ({row}, {col})", pl.page + 1));
                        SetBkMode(hdc, TRANSPARENT);
                        let n = label.len().saturating_sub(1);
                        let _ = TextOutW(hdc, d[0] + 4, d[1] + 4, &label[..n]);
                    }
                    if pl.border {
                        // A hairline frame (about 0.5 pt).
                        let t = (dpi_x / 144.0).round().max(1.0) as i32;
                        let _ = PatBlt(hdc, d[0], d[1], d[2], t, BLACKNESS);
                        let _ = PatBlt(hdc, d[0], d[1] + d[3] - t, d[2], t, BLACKNESS);
                        let _ = PatBlt(hdc, d[0], d[1], t, d[3], BLACKNESS);
                        let _ = PatBlt(hdc, d[0] + d[2] - t, d[1], t, d[3], BLACKNESS);
                    }
                }
                if EndPage(hdc) <= 0 {
                    return Err("the printer refused a page".into());
                }
                printed += 1;
            }
            Ok(())
        })();
        EndDoc(hdc);
        let _ = DeleteDC(hdc);
        result.map(|()| printed)
    }
}

/// The `clip` part (page points) of an image rendered at `scale` pixels per point.
fn crop(img: &PrintPage, clip: [f32; 4], scale: f32) -> PrintPage {
    let (w, h) = (img.width as usize, img.height as usize);
    let x0 = ((clip[0] * scale).round().max(0.0) as usize).min(w);
    let y0 = ((clip[1] * scale).round().max(0.0) as usize).min(h);
    let x1 = (((clip[0] + clip[2]) * scale).round().max(0.0) as usize).clamp(x0, w);
    let y1 = (((clip[1] + clip[3]) * scale).round().max(0.0) as usize).clamp(y0, h);
    let (cw, ch) = ((x1 - x0).max(1), (y1 - y0).max(1));
    let mut out = vec![255_u8; cw * ch * 4];
    let n = (x1 - x0) * 4;
    for y in 0..y1 - y0 {
        let s = ((y0 + y) * w + x0) * 4;
        out[y * cw * 4..y * cw * 4 + n].copy_from_slice(&img.pixels[s..s + n]);
    }
    PrintPage {
        width: cw as u32,
        height: ch as u32,
        pixels: out,
    }
}

/// The page turned a quarter clockwise.
fn rotate_cw(img: &PrintPage) -> PrintPage {
    let (w, h) = (img.width as usize, img.height as usize);
    let mut out = vec![0_u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            // (x, y) → (h - 1 - y, x) in a h × w image.
            let (nx, ny) = (h - 1 - y, x);
            let s = (y * w + x) * 4;
            let d = (ny * h + nx) * 4;
            out[d..d + 4].copy_from_slice(&img.pixels[s..s + 4]);
        }
    }
    PrintPage {
        width: img.height,
        height: img.width,
        pixels: out,
    }
}
