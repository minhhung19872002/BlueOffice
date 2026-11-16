//! The BlueSheet window (ADR-0018, plan N6): title bar, ribbon with a Home tab, formula bar,
//! grid, sheet tabs and status bar, with input routing. Sizes and colours follow black-box
//! measurements of a desktop spreadsheet at 125 % display scale (docs/ui/REPORT-sheet.md).
use crate::dialog::Dialog;
use crate::icons::{IconColors, Icons};
use crate::popup::{paint_tooltip, Choice, Menu, MenuItem};
use crate::ribbon::{Ribbon, RibbonHit};
use crate::sheet_ribbon;
use crate::sheet_view::{GridStore, SheetView, SEL_GREEN};
use commands_impl::SheetDialog;

#[path = "sheet_audit.rs"]
mod audit_impl;
#[path = "sheet_cfmenu.rs"]
mod cfmenu_impl;
#[path = "sheet_chart.rs"]
mod chart_impl;
#[path = "sheet_chartdata.rs"]
mod chartdata_impl;
#[path = "sheet_commands.rs"]
mod commands_impl;
#[path = "sheet_comments.rs"]
mod comments_impl;
#[path = "sheet_data.rs"]
mod data_impl;
#[path = "sheet_datadlg.rs"]
mod datadlg_impl;
#[path = "sheet_dataforms.rs"]
mod dataforms_impl;
#[path = "sheet_evaluate.rs"]
mod evaluate_impl;
#[path = "sheet_fbar.rs"]
mod fbar_impl;
#[path = "sheet_fcells.rs"]
mod fcells_impl;
#[path = "sheet_filter.rs"]
mod filter_impl;
#[path = "sheet_form.rs"]
mod form_impl;
#[path = "sheet_gallery.rs"]
mod gallery_impl;
#[path = "sheet_illus.rs"]
mod illus_impl;
#[path = "sheet_insert.rs"]
mod insert_impl;
#[path = "sheet_options.rs"]
mod options_impl;
#[path = "sheet_pagelayout.rs"]
mod pagelayout_impl;
#[path = "sheet_paste.rs"]
mod paste_impl;
#[path = "sheet_review.rs"]
mod review_impl;
#[path = "sheet_tabledesign.rs"]
mod tabledesign_impl;
#[path = "sheet_tabs.rs"]
mod tabs_impl;
#[path = "sheet_totals.rs"]
mod totals_impl;
#[path = "sheet_tstyle.rs"]
mod tstyle_impl;
#[path = "sheet_viewmode.rs"]
mod viewmode_impl;
use crate::strings::Strings;
use crate::theme::Theme;
use crate::titlebar::{NcHit, TitleBar};
use crate::{Font, Input, Key, Mods, Painter, Request, TextMeasure, TextStyle};
use bo_edit::FormatState;
use bo_model::Alignment;
use bo_render::{PathSeg, Rgba};
use serde_json::json;
pub use viewmode_impl::ViewMode;

/// Menus of the chart tabs.
pub(crate) fn chart_menu(id: &str) -> Vec<(String, String)> {
    chart_impl::chart_menu(id)
}

/// SmartArt layout families and built-in equations of the Insert tab's menus.
pub(crate) const SMARTART_FAMILIES: [&str; 7] = illus_impl::SMARTART;
pub(crate) const EQUATION_KEYS: [&str; 5] = [
    illus_impl::EQUATIONS[0].0,
    illus_impl::EQUATIONS[1].0,
    illus_impl::EQUATIONS[2].0,
    illus_impl::EQUATIONS[3].0,
    illus_impl::EQUATIONS[4].0,
];

/// Local time offset from UTC in minutes, set by the host ([`SheetWindow::set_utc_offset`]).
static UTC_OFFSET: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Today's date (`M/D/YYYY`) or the time (`h:mm AM`) in local time.
fn now_stamp(time: bool) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let offset = i64::from(UTC_OFFSET.load(std::sync::atomic::Ordering::Relaxed));
    let t = secs + offset * 60;
    let (days, rem) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    if time {
        let (h, mi) = (rem / 3600, rem % 3600 / 60);
        let (h12, ap) = match h {
            0 => (12, "AM"),
            1..=11 => (h, "AM"),
            12 => (12, "PM"),
            _ => (h - 12, "PM"),
        };
        return format!("{h12}:{mi:02} {ap}");
    }
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mo <= 2);
    format!("{mo}/{d}/{y}")
}

/// Tab strip and status bar greys of the reference spreadsheet.
const TABS_BG: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
const TABS_EDGE: Rgba = Rgba(0x99, 0x99, 0x99, 255);
const STATUS_BG: Rgba = Rgba(0xf3, 0xf3, 0xf3, 255);
const STATUS_EDGE: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const STATUS_TEXT: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
/// Corner mark of a cell with a comment: a red right triangle with 7 px legs inside the cell's
/// top-right corner (measured).
const COMMENT_MARK: Rgba = Rgba(0xff, 0x00, 0x00, 255);

/// Fonts offered by the font box.
const FONTS: [&str; 12] = [
    "Aptos Narrow",
    "Arial",
    "Calibri",
    "Cambria",
    "Candara",
    "Consolas",
    "Courier New",
    "Segoe UI",
    "Tahoma",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
];

/// Font sizes offered by the size box.
const SIZES: [f64; 16] = [
    8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0, 72.0,
];

const ACCOUNTING: &str = "_-* #,##0 [$₫-42A]_-;-* #,##0 [$₫-42A]_-;_-* \"-\" [$₫-42A]_-;_-@_-";
const COMMA_STYLE: &str = "_-* #,##0.00_-;-* #,##0.00_-;_-* \"-\"??_-;_-@_-";

/// Commands of the BlueSheet window.
const COMMANDS: [&str; 34] = [
    "edit.undo",
    "edit.redo",
    "edit.paste",
    "edit.cut",
    "edit.copy",
    "file.save",
    "format.painter",
    "format.growStep",
    "format.shrinkStep",
    "format.bold",
    "format.italic",
    "format.underline",
    "format.borders",
    "format.fillColor",
    "format.color",
    "format.alignTop",
    "format.alignMiddle",
    "format.alignBottom",
    "format.wrap",
    "format.alignLeft",
    "format.alignCenter",
    "format.alignRight",
    "format.outdent",
    "format.indent",
    "format.mergeCenter",
    "format.currency",
    "format.percent",
    "format.comma",
    "format.decimalInc",
    "format.decimalDec",
    "cells.insertRow",
    "cells.deleteRow",
    "cells.autofitCol",
    "cells.insertCol",
];

fn sheet_enabled(id: &str, f: &FormatState) -> bool {
    match id {
        "edit.undo" => f.can_undo,
        "edit.redo" => f.can_redo,
        // No screen capture or 3-D model support in BlueSheet: shown dimmed.
        "sheet.screenshot"
        | "sheet.models3d"
        // Not in BlueSheet yet (shown dimmed like the reference's unavailable commands).
        | "sheet.recentSources"
        | "sheet.fromWeb"
        | "sheet.queryProps"
        | "sheet.editLinks"
        | "sheet.filterReapply"
        | "sheet.relationships"
        | "sheet.dataModel"
        | "sheet.forecastSheet"
        | "sheet.watchWindow"
        | "sheet.ruler"
        | "sheet.newWindow"
        | "sheet.split"
        | "sheet.arrangeAll"
        | "sheet.hideWindow"
        | "sheet.unhideWindow"
        | "sheet.switchWindows"
        | "sheet.macros"
        | "sheet.themeEffects"
        | "sheet.tblExport"
        | "sheet.tblProperties"
        | "sheet.tblBrowser"
        | "sheet.tblUnlink"
        | "sheet.tblRefresh" => false,
        _ => COMMANDS.contains(&id) || id.starts_with("sheet."),
    }
}

/// Tooltip text key of a command.
fn tip_key(id: &str) -> String {
    let own = match id {
        "format.borders" => "sheetEditor.borders",
        "format.fillColor" => "sheetEditor.fillColor",
        "format.color" => "sheetEditor.fontColor",
        "format.alignTop" => "sheetEditor.alignTop",
        "format.alignMiddle" => "sheetEditor.alignMiddle",
        "format.alignBottom" => "sheetEditor.alignBottom",
        "format.wrap" => "sheetEditor.wrap",
        "format.mergeCenter" => "sheetEditor.mergeCenter",
        "format.currency" => "sheetEditor.currency",
        "format.percent" => "sheetEditor.percent",
        "format.comma" => "sheetEditor.comma",
        "format.decimalInc" => "sheetEditor.decimalInc",
        "format.decimalDec" => "sheetEditor.decimalDec",
        "format.outdent" => "sheetEditor.indentDec",
        "format.indent" => "sheetEditor.indentInc",
        _ => "",
    };
    if own.is_empty() {
        format!("commands.{id}")
    } else {
        own.to_owned()
    }
}

/// Number format with one decimal place more (`dir` 1) or less (−1), from `code` and the
/// decimals `shown` by a General cell.
pub fn step_decimals(code: &str, shown: usize, dir: i32) -> String {
    let general = code.is_empty() || code.eq_ignore_ascii_case("general");
    if general {
        let n = (shown as i32 + dir).max(0) as usize;
        return if n == 0 {
            "0".into()
        } else {
            format!("0.{}", "0".repeat(n))
        };
    }
    // Each section of the code gets the same step.
    code.split(';')
        .map(|sec| {
            if let Some(dot) = sec.find('.') {
                let zeros = sec[dot + 1..].chars().take_while(|c| *c == '0').count();
                let end = dot + 1 + zeros;
                if dir > 0 {
                    format!("{}0{}", &sec[..end], &sec[end..])
                } else if zeros <= 1 {
                    format!("{}{}", &sec[..dot], &sec[end..])
                } else {
                    format!("{}{}", &sec[..end - 1], &sec[end..])
                }
            } else if dir > 0 {
                match sec.rfind('0') {
                    Some(i) => format!("{}0.0{}", &sec[..i], &sec[i + 1..]),
                    None => sec.to_owned(),
                }
            } else {
                sec.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Window contents of BlueSheet.
pub struct SheetWindow {
    view: SheetView,
    title: TitleBar,
    name: String,
    requests: Vec<Request>,
    ribbon: Ribbon,
    theme: Theme,
    icons: Icons,
    strings: Strings,
    size: (f32, f32),
    scale: f32,
    menu: Option<Menu>,
    rest: u32,
    tooltip: Option<(String, f32, [f32; 4])>,
    /// Sheet tab rectangles (index, x, width) and the + button.
    tab_rects: Vec<(usize, f32, f32)>,
    /// Measured tab labels: name, bold, width.
    tab_label_widths: Vec<(String, bool, f32)>,
    add_rect: [f32; 4],
    /// Zoom controls: minus, slider track, plus, percentage.
    zoom_rects: [[f32; 4]; 4],
    /// Page layout view ruler in inches (the region's measurement system).
    ruler_inches: bool,
    /// The sheet's saved view (Page Layout / Page Break Preview) was applied.
    view_synced: bool,
    /// Sheet tab renamed in place / dragged.
    tab_edit: Option<tabs_impl::TabEdit>,
    tab_drag: Option<tabs_impl::TabDrag>,
    /// Status bar statistics shown ([`tabs_impl::STATUS_ITEMS`]).
    status_items: [bool; 6],
    /// Hidden sheets listed by Unhide, and the colours of Tab Color.
    unhide_list: Vec<usize>,
    tab_palette: Vec<String>,
    /// New Table Style… being made.
    tstyle: Option<tstyle_impl::TStyleDraft>,
    /// Open function list of the totals row drop-down.
    totals_list: Option<totals_impl::TotalsList>,
    /// Status bar statistics measured at input time (text, width), for painting.
    stat_widths: Vec<(String, f32)>,
    zoom_drag: bool,
    /// Scroll bar drags: pointer offset from the thumb start (vertical, horizontal).
    vdrag: Option<f32>,
    hdrag: Option<f32>,
    /// Scroll bar geometry: (track rect, thumb start, thumb length) for both bars.
    vbar: ([f32; 4], f32, f32),
    hbar: ([f32; 4], f32, f32),
    /// The pointer went down in the grid.
    grid_press: bool,
    /// Formula bar: name box and content box.
    name_rect: [f32; 4],
    fx_rect: [f32; 4],
    /// Format painter armed with this source range.
    painter: Option<serde_json::Value>,
    /// Pointer cursor wanted by the grid (`SheetView::cursor_at`).
    cursor: u8,
    status: String,
    /// The engine's worksheet function names.
    functions: Vec<String>,
    /// Open dialog and what it is for.
    dialog: Option<(SheetDialog, Dialog)>,
    /// Formula bar shown (View tab).
    show_fbar: bool,
    /// The ribbon shows the wide arrangement of the Home tab (it fits the window).
    wide_ribbon: bool,
    /// Lines of the formula bar's content box (1, or 3 when expanded).
    fbar_lines: u32,
    /// The name box while a reference or name is typed into it.
    name_edit: Option<fbar_impl::NameEdit>,
    /// Measured widths of name box text being typed.
    name_widths: Vec<(String, f32)>,
    /// Calculation: manual mode, and formulas waiting for F9 (the status bar says Calculate).
    calc_state: (bool, bool),
    /// Settings of the Text to Columns wizard's first step.
    ttc_values: Vec<String>,
    /// Open self-drawn form (Format Cells, Sort, …), what it is for, and the pointer
    /// position (wheel input scrolls the list under it).
    form: Option<form_impl::Form>,
    form_kind: form_impl::FormKind,
    form_ptr: (f32, f32),
    /// Format Cells state beyond its controls (border edges, orientation, fill).
    fc: fcells_impl::FcState,
    /// Sort dialog: the range sorted and the number of levels shown.
    sort_range: Option<serde_json::Value>,
    sort_levels: usize,
    /// Name Manager rows (name, refers to).
    name_rows: Vec<(String, String)>,
    /// Formula auditing arrows, the traced cell and its precedent / dependent levels.
    arrows: Vec<audit_impl::Arrow>,
    trace_cell: Option<(u32, u32)>,
    trace_levels: (u32, u32),
    /// Comment cards: the hovered comment cell, Show All Comments, and the laid-out cards.
    hover_comment: Option<(u32, u32)>,
    show_all_comments: bool,
    cards: Vec<comments_impl::Card>,
    /// Open AutoFilter drop-down, its texts, the column of its Custom AutoFilter / Top 10
    /// form, and whether the status bar shows its "x of y records found".
    filter: Option<filter_impl::FilterDrop>,
    filter_labels: filter_impl::Labels,
    filter_col: u32,
    filter_status: bool,
    /// The table range `[r0, c0, r1, c1]` (header to last data row) whose filter the open
    /// drop-down edits; `None` for the sheet's AutoFilter.
    pub(super) filter_table: Option<[u32; 4]>,
    /// Application preferences (File › Options), and their text when changed and not yet
    /// taken by the host.
    prefs: options_impl::SheetPrefs,
    prefs_out: Option<String>,
    /// The Options dialog's category.
    options_page: usize,
    /// The list `[r0, c0, r1, c1]` the open Subtotal / Remove Duplicates dialog works on.
    dd_list: Option<[u32; 4]>,
    /// The open Evaluate Formula dialog's steps.
    eval: Option<evaluate_impl::EvalState>,
    /// Create Sparklines: the kind chosen on the ribbon; Symbol: the selected grid cell.
    spark_kind: String,
    symbol: (usize, usize),
    /// A form waiting under More Colors…, and that form's new and current colour.
    form_stash: Option<(form_impl::Form, form_impl::FormKind)>,
    more_colors: (String, String),
    /// Open picture gallery (Cell Styles, Format as Table).
    gallery: Option<gallery_impl::Gallery>,
    /// What a file asked of the host (`Request::PickMergeData`) is for: `"styles"` (Merge
    /// Styles…) or `"text"` (Get Data from Text/CSV).
    pick: Option<&'static str>,
    /// Open Shapes / Icons gallery and the icon library (loaded when Icons first opens).
    picker: Option<crate::slide_pickers::Picker>,
    library: Vec<crate::slide_pickers::LibIcon>,
    /// Selected chart, a chart drag in progress, the charts' anchors for painting.
    chart_sel: Option<usize>,
    chart_drag: Option<chart_impl::ChartDrag>,
    /// Select Data Source state while its form is open.
    chart_data: Option<chartdata_impl::ChartDataState>,
    /// The Table Design tab is shown (the active cell is in a table).
    table_tab: bool,
    /// Open Conditional Formatting menu.
    cf_menu: Option<cfmenu_impl::CfMenu>,
    /// Rule of the open simple rule dialog (greaterThan, top, …).
    cf_which: String,
    /// Rule being edited by Edit Formatting Rule (block, rule).
    cf_edit: Option<(usize, usize)>,
    /// Rules shown by the rules manager and the chosen one.
    cf_rules: Vec<serde_json::Value>,
    cf_pick: Option<usize>,
    chart_cache: Vec<([u32; 2], [u32; 2])>,
}

impl SheetWindow {
    /// Wrap workbook `name` shown by `view` with BlueSheet's interface in `locale`.
    pub fn new(view: SheetView, locale: &str, name: &str) -> Self {
        let mut w = Self {
            view,
            title: {
                let mut t = TitleBar::new();
                t.set_app("BlueSheet", "app.sheet");
                t
            },
            name: name.to_owned(),
            requests: Vec::new(),
            ribbon: Ribbon::with_tabs(sheet_ribbon::tabs_wide())
                .with_enabled(sheet_enabled)
                .with_labeled_widths(|id| {
                    (id == "sheet.numberFormat").then(|| sheet_ribbon::number_box_width(true))
                })
                .with_trailing(sheet_ribbon::trailing)
                .with_late_rows(sheet_ribbon::late)
                .with_large_widths(chart_impl::large_width)
                .with_plain_contextual()
                .with_two_line_labels()
                // Measured on the reference's Insert tab at 125 % (round 27): large buttons
                // 50 px or the label + 6 px; a one-word menu label puts ▾ on its own line.
                .with_large_metrics(40.0, 5.0)
                .with_row_widths(tabledesign_impl::row_width)
                .with_mark_on_own_line(),
            wide_ribbon: true,
            theme: Theme::light("sheet"),
            icons: Icons::load(),
            strings: Strings::load(locale),
            size: (800.0, 600.0),
            scale: 1.0,
            menu: None,
            rest: 0,
            tooltip: None,
            tab_rects: Vec::new(),
            tab_label_widths: Vec::new(),
            add_rect: [0.0; 4],
            zoom_rects: [[0.0; 4]; 4],
            ruler_inches: false,
            view_synced: false,
            tab_edit: None,
            tab_drag: None,
            status_items: tabs_impl::STATUS_ITEMS.map(|s| s.3),
            unhide_list: Vec::new(),
            tab_palette: Vec::new(),
            tstyle: None,
            totals_list: None,
            stat_widths: Vec::new(),
            zoom_drag: false,
            vdrag: None,
            hdrag: None,
            vbar: ([0.0; 4], 0.0, 0.0),
            hbar: ([0.0; 4], 0.0, 0.0),
            grid_press: false,
            name_rect: [0.0; 4],
            fx_rect: [0.0; 4],
            painter: None,
            cursor: 0,
            status: String::new(),
            functions: Vec::new(),
            dialog: None,
            show_fbar: true,
            fbar_lines: 1,
            name_edit: None,
            name_widths: Vec::new(),
            calc_state: (false, false),
            ttc_values: Vec::new(),
            form: None,
            form_kind: form_impl::FormKind::default(),
            form_ptr: (0.0, 0.0),
            fc: fcells_impl::FcState::default(),
            sort_range: None,
            sort_levels: 1,
            name_rows: Vec::new(),
            arrows: Vec::new(),
            trace_cell: None,
            trace_levels: (0, 0),
            hover_comment: None,
            show_all_comments: false,
            cards: Vec::new(),
            filter: None,
            filter_labels: filter_impl::Labels::default(),
            filter_col: 0,
            filter_status: false,
            filter_table: None,
            prefs: options_impl::SheetPrefs::default(),
            prefs_out: None,
            options_page: 0,
            dd_list: None,
            eval: None,
            spark_kind: String::new(),
            symbol: (0, 0),
            form_stash: None,
            more_colors: (String::new(), String::new()),
            gallery: None,
            pick: None,
            picker: None,
            library: Vec::new(),
            chart_sel: None,
            chart_data: None,
            table_tab: false,
            cf_menu: None,
            cf_which: String::new(),
            cf_edit: None,
            cf_rules: Vec::new(),
            cf_pick: None,
            chart_drag: None,
            chart_cache: Vec::new(),
        };
        insert_impl::add_icons(&mut w.icons);
        data_impl::add_icons(&mut w.icons);
        chart_impl::add_icons(&mut w.icons);
        tabledesign_impl::add_icons(&mut w.icons);
        pagelayout_impl::add_icons(&mut w.icons);
        data_impl::install_labels(&mut w.strings);
        w.load_functions();
        w.refresh_calc_state();
        w
    }

    /// Local time offset from UTC in minutes (Ctrl+; and Ctrl+Shift+; stamps).
    pub fn set_utc_offset(minutes: i32) {
        UTC_OFFSET.store(minutes, std::sync::atomic::Ordering::Relaxed);
    }

    /// The grid view (saving).
    pub fn view(&self) -> &SheetView {
        &self.view
    }

    /// Apply stored preferences (`key=value` lines of File › Options).
    pub fn set_preferences(&mut self, text: &str) {
        self.set_prefs(options_impl::SheetPrefs::from_text(text));
        self.prefs_out = None;
    }

    /// Run a menu / ribbon command by id (`goto.<ref>` selects a range), as a choice of a
    /// menu does.
    pub fn run_command(&mut self, id: &str, text: &dyn TextMeasure) -> bool {
        let done = self.choose(Choice::Command(id.to_owned()), text);
        self.layout_form(text);
        done
    }

    /// Start the new workbook with the preferences' default font, size and sheets (call once,
    /// before the first frame, when no file was opened).
    pub fn apply_new_workbook_prefs(&mut self, name: &str) -> bool {
        self.new_book_with_prefs(name)
    }

    /// The preferences' text when File › Options changed them since the last call (the host
    /// stores it).
    pub fn take_preferences(&mut self) -> Option<String> {
        self.prefs_out.take()
    }

    /// Default font (`""`: the theme's body font) and size of new workbooks, sheets in a new
    /// workbook, AutoRecover interval in minutes (`None`: off).
    pub fn new_workbook_prefs(&self) -> (String, f64, u32, Option<u32>) {
        let p = &self.prefs;
        (
            p.font.clone(),
            p.size,
            p.sheets,
            p.autosave.then_some(p.autosave_min),
        )
    }

    /// The grid view, for the host's file operations.
    pub fn view_mut(&mut self) -> &mut SheetView {
        &mut self.view
    }

    /// The workbook now has this file name (after Save As).
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }

    /// Requests for the host made since the last call.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// Pointer cursor: `0` cell cross, `1` column resize, `2` row resize, `3` arrow.
    pub fn cursor(&self) -> u8 {
        self.cursor
    }

    /// What a window position is for the window manager.
    pub fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        if self.menu.is_some() {
            return NcHit::Client;
        }
        self.title.hit(x, y)
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    /// Formula bar band height (54 px at 125 %, boxes 30 px high, 12 px above and below).
    fn fbar_h(&self) -> f32 {
        if self.show_fbar {
            self.px(43.2) + self.fbar_extra()
        } else {
            0.0
        }
    }

    /// Sheet tab strip (31 px at 125 %) and status bar (32 px) heights.
    fn tabs_h(&self) -> f32 {
        self.px(24.8)
    }

    fn status_h(&self) -> f32 {
        self.px(25.6)
    }

    /// Vertical scroll bar column (25 px at 125 %: 3 px margins around 19 px boxes).
    fn sb_w(&self) -> f32 {
        self.px(20.0)
    }

    /// Arrow box size and its margin in the scroll bar column.
    fn sb_box(&self) -> (f32, f32) {
        (self.px(15.2), self.px(2.4))
    }

    /// Arrow buttons: up, down, left, right.
    fn arrow_rects(&self) -> [[f32; 4]; 4] {
        let (w, h) = self.size;
        let (b, m) = self.sb_box();
        let gt = self.grid_top();
        let tabs_y = h - self.status_h() - self.tabs_h();
        let x = w - self.sb_w() + m;
        let hy = (tabs_y + (self.tabs_h() - b) / 2.0).round();
        let hx = self.hbar_x();
        [
            [x, gt + 2.0, b, b],
            [x, tabs_y - 2.0 - b, b, b],
            [hx, hy, b, b],
            [w - self.sb_w() - b, hy, b, b],
        ]
    }

    /// Left edge of the horizontal scroll bar in the tab strip.
    fn hbar_x(&self) -> f32 {
        (self.size.0 * 0.6)
            .round()
            .max(self.add_rect[0] + self.add_rect[2] + self.px(24.0))
    }

    fn grid_top(&self) -> f32 {
        self.ribbon.bottom() + self.fbar_h()
    }

    fn relayout(&mut self, text: &dyn TextMeasure) {
        self.title
            .layout(text, &self.strings, &self.name, self.size.0, self.scale);
        let top = self.title.height();
        // The wide Home arrangement when it fits, else the compact one (as the reference
        // ribbon shrinks its groups with the window).
        for wide in [true, false] {
            if self.wide_ribbon != wide {
                self.wide_ribbon = wide;
                let active = self.ribbon.active_tab();
                self.ribbon.set_tabs(if wide {
                    sheet_ribbon::tabs_wide()
                } else {
                    sheet_ribbon::tabs()
                });
                if self.chart_sel.is_some() {
                    self.ribbon.set_contextual(chart_impl::chart_tabs(), false);
                } else if self.table_tab {
                    self.ribbon
                        .set_contextual(tabledesign_impl::table_tabs(), false);
                }
                // The same tab stays active (also a contextual one).
                self.ribbon.set_active_tab(active);
                self.ribbon.set_labeled_widths(if wide {
                    |id| (id == "sheet.numberFormat").then(|| sheet_ribbon::number_box_width(true))
                } else {
                    |id| (id == "sheet.numberFormat").then(|| sheet_ribbon::number_box_width(false))
                });
            }
            self.ribbon
                .layout(text, &self.strings, self.size.0, self.scale, top);
            if !self.ribbon.overflows() {
                break;
            }
        }
        let (w, h) = self.size;
        let gt = self.grid_top();
        let tabs_y = h - self.status_h() - self.tabs_h();
        self.view
            .set_frame(0.0, gt, (w - self.sb_w()).max(1.0), (tabs_y - gt).max(1.0));
        // Formula bar: name box, then the content box after the fx area.
        let fb = self.ribbon.bottom();
        let pad = self.px(9.6);
        let bh = self.px(24.0);
        self.name_rect = [self.px(4.8), fb + pad, self.px(101.6), bh];
        let fx_x = self.px(248.0);
        self.fx_rect = [
            fx_x,
            fb + pad,
            (w - fx_x - self.px(4.8)).max(1.0),
            bh + self.fbar_extra(),
        ];
        // Sheet tabs.
        self.layout_tabs(text);
        // Status bar zoom controls, right to left.
        let y = h - self.status_h();
        let sh = self.status_h();
        // Measured from the right edge at 125 %: percentage 65 px, plus centred 91 px, slider
        // track 257–102 px, minus centred 268 px.
        let pct = [w - self.px(52.0), y, self.px(44.0), sh];
        let plus = [w - self.px(79.2), y, self.px(12.8), sh];
        let track = [w - self.px(205.6), y, self.px(124.0), sh];
        let minus = [w - self.px(220.8), y, self.px(12.8), sh];
        self.zoom_rects = [minus, track, plus, pct];
        self.update_bars();
        self.view.measure_labels(text);
        if !self.view_synced && self.size.0 > 1.0 {
            self.view_synced = true;
            self.sync_view_mode(text);
        }
    }

    fn layout_tabs(&mut self, text: &dyn TextMeasure) {
        let y = self.size.1 - self.status_h() - self.tabs_h();
        let _ = y;
        let style = self.tab_style(false);
        let bold = self.tab_style(true);
        // Measured: first tab 112 px from the left, tab = label + 22 px at 125 %.
        let mut x = self.px(89.6);
        self.tab_rects.clear();
        self.tab_label_widths.clear();
        for (i, name) in self.view.sheet_names().into_iter().enumerate() {
            let Some(name) = name else { continue };
            let (wr, wb) = (text.measure(&name, style).0, text.measure(&name, bold).0);
            self.tab_label_widths.push((name.clone(), false, wr));
            self.tab_label_widths.push((name.clone(), true, wb));
            let w = wr.max(wb);
            let tw = (w + self.px(17.6)).round();
            self.tab_rects.push((i, x, tw));
            x += tw;
        }
        let s = self.px(16.0);
        let ty = self.size.1 - self.status_h() - self.tabs_h();
        self.add_rect = [
            x + self.px(11.2),
            ty + ((self.tabs_h() - s) / 2.0).round(),
            s,
            s,
        ];
    }

    fn tab_label_w(&self, name: &str, bold: bool) -> f32 {
        self.tab_label_widths
            .iter()
            .find(|(n, b, _)| n == name && *b == bold)
            .map_or(name.chars().count() as f32 * self.px(7.0), |e| e.2)
    }

    fn tab_style(&self, bold: bool) -> TextStyle {
        TextStyle {
            size: self.px(13.0),
            bold,
        }
    }

    /// Scroll bar geometry from the view's scroll state.
    fn update_bars(&mut self) {
        // Thumb tracks between the arrow boxes (the reference's layout).
        let [up, down, left, right] = self.arrow_rects();
        let vtrack = [
            up[0],
            up[1] + up[3] + 1.0,
            up[2],
            (down[1] - up[1] - up[3] - 2.0).max(0.0),
        ];
        let (pos, content, view) = self.view.scroll_info(true);
        self.vbar = Self::thumb(vtrack, pos, content, view, true, self.px(16.0));
        let htrack = [
            left[0] + left[2] + 1.0,
            left[1],
            (right[0] - left[0] - left[2] - 2.0).max(0.0),
            left[3],
        ];
        let (pos, content, view) = self.view.scroll_info(false);
        self.hbar = Self::thumb(htrack, pos, content, view, false, self.px(16.0));
    }

    fn thumb(
        track: [f32; 4],
        pos: f64,
        content: f64,
        view: f64,
        vertical: bool,
        min: f32,
    ) -> ([f32; 4], f32, f32) {
        let len = if vertical { track[3] } else { track[2] };
        let frac = if content > 0.0 {
            (view / content).min(1.0)
        } else {
            1.0
        };
        let thumb = (len * frac as f32).max(min).min(len);
        let room = (content - view).max(1.0);
        let start = ((len - thumb) * (pos / room).clamp(0.0, 1.0) as f32).round();
        (track, start, thumb.round())
    }

    fn bar_drag(&mut self, vertical: bool, at: f32) -> bool {
        let (track, _, thumb) = if vertical { self.vbar } else { self.hbar };
        let off = if vertical { self.vdrag } else { self.hdrag };
        let Some(off) = off else { return false };
        let (start, len) = if vertical {
            (track[1], track[3])
        } else {
            (track[0], track[2])
        };
        let t = ((at - off - start) / (len - thumb).max(1.0)).clamp(0.0, 1.0);
        let (_, content, view) = self.view.scroll_info(vertical);
        let moved = self
            .view
            .scroll_to(vertical, f64::from(t) * (content - view).max(0.0));
        self.update_bars();
        moved
    }

    fn bar_press(&mut self, vertical: bool, at: f32) -> bool {
        let (track, start, thumb) = if vertical { self.vbar } else { self.hbar };
        let origin = if vertical { track[1] } else { track[0] };
        let s = origin + start;
        if at >= s && at < s + thumb {
            if vertical {
                self.vdrag = Some(at - s);
            } else {
                self.hdrag = Some(at - s);
            }
            return false;
        }
        // Page towards the pointer.
        let (pos, _, view) = self.view.scroll_info(vertical);
        let to = if at < s { pos - view } else { pos + view };
        let moved = self.view.scroll_to(vertical, to);
        self.update_bars();
        moved
    }

    /// Ribbon formatting state of the active cell.
    fn format_state(&self) -> FormatState {
        let f = self.view.format();
        let e = self.view.engine();
        FormatState {
            bold: f.bold,
            italic: f.italic,
            underline: f.underline,
            strike: f.strike,
            font: Some(f.font.clone()),
            size_half_points: Some((f.size * 2.0).round() as u32),
            align: match f.h_align.as_str() {
                "center" | "centerContinuous" => Alignment::Center,
                "right" => Alignment::Right,
                "justify" => Alignment::Justify,
                _ => Alignment::Left,
            },
            can_undo: e.can_undo(),
            can_redo: e.can_redo(),
            ..FormatState::default()
        }
    }

    fn on_ids(&self) -> Vec<&'static str> {
        let f = self.view.format();
        let mut on = Vec::new();
        match f.v_align.as_str() {
            "top" => on.push("format.alignTop"),
            "center" => on.push("format.alignMiddle"),
            _ => on.push("format.alignBottom"),
        }
        if f.wrap {
            on.push("format.wrap");
        }
        if self.painter.is_some() {
            on.push("format.painter");
        }
        if self.view.show_formulas() {
            on.push("sheet.showFormulas");
        }
        if self.show_all_comments {
            on.push("sheet.showComments");
        }
        on
    }

    /// Show / print options drawn as check boxes over their icons, as the reference's View
    /// tab (Show group) and Page Layout tab (Sheet Options) show them: (id, checked).
    fn check_items(&self) -> [(&'static str, bool); 5] {
        let setup = self
            .view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
            .map(|ws| (ws.page_setup.print_grid_lines, ws.page_setup.print_headings))
            .unwrap_or_default();
        [
            ("sheet.gridlines", self.view.gridlines()),
            ("sheet.headings", self.view.headings()),
            ("sheet.formulaBar", self.show_fbar),
            ("sheet.printGridlines", setup.0),
            ("sheet.printHeadings", setup.1),
        ]
    }

    /// Check boxes over the icons of [`Self::check_items`] (13 px box, dark frame, tick).
    fn paint_check_items(&self, p: &mut dyn Painter) {
        for (id, on) in self.check_items() {
            let Some([x, y, _, h]) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
                continue;
            };
            // Cover the icon (18 px at x + 5) with the ribbon face, then the box.
            let size = self.px(18.0);
            let ix = x + self.px(5.0);
            let iy = y + (h - size) / 2.0;
            p.fill_rect(
                ix - 1.0,
                iy - 1.0,
                size + 2.0,
                size + 2.0,
                self.theme.ribbon,
            );
            let b = self.px(13.0);
            let (bx, by) = ((ix + (size - b) / 2.0).round(), (y + (h - b) / 2.0).round());
            form_impl::paint_check(p, [bx, by, b, b], on, true, false, self.scale);
        }
    }

    fn patch(&mut self, patch: serde_json::Value) -> bool {
        let mut cmd = json!({ "cmd": "format", "range": self.view.rng() });
        cmd["patch"] = patch;
        self.view.run(cmd)
    }

    /// Run window command `id`; `true` when the window changed.
    fn action(&mut self, id: &str, text: &dyn TextMeasure) -> bool {
        if let Some(done) = self.sheet_action(id, text) {
            return done;
        }
        if let Some(done) = self.tabs_action(id, text) {
            return done;
        }
        if let Some(done) = self.totals_action(id) {
            return done;
        }
        let f = self.view.format().clone();
        match id {
            "edit.undo" => self.view.undo(),
            "edit.redo" => self.view.redo(),
            "file.save" => {
                self.view.commit();
                self.requests.push(Request::Save);
                true
            }
            "file.saveAs" => {
                self.requests.push(Request::SaveAs);
                true
            }
            "file.open" => {
                self.requests.push(Request::Open);
                false
            }
            "file.new" => {
                self.requests.push(Request::New);
                false
            }
            "file.close" => {
                self.requests.push(Request::Close);
                false
            }
            "file.options" => self.open_options(),
            "edit.copy" | "edit.cut" => {
                let text = self.view.copy(id == "edit.cut");
                self.requests.push(Request::SetClipboard {
                    text,
                    html: String::new(),
                    internal: String::new(),
                });
                true
            }
            "edit.paste" => {
                self.requests.push(Request::Paste);
                false
            }
            "format.painter" => {
                self.painter = if self.painter.is_some() {
                    None
                } else {
                    Some(self.view.rng())
                };
                true
            }
            "format.bold" => self.patch(json!({ "bold": !f.bold })),
            "format.italic" => self.patch(json!({ "italic": !f.italic })),
            "format.underline" => {
                self.patch(json!({ "underline": if f.underline { "none" } else { "single" } }))
            }
            "format.strike" => self.patch(json!({ "strike": !f.strike })),
            "format.growStep" | "format.shrinkStep" => {
                let grow = id == "format.growStep";
                let next = if grow {
                    SIZES
                        .iter()
                        .copied()
                        .find(|s| *s > f.size)
                        .unwrap_or(f.size + 10.0)
                } else {
                    SIZES
                        .iter()
                        .rev()
                        .copied()
                        .find(|s| *s < f.size)
                        .unwrap_or(1.0)
                };
                self.patch(json!({ "fontSize": next }))
            }
            "format.borders" => {
                let range = self.view.rng();
                self.view.run(json!({
                    "cmd": "borders", "range": range,
                    "spec": { "preset": "bottom", "style": "thin" },
                }))
            }
            "format.fillColor" => self.patch(json!({ "fillColor": "FFFF00" })),
            "format.color" => self.patch(json!({ "fontColor": "FF0000" })),
            "format.alignTop" | "format.alignMiddle" | "format.alignBottom" => {
                let v = match id {
                    "format.alignTop" => "top",
                    "format.alignMiddle" => "center",
                    _ => "bottom",
                };
                self.patch(json!({ "vAlign": v }))
            }
            "format.alignLeft" | "format.alignCenter" | "format.alignRight" => {
                let h = match id {
                    "format.alignLeft" => "left",
                    "format.alignCenter" => "center",
                    _ => "right",
                };
                // A second click returns to General alignment.
                let h = if f.h_align == h { "" } else { h };
                self.patch(json!({ "hAlign": h }))
            }
            "format.wrap" => self.patch(json!({ "wrap": !f.wrap })),
            "format.indent" | "format.outdent" => {
                let cur = self.indent();
                let n = if id == "format.indent" {
                    cur + 1
                } else {
                    cur.saturating_sub(1)
                };
                self.patch(json!({ "indent": n }))
            }
            "format.mergeCenter" => {
                let range = self.view.rng();
                let (r0, c0, r1, c1) = self.view.range_bounds();
                let merged = self.view.engine().workbook().is_some_and(|w| {
                    w.sheets.get(self.view.sheet()).is_some_and(|ws| {
                        ws.merges.iter().any(|m| {
                            m.first.row <= r1
                                && m.last.row >= r0
                                && m.first.col <= c1
                                && m.last.col >= c0
                        })
                    })
                });
                if merged {
                    self.view.run(json!({ "cmd": "unmerge", "range": range }))
                } else {
                    self.view
                        .run(json!({ "cmd": "merge", "range": range, "center": true }))
                }
            }
            "format.currency" => self.patch(json!({ "numFmt": ACCOUNTING })),
            "format.percent" => self.patch(json!({ "numFmt": "0%" })),
            "format.comma" => self.patch(json!({ "numFmt": COMMA_STYLE })),
            "format.decimalInc" | "format.decimalDec" => {
                let display = self.view.formula_display();
                let shown = display.rsplit_once(['.', ',']).map_or(0, |(_, d)| {
                    d.chars().take_while(char::is_ascii_digit).count()
                });
                let dir = if id == "format.decimalInc" { 1 } else { -1 };
                let code = step_decimals(&f.num_fmt, shown, dir);
                self.patch(json!({ "numFmt": code }))
            }
            "cells.insertRow" | "cells.deleteRow" | "cells.insertCol" => {
                let (r0, c0, r1, c1) = self.view.range_bounds();
                let rows = id != "cells.insertCol";
                let insert = id != "cells.deleteRow";
                let range = if rows {
                    json!({ "r0": r0, "c0": 0, "r1": r1, "c1": bo_model::sheet::MAX_COLS - 1 })
                } else {
                    json!({ "r0": 0, "c0": c0, "r1": bo_model::sheet::MAX_ROWS - 1, "c1": c1 })
                };
                self.view.run(json!({
                    "cmd": "shiftCells", "range": range, "down": rows, "insert": insert,
                }))
            }
            "cells.autofitCol" => {
                let (_, c0, _, c1) = self.view.range_bounds();
                self.view
                    .run(json!({ "cmd": "autofit", "rows": false, "first": c0, "last": c1 }))
            }
            _ => false,
        }
    }

    fn indent(&self) -> u32 {
        self.view
            .engine()
            .workbook()
            .and_then(|w| {
                let ws = w.sheets.get(self.view.sheet())?;
                let (r, c) = {
                    let (r0, c0, _, _) = self.view.range_bounds();
                    (r0, c0)
                };
                let cell = ws.cell(r, c)?;
                let xf = w.styles.xf(cell.style)?;
                xf.alignment.as_ref().and_then(|a| a.indent)
            })
            .unwrap_or(0)
    }

    /// Paste the clipboard the host read (`text` form).
    pub fn paste(&mut self, text: Option<String>) -> bool {
        self.view.paste(text)
    }

    fn zoom_to(&mut self, z: f32, text: &dyn TextMeasure) -> bool {
        let changed = self.view.set_zoom(z);
        if changed {
            self.relayout(text);
        }
        changed
    }

    fn slider_zoom(&self, x: f32) -> f32 {
        let track = self.zoom_rects[1];
        let t = ((x - track[0]) / track[2]).clamp(0.0, 1.0);
        // Left half 10–100 %, right half 100–400 %, as on desktop spreadsheets.
        let z = if t <= 0.5 {
            0.1 + t / 0.5 * 0.9
        } else {
            1.0 + (t - 0.5) / 0.5 * 3.0
        };
        (z * 100.0).round() / 100.0
    }

    fn slider_pos(&self) -> f32 {
        let track = self.zoom_rects[1];
        let z = self.view.zoom();
        let t = if z <= 1.0 {
            (z - 0.1) / 0.9 * 0.5
        } else {
            0.5 + (z - 1.0) / 3.0 * 0.5
        };
        track[0] + track[2] * t.clamp(0.0, 1.0)
    }

    fn open_menu(&mut self, hit: RibbonHit) {
        let Some(anchor) = self.ribbon.rect_of(hit) else {
            return;
        };
        let f = self.view.format();
        let items = if hit == RibbonHit::FontBox {
            FONTS
                .iter()
                .map(|n| MenuItem {
                    label: (*n).to_owned(),
                    choice: Choice::Font((*n).to_owned()),
                    checked: f.font == *n,
                    dim: false,
                })
                .collect()
        } else {
            SIZES
                .iter()
                .map(|s| MenuItem {
                    label: format!("{s}"),
                    choice: Choice::Size((s * 2.0) as u32),
                    checked: (f.size - s).abs() < 0.01,
                    dim: false,
                })
                .collect()
        };
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    fn open_file_menu(&mut self) {
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::File) else {
            return;
        };
        let items = [
            ("file.new", "backstage.nav.new", "New"),
            ("file.open", "backstage.nav.open", "Open"),
            ("file.save", "backstage.nav.save", "Save"),
            ("file.saveAs", "backstage.nav.saveAs", "Save As"),
            ("file.close", "backstage.nav.close", "Close"),
            ("file.options", "sheetNative.optMenu", "Options"),
        ]
        .iter()
        .map(|(id, key, fb)| MenuItem {
            label: self.t(key, fb),
            choice: Choice::Command((*id).to_owned()),
            checked: false,
            dim: false,
        })
        .collect();
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    fn choose(&mut self, c: Choice, text: &dyn TextMeasure) -> bool {
        match c {
            Choice::Font(name) => self.patch(json!({ "fontName": name })),
            Choice::Size(half) => self.patch(json!({ "fontSize": f64::from(half) / 2.0 })),
            Choice::Command(id) => match id.strip_prefix("goto.") {
                Some(name) => self.go_to(name),
                None if id.starts_with("fnin.") => {
                    self.view
                        .edit_insert(&format!("{}(", &id["fnin.".len()..]), text);
                    true
                }
                None if id.starts_with("filterop.") => self.filter_choice(&id["filterop.".len()..]),
                None => self.action(&id, text),
            },
            Choice::Style(_) => false,
        }
    }

    fn menu_input(&mut self, input: &Input, text: &dyn TextMeasure) -> Option<bool> {
        let menu = self.menu.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => menu.hover(x, y),
            Input::Press { x, y, .. } => {
                if menu.contains(x, y) {
                    menu.press(x, y);
                } else {
                    self.menu = None;
                    self.ribbon.clear_pointer();
                }
                true
            }
            Input::Release { x, y } => {
                if let Some(choice) = menu.release(x, y) {
                    self.menu = None;
                    self.choose(choice, text);
                }
                true
            }
            Input::Wheel(dy) => menu.wheel(dy),
            Input::Key(Key::Escape, _) => {
                self.menu = None;
                true
            }
            Input::Key(Key::Up, _) => {
                menu.step(-1);
                true
            }
            Input::Key(Key::Down, _) => {
                menu.step(1);
                true
            }
            Input::Key(Key::Enter, _) => {
                let choice = menu.current();
                self.menu = None;
                if let Some(c) = choice {
                    self.choose(c, text);
                }
                true
            }
            Input::Text(_) | Input::Key(..) => true,
            _ => return None,
        })
    }

    fn tick(&mut self, text: &dyn TextMeasure) -> bool {
        let Some(RibbonHit::Command(id)) = self.ribbon.hovered() else {
            return false;
        };
        self.rest += 1;
        if self.rest != 5
            || self.menu.is_some()
            || self.gallery.is_some()
            || self.cf_menu.is_some()
            || self.filter.is_some()
        {
            return false;
        }
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return false;
        };
        let name = self.strings.get(&tip_key(id)).to_owned();
        let label = match crate::commands::shortcut(id) {
            Some(keys) => format!("{name} ({keys})"),
            None => name,
        };
        let style = TextStyle {
            size: (12.0 * self.scale).round(),
            bold: false,
        };
        let (w, _) = text.measure(&label, style);
        self.tooltip = Some((label, w, anchor));
        true
    }

    /// Handle an input event; `true` when a new frame is needed.
    pub fn handle(&mut self, input: Input, text: &dyn TextMeasure) -> bool {
        let sheets = self.view.sheet_names().len();
        let moved = match input {
            Input::Move { x, y } if self.form.is_none() && self.menu.is_none() => Some((x, y)),
            _ => None,
        };
        let mut redraw = self.route(input, text);
        self.refresh_chart_cache();
        self.sync_table_tab();
        // Contextual tabs shown or hidden by the input (chart selection) need a layout.
        if self.ribbon.needs_layout() {
            self.relayout(text);
            redraw = true;
        }
        if let Some((x, y)) = moved {
            redraw |= self.hover_comments(x, y);
        }
        self.layout_cards(text);
        if self.view.sheet_names().len() != sheets {
            self.layout_tabs(text);
        }
        self.update_bars();
        self.view.measure_labels(text);
        if let Some((_, d)) = self.dialog.as_mut() {
            d.layout(text, self.size, self.scale);
        }
        self.layout_form(text);
        let style = self.ui_style();
        self.stat_widths = self
            .status_stat_parts()
            .into_iter()
            .map(|t| {
                let w = text.measure(&t, style).0;
                (t, w)
            })
            .collect();
        if let Some(e) = self.view.take_error() {
            self.status = e;
        }
        self.measure_name(text);
        if self.calc_state.0 && redraw {
            self.refresh_calc_state();
        }
        redraw
    }

    fn in_rect(r: [f32; 4], x: f32, y: f32) -> bool {
        x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
    }

    #[allow(clippy::too_many_lines)]
    fn route(&mut self, input: Input, text: &dyn TextMeasure) -> bool {
        if self.form.is_some() {
            if let Some(r) = self.form_input(&input, text) {
                return r;
            }
        }
        // A dialog is modal: it takes keyboard and pointer input.
        if let Some((_, d)) = self.dialog.as_mut() {
            match input {
                Input::Text(ref s) => {
                    d.text(s);
                    return true;
                }
                Input::Key(k, m) => {
                    if let Some(b) = d.key(k, m) {
                        self.sheet_dialog_button(b, text);
                    }
                    return true;
                }
                Input::Move { x, y } => return d.hover(x, y),
                Input::Press { x, y, .. } => {
                    if let Some(b) = d.press(x, y) {
                        self.sheet_dialog_button(b, text);
                    }
                    return true;
                }
                Input::Blink => return true,
                Input::Wheel(_)
                | Input::Drag { .. }
                | Input::Release { .. }
                | Input::Context { .. }
                | Input::Preedit(_) => return false,
                _ => {}
            }
        }
        if self.menu.is_some() {
            if let Some(r) = self.menu_input(&input, text) {
                return r;
            }
        }
        if self.cf_menu.is_some() {
            if let Some(r) = self.cf_menu_input(&input, text) {
                return r;
            }
        }
        if self.gallery.is_some() {
            if let Some(r) = self.gallery_input(&input, text) {
                return r;
            }
        }
        if self.picker.is_some() {
            if let Some(r) = self.picker_input(&input) {
                return r;
            }
        }
        if self.form.is_none() && self.dialog.is_none() {
            if let Some(r) = self.chart_input(&input) {
                return r;
            }
        }
        if self.filter.is_some() {
            if let Some(r) = self.filter_input(&input, text) {
                return r;
            }
        }
        if let Some(r) = self.name_input(&input) {
            return r;
        }
        let tip_before = self.tooltip.is_some();
        if matches!(
            input,
            Input::Move { .. }
                | Input::Press { .. }
                | Input::Wheel(_)
                | Input::Key(..)
                | Input::Text(_)
        ) {
            self.rest = 0;
            self.tooltip = None;
        }
        // Window shortcuts.
        if let Input::Key(k, m) = input {
            let id = match (k, m) {
                (Key::Letter('s'), Mods { ctrl: true, .. }) => Some("file.save"),
                (Key::Letter('o'), Mods { ctrl: true, .. }) => Some("file.open"),
                (Key::Letter('n'), Mods { ctrl: true, .. }) => Some("file.new"),
                (Key::Letter('c'), Mods { ctrl: true, .. }) if !self.view.editing() => {
                    Some("edit.copy")
                }
                (Key::Letter('x'), Mods { ctrl: true, .. }) if !self.view.editing() => {
                    Some("edit.cut")
                }
                (
                    Key::Letter('v'),
                    Mods {
                        ctrl: true,
                        alt: true,
                        ..
                    },
                ) if !self.view.editing() => Some("sheet.pasteSpecial"),
                (Key::Letter('v'), Mods { ctrl: true, .. }) if !self.view.editing() => {
                    Some("edit.paste")
                }
                (Key::Letter('b'), Mods { ctrl: true, .. }) => Some("format.bold"),
                (Key::Letter('i'), Mods { ctrl: true, .. }) => Some("format.italic"),
                (Key::Letter('u'), Mods { ctrl: true, .. }) => Some("format.underline"),
                (Key::Letter('5'), Mods { ctrl: true, .. }) => Some("format.strike"),
                // Spreadsheet shortcuts: fill down / right, Format Cells, AutoSum.
                (Key::Letter('d'), Mods { ctrl: true, .. }) if !self.view.editing() => {
                    Some("fill.down")
                }
                (Key::Letter('r'), Mods { ctrl: true, .. }) if !self.view.editing() => {
                    Some("fill.right")
                }
                (Key::Letter('1'), Mods { ctrl: true, .. }) if !self.view.editing() => {
                    Some("sheet.formatCellsDialog")
                }
                (Key::Letter('='), Mods { alt: true, .. }) if !self.view.editing() => {
                    Some("autosum.sum")
                }
                _ => None,
            };
            // Ctrl+; today's date, Ctrl+Shift+; the time, typed into the cell (as the
            // reference: the editor opens with them).
            if k == Key::Letter(';') && m.ctrl {
                let stamp = now_stamp(m.shift);
                if self.view.editing() {
                    self.view.edit_insert(&stamp, text);
                } else {
                    self.view.edit_with(&stamp, text);
                }
                return true;
            }
            if let Some(id) = id {
                if self.view.editing() && id.starts_with("format.") {
                    self.view.commit();
                }
                self.action(id, text);
                return true;
            }
            if m.ctrl && matches!(k, Key::PageUp | Key::PageDown) {
                let names = self.view.sheet_names();
                let cur = self.view.sheet();
                let next = if k == Key::PageDown {
                    (cur + 1..names.len()).find(|i| names[*i].is_some())
                } else {
                    (0..cur).rev().find(|i| names[*i].is_some())
                };
                return next.is_some_and(|i| self.view.set_sheet(i));
            }
            if k == Key::Escape && self.painter.take().is_some() {
                return true;
            }
            // F9 calculates the workbook, Shift+F9 the sheet (manual calculation).
            if k == Key::F(9) && !self.view.editing() {
                return self.calculate(m.shift);
            }
        }
        if let Some(done) = self.totals_list_input(&input) {
            return done;
        }
        if let Some(done) = self.tabs_input(&input, text) {
            return done;
        }
        if let Input::Context { x, y } = input {
            if self.view.context_at(x, y).is_some() {
                self.context_menu(x, y);
                return true;
            }
            return false;
        }
        if let Input::Zoom(steps) = input {
            let z = ((self.view.zoom() + 0.1 * steps) * 10.0).round() / 10.0;
            return self.zoom_to(z, text);
        }
        let status_top = self.size.1 - self.status_h();
        let tabs_top = status_top - self.tabs_h();
        // Scroll bar arrows: one row / column.
        if let Input::Press { x, y, .. } = input {
            if let Some(i) = self
                .arrow_rects()
                .iter()
                .position(|r| Self::in_rect(*r, x, y))
            {
                let moved = match i {
                    0 => self.view.scroll_rows(-1),
                    1 => self.view.scroll_rows(1),
                    2 => self.view.scroll_cols(-1),
                    _ => self.view.scroll_cols(1),
                };
                self.update_bars();
                return moved;
            }
        }
        // Scroll bars.
        match input {
            Input::Press { x, y, .. } if Self::in_rect(self.vbar.0, x, y) => {
                return self.bar_press(true, y);
            }
            Input::Press { x, y, .. } if Self::in_rect(self.hbar.0, x, y) => {
                return self.bar_press(false, x);
            }
            Input::Drag { y, .. } if self.vdrag.is_some() => return self.bar_drag(true, y),
            Input::Drag { x, .. } if self.hdrag.is_some() => return self.bar_drag(false, x),
            Input::Release { .. } if self.vdrag.is_some() || self.hdrag.is_some() => {
                self.vdrag = None;
                self.hdrag = None;
                return true;
            }
            _ => {}
        }
        // Status bar zoom.
        match input {
            Input::Press { x, y, .. } if y >= status_top => {
                if let Some(done) = self.view_button_press(x, y, text) {
                    return done;
                }
                let hit = self.zoom_rects.iter().position(|r| Self::in_rect(*r, x, y));
                return match hit {
                    Some(0) => self.zoom_to(((self.view.zoom() - 0.1) * 10.0).round() / 10.0, text),
                    Some(1) => {
                        self.zoom_drag = true;
                        let z = self.slider_zoom(x);
                        self.zoom_to(z, text)
                    }
                    Some(2) => self.zoom_to(((self.view.zoom() + 0.1) * 10.0).round() / 10.0, text),
                    Some(_) => self.zoom_to(1.0, text),
                    None => false,
                };
            }
            Input::Drag { x, .. } if self.zoom_drag => {
                let z = self.slider_zoom(x);
                return self.zoom_to(z, text);
            }
            Input::Release { .. } if self.zoom_drag => {
                self.zoom_drag = false;
                return false;
            }
            _ => {}
        }
        // Sheet tabs.
        if let Input::Press { x, y, .. } = input {
            if y >= tabs_top && y < status_top {
                if Self::in_rect(self.add_rect, x, y) {
                    let added = self.view.add_sheet();
                    self.layout_tabs(text);
                    return added;
                }
                if let Some((i, ..)) = self
                    .tab_rects
                    .iter()
                    .find(|(_, tx, tw)| x >= *tx && x < tx + tw)
                {
                    let i = *i;
                    let done = self.view.set_sheet(i);
                    if done {
                        self.sync_view_mode(text);
                    }
                    return done;
                }
                return false;
            }
        }
        // Formula bar: name box (typed go-to, list of names), expand button; a click in the
        // content box edits the active cell there.
        if let Input::Press { x, y, .. } = input {
            if let Some(r) = self.fbar_press(x, y, text) {
                return r;
            }
            if Self::in_rect(self.fx_rect, x, y) {
                if !self.view.editing() {
                    self.view.edit_cell(text);
                }
                return true;
            }
        }
        let in_title = |w: &Self, y: f32| y < w.title.height();
        let in_ribbon = |w: &Self, y: f32| y >= w.ribbon.top() && y < w.ribbon.bottom();
        let redraw = match input {
            Input::Resize { width, height } => {
                self.size = (width.max(1.0), height.max(1.0));
                self.menu = None;
                self.relayout(text);
                true
            }
            Input::Scale(scale) => {
                self.scale = scale;
                self.menu = None;
                self.view.set_scale(scale);
                self.relayout(text);
                true
            }
            Input::Tick => self.tick(text),
            Input::Zoom(_) | Input::Context { .. } | Input::Blink => false,
            Input::Maximized(m) => self.title.set_maximized(m),
            Input::Leave => {
                let a = self.title.hover(-1.0, -1.0);
                self.ribbon.clear_pointer() || a
            }
            Input::Move { x, y } => {
                let a = self.title.hover(x, y);
                self.cursor = self.view.cursor_at(x, y);
                self.ribbon.hover(x, y) || a
            }
            Input::Press { x, y, .. } if in_title(self, y) => self.title.press(x, y),
            Input::Press { x, y, .. } if in_ribbon(self, y) => {
                if self.view.editing() {
                    self.view.commit();
                }
                self.ribbon.press(x, y)
            }
            Input::Press { x, y, .. } if self.totals_press(x, y) => true,
            Input::Press { x, y, .. } if self.filter_button_at(x, y).is_some() => {
                let (col, table) = self.filter_button_at(x, y).unwrap_or_default();
                if self.view.editing() {
                    self.view.commit();
                }
                self.open_filter(col, table)
            }
            Input::Press { .. } => {
                if std::mem::take(&mut self.filter_status) {
                    self.status.clear();
                }
                self.grid_press = true;
                self.ribbon.clear_pointer();
                self.view.handle(input, text)
            }
            Input::Drag { .. } if !self.grid_press => false,
            Input::Release { x, y } => {
                let mut changed = false;
                if self.grid_press {
                    changed |= self.view.handle(Input::Release { x, y }, text);
                    if let Some(from) = self.painter.take() {
                        let to = self.view.rng();
                        changed |= self
                            .view
                            .run(json!({ "cmd": "paintFormat", "from": from, "to": to }));
                    }
                }
                self.grid_press = false;
                let quick = match self.title.release(x, y) {
                    Some(id) => self.action(id, text),
                    None => false,
                };
                let (fired, pressed) = self.ribbon.release(x, y);
                let ran = match fired {
                    Some(RibbonHit::File) => {
                        self.open_file_menu();
                        true
                    }
                    Some(RibbonHit::Command(id)) => self.action(id, text),
                    Some(hit @ (RibbonHit::FontBox | RibbonHit::SizeBox)) => {
                        self.open_menu(hit);
                        true
                    }
                    _ => false,
                };
                if self.ribbon.needs_layout() {
                    self.relayout(text);
                }
                changed || quick || ran || pressed || fired.is_some()
            }
            Input::Wheel(_)
            | Input::Text(_)
            | Input::Key(..)
            | Input::Preedit(_)
            | Input::Drag { .. } => self.view.handle(input, text),
        };
        redraw || tip_before != self.tooltip.is_some()
    }

    /// Whether an animation needs frames (none yet).
    pub fn animating(&self) -> bool {
        false
    }

    /// Redraw the grid image if needed.
    pub fn prepare(&mut self, store: &mut dyn GridStore) -> Result<(), String> {
        self.view.prepare(store)
    }

    /// Caret rectangle (IME placement).
    pub fn caret_rect(&self) -> Option<[f32; 4]> {
        self.view.caret_rect()
    }

    /// Paint the whole window.
    pub fn paint(&self, p: &mut dyn Painter) {
        p.clear(self.theme.bg);
        self.paint_base(p);
        self.paint_top(p);
        self.paint_overlay(p);
    }

    /// Height of the top band (title bar and ribbon) that [`Self::paint_top`] draws; the
    /// host may cache it as an image.
    pub fn top_height(&self) -> f32 {
        self.ribbon.bottom()
    }

    /// The grid with its headers (below the overlay). With [`Self::paint_top`] and
    /// [`Self::paint_overlay`] it covers the whole window, so a host drawing the three
    /// needs no clear (one pass less over the window's pixels).
    pub fn paint_base(&self, p: &mut dyn Painter) {
        self.view.paint(p);
        self.paint_comment_marks(p);
    }

    /// Cells with a comment carry a mark in their top-right corner.
    fn paint_comment_marks(&self, p: &mut dyn Painter) {
        let s = self.px(5.6);
        for (r, c) in self.commented_cells() {
            let Some([_, t, right, _]) = self.view.cell_box(r, c) else {
                continue;
            };
            p.path(
                &[
                    PathSeg::M(right - s, t + 1.0),
                    PathSeg::L(right, t + 1.0),
                    PathSeg::L(right, t + 1.0 + s),
                    PathSeg::Z,
                ],
                Some(COMMENT_MARK),
                None,
            );
        }
    }

    /// Title bar, ribbon and number format box: everything above the formula bar.
    pub fn paint_top(&self, p: &mut dyn Painter) {
        p.fill_rect(0.0, 0.0, self.size.0, self.top_height(), self.theme.bg);
        let fmt = self.format_state();
        self.title.paint(
            p,
            &self.theme,
            &self.icons,
            &self.strings,
            &self.name,
            self.view.modified(),
            &fmt,
        );
        self.ribbon.paint(
            p,
            &self.theme,
            &self.icons,
            &self.strings,
            &fmt,
            &[],
            &self.on_ids(),
        );
        self.paint_number_box(p);
        self.paint_check_items(p);
        self.paint_chart_style_tiles(p);
        self.paint_table_checks(p);
    }

    /// Formula bar, sheet tabs, scroll bars, status bar, then menus, tooltips and dialogs
    /// (over the top band).
    pub fn paint_overlay(&self, p: &mut dyn Painter) {
        self.paint_arrows(p);
        self.paint_chart_selection(p);
        self.paint_cards(p);
        if self.show_fbar {
            self.paint_formula_bar(p);
        }
        self.paint_tabs(p);
        self.paint_tab_extras(p);
        self.paint_scrollbars(p);
        self.paint_status(p);
        self.paint_totals_button(p);
        self.paint_totals_list(p);
        self.paint_filter(p);
        self.paint_gallery(p);
        self.paint_cf_menu(p);
        self.paint_picker(p);
        if let Some(menu) = &self.menu {
            menu.paint(p, &self.theme, &self.icons);
        } else if let Some((label, tw, anchor)) = &self.tooltip {
            paint_tooltip(p, &self.theme, label, *tw, *anchor, self.size.0, self.scale);
        }
        if let Some((_, d)) = &self.dialog {
            d.paint(p, &self.theme, self.size, true);
        }
        if let Some(f) = &self.form {
            f.paint(p, self.size, true, &mut |id, r, p| {
                self.paint_form_canvas(f, id, r, p);
            });
        }
    }

    /// Window point of the open form's OK button (tests).
    #[cfg(test)]
    fn fc_ok_point(&self) -> (f32, f32) {
        let f = self.form.as_ref().expect("form");
        let r = f.get("ok").expect("ok").rect;
        let d = f.dev([r[0] + 10.0, r[1] + 10.0, 0.0, 0.0]);
        (d[0], d[1])
    }

    /// Lay out the open form and measure the texts its owner draws.
    fn layout_form(&mut self, text: &dyn TextMeasure) {
        let strings = match (self.form.is_some(), self.form_kind) {
            (true, form_impl::FormKind::FormatCells) => self.fc_strings(),
            (true, form_impl::FormKind::Evaluate) => self.eval_strings(),
            (true, _) => Vec::new(),
            _ => return,
        };
        if let Some(f) = self.form.as_mut() {
            f.layout(text, self.size, self.scale);
            f.measure(text, &strings);
        }
    }

    /// Input for the open form: `Some(redraw)` when the form took it.
    fn form_input(&mut self, input: &Input, text: &dyn TextMeasure) -> Option<bool> {
        let f = self.form.as_mut()?;
        let ev = match input {
            Input::Text(s) => f.type_text(s),
            Input::Key(k, m) => f.key(*k, *m),
            Input::Move { x, y } => {
                self.form_ptr = (*x, *y);
                return Some(f.hover(*x, *y));
            }
            Input::Press { x, y, count, .. } => {
                self.form_ptr = (*x, *y);
                f.press(*x, *y, *count)
            }
            Input::Wheel(d) => {
                let (x, y) = self.form_ptr;
                return Some(f.wheel(x, y, *d));
            }
            Input::Blink => return Some(true),
            Input::Drag { .. }
            | Input::Release { .. }
            | Input::Context { .. }
            | Input::Preedit(_) => return Some(false),
            _ => return None,
        };
        Some(match self.form_kind {
            form_impl::FormKind::FormatCells => self.fc_event(&ev),
            kind => self.form_event(kind, &ev, text),
        })
    }

    fn paint_form_canvas(&self, f: &form_impl::Form, id: &str, r: [f32; 4], p: &mut dyn Painter) {
        if self.form_kind == form_impl::FormKind::FormatCells {
            self.fc_paint_canvas(f, id, r, p);
        } else if self.form_kind == form_impl::FormKind::Symbol {
            self.paint_symbol_grid(f, r, p);
        } else if self.form_kind == form_impl::FormKind::MoreColors {
            self.paint_more_colors(r, p);
        } else if self.form_kind == form_impl::FormKind::CfManager {
            self.paint_cf_rules(r, p, self.scale / 1.25);
        } else if self.form_kind == form_impl::FormKind::TableStyle && id == "preview" {
            self.paint_tstyle_preview(r, p);
        } else if self.form_kind == form_impl::FormKind::Options {
            self.paint_options(id, r, p);
        } else if self.form_kind == form_impl::FormKind::Evaluate && id == "box" {
            self.paint_evaluate(f, r, p);
        } else if self.form_kind == form_impl::FormKind::Sort && id == "grid" {
            // The Sort dialog's level grid (measured on the reference, round 39).
            let k = self.scale / 1.25;
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x87, 0x90, 255));
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                Rgba(255, 255, 255, 255),
            );
            let hy = (r[1] + 32.0 * k).round();
            p.fill_rect(r[0] + 1.0, hy, r[2] - 2.0, 1.0, Rgba(0xE0, 0xDF, 0xE3, 255));
            for x in [267.0, 533.0] {
                p.fill_rect(
                    (r[0] + x * k).round(),
                    r[1] + 1.0,
                    1.0,
                    hy - r[1] - 1.0,
                    Rgba(0xE5, 0xE5, 0xE5, 255),
                );
            }
            // The chosen level's label cell.
            p.fill_rect(
                r[0] + 1.0,
                hy + 1.0,
                (72.0 * k).round(),
                (30.0 * k).round(),
                Rgba(0xE0, 0xDF, 0xE3, 255),
            );
        }
    }

    /// The number format box of the Home tab: the active cell's format category in a
    /// white box with a drop-down arrow, drawn over its ribbon placeholder.
    fn paint_number_box(&self, p: &mut dyn Painter) {
        let Some([x, y, w, h]) = self
            .ribbon
            .rect_of(RibbonHit::Command("sheet.numberFormat"))
        else {
            return;
        };
        let (y, h) = (y + self.px(1.0), h - self.px(2.0));
        p.fill_rect(x, y, w, h, Rgba(0x94, 0x94, 0x94, 255));
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, self.theme.surface);
        let label = self
            .strings
            .get(sheet_ribbon::number_category(&self.view.format().num_fmt));
        let style = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        let arrow = self.px(16.0);
        p.push_clip(x, y, w - arrow, h);
        p.text(
            label,
            x + self.px(5.0),
            (y + (h - style.size * 1.33) / 2.0).round(),
            style,
            self.theme.text,
        );
        p.pop_clip();
        let (cx, cy, k) = (
            (x + w - arrow / 2.0).round(),
            (y + h / 2.0).round(),
            self.px(3.0),
        );
        p.path(
            &[
                PathSeg::M(cx - k, cy - k / 2.0),
                PathSeg::L(cx + k, cy - k / 2.0),
                PathSeg::L(cx, cy + k / 2.0),
                PathSeg::Z,
            ],
            Some(self.theme.text_muted),
            None,
        );
    }

    fn ui_style(&self) -> TextStyle {
        TextStyle {
            size: self.px(12.0),
            bold: false,
        }
    }

    fn paint_formula_bar(&self, p: &mut dyn Painter) {
        let top = self.ribbon.bottom();
        let (w, h) = (self.size.0, self.fbar_h());
        // Measured: #E6E6E6 band, #FDFDFD boxes framed in #C6C6C6.
        p.fill_rect(0.0, top, w, h, TABS_BG);
        let edge = STATUS_EDGE;
        let boxed = |p: &mut dyn Painter, r: [f32; 4]| {
            p.fill_rect(r[0], r[1], r[2], r[3], edge);
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                Rgba(0xfd, 0xfd, 0xfd, 255),
            );
        };
        boxed(p, self.name_rect);
        boxed(p, self.fx_rect);
        // Box around the cancel / enter / function marks (measured 137 × 30 px at 125 %).
        boxed(
            p,
            [
                self.px(136.0),
                self.name_rect[1],
                self.px(109.6),
                self.name_rect[3],
            ],
        );
        let style = self.ui_style();
        let line = style.size * 1.33;
        let n = self.name_rect;
        p.text(
            &self.name_box_text(),
            n[0] + self.px(5.0),
            (n[1] + (n[3] - line) / 2.0).round(),
            style,
            self.theme.text,
        );
        // Cancel / enter / function marks between the boxes.
        let gx = self.px(152.0);
        let cy = (n[1] + n[3] / 2.0).round();
        let k = self.px(4.0);
        let ink = if self.view.editing() {
            self.theme.text
        } else {
            Rgba(0xa6, 0xa6, 0xa6, 255)
        };
        let lw = self.scale.max(1.0).round();
        p.path(
            &[
                PathSeg::M(gx - k, cy - k),
                PathSeg::L(gx + k, cy + k),
                PathSeg::M(gx + k, cy - k),
                PathSeg::L(gx - k, cy + k),
            ],
            None,
            Some((ink, lw)),
        );
        let gx2 = gx + self.px(36.0);
        p.path(
            &[
                PathSeg::M(gx2 - k, cy),
                PathSeg::L(gx2 - k / 3.0, cy + k * 0.8),
                PathSeg::L(gx2 + k, cy - k),
            ],
            None,
            Some((ink, lw)),
        );
        let fx_font = Font {
            family: "Cambria Math",
            size: self.px(14.0),
            weight: 400,
            italic: true,
            underline: false,
        };
        p.text_font(
            "fx",
            gx2 + self.px(28.0),
            (cy - fx_font.size * 0.7).round(),
            &fx_font,
            self.theme.text,
        );
        let f = self.fx_rect;
        let tx = f[0] + self.px(5.0);
        let ty = (f[1] + (f[3] - line) / 2.0).round();
        p.push_clip(
            f[0] + 1.0,
            f[1] + 1.0,
            f[2] - 2.0 - self.px(16.0),
            f[3] - 2.0,
        );
        // One line, or as many as the expanded bar shows (19.2 pt pitch, measured).
        // Empty while a chart is selected, as on the reference.
        let content = if self.chart_sel.is_some() {
            ""
        } else {
            self.view.formula_text()
        };
        for (i, shown) in content
            .lines()
            .take(self.fbar_lines.max(1) as usize)
            .enumerate()
        {
            let y = ty + i as f32 * self.px(19.2);
            p.text(shown, tx, y, style, self.theme.text);
        }
        p.pop_clip();
        self.paint_fbar_parts(p);
    }

    /// Scroll bars as measured: #E6E6E6 column, white arrow boxes framed in #949494 with
    /// #262626 triangles, #F3F3F3 track, white thumb framed in #949494.
    fn paint_scrollbars(&self, p: &mut dyn Painter) {
        let edge = Rgba(0x94, 0x94, 0x94, 255);
        let arrow = STATUS_TEXT;
        let boxed = |p: &mut dyn Painter, r: [f32; 4]| {
            p.fill_rect(r[0], r[1], r[2], r[3], edge);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
        };
        let (w, h) = self.size;
        let gt = self.grid_top();
        let tabs_y = h - self.status_h() - self.tabs_h();
        p.fill_rect(w - self.sb_w(), gt, self.sb_w(), tabs_y - gt, TABS_BG);
        let k = self.px(3.2);
        for (i, r) in self.arrow_rects().into_iter().enumerate() {
            boxed(p, r);
            let (cx, cy) = ((r[0] + r[2] / 2.0).round(), (r[1] + r[3] / 2.0).round());
            let segs = match i {
                0 => [
                    (cx - k, cy + k / 2.0),
                    (cx + k, cy + k / 2.0),
                    (cx, cy - k / 2.0 - 1.0),
                ],
                1 => [
                    (cx - k, cy - k / 2.0),
                    (cx + k, cy - k / 2.0),
                    (cx, cy + k / 2.0 + 1.0),
                ],
                2 => [
                    (cx + k / 2.0, cy - k),
                    (cx + k / 2.0, cy + k),
                    (cx - k / 2.0 - 1.0, cy),
                ],
                _ => [
                    (cx - k / 2.0, cy - k),
                    (cx - k / 2.0, cy + k),
                    (cx + k / 2.0 + 1.0, cy),
                ],
            };
            p.path(
                &[
                    PathSeg::M(segs[0].0, segs[0].1),
                    PathSeg::L(segs[1].0, segs[1].1),
                    PathSeg::L(segs[2].0, segs[2].1),
                    PathSeg::Z,
                ],
                Some(arrow),
                None,
            );
        }
        let (vt, vs, vl) = self.vbar;
        if vt[3] > 0.0 {
            p.fill_rect(vt[0], vt[1], vt[2], vt[3], STATUS_BG);
            if vl > 2.0 {
                boxed(p, [vt[0], vt[1] + vs, vt[2], vl]);
            }
        }
        let (ht, hs, hl) = self.hbar;
        if ht[2] > 0.0 {
            p.fill_rect(ht[0], ht[1], ht[2], ht[3], STATUS_BG);
            if hl > 2.0 {
                boxed(p, [ht[0] + hs, ht[1], hl, ht[3]]);
            }
        }
    }
    fn paint_tabs(&self, p: &mut dyn Painter) {
        let y = self.size.1 - self.status_h() - self.tabs_h();
        let h = self.tabs_h();
        let w = self.size.0;
        p.fill_rect(0.0, y, w, h, TABS_BG);
        p.fill_rect(0.0, y, w, 1.0, TABS_EDGE);
        // Navigation arrows.
        let ink = Rgba(0xa6, 0xa6, 0xa6, 255);
        let k = self.px(3.0);
        let cy = (y + h / 2.0).round();
        for (i, dir) in [(0, -1.0_f32), (1, 1.0)] {
            let cx = self.px(20.0) + i as f32 * self.px(18.0);
            p.path(
                &[
                    PathSeg::M(cx - dir * k / 2.0, cy - k),
                    PathSeg::L(cx + dir * k / 2.0, cy),
                    PathSeg::L(cx - dir * k / 2.0, cy + k),
                    PathSeg::Z,
                ],
                Some(ink),
                None,
            );
        }
        let names = self.view.sheet_names();
        let current = self.view.sheet();
        for (i, x, tw) in &self.tab_rects {
            let name = names.get(*i).and_then(Clone::clone).unwrap_or_default();
            let active = *i == current;
            let style = self.tab_style(active);
            let line = style.size * 1.33;
            // Measured: white tab from the strip's top edge to 3 px above its bottom, #999999
            // sides, a 2 px green bar 2 px above the tab's bottom, bold green label.
            let tab_h = h - self.px(2.4);
            if !active {
                self.paint_tab_fill(p, *i, [*x, y, *tw, tab_h], false);
            }
            if active {
                p.fill_rect(*x, y, *tw, tab_h, WHITE);
                self.paint_tab_fill(p, *i, [*x, y, *tw, tab_h], true);
                p.fill_rect(*x, y, 1.0, tab_h, TABS_EDGE);
                p.fill_rect(x + tw - 1.0, y, 1.0, tab_h, TABS_EDGE);
                let bar = self.px(1.6).max(2.0);
                p.fill_rect(
                    x + 1.0,
                    y + tab_h - bar - self.px(1.6),
                    tw - 2.0,
                    bar,
                    SEL_GREEN,
                );
            } else {
                p.fill_rect(
                    x + tw - 1.0,
                    y + self.px(6.0),
                    1.0,
                    h - self.px(12.0),
                    Rgba(0xb4, 0xb4, 0xb4, 255),
                );
            }
            let ink = if active {
                SEL_GREEN
            } else if self.tab_color(*i).is_some() {
                Rgba(0, 0, 0, 255)
            } else {
                Rgba(0x44, 0x44, 0x44, 255)
            };
            let lw = self.tab_label_w(&name, active);
            p.text(
                &name,
                (x + (tw - lw) / 2.0).round(),
                (y + (tab_h - line) / 2.0 - self.px(4.0)).round(),
                style,
                ink,
            );
        }
        // New sheet: a circled plus.
        let [ax, ay, aw, ah] = self.add_rect;
        let (cx, cy, r) = (ax + aw / 2.0, ay + ah / 2.0, aw / 2.0 - 1.0);
        let c = 0.5523 * r;
        let ink = Rgba(0x6e, 0x6e, 0x6e, 255);
        p.path(
            &[
                PathSeg::M(cx + r, cy),
                PathSeg::C(cx + r, cy + c, cx + c, cy + r, cx, cy + r),
                PathSeg::C(cx - c, cy + r, cx - r, cy + c, cx - r, cy),
                PathSeg::C(cx - r, cy - c, cx - c, cy - r, cx, cy - r),
                PathSeg::C(cx + c, cy - r, cx + r, cy - c, cx + r, cy),
                PathSeg::Z,
            ],
            None,
            Some((ink, self.scale.max(1.0))),
        );
        let a = r * 0.5;
        p.path(
            &[
                PathSeg::M(cx - a, cy),
                PathSeg::L(cx + a, cy),
                PathSeg::M(cx, cy - a),
                PathSeg::L(cx, cy + a),
            ],
            None,
            Some((ink, self.scale.max(1.0))),
        );
    }

    /// The selection statistics shown in the status bar ("Average: 11,5", "Count: 7", …),
    /// in the reference's order; empty for a single cell.
    fn status_stat_parts(&self) -> Vec<String> {
        let st = self.view.stats();
        if st.count <= 1 {
            return Vec::new();
        }
        // The workbook's decimal separator, as the reference ("Average: 11,5" in vi-VN).
        let decimal = self.view.engine().number_locale().decimal;
        let num = |v: f64| {
            let s = format!("{v:.10}");
            let s = s.trim_end_matches('0').trim_end_matches('.').to_owned();
            if s == "-0" {
                "0".to_owned()
            } else {
                s.replace('.', &decimal.to_string())
            }
        };
        // The statistics chosen in the status bar's menu, in the reference's order.
        let mut parts = Vec::new();
        for (id, key, fallback, _) in tabs_impl::STATUS_ITEMS {
            if !self.status_item_on(id) {
                continue;
            }
            let value = match id {
                "status.average" => st.average.map(num),
                "status.count" => Some(st.count.to_string()),
                "status.numCount" => Some(st.numbers.to_string()),
                "status.min" => st.min.filter(|_| st.numbers > 0).map(num),
                "status.max" => st.max.filter(|_| st.numbers > 0).map(num),
                _ => (st.numbers > 0).then(|| num(st.sum)),
            };
            if let Some(v) = value {
                parts.push(format!("{}: {v}", self.t(key, fallback)));
            }
        }
        parts
    }

    fn paint_status(&self, p: &mut dyn Painter) {
        let (w, h) = self.size;
        let sh = self.status_h();
        let y = h - sh;
        p.fill_rect(0.0, y, w, sh, STATUS_BG);
        p.fill_rect(0.0, y - 1.0, w, 1.0, STATUS_EDGE);
        let style = self.ui_style();
        let ty = (y + (sh - style.size * 1.33) / 2.0).round();
        let mode = if let Some(m) = self.view.edit_mode() {
            match m {
                "point" => self.t("sheetMenu.modePoint", "Point"),
                "edit" => self.t("sheetMenu.modeEdit", "Edit"),
                _ => self.t("sheetMenu.modeEnter", "Enter"),
            }
        } else if !self.status.is_empty() {
            self.status.clone()
        } else if self.calc_state.1 {
            // Manual calculation with formulas waiting (F9), as the reference shows it.
            format!(
                "{}      {}",
                self.t("sheetEditor.ready", "Ready"),
                self.t("sheetMenu.calculate", "Calculate")
            )
        } else {
            self.t("sheetEditor.ready", "Ready")
        };
        p.text(&mode, self.px(11.2), ty - 1.0, style, STATUS_TEXT);
        // Page layout view: the page of the active cell, as the reference ("Page: 1 of 6").
        if let Some((n, of)) = self.view.layout_page_of_active() {
            let s = self
                .t("sheetNative.pageOf", "Page: {n} of {of}")
                .replace("{n}", &n.to_string())
                .replace("{of}", &of.to_string());
            p.text(&s, self.px(112.0), ty - 1.0, style, STATUS_TEXT);
        }
        // Selection statistics, right-aligned before the view buttons: measured on the
        // reference at 125 %, the last one ends 41 px before the first view button and they
        // are 28 px apart.
        // (Computed and measured at input time: no engine query while painting.)
        let mut right = w - self.px(353.6) - self.px(32.8);
        for (part, width) in self.stat_widths.iter().rev() {
            p.text(part, (right - width).round(), ty, style, STATUS_TEXT);
            right -= width + self.px(22.4);
        }
        // Zoom: minus, track with the 100 % tick, thumb, plus, percentage.
        let [minus, track, plus, pct] = self.zoom_rects;
        let k = self.px(4.0);
        let lw = self.scale.max(1.0).round();
        let ink = STATUS_TEXT;
        let (mx, my) = (minus[0] + minus[2] / 2.0, minus[1] + minus[3] / 2.0);
        p.fill_rect((mx - k).round(), my.round(), 2.0 * k, lw, ink);
        let (qx, qy) = (plus[0] + plus[2] / 2.0, plus[1] + plus[3] / 2.0);
        p.fill_rect((qx - k).round(), qy.round(), 2.0 * k, lw, ink);
        p.fill_rect(qx.round(), (qy - k).round(), lw, 2.0 * k, ink);
        let ty2 = (track[1] + track[3] / 2.0).round();
        p.fill_rect(track[0], ty2, track[2], lw, Rgba(0x80, 0x80, 0x80, 255));
        p.fill_rect(
            (track[0] + track[2] / 2.0).round(),
            ty2 - k / 2.0,
            lw,
            k + lw,
            Rgba(0x80, 0x80, 0x80, 255),
        );
        let thumb = self.slider_pos();
        p.fill_rect(
            (thumb - k / 2.0).round(),
            ty2 - k - lw,
            k,
            2.0 * k + 3.0 * lw,
            ink,
        );
        let zoom = format!("{}%", (self.view.zoom() * 100.0).round() as i32);
        p.text(&zoom, pct[0], ty, style, ink);
        // View buttons before the zoom controls (the current view on).
        // Three 54 px view buttons ending 389 px from the right; the current one #C6C6C6.
        let bw = self.px(43.2);
        let size = self.px(16.0);
        let on = match self.view_mode() {
            ViewMode::Normal => 0,
            ViewMode::PageLayout => 1,
            ViewMode::PageBreak => 2,
        };
        for i in 0..3 {
            let x = w - self.px(353.6) + i as f32 * bw;
            if i == on {
                p.fill_rect(x, y, bw, sh, STATUS_EDGE);
            }
            let icon = ["view.normal", "layout.margins", "view.sorter"][i];
            self.icons.draw(
                p,
                icon,
                (x + (bw - size) / 2.0).round(),
                (y + (sh - size) / 2.0).round(),
                size,
                IconColors {
                    ink: STATUS_TEXT,
                    accent: self.theme.accent,
                    on: self.theme.accent_on,
                    tinted: false,
                },
            );
        }
    }
}

/// A painter that only hashes what it is asked to draw: equal hashes mean an equal picture,
/// so a cached image of that part can be reused.
#[derive(Default)]
pub struct PaintHash(std::collections::hash_map::DefaultHasher);

impl PaintHash {
    /// Hash of everything drawn so far.
    pub fn finish(&self) -> u64 {
        use std::hash::Hasher;
        self.0.finish()
    }

    fn put(&mut self, tag: u8, v: &[f32]) {
        use std::hash::Hasher;
        self.0.write_u8(tag);
        for x in v {
            self.0.write_u32(x.to_bits());
        }
    }

    fn color(&mut self, c: Rgba) {
        use std::hash::Hasher;
        self.0.write(&[c.0, c.1, c.2, c.3]);
    }
}

impl Painter for PaintHash {
    fn clear(&mut self, color: Rgba) {
        self.put(0, &[]);
        self.color(color);
    }
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        self.put(1, &[x, y, w, h]);
        self.color(color);
    }
    fn page(&mut self, index: usize, x: i32, y: i32) {
        self.put(2, &[index as f32, x as f32, y as f32]);
    }
    fn text(&mut self, text: &str, x: f32, y: f32, style: TextStyle, color: Rgba) {
        use std::hash::Hasher;
        self.put(3, &[x, y, style.size, f32::from(u8::from(style.bold))]);
        self.0.write(text.as_bytes());
        self.color(color);
    }
    fn text_font(&mut self, text: &str, x: f32, y: f32, font: &Font<'_>, color: Rgba) {
        use std::hash::Hasher;
        self.put(4, &[x, y, font.size, f32::from(font.weight)]);
        self.0.write(font.family.as_bytes());
        self.0
            .write(&[u8::from(font.italic), u8::from(font.underline)]);
        self.0.write(text.as_bytes());
        self.color(color);
    }
    fn path(&mut self, segs: &[PathSeg], fill: Option<Rgba>, stroke: Option<(Rgba, f32)>) {
        self.put(5, &[]);
        for s in segs {
            match *s {
                PathSeg::M(a, b) => self.put(6, &[a, b]),
                PathSeg::L(a, b) => self.put(7, &[a, b]),
                PathSeg::Q(a, b, c, d) => self.put(8, &[a, b, c, d]),
                PathSeg::C(a, b, c, d, e, f) => self.put(9, &[a, b, c, d, e, f]),
                PathSeg::Z => self.put(10, &[]),
            }
        }
        if let Some(c) = fill {
            self.color(c);
        }
        if let Some((c, w)) = stroke {
            self.put(11, &[w]);
            self.color(c);
        }
    }
    fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.put(12, &[x, y, w, h]);
    }
    fn pop_clip(&mut self) {
        self.put(13, &[]);
    }
    fn page_part(&mut self, index: usize, src: [f32; 4], dest: [f32; 4], opacity: f32) {
        self.put(14, &[index as f32, opacity]);
        self.put(15, &src);
        self.put(16, &dest);
    }
}

impl SheetView {
    /// Displayed text of the active cell.
    pub fn formula_display(&self) -> String {
        let (r0, c0, _, _) = self.range_bounds();
        self.engine()
            .cell(self.sheet(), r0, c0)
            .map(|d| d.display)
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "sheet_window_tests.rs"]
mod more_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet_view::GridStore;
    use bo_engine::SheetEngine;

    struct Fixed;
    impl TextMeasure for Fixed {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            (
                text.chars().count() as f32 * style.size * 0.5,
                style.size * 1.3,
            )
        }
    }

    struct Store(usize);
    impl GridStore for Store {
        fn update(
            &mut self,
            list: &bo_render::PageList,
            _: &SheetEngine,
            _: f32,
        ) -> Result<(), String> {
            self.0 = list.commands.len();
            Ok(())
        }
    }

    fn window() -> SheetWindow {
        let mut e = SheetEngine::with_bundled_fonts();
        e.new_workbook("Book1.xlsx").expect("workbook");
        let mut w = SheetWindow::new(SheetView::new(e), "en-US", "Book1.xlsx");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1000.0,
            },
            &Fixed,
        );
        w
    }

    fn type_text(w: &mut SheetWindow, s: &str) {
        for c in s.chars() {
            w.handle(Input::Text(c.to_string()), &Fixed);
        }
    }

    fn key(w: &mut SheetWindow, k: Key) {
        w.handle(Input::Key(k, Mods::default()), &Fixed);
    }

    #[test]
    fn typing_enters_values_and_formulas_recalculate() {
        let mut w = window();
        type_text(&mut w, "12");
        key(&mut w, Key::Enter);
        type_text(&mut w, "30");
        key(&mut w, Key::Enter);
        type_text(&mut w, "=A1+A2");
        key(&mut w, Key::Enter);
        let e = w.view().engine();
        assert_eq!(e.cell(0, 2, 0).expect("A3").display, "42");
        assert_eq!(w.view().range_bounds(), (3, 0, 3, 0), "Enter moves down");
        // Editing A1 recalculates A3 at once.
        w.handle(Input::Key(Key::Up, Mods::default()), &Fixed);
        for _ in 0..3 {
            key(&mut w, Key::Up);
        }
        type_text(&mut w, "2");
        key(&mut w, Key::Tab);
        assert_eq!(w.view().engine().cell(0, 2, 0).expect("A3").display, "32");
        assert_eq!(w.view().range_bounds(), (0, 1, 0, 1), "Tab moves right");
        assert!(w.view().modified());
    }

    #[test]
    fn escape_cancels_f2_edits_and_delete_clears() {
        let mut w = window();
        type_text(&mut w, "abc");
        key(&mut w, Key::Escape);
        assert_eq!(w.view().engine().cell(0, 0, 0).expect("A1").input, "");
        type_text(&mut w, "abc");
        key(&mut w, Key::Enter);
        key(&mut w, Key::Up);
        key(&mut w, Key::F(2));
        assert!(w.view().editing());
        key(&mut w, Key::Left);
        key(&mut w, Key::Backspace);
        key(&mut w, Key::Enter);
        assert_eq!(w.view().engine().cell(0, 0, 0).expect("A1").input, "ac");
        key(&mut w, Key::Up);
        key(&mut w, Key::Delete);
        assert_eq!(w.view().engine().cell(0, 0, 0).expect("A1").input, "");
        // Undo brings it back, redo clears again.
        w.handle(
            Input::Key(
                Key::Letter('z'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        assert_eq!(w.view().engine().cell(0, 0, 0).expect("A1").input, "ac");
        w.handle(
            Input::Key(
                Key::Letter('y'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        assert_eq!(w.view().engine().cell(0, 0, 0).expect("A1").input, "");
    }

    #[test]
    fn copy_paste_within_the_app_and_text_from_outside() {
        let mut w = window();
        type_text(&mut w, "5");
        key(&mut w, Key::Enter);
        key(&mut w, Key::Up);
        w.handle(
            Input::Key(
                Key::Letter('c'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        let reqs = w.take_requests();
        let text = reqs.iter().find_map(|r| match r {
            Request::SetClipboard { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(text.as_deref(), Some("5\r\n"));
        key(&mut w, Key::Right);
        assert!(w.paste(text));
        assert_eq!(w.view().engine().cell(0, 0, 1).expect("B1").input, "5");
        // Text from another application: tab-separated rows.
        key(&mut w, Key::Down);
        assert!(w.paste(Some("1\t2\r\n3\t4\r\n".into())));
        assert_eq!(w.view().engine().cell(0, 2, 2).expect("C3").input, "4");
    }

    #[test]
    fn ribbon_bold_and_statistics_and_sheet_tabs() {
        let mut w = window();
        for v in ["1", "2", "3"] {
            type_text(&mut w, v);
            key(&mut w, Key::Enter);
        }
        w.handle(
            Input::Key(
                Key::Up,
                Mods {
                    shift: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        w.handle(
            Input::Key(
                Key::Up,
                Mods {
                    shift: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        w.handle(
            Input::Key(
                Key::Up,
                Mods {
                    shift: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        let st = w.view().stats();
        assert_eq!((st.sum, st.count, st.average), (6.0, 3, Some(2.0)));
        assert!(w.action("format.bold", &Fixed));
        assert!(w.view().format().bold, "bold applied to the active cell");
        assert!(w.action("edit.undo", &Fixed));

        assert!(!w.view().format().bold);
        // + adds a sheet and shows it.
        let [x, y, ww, hh] = w.add_rect;
        w.handle(
            Input::Press {
                x: x + ww / 2.0,
                y: y + hh / 2.0,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        assert_eq!(w.view().sheet(), 1);
        assert_eq!(w.view().sheet_names().len(), 2);
    }

    #[test]
    fn grid_image_is_redrawn_only_when_cells_or_scroll_change() {
        let mut w = window();
        let mut store = Store(0);
        w.prepare(&mut store).expect("grid");
        assert!(store.0 > 0);
        store.0 = 0;
        key(&mut w, Key::Down);
        w.prepare(&mut store).expect("grid");
        assert_eq!(store.0, 0, "moving the selection keeps the grid image");
        type_text(&mut w, "x");
        w.prepare(&mut store).expect("grid");
        assert_eq!(store.0, 0, "typing in the editor keeps the grid image");
        key(&mut w, Key::Enter);
        w.prepare(&mut store).expect("grid");
        assert!(store.0 > 0, "committing redraws it");
        store.0 = 0;
        w.handle(Input::Wheel(75.0), &Fixed);
        w.prepare(&mut store).expect("grid");
        assert!(store.0 > 0, "scrolling redraws it");
    }

    #[test]
    fn scrolling_ten_thousand_rows_lays_out_only_visible_rows() {
        let mut e = SheetEngine::with_bundled_fonts();
        e.new_workbook("Big.xlsx").expect("workbook");
        let cells: Vec<(u32, u32, String)> =
            (0..10_000).map(|r| (r, 0, (r + 1).to_string())).collect();
        e.set_inputs(0, &cells).expect("cells");
        let mut w = SheetWindow::new(SheetView::new(e), "en-US", "Big.xlsx");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1000.0,
            },
            &Fixed,
        );
        let mut store = Store(0);
        w.handle(
            Input::Key(
                Key::Down,
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        assert_eq!(w.view().range_bounds().0, 9_999);
        w.prepare(&mut store).expect("grid");
        assert!(store.0 > 0 && store.0 < 2_000, "{} commands", store.0);
    }

    #[test]
    fn decimal_steps() {
        assert_eq!(step_decimals("General", 0, 1), "0.0");
        assert_eq!(step_decimals("General", 2, -1), "0.0");
        assert_eq!(step_decimals("#,##0.00", 0, 1), "#,##0.000");
        assert_eq!(step_decimals("#,##0.0", 0, -1), "#,##0");
        assert_eq!(step_decimals("0%", 0, 1), "0.0%");
    }
}
