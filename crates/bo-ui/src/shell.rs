//! The BlueDoc window: ribbon, document view and status bar, with input routing.
use crate::backstage::{Backstage, FileAction};
use crate::catalog;
use crate::design_gallery::{DesignGallery, DesignMenu};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::doc_borders::{BordersAction, BordersDialog};
use crate::doc_comments::{CommentAction, CommentsPane};
use crate::doc_find::{FindAction, FindDialog};
use crate::doc_tools::{self, Context, Tool};
use crate::doc_view::DocView;
use crate::icons::Icons;
use crate::nav_pane::{Heading, NavAction, NavPane, NavTab};
use crate::popup::{paint_tooltip, Choice, Menu, MenuItem};
use crate::ribbon::{Ribbon, RibbonHit};
use crate::ruler::Ruler;
use crate::spell_pane::{SpellAction, SpellPane};
use crate::strings::Strings;
use crate::symbol_grid::{SymbolGrid, SymbolPick};
use crate::table_grid::{GridPick, TableGrid};
use crate::theme::Theme;
use crate::titlebar::{NcHit, TitleBar};
use crate::{commands, Key, Mods};
use crate::{Input, PageStore, Painter, TextMeasure, TextStyle};
use bo_edit::CharFormat;
use bo_engine::edit::Command;
use bo_render::{PathSeg, Rgba};
use serde_json::json;

#[path = "doc_r3.rs"]
mod r3;
#[path = "doc_r4.rs"]
mod r4;
#[path = "doc_r5.rs"]
mod r5;
#[path = "doc_r6.rs"]
mod r6;
pub use r6::PrintJob;
pub(crate) use r6::{
    install_icons as install_round6_icons, install_strings as install_round6_strings,
};

/// Something only the platform host can do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Write the document to its file.
    Save,
    /// View › Arrange All: stack every open BlueDoc window over the screen.
    ArrangeAll,
    /// Put these forms on the system clipboard.
    SetClipboard {
        /// Plain text.
        text: String,
        /// HTML fragment.
        html: String,
        /// BlueOffice internal form (lossless within the suite).
        internal: String,
    },
    /// Read the system clipboard and call [`DocWindow::paste`].
    Paste,
    /// Let the user pick a mail-merge data file and call [`DocWindow::merge_data`].
    PickMergeData,
    /// New blank document (new window).
    New,
    /// Pick a file and open it (new window).
    Open,
    /// Pick a name and save the document there, then [`DocWindow::set_name`].
    SaveAs,
    /// Print the document.
    Print,
    /// Pick a name and export the document as PDF.
    ExportPdf,
    /// Minimise the window.
    Minimize,
    /// Maximise or restore the window.
    ToggleMaximize,
    /// Close the window.
    Close,
    /// Cover the whole screen without window frame (slide show), or go back.
    FullScreen(bool),
    /// Let the user pick picture files and call [`DocWindow::insert_picture`] for each.
    PickPicture,
    /// Let the user pick a document to compare with and call [`DocWindow::compare_file`].
    PickCompareFile,
    /// Open [`DocWindow::take_new_document`] in a new window (Envelopes and Labels).
    OpenNewDocument,
    /// Store the Design tab's theme, style set and effects as the defaults of new documents
    /// (Design › Set as Default; [`DocWindow::design_defaults`]).
    SaveDefaults,
    /// Open this file in a new window (File › Open / Home recent list).
    OpenPath(String),
    /// New document from one of BlueOffice's own templates (new window; the host calls
    /// [`DocWindow::apply_template`] on it).
    NewFromTemplate(String),
    /// Pick a name and save a copy as this type (`dotx`, `pdf`, `html`, `mht`, `xml`, `epub`).
    SaveAsType(String),
    /// Show this file in the system file manager (File › Info).
    OpenLocation(String),
    /// Print with the File screen's settings.
    PrintJob(PrintJob),
    /// Store the application preferences ([`DocWindow::take_prefs`]).
    SavePrefs,
}

/// Window contents of BlueDoc.
pub struct DocWindow {
    view: DocView,
    title: TitleBar,
    name: String,
    requests: Vec<Request>,
    ribbon: Ribbon,
    theme: Theme,
    icons: Icons,
    strings: Strings,
    size: (f32, f32),
    scale: f32,
    /// The pointer went down in the document (drags select text there).
    doc_press: bool,
    /// Open drop-down list.
    menu: Option<Menu>,
    /// Ticks the pointer has rested on the hovered control, and the tooltip shown.
    rest: u32,
    tooltip: Option<(String, f32, [f32; 4])>,
    /// The tooltip laid out with its description (title, wrapped lines).
    tip: Option<crate::doc_tips::Tip>,
    /// Quick styles of the gallery.
    styles: Vec<bo_engine::gallery::StyleEntry>,
    /// Status bar zoom controls: minus, slider track, plus, percentage.
    zoom_rects: [[f32; 4]; 4],
    /// Status bar view buttons (print layout, web layout, draft), as in a desktop word
    /// processor's status bar.
    view_rects: [[f32; 4]; 3],
    /// Current view layout (`print`, `web`, `draft`).
    view_mode: &'static str,
    /// Vertical scroll bar drag: pointer offset from the thumb top.
    sb_drag: Option<f32>,
    /// The pointer is dragging the zoom slider.
    zoom_drag: bool,
    dialog: Option<Dialog>,
    ruler: Ruler,
    show_ruler: bool,
    ruler_drag: bool,
    marks: bool,
    /// Format painter armed: the next selection gets the picked formatting.
    painter: bool,
    /// Mail-merge preview record, when previewing.
    merge_record: Option<usize>,
    /// Find state: query, matches and the current one.
    find: Option<(String, Vec<bo_edit::find::Match>, usize)>,
    /// Layout must be redone at the next input (needs text measurement).
    relayout_pending: bool,
    /// Last pointer position of a drag in the document (auto-scroll ticks).
    last_drag: Option<(f32, f32)>,
    /// The File screen, when open.
    backstage: Option<Backstage>,
    /// What the contextual tabs shown are for (table, header/footer, picture).
    context: Context,
    /// The next contextual tab that appears becomes active (after inserting a table or a
    /// picture, as the reference does).
    activate_context: bool,
    /// Navigation pane (View › Navigation Pane, Ctrl+F), when shown.
    nav: Option<NavPane>,
    /// The document changed since the pane's headings were read.
    nav_dirty: bool,
    /// Last pointer position (wheel over the pane).
    pointer: (f32, f32),
    /// The document changed since spelling was last checked, and the idle ticks since.
    spell_pending: bool,
    spell_idle: u32,
    /// The misspelt word the context menu offers corrections for.
    spell_menu: Option<bo_engine::spell::SpellInfo>,
    /// Measured widths of the "Header" / "Footer" tags shown while one is edited.
    tag_w: [f32; 2],
    /// Left edges of the status bar's word count, proofing icon and language, laid out one
    /// after the other from measured text as on the reference.
    status_x: [f32; 3],
    /// Status bar texts (page, word count) and the scale they were measured at: typing
    /// re-measures them only when they change.
    status_cache: (String, String, f32),
    /// Quick table look and style options of the Table Design tab.
    table_look: crate::doc_table::TableLook,
    /// Read Mode: zoom and ruler to restore when it ends.
    read_mode: Option<(f32, bool)>,
    /// Read Mode lays the text out for the window at the next layout.
    read_fit: bool,
    /// Width the Read Mode text was laid out for (device px).
    read_w: f32,
    /// A Cell Size box being typed in, and the width of its text (caret position).
    cell_spin: Option<crate::spin_box::SpinEdit>,
    cell_spin_w: f32,
    /// The box's value is selected: the first character typed replaces it.
    cell_spin_fresh: bool,
    /// Measurement unit and decimal separator of the region (set by the host).
    units: crate::doc_table::Units,
    /// Measured height of the caret's row (twips) while the Cell Size box shows it, and
    /// whether it must be measured again (at the next rest).
    row_measured: Option<i32>,
    row_measure_due: bool,
    /// Proofing pane (Review › Spelling & Grammar, F7), when shown.
    spell_pane: Option<SpellPane>,
    /// Comments pane (Review › Show Comments), when shown.
    comments_pane: Option<CommentsPane>,
    /// Thread the reply dialog answers.
    reply_to: Option<i32>,
    /// Text typed in Find what, carried to the next tab of Find and Replace.
    find_draft: Option<String>,
    /// The classic Find and Replace dialog (Ctrl+H, Ctrl+G), when open.
    find_dialog: Option<FindDialog>,
    /// Searches typed in it, most recent first (its combo boxes' history).
    find_history: Vec<String>,
    /// Search options of the matches in `find` (JSON), to search again when they change.
    find_options: String,
    /// Insert › Table size picker, when open.
    table_grid: Option<TableGrid>,
    /// Insert › Symbol gallery, when open.
    symbol_grid: Option<SymbolGrid>,
    /// Design › Themes / Colors / Fonts / Paragraph Spacing drop-down, when open.
    design_gallery: Option<DesignGallery>,
    /// Theme applied from the Design tab (framed in its gallery).
    design_theme: Option<String>,
    /// Document style set applied from the Design tab's gallery.
    design_style_set: Option<String>,
    /// Design › Page Borders dialog, when open.
    borders_dialog: Option<BordersDialog>,
    /// Symbols inserted from the gallery, most recent first.
    recent_symbols: Vec<char>,
    /// The format painter stays on after applying (double click), until Escape.
    painter_lock: bool,
    /// Parity round 3 (paste options, panes, dialogs, outline view).
    r3: r3::R3,
    r4: r4::R4,
    r5: r5::R5,
    r6: r6::R6,
}

/// Font sizes offered by the size box (points).
const SIZES: [f32; 18] = [
    8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0,
    48.0, 72.0,
];

/// Next standard font size, in half-points, matching the shared editor's size steps.
/// Points from twips, without trailing zeros (`12`, `7,5`).
fn fmt_pt(twips: i32) -> String {
    let pt = f64::from(twips) / 20.0;
    let s = format!("{pt:.1}");
    s.trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",")
}

/// A number typed with a comma or a point as decimal separator.
fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Tab of the Find and Replace dialog a dialog kind is (Find, Replace, Go To).
fn find_page(kind: DialogKind) -> Option<usize> {
    match kind {
        DialogKind::Find => Some(0),
        DialogKind::Replace => Some(1),
        DialogKind::GoTo => Some(2),
        _ => None,
    }
}

/// Read Mode zoom (the reference reads at 140 %).
const READ_ZOOM: f32 = 1.4;
/// Text margin either side in Read Mode (points at 100 %).
const READ_MARGIN_PT: f32 = 4.0;

/// Status bar view buttons: view layout and its icon (Bluestar's own drawings).
const VIEW_MODES: [(&str, &str); 3] = [
    ("read", "view.reading"),
    ("print", "layout.margins"),
    ("web", "view.normal"),
];

fn step_font_size(current: u32, grow: bool) -> u32 {
    if grow {
        if current >= 144 {
            return (current / 20)
                .saturating_add(1)
                .saturating_mul(20)
                .min(3276);
        }
        SIZES
            .iter()
            .map(|s| (s * 2.0) as u32)
            .find(|&s| s > current)
            .unwrap_or(144)
    } else {
        if current > 144 {
            return current
                .div_ceil(20)
                .saturating_sub(1)
                .saturating_mul(20)
                .max(144);
        }
        if current <= 16 {
            return current
                .div_ceil(2)
                .saturating_sub(1)
                .saturating_mul(2)
                .max(2);
        }
        SIZES
            .iter()
            .rev()
            .map(|s| (s * 2.0) as u32)
            .find(|&s| s < current)
            .unwrap_or(16)
    }
}
/// Ticks (about 100 ms each) of rest before a tooltip shows.
const TOOLTIP_TICKS: u32 = 5;

impl DocWindow {
    /// Wrap the document `name` shown by `view` with BlueDoc's interface in `locale`.
    pub fn new(view: DocView, locale: &str, name: &str) -> Self {
        let mut strings = Strings::load(locale);
        doc_tools::install_strings(&mut strings);
        crate::doc_table::install_strings(&mut strings);
        let mut icons = Icons::load();
        doc_tools::install_icons(&mut icons);
        crate::doc_table::install_icons(&mut icons);
        crate::doc_r3_menus::install_icons(&mut icons);
        crate::doc_r3_menus::install_strings(&mut strings);
        crate::doc_r4_menus::install_icons(&mut icons);
        crate::doc_r4_menus::install_strings(&mut strings);
        crate::doc_r4_fit::install_strings(&mut strings);
        crate::doc_r4_fit::install_icons(&mut icons);
        icons.set_grey_disabled(true);
        Self {
            view,
            title: TitleBar::new(),
            name: name.to_owned(),
            requests: Vec::new(),
            ribbon: Ribbon::new()
                .with_two_line_labels()
                .with_labeled_widths(crate::doc_table::labeled_w)
                .with_large_widths(crate::doc_table::large_w)
                .with_label_drop(4.0)
                .with_trailing(crate::doc_table::trailing)
                .with_mark_on_own_line()
                .with_row_widths(crate::doc_r4_fit::row_w)
                .with_scaling(
                    crate::doc_r4_fit::scaling,
                    crate::doc_r4_fit::shrink,
                    crate::doc_r4_fit::group_of,
                ),
            theme: Theme::light("doc"),
            icons,
            strings,
            size: (800.0, 600.0),
            scale: 1.0,
            doc_press: false,
            menu: None,
            rest: 0,
            tooltip: None,
            tip: None,
            styles: Vec::new(),
            zoom_rects: [[0.0; 4]; 4],
            view_rects: [[0.0; 4]; 3],
            view_mode: "print",
            sb_drag: None,
            zoom_drag: false,
            dialog: None,
            ruler: Ruler::default(),
            show_ruler: true,
            ruler_drag: false,
            marks: false,
            painter: false,
            merge_record: None,
            find: None,
            relayout_pending: false,
            last_drag: None,
            backstage: None,
            context: Context::default(),
            activate_context: false,
            nav: None,
            nav_dirty: false,
            pointer: (0.0, 0.0),
            spell_pending: true,
            spell_idle: 0,
            spell_menu: None,
            tag_w: [0.0; 2],
            status_x: [0.0; 3],
            status_cache: (String::new(), String::new(), 0.0),
            table_look: crate::doc_table::TableLook::default(),
            read_mode: None,
            read_fit: false,
            read_w: 0.0,
            cell_spin: None,
            cell_spin_w: 0.0,
            cell_spin_fresh: false,
            units: crate::doc_table::Units::default(),
            row_measured: None,
            row_measure_due: true,
            spell_pane: None,
            comments_pane: None,
            reply_to: None,
            find_draft: None,
            find_dialog: None,
            find_history: Vec::new(),
            find_options: String::new(),
            table_grid: None,
            symbol_grid: None,
            design_gallery: None,
            design_theme: None,
            design_style_set: None,
            borders_dialog: None,
            recent_symbols: crate::symbol_grid::DEFAULT_RECENT.to_vec(),
            painter_lock: false,
            r3: r3::R3::default(),
            r4: r4::R4::default(),
            r5: r5::R5::default(),
            r6: r6::R6::default(),
        }
    }

    /// Open the Insert › Table size picker under its button.
    fn open_table_grid(&mut self) -> bool {
        let anchor = self
            .ribbon
            .rect_of(RibbonHit::Command("insert.table"))
            .unwrap_or([0.0, self.ribbon.bottom(), 1.0, 0.0]);
        let commands = vec![self.t("commands.table.fromText", "Convert Text to Table")];
        self.menu = None;
        self.table_grid = Some(TableGrid::open(anchor, commands, self.scale));
        true
    }

    fn open_symbol_grid(&mut self) -> bool {
        let anchor = self
            .ribbon
            .rect_of(RibbonHit::Command("insert.symbol"))
            .unwrap_or([0.0, self.ribbon.bottom(), 1.0, 0.0]);
        let more = self.t("docTable.moreSymbols", "More Symbols…");
        self.menu = None;
        let grid = SymbolGrid::open(anchor, &self.recent_symbols, more, self.scale);
        self.symbol_grid = Some(grid);
        true
    }

    fn open_design_gallery(&mut self, id: &'static str, menu: DesignMenu) -> bool {
        let anchor = self.ribbon.rect_of(RibbonHit::Command(id)).unwrap_or([
            0.0,
            self.ribbon.bottom(),
            1.0,
            0.0,
        ]);
        let spacing = ["Compact", "Balanced", "Open", "Double"]
            .iter()
            .zip([
                "spacingCompact",
                "spacingBalanced",
                "spacingOpen",
                "spacingDouble",
            ])
            .map(|(fallback, key)| self.t(&format!("docTable.{key}"), fallback))
            .collect();
        let spacing = if menu == DesignMenu::StyleSets {
            crate::doc_design::STYLE_TILES
                .iter()
                .map(|(id, _)| self.strings.get(&format!("commands.{id}")).to_owned())
                .collect()
        } else {
            spacing
        };
        let title = if matches!(menu, DesignMenu::Spacing | DesignMenu::StyleSets) {
            self.t("docTable.builtIn", "Built-In")
        } else {
            "BlueOffice".to_owned()
        };
        self.menu = None;
        let mut g = DesignGallery::open(menu, anchor, title, spacing, self.scale);
        g.current.clone_from(&self.design_theme);
        self.design_gallery = Some(g);
        true
    }

    /// Input while a Design drop-down is open; `None` lets it through.
    fn design_gallery_input(&mut self, input: &Input) -> Option<bool> {
        let g = self.design_gallery.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => g.hover(x, y),
            Input::Press { x, y, .. } => {
                let pick = g.press(x, y);
                self.design_gallery = None;
                self.ribbon.clear_pointer();
                if let Some(v) = pick {
                    if v["cmd"] == "applyTheme" {
                        self.design_theme = v["id"].as_str().map(str::to_owned);
                    }
                    if v["cmd"] == "applyDocumentStyleSet" {
                        self.design_style_set = v["id"].as_str().map(str::to_owned);
                    }
                    if let Some(c) = catalog::from_json(v) {
                        self.view.run_command(c);
                    }
                }
                true
            }
            Input::Key(Key::Escape, _) => {
                self.design_gallery = None;
                true
            }
            Input::Text(_) | Input::Key(..) | Input::Wheel(_) | Input::Release { .. } => true,
            _ => return None,
        })
    }

    /// Input while the symbol gallery is open; `None` lets it through.
    fn symbol_grid_input(&mut self, input: &Input) -> Option<bool> {
        let grid = self.symbol_grid.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => grid.hover(x, y),
            Input::Press { x, y, .. } => match grid.press(x, y) {
                Some(SymbolPick::More) => {
                    grid.show_more(self.size.0);
                    true
                }
                Some(SymbolPick::Char(c)) => {
                    self.symbol_grid = None;
                    self.ribbon.clear_pointer();
                    self.recent_symbols.retain(|x| *x != c);
                    self.recent_symbols.insert(0, c);
                    self.recent_symbols.truncate(20);
                    self.view.handle(Input::Text(c.to_string()))
                }
                None => {
                    self.symbol_grid = None;
                    self.ribbon.clear_pointer();
                    true
                }
            },
            Input::Key(Key::Escape, _) => {
                self.symbol_grid = None;
                true
            }
            Input::Text(_) | Input::Key(..) | Input::Wheel(_) | Input::Release { .. } => true,
            _ => return None,
        })
    }

    /// Input while the table size picker is open; `None` lets it through.
    fn table_grid_input(&mut self, input: &Input) -> Option<bool> {
        let grid = self.table_grid.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => grid.hover(x, y),
            Input::Press { x, y, .. } => {
                let pick = grid.press(x, y);
                self.table_grid = None;
                self.ribbon.clear_pointer();
                match pick {
                    Some(GridPick::Size(cols, rows)) => {
                        self.activate_context = true;
                        self.view.run_command(Command::InsertTable { rows, cols })
                    }
                    Some(GridPick::Command(_)) => {
                        let cmd = catalog::from_json(json!({
                            "cmd": "tableEdit",
                            "edit": { "kind": "fromText", "separator": "\t" },
                        }));
                        self.activate_context = true;
                        cmd.is_some_and(|c| self.view.run_command(c))
                    }
                    None => true,
                }
            }
            Input::Key(Key::Escape, _) => {
                self.table_grid = None;
                true
            }
            Input::Text(_) | Input::Key(..) | Input::Wheel(_) | Input::Release { .. } => true,
            _ => return None,
        })
    }

    /// Right edge of the document area (the proofing pane takes the window's right side).
    fn doc_right(&self) -> f32 {
        self.size.0
            - if self.spell_pane.is_some() {
                SpellPane::width(self.scale)
            } else if self.comments_pane.is_some() {
                CommentsPane::width(self.scale)
            } else {
                self.r3_pane_w()
            }
    }

    /// Show or hide the comments pane (it shares the window's right side with the proofing
    /// pane, one at a time).
    fn toggle_comments(&mut self) -> bool {
        if self.comments_pane.take().is_none() {
            self.spell_pane = None;
            self.comments_pane = Some(CommentsPane::new());
        }
        self.relayout_pending = true;
        true
    }

    fn comments_action(&mut self, a: CommentAction) -> bool {
        let cmd = match a {
            CommentAction::Close => {
                self.comments_pane = None;
                self.relayout_pending = true;
                return true;
            }
            CommentAction::Redraw => return true,
            CommentAction::Reply(id) => {
                self.reply_to = Some(id);
                return self.open_dialog(DialogKind::CommentReply);
            }
            CommentAction::Select(id) => json!({ "cmd": "selectComment", "id": id }),
            CommentAction::Resolve(id, done) => {
                json!({ "cmd": "resolveComment", "id": id, "done": done })
            }
            CommentAction::Delete(id) => json!({ "cmd": "deleteComment", "id": id }),
        };
        if let Some(c) = catalog::from_json(cmd) {
            self.view.run_command(c);
        }
        true
    }

    /// Open the proofing pane on the first issue at or after the caret.
    fn open_spelling(&mut self) -> bool {
        self.comments_pane = None;
        let issues = self.view.proof_issues();
        let caret = self.view.caret_para().unwrap_or(0);
        let index = issues.iter().position(|i| i.para >= caret).unwrap_or(0);
        let mut pane = SpellPane::new(issues);
        pane.index = index;
        self.spell_pane = Some(pane);
        self.show_issue();
        self.relayout_pending = true;
        true
    }

    /// Select the issue the pane shows.
    fn show_issue(&mut self) {
        let at = self
            .spell_pane
            .as_ref()
            .and_then(|p| p.current())
            .map(|i| (i.para, i.start, i.end));
        if let Some((para, start, end)) = at {
            self.view.select_range(para, start, end);
        }
    }

    /// Check again after a change and show the next issue from paragraph `para`, offset `from`.
    fn next_issue(&mut self, para: usize, from: usize) {
        let issues = self.view.proof_issues();
        if let Some(pane) = self.spell_pane.as_mut() {
            pane.index = issues
                .iter()
                .position(|i| (i.para, i.start) >= (para, from))
                .unwrap_or(issues.len());
            pane.issues = issues;
        }
        self.show_issue();
        self.spell_pending = true;
    }

    fn spell_pane_action(&mut self, a: SpellAction) -> bool {
        let Some(issue) = self.spell_pane.as_ref().and_then(|p| p.current()).cloned() else {
            if a == SpellAction::Close {
                self.spell_pane = None;
                self.relayout_pending = true;
            }
            return true;
        };
        match a {
            SpellAction::Close => {
                self.spell_pane = None;
                self.relayout_pending = true;
            }
            SpellAction::Change(i) => {
                if let Some(s) = issue.suggestions.get(i) {
                    self.view
                        .replace_range(issue.para, issue.start, issue.end, s);
                    self.next_issue(issue.para, issue.start + s.len());
                }
            }
            SpellAction::IgnoreOnce => {
                if let Some(p) = self.spell_pane.as_mut() {
                    p.index += 1;
                }
                self.show_issue();
            }
            SpellAction::IgnoreAll | SpellAction::AddToDictionary => {
                self.view
                    .spell_word(a == SpellAction::AddToDictionary, &issue.word);
                self.next_issue(issue.para, issue.end);
            }
            SpellAction::Redraw => {}
        }
        true
    }

    /// Status bar texts: page, word count.
    fn status_texts(&self) -> (String, String) {
        // Read Mode counts screens, as the reference's status bar.
        if self.read_mode.is_some() {
            let (scroll, content, viewport) = self.view.scroll_info();
            let total = (content / viewport.max(1.0)).ceil().max(1.0) as usize;
            let current = ((scroll / viewport.max(1.0)).floor() as usize + 1).min(total);
            let page = self.strings.fmt(
                "docTable.screen",
                &[
                    ("current", &current.to_string()),
                    ("total", &total.to_string()),
                ],
            );
            let words = self.strings.fmt(
                "status.words",
                &[("count", &self.view.word_count().to_string())],
            );
            return (page, words);
        }
        let page = self.strings.fmt(
            "status.page",
            &[
                ("current", &(self.view.current_page() + 1).to_string()),
                ("total", &self.view.page_count().to_string()),
            ],
        );
        let words = self.strings.fmt(
            "status.words",
            &[("count", &self.view.word_count().to_string())],
        );
        (page, words)
    }

    /// Place the status bar items after their measured texts. Gaps measured on the
    /// reference at 125 %: 31 px after the page, 80 px after the word count (room of its
    /// macro-recording dot), 27 px after the proofing book (19 px wide).
    fn layout_status(&mut self, text: &dyn TextMeasure) {
        let style = TextStyle {
            size: (12.0 * self.scale).round(),
            bold: false,
        };
        let (page, words) = self.status_texts();
        let c = &self.status_cache;
        if c.0 == page && c.1 == words && c.2.to_bits() == self.scale.to_bits() {
            return;
        }
        let px = |v: f32| (v * self.scale).round();
        let x0 = px(12.0);
        let words_x = x0 + text.measure(&page, style).0.ceil() + px(17.0);
        let book_x = words_x + text.measure(&words, style).0.ceil() + px(54.0);
        let lang_x = book_x + px(15.0) + px(26.0);
        self.status_x = [words_x, book_x, lang_x];
        self.status_cache = (page, words, self.scale);
    }

    /// Style of the "Header" / "Footer" tags (the reference's are small: 60×21 px at 125 %).
    fn tag_style(&self) -> TextStyle {
        TextStyle {
            size: (10.0 * self.scale).round(),
            bold: false,
        }
    }

    /// Commands of the spelling part of the context menu.
    fn spell_action(&mut self, id: &str) -> Option<bool> {
        let info = self.spell_menu.take()?;
        self.spell_pending = true;
        Some(match id {
            "spell.ignoreAll" => self.view.spell_word(false, &info.word),
            "spell.addToDictionary" => self.view.spell_word(true, &info.word),
            _ => {
                let i: usize = id.strip_prefix("spell.use.")?.parse().ok()?;
                let word = info.suggestions.get(i)?;
                self.view
                    .replace_range(info.para, info.start, info.end, word)
            }
        })
    }

    /// Width taken by the navigation pane on the left (0 when hidden).
    fn nav_w(&self) -> f32 {
        let review = self.r6_review_w();
        if self.nav.is_some() {
            NavPane::width(self.scale) + review
        } else {
            review
        }
    }

    /// Show or hide the navigation pane; `search` puts the keyboard in its search box.
    fn toggle_nav(&mut self, show: bool, search: bool) -> bool {
        if show {
            let mut pane = self.nav.take().unwrap_or_else(NavPane::new);
            pane.focus = search;
            self.nav = Some(pane);
            self.refresh_nav();
        } else {
            self.nav = None;
            self.view.clear_highlights();
        }
        self.relayout_pending = true;
        true
    }

    /// Read the headings (and page count) again; mark the heading holding the caret.
    fn refresh_nav(&mut self) {
        self.nav_dirty = false;
        if self.nav.is_none() {
            return;
        }
        let items = self.view.outline();
        let pages = self.view.page_count();
        let caret = self.view.caret_para();
        let current_page = self.view.current_page();
        let aspect = self.view.page_aspect();
        if let Some(nav) = self.nav.as_mut() {
            nav.aspect = aspect;
            nav.headings = items
                .into_iter()
                .filter_map(|i| {
                    i.entry.level.map(|level| Heading {
                        para: i.entry.para,
                        level,
                        text: i.entry.text,
                        page: i.page,
                    })
                })
                .collect();
            nav.pages = pages;
            match nav.tab {
                NavTab::Headings => {
                    nav.current =
                        caret.and_then(|c| nav.headings.iter().rposition(|h| h.para <= c));
                }
                NavTab::Pages => nav.current = Some(current_page),
                NavTab::Results => {}
            }
        }
    }

    /// Carry out what the pane asked for.
    fn nav_action(&mut self, a: NavAction) -> bool {
        match a {
            NavAction::Close => self.toggle_nav(false, false),
            NavAction::Heading(para) => self.view.goto_para(para),
            NavAction::Page(i) => self.view.show_page(i),
            NavAction::Result(i) => {
                let query = self
                    .nav
                    .as_ref()
                    .map(|n| n.query.clone())
                    .unwrap_or_default();
                let fresh = self.find.as_ref().is_none_or(|f| f.0 != query);
                if fresh {
                    let matches = catalog::from_json(json!({ "cmd": "find", "query": query }))
                        .and_then(|c| self.view.exec(c))
                        .and_then(|r| r.matches)
                        .unwrap_or_default();
                    self.find = Some((query, matches, usize::MAX));
                }
                let m = self.find.as_ref().and_then(|f| f.1.get(i).copied());
                if let Some(f) = self.find.as_mut() {
                    f.2 = i;
                }
                match m
                    .and_then(|m| catalog::from_json(json!({ "cmd": "selectMatch", "match": m })))
                {
                    Some(c) => self.view.run_command(c),
                    None => true,
                }
            }
            NavAction::Search => {
                let query = self
                    .nav
                    .as_ref()
                    .map(|n| n.query.clone())
                    .unwrap_or_default();
                let results = if query.is_empty() {
                    self.view.clear_highlights();
                    Vec::new()
                } else {
                    self.view.find_results(&query)
                };
                self.find = None;
                let first = !results.is_empty();
                if let Some(nav) = self.nav.as_mut() {
                    nav.results = results
                        .into_iter()
                        .map(|r| (r.before, r.text, r.after))
                        .collect();
                    nav.searched = !query.is_empty();
                    nav.show(NavTab::Results);
                    nav.current = None;
                }
                // The reference moves to the first match as it searches.
                if first {
                    self.nav_action(NavAction::Result(0));
                    if let Some(nav) = self.nav.as_mut() {
                        nav.current = Some(0);
                    }
                }
                true
            }
            NavAction::Redraw => true,
        }
    }

    /// Input for the navigation pane; `None` lets it through to the window.
    fn nav_input(&mut self, input: &Input) -> Option<bool> {
        let nav = self.nav.as_mut()?;
        match *input {
            Input::Move { x, y } if nav.contains(x, y) => Some(nav.hover(x, y)),
            Input::Move { .. } => {
                nav.hover(-1.0, -1.0);
                None
            }
            Input::Press { x, y, .. } if nav.contains(x, y) => {
                let a = nav.press(x, y, &self.strings);
                Some(self.nav_action(a))
            }
            Input::Press { .. } => {
                nav.focus = false;
                None
            }
            Input::Wheel(dy) if nav.contains(self.pointer.0, self.pointer.1) => Some(nav.wheel(dy)),
            Input::Text(ref s) if nav.focus => {
                nav.text(s);
                Some(true)
            }
            Input::Key(k, m) if nav.focus && !m.ctrl => {
                let a = nav.key(k);
                Some(self.nav_action(a))
            }
            _ => None,
        }
    }

    /// Insert a picture file the host read (`name` with its extension).
    pub fn insert_picture(&mut self, bytes: Vec<u8>, name: &str) -> bool {
        self.activate_context = true;
        let ok = self.view.insert_picture(bytes, name);
        self.sync_context();
        ok
    }

    /// Bring the ribbon up to date after host calls such as [`DocWindow::insert_picture`]
    /// (contextual tabs, layout); `true` when it changed.
    pub fn refresh(&mut self, text: &dyn TextMeasure) -> bool {
        self.sync_context();
        let pending = self.relayout_pending;
        if pending {
            self.relayout_pending = false;
            self.relayout(text);
        }
        self.layout_status(text);
        self.r3_layout(text);
        if let Some(sp) = self.spell_pane.as_mut() {
            sp.measure(text);
        }
        if let Some(g) = self.symbol_grid.as_mut() {
            g.measure(text);
        }
        if let Some(fd) = self.find_dialog.as_mut() {
            fd.layout(text, &self.strings, self.size, self.scale);
        }
        if let Some(bd) = self.borders_dialog.as_mut() {
            bd.layout(text, &self.strings, self.size, self.scale);
        }
        if let Some(g) = self.design_gallery.as_mut() {
            g.measure(text);
        }
        if let Some(cp) = self.comments_pane.as_mut() {
            let caret = self.view.comment_at_caret();
            cp.measure(self.view.comments(), caret, &self.strings, text);
        }
        pending
    }

    /// Run interface command `id` (ribbon, menu entry or shortcut id) as if chosen, then
    /// bring the ribbon up to date; `text:…` types text at the caret, `tab:…` shows a ribbon
    /// tab (`tab:tableLayout`).
    pub fn run_interface_command(&mut self, id: &str, text: &dyn TextMeasure) -> bool {
        let done = if let Some(t) = id.strip_prefix("text:") {
            self.view.handle(Input::Text(t.to_owned()))
        } else if let Some(label) = id.strip_prefix("tab:") {
            self.sync_context();
            self.relayout_pending = true;
            self.ribbon.select_tab(&format!("ribbon.tabs.{label}"))
        } else {
            self.action(id)
        };
        self.refresh(text);
        done
    }

    /// Lines of the Word Count dialog, as the reference lists them: pages, words,
    /// characters without and with spaces, paragraphs (the selection's when one is made).
    fn word_count_lines(&mut self) -> Vec<String> {
        let pages = self.view.page_count();
        let Some(info) = self.view.word_counts() else {
            return vec![format!(
                "{}: {}",
                self.t("wordCount.words", "Words"),
                self.view.word_count()
            )];
        };
        let c = info.selection.unwrap_or(info.document);
        [
            ("wordCount.pages", "Pages", pages),
            ("wordCount.words", "Words", c.words),
            ("wordCount.chars", "Characters (no spaces)", c.chars),
            (
                "wordCount.charsWithSpaces",
                "Characters (with spaces)",
                c.chars_with_spaces,
            ),
            ("wordCount.paragraphs", "Paragraphs", c.paragraphs),
        ]
        .into_iter()
        .map(|(k, f, n)| format!("{}: {n}", self.t(k, f)))
        .collect()
    }

    /// Header/footer options of the open header or footer: (different first page,
    /// different odd and even pages, linked to the previous section).
    fn story_options(&self) -> Option<(bool, bool, bool)> {
        self.view
            .story()
            .filter(|s| s.kind == "header" || s.kind == "footer")
            .map(|s| (s.title_page, s.even_and_odd, s.linked))
    }

    /// Show the contextual tabs the selection needs (table, header/footer, picture).
    fn sync_context(&mut self) {
        let mut c = Context::of(self.view.format(), self.view.story().map(|s| s.kind));
        c.outline = self.r3.outline.is_some();
        if c == self.context && !self.activate_context {
            return;
        }
        // Opening a header or footer, inserting a table or a picture brings its tab
        // forward; moving the caret into an existing table only shows the tabs.
        let story_opened = c.story.is_some() && self.context.story.is_none();
        let new_object = (c.table && !self.context.table) || (c.picture && !self.context.picture);
        let activate = story_opened
            || (self.activate_context && new_object)
            || (c.outline && !self.context.outline);
        self.activate_context = false;
        if c != self.context || activate {
            self.context = c;
            self.ribbon.set_contextual(doc_tools::tabs(c), activate);
            self.relayout_pending = true;
        }
    }

    /// Measurement unit of the region (inches in the US system, else centimetres) and its
    /// decimal separator, as the reference shows sizes.
    pub fn set_units(&mut self, inches: bool, decimal: char) {
        self.units = crate::doc_table::Units { inches, decimal };
    }

    /// The document now has this file name (after Save As).
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
        self.relayout_pending = true;
    }

    fn file_action(&mut self, a: FileAction) -> bool {
        if a != FileAction::Back {
            let r = match a {
                FileAction::New => Request::New,
                FileAction::Open => Request::Open,
                FileAction::Save => Request::Save,
                FileAction::SaveAs => Request::SaveAs,
                FileAction::Print => Request::Print,
                FileAction::ExportPdf => Request::ExportPdf,
                _ => Request::Close,
            };
            self.requests.push(r);
        }
        self.backstage = None;
        true
    }

    /// Mail-merge data the host read (`name` with its extension, bytes in base64).
    pub fn merge_data(&mut self, name: &str, base64: &str) -> bool {
        catalog::from_json(json!({ "cmd": "mergeData", "name": name, "base64": base64 }))
            .is_some_and(|c| self.view.run_command(c))
    }

    /// Text of `key`, or `fallback` when the locale has no such key.
    fn t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    fn open_dialog(&mut self, kind: DialogKind) -> bool {
        let sel = self
            .find_draft
            .take()
            .or_else(|| self.find.as_ref().map(|f| f.0.clone()))
            .unwrap_or_default();
        let ok = self.t("common.ok", "OK");
        let cancel = self.t("common.cancel", "Cancel");
        let (title, fields, buttons, message) = match kind {
            DialogKind::Find => (
                self.t("commands.edit.find", "Find"),
                vec![(self.t("find.findWhat", "Find what"), sel)],
                vec![
                    (self.t("find.next", "Find Next"), DialogButton::FindNext),
                    (cancel, DialogButton::Cancel),
                ],
                vec![],
            ),
            DialogKind::Replace => (
                self.t("commands.edit.replace", "Replace"),
                vec![
                    (self.t("find.findWhat", "Find what"), sel),
                    (self.t("find.replaceWith", "Replace with"), String::new()),
                ],
                vec![
                    (self.t("find.next", "Find Next"), DialogButton::FindNext),
                    (self.t("find.replace", "Replace"), DialogButton::Replace),
                    (
                        self.t("find.replaceAll", "Replace All"),
                        DialogButton::ReplaceAll,
                    ),
                    (cancel, DialogButton::Cancel),
                ],
                vec![],
            ),
            DialogKind::Link => (
                self.t("commands.insert.link", "Insert link"),
                vec![
                    (self.t("link.address", "Address"), "https://".into()),
                    (self.t("link.display", "Text to display"), String::new()),
                ],
                vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                vec![],
            ),
            DialogKind::Font => {
                let f = self.view.format();
                let size = f.size_half_points.map_or_else(String::new, |h| {
                    if h % 2 == 0 {
                        (h / 2).to_string()
                    } else {
                        format!("{},5", h / 2)
                    }
                });
                (
                    self.t("docStyles.dialog.font", "Font"),
                    vec![
                        (
                            self.t("docStyles.dialog.font", "Font"),
                            f.font.clone().unwrap_or_default(),
                        ),
                        (self.t("docStyles.dialog.size", "Size (pt)"), size),
                        (
                            self.t("docStyles.dialog.color", "Colour"),
                            f.color.clone().unwrap_or_default(),
                        ),
                    ],
                    vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                    vec![],
                )
            }
            DialogKind::GoTo => (
                self.t("sheetEditor.goto.title", "Go To"),
                vec![(
                    self.t("ribbon.docTools.goToPage", "Enter page number"),
                    (self.view.current_page() + 1).to_string(),
                )],
                vec![
                    (self.t("inspect.goTo", "Go To"), DialogButton::Ok),
                    (cancel, DialogButton::Cancel),
                ],
                vec![],
            ),
            DialogKind::TableProperties | DialogKind::CellMargins => {
                let Some(t) = self.view.format().table.clone() else {
                    return false;
                };
                // In the region's unit, as the reference's table dialogs.
                let units = self.units;
                let cm = |twips: i32| units.number(twips);
                let unit = if units.inches { "in" } else { "cm" };
                let f = |key: &str, fallback: &str, v: String| {
                    (format!("{} ({unit})", self.t(key, fallback)), v)
                };
                let (title, fields) = if kind == DialogKind::TableProperties {
                    let width = match (t.width_pct, t.width) {
                        (None, w) if w > 0 => cm(w),
                        _ => String::new(),
                    };
                    (
                        self.t("docTable.properties", "Table Properties"),
                        vec![
                            f("docTable.preferredWidth", "Preferred width", width),
                            f(
                                "docTable.rowHeight",
                                "Row height",
                                if t.row_height > 0 {
                                    cm(t.row_height)
                                } else {
                                    String::new()
                                },
                            ),
                            f("docTable.colWidth", "Column width", cm(t.col_width)),
                        ],
                    )
                } else {
                    (
                        self.t("docTable.cellMargins", "Cell Margins"),
                        vec![
                            f("pageSetup.left", "Left", cm(t.cell_margin.0)),
                            f("pageSetup.right", "Right", cm(t.cell_margin.1)),
                        ],
                    )
                };
                (
                    title,
                    fields,
                    vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                    vec![],
                )
            }
            DialogKind::PageSetup => {
                let s = &self.view.format().section;
                // Centimetres with two decimals at most, decimal comma.
                let cm = |twips: i32| {
                    format!("{:.2}", f64::from(twips) * 2.54 / 1440.0)
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .replace('.', ",")
                };
                let f = |key: &str, fallback: &str, twips: i32| {
                    (format!("{} (cm)", self.t(key, fallback)), cm(twips))
                };
                (
                    self.t("pageSetup.title", "Page Setup"),
                    vec![
                        f("pageSetup.top", "Top", s.margin_top),
                        f("pageSetup.bottom", "Bottom", s.margin_bottom),
                        f("pageSetup.left", "Left", s.margin_left),
                        f("pageSetup.right", "Right", s.margin_right),
                        f("pageSetup.width", "Width", s.width),
                        f("pageSetup.height", "Height", s.height),
                        f("pageSetup.header", "Header from edge", s.header),
                        f("pageSetup.footer", "Footer from edge", s.footer),
                    ],
                    vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                    vec![],
                )
            }
            DialogKind::Paragraph => {
                let f = self.view.format();
                let lines = f64::from(f.line_spacing.value) / 240.0;
                let lines = format!("{lines:.2}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .replace('.', ",");
                (
                    self.t("docStyles.dialog.typeParagraph", "Paragraph"),
                    vec![
                        (
                            self.t("docStyles.dialog.indentLeft", "Left indent (pt)"),
                            fmt_pt(f.indent_left),
                        ),
                        (
                            self.t("docStyles.dialog.indentRight", "Right indent (pt)"),
                            fmt_pt(f.indent_right),
                        ),
                        (
                            self.t("docStyles.dialog.indentFirstLine", "First line (pt)"),
                            fmt_pt(f.first_line),
                        ),
                        (
                            self.t("docStyles.dialog.spaceBefore", "Space before (pt)"),
                            fmt_pt(f.space_before),
                        ),
                        (
                            self.t("docStyles.dialog.spaceAfter", "Space after (pt)"),
                            fmt_pt(f.space_after),
                        ),
                        (
                            self.t("docStyles.dialog.lineSpacing", "Line spacing (lines)"),
                            lines,
                        ),
                    ],
                    vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                    vec![],
                )
            }
            DialogKind::Comment => (
                self.t("commands.review.newComment", "New comment"),
                vec![(self.t("comments.text", "Comment"), String::new())],
                vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                vec![],
            ),
            DialogKind::CommentReply => (
                self.t("comments.reply", "Reply"),
                vec![(self.t("comments.replyPlaceholder", "Reply…"), String::new())],
                vec![(ok, DialogButton::Ok), (cancel, DialogButton::Cancel)],
                vec![],
            ),
            DialogKind::WordCount => (
                self.t("commands.review.wordCount", "Word Count"),
                vec![],
                vec![(self.t("common.close", "Close"), DialogButton::Cancel)],
                self.word_count_lines(),
            ),
        };
        self.menu = None;
        // Find, Replace and Go To are the three tabs of one "Find and Replace" dialog, as on
        // the reference.
        let page = find_page(kind);
        let title = if page.is_some() {
            self.t("docTable.findReplace", "Find and Replace")
        } else {
            title
        };
        let n = fields.len();
        let mut dialog = Dialog::new(kind, title, fields, buttons, message);
        if let Some(page) = page {
            let labels = vec![
                self.t("commands.edit.find", "Find"),
                self.t("commands.edit.replace", "Replace"),
                self.t("docTable.goToTab", "Go To"),
            ];
            dialog = dialog.with_tabs(labels, &vec![page; n.max(1)]);
            dialog.set_page(page);
        }
        if kind == DialogKind::Font {
            // Style check boxes and a preview, as in a desktop word processor's Font dialog.
            let f = self.view.format();
            let checks = vec![
                (self.t("docStyles.dialog.bold", "Bold"), f.bold),
                (self.t("docStyles.dialog.italic", "Italic"), f.italic),
                (
                    self.t("docStyles.dialog.underline", "Underline"),
                    f.underline,
                ),
            ];
            dialog = dialog.with_checks(checks);
        }
        self.dialog = Some(dialog);
        self.relayout_pending = true;
        true
    }

    /// Open the Find and Replace dialog on tab `tab` (0 Find, 1 Replace, 2 Go To).
    fn open_find_dialog(&mut self, tab: usize) -> bool {
        let sel = self.find.as_ref().map(|f| f.0.clone()).unwrap_or_default();
        self.menu = None;
        let mut fd = FindDialog::new(tab, &sel, self.find_history.clone());
        fd.bookmarks = self
            .view
            .engine()
            .document()
            .map(bo_edit::fields::bookmarks)
            .unwrap_or_default()
            .into_iter()
            .map(|b| b.name)
            .collect();
        self.find_dialog = Some(fd);
        self.relayout_pending = true;
        true
    }

    /// Carry out what the Find and Replace dialog asked for.
    fn find_dialog_action(&mut self, a: FindAction) -> bool {
        let Some(fd) = self.find_dialog.as_mut() else {
            return false;
        };
        let query = fd.query();
        let with = fd.replacement();
        let options = fd.options();
        if matches!(
            a,
            FindAction::FindNext | FindAction::Replace | FindAction::ReplaceAll
        ) {
            fd.remember();
            self.find_history = fd.history().to_vec();
        }
        match a {
            FindAction::None => return false,
            FindAction::Redraw => {}
            FindAction::Close => self.find_dialog = None,
            FindAction::FindNext => self.find_with(&query, &options),
            FindAction::Replace => {
                if let Some((q, matches, i)) = self.find.clone().filter(|f| f.0 == query) {
                    if let Some(m) = matches.get(i) {
                        let cmd = catalog::from_json(json!({
                            "cmd": "replace", "match": m, "with": with, "query": q,
                            "options": options,
                        }));
                        if let Some(c) = cmd {
                            self.view.run_command(c);
                        }
                    }
                    self.find = None;
                }
                self.find_with(&query, &options);
            }
            FindAction::ReplaceAll => {
                let cmd = catalog::from_json(json!({
                    "cmd": "replaceAll", "query": query, "with": with, "options": options,
                }));
                let n = cmd
                    .and_then(|c| self.view.exec(c))
                    .and_then(|r| r.count)
                    .unwrap_or(0);
                self.find = None;
                if let Some(fd) = self.find_dialog.as_mut() {
                    fd.message = fd
                        .label(crate::doc_find::L_REPLACED)
                        .replace("{{count}}", &n.to_string());
                }
            }
            FindAction::GoTo(what, typed) if what != 0 => {
                self.go_to_target(what, &typed);
            }
            FindAction::GoTo(_, typed) => {
                // Page numbers count from 1; "+n" / "-n" move from the current page.
                let t = typed.trim();
                let current = self.view.current_page() as i64;
                let target = if let Some(n) = t.strip_prefix('+') {
                    n.parse::<i64>().ok().map(|n| current + n)
                } else if let Some(n) = t.strip_prefix('-') {
                    n.parse::<i64>().ok().map(|n| current - n)
                } else {
                    t.parse::<i64>().ok().map(|n| n - 1)
                };
                if let Some(page) = target {
                    let last = self.view.page_count().saturating_sub(1) as i64;
                    self.view.show_page(page.clamp(0, last) as usize);
                }
            }
        }
        true
    }

    /// Go To for a target other than Page: section, line, bookmark, comment or heading
    /// number `typed` (1-based), "+n" / "-n" from the caret, or a bookmark's name.
    fn go_to_target(&mut self, what: usize, typed: &str) -> bool {
        let t = typed.trim();
        let relative = |t: &str| -> Option<i64> {
            if let Some(n) = t.strip_prefix('+') {
                n.trim().parse::<i64>().ok()
            } else if let Some(n) = t.strip_prefix('-').or_else(|| t.strip_prefix('\u{2013}')) {
                n.trim().parse::<i64>().ok().map(|n| -n)
            } else {
                None
            }
        };
        if what == 2 {
            // Lines through the layout: from the document start (absolute) or the caret.
            let (from_start, count) = match relative(t) {
                Some(n) => (false, n),
                None => (true, t.parse::<i64>().unwrap_or(1) - 1),
            };
            if from_start {
                let ctrl = Mods {
                    ctrl: true,
                    ..Mods::default()
                };
                self.view.handle(Input::Key(Key::Home, ctrl));
            }
            let key = if count < 0 { Key::Up } else { Key::Down };
            for _ in 0..count.unsigned_abs().min(100_000) {
                self.view.handle(Input::Key(key, Mods::default()));
            }
            return true;
        }
        // Targets in document order: (paragraph, offset).
        let mut names: Vec<String> = Vec::new();
        let targets: Vec<(usize, usize)> = match what {
            1 => {
                let mut last = None;
                self.view
                    .outline()
                    .into_iter()
                    .filter(|i| last.replace(i.entry.section) != Some(i.entry.section))
                    .map(|i| (i.entry.para, 0))
                    .collect()
            }
            crate::doc_find::GO_BOOKMARK => self
                .view
                .engine()
                .document()
                .map(bo_edit::fields::bookmarks)
                .unwrap_or_default()
                .into_iter()
                .map(|b| {
                    names.push(b.name);
                    (b.para, 0)
                })
                .collect(),
            4 => self
                .view
                .comments()
                .iter()
                .filter(|c| c.parent.is_none())
                .filter_map(|c| c.start.map(|p| (p.para, p.offset)))
                .collect(),
            12 => self
                .view
                .outline()
                .into_iter()
                .filter(|i| i.entry.level.is_some())
                .map(|i| (i.entry.para, 0))
                .collect(),
            // Footnote … Object (round 4): found in the document model.
            5..=11 => {
                let (t, n) = self
                    .view
                    .engine()
                    .document()
                    .map(|d| crate::doc_goto::named_targets(d, what))
                    .unwrap_or_default();
                names = n;
                t
            }
            _ => return false,
        };
        if targets.is_empty() {
            return false;
        }
        let caret = self.view.caret_para().unwrap_or(0);
        let current = targets.iter().rposition(|(p, _)| *p <= caret);
        let index = if let Some(n) = relative(t) {
            match current {
                Some(c) => c as i64 + n,
                None if n > 0 => n - 1,
                None => return false,
            }
        } else if let Ok(n) = t.parse::<i64>() {
            n - 1
        } else if let Some(i) = names.iter().position(|n| n.eq_ignore_ascii_case(t)) {
            i as i64
        } else {
            return false;
        };
        let last = targets.len() as i64 - 1;
        let (para, offset) = targets[index.clamp(0, last) as usize];
        self.view.select_range(para, offset, offset)
    }

    /// Find the next match of `query` with search `options` (JSON) and select it.
    fn find_with(&mut self, query: &str, options: &serde_json::Value) {
        if query.is_empty() {
            return;
        }
        let key = options.to_string();
        let fresh = self.find_options != key || self.find.as_ref().is_none_or(|f| f.0 != query);
        if fresh {
            self.find_options = key;
            let matches =
                catalog::from_json(json!({ "cmd": "find", "query": query, "options": options }))
                    .and_then(|c| self.view.exec(c))
                    .and_then(|r| r.matches)
                    .unwrap_or_default();
            self.find = Some((query.to_owned(), matches, usize::MAX));
        }
        let found = match self.find.as_mut() {
            Some((_, matches, i)) if !matches.is_empty() => {
                *i = if *i == usize::MAX {
                    0
                } else {
                    (*i + 1) % matches.len()
                };
                let (n, total, m) = (*i + 1, matches.len(), matches[*i]);
                if let Some(c) = catalog::from_json(json!({ "cmd": "selectMatch", "match": m })) {
                    self.view.run_command(c);
                }
                Some((n, total))
            }
            _ => None,
        };
        if let Some(fd) = self.find_dialog.as_mut() {
            fd.message = match found {
                Some((n, total)) => fd
                    .label(crate::doc_find::L_FOUND)
                    .replace("{{n}}", &n.to_string())
                    .replace("{{total}}", &total.to_string()),
                None => fd.label(crate::doc_find::L_NOT_FOUND),
            };
        }
    }

    /// The Find and Replace dialog shows another tab: open that tab's kind in its place,
    /// keeping the text typed in Find what.
    fn sync_find_tabs(&mut self) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let Some(cur) = find_page(d.kind) else {
            return;
        };
        let page = d.page();
        if page == cur {
            return;
        }
        if cur != 2 {
            self.find_draft = d.values().first().cloned();
        }
        let kind = [DialogKind::Find, DialogKind::Replace, DialogKind::GoTo][page.min(2)];
        if kind == DialogKind::GoTo {
            self.find_draft = None;
        }
        self.open_dialog(kind);
    }

    fn dialog_button(&mut self, b: DialogButton) {
        let Some(d) = self.dialog.as_ref() else {
            return;
        };
        let kind = d.kind;
        let values = d.values();
        let v = |i: usize| values.get(i).cloned().unwrap_or_default();
        match (kind, b) {
            (_, DialogButton::Cancel) | (DialogKind::WordCount, _) => self.dialog = None,
            // Owner-defined buttons are not used by BlueDoc's dialogs.
            (_, DialogButton::Extra(_)) => {}
            (DialogKind::Link, _) => {
                self.dialog = None;
                self.view.run_command(Command::SetLink {
                    target: v(0),
                    display: v(1),
                });
            }
            (DialogKind::Font, DialogButton::Ok) => {
                self.dialog = None;
                let f = self.view.format().clone();
                let (name, size, color) = (v(0), v(1), v(2));
                let name = name.trim();
                if !name.is_empty() && f.font.as_deref() != Some(name) {
                    self.view.run_command(Command::FormatText {
                        format: CharFormat::Font { name: name.into() },
                    });
                }
                if let Some(pt) = parse_num(&size) {
                    let half_points = (pt * 2.0).round().clamp(2.0, 3276.0) as u32;
                    if f.size_half_points != Some(half_points) {
                        self.view.run_command(Command::FormatText {
                            format: CharFormat::Size { half_points },
                        });
                    }
                }
                let hex = color.trim().trim_start_matches('#').to_uppercase();
                let hex =
                    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())).then_some(hex);
                if hex != f.color && (hex.is_some() || color.trim().is_empty()) {
                    self.view.run_command(Command::FormatText {
                        format: CharFormat::Color { hex },
                    });
                }
                // Bold, italic and underline toggle when their box changed.
                for (i, on, format) in [
                    (3, f.bold, CharFormat::Bold),
                    (4, f.italic, CharFormat::Italic),
                    (5, f.underline, CharFormat::Underline),
                ] {
                    if (v(i) == "1") != on {
                        self.view.run_command(Command::FormatText { format });
                    }
                }
            }
            (DialogKind::Paragraph, DialogButton::Ok) => {
                self.dialog = None;
                let f = self.view.format().clone();
                let twips = |s: &str| parse_num(s).map(|pt| (pt * 20.0).round() as i32);
                let (left, right, first) = (
                    twips(&v(0)).unwrap_or(f.indent_left),
                    twips(&v(1)).unwrap_or(f.indent_right),
                    twips(&v(2)).unwrap_or(f.first_line),
                );
                if (left, right, first) != (f.indent_left, f.indent_right, f.first_line) {
                    self.view.run_command(Command::FormatParagraph {
                        format: bo_edit::ParaFormat::Indents {
                            left,
                            right,
                            first_line: first,
                        },
                    });
                }
                if let Some(t) = twips(&v(3)).filter(|t| *t != f.space_before) {
                    self.view.run_command(Command::FormatParagraph {
                        format: bo_edit::ParaFormat::SpaceBefore { twips: t.max(0) },
                    });
                }
                if let Some(t) = twips(&v(4)).filter(|t| *t != f.space_after) {
                    self.view.run_command(Command::FormatParagraph {
                        format: bo_edit::ParaFormat::SpaceAfter { twips: t.max(0) },
                    });
                }
                if let Some(lines) = parse_num(&v(5)) {
                    let value = (lines * 240.0).round().clamp(60.0, 5000.0) as u32;
                    if value != f.line_spacing.value {
                        self.view.run_command(Command::FormatParagraph {
                            format: bo_edit::ParaFormat::LineSpacing { value },
                        });
                    }
                }
            }
            (DialogKind::PageSetup, DialogButton::Ok) => {
                self.dialog = None;
                let s = self.view.format().section;
                let twips = |i: usize, old: i32| {
                    parse_num(&v(i)).map_or(old, |cm| (cm * 1440.0 / 2.54).round() as i32)
                };
                // ECMA-376 Part 1, §17.6.11 pgMar, §17.6.13 pgSz.
                let cmd = catalog::from_json(json!({
                    "cmd": "setPageSetup",
                    "setup": {
                        "width": twips(4, s.width).max(1440), "height": twips(5, s.height).max(1440),
                        "marginTop": twips(0, s.margin_top), "marginBottom": twips(1, s.margin_bottom),
                        "marginLeft": twips(2, s.margin_left).max(0), "marginRight": twips(3, s.margin_right).max(0),
                        "header": twips(6, s.header).max(0), "footer": twips(7, s.footer).max(0),
                        "columns": s.columns.max(1), "columnSpace": s.column_space,
                    },
                    "scope": "document",
                }));
                if let Some(c) = cmd {
                    self.view.run_command(c);
                }
            }
            (DialogKind::TableProperties, DialogButton::Ok) => {
                self.dialog = None;
                let units = self.units;
                let twips = |i: usize| units.parse(&v(i));
                let edits = [
                    json!({ "kind": "properties", "width": twips(0).filter(|w| *w > 0) }),
                    json!({ "kind": "rowHeight", "twips": twips(1).filter(|h| *h > 0) }),
                ];
                for e in edits {
                    if let Some(c) = catalog::from_json(json!({ "cmd": "tableEdit", "edit": e })) {
                        self.view.run_command(c);
                    }
                }
                if let Some(c) = twips(2)
                    .filter(|w| *w > 0)
                    .and_then(|w| crate::doc_table::spin_command(crate::doc_table::SPINS[1], w))
                {
                    self.view.run_command(c);
                }
            }
            (DialogKind::CellMargins, DialogButton::Ok) => {
                self.dialog = None;
                let t = self
                    .view
                    .format()
                    .table
                    .as_ref()
                    .map_or((108, 108), |t| t.cell_margin);
                let twips = |i: usize, old: i32| self.units.parse(&v(i)).unwrap_or(old);
                let e =
                    json!({ "kind": "cellMargins", "left": twips(0, t.0), "right": twips(1, t.1) });
                if let Some(c) = catalog::from_json(json!({ "cmd": "tableEdit", "edit": e })) {
                    self.view.run_command(c);
                }
            }
            (DialogKind::GoTo, DialogButton::Ok) => {
                // Page numbers count from 1; "+n" / "-n" move from the current page.
                let typed = v(0);
                let t = typed.trim();
                let current = self.view.current_page() as i64;
                let target = if let Some(n) = t.strip_prefix('+') {
                    n.parse::<i64>().ok().map(|n| current + n)
                } else if let Some(n) = t.strip_prefix('-') {
                    n.parse::<i64>().ok().map(|n| current - n)
                } else {
                    t.parse::<i64>().ok().map(|n| n - 1)
                };
                if let Some(page) = target {
                    let last = self.view.page_count().saturating_sub(1) as i64;
                    self.dialog = None;
                    self.view.show_page(page.clamp(0, last) as usize);
                }
            }
            (
                DialogKind::Font
                | DialogKind::Paragraph
                | DialogKind::PageSetup
                | DialogKind::GoTo
                | DialogKind::TableProperties
                | DialogKind::CellMargins,
                _,
            ) => {
                self.dialog = None;
            }
            (DialogKind::CommentReply, _) => {
                self.dialog = None;
                let text = v(0);
                if let Some(id) = self.reply_to.take().filter(|_| !text.trim().is_empty()) {
                    if let Some(c) =
                        catalog::from_json(json!({ "cmd": "replyComment", "id": id, "text": text }))
                    {
                        self.view.run_command(c);
                    }
                }
            }
            (DialogKind::Comment, _) => {
                self.dialog = None;
                if let Some(c) = catalog::from_json(json!({ "cmd": "addComment", "text": v(0) })) {
                    self.view.run_command(c);
                }
            }
            (_, DialogButton::FindNext | DialogButton::Ok) => self.find_next(&v(0)),
            (_, DialogButton::Replace) => {
                let (query, with) = (v(0), v(1));
                if let Some((q, matches, i)) = self.find.clone().filter(|f| f.0 == query) {
                    if let Some(m) = matches.get(i) {
                        let cmd = catalog::from_json(json!({
                            "cmd": "replace", "match": m, "with": with, "query": q,
                        }));
                        if let Some(c) = cmd {
                            self.view.run_command(c);
                        }
                    }
                    self.find = None;
                }
                self.find_next(&query);
            }
            (_, DialogButton::ReplaceAll) => {
                let cmd =
                    catalog::from_json(json!({ "cmd": "replaceAll", "query": v(0), "with": v(1) }));
                let n = cmd
                    .and_then(|c| self.view.exec(c))
                    .and_then(|r| r.count)
                    .unwrap_or(0);
                self.find = None;
                let msg = format!("{n} {}", self.t("find.replacements", "replacements made"));
                if let Some(d) = self.dialog.as_mut() {
                    d.set_message(vec![msg]);
                }
            }
        }
    }

    fn find_next(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        let fresh = self.find.as_ref().is_none_or(|f| f.0 != query);
        if fresh {
            let matches = catalog::from_json(json!({ "cmd": "find", "query": query }))
                .and_then(|c| self.view.exec(c))
                .and_then(|r| r.matches)
                .unwrap_or_default();
            self.find = Some((query.to_owned(), matches, usize::MAX));
        }
        let msg = match self.find.as_mut() {
            Some((_, matches, i)) if !matches.is_empty() => {
                *i = if *i == usize::MAX {
                    0
                } else {
                    (*i + 1) % matches.len()
                };
                let (n, total, m) = (*i + 1, matches.len(), matches[*i]);
                if let Some(c) = catalog::from_json(json!({ "cmd": "selectMatch", "match": m })) {
                    self.view.run_command(c);
                }
                format!("{n} / {total}")
            }
            _ => self.t("find.none", "No matches"),
        };
        if let Some(d) = self.dialog.as_mut() {
            d.set_message(vec![msg]);
        }
    }

    fn open_command_menu(&mut self, id: &str, anchor: [f32; 4]) {
        let spacing = self.view.format().line_spacing;
        let (before, after) = (
            self.view.format().space_before,
            self.view.format().space_after,
        );
        let items = catalog::menu(id)
            .into_iter()
            .map(|e| MenuItem {
                label: match e.id {
                    "format.spaceBefore.toggle" if before > 0 => {
                        "Remove Space Before Paragraph".to_owned()
                    }
                    "format.spaceAfter.toggle" if after > 0 => {
                        "Remove Space After Paragraph".to_owned()
                    }
                    _ if e.label.contains('.')
                        && !e.label.contains(' ')
                        && e.label.chars().any(char::is_alphabetic) =>
                    {
                        self.strings.get(e.label).to_owned()
                    }
                    _ => e.label.to_owned(),
                },
                choice: Choice::Command(e.id.to_owned()),
                checked: doc_tools::checked(e.id, self.view.markup())
                    || spacing.rule == bo_edit::LineRule::Auto
                        && match e.id {
                            "format.lineSpacing1" => spacing.value == 240,
                            "format.lineSpacing115" => spacing.value == 276,
                            "format.lineSpacing15" => spacing.value == 360,
                            "format.lineSpacing2" => spacing.value == 480,
                            "format.lineSpacing25" => spacing.value == 600,
                            "format.lineSpacing3" => spacing.value == 720,
                            _ => false,
                        },
                dim: false,
            })
            .collect();
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    fn context_menu(&mut self, x: f32, y: f32) {
        // Clipboard, then insert commands behind a separator, with icons; cut and copy dim
        // without a selection, as in a desktop word processor's context menu.
        let selected = self.view.has_selection();
        let ids = [
            ("edit.cut", "edit.cut"),
            ("edit.copy", "edit.copy"),
            ("edit.paste", "edit.paste"),
            ("insert.link", "insert.link"),
            ("review.newComment", "review.newComment"),
        ];
        let items = ids
            .iter()
            .map(|(id, _)| MenuItem {
                label: self.strings.get(&format!("commands.{id}")).to_owned(),
                choice: Choice::Command((*id).to_owned()),
                checked: false,
                dim: !selected && matches!(*id, "edit.cut" | "edit.copy"),
            })
            .collect();
        let mut icons: Vec<Option<&'static str>> =
            ids.iter().map(|(_, icon)| Some(*icon)).collect();
        let mut items: Vec<MenuItem> = items;
        let mut separators = vec![3];
        // Over a misspelt word: its corrections first, then ignore / add, as on the
        // reference's context menu.
        self.spell_menu = self.view.spell_at(x, y);
        if let Some(info) = &self.spell_menu {
            let mut top: Vec<MenuItem> = info
                .suggestions
                .iter()
                .take(5)
                .enumerate()
                .map(|(i, s)| MenuItem {
                    label: s.clone(),
                    choice: Choice::Command(format!("spell.use.{i}")),
                    checked: false,
                    dim: false,
                })
                .collect();
            if top.is_empty() {
                top.push(MenuItem {
                    label: self.strings.get("spell.noSuggestions").to_owned(),
                    choice: Choice::Command("spell.none".into()),
                    checked: false,
                    dim: true,
                });
            }
            if info.rule.is_none() {
                for key in ["spell.ignoreAll", "spell.addToDictionary"] {
                    top.push(MenuItem {
                        label: self.strings.get(key).to_owned(),
                        choice: Choice::Command(key.to_owned()),
                        checked: false,
                        dim: false,
                    });
                }
            }
            let n = top.len();
            separators = vec![n, n + 3];
            let mut tops: Vec<Option<&'static str>> = vec![None; n];
            tops.append(&mut icons);
            icons = tops;
            top.append(&mut items);
            items = top;
        }
        self.menu = Some(
            Menu::open(items, [x, y - 2.0, 1.0, 0.0], self.size, self.scale)
                .with_decor(icons, separators),
        );
    }

    /// Ids drawn as switched on beyond the formatting state.
    fn on_ids(&self) -> Vec<&'static str> {
        let mut on = Vec::new();
        if self.show_ruler {
            on.push("view.ruler");
        }
        if self.marks {
            on.push("view.formattingMarks");
        }
        if self.nav.is_some() {
            on.push("view.navigationPane");
        }
        on.push(match self.view_mode {
            "web" => "view.webLayout",
            "draft" => "view.draft",
            "read" => "view.readMode",
            _ => "view.printLayout",
        });
        if self.painter {
            on.push("format.painter");
        }
        if self.view.track_changes() {
            on.push("review.trackChanges");
        }
        if self.merge_record.is_some() {
            on.push("mailings.preview");
        }
        on.extend(doc_tools::on_ids(self.view.format(), self.story_options()));
        on
    }

    /// Paste content the host read from the system clipboard.
    pub fn paste(
        &mut self,
        internal: Option<String>,
        html: Option<String>,
        text: Option<String>,
    ) -> bool {
        let mode = self.r3_paste_mode(internal.is_some(), html.is_some());
        self.view.paste_with(internal, html, text, Some(mode))
    }

    fn clipboard(&mut self, id: &str) -> bool {
        match id {
            "edit.copy" | "edit.cut" => {
                let cut = id == "edit.cut";
                if let Some(c) = self.view.copy(cut) {
                    self.requests.push(Request::SetClipboard {
                        text: c.text,
                        html: c.html,
                        internal: c.internal,
                    });
                }
                cut
            }
            _ => {
                self.requests.push(Request::Paste);
                false
            }
        }
    }

    /// Run interface command `id` from the ribbon, title bar, a menu or a shortcut.
    fn action(&mut self, id: &str) -> bool {
        if let Some(r) = self.r6_action(id) {
            return r;
        }
        if let Some(r) = self.r5_action(id) {
            return r;
        }
        if let Some(r) = self.r3_action(id) {
            return r;
        }
        if let Some(r) = self.r4_action(id) {
            return r;
        }
        // Space before / after toggles: remove when the paragraph has some, else 12 pt.
        let toggle = match id {
            "format.spaceBefore.toggle" => Some((true, self.view.format().space_before)),
            "format.spaceAfter.toggle" => Some((false, self.view.format().space_after)),
            _ => None,
        };
        if let Some((before, current)) = toggle {
            let twips = if current > 0 { 0 } else { 240 };
            let format = if before {
                bo_edit::ParaFormat::SpaceBefore { twips }
            } else {
                bo_edit::ParaFormat::SpaceAfter { twips }
            };
            return self.view.run_command(Command::FormatParagraph { format });
        }
        if let Some(mode) = id.strip_prefix("view.layout.") {
            // The text moves: spelling marks are checked again at the next tick.
            self.spell_pending = true;
            self.spell_idle = 3;
            if mode == "read" {
                return self.enter_read_mode();
            }
            if self.read_mode.is_some() {
                self.leave_read_mode();
            }
            self.view_mode = match mode {
                "web" => "web",
                "draft" => "draft",
                _ => "print",
            };
        }
        let merge = |w: &mut Self, record: Option<usize>| {
            w.merge_record = record;
            catalog::from_json(json!({ "cmd": "mergePreview", "record": record }))
                .is_some_and(|c| w.view.run_command(c))
        };
        match id {
            "file.save" => {
                self.requests.push(Request::Save);
                true
            }
            "edit.copy" | "edit.cut" | "edit.paste" => self.clipboard(id),
            // Find searches from the navigation pane, as on the reference.
            "edit.find" => self.toggle_nav(true, true),
            "review.spellingDialog" => self.open_spelling(),
            "layout.pageSetup" => self.open_dialog(DialogKind::PageSetup),
            "edit.goTo" => self.open_find_dialog(2),
            id if id == crate::doc_table::DIALOGS[0] => {
                self.open_dialog(DialogKind::TableProperties)
            }
            id if id == crate::doc_table::DIALOGS[1] => self.open_dialog(DialogKind::CellMargins),
            "view.printLayout" => self.action("view.layout.print"),
            "view.readMode" => self.action("view.layout.read"),
            "view.webLayout" => self.action("view.layout.web"),
            "view.draft" => self.action("view.layout.draft"),
            "view.onePage" | "view.pageWidth" => {
                let z = self.view.fit_zoom(id == "view.onePage");
                self.relayout_pending = true;
                self.view.set_zoom(z)
            }
            "insert.table" => self.open_table_grid(),
            "insert.symbol" => self.open_symbol_grid(),
            "design.themes" => self.open_design_gallery("design.themes", DesignMenu::Themes),
            "design.colors" => self.open_design_gallery("design.colors", DesignMenu::Colors),
            "design.fonts" => self.open_design_gallery("design.fonts", DesignMenu::Fonts),
            "design.paragraphSpacing" => {
                self.open_design_gallery("design.paragraphSpacing", DesignMenu::Spacing)
            }
            "design.styleSetMore" => {
                self.open_design_gallery("design.styleSetMore", DesignMenu::StyleSets)
            }
            "design.pageBorders" => {
                let has = self
                    .view
                    .engine()
                    .document()
                    .is_some_and(|d| d.sections.iter().any(|s| s.page_borders.is_some()));
                self.menu = None;
                self.borders_dialog = Some(BordersDialog::new(has));
                self.relayout_pending = true;
                true
            }
            id if crate::doc_design::style_set_of(id).is_some() => {
                let set = crate::doc_design::style_set_of(id).unwrap_or_default();
                self.design_style_set = Some(set.to_owned());
                catalog::from_json(json!({ "cmd": "applyDocumentStyleSet", "id": set }))
                    .is_some_and(|c| self.view.run_command(c))
            }
            "view.navigationPane" => {
                let show = self.nav.is_none();
                self.toggle_nav(show, false)
            }
            // The pane on its Pages tab (scripted comparisons).
            "view.navigationPane.pages" => {
                self.toggle_nav(true, false);
                if let Some(nav) = self.nav.as_mut() {
                    nav.show(NavTab::Pages);
                }
                self.refresh_nav();
                true
            }
            "edit.replace" => self.open_find_dialog(1),
            "insert.link" => self.open_dialog(DialogKind::Link),
            "format.fontSettings" => self.open_dialog(DialogKind::Font),
            "format.paragraphSettings" => self.open_dialog(DialogKind::Paragraph),
            "review.newComment" => self.open_dialog(DialogKind::Comment),
            "review.wordCount" => self.open_dialog(DialogKind::WordCount),
            "review.showComments" => self.toggle_comments(),
            "review.deleteComment" => match self.view.comment_at_caret() {
                Some(id) => catalog::from_json(json!({ "cmd": "deleteComment", "id": id }))
                    .is_some_and(|c| self.view.run_command(c)),
                None => false,
            },
            "review.trackChanges" => {
                let on = !self.view.track_changes();
                self.view.run_command(Command::SetTrackChanges { on })
            }
            "view.ruler" => {
                self.show_ruler = !self.show_ruler;
                self.relayout_pending = true;
                true
            }
            "view.formattingMarks" => {
                self.marks = !self.marks;
                let on = self.marks;
                self.view.run_command(Command::ShowFormattingMarks { on });
                true
            }
            "view.zoomIn" | "view.zoomOut" | "view.zoomReset" => {
                let z = self.view.zoom();
                let z = match id {
                    "view.zoomIn" => ((z + 0.1) * 10.0).round() / 10.0,
                    "view.zoomOut" => ((z - 0.1) * 10.0).round() / 10.0,
                    _ => 1.0,
                };
                self.relayout_pending = true;
                self.view.set_zoom(z)
            }
            "format.painter" => {
                self.painter_lock = false;
                self.painter = self.view.run_command(Command::PickFormat);
                true
            }
            "format.growStep" | "format.shrinkStep" => {
                let current = self.view.format().size_half_points.unwrap_or(26);
                let half_points = step_font_size(current, id == "format.growStep");
                self.view.run_command(Command::FormatText {
                    format: CharFormat::Size { half_points },
                })
            }
            "mailings.recipients" => {
                self.requests.push(Request::PickMergeData);
                false
            }
            "mailings.preview" => {
                let record = if self.merge_record.is_some() {
                    None
                } else {
                    Some(0)
                };
                merge(self, record)
            }
            "mailings.nextRecord" => {
                let r = self.merge_record.map_or(0, |r| r + 1);
                merge(self, Some(r))
            }
            "mailings.previousRecord" => {
                let r = self.merge_record.map_or(0, |r| r.saturating_sub(1));
                merge(self, Some(r))
            }
            "mailings.finish" => self.view.run_command(Command::MergeToDocument),
            "insert.picture" => {
                self.requests.push(Request::PickPicture);
                false
            }
            id if id.starts_with("spell.") => self.spell_action(id).unwrap_or(false),
            id if catalog::is_menu(id) => {
                let anchor = self
                    .ribbon
                    .rect_of(RibbonHit::Command(commands::static_id(id)))
                    .unwrap_or([0.0, 0.0, 1.0, 1.0]);
                self.open_command_menu(id, anchor);
                true
            }
            id if crate::doc_table::spin_id(id).is_some() => {
                let sid = crate::doc_table::spin_id(id).unwrap_or(crate::doc_table::SPINS[0]);
                let Some(r) = self.ribbon.rect_of(RibbonHit::Command(sid)) else {
                    return false;
                };
                let v = crate::doc_table::spin_value(sid, self.view.format(), self.row_measured);
                match crate::doc_table::spin_press(sid, r, v, self.units, self.scale, self.pointer)
                {
                    Ok(Some(cmd)) => self.view.run_command(cmd),
                    Ok(None) => false,
                    Err(edit) => {
                        self.cell_spin = Some(edit);
                        self.cell_spin_fresh = true;
                        true
                    }
                }
            }
            id if id.starts_with("table.look.") || id.starts_with("table.opt.") => {
                match self.table_look.command(id) {
                    Some(cmd) => self.view.run_command(cmd),
                    None => false,
                }
            }
            _ => {
                let page = self.view.current_page();
                let story = self.story_options();
                let markup = self.view.markup().clone();
                if let Some(tool) = doc_tools::command(id, self.view.format(), story, page, &markup)
                {
                    return match tool {
                        Tool::Edit(cmd) => self.view.run_command(cmd),
                        Tool::Picture(v) => self.view.picture_command(&v),
                    };
                }
                if id.starts_with("insert.table.") {
                    self.activate_context = true;
                }
                match catalog::command(id, &self.view.format().section) {
                    Some(cmd) => self.view.run_command(cmd),
                    None => self.view.run_action(id),
                }
            }
        }
    }

    fn zoom_to(&mut self, zoom: f32, text: &dyn TextMeasure) -> bool {
        let changed = self.view.set_zoom(zoom);
        if changed {
            self.relayout(text);
        }
        changed
    }

    /// Zoom of a slider position: the left half covers 10–100 %, the right half 100–500 %.
    fn slider_zoom(&self, x: f32) -> f32 {
        let [sx, _, sw, _] = self.zoom_rects[1];
        let t = ((x - sx) / sw).clamp(0.0, 1.0);
        let z = if t < 0.5 {
            0.1 + t / 0.5 * 0.9
        } else {
            1.0 + (t - 0.5) / 0.5 * 4.0
        };
        // Snap to 100 % near the middle, else to whole percents.
        if (z - 1.0).abs() < 0.04 {
            1.0
        } else {
            (z * 100.0).round() / 100.0
        }
    }

    fn slider_pos(&self) -> f32 {
        let [sx, _, sw, _] = self.zoom_rects[1];
        let z = self.view.zoom();
        let t = if z <= 1.0 {
            (z - 0.1) / 0.9 * 0.5
        } else {
            0.5 + (z - 1.0) / 4.0 * 0.5
        };
        sx + t.clamp(0.0, 1.0) * sw
    }

    /// The document view.
    pub fn view(&self) -> &DocView {
        &self.view
    }

    /// The document view, for the host's file operations.
    pub fn view_mut(&mut self) -> &mut DocView {
        &mut self.view
    }

    /// Requests for the host made since the last call.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// What a window position is for the window manager (caption, window buttons, client).
    pub fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        if self.menu.is_some() {
            return NcHit::Client;
        }
        self.title.hit(x, y)
    }

    /// Width of the vertical ruler (shown with the ruler, as on a desktop word processor).
    fn vr_w(&self) -> f32 {
        if self.show_ruler {
            (16.0 * self.scale).round()
        } else {
            0.0
        }
    }

    /// Width of the vertical scroll bar (as on a desktop word processor).
    fn sb_w(&self) -> f32 {
        // Read Mode turns screens with its arrows instead of a scroll bar.
        if self.read_mode.is_some() {
            0.0
        } else {
            (15.0 * self.scale).round()
        }
    }

    /// Read Mode's page-turn arrows at mid-height of the view: (previous, next).
    fn read_arrows(&self) -> Option<[[f32; 4]; 2]> {
        self.read_mode?;
        let d = (40.0 * self.scale).round();
        let top = self.view_top();
        let bottom = self.size.1 - self.status_h();
        let y = (top.midpoint(bottom) - d / 2.0).round();
        let m = (14.0 * self.scale).round();
        Some([[m, y, d, d], [self.size.0 - m - d, y, d, d]])
    }

    /// Turn a screen in Read Mode (`forward`: the next one).
    fn read_turn(&mut self, forward: bool) -> bool {
        let (scroll, _, viewport) = self.view.scroll_info();
        let step = (viewport * 0.95).round();
        self.view.scroll_to(if forward {
            scroll + step
        } else {
            scroll - step
        })
    }

    /// Draw Read Mode's round page-turn arrows (own drawing: white disc, grey ring and
    /// chevron, as the reference's light arrows).
    fn paint_read_arrows(&self, p: &mut dyn Painter) {
        let Some(arrows) = self.read_arrows() else {
            return;
        };
        let ring = Rgba(200, 200, 200, 255);
        let ink = Rgba(150, 150, 150, 255);
        for (i, [x, y, d, _]) in arrows.into_iter().enumerate() {
            let (cx, cy, r) = (x + d / 2.0, y + d / 2.0, d / 2.0);
            // A circle as a 16-sided path.
            let mut segs = Vec::with_capacity(18);
            for k in 0..=16 {
                let a = k as f32 / 16.0 * std::f32::consts::TAU;
                let pt = (cx + r * a.cos(), cy + r * a.sin());
                segs.push(if k == 0 {
                    PathSeg::M(pt.0, pt.1)
                } else {
                    PathSeg::L(pt.0, pt.1)
                });
            }
            segs.push(PathSeg::Z);
            p.path(
                &segs,
                Some(Rgba(255, 255, 255, 255)),
                Some((ring, self.scale.max(1.0) * 1.5)),
            );
            let k = d * 0.16;
            let dir = if i == 0 { -1.0 } else { 1.0 };
            p.path(
                &[
                    PathSeg::M(cx - dir * k * 0.5, cy - k),
                    PathSeg::L(cx + dir * k * 0.5, cy),
                    PathSeg::L(cx - dir * k * 0.5, cy + k),
                ],
                None,
                Some((ink, self.scale.max(1.0) * 1.5)),
            );
        }
    }

    /// Scroll bar parts: (bar, up button, down button, thumb) as [x, y, w, h].
    fn sb_rects(&self) -> [[f32; 4]; 4] {
        let w = self.sb_w();
        let x = self.doc_right() - w;
        let top = self.view_top();
        let bottom = self.size.1 - self.status_h();
        let bar = [x, top, w, (bottom - top).max(0.0)];
        let up = [x, top, w, w];
        let down = [x, bottom - w, w, w];
        let (scroll, content, view) = self.view.scroll_info();
        let track = (bar[3] - 2.0 * w).max(0.0);
        let thumb_h = if content > 0.0 {
            (track * (view / content).min(1.0))
                .max((12.0 * self.scale).round())
                .min(track)
        } else {
            track
        };
        let room = (content - view).max(1.0);
        let thumb_y = top + w + ((track - thumb_h) * (scroll / room).clamp(0.0, 1.0)).round();
        [bar, up, down, [x, thumb_y, w, thumb_h.round()]]
    }

    /// Scroll from a press or drag on the scroll bar at window `y`; `true` when it moved.
    fn sb_press(&mut self, y: f32) -> bool {
        let [_, up, down, thumb] = self.sb_rects();
        let (scroll, _, view) = self.view.scroll_info();
        let line = (40.0 * self.scale).round();
        if y < up[1] + up[3] {
            self.view.scroll_to(scroll - line)
        } else if y >= down[1] {
            self.view.scroll_to(scroll + line)
        } else if y < thumb[1] {
            self.view.scroll_to(scroll - view)
        } else if y >= thumb[1] + thumb[3] {
            self.view.scroll_to(scroll + view)
        } else {
            self.sb_drag = Some(y - thumb[1]);
            false
        }
    }

    /// Drag the thumb so its top follows the pointer.
    fn sb_drag_to(&mut self, y: f32) -> bool {
        let Some(offset) = self.sb_drag else {
            return false;
        };
        let [bar, _, _, thumb] = self.sb_rects();
        let w = self.sb_w();
        let track = (bar[3] - 2.0 * w - thumb[3]).max(1.0);
        let (_, content, view) = self.view.scroll_info();
        let t = ((y - offset - (bar[1] + w)) / track).clamp(0.0, 1.0);
        self.view.scroll_to(t * (content - view).max(0.0))
    }

    fn status_h(&self) -> f32 {
        (24.0 * self.scale).round()
    }

    fn relayout(&mut self, text: &dyn TextMeasure) {
        // Text sizes may have changed (Windows text size): measure the status bar again.
        self.status_cache.2 = 0.0;
        self.title
            .layout(text, &self.strings, &self.name, self.size.0, self.scale);
        let top = self.title.height();
        self.ribbon
            .layout(text, &self.strings, self.size.0, self.scale, top);
        let mut top = self.chrome_bottom();
        if self.show_ruler {
            top += Ruler::height(self.scale);
        }
        let h = (self.size.1 - top - self.status_h() - self.r6_review_h()).max(1.0);
        // The vertical scroll bar takes the right edge, the vertical ruler the left one.
        let vr = self.vr_w();
        let left = self.nav_w();
        let right = self.doc_right();
        self.view.set_frame(
            left + vr,
            top,
            (right - self.sb_w() - vr - left).max(1.0),
            h,
        );
        self.r5_layout_split([
            left + vr,
            top,
            (right - self.sb_w() - vr - left).max(1.0),
            h,
        ]);
        // Read Mode: two whole pages in the window.
        // Read Mode: the text reflows to the window at 140 % (the reference's reading
        // zoom), with a margin either side; again when the window width changes.
        if self.read_mode.is_some() {
            let width = (right - self.sb_w() - vr - left).max(1.0);
            if self.read_fit || (width - self.read_w).abs() > 0.5 {
                self.read_fit = false;
                self.read_w = width;
                self.view.set_zoom(READ_ZOOM);
                let pt = (width / self.view.scale() - 2.0 * READ_MARGIN_PT).max(144.0);
                if let Some(c) = catalog::from_json(
                    json!({ "cmd": "setViewLayout", "mode": "web", "width": pt }),
                ) {
                    self.view.run_command(c);
                }
            }
        }
        let (sp_top, sp_bottom, sp_right, scale) = (
            self.chrome_bottom(),
            self.size.1 - self.status_h(),
            self.size.0,
            self.scale,
        );
        if let Some(sp) = self.spell_pane.as_mut() {
            sp.layout(sp_right, sp_top, sp_bottom, scale);
        }
        if let Some(cp) = self.comments_pane.as_mut() {
            cp.layout(sp_right, sp_top, sp_bottom, scale);
        }
        let (nav_top, nav_bottom) = (self.chrome_bottom(), self.size.1 - self.status_h());
        if let Some(nav) = self.nav.as_mut() {
            nav.layout(nav_top, nav_bottom, self.scale);
        }
        self.layout_ruler();
        if let Some(d) = self.dialog.as_mut() {
            d.layout(text, self.size, self.scale);
        }
        if let Some(b) = self.backstage.as_mut() {
            b.layout(self.title.height(), self.size, self.scale);
        }
        if self.styles.is_empty() {
            self.styles = self.view.styles();
        }
        let tag = self.tag_style();
        self.tag_w = [
            text.measure(&self.t("hf.header", "Header"), tag).0,
            text.measure(&self.t("hf.footer", "Footer"), tag).0,
        ];
        self.ribbon.measure_previews(text, &self.styles);
        // Status bar zoom controls, right to left: percentage, plus, slider, minus.
        let px = |v: f32| (v * self.scale).round();
        let (w, sh) = (self.size.0, self.status_h());
        let y = self.size.1 - sh;
        let pct = [w - px(56.0), y, px(48.0), sh];
        let plus = [pct[0] - px(24.0), y, px(22.0), sh];
        let track = [plus[0] - px(108.0), y, px(104.0), sh];
        let minus = [track[0] - px(24.0), y, px(22.0), sh];
        self.zoom_rects = [minus, track, plus, pct];
        // View buttons before the zoom controls.
        let bw = px(32.0);
        let gap = px(2.0);
        let right = minus[0] - px(12.0);
        for (i, r) in self.view_rects.iter_mut().enumerate() {
            let x = right - (3 - i) as f32 * (bw + gap);
            *r = [x, y + 1.0, bw, sh - 1.0];
        }
    }

    /// Bottom of the title bar and ribbon (the title bar alone in Read Mode).
    fn chrome_bottom(&self) -> f32 {
        // Read Mode and View › Focus show no ribbon.
        if self.read_mode.is_some() || self.r4.focus {
            self.title.height()
        } else {
            self.ribbon.bottom()
        }
    }

    /// Read Mode as on the reference: no ribbon or rulers, two pages side by side
    /// fitted to the window; Esc or another view button ends it.
    fn enter_read_mode(&mut self) -> bool {
        if self.read_mode.is_some() {
            return false;
        }
        self.read_mode = Some((self.view.zoom(), self.show_ruler));
        // No proofing marks while reading, as the reference; white screen pages.
        self.view.set_hide_spelling(true);
        self.view.set_plain(true);
        self.view_mode = "read";
        self.show_ruler = false;
        self.menu = None;
        self.read_fit = true;
        self.relayout_pending = true;
        true
    }

    fn leave_read_mode(&mut self) {
        if let Some((zoom, ruler)) = self.read_mode.take() {
            if let Some(c) = catalog::from_json(json!({ "cmd": "setViewLayout", "mode": "print" }))
            {
                self.view.run_command(c);
            }
            self.view.set_zoom(zoom);
            self.view.set_hide_spelling(false);
            self.view.set_plain(false);
            self.show_ruler = ruler;
            self.view_mode = "print";
            self.relayout_pending = true;
            self.spell_pending = true;
            self.spell_idle = 3;
        }
    }

    fn layout_ruler(&mut self) {
        if let Some(page) = self.view.ruler_page() {
            self.ruler.layout(
                self.ribbon.bottom(),
                self.size.0,
                self.scale,
                page,
                self.view.scale(),
                self.view.format(),
            );
        }
    }

    fn zoom_hit(&self, x: f32, y: f32) -> Option<usize> {
        self.zoom_rects
            .iter()
            .position(|[rx, ry, rw, rh]| x >= *rx && x < rx + rw && y >= *ry && y < ry + rh)
    }

    /// Handle an input event; `true` when a new frame is needed.
    pub fn handle(&mut self, input: Input, text: &dyn TextMeasure) -> bool {
        let redraw = self.route(input, text);
        self.sync_context();
        if self.relayout_pending {
            self.relayout_pending = false;
            self.relayout(text);
        } else if self.show_ruler {
            self.layout_ruler();
        }
        self.r3_layout(text);
        self.r5_layout();
        if let Some(d) = self.dialog.as_mut() {
            d.layout(text, self.size, self.scale);
        }
        self.layout_status(text);
        if let Some(sp) = self.spell_pane.as_mut() {
            sp.measure(text);
        }
        if let Some(g) = self.symbol_grid.as_mut() {
            g.measure(text);
        }
        if let Some(fd) = self.find_dialog.as_mut() {
            fd.layout(text, &self.strings, self.size, self.scale);
        }
        if let Some(bd) = self.borders_dialog.as_mut() {
            bd.layout(text, &self.strings, self.size, self.scale);
        }
        if let Some(g) = self.design_gallery.as_mut() {
            g.measure(text);
        }
        if let Some(cp) = self.comments_pane.as_mut() {
            let caret = self.view.comment_at_caret();
            cp.measure(self.view.comments(), caret, &self.strings, text);
        }
        // The Pages tab of the navigation pane shows thumbnails of the pages in view there.
        let thumbs = match &self.nav {
            Some(nav) => {
                let rects = nav.thumb_rects();
                let w = nav.thumb_size().0;
                (rects.into_iter().map(|(i, _)| i).collect(), w)
            }
            None => (Vec::new(), 0.0),
        };
        self.view.set_thumbnails(thumbs.0, thumbs.1);
        self.r6_layout(text);
        if let Some(e) = &self.cell_spin {
            let style = TextStyle {
                size: (12.0 * self.scale).round(),
                bold: false,
            };
            self.cell_spin_w = text.measure(&e.text, style).0;
        }
        redraw
    }

    /// Keys and text for a Cell Size box being typed in; `None` lets the input through.
    fn cell_spin_input(&mut self, input: &Input) -> Option<bool> {
        let edit = self.cell_spin.as_mut()?;
        match input {
            Input::Text(s) => {
                if std::mem::take(&mut self.cell_spin_fresh) {
                    edit.text.clear();
                }
                edit.type_text(s);
                Some(true)
            }
            Input::Key(k, _) => {
                self.cell_spin_fresh = false;
                let action = edit.key(*k);
                let (id, typed) = (edit.id, edit.text.clone());
                match action {
                    crate::spin_box::SpinKey::Commit => {
                        self.cell_spin = None;
                        if let Some(c) = crate::doc_table::spin_parse(id, self.units, &typed)
                            .and_then(|tw| crate::doc_table::spin_command(id, tw))
                        {
                            self.view.run_command(c);
                        }
                    }
                    crate::spin_box::SpinKey::Cancel => self.cell_spin = None,
                    crate::spin_box::SpinKey::Up | crate::spin_box::SpinKey::Down => {
                        let up = action == crate::spin_box::SpinKey::Up;
                        let units = self.units;
                        if let Some(c) = crate::doc_table::spin_step(id, &typed, up, units) {
                            self.view.run_command(c);
                            let v = crate::doc_table::spin_value(id, self.view.format(), None);
                            if let Some(e) = self.cell_spin.as_mut() {
                                e.text = crate::doc_table::spin_number(id, units, v);
                            }
                        }
                    }
                    crate::spin_box::SpinKey::Edited | crate::spin_box::SpinKey::Ignored => {}
                }
                Some(true)
            }
            // Clicking elsewhere leaves the box (the typed value is kept, as on the
            // reference) and the click goes on.
            Input::Press { .. } => {
                let (id, typed) = (edit.id, edit.text.clone());
                self.cell_spin = None;
                if let Some(c) = crate::doc_table::spin_parse(id, self.units, &typed)
                    .and_then(|tw| crate::doc_table::spin_command(id, tw))
                {
                    self.view.run_command(c);
                }
                None
            }
            _ => None,
        }
    }

    fn route(&mut self, input: Input, text: &dyn TextMeasure) -> bool {
        if let Some(r) = self.r6_input(&input, text) {
            return r;
        }
        // The File screen covers everything below the title bar.
        if let Some(b) = self.backstage.as_mut() {
            match input {
                Input::Move { x, y } if y >= self.title.height() => return b.hover(x, y),
                Input::Press { x, y, .. } if y >= self.title.height() => {
                    return match b.press(x, y) {
                        Some(a) => self.file_action(a),
                        None => false,
                    };
                }
                Input::Key(Key::Escape, _) => {
                    self.backstage = None;
                    return true;
                }
                Input::Text(_)
                | Input::Key(..)
                | Input::Wheel(_)
                | Input::Context { .. }
                | Input::Preedit(_) => {
                    return false;
                }
                _ => {}
            }
        }
        if let Input::Press { x, y, .. } = input {
            self.pointer = (x, y);
        }
        if let Some(redraw) = self.r3_input(&input, text) {
            return redraw;
        }
        if let Some(redraw) = self.r5_input(&input) {
            return redraw;
        }
        if let Some(redraw) = self.cell_spin_input(&input) {
            return redraw;
        }
        // So is the Page Borders dialog.
        if let Some(bd) = self.borders_dialog.as_mut() {
            let a = match input {
                Input::Press { x, y, .. } => bd.press(x, y),
                Input::Key(k, m) => bd.key(k, m),
                Input::Move { .. }
                | Input::Blink
                | Input::Text(_)
                | Input::Wheel(_)
                | Input::Drag { .. }
                | Input::Release { .. }
                | Input::Context { .. } => return false,
                _ => BordersAction::None,
            };
            match a {
                BordersAction::None => {}
                BordersAction::Redraw => return true,
                BordersAction::Close => {
                    self.borders_dialog = None;
                    return true;
                }
                BordersAction::Apply(v) => {
                    self.borders_dialog = None;
                    if let Some(c) = catalog::from_json(v) {
                        self.view.run_command(c);
                    }
                    return true;
                }
            }
        }
        // The Find and Replace dialog is modal too.
        if let Some(fd) = self.find_dialog.as_mut() {
            let a = match input {
                Input::Text(ref s) => {
                    fd.text(s);
                    FindAction::Redraw
                }
                Input::Key(k, m) => fd.key(k, m),
                Input::Move { x, y } => return fd.hover(x, y),
                Input::Press { x, y, .. } => fd.press(x, y),
                Input::Blink => return true,
                Input::Wheel(dy) => return fd.wheel(dy),
                Input::Drag { .. } | Input::Release { .. } | Input::Context { .. } => return false,
                _ => FindAction::None,
            };
            if a != FindAction::None {
                return self.find_dialog_action(a);
            }
            if !matches!(input, Input::Resize { .. } | Input::Scale(_) | Input::Tick) {
                return false;
            }
        }
        // A dialog is modal: it takes keyboard and pointer input.
        if let Some(d) = self.dialog.as_mut() {
            match input {
                Input::Text(ref s) => {
                    d.text(s);
                    return true;
                }
                Input::Key(k, m) => {
                    if let Some(b) = d.key(k, m) {
                        self.dialog_button(b);
                    }
                    self.sync_find_tabs();
                    return true;
                }
                Input::Move { x, y } => return d.hover(x, y),
                Input::Press { x, y, .. } => {
                    if let Some(b) = d.press(x, y) {
                        self.dialog_button(b);
                    }
                    self.sync_find_tabs();
                    return true;
                }
                Input::Blink => return true,
                Input::Wheel(_)
                | Input::Drag { .. }
                | Input::Release { .. }
                | Input::Context { .. } => {
                    return false;
                }
                _ => {}
            }
        }
        if let Some(redraw) = self.table_grid_input(&input) {
            return redraw;
        }
        if let Some(redraw) = self.symbol_grid_input(&input) {
            return redraw;
        }
        if let Some(redraw) = self.design_gallery_input(&input) {
            return redraw;
        }
        // Pointer and keyboard input goes to an open menu first.
        // The ▾ column of the Design tab's style set gallery lists the sets.
        if let Input::Press { x, y, .. } = input {
            let more = crate::doc_design::more_rect(&self.ribbon, self.scale)
                .filter(|_| self.menu.is_none() && self.design_gallery.is_none());
            if more.is_some_and(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]) {
                return self.open_design_gallery("design.styleSet.bold", DesignMenu::StyleSets);
            }
        }
        if self.menu.is_some() {
            if let Some(redraw) = self.menu_input(&input) {
                return redraw;
            }
        }
        if let Input::Move { x, y } | Input::Drag { x, y } = input {
            self.pointer = (x, y);
        }
        // Spelling is checked again once typing rests for 400 ms (4 ticks).
        match input {
            Input::Text(_) | Input::Key(..) | Input::Release { .. } => {
                self.spell_pending = true;
                self.spell_idle = 0;
                self.row_measure_due = true;
            }
            Input::Tick if self.spell_pending => {
                self.spell_idle += 1;
                if self.spell_idle >= 4 {
                    self.spell_pending = false;
                    if self.view.refresh_spelling() {
                        return true;
                    }
                }
            }
            // The Cell Size box shows the caret row's laid-out height, measured at rest.
            Input::Tick if self.row_measure_due && self.context.table => {
                self.row_measure_due = false;
                let m = self.view.caret_row_height();
                if m != self.row_measured {
                    self.row_measured = m;
                    return true;
                }
            }
            // Thumbnails of edited pages are redrawn at rest.
            Input::Tick if self.nav.is_some() && self.view.refresh_thumbnails() => {
                return true;
            }
            // Pages scrolled into view are checked at the next tick.
            Input::Tick if self.view.spelling_out_of_view() => {
                if self.view.refresh_spelling() {
                    return true;
                }
            }
            // While typing rests, the rest of the document a few milliseconds per tick
            // (the status bar's proofing icon covers the whole document).
            Input::Tick
                if self.view.sweeping()
                    && self.view.spell_sweep(std::time::Duration::from_millis(3)) =>
            {
                return true;
            }
            _ => {}
        }
        if let Some(sp) = self.spell_pane.as_mut() {
            match input {
                Input::Move { x, y } if sp.contains(x, y) => return sp.hover(x, y),
                Input::Move { .. } => {
                    sp.hover(-1.0, -1.0);
                }
                Input::Press { x, y, .. } if sp.contains(x, y) => {
                    let a = sp.press(x, y);
                    return self.spell_pane_action(a);
                }
                _ => {}
            }
        }
        if let Some(cp) = self.comments_pane.as_mut() {
            match input {
                Input::Move { x, y } if cp.contains(x, y) => return cp.hover(x, y),
                Input::Move { .. } => {
                    cp.hover(-1.0, -1.0);
                }
                Input::Press { x, y, .. } if cp.contains(x, y) => {
                    let a = cp.press(x, y);
                    return self.comments_action(a);
                }
                Input::Press { .. } => cp.selected = None,
                // The release and drags of a press in the pane stay in the pane (the
                // document must not take them as the end of a selection).
                Input::Release { x, y } | Input::Drag { x, y } if cp.contains(x, y) => {
                    return false;
                }
                Input::Wheel(dy) if cp.contains(self.pointer.0, self.pointer.1) => {
                    return cp.wheel(-dy * self.scale);
                }
                _ => {}
            }
        }
        if let Input::Key(Key::F(7), _) = input {
            return self.open_spelling();
        }
        if self.nav.is_some() {
            if matches!(
                input,
                Input::Text(_) | Input::Key(..) | Input::Release { .. }
            ) {
                self.nav_dirty = true;
            }
            if let Input::Tick = input {
                if self.nav_dirty {
                    self.refresh_nav();
                    return true;
                }
            }
            if let Some(redraw) = self.nav_input(&input) {
                return redraw;
            }
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
        let in_title = |w: &Self, y: f32| y < w.title.height();
        let in_ribbon = |w: &Self, y: f32| {
            w.read_mode.is_none() && !w.r4.focus && y >= w.ribbon.top() && y < w.ribbon.bottom()
        };
        if let Input::Key(
            Key::Letter(c @ ('s' | 'c' | 'x' | 'v' | 'f' | 'h' | 'k' | 'g')),
            Mods {
                ctrl: true, shift, ..
            },
        ) = input
        {
            let id = match (c, shift) {
                ('s', _) => "file.save",
                ('c', true) => "format.painter",
                ('c', _) => "edit.copy",
                ('x', _) => "edit.cut",
                ('v', _) => "edit.paste",
                ('f', _) => "edit.find",
                ('h', _) => "edit.replace",
                ('g', _) => "edit.goTo",
                _ => "insert.link",
            };
            return self.action(id);
        }
        // Read Mode: the arrows and the left / right keys turn screens.
        if self.read_mode.is_some() {
            match input {
                Input::Press { x, y, .. } => {
                    if let Some(arrows) = self.read_arrows() {
                        for (i, r) in arrows.iter().enumerate() {
                            if x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3] {
                                return self.read_turn(i == 1);
                            }
                        }
                    }
                }
                Input::Key(Key::Right | Key::PageDown, _) => return self.read_turn(true),
                Input::Key(Key::Left | Key::PageUp, _) => return self.read_turn(false),
                _ => {}
            }
        }
        if let Input::Key(Key::Escape, _) = input {
            if self.read_mode.is_some() {
                self.leave_read_mode();
                return true;
            }
            if self.r4.focus {
                return self.action("view.focus");
            }
            if self.story_options().is_some() && !self.painter {
                return self.view.run_command(Command::CloseHeaderFooter);
            }
            if self.painter {
                self.painter = false;
                self.painter_lock = false;
                if let Some(c) = catalog::from_json(json!({ "cmd": "cancelFormat" })) {
                    self.view.run_command(c);
                }
                return true;
            }
        }
        if let Input::Context { x, y } = input {
            if y >= self.view_top() && y < self.size.1 - self.status_h() {
                self.context_menu(x, y);
                return true;
            }
            return false;
        }
        // Ruler: drag indent markers.
        if self.show_ruler {
            match input {
                Input::Press { x, y, .. } if self.ruler.contains_y(y) => {
                    self.ruler_drag = self.ruler.press(x, y);
                    return self.ruler_drag;
                }
                Input::Drag { x, .. } if self.ruler_drag => return self.ruler.drag(x),
                Input::Release { .. } if self.ruler_drag => {
                    self.ruler_drag = false;
                    if let Some((left, right, first_line)) = self.ruler.release(self.view.format())
                    {
                        let cmd = catalog::from_json(json!({
                            "cmd": "formatParagraph",
                            "format": { "kind": "indents", "left": left, "right": right, "firstLine": first_line },
                        }));
                        if let Some(c) = cmd {
                            self.view.run_command(c);
                        }
                    }
                    return true;
                }
                _ => {}
            }
        }
        // Drag-selection auto-scroll continues while the pointer rests past the view.
        if let Input::Tick = input {
            if let (true, Some((x, y))) = (self.doc_press, self.last_drag) {
                let top = self.view_top();
                let bottom = self.size.1 - self.status_h();
                if y < top || y > bottom {
                    self.view.handle(Input::Drag { x, y });
                    return true;
                }
            }
        }
        if let Input::Drag { x, y } = input {
            if self.doc_press {
                self.last_drag = Some((x, y));
            }
        }
        if let Input::Zoom(steps) = input {
            let z = ((self.view.zoom() + 0.1 * steps) * 10.0).round() / 10.0;
            return self.zoom_to(z, text);
        }
        let status_top = self.size.1 - self.status_h();
        // Vertical scroll bar on the right edge of the document area.
        match input {
            Input::Press { x, y, .. }
                if x >= self.doc_right() - self.sb_w()
                    && x < self.doc_right()
                    && y >= self.view_top()
                    && y < status_top
                    && self.menu.is_none()
                    && self.backstage.is_none() =>
            {
                return self.sb_press(y);
            }
            Input::Drag { y, .. } if self.sb_drag.is_some() => return self.sb_drag_to(y),
            Input::Release { .. } if self.sb_drag.is_some() => {
                self.sb_drag = None;
                return false;
            }
            _ => {}
        }
        match input {
            Input::Press { x, y, .. }
                if y >= status_top
                    && x >= self.status_x[1] - 4.0
                    && x < self.status_x[1] + (20.0 * self.scale).round() =>
            {
                return self.open_spelling();
            }
            Input::Press { x, y, .. } if y >= status_top => {
                if let Some(i) = self
                    .view_rects
                    .iter()
                    .position(|[rx, ry, rw, rh]| x >= *rx && x < rx + rw && y >= *ry && y < ry + rh)
                {
                    let id = format!("view.layout.{}", VIEW_MODES[i].0);
                    return self.action(&id);
                }
                return match self.zoom_hit(x, y) {
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
                self.view.handle(Input::Scale(scale));
                self.relayout(text);
                true
            }
            Input::Tick => self.tick(text),
            Input::Zoom(_) | Input::Context { .. } => false,
            Input::Maximized(m) => self.title.set_maximized(m),
            // Only the hover goes: a press keeps its control until the release, so a click
            // posted to a window the pointer is not over (measurement scripts) still runs.
            Input::Leave => {
                let a = self.title.hover(-1.0, -1.0);
                self.ribbon.hover(-1.0e6, -1.0e6) || a
            }
            Input::Move { x, y } => {
                let a = self.title.hover(x, y);
                self.ribbon.hover(x, y) || a
            }
            Input::Press { x, y, .. } if in_title(self, y) => {
                self.doc_press = false;
                self.title.press(x, y)
            }
            Input::Press { x, y, count, .. } if in_ribbon(self, y) => {
                self.doc_press = false;
                // A double click on the format painter keeps it on, as on the reference.
                if count == 2 && self.ribbon.hit(x, y) == Some(RibbonHit::Command("format.painter"))
                {
                    self.painter_lock = true;
                    if !self.painter {
                        self.painter = self.view.run_command(Command::PickFormat);
                    }
                    return true;
                }
                self.ribbon.press(x, y)
            }
            Input::Press { x, y, count, .. } => {
                self.doc_press = true;
                let cleared = self.ribbon.clear_pointer();
                // A double click in the top or bottom margin opens that page's header or
                // footer, as on a desktop word processor.
                if count == 2 && self.view.story().is_none() {
                    if let Some((page, header)) = self.view.hf_zone(x, y) {
                        let cmd = catalog::from_json(json!({
                            "cmd": "editHeaderFooter", "page": page, "header": header,
                        }));
                        if let Some(c) = cmd {
                            self.doc_press = false;
                            return self.view.run_command(c) || cleared;
                        }
                    }
                }
                self.view.handle(input) || cleared
            }
            Input::Drag { .. } if !self.doc_press => false,
            Input::Release { x, y } => {
                // The format painter applies to the selection just made in the document.
                if self.doc_press && self.painter {
                    self.painter = false;
                    self.view.run_command(Command::ApplyFormat);
                    // Locked by a double click: pick the format again for the next one.
                    if self.painter_lock {
                        self.painter = self.view.run_command(Command::PickFormat);
                    }
                }
                self.doc_press = false;
                self.last_drag = None;
                let quick = match self.title.release(x, y) {
                    Some(id) => self.action(id),
                    None => false,
                };
                let (fired, changed) = self.ribbon.release(x, y);
                let ran = match fired {
                    Some(RibbonHit::File) => {
                        self.r6_open_backstage(crate::doc_backstage6::BsPage::Info)
                    }
                    Some(RibbonHit::Command(id)) => self.action(id),
                    Some(RibbonHit::Style(i)) => match self.styles.get(i) {
                        Some(st) => {
                            let id = st.id.clone();
                            self.view.run_command(Command::ApplyStyle { id })
                        }
                        None => false,
                    },
                    Some(RibbonHit::StylesMore) => {
                        self.open_styles_menu();
                        true
                    }
                    Some(hit @ (RibbonHit::FontBox | RibbonHit::SizeBox)) => {
                        self.open_menu(hit);
                        true
                    }
                    _ => false,
                };
                if self.ribbon.needs_layout() {
                    self.relayout(text);
                }
                quick || ran || changed || fired.is_some()
            }
            Input::Wheel(_)
            | Input::Text(_)
            | Input::Key(..)
            | Input::Blink
            | Input::Preedit(_)
            | Input::Drag { .. } => self.view.handle(input),
        };
        redraw || tip_before != self.tooltip.is_some()
    }

    /// Input while a menu is open; `None` lets the event through to the window.
    fn menu_input(&mut self, input: &Input) -> Option<bool> {
        let menu = self.menu.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => menu.hover(x, y),
            Input::Press { x, y, .. } => {
                if menu.contains(x, y) {
                    menu.press(x, y);
                } else {
                    // A click outside closes the menu and is not passed on.
                    self.menu = None;
                    self.ribbon.clear_pointer();
                }
                true
            }
            Input::Release { x, y } => {
                if let Some(choice) = menu.release(x, y) {
                    self.menu = None;
                    self.choose(choice);
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
                    self.choose(c);
                }
                true
            }
            Input::Text(_) | Input::Key(..) => true,
            _ => return None,
        })
    }

    fn open_menu(&mut self, hit: RibbonHit) {
        let Some(anchor) = self.ribbon.rect_of(hit) else {
            return;
        };
        let f = self.view.format().clone();
        let items: Vec<MenuItem> = if hit == RibbonHit::FontBox {
            self.view
                .font_list()
                .into_iter()
                .map(|c| MenuItem {
                    checked: f.font.as_deref() == Some(c.name.as_str()),
                    dim: !c.available && c.substitute.is_none(),
                    label: c.name.clone(),
                    choice: Choice::Font(c.name),
                })
                .collect()
        } else {
            SIZES
                .iter()
                .map(|pt| {
                    let half = (pt * 2.0).round() as u32;
                    MenuItem {
                        label: if half & 1 == 0 {
                            (half / 2).to_string()
                        } else {
                            format!("{},5", half / 2)
                        },
                        choice: Choice::Size(half),
                        checked: f.size_half_points == Some(half),
                        dim: false,
                    }
                })
                .collect()
        };
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    fn open_styles_menu(&mut self) {
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::StylesMore) else {
            return;
        };
        let current = self.view.format().style.clone();
        let items = self
            .styles
            .iter()
            .map(|st| MenuItem {
                label: commands::style_name_key(&st.name)
                    .map_or_else(|| st.name.clone(), |k| self.strings.get(k).to_owned()),
                choice: Choice::Style(st.id.clone()),
                checked: current.as_deref() == Some(st.id.as_str()),
                dim: false,
            })
            .collect();
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    fn choose(&mut self, choice: Choice) {
        let format = match choice {
            Choice::Font(name) => CharFormat::Font { name },
            Choice::Size(half_points) => CharFormat::Size { half_points },
            Choice::Style(id) => {
                self.view.run_command(Command::ApplyStyle { id });
                return;
            }
            Choice::Command(id) => {
                self.action(&id);
                return;
            }
        };
        self.view.run_command(Command::FormatText { format });
    }

    fn tick(&mut self, text: &dyn TextMeasure) -> bool {
        let Some(RibbonHit::Command(id)) = self.ribbon.hovered() else {
            return false;
        };
        self.rest += 1;
        // File › Options › ScreenTip style: none.
        if self.rest != TOOLTIP_TICKS || self.menu.is_some() || self.r6_screentips() == 2 {
            return false;
        }
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return false;
        };
        let name = self.strings.get(&format!("commands.{id}")).to_owned();
        let label = match commands::shortcut(id) {
            Some(keys) => format!("{name} ({keys})"),
            None => name,
        };
        let style = TextStyle {
            size: (12.0 * self.scale).round(),
            bold: false,
        };
        let (w, _) = text.measure(&label, style);
        let desc =
            crate::doc_tips::description(&self.strings, id).filter(|_| self.r6_screentips() == 0);
        self.tip = Some(crate::doc_tips::layout(
            &label,
            desc.as_deref(),
            text,
            self.scale,
        ));
        self.tooltip = Some((label, w, anchor));
        true
    }

    /// Whether an animation needs more frames.
    pub fn animating(&self) -> bool {
        self.view.animating() || self.view.thumbnails_pending()
    }

    /// Advance animations to the host clock (ms); `true` while frames are still needed.
    pub fn advance(&mut self, now_ms: f64) -> bool {
        self.view.advance(now_ms)
    }

    /// Forget page images when the host changes how document text is rendered.
    pub fn invalidate_page_images(&mut self) {
        self.view.invalidate_page_images();
    }

    /// Update page images of the visible pages.
    pub fn prepare(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        self.view.prepare(store)
    }

    /// Paint the whole window.
    pub fn paint(&self, p: &mut dyn Painter) {
        p.clear(self.theme.bg);
        self.view.paint(p);
        self.r4_paint_grid(p);
        self.paint_read_arrows(p);
        self.paint_story_frame(p);
        // Title bar and ribbon: one cached layer, redrawn only when what it shows changes
        // (typing changes the document, not the ribbon).
        let chrome_h = self.chrome_bottom();
        let on = self.on_ids();
        Self::layered(p, 0, [0.0, 0.0, self.size.0, chrome_h], &mut |p| {
            p.fill_rect(0.0, 0.0, self.size.0, chrome_h, self.theme.bg);
            self.title.paint(
                p,
                &self.theme,
                &self.icons,
                &self.strings,
                &self.name,
                self.view.modified(),
                self.view.format(),
            );
            if self.read_mode.is_none() && !self.r4.focus {
                self.ribbon.paint(
                    p,
                    &self.theme,
                    &self.icons,
                    &self.strings,
                    self.view.format(),
                    &self.styles,
                    &on,
                );
            }
            if let Some((first, odd_even, _)) = self.story_options() {
                crate::doc_table::paint_checks(
                    p,
                    &self.ribbon,
                    &[
                        (crate::doc_table::HF_CHECKS[0], first),
                        (crate::doc_table::HF_CHECKS[1], odd_even),
                    ],
                    &self.theme,
                    &self.strings,
                    self.scale,
                );
            }
            {
                let caret = self.cell_spin_w;
                crate::doc_table::paint_para_spins(
                    p,
                    &self.ribbon,
                    self.view.format(),
                    self.units,
                    self.cell_spin.as_ref(),
                    (&self.theme, &self.icons, &self.strings),
                    &|_| caret,
                    self.scale,
                );
            }
            // View › Show: check boxes as the reference (round 4).
            crate::doc_table::paint_checks(
                p,
                &self.ribbon,
                &[
                    ("view.ruler", self.show_ruler),
                    ("view.gridlines", self.r4.gridlines),
                    ("view.navigationPane", self.nav.is_some()),
                ],
                &self.theme,
                &self.strings,
                self.scale,
            );
            if self.context.table {
                let caret = self.cell_spin_w;
                crate::doc_table::paint_spins(
                    p,
                    &self.ribbon,
                    self.view.format(),
                    (self.units, self.row_measured),
                    self.cell_spin.as_ref(),
                    &self.theme,
                    &|_| caret,
                    self.scale,
                );
                crate::doc_table::paint(
                    p,
                    &self.ribbon,
                    &self.table_look,
                    &self.theme,
                    &self.strings,
                    self.scale,
                );
            }
            // Design › Document Formatting: the style set gallery in the theme's faces.
            if crate::doc_design::more_rect(&self.ribbon, self.scale).is_some() {
                let (major, minor) = self
                    .view
                    .engine()
                    .document()
                    .map(|d| {
                        (
                            d.theme_fonts.major_latin.clone(),
                            d.theme_fonts.minor_latin.clone(),
                        )
                    })
                    .unwrap_or_default();
                let look = crate::doc_design::PreviewLook::new(
                    self.design_theme.as_deref(),
                    &major,
                    &minor,
                );
                crate::doc_design::paint(
                    p,
                    &self.ribbon,
                    self.design_style_set.as_deref(),
                    &look,
                    &self.theme,
                    &self.strings,
                    self.scale,
                );
            }
        });
        if self.show_ruler {
            self.ruler.paint(p, &self.theme);
            if let Some(page) = self.view.ruler_page_v() {
                let top = self.view_top();
                let bottom = self.size.1 - self.status_h();
                crate::ruler::paint_vertical(
                    p,
                    &self.theme,
                    (self.nav_w(), self.vr_w(), top, bottom),
                    page,
                    &self.view.format().section,
                    self.view.scale() / 20.0,
                    self.scale,
                );
            }
        }
        if let Some(nav) = &self.nav {
            nav.paint(p, &self.theme, &self.strings);
        }
        if let Some(sp) = &self.spell_pane {
            sp.paint(p, &self.theme, &self.strings);
        }
        if let Some(cp) = &self.comments_pane {
            cp.paint(p, &self.theme, &self.strings, self.view.comment_at_caret());
        }
        self.r3_paint_panes(p);
        self.r5_paint(p);
        self.r6_paint_panes(p);
        // Status bar.
        let (w, h) = self.size;
        let sh = self.status_h();
        let y = h - sh;
        // Vertical scroll bar: light track, white buttons and thumb framed in grey, dark
        // arrows (measured on a desktop word processor: 243 / 255 / 148 / 38).
        let [bar, up, down, thumb] = self.sb_rects();
        if bar[3] > 0.0 && bar[2] > 0.0 {
            let edge = Rgba(0x94, 0x94, 0x94, 255);
            p.fill_rect(bar[0], bar[1], bar[2], bar[3], Rgba(0xf3, 0xf3, 0xf3, 255));
            let boxed = |p: &mut dyn Painter, r: [f32; 4]| {
                p.fill_rect(r[0], r[1], r[2], r[3], edge);
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    self.theme.surface,
                );
            };
            boxed(p, up);
            boxed(p, down);
            if thumb[3] > 2.0 {
                boxed(p, thumb);
            }
            let k = (3.0 * self.scale).round();
            let arrow = Rgba(0x26, 0x26, 0x26, 255);
            let (cx, cy) = (up[0] + up[2] / 2.0, up[1] + up[3] / 2.0);
            p.path(
                &[
                    PathSeg::M(cx - k, cy + k / 2.0),
                    PathSeg::L(cx + k, cy + k / 2.0),
                    PathSeg::L(cx, cy - k / 2.0 - 1.0),
                    PathSeg::Z,
                ],
                Some(arrow),
                None,
            );
            let (cx, cy) = (down[0] + down[2] / 2.0, down[1] + down[3] / 2.0);
            p.path(
                &[
                    PathSeg::M(cx - k, cy - k / 2.0),
                    PathSeg::L(cx + k, cy - k / 2.0),
                    PathSeg::L(cx, cy + k / 2.0 + 1.0),
                    PathSeg::Z,
                ],
                Some(arrow),
                None,
            );
        }
        Self::layered(p, 1, [0.0, y, w, sh], &mut |p| self.paint_status(p));
        if let Some(grid) = &self.symbol_grid {
            grid.paint(p, &self.theme);
        }
        if let Some(g) = &self.design_gallery {
            g.paint(p, &self.theme);
        }
        if let Some(grid) = &self.table_grid {
            let label = |c: usize, r: usize| {
                self.strings.fmt(
                    "ribbon.docTools.tableSize",
                    &[("cols", &c.to_string()), ("rows", &r.to_string())],
                )
            };
            grid.paint(
                p,
                &self.theme,
                &self.t("ribbon.docTools.insertTable", "Insert Table"),
                &label,
            );
        }
        if let Some(menu) = &self.menu {
            menu.paint(p, &self.theme, &self.icons);
        } else if let Some((label, tw, anchor)) = &self.tooltip {
            match &self.tip {
                Some(tip) if tip.title == *label => {
                    crate::doc_tips::paint(p, &self.theme, tip, *anchor, w, self.scale);
                }
                _ => paint_tooltip(p, &self.theme, label, *tw, *anchor, w, self.scale),
            }
        }
        if let Some(b) = &self.backstage {
            b.paint(
                p,
                &self.theme,
                &self.strings,
                &self.name,
                self.view.page_count(),
                self.view.word_count(),
                self.view.modified(),
            );
            b.paint_icons(p, &self.theme, &self.icons);
        }
        if let Some(d) = &self.dialog {
            d.paint(p, &self.theme, self.size, true);
        }
        if let Some(fd) = &self.find_dialog {
            fd.paint(p, &self.theme, true);
        }
        if let Some(bd) = &self.borders_dialog {
            bd.paint(p, &self.theme);
        }
        self.r3_paint_popups(p);
        self.r6_paint(p);
    }

    /// Paint `paint` through cached layer `id` over `rect` (window pixels): the backend
    /// keeps its image and draws it again while the drawing it records hashes the same.
    fn layered(
        p: &mut dyn Painter,
        id: usize,
        rect: [f32; 4],
        paint: &mut dyn FnMut(&mut dyn Painter),
    ) {
        let mut hash = crate::PaintHash::default();
        paint(&mut hash);
        if !p.layer_begin(id, rect, hash.finish()) {
            paint(p);
            p.layer_end(id);
        }
    }

    /// The status bar: page, words, proofing state, language, view buttons and zoom.
    fn paint_status(&self, p: &mut dyn Painter) {
        let w = self.size.0;
        let sh = self.status_h();
        let y = self.size.1 - sh;
        // Neutral grey bar with dark text, like the ribbon body (and a desktop word
        // processor's status bar).
        p.fill_rect(0.0, y, w, sh, self.theme.ribbon);
        p.fill_rect(0.0, y, w, 1.0, self.theme.border);
        for (i, r) in self.view_rects.iter().enumerate() {
            let (mode, icon) = VIEW_MODES[i];
            if mode == self.view_mode {
                p.fill_rect(r[0], r[1], r[2], r[3], self.theme.active);
            }
            let size = (16.0 * self.scale).round();
            self.icons.draw(
                p,
                icon,
                (r[0] + (r[2] - size) / 2.0).round(),
                (r[1] + (r[3] - size) / 2.0).round(),
                size,
                crate::icons::IconColors {
                    ink: self.theme.text,
                    accent: self.theme.accent,
                    on: self.theme.accent_on,
                    tinted: true,
                },
            );
        }
        let style = TextStyle {
            size: (12.0 * self.scale).round(),
            bold: false,
        };
        let ty = y + (sh - style.size * 1.33) / 2.0;
        let (page, words) = self.status_texts();
        let x0 = (12.0 * self.scale).round();
        let [words_x, book_x, lang_x] = self.status_x;
        p.text(&page, x0, ty, style, self.theme.text);
        p.text(&words, words_x, ty, style, self.theme.text);
        // Proofing state: a book with a cross while issues remain, with a tick when none
        // (own drawing, at the reference's place between word count and language).
        {
            let u = self.scale;
            let (bx, by) = (book_x.round(), (y + (sh - 13.0 * u) / 2.0).round());
            let lw = u.max(1.0);
            // Grey 116 measured on the reference's icon.
            let ink = Rgba(116, 116, 116, 255);
            p.path(
                &[
                    PathSeg::M(bx, by + 1.0 * u),
                    PathSeg::L(bx + 6.0 * u, by),
                    PathSeg::L(bx + 12.0 * u, by + 1.0 * u),
                    PathSeg::L(bx + 12.0 * u, by + 12.0 * u),
                    PathSeg::L(bx + 6.0 * u, by + 11.0 * u),
                    PathSeg::L(bx, by + 12.0 * u),
                    PathSeg::Z,
                    PathSeg::M(bx + 6.0 * u, by),
                    PathSeg::L(bx + 6.0 * u, by + 11.0 * u),
                ],
                Some(self.theme.ribbon),
                Some((ink, lw)),
            );
            let (mx, my) = (bx + 12.0 * u, by + 9.0 * u);
            if self.view.document_has_issues() {
                let k = 3.0 * u;
                p.path(
                    &[
                        PathSeg::M(mx - k, my - k),
                        PathSeg::L(mx + k, my + k),
                        PathSeg::M(mx + k, my - k),
                        PathSeg::L(mx - k, my + k),
                    ],
                    None,
                    Some((ink, 1.5 * lw)),
                );
            } else {
                p.path(
                    &[
                        PathSeg::M(mx - 3.0 * u, my),
                        PathSeg::L(mx - 1.0 * u, my + 2.0 * u),
                        PathSeg::L(mx + 3.5 * u, my - 3.0 * u),
                    ],
                    None,
                    Some((ink, 1.5 * lw)),
                );
            }
        }
        // Proofing language at the caret, as a desktop word processor's status bar shows it.
        // Text without a language of its own shows the editing language (the reference
        // shows its default, English (United States), there).
        {
            let lang = self.view.format().lang.as_deref().unwrap_or("en-US");
            // The status bar's long names ("English (United States)" as on the reference),
            // else the proofing language names.
            let long = format!("statusLang.{lang}");
            let key = format!("spell.lang.{lang}");
            let shown = [&long, &key]
                .into_iter()
                .map(|k| (k, self.strings.get(k)))
                .find(|(k, v)| *v != k.as_str())
                .map_or(lang, |(_, v)| v);
            p.text(shown, lang_x, ty, style, self.theme.text);
        }
        let zoom = self.strings.fmt(
            "status.zoom",
            &[(
                "value",
                &((self.view.zoom() * 100.0).round() as i32).to_string(),
            )],
        );
        let [minus, track, plus, pct] = self.zoom_rects;
        let ink = self.theme.text;
        let k = (4.0 * self.scale).round();
        let line = self.scale.max(1.0).round();
        let (mx, my) = (minus[0] + minus[2] / 2.0, minus[1] + minus[3] / 2.0);
        p.fill_rect((mx - k).round(), my.round(), 2.0 * k, line, ink);
        let (px_, py) = (plus[0] + plus[2] / 2.0, plus[1] + plus[3] / 2.0);
        p.fill_rect((px_ - k).round(), py.round(), 2.0 * k, line, ink);
        p.fill_rect(px_.round(), (py - k).round(), line, 2.0 * k, ink);
        let ty2 = (track[1] + track[3] / 2.0).round();
        p.fill_rect(track[0], ty2, track[2], line, self.theme.border);
        // 100 % tick in the middle, the thumb at the current zoom.
        p.fill_rect(
            (track[0] + track[2] / 2.0).round(),
            ty2 - k,
            line,
            2.0 * k + line,
            self.theme.border,
        );
        let thumb = self.slider_pos();
        p.fill_rect(
            (thumb - k / 2.0).round(),
            ty2 - k - line,
            k,
            2.0 * k + 3.0 * line,
            self.theme.text,
        );
        p.text(&zoom, pct[0] + (6.0 * self.scale).round(), ty, style, ink);
    }

    /// While a header or footer is edited: the body text faded, a dashed line under the
    /// header and over the footer with a "Header" / "Footer" tag, as measured on a desktop
    /// word processor (text 0 → 128, line 119 in 3 px dashes, tag 225 with text 68).
    fn paint_story_frame(&self, p: &mut dyn Painter) {
        if self.story_options().is_none() {
            return;
        }
        let sec = &self.view.format().section;
        let s = self.view.scale();
        let line = Rgba(119, 119, 119, 255);
        let style = self.tag_style();
        let (header, footer) = (self.t("hf.header", "Header"), self.t("hf.footer", "Footer"));
        let dash = (3.0 * self.scale).round().max(1.0);
        for slot in self.view.slots() {
            let (x, w) = (slot.x as f32, slot.width as f32);
            let top = (slot.y as f32 + sec.margin_top as f32 / 20.0 * s).round();
            let bottom =
                (slot.y as f32 + slot.height as f32 - sec.margin_bottom as f32 / 20.0 * s).round();
            if bottom > top {
                p.fill_rect(x, top, w, bottom - top, Rgba(255, 255, 255, 128));
            }
            for y in [top, bottom] {
                let mut dx = 0.0;
                while dx < w {
                    p.fill_rect(x + dx, y, dash.min(w - dx), 1.0, line);
                    dx += 2.0 * dash;
                }
            }
            // Tags at the left edge: under the header line, over the footer line.
            let pad = (6.0 * self.scale).round();
            let tag_h = (style.size * 1.33 + 2.0 * self.scale).round();
            for (i, (label, y)) in [(&header, top + 1.0), (&footer, bottom - tag_h)]
                .into_iter()
                .enumerate()
            {
                let tw = self.tag_w[i].ceil() + 2.0 * pad;
                let tx = x + (6.0 * self.scale).round();
                p.fill_rect(tx, y, tw, tag_h, line);
                p.fill_rect(
                    tx + 1.0,
                    y + 1.0,
                    tw - 2.0,
                    tag_h - 2.0,
                    Rgba(225, 225, 225, 255),
                );
                p.text(
                    label,
                    tx + pad,
                    y + (tag_h - style.size * 1.33) / 2.0,
                    style,
                    Rgba(68, 68, 68, 255),
                );
            }
        }
    }

    fn view_top(&self) -> f32 {
        self.chrome_bottom()
            + if self.show_ruler {
                Ruler::height(self.scale)
            } else {
                0.0
            }
    }

    /// Caret rectangle in window pixels (IME placement).
    pub fn caret_rect(&self) -> Option<[f32; 4]> {
        self.view.caret_rect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Key, Mods};
    use bo_engine::Engine;

    struct Fixed;
    impl TextMeasure for Fixed {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            (
                text.chars().count() as f32 * style.size * 0.55,
                style.size * 1.33,
            )
        }
    }

    #[test]
    fn font_size_steps_match_shared_editor_at_boundaries() {
        assert_eq!(step_font_size(26, true), 28);
        assert_eq!(step_font_size(28, false), 26);
        assert_eq!(step_font_size(25, true), 26);
        assert_eq!(step_font_size(25, false), 24);
        assert_eq!(step_font_size(144, true), 160);
        assert_eq!(step_font_size(160, false), 144);
        assert_eq!(step_font_size(16, false), 14);
        assert_eq!(step_font_size(2, false), 2);
        assert_eq!(step_font_size(3276, true), 3276);
    }

    #[test]
    fn font_ribbon_controls_change_and_clear_formatting() {
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "en-US", "a.docx");
        w.handle(
            Input::Resize {
                width: 1800.0,
                height: 900.0,
            },
            &Fixed,
        );
        w.handle(Input::Text("abc".into()), &Fixed);
        w.handle(
            Input::Key(
                Key::Letter('a'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        w.view.run_command(Command::FormatText {
            format: CharFormat::Size { half_points: 26 },
        });
        let click = |window: &mut DocWindow, id| {
            let [x, y, width, height] = window.ribbon.rect_of(RibbonHit::Command(id)).unwrap();
            let (x, y) = (x + width / 2.0, y + height / 2.0);
            window.handle(
                Input::Press {
                    x,
                    y,
                    count: 1,
                    shift: false,
                },
                &Fixed,
            );
            window.handle(Input::Release { x, y }, &Fixed);
        };
        click(&mut w, "format.growStep");
        assert_eq!(w.view.format().size_half_points, Some(28));
        click(&mut w, "format.shrinkStep");
        assert_eq!(w.view.format().size_half_points, Some(26));
        click(&mut w, "format.bold");
        assert!(w.view.format().bold);
        click(&mut w, "format.color");
        assert_eq!(w.view.format().color.as_deref(), Some("FF0000"));
        click(&mut w, "format.highlight");
        assert!(w.view.format().highlight.is_some());
        click(&mut w, "format.clear");
        assert!(!w.view.format().bold);
        // Menu entries of the Paragraph group run through the engine.
        let sec = bo_edit::SectionState::default();
        for id in [
            "format.multilevel.outline",
            "format.paraBorders.all",
            "format.sortParagraphs.asc",
            "format.textEffects.glow",
            "format.textEffects.none",
        ] {
            let cmd = crate::catalog::command(id, &sec).expect(id);
            w.view.run_command(cmd);
        }
        assert!(w.view.format().numbering);
    }

    #[test]
    fn insert_layout_and_reference_commands_reach_the_saved_document() {
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "en-US", "a.docx");
        w.handle(
            Input::Resize {
                width: 1800.0,
                height: 900.0,
            },
            &Fixed,
        );
        w.handle(Input::Text("Hello world".into()), &Fixed);
        let sec = w.view.format().section;
        let run = |w: &mut DocWindow, id: &str| {
            let cmd = crate::catalog::command(id, &sec).expect(id);
            assert!(w.view.run_command(cmd), "{id} rejected by the engine");
        };
        for id in [
            "insert.field.PAGE",
            "insert.field.NUMPAGES",
            "insert.dateTime",
            "insert.dropCap.drop",
            "layout.lineNumbers.continuous",
            "layout.hyphenation.auto",
            "references.caption.Figure",
            "insert.signatureLine",
            "insert.shape.ellipse",
        ] {
            run(&mut w, id);
        }
        let bytes = w.view.save_docx().unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut read = |name: &str| {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut zip.by_name(name).unwrap(), &mut s).unwrap();
            s
        };
        let doc = read("word/document.xml");
        // §17.16.5.44 PAGE, §17.16.5.42 NUMPAGES, §17.16.5.16 DATE.
        for instr in ["PAGE", "NUMPAGES", "DATE"] {
            assert!(doc.contains(instr), "{instr} field missing");
        }
        assert!(doc.contains("w:dropCap=\"drop\""), "framePr dropCap");
        assert!(doc.contains("w:lnNumType"), "line numbering");
        assert!(doc.contains("Figure"), "caption");
        assert!(doc.contains("prstGeom prst=\"ellipse\""), "ellipse shape");
        assert!(read("word/settings.xml").contains("w:autoHyphenation"));
        // A text box: shape holding txbxContent.
        run(&mut w, "insert.textBox");
        let bytes = w.view.save_docx().unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut doc = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut doc)
            .unwrap();
        assert!(doc.contains("txbxContent"), "text box");
    }

    #[test]
    fn group_settings_buttons_open_font_and_paragraph_dialogs() {
        assert_eq!(parse_num(" 7,5 "), Some(7.5));
        assert_eq!(parse_num("12"), Some(12.0));
        assert_eq!(parse_num(""), None);
        assert_eq!(fmt_pt(150), "7,5");
        assert_eq!(fmt_pt(240), "12");
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "en-US", "a.docx");
        w.handle(
            Input::Resize {
                width: 1800.0,
                height: 900.0,
            },
            &Fixed,
        );
        for (id, kind) in [
            ("format.fontSettings", DialogKind::Font),
            ("format.paragraphSettings", DialogKind::Paragraph),
        ] {
            assert!(
                w.ribbon.rect_of(RibbonHit::Command(id)).is_some(),
                "{id} on the ribbon"
            );
            w.action(id);
            assert_eq!(w.dialog.as_ref().map(|d| d.kind), Some(kind));
            w.dialog = None;
        }
        // Paragraph dialog: the left indent typed in points applies on Enter ("0" + "36").
        w.action("format.paragraphSettings");
        w.handle(Input::Text("36".into()), &Fixed);
        w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
        assert!(w.dialog.is_none());
        assert_eq!(w.view.format().indent_left, 720);
        // Font dialog: the size field (second) takes 7,5 pt.
        w.action("format.fontSettings");
        w.handle(Input::Key(Key::Tab, Mods::default()), &Fixed);
        for _ in 0..4 {
            w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed);
        }
        w.handle(Input::Text("7,5".into()), &Fixed);
        // Bold check box (fourth field): Tab twice from the size, Space ticks it.
        w.handle(Input::Key(Key::Tab, Mods::default()), &Fixed);
        w.handle(Input::Key(Key::Tab, Mods::default()), &Fixed);
        w.handle(Input::Text(" ".into()), &Fixed);
        w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
        assert_eq!(w.view.format().size_half_points, Some(15));
        assert!(w.view.format().bold);
    }

    #[test]
    fn line_spacing_menu_applies_and_undoes_paragraph_change() {
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "vi-VN", "a.docx");
        w.handle(
            Input::Resize {
                width: 1800.0,
                height: 900.0,
            },
            &Fixed,
        );
        w.handle(Input::Text("hello".into()), &Fixed);
        let before = w.view.format().line_spacing;
        let [x, y, width, height] = w
            .ribbon
            .rect_of(RibbonHit::Command("format.lineSpacing"))
            .unwrap();
        let center = (x + width / 2.0, y + height / 2.0);
        w.handle(
            Input::Press {
                x: center.0,
                y: center.1,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(
            Input::Release {
                x: center.0,
                y: center.1,
            },
            &Fixed,
        );
        assert!(w.menu.is_some());
        // Menu opens below the button: select the third entry (1.0, 1.15, 1.5).
        let (mx, my) = (x + 12.0, y + height + 2.0 + 4.0 + 2.0 * 26.0 + 13.0);
        w.handle(
            Input::Press {
                x: mx,
                y: my,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(Input::Release { x: mx, y: my }, &Fixed);
        assert_eq!(w.view.format().line_spacing.rule, bo_edit::LineRule::Auto);
        assert_eq!(w.view.format().line_spacing.value, 360);
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
        assert_eq!(w.view.format().line_spacing, before);
    }

    #[test]
    fn ribbon_button_runs_its_command_and_undo_restores() {
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "vi-VN", "a.docx");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1600.0,
                height: 900.0,
            },
            &Fixed,
        );
        w.handle(Input::Text("abc".into()), &Fixed);
        w.handle(
            Input::Key(
                Key::Letter('a'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        // Find the Bold button on a grid over the ribbon.
        let (mut bx, mut by) = (None, 0.0);
        'scan: for y in (0..w.ribbon.height() as i32).step_by(4) {
            for x in (0..1600).step_by(4) {
                if w.ribbon.hit(x as f32, y as f32) == Some(RibbonHit::Command("format.bold")) {
                    (bx, by) = (Some(x as f32), y as f32);
                    break 'scan;
                }
            }
        }
        let bx = bx.expect("bold button");
        assert!(!w.view().format().bold);
        w.handle(
            Input::Press {
                x: bx,
                y: by,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        assert!(w.handle(Input::Release { x: bx, y: by }, &Fixed));
        assert!(w.view().format().bold, "the button made the selection bold");
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
        assert!(!w.view().format().bold, "undo");
        // A release over another control than the press does nothing.
        w.handle(
            Input::Press {
                x: bx,
                y: by,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(Input::Release { x: 5.0, y: 600.0 }, &Fixed);
        assert!(!w.view().format().bold);
    }
    #[test]
    fn size_menu_sets_the_size_and_escape_closes() {
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "vi-VN", "a.docx");
        w.handle(Input::Scale(1.0), &Fixed);
        w.handle(
            Input::Resize {
                width: 1400.0,
                height: 900.0,
            },
            &Fixed,
        );
        w.handle(Input::Text("abc".into()), &Fixed);
        w.handle(
            Input::Key(
                Key::Letter('a'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        let [x, y, bw, bh] = w.ribbon.rect_of(RibbonHit::SizeBox).expect("size box");
        let (cx, cy) = (x + bw / 2.0, y + bh / 2.0);
        w.handle(
            Input::Press {
                x: cx,
                y: cy,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(Input::Release { x: cx, y: cy }, &Fixed);
        assert!(w.menu.is_some(), "the size box opens its list");
        w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
        assert!(w.menu.is_none(), "Escape closes it");
        w.handle(
            Input::Press {
                x: cx,
                y: cy,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(Input::Release { x: cx, y: cy }, &Fixed);
        // Highlight "14" with the keyboard and choose it.
        let menu = w.menu.as_mut().expect("open again");
        let fourteen = SIZES.iter().position(|s| *s == 14.0).unwrap() as i32;
        menu.step(-100);
        menu.step(fourteen);
        w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
        assert!(w.menu.is_none());
        assert_eq!(w.view().format().size_half_points, Some(28));
    }

    #[test]
    fn zoom_and_clipboard_requests() {
        let mut engine = Engine::with_bundled_fonts();
        engine.new_document("a.docx").unwrap();
        let mut w = DocWindow::new(DocView::new(engine).unwrap(), "vi-VN", "a.docx");
        w.handle(Input::Scale(1.0), &Fixed);
        w.handle(
            Input::Resize {
                width: 1400.0,
                height: 900.0,
            },
            &Fixed,
        );
        assert!(w.handle(Input::Zoom(2.0), &Fixed));
        assert!((w.view().zoom() - 1.2).abs() < 1e-4);
        // The percentage resets to 100 %.
        let [x, y, pw, ph] = w.zoom_rects[3];
        w.handle(
            Input::Press {
                x: x + pw / 2.0,
                y: y + ph / 2.0,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        assert!((w.view().zoom() - 1.0).abs() < 1e-4);
        w.handle(Input::Text("xin chào".into()), &Fixed);
        w.handle(
            Input::Key(
                Key::Letter('a'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
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
        assert!(
            matches!(&reqs[..], [Request::SetClipboard { text, html, .. }] if text == "xin chào" && !html.is_empty()),
            "{reqs:?}"
        );
        w.handle(
            Input::Key(
                Key::Letter('v'),
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
        assert_eq!(w.take_requests(), vec![Request::Paste]);
    }
}

#[cfg(test)]
#[path = "doc_comments_tests.rs"]
mod doc_comments_tests;
#[cfg(test)]
#[path = "doc_design_tests.rs"]
mod doc_design_tests;
#[cfg(test)]
#[path = "doc_find_tests.rs"]
mod doc_find_tests;
#[cfg(test)]
#[path = "doc_perf_tests.rs"]
mod doc_perf_tests;
#[cfg(test)]
#[path = "doc_symbol_tests.rs"]
mod doc_symbol_tests;
#[cfg(test)]
#[path = "doc_window_tests.rs"]
mod doc_window_tests;
