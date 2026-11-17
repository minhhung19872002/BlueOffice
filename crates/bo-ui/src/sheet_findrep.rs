//! Home › Find & Select › Find / Replace as the reference's Find and Replace dialog (round
//! 54, measured at 125 %: 683 × 258 px, 320 px with Options >>, a 125 px list below after
//! Find All; `show/sheet-62`): Find / Replace tabs, "Find what" (and "Replace with") boxes,
//! Options >> with Within (Sheet / Workbook), Search (By Rows / By Columns), Look in
//! (Formulas / Values), Match case, Match entire cell contents; Find All lists Book / Sheet /
//! Name / Cell / Value / Formula with "n cell(s) found"; Replace All / Replace / Find Next.
//! The engine's `find` / `replace` do the work.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::Painter;
use bo_model::sheet::col_to_letters;
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

/// Height of the Find All list band below the buttons.
const LIST_H: f32 = 125.0;
/// Columns of the Find All list (left edges, reference pixels from the dialog's edge).
const COLS: [f32; 6] = [17.0, 101.0, 154.0, 209.0, 259.0, 311.0];

/// The dialog's settings, kept between openings (as the reference keeps them).
#[derive(Debug, Clone, Default)]
pub(super) struct FindState {
    pub replace: bool,
    pub expanded: bool,
    pub what: String,
    pub with: String,
    /// Within: 0 Sheet, 1 Workbook; Search: 0 By Rows, 1 By Columns; Look in: 0 Formulas,
    /// 1 Values.
    pub within: usize,
    pub by_cols: usize,
    pub look_in: usize,
    pub match_case: bool,
    pub whole: bool,
    /// Find All results (sheet, row, col) and the selected one.
    pub found: Option<Vec<(usize, u32, u32)>>,
    pub sel: Option<usize>,
    /// First row of the list shown.
    pub top: usize,
}

impl SheetWindow {
    /// Open Find and Replace on its Find (or Replace) tab.
    pub(super) fn open_find_replace(&mut self, replace: bool) -> bool {
        let mut st = self.find_state.clone();
        st.replace = replace;
        st.found = None;
        st.sel = None;
        self.find_state = st;
        self.find_form();
        if let Some(f) = self.form.as_mut() {
            f.focus("what");
        }
        true
    }

    /// Build the form from the state (texts kept).
    fn find_form(&mut self) {
        let st = self.find_state.clone();
        let t = |k: &str, d: &str| self.t(k, d);
        let base = if st.expanded { 320.0 } else { 258.0 };
        let h = base + if st.found.is_some() { LIST_H } else { 0.0 };
        let mut f = Form::new(
            &t("sheetFind.title", "Find and Replace"),
            (683.0, h),
            vec![
                t("sheetFind.find", "Find"),
                t("sheetFind.replace", "Replace"),
            ],
        );
        f.page_h = Some(base - 74.0 - 61.0);
        let what_w = if st.expanded { 302.0 } else { 529.0 };
        f.add(
            "whatL",
            None,
            [32.0, 92.0, 0.0, 0.0],
            Kind::Label(t("sheetFind.what", "Find what:")),
        );
        f.add(
            "what",
            None,
            [122.0, 87.0, what_w, 26.0],
            Kind::Edit {
                value: st.what.chars().collect(),
                caret: st.what.chars().count(),
            },
        );
        f.add(
            "whatDrop",
            None,
            [122.0 + what_w - 22.0, 88.0, 21.0, 24.0],
            Kind::Canvas,
        );
        f.add(
            "withL",
            Some(1),
            [32.0, 124.0, 0.0, 0.0],
            Kind::Label(t("sheetFind.with", "Replace with:")),
        );
        f.add(
            "with",
            Some(1),
            [122.0, 119.0, what_w, 26.0],
            Kind::Edit {
                value: st.with.chars().collect(),
                caret: st.with.chars().count(),
            },
        );
        f.add(
            "withDrop",
            Some(1),
            [122.0 + what_w - 22.0, 120.0, 21.0, 24.0],
            Kind::Canvas,
        );
        if st.expanded {
            // No Format Set (format search is not offered: Format… is dimmed).
            f.add("fmt", None, [438.0, 87.0, 106.0, 26.0], Kind::Canvas);
            f.add(
                "format",
                None,
                [559.0, 87.0, 90.0, 26.0],
                Kind::Button {
                    label: t("sheetFind.format", "Format...") + " ▾",
                    default: false,
                },
            );
            f.enable("format", false);
            f.add("fmt2", Some(1), [438.0, 119.0, 106.0, 26.0], Kind::Canvas);
            f.add(
                "format2",
                Some(1),
                [559.0, 119.0, 90.0, 26.0],
                Kind::Button {
                    label: t("sheetFind.format", "Format...") + " ▾",
                    default: false,
                },
            );
            f.enable("format2", false);
            let combo = |items: Vec<String>, sel: usize| Kind::Combo { items, sel };
            f.add(
                "withinL",
                None,
                [32.0, 162.0, 0.0, 0.0],
                Kind::Label(t("sheetFind.within", "Within:")),
            );
            f.add(
                "within",
                None,
                [91.0, 157.0, 110.0, 26.0],
                combo(
                    vec![
                        t("sheetFind.sheet", "Sheet"),
                        t("sheetFind.workbook", "Workbook"),
                    ],
                    st.within,
                ),
            );
            f.add(
                "searchL",
                None,
                [32.0, 194.0, 0.0, 0.0],
                Kind::Label(t("sheetFind.search", "Search:")),
            );
            f.add(
                "byCols",
                None,
                [91.0, 189.0, 110.0, 26.0],
                combo(
                    vec![
                        t("sheetFind.byRows", "By Rows"),
                        t("sheetFind.byCols", "By Columns"),
                    ],
                    st.by_cols,
                ),
            );
            f.add(
                "lookL",
                None,
                [32.0, 226.0, 0.0, 0.0],
                Kind::Label(t("sheetFind.lookIn", "Look in:")),
            );
            f.add(
                "lookIn",
                None,
                [91.0, 221.0, 110.0, 26.0],
                combo(
                    vec![
                        t("sheetFind.formulas", "Formulas"),
                        t("sheetFind.values", "Values"),
                    ],
                    st.look_in,
                ),
            );
            f.add(
                "matchCase",
                None,
                [217.0, 158.0, 200.0, 20.0],
                Kind::Check {
                    label: t("sheetFind.matchCase", "Match case"),
                    on: st.match_case,
                },
            );
            f.add(
                "whole",
                None,
                [217.0, 182.0, 260.0, 20.0],
                Kind::Check {
                    label: t("sheetFind.whole", "Match entire cell contents"),
                    on: st.whole,
                },
            );
            f.set_access("withinL", 3);
            f.set_access("searchL", 0);
            f.set_access("lookL", 0);
            f.set_access("matchCase", 6);
            f.set_access("whole", 15);
        }
        let options = if st.expanded {
            t("sheetFind.optionsLess", "Options <<")
        } else {
            t("sheetFind.optionsMore", "Options >>")
        };
        let oy = if st.expanded { 218.0 } else { 156.0 };
        f.add(
            "options",
            None,
            [538.0, oy, 111.0, 28.0],
            Kind::Button {
                label: options,
                default: false,
            },
        );
        let by = base - 48.0;
        let buttons: [(&'static str, Option<usize>, &str, &str, f32); 5] = [
            (
                "replaceAll",
                Some(1),
                "sheetFind.replaceAll",
                "Replace All",
                18.0,
            ),
            (
                "replaceOne",
                Some(1),
                "sheetFind.replaceOne",
                "Replace",
                145.0,
            ),
            ("findAll", None, "sheetFind.findAll", "Find All", 285.0),
            ("findNext", None, "sheetFind.findNext", "Find Next", 412.0),
            ("close", None, "sheetFind.close", "Close", 552.0),
        ];
        for (id, page, key, d, x) in buttons {
            f.add(
                id,
                page,
                [x, by, 111.0, 28.0],
                Kind::Button {
                    label: t(key, d),
                    default: id == "findNext",
                },
            );
        }
        f.set_access("whatL", 1);
        f.set_access("withL", 1);
        f.set_access("options", 1);
        f.set_access("replaceAll", 1);
        f.set_access("replaceOne", 0);
        f.set_access("findAll", 1);
        f.set_access("findNext", 0);
        if st.found.is_some() {
            f.add(
                "results",
                None,
                [9.0, base - 10.0, 665.0, LIST_H],
                Kind::Canvas,
            );
        }
        if st.replace {
            f.set_tab(1);
        }
        self.form = Some(f);
        self.form_kind = FormKind::FindReplace;
    }

    /// Read the controls back into the state.
    fn find_take(&mut self) {
        let Some(f) = self.form.as_ref() else {
            return;
        };
        let st = &mut self.find_state;
        st.what = f.text("what");
        st.with = f.text("with");
        st.replace = f.tab == 1;
        if st.expanded {
            st.within = f.sel("within").unwrap_or(0);
            st.by_cols = f.sel("byCols").unwrap_or(0);
            st.look_in = f.sel("lookIn").unwrap_or(0);
            st.match_case = f.on("matchCase");
            st.whole = f.on("whole");
        }
    }

    /// The matches in search order.
    fn find_matches(&mut self) -> Vec<(usize, u32, u32)> {
        let st = self.find_state.clone();
        let sheet = (st.within == 0).then(|| self.view.sheet());
        let out = self
            .view
            .query(json!({
                "cmd": "find", "query": st.what, "matchCase": st.match_case,
                "wholeCell": st.whole, "inFormulas": st.look_in == 0, "sheet": sheet,
            }))
            .unwrap_or(Value::Null);
        let n = |m: &Value, k: &str| m.get(k).and_then(Value::as_u64).unwrap_or(0);
        let mut list: Vec<(usize, u32, u32)> = out
            .get("matches")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|m| {
                        (
                            n(m, "sheet") as usize,
                            n(m, "row") as u32,
                            n(m, "col") as u32,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        if st.by_cols == 1 {
            list.sort_unstable_by_key(|(s, r, c)| (*s, *c, *r));
        } else {
            list.sort_unstable();
        }
        list
    }

    /// Go to a match (switching sheets when needed).
    fn find_go(&mut self, (s, r, c): (usize, u32, u32)) {
        if s != self.view.sheet() {
            self.view.set_sheet(s);
        }
        self.view.select((r, c), (r, c), (r, c));
        self.view.scroll_to_cell(r, c);
    }

    /// Find Next from the active cell (wrapping round).
    fn find_next_match(&mut self) -> bool {
        let list = self.find_matches();
        let (r, c) = self.view.active();
        let here = (self.view.sheet(), r, c);
        let by_cols = self.find_state.by_cols == 1;
        let key = |m: &(usize, u32, u32)| {
            if by_cols {
                (m.0, m.2, m.1)
            } else {
                *m
            }
        };
        let next = list
            .iter()
            .find(|m| key(m) > key(&here))
            .or_else(|| list.first())
            .copied();
        if let Some(m) = next {
            self.find_go(m);
            true
        } else {
            self.status = self.t(
                "sheetFind.none",
                "We couldn't find what you were looking for.",
            );
            false
        }
    }

    /// Events of Find and Replace.
    pub(super) fn find_event(&mut self, ev: &Event) {
        match ev {
            Event::Cancel | Event::Clicked("close") => {
                self.find_take();
                self.form = None;
            }
            Event::Clicked("options") => {
                self.find_take();
                self.find_state.expanded = !self.find_state.expanded;
                self.find_form();
            }
            Event::Ok | Event::Clicked("findNext") => {
                self.find_take();
                self.find_next_match();
            }
            Event::Clicked("findAll") => {
                self.find_take();
                let list = self.find_matches();
                let first = list.first().copied();
                self.find_state.found = Some(list);
                self.find_state.sel = first.map(|_| 0);
                self.find_state.top = 0;
                self.find_form();
                if let Some(m) = first {
                    self.find_go(m);
                }
            }
            Event::Clicked("replaceAll") => {
                self.find_take();
                let st = self.find_state.clone();
                let sheet = (st.within == 0).then(|| self.view.sheet());
                let out = self.view.query(json!({
                    "cmd": "replace", "query": st.what, "with": st.with,
                    "matchCase": st.match_case, "wholeCell": st.whole, "sheet": sheet,
                }));
                self.view.changed();
                let n = out
                    .as_ref()
                    .and_then(|o| o.get("count"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                self.status = self
                    .t("sheetFind.replaced", "All done. We made {n} replacements.")
                    .replace("{n}", &n.to_string());
            }
            Event::Clicked("replaceOne") => {
                self.find_take();
                let st = self.find_state.clone();
                let (r, c) = self.view.active();
                let here = (self.view.sheet(), r, c);
                // Replace in the active cell when it matches, then find the next one.
                if self.find_matches().contains(&here) {
                    let _ = self.view.query(json!({
                        "cmd": "replace", "query": st.what, "with": st.with,
                        "matchCase": st.match_case, "wholeCell": st.whole,
                        "sheet": here.0, "at": { "sheet": here.0, "row": r, "col": c },
                    }));
                    self.view.changed();
                }
                self.find_next_match();
            }
            Event::Canvas("results", _, y) => {
                let i = (((y - 24.0) / 21.0).floor().max(0.0)) as usize + self.find_state.top;
                let m = self
                    .find_state
                    .found
                    .as_ref()
                    .and_then(|l| l.get(i))
                    .copied();
                if let Some(m) = m {
                    self.find_state.sel = Some(i);
                    self.find_go(m);
                }
            }
            _ => {}
        }
    }

    /// The owner-drawn parts: drop arrows, "No Format Set", the Find All list.
    pub(super) fn paint_find(&self, f: &Form, id: &str, r: [f32; 4], p: &mut dyn Painter) {
        let k = f.scale_k();
        let font = f.font();
        let ink = Rgba(0, 0, 0, 255);
        match id {
            "whatDrop" | "withDrop" => {
                p.fill_rect(r[0], r[1], 1.0, r[3], Rgba(0xAB, 0xAD, 0xB3, 255));
                let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                let s = 4.0 * k;
                p.path(
                    &[
                        PathSeg::M(cx - s, cy - s / 2.0),
                        PathSeg::L(cx, cy + s / 2.0),
                        PathSeg::L(cx + s, cy - s / 2.0),
                    ],
                    None,
                    Some((Rgba(0x60, 0x60, 0x60, 255), k.max(1.0))),
                );
            }
            "fmt" | "fmt2" => {
                p.fill_rect(r[0], r[1], r[2], r[3], ink);
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    Rgba(255, 255, 255, 255),
                );
                let label = self.t("sheetFind.noFormat", "No Format Set");
                let w = f.width(&label);
                p.text_font(
                    &label,
                    (r[0] + (r[2] - w) / 2.0).round(),
                    (r[1] + (r[3] - font.size * 1.33) / 2.0).round(),
                    &font,
                    ink,
                );
            }
            "results" => self.paint_find_results(f, r, p),
            _ => {}
        }
    }

    fn paint_find_results(&self, f: &Form, r: [f32; 4], p: &mut dyn Painter) {
        let k = f.scale_k();
        let font = f.font();
        let ink = Rgba(0, 0, 0, 255);
        let list = self.find_state.found.clone().unwrap_or_default();
        // List box framed #828790, header row with #E5E5E5 rules, rows 21 px; the status
        // line under it.
        let lh = (101.0 * k).round();
        p.fill_rect(r[0], r[1], r[2], lh, Rgba(0x82, 0x87, 0x90, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            lh - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let heads = [
            ("sheetFn.colBook", "Book"),
            ("sheetFn.colSheet", "Sheet"),
            ("sheetFn.colName", "Name"),
            ("sheetFn.colCell", "Cell"),
            ("sheetFn.colValue", "Value"),
            ("sheetFn.colFormula", "Formula"),
        ];
        let (ox, _) = f.origin();
        let line = font.size * 1.33;
        for (i, (key, d)) in heads.iter().enumerate() {
            let x = (ox + COLS[i] * k).round();
            if i > 0 {
                p.fill_rect(
                    x - 3.0 * k,
                    r[1] + 1.0,
                    1.0,
                    (22.0 * k).round(),
                    Rgba(0xE5, 0xE5, 0xE5, 255),
                );
            }
            p.text_font(&self.t(key, d), x, r[1] + (3.0 * k).round(), &font, ink);
        }
        p.push_clip(r[0] + 1.0, r[1] + 24.0 * k, r[2] - 2.0, lh - 25.0 * k);
        let names = self.view.sheet_names();
        let rows = ((lh / k - 25.0) / 21.0).ceil() as usize;
        for (n, &(s, row, col)) in list.iter().enumerate().skip(self.find_state.top).take(rows) {
            let y = r[1] + ((24.0 + (n - self.find_state.top) as f32 * 21.0) * k).round();
            let sel = self.find_state.sel == Some(n);
            let fg = if sel {
                p.fill_rect(
                    r[0] + 1.0,
                    y,
                    r[2] - 2.0,
                    (21.0 * k).round(),
                    Rgba(0x00, 0x78, 0xD4, 255),
                );
                Rgba(255, 255, 255, 255)
            } else {
                ink
            };
            let d = self.view.engine().cell(s, row, col).ok();
            let cells = [
                self.name.clone(),
                names.get(s).cloned().flatten().unwrap_or_default(),
                String::new(),
                format!("${}${}", col_to_letters(col), row + 1),
                d.as_ref().map(|d| d.display.clone()).unwrap_or_default(),
                d.as_ref()
                    .map(|d| d.input.clone())
                    .filter(|i| i.starts_with('='))
                    .unwrap_or_default(),
            ];
            for (i, c) in cells.iter().enumerate() {
                let w = COLS.get(i + 1).map_or(660.0, |n| *n) - COLS[i] - 6.0;
                let max = (w / (font.size / k * 0.55)).max(2.0) as usize;
                let shown: String = if c.chars().count() > max {
                    c.chars().take(max - 1).chain(['…']).collect()
                } else {
                    c.clone()
                };
                p.text_font(
                    &shown,
                    (ox + COLS[i] * k).round(),
                    y + ((21.0 * k - line) / 2.0).round(),
                    &font,
                    fg,
                );
            }
        }
        p.pop_clip();
        let msg = self
            .t("sheetFind.found", "{n} cell(s) found")
            .replace("{n}", &list.len().to_string());
        p.text_font(
            &msg,
            (ox + 11.0 * k).round(),
            r[1] + lh + (5.0 * k).round(),
            &font,
            ink,
        );
    }
}
