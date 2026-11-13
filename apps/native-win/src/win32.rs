//! Win32 window, message loop and input translation (the crate's only `unsafe` code).
// The generated bindings take references for pointer parameters.
#![allow(clippy::borrow_as_ptr)]
use bo_ui::{Input, Key, Mods};
use std::cell::{Cell, RefCell};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{ValidateRect, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::Ime::{
    ImmGetContext, ImmReleaseContext, ImmSetCompositionWindow, CFS_POINT, COMPOSITIONFORM,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetDoubleClickTime, GetKeyState, ReleaseCapture, SetCapture, VIRTUAL_KEY, VK_BACK, VK_CONTROL,
    VK_DELETE, VK_DOWN, VK_END, VK_HOME, VK_LEFT, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT,
    VK_SHIFT, VK_TAB, VK_UP,
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
}

thread_local! {
    static HANDLER: RefCell<Option<Box<dyn Handler>>> = RefCell::new(None);
    static HIGH_SURROGATE: Cell<u16> = const { Cell::new(0) };
    static LAST_DOUBLE: Cell<(u32, i32, i32)> = const { Cell::new((0, 0, 0)) };
}

const BLINK_TIMER: usize = 1;

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

fn input(i: Input) {
    let age = age();
    with_handler(|h| h.input(i, age));
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
                input(Input::Text(text));
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let m = mods();
            match key(wparam.0 as u16, m) {
                Some(k) => {
                    input(Input::Key(k, m));
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
            // About one text line (20 logical px) per wheel line.
            let px = -delta / 120.0 * lines as f32 * 20.0 * scale(hwnd);
            input(Input::Wheel(px));
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
            input(Input::Press {
                x,
                y,
                count,
                shift: mods().shift,
            });
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(lparam);
            // MK_LBUTTON
            if wparam.0 & 0x0001 != 0 {
                input(Input::Drag { x, y });
            } else {
                input(Input::Move { x, y });
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            // SAFETY: releases this thread's mouse capture.
            unsafe {
                let _ = ReleaseCapture();
            }
            let (x, y) = point(lparam);
            input(Input::Release { x, y });
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == BLINK_TIMER => {
            with_handler(|h| h.input(Input::Blink, 0));
            LRESULT(0)
        }
        WM_SIZE => {
            let v = lparam.0 as u32;
            let (w, h) = (v & 0xFFFF, v >> 16);
            if w > 0 && h > 0 {
                with_handler(|hd| hd.resized(w, h));
            }
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
            input(Input::Scale(dpi / 96.0));
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
