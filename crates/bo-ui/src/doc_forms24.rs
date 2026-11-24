//! Dialogs of BlueDoc round 24 (docs/ui/REPORT.md "Vòng 114"): Drop Cap (§17.3.1.11 framePr
//! dropCap / lines / hSpace, font of the letter), Hyphenation (§17.15.1.10 autoHyphenation,
//! §17.15.1.53 hyphenationZone, §17.15.1.21 consecutiveHyphenLimit, §17.15.1.14
//! doNotHyphenateCaps), Manual Hyphenation (an optional hyphen, §17.3.3.29 softHyphen, at a
//! chosen point of each line-start word) and the Building Blocks Organizer (§17.12 glossary
//! document parts: Insert, Delete, Edit Properties…). Laid out after the reference's dialogs.
//! A child module of `doc_forms`.
use super::{DocForm, DocFormKind, FormContext, FormOut};
use crate::strings::Strings;
use bo_render::PathSeg;
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
            // The reference's dialog at 125 %: 321 × 390 px, three position tiles with
            // their names under them, then Font / Lines to drop / Distance from text.
            let mut f = DocForm::new(
                kind,
                &tr("doc24.dropCapTitle", "Drop Cap"),
                (321.0, 390.0),
                vec![],
            );
            f.group(
                "posG",
                [14.0, 46.0, 293.0, 18.0],
                &tr("doc24.position", "Position"),
            );
            let pos = x["position"].as_str().unwrap_or("none");
            // Picture tiles to click (the chosen one framed) with their names under them,
            // as the reference (no radio buttons).
            for (i, (lab, tile, k, en)) in [
                ("dcL0", "dcTile0", "doc24.dcNone", "None"),
                ("dcL1", "dcTile1", "doc24.dcDropped", "Dropped"),
                ("dcL2", "dcTile2", "doc24.dcMargin", "In margin"),
            ]
            .iter()
            .enumerate()
            {
                let x0 = 33.0 + 94.0 * i as f32;
                f.canvas(tile, [x0, 72.0, 76.0, 76.0]);
                let text = tr(k, en);
                // Centred under the tile (about 7 px per character at 125 %).
                let w = 7.2 * text.chars().count() as f32;
                f.label(lab, [x0 + 38.0 - w / 2.0, 152.0, w + 8.0, 18.0], &text);
            }
            f.ids = vec![pos.to_owned()];
            f.group(
                "optG",
                [14.0, 180.0, 293.0, 18.0],
                &tr("doc24.options", "Options"),
            );
            f.label(
                "fontL",
                [33.0, 206.0, 200.0, 18.0],
                &tr("doc24.font", "Font:"),
            );
            let mut fonts = c.fonts.clone();
            let cur = x["font"].as_str().unwrap_or(&c.font).to_owned();
            if !fonts.contains(&cur) {
                fonts.insert(0, cur.clone());
            }
            let fi = fonts.iter().position(|n| *n == cur).unwrap_or(0);
            f.combo("font", [33.0, 229.0, 270.0, 24.0], fonts, fi);
            f.label(
                "linesL",
                [33.0, 262.0, 120.0, 18.0],
                &tr("doc24.linesToDrop", "Lines to drop:"),
            );
            let lines = x["lines"].as_i64().unwrap_or(3) as i32;
            f.spin("lines", [155.0, 258.0, 148.0, 24.0], lines, 1, 10);
            f.label(
                "distL",
                [33.0, 292.0, 120.0, 18.0],
                &tr("doc24.distance", "Distance from text:"),
            );
            let dist = x["distance"].as_i64().unwrap_or(0) as i32;
            f.edit(
                "distance",
                [155.0, 288.0, 148.0, 24.0],
                &super::r13::show_len(dist, c.inches),
            );
            f.preview = vec![if c.inches { "in" } else { "cm" }.to_owned()];
            if pos == "none" {
                for id in ["font", "lines", "distance"] {
                    f.disable(id);
                }
            }
            f.ok_cancel(s);
            f
        }
        DocFormKind::Hyphenation24 => {
            // The reference's dialog at 125 %: 403 × 227 px.
            let mut f = DocForm::new(
                kind,
                &tr("doc24.hyphTitle", "Hyphenation"),
                (403.0, 227.0),
                vec![],
            );
            f.check(
                "auto",
                [18.0, 48.0, 340.0, 20.0],
                &tr("doc24.autoHyph", "Automatically hyphenate document"),
                x["auto"] == true,
            );
            f.check(
                "caps",
                [18.0, 73.0, 340.0, 20.0],
                &tr("doc24.hyphCaps", "Hyphenate words in CAPS"),
                x["noCaps"] != true,
            );
            f.label(
                "zoneL",
                [18.0, 114.0, 180.0, 18.0],
                &tr("doc24.hyphZone", "Hyphenation zone:"),
            );
            let zone = x["zone"].as_i64().unwrap_or(360) as i32;
            f.edit(
                "zone",
                [201.0, 110.0, 128.0, 24.0],
                &super::r13::show_len(zone, c.inches),
            );
            f.label(
                "limitL",
                [18.0, 143.0, 180.0, 18.0],
                &tr("doc24.hyphLimit", "Limit consecutive hyphens to:"),
            );
            let limit = x["limit"].as_i64().unwrap_or(0);
            let mut items = vec![tr("doc24.noLimit", "No limit")];
            items.extend((1..=10).map(|n| n.to_string()));
            f.combo(
                "limit",
                [201.0, 139.0, 128.0, 24.0],
                items,
                (limit.clamp(0, 10)) as usize,
            );
            f.button(
                "manual",
                [18.0, 180.0, 113.0, 28.0],
                &tr("doc24.manual", "Manual..."),
                false,
            );
            f.preview = vec![if c.inches { "in" } else { "cm" }.to_owned()];
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

    /// A click on a Drop Cap position tile chooses it.
    pub(super) fn r24_canvas(&mut self, id: &str) -> bool {
        let pos = match id {
            "dcTile0" => "none",
            "dcTile1" => "drop",
            "dcTile2" => "margin",
            _ => return false,
        };
        self.r24_set_pos(pos);
        true
    }

    /// Drop Cap: the chosen position (`none`, `drop`, `margin`).
    pub(crate) fn r24_pos(&self) -> &str {
        self.ids.first().map_or("none", String::as_str)
    }

    /// Drop Cap: choose a position (the options are greyed for None).
    pub(crate) fn r24_set_pos(&mut self, pos: &str) {
        self.ids = vec![pos.to_owned()];
        let on = pos != "none";
        for c in ["font", "lines", "distance"] {
            self.form.enable(c, on);
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
                let pos = self.r24_pos().to_owned();
                let pos = pos.as_str();
                let spec = (pos != "none").then(|| {
                    let font = self.text("font");
                    json!({
                        "position": pos,
                        "font": (!font.trim().is_empty()).then(|| font.trim().to_owned()),
                        "lines": self.spin_value("lines").clamp(1, 10),
                        "distance": self.r24_len("distance").unwrap_or(0).clamp(0, 31_680),
                    })
                });
                FormOut::Run(vec![json!({ "cmd": "setDropCap", "dropCap": spec })], true)
            }
            DocFormKind::Hyphenation24 => {
                let zone = self.r24_len("zone").unwrap_or(360).clamp(0, 31_680);
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

    /// A length box in the window's unit (a bare number), any unit typed.
    fn r24_len(&self, id: &str) -> Option<i32> {
        let bare = if self.preview.first().is_some_and(|u| u == "in") {
            1440.0
        } else {
            566.929
        };
        super::r13::parse_len(&self.text(id), bare)
    }

    fn spin_value(&self, id: &str) -> i32 {
        self.text(id).trim().parse().unwrap_or(1)
    }
}

/// Paint a round-24 canvas: the Drop Cap position tiles (BlueOffice's drawing: grey text
/// lines and a large letter where the drop cap goes; the chosen tile framed in the accent).
pub(super) fn paint_canvas(
    f: &DocForm,
    id: &str,
    r: [f32; 4],
    p: &mut dyn crate::Painter,
    font: &crate::Font,
) -> bool {
    use bo_render::Rgba;
    let which = match id {
        "dcTile0" => 0,
        "dcTile1" => 1,
        "dcTile2" => 2,
        _ => return false,
    };
    let chosen = f.r24_pos() == ["none", "drop", "margin"][which];
    let k = r[3] / 76.0;
    let edge = Rgba(0x7a, 0x7a, 0x7a, 255);
    if chosen {
        let a = Rgba(0x00, 0x5a, 0x9e, 255);
        p.fill_rect(r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0, a);
        p.fill_rect(
            r[0] - 1.0,
            r[1] - 1.0,
            r[2] + 2.0,
            r[3] + 2.0,
            Rgba(255, 255, 255, 255),
        );
    }
    p.fill_rect(r[0], r[1], r[2], r[3], edge);
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(255, 255, 255, 255),
    );
    let grey = Rgba(0xa6, 0xa6, 0xa6, 255);
    let line = |p: &mut dyn crate::Painter, x0: f32, y: f32| {
        p.fill_rect(
            r[0] + x0 * k,
            r[1] + y * k,
            r[2] - (x0 + 8.0) * k,
            (4.0 * k).max(2.0),
            grey,
        );
    };
    // Where the letter sits: none (small, in the line), dropped (beside three lines), in
    // the margin (left of the lines).
    let (letter_x, text_x, size) = match which {
        0 => (8.0, 8.0, 9.0),
        1 => (7.0, 34.0, 30.0),
        _ => (3.0, 30.0, 30.0),
    };
    for i in 0..6 {
        let y = 10.0 + 10.0 * i as f32;
        let x0 = if which == 0 {
            if i == 0 {
                16.0
            } else {
                8.0
            }
        } else if i < 3 || which == 2 {
            text_x
        } else {
            8.0
        };
        line(p, x0, y);
    }
    // A serif "W" drawn as strokes (BlueOffice's drawing; the dialog font has no serifs):
    // thick and thin diagonals with serifs on the three tops.
    let _ = font;
    let ink = Rgba(0x20, 0x20, 0x20, 255);
    let h = size * k * 0.72;
    let w = h * 1.25;
    let x = r[0] + letter_x * k;
    let y = r[1] + if which == 0 { 7.0 } else { 10.0 } * k;
    let pt = |u: f32, v: f32| (x + u * w, y + v * h);
    let thick = (h * 0.11).max(1.0);
    let thin = (h * 0.045).max(1.0);
    let seg = |p: &mut dyn crate::Painter, a: (f32, f32), b: (f32, f32), wd: f32| {
        p.path(
            &[PathSeg::M(a.0, a.1), PathSeg::L(b.0, b.1)],
            None,
            Some((ink, wd)),
        );
    };
    // Downstrokes thick, upstrokes thin (a roman letter's contrast).
    seg(p, pt(0.08, 0.0), pt(0.3, 1.0), thick);
    seg(p, pt(0.3, 1.0), pt(0.5, 0.18), thin);
    seg(p, pt(0.5, 0.18), pt(0.7, 1.0), thick);
    seg(p, pt(0.7, 1.0), pt(0.92, 0.0), thin);
    for (u0, u1) in [(-0.04, 0.2), (0.4, 0.58), (0.82, 1.02)] {
        seg(p, pt(u0, 0.0), pt(u1, 0.0), thin);
    }
    true
}
