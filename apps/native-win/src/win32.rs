//! Win32 window, message loop and input translation (the crate's only `unsafe` code).
// The generated bindings take references for pointer parameters.
#![allow(clippy::borrow_as_ptr)]
use bo_ui::{Input, Key, Mods, NcHit, Request};
use std::cell::{Cell, RefCell};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::Graphics::Gdi::{ValidateRect, HBRUSH};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, GetSystemMetricsForDpi, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::Ime::{
    ImmGetContext, ImmReleaseContext, ImmSetCompositionWindow, CFS_POINT, COMPOSITIONFORM,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetDoubleClickTime, GetKeyState, ReleaseCapture, SetCapture, VIRTUAL_KEY, VK_BACK, VK_CONTROL,
    VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN,
    VK_RIGHT, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageTime, GetMessageW,
    LoadCursorW, MessageBoxW, PostQuitMessage, RegisterClassExW, SetTimer, SetWindowPos,
    ShowWindow, SystemParametersInfoW, TranslateMessage, CS_DBLCLKS, CW_USEDEFAULT, IDC_IBEAM,
    MB_ICONERROR, MB_OK, MSG, SPI_GETWHEELSCROLLLINES, SWP_NOACTIVATE, SWP_NOZORDER,
    SW_SHOWMAXIMIZED, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WINDOW_EX_STYLE, WM_CHAR, WM_DESTROY,
    WM_DPICHANGED, WM_ERASEBKGND, WM_IME_COMPOSITION, WM_IME_STARTCOMPOSITION, WM_KEYDOWN,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_SIZE,
    WM_TIMER, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    IsZoomed, PostMessageW, HTCAPTION, HTCLIENT, HTCLOSE, HTMAXBUTTON, HTMINBUTTON, HTTOP,
    HTTOPLEFT, HTTOPRIGHT, NCCALCSIZE_PARAMS, SM_CXPADDEDBORDER, SM_CXSIZEFRAME, SM_CYSIZEFRAME,
    SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WM_CLOSE,
    WM_NCCALCSIZE, WM_NCHITTEST, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE,
};

/// What the window reports to the application.
pub trait Handler {
    /// The window exists: create the swap chain for `hwnd` at its client size and scale.
    fn created(&mut self, hwnd: isize, width: u32, height: u32, scale: f32) -> Result<(), String>;
    /// An input event; `age_ms` is how long it waited in the message queue.
    fn input(&mut self, input: Input, age_ms: u32);
    /// The client area changed size.
    fn resized(&mut self, width: u32, height: u32);
    /// The window must be redrawn.
    fn paint(&mut self);
    /// Caret rectangle (x, y, width, height in client pixels), for IME windows.
    fn caret(&self) -> Option<[f32; 4]>;
    /// What a client position is for the window manager (title bar parts).
    fn nc_hit(&self, x: f32, y: f32) -> NcHit;
    /// Window operations asked for by the interface since the last call.
    fn window_requests(&mut self) -> Vec<Request>;
}

thread_local! {
    static HANDLER: RefCell<Option<Box<dyn Handler>>> = RefCell::new(None);
    /// Window button pressed in the non-client area (HTMINBUTTON / HTMAXBUTTON / HTCLOSE).
    static NC_PRESSED: Cell<u32> = const { Cell::new(0) };
    /// A WM_MOUSELEAVE / WM_NCMOUSELEAVE notification is requested.
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static HIGH_SURROGATE: Cell<u16> = const { Cell::new(0) };
    static LAST_DOUBLE: Cell<(u32, i32, i32)> = const { Cell::new((0, 0, 0)) };
}

/// WM_MOUSELEAVE (WinUser.h); not exported by this module of the bindings.
const WM_MOUSELEAVE: u32 = 0x02A3;
const BLINK_TIMER: usize = 1;
const TICK_TIMER: usize = 2;

/// Show an error and exit.
pub fn fatal(message: &str) {
    let text: Vec<u16> = message.encode_utf16().chain([0]).collect();
    // SAFETY: both strings are NUL-terminated and outlive the call.
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            w!("BlueDoc"),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn with_handler<R>(f: impl FnOnce(&mut dyn Handler) -> R) -> Option<R> {
    HANDLER.with(|h| {
        // A handler re-entered from inside itself (nested message) is skipped.
        let mut h = h.try_borrow_mut().ok()?;
        h.as_mut().map(|h| f(h.as_mut()))
    })
}

fn age() -> u32 {
    // SAFETY: plain queries of the current message's time and the tick count.
    unsafe { GetTickCount().wrapping_sub(GetMessageTime() as u32) }
}

fn mods() -> Mods {
    // SAFETY: GetKeyState only reads the thread's keyboard state.
    let down = |vk: VIRTUAL_KEY| unsafe { GetKeyState(i32::from(vk.0)) } < 0;
    Mods {
        shift: down(VK_SHIFT),
        ctrl: down(VK_CONTROL),
        alt: down(VK_MENU),
    }
}

fn scale(hwnd: HWND) -> f32 {
    // SAFETY: `hwnd` is a live window of this thread.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        1.0
    } else {
        dpi as f32 / 96.0
    }
}

fn point(lparam: LPARAM) -> (f32, f32) {
    let v = lparam.0 as u32;
    (
        f32::from(v as u16 as i16),
        f32::from((v >> 16) as u16 as i16),
    )
}

fn key(vk: u16, m: Mods) -> Option<Key> {
    let k = VIRTUAL_KEY(vk);
    Some(match k {
        VK_LEFT => Key::Left,
        VK_RIGHT => Key::Right,
        VK_UP => Key::Up,
        VK_DOWN => Key::Down,
        VK_HOME => Key::Home,
        VK_END => Key::End,
        VK_PRIOR => Key::PageUp,
        VK_NEXT => Key::PageDown,
        VK_BACK => Key::Backspace,
        VK_DELETE => Key::Delete,
        VK_RETURN => Key::Enter,
        VK_TAB => Key::Tab,
        VK_ESCAPE => Key::Escape,
        // Ctrl+letter shortcuts; plain letters arrive as text (WM_CHAR).
        _ if m.ctrl && (0x41..=0x5A).contains(&vk) => {
            Key::Letter(char::from(vk as u8).to_ascii_lowercase())
        }
        _ => return None,
    })
}

fn place_ime(hwnd: HWND) {
    let Some(Some([x, y, _, h])) = with_handler(|h| h.caret()) else {
        return;
    };
    // SAFETY: the input context belongs to `hwnd` and is released after use.
    unsafe {
        let ctx = ImmGetContext(hwnd);
        if ctx.is_invalid() {
            return;
        }
        let form = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: POINT {
                x: x as i32,
                y: (y + h) as i32,
            },
            ..Default::default()
        };
        let _ = ImmSetCompositionWindow(ctx, &form);
        let _ = ImmReleaseContext(hwnd, ctx);
    }
}

fn input(hwnd: HWND, i: Input) {
    let age = age();
    with_handler(|h| h.input(i, age));
    let requests = with_handler(|h| h.window_requests()).unwrap_or_default();
    for r in requests {
        window_request(hwnd, &r);
    }
}

fn window_request(hwnd: HWND, r: &Request) {
    // SAFETY: plain window operations on this thread's window.
    unsafe {
        match r {
            Request::Minimize => {
                let _ = ShowWindow(hwnd, SW_MINIMIZE);
            }
            Request::ToggleMaximize => {
                let cmd = if IsZoomed(hwnd).as_bool() {
                    SW_RESTORE
                } else {
                    SW_MAXIMIZE
                };
                let _ = ShowWindow(hwnd, cmd);
            }
            Request::Close => {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            Request::Save | Request::SetClipboard { .. } | Request::Paste => {}
        }
    }
}

/// Resize-frame thickness at the window's DPI (the part of the frame kept as non-client).
fn frame(hwnd: HWND) -> (i32, i32) {
    // SAFETY: plain metric queries.
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        let pad = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
        (
            GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) + pad,
            GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + pad,
        )
    }
}

fn track_leave(hwnd: HWND, nonclient: bool) {
    if TRACKING.with(Cell::get) {
        return;
    }
    let mut flags = TME_LEAVE;
    if nonclient {
        flags |= TME_NONCLIENT;
    }
    let mut t = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: flags,
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    // SAFETY: the struct is fully initialised and outlives the call.
    if unsafe { TrackMouseEvent(&mut t) }.is_ok() {
        TRACKING.with(|c| c.set(true));
    }
}

fn client_point(hwnd: HWND, lparam: LPARAM) -> (f32, f32) {
    let v = lparam.0 as u32;
    let mut pt = POINT {
        x: i32::from(v as u16 as i16),
        y: i32::from((v >> 16) as u16 as i16),
    };
    // SAFETY: converts a screen point of this thread's window.
    unsafe {
        let _ = ScreenToClient(hwnd, &mut pt);
    }
    (pt.x as f32, pt.y as f32)
}

#[allow(clippy::too_many_lines)] // one arm per window message
extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CHAR => {
            let unit = wparam.0 as u16;
            let text = match unit {
                0xD800..=0xDBFF => {
                    HIGH_SURROGATE.with(|s| s.set(unit));
                    return LRESULT(0);
                }
                0xDC00..=0xDFFF => {
                    let high = HIGH_SURROGATE.with(|s| s.replace(0));
                    String::from_utf16(&[high, unit]).ok()
                }
                // Control characters (Backspace, Enter, Ctrl+letters) come as WM_KEYDOWN.
                0..=0x1F | 0x7F => None,
                _ => String::from_utf16(&[unit]).ok(),
            };
            if let Some(text) = text {
                input(hwnd, Input::Text(text));
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let m = mods();
            match key(wparam.0 as u16, m) {
                Some(k) => {
                    input(hwnd, Input::Key(k, m));
                    LRESULT(0)
                }
                // SAFETY: default processing of this window's own message.
                None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            }
        }
        WM_MOUSEWHEEL => {
            let delta = f32::from((wparam.0 >> 16) as u16 as i16);
            let mut lines = 3_u32;
            // SAFETY: the out pointer refers to a live u32.
            unsafe {
                let _ = SystemParametersInfoW(
                    SPI_GETWHEELSCROLLLINES,
                    0,
                    Some((&raw mut lines).cast()),
                    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
                );
            }
            if mods().ctrl {
                // Ctrl + wheel zooms, one 10 % step per notch.
                input(hwnd, Input::Zoom(delta / 120.0));
                return LRESULT(0);
            }
            // About one text line (20 logical px) per wheel line.
            let px = -delta / 120.0 * lines as f32 * 20.0 * scale(hwnd);
            input(hwnd, Input::Wheel(px));
            LRESULT(0)
        }
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            let (x, y) = point(lparam);
            // SAFETY: plain queries and capture of this thread's window.
            let (now, limit) = unsafe {
                let _ = SetCapture(hwnd);
                (GetMessageTime() as u32, GetDoubleClickTime())
            };
            let count = if msg == WM_LBUTTONDBLCLK {
                LAST_DOUBLE.with(|d| d.set((now, x as i32, y as i32)));
                2
            } else {
                // A click soon after a double click, near it, is a triple click.
                let (t, dx, dy) = LAST_DOUBLE.with(Cell::get);
                let near = (dx - x as i32).abs() < 4 && (dy - y as i32).abs() < 4;
                if t != 0 && now.wrapping_sub(t) <= limit && near {
                    3
                } else {
                    1
                }
            };
            input(
                hwnd,
                Input::Press {
                    x,
                    y,
                    count,
                    shift: mods().shift,
                },
            );
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(lparam);
            // MK_LBUTTON
            if wparam.0 & 0x0001 != 0 {
                input(hwnd, Input::Drag { x, y });
            } else {
                input(hwnd, Input::Move { x, y });
            }
            track_leave(hwnd, false);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            // SAFETY: releases this thread's mouse capture.
            unsafe {
                let _ = ReleaseCapture();
            }
            let (x, y) = point(lparam);
            input(hwnd, Input::Release { x, y });
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == BLINK_TIMER => {
            with_handler(|h| h.input(Input::Blink, 0));
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == TICK_TIMER => {
            with_handler(|h| h.input(Input::Tick, 0));
            LRESULT(0)
        }
        WM_SIZE => {
            let v = lparam.0 as u32;
            let (w, h) = (v & 0xFFFF, v >> 16);
            if w > 0 && h > 0 {
                with_handler(|hd| hd.resized(w, h));
            }
            // SIZE_MAXIMIZED = 2
            input(hwnd, Input::Maximized(wparam.0 == 2));
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let dpi = (wparam.0 & 0xFFFF) as f32;
            // SAFETY: for WM_DPICHANGED, lparam points to the suggested window rectangle.
            unsafe {
                let r = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            input(hwnd, Input::Scale(dpi / 96.0));
            LRESULT(0)
        }
        WM_IME_STARTCOMPOSITION | WM_IME_COMPOSITION => {
            place_ime(hwnd);
            // SAFETY: default IME processing turns the result into WM_CHAR messages.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_PAINT => {
            // SAFETY: validates this window's whole client area.
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
            with_handler(|h| h.paint());
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_NCCALCSIZE if wparam.0 != 0 => {
            // The client area covers the title bar (drawn by the interface); the resize frame
            // stays non-client on the left, right and bottom, and off-screen when maximised.
            let (fx, fy) = frame(hwnd);
            // SAFETY: for WM_NCCALCSIZE with wParam TRUE, lParam points to NCCALCSIZE_PARAMS.
            unsafe {
                let params = &mut *(lparam.0 as *mut NCCALCSIZE_PARAMS);
                let r = &mut params.rgrc[0];
                r.left += fx;
                r.right -= fx;
                r.bottom -= fy;
                if IsZoomed(hwnd).as_bool() {
                    r.top += fy;
                }
            }
            LRESULT(0)
        }
        WM_NCHITTEST => {
            // SAFETY: default processing of this window's own message.
            let def = unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            if def.0 != HTCLIENT as isize {
                return def;
            }
            let (x, y) = client_point(hwnd, lparam);
            let (fx, fy) = frame(hwnd);
            // SAFETY: plain query of this window's state and size.
            let (zoomed, width) = unsafe {
                let mut r = RECT::default();
                let _ = GetClientRect(hwnd, &mut r);
                (IsZoomed(hwnd).as_bool(), r.right as f32)
            };
            if !zoomed && y < fy as f32 {
                let code = if x < 2.0 * fx as f32 {
                    HTTOPLEFT
                } else if x > width - 2.0 * fx as f32 {
                    HTTOPRIGHT
                } else {
                    HTTOP
                };
                return LRESULT(code as isize);
            }
            let code = match with_handler(|h| h.nc_hit(x, y)).unwrap_or(NcHit::Client) {
                NcHit::Caption => HTCAPTION,
                NcHit::Minimize => HTMINBUTTON,
                // Windows 11 shows the snap layouts flyout over HTMAXBUTTON.
                NcHit::Maximize => HTMAXBUTTON,
                NcHit::Close => HTCLOSE,
                NcHit::Client => HTCLIENT,
            };
            LRESULT(code as isize)
        }
        WM_NCMOUSEMOVE => {
            let (x, y) = client_point(hwnd, lparam);
            input(hwnd, Input::Move { x, y });
            track_leave(hwnd, true);
            // SAFETY: default processing of this window's own message.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_MOUSELEAVE | WM_NCMOUSELEAVE => {
            TRACKING.with(|c| c.set(false));
            input(hwnd, Input::Leave);
            LRESULT(0)
        }
        WM_NCLBUTTONDOWN if matches!(wparam.0 as u32, HTMINBUTTON | HTMAXBUTTON | HTCLOSE) => {
            // The interface draws these buttons: no default caption-button tracking.
            NC_PRESSED.with(|c| c.set(wparam.0 as u32));
            LRESULT(0)
        }
        WM_NCLBUTTONUP if matches!(wparam.0 as u32, HTMINBUTTON | HTMAXBUTTON | HTCLOSE) => {
            if NC_PRESSED.with(|c| c.replace(0)) == wparam.0 as u32 {
                let r = match wparam.0 as u32 {
                    HTMINBUTTON => Request::Minimize,
                    HTMAXBUTTON => Request::ToggleMaximize,
                    _ => Request::Close,
                };
                window_request(hwnd, &r);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: default processing of this window's own message.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Create the main window, hand it to `handler` and run the message loop until it closes.
pub fn run(title: &str, handler: Box<dyn Handler>) -> Result<(), String> {
    HANDLER.with(|h| *h.borrow_mut() = Some(handler));
    let title: Vec<u16> = title.encode_utf16().chain([0]).collect();
    // SAFETY: the class and title strings outlive the window; the window procedure only touches
    // thread-local state; every handle belongs to this thread.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_IBEAM).unwrap_or_default(),
            hbrBackground: HBRUSH::default(),
            lpszClassName: w!("BlueOfficeNativeDoc"),
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err("RegisterClassExW failed".into());
        }
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
        // Recompute the frame now that WM_NCCALCSIZE removes the system caption.
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
        let _ = ShowWindow(hwnd, SW_SHOWMAXIMIZED);
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        let (w, h) = (
            (rect.right - rect.left).max(1) as u32,
            (rect.bottom - rect.top).max(1) as u32,
        );
        if let Some(Err(e)) = with_handler(|hd| hd.created(hwnd.0 as isize, w, h, scale(hwnd))) {
            return Err(e);
        }
        with_handler(|hd| hd.paint());
        let _ = SetTimer(Some(hwnd), BLINK_TIMER, GetCaretBlinkTimeOr530(), None);
        let _ = SetTimer(Some(hwnd), TICK_TIMER, 100, None);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

#[allow(non_snake_case)]
fn GetCaretBlinkTimeOr530() -> u32 {
    // SAFETY: plain query of the user's caret blink setting.
    let t = unsafe { windows::Win32::UI::WindowsAndMessaging::GetCaretBlinkTime() };
    if t == 0 || t == u32::MAX {
        530
    } else {
        t
    }
}

// ---------------------------------------------------------------- clipboard

/// CF_UNICODETEXT (WinUser.h).
const CF_UNICODETEXT: u32 = 13;

fn format_id(name: &str) -> u32 {
    let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
    // SAFETY: NUL-terminated string that outlives the call.
    unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
}

/// Copy `bytes` into a movable global block and hand it to the clipboard as `format`.
fn put(format: u32, bytes: &[u8]) {
    // SAFETY: the block is allocated with the needed size, written while locked, and its
    // ownership passes to the clipboard on success (freed here only on failure).
    unsafe {
        let Ok(block) = GlobalAlloc(GMEM_MOVEABLE, bytes.len()) else {
            return;
        };
        let ptr = GlobalLock(block).cast::<u8>();
        if ptr.is_null() {
            let _ = GlobalFree(Some(block));
            return;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(block);
        if SetClipboardData(format, Some(HANDLE(block.0))).is_err() {
            let _ = GlobalFree(Some(block));
        }
    }
}

/// Read clipboard data of `format` as bytes (the block's full size).
fn get(format: u32) -> Option<Vec<u8>> {
    // SAFETY: the handle belongs to the open clipboard; it is read only while locked and
    // within the size reported for the block.
    unsafe {
        if IsClipboardFormatAvailable(format).is_err() {
            return None;
        }
        let handle = GetClipboardData(format).ok()?;
        let block = HGLOBAL(handle.0);
        let size = GlobalSize(block);
        let ptr = GlobalLock(block).cast::<u8>();
        if ptr.is_null() {
            return None;
        }
        let out = std::slice::from_raw_parts(ptr, size).to_vec();
        let _ = GlobalUnlock(block);
        Some(out)
    }
}

/// CF_HTML ("HTML Format") wrapper with its byte offsets.
fn cf_html(fragment: &str) -> String {
    const HEADER: &str =
        "Version:0.9\r\nStartHTML:{0}\r\nEndHTML:{1}\r\nStartFragment:{2}\r\nEndFragment:{3}\r\n";
    let pre = "<html><body><!--StartFragment-->";
    let post = "<!--EndFragment--></body></html>";
    let header_len = HEADER
        .replace("{0}", "0000000000")
        .replace("{1}", "0000000000")
        .replace("{2}", "0000000000")
        .replace("{3}", "0000000000")
        .len();
    let start_html = header_len;
    let start_frag = start_html + pre.len();
    let end_frag = start_frag + fragment.len();
    let end_html = end_frag + post.len();
    let n = |v: usize| format!("{v:010}");
    HEADER
        .replace("{0}", &n(start_html))
        .replace("{1}", &n(end_html))
        .replace("{2}", &n(start_frag))
        .replace("{3}", &n(end_frag))
        + pre
        + fragment
        + post
}

fn html_fragment(cf: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(cf);
    let field = |name: &str| -> Option<usize> {
        let at = text.find(name)? + name.len();
        text[at..].lines().next()?.trim().parse().ok()
    };
    let (start, end) = (field("StartFragment:")?, field("EndFragment:")?);
    let bytes = cf.get(start..end)?;
    Some(String::from_utf8_lossy(bytes).into_owned())
}

/// Put text, HTML and the internal form on the clipboard.
pub fn set_clipboard(text: &str, html: &str, internal: &str) {
    // SAFETY: the clipboard is opened for this thread and closed before returning.
    unsafe {
        if OpenClipboard(None).is_err() {
            return;
        }
        let _ = EmptyClipboard();
        let wide: Vec<u8> = text
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        put(CF_UNICODETEXT, &wide);
        if !html.is_empty() {
            let mut h = cf_html(html).into_bytes();
            h.push(0);
            put(format_id("HTML Format"), &h);
        }
        if !internal.is_empty() {
            let mut i = internal.as_bytes().to_vec();
            i.push(0);
            put(format_id("BlueOffice.Internal"), &i);
        }
        let _ = CloseClipboard();
    }
}

/// The clipboard's internal form, HTML fragment and text, as far as they are present.
#[allow(clippy::chunks_exact_to_as_chunks)] // `as_chunks` exceeds the workspace MSRV.
pub fn read_clipboard() -> (Option<String>, Option<String>, Option<String>) {
    let trim = |mut v: Vec<u8>| {
        while v.last() == Some(&0) {
            v.pop();
        }
        String::from_utf8_lossy(&v).into_owned()
    };
    // SAFETY: the clipboard is opened for this thread and closed before returning.
    unsafe {
        if OpenClipboard(None).is_err() {
            return (None, None, None);
        }
        let internal = get(format_id("BlueOffice.Internal")).map(trim);
        let html = get(format_id("HTML Format")).and_then(|b| html_fragment(&b));
        let text = get(CF_UNICODETEXT).map(|b| {
            let units: Vec<u16> = b
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|u| *u != 0)
                .collect();
            String::from_utf16_lossy(&units)
        });
        let _ = CloseClipboard();
        (internal, html, text)
    }
}
