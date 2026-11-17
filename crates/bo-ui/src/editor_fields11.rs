//! BlueEditor round 11: form design as the reference editor's Form tab — with a field tool on,
//! the page's fields can be selected (name tag, red handles), moved, deleted, aligned, centred,
//! distributed and matched in size from their context menu, given a tab order (rows, columns,
//! structure, or clicked one by one), and opened in the tabbed field Properties dialog
//! (General / Appearance / Position / Options / Format / Validate / Calculate / Actions; sizes and
//! places measured on the reference's dialog at 125 %, `docs/ui/REPORT-editor.md` round 11),
//! through `bo_pdfread::FieldProps`.
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{button, check, combo, edit, group, label, rlabel, Ctl, Form, On, Val};
use crate::editor_window::{inside, EditorWindow};
use crate::popup::{Choice, MenuItem};
use crate::{Input, Painter, TextStyle};
use bo_pdfread::{
    format_example, FieldAction, FieldActionKind, FieldCalc, FieldFormat, FieldKind, FieldProps,
    FieldValidate, FormField, CALC_OPS, DATE_FORMATS, FIELD_FONTS, TRIGGERS,
};
use bo_render::{PathSeg, Rgba};

/// Form design state of round 11.
#[derive(Debug, Clone, Default)]
pub(crate) struct Fields11 {
    /// Selected widgets (field mode).
    pub selected: Vec<FormField>,
    /// Pointer press on a selected field: page, start point (page points), current point.
    pub moving: Option<crate::editor_actions::Marking>,
    /// The field the Properties dialog edits.
    pub target: Option<FormField>,
    /// Its actions while the dialog is open.
    pub actions: Vec<FieldAction>,
    /// Box of the Position tab in points (left, bottom, right, top) and the texts shown.
    pub rect: [f32; 4],
    pub shown: [String; 6],
    /// Set Tab Order › Manually: the fields clicked so far.
    pub manual: Option<Vec<FormField>>,
    /// Where the field context menu opened (its sub-menus open there too).
    pub menu_at: [f32; 2],
}

/// Pages of the Properties dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    General,
    Appearance,
    Position,
    Options,
    Format,
    Validate,
    Calculate,
    Actions,
}

/// The dialog's pages for a field kind (as the reference's).
pub(crate) fn pages(kind: FieldKind) -> Vec<Page> {
    use Page::{Actions, Appearance, Calculate, Format, General, Options, Position, Validate};
    match kind {
        FieldKind::Text | FieldKind::Combo => vec![
            General, Appearance, Position, Options, Format, Validate, Calculate, Actions,
        ],
        FieldKind::Signature => vec![General, Appearance, Position, Actions],
        _ => vec![General, Appearance, Position, Options, Actions],
    }
}

/// Dialog size (125 % pixels, caption included) of each page, measured on the reference's text
/// field Properties; pages it has not (other kinds' Options) take the General size grown to fit.
pub(crate) fn page_size(page: Page, kind: FieldKind) -> (f32, f32) {
    match page {
        Page::General => (660.0, 452.0),
        Page::Appearance => (678.0, 615.0),
        Page::Position => (834.0, 495.0),
        Page::Options => match kind {
            FieldKind::Text => (628.0, 667.0),
            FieldKind::Combo | FieldKind::List => (660.0, 600.0),
            _ => (660.0, 452.0),
        },
        Page::Format => (625.0, 755.0),
        Page::Validate => (675.0, 540.0),
        Page::Calculate => (660.0, 752.0),
        Page::Actions => (678.0, 690.0),
    }
}

/// Font sizes of the Appearance page (0 = Auto).
const SIZES: [f32; 16] = [
    0.0, 6.0, 8.0, 9.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 30.0, 36.0,
];

/// Units of the Position page and their points.
const UNITS: [(&str, f32); 5] = [
    ("pdfEditor11.points", 1.0),
    ("pdfEditor11.inches", 72.0),
    ("pdfEditor11.centimeters", 72.0 / 2.54),
    ("pdfEditor11.picas", 12.0),
    ("pdfEditor11.millimeters", 72.0 / 25.4),
];

/// Currency symbols of the Number format.
const CURRENCIES: [&str; 7] = ["", "$", "€", "£", "¥", "₫", "CHF "];

/// Time formats of the Format page (`AFTime_Format` styles).
const TIMES: [&str; 4] = ["HH:MM", "h:MM tt", "HH:MM:ss", "h:MM:ss tt"];

/// Named actions of "Execute a menu item".
const MENU_ITEMS: [&str; 6] = [
    "NextPage",
    "PrevPage",
    "FirstPage",
    "LastPage",
    "GoBack",
    "FullScreen",
];

fn rgb8(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn rgbf(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| f32::from(v) / 255.0)
}

fn num_text(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_owned()
    }
}

fn kind_key(k: FieldKind) -> &'static str {
    match k {
        FieldKind::Text => "pdfEditor11.textProps",
        FieldKind::CheckBox => "pdfEditor11.checkProps",
        FieldKind::Radio => "pdfEditor11.radioProps",
        FieldKind::Combo => "pdfEditor11.comboProps",
        FieldKind::List => "pdfEditor11.listProps",
        FieldKind::Button => "pdfEditor11.buttonProps",
        FieldKind::Signature => "pdfEditor11.signatureProps",
    }
}

fn page_key(p: Page) -> &'static str {
    match p {
        Page::General => "pdfEditor11.general",
        Page::Appearance => "pdfEditor11.appearance",
        Page::Position => "pdfEditor11.position",
        Page::Options => "pdfEditor11.options",
        Page::Format => "pdfEditor11.format",
        Page::Validate => "pdfEditor11.validate",
        Page::Calculate => "pdfEditor11.calculate",
        Page::Actions => "pdfEditor11.actions",
    }
}

fn trigger_key(t: &str) -> &'static str {
    match t {
        "D" => "pdfEditor11.mouseDown",
        "E" => "pdfEditor11.mouseEnter",
        "X" => "pdfEditor11.mouseExit",
        "Fo" => "pdfEditor11.onFocus",
        "Bl" => "pdfEditor11.onBlur",
        _ => "pdfEditor11.mouseUp",
    }
}

/// Action kinds of "Select Action", in the reference's order.
const ACTION_KEYS: [&str; 10] = [
    "pdfEditor11.actMenu",
    "pdfEditor11.actGoTo",
    "pdfEditor11.actImport",
    "pdfEditor11.actWeb",
    "pdfEditor11.actFile",
    "pdfEditor11.actReset",
    "pdfEditor11.actJs",
    "pdfEditor11.actShowHide",
    "pdfEditor11.actSubmit",
    "pdfEditor11.actLayer",
];

fn action_index(k: &FieldActionKind) -> usize {
    match k {
        FieldActionKind::MenuItem(_) => 0,
        FieldActionKind::GoTo(_) => 1,
        FieldActionKind::ImportData(_) => 2,
        FieldActionKind::WebLink(_) => 3,
        FieldActionKind::OpenFile(_) => 4,
        FieldActionKind::ResetForm => 5,
        FieldActionKind::JavaScript(_) => 6,
        FieldActionKind::ShowHide(..) => 7,
        FieldActionKind::Submit(_) => 8,
    }
}

impl EditorWindow {
    fn t11(&self, k: &str) -> String {
        self.t(k)
    }

    // --- Field mode on the page --------------------------------------------------------------

    /// The field whose widget is under window point (x, y).
    pub(crate) fn field_at11(&mut self, x: f32, y: f32) -> Option<FormField> {
        let fields = self.fields();
        let v = self.view()?;
        fields.into_iter().rev().find(|f| {
            v.doc_rect_to_window(f.page, f.rect)
                .is_some_and(|r| inside([r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0], x, y))
        })
    }

    /// Pointer input of field mode before a new field is drawn: select, move, open the
    /// Properties dialog (double click), the context menu; true when used.
    pub(crate) fn fields11_input(&mut self, input: &Input) -> bool {
        let area = self.r.doc_area();
        match *input {
            Input::Press {
                x, y, count, shift, ..
            } if inside(area, x, y) => {
                let hit = self.field_at11(x, y);
                if let Some(order) = self.f11.manual.as_mut() {
                    if let Some(f) = hit {
                        if !order.iter().any(|o| o.same_widget(&f)) {
                            order.push(f);
                        }
                    }
                    return true;
                }
                let Some(f) = hit else {
                    self.f11.selected.clear();
                    return false;
                };
                if count >= 2 {
                    self.f11.selected = vec![f.clone()];
                    self.open_field_props11(&f);
                    return true;
                }
                let picked = self.f11.selected.iter().any(|s| s.same_widget(&f));
                if shift {
                    if picked {
                        self.f11.selected.retain(|s| !s.same_widget(&f));
                    } else {
                        self.f11.selected.push(f);
                    }
                } else if !picked {
                    self.f11.selected = vec![f];
                }
                if let Some((page, px, py)) = self.view().and_then(|v| v.window_to_doc(x, y)) {
                    self.f11.moving = Some((page, (px, py), (px, py)));
                }
                true
            }
            Input::Drag { x, y } if self.f11.moving.is_some() => {
                if let (Some((page, a, _)), Some((_, px, py))) = (
                    self.f11.moving,
                    self.view().and_then(|v| v.window_to_doc(x, y)),
                ) {
                    self.f11.moving = Some((page, a, (px, py)));
                }
                true
            }
            Input::Release { .. } if self.f11.moving.is_some() => {
                let Some((_, a, b)) = self.f11.moving.take() else {
                    return true;
                };
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                if dx.abs() + dy.abs() >= 1.0 {
                    self.move_fields11(dx, dy);
                }
                true
            }
            Input::Context { x, y } if inside(area, x, y) => {
                let Some(f) = self.field_at11(x, y) else {
                    return false;
                };
                if !self.f11.selected.iter().any(|s| s.same_widget(&f)) {
                    self.f11.selected = vec![f];
                }
                self.field_menu11(x, y);
                true
            }
            Input::Key(crate::Key::Delete, _) if !self.f11.selected.is_empty() => {
                self.fields11_command("fd11.delete");
                true
            }
            Input::Key(crate::Key::Enter | crate::Key::Escape, _) if self.f11.manual.is_some() => {
                self.finish_manual11(matches!(input, Input::Key(crate::Key::Enter, _)));
                true
            }
            _ => false,
        }
    }

    /// Move the selected widgets by (dx, dy) page points (y down).
    fn move_fields11(&mut self, dx: f32, dy: f32) {
        let moves: Vec<(FormField, [f32; 4])> = self
            .f11
            .selected
            .iter()
            .filter_map(|f| {
                let v = self.view()?;
                let doc = v.doc();
                let rot = doc.page_info(f.page)?.rotate;
                let r = doc.field_rect(f);
                // Page points (y down, after /Rotate) back to user space.
                let (ux, uy) = match rot {
                    90 => (dy, dx),
                    180 => (-dx, dy),
                    270 => (-dy, -dx),
                    _ => (dx, -dy),
                };
                Some((f.clone(), [r[0] + ux, r[1] + uy, r[2] + ux, r[3] + uy]))
            })
            .collect();
        self.set_rects11(&moves);
    }

    fn set_rects11(&mut self, moves: &[(FormField, [f32; 4])]) {
        if moves.is_empty() {
            return;
        }
        let pages: Vec<usize> = moves.iter().map(|m| m.0.page).collect();
        match self.doc_edit(|d| d.set_field_rects(moves)) {
            Ok(()) => {
                self.after_fields11(&pages);
                // Keep the same widgets selected (their boxes changed).
                let ids: Vec<FormField> = self.f11.selected.clone();
                let now = self.fields();
                self.f11.selected = ids
                    .into_iter()
                    .filter_map(|w| now.iter().find(|f| f.same_widget(&w)).cloned())
                    .collect();
            }
            Err(e) => self.set_status(&e),
        }
    }

    fn after_fields11(&mut self, pages: &[usize]) {
        if let Some(v) = self.r.view_mut() {
            for p in pages {
                v.invalidate_page(*p);
            }
        }
        self.after_doc_edit();
    }

    /// The context menu of selected fields (as the reference's).
    fn field_menu11(&mut self, x: f32, y: f32) {
        self.f11.menu_at = [x, y];
        self.field_submenu11("");
    }

    /// The context menu (`which` empty) or one of its sub-menus (`align`, `center`,
    /// `distribute`, `size`, `tabs`), as the reference's rows; a sub-menu opens where the
    /// context menu was.
    fn field_submenu11(&mut self, which: &str) {
        let n = self.f11.selected.len();
        let order = self
            .f11
            .selected
            .first()
            .and_then(|f| self.view().map(|v| v.doc().tab_order(f.page)))
            .unwrap_or_default();
        let item = |w: &Self, cmd: &str, key: &str, dim: bool, checked: bool| MenuItem {
            label: w.t(key),
            choice: Choice::Command(cmd.to_owned()),
            checked,
            dim,
        };
        let sub = |w: &Self, cmd: &str, key: &str, dim: bool| MenuItem {
            label: format!("{}  \u{203A}", w.t(key)),
            choice: Choice::Command(cmd.to_owned()),
            checked: false,
            dim,
        };
        let rows: &[(&str, &str, usize)] = match which {
            "align" => &[
                ("fd11.alignLeft", "pdfEditor11.alignLeft", 2),
                ("fd11.alignRight", "pdfEditor11.alignRight", 2),
                ("fd11.alignTop", "pdfEditor11.alignTop", 2),
                ("fd11.alignBottom", "pdfEditor11.alignBottom", 2),
                ("fd11.alignVCenter", "pdfEditor11.alignVertical", 2),
                ("fd11.alignHCenter", "pdfEditor11.alignHorizontal", 2),
            ],
            "center" => &[
                ("fd11.centerH", "pdfEditor11.centerHorizontal", 1),
                ("fd11.centerV", "pdfEditor11.centerVertical", 1),
                ("fd11.centerBoth", "pdfEditor11.centerBoth", 1),
            ],
            "distribute" => &[
                ("fd11.distV", "pdfEditor11.distributeVertical", 3),
                ("fd11.distH", "pdfEditor11.distributeHorizontal", 3),
            ],
            "size" => &[
                ("fd11.sizeH", "pdfEditor11.sizeHeight", 2),
                ("fd11.sizeW", "pdfEditor11.sizeWidth", 2),
                ("fd11.sizeBoth", "pdfEditor11.sizeBoth", 2),
            ],
            _ => &[],
        };
        let items: Vec<MenuItem> = if which == "tabs" {
            [
                ("fd11.tabRow", "pdfEditor11.tabRow", "R"),
                ("fd11.tabColumn", "pdfEditor11.tabColumn", "C"),
                ("fd11.tabStructure", "pdfEditor11.tabStructure", "S"),
                ("fd11.tabManual", "pdfEditor11.tabManual", "-"),
            ]
            .iter()
            .map(|(cmd, key, code)| item(self, cmd, key, n == 0, order == *code))
            .collect()
        } else if rows.is_empty() {
            vec![
                item(self, "fd11.props", "pdfEditor11.properties", n != 1, false),
                item(self, "fd11.delete", "pdfEditor11.delete", n == 0, false),
                sub(self, "fd11.menu.align", "pdfEditor11.align", n < 2),
                sub(self, "fd11.menu.center", "pdfEditor11.center", n == 0),
                sub(
                    self,
                    "fd11.menu.distribute",
                    "pdfEditor11.distribute",
                    n < 3,
                ),
                sub(self, "fd11.menu.size", "pdfEditor11.size", n < 2),
                sub(self, "fd11.menu.tabs", "pdfEditor11.setTabOrder", n == 0),
                item(
                    self,
                    "form.highlight",
                    "pdfEditor11.highlightFields",
                    false,
                    self.x.highlight_fields,
                ),
            ]
        } else {
            rows.iter()
                .map(|(cmd, key, need)| item(self, cmd, key, n < *need, false))
                .collect()
        };
        let [x, y] = self.f11.menu_at;
        self.menu_items(items, [x, y, 1.0, 1.0]);
    }

    /// Commands of field mode (`fd11.*`); false when `cmd` is not one.
    pub(crate) fn fields11_command(&mut self, cmd: &str) -> bool {
        if !cmd.starts_with("fd11.") {
            return false;
        }
        let sel = self.f11.selected.clone();
        let user = |w: &Self, f: &FormField| w.view().map(|v| v.doc().field_rect(f));
        if let Some(which) = cmd.strip_prefix("fd11.menu.") {
            self.field_submenu11(which);
            return true;
        }
        match cmd {
            "fd11.props" => {
                if let Some(f) = sel.first() {
                    self.open_field_props11(f);
                }
            }
            "fd11.delete" => {
                let pages: Vec<usize> = sel.iter().map(|f| f.page).collect();
                match self.doc_edit(|d| d.delete_fields(&sel)) {
                    Ok(()) => {
                        self.f11.selected.clear();
                        self.after_fields11(&pages);
                    }
                    Err(e) => self.set_status(&e),
                }
            }
            "fd11.tabRow" | "fd11.tabColumn" | "fd11.tabStructure" => {
                let code = match cmd {
                    "fd11.tabRow" => "R",
                    "fd11.tabColumn" => "C",
                    _ => "S",
                };
                let mut pages: Vec<usize> = sel.iter().map(|f| f.page).collect();
                pages.dedup();
                for p in pages {
                    if let Err(e) = self.doc_edit(|d| d.set_tab_order(p, Some(code))) {
                        self.set_status(&e);
                    }
                }
                self.after_fields11(&[]);
            }
            "fd11.tabManual" => {
                self.f11.manual = Some(Vec::new());
                let s = self.t("pdfEditor11.tabManualHint");
                self.set_status(&s);
            }
            _ => {
                let rects: Vec<(FormField, [f32; 4])> = sel
                    .iter()
                    .filter_map(|f| user(self, f).map(|r| (f.clone(), r)))
                    .collect();
                if let Some(moves) = self.arrange11(cmd, &rects) {
                    self.set_rects11(&moves);
                }
            }
        }
        true
    }

    /// New boxes for an align / centre / distribute / size command. The first selected field
    /// is the anchor (as the reference's: the field right-clicked).
    fn arrange11(
        &self,
        cmd: &str,
        rects: &[(FormField, [f32; 4])],
    ) -> Option<Vec<(FormField, [f32; 4])>> {
        let (_, a) = rects.first()?.clone();
        let mut out: Vec<(FormField, [f32; 4])> = rects.to_vec();
        let w = |r: [f32; 4]| r[2] - r[0];
        let h = |r: [f32; 4]| r[3] - r[1];
        let shift = |r: &mut [f32; 4], dx: f32, dy: f32| {
            r[0] += dx;
            r[2] += dx;
            r[1] += dy;
            r[3] += dy;
        };
        match cmd {
            "fd11.alignLeft" => out.iter_mut().for_each(|(_, r)| shift(r, a[0] - r[0], 0.0)),
            "fd11.alignRight" => out.iter_mut().for_each(|(_, r)| shift(r, a[2] - r[2], 0.0)),
            "fd11.alignTop" => out.iter_mut().for_each(|(_, r)| shift(r, 0.0, a[3] - r[3])),
            "fd11.alignBottom" => out.iter_mut().for_each(|(_, r)| shift(r, 0.0, a[1] - r[1])),
            "fd11.alignVCenter" => out.iter_mut().for_each(|(_, r)| {
                shift(r, (a[0] + a[2] - r[0] - r[2]) / 2.0, 0.0);
            }),
            "fd11.alignHCenter" => out.iter_mut().for_each(|(_, r)| {
                shift(r, 0.0, (a[1] + a[3] - r[1] - r[3]) / 2.0);
            }),
            "fd11.centerH" | "fd11.centerV" | "fd11.centerBoth" => {
                // The selection as a whole goes to the middle of the page.
                let v = self.view()?;
                let page = rects.first()?.0.page;
                let pb = v.doc().page_box(page);
                let x0 = out.iter().map(|o| o.1[0]).fold(f32::MAX, f32::min);
                let x1 = out.iter().map(|o| o.1[2]).fold(f32::MIN, f32::max);
                let y0 = out.iter().map(|o| o.1[1]).fold(f32::MAX, f32::min);
                let y1 = out.iter().map(|o| o.1[3]).fold(f32::MIN, f32::max);
                let dx = if cmd == "fd11.centerV" {
                    0.0
                } else {
                    (pb[0] + pb[2] - x0 - x1) / 2.0
                };
                let dy = if cmd == "fd11.centerH" {
                    0.0
                } else {
                    (pb[1] + pb[3] - y0 - y1) / 2.0
                };
                for (_, r) in &mut out {
                    shift(r, dx, dy);
                }
            }
            "fd11.distV" | "fd11.distH" => {
                let vert = cmd == "fd11.distV";
                let key = |r: &[f32; 4]| if vert { -(r[1] + r[3]) } else { r[0] + r[2] };
                out.sort_by(|p, q| {
                    key(&p.1)
                        .partial_cmp(&key(&q.1))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                let n = out.len();
                if n < 3 {
                    return None;
                }
                let first = key(&out[0].1) / 2.0;
                let last = key(&out[n - 1].1) / 2.0;
                for (i, (_, r)) in out.iter_mut().enumerate() {
                    let want = first + (last - first) * i as f32 / (n - 1) as f32;
                    let d = want - key(r) / 2.0;
                    if vert {
                        shift(r, 0.0, -d);
                    } else {
                        shift(r, d, 0.0);
                    }
                }
            }
            "fd11.sizeH" | "fd11.sizeW" | "fd11.sizeBoth" => {
                for (_, r) in &mut out {
                    if cmd != "fd11.sizeW" {
                        r[1] = r[3] - h(a);
                    }
                    if cmd != "fd11.sizeH" {
                        r[2] = r[0] + w(a);
                    }
                }
            }
            _ => return None,
        }
        Some(out)
    }

    /// Set Tab Order › Manually ends: Enter keeps the clicked order, Esc drops it.
    fn finish_manual11(&mut self, keep: bool) {
        let Some(order) = self.f11.manual.take() else {
            return;
        };
        self.set_status("");
        if !keep || order.is_empty() {
            return;
        }
        let mut pages: Vec<usize> = order.iter().map(|f| f.page).collect();
        pages.sort_unstable();
        pages.dedup();
        for p in pages {
            let on: Vec<FormField> = order.iter().filter(|f| f.page == p).cloned().collect();
            if let Err(e) = self.doc_edit(|d| d.set_field_order(p, &on)) {
                self.set_status(&e);
            }
        }
        self.after_fields11(&[]);
    }

    /// Field mode on the page: every field's box with its name, the selection's handles, and
    /// the tab order numbers while ordering by hand.
    pub(crate) fn paint_fields11(&mut self, p: &mut dyn Painter) {
        if self.design.tool.is_none() || !self.showing_doc() || self.organize_on() {
            return;
        }
        let fields = self.fields();
        let k = self.ui_scale();
        let Some(v) = self.view() else { return };
        let frame = v.frame();
        p.push_clip(frame[0], frame[1], frame[2], frame[3]);
        let visible = v.visible_pages();
        let style = TextStyle {
            size: (12.0 * k).round(),
            bold: false,
        };
        let order: Vec<FormField> = if self.f11.manual.is_some() {
            self.f11.manual.clone().unwrap_or_default()
        } else {
            Vec::new()
        };
        let (dx, dy) = self
            .f11
            .moving
            .map_or((0.0, 0.0), |(_, a, b)| (b.0 - a.0, b.1 - a.1));
        let s = v.scale();
        for f in fields.iter().filter(|f| visible.contains(&f.page)) {
            let picked = self.f11.selected.iter().any(|x| x.same_widget(f));
            let Some(mut r) = v.doc_rect_to_window(f.page, f.rect) else {
                continue;
            };
            if picked {
                r[0] += dx * s;
                r[1] += dy * s;
            }
            let (x0, y0) = (r[0].round() + 0.5, r[1].round() + 0.5);
            let (x1, y1) = ((r[0] + r[2]).round() - 0.5, (r[1] + r[3]).round() - 0.5);
            let edge = if picked {
                Rgba(0, 0, 0, 255)
            } else {
                Rgba(0x80, 0x80, 0x80, 255)
            };
            p.path(
                &[
                    PathSeg::M(x0, y0),
                    PathSeg::L(x1, y0),
                    PathSeg::L(x1, y1),
                    PathSeg::L(x0, y1),
                    PathSeg::Z,
                ],
                None,
                Some((edge, 1.0)),
            );
            // The name tag in the middle (as the reference: red with white text when selected).
            let tag = if order.is_empty() {
                f.name.rsplit('.').next().unwrap_or(&f.name).to_owned()
            } else {
                order
                    .iter()
                    .position(|o| o.same_widget(f))
                    .map_or_else(String::new, |i| (i + 1).to_string())
            };
            if !tag.is_empty() {
                let tw = crate::editor_form8::text_w(&tag, style.size) * 0.9;
                let th = style.size * 1.3;
                let tx = ((x0 + x1 - tw) / 2.0).round();
                let ty = ((y0 + y1 - th) / 2.0).round();
                let (bg, fg) = if picked || !order.is_empty() {
                    (Rgba(0xFF, 0, 0, 255), Rgba(255, 255, 255, 255))
                } else {
                    (Rgba(0xFF, 0xFF, 0xFF, 200), Rgba(0x40, 0x40, 0x40, 255))
                };
                p.fill_rect(tx - 2.0, ty, tw + 4.0, th, bg);
                p.text(&tag, tx, ty, style, fg);
            }
            if picked {
                // Red 6 px handles at the corners and edge middles (measured at 125 %).
                let hs = (6.0 * k / 1.25).round().max(4.0);
                let (mx, my) = (f32::midpoint(x0, x1), f32::midpoint(y0, y1));
                for (hx, hy) in [
                    (x0, y0),
                    (mx, y0),
                    (x1, y0),
                    (x0, my),
                    (x1, my),
                    (x0, y1),
                    (mx, y1),
                    (x1, y1),
                ] {
                    p.fill_rect(
                        (hx - hs / 2.0).round(),
                        (hy - hs / 2.0).round(),
                        hs,
                        hs,
                        Rgba(0xFF, 0, 0, 255),
                    );
                }
            }
        }
        p.pop_clip();
    }

    // --- The Properties dialog --------------------------------------------------------------

    /// Open the Properties dialog of field `f`.
    pub(crate) fn open_field_props11(&mut self, f: &FormField) {
        let Some(props) = self.view().map(|v| v.doc().field_props(f)) else {
            return;
        };
        self.f11.target = Some(f.clone());
        self.f11.actions.clone_from(&props.actions);
        self.f11.rect = props.rect;
        let kind = f.kind;
        let mut form = Form::new(
            self.t11(kind_key(kind)),
            page_size(Page::General, kind),
            Vec::new(),
        );
        form.nullable = vec!["bcolor", "fcolor"];
        form.light = true;
        fill_form(&mut form, &props);
        form.set("tab", Val::Pick(0));
        form.set("units", Val::Pick(1));
        self.f11.shown = Default::default();
        self.show_position11(&mut form);
        form.ctls = self.fprops11_ctls(&form, kind);
        let form = form.focused("name");
        self.form8.push((form, FormWhat::FieldProps11));
    }

    fn kind11(&self) -> FieldKind {
        self.f11.target.as_ref().map_or(FieldKind::Text, |f| f.kind)
    }

    /// The Position page's six boxes from the box in points, in the chosen unit.
    fn show_position11(&mut self, f: &mut Form) {
        let u = UNITS.get(f.pick("units")).map_or(72.0, |u| u.1);
        let r = self.f11.rect;
        let vals = [r[0], r[1], r[2], r[3], r[2] - r[0], r[3] - r[1]].map(|v| num_text(v / u));
        for (id, v) in ["pl", "pb", "pr", "pt", "pw", "ph"].into_iter().zip(&vals) {
            f.set(id, Val::Text(v.clone()));
        }
        self.f11.shown = vals;
    }

    /// The Position page after an input: a changed box moves or sizes the field.
    fn read_position11(&mut self, f: &mut Form) {
        let u = UNITS.get(f.pick("units")).map_or(72.0, |u| u.1);
        let ids = ["pl", "pb", "pr", "pt", "pw", "ph"];
        let now: Vec<String> = ids.iter().map(|id| f.text(id)).collect();
        let Some(i) = (0..6).find(|i| now[*i] != self.f11.shown[*i]) else {
            return;
        };
        let Ok(v) = now[i].trim().parse::<f32>() else {
            return;
        };
        let v = v * u;
        let keep = f.pick("keepSize") == 1;
        let r = &mut self.f11.rect;
        let (w, h) = (r[2] - r[0], r[3] - r[1]);
        match i {
            0 => {
                r[0] = v;
                if keep {
                    r[2] = v + w;
                }
            }
            1 => {
                r[1] = v;
                if keep {
                    r[3] = v + h;
                }
            }
            2 => {
                r[2] = v;
                if keep {
                    r[0] = v - w;
                }
            }
            3 => {
                r[3] = v;
                if keep {
                    r[1] = v - h;
                }
            }
            4 => r[2] = r[0] + v.max(1.0),
            _ => r[1] = r[3] - v.max(1.0),
        }
        // Only the edited box keeps its text; the others follow.
        let r = self.f11.rect;
        let vals = [r[0], r[1], r[2], r[3], r[2] - r[0], r[3] - r[1]].map(|v| num_text(v / u));
        for (k, (id, v)) in ids.iter().zip(&vals).enumerate() {
            if k != i {
                f.set(id, Val::Text(v.clone()));
            }
        }
        let mut shown = vals;
        shown[i].clone_from(&now[i]);
        self.f11.shown = shown;
    }

    /// After any input on the dialog: the shown page and its rows follow the values.
    pub(crate) fn fprops11_after_input(&mut self, units_before: usize) {
        let kind = self.kind11();
        let Some((mut f, FormWhat::FieldProps11)) = self.form8.pop() else {
            return;
        };
        if f.pick("units") == units_before {
            self.read_position11(&mut f);
        } else {
            self.show_position11(&mut f);
        }
        let ctls = self.fprops11_ctls(&f, kind);
        if ctls.len() != f.ctls.len() {
            f.focus = None;
            f.list = None;
        }
        f.ctls = ctls;
        let page = pages(kind)
            .get(f.pick("tab"))
            .copied()
            .unwrap_or(Page::General);
        f.size = page_size(page, kind);
        self.form8.push((f, FormWhat::FieldProps11));
    }

    /// The controls of the shown page.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn fprops11_ctls(&self, f: &Form, kind: FieldKind) -> Vec<Ctl> {
        let t = |k: &str| self.t11(k);
        let list = pages(kind);
        let page = list.get(f.pick("tab")).copied().unwrap_or(Page::General);
        let (w, h) = page_size(page, kind);
        // The page frame, then the tabs over its top line.
        let mut c = vec![Ctl::Box {
            r: [16.0, 72.0, w - 47.0, h - 143.0],
        }];
        let mut x = 18.0;
        for (k, p) in list.iter().enumerate() {
            let label = t(page_key(*p));
            let tw = (crate::editor_form8::text_w(&label, 14.4) + 10.0).round();
            c.push(Ctl::Tab { k, x, w: tw, label });
            x += tw + 4.0;
        }
        c.push(check("locked", 17.0, h - 44.0, t("pdfEditor11.locked")));
        c.push(button(
            "close",
            [w - 110.0, h - 58.0, 91.0, 27.0],
            t("pdfEditor11.close"),
            On::Always,
        ));
        match page {
            Page::General => {
                c.extend([
                    rlabel(168.0, 127.0, t("pdfEditor11.name"), On::Always),
                    edit("name", [185.0, 110.0, 417.0, 36.0], On::Always),
                    rlabel(168.0, 180.0, t("pdfEditor11.tooltip"), On::Always),
                    edit("tooltip", [185.0, 163.0, 417.0, 32.0], On::Always),
                    group(t("pdfEditor11.common"), [48.0, 225.0, 555.0, 137.0]),
                    rlabel(168.0, 268.0, t("pdfEditor11.formField"), On::Always),
                    combo(
                        "vis",
                        [185.0, 253.0, 251.0, 27.0],
                        [
                            "pdfEditor11.visible",
                            "pdfEditor11.hidden",
                            "pdfEditor11.visibleNoPrint",
                            "pdfEditor11.hiddenPrint",
                        ]
                        .iter()
                        .map(|k| t(k))
                        .collect(),
                        On::Always,
                    ),
                    check("readOnly", 460.0, 268.0, t("pdfEditor11.readOnly")),
                    rlabel(168.0, 325.0, t("pdfEditor11.orientation"), On::Always),
                    combo(
                        "orient",
                        [185.0, 310.0, 95.0, 27.0],
                        ["0", "90", "180", "270"]
                            .iter()
                            .map(|s| (*s).to_owned())
                            .collect(),
                        On::Always,
                    ),
                    label(299.0, 325.0, t("pdfEditor11.degrees")),
                    check("required", 460.0, 324.0, t("pdfEditor11.required")),
                ]);
            }
            Page::Appearance => {
                c.extend([
                    group(t("pdfEditor11.borders"), [43.0, 110.0, 580.0, 188.0]),
                    rlabel(188.0, 153.0, t("pdfEditor11.lineStyle"), On::Always),
                    combo(
                        "lstyle",
                        [210.0, 139.0, 150.0, 26.0],
                        [
                            "pdfEditor11.solid",
                            "pdfEditor11.dashed",
                            "pdfEditor11.beveled",
                            "pdfEditor11.inset",
                            "pdfEditor11.underline",
                        ]
                        .iter()
                        .map(|k| t(k))
                        .collect(),
                        On::Always,
                    ),
                    rlabel(188.0, 206.0, t("pdfEditor11.thickness"), On::Always),
                    combo(
                        "thick",
                        [210.0, 192.0, 150.0, 26.0],
                        vec![
                            String::new(),
                            t("pdfEditor11.thin"),
                            t("pdfEditor11.medium"),
                            t("pdfEditor11.thick"),
                        ],
                        On::Always,
                    ),
                    rlabel(188.0, 264.0, t("pdfEditor11.borderColor"), On::Always),
                    Ctl::Swatch {
                        id: "bcolor",
                        r: [210.0, 247.0, 32.0, 32.0],
                    },
                    rlabel(535.0, 264.0, t("pdfEditor11.fillColor"), On::Always),
                    Ctl::Swatch {
                        id: "fcolor",
                        r: [548.0, 247.0, 32.0, 32.0],
                    },
                    group(t("pdfEditor11.text"), [43.0, 320.0, 580.0, 202.0]),
                    rlabel(188.0, 368.0, t("pdfEditor11.fontSize"), On::Always),
                    combo(
                        "fsize",
                        [210.0, 349.0, 150.0, 28.0],
                        SIZES
                            .iter()
                            .map(|s| {
                                if *s == 0.0 {
                                    t("pdfEditor11.auto")
                                } else {
                                    num_text(*s)
                                }
                            })
                            .collect(),
                        On::Always,
                    ),
                    rlabel(535.0, 368.0, t("pdfEditor11.textColor"), On::Always),
                    Ctl::Swatch {
                        id: "tcolor",
                        r: [545.0, 352.0, 32.0, 32.0],
                    },
                    rlabel(188.0, 429.0, t("pdfEditor11.font"), On::Always),
                    combo(
                        "font",
                        [210.0, 413.0, 370.0, 28.0],
                        FIELD_FONTS.iter().map(|f| f.0.to_owned()).collect(),
                        On::Always,
                    ),
                    rlabel(188.0, 493.0, t("pdfEditor11.direction"), On::Always),
                    crate::editor_form8::radio("dir", 0, 210.0, 494.0, t("pdfEditor11.ltr")),
                    crate::editor_form8::radio("dir", 1, 403.0, 494.0, t("pdfEditor11.rtl")),
                ]);
            }
            Page::Position => {
                c.extend([
                    label(51.0, 128.0, t("pdfEditor11.units")),
                    combo(
                        "units",
                        [126.0, 115.0, 268.0, 27.0],
                        UNITS.iter().map(|u| t(u.0)).collect(),
                        On::Always,
                    ),
                    group(t("pdfEditor11.position"), [51.0, 170.0, 712.0, 198.0]),
                ]);
                for (i, (id, key)) in [
                    ("pl", "pdfEditor11.posLeft"),
                    ("pr", "pdfEditor11.posRight"),
                    ("pw", "pdfEditor11.width"),
                    ("pb", "pdfEditor11.bottom"),
                    ("pt", "pdfEditor11.top"),
                    ("ph", "pdfEditor11.height"),
                ]
                .into_iter()
                .enumerate()
                {
                    let (lx, ex) = if i < 3 {
                        (157.0, 176.0)
                    } else {
                        (492.0, 514.0)
                    };
                    let y = [199.0, 254.0, 311.0][i % 3];
                    c.push(rlabel(lx, y + 16.0, t(key), On::Always));
                    c.push(crate::editor_form8::spin(
                        id,
                        [ex, y, 216.0, 34.0],
                        1.0,
                        (-100_000.0, 100_000.0),
                        On::Always,
                    ));
                }
                c.push(check("keepSize", 51.0, 394.0, t("pdfEditor11.keepSize")));
            }
            Page::Options => c.extend(self.options11(f, kind)),
            Page::Format => c.extend(self.format11(f)),
            Page::Validate => {
                let numeric = On::Either("fcat", 1, "fcat", 2);
                c.extend([
                    crate::editor_form8::radio(
                        "val",
                        0,
                        48.0,
                        125.0,
                        t("pdfEditor11.notValidated"),
                    ),
                    Ctl::Radio {
                        id: "val",
                        k: 1,
                        x: 48.0,
                        y: 167.0,
                        label: t("pdfEditor11.inRange"),
                        on: numeric,
                    },
                    rlabel(141.0, 209.0, t("pdfEditor11.from"), numeric),
                    edit("vfrom", [155.0, 193.0, 155.0, 32.0], numeric),
                    rlabel(399.0, 209.0, t("pdfEditor11.to"), numeric),
                    edit("vto", [410.0, 193.0, 155.0, 32.0], numeric),
                    crate::editor_form8::radio(
                        "val",
                        2,
                        48.0,
                        262.0,
                        t("pdfEditor11.customValidation"),
                    ),
                    Ctl::Edit {
                        id: "vjs",
                        r: [75.0, 292.0, 395.0, 154.0],
                        multi: true,
                        on: On::If("val", 2),
                    },
                    button(
                        "vjsEdit",
                        [509.0, 308.0, 111.0, 36.0],
                        t("pdfEditor11.edit"),
                        On::If("val", 2),
                    ),
                ]);
            }
            Page::Calculate => {
                c.extend([
                    crate::editor_form8::radio(
                        "calc",
                        0,
                        45.0,
                        122.0,
                        t("pdfEditor11.notCalculated"),
                    ),
                    crate::editor_form8::radio("calc", 1, 45.0, 163.0, t("pdfEditor11.valueIs")),
                    combo(
                        "op",
                        [273.0, 149.0, 125.0, 28.0],
                        [
                            "pdfEditor11.opSum",
                            "pdfEditor11.opProduct",
                            "pdfEditor11.opAverage",
                            "pdfEditor11.opMin",
                            "pdfEditor11.opMax",
                        ]
                        .iter()
                        .map(|k| t(k))
                        .collect(),
                        On::If("calc", 1),
                    ),
                    label(410.0, 163.0, t("pdfEditor11.ofFields")),
                    Ctl::Edit {
                        id: "cfields",
                        r: [45.0, 197.0, 412.0, 118.0],
                        multi: true,
                        on: On::If("calc", 1),
                    },
                    button(
                        "pick",
                        [490.0, 205.0, 111.0, 36.0],
                        t("pdfEditor11.pick"),
                        On::If("calc", 1),
                    ),
                    crate::editor_form8::radio("calc", 2, 45.0, 344.0, t("pdfEditor11.notation")),
                    Ctl::Edit {
                        id: "cnot",
                        r: [45.0, 367.0, 412.0, 118.0],
                        multi: true,
                        on: On::If("calc", 2),
                    },
                    button(
                        "cnotEdit",
                        [490.0, 375.0, 111.0, 36.0],
                        t("pdfEditor11.edit"),
                        On::If("calc", 2),
                    ),
                    crate::editor_form8::radio("calc", 3, 45.0, 509.0, t("pdfEditor11.customCalc")),
                    Ctl::Edit {
                        id: "cjs",
                        r: [45.0, 534.0, 412.0, 118.0],
                        multi: true,
                        on: On::If("calc", 3),
                    },
                    button(
                        "cjsEdit",
                        [490.0, 545.0, 111.0, 36.0],
                        t("pdfEditor11.edit"),
                        On::If("calc", 3),
                    ),
                ]);
            }
            Page::Actions => {
                let rows: Vec<String> = self
                    .f11
                    .actions
                    .iter()
                    .map(|a| {
                        format!(
                            "{} › {}{}",
                            t(trigger_key(&a.trigger)),
                            t(ACTION_KEYS[action_index(&a.kind)]),
                            match &a.kind {
                                FieldActionKind::WebLink(s)
                                | FieldActionKind::Submit(s)
                                | FieldActionKind::OpenFile(s)
                                | FieldActionKind::ImportData(s)
                                | FieldActionKind::MenuItem(s) => format!(": {s}"),
                                FieldActionKind::GoTo(p) => format!(": {}", p + 1),
                                FieldActionKind::ShowHide(n, _) => format!(": {n}"),
                                _ => String::new(),
                            }
                        )
                    })
                    .collect();
                let chosen = if f.pick("alist") > 0 {
                    On::Always
                } else {
                    On::Never
                };
                c.extend([
                    group(t("pdfEditor11.addAction"), [45.0, 110.0, 575.0, 138.0]),
                    rlabel(220.0, 153.0, t("pdfEditor11.trigger"), On::Always),
                    combo(
                        "trig",
                        [230.0, 139.0, 260.0, 28.0],
                        TRIGGERS.iter().map(|k| t(trigger_key(k))).collect(),
                        On::Always,
                    ),
                    rlabel(220.0, 206.0, t("pdfEditor11.action"), On::Always),
                    combo(
                        "act",
                        [230.0, 192.0, 260.0, 28.0],
                        ACTION_KEYS[..9].iter().map(|k| t(k)).collect(),
                        On::Always,
                    ),
                    button(
                        "addAct",
                        [496.0, 188.0, 110.0, 32.0],
                        t("pdfEditor11.add"),
                        On::Always,
                    ),
                    group(t("pdfEditor11.actions"), [45.0, 272.0, 575.0, 313.0]),
                    Ctl::ListBox {
                        id: "alist",
                        r: [70.0, 299.0, 525.0, 220.0],
                        items: rows,
                        on: On::Always,
                    },
                    button(
                        "actUp",
                        [96.0, 538.0, 101.0, 34.0],
                        t("pdfEditor11.up"),
                        chosen,
                    ),
                    button(
                        "actDown",
                        [219.0, 538.0, 101.0, 34.0],
                        t("pdfEditor11.down"),
                        chosen,
                    ),
                    button(
                        "actEdit",
                        [341.0, 538.0, 101.0, 34.0],
                        t("pdfEditor11.editBtn"),
                        chosen,
                    ),
                    button(
                        "actDelete",
                        [463.0, 538.0, 101.0, 34.0],
                        t("pdfEditor11.deleteBtn"),
                        chosen,
                    ),
                ]);
            }
        }
        c
    }

    /// The Options page of each kind.
    fn options11(&self, f: &Form, kind: FieldKind) -> Vec<Ctl> {
        let t = |k: &str| self.t11(k);
        match kind {
            FieldKind::Text => {
                let limit = On::If("limit", 1);
                vec![
                    rlabel(190.0, 136.0, t("pdfEditor11.alignment"), On::Always),
                    combo(
                        "align",
                        [203.0, 120.0, 147.0, 27.0],
                        [
                            "pdfEditor11.left",
                            "pdfEditor11.center",
                            "pdfEditor11.right",
                        ]
                        .iter()
                        .map(|k| t(k))
                        .collect(),
                        On::Always,
                    ),
                    rlabel(190.0, 180.0, t("pdfEditor11.defaultValue"), On::Always),
                    Ctl::Edit {
                        id: "dv",
                        r: [203.0, 167.0, 362.0, 73.0],
                        multi: true,
                        on: On::Always,
                    },
                    check("scroll", 203.0, 262.0, t("pdfEditor11.scroll")),
                    check("rich", 203.0, 295.0, t("pdfEditor11.rich")),
                    check("limit", 203.0, 329.0, t("pdfEditor11.limitOf")),
                    edit("maxLen", [326.0, 313.0, 66.0, 29.0], limit),
                    crate::editor_form8::label_on(400.0, 329.0, t("pdfEditor11.characters"), limit),
                    Ctl::Check {
                        id: "password",
                        x: 203.0,
                        y: 362.0,
                        label: t("pdfEditor11.password"),
                        on: On::IfNot("multi", 1),
                    },
                    Ctl::Check {
                        id: "fileSel",
                        x: 203.0,
                        y: 392.0,
                        label: t("pdfEditor11.fileSelect"),
                        on: On::IfNot("multi", 1),
                    },
                    check("spell", 203.0, 425.0, t("pdfEditor11.spell")),
                    check("comb", 203.0, 459.0, t("pdfEditor11.combOf")),
                    edit("combN", [333.0, 446.0, 66.0, 29.0], On::If("comb", 1)),
                    crate::editor_form8::label_on(
                        411.0,
                        459.0,
                        t("pdfEditor11.characters"),
                        On::If("comb", 1),
                    ),
                    check("multi", 203.0, 496.0, t("pdfEditor11.multiline")),
                    rlabel(328.0, 529.0, t("pdfEditor11.lineSpacing"), On::Never),
                    combo(
                        "lsp",
                        [340.0, 515.0, 138.0, 27.0],
                        vec![t("pdfEditor11.auto")],
                        On::Never,
                    ),
                ]
            }
            FieldKind::Combo | FieldKind::List => {
                let chosen = if f.pick("opts") > 0 {
                    On::Always
                } else {
                    On::Never
                };
                let mut v = vec![
                    rlabel(150.0, 125.0, t("pdfEditor11.item"), On::Always),
                    edit("item", [165.0, 110.0, 300.0, 30.0], On::Always),
                    button(
                        "optAdd",
                        [485.0, 110.0, 110.0, 30.0],
                        t("pdfEditor11.add"),
                        On::Always,
                    ),
                    rlabel(150.0, 170.0, t("pdfEditor11.exportValue"), On::Always),
                    edit("export", [165.0, 155.0, 300.0, 30.0], On::Always),
                    label(48.0, 212.0, t("pdfEditor11.itemList")),
                    Ctl::ListBox {
                        id: "opts",
                        r: [48.0, 228.0, 417.0, 170.0],
                        items: self.options_list11(f),
                        on: On::Always,
                    },
                    button(
                        "optUp",
                        [485.0, 228.0, 110.0, 30.0],
                        t("pdfEditor11.up"),
                        chosen,
                    ),
                    button(
                        "optDown",
                        [485.0, 266.0, 110.0, 30.0],
                        t("pdfEditor11.down"),
                        chosen,
                    ),
                    button(
                        "optDelete",
                        [485.0, 304.0, 110.0, 30.0],
                        t("pdfEditor11.deleteBtn"),
                        chosen,
                    ),
                    check("sort", 48.0, 420.0, t("pdfEditor11.sortItems")),
                ];
                if kind == FieldKind::Combo {
                    v.push(check("editable", 48.0, 450.0, t("pdfEditor11.allowCustom")));
                    v.push(check("spell", 48.0, 480.0, t("pdfEditor11.spell")));
                } else {
                    v.push(check("multiSel", 48.0, 450.0, t("pdfEditor11.multiSelect")));
                }
                v.push(check("commit", 48.0, 510.0, t("pdfEditor11.commitNow")));
                v
            }
            FieldKind::CheckBox | FieldKind::Radio => vec![
                rlabel(190.0, 127.0, t("pdfEditor11.exportValue"), On::Always),
                edit("export", [205.0, 112.0, 300.0, 30.0], On::Always),
                check(
                    "defOn",
                    48.0,
                    185.0,
                    t(if kind == FieldKind::Radio {
                        "pdfEditor11.radioDefault"
                    } else {
                        "pdfEditor11.checkDefault"
                    }),
                ),
            ],
            _ => vec![
                rlabel(190.0, 127.0, t("pdfEditor11.caption"), On::Always),
                edit("caption", [205.0, 112.0, 380.0, 30.0], On::Always),
            ],
        }
    }

    #[allow(clippy::unused_self)]
    fn options_list11(&self, f: &Form) -> Vec<String> {
        f.text("optList")
            .split('\n')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// The Format page: the category list and the chosen category's options.
    fn format11(&self, f: &Form) -> Vec<Ctl> {
        let t = |k: &str| self.t11(k);
        let cat = f.pick("fcat");
        let mut c = vec![
            rlabel(285.0, 128.0, t("pdfEditor11.formatCategory"), On::Always),
            combo(
                "fcat",
                [325.0, 114.0, 167.0, 27.0],
                [
                    "pdfEditor11.none",
                    "pdfEditor11.number",
                    "pdfEditor11.percentage",
                    "pdfEditor11.date",
                    "pdfEditor11.time",
                    "pdfEditor11.special",
                    "pdfEditor11.custom",
                ]
                .iter()
                .map(|k| t(k))
                .collect(),
                On::Always,
            ),
        ];
        let desc = [
            "pdfEditor11.descNone",
            "pdfEditor11.descNumber",
            "pdfEditor11.descPercent",
            "pdfEditor11.descDate",
            "pdfEditor11.descTime",
            "pdfEditor11.descSpecial",
            "pdfEditor11.descCustom",
        ];
        let title = [
            "",
            "pdfEditor11.numberOptions",
            "pdfEditor11.percentOptions",
            "pdfEditor11.dateOptions",
            "pdfEditor11.timeOptions",
            "pdfEditor11.specialOptions",
            "pdfEditor11.customOptions",
        ];
        if cat == 0 {
            c.push(Ctl::Box {
                r: [55.0, 164.0, 505.0, 384.0],
            });
        } else {
            c.push(group(t(title[cat]), [55.0, 164.0, 505.0, 384.0]));
        }
        // The description under the frame, wrapped in two lines.
        let d = t(desc[cat]);
        let (l1, l2) = split_two(&d, 70);
        c.push(label(59.0, 575.0, l1));
        if !l2.is_empty() {
            c.push(label(59.0, 597.0, l2));
        }
        let example = format_example(&self.format_of11(f));
        let seps: Vec<String> = ["1,234.56", "1234.56", "1.234,56", "1234,56", "1'234.56"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let decimals: Vec<String> = (0..=10).map(|d| d.to_string()).collect();
        match cat {
            1 | 2 => {
                c.extend([
                    rlabel(275.0, 222.0, t("pdfEditor11.decimals"), On::Always),
                    combo("dec", [305.0, 206.0, 220.0, 32.0], decimals, On::Always),
                    rlabel(275.0, 271.0, t("pdfEditor11.separator"), On::Always),
                    combo("sep", [305.0, 256.0, 220.0, 32.0], seps, On::Always),
                ]);
                if cat == 1 {
                    c.extend([
                        rlabel(275.0, 322.0, t("pdfEditor11.currency"), On::Always),
                        combo(
                            "cur",
                            [305.0, 307.0, 220.0, 32.0],
                            CURRENCIES
                                .iter()
                                .map(|s| {
                                    if s.is_empty() {
                                        t("pdfEditor11.none")
                                    } else {
                                        s.trim().to_owned()
                                    }
                                })
                                .collect(),
                            On::Always,
                        ),
                        rlabel(275.0, 382.0, t("pdfEditor11.negative"), On::Always),
                        check("parens", 305.0, 381.0, t("pdfEditor11.parens")),
                        check("red", 305.0, 424.0, t("pdfEditor11.redText")),
                    ]);
                }
                c.push(label(130.0, 490.0, t("pdfEditor11.example")));
                c.push(label(335.0, 490.0, example));
            }
            3..=5 => {
                let items: Vec<String> = match cat {
                    3 => DATE_FORMATS.iter().map(|s| (*s).to_owned()).collect(),
                    4 => TIMES.iter().map(|s| (*s).to_owned()).collect(),
                    _ => [
                        "pdfEditor11.zip",
                        "pdfEditor11.zip4",
                        "pdfEditor11.phone",
                        "pdfEditor11.ssn",
                        "pdfEditor11.mask",
                    ]
                    .iter()
                    .map(|k| t(k))
                    .collect(),
                };
                let id = ["dfmt", "tfmt", "sfmt"][cat - 3];
                c.push(Ctl::ListBox {
                    id,
                    r: [91.0, 200.0, 406.0, 233.0],
                    items,
                    on: On::Always,
                });
                if cat == 5 {
                    c.push(edit("mask", [91.0, 448.0, 406.0, 30.0], On::If("sfmt", 5)));
                } else {
                    c.push(label(126.0, 502.0, t("pdfEditor11.example")));
                    c.push(label(290.0, 502.0, example));
                }
            }
            6 => c.extend([
                label(63.0, 211.0, t("pdfEditor11.customFormat")),
                Ctl::Edit {
                    id: "cfmt",
                    r: [71.0, 237.0, 335.0, 106.0],
                    multi: true,
                    on: On::Always,
                },
                button(
                    "cfmtEdit",
                    [423.0, 238.0, 110.0, 35.0],
                    t("pdfEditor11.edit"),
                    On::Always,
                ),
                label(63.0, 389.0, t("pdfEditor11.customKeystroke")),
                Ctl::Edit {
                    id: "ckey",
                    r: [71.0, 412.0, 335.0, 106.0],
                    multi: true,
                    on: On::Always,
                },
                button(
                    "ckeyEdit",
                    [423.0, 413.0, 110.0, 35.0],
                    t("pdfEditor11.edit"),
                    On::Always,
                ),
            ]),
            _ => {}
        }
        c
    }

    /// The format the Format page's values describe.
    #[allow(clippy::unused_self)]
    fn format_of11(&self, f: &Form) -> FieldFormat {
        match f.pick("fcat") {
            1 => FieldFormat::Number {
                decimals: f.pick("dec").min(10) as u8,
                sep: f.pick("sep").min(4) as u8,
                currency: CURRENCIES
                    .get(f.pick("cur"))
                    .copied()
                    .unwrap_or("")
                    .to_owned(),
                parens: f.pick("parens") == 1,
                red: f.pick("red") == 1,
            },
            2 => FieldFormat::Percent {
                decimals: f.pick("dec").min(10) as u8,
                sep: f.pick("sep").min(4) as u8,
            },
            3 => FieldFormat::Date(
                DATE_FORMATS
                    .get(f.pick("dfmt").saturating_sub(1))
                    .copied()
                    .unwrap_or("m/d/yyyy")
                    .to_owned(),
            ),
            4 => FieldFormat::Time(f.pick("tfmt").saturating_sub(1).min(3) as u8),
            5 => {
                let k = f.pick("sfmt").saturating_sub(1).min(4);
                FieldFormat::Special(
                    k as u8,
                    if k == 4 {
                        f.text("mask")
                    } else {
                        String::new()
                    },
                )
            }
            6 => FieldFormat::Custom {
                format: f.text("cfmt"),
                keystroke: f.text("ckey"),
            },
            _ => FieldFormat::None,
        }
    }

    /// Buttons of the Properties dialog and its small dialogs; true when handled.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn fprops11_button(&mut self, id: &'static str) -> bool {
        let Some((f, what)) = self.form8.last().cloned() else {
            return false;
        };
        match what {
            FormWhat::FieldProps11 => {}
            FormWhat::Script11(target) => {
                if id == "ok" {
                    let text = f.text("js");
                    self.form8.pop();
                    if let Some((p, _)) = self.form8.last_mut() {
                        p.set(target, Val::Text(text));
                    }
                    return true;
                }
                return false;
            }
            FormWhat::Pick11(names) => {
                if id == "ok" {
                    let chosen: Vec<String> = names
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| f.pick(PICK_IDS.get(*i).copied().unwrap_or("")) == 1)
                        .map(|(_, n)| n.clone())
                        .collect();
                    self.form8.pop();
                    if let Some((p, _)) = self.form8.last_mut() {
                        p.set("cfields", Val::Text(chosen.join(", ")));
                    }
                    return true;
                }
                return false;
            }
            FormWhat::Action11(trigger, kind, at) => {
                if id != "ok" {
                    return false;
                }
                let (trigger, kind, at) = (trigger.clone(), kind, at);
                let v = f.text("value");
                let a = match kind {
                    0 => FieldActionKind::MenuItem(
                        MENU_ITEMS
                            .get(f.pick("menu"))
                            .copied()
                            .unwrap_or("NextPage")
                            .to_owned(),
                    ),
                    1 => FieldActionKind::GoTo(
                        v.trim().parse::<usize>().unwrap_or(1).saturating_sub(1),
                    ),
                    2 => FieldActionKind::ImportData(v),
                    3 => FieldActionKind::WebLink(v),
                    4 => FieldActionKind::OpenFile(v),
                    6 => FieldActionKind::JavaScript(v),
                    7 => FieldActionKind::ShowHide(
                        self.fields()
                            .get(f.pick("field"))
                            .map(|x| x.name.clone())
                            .unwrap_or_default(),
                        f.pick("hide") == 0,
                    ),
                    8 => FieldActionKind::Submit(v),
                    _ => FieldActionKind::ResetForm,
                };
                self.form8.pop();
                let a = FieldAction { trigger, kind: a };
                match at {
                    Some(i) if i < self.f11.actions.len() => self.f11.actions[i] = a,
                    _ => self.f11.actions.push(a),
                }
                self.fprops11_after_input(usize::MAX - 1);
                return true;
            }
            _ => return false,
        }
        let sel = f.pick("alist");
        match id {
            "tab" => self.fprops11_after_input(f.pick("units")),
            "cancel" | "close" => {
                if let Some((f, _)) = self.form8.pop() {
                    self.apply_props11(&f);
                }
            }
            "vjsEdit" | "cnotEdit" | "cjsEdit" | "cfmtEdit" | "ckeyEdit" => {
                let target = match id {
                    "vjsEdit" => "vjs",
                    "cnotEdit" => "cnot",
                    "cjsEdit" => "cjs",
                    "cfmtEdit" => "cfmt",
                    _ => "ckey",
                };
                let text = f.text(target);
                let c = vec![
                    Ctl::Edit {
                        id: "js",
                        r: [17.0, 50.0, 640.0, 380.0],
                        multi: true,
                        on: On::Always,
                    },
                    button(
                        "ok",
                        [466.0, 445.0, 91.0, 28.0],
                        self.t("pdfReader.ok"),
                        On::Always,
                    ),
                    button(
                        "cancel",
                        [566.0, 445.0, 91.0, 28.0],
                        self.t("pdfReader.cancel"),
                        On::Always,
                    ),
                ];
                let d = Form::new(self.t("pdfEditor11.jsEditor"), (674.0, 490.0), c)
                    .with("js", Val::Text(text))
                    .focused("js");
                self.form8.push((d, FormWhat::Script11(target)));
            }
            "pick" => {
                let mut names: Vec<String> = Vec::new();
                let me = self
                    .f11
                    .target
                    .as_ref()
                    .map(|t| t.name.clone())
                    .unwrap_or_default();
                for x in self.fields() {
                    if x.name != me && !names.contains(&x.name) && names.len() < PICK_IDS.len() {
                        names.push(x.name);
                    }
                }
                let have: Vec<String> = f
                    .text("cfields")
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .collect();
                let mut c = vec![label(17.0, 55.0, self.t("pdfEditor11.pickFields"))];
                let mut d0 = Vec::new();
                for (i, n) in names.iter().enumerate() {
                    let id = PICK_IDS[i];
                    c.push(check(id, 25.0, 88.0 + 30.0 * i as f32, n.clone()));
                    d0.push((id, have.contains(n)));
                }
                let h = 140.0 + 30.0 * names.len() as f32;
                c.push(button(
                    "ok",
                    [196.0, h - 45.0, 91.0, 28.0],
                    self.t("pdfReader.ok"),
                    On::Always,
                ));
                c.push(button(
                    "cancel",
                    [296.0, h - 45.0, 91.0, 28.0],
                    self.t("pdfReader.cancel"),
                    On::Always,
                ));
                let mut d = Form::new(self.t("pdfEditor11.fieldSelection"), (404.0, h), c);
                for (id, on) in d0 {
                    d.set(id, Val::Pick(usize::from(on)));
                }
                self.form8.push((d, FormWhat::Pick11(names)));
            }
            "addAct" => {
                let trig = TRIGGERS
                    .get(f.pick("trig"))
                    .copied()
                    .unwrap_or("U")
                    .to_owned();
                let kind = f.pick("act");
                self.open_action11(trig, kind, None, None);
            }
            "actEdit" if sel > 0 => {
                if let Some(a) = self.f11.actions.get(sel - 1).cloned() {
                    let k = action_index(&a.kind);
                    self.open_action11(a.trigger.clone(), k, Some(sel - 1), Some(&a.kind));
                }
            }
            "actDelete" if sel > 0 => {
                if sel - 1 < self.f11.actions.len() {
                    self.f11.actions.remove(sel - 1);
                }
                if let Some((f, _)) = self.form8.last_mut() {
                    f.set("alist", Val::Pick(0));
                }
                self.fprops11_after_input(usize::MAX - 1);
            }
            "actUp" | "actDown" if sel > 0 => {
                let i = sel - 1;
                let j = if id == "actUp" {
                    i.checked_sub(1)
                } else {
                    Some(i + 1)
                };
                if let Some(j) = j.filter(|j| *j < self.f11.actions.len()) {
                    self.f11.actions.swap(i, j);
                    if let Some((f, _)) = self.form8.last_mut() {
                        f.set("alist", Val::Pick(j + 1));
                    }
                }
                self.fprops11_after_input(usize::MAX - 1);
            }
            "optAdd" | "optDelete" | "optUp" | "optDown" => {
                let Some((f, _)) = self.form8.last_mut() else {
                    return true;
                };
                let mut list: Vec<String> = f
                    .text("optList")
                    .split('\n')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
                let k = f.pick("opts");
                match id {
                    "optAdd" => {
                        let item = f.text("item").trim().to_owned();
                        if !item.is_empty() && !list.contains(&item) {
                            list.push(item);
                            f.set("item", Val::Text(String::new()));
                        }
                    }
                    "optDelete" if k > 0 && k <= list.len() => {
                        list.remove(k - 1);
                        f.set("opts", Val::Pick(0));
                    }
                    "optUp" if k > 1 && k <= list.len() => {
                        list.swap(k - 1, k - 2);
                        f.set("opts", Val::Pick(k - 1));
                    }
                    "optDown" if k > 0 && k < list.len() => {
                        list.swap(k - 1, k);
                        f.set("opts", Val::Pick(k + 1));
                    }
                    _ => {}
                }
                f.set("optList", Val::Text(list.join("\n")));
                self.fprops11_after_input(usize::MAX - 1);
            }
            _ => return id.starts_with("act"),
        }
        true
    }

    /// The small dialog of an action kind (web link, page, script…); `at`: the action edited.
    fn open_action11(
        &mut self,
        trigger: String,
        kind: usize,
        at: Option<usize>,
        old: Option<&FieldActionKind>,
    ) {
        let field_names: Vec<String> = self.fields().into_iter().map(|f| f.name).collect();
        let t = |k: &str| self.t(k);
        let value = match old {
            Some(
                FieldActionKind::WebLink(s)
                | FieldActionKind::Submit(s)
                | FieldActionKind::OpenFile(s)
                | FieldActionKind::ImportData(s)
                | FieldActionKind::JavaScript(s),
            ) => s.clone(),
            Some(FieldActionKind::GoTo(p)) => (p + 1).to_string(),
            _ => String::new(),
        };
        let mut c = vec![label(17.0, 58.0, t(ACTION_KEYS[kind.min(9)]))];
        let mut h = 190.0;
        match kind {
            0 => c.push(combo(
                "menu",
                [17.0, 80.0, 420.0, 28.0],
                MENU_ITEMS.iter().map(|s| (*s).to_owned()).collect(),
                On::Always,
            )),
            5 => {}
            6 => {
                c.push(Ctl::Edit {
                    id: "value",
                    r: [17.0, 80.0, 420.0, 200.0],
                    multi: true,
                    on: On::Always,
                });
                h = 345.0;
            }
            7 => {
                let names = field_names.clone();
                c.push(combo("field", [17.0, 80.0, 420.0, 28.0], names, On::Always));
                c.push(crate::editor_form8::radio(
                    "hide",
                    0,
                    17.0,
                    130.0,
                    t("pdfEditor11.hideField"),
                ));
                c.push(crate::editor_form8::radio(
                    "hide",
                    1,
                    220.0,
                    130.0,
                    t("pdfEditor11.showField"),
                ));
                h = 220.0;
            }
            _ => c.push(edit("value", [17.0, 80.0, 420.0, 30.0], On::Always)),
        }
        c.push(button(
            "ok",
            [246.0, h - 50.0, 91.0, 28.0],
            t("pdfReader.ok"),
            On::Always,
        ));
        c.push(button(
            "cancel",
            [346.0, h - 50.0, 91.0, 28.0],
            t("pdfReader.cancel"),
            On::Always,
        ));
        let mut d =
            Form::new(t("pdfEditor11.actionTitle"), (454.0, h), c).with("value", Val::Text(value));
        if let Some(FieldActionKind::ShowHide(_, hide)) = old {
            d.set("hide", Val::Pick(usize::from(!*hide)));
        }
        if let Some(FieldActionKind::MenuItem(m)) = old {
            d.set(
                "menu",
                Val::Pick(MENU_ITEMS.iter().position(|x| x == m).unwrap_or(0)),
            );
        }
        let d = if matches!(kind, 0 | 5 | 7) {
            d
        } else {
            d.focused("value")
        };
        self.form8.push((d, FormWhat::Action11(trigger, kind, at)));
    }

    /// Write the dialog's values to the field (the reference applies them as they change; we
    /// apply on Close as one undo step).
    pub(crate) fn apply_props11(&mut self, f: &Form) {
        let Some(target) = self.f11.target.take() else {
            return;
        };
        let props = self.props_of11(f, &target);
        if self.view().map(|v| v.doc().field_props(&target)) == Some(props.clone()) {
            // Nothing changed: the document stays unmodified.
            return;
        }
        let page = target.page;
        match self.doc_edit(|d| d.set_field_props(&target, &props)) {
            Ok(()) => {
                self.f11.selected.clear();
                self.after_fields11(&[page]);
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// The field properties the dialog's values describe.
    pub(crate) fn props_of11(&self, f: &Form, target: &FormField) -> FieldProps {
        let base = self
            .view()
            .map(|v| v.doc().field_props(target))
            .unwrap_or_default();
        let color = |id: &str| match f.vals.get(id) {
            Some(Val::Text(t)) if t == "none" => None,
            Some(Val::Color(c)) => Some(rgbf(*c)),
            _ => None,
        };
        let pick = |id: &str| f.pick(id);
        let on = |id: &str| f.pick(id) == 1;
        let calc = match pick("calc") {
            1 => FieldCalc::Simple(
                CALC_OPS
                    .get(pick("op"))
                    .copied()
                    .unwrap_or("SUM")
                    .to_owned(),
                f.text("cfields")
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect(),
            ),
            2 => FieldCalc::Notation(f.text("cnot")),
            3 => FieldCalc::Script(f.text("cjs")),
            _ => FieldCalc::None,
        };
        let validate = match pick("val") {
            1 => FieldValidate::Range(
                f.text("vfrom").trim().parse().ok(),
                f.text("vto").trim().parse().ok(),
            ),
            2 => FieldValidate::Script(f.text("vjs")),
            _ => FieldValidate::None,
        };
        FieldProps {
            name: f.text("name"),
            tooltip: f.text("tooltip"),
            visibility: pick("vis").min(3) as u8,
            rotation: [0, 90, 180, 270][pick("orient").min(3)],
            read_only: on("readOnly"),
            required: on("required"),
            locked: on("locked"),
            line_style: pick("lstyle").min(4) as u8,
            thickness: pick("thick").min(3) as u8,
            border_color: color("bcolor"),
            fill_color: color("fcolor"),
            font_size: SIZES.get(pick("fsize")).copied().unwrap_or(0.0),
            text_color: rgbf(f.color("tcolor")),
            font: FIELD_FONTS
                .get(pick("font"))
                .map_or("Helvetica", |x| x.0)
                .to_owned(),
            rtl: pick("dir") == 1,
            rect: self.f11.rect,
            align: pick("align").min(2) as u8,
            default_value: f.text("dv"),
            scroll: on("scroll"),
            rich: on("rich"),
            max_len: if on("comb") {
                f.text("combN").trim().parse().unwrap_or(0)
            } else if on("limit") {
                f.text("maxLen").trim().parse().unwrap_or(0)
            } else {
                0
            },
            password: on("password"),
            file_select: on("fileSel"),
            spell_check: on("spell"),
            comb: on("comb"),
            multiline: on("multi"),
            options: self.options_list11(f),
            editable: on("editable"),
            multi_select: on("multiSel"),
            sort: on("sort"),
            commit_now: on("commit"),
            export_value: {
                let e = f.text("export");
                if e.trim().is_empty() {
                    base.export_value.clone()
                } else {
                    e
                }
            },
            default_on: on("defOn"),
            caption: f.text("caption"),
            format: if matches!(target.kind, FieldKind::Text | FieldKind::Combo) {
                self.format_of11(f)
            } else {
                base.format.clone()
            },
            validate,
            calculate: calc,
            actions: self.f11.actions.clone(),
        }
    }
}

/// Ids of the Field Selection check boxes (at most this many fields are offered).
const PICK_IDS: [&str; 12] = [
    "p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9", "p10", "p11",
];

/// Split `s` in two lines near `width` characters.
fn split_two(s: &str, width: usize) -> (String, String) {
    if s.chars().count() <= width {
        return (s.to_owned(), String::new());
    }
    let mut cut = 0;
    for (i, c) in s.char_indices() {
        if s[..i].chars().count() > width {
            break;
        }
        if c == ' ' {
            cut = i;
        }
    }
    if cut == 0 {
        return (s.to_owned(), String::new());
    }
    (s[..cut].to_owned(), s[cut + 1..].to_owned())
}

/// Fill the dialog's values from field properties.
fn fill_form(f: &mut Form, p: &FieldProps) {
    let pick = |v: bool| Val::Pick(usize::from(v));
    f.set("name", Val::Text(p.name.clone()));
    f.set("tooltip", Val::Text(p.tooltip.clone()));
    f.set("vis", Val::Pick(usize::from(p.visibility)));
    f.set("orient", Val::Pick(usize::from(p.rotation / 90).min(3)));
    f.set("readOnly", pick(p.read_only));
    f.set("required", pick(p.required));
    f.set("locked", pick(p.locked));
    f.set("lstyle", Val::Pick(usize::from(p.line_style)));
    f.set("thick", Val::Pick(usize::from(p.thickness)));
    let col = |c: Option<[f32; 3]>| c.map_or(Val::Text("none".into()), |c| Val::Color(rgb8(c)));
    f.set("bcolor", col(p.border_color));
    f.set("fcolor", col(p.fill_color));
    f.set("tcolor", Val::Color(rgb8(p.text_color)));
    f.set(
        "fsize",
        Val::Pick(
            SIZES
                .iter()
                .position(|s| (s - p.font_size).abs() < 0.01)
                .unwrap_or(0),
        ),
    );
    f.set(
        "font",
        Val::Pick(FIELD_FONTS.iter().position(|x| x.0 == p.font).unwrap_or(4)),
    );
    f.set("dir", Val::Pick(usize::from(p.rtl)));
    f.set("align", Val::Pick(usize::from(p.align)));
    f.set("dv", Val::Text(p.default_value.clone()));
    f.set("scroll", pick(p.scroll));
    f.set("rich", pick(p.rich));
    f.set("limit", pick(p.max_len > 0 && !p.comb));
    f.set(
        "combN",
        Val::Text(if p.comb {
            p.max_len.to_string()
        } else {
            String::new()
        }),
    );
    f.set(
        "maxLen",
        Val::Text(if p.max_len > 0 {
            p.max_len.to_string()
        } else {
            "1".into()
        }),
    );
    f.set("password", pick(p.password));
    f.set("fileSel", pick(p.file_select));
    f.set("spell", pick(p.spell_check));
    f.set("comb", pick(p.comb));
    f.set("multi", pick(p.multiline));
    f.set("optList", Val::Text(p.options.join("\n")));
    f.set("editable", pick(p.editable));
    f.set("multiSel", pick(p.multi_select));
    f.set("sort", pick(p.sort));
    f.set("commit", pick(p.commit_now));
    f.set("export", Val::Text(p.export_value.clone()));
    f.set("defOn", pick(p.default_on));
    f.set("caption", Val::Text(p.caption.clone()));
    f.set("dec", Val::Pick(2));
    f.set("dfmt", Val::Pick(1));
    f.set("tfmt", Val::Pick(1));
    f.set("sfmt", Val::Pick(1));
    let (cat, dec, sep) = match &p.format {
        FieldFormat::None => (0, 2, 0),
        FieldFormat::Number {
            decimals,
            sep,
            currency,
            parens,
            red,
        } => {
            f.set(
                "cur",
                Val::Pick(CURRENCIES.iter().position(|c| c == currency).unwrap_or(0)),
            );
            f.set("parens", pick(*parens));
            f.set("red", pick(*red));
            (1, *decimals, *sep)
        }
        FieldFormat::Percent { decimals, sep } => (2, *decimals, *sep),
        FieldFormat::Date(d) => {
            f.set(
                "dfmt",
                Val::Pick(
                    DATE_FORMATS
                        .iter()
                        .position(|x| x == d)
                        .map_or(1, |i| i + 1),
                ),
            );
            (3, 2, 0)
        }
        FieldFormat::Time(k) => {
            f.set("tfmt", Val::Pick(usize::from(*k) + 1));
            (4, 2, 0)
        }
        FieldFormat::Special(k, mask) => {
            f.set("sfmt", Val::Pick(usize::from(*k) + 1));
            f.set("mask", Val::Text(mask.clone()));
            (5, 2, 0)
        }
        FieldFormat::Custom { format, keystroke } => {
            f.set("cfmt", Val::Text(format.clone()));
            f.set("ckey", Val::Text(keystroke.clone()));
            (6, 2, 0)
        }
    };
    f.set("fcat", Val::Pick(cat));
    f.set("dec", Val::Pick(usize::from(dec)));
    f.set("sep", Val::Pick(usize::from(sep)));
    match &p.validate {
        FieldValidate::None => f.set("val", Val::Pick(0)),
        FieldValidate::Range(a, b) => {
            f.set("val", Val::Pick(1));
            f.set(
                "vfrom",
                Val::Text(a.map(|v| v.to_string()).unwrap_or_default()),
            );
            f.set(
                "vto",
                Val::Text(b.map(|v| v.to_string()).unwrap_or_default()),
            );
        }
        FieldValidate::Script(s) => {
            f.set("val", Val::Pick(2));
            f.set("vjs", Val::Text(s.clone()));
        }
    }
    match &p.calculate {
        FieldCalc::None => f.set("calc", Val::Pick(0)),
        FieldCalc::Simple(op, names) => {
            f.set("calc", Val::Pick(1));
            f.set(
                "op",
                Val::Pick(CALC_OPS.iter().position(|o| o == op).unwrap_or(0)),
            );
            f.set("cfields", Val::Text(names.join(", ")));
        }
        FieldCalc::Notation(e) => {
            f.set("calc", Val::Pick(2));
            f.set("cnot", Val::Text(e.clone()));
        }
        FieldCalc::Script(s) => {
            f.set("calc", Val::Pick(3));
            f.set("cjs", Val::Text(s.clone()));
        }
    }
}

#[cfg(test)]
#[path = "editor_round11_tests.rs"]
mod tests;
