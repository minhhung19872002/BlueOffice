//! OLE drag and drop of text (BlueDoc round 10): dragged selections leave the window as plain
//! text, rich text, HTML and the internal form, so other applications take them; text dragged
//! from other applications drops into the document. The system draws the drag cursors (move,
//! copy, no drop: `DRAGDROP_S_USEDEFAULTCURSORS`), as the desktop word processor shows them.
//! A child module of `win32` (its window and handler); every call stays on the UI thread.
#![allow(
    clippy::ref_as_ptr,
    clippy::borrow_as_ptr,
    clippy::inline_always,
    clippy::needless_pass_by_value,
    clippy::cast_ptr_alignment,
    non_snake_case
)]
use super::{cf_html, format_id, html_fragment, with_handler, CF_UNICODETEXT};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use windows::core::{implement, Ref, Result, BOOL, HRESULT};
use windows::Win32::Foundation::{
    GlobalFree, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, HGLOBAL, HWND,
    POINT, POINTL, S_OK,
};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Com::{
    IDataObject, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::System::Ole::{
    DoDragDrop, IDropSource, IDropSource_Impl, IDropTarget, IDropTarget_Impl, RegisterDragDrop,
    ReleaseStgMedium, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_MOVE, DROPEFFECT_NONE,
};
use windows::Win32::System::SystemServices::{MK_CONTROL, MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::SHCreateDataObject;

/// Text dropped on the window: internal form, HTML fragment and plain text, as present.
pub struct DropData {
    /// BlueOffice internal form (lossless within the suite).
    pub internal: Option<String>,
    /// HTML fragment ("HTML Format").
    pub html: Option<String>,
    /// Plain text (CF_UNICODETEXT).
    pub text: Option<String>,
}

/// Plain text as RTF ("Rich Text Format"; RTF 1.9.1: `\uN?` for characters beyond ASCII,
/// `\par` between paragraphs, `\tab` for tabs).
pub fn rtf(text: &str) -> String {
    let mut out =
        String::from("{\\rtf1\\ansi\\ansicpg1252\\uc1\\deff0{\\fonttbl{\\f0 Calibri;}}\\f0\\fs22 ");
    for c in text.chars() {
        match c {
            '\\' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\par\n"),
            '\r' => {}
            '\t' => out.push_str("\\tab "),
            c if c.is_ascii() => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    let _ = write!(out, "\\u{}?", *u as i16);
                }
            }
        }
    }
    out.push('}');
    out
}

fn etc(format: u32) -> FORMATETC {
    FORMATETC {
        cfFormat: format as u16,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

/// Give `bytes` to the data object as `format` (it owns the block afterwards).
fn set(obj: &IDataObject, format: u32, bytes: &[u8]) {
    // SAFETY: the block is allocated with the needed size and written while locked; the data
    // object takes ownership on success (fRelease), it is freed here only on failure.
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
        let medium = STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: block },
            pUnkForRelease: std::mem::ManuallyDrop::new(None),
        };
        if obj.SetData(&etc(format), &medium, true).is_err() {
            let _ = GlobalFree(Some(block));
        }
    }
}

/// The bytes of `format` in a dropped data object.
fn get(obj: &IDataObject, format: u32) -> Option<Vec<u8>> {
    // SAFETY: the medium is read only while its block is locked, within the block's size, and
    // released afterwards.
    unsafe {
        let mut medium = obj.GetData(&etc(format)).ok()?;
        let block: HGLOBAL = medium.u.hGlobal;
        let size = GlobalSize(block);
        let ptr = GlobalLock(block).cast::<u8>();
        let out = (!ptr.is_null()).then(|| std::slice::from_raw_parts(ptr, size).to_vec());
        let _ = GlobalUnlock(block);
        ReleaseStgMedium(&mut medium);
        out
    }
}

fn trim(mut v: Vec<u8>) -> String {
    while v.last() == Some(&0) {
        v.pop();
    }
    String::from_utf8_lossy(&v).into_owned()
}

#[allow(clippy::chunks_exact_to_as_chunks)] // `as_chunks` exceeds the workspace MSRV.
fn read(obj: &IDataObject) -> DropData {
    DropData {
        internal: get(obj, format_id("BlueOffice.Internal")).map(trim),
        html: get(obj, format_id("HTML Format")).and_then(|b| html_fragment(&b)),
        text: get(obj, CF_UNICODETEXT).map(|b| {
            let units: Vec<u16> = b
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|u| *u != 0)
                .collect();
            String::from_utf16_lossy(&units)
        }),
    }
}

fn has_text(obj: &IDataObject) -> bool {
    [
        CF_UNICODETEXT,
        format_id("HTML Format"),
        format_id("BlueOffice.Internal"),
    ]
    .into_iter()
    // SAFETY: a plain query of the data object.
    .any(|f| unsafe { obj.QueryGetData(&etc(f)) } == S_OK)
}

/// Draw the frame the last drag event asked for (the drag's modal loop draws nothing else).
fn draw() {
    if with_handler(|h| h.wants_frame()).unwrap_or(false) {
        with_handler(|h| h.frame());
    }
}

#[implement(IDropSource)]
struct Source;

impl IDropSource_Impl for Source_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        if escape.as_bool() {
            DRAGDROP_S_CANCEL
        } else if keys.0 & MK_LBUTTON.0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

#[implement(IDropTarget)]
struct Target {
    hwnd: AtomicIsize,
    text: AtomicBool,
}

impl Target_Impl {
    fn client(&self, pt: &POINTL) -> (f32, f32) {
        let mut p = POINT { x: pt.x, y: pt.y };
        // SAFETY: the window belongs to this thread; the point is a local.
        unsafe {
            let _ = ScreenToClient(HWND(self.hwnd.load(Ordering::Relaxed) as *mut _), &mut p);
        }
        (p.x as f32, p.y as f32)
    }

    /// The effect of a drop here: copy with Ctrl or when the source only lets copy, else move.
    fn over(&self, keys: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) {
        let (x, y) = self.client(pt);
        // SAFETY: OLE passes a valid in/out effect pointer.
        let allowed = unsafe { *effect };
        let copy = keys.0 & MK_CONTROL.0 != 0 || allowed.0 & DROPEFFECT_MOVE.0 == 0;
        let ok = self.text.load(Ordering::Relaxed)
            && with_handler(|h| h.drag_over(x, y)).unwrap_or(false);
        let e = match (ok, copy) {
            (false, _) => DROPEFFECT_NONE,
            (true, true) => DROPEFFECT(allowed.0 & DROPEFFECT_COPY.0),
            (true, false) => DROPEFFECT_MOVE,
        };
        // SAFETY: as above.
        unsafe { *effect = e };
        draw();
    }
}

impl IDropTarget_Impl for Target_Impl {
    fn DragEnter(
        &self,
        data: Ref<IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let text = data.ok().is_ok_and(has_text);
        self.text.store(text, Ordering::Relaxed);
        self.over(keys, pt, effect);
        Ok(())
    }

    fn DragOver(
        &self,
        keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        self.over(keys, pt, effect);
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        with_handler(|h| h.drag_leave());
        draw();
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        self.over(keys, pt, effect);
        // SAFETY: OLE passes a valid in/out effect pointer.
        let e = unsafe { *effect };
        let copy = e != DROPEFFECT_MOVE;
        let done = e != DROPEFFECT_NONE
            && data.ok().is_ok_and(|obj| {
                let (x, y) = self.client(pt);
                let d = read(obj);
                with_handler(|h| h.drop_data(x, y, copy, d)).unwrap_or(false)
            });
        if !done {
            with_handler(|h| h.drag_leave());
            // SAFETY: as above.
            unsafe { *effect = DROPEFFECT_NONE };
        }
        draw();
        Ok(())
    }
}

/// Initialise OLE and accept drops on `hwnd` when the handler takes them.
pub fn register_if_wanted(hwnd: HWND) {
    if with_handler(|h| h.accepts_drops()).unwrap_or(false) {
        // SAFETY: OLE initialised once on the UI thread that owns the window.
        let _ = unsafe { windows::Win32::System::Ole::OleInitialize(None) };
        register(hwnd);
    }
}

/// Accept text dropped on `hwnd` (OLE must be initialised on this thread).
pub fn register(hwnd: HWND) {
    let target: IDropTarget = Target {
        hwnd: AtomicIsize::new(hwnd.0 as isize),
        text: AtomicBool::new(false),
    }
    .into();
    // SAFETY: the window belongs to this thread; OLE keeps its own reference to the target.
    unsafe {
        let _ = RegisterDragDrop(hwnd, &target);
    }
}

/// Drag `text` / `html` / `internal` (modal until the button goes up); the effect of the drop
/// (`DROPEFFECT_MOVE` 2: the text went elsewhere and leaves the document).
pub fn drag(text: &str, html: &str, internal: &str) -> u32 {
    // SAFETY: the data object and source live until DoDragDrop returns; formats are set with
    // blocks the object owns.
    unsafe {
        let Ok(obj) = SHCreateDataObject::<_, IDataObject>(None, None, None::<&IDataObject>) else {
            return 0;
        };
        let wide: Vec<u8> = text
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        set(&obj, CF_UNICODETEXT, &wide);
        let mut r = rtf(text).into_bytes();
        r.push(0);
        set(&obj, format_id("Rich Text Format"), &r);
        if !html.is_empty() {
            let mut h = cf_html(html).into_bytes();
            h.push(0);
            set(&obj, format_id("HTML Format"), &h);
        }
        if !internal.is_empty() {
            let mut i = internal.as_bytes().to_vec();
            i.push(0);
            set(&obj, format_id("BlueOffice.Internal"), &i);
        }
        let source: IDropSource = Source.into();
        let mut effect = DROPEFFECT_NONE;
        let hr = DoDragDrop(
            &obj,
            &source,
            DROPEFFECT_COPY | DROPEFFECT_MOVE,
            &mut effect,
        );
        if hr == DRAGDROP_S_DROP {
            effect.0
        } else {
            0
        }
    }
}
