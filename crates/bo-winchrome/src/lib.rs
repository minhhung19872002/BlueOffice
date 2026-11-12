//! Native window chrome of the BlueOffice desktop shell on Windows (ADR-0016).
//!
//! The desktop windows have no system frame (`decorations: false`): the web title bar draws
//! the caption and its minimise / maximise / close buttons. Windows 11 only offers its snap
//! layouts flyout when the window under the cursor answers `WM_NCHITTEST` with `HTMAXBUTTON`,
//! which the web view (a child window filling the client area) never does. This crate puts a
//! small invisible child window over the web maximise button that answers `HTMAXBUTTON`,
//! reports hover / press back to the web UI and maximises or restores the window on click — the
//! same technique as Tauri's own undecorated resize borders.
//!
//! [`to_physical`] maps the button rectangle the web UI reports (CSS pixels of the web view,
//! with its device pixel ratio) to physical client pixels; it is pure and tested on every
//! platform. The Win32 part ([`win32`]) only exists on Windows.

#[cfg(windows)]
pub mod win32;

/// A rectangle in CSS pixels of the web view (as `getBoundingClientRect` returns it).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CssRect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// A rectangle in physical pixels of the window's client area (Win32 `RECT` convention:
/// `right` / `bottom` exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhysRect {
    /// Left edge.
    pub left: i32,
    /// Top edge.
    pub top: i32,
    /// Right edge (exclusive).
    pub right: i32,
    /// Bottom edge (exclusive).
    pub bottom: i32,
}

impl PhysRect {
    /// Width in pixels.
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    /// Height in pixels.
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
    /// Whether the point (client pixels) is inside.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// Smallest and largest device pixel ratio accepted from the web view (100 %–500 % scaling).
const SCALE_RANGE: (f64, f64) = (0.5, 5.0);

/// Maps a CSS rectangle of the web view to physical client pixels.
///
/// `scale` is the web view's `devicePixelRatio` (monitor scale × page zoom); the web view fills
/// the client area from its origin, so CSS `(0, 0)` is client `(0, 0)`. Edges are rounded
/// outwards (the hit area never shrinks below the drawn button), a top edge within one CSS
/// pixel of the client top snaps to 0 (Fitts: the topmost row of a maximised window belongs to
/// the button), and the result is clipped to `client` (width, height). `None` when the input is
/// not finite, the scale is out of range or nothing is left after clipping.
pub fn to_physical(rect: CssRect, scale: f64, client: (i32, i32)) -> Option<PhysRect> {
    let finite = [rect.x, rect.y, rect.width, rect.height, scale]
        .iter()
        .all(|v| v.is_finite());
    if !finite || rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }
    if !(SCALE_RANGE.0..=SCALE_RANGE.1).contains(&scale) {
        return None;
    }
    let top_css = if rect.y < 1.0 { 0.0 } else { rect.y };
    let left = (rect.x * scale).floor();
    let top = (top_css * scale).floor();
    let right = ((rect.x + rect.width) * scale).ceil();
    let bottom = ((rect.y + rect.height) * scale).ceil();
    let clip = |v: f64, max: i32| v.clamp(0.0, f64::from(max.max(0))) as i32;
    let r = PhysRect {
        left: clip(left, client.0),
        top: clip(top, client.1),
        right: clip(right, client.0),
        bottom: clip(bottom, client.1),
    };
    (r.width() > 0 && r.height() > 0).then_some(r)
}

/// First Windows build with Mica (Windows 11 21H2); older builds get the solid fallback.
pub const MICA_MIN_BUILD: u32 = 22000;

/// Whether a Windows build has the Mica backdrop.
pub fn has_mica(build: u32) -> bool {
    build >= MICA_MIN_BUILD
}

#[cfg(test)]
mod tests {
    use super::*;

    fn css(x: f64, y: f64, width: f64, height: f64) -> CssRect {
        CssRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn maps_at_100_and_150_percent() {
        // A 46×40 button left of the close button of a 1280 px wide window.
        let r = css(1188.0, 0.0, 46.0, 40.0);
        assert_eq!(
            to_physical(r, 1.0, (1280, 860)),
            Some(PhysRect {
                left: 1188,
                top: 0,
                right: 1234,
                bottom: 40
            })
        );
        assert_eq!(
            to_physical(r, 1.5, (1920, 1290)),
            Some(PhysRect {
                left: 1782,
                top: 0,
                right: 1851,
                bottom: 60
            })
        );
    }

    #[test]
    fn rounds_outwards_at_fractional_scales() {
        let r = to_physical(css(10.3, 4.2, 46.0, 32.0), 1.25, (4000, 4000)).unwrap_or_default();
        // 10.3 × 1.25 = 12.875 → 12; (10.3 + 46) × 1.25 = 70.375 → 71.
        assert_eq!((r.left, r.right), (12, 71));
        // 4.2 × 1.25 = 5.25 → 5; 36.2 × 1.25 = 45.25 → 46.
        assert_eq!((r.top, r.bottom), (5, 46));
    }

    #[test]
    fn snaps_to_the_top_edge_and_clips_to_the_client() {
        let r = to_physical(css(1250.0, 0.6, 46.0, 40.0), 2.0, (2560, 70)).unwrap_or_default();
        assert_eq!(r.top, 0);
        assert_eq!(r.bottom, 70);
        assert_eq!(r.right, 2560);
        assert!(r.contains(2559, 0));
        assert!(!r.contains(2560, 0));
    }

    #[test]
    fn rejects_bad_input() {
        let r = css(0.0, 0.0, 46.0, 40.0);
        assert_eq!(to_physical(r, f64::NAN, (100, 100)), None);
        assert_eq!(to_physical(r, 0.0, (100, 100)), None);
        assert_eq!(to_physical(r, 9.0, (100, 100)), None);
        assert_eq!(to_physical(css(0.0, 0.0, 0.0, 40.0), 1.0, (100, 100)), None);
        assert_eq!(
            to_physical(css(f64::INFINITY, 0.0, 4.0, 4.0), 1.0, (100, 100)),
            None
        );
        // Entirely outside the client area.
        assert_eq!(to_physical(css(200.0, 0.0, 46.0, 40.0), 1.0, (100, 100)), None);
        assert_eq!(to_physical(r, 1.0, (0, 0)), None);
    }

    #[test]
    fn mica_needs_windows_11() {
        assert!(!has_mica(19045));
        assert!(has_mica(22000));
        assert!(has_mica(26100));
    }
}
