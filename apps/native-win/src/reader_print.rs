//! Printing for BlueReader through the Windows GDI print path: the installed printers, and a
//! job that draws each sheet of the plan (`bo_ui::print_plan`: one page fitted / shrunk / at a
//! scale, several pages per sheet, booklet, current view) with every page as a bitmap rendered
//! at the printer's resolution (capped), on one or both sides of the paper.
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, GetDeviceCaps, PatBlt, SetStretchBltMode, StretchDIBits, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, BLACKNESS, DEVMODEW, DIB_RGB_COLORS, DMDUP_HORIZONTAL,
    DMDUP_VERTICAL, DM_DUPLEX, DM_OUT_BUFFER, HALFTONE, HDC, HORZRES, LOGPIXELSX, LOGPIXELSY,
    SRCCOPY, VERTRES,
};
use windows::Win32::Graphics::Printing::{
    ClosePrinter, DocumentPropertiesW, EnumPrintersW, GetDefaultPrinterW, OpenPrinterW,
    PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_HANDLE, PRINTER_INFO_4W,
};
use windows::Win32::Storage::Xps::{EndDoc, EndPage, StartDocW, StartPage, DOCINFOW};

/// Highest resolution pages are rendered at (dots per inch); the printer scales up.
const MAX_DPI: f32 = 300.0;

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
}

/// A device mode of `printer` asking for duplex printing, when the driver gives one.
fn devmode(printer: &[u16], duplex: u8) -> Option<Vec<u64>> {
    if duplex == 0 {
        return None;
    }
    // SAFETY: the printer handle is closed on every path; the buffer is as large as the
    // driver asked and u64-aligned for DEVMODEW.
    unsafe {
        let mut h = PRINTER_HANDLE::default();
        OpenPrinterW(PCWSTR(printer.as_ptr()), &raw mut h, None).ok()?;
        let size = DocumentPropertiesW(None, h, PCWSTR(printer.as_ptr()), None, None, 0);
        let mut out = None;
        if size > 0 {
            let mut buf = vec![0_u64; (size as usize).div_ceil(8)];
            let dm = buf.as_mut_ptr().cast::<DEVMODEW>();
            if DocumentPropertiesW(
                None,
                h,
                PCWSTR(printer.as_ptr()),
                Some(dm),
                None,
                DM_OUT_BUFFER.0,
            ) >= 0
            {
                (*dm).dmFields |= DM_DUPLEX;
                (*dm).dmDuplex = if duplex == 1 {
                    DMDUP_VERTICAL
                } else {
                    DMDUP_HORIZONTAL
                };
                out = Some(buf);
            }
        }
        let _ = ClosePrinter(h);
        out
    }
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
    let dm = devmode(&name, opts.duplex);
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
                    let printed_dpi = (across / page_w.max(1.0) * dpi_x).clamp(36.0, MAX_DPI);
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
