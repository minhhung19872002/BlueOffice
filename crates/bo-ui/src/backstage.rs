//! The File screen (backstage): navigation down the left, Info on the right.
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Painter, TextStyle};
use bo_render::PathSeg;

/// File screen commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAction {
    /// Leave the File screen.
    Back,
    /// New blank document (a new window).
    New,
    /// Open a file (a new window).
    Open,
    /// Save.
    Save,
    /// Save under another name.
    SaveAs,
    /// Print.
    Print,
    /// Export as PDF.
    ExportPdf,
    /// Close the window.
    Close,
}

const ITEMS: [(FileAction, &str, &str); 7] = [
    (FileAction::New, "backstage.nav.new", "New"),
    (FileAction::Open, "backstage.nav.open", "Open"),
    (FileAction::Save, "backstage.nav.save", "Save"),
    (FileAction::SaveAs, "backstage.nav.saveAs", "Save As"),
    (FileAction::Print, "backstage.nav.print", "Print"),
    (
        FileAction::ExportPdf,
        "backstage.nav.exportPdf",
        "Export to PDF",
    ),
    (FileAction::Close, "backstage.nav.close", "Close"),
];

/// The open File screen.
#[derive(Default)]
pub struct Backstage {
    top: f32,
    size: (f32, f32),
    scale: f32,
    hover: Option<usize>,
}

impl Backstage {
    /// Place below the title bar (`top`) in a window of `size`.
    pub fn layout(&mut self, top: f32, size: (f32, f32), scale: f32) {
        (self.top, self.size, self.scale) = (top, size, scale);
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn nav_w(&self) -> f32 {
        self.px(220.0)
    }

    fn row(&self, i: usize) -> (f32, f32) {
        let h = self.px(40.0);
        (self.top + self.px(64.0) + i as f32 * h, h)
    }

    fn index_at(&self, x: f32, y: f32) -> Option<usize> {
        if x >= self.nav_w() {
            return None;
        }
        if y >= self.top && y < self.top + self.px(52.0) {
            return Some(usize::MAX); // back arrow
        }
        (0..ITEMS.len()).find(|&i| {
            let (ry, h) = self.row(i);
            y >= ry && y < ry + h
        })
    }

    /// Pointer moved: `true` when the highlight changed.
    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.index_at(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    /// Click: the chosen action.
    pub fn press(&mut self, x: f32, y: f32) -> Option<FileAction> {
        match self.index_at(x, y)? {
            usize::MAX => Some(FileAction::Back),
            i => Some(ITEMS[i].0),
        }
    }

    /// Paint over the window below the title bar.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        p: &mut dyn Painter,
        t: &Theme,
        strings: &Strings,
        name: &str,
        pages: usize,
        words: usize,
        modified: bool,
    ) {
        let (w, h) = self.size;
        let label = |key: &str, fallback: &'static str| {
            let v = strings.get(key);
            if v == key {
                fallback.to_owned()
            } else {
                v.to_owned()
            }
        };
        p.fill_rect(0.0, self.top, w, h - self.top, t.surface);
        p.fill_rect(0.0, self.top, self.nav_w(), h - self.top, t.accent_strong);
        // Back arrow.
        let (cx, cy) = (self.px(28.0), self.top + self.px(28.0));
        let k = self.px(8.0);
        if self.hover == Some(usize::MAX) {
            p.fill_rect(0.0, self.top, self.nav_w(), self.px(52.0), ON_ACCENT_HOVER);
        }
        p.path(
            &[
                PathSeg::M(cx + k, cy),
                PathSeg::L(cx - k, cy),
                PathSeg::M(cx - k / 2.0, cy - k / 2.0 - 1.0),
                PathSeg::L(cx - k, cy),
                PathSeg::L(cx - k / 2.0, cy + k / 2.0 + 1.0),
            ],
            None,
            Some((t.accent_on, self.scale.max(1.0) * 1.5)),
        );
        let style = TextStyle {
            size: self.px(14.0),
            bold: false,
        };
        for (i, (_, key, fallback)) in ITEMS.iter().enumerate() {
            let (y, rh) = self.row(i);
            if self.hover == Some(i) {
                p.fill_rect(0.0, y, self.nav_w(), rh, ON_ACCENT_HOVER);
            }
            p.text(
                &label(key, fallback),
                self.px(28.0),
                y + (rh - style.size * 1.33) / 2.0,
                style,
                t.accent_on,
            );
        }
        // Info.
        let x = self.nav_w() + self.px(48.0);
        let mut y = self.top + self.px(36.0);
        p.text(
            &label("backstage.nav.info", "Info"),
            x,
            y,
            TextStyle {
                size: self.px(28.0),
                bold: false,
            },
            t.text,
        );
        y += self.px(64.0);
        let state = label(
            if modified {
                "titleBar.unsaved"
            } else {
                "titleBar.saved"
            },
            if modified { "Not saved" } else { "Saved" },
        );
        for line in [
            name.to_owned(),
            format!("{}: {pages}", label("wordCount.pages", "Pages")),
            format!("{}: {words}", label("wordCount.words", "Words")),
            state,
        ] {
            p.text(&line, x, y, style, t.text);
            y += self.px(28.0);
        }
    }
}

/// Translucent white for hovered items on the accent column.
const ON_ACCENT_HOVER: bo_render::Rgba = bo_render::Rgba(255, 255, 255, 40);
