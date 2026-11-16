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
mod doc_table;
mod doc_tips;
mod doc_tools;
mod doc_view;
mod editor_actions;
#[cfg(test)]
mod editor_actions_tests;
mod editor_arrange;
#[cfg(test)]
mod editor_arrange_tests;
mod editor_backstage;
mod editor_close;
#[cfg(test)]
mod editor_close_tests;
mod editor_comment;
#[cfg(test)]
mod editor_comment_tests;
mod editor_convert;
mod editor_design;
#[cfg(test)]
mod editor_design_tests;
mod editor_edit;
mod editor_file7;
mod editor_five;
#[cfg(test)]
mod editor_five_tests;
mod editor_format;
#[cfg(test)]
mod editor_format_tests;
mod editor_icons;
mod editor_more;
mod editor_more2;
#[cfg(test)]
mod editor_more_tests;
mod editor_objtext;
#[cfg(test)]
mod editor_objtext_tests;
mod editor_organize;
#[cfg(test)]
mod editor_organize5_tests;
mod editor_pages5;
mod editor_portfolio;
#[cfg(test)]
mod editor_round7_tests;
mod editor_sign;
#[cfg(test)]
mod editor_tests;
mod editor_textbox;
mod editor_tools;
#[cfg(test)]
mod editor_tools_tests;
mod editor_trans7;
mod editor_typewriter;
mod editor_window;
mod icons;
mod input;
mod nav_pane;
mod pdf_window;
mod popup;
mod reader_icons;
mod reader_impose;
mod reader_print;
#[cfg(test)]
mod reader_tests;
mod reader_view;
mod reader_viewx;
mod reader_window;
mod ribbon;
mod ruler;
mod sheet_ribbon;
mod sheet_view;
mod sheet_window;
mod shell;
mod slide_anim_pane;
mod slide_bgpane;
mod slide_comments;
mod slide_pickers;
mod slide_reveal;
mod slide_ribbon;
mod slide_show;
mod slide_view;
mod slide_window;
mod spell_pane;
mod spin_box;
mod strings;
mod table_grid;
mod theme;
mod titlebar;

pub use backstage::{Backstage, FileAction};
pub use dialog::{Dialog, DialogButton, DialogKind};
pub use doc_view::{DocView, PageSlot, CANVAS_BG};
pub use editor_window::{EditTool, EditorWindow};
pub use icons::{parse_path, IconColors, Icons};
pub use input::{Input, Key, Mods};
pub use pdf_window::PdfWindow;
pub use popup::{Choice, Menu, MenuItem, MenuLook};
pub use reader_impose::{
    plan as print_plan, BookletSides, Duplex, PageOrder, Placement, PlanOptions, PrintLayout, Sheet,
};
pub use reader_print::{
    black_white, parse_pages, Orientation, Paper, PrintAction, PrintColor, PrintDialog,
    PrintScaling, PrintSettings,
};
pub use reader_view::{Fit, Layout, PdfStore, ReaderView, THUMB_KEY};
pub use reader_window::{Panel, ReaderRequest, ReaderWindow, Tool};
pub use ribbon::{Ribbon, RibbonHit};
pub use ruler::Ruler;
pub use sheet_view::{CellFormat, GridBand, GridMove, GridRing, GridStore, SheetView, Stats};
pub use sheet_window::{PaintHash, SheetWindow};
pub use shell::{DocWindow, Request};
pub use slide_view::{Focus, SlideView, ViewMode, SLIDE_CANVAS};
pub use slide_window::{InsertFile, SlideWindow, WindowRequest};
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

    /// Bring image `index` up to date with a slide show layer `list` drawn on a transparent
    /// background (shapes that animate over the slide). Backends without transparent images
    /// draw it as a page.
    fn update_layer(
        &mut self,
        index: usize,
        list: &PageList,
        engine: &bo_engine::Engine,
        scale: f32,
    ) -> Result<(), String> {
        self.update(index, list, engine, scale, None)
    }
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
    /// Draw the `src` part (device pixels of the stored image) of page `index` into `dest`
    /// (scaled when the sizes differ) at `opacity` 0–1, for slide show transitions and
    /// effects. Backends without scaling or opacity draw it 1:1, clipped, when mostly opaque.
    fn page_part(&mut self, index: usize, src: [f32; 4], dest: [f32; 4], opacity: f32) {
        if opacity >= 0.5 {
            self.push_clip(dest[0], dest[1], dest[2], dest[3]);
            self.page(index, (dest[0] - src[0]) as i32, (dest[1] - src[1]) as i32);
            self.pop_clip();
        }
    }
    /// Draw like [`Painter::page_part`], turned `degrees` clockwise about the centre of `dest`
    /// (slide show spin, Morph). Backends without rotation draw it unturned.
    fn page_part_rotated(
        &mut self,
        index: usize,
        src: [f32; 4],
        dest: [f32; 4],
        opacity: f32,
        degrees: f32,
    ) {
        let _ = degrees;
        self.page_part(index, src, dest, opacity);
    }
    /// Start cached layer `id` over `rect` (x, y, width, height) showing content `key`.
    /// `true`: the backend's image of the layer still shows `key` and was drawn, so the
    /// caller skips painting; `false`: paint the layer, then call [`Painter::layer_end`].
    /// Backends without layers paint directly (the default).
    fn layer_begin(&mut self, id: usize, rect: [f32; 4], key: u64) -> bool {
        let _ = (id, rect, key);
        false
    }
    /// End the layer started by [`Painter::layer_begin`] (its image goes on the frame).
    fn layer_end(&mut self, id: usize) {
        let _ = id;
    }
}
