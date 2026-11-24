//! Dialogs of BlueDoc round 24 (docs/ui/REPORT.md "Vòng 114"): Drop Cap (§17.3.1.11 framePr
//! dropCap / lines / hSpace, font of the letter), Hyphenation (§17.15.1.10 autoHyphenation,
//! §17.15.1.53 hyphenationZone, §17.15.1.21 consecutiveHyphenLimit, §17.15.1.14
//! doNotHyphenateCaps), Manual Hyphenation (an optional hyphen, §17.3.3.29 softHyphen, at a
//! chosen point of each line-start word) and the Building Blocks Organizer (§17.12 glossary
//! document parts: Insert, Delete, Edit Properties…). Laid out after the reference's dialogs.
//! A child module of `doc_forms`.
use super::{DocForm, DocFormKind, FormContext, FormOut};
use crate::strings::Strings;
use serde_json::{json, Value};

fn t(s: &Strings, key: &str, fallback: &str) -> String {
    let v = s.get(key);
    if v == key {
        fallback.to_owned()
    } else {
        v.to_owned()
    }
}

/// Which round-24 dialog `kind` is.
pub(super) fn handles(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::DropCap24
            | DocFormKind::Hyphenation24
            | DocFormKind::ManualHyph24
            | DocFormKind::Organizer24
    )
}

/// Points of `text` ("1 pt", "0,5", "12") rounded to twips.
fn twips_of(text: &str) -> Option<i32> {
    let v: f32 = text
        .trim()
        .trim_end_matches("pt")
        .trim()
        .replace(',', ".")
        .parse()
        .ok()?;
    Some((v * 20.0).round() as i32)
}

fn pt_text(twips: i32) -> String {
    let v = twips as f32 / 20.0;
    if (v - v.round()).abs() < 0.01 {
        format!("{} pt", v.round() as i32)
    } else {
        format!("{v:.1} pt")
    }
}

/// The word `w` with a hyphen at byte offset `at` (and `·` at the other points).
pub(crate) fn hyphenated(word: &str, points: &[usize], at: usize) -> String {
    let mut out = String::new();
    for (i, c) in word.char_indices() {
        if i == at {
            out.push('-');
        } else if points.contains(&i) {
            out.push('\u{b7}');
        }
        out.push(c);
    }
    out
}

pub(super) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, f: &str| t(s, k, f);
    let x = &c.extra["r24"];
    match kind {
        DocFormKind::DropCap24 => {
            let mut f = DocForm::new(
                kind,
                &tr("doc24.dropCapTitle", "Drop Cap"),
                (360.0, 330.0),
                vec![],
            );
            f.group(
                "posG",
                [16.0, 46.0, 328.0, 18.0],
                &tr("doc24.position", "Position"),
            );
            let pos = x["position"].as_str().unwrap_or("none");
            for (i, (id, k, en)) in [
                ("none", "doc24.dcNone", "None"),
                ("drop", "doc24.dcDropped", "Dropped"),
                ("margin", "doc24.dcMargin", "In margin"),
            ]
            .iter()
            .enumerate()
            {
                f.radio(
                    id,
                    "pos",
                    [30.0 + 104.0 * i as f32, 74.0, 100.0, 20.0],
                    &tr(k, en),
                    pos == *id,
                );
            }
            f.group(
                "optG",
                [16.0, 110.0, 328.0, 18.0],
                &tr("doc24.options", "Options"),
            );
            f.label(
                "fontL",
                [30.0, 138.0, 200.0, 18.0],
                &tr("doc24.font", "Font:"),
            );
            let mut fonts = c.fonts.clone();
            let cur = x["font"].as_str().unwrap_or(&c.font).to_owned();
            if !fonts.contains(&cur) {
                fonts.insert(0, cur.clone());
            }
            let fi = fonts.iter().position(|n| *n == cur).unwrap_or(0);
            f.combo("font", [30.0, 160.0, 300.0, 26.0], fonts, fi);
            f.label(
                "linesL",
                [30.0, 196.0, 160.0, 18.0],
                &tr("doc24.linesToDrop", "Lines to drop:"),
            );
            let lines = x["lines"].as_i64().unwrap_or(3) as i32;
            f.spin("lines", [200.0, 192.0, 130.0, 26.0], lines, 1, 10);
            f.label(
                "distL",
                [30.0, 232.0, 160.0, 18.0],
                &tr("doc24.distance", "Distance from text:"),
            );
            let dist = x["distance"].as_i64().unwrap_or(0) as i32;
            f.edit("distance", [200.0, 228.0, 130.0, 26.0], &pt_text(dist));
            if pos == "none" {
                for id in ["font", "lines", "distance"] {
                    f.disable(id);
                }
            }
            f.ok_cancel(s);
            f
        }
        DocFormKind::Hyphenation24 => {
            let mut f = DocForm::new(
                kind,
                &tr("doc24.hyphTitle", "Hyphenation"),
                (360.0, 280.0),
                vec![],
            );
            f.check(
                "auto",
                [18.0, 50.0, 320.0, 20.0],
                &tr("doc24.autoHyph", "Automatically hyphenate document"),
                x["auto"] == true,
            );
            f.check(
                "caps",
                [18.0, 78.0, 320.0, 20.0],
                &tr("doc24.hyphCaps", "Hyphenate words in CAPS"),
                x["noCaps"] != true,
            );
            f.label(
                "zoneL",
                [18.0, 116.0, 190.0, 18.0],
                &tr("doc24.hyphZone", "Hyphenation zone:"),
            );
            let zone = x["zone"].as_i64().unwrap_or(360) as i32;
            f.edit("zone", [214.0, 112.0, 126.0, 26.0], &pt_text(zone));
            f.label(
                "limitL",
                [18.0, 152.0, 190.0, 18.0],
                &tr("doc24.hyphLimit", "Limit consecutive hyphens to:"),
            );
            let limit = x["limit"].as_i64().unwrap_or(0);
            let mut items = vec![tr("doc24.noLimit", "No limit")];
            items.extend((1..=10).map(|n| n.to_string()));
            f.combo(
                "limit",
                [214.0, 148.0, 126.0, 26.0],
                items,
                (limit.clamp(0, 10)) as usize,
            );
            f.button(
                "manual",
                [18.0, 228.0, 110.0, 28.0],
                &tr("doc24.manual", "Manual..."),
                false,
            );
            f.ok_cancel(s);
            f
        }
        DocFormKind::ManualHyph24 => {
            let mut f = DocForm::new(
                kind,
                &tr(
                    "doc24.manualTitle",
                    "Manual Hyphenation: English (United States)",
                ),
                (420.0, 170.0),
                vec![],
            );
            // Candidates: "para|offset|word|p1,p2,…" (byte offsets of the points).
            f.preview = x["words"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            f.label(
                "atL",
                [18.0, 54.0, 120.0, 18.0],
                &tr("doc24.hyphAt", "Hyphenate at:"),
            );
            f.combo("at", [130.0, 50.0, 272.0, 26.0], Vec::new(), 0);
            f.button(
                "yes",
                [56.0, 110.0, 100.0, 28.0],
                &tr("doc24.yes", "Yes"),
                true,
            );
            f.button(
                "no",
                [164.0, 110.0, 100.0, 28.0],
                &tr("doc24.no", "No"),
                false,
            );
            f.button(
                "cancel",
                [272.0, 110.0, 100.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f.label_w = vec![0.0];
            f.r24_show_word();
            f
        }
        DocFormKind::Organizer24 => {
            let mut f = DocForm::new(
                kind,
                &tr("doc24.organizerTitle", "Building Blocks Organizer"),
                (720.0, 470.0),
                vec![],
            );
            f.label(
                "blocksL",
                [16.0, 46.0, 400.0, 18.0],
                &tr("doc24.blocks", "Building blocks:"),
            );
            let rows: Vec<(String, String, String, String, String)> = x["blocks"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|b| {
                            let s = |k: &str| b[k].as_str().unwrap_or("").to_owned();
                            (s("key"), s("name"), s("gallery"), s("category"), s("store"))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let template = |store: &str| match store {
                "user" => tr("doc24.storeUser", "Building Blocks"),
                "document" => tr("doc24.storeDoc", "This document"),
                _ => tr("doc24.storeBuiltIn", "Built-In Building Blocks"),
            };
            // Name, Gallery, Category, Template as the reference's columns (tab-separated).
            let items: Vec<String> = rows
                .iter()
                .map(|r| format!("{}\t{}\t{}\t{}", r.1, r.2, r.3, template(&r.4)))
                .collect();
            f.ids = rows.iter().map(|r| r.0.clone()).collect();
            let sel = (!items.is_empty()).then_some(0);
            f.list("list", [16.0, 68.0, 450.0, 328.0], items, sel);
            f.label(
                "previewL",
                [482.0, 46.0, 220.0, 18.0],
                &tr(
                    "doc24.clickPreview",
                    "Click a building block to see its preview",
                ),
            );
            f.button(
                "insert",
                [16.0, 412.0, 100.0, 28.0],
                &tr("doc24.insert", "Insert"),
                false,
            );
            f.button(
                "delete",
                [124.0, 412.0, 100.0, 28.0],
                &tr("doc24.delete", "Delete"),
                false,
            );
            f.button(
                "editProps",
                [232.0, 412.0, 140.0, 28.0],
                &tr("doc24.editProps", "Edit Properties..."),
                false,
            );
            f.close_only(s);
            f
        }
        _ => DocForm::new(kind, "", (300.0, 200.0), vec![]),
    }
}

impl DocForm {
    /// Manual Hyphenation: show the current candidate word's points in the box.
    fn r24_show_word(&mut self) {
        let i = self.label_w.first().map_or(0, |v| *v as usize);
        let Some(row) = self.preview.get(i).cloned() else {
            self.set_items("at", Vec::new());
            return;
        };
        let parts: Vec<&str> = row.splitn(4, '|').collect();
        let word = parts.get(2).copied().unwrap_or("");
        let points: Vec<usize> = parts
            .get(3)
            .map(|p| p.split(',').filter_map(|v| v.parse().ok()).collect())
            .unwrap_or_default();
        let items: Vec<String> = points
            .iter()
            .rev()
            .map(|&at| hyphenated(word, &points, at))
            .collect();
        self.set_items("at", items);
    }

    fn set_items(&mut self, id: &str, items: Vec<String>) {
        if let Some(c) = self.form.get_mut(id) {
            if let super::Kind::Combo { items: v, sel } = &mut c.kind {
                *v = items;
                *sel = 0;
            }
        }
    }

    /// Manual Hyphenation: the next word (none left: the dialog closes).
    fn r24_next_word(&mut self, cmds: Vec<Value>) -> FormOut {
        let i = self.label_w.first().map_or(0, |v| *v as usize) + 1;
        if i >= self.preview.len() {
            return FormOut::Run(cmds, true);
        }
        self.label_w = vec![i as f32];
        self.r24_show_word();
        if cmds.is_empty() {
            FormOut::Redraw
        } else {
            FormOut::Run(cmds, false)
        }
    }

    pub(crate) fn r24_changed(&mut self, id: &str) {
        if self.kind == DocFormKind::DropCap24 && matches!(id, "none" | "drop" | "margin") {
            let on = self.chosen("pos") != Some("none");
            for c in ["font", "lines", "distance"] {
                self.form.enable(c, on);
            }
        }
    }

    pub(super) fn r24_clicked(&mut self, id: &str) -> Option<FormOut> {
        if !handles(self.kind) {
            return None;
        }
        Some(match (self.kind, id) {
            (DocFormKind::Hyphenation24, "manual") => FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "r24.manualHyph" })],
                true,
            ),
            (DocFormKind::ManualHyph24, "yes") => {
                let i = self.label_w.first().map_or(0, |v| *v as usize);
                let row = self.preview.get(i).cloned().unwrap_or_default();
                let parts: Vec<&str> = row.splitn(4, '|').collect();
                let para: usize = parts.first().and_then(|v| v.parse().ok()).unwrap_or(0);
                let start: usize = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                let mut points: Vec<usize> = parts
                    .get(3)
                    .map(|p| p.split(',').filter_map(|v| v.parse().ok()).collect())
                    .unwrap_or_default();
                points.reverse();
                let at = points.get(self.sel("at").unwrap_or(0)).copied();
                let Some(at) = at else {
                    return Some(self.r24_next_word(Vec::new()));
                };
                // Later candidates of the same paragraph move by the hyphen's two bytes.
                for r in self.preview.iter_mut().skip(i + 1) {
                    let mut p: Vec<String> = r.splitn(4, '|').map(str::to_owned).collect();
                    if p.first().and_then(|v| v.parse::<usize>().ok()) == Some(para) {
                        if let Some(o) = p.get_mut(1) {
                            let v: usize = o.parse().unwrap_or(0);
                            if v > start + at {
                                *o = (v + 2).to_string();
                            }
                        }
                        *r = p.join("|");
                    }
                }
                let cmds = vec![
                    json!({ "cmd": "setSelection", "selection": {
                        "anchor": { "p": para, "o": start + at },
                        "focus": { "p": para, "o": start + at } } }),
                    json!({ "cmd": "insertText", "text": "\u{ad}" }),
                ];
                self.r24_next_word(cmds)
            }
            (DocFormKind::ManualHyph24, "no") => self.r24_next_word(Vec::new()),
            (DocFormKind::Organizer24, "insert" | "delete" | "editProps") => {
                let Some(key) = self.sel("list").and_then(|i| self.ids.get(i)).cloned() else {
                    return Some(FormOut::Redraw);
                };
                match id {
                    "insert" => FormOut::Run(
                        vec![json!({ "cmd": "insertBuildingBlock", "key": key })],
                        true,
                    ),
                    "delete" => FormOut::Run(
                        vec![json!({ "cmd": "ui.action", "id": format!("r24.bbDelete:{key}") })],
                        false,
                    ),
                    _ => FormOut::Run(
                        vec![json!({ "cmd": "ui.action", "id": format!("r24.bbEdit:{key}") })],
                        true,
                    ),
                }
            }
            _ => return None,
        })
    }

    pub(super) fn r24_ok(&mut self) -> Option<FormOut> {
        if !handles(self.kind) {
            return None;
        }
        Some(match self.kind {
            DocFormKind::DropCap24 => {
                let pos = self.chosen("pos").unwrap_or("none");
                let spec = (pos != "none").then(|| {
                    let font = self.text("font");
                    json!({
                        "position": pos,
                        "font": (!font.trim().is_empty()).then(|| font.trim().to_owned()),
                        "lines": self.spin_value("lines").clamp(1, 10),
                        "distance": twips_of(&self.text("distance")).unwrap_or(0).clamp(0, 31_680),
                    })
                });
                FormOut::Run(vec![json!({ "cmd": "setDropCap", "dropCap": spec })], true)
            }
            DocFormKind::Hyphenation24 => {
                let zone = twips_of(&self.text("zone")).unwrap_or(360).clamp(0, 31_680);
                let limit = self.sel("limit").unwrap_or(0);
                FormOut::Run(
                    vec![json!({ "cmd": "setHyphenation", "hyphenation": {
                        "auto": self.on("auto"),
                        "zone": zone,
                        "consecutiveLimit": (limit > 0).then_some(limit),
                        "noCaps": !self.on("caps"),
                    } })],
                    true,
                )
            }
            _ => FormOut::Close,
        })
    }

    fn spin_value(&self, id: &str) -> i32 {
        self.text(id).trim().parse().unwrap_or(1)
    }
}
