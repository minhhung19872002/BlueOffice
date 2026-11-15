//! Printing for BlueReader through the Windows GDI print path: the installed printers, and a
//! job that draws each page as a bitmap rendered at the printer's resolution (capped) into
//! the printable area, fitted or at actual size, centred, turned to the paper's orientation.
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, GetDeviceCaps, SetStretchBltMode, StretchDIBits, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HALFTONE, HDC, HORZRES, LOGPIXELSX, LOGPIXELSY,
    SRCCOPY, VERTRES,
};
use windows::Win32::Graphics::Printing::{
    EnumPrintersW, GetDefaultPrinterW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL,
    PRINTER_INFO_4W,
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

/// How a page is sized on paper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scaling {
    /// Grow or shrink to the printable area.
    Fit,
    /// Shrink pages larger than the printable area only.
    Shrink,
    /// 100 %.
    Actual,
}

/// How a job prints.
#[allow(clippy::struct_excessive_bools)]
pub struct Options {
    /// Copies.
    pub copies: u32,
    /// Each copy all pages (else each page `copies` times).
    pub collate: bool,
    /// Page sizing.
    pub scaling: Scaling,
    /// Turn pages to the paper's orientation.
    pub auto_rotate: bool,
    /// Centre pages on the paper (else top-left of the printable area).
    pub auto_center: bool,
    /// Shades of grey.
    pub grayscale: bool,
}

/// Print `pages` (index, size in points) on `printer`. `render` draws page `index` at a
/// scale in device pixels per point. Returns sheets printed.
#[allow(clippy::chunks_exact_to_as_chunks, clippy::too_many_lines)] // `as_chunks_mut`: MSRV.
pub fn print(
    printer: &str,
    title: &str,
    pages: &[(usize, (f32, f32))],
    opts: &Options,
    render: &mut dyn FnMut(usize, f32) -> Option<PrintPage>,
) -> Result<usize, String> {
    let name = wide(printer);
    let driver = wide("WINSPOOL");
    let doc_name = wide(title);
    // SAFETY: the device context is created and deleted here; every buffer passed to GDI
    // lives for the duration of the call; the job is ended (or aborted by EndDoc) on all paths.
    unsafe {
        let hdc: HDC = CreateDCW(
            PCWSTR(driver.as_ptr()),
            PCWSTR(name.as_ptr()),
            PCWSTR::null(),
            None,
        );
        if hdc.is_invalid() {
            return Err(format!("cannot open printer {printer}"));
        }
        let dpi_x = GetDeviceCaps(Some(hdc), LOGPIXELSX).max(72) as f32;
        let dpi_y = GetDeviceCaps(Some(hdc), LOGPIXELSY).max(72) as f32;
        let area = (
            GetDeviceCaps(Some(hdc), HORZRES).max(1) as f32,
            GetDeviceCaps(Some(hdc), VERTRES).max(1) as f32,
        );
        let info = DOCINFOW {
            cbSize: std::mem::size_of::<DOCINFOW>() as i32,
            lpszDocName: PCWSTR(doc_name.as_ptr()),
            ..Default::default()
        };
        if StartDocW(hdc, &raw const info) <= 0 {
            let _ = DeleteDC(hdc);
            return Err("the printer refused the job".into());
        }
        SetStretchBltMode(hdc, HALFTONE);
        let mut printed = 0;
        let result = (|| -> Result<(), String> {
            // Collated: every page per copy; else each page all its copies in a row.
            let n = opts.copies.max(1) as usize;
            let order: Vec<&(usize, (f32, f32))> = if opts.collate {
                (0..n).flat_map(|_| pages.iter()).collect()
            } else {
                pages
                    .iter()
                    .flat_map(|p| std::iter::repeat_n(p, n))
                    .collect()
            };
            {
                for &&(index, (w_pt, h_pt)) in &order {
                    if StartPage(hdc) <= 0 {
                        return Err("the printer refused a page".into());
                    }
                    // Turn the page when its orientation differs from the paper's.
                    let turn = opts.auto_rotate
                        && (w_pt > h_pt) != (area.0 > area.1)
                        && (w_pt - h_pt).abs() > 1.0;
                    let (pw, ph) = if turn { (h_pt, w_pt) } else { (w_pt, h_pt) };
                    // Natural size in printer pixels.
                    let natural = (pw / 72.0 * dpi_x, ph / 72.0 * dpi_y);
                    let fit = (area.0 / natural.0).min(area.1 / natural.1);
                    let k = match opts.scaling {
                        Scaling::Fit => fit,
                        Scaling::Shrink => fit.min(1.0),
                        Scaling::Actual => 1.0,
                    };
                    let dest = (natural.0 * k, natural.1 * k);
                    // Render resolution: the printed size, at most MAX_DPI.
                    let printed_dpi = (dpi_x * k).clamp(36.0, MAX_DPI);
                    let scale = printed_dpi / 72.0;
                    if let Some(img) = render(index, scale) {
                        let img = if turn { rotate_cw(&img) } else { img };
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
                        let (x, y) = if opts.auto_center {
                            (
                                ((area.0 - dest.0) / 2.0).round() as i32,
                                ((area.1 - dest.1) / 2.0).round() as i32,
                            )
                        } else {
                            (0, 0)
                        };
                        StretchDIBits(
                            hdc,
                            x,
                            y,
                            dest.0.round() as i32,
                            dest.1.round() as i32,
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
                    if EndPage(hdc) <= 0 {
                        return Err("the printer refused a page".into());
                    }
                    printed += 1;
                }
            }
            Ok(())
        })();
        EndDoc(hdc);
        let _ = DeleteDC(hdc);
        result.map(|()| printed)
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
