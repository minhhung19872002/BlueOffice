//! The page view's UI Automation Text pattern (round 16): the text of the pages in view
//! ([`bo_ui::AccessText`] snapshots), ranges by character, word, line, page and document,
//! their boxes on screen, the selection, finding text and selecting a range (run on the UI
//! thread's next tick, like Invoke).
// Range arithmetic reads best with short names (start / end / text / count).
#![allow(clippy::many_single_char_names)]
use super::{element_simple, hwnd, log, post_tick};
use bo_ui::AccessText;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use windows::core::{implement, AsImpl as _, Error, Interface, Ref, Result, BOOL, BSTR, HRESULT};
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{SafeArrayCreateVector, SafeArrayPutElement};
use windows::Win32::System::Variant::{VARIANT, VT_R8, VT_UNKNOWN};
use windows::Win32::UI::Accessibility::{
    IRawElementProviderSimple, ITextRangeProvider, ITextRangeProvider_Impl,
    TextPatternRangeEndpoint, TextPatternRangeEndpoint_Start, TextUnit, TextUnit_Character,
    TextUnit_Document, TextUnit_Line, TextUnit_Page, TextUnit_Paragraph, UiaPoint,
    UIA_TEXTATTRIBUTE_ID,
};

/// The text now (characters, boxes, page map, selection).
struct Snap {
    chars: Vec<char>,
    boxes: Vec<Option<[f32; 4]>>,
    map: Vec<Option<(usize, usize)>>,
    pages: Vec<usize>,
    selection: Option<(usize, usize)>,
}

static TEXT: Mutex<Option<Snap>> = Mutex::new(None);
/// A range a client selected: (page, first, last character), run on the next tick.
static SELECT: Mutex<Option<(usize, usize, usize)>> = Mutex::new(None);

/// Replace the snapshot; true when the selection changed (a selection event is due).
pub fn update(t: Option<AccessText>) -> bool {
    let Ok(mut slot) = TEXT.lock() else {
        return false;
    };
    let old = slot.as_ref().and_then(|s| s.selection);
    *slot = t.map(|t| Snap {
        chars: t.text.chars().collect(),
        boxes: t.boxes,
        map: t.chars,
        pages: t.pages.iter().map(|p| p.1).collect(),
        selection: t.selection,
    });
    let new = slot.as_ref().and_then(|s| s.selection);
    old != new && new.is_some()
}

/// The selection a client asked for since the last call.
pub fn take_select() -> Option<(usize, usize, usize)> {
    SELECT.lock().ok().and_then(|mut s| s.take())
}

fn len() -> usize {
    TEXT.lock()
        .ok()
        .and_then(|s| s.as_ref().map(|s| s.chars.len()))
        .unwrap_or(0)
}

/// Whether a `unit` starts at `pos` (0 and the end always are boundaries).
fn boundary(s: &Snap, unit: TextUnit, pos: usize) -> bool {
    let n = s.chars.len();
    if pos == 0 || pos >= n {
        return true;
    }
    let prev = s.chars[pos - 1];
    let cur = s.chars[pos];
    match unit {
        u if u == TextUnit_Character => true,
        u if u == TextUnit_Line || u == TextUnit_Paragraph => prev == '\n',
        u if u == TextUnit_Page => s.pages.contains(&pos),
        u if u == TextUnit_Document => false,
        // Word (and Format): a word starts after white space; white space runs belong to the
        // word before them, as text editors report it.
        _ => prev.is_whitespace() && !cur.is_whitespace() || prev == '\n',
    }
}

fn next_boundary(s: &Snap, unit: TextUnit, pos: usize) -> usize {
    let n = s.chars.len();
    let mut p = pos + 1;
    while p < n && !boundary(s, unit, p) {
        p += 1;
    }
    p.min(n)
}

fn prev_boundary(s: &Snap, unit: TextUnit, pos: usize) -> usize {
    let mut p = pos.saturating_sub(1);
    while p > 0 && !boundary(s, unit, p) {
        p -= 1;
    }
    p
}

/// Interface pointers of the live ranges this provider made (a client may hand back only
/// these to `Compare` and friends; anything else is refused).
static LIVE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// A new range provider over `start..end`.
pub fn range(start: usize, end: usize) -> ITextRangeProvider {
    let r: ITextRangeProvider = Range {
        span: Mutex::new((start, end)),
        me: AtomicUsize::new(0),
    }
    .into();
    let raw = r.as_raw() as usize;
    // SAFETY: `r` was made from a `Range` just above.
    unsafe { r.as_impl() }.me.store(raw, Ordering::Relaxed);
    if let Ok(mut l) = LIVE.lock() {
        l.push(raw);
    }
    r
}

/// The Text pattern's selection (one range; an empty one at the start without a selection).
pub fn selection() -> Result<*mut SAFEARRAY> {
    let sel = TEXT
        .lock()
        .ok()
        .and_then(|s| s.as_ref().and_then(|s| s.selection))
        .unwrap_or((0, 0));
    ranges(&[range(sel.0, sel.1)])
}

/// The whole text in view.
pub fn visible() -> Result<*mut SAFEARRAY> {
    ranges(&[range(0, len())])
}

pub fn document() -> ITextRangeProvider {
    range(0, len())
}

/// The degenerate range at the character nearest a screen point.
pub fn from_point(p: &UiaPoint) -> ITextRangeProvider {
    let mut pt = POINT {
        x: p.x as i32,
        y: p.y as i32,
    };
    // SAFETY: plain coordinate conversion for this thread's window.
    let _ = unsafe { ScreenToClient(hwnd(), &mut pt) };
    let (x, y) = (pt.x as f32, pt.y as f32);
    let best = TEXT.lock().ok().and_then(|s| {
        let s = s.as_ref()?;
        s.boxes
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.map(|b| (i, b)))
            .min_by(|a, b| dist(a.1, x, y).total_cmp(&dist(b.1, x, y)))
            .map(|(i, _)| i)
    });
    let at = best.unwrap_or(0);
    range(at, at)
}

fn dist(b: [f32; 4], x: f32, y: f32) -> f32 {
    let dx = (b[0] - x).max(x - b[0] - b[2]).max(0.0);
    let dy = (b[1] - y).max(y - b[1] - b[3]).max(0.0);
    dx * dx + dy * dy * 4.0
}

/// A SAFEARRAY of range providers (VT_UNKNOWN; UIA frees it).
fn ranges(list: &[ITextRangeProvider]) -> Result<*mut SAFEARRAY> {
    // SAFETY: a vector of `list.len()` IUnknown slots, each set to a live provider (the array
    // adds its own reference).
    unsafe {
        let sa = SafeArrayCreateVector(VT_UNKNOWN, 0, list.len() as u32);
        if sa.is_null() {
            return Err(Error::from_hresult(HRESULT(0x8007_000E_u32 as i32)));
        }
        for (i, r) in list.iter().enumerate() {
            let idx = i as i32;
            SafeArrayPutElement(sa, &idx, r.as_raw())?;
        }
        Ok(sa)
    }
}

#[implement(ITextRangeProvider)]
struct Range {
    span: Mutex<(usize, usize)>,
    /// Its interface pointer (an entry of [`LIVE`]).
    me: AtomicUsize,
}

impl Drop for Range {
    fn drop(&mut self) {
        let me = self.me.load(Ordering::Relaxed);
        if let Ok(mut l) = LIVE.lock() {
            l.retain(|&p| p != me);
        }
    }
}

impl Range_Impl {
    fn get(&self) -> (usize, usize) {
        let (a, b) = self.span.lock().map_or((0, 0), |s| *s);
        let n = len();
        (a.min(n), b.min(n))
    }
    fn set(&self, a: usize, b: usize) {
        if let Ok(mut s) = self.span.lock() {
            *s = (a.min(b), a.max(b));
        }
    }
    fn endpoint(&self, e: TextPatternRangeEndpoint) -> usize {
        let (a, b) = self.get();
        if e == TextPatternRangeEndpoint_Start {
            a
        } else {
            b
        }
    }
}

/// The span of another range provider of ours.
fn span_of(r: Ref<ITextRangeProvider>) -> Result<(usize, usize)> {
    let r = r.ok()?;
    let raw = r.as_raw() as usize;
    let ours = LIVE.lock().is_ok_and(|l| l.contains(&raw));
    if !ours {
        // E_INVALIDARG: not a range of this provider.
        return Err(Error::from_hresult(HRESULT(0x8007_0057_u32 as i32)));
    }
    // SAFETY: `raw` is the interface of a live `Range` (checked against LIVE above).
    let imp = unsafe { r.as_impl() };
    Ok(imp.span.lock().map_or((0, 0), |s| *s))
}

impl ITextRangeProvider_Impl for Range_Impl {
    fn Clone(&self) -> Result<ITextRangeProvider> {
        let (a, b) = self.get();
        Ok(range(a, b))
    }
    fn Compare(&self, other: Ref<ITextRangeProvider>) -> Result<BOOL> {
        Ok((span_of(other)? == self.get()).into())
    }
    fn CompareEndpoints(
        &self,
        e: TextPatternRangeEndpoint,
        target: Ref<ITextRangeProvider>,
        te: TextPatternRangeEndpoint,
    ) -> Result<i32> {
        let mine = self.endpoint(e) as i64;
        let (ta, tb) = span_of(target)?;
        let theirs = if te == TextPatternRangeEndpoint_Start {
            ta
        } else {
            tb
        } as i64;
        Ok((mine - theirs).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
    }
    fn ExpandToEnclosingUnit(&self, unit: TextUnit) -> Result<()> {
        let (a, _) = self.get();
        let span = TEXT.lock().ok().and_then(|s| {
            let s = s.as_ref()?;
            let n = s.chars.len();
            if unit == TextUnit_Document || n == 0 {
                return Some((0, n));
            }
            let start = if boundary(s, unit, a) && a < n {
                a
            } else {
                prev_boundary(s, unit, a)
            };
            Some((start, next_boundary(s, unit, start)))
        });
        let (a, b) = span.unwrap_or((0, 0));
        self.set(a, b);
        Ok(())
    }
    fn FindAttribute(
        &self,
        _id: UIA_TEXTATTRIBUTE_ID,
        _val: &VARIANT,
        _backward: BOOL,
    ) -> Result<ITextRangeProvider> {
        Err(Error::empty())
    }
    fn FindText(
        &self,
        text: &BSTR,
        backward: BOOL,
        ignore_case: BOOL,
    ) -> Result<ITextRangeProvider> {
        let needle: Vec<char> = text.to_string().chars().collect();
        let (a, b) = self.get();
        let found = TEXT.lock().ok().and_then(|s| {
            let s = s.as_ref()?;
            let hay = &s.chars[a..b];
            if needle.is_empty() || needle.len() > hay.len() {
                return None;
            }
            let eq = |x: char, y: char| {
                if ignore_case.as_bool() {
                    x.to_lowercase().eq(y.to_lowercase())
                } else {
                    x == y
                }
            };
            let hit = |i: usize| needle.iter().enumerate().all(|(k, c)| eq(hay[i + k], *c));
            let last = hay.len() - needle.len();
            if backward.as_bool() {
                (0..=last).rev().find(|&i| hit(i))
            } else {
                (0..=last).find(|&i| hit(i))
            }
            .map(|i| (a + i, a + i + needle.len()))
        });
        match found {
            Some((x, y)) => Ok(range(x, y)),
            None => Err(Error::empty()),
        }
    }
    fn GetAttributeValue(&self, _id: UIA_TEXTATTRIBUTE_ID) -> Result<VARIANT> {
        Ok(VARIANT::default())
    }
    fn GetBoundingRectangles(&self) -> Result<*mut SAFEARRAY> {
        let (a, b) = self.get();
        // One rectangle per line: characters on the same line merge.
        let mut lines: Vec<[f32; 4]> = Vec::new();
        if let Ok(s) = TEXT.lock() {
            if let Some(s) = s.as_ref() {
                for bx in s.boxes[a..b].iter().flatten() {
                    match lines.last_mut() {
                        Some(l) if (l[1] - bx[1]).abs() < bx[3].max(1.0) * 0.5 => {
                            let right = (l[0] + l[2]).max(bx[0] + bx[2]);
                            let bottom = (l[1] + l[3]).max(bx[1] + bx[3]);
                            l[0] = l[0].min(bx[0]);
                            l[1] = l[1].min(bx[1]);
                            l[2] = right - l[0];
                            l[3] = bottom - l[1];
                        }
                        _ => lines.push(*bx),
                    }
                }
            }
        }
        // SAFETY: a VT_R8 vector of 4 × lines doubles, filled element by element.
        unsafe {
            let sa = SafeArrayCreateVector(VT_R8, 0, (lines.len() * 4) as u32);
            if sa.is_null() {
                return Err(Error::from_hresult(HRESULT(0x8007_000E_u32 as i32)));
            }
            for (i, l) in lines.iter().enumerate() {
                let mut p = POINT {
                    x: l[0].round() as i32,
                    y: l[1].round() as i32,
                };
                let _ = ClientToScreen(hwnd(), &mut p);
                let v = [
                    f64::from(p.x),
                    f64::from(p.y),
                    f64::from(l[2]),
                    f64::from(l[3]),
                ];
                for (k, x) in v.iter().enumerate() {
                    let idx = (i * 4 + k) as i32;
                    SafeArrayPutElement(sa, &idx, std::ptr::from_ref(x).cast())?;
                }
            }
            Ok(sa)
        }
    }
    fn GetEnclosingElement(&self) -> Result<IRawElementProviderSimple> {
        element_simple("doc")
    }
    fn GetText(&self, max: i32) -> Result<BSTR> {
        let (a, b) = self.get();
        let s: String = TEXT
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|s| s.chars[a..b].iter().collect()))
            .unwrap_or_default();
        let s: String = if max >= 0 {
            s.chars().take(max as usize).collect()
        } else {
            s
        };
        Ok(BSTR::from(s.as_str()))
    }
    fn Move(&self, unit: TextUnit, count: i32) -> Result<i32> {
        let (a, b) = self.get();
        let degenerate = a == b;
        let moved = TEXT.lock().ok().and_then(|s| {
            let s = s.as_ref()?;
            let n = s.chars.len();
            let mut pos = if boundary(s, unit, a) {
                a
            } else {
                prev_boundary(s, unit, a)
            };
            let mut moved = 0;
            if count > 0 {
                while moved < count {
                    let next = next_boundary(s, unit, pos);
                    if next >= n || next == pos {
                        break;
                    }
                    pos = next;
                    moved += 1;
                }
            } else {
                while moved > count && pos > 0 {
                    pos = prev_boundary(s, unit, pos);
                    moved -= 1;
                }
            }
            let end = if degenerate {
                pos
            } else {
                next_boundary(s, unit, pos)
            };
            Some((pos, end, moved))
        });
        let (x, y, m) = moved.unwrap_or((0, 0, 0));
        self.set(x, y);
        Ok(m)
    }
    fn MoveEndpointByUnit(
        &self,
        e: TextPatternRangeEndpoint,
        unit: TextUnit,
        count: i32,
    ) -> Result<i32> {
        let (a, b) = self.get();
        let at = self.endpoint(e);
        let moved = TEXT.lock().ok().and_then(|s| {
            let s = s.as_ref()?;
            let n = s.chars.len();
            let mut pos = at;
            let mut moved = 0;
            while moved < count && pos < n {
                pos = next_boundary(s, unit, pos);
                moved += 1;
            }
            while moved > count && pos > 0 {
                pos = prev_boundary(s, unit, pos);
                moved -= 1;
            }
            Some((pos, moved))
        });
        let (pos, m) = moved.unwrap_or((at, 0));
        let (na, nb) = if e == TextPatternRangeEndpoint_Start {
            (pos, b.max(pos))
        } else {
            (a.min(pos), pos)
        };
        if let Ok(mut sp) = self.span.lock() {
            *sp = (na, nb);
        }
        Ok(m)
    }
    fn MoveEndpointByRange(
        &self,
        e: TextPatternRangeEndpoint,
        target: Ref<ITextRangeProvider>,
        te: TextPatternRangeEndpoint,
    ) -> Result<()> {
        let (ta, tb) = span_of(target)?;
        let pos = if te == TextPatternRangeEndpoint_Start {
            ta
        } else {
            tb
        };
        let (a, b) = self.get();
        let span = if e == TextPatternRangeEndpoint_Start {
            (pos, b.max(pos))
        } else {
            (a.min(pos), pos)
        };
        if let Ok(mut sp) = self.span.lock() {
            *sp = span;
        }
        Ok(())
    }
    fn Select(&self) -> Result<()> {
        let (a, b) = self.get();
        let pick = TEXT.lock().ok().and_then(|s| {
            let s = s.as_ref()?;
            let first = s.map[a..b.max(a + 1).min(s.map.len())]
                .iter()
                .flatten()
                .next()
                .copied()?;
            // Within the first character's page.
            let last = s.map[a..b.min(s.map.len())]
                .iter()
                .flatten()
                .rfind(|m| m.0 == first.0)
                .copied()
                .unwrap_or(first);
            Some((first.0, first.1, if a == b { first.1 } else { last.1 }))
        });
        if let Some(p) = pick {
            log(&format!("select {p:?}"));
            if let Ok(mut s) = SELECT.lock() {
                *s = Some(p);
            }
            post_tick();
        }
        Ok(())
    }
    fn AddToSelection(&self) -> Result<()> {
        Err(Error::from_hresult(HRESULT(0x8000_4001_u32 as i32)))
    }
    fn RemoveFromSelection(&self) -> Result<()> {
        Err(Error::from_hresult(HRESULT(0x8000_4001_u32 as i32)))
    }
    fn ScrollIntoView(&self, _top: BOOL) -> Result<()> {
        // The text is the pages in view already.
        Ok(())
    }
    fn GetChildren(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }
}
