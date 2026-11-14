//! Self-drawn user interface of BlueOffice, shared by every platform (ADR-0018).
//!
//! This crate holds the interface state and logic only. A platform host turns OS events into
//! [`Input`], keeps page images in a [`PageStore`] (GPU bitmaps on Windows, canvases on the web),
//! measures text with [`TextMeasure`] and paints frames through a [`Painter`]. Editing goes
//! straight to the in-process [`bo_engine::Engine`]; nothing is serialised.
#![forbid(unsafe_code)]

mod backstage;
mod catalog;
mod commands;
mod dialog;
mod doc_view;
mod icons;
mod input;
mod popup;
mod ribbon;
mod ruler;
mod shell;
mod strings;
mod theme;
mod titlebar;

pub use backstage::{Backstage, FileAction};
pub use dialog::{Dialog, DialogButton, DialogKind};
pub use doc_view::{DocView, PageSlot, CANVAS_BG};
pub use icons::{parse_path, IconColors, Icons};
pub use input::{Input, Key, Mods};
pub use popup::{Choice, Menu, MenuItem};
pub use ribbon::{Ribbon, RibbonHit};
pub use ruler::Ruler;
pub use shell::{DocWindow, Request};
pub use strings::Strings;
pub use theme::{parse_color, Theme};
pub use titlebar::{NcHit, TitleBar};

use bo_render::{PageList, PathSeg, Rgba};

/// Page images kept by a backend, one per page index.
pub trait PageStore {
    /// Bring page `index` up to date with `list` at `scale` device pixels per point (fonts and
    /// pictures come from `engine`). `band` (left, top, width, height in device pixels) limits
    /// the redraw to the part that changed since the image was last drawn at the same scale;
    /// `None` redraws the whole page.
    fn update(
        &mut self,
        index: usize,
        list: &PageList,
        engine: &bo_engine::Engine,
        scale: f32,
        band: Option<[u32; 4]>,
    ) -> Result<(), String>;

    /// Forget the image of page `index`.
    fn remove(&mut self, index: usize);
}

/// Interface text style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// Font size in device pixels.
    pub size: f32,
    /// Semibold.
    pub bold: bool,
}

/// A font for previews (style gallery, font list): family, size in device pixels, weight
/// (400 regular, 700 bold), italic and underline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Font<'a> {
    /// Family name.
    pub family: &'a str,
    /// Size in device pixels.
    pub size: f32,
    /// OpenType weight.
    pub weight: u16,
    /// Italic.
    pub italic: bool,
    /// Underlined.
    pub underline: bool,
}

impl Font<'_> {
    fn as_style(&self) -> TextStyle {
        TextStyle {
            size: self.size,
            bold: self.weight >= 600,
        }
    }
}

/// Text measurement by the platform's text engine (DirectWrite, canvas `measureText`, …).
pub trait TextMeasure {
    /// Width and height in device pixels of `text` in `style`.
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32);
    /// Width and height of `text` in `font` (backends without font choice use the UI font).
    fn measure_font(&self, text: &str, font: &Font<'_>) -> (f32, f32) {
        self.measure(text, font.as_style())
    }
}

/// Drawing of one frame, in device pixels.
pub trait Painter {
    /// Fill the whole frame.
    fn clear(&mut self, color: Rgba);
    /// Fill a rectangle.
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba);
    /// Draw the stored image of page `index` 1:1 with its top-left corner at (`x`, `y`).
    fn page(&mut self, index: usize, x: i32, y: i32);
    /// Draw text with its layout box's top-left corner at (`x`, `y`).
    fn text(&mut self, text: &str, x: f32, y: f32, style: TextStyle, color: Rgba);
    /// Draw text in `font` (backends without font choice use the UI font).
    fn text_font(&mut self, text: &str, x: f32, y: f32, font: &Font<'_>, color: Rgba) {
        self.text(text, x, y, font.as_style(), color);
    }
    /// Fill and / or stroke (round caps and joins) a path.
    fn path(&mut self, segs: &[PathSeg], fill: Option<Rgba>, stroke: Option<(Rgba, f32)>);
    /// Restrict drawing to a rectangle until [`Painter::pop_clip`].
    fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32);
    /// Undo the last [`Painter::push_clip`].
    fn pop_clip(&mut self);
}
