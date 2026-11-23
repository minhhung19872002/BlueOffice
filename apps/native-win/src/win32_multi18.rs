//! BlueDoc round 18 (docs/ui/REPORT.md "Vòng 108"): several windows in one process (View ›
//! New Window shows the same document in a second window), Synchronous Scrolling between
//! BlueDoc windows (also across processes, by a registered window message) and Insert ›
//! Screenshot › Screen Clipping (a frozen picture of the screen over every monitor, a region
//! dragged with the user's own mouse, Esc cancels). A child module of `win32`.
// Used by BlueDoc only; the other binaries include `win32` too.
#![allow(dead_code)]
// Rectangles and pixel loops read best with x / y / w / h.
#![allow(clippy::many_single_char_names, clippy::option_option)]
use super::{ole_drag, scale, with_handler, GetCaretBlinkTimeOr530, BLINK_TIMER, TICK_TIMER};
use std::cell::{Cell, RefCell};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
    EndPaint, GetDC, GetDIBits, InvalidateRect, ReleaseDC, SelectObject, SetDIBits, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, PAINTSTRUCT,
    SRCCOPY,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    GetSystemMetrics, GetWindowRect, IsIconic, IsZoomed, LoadCursorW, PostMessageW,
    PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SetForegroundWindow, SetTimer,
    SetWindowPos, ShowWindow, TranslateMessage, CW_USEDEFAULT, IDC_CROSS, MSG, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_FRAMECHANGED, SWP_NOZORDER,
    SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SW_SHOWNOACTIVATE, WINDOW_EX_STYLE, WM_CLOSE,
    WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT,
    WM_QUIT, WM_RBUTTONDOWN, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW,
    WS_POPUP,
};

thread_local! {
    /// The window whose message is being handled (0 outside a window message).
    static CURRENT: Cell<isize> = const { Cell::new(0) };
    /// Windows of the document class open in this process.
    static OPEN: Cell<usize> = const { Cell::new(0) };
    /// The registered Synchronous Scrolling message (0: not registered yet).
    static SYNC: Cell<u32> = const { Cell::new(0) };
    /// Screen Clipping in progress.
    static CLIP: RefCell<Option<Clip>> = const { RefCell::new(None) };
}

/// The window the current [`super::Handler`] call is for: the window of the message being
/// handled (0 for calls from the message loop itself: `wants_frame`, `frame`).
pub fn current_window() -> isize {
    CURRENT.with(Cell::get)
}

/// The window procedure: remembers the window for [`current_window`], delivers the
/// Synchronous Scrolling message, then runs `inner`.
pub(super) fn dispatch(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    inner: fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
) -> LRESULT {
    let prev = CURRENT.with(|c| c.replace(hwnd.0 as isize));
    let sync = SYNC.with(Cell::get);
    let r = if sync != 0 && msg == sync {
        // Low 32 bits: the distance in 1/16 device pixels, signed.
        let dy = (wparam.0 as u32 as i32) as f32 / 16.0;
        with_handler(|h| h.sync_scroll(dy));
        LRESULT(0)
    } else {
        inner(hwnd, msg, wparam, lparam)
    };
    CURRENT.with(|c| c.set(prev));
    r
}

/// A document window was created.
pub(super) fn window_opened() {
    sync_message();
    OPEN.with(|c| c.set(c.get() + 1));
}

/// A document window was destroyed: `true` when it was the last one.
pub(super) fn window_closed() -> bool {
    OPEN.with(|c| {
        let n = c.get().saturating_sub(1);
        c.set(n);
        n == 0
    })
}

/// The registered message that carries Synchronous Scrolling to another BlueDoc window.
fn sync_message() -> u32 {
    SYNC.with(|c| {
        if c.get() == 0 {
            // SAFETY: the name is a static NUL-terminated string.
            c.set(unsafe { RegisterWindowMessageW(w!("BlueOfficeDocSyncScroll")) });
        }
        c.get()
    })
}

/// Tell BlueDoc window `hwnd` (this process or another) that a linked window scrolled by `dy`
/// device pixels (Synchronous Scrolling).
pub fn post_sync_scroll(hwnd: isize, dy: f32) {
    let msg = sync_message();
    if msg == 0 || hwnd == 0 || !dy.is_finite() {
        return;
    }
    let v = (dy * 16.0).round().clamp(-1e9, 1e9) as i32;
    // SAFETY: posting a message to a window handle; a stale handle fails harmlessly.
    unsafe {
        let _ = PostMessageW(
            Some(HWND(hwnd as *mut _)),
            msg,
            WPARAM(v as u32 as usize),
            LPARAM(0),
        );
    }
}

/// Open another document window (View › New Window) at `place` (left, top, width, height in
/// screen pixels; maximised when `maximize`). Returns the window, its client size and scale.
/// Messages the window gets while it is created reach the handler only if it is free (the
/// caller is usually the handler itself, which sets the window up with the result).
pub fn open_window(
    title: &str,
    place: Option<[i32; 4]>,
    maximize: bool,
) -> Result<(isize, u32, u32, f32), String> {
    let title: Vec<u16> = title.encode_utf16().chain([0]).collect();
    // SAFETY: the class was registered by `run` on this thread; the title outlives the call;
    // every handle belongs to this thread.
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BlueOfficeNativeDoc"),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1280,
            860,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        let [x, y, cx, cy] = place.unwrap_or([60, 60, 1280, 860]);
        let _ = SetWindowPos(hwnd, None, x, y, cx, cy, SWP_FRAMECHANGED | SWP_NOZORDER);
        let quiet = std::env::var_os("BLUEOFFICE_SHOW_NOACTIVATE").is_some();
        let show = if maximize {
            SW_MAXIMIZE
        } else if quiet {
            SW_SHOWNOACTIVATE
        } else {
            SW_SHOW
        };
        let _ = ShowWindow(hwnd, show);
        window_opened();
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        let (w, h) = (
            (rect.right - rect.left).max(1) as u32,
            (rect.bottom - rect.top).max(1) as u32,
        );
        ole_drag::register_if_wanted(hwnd);
        let _ = SetTimer(Some(hwnd), BLINK_TIMER, GetCaretBlinkTimeOr530(), None);
        let _ = SetTimer(Some(hwnd), TICK_TIMER, 100, None);
        Ok((hwnd.0 as isize, w, h, scale(hwnd)))
    }
}

/// The window's rectangle on the screen (left, top, width, height) and whether it is
/// maximised.
pub fn window_place(hwnd: isize) -> ([i32; 4], bool) {
    let mut r = RECT::default();
    // SAFETY: plain queries on a window handle.
    unsafe {
        let h = HWND(hwnd as *mut _);
        let _ = GetWindowRect(h, &raw mut r);
        (
            [r.left, r.top, r.right - r.left, r.bottom - r.top],
            IsZoomed(h).as_bool(),
        )
    }
}

/// The client size of window `hwnd` in pixels (0 × 0 while minimised).
pub fn client_size(hwnd: isize) -> (u32, u32) {
    let mut r = RECT::default();
    // SAFETY: plain query on a window handle.
    unsafe {
        let _ = GetClientRect(HWND(hwnd as *mut _), &mut r);
    }
    (
        (r.right - r.left).max(0) as u32,
        (r.bottom - r.top).max(0) as u32,
    )
}

/// Ask window `hwnd` to close (as its close button does).
pub fn close_window(hwnd: isize) {
    // SAFETY: posting a message to a window handle.
    unsafe {
        let _ = PostMessageW(Some(HWND(hwnd as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

/// The whole virtual screen (every monitor): left, top, width, height in pixels.
fn virtual_screen() -> [i32; 4] {
    // SAFETY: plain metric queries.
    unsafe {
        [
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        ]
    }
}

/// Screen Clipping in progress: the frozen screen and its washed-out copy (memory DCs), the
/// drag.
struct Clip {
    size: (i32, i32),
    shot: (HDC, HBITMAP, HGDIOBJ),
    washed: (HDC, HBITMAP, HGDIOBJ),
    start: Option<(i32, i32)>,
    end: (i32, i32),
    /// `Some(Some(rect))`: a region was chosen; `Some(None)`: cancelled.
    done: Option<Option<[i32; 4]>>,
}

/// Lighten pixels (B, G, R, A) towards white like the reference's frozen screen.
fn wash(px: &mut [u8]) {
    for p in px.as_chunks_mut::<4>().0 {
        for c in &mut p[..3] {
            *c = (f32::from(*c) + (255.0 - f32::from(*c)) * WASH).round() as u8;
        }
    }
}

/// Share of white laid over the frozen screen outside the region.
const WASH: f32 = 0.6;

/// Copy `pixels` (B, G, R, A rows top-down) into a new memory DC compatible with `screen`.
///
/// # Safety
/// `screen` is a live device context.
unsafe fn memory_dc(screen: HDC, w: i32, h: i32, pixels: &[u8]) -> (HDC, HBITMAP, HGDIOBJ) {
    // SAFETY: the caller's DC is live; the buffer holds w × h × 4 bytes as the header says.
    unsafe {
        let dc = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let info = bitmap_info(w, h);
        SetDIBits(
            Some(screen),
            bmp,
            0,
            h as u32,
            pixels.as_ptr().cast(),
            &raw const info,
            DIB_RGB_COLORS,
        );
        let old = SelectObject(dc, bmp.into());
        (dc, bmp, old)
    }
}

fn bitmap_info(w: i32, h: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Free a memory DC made by [`memory_dc`].
///
/// # Safety
/// The parts come from [`memory_dc`] and are not used afterwards.
unsafe fn free_dc((dc, bmp, old): (HDC, HBITMAP, HGDIOBJ)) {
    // SAFETY: as documented.
    unsafe {
        SelectObject(dc, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(dc);
    }
}

/// The pixels (B, G, R, A, top-down) of the virtual screen now.
fn grab_screen(v: [i32; 4]) -> Option<Vec<u8>> {
    let [x, y, w, h] = v;
    if w <= 0 || h <= 0 {
        return None;
    }
    // SAFETY: GDI objects are created, used and released within this block; the buffer is
    // as large as the bitmap info says.
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(dc, bmp.into());
        let ok = BitBlt(dc, 0, 0, w, h, Some(screen), x, y, SRCCOPY | CAPTUREBLT).is_ok();
        let mut info = bitmap_info(w, h);
        let mut px = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(
            dc,
            bmp,
            0,
            h as u32,
            Some(px.as_mut_ptr().cast()),
            &raw mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(dc, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        (ok && lines > 0).then_some(px)
    }
}

/// Normalised region of a drag (left, top, width, height in overlay pixels).
fn region(a: (i32, i32), b: (i32, i32)) -> [i32; 4] {
    let (l, r) = (a.0.min(b.0), a.0.max(b.0));
    let (t, bt) = (a.1.min(b.1), a.1.max(b.1));
    [l, t, r - l, bt - t]
}

/// Insert › Screenshot › Screen Clipping: hide `own` windows, freeze the screen (every
/// monitor) under a white wash, let the user drag a region with the mouse (Esc or the right
/// button cancels), bring the windows back and return the region's pixels (R, G, B, A).
pub fn screen_clip(own: &[isize]) -> Option<(u32, u32, Vec<u8>)> {
    // SAFETY: minimising and restoring this thread's own windows.
    unsafe {
        for &h in own {
            let _ = ShowWindow(HWND(h as *mut _), SW_MINIMIZE);
        }
    }
    // The minimise animation ends before the screen is taken.
    std::thread::sleep(std::time::Duration::from_millis(450));
    let v = virtual_screen();
    let result = grab_screen(v).and_then(|px| pick_region(v, &px));
    // SAFETY: restoring this thread's own windows (maximised ones come back maximised).
    unsafe {
        for &h in own.iter().rev() {
            let hwnd = HWND(h as *mut _);
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
        }
        if let Some(&h) = own.first() {
            let _ = SetForegroundWindow(HWND(h as *mut _));
        }
    }
    result
}

/// Show the frozen screen `px` over the virtual screen `v` and run until a region is chosen.
fn pick_region(v: [i32; 4], px: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let [vx, vy, w, h] = v;
    let mut washed = px.to_vec();
    wash(&mut washed);
    // SAFETY: the overlay window, its class and the memory DCs belong to this thread and are
    // released before returning; the message loop only dispatches this thread's messages.
    unsafe {
        let instance = GetModuleHandleW(None).ok()?;
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(clip_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            lpszClassName: w!("BlueOfficeScreenClip"),
            ..Default::default()
        };
        // Registered once per process (a second registration fails harmlessly).
        let _ = RegisterClassExW(&class);
        let screen = GetDC(None);
        let shot = memory_dc(screen, w, h, px);
        let wash_dc = memory_dc(screen, w, h, &washed);
        ReleaseDC(None, screen);
        CLIP.with(|c| {
            *c.borrow_mut() = Some(Clip {
                size: (w, h),
                shot,
                washed: wash_dc,
                start: None,
                end: (0, 0),
                done: None,
            });
        });
        let overlay = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("BlueOfficeScreenClip"),
            w!("Screen Clipping"),
            WS_POPUP,
            vx,
            vy,
            w,
            h,
            None,
            None,
            Some(instance.into()),
            None,
        );
        let chosen = if let Ok(overlay) = overlay {
            let _ = ShowWindow(overlay, SW_SHOW);
            let _ = SetForegroundWindow(overlay);
            let mut msg = MSG::default();
            let mut quit = false;
            loop {
                if CLIP
                    .with(|c| c.borrow().as_ref().and_then(|c| c.done))
                    .is_some()
                {
                    break;
                }
                if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    quit = msg.message == WM_QUIT;
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = DestroyWindow(overlay);
            if quit {
                PostQuitMessage(0);
            }
            CLIP.with(|c| c.borrow().as_ref().and_then(|c| c.done).flatten())
        } else {
            None
        };
        if let Some(c) = CLIP.with(|c| c.borrow_mut().take()) {
            free_dc(c.shot);
            free_dc(c.washed);
        }
        let [l, t, rw, rh] = chosen?;
        crop(px, w, [l, t, rw, rh])
    }
}

/// The `rect` part of `px` (B, G, R, A rows `width` wide) as R, G, B, A.
fn crop(px: &[u8], width: i32, rect: [i32; 4]) -> Option<(u32, u32, Vec<u8>)> {
    let [l, t, w, h] = rect;
    if w <= 0 || h <= 0 || l < 0 || t < 0 || l + w > width {
        return None;
    }
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in t..t + h {
        let start = ((row * width + l) * 4) as usize;
        let line = px.get(start..start + (w * 4) as usize)?;
        for p in line.as_chunks::<4>().0 {
            out.extend_from_slice(&[p[2], p[1], p[0], 255]);
        }
    }
    Some((w as u32, h as u32, out))
}

fn lparam_point(lparam: LPARAM) -> (i32, i32) {
    let v = lparam.0 as u32;
    (
        i32::from(v as u16 as i16),
        i32::from((v >> 16) as u16 as i16),
    )
}

/// Window procedure of the Screen Clipping overlay.
extern "system" fn clip_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let finish = |done: Option<[i32; 4]>| {
        CLIP.with(|c| {
            if let Some(c) = c.borrow_mut().as_mut() {
                c.done = Some(done);
            }
        });
    };
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: painting this window between BeginPaint and EndPaint from the memory DCs
            // of the clip state, which live while the overlay does.
            unsafe {
                let dc = BeginPaint(hwnd, &mut ps);
                CLIP.with(|c| {
                    if let Some(c) = c.borrow().as_ref() {
                        paint_clip(dc, c);
                    }
                });
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let p = lparam_point(lparam);
            CLIP.with(|c| {
                if let Some(c) = c.borrow_mut().as_mut() {
                    c.start = Some(p);
                    c.end = p;
                }
            });
            // SAFETY: capturing the mouse for this thread's overlay.
            unsafe {
                let _ = SetCapture(hwnd);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let p = lparam_point(lparam);
            let moved = CLIP.with(|c| {
                c.borrow_mut().as_mut().is_some_and(|c| {
                    if c.start.is_some() {
                        c.end = p;
                        true
                    } else {
                        false
                    }
                })
            });
            if moved {
                // SAFETY: invalidating this thread's overlay.
                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            // SAFETY: releasing this thread's capture.
            unsafe {
                let _ = ReleaseCapture();
            }
            let p = lparam_point(lparam);
            let rect = CLIP.with(|c| {
                c.borrow()
                    .as_ref()
                    .and_then(|c| c.start.map(|s| region(s, p)))
            });
            // A click without a drag chooses nothing (as the reference: the screen stays).
            match rect {
                Some(r) if r[2] >= 3 && r[3] >= 3 => finish(Some(r)),
                _ => {
                    CLIP.with(|c| {
                        if let Some(c) = c.borrow_mut().as_mut() {
                            c.start = None;
                        }
                    });
                }
            }
            LRESULT(0)
        }
        // Esc (VK_ESCAPE) or the right button cancels.
        WM_KEYDOWN if wparam.0 == 0x1B => {
            finish(None);
            LRESULT(0)
        }
        WM_RBUTTONDOWN | WM_CLOSE | WM_DESTROY => {
            finish(None);
            LRESULT(0)
        }
        // SAFETY: default processing of the overlay's own message.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Draw the overlay: the washed screen with the dragged region in its true colours and a
/// thin outline, composed off screen first (no flicker while dragging).
///
/// # Safety
/// `dc` is the overlay's paint DC; the clip's memory DCs are live.
unsafe fn paint_clip(dc: HDC, c: &Clip) {
    let (w, h) = c.size;
    // SAFETY: as documented; the back buffer is released before returning.
    unsafe {
        let back = CreateCompatibleDC(Some(dc));
        let bmp = CreateCompatibleBitmap(dc, w, h);
        let old = SelectObject(back, bmp.into());
        let _ = BitBlt(back, 0, 0, w, h, Some(c.washed.0), 0, 0, SRCCOPY);
        if let Some(s) = c.start {
            let [l, t, rw, rh] = region(s, c.end);
            if rw > 0 && rh > 0 {
                let _ = BitBlt(back, l, t, rw, rh, Some(c.shot.0), l, t, SRCCOPY);
            }
        }
        let _ = BitBlt(dc, 0, 0, w, h, Some(back), 0, 0, SRCCOPY);
        SelectObject(back, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(back);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_and_crops() {
        assert_eq!(region((10, 20), (4, 30)), [4, 20, 6, 10]);
        // 3 × 2 pixels, B G R A.
        let px: Vec<u8> = (0..6).flat_map(|i| [i, 100 + i, 200 + i, 0]).collect();
        let (w, h, out) = crop(&px, 3, [1, 0, 2, 2]).expect("crop");
        assert_eq!((w, h), (2, 2));
        assert_eq!(&out[..4], &[201, 101, 1, 255]);
        assert_eq!(&out[12..16], &[205, 105, 5, 255]);
        assert!(crop(&px, 3, [2, 0, 2, 1]).is_none());
        let mut p = vec![0u8, 255, 100, 255];
        wash(&mut p);
        assert_eq!(&p[..3], &[153, 255, 193]);
    }
}
