//! BlueSheet formula bar parts measured on the reference spreadsheet (round 3): the name box
//! with its drop-down of defined names and typed go-to, the splitter dots, the box around the
//! cancel / enter / function marks, and the expand button that shows three lines.
use super::SheetWindow;
use crate::popup::{Choice, Menu, MenuItem};
use crate::{Input, Key, Painter, TextMeasure};
use bo_render::{PathSeg, Rgba};

/// Name box arrow (#838688), splitter dots (#B3B3B3) and expand chevron (#777777), measured.
const NAME_ARROW: Rgba = Rgba(0x83, 0x86, 0x88, 255);
const SPLIT_DOT: Rgba = Rgba(0xb3, 0xb3, 0xb3, 255);
const CHEVRON: Rgba = Rgba(0x77, 0x77, 0x77, 255);

/// The name box while a reference or name is typed into it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct NameEdit {
    pub(super) text: String,
    /// The whole text is selected (typing replaces it), as after the click that opens it.
    pub(super) all: bool,
}

/// Functions the name box offers while a formula is typed (the reference's default
/// "recently used" list).
pub(super) const RECENT_FUNCTIONS: [&str; 10] = [
    "SUM",
    "AVERAGE",
    "IF",
    "HYPERLINK",
    "COUNT",
    "MAX",
    "SIN",
    "SUMIF",
    "PMT",
    "STDEV",
];

impl SheetWindow {
    /// A formula is being typed in the cell or the formula bar.
    pub(super) fn formula_editing(&self) -> bool {
        self.view.editing() && self.view.formula_text().starts_with('=')
    }

    /// The name box text: the function being typed (or the first recent one) while a
    /// formula is edited, as the reference shows it; else the cell reference or name.
    pub(super) fn name_box_text(&self) -> String {
        // A selected chart is named in the name box ("Chart 1"), as on the reference.
        if let Some(i) = self.chart_sel {
            return format!("{} {}", self.t("sheetChart.chartName", "Chart"), i + 1);
        }
        if !self.formula_editing() {
            return self.view.name_box();
        }
        let text = self.view.formula_text();
        let upto = self
            .view
            .formula_caret()
            .map_or(text.len(), |c| c.min(text.len()));
        let head = text.get(..upto).unwrap_or(text);
        // The last name followed by "(" before the caret.
        let mut best = None;
        for (i, _) in head.match_indices('(') {
            let name: String = head[..i]
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            if !name.is_empty() && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
                best = Some(name.to_uppercase());
            }
        }
        best.unwrap_or_else(|| RECENT_FUNCTIONS[0].to_owned())
    }

    /// Extra formula bar height of the expanded (three-line) bar.
    pub(super) fn fbar_extra(&self) -> f32 {
        if self.fbar_lines > 1 {
            self.px(19.2) * (self.fbar_lines - 1) as f32
        } else {
            0.0
        }
    }

    /// Drop-down arrow part of the name box.
    pub(super) fn name_arrow_rect(&self) -> [f32; 4] {
        let n = self.name_rect;
        let w = self.px(19.2);
        [n[0] + n[2] - w, n[1], w, n[3]]
    }

    /// Expand / collapse button in the content box's top-right corner.
    pub(super) fn expand_rect(&self) -> [f32; 4] {
        let f = self.fx_rect;
        let w = self.px(16.0);
        [f[0] + f[2] - w - 1.0, f[1] + 1.0, w, self.px(24.0) - 2.0]
    }

    /// Defined names shown by the name box list (visible, not built in), sorted.
    pub(super) fn name_list(&self) -> Vec<String> {
        let sheet = self.view.sheet() as u32;
        let mut names: Vec<String> = self
            .view
            .engine()
            .workbook()
            .map(|w| {
                w.defined_names
                    .iter()
                    .filter(|d| {
                        !d.hidden
                            && !d.name.starts_with("_xlnm.")
                            && d.local_sheet.is_none_or(|s| s == sheet)
                    })
                    .map(|d| d.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        names
    }

    /// Pointer press on the formula bar; `None` when it is elsewhere.
    pub(super) fn fbar_press(&mut self, x: f32, y: f32, text: &dyn TextMeasure) -> Option<bool> {
        if !self.show_fbar {
            return None;
        }
        if Self::in_rect(self.name_arrow_rect(), x, y) && self.formula_editing() {
            // While a formula is typed the name box lists functions (the reference's recently
            // used list), then More Functions… (Insert Function).
            let mut items: Vec<MenuItem> = RECENT_FUNCTIONS
                .iter()
                .map(|n| MenuItem {
                    choice: Choice::Command(format!("fnin.{n}")),
                    label: (*n).to_owned(),
                    checked: false,
                    dim: false,
                })
                .collect();
            items.push(MenuItem {
                choice: Choice::Command("sheet.insertFunction".into()),
                label: self.t("sheetMenu.moreFunctions", "More Functions..."),
                checked: false,
                dim: false,
            });
            self.menu = Some(Menu::open(items, self.name_rect, self.size, self.scale));
            return Some(true);
        }
        if Self::in_rect(self.name_arrow_rect(), x, y) {
            self.name_edit = None;
            let names = self.name_list();
            if names.is_empty() {
                return Some(false);
            }
            let items = names
                .into_iter()
                .map(|n| MenuItem {
                    choice: Choice::Command(format!("goto.{n}")),
                    label: n,
                    checked: false,
                    dim: false,
                })
                .collect();
            self.menu = Some(Menu::open(items, self.name_rect, self.size, self.scale));
            return Some(true);
        }
        if Self::in_rect(self.name_rect, x, y) {
            if self.view.editing() {
                self.view.commit();
            }
            self.name_edit = Some(NameEdit {
                text: self.view.name_box(),
                all: true,
            });
            return Some(true);
        }
        if self.name_edit.take().is_some() && !Self::in_rect(self.fx_rect, x, y) {
            // A click elsewhere ends typing in the name box (and is handled there).
            let _ = text;
            return None;
        }
        if Self::in_rect(self.expand_rect(), x, y) {
            self.fbar_lines = if self.fbar_lines > 1 { 1 } else { 3 };
            self.relayout(text);
            return Some(true);
        }
        None
    }

    /// Keyboard input while the name box is being typed into; `None` when it is not.
    pub(super) fn name_input(&mut self, input: &Input) -> Option<bool> {
        let edit = self.name_edit.as_mut()?;
        match input {
            Input::Text(s) => {
                if edit.all {
                    edit.text.clear();
                    edit.all = false;
                }
                edit.text.push_str(s);
                Some(true)
            }
            Input::Key(Key::Backspace, _) => {
                if edit.all {
                    edit.text.clear();
                    edit.all = false;
                } else {
                    edit.text.pop();
                }
                Some(true)
            }
            Input::Key(Key::Escape, _) => {
                self.name_edit = None;
                Some(true)
            }
            Input::Key(Key::Enter, _) => {
                let typed = self.name_edit.take().map(|e| e.text).unwrap_or_default();
                self.go_to(&typed);
                Some(true)
            }
            Input::Key(..) | Input::Preedit(_) => Some(false),
            _ => None,
        }
    }

    /// Name box drop-down arrow, splitter dots, the marks' box and the expand chevron.
    pub(super) fn paint_fbar_parts(&self, p: &mut dyn Painter) {
        // Name box arrow: a 7 × 4 px triangle 15 px left of the box's right edge, 13 px below
        // its top (measured).
        let n = self.name_rect;
        let (ax, ay) = (n[0] + n[2] - self.px(12.0), n[1] + self.px(10.4));
        let k = self.px(5.6);
        p.path(
            &[
                PathSeg::M(ax, ay),
                PathSeg::L(ax + k, ay),
                PathSeg::L(ax + k / 2.0, ay + k / 2.0),
                PathSeg::Z,
            ],
            Some(NAME_ARROW),
            None,
        );
        // Splitter: three 3 px dots 5 px apart, 17 px right of the name box.
        let d = self.px(2.4);
        let sx = n[0] + n[2] + self.px(13.6);
        for i in 0..3 {
            p.fill_rect(
                sx,
                n[1] + self.px(6.4) + i as f32 * self.px(4.0),
                d,
                d,
                SPLIT_DOT,
            );
        }
        // Expand chevron (down when collapsed, up when expanded): a thick 9 × 6 px V, 15 px
        // left of the content box's right edge and 12 px below its top (measured).
        let f = self.fx_rect;
        let (x0, y0) = (f[0] + f[2] - self.px(12.0), f[1] + self.px(9.6));
        let (w, h, t) = (self.px(7.2), self.px(4.8), self.px(2.4));
        let pts: [(f32, f32); 6] = [
            (0.0, 0.0),
            (t, 0.0),
            (w / 2.0, t / 2.0),
            (w - t, 0.0),
            (w, 0.0),
            (w / 2.0, h),
        ];
        let up = self.fbar_lines > 1;
        let at = |(dx, dy): (f32, f32)| (x0 + dx, if up { y0 + h - dy - 1.0 } else { y0 + dy });
        let mut segs = Vec::with_capacity(7);
        for (i, pt) in pts.iter().enumerate() {
            let (x, y) = at(*pt);
            segs.push(if i == 0 {
                PathSeg::M(x, y)
            } else {
                PathSeg::L(x, y)
            });
        }
        segs.push(PathSeg::Z);
        p.path(&segs, Some(CHEVRON), None);
        // Typing in the name box: its text all selected or with a caret at the end.
        if let Some(e) = &self.name_edit {
            let style = self.ui_style();
            let line = style.size * 1.33;
            let tx = n[0] + self.px(5.0);
            let ty = (n[1] + (n[3] - line) / 2.0).round();
            let inner = [
                n[0] + 1.0,
                n[1] + 1.0,
                n[2] - 2.0 - self.px(19.2),
                n[3] - 2.0,
            ];
            p.fill_rect(
                inner[0],
                inner[1],
                inner[2],
                inner[3],
                Rgba(0xfd, 0xfd, 0xfd, 255),
            );
            let w = self.name_text_w(&e.text);
            if e.all && !e.text.is_empty() {
                p.fill_rect(tx - 1.0, ty, w + 2.0, line, Rgba(0x00, 0x78, 0xd7, 255));
                p.text(&e.text, tx, ty, style, Rgba(255, 255, 255, 255));
            } else {
                p.text(&e.text, tx, ty, style, self.theme.text);
                p.fill_rect((tx + w).round(), ty, 1.0, line, self.theme.text);
            }
        }
    }

    /// Width of name box text (measured while laying out; estimated before).
    fn name_text_w(&self, s: &str) -> f32 {
        self.name_widths
            .iter()
            .find(|(t, _)| t == s)
            .map_or(s.chars().count() as f32 * self.px(7.0), |e| e.1)
    }

    /// Measure the name box text being typed (for its caret).
    pub(super) fn measure_name(&mut self, text: &dyn TextMeasure) {
        if let Some(e) = &self.name_edit {
            if !self.name_widths.iter().any(|(t, _)| *t == e.text) {
                let w = text.measure(&e.text, self.ui_style()).0;
                self.name_widths.push((e.text.clone(), w));
                if self.name_widths.len() > 64 {
                    self.name_widths.remove(0);
                }
            }
        }
    }
}
