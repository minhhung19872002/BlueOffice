//! Review › Protect Sheet / Protect Workbook as the reference's dialogs (round 47, measured
//! at 125 %: Protect Sheet 427 × 534 px with its larger text, a password box, "Protect
//! worksheet and contents of locked cells", a scrolling list of 15 permissions 31 px apart;
//! `show/sheet-44`), the password confirmation, and Unprotect Sheet / Workbook asking for the
//! password when there is one. The engine's `protectSheet` (with `allow`),
//! `unprotectSheet`, `protectWorkbook`, `unprotectWorkbook` do the work.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use serde_json::{json, Value};

/// The permissions: (engine field, label key, fallback, access letter index).
const PERMS: [(&str, &str, &str, usize); 15] = [
    (
        "selectLockedCells",
        "sheetProt.selLocked",
        "Select locked cells",
        7,
    ),
    (
        "selectUnlockedCells",
        "sheetProt.selUnlocked",
        "Select unlocked cells",
        7,
    ),
    ("formatCells", "sheetProt.fmtCells", "Format cells", 0),
    ("formatColumns", "sheetProt.fmtCols", "Format columns", 3),
    ("formatRows", "sheetProt.fmtRows", "Format rows", 7),
    ("insertColumns", "sheetProt.insCols", "Insert columns", 7),
    ("insertRows", "sheetProt.insRows", "Insert rows", 9),
    (
        "insertHyperlinks",
        "sheetProt.insLinks",
        "Insert hyperlinks",
        7,
    ),
    ("deleteColumns", "sheetProt.delCols", "Delete columns", 0),
    ("deleteRows", "sheetProt.delRows", "Delete rows", 7),
    ("sort", "sheetProt.sort", "Sort", 0),
    ("autoFilter", "sheetProt.autoFilter", "Use AutoFilter", 4),
    (
        "pivotTables",
        "sheetProt.pivot",
        "Use PivotTable & PivotChart",
        4,
    ),
    ("objects", "sheetProt.objects", "Edit objects", 5),
    ("scenarios", "sheetProt.scenarios", "Edit scenarios", 5),
];
/// Check box ids of the permissions.
const IDS: [&str; 15] = [
    "p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9", "p10", "p11", "p12", "p13", "p14",
];
/// Rows of the permission list on screen, their pitch and the list's top.
const ROWS: usize = 9;
const PITCH: f32 = 31.0;
const LIST_TOP: f32 = 184.0;

/// What the password dialogs are for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ProtStep {
    #[default]
    ProtectSheet,
    ProtectWorkbook,
    /// Confirm Password of a sheet / workbook protection.
    ConfirmSheet,
    ConfirmWorkbook,
    UnprotectSheet,
    UnprotectWorkbook,
}

/// State of the open protection dialog.
#[derive(Debug, Clone, Default)]
pub(super) struct ProtState {
    pub step: ProtStep,
    /// First scrolled permission row.
    pub top: usize,
    /// The password typed first, and the permissions chosen (waiting for confirmation).
    pub password: String,
    pub allow: Value,
}

impl SheetWindow {
    fn protection_info(&mut self) -> Value {
        let sheet = self.view.sheet();
        self.view
            .query(json!({ "cmd": "protection", "sheet": sheet }))
            .unwrap_or(Value::Null)
    }

    /// Review › Protect Sheet (or Unprotect Sheet when the sheet is protected).
    pub(super) fn open_protect_sheet(&mut self) -> bool {
        let info = self.protection_info();
        let sheet = &info["sheet"];
        if sheet["protected"].as_bool().unwrap_or(false) {
            if sheet["hasPassword"].as_bool().unwrap_or(false) {
                self.open_unprotect(ProtStep::UnprotectSheet);
            } else {
                self.unprotect_now(false, "");
            }
            return true;
        }
        let allow = sheet.get("allow").cloned().unwrap_or(Value::Null);
        let t = |k: &str, f: &str| self.t(k, f);
        let mut f = Form::new(
            &t("sheetProt.title", "Protect Sheet"),
            (427.0, 534.0),
            vec![],
        );
        f.set_font_scale(1.3);
        // The reference's newer dialogs have a white face.
        f.add("face", None, [1.0, 38.0, 425.0, 495.0], Kind::Canvas);
        f.add(
            "pwL",
            None,
            [17.0, 47.0, 390.0, 26.0],
            Kind::Label(t("sheetProt.password", "Password to unprotect sheet:")),
        );
        f.set_access("pwL", 0);
        f.add(
            "pw",
            None,
            [17.0, 77.0, 393.0, 33.0],
            Kind::Edit {
                value: Vec::new(),
                caret: 0,
            },
        );
        f.masked.push("pw");
        let contents = t(
            "sheetProt.contents",
            "Protect worksheet and contents of locked cells",
        );
        let at = contents
            .find("contents")
            .map_or(0, |b| contents[..b].chars().count());
        f.add(
            "lock",
            None,
            [17.0, 119.0, 400.0, 26.0],
            Kind::Check {
                label: contents,
                on: true,
            },
        );
        f.set_access("lock", at);
        f.add(
            "allowL",
            None,
            [17.0, 152.0, 390.0, 26.0],
            Kind::Label(t(
                "sheetProt.allowAll",
                "Allow all users of this worksheet to:",
            )),
        );
        f.set_access("allowL", 1);
        for (i, (key, lk, fb, acc)) in PERMS.iter().enumerate() {
            let on = allow.get(*key).and_then(Value::as_bool).unwrap_or(i < 2);
            f.add(
                IDS[i],
                None,
                [17.0, LIST_TOP - 3.0 + PITCH * i as f32, 360.0, 28.0],
                Kind::Check {
                    label: t(lk, fb),
                    on,
                },
            );
            f.set_access(IDS[i], *acc);
        }
        // Edit scenarios has no engine flag yet.
        f.enable("p14", false);
        f.add(
            "pscroll",
            None,
            [390.0, LIST_TOP + 1.0, 19.0, 288.0],
            Kind::Canvas,
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [255.0, 482.0, 71.0, 33.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [340.0, 482.0, 71.0, 33.0];
        }
        f.focus("pw");
        self.form = Some(f);
        self.form_kind = FormKind::Protect;
        self.prot = ProtState::default();
        self.prot_scroll(0);
        true
    }

    /// Show permission rows `top`… of the list.
    fn prot_scroll(&mut self, top: usize) {
        let top = top.min(IDS.len() - ROWS);
        self.prot.top = top;
        let Some(f) = self.form.as_mut() else {
            return;
        };
        for (i, id) in IDS.iter().enumerate() {
            let shown = (top..top + ROWS).contains(&i);
            f.show(id, shown);
            if let Some(c) = f.get_mut(id) {
                c.rect[1] = LIST_TOP - 3.0 + PITCH * (i as f32 - top as f32);
            }
        }
    }

    /// Wheel over the open Protect Sheet list.
    pub(super) fn prot_wheel(&mut self, delta: f32) -> bool {
        if self.form_kind != FormKind::Protect || self.prot.step != ProtStep::ProtectSheet {
            return false;
        }
        let top = if delta > 0.0 {
            self.prot.top + 1
        } else {
            self.prot.top.saturating_sub(1)
        };
        self.prot_scroll(top);
        true
    }

    /// Review › Protect Workbook (or Unprotect when the structure is locked).
    pub(super) fn open_protect_workbook(&mut self) -> bool {
        let info = self.protection_info();
        let wb = &info["workbook"];
        if wb["structure"].as_bool().unwrap_or(false) {
            if wb["hasPassword"].as_bool().unwrap_or(false) {
                self.open_unprotect(ProtStep::UnprotectWorkbook);
            } else {
                self.unprotect_now(true, "");
            }
            return true;
        }
        let t = |k: &str, f: &str| self.t(k, f);
        let mut f = Form::new(
            &t("sheetProt.wbTitle", "Protect Structure and Windows"),
            (330.0, 258.0),
            vec![],
        );
        f.set_font_scale(1.15);
        f.add(
            "forG",
            None,
            [17.0, 46.0, 296.0, 22.0],
            Kind::Group(t("sheetProt.wbFor", "Protect workbook for")),
        );
        f.add(
            "structure",
            None,
            [33.0, 74.0, 260.0, 24.0],
            Kind::Check {
                label: t("sheetProt.structure", "Structure"),
                on: true,
            },
        );
        f.set_access("structure", 0);
        f.add(
            "windows",
            None,
            [33.0, 100.0, 260.0, 24.0],
            Kind::Check {
                label: t("sheetProt.windows", "Windows"),
                on: false,
            },
        );
        f.set_access("windows", 0);
        f.enable("windows", false);
        f.add(
            "pwL",
            None,
            [17.0, 136.0, 290.0, 24.0],
            Kind::Label(t("sheetProt.wbPassword", "Password (optional):")),
        );
        f.set_access("pwL", 0);
        f.add(
            "pw",
            None,
            [17.0, 162.0, 296.0, 28.0],
            Kind::Edit {
                value: Vec::new(),
                caret: 0,
            },
        );
        f.masked.push("pw");
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [96.0, 210.0, 102.0, 30.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [211.0, 210.0, 102.0, 30.0];
        }
        f.focus("pw");
        self.form = Some(f);
        self.form_kind = FormKind::Protect;
        self.prot = ProtState {
            step: ProtStep::ProtectWorkbook,
            ..ProtState::default()
        };
        true
    }

    /// A one-box password dialog: Confirm Password or Unprotect Sheet / Workbook.
    fn open_unprotect(&mut self, step: ProtStep) {
        let t = |k: &str, f: &str| self.t(k, f);
        let confirm = matches!(step, ProtStep::ConfirmSheet | ProtStep::ConfirmWorkbook);
        let title = match step {
            ProtStep::UnprotectSheet => t("sheetProt.unprotectSheet", "Unprotect Sheet"),
            ProtStep::UnprotectWorkbook => t("sheetProt.unprotectWorkbook", "Unprotect Workbook"),
            _ => t("sheetProt.confirm", "Confirm Password"),
        };
        let (w, h) = if confirm {
            (402.0, 300.0)
        } else {
            (290.0, 150.0)
        };
        let mut f = Form::new(&title, (w, h), vec![]);
        let label = if confirm {
            t("sheetProt.reenter", "Reenter password to proceed.")
        } else {
            t("sheetProt.passwordShort", "Password:")
        };
        f.add("pwL", None, [17.0, 50.0, 200.0, 22.0], Kind::Label(label));
        f.set_access("pwL", 0);
        let (ex, ew) = if confirm {
            (17.0, 366.0)
        } else {
            (95.0, 178.0)
        };
        let ey = if confirm { 76.0 } else { 48.0 };
        f.add(
            "pw",
            None,
            [ex, ey, ew, 26.0],
            Kind::Edit {
                value: Vec::new(),
                caret: 0,
            },
        );
        f.masked.push("pw");
        if confirm {
            f.add(
                "caution",
                None,
                [17.0, 114.0, 370.0, 120.0],
                Kind::Note(t(
                    "sheetProt.caution",
                    "Caution: If you lose or forget the password, it cannot be recovered. It is advisable to keep a list of passwords and their corresponding workbook and sheet names in a safe place.  (Remember that passwords are case-sensitive.)",
                )),
            );
        }
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        let by = h - 46.0;
        if let Some(c) = f.get_mut("ok") {
            c.rect = [w - 218.0, by, 95.0, 28.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [w - 113.0, by, 95.0, 28.0];
        }
        f.focus("pw");
        let keep = std::mem::take(&mut self.prot);
        self.form = Some(f);
        self.form_kind = FormKind::Protect;
        self.prot = ProtState { step, ..keep };
    }

    fn unprotect_now(&mut self, workbook: bool, password: &str) -> bool {
        let sheet = self.view.sheet();
        let cmd = if workbook {
            json!({ "cmd": "unprotectWorkbook", "password": password })
        } else {
            json!({ "cmd": "unprotectSheet", "sheet": sheet, "password": password })
        };
        let ok = self.view.query(cmd).is_some();
        if ok {
            self.view.changed();
            self.status = if workbook {
                self.t(
                    "sheetMenu.workbookUnprotected",
                    "The workbook structure is no longer protected.",
                )
            } else {
                self.t(
                    "sheetMenu.sheetUnprotected",
                    "The sheet is no longer protected.",
                )
            };
        } else {
            // The reference's message instead of the engine's.
            let _ = self.view.take_error();
            self.status = self.t(
                "sheetProt.wrong",
                "The password you supplied is not correct. Verify that the CAPS LOCK key is off and be sure to use the correct capitalization.",
            );
        }
        ok
    }

    fn protect_now(&mut self, workbook: bool) {
        let sheet = self.view.sheet();
        let pw = self.prot.password.clone();
        let cmd = if workbook {
            json!({ "cmd": "protectWorkbook", "password": pw })
        } else {
            json!({ "cmd": "protectSheet", "sheet": sheet, "password": pw, "allow": self.prot.allow })
        };
        if self.view.query(cmd).is_some() {
            self.view.changed();
            self.status = if workbook {
                self.t(
                    "sheetMenu.workbookProtected",
                    "The workbook structure is protected.",
                )
            } else {
                self.t("sheetMenu.sheetProtected", "The sheet is protected.")
            };
        }
    }

    /// Events of the protection dialogs.
    pub(super) fn protect_event(&mut self, ev: &Event) {
        let step = self.prot.step;
        match ev {
            Event::Cancel | Event::Clicked("cancel") => self.form = None,
            Event::Canvas("pscroll", _, y) => {
                let top = if *y < 144.0 {
                    self.prot.top.saturating_sub(3)
                } else {
                    self.prot.top + 3
                };
                self.prot_scroll(top);
            }
            Event::Changed("lock") => {
                let on = self.form.as_ref().is_some_and(|f| f.on("lock"));
                if let Some(f) = self.form.as_mut() {
                    f.enable("ok", on);
                }
            }
            Event::Ok | Event::Clicked("ok") => {
                let Some(f) = self.form.as_ref() else {
                    return;
                };
                let pw = f.text("pw");
                match step {
                    ProtStep::ProtectSheet | ProtStep::ProtectWorkbook => {
                        let workbook = step == ProtStep::ProtectWorkbook;
                        if !workbook {
                            let mut allow = serde_json::Map::new();
                            for (i, (key, ..)) in PERMS.iter().enumerate().take(14) {
                                allow.insert((*key).to_owned(), json!(f.on(IDS[i])));
                            }
                            self.prot.allow = Value::Object(allow);
                        }
                        self.prot.password = pw.clone();
                        if pw.is_empty() {
                            self.form = None;
                            self.protect_now(workbook);
                        } else {
                            self.open_unprotect(if workbook {
                                ProtStep::ConfirmWorkbook
                            } else {
                                ProtStep::ConfirmSheet
                            });
                        }
                    }
                    ProtStep::ConfirmSheet | ProtStep::ConfirmWorkbook => {
                        self.form = None;
                        if pw == self.prot.password {
                            self.protect_now(step == ProtStep::ConfirmWorkbook);
                        } else {
                            self.status = self.t(
                                "sheetProt.mismatch",
                                "Confirmation password is not identical.",
                            );
                        }
                    }
                    ProtStep::UnprotectSheet | ProtStep::UnprotectWorkbook => {
                        self.form = None;
                        self.unprotect_now(step == ProtStep::UnprotectWorkbook, &pw);
                    }
                }
            }
            _ => {}
        }
    }

    /// The permission list's scroll bar (own drawing in the reference's greys).
    pub(super) fn paint_protect(
        &self,
        f: &Form,
        id: &str,
        r: [f32; 4],
        p: &mut dyn crate::Painter,
    ) {
        use bo_render::{PathSeg, Rgba};
        if id != "pscroll" {
            return;
        }
        let k = f.scale_k();
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xF0, 0xF0, 0xF0, 255));
        let grey = Rgba(0x60, 0x60, 0x60, 255);
        let (cx, a) = ((r[0] + r[2] / 2.0).round(), (4.0 * k).round());
        for (cy, up) in [(r[1] + 9.0 * k, true), (r[1] + r[3] - 9.0 * k, false)] {
            let s = if up { -1.0 } else { 1.0 };
            p.path(
                &[
                    PathSeg::M(cx - a, cy - s * a / 2.0),
                    PathSeg::L(cx + a, cy - s * a / 2.0),
                    PathSeg::L(cx, cy + s * a / 2.0),
                    PathSeg::Z,
                ],
                Some(grey),
                None,
            );
        }
        let track = r[3] - 36.0 * k;
        let n = IDS.len() as f32;
        let th = track * ROWS as f32 / n;
        let ty = r[1] + 18.0 * k + (track - th) * self.prot.top as f32 / (n - ROWS as f32);
        p.fill_rect(
            r[0] + 3.0 * k,
            ty,
            r[2] - 6.0 * k,
            th,
            Rgba(0x8A, 0x8A, 0x8A, 255),
        );
    }
}
