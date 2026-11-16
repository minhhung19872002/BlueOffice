//! Formulas › Evaluate Formula as the reference's (round 39, measured at 125 %, 781 × 349 px,
//! `show/sheet-37`): the cell's reference on the left, the formula in a white box with the
//! part evaluated next underlined; Evaluate replaces it by its value (shown in italics) one
//! step at a time, then shows the result and turns into Restart. Steps come from the
//! engine's `evaluateFormula` (`before`, `part`, `after`, `shown`).

use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::{Font, Painter};
use bo_render::Rgba;
use serde_json::{json, Value};

/// One evaluation step: the formula text before the underlined part, the part, after it,
/// and the part's value.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct EvalStep {
    pub before: String,
    pub part: String,
    pub after: String,
    pub shown: String,
}

/// The open Evaluate Formula dialog's state.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct EvalState {
    pub formula: String,
    pub steps: Vec<EvalStep>,
    pub result: String,
    /// Steps done (`steps.len()`: the result is shown).
    pub at: usize,
}

impl EvalState {
    /// The box's text as (before, underlined part, after, italic value just computed).
    pub(super) fn pieces(&self) -> (String, String, String, String) {
        match self.steps.get(self.at) {
            Some(s) => {
                // The value of the previous step appears in italics where its part was.
                let prev = self
                    .at
                    .checked_sub(1)
                    .and_then(|i| self.steps.get(i))
                    .map(|p| p.shown.clone())
                    .unwrap_or_default();
                (s.before.clone(), s.part.clone(), s.after.clone(), prev)
            }
            None if self.steps.is_empty() => (
                self.formula.clone(),
                String::new(),
                String::new(),
                String::new(),
            ),
            None => (
                String::new(),
                String::new(),
                String::new(),
                self.result.clone(),
            ),
        }
    }
}

impl SheetWindow {
    /// Evaluate Formula on the active cell.
    pub(super) fn open_evaluate_formula(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let (r, c) = self.view.active();
        let out = self
            .view
            .query(json!({ "cmd": "evaluateFormula", "row": r, "col": c }))
            .unwrap_or(Value::Null);
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let steps: Vec<EvalStep> = out
            .get("steps")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|st| EvalStep {
                        before: s(st, "before"),
                        part: s(st, "part"),
                        after: s(st, "after"),
                        shown: s(st, "shown"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let formula = s(&out, "formula");
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetAudit.evaluate", "Evaluate Formula"),
            (781.0, 349.0),
            Vec::new(),
        );
        f.add(
            "refL",
            None,
            [18.0, 48.0, 200.0, 22.0],
            Kind::Label(t("sheetAudit.reference", "Reference:")),
        );
        f.add(
            "ref",
            None,
            [18.0, 73.0, 215.0, 22.0],
            Kind::Label(s(&out, "cell")),
        );
        f.add(
            "eq",
            None,
            [236.0, 73.0, 14.0, 22.0],
            Kind::Label("=".into()),
        );
        f.add(
            "evL",
            None,
            [257.0, 48.0, 200.0, 22.0],
            Kind::Label(t("sheetAudit.evaluation", "Evaluation:")),
        );
        f.add("box", None, [257.0, 68.0, 470.0, 85.0], Kind::Canvas);
        f.add(
            "note",
            None,
            [18.0, 258.0, 745.0, 22.0],
            Kind::Label(if formula.is_empty() {
                t("sheetAudit.noFormula", "The active cell has no formula.")
            } else {
                t(
                    "sheetNative.evalNote",
                    "To show the result of the underlined expression, click Evaluate.  The most recent result appears italicized.",
                )
            }),
        );
        for (id, key, fb, x) in [
            ("evaluate", "sheetNative.evalEvaluate", "Evaluate", 269.0),
            ("stepIn", "sheetNative.evalStepIn", "Step In", 396.0),
            ("stepOut", "sheetNative.evalStepOut", "Step Out", 523.0),
        ] {
            f.add(
                id,
                None,
                [x, 301.0, 111.0, 29.0],
                Kind::Button {
                    label: t(key, fb),
                    default: id == "evaluate",
                },
            );
        }
        f.enable("stepIn", false);
        f.enable("stepOut", false);
        f.enable("evaluate", !formula.is_empty());
        f.add(
            "cancel",
            None,
            [650.0, 301.0, 111.0, 29.0],
            Kind::Button {
                label: t("sheetNames.close", "Close"),
                default: false,
            },
        );
        self.eval = Some(EvalState {
            formula,
            steps,
            result: s(&out, "result"),
            at: 0,
        });
        self.form = Some(f);
        self.form_kind = FormKind::Evaluate;
        true
    }

    /// Strings the box draws, measured at layout time (underline widths).
    pub(super) fn eval_strings(&self) -> Vec<String> {
        let Some(e) = &self.eval else {
            return Vec::new();
        };
        let (b, p, a, v) = e.pieces();
        vec![b, p, a, v]
    }

    /// Events of Evaluate Formula.
    pub(super) fn evaluate_event(&mut self, ev: &Event) {
        match ev {
            Event::Cancel | Event::Clicked("cancel") => {
                self.form = None;
                self.eval = None;
            }
            Event::Ok | Event::Clicked("evaluate") => {
                let label = {
                    let Some(e) = self.eval.as_mut() else {
                        return;
                    };
                    if e.at >= e.steps.len() {
                        e.at = 0;
                    } else {
                        e.at += 1;
                    }
                    if e.at >= e.steps.len() {
                        self.t("sheetNative.evalRestart", "Restart")
                    } else {
                        self.t("sheetNative.evalEvaluate", "Evaluate")
                    }
                };
                if let Some(Kind::Button { label: l, .. }) = self
                    .form
                    .as_mut()
                    .and_then(|f| f.get_mut("evaluate"))
                    .map(|c| &mut c.kind)
                {
                    *l = label;
                }
            }
            _ => {}
        }
    }

    /// The Evaluation box: white, framed #828790, the formula with the next part underlined
    /// and the latest value in italics.
    pub(super) fn paint_evaluate(&self, f: &Form, r: [f32; 4], p: &mut dyn Painter) {
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x87, 0x90, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let Some(e) = &self.eval else {
            return;
        };
        let font = f.font();
        let ink = Rgba(0, 0, 0, 255);
        let k = self.scale / 1.25;
        let (mut x, y) = (r[0] + 4.0 * k, r[1] + 4.0 * k);
        let line = font.size * 1.33;
        let (before, part, after, value) = e.pieces();
        if part.is_empty() && before.is_empty() {
            // The result.
            let it = Font {
                italic: true,
                ..font
            };
            p.text_font(&value, x, y, &it, ink);
            return;
        }
        p.text_font(&before, x, y, &font, ink);
        x += f.width(&before);
        p.text_font(&part, x, y, &font, ink);
        let w = f.width(&part);
        p.fill_rect(
            x,
            (y + line - 2.0 * k).round(),
            w,
            1.0_f32.max(k.round()),
            ink,
        );
        x += w;
        p.text_font(&after, x, y, &font, ink);
    }
}
