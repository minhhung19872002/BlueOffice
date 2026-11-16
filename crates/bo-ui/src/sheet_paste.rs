//! Paste Special of BlueSheet (round 27): Ctrl+Alt+V and the grid's context menu open the
//! reference's dialog (Paste: All, Formulas, Values, Formats, All except borders; Operation;
//! Skip blanks, Transpose) over the internal clipboard (engine `paste` with `mode`,
//! `skipBlanks`, `transpose`, `noBorders`). Operation adds / subtracts / multiplies / divides into the destination (round 36);
//! choices the engine cannot paste yet are dimmed.

use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use serde_json::json;

/// The Paste choices: (control id, label key, fallback, engine mode, available).
const PASTE: [(&str, &str, &str, &str, bool); 12] = [
    ("pAll", "sheetPaste.all", "All", "all", true),
    (
        "pFormulas",
        "sheetPaste.formulas",
        "Formulas",
        "formulas",
        true,
    ),
    ("pValues", "sheetPaste.values", "Values", "values", true),
    ("pFormats", "sheetPaste.formats", "Formats", "formats", true),
    (
        "pComments",
        "sheetPaste.comments",
        "Comments and Notes",
        "",
        false,
    ),
    (
        "pValidation",
        "sheetPaste.validation",
        "Validation",
        "",
        false,
    ),
    (
        "pTheme",
        "sheetPaste.theme",
        "All using Source theme",
        "",
        false,
    ),
    (
        "pNoBorders",
        "sheetPaste.noBorders",
        "All except borders",
        "all",
        true,
    ),
    ("pWidths", "sheetPaste.widths", "Column widths", "", false),
    (
        "pFormulasNum",
        "sheetPaste.formulasNum",
        "Formulas and number formats",
        "",
        false,
    ),
    (
        "pValuesNum",
        "sheetPaste.valuesNum",
        "Values and number formats",
        "",
        false,
    ),
    (
        "pMergeCf",
        "sheetPaste.mergeCf",
        "All merging conditional formats",
        "",
        false,
    ),
];

/// The Operation choices: (control id, label key, fallback, engine `operation`).
const OPERATION: [(&str, &str, &str, &str); 5] = [
    ("oNone", "sheetPaste.none", "None", "none"),
    ("oAdd", "sheetPaste.add", "Add", "add"),
    ("oSubtract", "sheetPaste.subtract", "Subtract", "subtract"),
    ("oMultiply", "sheetPaste.multiply", "Multiply", "multiply"),
    ("oDivide", "sheetPaste.divide", "Divide", "divide"),
];

impl SheetWindow {
    /// Open Paste Special (`false` when nothing was copied in this window).
    pub(super) fn open_paste_special(&mut self) -> bool {
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetPaste.title", "Paste Special"),
            (470.0, 440.0),
            vec![],
        );
        f.add(
            "pasteGroup",
            None,
            [16.0, 52.0, 438.0, 196.0],
            Kind::Group(t("sheetPaste.paste", "Paste")),
        );
        for (i, (id, key, fallback, _, ok)) in PASTE.iter().enumerate() {
            let (col, row) = (i / 6, i % 6);
            f.add(
                id,
                None,
                [
                    32.0 + col as f32 * 206.0,
                    74.0 + row as f32 * 28.0,
                    200.0,
                    22.0,
                ],
                Kind::Check {
                    label: t(key, fallback),
                    on: i == 0,
                },
            );
            f.enable(id, *ok);
        }
        f.add(
            "opGroup",
            None,
            [16.0, 258.0, 438.0, 100.0],
            Kind::Group(t("sheetPaste.operation", "Operation")),
        );
        for (i, (id, key, fallback, _)) in OPERATION.iter().enumerate() {
            let (col, row) = (i / 3, i % 3);
            f.add(
                id,
                None,
                [
                    32.0 + col as f32 * 206.0,
                    280.0 + row as f32 * 26.0,
                    180.0,
                    22.0,
                ],
                Kind::Check {
                    label: t(key, fallback),
                    on: i == 0,
                },
            );
        }
        f.add(
            "skipBlanks",
            None,
            [32.0, 366.0, 180.0, 22.0],
            Kind::Check {
                label: t("sheetPaste.skipBlanks", "Skip blanks"),
                on: false,
            },
        );
        f.add(
            "transpose",
            None,
            [238.0, 366.0, 180.0, 22.0],
            Kind::Check {
                label: t("sheetPaste.transpose", "Transpose"),
                on: false,
            },
        );
        f.add(
            "link",
            None,
            [16.0, 400.0, 120.0, 28.0],
            Kind::Button {
                label: t("sheetPaste.pasteLink", "Paste Link"),
                default: false,
            },
        );
        f.enable("link", false);
        let (ok, cancel) = (t("common.ok", "OK"), t("common.cancel", "Cancel"));
        f.add(
            "ok",
            None,
            [222.0, 400.0, 112.0, 28.0],
            Kind::Button {
                label: ok,
                default: true,
            },
        );
        f.add(
            "cancel",
            None,
            [342.0, 400.0, 112.0, 28.0],
            Kind::Button {
                label: cancel,
                default: false,
            },
        );
        self.form = Some(f);
        self.form_kind = FormKind::PasteSpecial;
        true
    }

    /// Events of Paste Special: the Paste and Operation choices act as option buttons.
    pub(super) fn paste_special_event(&mut self, ev: &Event) {
        match ev {
            Event::Changed(id) => {
                let group: Vec<&str> = if PASTE.iter().any(|p| p.0 == *id) {
                    PASTE.iter().map(|p| p.0).collect()
                } else if OPERATION.iter().any(|o| o.0 == *id) {
                    OPERATION.iter().map(|o| o.0).collect()
                } else {
                    return;
                };
                if let Some(f) = self.form.as_mut() {
                    for other in group {
                        f.set_on(other, other == *id);
                    }
                }
            }
            Event::Ok => {
                let Some(f) = self.form.as_ref() else { return };
                let pick = PASTE.iter().find(|p| f.on(p.0)).map_or(PASTE[0], |p| *p);
                let op = OPERATION.iter().find(|o| f.on(o.0)).map_or("none", |o| o.3);
                let (r, c) = self.view.active();
                let cmd = json!({
                    "cmd": "paste", "row": r, "col": c, "mode": pick.3, "operation": op,
                    "transpose": f.on("transpose"), "skipBlanks": f.on("skipBlanks"),
                    "noBorders": pick.0 == "pNoBorders", "range": self.view.rng(),
                });
                if self.view.run(cmd) {
                    self.form = None;
                } else {
                    self.status = self.view.take_error().unwrap_or_default();
                    self.form = None;
                }
            }
            Event::Cancel => self.form = None,
            _ => {}
        }
    }
}
