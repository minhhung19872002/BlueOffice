//! Formulas › Name Manager and New / Edit Name as the reference's (round 39, measured at
//! 125 %, `show/sheet-36`): New… / Edit… / Delete and Filter ▾ over a table of the names
//! (Name with a name / table mark, Value — the evaluated value, Refers To, Scope, Comment),
//! "Refers to" under it edited in place with ✗ / ✓, Close. New… and Edit… open the New Name
//! dialog (393 × 297 px: Name, Scope, Comment, Refers to).

use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::Painter;
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

/// One row of the Name Manager.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct NameRow {
    pub name: String,
    pub refers: String,
    pub value: String,
    /// `None`: the workbook; `Some(i)`: sheet `i`.
    pub scope: Option<usize>,
    pub scope_label: String,
    pub comment: String,
    /// A table (its name is a structured reference) rather than a defined name.
    pub table: bool,
}

/// Filter choices: label key, fallback.
const FILTERS: [(&str, &str); 7] = [
    ("sheetNative.nmFilter", "Filter"),
    ("sheetNative.nmScopedSheet", "Names Scoped to Worksheet"),
    ("sheetNative.nmScopedBook", "Names Scoped to Workbook"),
    ("sheetNative.nmErrors", "Names with Errors"),
    ("sheetNative.nmNoErrors", "Names without Errors"),
    ("sheetNative.nmDefined", "Defined Names"),
    ("sheetNative.nmTables", "Table Names"),
];

/// Row height and header height of the table (reference pixels).
const ROW: f32 = 22.0;
const HEAD: f32 = 26.0;

impl SheetWindow {
    /// Open the Name Manager.
    pub(super) fn open_name_manager(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetNames.title", "Name Manager"),
            (705.0, 547.0),
            Vec::new(),
        );
        for (id, key, fb, x, at) in [
            ("new", "sheetNames.new", "New...", 18.0, 0),
            ("apply", "sheetNames.edit", "Edit...", 145.0, 0),
            ("del", "sheetNames.delete", "Delete", 272.0, 0),
        ] {
            f.add(
                id,
                None,
                [x, 48.0, 111.0, 29.0],
                Kind::Button {
                    label: t(key, fb),
                    default: false,
                },
            );
            f.set_access(id, at);
        }
        f.add(
            "filter",
            None,
            [575.0, 48.0, 111.0, 29.0],
            Kind::Combo {
                items: FILTERS.iter().map(|(k, d)| t(k, d)).collect(),
                sel: 0,
            },
        );
        // The list the events read (hidden) and the table drawn over its place.
        f.add(
            "names",
            None,
            [18.0, 82.0, 668.0, 350.0],
            Kind::List {
                items: Vec::new(),
                sel: None,
                top: 0,
            },
        );
        f.show("names", false);
        f.add("ntable", None, [18.0, 82.0, 668.0, 350.0], Kind::Canvas);
        f.add(
            "refL",
            None,
            [18.0, 440.0, 200.0, 22.0],
            Kind::Label(t("sheetNames.refersToLabel", "Refers to:")),
        );
        f.set_access("refL", 0);
        f.add("refX", None, [18.0, 458.0, 24.0, 26.0], Kind::Canvas);
        f.add("refOk", None, [44.0, 458.0, 24.0, 26.0], Kind::Canvas);
        f.add(
            "ref",
            None,
            [83.0, 458.0, 575.0, 26.0],
            Kind::Edit {
                value: Vec::new(),
                caret: 0,
            },
        );
        f.add("refPick", None, [660.0, 458.0, 26.0, 26.0], Kind::Canvas);
        f.add(
            "err",
            None,
            [83.0, 488.0, 470.0, 22.0],
            Kind::Note(String::new()),
        );
        f.add(
            "cancel",
            None,
            [575.0, 500.0, 111.0, 29.0],
            Kind::Button {
                label: t("sheetNames.close", "Close"),
                default: true,
            },
        );
        self.form = Some(f);
        self.form_kind = FormKind::Names;
        self.names_refresh();
        true
    }

    /// Rebuild the rows (names of the workbook, then tables), filtered by Filter ▾.
    pub(super) fn names_refresh(&mut self) {
        let list = self
            .view
            .query(json!({ "cmd": "nameList" }))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        let workbook = self.t("sheetNames.workbook", "Workbook");
        let sheets = self.view.sheet_names();
        let mut rows: Vec<NameRow> = list
            .iter()
            .map(|n| {
                let s = |k: &str| n.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
                let scope = n.get("scope").and_then(Value::as_u64).map(|s| s as usize);
                let refers = s("refersTo");
                NameRow {
                    value: self.name_value(&refers),
                    name: s("name"),
                    refers,
                    scope,
                    scope_label: scope
                        .and_then(|i| sheets.get(i).cloned().flatten())
                        .unwrap_or_else(|| workbook.clone()),
                    comment: s("comment"),
                    table: false,
                }
            })
            .collect();
        // Tables of every sheet (the reference lists them with a table mark).
        let mut tables: Vec<(String, String)> = Vec::new();
        if let Some(wb) = self.view.engine().workbook() {
            for (si, ws) in wb.sheets.iter().enumerate() {
                for tb in &ws.tables {
                    let sheet = sheets.get(si).cloned().flatten().unwrap_or_default();
                    let r = tb.range;
                    let refers = format!(
                        "={sheet}!${}${}:${}${}",
                        bo_model::sheet::col_to_letters(r.first.col),
                        r.first.row + 1,
                        bo_model::sheet::col_to_letters(r.last.col),
                        r.last.row + 1
                    );
                    tables.push((tb.name.clone(), refers));
                }
            }
        }
        for (name, refers) in tables {
            rows.push(NameRow {
                value: self.name_value(&refers),
                name,
                refers,
                scope: None,
                scope_label: workbook.clone(),
                comment: String::new(),
                table: true,
            });
        }
        let filter = self
            .form
            .as_ref()
            .and_then(|f| f.sel("filter"))
            .unwrap_or(0);
        let error = |r: &NameRow| r.value.starts_with('#') || r.refers.contains("#REF!");
        rows.retain(|r| match filter {
            1 => r.scope.is_some(),
            2 => r.scope.is_none(),
            3 => error(r),
            4 => !error(r),
            5 => !r.table,
            6 => r.table,
            _ => true,
        });
        let items: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
        self.name_table = rows;
        let refers = self.name_table.first().map(|r| r.refers.clone());
        if let Some(f) = self.form.as_mut() {
            let any = !items.is_empty();
            f.set_items("names", items, any.then_some(0));
            f.set_text("ref", refers.as_deref().unwrap_or(""));
            f.enable("ref", any);
            f.enable("apply", any);
            f.enable("del", any);
        }
    }

    /// The value a name's formula stands for, as the reference's Value column shows it:
    /// the cells of a range as `{"a";"b";…}` (rows `;`, columns `,`), a constant as is.
    fn name_value(&mut self, refers: &str) -> String {
        let body = refers.trim_start_matches('=');
        let Some(out) = self.view.query(json!({ "cmd": "resolve", "text": body })) else {
            return body.to_owned();
        };
        let (Some(r), sheet) = (
            out.get("range"),
            out.get("sheet").and_then(Value::as_u64).map(|s| s as usize),
        ) else {
            return body.to_owned();
        };
        let n = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        let (r0, c0, r1, c1) = (n("r0"), n("c0"), n("r1"), n("c1"));
        let sheet = sheet.unwrap_or(self.view.sheet());
        let cell = |row: u32, col: u32| {
            self.view
                .engine()
                .cell(sheet, row, col)
                .map(|d| d.display)
                .unwrap_or_default()
        };
        if (r0, c0) == (r1, c1) {
            return cell(r0, c0);
        }
        let mut s = String::from("{");
        'rows: for row in r0..=r1.min(r0 + 50) {
            if row > r0 {
                s.push(';');
            }
            for col in c0..=c1.min(c0 + 20) {
                if col > c0 {
                    s.push(',');
                }
                s.push('"');
                s.push_str(&cell(row, col));
                s.push('"');
                if s.chars().count() > 60 {
                    break 'rows;
                }
            }
        }
        s.push('}');
        s
    }

    fn names_set_error(&mut self, msg: String) {
        if let Some(Kind::Note(n)) = self
            .form
            .as_mut()
            .and_then(|f| f.get_mut("err"))
            .map(|c| &mut c.kind)
        {
            *n = msg;
        }
    }

    /// Define (or replace `old`) a name; the engine's message on failure.
    fn names_define(
        &mut self,
        name: &str,
        refers: &str,
        scope: Option<usize>,
        comment: &str,
        old: Option<(&str, Option<usize>)>,
    ) -> Result<(), String> {
        let mut cmd = json!({
            "cmd": "defineName", "name": name, "refersTo": refers, "scope": scope,
            "comment": comment,
        });
        if let Some((n, s)) = old {
            cmd["old"] = json!({ "name": n, "scope": s });
        }
        if self.view.query(cmd).is_some() {
            self.view.changed();
            Ok(())
        } else {
            Err(self
                .view
                .take_error()
                .unwrap_or_else(|| self.t("sheetNames.invalid", "The name is not valid.")))
        }
    }

    /// Events of the Name Manager.
    pub(super) fn names_event(&mut self, ev: &Event) {
        let chosen = self
            .form
            .as_ref()
            .and_then(|f| f.sel("names"))
            .and_then(|i| self.name_table.get(i).cloned());
        match ev {
            Event::Changed("filter") => self.names_refresh(),
            Event::Canvas("ntable", _, y) => {
                let row = ((y - HEAD) / ROW).floor();
                if row >= 0.0 && (row as usize) < self.name_table.len() {
                    let r = self.name_table[row as usize].refers.clone();
                    if let Some(f) = self.form.as_mut() {
                        f.set_sel("names", Some(row as usize));
                        f.set_text("ref", &r);
                    }
                }
            }
            Event::Clicked("new") => self.open_new_name(None),
            Event::Clicked("apply") => {
                if let Some(r) = chosen.filter(|r| !r.table) {
                    self.open_new_name(Some(r));
                }
            }
            Event::Clicked("del") => {
                if let Some(r) = chosen.filter(|r| !r.table) {
                    self.view
                        .query(json!({ "cmd": "deleteName", "name": r.name, "scope": r.scope }));
                    self.view.changed();
                    self.names_refresh();
                }
            }
            // ✓ keeps the edited Refers to, ✗ puts it back.
            Event::Canvas("refOk", ..) => {
                if let Some(r) = chosen.filter(|r| !r.table) {
                    let refers = self
                        .form
                        .as_ref()
                        .map(|f| f.text("ref"))
                        .unwrap_or_default();
                    match self.names_define(
                        &r.name,
                        &refers,
                        r.scope,
                        &r.comment,
                        Some((&r.name, r.scope)),
                    ) {
                        Ok(()) => {
                            self.names_set_error(String::new());
                            self.names_refresh();
                        }
                        Err(e) => self.names_set_error(e),
                    }
                }
            }
            Event::Canvas("refX", ..) => {
                if let (Some(r), Some(f)) = (chosen, self.form.as_mut()) {
                    f.set_text("ref", &r.refers);
                }
            }
            Event::Ok | Event::Cancel | Event::Clicked("cancel") => self.form = None,
            _ => {}
        }
    }

    /// New Name (`edit`: Edit Name of that row) over the stashed Name Manager.
    fn open_new_name(&mut self, edit: Option<NameRow>) {
        let t = |k: &str, d: &str| self.t(k, d);
        let title = if edit.is_some() {
            t("sheetNames.editTitle", "Edit Name")
        } else {
            t("sheetNames.newTitle", "New Name")
        };
        // A new name proposes the selection (and the label above it), as the reference.
        let (r0, c0, r1, c1) = self.view.range_bounds();
        let sheet_name = self
            .view
            .sheet_names()
            .get(self.view.sheet())
            .cloned()
            .flatten()
            .unwrap_or_default();
        let a = |r: u32, c: u32| format!("${}${}", bo_model::sheet::col_to_letters(c), r + 1);
        let sel_ref = if (r0, c0) == (r1, c1) {
            format!("={sheet_name}!{}", a(r0, c0))
        } else {
            format!("={sheet_name}!{}:{}", a(r0, c0), a(r1, c1))
        };
        let label_above = r0
            .checked_sub(1)
            .and_then(|r| self.view.engine().cell(self.view.sheet(), r, c0).ok())
            .map(|d| d.display)
            .filter(|s| !s.trim().is_empty() && !s.contains(' '))
            .unwrap_or_default();
        let (name, refers, scope, comment) = match &edit {
            Some(r) => (r.name.clone(), r.refers.clone(), r.scope, r.comment.clone()),
            None => (label_above, sel_ref, None, String::new()),
        };
        let mut scopes = vec![t("sheetNames.workbook", "Workbook")];
        scopes.extend(self.view.sheet_names().into_iter().flatten());
        let mut f = Form::new(&title, (393.0, 297.0), Vec::new());
        let edit_kind = |s: &str| Kind::Edit {
            value: s.chars().collect(),
            caret: s.chars().count(),
        };
        for (id, key, fb, y, at) in [
            ("nameL", "sheetNames.nameLabel", "Name:", 50.0, 0),
            ("scopeL", "sheetNative.nmScope", "Scope:", 82.0, 0),
            ("commentL", "sheetNative.nmComment", "Comment:", 113.0, 0),
            ("refL", "sheetNames.refersToLabel", "Refers to:", 222.0, 0),
        ] {
            f.add(id, None, [18.0, y, 72.0, 22.0], Kind::Label(t(key, fb)));
            f.set_access(id, at);
        }
        f.add("name", None, [91.0, 48.0, 284.0, 25.0], edit_kind(&name));
        f.add(
            "scope",
            None,
            [91.0, 79.0, 224.0, 28.0],
            Kind::Combo {
                items: scopes,
                sel: scope.map_or(0, |s| s + 1),
            },
        );
        f.add(
            "comment",
            None,
            [91.0, 110.0, 284.0, 105.0],
            edit_kind(&comment),
        );
        f.add("ref", None, [91.0, 219.0, 255.0, 26.0], edit_kind(&refers));
        f.add("refPick", None, [349.0, 219.0, 26.0, 26.0], Kind::Canvas);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [136.0, 249.0, 111.0, 30.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [262.0, 249.0, 111.0, 30.0];
        }
        f.focus("name");
        self.name_old = edit.map(|r| (r.name, r.scope));
        if let Some(main) = self.form.replace(f) {
            self.form_stash = Some((main, self.form_kind));
        }
        self.form_kind = FormKind::NewName;
    }

    /// Events of New / Edit Name.
    pub(super) fn new_name_event(&mut self, ev: &Event) {
        match ev {
            Event::Ok | Event::Clicked("ok") => {
                let Some(f) = self.form.as_ref() else {
                    return;
                };
                let (name, refers, comment) = (
                    f.text("name").trim().to_owned(),
                    f.text("ref"),
                    f.text("comment"),
                );
                let scope = f.sel("scope").and_then(|s| s.checked_sub(1));
                let old = self.name_old.clone();
                let res = self.names_define(
                    &name,
                    &refers,
                    scope,
                    &comment,
                    old.as_ref().map(|(n, s)| (n.as_str(), *s)),
                );
                match res {
                    Ok(()) => self.close_new_name(),
                    Err(e) => self.status = e,
                }
            }
            Event::Cancel | Event::Clicked("cancel") => self.close_new_name(),
            _ => {}
        }
    }

    fn close_new_name(&mut self) {
        self.name_old = None;
        if let Some((f, k)) = self.form_stash.take() {
            self.form = Some(f);
            self.form_kind = k;
            self.names_refresh();
        } else {
            self.form = None;
        }
    }

    /// The Name Manager's table and its small buttons.
    pub(super) fn paint_names(&self, f: &Form, id: &str, r: [f32; 4], p: &mut dyn Painter) {
        let k = self.scale / 1.25;
        let ink = Rgba(0, 0, 0, 255);
        let grey = Rgba(0xA0, 0xA0, 0xA0, 255);
        match id {
            "ntable" => {}
            "refX" | "refOk" => {
                let on = f.get("ref").is_some_and(|c| c.enabled);
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xCC, 0xCC, 0xCC, 255));
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    Rgba(0xF0, 0xF0, 0xF0, 255),
                );
                let c = if on { ink } else { grey };
                let (cx, cy, s) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, 6.0 * k);
                let segs = if id == "refX" {
                    vec![
                        PathSeg::M(cx - s, cy - s),
                        PathSeg::L(cx + s, cy + s),
                        PathSeg::M(cx + s, cy - s),
                        PathSeg::L(cx - s, cy + s),
                    ]
                } else {
                    vec![
                        PathSeg::M(cx - s, cy),
                        PathSeg::L(cx - s * 0.3, cy + s * 0.7),
                        PathSeg::L(cx + s, cy - s * 0.8),
                    ]
                };
                p.path(&segs, None, Some((c, 1.5 * k)));
                return;
            }
            "refPick" => {
                // Collapse-the-dialog button (picking a range on the sheet is not done here).
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xCC, 0xCC, 0xCC, 255));
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    Rgba(0xFD, 0xFD, 0xFD, 255),
                );
                let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                p.path(
                    &[
                        PathSeg::M(cx - 5.0 * k, cy),
                        PathSeg::L(cx + 5.0 * k, cy),
                        PathSeg::L(cx, cy - 6.0 * k),
                        PathSeg::Z,
                    ],
                    Some(grey),
                    None,
                );
                p.fill_rect(cx - 5.0 * k, cy + 3.0 * k, 10.0 * k, 1.5 * k, grey);
                return;
            }
            _ => return,
        }
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x87, 0x90, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let font = f.font();
        let line = font.size * 1.33;
        let cols = [
            (8.0, "sheetNames.name", "Name"),
            (150.0, "sheetNative.nmValue", "Value"),
            (300.0, "sheetNative.nmRefersTo", "Refers To"),
            (450.0, "sheetNames.scope", "Scope"),
            (532.0, "sheetNames.comment", "Comment"),
        ];
        let hh = (HEAD * k).round();
        for (i, (x, key, fb)) in cols.iter().enumerate() {
            if i > 0 {
                p.fill_rect(
                    (r[0] + (x - 7.0) * k).round(),
                    r[1] + 1.0,
                    1.0,
                    hh - 1.0,
                    Rgba(0xE5, 0xE5, 0xE5, 255),
                );
            }
            p.text_font(
                &self.t(key, fb),
                r[0] + x * k,
                (r[1] + (hh - line) / 2.0).round(),
                &font,
                ink,
            );
        }
        let sel = f.sel("names");
        let rh = (ROW * k).round();
        for (i, row) in self.name_table.iter().enumerate() {
            let y = r[1] + hh + i as f32 * rh;
            if y + rh > r[1] + r[3] {
                break;
            }
            let chosen = sel == Some(i);
            let fg = if chosen {
                p.fill_rect(r[0] + 2.0, y, r[2] - 4.0, rh, Rgba(0x00, 0x78, 0xD7, 255));
                Rgba(255, 255, 255, 255)
            } else {
                ink
            };
            // Our own marks: a small grid for a name, with a dark top row for a table.
            let (ix, iy, iw) = (r[0] + 6.0 * k, y + 4.0 * k, 16.0 * k);
            let mark = if chosen {
                Rgba(255, 255, 255, 255)
            } else {
                Rgba(0x3C, 0x7F, 0xC0, 255)
            };
            p.fill_rect(ix, iy, iw, rh - 8.0 * k, mark);
            p.fill_rect(
                ix + 1.0,
                iy + 1.0,
                iw - 2.0,
                rh - 8.0 * k - 2.0,
                if chosen {
                    Rgba(0x00, 0x78, 0xD7, 255)
                } else {
                    Rgba(0xDD, 0xEB, 0xF7, 255)
                },
            );
            if row.table {
                p.fill_rect(ix, iy, iw, 4.0 * k, mark);
            }
            p.fill_rect(ix + iw / 2.0, iy, 1.0, rh - 8.0 * k, mark);
            let ty = (y + (rh - line) / 2.0).round();
            let texts = [
                (cols[0].0 + 18.0, cols[1].0 - 7.0, &row.name),
                (cols[1].0, cols[2].0 - 7.0, &row.value),
                (cols[2].0, cols[3].0 - 7.0, &row.refers),
                (cols[3].0, cols[4].0 - 7.0, &row.scope_label),
                (cols[4].0, 660.0, &row.comment),
            ];
            for (x0, x1, text) in texts {
                let (x, w) = (r[0] + x0 * k, (x1 - x0 - 4.0) * k);
                p.push_clip(x, y, w, rh);
                // Long values end in "…" (measured at layout time).
                let shown = if f.width(text) > w {
                    let mut s: String = text.chars().take(40).collect();
                    while !s.is_empty() && f.width(&format!("{s}...")) > w {
                        s.pop();
                    }
                    format!("{s}...")
                } else {
                    (*text).clone()
                };
                p.text_font(&shown, x, ty, &font, fg);
                p.pop_clip();
            }
        }
    }

    /// Texts the table draws (for measuring at layout time).
    pub(super) fn names_strings(&self) -> Vec<String> {
        let mut v = Vec::new();
        for r in &self.name_table {
            v.push(r.value.clone());
            v.push(r.refers.clone());
            v.push(r.name.clone());
            let mut s: String = r.value.chars().take(40).collect();
            while !s.is_empty() {
                v.push(format!("{s}..."));
                s.pop();
            }
        }
        v
    }
}
