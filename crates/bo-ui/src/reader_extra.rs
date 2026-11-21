//! BlueReader additions kept in their own file (a child of `reader_window`, so they reach its
//! state): the Print dialog, Read Mode, Full Screen with presentation-like page flipping, and
//! selecting, moving, recolouring, editing, replying to and deleting existing comments.
use super::{inside, Panel, ReaderRequest, ReaderWindow, Tool};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, MenuItem};
use crate::reader_print::{PrintAction, PrintDialog};
use crate::reader_view::{Fit, Layout, PREVIEW_KEY};
use crate::{Input, Key, Painter, Request, TextMeasure};
use bo_pdfread::Annot;
use bo_render::Rgba;

/// How much of the window the document gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Mode {
    /// Ribbon, panels and status bar.
    #[default]
    Normal,
    /// Read Mode: the ribbon, document tabs and navigation panel are hidden.
    Read,
    /// Full Screen: one page fitted to the screen, flipped like slides.
    Full,
}

/// Comment colours offered in the context menu (label key, colour).
const COLORS: [(&str, [u8; 3]); 6] = [
    ("pdfExtra.yellow", [255, 230, 0]),
    ("pdfExtra.red", [229, 57, 53]),
    ("pdfExtra.green", [67, 160, 71]),
    ("pdfExtra.blue", [30, 136, 229]),
    ("pdfExtra.purple", [142, 36, 170]),
    ("pdfExtra.black", [33, 33, 33]),
];
/// Frame of the selected comment.
const SELECTED: Rgba = Rgba(0x1f, 0x5f, 0xbf, 255);
/// Background around the page in Full Screen.
const FULL_BG: Rgba = Rgba(0, 0, 0, 255);

/// State of the additions.
#[derive(Default)]
pub(crate) struct Extra {
    pub(crate) mode: Mode,
    /// Layout, fit, zoom and panel before Full Screen / Read Mode.
    saved: Option<(Layout, Fit, f32, Option<Panel>)>,
    pub(crate) print: Option<PrintDialog>,
    /// The selected existing comment.
    pub(crate) selected: Option<Annot>,
    /// Moving the selected comment: pointer start and now (window pixels).
    pub(crate) moving: Option<(f32, f32, f32, f32)>,
    /// Text dialog for a comment: edit (`false`) or reply (`true`).
    text: Option<(Dialog, Annot, bool)>,
}

impl ReaderWindow {
    /// Show the Print dialog with the host's printers.
    pub fn open_print(
        &mut self,
        printers: Vec<String>,
        default: Option<&str>,
        m: &dyn TextMeasure,
    ) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let n = t.view.page_count();
        let current = t.view.current_page();
        let mut d = PrintDialog::new(printers, default, n, current);
        if let Some(r) = visible_part(&t.view, current) {
            d.set_view(current, r);
        }
        d.set_page_sizes((0..n).map(|i| t.view.page_size(i)).collect());
        d.set_has_comments(!t.view.doc().comment_threads().is_empty());
        d.layout(m, self.size, self.scale);
        self.extra.print = Some(d);
        self.update_print_preview();
    }

    /// Full Screen or a dialog of the additions takes the whole window.
    pub(crate) fn full_screen8(&self) -> bool {
        self.extra.mode == Mode::Full
    }

    /// Full Screen or a dialog of the additions takes the whole window.
    pub(crate) fn extra_modal(&self) -> bool {
        self.extra.mode == Mode::Full
            || self.extra.print.is_some()
            || self.extra.text.is_some()
            || self.tabs4.field_dialog.is_some()
            || self.tabs4.search_dialog.is_some()
    }

    /// Interface text `key` with `{{name}}` placeholders filled (host messages).
    pub fn strings_fmt(&self, key: &str, args: &[(&str, &str)]) -> String {
        self.strings.fmt(key, args)
    }

    /// The open Print dialog.
    pub fn print_dialog(&self) -> Option<&PrintDialog> {
        self.extra.print.as_ref()
    }

    /// The Print dialog is open.
    pub fn print_open(&self) -> bool {
        self.extra.print.is_some()
    }

    /// Paper of each printer of the open Print dialog (the host measures them).
    pub fn set_print_papers(&mut self, papers: Vec<Option<crate::Paper>>) {
        if let Some(d) = self.extra.print.as_mut() {
            d.set_papers(papers);
        }
    }

    /// Paper of printer `name` after its driver settings changed.
    pub fn set_print_paper(&mut self, name: &str, paper: Option<crate::Paper>) {
        if let Some(d) = self.extra.print.as_mut() {
            d.set_paper(name, paper);
        }
    }

    /// The page images of the sheet shown in the Print dialog's preview, each at the size it
    /// is drawn on the preview's paper.
    fn update_print_preview(&mut self) {
        let (Some(d), Some(t)) = (self.extra.print.as_mut(), self.tabs.get_mut(self.active)) else {
            return;
        };
        let wanted = d.preview_requests();
        t.view.want_previews(&wanted);
        // The images are drawn by the view's prepare right after this; until then the
        // painter has no image for a key and the preview shows grey pages.
        let keys = (0..wanted.len())
            .map(|k| Some(t.key_base + PREVIEW_KEY + 1 + k))
            .collect();
        d.set_preview_keys(keys);
    }

    /// Before page images are brought up to date (the print preview wants its thumbnail).
    pub(crate) fn prepare_extra(&mut self) {
        self.sync_viewx();
        self.prepare_view4();
        self.prepare_thirteen();
        if self.extra.print.is_some() {
            self.update_print_preview();
        }
    }

    /// Enter or leave Full Screen.
    pub(crate) fn toggle_full_screen(&mut self, m: &dyn TextMeasure) {
        if self.extra.mode == Mode::Full {
            self.leave_mode(m);
            self.requests
                .push(ReaderRequest::Window(Request::FullScreen(false)));
            return;
        }
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        if self.extra.mode == Mode::Normal {
            self.extra.saved = Some((t.view.layout(), t.view.fit(), t.view.zoom(), t.panel));
        }
        let page = t.view.current_page();
        t.panel = None;
        // The page fills the screen height like the reference (1 px around it).
        t.view.set_gap(1.0);
        t.view.set_layout(Layout::Single);
        t.view.set_fit(Fit::Page);
        t.view.go_to(page, None);
        self.extra.mode = Mode::Full;
        self.extra.selected = None;
        self.requests
            .push(ReaderRequest::Window(Request::FullScreen(true)));
        self.relayout(m);
    }

    /// Top of the document tab strip in Read Mode (just under the ribbon tab row).
    fn read_strip_y(&self) -> f32 {
        self.title_h() + 1.0 + self.px(23.2)
    }

    /// Enter or leave Read Mode.
    pub(crate) fn toggle_read_mode(&mut self, m: &dyn TextMeasure) {
        if self.extra.mode != Mode::Normal {
            let full = self.extra.mode == Mode::Full;
            self.leave_mode(m);
            if full {
                self.requests
                    .push(ReaderRequest::Window(Request::FullScreen(false)));
            }
            return;
        }
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        self.extra.saved = Some((t.view.layout(), t.view.fit(), t.view.zoom(), t.panel));
        t.panel = None;
        self.extra.mode = Mode::Read;
        self.relayout(m);
    }

    fn leave_mode(&mut self, m: &dyn TextMeasure) {
        let was = self.extra.mode;
        self.extra.mode = Mode::Normal;
        if let (Some((layout, fit, zoom, panel)), Some(t)) =
            (self.extra.saved.take(), self.tabs.get_mut(self.active))
        {
            let page = t.view.current_page();
            t.panel = panel;
            t.view.set_gap(6.0);
            if was == Mode::Full {
                t.view.set_layout(layout);
                if fit == Fit::None {
                    t.view.set_zoom(zoom);
                } else {
                    t.view.set_fit(fit);
                }
                t.view.go_to(page, None);
            }
        }
        self.relayout(m);
    }

    /// Document area and controls for the current mode (after the normal layout).
    pub(crate) fn mode_layout(&mut self) {
        let (w, h) = self.size;
        match self.extra.mode {
            Mode::Normal => {}
            Mode::Full => {
                self.doc_rect = [0.0, 0.0, w.max(1.0), h.max(1.0)];
                self.vbar = [w, 0.0, 0.0, 0.0];
                self.panel_rect = [0.0; 4];
                self.strip_rect = [0.0; 4];
                self.nav_rect = [0.0; 4];
            }
            Mode::Read => {
                // Measured on the reference: the tab row stays, the document tabs move up to
                // y 100 (125 %), the ribbon body and the status bar go; 8 px of chrome remain
                // under the document.
                let top = self.read_strip_y() + self.doc_tabs_h();
                let bottom = h - self.px(6.4) - 1.0;
                let sbw = self.px(16.8);
                let margin = self.px(6.4);
                self.vbar = [w - margin - sbw, top, sbw, bottom - top];
                self.doc_rect = [
                    1.0,
                    top,
                    (w - margin - sbw - 1.0).max(1.0),
                    (bottom - top).max(1.0),
                ];
                self.panel_rect = [0.0; 4];
                self.strip_rect = [0.0; 4];
                self.nav_rect = [0.0; 4];
                // Ribbon, navigation and status bar controls are hidden: drop them from hit
                // testing (the document tabs are placed again when painted).
                let th = self.title_h();
                self.controls.retain(|c| c.r[1] + c.r[3] <= th + 1.0);
                self.doc_tab_rects.clear();
                self.start_tab = [0.0; 4];
                self.page_box = [0.0; 4];
                self.zoom_box = [0.0; 4];
                self.slider = [0.0; 4];
            }
        }
    }

    /// Input for the additions first: `Some(redraw)` when handled.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn route_extra(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        // Marquee zoom, Select Annotation (BlueReader round 12).
        if self.extra.print.is_none() {
            if let Some(r) = self.route_twelve(input, m) {
                return Some(r);
            }
        }
        // Forms, Fill & Sign, the signature dialog (BlueReader round 11).
        if self.extra.print.is_none() {
            if let Some(r) = self.route_eleven(input, m) {
                return Some(r);
            }
        }
        if self.extra.print.is_none() && self.menu.is_none() && self.dialog.is_none() {
            if let Some(r) = self.route_search9(input, m) {
                return Some(r);
            }
            if let Some(r) = self.route_marks9(input, m) {
                return Some(r);
            }
        }
        if self.extra.print.is_none() {
            if let Some(r) = self.route_panels8(input, m) {
                return Some(r);
            }
        }
        if let Some(r) = self.route_prefs6(input, m) {
            return Some(r);
        }
        if self.extra.print.is_none() {
            if let Some(r) = self.route_keys7(input, m) {
                return Some(r);
            }
            if let Some(r) = self.route_tabs5(input, m) {
                return Some(r);
            }
            if let Some(r) = self.route_tabs4(input, m) {
                return Some(r);
            }
            if let Some(r) = self.route_view4(input, m) {
                return Some(r);
            }
            if let Some(r) = self.route_viewx(input, m) {
                return Some(r);
            }
            if self.extra.text.is_none() && self.menu.is_none() && self.dialog.is_none() {
                // Pop-up notes, the selection toolbar (BlueReader round 10).
                if let Some(r) = self.route_ten(input, m) {
                    return Some(r);
                }
            }
            if self.extra.text.is_none() {
                if let Some(r) = self.route_draw(input, m) {
                    return Some(r);
                }
            }
        }
        // Print dialog.
        if let Some(d) = self.extra.print.as_mut() {
            let done = match input {
                Input::Text(s) => {
                    d.text(s);
                    None
                }
                Input::Key(k, mods) => d.key(*k, *mods),
                Input::Press { x, y, .. } => d.press(*x, *y),
                Input::Move { x, y } => {
                    d.hover(*x, *y);
                    None
                }
                Input::Resize { .. } | Input::Scale(_) => return None,
                _ => None,
            };
            match done {
                Some(PrintAction::Print(s)) => {
                    self.extra.print = None;
                    self.requests.push(ReaderRequest::PrintPages(s));
                }
                Some(PrintAction::Cancel) => self.extra.print = None,
                Some(PrintAction::Properties(name)) => {
                    self.requests.push(ReaderRequest::PrinterProperties(name));
                }
                Some(PrintAction::Summarize) => {
                    self.extra.print = None;
                    self.run_command("comment.summarize", m);
                    return Some(true);
                }
                None => {}
            }
            if let Some(d) = self.extra.print.as_mut() {
                d.layout(m, self.size, self.scale);
            }
            self.update_print_preview();
            return Some(true);
        }
        // Comment text dialog.
        if let Some((d, _, _)) = self.extra.text.as_mut() {
            let done = match input {
                Input::Text(s) => {
                    d.text(s);
                    None
                }
                Input::Key(k, mods) => d.key(*k, *mods),
                Input::Press { x, y, .. } if d.contains(*x, *y) => d.press(*x, *y),
                Input::Move { x, y } => {
                    d.hover(*x, *y);
                    None
                }
                Input::Resize { .. } | Input::Scale(_) => return None,
                _ => None,
            };
            if let Some(b) = done {
                if let Some((d, a, reply)) = self.extra.text.take() {
                    if b == DialogButton::Ok {
                        let text = d.values().into_iter().next().unwrap_or_default();
                        self.comment_text(&a, &text, reply);
                    }
                }
            } else if let Some((d, _, _)) = self.extra.text.as_mut() {
                d.layout(m, self.size, self.scale);
            }
            return Some(true);
        }
        if self.menu.is_some() || self.dialog.is_some() {
            return None;
        }
        if let Input::Key(Key::Letter('h'), mods) = input {
            if mods.ctrl {
                self.toggle_read_mode(m);
                return Some(true);
            }
        }
        if self.extra.mode == Mode::Full {
            return Some(self.full_screen_input(input, m));
        }
        if let Some(r) = self.route_menus9(input, m) {
            return Some(r);
        }
        self.comment_input(input, m)
    }

    fn full_screen_input(&mut self, input: &Input, m: &dyn TextMeasure) -> bool {
        // Preferences › Full Screen (round 12): Escape exits, clicks turn pages.
        let (escape, click) = self.full_screen_prefs6();
        let step = match input {
            Input::Key(Key::F(11), _) => {
                self.toggle_full_screen(m);
                return true;
            }
            Input::Key(Key::Escape, _) => {
                if escape {
                    self.toggle_full_screen(m);
                }
                return true;
            }
            Input::Press { .. } | Input::Context { .. } if !click => return true,
            Input::Key(Key::Right | Key::Down | Key::PageDown | Key::Enter, _)
            | Input::Press { .. } => 1,
            Input::Text(s) if s == " " => 1,
            Input::Key(Key::Left | Key::Up | Key::PageUp | Key::Backspace, _)
            | Input::Context { .. } => -1,
            Input::Wheel(dy) if dy.abs() > 0.0 => {
                if *dy > 0.0 {
                    1
                } else {
                    -1
                }
            }
            Input::Key(Key::Home, _) => {
                if let Some(t) = self.tabs.get_mut(self.active) {
                    t.view.go_to(0, None);
                }
                return true;
            }
            Input::Key(Key::End, _) => {
                if let Some(t) = self.tabs.get_mut(self.active) {
                    let n = t.view.page_count();
                    t.view.go_to(n.saturating_sub(1), None);
                }
                return true;
            }
            _ => return false,
        };
        if let Some(t) = self.tabs.get_mut(self.active) {
            let p = t.view.current_page();
            let n = t.view.page_count();
            let next = if step > 0 {
                (p + 1).min(n.saturating_sub(1))
            } else {
                p.saturating_sub(1)
            };
            t.view.go_to(next, None);
        }
        true
    }

    /// The comment of the active document under a window point (topmost first).
    pub(crate) fn comment_at(&self, x: f32, y: f32) -> Option<Annot> {
        let t = self.tabs.get(self.active)?;
        let (page, px, py) = t.view.page_at(x, y)?;
        let pad = 3.0 / t.view.px_per_pt().max(0.01);
        t.view
            .doc()
            .comment_threads()
            .into_iter()
            .map(|th| th.comment)
            .filter(|a| a.page == page)
            .rev()
            .find(|a| {
                // NoZoom notes: the drawn icon, not the whole /Rect (round 11).
                let r = self.shown_rect11(a);
                px >= r[0] - pad && px <= r[2] + pad && py >= r[1] - pad && py <= r[3] + pad
            })
    }

    fn comment_input(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if self.show_start || !matches!(self.tool, Tool::Hand | Tool::Select) {
            return None;
        }
        match *input {
            Input::Press { x, y, count, .. } => {
                if !inside(self.doc_rect, x, y) || self.control_at(x, y).is_some() {
                    return None;
                }
                let Some(a) = self.comment_at(x, y) else {
                    // Elsewhere on the page: no comment selected, the tool does the rest.
                    self.extra.selected = None;
                    return None;
                };
                if count >= 2 {
                    self.ask_comment_text(a, false, m);
                    return Some(true);
                }
                self.extra.selected = Some(a);
                self.extra.moving = Some((x, y, x, y));
                Some(true)
            }
            Input::Drag { x, y } => {
                let mv = self.extra.moving.as_mut()?;
                mv.2 = x;
                mv.3 = y;
                Some(true)
            }
            Input::Release { x, y } => {
                let (x0, y0, _, _) = self.extra.moving.take()?;
                let (dx, dy) = (x - x0, y - y0);
                if dx.abs() + dy.abs() > 3.0 {
                    let a = self.extra.selected.clone()?;
                    let Some(t) = self.tabs.get_mut(self.active) else {
                        return Some(true);
                    };
                    let s = t.view.px_per_pt().max(0.01);
                    if let Err(e) = t.view.doc_mut().move_annotation(&a, dx / s, dy / s) {
                        self.status = e;
                    }
                    self.after_comment_change(Some(a.rect), (dx / s, dy / s));
                }
                Some(true)
            }
            Input::Key(Key::Delete, _) => {
                let a = self.extra.selected.take()?;
                if let Some(t) = self.tabs.get_mut(self.active) {
                    if let Err(e) = t.view.doc_mut().delete_annotation(&a) {
                        self.status = e;
                    }
                }
                self.after_comment_change(None, (0.0, 0.0));
                Some(true)
            }
            Input::Key(Key::Escape, _) if self.extra.selected.is_some() => {
                self.extra.selected = None;
                Some(true)
            }
            Input::Context { x, y } => {
                let a = self.comment_at(x, y)?;
                self.extra.selected = Some(a);
                let mut items = vec![
                    ("comment.edit", self.t("pdfExtra.editText")),
                    ("comment.reply", self.t("pdfExtra.reply")),
                ];
                for (k, (key, _)) in COLORS.iter().enumerate() {
                    items.push((
                        [
                            "comment.color.0",
                            "comment.color.1",
                            "comment.color.2",
                            "comment.color.3",
                            "comment.color.4",
                            "comment.color.5",
                        ][k],
                        format!("{}: {}", self.t("pdfExtra.color"), self.t(key)),
                    ));
                }
                items.push(("comment.delete", self.t("pdfExtra.delete")));
                if self.profile.is_none() {
                    // Review status and checkmark (BlueReader round 8, reader_panels8_win.rs).
                    items.push(("comment.setStatus", self.t("pdfPanels.setStatus")));
                    items.push(("comment.mark", self.t("pdfPanels.mark")));
                }
                let items = items
                    .into_iter()
                    .map(|(cmd, label)| MenuItem {
                        label,
                        choice: Choice::Command(cmd.to_owned()),
                        checked: false,
                        dim: false,
                    })
                    .collect();
                self.menu_at(items, [x, y, 0.0, 0.0], "comment");
                Some(true)
            }
            _ => None,
        }
    }

    /// Commands of the additions; `true` when handled.
    pub(crate) fn command_extra(&mut self, cmd: &str, m: &dyn TextMeasure) -> bool {
        if self.command_twelve(cmd, m)
            || self.command_eleven(cmd, m)
            || self.command_prefs6(cmd, m)
            || self.command_menus9(cmd)
            || self.command_tabs9(cmd)
            || self.command_marks9(cmd)
            || self.command_panels8(cmd, m)
            || self.command_viewx(cmd, m)
            || self.command_view4(cmd, m)
            || self.command_tabs5(cmd, m)
            || self.command_tabs4(cmd, m)
            || self.field_choice(cmd)
            || self.command_draw(cmd, m)
        {
            return true;
        }
        match cmd {
            "view.fullscreen" => self.toggle_full_screen(m),
            "view.readMode" => self.toggle_read_mode(m),
            "comment.edit" | "comment.reply" => {
                if let Some(a) = self.extra.selected.clone() {
                    self.ask_comment_text(a, cmd == "comment.reply", m);
                }
            }
            "comment.delete" => {
                if let (Some(a), Some(t)) =
                    (self.extra.selected.take(), self.tabs.get_mut(self.active))
                {
                    if let Err(e) = t.view.doc_mut().delete_annotation(&a) {
                        self.status = e;
                    }
                    self.after_comment_change(None, (0.0, 0.0));
                }
            }
            _ => {
                let Some(k) = cmd
                    .strip_prefix("comment.color.")
                    .and_then(|v| v.parse::<usize>().ok())
                else {
                    return false;
                };
                let (Some(a), Some((_, c))) = (self.extra.selected.clone(), COLORS.get(k)) else {
                    return true;
                };
                if let Some(t) = self.tabs.get_mut(self.active) {
                    if let Err(e) = t.view.doc_mut().set_annotation_color(&a, *c) {
                        self.status = e;
                    }
                }
                self.after_comment_change(Some(a.rect), (0.0, 0.0));
            }
        }
        true
    }

    pub(crate) fn ask_comment_text(&mut self, a: Annot, reply: bool, m: &dyn TextMeasure) {
        let (title, value) = if reply {
            (self.t("pdfExtra.replyTitle"), String::new())
        } else {
            (self.t("pdfExtra.editTitle"), a.contents.clone())
        };
        let mut d = Dialog::new(
            DialogKind::Comment,
            title,
            vec![(self.t("pdfExtra.text"), value)],
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfPrint.cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        );
        d.layout(m, self.size, self.scale);
        self.extra.text = Some((d, a, reply));
    }

    fn comment_text(&mut self, a: &Annot, text: &str, reply: bool) {
        let author = self.author.clone();
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let r = if reply {
            t.view.doc_mut().reply_to_annotation(a, &author, text)
        } else {
            t.view.doc_mut().set_annotation_contents(a, text)
        };
        if let Err(e) = r {
            self.status = e;
        }
        self.after_comment_change(Some(a.rect), (0.0, 0.0));
    }

    /// Redraw after a comment changed; keep the selection on the comment that moved by
    /// `delta` from `rect` (page points).
    pub(crate) fn after_comment_change(&mut self, rect: Option<[f32; 4]>, (dx, dy): (f32, f32)) {
        self.document_changed();
        self.extra.selected = rect.and_then(|r| {
            let t = self.tabs.get(self.active)?;
            let moved = [r[0] + dx, r[1] + dy, r[2] + dx, r[3] + dy];
            t.view
                .doc()
                .annotations()
                .into_iter()
                .find(|a| a.rect.iter().zip(moved).all(|(p, q)| (p - q).abs() < 0.05))
        });
    }

    /// Full Screen paints only the page; `true` when it did.
    pub(crate) fn paint_full(&mut self, p: &mut dyn Painter) -> bool {
        if self.extra.mode != Mode::Full {
            return false;
        }
        let (w, h) = self.size;
        p.clear(FULL_BG);
        let Some(t) = self.tabs.get(self.active) else {
            return true;
        };
        for i in t.view.visible_pages() {
            let Some(r) = t.view.page_rect(i) else {
                continue;
            };
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(255, 255, 255, 255));
            if t.view.has_image(i) {
                let (ox, oy) = t.view.image_offset(i);
                p.page(t.key_base + i, (r[0] + ox) as i32, (r[1] + oy) as i32);
            }
        }
        // A 1 px frame like the reference's full-screen window.
        p.fill_rect(0.0, 0.0, w, 1.0, self.look.canvas);
        p.fill_rect(0.0, 1.0, 1.0, h - 1.0, self.look.edge);
        p.fill_rect(w - 1.0, 1.0, 1.0, h - 1.0, self.look.edge);
        p.fill_rect(0.0, h - 1.0, w, 1.0, self.look.edge);
        self.paint_full_exit8(p);
        true
    }

    /// Paint over the normal window: Read Mode's larger document area, the selected comment,
    /// a comment being moved and the dialogs of the additions.
    pub(crate) fn paint_extra(&mut self, p: &mut dyn Painter) {
        let (w, h) = self.size;
        if self.extra.mode == Mode::Read && !self.show_start && !self.tabs.is_empty() {
            let y = self.read_strip_y();
            p.fill_rect(1.0, y, w - 2.0, h - y - 1.0, self.look.chrome);
            self.paint_doc_tabs(p, y);
            let top = y + self.doc_tabs_h();
            p.fill_rect(0.0, top - 1.0, w, 1.0, self.look.edge);
            self.paint_doc(p);
            self.paint_vbar(p);
        }
        // BlueReader draws its own handles (round 10, reader_handles10_win.rs).
        let own = self.handles10_on();
        if let (Some(a), Some(t), false) = (
            self.extra.selected.as_ref(),
            self.tabs.get(self.active),
            own,
        ) {
            if let Some(r) = t.view.page_rect(a.page) {
                let s = t.view.px_per_pt();
                let (dx, dy) = self
                    .extra
                    .moving
                    .map_or((0.0, 0.0), |(x0, y0, x1, y1)| (x1 - x0, y1 - y0));
                let x0 = (r[0] + a.rect[0] * s + dx).round() - 2.0;
                let y0 = (r[1] + a.rect[1] * s + dy).round() - 2.0;
                let bw = ((a.rect[2] - a.rect[0]) * s).round() + 4.0;
                let bh = ((a.rect[3] - a.rect[1]) * s).round() + 4.0;
                let d = self.doc_rect;
                p.push_clip(d[0], d[1], d[2], d[3]);
                let k = self.scale.max(1.0).round();
                p.fill_rect(x0, y0, bw, k, SELECTED);
                p.fill_rect(x0, y0 + bh - k, bw, k, SELECTED);
                p.fill_rect(x0, y0, k, bh, SELECTED);
                p.fill_rect(x0 + bw - k, y0, k, bh, SELECTED);
                // Corner handles.
                let hs = self.px(6.0);
                for (cx, cy) in [(x0, y0), (x0 + bw, y0), (x0, y0 + bh), (x0 + bw, y0 + bh)] {
                    p.fill_rect(cx - hs / 2.0, cy - hs / 2.0, hs, hs, SELECTED);
                    p.fill_rect(
                        cx - hs / 2.0 + k,
                        cy - hs / 2.0 + k,
                        hs - 2.0 * k,
                        hs - 2.0 * k,
                        Rgba(255, 255, 255, 255),
                    );
                }
                p.pop_clip();
            }
        }
        self.paint_draw(p);
        self.paint_tabs4(p);
        self.paint_tabs5(p);
        self.paint_keys7(p);
        self.paint15(p);
        self.paint16(p);
    }

    /// Dialogs of the additions, over everything else.
    pub(crate) fn paint_extra_dialogs(&mut self, p: &mut dyn Painter) {
        if let Some(d) = &self.extra.print {
            d.paint(p, &self.theme, &self.strings, self.size, true);
        }
        if let Some((d, _, _)) = &self.extra.text {
            d.paint(p, &self.theme, self.size, true);
        }
        self.paint_draw_dialog(p);
        self.paint_tabs4_dialog(p);
        self.paint_tabs5_dialog(p);
        self.paint_prefs6(p);
    }
}

/// The part of page `i` inside the view (page points x y w h), for printing the current view;
/// `None` when the page is not shown or the view is turned.
fn visible_part(v: &crate::reader_view::ReaderView, i: usize) -> Option<[f32; 4]> {
    if !v.rotation().is_multiple_of(360) {
        return None;
    }
    let r = v.page_rect(i)?;
    let f = v.frame();
    let (x0, y0) = (r[0].max(f[0]), r[1].max(f[1]));
    let (x1, y1) = (
        (r[0] + r[2]).min(f[0] + f[2]),
        (r[1] + r[3]).min(f[1] + f[3]),
    );
    let k = r[2] / v.page_size(i).0.max(1.0);
    if x1 - x0 < 1.0 || y1 - y0 < 1.0 || k <= 0.0 {
        return None;
    }
    Some([
        (x0 - r[0]) / k,
        (y0 - r[1]) / k,
        (x1 - x0) / k,
        (y1 - y0) / k,
    ])
}
