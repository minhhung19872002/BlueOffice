//! Data and Formulas dialogs of BlueSheet on the self-drawn form (`sheet_form.rs`): Sort
//! (several levels), Data Validation (three pages), Goal Seek, Name Manager and Insert
//! Function (categories, search and argument help). Engine commands do the work
//! (`sort`, `sortRegion`, `setValidation`, `goalSeek`, `nameList` / `defineName` /
//! `deleteName`, `functionList`).
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::sheet_ribbon::FUNCTION_CATEGORIES;
use bo_model::sheet::col_to_letters;
use serde_json::{json, Value};

/// Sort levels shown at most (ids of the level label, column, sort-on and order boxes).
const LEVELS: [(&str, &str, &str, &str); 5] = [
    ("lv0", "col0", "on0", "ord0"),
    ("lv1", "col1", "on1", "ord1"),
    ("lv2", "col2", "on2", "ord2"),
    ("lv3", "col3", "on3", "ord3"),
    ("lv4", "col4", "on4", "ord4"),
];

/// Argument names of common functions for the Insert Function help (BlueOffice's own
/// wording, as the web edition's formula hint; `[x]` optional, `…` repeats).
pub(super) const FUNCTION_ARGS: [(&str, &str); 58] = [
    ("SUM", "number1, [number2], …"),
    ("AVERAGE", "number1, [number2], …"),
    ("COUNT", "value1, [value2], …"),
    ("COUNTA", "value1, [value2], …"),
    ("COUNTBLANK", "range"),
    ("MIN", "number1, [number2], …"),
    ("MAX", "number1, [number2], …"),
    ("PRODUCT", "number1, [number2], …"),
    ("IF", "test, value_if_true, [value_if_false]"),
    ("IFS", "test1, value1, [test2, value2], …"),
    ("IFERROR", "value, value_if_error"),
    ("AND", "test1, [test2], …"),
    ("OR", "test1, [test2], …"),
    ("NOT", "test"),
    ("SUMIF", "range, criteria, [sum_range]"),
    ("SUMIFS", "sum_range, criteria_range1, criteria1, …"),
    ("COUNTIF", "range, criteria"),
    ("COUNTIFS", "criteria_range1, criteria1, …"),
    ("AVERAGEIF", "range, criteria, [average_range]"),
    (
        "VLOOKUP",
        "lookup_value, table, column_index, [approximate]",
    ),
    ("HLOOKUP", "lookup_value, table, row_index, [approximate]"),
    (
        "XLOOKUP",
        "lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode]",
    ),
    ("INDEX", "array, row, [column]"),
    ("MATCH", "lookup_value, lookup_array, [match_type]"),
    ("ROUND", "number, digits"),
    ("ROUNDUP", "number, digits"),
    ("ROUNDDOWN", "number, digits"),
    ("INT", "number"),
    ("ABS", "number"),
    ("MOD", "number, divisor"),
    ("LEFT", "text, [count]"),
    ("RIGHT", "text, [count]"),
    ("MID", "text, start, count"),
    ("LEN", "text"),
    ("TRIM", "text"),
    ("UPPER", "text"),
    ("LOWER", "text"),
    ("PROPER", "text"),
    ("CONCAT", "text1, [text2], …"),
    ("CONCATENATE", "text1, [text2], …"),
    ("TEXT", "value, format"),
    ("VALUE", "text"),
    ("DATE", "year, month, day"),
    ("YEAR", "date"),
    ("MONTH", "date"),
    ("DAY", "date"),
    ("TODAY", ""),
    ("NOW", ""),
    ("DATEDIF", "start_date, end_date, unit"),
    ("NETWORKDAYS", "start_date, end_date, [holidays]"),
    ("EDATE", "start_date, months"),
    ("EOMONTH", "start_date, months"),
    ("WEEKDAY", "date, [type]"),
    ("UNIQUE", "array, [by_column], [exactly_once]"),
    ("FILTER", "array, include, [if_empty]"),
    ("SORT", "array, [sort_index], [order], [by_column]"),
    ("SUBTOTAL", "function_number, ref1, [ref2], …"),
    ("PMT", "rate, nper, pv, [fv], [type]"),
];

const ALLOW: [(&str, &str, &str); 8] = [
    ("sheetDv.any", "Any value", "none"),
    ("sheetDv.whole", "Whole number", "whole"),
    ("sheetDv.decimal", "Decimal", "decimal"),
    ("sheetDv.list", "List", "list"),
    ("sheetDv.date", "Date", "date"),
    ("sheetDv.time", "Time", "time"),
    ("sheetDv.textLength", "Text length", "textLength"),
    ("sheetDv.custom", "Custom", "custom"),
];

const OPERATORS: [(&str, &str, &str); 8] = [
    ("sheetDv.between", "between", "between"),
    ("sheetDv.notBetween", "not between", "notBetween"),
    ("sheetDv.equal", "equal to", "equal"),
    ("sheetDv.notEqual", "not equal to", "notEqual"),
    ("sheetDv.greater", "greater than", "greaterThan"),
    ("sheetDv.less", "less than", "lessThan"),
    (
        "sheetDv.greaterEqual",
        "greater than or equal to",
        "greaterThanOrEqual",
    ),
    (
        "sheetDv.lessEqual",
        "less than or equal to",
        "lessThanOrEqual",
    ),
];

impl SheetWindow {
    fn edit(value: &str) -> Kind {
        Kind::Edit {
            value: value.chars().collect(),
            caret: value.chars().count(),
        }
    }

    fn button(label: String) -> Kind {
        Kind::Button {
            label,
            default: false,
        }
    }

    // ---------------------------------------------------------------- Sort

    /// Open the Sort dialog over the selection's region.
    pub(super) fn open_sort_dialog(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let sheet = self.view.sheet();
        let range = self.view.rng();
        let region = self
            .view
            .query(json!({ "cmd": "sortRegion", "sheet": sheet, "range": range }))
            .unwrap_or(Value::Null);
        let r = region.get("range").cloned().unwrap_or(range);
        let header = region
            .get("hasHeader")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let t = |k: &str, d: &str| self.t(k, d);
        // Geometry measured on the reference's Sort dialog at 125 % (round 39, `sheet-31`):
        // 767 × 344 px, a row of 34 px buttons at y 48 (icon buttons, a separator, ▲ ▼,
        // Options…, the headers box), the level grid from y 88, combo boxes 28 px high,
        // OK / Cancel 106 × 33 at y 291.
        let mut f = Form::new(&t("sheetSort.title", "Sort"), (767.0, 344.0), Vec::new());
        // The level grid: white, framed #828790, a header row with #E5E5E5 separators.
        f.add("grid", None, [16.0, 88.0, 735.0, 197.0], Kind::Canvas);
        let bx = [
            ("add", "sheetSort.add", "Add Level", 18.0, 114.0),
            ("del", "sheetSort.delete", "Delete Level", 138.0, 130.0),
            ("copy", "sheetSort.copy", "Copy Level", 274.0, 120.0),
        ];
        for (id, k, d, x, w) in bx {
            f.add(id, None, [x, 48.0, w, 34.0], Self::button(t(k, d)));
            // Our own icon left of the label (drawn over the button), as the reference.
            f.set_label_left(id, 36.0);
            f.set_access(id, 0);
        }
        f.add("tsep", None, [399.0, 48.0, 1.0, 34.0], Kind::Canvas);
        f.add(
            "up",
            None,
            [404.0, 48.0, 32.0, 34.0],
            Self::button(String::new()),
        );
        f.add(
            "down",
            None,
            [442.0, 48.0, 32.0, 34.0],
            Self::button(String::new()),
        );
        f.add(
            "opts",
            None,
            [480.0, 48.0, 84.0, 34.0],
            Self::button(t("sheetNative.sortOptions", "Options...")),
        );
        f.set_access("opts", 0);
        // Icons over their buttons (a press on one is the button's).
        for (id, rect) in [
            ("icAdd", [22.0, 53.0, 26.0, 24.0]),
            ("icDel", [142.0, 53.0, 26.0, 24.0]),
            ("icCopy", [278.0, 53.0, 26.0, 24.0]),
            ("icUp", [405.0, 49.0, 30.0, 32.0]),
            ("icDown", [443.0, 49.0, 30.0, 32.0]),
        ] {
            f.add(id, None, rect, Kind::Canvas);
        }
        let headers = t("sheetSort.headers", "My data has headers");
        let h_at = headers
            .to_lowercase()
            .chars()
            .collect::<Vec<_>>()
            .iter()
            .rposition(|c| *c == 'h')
            .unwrap_or(0);
        f.add(
            "header",
            None,
            [582.0, 55.0, 175.0, 20.0],
            Kind::Check {
                label: headers,
                on: header,
            },
        );
        f.set_access("header", h_at);
        f.add(
            "hCol",
            None,
            [23.0, 96.0, 0.0, 0.0],
            Kind::Label(t("sheetSort.column", "Column")),
        );
        f.add(
            "hOn",
            None,
            [289.0, 96.0, 0.0, 0.0],
            Kind::Label(t("sheetSort.sortOn", "Sort On")),
        );
        f.add(
            "hOrd",
            None,
            [555.0, 96.0, 0.0, 0.0],
            Kind::Label(t("sheetSort.order", "Order")),
        );
        let orders = vec![t("sheetSort.az", "A to Z"), t("sheetSort.za", "Z to A")];
        for (i, (lv, col, on, ord)) in LEVELS.iter().enumerate() {
            let y = 121.0 + i as f32 * 31.0;
            let label = if i == 0 {
                t("sheetSort.sortBy", "Sort by")
            } else {
                t("sheetSort.thenBy", "Then by")
            };
            f.add(lv, None, [20.0, y + 4.0, 0.0, 0.0], Kind::Label(label));
            f.add(
                col,
                None,
                [90.0, y, 178.0, 28.0],
                Kind::Combo {
                    items: Vec::new(),
                    sel: 0,
                },
            );
            f.add(
                on,
                None,
                [285.0, y, 250.0, 28.0],
                Kind::Combo {
                    items: vec![t("sheetSort.cellValues", "Cell Values")],
                    sel: 0,
                },
            );
            f.add(
                ord,
                None,
                [551.0, y, 196.0, 28.0],
                Kind::Combo {
                    items: orders.clone(),
                    sel: 0,
                },
            );
        }
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [522.0, 291.0, 106.0, 33.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [645.0, 291.0, 106.0, 33.0];
        }
        self.sort_range = Some(r);
        self.sort_levels = 1;
        self.sort_sel = 0;
        self.sort_case = false;
        self.sort_ltr = false;
        self.form = Some(f);
        self.form_kind = FormKind::Sort;
        self.sort_refresh();
        true
    }

    /// Key names of the sort range: an empty first entry (no key chosen yet, as the
    /// reference opens), then the header texts or "Column X" (rows: "Row N" when sorting
    /// left to right); level visibility and button states.
    fn sort_refresh(&mut self) {
        let Some(r) = self.sort_range.clone() else {
            return;
        };
        let get = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        let (r0, c0, r1, c1) = (get("r0"), get("c0"), get("r1"), get("c1"));
        let header = self.form.as_ref().is_some_and(|f| f.on("header"));
        let names: Vec<String> = if self.sort_ltr {
            let row = self.t("sheetNative.sortRowN", "Row");
            std::iter::once(String::new())
                .chain((r0..=r1.min(r0 + 200)).map(|r| format!("{row} {}", r + 1)))
                .collect()
        } else {
            let column = self.t("sheetSort.columnN", "Column");
            std::iter::once(String::new())
                .chain((c0..=c1.min(c0 + 200)).map(|c| {
                    let label = if header {
                        self.view
                            .engine()
                            .cell(self.view.sheet(), r0, c)
                            .map(|d| d.display)
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    if label.trim().is_empty() {
                        format!("{column} {}", col_to_letters(c))
                    } else {
                        label
                    }
                }))
                .collect()
        };
        let col_head = if self.sort_ltr {
            self.t("sheetNative.sortRow", "Row")
        } else {
            self.t("sheetSort.column", "Column")
        };
        let levels = self.sort_levels;
        let sel = self.sort_sel;
        let Some(f) = self.form.as_mut() else {
            return;
        };
        if let Some(Kind::Label(l)) = f.get_mut("hCol").map(|c| &mut c.kind) {
            *l = col_head;
        }
        for (i, (lv, col, on, ord)) in LEVELS.iter().enumerate() {
            let shown = i < levels;
            for id in [lv, col, on, ord] {
                f.show(id, shown);
            }
            let same = matches!(f.get(col).map(|c| &c.kind), Some(Kind::Combo { items, .. }) if *items == names);
            if !same {
                let s = f.sel(col).unwrap_or(0).min(names.len().saturating_sub(1));
                f.set_items(col, names.clone(), Some(s));
            }
        }
        f.enable("del", levels > 0);
        f.enable("add", levels < LEVELS.len());
        f.enable("copy", levels > 0 && levels < LEVELS.len());
        f.enable("up", sel > 0);
        f.enable("down", sel + 1 < levels);
    }

    /// Swap sort levels `a` and `b` (their key, sort-on and order choices).
    fn sort_swap(&mut self, a: usize, b: usize) {
        let Some(f) = self.form.as_mut() else {
            return;
        };
        let (_, ac, ao, ad) = LEVELS[a];
        let (_, bc, bo, bd) = LEVELS[b];
        for (x, y) in [(ac, bc), (ao, bo), (ad, bd)] {
            let (sx, sy) = (f.sel(x), f.sel(y));
            f.set_sel(x, sy);
            f.set_sel(y, sx);
        }
    }

    fn sort_event(&mut self, ev: &Event) {
        // A press on an icon is a press on its button.
        let ev = match ev {
            Event::Canvas("icAdd", ..) => &Event::Clicked("add"),
            Event::Canvas("icDel", ..) => &Event::Clicked("del"),
            Event::Canvas("icCopy", ..) => &Event::Clicked("copy"),
            Event::Canvas("icUp", ..) => &Event::Clicked("up"),
            Event::Canvas("icDown", ..) => &Event::Clicked("down"),
            other => other,
        };
        match ev {
            Event::Clicked("add") if self.sort_levels < LEVELS.len() => {
                if let Some(f) = self.form.as_mut() {
                    let (_, c, o, d) = LEVELS[self.sort_levels];
                    f.set_sel(c, Some(0));
                    f.set_sel(o, Some(0));
                    f.set_sel(d, Some(0));
                }
                self.sort_sel = self.sort_levels;
                self.sort_levels += 1;
                self.sort_refresh();
            }
            Event::Clicked("copy") if self.sort_levels < LEVELS.len() => {
                // The copy goes right under the chosen level.
                let n = self.sort_levels;
                let at = self.sort_sel.min(n - 1);
                if let Some(f) = self.form.as_mut() {
                    for k in (at + 1..n).rev() {
                        let (_, pc, po, pd) = LEVELS[k];
                        let (_, nc, no, nd) = LEVELS[k + 1];
                        for (x, y) in [(pc, nc), (po, no), (pd, nd)] {
                            let v = f.sel(x);
                            f.set_sel(y, v);
                        }
                    }
                    let (_, pc, po, pd) = LEVELS[at];
                    let (_, nc, no, nd) = LEVELS[at + 1];
                    for (x, y) in [(pc, nc), (po, no), (pd, nd)] {
                        let v = f.sel(x);
                        f.set_sel(y, v);
                    }
                }
                self.sort_levels += 1;
                self.sort_sel = at + 1;
                self.sort_refresh();
            }
            Event::Clicked("del") if self.sort_levels > 0 => {
                // The chosen level goes; the ones under it move up.
                let n = self.sort_levels;
                let at = self.sort_sel.min(n - 1);
                if let Some(f) = self.form.as_mut() {
                    for k in at..n - 1 {
                        let (_, pc, po, pd) = LEVELS[k];
                        let (_, nc, no, nd) = LEVELS[k + 1];
                        for (x, y) in [(nc, pc), (no, po), (nd, pd)] {
                            let v = f.sel(x);
                            f.set_sel(y, v);
                        }
                    }
                }
                self.sort_levels -= 1;
                self.sort_sel = at.min(self.sort_levels.saturating_sub(1));
                self.sort_refresh();
            }
            Event::Clicked("up") if self.sort_sel > 0 => {
                self.sort_swap(self.sort_sel, self.sort_sel - 1);
                self.sort_sel -= 1;
                self.sort_refresh();
            }
            Event::Clicked("down") if self.sort_sel + 1 < self.sort_levels => {
                self.sort_swap(self.sort_sel, self.sort_sel + 1);
                self.sort_sel += 1;
                self.sort_refresh();
            }
            Event::Clicked("opts") => self.open_sort_options(),
            Event::Canvas("grid", _, y) => {
                // A press on a level's row chooses it.
                let row = ((y - 33.0) / 31.0).floor();
                if row >= 0.0 && (row as usize) < self.sort_levels {
                    self.sort_sel = row as usize;
                    self.sort_refresh();
                }
            }
            Event::Changed(id)
                if id.len() > 2
                    && LEVELS
                        .iter()
                        .any(|l| l.1 == *id || l.2 == *id || l.3 == *id) =>
            {
                if let Some(i) = LEVELS
                    .iter()
                    .position(|l| l.1 == *id || l.2 == *id || l.3 == *id)
                {
                    self.sort_sel = i;
                    self.sort_refresh();
                }
            }
            Event::Changed("header") => self.sort_refresh(),
            Event::Ok | Event::Clicked("ok") => {
                let Some(r) = self.sort_range.clone() else {
                    return;
                };
                let first = if self.sort_ltr {
                    r.get("r0")
                } else {
                    r.get("c0")
                }
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;
                let case = self.sort_case;
                // Levels without a key are skipped (the reference asks for one).
                let keys: Vec<Value> = LEVELS
                    .iter()
                    .take(self.sort_levels)
                    .filter_map(|(_, col, _, ord)| {
                        let f = self.form.as_ref()?;
                        let k = f.sel(col).unwrap_or(0);
                        (k > 0).then(|| {
                            json!({
                                "col": first + k as u32 - 1,
                                "descending": f.sel(ord) == Some(1),
                                "caseSensitive": case,
                            })
                        })
                    })
                    .collect();
                if keys.is_empty() {
                    self.status = self.t(
                        "sheetNative.sortNoKey",
                        "Choose a column to sort by in each level.",
                    );
                    return;
                }
                let header = self.form.as_ref().is_some_and(|f| f.on("header"));
                let sheet = self.view.sheet();
                self.view.run(json!({
                    "cmd": "sort", "sheet": sheet, "range": r, "keys": keys,
                    "hasHeader": header && !self.sort_ltr, "leftToRight": self.sort_ltr,
                }));
                self.form = None;
            }
            Event::Cancel | Event::Clicked("cancel") => self.form = None,
            _ => {}
        }
    }

    /// Sort Options over the Sort dialog: case sensitive, top to bottom / left to right.
    fn open_sort_options(&mut self) {
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetNative.sortOptionsTitle", "Sort Options"),
            (300.0, 262.0),
            Vec::new(),
        );
        f.add(
            "case",
            None,
            [20.0, 52.0, 260.0, 24.0],
            Kind::Check {
                label: t("sheetNative.sortCase", "Case sensitive"),
                on: self.sort_case,
            },
        );
        f.set_access("case", 0);
        f.add(
            "orient",
            None,
            [20.0, 92.0, 260.0, 100.0],
            Kind::Group(t("sheetNative.sortOrientation", "Orientation")),
        );
        f.add(
            "ttb",
            None,
            [36.0, 120.0, 230.0, 24.0],
            Kind::Check {
                label: t("sheetNative.sortTopBottom", "Sort top to bottom"),
                on: !self.sort_ltr,
            },
        );
        f.add(
            "ltr",
            None,
            [36.0, 150.0, 230.0, 24.0],
            Kind::Check {
                label: t("sheetNative.sortLeftRight", "Sort left to right"),
                on: self.sort_ltr,
            },
        );
        f.set_radios(&["ttb", "ltr"]);
        f.set_access("ttb", 5);
        f.set_access("ltr", 5);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [44.0, 212.0, 112.0, 30.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [168.0, 212.0, 112.0, 30.0];
        }
        if let Some(main) = self.form.replace(f) {
            self.form_stash = Some((main, self.form_kind));
        }
        self.form_kind = FormKind::SortOptions;
    }

    /// Events of Sort Options.
    pub(super) fn sort_options_event(&mut self, ev: &Event) {
        match ev {
            Event::Changed(id @ ("ttb" | "ltr")) => {
                if let Some(f) = self.form.as_mut() {
                    f.set_on("ttb", *id == "ttb");
                    f.set_on("ltr", *id == "ltr");
                }
            }
            Event::Ok | Event::Cancel | Event::Clicked("ok" | "cancel") => {
                let apply = matches!(ev, Event::Ok | Event::Clicked("ok"));
                if apply {
                    if let Some(f) = self.form.as_ref() {
                        let ltr = f.on("ltr");
                        self.sort_case = f.on("case");
                        if ltr != self.sort_ltr {
                            self.sort_ltr = ltr;
                            // Keys change meaning (columns ↔ rows): start over.
                            if let Some((main, _)) = self.form_stash.as_mut() {
                                for (_, c, _, _) in LEVELS {
                                    main.set_sel(c, Some(0));
                                }
                            }
                        }
                    }
                }
                if let Some((f, k)) = self.form_stash.take() {
                    self.form = Some(f);
                    self.form_kind = k;
                    self.sort_refresh();
                } else {
                    self.form = None;
                }
            }
            _ => {}
        }
    }

    /// Owner-drawn parts of the Sort dialog: the level grid with the chosen level's label
    /// shaded, the toolbar separator and our own button icons.
    pub(super) fn paint_sort_canvas(
        &self,
        f: &Form,
        id: &str,
        r: [f32; 4],
        p: &mut dyn crate::Painter,
    ) {
        use bo_render::{PathSeg, Rgba};
        let k = self.scale / 1.25;
        let on = |b: &str| f.get(b).is_some_and(|c| c.enabled);
        let ink = |enabled: bool| {
            if enabled {
                Rgba(0x30, 0x30, 0x30, 255)
            } else {
                Rgba(0xA0, 0xA0, 0xA0, 255)
            }
        };
        let accent = Rgba(0x1E, 0x7B, 0xC4, 255);
        match id {
            "grid" => {
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
                if self.sort_levels > 0 {
                    p.fill_rect(
                        r[0] + 1.0,
                        (hy + 1.0 + self.sort_sel as f32 * 31.0 * k).round(),
                        (72.0 * k).round(),
                        (30.0 * k).round(),
                        Rgba(0xE0, 0xDF, 0xE3, 255),
                    );
                }
            }
            "tsep" => p.fill_rect(
                r[0],
                r[1],
                1.0_f32.max(k.round()),
                r[3],
                Rgba(0x40, 0x40, 0x40, 255),
            ),
            "icAdd" => {
                // A small list with a plus.
                let c = ink(on("add"));
                for i in 0..3 {
                    let y = r[1] + (6.0 + 5.0 * i as f32) * k;
                    p.fill_rect(r[0] + 9.0 * k, y.round(), 13.0 * k, (1.5 * k).max(1.0), c);
                }
                p.fill_rect(
                    r[0] + 2.0 * k,
                    r[1] + 5.0 * k,
                    6.0 * k,
                    (1.5 * k).max(1.0),
                    accent,
                );
                p.fill_rect(
                    r[0] + 4.25 * k,
                    r[1] + 2.75 * k,
                    (1.5 * k).max(1.0),
                    6.0 * k,
                    accent,
                );
            }
            "icDel" => {
                // A cross.
                let c = ink(on("del"));
                let (x0, y0, s) = (r[0] + 5.0 * k, r[1] + 4.0 * k, 16.0 * k);
                p.path(
                    &[
                        PathSeg::M(x0, y0),
                        PathSeg::L(x0 + s, y0 + s),
                        PathSeg::M(x0 + s, y0),
                        PathSeg::L(x0, y0 + s),
                    ],
                    None,
                    Some((c, 2.0 * k)),
                );
            }
            "icCopy" => {
                // Two overlapping sheets.
                let c = ink(on("copy"));
                for (dx, dy) in [(3.0, 3.0), (9.0, 8.0)] {
                    let (x, y, w, h) = (r[0] + dx * k, r[1] + dy * k, 12.0 * k, 13.0 * k);
                    p.fill_rect(x, y, w, h, c);
                    p.fill_rect(
                        x + 1.0,
                        y + 1.0,
                        w - 2.0,
                        h - 2.0,
                        Rgba(0xFD, 0xFD, 0xFD, 255),
                    );
                }
                p.fill_rect(r[0] + 12.0 * k, r[1] + 13.0 * k, 6.0 * k, 1.0_f32.max(k), c);
                p.fill_rect(r[0] + 12.0 * k, r[1] + 16.0 * k, 6.0 * k, 1.0_f32.max(k), c);
            }
            "icUp" | "icDown" => {
                let c = ink(on(if id == "icUp" { "up" } else { "down" }));
                let (cx, cy, h) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, 5.0 * k);
                let segs = if id == "icUp" {
                    [
                        PathSeg::M(cx - h * 1.4, cy + h * 0.6),
                        PathSeg::L(cx + h * 1.4, cy + h * 0.6),
                        PathSeg::L(cx, cy - h * 0.8),
                        PathSeg::Z,
                    ]
                } else {
                    [
                        PathSeg::M(cx - h * 1.4, cy - h * 0.6),
                        PathSeg::L(cx + h * 1.4, cy - h * 0.6),
                        PathSeg::L(cx, cy + h * 0.8),
                        PathSeg::Z,
                    ]
                };
                p.path(&segs, Some(c), None);
            }
            _ => {}
        }
    }

    // ---------------------------------------------------------------- Data Validation

    /// Open Data Validation for the selection (the active cell's rule filled in).
    pub(super) fn open_validation_dialog(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let (r, c) = self.view.active();
        let sheet = self.view.sheet();
        let now = self
            .view
            .query(json!({ "cmd": "validation", "sheet": sheet, "row": r, "col": c }))
            .unwrap_or(Value::Null);
        let s = |k: &str| now.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let b = |k: &str, d: bool| now.get(k).and_then(Value::as_bool).unwrap_or(d);
        self.dv_sqref = now
            .get("sqref")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let t = |k: &str, d: &str| self.t(k, d);
        let tabs = vec![
            t("sheetDv.settings", "Settings"),
            t("sheetDv.inputMessage", "Input Message"),
            t("sheetDv.errorAlert", "Error Alert"),
        ];
        // Window 604 × 420 px at 125 % as the reference's (round 45, `show/sheet-41`).
        let mut f = Form::new(&t("sheetDv.title", "Data Validation"), (604.0, 420.0), tabs);
        let p = Some(0);
        f.add(
            "crit",
            p,
            [32.0, 85.0, 540.0, 22.0],
            Kind::Group(t("sheetDv.criteria", "Validation criteria")),
        );
        f.add(
            "allowL",
            p,
            [46.0, 112.0, 120.0, 22.0],
            Kind::Label(t("sheetDv.allow", "Allow:")),
        );
        f.set_access("allowL", 0);
        let kind = s("kind");
        f.add(
            "allow",
            p,
            [46.0, 134.0, 185.0, 28.0],
            Kind::Combo {
                items: ALLOW.iter().map(|(k, d, _)| t(k, d)).collect(),
                sel: ALLOW.iter().position(|(_, _, v)| *v == kind).unwrap_or(0),
            },
        );
        f.add(
            "blank",
            p,
            [245.0, 135.0, 200.0, 22.0],
            Kind::Check {
                label: t("sheetDv.ignoreBlank", "Ignore blank"),
                on: b("allowBlank", true),
            },
        );
        f.set_access("blank", 7);
        f.add(
            "dropdown",
            p,
            [245.0, 160.0, 200.0, 22.0],
            Kind::Check {
                label: t("sheetDv.inCellDropdown", "In-cell dropdown"),
                on: b("inCellDropdown", true),
            },
        );
        f.set_access("dropdown", 8);
        f.add(
            "dataL",
            p,
            [46.0, 165.0, 120.0, 22.0],
            Kind::Label(t("sheetDv.data", "Data:")),
        );
        f.set_access("dataL", 0);
        let op = s("operator");
        f.add(
            "op",
            p,
            [46.0, 187.0, 185.0, 28.0],
            Kind::Combo {
                items: OPERATORS.iter().map(|(k, d, _)| t(k, d)).collect(),
                sel: OPERATORS.iter().position(|(_, _, v)| *v == op).unwrap_or(0),
            },
        );
        f.add(
            "f1L",
            p,
            [46.0, 218.0, 300.0, 22.0],
            Kind::Label(t("sheetDv.minimum", "Minimum:")),
        );
        f.set_access("f1L", 1);
        f.add(
            "f1",
            p,
            [46.0, 240.0, 352.0, 26.0],
            Self::edit(&s("formula1")),
        );
        f.add("pick1", p, [397.0, 240.0, 26.0, 26.0], Kind::Canvas);
        f.add(
            "f2L",
            p,
            [46.0, 269.0, 300.0, 22.0],
            Kind::Label(t("sheetDv.maximum", "Maximum:")),
        );
        f.set_access("f2L", 1);
        f.add(
            "f2",
            p,
            [46.0, 291.0, 352.0, 26.0],
            Self::edit(&s("formula2")),
        );
        f.add("pick2", p, [397.0, 291.0, 26.0, 26.0], Kind::Canvas);
        f.add(
            "apply",
            p,
            [32.0, 327.0, 540.0, 22.0],
            Kind::Check {
                label: t(
                    "sheetDv.applyAll",
                    "Apply these changes to all other cells with the same settings",
                ),
                on: false,
            },
        );
        f.set_access("apply", 1);
        let p = Some(1);
        f.add(
            "showIn",
            p,
            [32.0, 91.0, 520.0, 22.0],
            Kind::Check {
                label: t(
                    "sheetDv.showInput",
                    "Show input message when cell is selected",
                ),
                on: b("showInput", true),
            },
        );
        f.set_access("showIn", 0);
        f.add(
            "inG",
            p,
            [32.0, 122.0, 540.0, 22.0],
            Kind::Group(t(
                "sheetDv.whenSelected",
                "When cell is selected, show this input message:",
            )),
        );
        f.add(
            "inTitleL",
            p,
            [46.0, 147.0, 120.0, 22.0],
            Kind::Label(t("sheetDv.titleLabel", "Title:")),
        );
        f.set_access("inTitleL", 0);
        f.add(
            "inTitle",
            p,
            [46.0, 169.0, 330.0, 26.0],
            Self::edit(&s("promptTitle")),
        );
        f.add(
            "inMsgL",
            p,
            [46.0, 201.0, 200.0, 22.0],
            Kind::Label(t("sheetDv.message", "Input message:")),
        );
        f.set_access("inMsgL", 0);
        f.add(
            "inMsg",
            p,
            [46.0, 223.0, 510.0, 120.0],
            Self::edit(&s("prompt")),
        );
        let p = Some(2);
        f.add(
            "showErr",
            p,
            [32.0, 91.0, 520.0, 22.0],
            Kind::Check {
                label: t(
                    "sheetDv.showError",
                    "Show error alert after invalid data is entered",
                ),
                on: b("showError", true),
            },
        );
        f.set_access("showErr", 0);
        f.add(
            "errG",
            p,
            [32.0, 122.0, 540.0, 22.0],
            Kind::Group(t(
                "sheetDv.whenInvalid",
                "When user enters invalid data, show this error alert:",
            )),
        );
        f.add(
            "styleL",
            p,
            [46.0, 147.0, 120.0, 22.0],
            Kind::Label(t("sheetDv.style", "Style:")),
        );
        f.set_access("styleL", 1);
        let style = s("errorStyle");
        f.add(
            "estyle",
            p,
            [46.0, 169.0, 150.0, 28.0],
            Kind::Combo {
                items: vec![
                    t("sheetDv.stop", "Stop"),
                    t("sheetDv.warning", "Warning"),
                    t("sheetDv.information", "Information"),
                ],
                sel: ["stop", "warning", "information"]
                    .iter()
                    .position(|v| *v == style)
                    .unwrap_or(0),
            },
        );
        f.add("eicon", p, [70.0, 215.0, 64.0, 64.0], Kind::Canvas);
        f.add(
            "eTitleL",
            p,
            [222.0, 147.0, 120.0, 22.0],
            Kind::Label(t("sheetDv.titleLabel", "Title:")),
        );
        f.set_access("eTitleL", 0);
        f.add(
            "eTitle",
            p,
            [222.0, 169.0, 334.0, 26.0],
            Self::edit(&s("errorTitle")),
        );
        f.add(
            "eMsgL",
            p,
            [222.0, 201.0, 200.0, 22.0],
            Kind::Label(t("sheetDv.errorMessage", "Error message:")),
        );
        f.set_access("eMsgL", 0);
        f.add(
            "eMsg",
            p,
            [222.0, 223.0, 334.0, 120.0],
            Self::edit(&s("error")),
        );
        f.add(
            "clear",
            None,
            [18.0, 372.0, 111.0, 28.0],
            Self::button(t("sheetDv.clearAll", "Clear All")),
        );
        f.set_access("clear", 0);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [346.0, 372.0, 111.0, 28.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [473.0, 372.0, 111.0, 28.0];
        }
        f.focus("allow");
        self.form = Some(f);
        self.form_kind = FormKind::Validation;
        self.validation_refresh();
        true
    }

    /// Settings page as the reference lays it out for the chosen Allow / Data: the Data box
    /// greyed (Any value, List, Custom), the value boxes and their captions, Ignore blank
    /// greyed for Any value, In-cell dropdown for lists, Apply… only when the cell has a
    /// rule.
    pub(super) fn validation_refresh(&mut self) {
        let labels = [
            self.t("sheetDv.minimum", "Minimum:"),
            self.t("sheetDv.maximum", "Maximum:"),
            self.t("sheetDv.source", "Source:"),
            self.t("sheetDv.formula", "Formula:"),
            self.t("sheetDv.value", "Value:"),
            self.t("sheetDv.startDate", "Start date:"),
            self.t("sheetDv.endDate", "End date:"),
            self.t("sheetDv.dateLabel", "Date:"),
            self.t("sheetDv.startTime", "Start time:"),
            self.t("sheetDv.endTime", "End time:"),
            self.t("sheetDv.timeLabel", "Time:"),
            self.t("sheetDv.length", "Length:"),
        ];
        let has_rule = !self.dv_sqref.is_empty();
        let Some(f) = self.form.as_mut() else {
            return;
        };
        let allow = f.sel("allow").unwrap_or(0);
        let op = f.sel("op").unwrap_or(0);
        let ranged = matches!(allow, 1 | 2 | 4 | 5 | 6);
        let list = allow == 3;
        // Data stays visible, greyed when it does not apply (Any value).
        f.show("dataL", allow == 0 || ranged);
        f.show("op", allow == 0 || ranged);
        f.enable("dataL", ranged);
        f.enable("op", ranged);
        f.enable("blank", allow > 0);
        f.show("dropdown", list);
        let one = allow > 0;
        let two = ranged && op < 2;
        // List / Custom: the value box sits where Data is.
        let y1 = if ranged { 240.0 } else { 187.0 };
        for (id, y) in [("f1L", y1 - 22.0), ("f1", y1), ("pick1", y1)] {
            f.show(id, one);
            if let Some(c) = f.get_mut(id) {
                c.rect[1] = y;
            }
        }
        for id in ["f2L", "f2", "pick2"] {
            f.show(id, two);
        }
        let (first, second) = match (allow, two) {
            (3, _) => (labels[2].clone(), String::new()),
            (7, _) => (labels[3].clone(), String::new()),
            (4, true) => (labels[5].clone(), labels[6].clone()),
            (4, false) => (labels[7].clone(), String::new()),
            (5, true) => (labels[8].clone(), labels[9].clone()),
            (5, false) => (labels[10].clone(), String::new()),
            (_, true) => (labels[0].clone(), labels[1].clone()),
            (6, false) => (labels[11].clone(), String::new()),
            (_, false) => match op {
                4 | 6 => (labels[0].clone(), String::new()),
                5 | 7 => (labels[1].clone(), String::new()),
                _ => (labels[4].clone(), String::new()),
            },
        };
        if let Some(Kind::Label(s)) = f.get_mut("f1L").map(|c| &mut c.kind) {
            *s = first;
        }
        if !second.is_empty() {
            if let Some(Kind::Label(s)) = f.get_mut("f2L").map(|c| &mut c.kind) {
                *s = second;
            }
        }
        f.enable("apply", has_rule);
        let input = f.on("showIn");
        for id in ["inTitleL", "inTitle", "inMsgL", "inMsg"] {
            f.enable(id, input);
        }
        let err = f.on("showErr");
        for id in ["styleL", "estyle", "eTitleL", "eTitle", "eMsgL", "eMsg"] {
            f.enable(id, err);
        }
    }

    fn validation_event(&mut self, ev: &Event) {
        match ev {
            Event::Changed("allow" | "op" | "showIn" | "showErr") => self.validation_refresh(),
            Event::Clicked("clear") => {
                if let Some(f) = self.form.as_mut() {
                    f.set_sel("allow", Some(0));
                    f.set_sel("op", Some(0));
                    for id in ["f1", "f2", "inTitle", "inMsg", "eTitle", "eMsg"] {
                        f.set_text(id, "");
                    }
                    f.set_on("blank", true);
                    f.set_on("showIn", true);
                    f.set_on("showErr", true);
                    f.set_sel("estyle", Some(0));
                }
                self.validation_refresh();
            }
            Event::Ok => {
                let Some(f) = self.form.as_ref() else {
                    return;
                };
                let allow = ALLOW[f.sel("allow").unwrap_or(0).min(7)].2;
                let op = OPERATORS[f.sel("op").unwrap_or(0).min(7)].2;
                let opt = |id: &str| {
                    let v = f.text(id);
                    (!v.trim().is_empty()).then_some(v)
                };
                let style = ["stop", "warning", "information"][f.sel("estyle").unwrap_or(0).min(2)];
                let mut spec = json!({
                    "kind": allow,
                    "formula1": f.text("f1"),
                    "allowBlank": f.on("blank"),
                    "showInput": f.on("showIn"),
                    "showError": f.on("showErr"),
                    "inCellDropdown": f.on("dropdown"),
                    "errorStyle": style,
                });
                if matches!(allow, "whole" | "decimal" | "date" | "time" | "textLength") {
                    spec["operator"] = json!(op);
                    if let Some(v) = opt("f2").filter(|_| op == "between" || op == "notBetween") {
                        spec["formula2"] = json!(v);
                    }
                }
                for (id, key) in [
                    ("inTitle", "promptTitle"),
                    ("inMsg", "prompt"),
                    ("eTitle", "errorTitle"),
                    ("eMsg", "error"),
                ] {
                    if let Some(v) = opt(id) {
                        spec[key] = json!(v);
                    }
                }
                // Apply these changes to all other cells with the same settings: the
                // rule's other ranges change in the same undo step.
                let also: Vec<Value> = if f.on("apply") {
                    self.dv_sqref.clone()
                } else {
                    Vec::new()
                };
                let sheet = self.view.sheet();
                let range = self.view.rng();
                self.view.run(json!({
                    "cmd": "setValidation", "sheet": sheet, "range": range, "spec": spec,
                    "also": also,
                }));
                self.form = None;
            }
            Event::Cancel => self.form = None,
            _ => {}
        }
    }

    /// The Error Alert page's style icon (own drawings: a red disc with a white bar for
    /// Stop, an amber triangle with "!" for Warning, a blue disc with "i" for Information).
    pub(super) fn paint_validation_icon(f: &Form, r: [f32; 4], p: &mut dyn crate::Painter) {
        use bo_render::{PathSeg, Rgba};
        let k = f.scale_k();
        let style = f.sel("estyle").unwrap_or(0);
        let (cx, cy) = ((r[0] + r[2] / 2.0).round(), (r[1] + r[3] / 2.0).round());
        let rad = 22.0 * k;
        let white = Rgba(255, 255, 255, 255);
        let circle = |p: &mut dyn crate::Painter, fill: Rgba| {
            // Polygon approximation of a disc.
            let mut segs = Vec::with_capacity(34);
            for i in 0..32 {
                let a = i as f32 / 32.0 * std::f32::consts::TAU;
                let (x, y) = (cx + rad * a.cos(), cy + rad * a.sin());
                segs.push(if i == 0 {
                    PathSeg::M(x, y)
                } else {
                    PathSeg::L(x, y)
                });
            }
            segs.push(PathSeg::Z);
            p.path(&segs, Some(fill), None);
        };
        match style {
            0 => {
                circle(p, Rgba(0xC4, 0x2B, 0x1C, 255));
                p.fill_rect(cx - 12.0 * k, cy - 3.0 * k, 24.0 * k, 6.0 * k, white);
            }
            1 => {
                p.path(
                    &[
                        PathSeg::M(cx, cy - rad),
                        PathSeg::L(cx + rad, cy + rad * 0.8),
                        PathSeg::L(cx - rad, cy + rad * 0.8),
                        PathSeg::Z,
                    ],
                    Some(Rgba(0xF2, 0xB1, 0x1A, 255)),
                    None,
                );
                let ink = Rgba(0, 0, 0, 255);
                p.fill_rect(cx - 2.0 * k, cy - 8.0 * k, 4.0 * k, 14.0 * k, ink);
                p.fill_rect(cx - 2.0 * k, cy + 9.0 * k, 4.0 * k, 4.0 * k, ink);
            }
            _ => {
                circle(p, Rgba(0x1F, 0x6F, 0xC5, 255));
                p.fill_rect(cx - 2.0 * k, cy - 4.0 * k, 4.0 * k, 16.0 * k, white);
                p.fill_rect(cx - 2.0 * k, cy - 12.0 * k, 4.0 * k, 4.0 * k, white);
            }
        }
    }

    // ------------------------------------------------------- Circle Invalid Data

    /// Data › Data Validation › Circle Invalid Data: red ellipses round the cells that break
    /// their rule (engine `invalidCells`), until Clear Validation Circles or an edit of the
    /// circled cell.
    pub(super) fn circle_invalid(&mut self) -> bool {
        let sheet = self.view.sheet();
        let out = self
            .view
            .query(json!({ "cmd": "invalidCells", "sheet": sheet }))
            .unwrap_or(Value::Null);
        self.invalid_circles = out
            .get("cells")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|c| {
                        let c = c.as_array()?;
                        Some((c.first()?.as_u64()? as u32, c.get(1)?.as_u64()? as u32))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if self.invalid_circles.is_empty() {
            self.status = self.t("sheetDv.noInvalid", "No invalid data was found.");
        }
        true
    }

    // ---------------------------------------------------------------- Goal Seek

    /// Open Goal Seek (What-If Analysis) with the active cell as the formula cell.
    pub(super) fn open_goal_seek(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let (r, c) = self.view.active();
        let here = format!("{}{}", col_to_letters(c), r + 1);
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetGoal.title", "Goal Seek"),
            (420.0, 250.0),
            Vec::new(),
        );
        for (i, (id, k, d, v)) in [
            ("set", "sheetGoal.setCell", "Set cell:", here.as_str()),
            ("to", "sheetGoal.toValue", "To value:", ""),
            ("by", "sheetGoal.byChanging", "By changing cell:", ""),
        ]
        .iter()
        .enumerate()
        {
            let y = 56.0 + i as f32 * 40.0;
            let lid = ["setL", "toL", "byL"][i];
            f.add(lid, None, [20.0, y + 4.0, 0.0, 0.0], Kind::Label(t(k, d)));
            f.add(id, None, [170.0, y, 230.0, 26.0], Self::edit(v));
        }
        f.add(
            "status",
            None,
            [20.0, 178.0, 0.0, 0.0],
            Kind::Note(String::new()),
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("to");
        self.form = Some(f);
        self.form_kind = FormKind::GoalSeek;
        true
    }

    fn goal_event(&mut self, ev: &Event) {
        match ev {
            Event::Ok => {
                let Some(f) = self.form.as_ref() else {
                    return;
                };
                let goal = f.text("to").trim().replace(',', ".").parse::<f64>();
                let (target, changing) = (f.text("set"), f.text("by"));
                let Ok(goal) = goal else {
                    let msg = self.t("sheetGoal.needNumber", "To value must contain a number.");
                    self.goal_status(msg);
                    return;
                };
                let sheet = self.view.sheet();
                let out = self.view.query(json!({
                    "cmd": "goalSeek", "sheet": sheet, "target": target.trim(), "goal": goal,
                    "changing": changing.trim(), "apply": true,
                }));
                self.view.changed();
                let found = out
                    .as_ref()
                    .and_then(|o| o.get("found"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if found {
                    self.form = None;
                } else {
                    let msg = self.t(
                        "sheetGoal.notFound",
                        "Goal Seek may not have found a solution.",
                    );
                    self.goal_status(msg);
                }
            }
            Event::Cancel => self.form = None,
            _ => {}
        }
    }

    fn goal_status(&mut self, msg: String) {
        if let Some(Kind::Note(s)) = self
            .form
            .as_mut()
            .and_then(|f| f.get_mut("status"))
            .map(|c| &mut c.kind)
        {
            *s = msg;
        }
    }

    // ---------------------------------------------------------------- Name Manager

    // ---------------------------------------------------------------- Insert Function

    /// Open Insert Function: search, category, function list and its arguments.
    pub(super) fn open_insert_function(&mut self) -> bool {
        if !self.view.editing() {
            let (r, c) = self.view.active();
            let input = self
                .view
                .engine()
                .cell(self.view.sheet(), r, c)
                .map(|d| d.input)
                .unwrap_or_default();
            if let Some((name, args)) = super::fnargs_impl::split_call(&input) {
                return self.open_fn_args(&name, args);
            }
        }
        let t = |k: &str, d: &str| self.t(k, d);
        // Geometry of the reference's Insert Function at 125 % (round 39, `sheet-35`):
        // 645 × 483 px, a two-line search box, the category box right of its label, a
        // 7-row function list, signature and description under it, OK / Cancel 111 × 30.
        let mut f = Form::new(
            &t("sheetFn.title", "Insert Function"),
            (645.0, 483.0),
            Vec::new(),
        );
        f.add(
            "searchL",
            None,
            [18.0, 48.0, 0.0, 0.0],
            Kind::Label(t("sheetFn.search", "Search for a function:")),
        );
        f.add("search", None, [32.0, 73.0, 467.0, 40.0], Self::edit(""));
        f.add(
            "go",
            None,
            [514.0, 73.0, 111.0, 29.0],
            Self::button(t("sheetFn.go", "Go")),
        );
        f.add(
            "catL",
            None,
            [32.0, 122.0, 0.0, 0.0],
            Kind::Label(t("sheetFn.category", "Or select a category:")),
        );
        // Most Recently Used first (kept in the preferences), as the reference.
        let mut cats = vec![
            t("sheetNative.fnRecent", "Most Recently Used"),
            t("sheetFn.all", "All"),
        ];
        cats.extend(FUNCTION_CATEGORIES.iter().map(|(id, _)| {
            let key = format!("commands.{id}");
            t(&key, id.trim_start_matches("sheet.fn"))
        }));
        f.add(
            "cat",
            None,
            [158.0, 118.0, 341.0, 27.0],
            Kind::Combo {
                items: cats,
                sel: 0,
            },
        );
        f.add(
            "fnL",
            None,
            [18.0, 155.0, 0.0, 0.0],
            Kind::Label(t("sheetFn.select", "Select a function:")),
        );
        f.add(
            "fns",
            None,
            [32.0, 181.0, 595.0, 122.0],
            Kind::List {
                items: Vec::new(),
                sel: None,
                top: 0,
            },
        );
        f.add(
            "sig",
            None,
            [32.0, 309.0, 0.0, 0.0],
            Kind::Label(String::new()),
        );
        f.add(
            "help",
            None,
            [32.0, 330.0, 0.0, 0.0],
            Kind::Note(String::new()),
        );
        f.set_placeholder(
            "search",
            &t(
                "sheetNative.fnSearchHint",
                "Type a brief description of what you want to do and then click Go",
            ),
        );
        f.set_bold("sig");
        f.add(
            "helpLink",
            None,
            [18.0, 442.0, 250.0, 22.0],
            Kind::Label(t("sheetNative.fnHelpLink", "Help on this function")),
        );
        // No per-function reference in BlueSheet yet: shown greyed.
        f.enable("helpLink", false);
        f.set_access("searchL", 0);
        f.set_access("catL", 15);
        f.set_access("fnL", 16);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [387.0, 435.0, 111.0, 30.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [514.0, 435.0, 111.0, 30.0];
        }
        self.form = Some(f);
        self.form_kind = FormKind::Function;
        self.function_refresh();
        if let Some(f) = self.form.as_mut() {
            f.focus("search");
        }
        true
    }

    fn function_refresh(&mut self) {
        let Some(f) = self.form.as_ref() else {
            return;
        };
        let cat = f.sel("cat").unwrap_or(0);
        let query = f.text("search").trim().to_owned();
        let mut names: Vec<String> = match cat {
            0 if query.is_empty() => self.prefs.recent_functions.clone(),
            0 | 1 => self.functions.clone(),
            _ => FUNCTION_CATEGORIES
                .get(cat - 2)
                .map(|(_, n)| n.iter().map(|s| (*s).to_owned()).collect())
                .unwrap_or_default(),
        };
        if !query.is_empty() {
            // Names and descriptions, every word of the query (Go).
            let words: Vec<String> = query
                .to_lowercase()
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            names.retain(|n| {
                let hay = format!(
                    "{} {}",
                    n.to_lowercase(),
                    self.function_description(n).to_lowercase()
                );
                words.iter().all(|w| hay.contains(w.as_str()))
            });
        }
        let query = query.to_ascii_uppercase();
        let pick = names
            .iter()
            .position(|n| *n == query)
            .or((!names.is_empty()).then_some(0));
        if let Some(f) = self.form.as_mut() {
            f.set_items("fns", names, pick);
        }
        self.function_help();
    }

    fn function_help(&mut self) {
        let name = self
            .form
            .as_ref()
            .map(|f| f.text("fns"))
            .unwrap_or_default();
        let args = FUNCTION_ARGS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, a)| (*a).to_owned());
        let sig = if name.is_empty() {
            String::new()
        } else {
            format!("{name}({})", args.clone().unwrap_or_else(|| "…".into()))
        };
        let desc = self.function_description(&name);
        let help = if name.is_empty() {
            String::new()
        } else if !desc.is_empty() {
            desc
        } else if args.is_some() {
            self.t(
                "sheetFn.helpArgs",
                "Arguments in brackets are optional; … repeats the last one.",
            )
        } else {
            self.t(
                "sheetFn.helpNone",
                "Choose OK to type the arguments in the formula bar.",
            )
        };
        if let Some(f) = self.form.as_mut() {
            if let Some(Kind::Label(s)) = f.get_mut("sig").map(|c| &mut c.kind) {
                *s = sig;
            }
            if let Some(Kind::Note(s)) = f.get_mut("help").map(|c| &mut c.kind) {
                *s = help;
            }
        }
    }

    /// Our own one-line description of function `name` (`""` when there is none).
    pub(super) fn function_description(&self, name: &str) -> String {
        let key = format!("sheetNative.fnd{}", name.replace('.', "_"));
        let d = self.t(&key, "");
        if d == key {
            String::new()
        } else {
            d
        }
    }

    /// Put `name` first in the Most Recently Used list (ten names, stored in the
    /// preferences).
    fn note_recent_function(&mut self, name: &str) {
        let mut p = self.prefs.clone();
        p.recent_functions.retain(|n| n != name);
        p.recent_functions.insert(0, name.to_owned());
        p.recent_functions.truncate(10);
        self.set_prefs(p);
    }

    fn function_event(&mut self, ev: &Event, text: &dyn crate::TextMeasure) {
        match ev {
            Event::Changed("cat" | "search") | Event::Clicked("go") => self.function_refresh(),
            Event::Changed("fns") => self.function_help(),
            Event::Ok | Event::Clicked("fns") => {
                let name = self
                    .form
                    .as_ref()
                    .map(|f| f.text("fns"))
                    .unwrap_or_default();
                self.form = None;
                if !name.is_empty() {
                    self.note_recent_function(&name);
                    let _ = text;
                    self.open_fn_args(&name, Vec::new());
                }
            }
            Event::Cancel => self.form = None,
            _ => {}
        }
    }

    // ---------------------------------------------------------------- Print Titles

    fn print_titles_event(&mut self, ev: &Event) {
        match ev {
            Event::Ok => {
                let text = self
                    .form
                    .as_ref()
                    .map(|f| f.text("rows"))
                    .unwrap_or_default();
                let nums: Vec<u32> = text
                    .split(':')
                    .filter_map(|p| p.trim().trim_start_matches('$').parse::<u32>().ok())
                    .filter(|n| *n >= 1)
                    .collect();
                let rows = match nums.as_slice() {
                    [] => Value::Null,
                    [a] => json!([a - 1, a - 1]),
                    [a, b, ..] => json!([a.min(b) - 1, a.max(b) - 1]),
                };
                let sheet = self.view.sheet();
                if self
                    .view
                    .query(json!({ "cmd": "setPageSetup", "sheet": sheet, "titleRows": rows }))
                    .is_some()
                {
                    self.view.changed();
                }
                self.form = None;
            }
            Event::Cancel => self.form = None,
            _ => {}
        }
    }

    // ---------------------------------------------------------------- dispatch

    /// An event of form `kind`.
    pub(super) fn form_event(
        &mut self,
        kind: FormKind,
        ev: &Event,
        text: &dyn crate::TextMeasure,
    ) -> bool {
        match kind {
            FormKind::Sort => self.sort_event(ev),
            FormKind::SortOptions => self.sort_options_event(ev),
            FormKind::Validation => self.validation_event(ev),
            FormKind::GoalSeek => self.goal_event(ev),
            FormKind::Names => self.names_event(ev),
            FormKind::NewName => self.new_name_event(ev),
            FormKind::Function => self.function_event(ev, text),
            FormKind::PrintTitles => self.print_titles_event(ev),
            FormKind::CreateTable
            | FormKind::CreatePivot
            | FormKind::Sparklines
            | FormKind::HeaderFooter
            | FormKind::Symbol => self.insert_form_event(ev, text),
            FormKind::MoreColors => self.more_colors_event(ev),
            FormKind::CellStyle => self.cell_style_event(ev),
            FormKind::Queries
            | FormKind::Consolidate
            | FormKind::WhatIfTable
            | FormKind::CustomViews
            | FormKind::Zoom => self.data_form_event(ev, text),
            FormKind::CustomFilter | FormKind::Top10 => {
                self.filter_form_event(ev);
            }
            FormKind::Help => {
                if matches!(ev, Event::Ok | Event::Cancel) {
                    self.form = None;
                }
            }
            FormKind::ChartData
            | FormKind::ChartSeries
            | FormKind::ChartAxisLabels
            | FormKind::MoveChart => {
                self.chart_data_event(kind, ev);
            }
            FormKind::TableName | FormKind::TableResize => self.table_form_event(kind, ev),
            FormKind::PivotMove | FormKind::PivotSource => self.pivot_tab_form_event(kind, ev),
            FormKind::PivotGrouping
            | FormKind::PivotCalc
            | FormKind::PivotFieldSettings
            | FormKind::PivotOptions => self.pivot_dlg_event(kind, ev),
            FormKind::PivotFilter => self.pivot_filter_form_event(ev),
            FormKind::CfRule | FormKind::CfNewRule | FormKind::CfManager => {
                self.cf_form_event(kind, ev);
            }
            FormKind::PasteSpecial => self.paste_special_event(ev),
            FormKind::Unhide | FormKind::TabColor => self.tabs_form_event(kind, ev),
            FormKind::TableStyle => self.tstyle_event(ev),
            FormKind::Options => self.options_event(ev),
            FormKind::Evaluate => {
                self.evaluate_event(ev);
                self.layout_form(text);
            }
            FormKind::Subtotal | FormKind::RemoveDups | FormKind::AdvancedFilter => {
                self.datadlg_event(kind, ev);
            }
            FormKind::TextToColumns => self.ttc_event(ev, self.form_count),
            FormKind::TtcAdvanced => self.ttc_advanced_event(ev),
            FormKind::TtcConfirm => self.ttc_confirm_event(ev),
            FormKind::FnArgs => self.fnargs_event(ev, text),
            FormKind::ErrorCheck => self.errcheck_event(ev, text),
            FormKind::FindReplace => self.find_event(ev),
            FormKind::PivotNew => self.new_pivot_event(ev, text),
            FormKind::PivotValue => self.value_settings_event(ev),
            FormKind::RangePick => self.range_pick_event(ev),
            FormKind::Protect => self.protect_event(ev),
            FormKind::Spelling => self.spelling_event(ev),
            FormKind::Link | FormKind::LinkTip => self.link_event(ev),
            FormKind::FormatCells => {}
        }
        true
    }
}
