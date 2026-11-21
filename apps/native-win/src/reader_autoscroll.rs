//! Middle-button auto-scroll of BlueReader (round 16), measured on the reference reader with
//! posted messages: the middle button drops an anchor mark where it was pressed; from then on
//! the document scrolls by the pointer's vertical distance from the anchor (nothing within
//! 28 px at 125 %, then about 16.5 px per second for every further pixel) until any button is
//! pressed or Esc. The window procedure is subclassed here (like `reader_uia.rs`) so the shared
//! `win32.rs` stays as it is; the window's tick runs each step.
// `Some(None)`: the anchor went away (auto-scroll ended), as against no change.
#![allow(clippy::option_option)]
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, KillTimer, LoadCursorW, SetCursor, SetTimer, SetWindowLongPtrW, GWLP_WNDPROC,
    IDC_SIZEALL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
    WM_RBUTTONDOWN, WM_SETCURSOR, WM_TIMER, WNDPROC,
};

/// The window's tick timer (win32.rs `TICK_TIMER`).
const TICK_TIMER: usize = 2;
/// Our own step timer (about 60 a second while scrolling).
const STEP_TIMER: usize = 0x5216;

static OLD_PROC: AtomicIsize = AtomicIsize::new(0);

#[derive(Debug, Default)]
struct State {
    /// Anchor in client pixels while scrolling.
    anchor: Option<(f32, f32)>,
    /// Pointer's vertical distance from the anchor.
    dy: f32,
    last: Option<Instant>,
    /// Anchor changes not yet taken by the window (`Some(None)`: ended).
    pending: Option<Option<(f32, f32)>>,
    /// Scroll step not yet taken: distance, milliseconds.
    step: Option<(f32, f32)>,
}

static STATE: Mutex<State> = Mutex::new(State {
    anchor: None,
    dy: 0.0,
    last: None,
    pending: None,
    step: None,
});

/// Subclass `hwnd` (after `reader_uia::install`).
pub fn install(hwnd: isize) {
    // SAFETY: the window belongs to this thread; the new procedure forwards every other message
    // to the old one, kept for the window's lifetime.
    unsafe {
        let old = SetWindowLongPtrW(HWND(hwnd as _), GWLP_WNDPROC, proc_ as *const () as isize);
        OLD_PROC.store(old, Ordering::Relaxed);
    }
}

/// The anchor to show (or `None` to hide it), when it changed.
pub fn take_anchor() -> Option<Option<(f32, f32)>> {
    STATE.lock().ok().and_then(|mut s| s.pending.take())
}

/// The scroll step to make now: (distance from the anchor, milliseconds since the last one).
pub fn take_step() -> Option<(f32, f32)> {
    STATE.lock().ok().and_then(|mut s| s.step.take())
}

fn point(lparam: LPARAM) -> (f32, f32) {
    let x = f32::from((lparam.0 & 0xFFFF) as u16 as i16);
    let y = f32::from(((lparam.0 >> 16) & 0xFFFF) as u16 as i16);
    (x, y)
}

fn tick(hwnd: HWND) {
    // SAFETY: posting to this thread's window.
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
            Some(hwnd),
            WM_TIMER,
            WPARAM(TICK_TIMER),
            LPARAM(0),
        );
    }
}

/// End auto-scroll; true when it was on.
fn end(hwnd: HWND) -> bool {
    let was = STATE.lock().is_ok_and(|mut s| {
        let on = s.anchor.take().is_some();
        if on {
            s.pending = Some(None);
            s.step = None;
            s.last = None;
        }
        on
    });
    if was {
        // SAFETY: this thread's window and timer.
        unsafe {
            let _ = KillTimer(Some(hwnd), STEP_TIMER);
            let _ = ReleaseCapture();
        }
        tick(hwnd);
    }
    was
}

extern "system" fn proc_(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let on = STATE.lock().is_ok_and(|s| s.anchor.is_some());
    match msg {
        WM_MBUTTONDOWN if !on => {
            let at = point(lparam);
            if let Ok(mut s) = STATE.lock() {
                s.anchor = Some(at);
                s.dy = 0.0;
                s.last = Some(Instant::now());
                s.pending = Some(Some(at));
            }
            // SAFETY: this thread's window; the timer is killed when auto-scroll ends.
            unsafe {
                let _ = SetCapture(hwnd);
                let _ = SetTimer(Some(hwnd), STEP_TIMER, 16, None);
            }
            tick(hwnd);
            return LRESULT(0);
        }
        WM_MBUTTONDOWN | WM_LBUTTONDOWN | WM_RBUTTONDOWN if on => {
            end(hwnd);
            return LRESULT(0);
        }
        WM_MBUTTONUP if on => return LRESULT(0),
        // Esc ends it.
        WM_KEYDOWN if on && wparam.0 == 0x1B => {
            end(hwnd);
            return LRESULT(0);
        }
        WM_MOUSEMOVE if on => {
            let (_, y) = point(lparam);
            if let Ok(mut s) = STATE.lock() {
                if let Some((_, ay)) = s.anchor {
                    s.dy = y - ay;
                }
            }
            return LRESULT(0);
        }
        WM_SETCURSOR if on => {
            // SAFETY: a system cursor.
            unsafe {
                if let Ok(c) = LoadCursorW(None, IDC_SIZEALL) {
                    SetCursor(Some(c));
                }
            }
            return LRESULT(1);
        }
        WM_TIMER if wparam.0 == STEP_TIMER => {
            if on {
                if let Ok(mut s) = STATE.lock() {
                    let now = Instant::now();
                    let ms = s
                        .last
                        .map_or(16.0, |t| now.duration_since(t).as_secs_f32() * 1000.0)
                        .min(100.0);
                    s.last = Some(now);
                    let prev = s.step.map_or(0.0, |p| p.1);
                    s.step = Some((s.dy, prev + ms));
                }
                tick(hwnd);
            }
            return LRESULT(0);
        }
        _ => {}
    }
    let old = OLD_PROC.load(Ordering::Relaxed);
    // SAFETY: `old` is the procedure the window had before `install`.
    unsafe {
        let old: WNDPROC = std::mem::transmute::<isize, WNDPROC>(old);
        CallWindowProcW(old, hwnd, msg, wparam, lparam)
    }
}
