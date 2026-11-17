//! Printer status for BlueDoc's File › Print (round 6): the spooler's status bits and the
//! number of jobs waiting (GetPrinter level 2), read only — nothing is ever sent. Win32
//! calls stay in this module, like `win32.rs`.
use windows::core::PCWSTR;
use windows::Win32::Graphics::Printing::{
    ClosePrinter, GetPrinterW, OpenPrinterW, PRINTER_HANDLE, PRINTER_INFO_2W,
};

/// Status bits of `PRINTER_INFO_2.Status` that mean an error (paper jam, out of paper,
/// paper problem, error, offline, door open, no toner, user intervention, not available).
const ERROR_BITS: u32 = 0x0000_0002
    | 0x0000_0008
    | 0x0000_0010
    | 0x0000_0040
    | 0x0000_0080
    | 0x0000_1000
    | 0x0004_0000
    | 0x0010_0000
    | 0x0040_0000
    | 0x0080_0000;
/// `PRINTER_STATUS_OFFLINE`.
const OFFLINE: u32 = 0x0000_0080;

/// (status bits, jobs queued) of `printer`, `None` when it cannot be opened.
pub fn status(printer: &str) -> Option<(u32, u32)> {
    let name: Vec<u16> = printer.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: the handle is closed on every path; the u64 buffer is as large as the spooler
    // asked and aligned for PRINTER_INFO_2W, read while it lives.
    unsafe {
        let mut h = PRINTER_HANDLE::default();
        OpenPrinterW(PCWSTR(name.as_ptr()), &raw mut h, None).ok()?;
        let mut needed = 0_u32;
        let _ = GetPrinterW(h, 2, None, &raw mut needed);
        let mut out = None;
        if needed as usize >= std::mem::size_of::<PRINTER_INFO_2W>() {
            let mut buf = vec![0_u64; (needed as usize).div_ceil(8)];
            let bytes =
                std::slice::from_raw_parts_mut(buf.as_mut_ptr().cast::<u8>(), needed as usize);
            if GetPrinterW(h, 2, Some(bytes), &raw mut needed).is_ok() {
                let info = &*buf.as_ptr().cast::<PRINTER_INFO_2W>();
                out = Some((info.Status, info.cJobs));
            }
        }
        let _ = ClosePrinter(h);
        out
    }
}

/// (error, offline, jobs) of `printer` for the File › Print status line.
pub fn state(printer: &str) -> (bool, bool, u32) {
    status(printer).map_or((false, false, 0), |(bits, jobs)| {
        (bits & ERROR_BITS != 0, bits & OFFLINE != 0, jobs)
    })
}
