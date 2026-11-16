//! Self-drawn dialog forms of BlueSheet (Format Cells, Sort, Data Validation, Name Manager,
//! Insert Function, Goal Seek …): list boxes, drop-down boxes, spin boxes, check boxes, option
//! buttons, colour swatches, push buttons and areas drawn by the owner, on tab pages.
//!
//! Sizes and colours were measured black-box on the reference spreadsheet's Format Cells
//! dialog at 125 % display scale (docs/ui/REPORT-sheet.md round 13): 692 × 636 px window,
//! 38 px title bar #F3F3F3, face #F0F0F0, tab page #F9F9F9 framed #E5E5E5, list boxes framed
//! #828790 with 17 px rows, selection #0078D7, push buttons #FDFDFD framed #D0D0D0 (the default
//! one #0078D4), Segoe UI 9 pt (15 px). Coordinates of controls are in those reference pixels
//! and scale with the window's display scale.
use crate::{Font, Key, Mods, Painter, TextMeasure};
use bo_render::{PathSeg, Rgba};

const TITLE_BG: Rgba = Rgba(0xf3, 0xf3, 0xf3, 255);
const FACE: Rgba = Rgba(0xf0, 0xf0, 0xf0, 255);
const PAGE: Rgba = Rgba(0xf9, 0xf9, 0xf9, 255);
const PAGE_EDGE: Rgba = Rgba(0xe5, 0xe5, 0xe5, 255);
const LIST_EDGE: Rgba = Rgba(0x82, 0x87, 0x90, 255);
const EDIT_EDGE: Rgba = Rgba(0x7a, 0x7a, 0x7a, 255);
const SEL: Rgba = Rgba(0x00, 0x78, 0xd7, 255);
const BUTTON: Rgba = Rgba(0xfd, 0xfd, 0xfd, 255);
const BUTTON_EDGE: Rgba = Rgba(0xd0, 0xd0, 0xd0, 255);
const DEFAULT_EDGE: Rgba = Rgba(0x00, 0x78, 0xd4, 255);
const HOVER: Rgba = Rgba(0xe5, 0xf1, 0xfb, 255);
const GROUP_LINE: Rgba = Rgba(0xdc, 0xdc, 0xdc, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);
const GREY_INK: Rgba = Rgba(0x83, 0x83, 0x83, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const ARROW: Rgba = Rgba(0x60, 0x60, 0x60, 255);

/// Reference pixels per point of display scale (the reference was measured at 125 %).
const REF_SCALE: f32 = 1.25;
/// Title bar and tab strip (reference pixels).
const TITLE_H: f32 = 38.0;
/// List box row.
pub(super) const ROW_H: f32 = 17.0;

/// Draws an owner-drawn area: control id, device rectangle.
pub(super) type CanvasFn<'a> = dyn FnMut(&str, [f32; 4], &mut dyn Painter) + 'a;

/// What a form is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum FormKind {
    /// Format Cells (six pages).
    #[default]
    FormatCells,
    /// Multi-level Sort (Data tab).
    Sort,
    /// Data Validation (Data tab).
    Validation,
    /// What-If Analysis: Goal Seek.
    GoalSeek,
    /// Name Manager (Formulas tab).
    Names,
    /// Insert Function with categories and argument help (Formulas tab).
    Function,
    /// Print Titles: rows repeated at the top of each printed page (Page Layout tab).
    PrintTitles,
    /// Help tab: about BlueSheet and its keyboard shortcuts.
    Help,
    /// Custom AutoFilter (filter drop-down submenu).
    CustomFilter,
    /// Top 10 AutoFilter.
    Top10,
    /// Insert tab: Create Table, Create PivotTable, Create Sparklines, Insert Hyperlink,
    /// Header / Footer, Symbol.
    CreateTable,
    CreatePivot,
    Sparklines,
    Link,
    HeaderFooter,
    Symbol,
    /// More Colors… of a colour drop-down (over a stashed form).
    MoreColors,
    /// Style (New Cell Style… of the Cell Styles gallery).
    CellStyle,
    /// Data tab: Queries & Connections, Consolidate, What-If Data Table; View tab: Custom
    /// Views, Zoom.
    Queries,
    Consolidate,
    WhatIfTable,
    CustomViews,
    Zoom,
    /// Chart Design: Select Data Source, Edit Series, Axis Labels, Move Chart.
    ChartData,
    ChartSeries,
    ChartAxisLabels,
    MoveChart,
    /// Table Design: Table Name, Resize Table.
    TableName,
    TableResize,
    /// Conditional formatting: a rule dialog (Greater Than…), New / Edit Formatting Rule, the
    /// rules manager.
    CfRule,
    CfNewRule,
    CfManager,
    /// Paste Special.
    PasteSpecial,
    /// Sheet tab menu: Unhide, Tab Color.
    Unhide,
    TabColor,
    /// New Table Style….
    TableStyle,
    /// File › Options.
    Options,
    /// Formulas › Evaluate Formula.
    Evaluate,
    /// Options… of the Sort dialog (over the stashed Sort form).
    SortOptions,
    /// New / Edit Name over the stashed Name Manager.
    NewName,
    /// Data tab: Subtotal, Remove Duplicates, Advanced Filter.
    Subtotal,
    RemoveDups,
    AdvancedFilter,
    /// Data › Text to Columns wizard and its Advanced Text Import Settings.
    TextToColumns,
    TtcAdvanced,
    /// A dialog collapsed to its reference box while cells are picked on the sheet.
    RangePick,
    /// Review: Protect Sheet / Protect Workbook / passwords.
    Protect,
    /// Review: Spelling.
    Spelling,
}

/// The value of a colour drop-down that has no colour (drawn as an empty swatch; `""` is
/// the automatic colour).
pub(super) const NO_COLOR: &str = "-";

/// What a control is.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Kind {
    /// Text.
    Label(String),
    /// Text in grey (descriptions).
    Note(String),
    /// Group caption with a line to its right (or a frame when the rectangle is tall).
    Group(String),
    /// One-line text box.
    Edit { value: Vec<char>, caret: usize },
    /// Check box.
    Check { label: String, on: bool },
    /// List box.
    List {
        items: Vec<String>,
        sel: Option<usize>,
        top: usize,
    },
    /// Drop-down list box.
    Combo { items: Vec<String>, sel: usize },
    /// Number box with up / down arrows.
    Spin { value: i32, min: i32, max: i32 },
    /// Push button.
    Button { label: String, default: bool },
    /// Colour cells (`None` draws an empty cell), `cols` per row; gaps after rows listed.
    Swatches {
        colors: Vec<Rgba>,
        cols: usize,
        sel: Option<usize>,
        breaks: Vec<usize>,
    },
    /// Area drawn by the owner (previews, the border box, the orientation dial).
    Canvas,
    /// Colour drop-down button (`hex` `RRGGBB`, `""` = automatic) opening a palette of
    /// `palette` colours: 60 theme colours (10 × 6) then 10 standard colours, as the
    /// reference's colour pickers.
    ColorPick { hex: String, palette: Vec<String> },
}

/// A control on a form.
#[derive(Debug, Clone)]
pub(super) struct Ctl {
    pub id: &'static str,
    /// Tab page (None: on every page).
    pub page: Option<usize>,
    /// Left, top, width, height in reference pixels from the form's top-left corner.
    pub rect: [f32; 4],
    pub kind: Kind,
    pub enabled: bool,
    pub visible: bool,
}

/// What an input did.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Event {
    /// Nothing for the owner (a redraw may still be needed).
    None,
    /// The value of a control changed.
    Changed(&'static str),
    /// A push button other than OK / Cancel, or a double click on a list item.
    Clicked(&'static str),
    /// A click in an owner-drawn area at (x, y) reference pixels inside it.
    Canvas(&'static str, f32, f32),
    /// OK (or Enter).
    Ok,
    /// Cancel, Escape or the close button.
    Cancel,
}

/// A modal form.
pub(super) struct Form {
    pub title: String,
    /// Size in reference pixels (including the title bar).
    pub size: (f32, f32),
    pub tabs: Vec<String>,
    pub tab: usize,
    /// Top-left corner in device pixels instead of centred (a collapsed dialog), and no
    /// dimming of the window behind it.
    pub pos: Option<(f32, f32)>,
    /// Edit boxes whose text is shown as bullets (passwords).
    pub masked: Vec<&'static str>,
    pub ctls: Vec<Ctl>,
    focus: Option<usize>,
    /// Open drop-down list (control index) and its highlighted item.
    open: Option<(usize, usize)>,
    hover: Option<usize>,
    /// Top-left corner (device pixels) and device pixels per reference pixel.
    origin: (f32, f32),
    k: f32,
    /// Tab rectangles (reference pixels).
    tab_rects: Vec<[f32; 4]>,
    /// Text widths measured at layout time (device pixels), for painting without a measurer.
    widths: Vec<(String, f32)>,
    /// Width of the focused edit box's text before the caret.
    caret_w: f32,
    /// Notes wrapped to the form's width at layout time (control index, lines).
    wrapped: Vec<(usize, Vec<String>)>,
    /// Open colour palette (control index) and its hovered entry.
    palette: Option<(usize, Option<PaletteHit>)>,
    /// The colour control More Colors… was chosen for.
    pub more_for: Option<&'static str>,
    /// Palette texts: Automatic, Theme Colors, Standard Colors, More Colors….
    pub palette_labels: Vec<String>,
    /// Text size of the controls relative to the classic dialogs (File › Options is larger).
    font_k: f32,
    /// Group captions drawn as bold section headings over a full-width rule (File › Options).
    headings: bool,
    /// Check boxes drawn as option buttons (round; the owner keeps one of a group on).
    radios: Vec<&'static str>,
    /// Access keys: control id and the char index of its label's underlined letter
    /// (Alt+letter presses the button / toggles the box).
    access: Vec<(&'static str, usize)>,
    /// Buttons whose label starts `pad` reference pixels from the left edge (an icon the
    /// owner draws sits before it) instead of being centred.
    left_labels: Vec<(&'static str, f32)>,
    /// Text shown selected in an empty edit box until something is typed.
    placeholders: Vec<(&'static str, String)>,
    /// Labels drawn bold.
    bold: Vec<&'static str>,
}

/// An entry of an open colour palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PaletteHit {
    Automatic,
    Color(usize),
    More,
}

/// Colour palette geometry (reference pixels from its top-left corner).
const PAL_W: f32 = 196.0;
const PAL_H: f32 = 218.0;

fn pal_cell(k: usize) -> [f32; 4] {
    let (row, col) = (k / 10, k % 10);
    let y = match row {
        0 => 50.0,
        1..=5 => 68.0 + (row - 1) as f32 * 15.0,
        _ => 166.0,
    };
    [8.0 + col as f32 * 18.0, y, 14.0, 14.0]
}

fn pal_hit(lx: f32, ly: f32, n: usize) -> Option<PaletteHit> {
    if !(0.0..PAL_W).contains(&lx) || !(0.0..PAL_H).contains(&ly) {
        return None;
    }
    if (4.0..26.0).contains(&ly) {
        return Some(PaletteHit::Automatic);
    }
    if (190.0..212.0).contains(&ly) {
        return Some(PaletteHit::More);
    }
    (0..n)
        .find(|k| in_rect(pal_cell(*k), lx, ly))
        .map(PaletteHit::Color)
}

impl Form {
    /// A form of `size` reference pixels titled `title`, with tab pages when `tabs` is not empty.
    pub fn new(title: &str, size: (f32, f32), tabs: Vec<String>) -> Self {
        Self {
            title: title.to_owned(),
            size,
            tabs,
            tab: 0,
            ctls: Vec::new(),
            focus: None,
            open: None,
            hover: None,
            origin: (0.0, 0.0),
            k: 1.0,
            tab_rects: Vec::new(),
            widths: Vec::new(),
            caret_w: 0.0,
            wrapped: Vec::new(),
            palette: None,
            more_for: None,
            palette_labels: Vec::new(),
            font_k: 1.0,
            headings: false,
            radios: Vec::new(),
            access: Vec::new(),
            left_labels: Vec::new(),
            placeholders: Vec::new(),
            bold: Vec::new(),
            pos: None,
            masked: Vec::new(),
        }
    }

    /// Controls' text `k` times the classic dialogs' size (the title keeps its size).
    pub fn set_font_scale(&mut self, k: f32) {
        self.font_k = k;
    }

    /// Underline letter `at` (char index) of control `id`'s label; Alt+that letter presses
    /// it.
    pub fn set_access(&mut self, id: &'static str, at: usize) {
        self.access.retain(|(i, _)| *i != id);
        self.access.push((id, at));
    }

    /// Show `text` selected in edit box `id` while it is empty (the reference's search hint).
    pub fn set_placeholder(&mut self, id: &'static str, text: &str) {
        self.placeholders.retain(|(i, _)| *i != id);
        self.placeholders.push((id, text.to_owned()));
    }

    /// Draw label `id` bold.
    pub fn set_bold(&mut self, id: &'static str) {
        if !self.bold.contains(&id) {
            self.bold.push(id);
        }
    }

    /// Start button `id`'s label `pad` reference pixels from its left edge.
    pub fn set_label_left(&mut self, id: &'static str, pad: f32) {
        self.left_labels.retain(|(i, _)| *i != id);
        self.left_labels.push((id, pad));
    }

    /// The label prefix before the access letter and the letter, of control `i`.
    fn access_parts(&self, i: usize) -> Option<(String, String)> {
        let c = self.ctls.get(i)?;
        let at = self.access.iter().find(|(id, _)| *id == c.id)?.1;
        let (Kind::Button { label, .. } | Kind::Check { label, .. } | Kind::Label(label)) = &c.kind
        else {
            return None;
        };
        let pre: String = label.chars().take(at).collect();
        let ch: String = label.chars().skip(at).take(1).collect();
        (!ch.is_empty()).then_some((pre, ch))
    }

    /// Draw the access-key underline of control `i` whose label starts at `x`, `y`.
    fn paint_access(&self, p: &mut dyn Painter, i: usize, x: f32, y: f32, ink: Rgba) {
        if let Some((pre, ch)) = self.access_parts(i) {
            let line = self.font().size * 1.33;
            let ux = x + self.width(&pre);
            p.fill_rect(
                ux.round(),
                (y + line - 2.0 * self.k).round(),
                self.width(&ch).max(1.0),
                1.0,
                ink,
            );
        }
    }

    /// Draw the check boxes `ids` as option buttons.
    pub fn set_radios(&mut self, ids: &[&'static str]) {
        self.radios = ids.to_vec();
    }

    /// Draw group captions as bold section headings with a rule under them.
    pub fn set_headings(&mut self, on: bool) {
        self.headings = on;
    }

    /// Measured width of `s` in the dialog font (device pixels; measured at layout time).
    pub fn width(&self, s: &str) -> f32 {
        self.widths.iter().find(|(t, _)| t == s).map_or_else(
            || s.chars().count() as f32 * self.font().size * 0.5,
            |(_, w)| *w,
        )
    }

    /// Measure more strings (the owner's canvas texts) in font `f` at layout time.
    pub fn measure(&mut self, m: &dyn TextMeasure, strings: &[String]) {
        let font = self.font();
        for s in strings {
            if !self.widths.iter().any(|(t, _)| t == s) {
                self.widths.push((s.clone(), m.measure_font(s, &font).0));
            }
        }
    }

    /// Add a control (on page `page`).
    pub fn add(&mut self, id: &'static str, page: Option<usize>, rect: [f32; 4], kind: Kind) {
        self.ctls.push(Ctl {
            id,
            page,
            rect,
            kind,
            enabled: true,
            visible: true,
        });
    }

    /// OK and Cancel at the reference positions (bottom right).
    pub fn ok_cancel(&mut self, ok: &str, cancel: &str) {
        let (w, h) = self.size;
        self.add(
            "ok",
            None,
            [w - 258.0, h - 48.0, 112.0, 28.0],
            Kind::Button {
                label: ok.to_owned(),
                default: true,
            },
        );
        self.add(
            "cancel",
            None,
            [w - 131.0, h - 48.0, 112.0, 28.0],
            Kind::Button {
                label: cancel.to_owned(),
                default: false,
            },
        );
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.ctls.iter().position(|c| c.id == id)
    }

    /// The control `id`.
    pub fn get(&self, id: &str) -> Option<&Ctl> {
        self.ctls.iter().find(|c| c.id == id)
    }

    /// The control `id`, to change.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Ctl> {
        self.ctls.iter_mut().find(|c| c.id == id)
    }

    /// Text of an edit box (or the chosen item of a list / drop-down, or a spin value).
    pub fn text(&self, id: &str) -> String {
        match self.get(id).map(|c| &c.kind) {
            Some(Kind::Edit { value, .. }) => value.iter().collect(),
            Some(Kind::List { items, sel, .. }) => {
                sel.and_then(|i| items.get(i)).cloned().unwrap_or_default()
            }
            Some(Kind::Combo { items, sel }) => items.get(*sel).cloned().unwrap_or_default(),
            Some(Kind::Spin { value, .. }) => value.to_string(),
            _ => String::new(),
        }
    }

    /// Set the text of an edit box (caret at the end).
    pub fn set_text(&mut self, id: &str, s: &str) {
        if let Some(Kind::Edit { value, caret }) = self.get_mut(id).map(|c| &mut c.kind) {
            *value = s.chars().collect();
            *caret = value.len();
        }
    }

    /// Chosen item of a list or drop-down box, swatch cell.
    pub fn sel(&self, id: &str) -> Option<usize> {
        match self.get(id).map(|c| &c.kind) {
            Some(Kind::List { sel, .. } | Kind::Swatches { sel, .. }) => *sel,
            Some(Kind::Combo { sel, .. }) => Some(*sel),
            _ => None,
        }
    }

    /// Choose an item of a list / drop-down box or a swatch (scrolling a list to show it).
    pub fn set_sel(&mut self, id: &str, i: Option<usize>) {
        let rows = self
            .get(id)
            .map_or(1, |c| ((c.rect[3] - 4.0) / ROW_H).floor() as usize);
        match self.get_mut(id).map(|c| &mut c.kind) {
            Some(Kind::List { items, sel, top }) => {
                *sel = i.filter(|i| *i < items.len());
                if let Some(i) = *sel {
                    if i < *top {
                        *top = i;
                    } else if i >= *top + rows.max(1) {
                        *top = i + 1 - rows.max(1);
                    }
                }
            }
            Some(Kind::Swatches { sel, .. }) => *sel = i,
            Some(Kind::Combo { items, sel }) => {
                if let Some(i) = i.filter(|i| *i < items.len()) {
                    *sel = i;
                }
            }
            _ => {}
        }
    }

    /// Replace the items of a list / drop-down box (choosing `sel`).
    pub fn set_items(&mut self, id: &str, new: Vec<String>, choose: Option<usize>) {
        match self.get_mut(id).map(|c| &mut c.kind) {
            Some(Kind::List { items, sel, top }) => {
                *items = new;
                *top = 0;
                *sel = None;
            }
            Some(Kind::Combo { items, sel }) => {
                *items = new;
                *sel = 0;
            }
            _ => return,
        }
        self.set_sel(id, choose);
    }

    /// Check box state.
    pub fn on(&self, id: &str) -> bool {
        matches!(
            self.get(id).map(|c| &c.kind),
            Some(Kind::Check { on: true, .. })
        )
    }

    /// Set a check box.
    pub fn set_on(&mut self, id: &str, v: bool) {
        if let Some(Kind::Check { on, .. }) = self.get_mut(id).map(|c| &mut c.kind) {
            *on = v;
        }
    }

    /// Spin box value.
    pub fn num(&self, id: &str) -> i32 {
        match self.get(id).map(|c| &c.kind) {
            Some(Kind::Spin { value, .. }) => *value,
            _ => 0,
        }
    }

    /// Set a spin box value (clamped).
    pub fn set_num(&mut self, id: &str, v: i32) {
        if let Some(Kind::Spin { value, min, max }) = self.get_mut(id).map(|c| &mut c.kind) {
            *value = v.clamp(*min, *max);
        }
    }

    /// Show or hide a control.
    pub fn show(&mut self, id: &str, v: bool) {
        if let Some(c) = self.get_mut(id) {
            c.visible = v;
        }
    }

    /// Enable or disable a control.
    pub fn enable(&mut self, id: &str, v: bool) {
        if let Some(c) = self.get_mut(id) {
            c.enabled = v;
        }
    }

    /// Show page `i` of a form without a tab strip (its owner draws the page chooser, as the
    /// category list of File › Options).
    pub fn set_page(&mut self, i: usize) {
        self.tab = i;
        self.open = None;
        self.focus = self.first_focus();
    }

    /// Show tab page `i`.
    pub fn set_tab(&mut self, i: usize) {
        if i < self.tabs.len() {
            self.tab = i;
            self.open = None;
            self.focus = self.first_focus();
        }
    }

    fn shown(&self, c: &Ctl) -> bool {
        c.visible && c.page.is_none_or(|p| p == self.tab)
    }

    fn focusable(c: &Kind) -> bool {
        matches!(
            c,
            Kind::Edit { .. }
                | Kind::Check { .. }
                | Kind::List { .. }
                | Kind::Combo { .. }
                | Kind::Spin { .. }
                | Kind::Button { .. }
                | Kind::Swatches { .. }
                | Kind::ColorPick { .. }
        )
    }

    /// Colour of a colour drop-down (`""` = automatic).
    pub fn color(&self, id: &str) -> String {
        match self.get(id).map(|c| &c.kind) {
            Some(Kind::ColorPick { hex, .. }) => hex.clone(),
            _ => String::new(),
        }
    }

    /// Set the colour of a colour drop-down.
    pub fn set_color(&mut self, id: &str, v: &str) {
        if let Some(Kind::ColorPick { hex, .. }) = self.get_mut(id).map(|c| &mut c.kind) {
            v.clone_into(hex);
        }
    }

    /// The open palette's rectangle (reference pixels): under its control.
    fn palette_rect(&self) -> Option<[f32; 4]> {
        let (i, _) = self.palette?;
        let r = self.ctls.get(i)?.rect;
        Some([r[0], r[1] + r[3], PAL_W, PAL_H])
    }

    fn first_focus(&self) -> Option<usize> {
        self.ctls
            .iter()
            .position(|c| self.shown(c) && c.enabled && Self::focusable(&c.kind))
    }

    /// Focus control `id`.
    pub fn focus(&mut self, id: &str) {
        self.focus = self.index(id);
    }

    /// Centre in a window of `size` device pixels at display `scale`.
    pub fn layout(&mut self, m: &dyn TextMeasure, size: (f32, f32), scale: f32) {
        self.k = scale / REF_SCALE;
        let (w, h) = (self.size.0 * self.k, self.size.1 * self.k);
        self.origin = (
            ((size.0 - w) / 2.0).round().max(0.0),
            ((size.1 - h) / 2.0).round().max(0.0),
        );
        if let Some(at) = self.pos {
            self.origin = at;
        }
        // Tabs: label + 30 px, the first at x = 18, y = 50, 24 px high (reference).
        self.tab_rects.clear();
        let mut x = 18.0;
        for t in &self.tabs {
            let tw = (m.measure_font(t, &self.font()).0 / self.k + 34.0).round();
            self.tab_rects.push([x, 50.0, tw, 24.0]);
            x += tw;
        }
        if self.focus.is_none() {
            self.focus = self.first_focus();
        }
        let mut strings: Vec<String> = self.tabs.clone();
        for c in &self.ctls {
            match &c.kind {
                Kind::Button { label, .. } | Kind::Group(label) => strings.push(label.clone()),
                _ => {}
            }
        }
        let parts: Vec<String> = (0..self.ctls.len())
            .filter_map(|i| self.access_parts(i))
            .flat_map(|(a, b)| [a, b])
            .collect();
        let hints: Vec<String> = self.placeholders.iter().map(|(_, t)| t.clone()).collect();
        self.measure(m, &strings);
        self.measure(m, &parts);
        self.measure(m, &hints);
        let font = self.font();
        self.wrapped = self
            .ctls
            .iter()
            .enumerate()
            .filter_map(|(i, c)| match &c.kind {
                Kind::Note(s) => {
                    let max = (self.size.0 - c.rect[0] - 24.0) * self.k;
                    let mut lines = Vec::new();
                    let mut line = String::new();
                    for word in s.split(' ') {
                        let next = if line.is_empty() {
                            word.to_owned()
                        } else {
                            format!("{line} {word}")
                        };
                        if !line.is_empty() && m.measure_font(&next, &font).0 > max {
                            lines.push(std::mem::take(&mut line));
                            line = word.to_owned();
                        } else {
                            line = next;
                        }
                    }
                    lines.push(line);
                    Some((i, lines))
                }
                _ => None,
            })
            .collect();
        self.caret_w = self
            .caret_text()
            .map_or(0.0, |(_, before)| m.measure_font(&before, &font).0);
    }

    /// Dialog font (Segoe UI 9 pt).
    pub fn font(&self) -> Font<'static> {
        Font {
            family: "Segoe UI",
            size: 10.8 * REF_SCALE * self.k * self.font_k,
            weight: 400,
            italic: false,
            underline: false,
        }
    }

    /// Device rectangle of reference rectangle `r`.
    pub fn dev(&self, r: [f32; 4]) -> [f32; 4] {
        [
            (self.origin.0 + r[0] * self.k).round(),
            (self.origin.1 + r[1] * self.k).round(),
            (r[2] * self.k).round(),
            (r[3] * self.k).round(),
        ]
    }

    /// Reference coordinates of device point (x, y).
    pub fn to_local(&self, x: f32, y: f32) -> (f32, f32) {
        self.local(x, y)
    }

    /// Device pixels per reference pixel.
    pub fn scale_k(&self) -> f32 {
        self.k
    }

    /// Top-left corner in device pixels.
    pub fn origin(&self) -> (f32, f32) {
        self.origin
    }

    /// Device point (x, y) is on the form.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let (lx, ly) = self.local(x, y);
        (0.0..self.size.0).contains(&lx) && (0.0..self.size.1).contains(&ly)
    }

    fn local(&self, x: f32, y: f32) -> (f32, f32) {
        ((x - self.origin.0) / self.k, (y - self.origin.1) / self.k)
    }

    fn hit(&self, lx: f32, ly: f32) -> Option<usize> {
        self.ctls.iter().rposition(|c| {
            self.shown(c)
                && c.enabled
                && !matches!(c.kind, Kind::Label(_) | Kind::Note(_) | Kind::Group(_))
                && in_rect(c.rect, lx, ly)
        })
    }

    /// The open drop-down list's rectangle (reference pixels) and its item count.
    fn drop_rect(&self) -> Option<([f32; 4], usize)> {
        let (i, _) = self.open?;
        let c = self.ctls.get(i)?;
        let Kind::Combo { items, .. } = &c.kind else {
            return None;
        };
        let n = items.len().min(12);
        Some((
            [
                c.rect[0],
                c.rect[1] + c.rect[3],
                c.rect[2],
                n as f32 * ROW_H + 2.0,
            ],
            n,
        ))
    }

    /// Pointer moved: `true` when the hovered button / drop-down item changed.
    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        let (lx, ly) = self.local(x, y);
        if let (Some(r), Some((i, old))) = (self.palette_rect(), self.palette) {
            let n = match &self.ctls[i].kind {
                Kind::ColorPick { palette, .. } => palette.len(),
                _ => 0,
            };
            let h = pal_hit(lx - r[0], ly - r[1], n);
            self.palette = Some((i, h));
            return h != old;
        }
        if let (Some((r, n)), Some((i, item))) = (self.drop_rect(), self.open) {
            if in_rect(r, lx, ly) {
                let at = (((ly - r[1] - 1.0) / ROW_H).floor().max(0.0) as usize).min(n - 1);
                if at != item {
                    self.open = Some((i, at));
                    return true;
                }
                return false;
            }
        }
        let h = self
            .hit(lx, ly)
            .filter(|i| matches!(self.ctls[*i].kind, Kind::Button { .. }));
        std::mem::replace(&mut self.hover, h) != h
    }

    /// A click (`count` 2 = double click) at device point (x, y).
    pub fn press(&mut self, x: f32, y: f32, count: u8) -> Event {
        let (lx, ly) = self.local(x, y);
        if let Some((i, _)) = self.palette {
            let r = self.palette_rect().unwrap_or_default();
            self.palette = None;
            let id = self.ctls[i].id;
            let Kind::ColorPick { hex, palette } = &mut self.ctls[i].kind else {
                return Event::None;
            };
            return match pal_hit(lx - r[0], ly - r[1], palette.len()) {
                Some(PaletteHit::Automatic) => {
                    hex.clear();
                    Event::Changed(id)
                }
                Some(PaletteHit::Color(k)) => {
                    palette[k].clone_into(hex);
                    Event::Changed(id)
                }
                Some(PaletteHit::More) => {
                    self.more_for = Some(id);
                    Event::Clicked("moreColors")
                }
                None => Event::None,
            };
        }
        if let Some((i, _)) = self.open.take() {
            if let Some((r, n)) = self.drop_rect_for(i) {
                if in_rect(r, lx, ly) {
                    let at = (((ly - r[1] - 1.0) / ROW_H).floor().max(0.0) as usize).min(n - 1);
                    let id = self.ctls[i].id;
                    if let Kind::Combo { sel, .. } = &mut self.ctls[i].kind {
                        *sel = at;
                    }
                    return Event::Changed(id);
                }
            }
            return Event::None;
        }
        // Close button of the title bar.
        if ly < TITLE_H && lx > self.size.0 - 46.0 {
            return Event::Cancel;
        }
        if let Some(t) = self.tab_rects.iter().position(|r| in_rect(*r, lx, ly)) {
            self.set_tab(t);
            return Event::None;
        }
        let Some(i) = self.hit(lx, ly) else {
            return Event::None;
        };
        self.focus = Some(i);
        let c = &mut self.ctls[i];
        let id = c.id;
        let r = c.rect;
        match &mut c.kind {
            Kind::Edit { value, caret } => {
                *caret = value.len();
                Event::None
            }
            Kind::Check { on, .. } => {
                *on = !*on;
                Event::Changed(id)
            }
            Kind::List { items, sel, top } => {
                let at = *top + ((ly - r[1] - 2.0) / ROW_H).floor().max(0.0) as usize;
                if at < items.len() {
                    *sel = Some(at);
                    if count >= 2 {
                        return Event::Clicked(id);
                    }
                    return Event::Changed(id);
                }
                Event::None
            }
            Kind::Combo { sel, .. } => {
                self.open = Some((i, *sel));
                Event::None
            }
            Kind::Spin { value, min, max } => {
                if lx >= r[0] + r[2] - 18.0 {
                    let up = ly < r[1] + r[3] / 2.0;
                    *value = (*value + if up { 1 } else { -1 }).clamp(*min, *max);
                    return Event::Changed(id);
                }
                Event::None
            }
            Kind::Button { .. } => match id {
                "ok" => Event::Ok,
                "cancel" => Event::Cancel,
                _ => Event::Clicked(id),
            },
            Kind::Swatches {
                colors,
                cols,
                sel,
                breaks,
            } => match swatch_at(r, colors.len(), *cols, breaks, lx, ly) {
                Some(at) => {
                    *sel = Some(at);
                    Event::Changed(id)
                }
                None => Event::None,
            },
            Kind::Canvas => Event::Canvas(id, lx - r[0], ly - r[1]),
            Kind::ColorPick { .. } => {
                self.palette = Some((i, None));
                Event::None
            }
            Kind::Label(_) | Kind::Note(_) | Kind::Group(_) => Event::None,
        }
    }

    fn drop_rect_for(&self, i: usize) -> Option<([f32; 4], usize)> {
        let c = self.ctls.get(i)?;
        let Kind::Combo { items, .. } = &c.kind else {
            return None;
        };
        let n = items.len().min(12);
        (n > 0).then_some((
            [
                c.rect[0],
                c.rect[1] + c.rect[3],
                c.rect[2],
                n as f32 * ROW_H + 2.0,
            ],
            n,
        ))
    }

    /// Mouse wheel over a list box scrolls it.
    pub fn wheel(&mut self, x: f32, y: f32, delta: f32) -> bool {
        let (lx, ly) = self.local(x, y);
        let Some(i) = self.hit(lx, ly) else {
            return false;
        };
        let rows = ((self.ctls[i].rect[3] - 4.0) / ROW_H).floor() as usize;
        if let Kind::List { items, top, .. } = &mut self.ctls[i].kind {
            let max = items.len().saturating_sub(rows.max(1));
            let step = if delta > 0.0 { 3_i64 } else { -3 };
            *top = (*top as i64 - step).clamp(0, max as i64) as usize;
            return true;
        }
        false
    }

    /// Typed text goes to the focused edit / spin box (a space toggles a check box).
    pub fn type_text(&mut self, s: &str) -> Event {
        let Some(i) = self.focus else {
            return Event::None;
        };
        let id = self.ctls[i].id;
        match &mut self.ctls[i].kind {
            Kind::Edit { value, caret } => {
                for ch in s.chars().filter(|c| !c.is_control()) {
                    value.insert(*caret, ch);
                    *caret += 1;
                }
                Event::Changed(id)
            }
            Kind::Spin { value, min, max } => {
                let digits: String = s.chars().filter(char::is_ascii_digit).collect();
                if digits.is_empty() {
                    return Event::None;
                }
                let v = format!("{value}{digits}").parse::<i32>().unwrap_or(*max);
                *value = v.clamp(*min, *max);
                Event::Changed(id)
            }
            Kind::Check { on, .. } if s == " " => {
                *on = !*on;
                Event::Changed(id)
            }
            Kind::List { items, sel, .. } => {
                // Type-ahead: the next item starting with the typed letter.
                let Some(ch) = s.chars().next().map(|c| c.to_lowercase().to_string()) else {
                    return Event::None;
                };
                let start = sel.map_or(0, |v| v + 1);
                let n = items.len();
                let found = (0..n).map(|k| (start + k) % n.max(1)).find(|&k| {
                    items
                        .get(k)
                        .is_some_and(|t| t.to_lowercase().starts_with(&ch))
                });
                match found {
                    Some(at) => {
                        self.set_sel(id, Some(at));
                        Event::Changed(id)
                    }
                    None => Event::None,
                }
            }
            _ => Event::None,
        }
    }

    /// A key: editing, moving in lists, Tab between controls, Enter (OK) and Escape (Cancel).
    pub fn key(&mut self, key: Key, m: Mods) -> Event {
        if let Some((i, item)) = self.open {
            let n = self.drop_rect_for(i).map_or(0, |(_, n)| n);
            match key {
                Key::Escape => self.open = None,
                Key::Up => self.open = Some((i, item.saturating_sub(1))),
                Key::Down => self.open = Some((i, (item + 1).min(n.saturating_sub(1)))),
                Key::Enter => {
                    self.open = None;
                    let id = self.ctls[i].id;
                    if let Kind::Combo { sel, .. } = &mut self.ctls[i].kind {
                        *sel = item;
                    }
                    return Event::Changed(id);
                }
                _ => {}
            }
            return Event::None;
        }
        if let (Key::Letter(ch), true) = (key, m.alt) {
            let want = ch.to_lowercase().next().unwrap_or(ch);
            let hit = (0..self.ctls.len()).find(|&i| {
                let c = &self.ctls[i];
                self.shown(c)
                    && c.enabled
                    && self
                        .access_parts(i)
                        .is_some_and(|(_, l)| l.to_lowercase().starts_with(want))
            });
            if let Some(i) = hit {
                let id = self.ctls[i].id;
                self.focus = Some(i);
                return match &mut self.ctls[i].kind {
                    Kind::Button { .. } => Event::Clicked(id),
                    Kind::Check { on, .. } => {
                        *on = !*on;
                        Event::Changed(id)
                    }
                    _ => Event::None,
                };
            }
        }
        let pages = self.tabs.len();
        match key {
            Key::Tab | Key::PageDown | Key::PageUp if m.ctrl && pages > 1 => {
                let back = m.shift || key == Key::PageUp;
                self.set_tab(if back {
                    (self.tab + pages - 1) % pages
                } else {
                    (self.tab + 1) % pages
                });
                return Event::None;
            }
            Key::Tab => {
                let n = self.ctls.len();
                let mut i = self.focus.unwrap_or(0);
                for _ in 0..n {
                    i = if m.shift {
                        (i + n - 1) % n
                    } else {
                        (i + 1) % n
                    };
                    let c = &self.ctls[i];
                    if self.shown(c) && c.enabled && Self::focusable(&c.kind) {
                        break;
                    }
                }
                self.focus = Some(i);
                return Event::None;
            }
            Key::Escape => return Event::Cancel,
            _ => {}
        }
        let Some(i) = self.focus else {
            return if key == Key::Enter {
                Event::Ok
            } else {
                Event::None
            };
        };
        let id = self.ctls[i].id;
        let rows = ((self.ctls[i].rect[3] - 4.0) / ROW_H).floor().max(1.0) as usize;
        match &mut self.ctls[i].kind {
            Kind::Edit { value, caret } => {
                match key {
                    Key::Left => *caret = caret.saturating_sub(1),
                    Key::Right => *caret = (*caret + 1).min(value.len()),
                    Key::Home => *caret = 0,
                    Key::End => *caret = value.len(),
                    Key::Backspace if *caret > 0 => {
                        *caret -= 1;
                        value.remove(*caret);
                        return Event::Changed(id);
                    }
                    Key::Delete if *caret < value.len() => {
                        value.remove(*caret);
                        return Event::Changed(id);
                    }
                    Key::Enter => return Event::Ok,
                    _ => {}
                }
                Event::None
            }
            Kind::List { items, sel, .. } => {
                let n = items.len();
                let cur = sel.unwrap_or(0);
                let to = match key {
                    Key::Up => cur.saturating_sub(1),
                    Key::Down => (cur + 1).min(n.saturating_sub(1)),
                    Key::PageUp => cur.saturating_sub(rows),
                    Key::PageDown => (cur + rows).min(n.saturating_sub(1)),
                    Key::Home => 0,
                    Key::End => n.saturating_sub(1),
                    Key::Enter => return Event::Ok,
                    _ => return Event::None,
                };
                if n == 0 {
                    return Event::None;
                }
                self.set_sel(id, Some(to));
                Event::Changed(id)
            }
            Kind::Combo { items, sel } => {
                let n = items.len();
                match key {
                    Key::Up if *sel > 0 => *sel -= 1,
                    Key::Down if *sel + 1 < n => *sel += 1,
                    Key::Enter => return Event::Ok,
                    _ => return Event::None,
                }
                Event::Changed(id)
            }
            Kind::Spin { value, min, max } => {
                match key {
                    Key::Up => *value = (*value + 1).min(*max),
                    Key::Down => *value = (*value - 1).max(*min),
                    Key::Backspace => *value /= 10,
                    Key::Enter => return Event::Ok,
                    _ => return Event::None,
                }
                Event::Changed(id)
            }
            Kind::Button { .. } if key == Key::Enter => match id {
                "ok" => Event::Ok,
                "cancel" => Event::Cancel,
                _ => Event::Clicked(id),
            },
            _ if key == Key::Enter => Event::Ok,
            _ => Event::None,
        }
    }

    /// Caret of the focused edit box (device x of the caret is measured at paint time).
    fn caret_text(&self) -> Option<(usize, String)> {
        let i = self.focus?;
        match &self.ctls.get(i)?.kind {
            Kind::Edit { value, caret } if self.masked.contains(&self.ctls[i].id) => {
                Some((i, "\u{25CF}".repeat(value[..*caret].len())))
            }
            Kind::Edit { value, caret } => Some((i, value[..*caret].iter().collect())),
            _ => None,
        }
    }

    /// Paint over the window (dimmed); `canvas` draws the owner's areas (id, device rect).
    #[allow(clippy::too_many_lines)]
    pub fn paint(
        &self,
        p: &mut dyn Painter,
        window: (f32, f32),
        caret_on: bool,
        canvas: &mut CanvasFn<'_>,
    ) {
        if self.pos.is_none() {
            p.fill_rect(0.0, 0.0, window.0, window.1, Rgba(0, 0, 0, 40));
        }
        let f = self.dev([0.0, 0.0, self.size.0, self.size.1]);
        // Shadow, 1 px frame, title bar, face.
        for (o, a) in [(6.0, 12u8), (3.0, 20), (1.0, 30)] {
            p.fill_rect(
                f[0] - o,
                f[1] - o + 3.0,
                f[2] + 2.0 * o,
                f[3] + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(f[0], f[1], f[2], f[3], Rgba(0x9a, 0x9a, 0x9a, 255));
        p.fill_rect(f[0] + 1.0, f[1] + 1.0, f[2] - 2.0, f[3] - 2.0, FACE);
        let t = self.dev([0.0, 0.0, self.size.0, TITLE_H]);
        p.fill_rect(t[0] + 1.0, t[1] + 1.0, t[2] - 2.0, t[3] - 1.0, TITLE_BG);
        let font = self.font();
        let line = font.size * 1.33;
        let text_y = |r: [f32; 4]| (r[1] + (r[3] - line) / 2.0).round();
        let title_font = Font {
            size: font.size / self.font_k,
            ..font
        };
        let title_line = title_font.size * 1.33;
        p.text_font(
            &self.title,
            t[0] + 19.0 * self.k,
            (t[1] + (t[3] - title_line) / 2.0).round(),
            &title_font,
            INK,
        );
        // Close (×).
        let c = self.dev([self.size.0 - 30.0, 13.0, 11.0, 11.0]);
        p.path(
            &[
                PathSeg::M(c[0], c[1]),
                PathSeg::L(c[0] + c[2], c[1] + c[3]),
                PathSeg::M(c[0] + c[2], c[1]),
                PathSeg::L(c[0], c[1] + c[3]),
            ],
            None,
            Some((Rgba(0x40, 0x40, 0x40, 255), 1.0)),
        );
        // Help (?) left of the close button, as the reference's dialogs show it (its centre
        // 96 px from the right edge at 125 %).
        let q = self.dev([self.size.0 - 101.0, 0.0, 10.0, TITLE_H]);
        let qfont = Font {
            size: (font.size * 1.2).round(),
            ..font
        };
        p.text_font(
            "?",
            q[0],
            (q[1] + (q[3] - qfont.size * 1.33) / 2.0).round(),
            &qfont,
            Rgba(0x40, 0x40, 0x40, 255),
        );
        if !self.tabs.is_empty() {
            let pg = self.dev([18.0, 74.0, self.size.0 - 36.0, self.size.1 - 74.0 - 61.0]);
            p.fill_rect(pg[0], pg[1], pg[2], pg[3], PAGE_EDGE);
            p.fill_rect(pg[0] + 1.0, pg[1] + 1.0, pg[2] - 2.0, pg[3] - 2.0, PAGE);
            for (i, (label, r)) in self.tabs.iter().zip(&self.tab_rects).enumerate() {
                let sel = i == self.tab;
                let r = if sel {
                    self.dev([r[0], r[1] - 2.0, r[2], r[3] + 3.0])
                } else {
                    self.dev(*r)
                };
                p.fill_rect(r[0], r[1], r[2], r[3], PAGE_EDGE);
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - if sel { 1.0 } else { 2.0 },
                    if sel { PAGE } else { TITLE_BG },
                );
                let tw = self.width(label);
                p.text_font(
                    label,
                    (r[0] + (r[2] - tw) / 2.0).round(),
                    text_y(r),
                    &font,
                    INK,
                );
            }
        }
        for (i, c) in self.ctls.iter().enumerate() {
            if !self.shown(c) {
                continue;
            }
            let r = self.dev(c.rect);
            let ink = if c.enabled { INK } else { GREY_INK };
            let focused = self.focus == Some(i);
            match &c.kind {
                Kind::Label(s) => {
                    if self.bold.contains(&c.id) {
                        let b = Font {
                            weight: 700,
                            ..font
                        };
                        p.text_font(s, r[0], r[1], &b, ink);
                    } else {
                        p.text_font(s, r[0], r[1], &font, ink);
                    }
                    self.paint_access(p, i, r[0], r[1], ink);
                }
                Kind::Note(s) => match self.wrapped.iter().find(|(k, _)| *k == i) {
                    Some((_, lines)) => {
                        for (n, l) in lines.iter().enumerate() {
                            p.text_font(l, r[0], r[1] + n as f32 * line, &font, ink);
                        }
                    }
                    None => p.text_font(s, r[0], r[1], &font, ink),
                },
                Kind::Group(s) if self.headings => {
                    let bold = Font {
                        weight: 700,
                        ..font
                    };
                    p.text_font(s, r[0], r[1], &bold, ink);
                    let ly = (r[1] + line + 6.0 * self.k).round();
                    p.fill_rect(r[0], ly, r[2], 1.0, Rgba(0xe6, 0xe6, 0xe6, 255));
                }
                Kind::Group(s) => {
                    let tw = self.width(s);

                    let ly = (r[1] + line / 2.0).round();
                    if c.rect[3] > 30.0 {
                        // Frame with the caption over its top edge.
                        let fr = [r[0], ly, r[2], r[3] - (ly - r[1])];
                        p.fill_rect(fr[0], fr[1], 1.0, fr[3], GROUP_LINE);
                        p.fill_rect(fr[0] + fr[2] - 1.0, fr[1], 1.0, fr[3], GROUP_LINE);
                        p.fill_rect(fr[0], fr[1] + fr[3] - 1.0, fr[2], 1.0, GROUP_LINE);
                        p.fill_rect(fr[0], fr[1], 8.0 * self.k, 1.0, GROUP_LINE);
                        let after = r[0] + 8.0 * self.k + tw + 6.0 * self.k;
                        p.fill_rect(after, fr[1], r[0] + r[2] - after, 1.0, GROUP_LINE);
                        // Caption sits on the frame's top edge, indented.
                        p.fill_rect(
                            r[0] + 6.0 * self.k,
                            r[1],
                            tw + 4.0 * self.k,
                            line,
                            if self.tabs.is_empty() { FACE } else { PAGE },
                        );
                        p.text_font(s, r[0] + 8.0 * self.k, r[1], &font, ink);
                    } else {
                        p.text_font(s, r[0], r[1], &font, ink);
                        let x = r[0] + tw + 6.0 * self.k;
                        p.fill_rect(x, ly, r[0] + r[2] - x, 1.0, GROUP_LINE);
                    }
                }
                Kind::Edit { value, .. } => {
                    p.fill_rect(
                        r[0],
                        r[1],
                        r[2],
                        r[3],
                        if focused { SEL } else { EDIT_EDGE },
                    );
                    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
                    p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
                    let s: String = if self.masked.contains(&c.id) {
                        "\u{25CF}".repeat(value.len())
                    } else {
                        value.iter().collect()
                    };
                    let tx = r[0] + 5.0 * self.k;
                    match self.placeholders.iter().find(|(id, _)| *id == c.id) {
                        Some((_, hint)) if s.is_empty() => {
                            // Shown selected, as the reference's search box opens.
                            let w = self.width(hint);
                            p.fill_rect(
                                tx,
                                text_y(r),
                                w,
                                font.size * 1.33,
                                Rgba(0xc8, 0xc8, 0xc8, 255),
                            );
                            p.text_font(hint, tx, text_y(r), &font, ink);
                        }
                        _ => p.text_font(&s, tx, text_y(r), &font, ink),
                    }
                    if focused && caret_on {
                        let cx = (tx + self.caret_w).round();
                        p.fill_rect(cx, r[1] + 4.0 * self.k, 1.0, r[3] - 8.0 * self.k, INK);
                    }
                    p.pop_clip();
                }
                Kind::Check { label, on } if self.radios.contains(&c.id) => {
                    // Option button: a ring, filled in the accent with a white dot when on
                    // (the reference's Windows 11 controls).
                    let b = (13.0 * self.k * self.font_k * 1.12).round();
                    let (bx, by) = (r[0], (r[1] + (r[3] - b) / 2.0).round());
                    let (cx, cy, rr) = (bx + b / 2.0, by + b / 2.0, b / 2.0);
                    let accent = if c.enabled { ACCENT } else { GREY_INK };
                    if *on {
                        p.path(&ring(cx, cy, rr), Some(accent), None);
                        p.path(&ring(cx, cy, rr * 0.42), Some(WHITE), None);
                    } else {
                        let edge = if c.enabled {
                            Rgba(0x80, 0x80, 0x80, 255)
                        } else {
                            Rgba(0xc6, 0xc6, 0xc6, 255)
                        };
                        p.path(&ring(cx, cy, rr - 0.5), Some(WHITE), Some((edge, 1.0)));
                    }
                    p.text_font(label, bx + b + 7.0 * self.k, text_y(r), &font, ink);
                    self.paint_access(p, i, bx + b + 7.0 * self.k, text_y(r), ink);
                }
                Kind::Check { label, on } => {
                    // Windows 11 boxes as the reference's dialogs: 19 px at its larger text,
                    // 15 px in the classic dialogs.
                    let b = (13.0 * self.k * self.font_k * 1.12).round();
                    let (bx, by) = (r[0], (r[1] + (r[3] - b) / 2.0).round());
                    paint_check_box(p, [bx, by, b, b], *on, c.enabled, focused, self.k);
                    p.text_font(label, bx + b + 7.0 * self.k, text_y(r), &font, ink);
                    self.paint_access(p, i, bx + b + 7.0 * self.k, text_y(r), ink);
                }
                Kind::List { items, sel, top } => {
                    p.fill_rect(r[0], r[1], r[2], r[3], LIST_EDGE);
                    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
                    let rows = ((c.rect[3] - 4.0) / ROW_H).floor() as usize;
                    let bar = items.len() > rows;
                    // The reference's Category list keeps a disabled scroll bar when its
                    // items fit (LBS_DISABLENOSCROLL).
                    let idle_bar = !bar && c.id == "cat";
                    let inner_w = r[2] - 2.0 - if bar || idle_bar { 17.0 * self.k } else { 0.0 };
                    p.push_clip(r[0] + 1.0, r[1] + 1.0, inner_w, r[3] - 2.0);
                    for (k, item) in items.iter().enumerate().skip(*top).take(rows + 1) {
                        let y = r[1] + ((2.0 + (k - top) as f32 * ROW_H) * self.k).round();
                        let rh = (ROW_H * self.k).round();
                        let chosen = *sel == Some(k);
                        if chosen {
                            p.fill_rect(r[0] + 2.0, y, inner_w - 2.0, rh, SEL);
                        }
                        p.text_font(
                            item,
                            r[0] + 5.0 * self.k,
                            (y + (rh - line) / 2.0).round(),
                            &font,
                            if chosen { WHITE } else { ink },
                        );
                    }
                    p.pop_clip();
                    if bar {
                        let sx = r[0] + r[2] - 1.0 - 17.0 * self.k;
                        p.fill_rect(sx, r[1] + 1.0, 17.0 * self.k, r[3] - 2.0, FACE);
                        let n = items.len().max(1) as f32;
                        let track = r[3] - 2.0 - 34.0 * self.k;
                        let th = (track * rows as f32 / n).max(16.0 * self.k);
                        let ty = r[1]
                            + 1.0
                            + 17.0 * self.k
                            + (track - th) * (*top as f32) / (n - rows as f32).max(1.0);
                        p.fill_rect(
                            sx + 4.0 * self.k,
                            ty,
                            9.0 * self.k,
                            th,
                            Rgba(0xcd, 0xcd, 0xcd, 255),
                        );
                        arrow(p, sx + 8.5 * self.k, r[1] + 10.0 * self.k, self.k, true);
                        arrow(
                            p,
                            sx + 8.5 * self.k,
                            r[1] + r[3] - 10.0 * self.k,
                            self.k,
                            false,
                        );
                    } else if idle_bar {
                        let sx = r[0] + r[2] - 1.0 - 17.0 * self.k;
                        p.fill_rect(sx, r[1] + 1.0, 17.0 * self.k, r[3] - 2.0, FACE);
                        let dim = Rgba(0xbf, 0xbf, 0xbf, 255);
                        arrow_in(
                            p,
                            sx + 8.5 * self.k,
                            r[1] + 10.0 * self.k,
                            self.k,
                            true,
                            dim,
                        );
                        let by = r[1] + r[3] - 10.0 * self.k;
                        arrow_in(p, sx + 8.5 * self.k, by, self.k, false, dim);
                    }
                }
                Kind::Combo { items, sel } => {
                    let open = self.open.is_some_and(|(o, _)| o == i);
                    p.fill_rect(
                        r[0],
                        r[1],
                        r[2],
                        r[3],
                        if focused || open { SEL } else { EDIT_EDGE },
                    );
                    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
                    let bw = (22.0 * self.k).round();
                    p.fill_rect(
                        r[0] + r[2] - bw - 1.0,
                        r[1] + 1.0,
                        1.0,
                        r[3] - 2.0,
                        EDIT_EDGE,
                    );
                    if let Some(s) = items.get(*sel) {
                        p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - bw - 3.0, r[3] - 2.0);
                        p.text_font(s, r[0] + 5.0 * self.k, text_y(r), &font, ink);
                        p.pop_clip();
                    }
                    chevron(p, r[0] + r[2] - bw / 2.0 - 1.0, r[1] + r[3] / 2.0, self.k);
                }
                Kind::Spin { value, .. } => {
                    p.fill_rect(
                        r[0],
                        r[1],
                        r[2],
                        r[3],
                        if focused { SEL } else { EDIT_EDGE },
                    );
                    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
                    p.text_font(
                        &value.to_string(),
                        r[0] + 5.0 * self.k,
                        text_y(r),
                        &font,
                        ink,
                    );
                    let bw = (18.0 * self.k).round();
                    let bx = r[0] + r[2] - bw - 1.0;
                    p.fill_rect(bx, r[1] + 1.0, 1.0, r[3] - 2.0, EDIT_EDGE);
                    p.fill_rect(bx, (r[1] + r[3] / 2.0).round(), bw, 1.0, EDIT_EDGE);
                    arrow(p, bx + bw / 2.0, r[1] + r[3] * 0.27, self.k * 0.8, true);
                    arrow(p, bx + bw / 2.0, r[1] + r[3] * 0.73, self.k * 0.8, false);
                }
                Kind::Button { label, default } => {
                    let hover = self.hover == Some(i);
                    let edge = if *default || focused || hover {
                        DEFAULT_EDGE
                    } else {
                        BUTTON_EDGE
                    };
                    p.fill_rect(r[0], r[1], r[2], r[3], edge);
                    p.fill_rect(
                        r[0] + 1.0,
                        r[1] + 1.0,
                        r[2] - 2.0,
                        r[3] - 2.0,
                        if hover { HOVER } else { BUTTON },
                    );
                    let tw = self.width(label);
                    let tx = match self.left_labels.iter().find(|(id, _)| *id == c.id) {
                        Some((_, pad)) => (r[0] + pad * self.k).round(),
                        None => (r[0] + (r[2] - tw) / 2.0).round(),
                    };
                    p.text_font(label, tx, text_y(r), &font, ink);
                    self.paint_access(p, i, tx, text_y(r), ink);
                }
                Kind::Swatches {
                    colors,
                    cols,
                    sel,
                    breaks,
                } => {
                    for (k, col) in colors.iter().enumerate() {
                        let s = swatch_rect(c.rect, k, *cols, breaks);
                        let d = self.dev(s);
                        let chosen = *sel == Some(k);
                        p.fill_rect(
                            d[0],
                            d[1],
                            d[2],
                            d[3],
                            if chosen {
                                Rgba(0xe2, 0x6f, 0x0d, 255)
                            } else {
                                Rgba(0xd0, 0xd0, 0xd0, 255)
                            },
                        );
                        p.fill_rect(d[0] + 1.0, d[1] + 1.0, d[2] - 2.0, d[3] - 2.0, *col);
                    }
                }
                Kind::Canvas => canvas(c.id, r, p),
                Kind::ColorPick { hex, .. } => {
                    let open = self.palette.is_some_and(|(o, _)| o == i);
                    p.fill_rect(
                        r[0],
                        r[1],
                        r[2],
                        r[3],
                        if focused || open { SEL } else { EDIT_EDGE },
                    );
                    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
                    let bw = (22.0 * self.k).round();
                    let sr = [
                        r[0] + (5.0 * self.k).round(),
                        r[1] + (5.0 * self.k).round(),
                        r[2] - bw - (10.0 * self.k).round(),
                        r[3] - (10.0 * self.k).round(),
                    ];
                    if hex == NO_COLOR {
                        // No colour: an empty swatch in a grey outline.
                        p.fill_rect(sr[0], sr[1], sr[2], sr[3], EDIT_EDGE);
                        p.fill_rect(sr[0] + 1.0, sr[1] + 1.0, sr[2] - 2.0, sr[3] - 2.0, WHITE);
                    } else {
                        let sw = rgba_hex(hex).unwrap_or(INK);
                        p.fill_rect(sr[0], sr[1], sr[2], sr[3], sw);
                    }
                    chevron(p, r[0] + r[2] - bw / 2.0 - 1.0, r[1] + r[3] / 2.0, self.k);
                }
            }
        }
        if let (Some(pr), Some((i, hot))) = (self.palette_rect(), self.palette) {
            if let Kind::ColorPick { hex, palette } = &self.ctls[i].kind {
                self.paint_palette(p, pr, palette, hex, hot, &font);
            }
        }
        // Open drop-down list on top.
        if let (Some((i, item)), Some((dr, n))) = (self.open, self.drop_rect()) {
            if let Kind::Combo { items, .. } = &self.ctls[i].kind {
                let d = self.dev(dr);
                p.fill_rect(d[0], d[1], d[2], d[3], Rgba(0x64, 0x64, 0x64, 255));
                p.fill_rect(d[0] + 1.0, d[1] + 1.0, d[2] - 2.0, d[3] - 2.0, WHITE);
                for (k, s) in items.iter().enumerate().take(n) {
                    let y = d[1] + ((1.0 + k as f32 * ROW_H) * self.k).round();
                    let rh = (ROW_H * self.k).round();
                    if k == item {
                        p.fill_rect(d[0] + 1.0, y, d[2] - 2.0, rh, SEL);
                    }
                    p.text_font(
                        s,
                        d[0] + 5.0 * self.k,
                        (y + (rh - line) / 2.0).round(),
                        &font,
                        if k == item { WHITE } else { INK },
                    );
                }
            }
        }
    }
}

impl Form {
    /// The open colour palette: Automatic, Theme Colors, Standard Colors, More Colors….
    fn paint_palette(
        &self,
        p: &mut dyn Painter,
        pr: [f32; 4],
        palette: &[String],
        hex: &str,
        hot: Option<PaletteHit>,
        font: &Font<'_>,
    ) {
        let d = self.dev(pr);
        let line = font.size * 1.33;
        let label = |i: usize| self.palette_labels.get(i).map_or("", String::as_str);
        p.fill_rect(d[0], d[1], d[2], d[3], Rgba(0xc6, 0xc6, 0xc6, 255));
        p.fill_rect(d[0] + 1.0, d[1] + 1.0, d[2] - 2.0, d[3] - 2.0, WHITE);
        let at = |x: f32, y: f32, w: f32, h: f32| self.dev([pr[0] + x, pr[1] + y, w, h]);
        let row = |p: &mut dyn Painter, y: f32, s: &str, on: bool| {
            let r = at(3.0, y, PAL_W - 6.0, 22.0);
            if on {
                p.fill_rect(r[0], r[1], r[2], r[3], HOVER);
            }
            let ty = (r[1] + (r[3] - line) / 2.0).round();
            p.text_font(s, r[0] + 30.0 * self.k, ty, font, INK);
        };
        row(p, 4.0, label(0), hot == Some(PaletteHit::Automatic));
        let auto = at(10.0, 9.0, 12.0, 12.0);
        p.fill_rect(auto[0], auto[1], auto[2], auto[3], INK);
        let caption = |p: &mut dyn Painter, y: f32, s: &str| {
            let r = at(0.0, y, PAL_W, 18.0);
            p.fill_rect(r[0] + 1.0, r[1], r[2] - 2.0, r[3], FACE);
            let ty = (r[1] + (r[3] - line) / 2.0).round();
            p.text_font(s, r[0] + 6.0 * self.k, ty, font, INK);
        };
        caption(p, 30.0, label(1));
        caption(p, 146.0, label(2));
        for (k, c) in palette.iter().enumerate() {
            let cell = pal_cell(k);
            let r = at(cell[0], cell[1], cell[2], cell[3]);
            if c.eq_ignore_ascii_case(hex) || hot == Some(PaletteHit::Color(k)) {
                let o = Rgba(0xf2, 0x94, 0x36, 255);
                p.fill_rect(r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0, o);
            }
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xd0, 0xd0, 0xd0, 255));
            let fill = rgba_hex(c).unwrap_or(WHITE);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, fill);
        }
        row(p, 190.0, label(3), hot == Some(PaletteHit::More));
    }
}

/// `RRGGBB` as a colour.
fn rgba_hex(h: &str) -> Option<Rgba> {
    let h = h.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255))
}

fn in_rect(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}

/// Swatch cell `k` of a palette in `r` (16 × 16 px cells every 22 px, an 8 px gap after the
/// rows listed in `breaks`; reference Fill page).
fn swatch_rect(r: [f32; 4], k: usize, cols: usize, breaks: &[usize]) -> [f32; 4] {
    let (row, col) = (k / cols.max(1), k % cols.max(1));
    let gaps = breaks.iter().filter(|b| **b <= row).count() as f32;
    [
        r[0] + col as f32 * 22.0,
        r[1] + row as f32 * 21.0 + gaps * 10.0,
        16.0,
        16.0,
    ]
}

fn swatch_at(
    r: [f32; 4],
    n: usize,
    cols: usize,
    breaks: &[usize],
    x: f32,
    y: f32,
) -> Option<usize> {
    (0..n).find(|k| {
        let s = swatch_rect(r, *k, cols, breaks);
        in_rect([s[0] - 2.0, s[1] - 2.0, s[2] + 4.0, s[3] + 4.0], x, y)
    })
}

/// BlueSheet's accent (the selection green), for checked boxes and option buttons.
const ACCENT: Rgba = Rgba(0x21, 0x73, 0x46, 255);

/// A Windows 11 style check box as the reference's dialogs draw it (measured at 125 %):
/// unchecked white framed #808080; checked filled with the accent, framed a darker shade,
/// a white tick; greyed #C6C6C6. A focused box gets an outer focus line.
fn paint_check_box(
    p: &mut dyn Painter,
    b: [f32; 4],
    on: bool,
    enabled: bool,
    focused: bool,
    k: f32,
) {
    let grey = Rgba(0xc6, 0xc6, 0xc6, 255);
    if focused {
        p.fill_rect(b[0] - 2.0, b[1] - 2.0, b[2] + 4.0, b[3] + 4.0, INK);
        p.fill_rect(b[0] - 1.0, b[1] - 1.0, b[2] + 2.0, b[3] + 2.0, WHITE);
    }
    if on {
        let (edge, fill) = if enabled {
            (Rgba(0x0e, 0x5c, 0x2f, 255), ACCENT)
        } else {
            (grey, grey)
        };
        p.fill_rect(b[0], b[1], b[2], b[3], edge);
        p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, fill);
        let (x, y, s) = (b[0], b[1], b[2]);
        p.path(
            &[
                PathSeg::M(x + s * 0.22, y + s * 0.45),
                PathSeg::L(x + s * 0.42, y + s * 0.65),
                PathSeg::L(x + s * 0.76, y + s * 0.27),
            ],
            None,
            Some((WHITE, (1.6 * k).max(1.4))),
        );
    } else {
        p.fill_rect(
            b[0],
            b[1],
            b[2],
            b[3],
            if enabled {
                Rgba(0x80, 0x80, 0x80, 255)
            } else {
                grey
            },
        );
        p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, WHITE);
    }
}

/// A check box (white, framed #333333, blue when focused; a dark tick).
pub(crate) fn paint_check(
    p: &mut dyn Painter,
    b: [f32; 4],
    on: bool,
    enabled: bool,
    focused: bool,
    k: f32,
) {
    let edge = if !enabled {
        Rgba(0xbc, 0xbc, 0xbc, 255)
    } else if focused {
        SEL
    } else {
        Rgba(0x33, 0x33, 0x33, 255)
    };
    p.fill_rect(b[0], b[1], b[2], b[3], edge);
    p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, WHITE);
    if on {
        let q = b[2] / 4.0;
        p.path(
            &[
                PathSeg::M(b[0] + q * 0.9, b[1] + b[3] / 2.0),
                PathSeg::L(b[0] + b[2] / 2.0 - q / 3.0, b[1] + b[3] - q * 1.1),
                PathSeg::L(b[0] + b[2] - q * 0.8, b[1] + q),
            ],
            None,
            Some((INK, (1.6 * k).max(1.4))),
        );
    }
}

/// Small filled triangle (scroll / spin arrows) centred at (x, y).
fn arrow(p: &mut dyn Painter, x: f32, y: f32, k: f32, up: bool) {
    arrow_in(p, x, y, k, up, ARROW);
}

/// [`arrow`] in colour `c` (disabled arrows are lighter).
fn arrow_in(p: &mut dyn Painter, x: f32, y: f32, k: f32, up: bool, c: Rgba) {
    let (w, h) = (4.0 * k, 3.0 * k);
    let s = if up { -1.0 } else { 1.0 };
    p.path(
        &[
            PathSeg::M(x - w, y - s * h / 2.0),
            PathSeg::L(x + w, y - s * h / 2.0),
            PathSeg::L(x, y + s * h / 2.0),
            PathSeg::Z,
        ],
        Some(c),
        None,
    );
}

/// Drop-down chevron centred at (x, y).
fn chevron(p: &mut dyn Painter, x: f32, y: f32, k: f32) {
    let (w, h) = (5.0 * k, 3.0 * k);
    p.path(
        &[
            PathSeg::M(x - w, y - h / 2.0),
            PathSeg::L(x, y + h / 2.0 + 0.5),
            PathSeg::L(x + w, y - h / 2.0),
        ],
        None,
        Some((ARROW, 1.0)),
    );
}

/// A circle of radius `r` around (`cx`, `cy`) as four cubic Béziers.
fn ring(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let k = r * 0.552_284_8;
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathSeg::Z,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextStyle;

    struct Fixed;
    impl TextMeasure for Fixed {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            (
                text.chars().count() as f32 * style.size * 0.5,
                style.size * 1.3,
            )
        }
    }

    fn form() -> Form {
        let mut f = Form::new("Test", (692.0, 636.0), vec!["One".into(), "Two".into()]);
        f.add(
            "list",
            Some(0),
            [32.0, 108.0, 179.0, 343.0],
            Kind::List {
                items: (0..40).map(|i| format!("Item {i}")).collect(),
                sel: Some(0),
                top: 0,
            },
        );
        f.add(
            "combo",
            Some(0),
            [300.0, 108.0, 150.0, 26.0],
            Kind::Combo {
                items: vec!["a".into(), "b".into(), "c".into()],
                sel: 0,
            },
        );
        f.add(
            "spin",
            Some(0),
            [300.0, 160.0, 80.0, 24.0],
            Kind::Spin {
                value: 2,
                min: 0,
                max: 30,
            },
        );
        f.add(
            "check",
            Some(1),
            [46.0, 280.0, 200.0, 20.0],
            Kind::Check {
                label: "Wrap".into(),
                on: false,
            },
        );
        f.ok_cancel("OK", "Cancel");
        f.layout(&Fixed, (1500.0, 1102.0), 1.25);
        f
    }

    fn at(f: &Form, x: f32, y: f32) -> (f32, f32) {
        let d = f.dev([x, y, 0.0, 0.0]);
        (d[0], d[1])
    }

    #[test]
    fn lists_combos_spins_checks_and_buttons_answer() {
        let mut f = form();
        // Third list row.
        let (x, y) = at(&f, 60.0, 108.0 + 2.0 + 2.0 * ROW_H + 5.0);
        assert_eq!(f.press(x, y, 1), Event::Changed("list"));
        assert_eq!(f.sel("list"), Some(2));
        assert_eq!(f.key(Key::End, Mods::default()), Event::Changed("list"));
        assert_eq!(f.text("list"), "Item 39");
        assert!(matches!(f.get("list").map(|c| &c.kind), Some(Kind::List { top, .. }) if *top > 0));
        // Drop-down: open, pick the third item.
        let (x, y) = at(&f, 350.0, 120.0);
        assert_eq!(f.press(x, y, 1), Event::None);
        let (x, y) = at(&f, 350.0, 134.0 + 1.0 + 2.0 * ROW_H + 4.0);
        assert_eq!(f.press(x, y, 1), Event::Changed("combo"));
        assert_eq!(f.text("combo"), "c");
        // Spin arrows.
        let (x, y) = at(&f, 370.0, 165.0);
        assert_eq!(f.press(x, y, 1), Event::Changed("spin"));
        assert_eq!(f.num("spin"), 3);
        // Second page: the check box.
        let r = f.tab_rects[1];
        let (x, y) = at(&f, r[0] + 5.0, r[1] + 5.0);
        f.press(x, y, 1);
        assert_eq!(f.tab, 1);
        let (x, y) = at(&f, 50.0, 290.0);
        assert_eq!(f.press(x, y, 1), Event::Changed("check"));
        assert!(f.on("check"));
        let (x, y) = at(&f, 692.0 - 200.0, 636.0 - 30.0);
        assert_eq!(f.press(x, y, 1), Event::Ok);
        assert_eq!(f.key(Key::Escape, Mods::default()), Event::Cancel);
    }
}
