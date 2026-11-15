//! A picture on the clipboard (BlueReader's SnapShot): a device-independent bitmap
//! (`CF_DIB`), Win32 calls confined to this child module like win32.rs.
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_RGB};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

/// Standard clipboard format of a DIB.
const CF_DIB: u32 = 8;

/// Put `width × height` RGBA pixels (top row first) on the clipboard as a bitmap.
#[allow(clippy::chunks_exact_to_as_chunks)] // `as_chunks`: MSRV.
pub fn set_image(width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let (w, h) = (width as usize, height as usize);
    if rgba.len() < w * h * 4 || w == 0 || h == 0 {
        return Err("empty picture".into());
    }
    let header = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width as i32,
        // Positive height: rows bottom up.
        biHeight: height as i32,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: (w * h * 4) as u32,
        ..Default::default()
    };
    let head = std::mem::size_of::<BITMAPINFOHEADER>();
    let mut bytes = vec![0_u8; head + w * h * 4];
    // SAFETY: BITMAPINFOHEADER is plain data copied into the first bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(
            std::ptr::from_ref(&header).cast::<u8>(),
            bytes.as_mut_ptr(),
            head,
        );
    }
    for y in 0..h {
        let src = &rgba[(h - 1 - y) * w * 4..(h - y) * w * 4];
        let dst = &mut bytes[head + y * w * 4..head + (y + 1) * w * 4];
        for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = 255;
        }
    }
    // SAFETY: the clipboard is opened for this thread and closed on every path; the global
    // block is filled while locked and then owned by the clipboard.
    unsafe {
        OpenClipboard(None).map_err(|e| e.to_string())?;
        let _ = EmptyClipboard();
        let result = (|| -> Result<(), String> {
            let mem = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(|e| e.to_string())?;
            let p = GlobalLock(mem).cast::<u8>();
            if p.is_null() {
                return Err("no memory".into());
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
            let _ = GlobalUnlock(mem);
            SetClipboardData(CF_DIB, Some(HANDLE(mem.0))).map_err(|e| e.to_string())?;
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}
