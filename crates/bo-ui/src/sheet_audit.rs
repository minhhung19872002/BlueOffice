//! Formulas tab of BlueSheet beyond the function library: Defined Names (Name Manager,
//! Create from Selection), Formula Auditing (trace precedents / dependents arrows, remove
//! arrows, show formulas, error checking, evaluate formula), and the Data tab's dialogs
//! (Sort, Data Validation, Goal Seek). The engine finds the arrows (`traceArrows`), errors
//! (`errorCheck`) and evaluation steps (`evaluateFormula`).
use super::form_impl::{Form, FormKind, Kind};
use super::SheetWindow;
use crate::{Painter, TextMeasure};
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

/// Colour of the auditing arrows: blue lines with a dot at the source and a head at the
/// target, as on the reference.
const ARROW_BLUE: Rgba = Rgba(0x00, 0x00, 0xff, 255);

/// One auditing arrow: from a range (r0, c0, r1, c1) to a cell, or from another sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Arrow {
    pub from: (u32, u32, u32, u32),
    pub to: (u32, u32),
    /// The source is on another sheet (drawn as a sheet icon arrow).
    pub other_sheet: bool,
}

impl SheetWindow {
    /// Formulas / Data tab commands handled here; `None` when `id` is not one of them.
    pub(super) fn audit_action(&mut self, id: &str, text: &dyn TextMeasure) -> Option<bool> {
        Some(match id {
            "sheet.insertFunction" => self.open_insert_function(),
            "sheet.nameManager" | "sheet.defineName" => self.open_name_manager(),
            "sheet.createNames" => {
                let sheet = self.view.sheet();
                let range = self.view.rng();
                {
                    let made = self.view.query(json!({
                        "cmd": "createNames", "sheet": sheet, "range": range, "top": true,
                    }));
                    self.view.changed();
                    made.is_some()
                }
            }
            "sheet.tracePrecedents" => self.trace(false),
            "sheet.traceDependents" => self.trace(true),
            "sheet.removeArrows" => {
                self.arrows.clear();
                self.trace_levels = (0, 0);
                true
            }
            "sheet.showFormulas" => {
                let on = !self.view.show_formulas();
                self.view.set_show_formulas(on);
                true
            }
            "sheet.errorCheck" => self.error_check(),
            "sheet.evaluateFormula" => self.open_evaluate_formula(),
            "sheet.sortDialog" => self.open_sort_dialog(),
            "sheet.dataValidation" => self.open_validation_dialog(),
            "sheet.goalSeek" => self.open_goal_seek(),
            "sheet.showComments" => self.toggle_all_comments(),
            "sheet.printTitles" => self.open_print_titles(),
            "sheet.help" => self.open_help(false),
            "sheet.shortcuts" => self.open_help(true),
            _ => {
                let _ = text;
                return None;
            }
        })
    }

    /// Print Titles: the rows repeated at the top of each page (`$1:$2`).
    fn open_print_titles(&mut self) -> bool {
        let rows = self
            .view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
            .and_then(|ws| ws.print_titles.clone())
            .unwrap_or_default();
        let rows = rows
            .split(',')
            .find(|p| p.contains('$') && !p.chars().any(|c| c.is_ascii_alphabetic() && c != '$'))
            .or_else(|| rows.split('!').nth(1))
            .unwrap_or("")
            .rsplit('!')
            .next()
            .unwrap_or("")
            .to_owned();
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetPrint.titles", "Print Titles"),
            (520.0, 210.0),
            Vec::new(),
        );
        f.add(
            "rowsL",
            None,
            [20.0, 60.0, 0.0, 0.0],
            Kind::Label(t("sheetPrint.rows", "Rows to repeat at top:")),
        );
        f.add(
            "rows",
            None,
            [230.0, 56.0, 270.0, 26.0],
            Kind::Edit {
                value: rows.chars().collect(),
                caret: rows.chars().count(),
            },
        );
        f.add(
            "hint",
            None,
            [20.0, 100.0, 0.0, 0.0],
            Kind::Note(t(
                "sheetPrint.hint",
                "For example $1:$2; leave empty for none.",
            )),
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("rows");
        self.form = Some(f);
        self.form_kind = FormKind::PrintTitles;
        true
    }

    /// Help (about) or the keyboard shortcuts list.
    fn open_help(&mut self, shortcuts: bool) -> bool {
        let t = |k: &str, d: &str| self.t(k, d);
        let title = if shortcuts {
            t("sheetHelp.shortcuts", "Keyboard Shortcuts")
        } else {
            t("sheetHelp.title", "BlueSheet Help")
        };
        let mut f = Form::new(&title, (560.0, 440.0), Vec::new());
        let items: Vec<String> =
            if shortcuts {
                [
                    (
                        "Ctrl+C / Ctrl+X / Ctrl+V",
                        "sheetHelp.clip",
                        "Copy / cut / paste",
                    ),
                    ("Ctrl+Z / Ctrl+Y", "sheetHelp.undo", "Undo / redo"),
                    ("F2", "sheetHelp.edit", "Edit the active cell"),
                    (
                        "F9 / Shift+F9",
                        "sheetHelp.calc",
                        "Calculate workbook / sheet",
                    ),
                    ("Ctrl+Arrow", "sheetHelp.edge", "Go to the edge of the data"),
                    (
                        "Ctrl+B / Ctrl+I / Ctrl+U",
                        "sheetHelp.style",
                        "Bold / italic / underline",
                    ),
                    ("Ctrl+5", "sheetHelp.strike", "Strikethrough"),
                    ("Ctrl+S", "sheetHelp.save", "Save"),
                    ("Ctrl+Tab", "sheetHelp.pages", "Next page of a dialog"),
                ]
                .iter()
                .map(|(k, key, d)| format!("{k:<26}{}", t(key, d)))
                .collect()
            } else {
                vec![
                format!("BlueSheet {}", env!("CARGO_PKG_VERSION")),
                t("sheetHelp.about", "Spreadsheet of the BlueOffice suite (Bluestar)."),
                t(
                    "sheetHelp.formats",
                    "Opens and saves workbooks compatible with the Microsoft Excel (.xlsx) format.",
                ),
            ]
            };
        f.add(
            "items",
            None,
            [20.0, 52.0, 520.0, 320.0],
            Kind::List {
                items,
                sel: None,
                top: 0,
            },
        );
        f.add(
            "cancel",
            None,
            [560.0 - 131.0, 440.0 - 48.0, 112.0, 28.0],
            Kind::Button {
                label: t("sheetNames.close", "Close"),
                default: true,
            },
        );
        self.form = Some(f);
        self.form_kind = FormKind::Help;
        true
    }

    /// Add one level of precedent (or dependent) arrows of the active cell.
    fn trace(&mut self, dependents: bool) -> bool {
        let (r, c) = self.view.active();
        if self.trace_cell != Some((r, c)) {
            self.arrows.clear();
            self.trace_levels = (0, 0);
            self.trace_cell = Some((r, c));
        }
        let level = if dependents {
            &mut self.trace_levels.1
        } else {
            &mut self.trace_levels.0
        };
        *level += 1;
        let levels = *level;
        let sheet = self.view.sheet();
        let Some(out) = self.view.query(json!({
            "cmd": "traceArrows", "sheet": sheet, "row": r, "col": c,
            "dependents": dependents, "levels": levels,
        })) else {
            return false;
        };
        let num = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        let list = out
            .get("arrows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for a in list {
            let (from, to) = (
                a.get("from").cloned().unwrap_or(Value::Null),
                a.get("to").cloned().unwrap_or(Value::Null),
            );
            // Dependents: `from` is the traced cell's range side, `to` the dependent cell.
            let other =
                num(&from, "sheet") as usize != sheet || num(&to, "sheet") as usize != sheet;
            let arrow = Arrow {
                from: (
                    num(&from, "r0"),
                    num(&from, "c0"),
                    num(&from, "r1"),
                    num(&from, "c1"),
                ),
                to: (num(&to, "row"), num(&to, "col")),
                other_sheet: other,
            };
            if !self.arrows.contains(&arrow) {
                self.arrows.push(arrow);
            }
        }
        if self.arrows.is_empty() {
            self.status = self.t(
                "sheetAudit.noRefs",
                "The active cell does not refer to or is not referred to by other cells.",
            );
        }
        true
    }

    /// Select the next cell with a formula error and say what it is.
    fn error_check(&mut self) -> bool {
        let sheet = self.view.sheet();
        let out = self
            .view
            .query(json!({ "cmd": "errorCheck", "sheet": sheet }))
            .unwrap_or(Value::Null);
        let list = out
            .get("errors")
            .or_else(|| out.get("cells"))
            .and_then(Value::as_array)
            .cloned()
            .or_else(|| out.as_array().cloned())
            .unwrap_or_default();
        let (ar, ac) = self.view.active();
        let num = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).map(|n| n as u32);
        let cells: Vec<(u32, u32, String)> = list
            .iter()
            .filter_map(|e| {
                Some((
                    num(e, "row")?,
                    num(e, "col")?,
                    e.get("message")
                        .or_else(|| e.get("rule"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                ))
            })
            .collect();
        let next = cells
            .iter()
            .find(|(r, c, _)| (*r, *c) > (ar, ac))
            .or_else(|| cells.first());
        match next {
            Some((r, c, msg)) => {
                self.view.select((*r, *c), (*r, *c), (*r, *c));
                self.status = if msg.is_empty() {
                    self.t("sheetAudit.errorFound", "Error in cell")
                } else {
                    msg.clone()
                };
            }
            None => {
                self.status = self.t(
                    "sheetAudit.noErrors",
                    "The error check is complete for the entire sheet.",
                );
            }
        }
        true
    }

    /// Auditing arrows over the grid (cells on screen only).
    pub(super) fn paint_arrows(&self, p: &mut dyn Painter) {
        if self.arrows.is_empty() {
            return;
        }
        let [fx, fy, fw, fh] = self.view.grid_frame();
        p.push_clip(fx, fy, fw, fh);
        let k = self.scale.max(1.0);
        for a in &self.arrows {
            let (r0, c0, r1, c1) = a.from;
            let Some(to) = self.view.cell_rect(a.to.0, a.to.1) else {
                continue;
            };
            let end = (to[0] + to[2] / 2.0, to[1] + to[3] / 2.0);
            let color = ARROW_BLUE;
            let start = if a.other_sheet {
                // A small sheet mark above-left of the target.
                let (x, y) = (end.0 - 40.0 * k, end.1 - 26.0 * k);
                p.fill_rect(
                    x - 7.0 * k,
                    y - 5.0 * k,
                    14.0 * k,
                    10.0 * k,
                    Rgba(0, 0, 0, 255),
                );
                p.fill_rect(
                    x - 6.0 * k,
                    y - 4.0 * k,
                    12.0 * k,
                    8.0 * k,
                    Rgba(255, 255, 255, 255),
                );
                (x, y)
            } else {
                let (Some(a0), Some(a1)) =
                    (self.view.cell_rect(r0, c0), self.view.cell_rect(r1, c1))
                else {
                    continue;
                };
                if (r0, c0) != (r1, c1) {
                    // Box around a range precedent.
                    let (x0, y0) = (a0[0], a0[1]);
                    let (x1, y1) = (a1[0] + a1[2], a1[1] + a1[3]);
                    for (x, y, w, h) in [
                        (x0, y0, x1 - x0, 1.0),
                        (x0, y1 - 1.0, x1 - x0, 1.0),
                        (x0, y0, 1.0, y1 - y0),
                        (x1 - 1.0, y0, 1.0, y1 - y0),
                    ] {
                        p.fill_rect(x, y, w, h.max(k.round()), color);
                    }
                }
                (a0[0] + a0[2] / 2.0, a0[1] + a0[3] / 2.0)
            };
            let w = k.round().max(1.0);
            p.path(
                &[PathSeg::M(start.0, start.1), PathSeg::L(end.0, end.1)],
                None,
                Some((color, w)),
            );
            // Dot at the source, head at the target.
            let d = 2.5 * k;
            p.fill_rect(
                (start.0 - d).round(),
                (start.1 - d).round(),
                (2.0 * d).round(),
                (2.0 * d).round(),
                color,
            );
            let (dx, dy) = (end.0 - start.0, end.1 - start.1);
            let len = dx.hypot(dy).max(1.0);
            let (ux, uy) = (dx / len, dy / len);
            let (hl, hw) = (9.0 * k, 4.0 * k);
            let back = (end.0 - ux * hl, end.1 - uy * hl);
            p.path(
                &[
                    PathSeg::M(end.0, end.1),
                    PathSeg::L(back.0 - uy * hw, back.1 + ux * hw),
                    PathSeg::L(back.0 + uy * hw, back.1 - ux * hw),
                    PathSeg::Z,
                ],
                Some(color),
                None,
            );
        }
        p.pop_clip();
    }
}
