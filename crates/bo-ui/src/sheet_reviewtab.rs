//! Review tab of BlueSheet as the reference's (round 47, `show/sheet-45`): Proofing
//! (Spelling with the reference's dialog — 673 × 398 px —, Thesaurus greyed, Workbook
//! Statistics), Accessibility (Check Accessibility: inspection results), Language
//! (Translate greyed), Comments (New, Delete, Previous, Next, Show/Hide Comment, Show All
//! Comments), Protect (Protect Sheet, Protect Workbook, Allow Edit Ranges and Unshare Workbook
//! greyed), Ink (Hide Ink greyed). Icons are our own drawings.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::icons::Icons;
use bo_model::sheet::CellValue;
use bo_render::Rgba;
use serde_json::json;

const BLUE: Rgba = Rgba(0x2e, 0x75, 0xb6, 255);
const PALE: Rgba = Rgba(0xd6, 0xe4, 0xf2, 255);
const GOLD: Rgba = Rgba(0xe8, 0xa8, 0x2a, 255);
const GREEN: Rgba = Rgba(0x21, 0x73, 0x46, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);

/// Ribbon labels of the Review tab: (command, key in `sheetRev`, fallback).
const LABELS: [(&str, &str, &str); 9] = [
    ("spelling", "spelling", "Spelling"),
    ("thesaurus", "thesaurus", "Thesaurus"),
    ("workbookStats", "workbookStats", "Workbook Statistics"),
    (
        "checkAccessibility",
        "checkAccessibility",
        "Check Accessibility",
    ),
    ("translate", "translate", "Translate"),
    ("showHideComment", "showHideComment", "Show/Hide Comment"),
    ("allowEditRanges", "allowEditRanges", "Allow Edit Ranges"),
    ("unshareWorkbook", "unshareWorkbook", "Unshare Workbook"),
    ("hideInk", "hideInk", "Hide Ink"),
];

/// Commands of the tab that are shown but not available (greyed as the reference greys
/// what does not apply).
pub(super) const GREYED: [&str; 5] = [
    "sheet.thesaurus",
    "sheet.translate",
    "sheet.allowEditRanges",
    "sheet.unshareWorkbook",
    "sheet.hideInk",
];

/// Use the Review tab's labels.
pub(super) fn install_labels(s: &mut crate::strings::Strings) {
    for (id, key, fb) in LABELS {
        let v = s.get(&format!("sheetRev.{key}")).to_owned();
        let v = if v.is_empty() || v.starts_with("sheetRev.") {
            fb.to_owned()
        } else {
            v
        };
        s.set(&format!("commands.sheet.{id}"), &v);
    }
    let all = s.get("sheetRev.showAllComments").to_owned();
    if !all.is_empty() && !all.starts_with("sheetRev.") {
        s.set("commands.sheet.showComments", &all);
    }
}

/// Own drawings of the tab's buttons (24-unit grid).
pub(super) fn add_icons(icons: &mut Icons) {
    let sheet = |extra: &[(&'static str, Option<Rgba>)]| {
        let mut v: Vec<(&'static str, Option<Rgba>)> = vec![
            ("M3 3h13v15h-13Z", Some(WHITE)),
            ("M3 3h13v15h-13Z", None),
            ("M3 8h13M3 13h13M8 3v15", None),
        ];
        v.extend_from_slice(extra);
        v
    };
    icons.add_solid(
        "sheet.spelling",
        &[
            ("M3 11l3-8h1l3 8M4.2 8h4.6", None),
            ("M12 11v-8h3a2 2 0 0 1 0 4h-3h3.5a2 2 0 0 1 0 4Z", None),
            ("M7 17l4 4l9-10", None),
            ("M7 17l4 4l9-10", Some(BLUE)),
        ],
    );
    icons.add_solid(
        "sheet.thesaurus",
        &[
            ("M2 5c3-1 6-1 9 1v14c-3-2-6-2-9-1Z", Some(WHITE)),
            ("M2 5c3-1 6-1 9 1v14c-3-2-6-2-9-1Z", None),
            ("M22 5c-3-1-6-1-9 1v14c3-2 6-2 9-1Z", Some(PALE)),
            ("M22 5c-3-1-6-1-9 1v14c3-2 6-2 9-1Z", None),
        ],
    );
    icons.add_solid(
        "sheet.workbookStats",
        &sheet(&[
            ("M14 21v-6h2.5v6Z", Some(BLUE)),
            ("M17.5 21v-9h2.5v9Z", Some(BLUE)),
            ("M21 21v-4h2v4Z", Some(GOLD)),
        ]),
    );
    icons.add_solid(
        "sheet.checkAccessibility",
        &[
            ("M5 2h9l5 5v15h-14Z", Some(WHITE)),
            ("M5 2h9l5 5v15h-14Z", None),
            ("M14 2v5h5", None),
            ("M12 14m-4 0a4 4 0 1 0 8 0a4 4 0 1 0-8 0", Some(BLUE)),
            ("M10.5 13.5l1.5 1.5l2.5-3", None),
        ],
    );
    icons.add_solid(
        "sheet.translate",
        &[
            ("M2 3h11v9h-6l-3 3v-3h-2Z", Some(BLUE)),
            ("M11 10h11v9h-2v3l-3-3h-6Z", Some(PALE)),
            ("M11 10h11v9h-2v3l-3-3h-6Z", None),
        ],
    );
    icons.add_solid(
        "sheet.showHideComment",
        &[
            ("M3 5h17v10h-9l-4 4v-4h-4Z", Some(WHITE)),
            ("M3 5h17v10h-9l-4 4v-4h-4Z", None),
            ("M7 9h9M7 12h6", None),
        ],
    );
    icons.add_solid(
        "sheet.protectSheet",
        &sheet(&[
            ("M13 14h9v8h-9Z", Some(GOLD)),
            ("M15 14v-2.5a2.5 2.5 0 0 1 5 0v2.5", None),
        ]),
    );
    icons.add_solid(
        "sheet.protectWorkbook",
        &[
            ("M3 4h12v16h-12Z", Some(PALE)),
            ("M3 4h12v16h-12Z", None),
            ("M6 4v16", None),
            ("M13 14h9v8h-9Z", Some(GOLD)),
            ("M15 14v-2.5a2.5 2.5 0 0 1 5 0v2.5", None),
        ],
    );
    icons.add_solid(
        "sheet.allowEditRanges",
        &sheet(&[("M12 20l2-5l7-7l3 3l-7 7Z", Some(BLUE))]),
    );
    icons.add_solid(
        "sheet.unshareWorkbook",
        &sheet(&[("M12 19h10M19 16l3 3l-3 3", None)]),
    );
    icons.add_solid(
        "sheet.hideInk",
        &[
            ("M4 20c4-8 8-12 16-14", None),
            ("M14 4l6 6l-2 2l-6-6Z", Some(GREEN)),
        ],
    );
}

/// What the Spelling dialog walks through.
#[derive(Debug, Clone, Default)]
pub(super) struct SpellState {
    /// Text cells still to check (row, column), in order from the active cell.
    pub queue: Vec<(u32, u32)>,
    /// The cell and byte range of the word shown.
    pub at: Option<(u32, u32, std::ops::Range<usize>)>,
    pub ignored: Vec<String>,
    /// Change All: (word, replacement).
    pub change_all: Vec<(String, String)>,
    /// Dictionary language: 0 English, 1 Vietnamese.
    pub lang: usize,
}

const LANGS: [(&str, &str, &str); 2] = [
    ("en", "sheetRev.langEn", "English (United States)"),
    ("vi", "sheetRev.langVi", "Vietnamese"),
];

fn new_speller() -> bo_spell::Speller {
    let mut s = bo_spell::Speller::new();
    s.add_language("vi", Box::new(bo_spell::vi::Vietnamese));
    if let Some((aff, dic)) = bo_engine::bundled_dictionary() {
        if let Ok(d) = bo_spell::Dictionary::from_aff_dic(aff, dic) {
            s.add_language("en", Box::new(d));
        }
    }
    s
}

impl SheetWindow {
    /// Review tab commands of this module: `Some(redraw)` when `id` is one.
    pub(super) fn review_tab_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "sheet.spelling" => self.open_spelling(),
            "sheet.workbookStats" => self.open_workbook_stats(),
            "sheet.checkAccessibility" => self.open_accessibility(),
            "sheet.showHideComment" => {
                let at = self.view.active();
                if self.commented_cells().contains(&at) {
                    if let Some(i) = self.pinned_comments.iter().position(|c| *c == at) {
                        self.pinned_comments.remove(i);
                    } else {
                        self.pinned_comments.push(at);
                    }
                }
                true
            }
            id if GREYED.contains(&id) => false,
            _ => return None,
        })
    }

    fn text_cells(&self) -> Vec<(u32, u32)> {
        let Some(ws) = self
            .view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
        else {
            return Vec::new();
        };
        let at = self.view.active();
        let mut cells: Vec<(u32, u32)> = ws
            .cells
            .iter()
            .filter(|(_, c)| c.formula.is_none() && matches!(c.value, CellValue::Text(_)))
            .map(|(k, _)| *k)
            .collect();
        // From the active cell on, then the cells before it (the reference wraps round).
        let split = cells.partition_point(|k| *k < at);
        cells.rotate_left(split);
        cells
    }

    fn cell_text(&self, r: u32, c: u32) -> String {
        self.view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
            .and_then(|ws| ws.cell(r, c))
            .and_then(|c| c.value.as_text())
            .unwrap_or_default()
    }

    /// The next misspelt word from the queue (`None`: the check is complete).
    fn spell_next(&mut self, from: usize) -> Option<(u32, u32, std::ops::Range<usize>)> {
        if self.speller.is_none() {
            self.speller = Some(new_speller());
        }
        let st = self.spell.clone()?;
        let lang = LANGS[st.lang.min(1)].0;
        let mut skip = from;
        for &(r, c) in &st.queue {
            let text = self.cell_text(r, c);
            let sp = self.speller.as_ref()?;
            for w in bo_spell::checkable_words(&text) {
                if w.start < skip {
                    continue;
                }
                let word = &text[w.clone()];
                if st.ignored.iter().any(|x| x == word) {
                    continue;
                }
                if sp.check_in(word, Some(lang)) == Some(false) {
                    return Some((r, c, w));
                }
            }
            skip = 0;
        }
        None
    }

    fn open_spelling(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        // The dictionary of the interface language (Vietnamese when the ribbon is).
        let lang = usize::from(self.t("common.cancel", "Cancel") == "Huỷ");
        self.spell = Some(SpellState {
            queue: self.text_cells(),
            lang,
            ..SpellState::default()
        });
        self.spell_show(0);
        true
    }

    /// Show the next misspelt word in the dialog, or end the check.
    fn spell_show(&mut self, from: usize) {
        // Drop the cells before the current one.
        if let Some(st) = self.spell.as_mut() {
            if let Some((r, c, _)) = st.at.clone() {
                if let Some(i) = st.queue.iter().position(|k| *k == (r, c)) {
                    st.queue.drain(..i);
                }
            }
        }
        let next = self.spell_next(from);
        let Some((r, c, w)) = next else {
            self.form = None;
            self.spell = None;
            self.status = self.t(
                "sheetRev.spellDone",
                "Spell check complete. You're good to go!",
            );
            return;
        };
        self.view.select((r, c), (r, c), (r, c));
        let text = self.cell_text(r, c);
        let word = text[w.clone()].to_owned();
        let lang = self.spell.as_ref().map_or(0, |s| s.lang);
        let suggestions = self
            .speller
            .as_ref()
            .map(|s| s.suggest_in(&word, Some(LANGS[lang].0)))
            .unwrap_or_default();
        if let Some(st) = self.spell.as_mut() {
            st.at = Some((r, c, w));
        }
        let t = |k: &str, f: &str| self.t(k, f);
        let lang_name = t(LANGS[lang].1, LANGS[lang].2);
        let mut f = Form::new(
            &format!("{}: {lang_name}", t("sheetRev.spellTitle", "Spelling")),
            (673.0, 398.0),
            vec![],
        );
        f.add(
            "wordL",
            None,
            [18.0, 46.0, 300.0, 22.0],
            Kind::Label(t("sheetRev.notInDict", "Not in Dictionary:")),
        );
        f.set_access("wordL", 7);
        f.add(
            "word",
            None,
            [18.0, 68.0, 494.0, 26.0],
            Kind::Edit {
                value: word.chars().collect(),
                caret: word.chars().count(),
            },
        );
        f.add(
            "sugL",
            None,
            [18.0, 179.0, 300.0, 22.0],
            Kind::Label(t("sheetRev.suggestions", "Suggestions:")),
        );
        f.set_access("sugL", 9);
        let none = suggestions.is_empty();
        let items = if none {
            vec![t("sheetRev.noSuggestions", "(No Suggestions)")]
        } else {
            suggestions
        };
        f.add(
            "sug",
            None,
            [18.0, 200.0, 494.0, 106.0],
            Kind::List {
                items,
                sel: Some(0),
                top: 0,
            },
        );
        f.add(
            "langL",
            None,
            [18.0, 314.0, 140.0, 22.0],
            Kind::Label(t("sheetRev.dictLang", "Dictionary language:")),
        );
        f.set_access("langL", 2);
        f.add(
            "lang",
            None,
            [151.0, 311.0, 361.0, 27.0],
            Kind::Combo {
                items: LANGS.iter().map(|(_, k, fb)| t(k, fb)).collect(),
                sel: lang,
            },
        );
        for (id, y, key, fb, acc, on) in [
            (
                "ignore",
                68.0,
                "sheetRev.ignoreOnce",
                "Ignore Once",
                0,
                true,
            ),
            (
                "ignoreAll",
                108.0,
                "sheetRev.ignoreAll",
                "Ignore All",
                1,
                true,
            ),
            (
                "add",
                146.0,
                "sheetRev.addDict",
                "Add to Dictionary",
                0,
                true,
            ),
            ("change", 201.0, "sheetRev.change", "Change", 0, !none),
            (
                "changeAll",
                240.0,
                "sheetRev.changeAll",
                "Change All",
                7,
                !none,
            ),
            (
                "autocorrect",
                279.0,
                "sheetRev.autoCorrect",
                "AutoCorrect",
                4,
                false,
            ),
        ] {
            f.add(
                id,
                None,
                [526.0, y, 128.0, 28.0],
                Kind::Button {
                    label: t(key, fb),
                    default: id == "ignore",
                },
            );
            f.set_access(id, acc);
            f.enable(id, on);
        }
        for (id, x, key, fb, on) in [
            ("options", 18.0, "sheetRev.options", "Options...", false),
            ("undo", 383.0, "sheetRev.undoLast", "Undo Last", false),
            ("cancel", 526.0, "common.cancel", "Cancel", true),
        ] {
            f.add(
                id,
                None,
                [x, 351.0, 128.0, 28.0],
                Kind::Button {
                    label: t(key, fb),
                    default: false,
                },
            );
            f.enable(id, on);
        }
        f.set_access("options", 0);
        f.focus("sug");
        self.form = Some(f);
        self.form_kind = FormKind::Spelling;
    }

    /// Replace the shown word with `with` (all its occurrences when `all`).
    fn spell_change(&mut self, with: &str, all: bool) {
        let Some((r, c, w)) = self.spell.as_ref().and_then(|s| s.at.clone()) else {
            return;
        };
        let text = self.cell_text(r, c);
        let word = text[w.clone()].to_owned();
        let mut cells = vec![(
            r,
            c,
            format!("{}{with}{}", &text[..w.start], &text[w.end..]),
        )];
        if all {
            let queue = self
                .spell
                .as_ref()
                .map(|s| s.queue.clone())
                .unwrap_or_default();
            for (qr, qc) in queue.into_iter().filter(|k| *k != (r, c)) {
                let t = self.cell_text(qr, qc);
                if t.contains(&word) {
                    cells.push((qr, qc, t.replace(&word, with)));
                }
            }
            cells[0].2 = text.replace(&word, with);
            if let Some(st) = self.spell.as_mut() {
                st.change_all.push((word, with.to_owned()));
            }
        }
        let sheet = self.view.sheet();
        let list: Vec<serde_json::Value> = cells.iter().map(|(r, c, t)| json!([r, c, t])).collect();
        if self
            .view
            .query(json!({ "cmd": "setInputs", "sheet": sheet, "cells": list }))
            .is_some()
        {
            self.view.changed();
        }
        self.spell_show(w.start + with.len());
    }

    /// Events of the Spelling dialog.
    pub(super) fn spelling_event(&mut self, ev: &Event) {
        let from = self
            .spell
            .as_ref()
            .and_then(|s| s.at.as_ref())
            .map_or(0, |a| a.2.end);
        let word = self
            .spell
            .as_ref()
            .and_then(|s| s.at.clone())
            .map(|(r, c, w)| self.cell_text(r, c)[w].to_owned())
            .unwrap_or_default();
        match ev {
            Event::Cancel | Event::Clicked("cancel") => {
                self.form = None;
                self.spell = None;
            }
            Event::Ok | Event::Clicked("ignore") => self.spell_show(from),
            Event::Clicked("ignoreAll") => {
                if let Some(st) = self.spell.as_mut() {
                    st.ignored.push(word);
                }
                self.spell_show(from);
            }
            Event::Clicked("add") => {
                if let Some(sp) = self.speller.as_mut() {
                    sp.add_user_word(&word);
                }
                self.spell_show(from);
            }
            Event::Clicked(id @ ("change" | "changeAll" | "sug")) => {
                let Some(f) = self.form.as_ref() else {
                    return;
                };
                let typed = f.text("word");
                let with = if typed == word {
                    match &f.get("sug").map(|c| &c.kind) {
                        Some(Kind::List { items, sel, .. }) => {
                            sel.and_then(|i| items.get(i)).cloned().unwrap_or_default()
                        }
                        _ => String::new(),
                    }
                } else {
                    typed
                };
                if !with.is_empty() {
                    self.spell_change(&with, *id == "changeAll");
                }
            }
            Event::Changed("lang") => {
                let lang = self.form.as_ref().and_then(|f| f.sel("lang")).unwrap_or(0);
                if let Some(st) = self.spell.as_mut() {
                    st.lang = lang;
                }
                let start = self
                    .spell
                    .as_ref()
                    .and_then(|s| s.at.as_ref())
                    .map_or(0, |a| a.2.start);
                self.spell_show(start);
            }
            _ => {}
        }
    }

    /// Workbook Statistics: counts of the active sheet and of the workbook.
    fn open_workbook_stats(&mut self) -> bool {
        let Some(wb) = self.view.engine().workbook() else {
            return false;
        };
        let sheet = self.view.sheet();
        let count = |ws: &bo_model::sheet::Worksheet| {
            let data = ws
                .cells
                .values()
                .filter(|c| c.formula.is_some() || !matches!(c.value, CellValue::Empty))
                .count();
            let formulas = ws.cells.values().filter(|c| c.formula.is_some()).count();
            let notes = ws.comments.len();
            let threads = ws
                .threaded_comments
                .iter()
                .filter(|c| c.parent_id.is_none())
                .count();
            [
                data,
                ws.tables.len(),
                ws.pivots.len(),
                formulas,
                ws.charts.len(),
                threads,
                notes,
            ]
        };
        let cur = wb.sheets.get(sheet).map(count).unwrap_or_default();
        let end = wb
            .sheets
            .get(sheet)
            .and_then(|ws| {
                ws.cells.keys().fold(None, |m: Option<(u32, u32)>, k| {
                    Some(m.map_or(*k, |m| (m.0.max(k.0), m.1.max(k.1))))
                })
            })
            .map_or_else(
                || "A1".to_owned(),
                |(r, c)| format!("{}{}", bo_model::sheet::col_to_letters(c), r + 1),
            );
        let mut all = [0usize; 7];
        for ws in &wb.sheets {
            for (a, b) in all.iter_mut().zip(count(ws)) {
                *a += b;
            }
        }
        let n_sheets = wb.sheets.len();
        let sheet_name = wb
            .sheets
            .get(sheet)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        let t = |k: &str, f: &str| self.t(k, f);
        let rows_cur = [
            (t("sheetRev.endOfSheet", "End of sheet"), end),
            (
                t("sheetRev.cellsData", "Cells with data"),
                cur[0].to_string(),
            ),
            (t("sheetRev.tables", "Tables"), cur[1].to_string()),
            (t("sheetRev.pivots", "PivotTables"), cur[2].to_string()),
            (t("sheetRev.formulas", "Formulas"), cur[3].to_string()),
            (t("sheetRev.charts", "Charts"), cur[4].to_string()),
            (t("sheetRev.comments", "Comments"), cur[5].to_string()),
            (t("sheetRev.notes", "Notes"), cur[6].to_string()),
        ];
        let rows_wb = [
            (t("sheetRev.sheets", "Sheets"), n_sheets.to_string()),
            (
                t("sheetRev.cellsData", "Cells with data"),
                all[0].to_string(),
            ),
            (t("sheetRev.tables", "Tables"), all[1].to_string()),
            (t("sheetRev.pivots", "PivotTables"), all[2].to_string()),
            (t("sheetRev.formulas", "Formulas"), all[3].to_string()),
            (t("sheetRev.charts", "Charts"), all[4].to_string()),
            (t("sheetRev.comments", "Comments"), all[5].to_string()),
            (t("sheetRev.notes", "Notes"), all[6].to_string()),
        ];
        let mut f = Form::new(
            &t("sheetRev.statsTitle", "Workbook Statistics"),
            (420.0, 560.0),
            vec![],
        );
        f.set_font_scale(1.15);
        f.add(
            "curG",
            None,
            [18.0, 46.0, 384.0, 22.0],
            Kind::Group(format!(
                "{} {sheet_name}",
                t("sheetRev.currentSheet", "Current sheet:")
            )),
        );
        let ids_l: [&'static str; 16] = [
            "l0", "l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8", "l9", "l10", "l11", "l12", "l13",
            "l14", "l15",
        ];
        let ids_v: [&'static str; 16] = [
            "v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11", "v12", "v13",
            "v14", "v15",
        ];
        for (i, (l, v)) in rows_cur.iter().chain(rows_wb.iter()).enumerate() {
            let y = if i < 8 {
                76.0 + 27.0 * i as f32
            } else {
                76.0 + 27.0 * i as f32 + 36.0
            };
            f.add(
                ids_l[i],
                None,
                [34.0, y, 260.0, 24.0],
                Kind::Label(l.clone()),
            );
            f.add(
                ids_v[i],
                None,
                [300.0, y, 100.0, 24.0],
                Kind::Label(v.clone()),
            );
        }
        f.add(
            "wbG",
            None,
            [18.0, 76.0 + 27.0 * 8.0 + 6.0, 384.0, 22.0],
            Kind::Group(t("sheetRev.workbook", "Workbook")),
        );
        f.add(
            "ok",
            None,
            [292.0, 512.0, 110.0, 32.0],
            Kind::Button {
                label: t("common.ok", "OK"),
                default: true,
            },
        );
        self.form = Some(f);
        self.form_kind = FormKind::Help;
        true
    }

    /// Check Accessibility: the inspection results of the workbook (our own checks: merged
    /// cells, default sheet names, charts without a title, tables without a header row).
    fn open_accessibility(&mut self) -> bool {
        let Some(wb) = self.view.engine().workbook() else {
            return false;
        };
        let mut warn = Vec::new();
        let mut errors = Vec::new();
        let default_name = |n: &str| {
            let l = n.to_lowercase();
            ["sheet", "trang tính"].iter().any(|p| {
                l.strip_prefix(p)
                    .is_some_and(|d| d.trim().parse::<u32>().is_ok())
            })
        };
        for ws in &wb.sheets {
            if !ws.merges.is_empty() {
                warn.push(format!(
                    "{} ({})",
                    self.t("sheetRev.merged", "Merged Cells"),
                    ws.name
                ));
            }
            if default_name(&ws.name) && !ws.cells.is_empty() {
                warn.push(format!(
                    "{} ({})",
                    self.t("sheetRev.defaultName", "Default Sheet Names"),
                    ws.name
                ));
            }
            for ch in &ws.charts {
                if ch.title.as_deref().unwrap_or("").is_empty() {
                    errors.push(format!(
                        "{} ({})",
                        self.t("sheetRev.noAlt", "Missing Alternative Text"),
                        ws.name
                    ));
                    let _ = ch;
                }
            }
            for tb in &ws.tables {
                if tb.header_row_count == Some(0) {
                    errors.push(format!(
                        "{} ({})",
                        self.t("sheetRev.noHeader", "No Header Row Specified"),
                        tb.display_name
                    ));
                }
            }
        }
        let t = |k: &str, f: &str| self.t(k, f);
        let mut items = Vec::new();
        if !errors.is_empty() {
            items.push(format!(
                "{} ({})",
                t("sheetRev.errors", "Errors"),
                errors.len()
            ));
            items.extend(errors.into_iter().map(|e| format!("    {e}")));
        }
        if !warn.is_empty() {
            items.push(format!(
                "{} ({})",
                t("sheetRev.warnings", "Warnings"),
                warn.len()
            ));
            items.extend(warn.into_iter().map(|e| format!("    {e}")));
        }
        let clean = items.is_empty();
        if clean {
            items.push(t(
                "sheetRev.noIssues",
                "No accessibility issues found. People with disabilities should not have difficulty reading this workbook.",
            ));
        }
        let mut f = Form::new(
            &t("sheetRev.accTitle", "Accessibility"),
            (420.0, 520.0),
            vec![],
        );
        f.add(
            "resL",
            None,
            [18.0, 46.0, 380.0, 22.0],
            Kind::Group(t("sheetRev.inspection", "Inspection Results")),
        );
        f.add(
            "list",
            None,
            [18.0, 74.0, 384.0, 300.0],
            Kind::List {
                items,
                sel: None,
                top: 0,
            },
        );
        f.add(
            "infoG",
            None,
            [18.0, 386.0, 380.0, 22.0],
            Kind::Group(t("sheetRev.additional", "Additional Information")),
        );
        f.add(
            "info",
            None,
            [18.0, 410.0, 384.0, 50.0],
            Kind::Note(t(
                "sheetRev.accNote",
                "Select an issue to see why it matters and how to fix it.",
            )),
        );
        f.add(
            "ok",
            None,
            [292.0, 472.0, 110.0, 32.0],
            Kind::Button {
                label: t("common.close", "Close"),
                default: true,
            },
        );
        self.form = Some(f);
        self.form_kind = FormKind::Help;
        true
    }
}
