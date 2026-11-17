//! Round 17 helpers of BlueSlide's editing area (N6): Snap objects to grid (View > Grid and
//! Guides: a moved selection's top-left corner lands on the grid, spacing 1/12" by default
//! as on the reference), the Zoom dialog's "Fit", slide titles for Define Custom Show.
use super::SlideView;

impl SlideView {
    /// Grid snapping: the spacing in points when moves snap to the grid.
    pub fn grid_snap(&self) -> Option<f64> {
        self.grid_snap.then_some(self.grid_spacing)
    }

    /// Grid spacing in points (also the spacing of the gridlines drawn).
    pub fn grid_spacing(&self) -> f64 {
        self.grid_spacing
    }

    /// Set grid snapping and spacing (points).
    pub fn set_grid(&mut self, snap: bool, spacing: f64) {
        self.grid_snap = snap;
        self.grid_spacing = spacing.clamp(1.0, 288.0);
    }

    /// Whether the zoom fits the slide to the window.
    pub fn zoom_is_fit(&self) -> bool {
        self.zoom.is_none()
    }

    /// Title text of slide `i` (first line), when it has one.
    pub fn slide_title(&self, i: usize) -> Option<String> {
        let pres = self.engine.presentation()?;
        let s = pres.slides.get(i)?;
        let o = bo_engine::slide_edit::outline::slide_outline(s);
        let t = o.title.lines().next().unwrap_or("").trim().to_owned();
        Some(t)
    }

    /// A move of the selection by (`dx`, `dy`) points, adjusted so its top-left corner
    /// lands on the grid when snapping is on.
    pub(super) fn grid_move(&self, dx: f64, dy: f64) -> (f64, f64) {
        let Some(g) = self.grid_snap() else {
            return (dx, dy);
        };
        let sel = self.selection();
        if sel.is_empty() {
            return (dx, dy);
        }
        let left = sel
            .iter()
            .map(|s| s.rect.cx - s.rect.w / 2.0)
            .fold(f64::INFINITY, f64::min);
        let top = sel
            .iter()
            .map(|s| s.rect.cy - s.rect.h / 2.0)
            .fold(f64::INFINITY, f64::min);
        snap_to(left, top, dx, dy, g)
    }
}

/// The move (`dx`, `dy`) of a box whose corner is at (`left`, `top`), with the corner put on
/// the nearest grid point of spacing `g`.
#[must_use]
pub fn snap_to(left: f64, top: f64, dx: f64, dy: f64, g: f64) -> (f64, f64) {
    let nx = ((left + dx) / g).round() * g;
    let ny = ((top + dy) / g).round() * g;
    (nx - left, ny - top)
}

#[cfg(test)]
mod tests {
    #[test]
    fn grid_snap() {
        // Corner at (10, 10), moved by (7, 2) on a 6 pt grid → lands on (18, 12).
        let (dx, dy) = super::snap_to(10.0, 10.0, 7.0, 2.0, 6.0);
        assert!((dx - 8.0).abs() < 1e-9 && (dy - 2.0).abs() < 1e-9);
    }
}
