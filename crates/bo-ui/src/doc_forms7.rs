//! BlueDoc dialogs of parity round 7 (docs/ui/REPORT.md rounds 93+), laid out after the
//! reference word processor's dialogs measured at 125 %: Alt Text for the selected picture or
//! shape, Encrypt Document (the engine's [MS-OFFCRYPTO] agile encryption on save), Document
//! Inspector (inspect, then Remove All per category) and the Compatibility Checker. A child
//! module of `doc_forms` so the dialog builder is at hand; every dialog ends in engine commands
//! (ADR-0008) or in a request to the window (`FormOut::Inspect`).
use super::{t, DocForm, DocFormKind, FormOut};
use crate::strings::Strings;
use serde_json::json;

/// Document Inspector categories: engine name, title key, title, description.
pub(crate) const INSPECT: [(&str, &str, &str, &str); 6] = [
    (
        "comments",
        "docR7.ins.comments",
        "Comments, Revisions, and Versions",
        "Inspects the document for comments and tracked changes.",
    ),
    (
        "properties",
        "docR7.ins.properties",
        "Document Properties and Personal Information",
        "Inspects for hidden metadata or personal information saved with the document.",
    ),
    (
        "customXml",
        "docR7.ins.customXml",
        "Custom XML Data",
        "Inspects for custom XML data stored with this document.",
    ),
    (
        "headersFooters",
        "docR7.ins.headers",
        "Headers, Footers, and Watermarks",
        "Inspects the document for information in headers, footers, and watermarks.",
    ),
    (
        "hiddenText",
        "docR7.ins.hiddenText",
        "Hidden Text",
        "Inspects the document for text that has been formatted as hidden.",
    ),
    (
        "macros",
        "docR7.ins.macros",
        "Macros, Forms, and ActiveX Controls",
        "Inspects for macros, forms, and ActiveX controls.",
    ),
];

const CHECK_IDS: [&str; 6] = ["c0", "c1", "c2", "c3", "c4", "c5"];
const RESULT_IDS: [&str; 6] = ["r0", "r1", "r2", "r3", "r4", "r5"];
const DETAIL_IDS: [&str; 6] = ["d0", "d1", "d2", "d3", "d4", "d5"];
const REMOVE_IDS: [&str; 6] = ["x0", "x1", "x2", "x3", "x4", "x5"];

/// Whether `kind` is a round-7 dialog.
pub(super) fn handles(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::AltText
            | DocFormKind::Encrypt
            | DocFormKind::Inspect
            | DocFormKind::Compatibility
    )
}

/// Build round-7 dialog `kind`.
pub(super) fn open(kind: DocFormKind, s: &Strings) -> DocForm {
    let tr = |k: &str, f: &str| t(s, k, f);
    match kind {
        // A pane on the reference; a dialog of the same contents here.
        DocFormKind::AltText => {
            let mut f = DocForm::new(
                kind,
                &tr("docR7.alt.title", "Alt Text"),
                (460.0, 300.0),
                vec![],
            );
            f.label(
                "hint",
                [16.0, 48.0, 428.0, 40.0],
                &tr(
                    "docR7.alt.hint",
                    "How would you describe this object and its context to someone who is blind or has low vision?",
                ),
            );
            f.edit("alt", [16.0, 96.0, 428.0, 96.0], "");
            f.check(
                "decorative",
                [16.0, 204.0, 300.0, 22.0],
                &tr("docR7.alt.decorative", "Mark as decorative"),
                false,
            );
            f.ok_cancel(s);
            f
        }
        // Measured on the reference at 125 %: 340 × 254.
        DocFormKind::Encrypt => {
            let mut f = DocForm::new(
                kind,
                &tr("docR7.enc.title", "Encrypt Document"),
                (340.0, 254.0),
                vec![],
            );
            f.label(
                "head",
                [14.0, 44.0, 310.0, 18.0],
                &tr("docR7.enc.head", "Encrypt the contents of this file"),
            );
            f.label(
                "pwL",
                [14.0, 68.0, 200.0, 18.0],
                &tr("docR7.enc.password", "Password:"),
            );
            f.edit("pw", [14.0, 88.0, 312.0, 24.0], "");
            f.form.masked.push("pw");
            f.label(
                "warn",
                [14.0, 120.0, 312.0, 72.0],
                &tr(
                    "docR7.enc.warn",
                    "Caution: If you lose or forget the password, it cannot be recovered. Passwords are case-sensitive.",
                ),
            );
            f.ok_cancel(s);
            f
        }
        // Measured on the reference at 125 %: 553 × 638 (the inspector list).
        DocFormKind::Inspect => {
            let mut f = DocForm::new(
                kind,
                &tr("docR7.ins.title", "Document Inspector"),
                (553.0, 560.0),
                vec![],
            );
            f.label(
                "intro",
                [14.0, 44.0, 520.0, 18.0],
                &tr(
                    "docR7.ins.intro",
                    "To check the document for the selected content, click Inspect.",
                ),
            );
            for (i, (_, key, title, desc)) in INSPECT.iter().enumerate() {
                let y = 74.0 + i as f32 * 66.0;
                f.check(CHECK_IDS[i], [24.0, y, 340.0, 20.0], &tr(key, title), true);
                f.label(
                    DETAIL_IDS[i],
                    [44.0, y + 22.0, 470.0, 36.0],
                    &tr(&format!("{key}Desc"), desc),
                );
            }
            let (w, h) = f.form.size;
            f.button(
                "inspect",
                [w - 250.0, h - 48.0, 112.0, 28.0],
                &tr("docR7.ins.inspect", "Inspect"),
                true,
            );
            f.button(
                "cancel",
                [w - 131.0, h - 48.0, 112.0, 28.0],
                &tr("docR3.close", "Close"),
                false,
            );
            f
        }
        _ => {
            let mut f = DocForm::new(
                kind,
                &tr("docR7.cmp.title", "Microsoft Word Compatibility Checker"),
                (560.0, 420.0),
                vec![],
            );
            f.label(
                "intro",
                [14.0, 44.0, 530.0, 36.0],
                &tr(
                    "docR7.cmp.intro",
                    "The following features in this document are not supported by earlier versions of the word processor format. These features might be lost or degraded when you save this document in an earlier file format.",
                ),
            );
            f.list("issues", [14.0, 96.0, 532.0, 250.0], Vec::new(), None);
            f.check(
                "onSave",
                [14.0, 356.0, 400.0, 20.0],
                &tr(
                    "docR7.cmp.onSave",
                    "Check compatibility when saving documents",
                ),
                true,
            );
            f.close_only(s);
            f
        }
    }
}

impl DocForm {
    /// OK of a round-7 dialog.
    pub(super) fn r7_ok(&mut self) -> FormOut {
        match self.kind {
            DocFormKind::AltText => {
                let (Some(para), Some(at)) = (
                    self.ids.first().and_then(|v| v.parse::<usize>().ok()),
                    self.ids.get(1).and_then(|v| v.parse::<usize>().ok()),
                ) else {
                    return FormOut::Close;
                };
                let text = if self.on("decorative") {
                    String::new()
                } else {
                    self.text("alt")
                };
                FormOut::Run(
                    vec![json!({
                        "cmd": "inspectFix",
                        "fix": {
                            "kind": "altText",
                            "target": { "kind": "drawing", "para": para, "at": at },
                            "text": text,
                        },
                    })],
                    true,
                )
            }
            // An empty password removes the encryption (the reference's behaviour).
            DocFormKind::Encrypt => FormOut::Run(
                vec![json!({ "cmd": "setSavePassword", "password": self.text("pw") })],
                true,
            ),
            DocFormKind::Inspect => self.r7_clicked("inspect"),
            _ => FormOut::Close,
        }
    }

    /// A button of a round-7 dialog.
    pub(super) fn r7_clicked(&mut self, id: &str) -> FormOut {
        if self.kind != DocFormKind::Inspect {
            return FormOut::Redraw;
        }
        if id == "inspect" {
            let cats = INSPECT
                .iter()
                .enumerate()
                .filter(|(i, _)| self.on(CHECK_IDS[*i]))
                .map(|(_, c)| c.0.to_owned())
                .collect();
            return FormOut::Inspect(cats, None);
        }
        if let Some(i) = REMOVE_IDS.iter().position(|r| *r == id) {
            return FormOut::Inspect(Vec::new(), Some(INSPECT[i].0.to_owned()));
        }
        FormOut::Redraw
    }

    /// Show inspection results: per category, what was found (`None`: nothing) — a
    /// "Remove All" button where something can be removed, as the reference's results.
    pub(crate) fn r7_results(&mut self, s: &Strings, found: &[(usize, Option<String>, bool)]) {
        let tr = |k: &str, f: &str| t(s, k, f);
        self.set_label(
            "intro",
            &tr("docR7.ins.review", "Review the inspection results."),
        );
        for &(i, ref what, removable) in found {
            let Some(cid) = CHECK_IDS.get(i) else {
                continue;
            };
            self.form.enable(cid, false);
            let text = what
                .clone()
                .unwrap_or_else(|| tr("docR7.ins.nothing", "No items were found."));
            self.set_label(DETAIL_IDS[i], &text);
            if removable && what.is_some() && self.form.get(REMOVE_IDS[i]).is_none() {
                let y = 74.0 + i as f32 * 66.0;
                self.button(
                    REMOVE_IDS[i],
                    [420.0, y + 4.0, 112.0, 28.0],
                    &tr("docR7.ins.removeAll", "Remove All"),
                    false,
                );
            }
            if what.is_none() {
                if self.form.get(REMOVE_IDS[i]).is_some() {
                    self.form.enable(REMOVE_IDS[i], false);
                }
                self.set_label(RESULT_IDS.get(i).copied().unwrap_or("r0"), "");
            }
        }
        if let Some(super::Kind::Button { label, .. }) =
            self.form.get_mut("inspect").map(|c| &mut c.kind)
        {
            *label = tr("docR7.ins.reinspect", "Reinspect");
        }
    }

    /// Fill the Compatibility Checker's list.
    pub(crate) fn r7_issues(&mut self, s: &Strings, issues: Vec<String>) {
        let items = if issues.is_empty() {
            vec![t(
                s,
                "docR7.cmp.none",
                "No compatibility issues were found.",
            )]
        } else {
            issues
        };
        self.form.set_items("issues", items, None);
    }
}
