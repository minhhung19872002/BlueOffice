//! A minimal UI Automation provider for BlueReader's self-drawn window (round 12): the window's
//! ribbon tabs and commands as Invoke-able tab items / buttons and the page view as a document,
//! read from [`bo_ui::ReaderWindow::accessibility12`] snapshots. Narrator, Inspect and other
//! UIA clients list them by name, box and enabled state; Invoke runs the command on the UI
//! thread (queued, then a tick message wakes the window).
//!
//! The window procedure is subclassed here (`WM_GETOBJECT`) so the shared `win32.rs` stays as
//! it is. The snapshot and the invoke queue are process-wide (a mutex each), so providers work
//! on whichever thread UI Automation calls them.
// The `#[implement]` macro of the bindings generates code these pedantic lints flag.
#![allow(
    clippy::ref_as_ptr,
    clippy::borrow_as_ptr,
    clippy::inline_always,
    clippy::needless_pass_by_value,
    clippy::cast_ptr_alignment,
    non_snake_case
)]
use bo_ui::{AccessItem, AccessRole};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Mutex;
use windows::core::{implement, Error, Interface, Result, BSTR, HRESULT};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, SAFEARRAY};
use windows::Win32::System::Ole::{SafeArrayCreateVector, SafeArrayPutElement};
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::Accessibility::{
    IInvokeProvider, IInvokeProvider_Impl, IRawElementProviderFragment,
    IRawElementProviderFragmentRoot, IRawElementProviderFragmentRoot_Impl,
    IRawElementProviderFragment_Impl, IRawElementProviderSimple, IRawElementProviderSimple_Impl,
    NavigateDirection, NavigateDirection_FirstChild, NavigateDirection_LastChild,
    NavigateDirection_NextSibling, NavigateDirection_Parent, NavigateDirection_PreviousSibling,
    ProviderOptions, ProviderOptions_ServerSideProvider, ProviderOptions_UseComThreading,
    UIA_AutomationIdPropertyId, UIA_ButtonControlTypeId, UIA_ControlTypePropertyId,
    UIA_DocumentControlTypeId, UIA_FrameworkIdPropertyId, UIA_HasKeyboardFocusPropertyId,
    UIA_InvokePatternId, UIA_IsContentElementPropertyId, UIA_IsControlElementPropertyId,
    UIA_IsEnabledPropertyId, UIA_IsKeyboardFocusablePropertyId, UIA_NamePropertyId,
    UIA_TabItemControlTypeId, UiaHostProviderFromHwnd, UiaRect, UiaReturnRawElementProvider,
    UIA_PATTERN_ID, UIA_PROPERTY_ID,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, PostMessageW, SetWindowLongPtrW, GWLP_WNDPROC, WM_GETOBJECT, WM_TIMER, WNDPROC,
};
use windows_core::IUnknownImpl as _;

/// `UiaRootObjectId` (UIAutomationCoreApi.h).
const UIA_ROOT_OBJECT_ID: i32 = -25;
/// `UiaAppendRuntimeId` (UIAutomationCoreApi.h).
const UIA_APPEND_RUNTIME_ID: i32 = 3;
/// `UIA_E_ELEMENTNOTAVAILABLE`.
const NOT_AVAILABLE: HRESULT = HRESULT(0x8004_0201_u32 as i32);
/// The window's tick timer (win32.rs `TICK_TIMER`): a WM_TIMER with it reaches the handler
/// as `Input::Tick`.
const TICK_TIMER: usize = 2;

// Process-wide state: UIA may call providers on another thread when the UI thread is not a
// single-threaded apartment, so nothing here is thread-local.
static HWND_: AtomicIsize = AtomicIsize::new(0);
static OLD_PROC: AtomicIsize = AtomicIsize::new(0);
/// A UIA client asked for the window at least once (snapshots are kept from then on).
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ITEMS: Mutex<Vec<AccessItem>> = Mutex::new(Vec::new());
/// Keys invoked by clients, run by the window on its next tick.
static INVOKED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Subclass `hwnd` to answer `WM_GETOBJECT` with the provider.
pub fn install(hwnd: isize) {
    HWND_.store(hwnd, Ordering::Relaxed);
    // SAFETY: the window belongs to this thread; the new procedure forwards every other
    // message to the old one, kept for the window's lifetime.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let old = SetWindowLongPtrW(HWND(hwnd as _), GWLP_WNDPROC, proc_ as *const () as isize);
        OLD_PROC.store(old, Ordering::Relaxed);
    }
}

/// Whether snapshots are wanted (a client connected).
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// The window's elements now.
pub fn update(items: Vec<AccessItem>) {
    if let Ok(mut i) = ITEMS.lock() {
        *i = items;
    }
}

/// Keys invoked since the last call.
pub fn take_invokes() -> Vec<String> {
    INVOKED
        .lock()
        .map(|mut v| std::mem::take(&mut *v))
        .unwrap_or_default()
}

extern "system" fn proc_(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_GETOBJECT && lparam.0 as i32 == UIA_ROOT_OBJECT_ID {
        if !ACTIVE.swap(true, Ordering::Relaxed) {
            // First client: a tick makes the window take its first snapshot now.
            // SAFETY: posting to this window.
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_TIMER, WPARAM(TICK_TIMER), LPARAM(0));
            }
        }
        let root: IRawElementProviderSimple = Root.into();
        // SAFETY: the provider is a live COM object.
        return unsafe { UiaReturnRawElementProvider(hwnd, wparam, lparam, &root) };
    }
    let old = OLD_PROC.load(Ordering::Relaxed);
    // SAFETY: `old` is the procedure the window had before `install`.
    unsafe {
        let old: WNDPROC = std::mem::transmute::<isize, WNDPROC>(old);
        CallWindowProcW(old, hwnd, msg, wparam, lparam)
    }
}

fn hwnd() -> HWND {
    HWND(HWND_.load(Ordering::Relaxed) as _)
}

fn item(key: &str) -> Option<(usize, AccessItem)> {
    let i = ITEMS.lock().ok()?;
    i.iter()
        .position(|x| x.key == key)
        .map(|k| (k, i[k].clone()))
}

fn item_at(k: usize) -> Option<AccessItem> {
    ITEMS.lock().ok()?.get(k).cloned()
}

fn count() -> usize {
    ITEMS.lock().map_or(0, |i| i.len())
}

fn element(k: usize) -> Result<IRawElementProviderFragment> {
    let it = item_at(k).ok_or_else(Error::empty)?;
    Ok(Element { key: it.key }.into())
}

fn root() -> Result<IRawElementProviderFragmentRoot> {
    let r: IRawElementProviderSimple = Root.into();
    r.cast()
}

/// A stable number for a key (runtime ids).
fn key_id(key: &str) -> i32 {
    let mut h: u32 = 2_166_136_261;
    for b in key.bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(16_777_619);
    }
    (h & 0x7fff_ffff) as i32
}

#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IRawElementProviderFragmentRoot
)]
struct Root;

impl IRawElementProviderSimple_Impl for Root_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider | ProviderOptions_UseComThreading)
    }
    fn GetPatternProvider(&self, _: UIA_PATTERN_ID) -> Result<windows::core::IUnknown> {
        Err(Error::empty())
    }
    fn GetPropertyValue(&self, id: UIA_PROPERTY_ID) -> Result<VARIANT> {
        Ok(if id == UIA_FrameworkIdPropertyId {
            VARIANT::from(BSTR::from("BlueOffice"))
        } else {
            VARIANT::default()
        })
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        // SAFETY: the window handle is this thread's window.
        unsafe { UiaHostProviderFromHwnd(hwnd()) }
    }
}

impl IRawElementProviderFragment_Impl for Root_Impl {
    fn Navigate(&self, d: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let n = count();
        if d == NavigateDirection_FirstChild && n > 0 {
            element(0)
        } else if d == NavigateDirection_LastChild && n > 0 {
            element(n - 1)
        } else {
            Err(Error::empty())
        }
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        Ok(UiaRect::default())
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn SetFocus(&self) -> Result<()> {
        Ok(())
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        root()
    }
}

impl IRawElementProviderFragmentRoot_Impl for Root_Impl {
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        let mut p = POINT {
            x: x as i32,
            y: y as i32,
        };
        // SAFETY: plain coordinate conversion for this thread's window.
        let _ = unsafe { ScreenToClient(hwnd(), &mut p) };
        let (px, py) = (p.x as f32, p.y as f32);
        let within = |it: &AccessItem| {
            px >= it.rect[0]
                && px < it.rect[0] + it.rect[2]
                && py >= it.rect[1]
                && py < it.rect[1] + it.rect[3]
        };
        let hit = ITEMS.lock().ok().and_then(|i| {
            // Commands before the page view that contains them.
            i.iter()
                .position(|it| it.role != AccessRole::Document && within(it))
                .or_else(|| i.iter().position(within))
        });
        match hit {
            Some(k) => element(k),
            None => root()?.cast(),
        }
    }
    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        Err(Error::empty())
    }
}

#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IInvokeProvider
)]
struct Element {
    key: String,
}

impl Element_Impl {
    fn current(&self) -> Result<(usize, AccessItem)> {
        item(&self.key).ok_or_else(|| Error::from_hresult(NOT_AVAILABLE))
    }
}

impl IRawElementProviderSimple_Impl for Element_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider | ProviderOptions_UseComThreading)
    }
    fn GetPatternProvider(&self, id: UIA_PATTERN_ID) -> Result<windows::core::IUnknown> {
        let (_, it) = self.current()?;
        if id == UIA_InvokePatternId && it.role != AccessRole::Document {
            let me: IInvokeProvider = self.to_interface();
            return me.cast();
        }
        Err(Error::empty())
    }
    fn GetPropertyValue(&self, id: UIA_PROPERTY_ID) -> Result<VARIANT> {
        let (_, it) = self.current()?;
        let control = match it.role {
            AccessRole::Button => UIA_ButtonControlTypeId,
            AccessRole::Tab => UIA_TabItemControlTypeId,
            AccessRole::Document => UIA_DocumentControlTypeId,
        };
        Ok(match id {
            i if i == UIA_ControlTypePropertyId => VARIANT::from(control.0),
            i if i == UIA_NamePropertyId => VARIANT::from(BSTR::from(it.name.as_str())),
            i if i == UIA_AutomationIdPropertyId => VARIANT::from(BSTR::from(it.key.as_str())),
            i if i == UIA_IsEnabledPropertyId => VARIANT::from(it.enabled),
            i if i == UIA_IsKeyboardFocusablePropertyId => {
                VARIANT::from(it.role == AccessRole::Document)
            }
            i if i == UIA_HasKeyboardFocusPropertyId => VARIANT::from(false),
            i if i == UIA_IsControlElementPropertyId || i == UIA_IsContentElementPropertyId => {
                VARIANT::from(true)
            }
            i if i == UIA_FrameworkIdPropertyId => VARIANT::from(BSTR::from("BlueOffice")),
            _ => VARIANT::default(),
        })
    }
    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        Err(Error::empty())
    }
}

impl IRawElementProviderFragment_Impl for Element_Impl {
    fn Navigate(&self, d: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let (k, _) = self.current()?;
        if d == NavigateDirection_Parent {
            root()?.cast()
        } else if d == NavigateDirection_NextSibling && k + 1 < count() {
            element(k + 1)
        } else if d == NavigateDirection_PreviousSibling && k > 0 {
            element(k - 1)
        } else {
            Err(Error::empty())
        }
    }
    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        let ids = [UIA_APPEND_RUNTIME_ID, key_id(&self.key)];
        // SAFETY: a two-element VT_I4 vector filled element by element; UIA frees it.
        unsafe {
            let sa = SafeArrayCreateVector(VT_I4, 0, 2);
            if sa.is_null() {
                return Err(Error::from_hresult(HRESULT(0x8007_000E_u32 as i32)));
            }
            for (i, v) in ids.iter().enumerate() {
                let idx = i as i32;
                SafeArrayPutElement(sa, &idx, std::ptr::from_ref(v).cast())?;
            }
            Ok(sa)
        }
    }
    fn BoundingRectangle(&self) -> Result<UiaRect> {
        let (_, it) = self.current()?;
        let mut p = POINT {
            x: it.rect[0].round() as i32,
            y: it.rect[1].round() as i32,
        };
        // SAFETY: plain coordinate conversion for this thread's window.
        let _ = unsafe { ClientToScreen(hwnd(), &mut p) };
        Ok(UiaRect {
            left: f64::from(p.x),
            top: f64::from(p.y),
            width: f64::from(it.rect[2]),
            height: f64::from(it.rect[3]),
        })
    }
    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
    fn SetFocus(&self) -> Result<()> {
        Ok(())
    }
    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        root()
    }
}

impl IInvokeProvider_Impl for Element_Impl {
    fn Invoke(&self) -> Result<()> {
        let (_, it) = self.current()?;
        if !it.enabled {
            // UIA_E_ELEMENTNOTENABLED
            return Err(Error::from_hresult(HRESULT(0x8004_0200_u32 as i32)));
        }
        if let Ok(mut v) = INVOKED.lock() {
            v.push(it.key.clone());
        }
        // SAFETY: posting to this thread's window; the tick runs the queued key.
        unsafe {
            let _ = PostMessageW(Some(hwnd()), WM_TIMER, WPARAM(TICK_TIMER), LPARAM(0));
        }
        Ok(())
    }
}
