//! Win32 side (ADR-0016): the maximise-button overlay window and the system menu.
//!
//! Every function must run on the thread that owns the window (Tauri's main thread). The only
//! `unsafe` code of the crate lives here; each block calls documented Win32 functions with
//! window handles the caller owns and plain values or stack-allocated structures.
#![allow(unsafe_code)]

use crate::PhysRect;
use std::sync::{Arc, Mutex, Once};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, EnableMenuItem, GetClientRect, GetSystemMenu,
    IsWindow, IsZoomed, LoadCursorW, PostMessageW, RegisterClassExW, SetCursor,
    SetLayeredWindowAttributes, SetMenuDefaultItem, SetWindowPos, TrackPopupMenu, HTMAXBUTTON,
    HWND_TOP, IDC_ARROW, LWA_ALPHA, MA_NOACTIVATE, MF_BYCOMMAND, MF_ENABLED, MF_GRAYED, SC_CLOSE,
    SC_MAXIMIZE, SC_MINIMIZE, SC_MOVE, SC_RESTORE, SC_SIZE, SWP_HIDEWINDOW, SWP_NOACTIVATE,
    SWP_NOOWNERZORDER, SWP_SHOWWINDOW, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_MOUSEACTIVATE,
    WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP,
    WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_NCRBUTTONUP, WM_SETCURSOR, WM_SYSCOMMAND, WNDCLASSEXW,
    WS_CHILD, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
};

/// Hover / press state of the native maximise button, mirrored by the web button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ButtonState {
    /// The pointer is over the button.
    pub hover: bool,
    /// The left button is down on it.
    pub pressed: bool,
}

/// Receives the button state changes (on the window thread).
pub type StateCallback = Arc<dyn Fn(ButtonState) + Send + Sync>;

struct Overlay {
    parent: isize,
    hwnd: isize,
    state: ButtonState,
    tracking: bool,
    on_state: StateCallback,
}

static OVERLAYS: Mutex<Vec<Overlay>> = Mutex::new(Vec::new());
static REGISTER: Once = Once::new();

/// Window class of the overlay (UTF-16, NUL-terminated).
const CLASS: &[u16] = &[
    b'B' as u16,
    b'O' as u16,
    b'_' as u16,
    b'M' as u16,
    b'A' as u16,
    b'X' as u16,
    b'_' as u16,
    b'B' as u16,
    b'U' as u16,
    b'T' as u16,
    b'T' as u16,
    b'O' as u16,
    b'N' as u16,
    0,
];

/// The running Windows build (e.g. 22631), for the Mica fallback.
pub fn os_build() -> u32 {
    windows_version::OsVersion::current().build
}

fn hwnd(h: isize) -> HWND {
    h as HWND
}

/// Size of the parent's client area in physical pixels.
pub fn client_size(parent: isize) -> Option<(i32, i32)> {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `r` is a valid out pointer; an invalid handle only makes the call fail.
    let ok = unsafe { GetClientRect(hwnd(parent), &raw mut r) } != 0;
    ok.then_some((r.right - r.left, r.bottom - r.top))
}

fn with_overlay<T>(overlay: isize, f: impl FnOnce(&mut Overlay) -> T) -> Option<T> {
    let mut list = OVERLAYS.lock().ok()?;
    list.iter_mut().find(|o| o.hwnd == overlay).map(f)
}

fn notify(overlay: isize, update: impl FnOnce(&mut ButtonState)) {
    // The callback runs outside the lock (it may call back into this module).
    let changed = with_overlay(overlay, |o| {
        let before = o.state;
        update(&mut o.state);
        (o.state != before).then(|| (o.on_state.clone(), o.state))
    })
    .flatten();
    if let Some((cb, state)) = changed {
        cb(state);
    }
}

fn parent_of(overlay: isize) -> Option<isize> {
    with_overlay(overlay, |o| o.parent)
}

fn post_syscommand(parent: isize, cmd: u32) {
    // SAFETY: posting a message to a window handle has no memory effects on our side.
    unsafe { PostMessageW(hwnd(parent), WM_SYSCOMMAND, cmd as WPARAM, 0) };
}

/// Maximises or restores `parent` (what the maximise button does).
pub fn toggle_maximize(parent: isize) {
    // SAFETY: plain query on a window handle.
    let zoomed = unsafe { IsZoomed(hwnd(parent)) } != 0;
    post_syscommand(parent, if zoomed { SC_RESTORE } else { SC_MAXIMIZE });
}

unsafe extern "system" fn overlay_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let me = h as isize;
    match msg {
        // The whole overlay is the maximise button: Windows 11 shows the snap layouts.
        WM_NCHITTEST => HTMAXBUTTON as LRESULT,
        // Never take the focus from the web view.
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_SETCURSOR => {
            // SAFETY: loading a system cursor (no module handle) and setting it.
            unsafe { SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_ARROW)) };
            1
        }
        WM_NCMOUSEMOVE => {
            let start = with_overlay(me, |o| !std::mem::replace(&mut o.tracking, true));
            if start == Some(true) {
                let mut t = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: h,
                    dwHoverTime: 0,
                };
                // SAFETY: `t` is a valid, initialised structure for the duration of the call.
                unsafe { TrackMouseEvent(&raw mut t) };
            }
            notify(me, |s| s.hover = true);
            0
        }
        WM_NCMOUSELEAVE => {
            with_overlay(me, |o| o.tracking = false);
            notify(me, |s| *s = ButtonState::default());
            0
        }
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => {
            notify(me, |s| s.pressed = true);
            0
        }
        WM_NCLBUTTONUP => {
            let was = with_overlay(me, |o| o.state.pressed).unwrap_or(false);
            notify(me, |s| s.pressed = false);
            if was {
                if let Some(p) = parent_of(me) {
                    toggle_maximize(p);
                }
            }
            0
        }
        WM_NCRBUTTONUP => {
            if let Some(p) = parent_of(me) {
                let x = (lp & 0xffff) as i16 as i32;
                let y = ((lp >> 16) & 0xffff) as i16 as i32;
                show_system_menu_at_screen(p, x, y);
            }
            0
        }
        WM_NCDESTROY => {
            if let Ok(mut list) = OVERLAYS.lock() {
                list.retain(|o| o.hwnd != me);
            }
            // SAFETY: default handling of a message of our own window.
            unsafe { DefWindowProcW(h, msg, wp, lp) }
        }
        // SAFETY: default handling of a message of our own window.
        _ => unsafe { DefWindowProcW(h, msg, wp, lp) },
    }
}

fn register_class() {
    REGISTER.call_once(|| {
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(overlay_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            // SAFETY: the module handle of the running executable.
            hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: CLASS.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        // SAFETY: `class` and the static class name outlive the call.
        unsafe { RegisterClassExW(&raw const class) };
    });
}

fn create_overlay(parent: isize, on_state: StateCallback) -> Result<isize, String> {
    register_class();
    // SAFETY: creates a child of `parent` (a live window of this thread) with our class; a
    // layered window without a redirection bitmap draws nothing but is hit-tested.
    let h = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE,
            CLASS.as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            hwnd(parent),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if h.is_null() {
        return Err(format!(
            "CreateWindowExW: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: `h` was just created; full opacity keeps it hit-testable.
    unsafe { SetLayeredWindowAttributes(h, 0, 255, LWA_ALPHA) };
    let id = h as isize;
    OVERLAYS.lock().map_err(|e| e.to_string())?.push(Overlay {
        parent,
        hwnd: id,
        state: ButtonState::default(),
        tracking: false,
        on_state,
    });
    Ok(id)
}

/// Places the snap-layouts overlay of `parent` over `rect` (client pixels), creating it on first
/// use; `None` hides it. `on_state` receives hover / press changes (kept from the first call).
pub fn set_maximize_button(
    parent: isize,
    rect: Option<PhysRect>,
    on_state: impl Fn(ButtonState) + Send + Sync + 'static,
) -> Result<(), String> {
    // SAFETY: plain query on a window handle.
    if unsafe { IsWindow(hwnd(parent)) } == 0 {
        return Err("not a window".into());
    }
    let existing = OVERLAYS
        .lock()
        .map_err(|e| e.to_string())?
        .iter()
        .find(|o| o.parent == parent)
        .map(|o| o.hwnd);
    let overlay = match (existing, rect) {
        (Some(h), _) => h,
        (None, None) => return Ok(()),
        (None, Some(_)) => create_overlay(parent, Arc::new(on_state))?,
    };
    let flags = SWP_NOACTIVATE | SWP_NOOWNERZORDER;
    // SAFETY: moves / shows / hides our own child window above its siblings (the web view).
    unsafe {
        match rect {
            Some(r) => SetWindowPos(
                hwnd(overlay),
                HWND_TOP,
                r.left,
                r.top,
                r.width(),
                r.height(),
                flags | SWP_SHOWWINDOW,
            ),
            None => SetWindowPos(hwnd(overlay), HWND_TOP, 0, 0, 0, 0, flags | SWP_HIDEWINDOW),
        };
    }
    Ok(())
}

/// Destroys the overlay of `parent` (if any).
pub fn release(parent: isize) {
    let found = OVERLAYS
        .lock()
        .ok()
        .and_then(|l| l.iter().find(|o| o.parent == parent).map(|o| o.hwnd));
    if let Some(h) = found {
        // SAFETY: destroys our own child window (WM_NCDESTROY removes its entry).
        unsafe { DestroyWindow(hwnd(h)) };
    }
}

/// Shows the window's system menu (Restore, Move, Size, Minimise, Maximise, Close) at a client
/// point in physical pixels; the chosen command goes to the window as `WM_SYSCOMMAND`.
pub fn show_system_menu(parent: isize, x: i32, y: i32) {
    let mut p = POINT { x, y };
    // SAFETY: `p` is a valid in/out pointer.
    if unsafe { ClientToScreen(hwnd(parent), &raw mut p) } != 0 {
        show_system_menu_at_screen(parent, p.x, p.y);
    }
}

fn show_system_menu_at_screen(parent: isize, x: i32, y: i32) {
    let w = hwnd(parent);
    // SAFETY: the window's own system menu (not destroyed by us), items updated to the window
    // state, then tracked modally on this thread.
    unsafe {
        let menu = GetSystemMenu(w, 0);
        if menu.is_null() {
            return;
        }
        let zoomed = IsZoomed(w) != 0;
        let set = |id: u32, on: bool| {
            EnableMenuItem(
                menu,
                id,
                MF_BYCOMMAND | if on { MF_ENABLED } else { MF_GRAYED },
            );
        };
        set(SC_RESTORE, zoomed);
        set(SC_MOVE, !zoomed);
        set(SC_SIZE, !zoomed);
        set(SC_MINIMIZE, true);
        set(SC_MAXIMIZE, !zoomed);
        set(SC_CLOSE, true);
        SetMenuDefaultItem(menu, SC_CLOSE, 0);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            x,
            y,
            0,
            w,
            std::ptr::null(),
        );
        if cmd != 0 {
            post_syscommand(parent, cmd as u32);
        }
    }
}
