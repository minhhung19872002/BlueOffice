//! Workbook views of BlueSheet (round 34): Normal, Page Layout and Page Break Preview from
//! the View tab and the three status bar buttons, as the reference. The view type and its
//! zoom are kept per sheet in the workbook (engine `setSheetView`, ECMA-376 Part 1,
//! §18.3.1.87 sheetView/@view, @zoomScaleNormal / @zoomScalePageLayoutView /
//! @zoomScaleSheetLayoutView); Page Break Preview opens at 60 % the first time, as the
//! reference.
use super::SheetWindow;
use crate::TextMeasure;
use serde_json::json;

/// The three workbook views, in status bar order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Normal,
    PageLayout,
    PageBreak,
}

impl ViewMode {
    fn engine(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::PageLayout => "pageLayout",
            Self::PageBreak => "pageBreakPreview",
        }
    }

    fn from_engine(s: &str) -> Self {
        match s {
            "pageLayout" => Self::PageLayout,
            "pageBreakPreview" => Self::PageBreak,
            _ => Self::Normal,
        }
    }
}

impl SheetWindow {
    /// The view shown.
    pub fn view_mode(&self) -> ViewMode {
        if self.view.page_layout() {
            ViewMode::PageLayout
        } else if self.view.break_preview() {
            ViewMode::PageBreak
        } else {
            ViewMode::Normal
        }
    }

    /// Ruler units of the page layout view: inches when the region measures in inches.
    pub fn set_ruler_inches(&mut self, on: bool) {
        self.ruler_inches = on;
    }

    /// Show `mode`, remembering it and its zoom in the sheet.
    pub(super) fn set_view_mode(&mut self, mode: ViewMode, text: &dyn TextMeasure) -> bool {
        if mode == self.view_mode() {
            return false;
        }
        self.view.commit();
        let state = self
            .view
            .query(json!({ "cmd": "setSheetView", "view": mode.engine() }));
        let mut zoom = state
            .as_ref()
            .and_then(|s| s["zoom"].as_u64())
            .unwrap_or(100);
        if mode == ViewMode::PageBreak && zoom == 100 {
            // The reference opens Page Break Preview at 60 % until another zoom is chosen.
            zoom = 60;
            let _ = self
                .view
                .query(json!({ "cmd": "setSheetView", "zoom": zoom }));
        }
        self.apply_view_mode(mode, zoom as f32 / 100.0, text);
        true
    }

    fn apply_view_mode(&mut self, mode: ViewMode, zoom: f32, text: &dyn TextMeasure) {
        let labels = (
            self.t("sheetNative.pageN", "Page {n}"),
            self.t("sheetNative.addHeader", "Add header"),
        );
        self.view.set_page_labels(labels.0, labels.1);
        self.view
            .set_page_layout(mode == ViewMode::PageLayout, self.ruler_inches);
        self.view.set_break_preview(mode == ViewMode::PageBreak);
        self.view.set_zoom(zoom);
        self.relayout(text);
    }

    /// Show the view the sheet was saved in (opening a file, changing sheets).
    pub(super) fn sync_view_mode(&mut self, text: &dyn TextMeasure) {
        let Some(state) = self.view.query(json!({ "cmd": "sheetViewState" })) else {
            return;
        };
        let mode = ViewMode::from_engine(state["view"].as_str().unwrap_or("normal"));
        let zoom = state["zoom"].as_u64().unwrap_or(100) as f32 / 100.0;
        if mode != self.view_mode() || (zoom - self.view.zoom()).abs() > 1e-3 {
            self.apply_view_mode(mode, zoom, text);
        }
    }

    /// Status bar view buttons (normal, page layout, page break preview), window rectangles.
    pub(super) fn view_button_rects(&self) -> [[f32; 4]; 3] {
        let (w, h) = self.size;
        let sh = self.status_h();
        let bw = self.px(43.2);
        let x0 = w - self.px(353.6);
        std::array::from_fn(|i| [x0 + i as f32 * bw, h - sh, bw, sh])
    }

    /// A press on a status bar view button.
    pub(super) fn view_button_press(
        &mut self,
        x: f32,
        y: f32,
        text: &dyn TextMeasure,
    ) -> Option<bool> {
        let i = self
            .view_button_rects()
            .iter()
            .position(|r| Self::in_rect(*r, x, y))?;
        let mode = [ViewMode::Normal, ViewMode::PageLayout, ViewMode::PageBreak][i];
        Some(self.set_view_mode(mode, text))
    }
}
