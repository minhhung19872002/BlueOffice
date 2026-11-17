//! BlueDoc parity round 5 (docs/ui/REPORT.md rounds 80+): Mailings › Rules dialogs, Match
//! Fields, Find Recipient, typing in Outline view, View › Split and Arrange All. A child
//! module of the shell so the window's state is at hand; dialogs live in `doc_forms5`.
use super::{DocWindow, Request};
use crate::doc_forms::DocFormKind;
use crate::doc_outline::OutlineEdit;
use crate::doc_reveal::{InfoHit, InfoPane, InfoSection};
use crate::{Input, Key, Painter};
use bo_render::Rgba;
use serde_json::json;
use std::collections::BTreeMap;

/// Round-5 state of the window.
#[derive(Default)]
pub(super) struct R5 {
    /// Mailings › Match Fields: address field → recipient column.
    pub(super) match_fields: BTreeMap<String, String>,
    /// View › Split: two panes on the document.
    pub(super) split: Option<Split>,
    /// Reveal Formatting (`false`) or Style Inspector (`true`) pane.
    pub(super) info: Option<(bool, InfoPane)>,
    /// Document version and whether it held a merge field then.
    fields_at: Option<(u64, bool)>,
    /// The record box's number while a data source is attached.
    pub(super) record_box: Option<usize>,
}

/// The document has a MERGEFIELD (simple or complex field, ECMA-376 Part 1, §17.16.5.35).
fn has_merge_field(doc: &bo_model::Document) -> bool {
    bo_model::index::body_paragraphs(doc).iter().any(|path| {
        bo_model::index::para_at(doc, path).is_some_and(|p| {
            p.runs.iter().flat_map(|r| &r.content).any(|c| match c {
                bo_model::RunContent::Field { instr, .. } => instr
                    .trim_start()
                    .to_ascii_uppercase()
                    .starts_with("MERGEFIELD"),
                bo_model::RunContent::Opaque(raw) => {
                    raw.xml.contains("instrText") && raw.xml.contains("MERGEFIELD")
                }
                _ => false,
            })
        })
    })
}

/// View › Split: the panes share the document area, a bar between them.
#[derive(Debug, Clone, Copy)]
pub(super) struct Split {
    /// Share of the area the top pane takes.
    pub(super) frac: f32,
    /// Scroll offset of the pane that is not the main view.
    pub(super) other: f32,
    /// The main view (caret, input) is the top pane.
    pub(super) active_top: bool,
    /// The bar is being dragged.
    drag: bool,
    /// Document area (x, y, width, height).
    pub(super) area: [f32; 4],
    /// The bar between the panes.
    pub(super) bar: [f32; 4],
}

/// Mailings commands the reference dims without a data source (Envelopes, Labels, Start Mail
/// Merge and Select Recipients stay lit).
const NEED_DATA: [&str; 16] = [
    "mailings.editRecipients",
    "mailings.highlightFields",
    "mailings.addressBlock",
    "mailings.greetingLine",
    "mailings.insertField",
    "mailings.rules",
    "mailings.matchFields",
    "mailings.updateLabels",
    "mailings.preview",
    "mailings.firstRecord",
    "mailings.previousRecord",
    "mailings.recordBox",
    "mailings.nextRecord",
    "mailings.lastRecord",
    "mailings.findRecipient",
    "mailings.checkErrors",
];

/// Of those, the ones that also need a merge field in the document.
const NEED_FIELDS: [&str; 3] = [
    "mailings.highlightFields",
    "mailings.preview",
    "mailings.findRecipient",
];

/// The Mailings commands the reference dims in a merge state (measured on its ribbon:
/// no data source; data but no merge field; merge fields; Update Labels only for labels).
pub(super) fn mailings_dimmed(has_data: bool, has_fields: bool, labels: bool) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = if has_data {
        if has_fields {
            Vec::new()
        } else {
            NEED_FIELDS.to_vec()
        }
    } else {
        let mut v = NEED_DATA.to_vec();
        v.push("mailings.finish");
        v
    };
    if !labels && !out.contains(&"mailings.updateLabels") {
        out.push("mailings.updateLabels");
    }
    out
}

/// Height of the split bar at 100 % (the reference's bar is 7 px at 125 %).
const BAR: f32 = 5.6;

/// A property key in words ("spaceBefore" → "Space before").
fn words(key: &str) -> String {
    let mut out = String::new();
    for (i, c) in key.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else if c.is_uppercase() {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

impl DocWindow {
    /// Run round-5 interface command `id` (`None`: not a round-5 command).
    pub(super) fn r5_action(&mut self, id: &str) -> Option<bool> {
        if let Some(map) = id.strip_prefix("mailings.matchFields.set:") {
            let v: serde_json::Value = serde_json::from_str(map).unwrap_or_default();
            self.r5.match_fields = v
                .as_object()
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
                        .collect()
                })
                .unwrap_or_default();
            return Some(true);
        }
        if let Some(q) = id.strip_prefix("mailings.find:") {
            let (field, what) = q.split_once('\u{1f}').unwrap_or((q, ""));
            return Some(self.find_recipient(field, what));
        }
        // Scripted measurements (BLUEOFFICE_ACTIONS): show page N (1-based) at the top.
        if let Some(n) = id.strip_prefix("view.goPage.") {
            let n = n.parse::<usize>().unwrap_or(1).max(1);
            return Some(self.view.show_page(n - 1));
        }
        if let Some(v) = id.strip_prefix("styles.setDefaults:") {
            let (font, size) = v.split_once('\u{1f}').unwrap_or((v, ""));
            return Some(self.set_style_defaults(font, size));
        }
        if self.ribbon.extra_dimmed(id) {
            // Dimmed for the merge state.
            return Some(false);
        }
        match id {
            "styles.inspector" => return Some(self.toggle_info(true)),
            "format.revealFormatting" => return Some(self.toggle_info(false)),
            "styles.manage" => return Some(self.open_manage_styles()),
            "view.split" => return Some(self.toggle_split()),
            "view.arrangeAll" => {
                self.requests.push(Request::ArrangeAll);
                return Some(false);
            }
            _ => {}
        }
        let form = match id {
            "mailings.rule.ask" => DocFormKind::AskRule,
            "mailings.rule.fillIn" => DocFormKind::FillInRule,
            "mailings.rule.if" => DocFormKind::IfRule,
            "mailings.rule.nextIf" => DocFormKind::NextIfRule,
            "mailings.rule.set" => DocFormKind::SetRule,
            "mailings.rule.skipIf" => DocFormKind::SkipIfRule,
            "mailings.matchFields" => DocFormKind::MatchFields,
            "mailings.findRecipient" => DocFormKind::FindRecipient,
            "mailings.addressBlock" if !self.r5.match_fields.is_empty() => {
                return Some(self.matched_address_block());
            }
            _ => return None,
        };
        Some(self.r3_open_form(form))
    }

    /// Width the round-5 pane takes at the right.
    pub(super) fn r5_pane_w(&self) -> f32 {
        if self.r5.info.is_some() {
            InfoPane::width(self.scale)
        } else {
            0.0
        }
    }

    /// Show or hide the Style Inspector (`inspector`) or Reveal Formatting pane.
    fn toggle_info(&mut self, inspector: bool) -> bool {
        match self.r5.info.take() {
            Some((was, _)) if was == inspector => {}
            _ => {
                let title = if inspector {
                    self.r5_t("docR5.inspectorTitle", "Style Inspector")
                } else {
                    self.r5_t("docR5.revealTitle", "Reveal Formatting")
                };
                self.r5.info = Some((inspector, InfoPane::new(title)));
                self.refresh_info();
            }
        }
        self.relayout_pending = true;
        true
    }

    fn r5_t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    /// The pane's sections from the engine (formatting at the caret).
    pub(super) fn refresh_info(&mut self) {
        let Some(inspector) = self.r5.info.as_ref().map(|i| i.0) else {
            return;
        };
        let sections = if inspector {
            self.inspector_sections()
        } else {
            self.reveal_sections()
        };
        if let Some((_, pane)) = self.r5.info.as_mut() {
            pane.sections = sections;
        }
    }

    fn reveal_sections(&mut self) -> Vec<InfoSection> {
        let v = self
            .view
            .run_json(&json!({ "cmd": "revealFormatting" }))
            .unwrap_or_default();
        let items = v["styleMgr"]["reveal"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::new();
        for (group, key, fallback) in [
            ("font", "docR5.revealFont", "Font"),
            ("paragraph", "docR5.revealParagraph", "Paragraph"),
            ("section", "docR5.revealSection", "Section"),
        ] {
            let lines: Vec<String> = items
                .iter()
                .filter(|i| i["group"] == group)
                .map(|i| {
                    let k = i["key"].as_str().unwrap_or("");
                    let val = i["value"].as_str().unwrap_or("");
                    let from = match i["source"].as_str() {
                        Some("default") => " (Default)",
                        _ => "",
                    };
                    format!("{}: {val}{from}", words(k))
                })
                .collect();
            if !lines.is_empty() {
                out.push(InfoSection {
                    header: self.r5_t(key, fallback),
                    lines,
                    clear: None,
                });
            }
        }
        out
    }

    fn inspector_sections(&mut self) -> Vec<InfoSection> {
        let v = self
            .view
            .run_json(&json!({ "cmd": "styleInspector" }))
            .unwrap_or_default();
        let i = &v["styleMgr"]["inspector"];
        let none = self.r5_t("docR5.none", "<none>");
        let direct = |a: &serde_json::Value| -> Vec<String> {
            let lines: Vec<String> = a
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|d| {
                            format!(
                                "{}: {}",
                                words(d["key"].as_str().unwrap_or("")),
                                d["value"].as_str().unwrap_or("")
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            if lines.is_empty() {
                vec![none.clone()]
            } else {
                lines
            }
        };
        let para = i["paragraphStyle"]["name"]
            .as_str()
            .unwrap_or("Normal")
            .to_owned();
        let chars = i["characterStyle"]["name"].as_str().map_or_else(
            || self.r5_t("docR5.defaultParagraphFont", "Default Paragraph Font"),
            str::to_owned,
        );
        let plus = self.r5_t("docR5.plus", "Plus:");
        vec![
            InfoSection {
                header: self.r5_t("docR5.paragraphFormatting", "Paragraph formatting"),
                lines: vec![para],
                clear: Some("paragraphStyle"),
            },
            InfoSection {
                header: plus.clone(),
                lines: direct(&i["paragraphDirect"]),
                clear: Some("paragraphDirect"),
            },
            InfoSection {
                header: self.r5_t("docR5.textFormatting", "Text level formatting"),
                lines: vec![chars],
                clear: Some("characterStyle"),
            },
            InfoSection {
                header: plus,
                lines: direct(&i["characterDirect"]),
                clear: Some("characterDirect"),
            },
        ]
    }

    /// Dim the Mailings commands as the reference does for the merge state (while the
    /// Mailings tab shows; the field scan is cached by document version).
    fn sync_mailings(&mut self) {
        if self
            .ribbon
            .rect_of(crate::ribbon::RibbonHit::Command("mailings.rules"))
            .is_none()
        {
            return;
        }
        let has_data = self.merge_fields().1 > 0;
        let version = self
            .view
            .engine()
            .session()
            .map_or(0, bo_edit::Session::version);
        let has_fields = match self.r5.fields_at {
            Some((v, f)) if v == version => f,
            _ => {
                let f = self.view.engine().document().is_some_and(has_merge_field);
                self.r5.fields_at = Some((version, f));
                f
            }
        };
        let labels = self.r3.merge_type == "labels";
        self.ribbon
            .set_extra_dim(mailings_dimmed(has_data, has_fields, labels));
        self.r5.record_box = has_data.then_some(0);
    }

    /// Lay the pane out at the right, left of the round-3 panes; refresh its values.
    pub(super) fn r5_layout(&mut self) {
        self.sync_mailings();
        if self.r5.info.is_none() {
            return;
        }
        self.refresh_info();
        let right = self.size.0 - self.r3_pane_w() + self.r5_pane_w();
        let (top, bottom, scale) = (
            self.chrome_bottom(),
            self.size.1 - self.status_h(),
            self.scale,
        );
        if let Some((_, pane)) = self.r5.info.as_mut() {
            pane.layout(right, top, bottom, scale);
        }
    }

    /// Manage Styles from the document's styles.
    fn open_manage_styles(&mut self) -> bool {
        let styles = self
            .view
            .all_styles()
            .into_iter()
            .filter(|s| s.kind == "paragraph" || s.kind == "character")
            .map(|s| crate::doc_forms::ManagedStyle {
                id: s.id,
                name: s.name,
                font: s.preview.font.clone(),
                size: s.preview.size,
                bold: s.preview.bold,
                italic: s.preview.italic,
                recommended: s.recommended,
                paragraph: s.kind == "paragraph",
            })
            .collect();
        self.menu = None;
        self.r3.form = Some(crate::doc_forms::open_manage_styles(&self.strings, styles));
        self.relayout_pending = true;
        true
    }

    /// Manage Styles › Set Defaults: Normal's font and size (one undo step).
    fn set_style_defaults(&mut self, font: &str, size: &str) -> bool {
        let Some(v) = self
            .view
            .run_json(&json!({ "cmd": "styleDetails", "id": "Normal" }))
        else {
            return false;
        };
        let mut spec = v["styleMgr"]["spec"].clone();
        if !spec.is_object() {
            return false;
        }
        spec["font"] = json!(font);
        if let Ok(pt) = size.parse::<f32>() {
            // Half points (§17.3.2.38 sz).
            spec["size"] = json!((pt * 2.0).round() as u32);
        }
        self.r3_run(vec![
            json!({ "cmd": "styleModify", "id": "Normal", "spec": spec }),
        ])
    }

    /// View › Split on or off (the button reads Remove Split while split, as the reference).
    fn toggle_split(&mut self) -> bool {
        if self.r5.split.take().is_some() {
            self.view.set_secondary(None);
            let label = self.split_label(false);
            self.strings.set("commands.view.split", &label);
        } else {
            let (scroll, ..) = self.view.scroll_info();
            self.r5.split = Some(Split {
                frac: 0.5,
                other: scroll,
                active_top: false,
                drag: false,
                area: [0.0; 4],
                bar: [0.0; 4],
            });
            let label = self.split_label(true);
            self.strings.set("commands.view.split", &label);
        }
        self.relayout_pending = true;
        true
    }

    fn split_label(&self, split: bool) -> String {
        let (key, fallback) = if split {
            ("docR5.removeSplit", "Remove Split")
        } else {
            ("docR5.split", "Split")
        };
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    /// Lay the panes of View › Split out in the document area (x, y, width, height): the main
    /// view takes its pane, the other one is drawn from the same pages.
    pub(super) fn r5_layout_split(&mut self, area: [f32; 4]) {
        let Some(mut s) = self.r5.split else {
            return;
        };
        let [x, y, w, h] = area;
        let bar = (BAR * self.scale).round().max(3.0);
        let top_h = ((h - bar) * s.frac).round().max(1.0);
        let bottom_h = (h - bar - top_h).max(1.0);
        let top = [x, y, w, top_h];
        let bottom = [x, y + top_h + bar, w, bottom_h];
        let (main, other) = if s.active_top {
            (top, bottom)
        } else {
            (bottom, top)
        };
        self.view.set_frame(main[0], main[1], main[2], main[3]);
        s.other = self.view.set_secondary(Some((other, s.other)));
        s.area = area;
        // The bar spans the scroll bar too, as the reference's.
        s.bar = [x, y + top_h, self.size.0 - x, bar];
        self.r5.split = Some(s);
    }

    /// The pane that is not the main view.
    fn other_pane(s: &Split) -> [f32; 4] {
        let [x, y, w, h] = s.area;
        if s.active_top {
            let top = s.bar[1] + s.bar[3];
            [x, top, w, (y + h - top).max(0.0)]
        } else {
            [x, y, w, s.bar[1] - y]
        }
    }

    /// Pointer input of View › Split: drag the bar, double-click it to remove the split, click
    /// in the other pane to work there, wheel over it to scroll it. `Some(redraw)` when taken.
    pub(super) fn r5_input(&mut self, input: &Input) -> Option<bool> {
        if let Input::Key(Key::F(1), m) = *input {
            if m.shift && !m.ctrl && !m.alt {
                return Some(self.toggle_info(false));
            }
        }
        if let Some(r) = self.info_input(input) {
            return Some(r);
        }
        let mut s = self.r5.split?;
        let other_rect = Self::other_pane(&s);
        match *input {
            Input::Press { x, y, count, .. } if inside(s.bar, x, y) => {
                if count >= 2 {
                    return Some(self.toggle_split());
                }
                s.drag = true;
                self.r5.split = Some(s);
                Some(false)
            }
            Input::Drag { y, .. } if s.drag => {
                let room = (s.area[3] - s.bar[3]).max(1.0);
                s.frac = ((y - s.area[1] - s.bar[3] / 2.0) / room).clamp(0.08, 0.92);
                self.r5.split = Some(s);
                self.r5_layout_split(s.area);
                Some(true)
            }
            Input::Release { .. } if s.drag => {
                s.drag = false;
                self.r5.split = Some(s);
                Some(false)
            }
            Input::Press { x, y, .. } if inside(other_rect, x, y) => {
                // Work in the other pane: it becomes the main view, scrolled where it was.
                let (now, ..) = self.view.scroll_info();
                let then = s.other;
                s.other = now;
                s.active_top = !s.active_top;
                self.r5.split = Some(s);
                self.r5_layout_split(s.area);
                self.view.scroll_to(then);
                None
            }
            Input::Wheel(dy) if inside(other_rect, self.pointer.0, self.pointer.1) => {
                s.other = self.view.set_secondary(Some((other_rect, s.other + dy)));
                self.r5.split = Some(s);
                Some(true)
            }
            _ => None,
        }
    }

    /// Input on the Reveal Formatting / Style Inspector pane.
    fn info_input(&mut self, input: &Input) -> Option<bool> {
        let (_, pane) = self.r5.info.as_mut()?;
        match *input {
            Input::Press { x, y, .. } if pane.contains(x, y) => match pane.press(x, y) {
                InfoHit::Close => {
                    self.r5.info = None;
                    self.relayout_pending = true;
                    Some(true)
                }
                InfoHit::Clear(level) => {
                    let ok = self.r3_run(vec![json!({ "cmd": "styleClear", "level": level })]);
                    self.refresh_info();
                    Some(ok)
                }
                InfoHit::None => Some(false),
            },
            Input::Wheel(dy) if pane.contains(self.pointer.0, self.pointer.1) => {
                Some(pane.wheel(dy))
            }
            Input::Move { x, y } | Input::Drag { x, y } | Input::Release { x, y }
                if pane.contains(x, y) =>
            {
                Some(false)
            }
            _ => None,
        }
    }

    /// The split bar and the round-5 pane.
    pub(super) fn r5_paint(&self, p: &mut dyn Painter) {
        if let Some((_, pane)) = &self.r5.info {
            pane.paint(p);
        }
        let Some(s) = self.r5.split else {
            return;
        };
        let [x, y, w, h] = s.bar;
        let line = self.scale.round().max(1.0);
        p.fill_rect(x, y, w, h, Rgba(0xe6, 0xe6, 0xe6, 255));
        p.fill_rect(x, y, w, line, Rgba(0xc6, 0xc6, 0xc6, 255));
        p.fill_rect(x, y + h - line, w, line, Rgba(0xc6, 0xc6, 0xc6, 255));
    }

    /// Typing in Outline view: edit body paragraph `para` at byte offset `at` through the engine
    /// (the caret moves there first), then show the outline again with the caret after the
    /// edit. Enter splits the paragraph: the new item keeps the style, so its level.
    pub(super) fn outline_edit(&mut self, para: usize, at: usize, edit: &OutlineEdit) -> bool {
        let level = self
            .r3
            .outline
            .as_ref()
            .and_then(crate::doc_outline::OutlineView::selected)
            .and_then(|(_, l)| l);
        if !self.view.select_range(para, at, at) {
            return false;
        }
        let cmd = match edit {
            OutlineEdit::Type(t) => json!({ "cmd": "insertText", "text": t }),
            OutlineEdit::Split => json!({ "cmd": "splitParagraph" }),
            OutlineEdit::Back => json!({ "cmd": "deleteBackward" }),
            OutlineEdit::Forward => json!({ "cmd": "deleteForward" }),
        };
        if !self.r3_run(vec![cmd]) {
            return false;
        }
        if let (OutlineEdit::Split, Some(l)) = (edit, level) {
            // The new item is a heading of the same level, as the reference's outline.
            let next = self.view.caret_para().unwrap_or(para + 1);
            self.r3_run(vec![
                json!({ "cmd": "setOutlineLevel", "para": next, "level": l }),
            ]);
        }
        self.refresh_outline();
        // The caret where the engine left it (paragraph id, byte offset → characters).
        if let Some(sel) = self
            .view
            .engine()
            .session()
            .map(bo_edit::Session::selection)
        {
            let p = sel.focus.para;
            let chars = self
                .view
                .engine()
                .session()
                .and_then(|s| s.para_text(p))
                .map_or(0, |t| {
                    t.get(..sel.focus.offset).map_or(0, |b| b.chars().count())
                });
            if let Some(o) = self.r3.outline.as_mut() {
                o.select_para(p, chars);
            }
        }
        self.relayout_pending = true;
        true
    }

    /// Find Recipient: preview the next record (after the one shown, wrapping around) whose
    /// `field` contains `what` (case-insensitive).
    fn find_recipient(&mut self, field: &str, what: &str) -> bool {
        let Some(v) = self
            .view
            .run_json(&json!({ "cmd": "mergeRecords", "limit": 100_000 }))
        else {
            return false;
        };
        let r = &v["mergeOut"]["records"];
        let fields: Vec<&str> = r["fields"]
            .as_array()
            .map(|a| a.iter().filter_map(|f| f.as_str()).collect())
            .unwrap_or_default();
        let Some(col) = fields.iter().position(|f| *f == field) else {
            return false;
        };
        let rows = r["rows"].as_array().cloned().unwrap_or_default();
        if rows.is_empty() {
            return false;
        }
        let want = what.to_lowercase();
        let start = self.merge_record.map_or(0, |n| n + 1);
        let hit = (0..rows.len())
            .map(|k| (start + k) % rows.len())
            .find(|&i| {
                rows[i]["values"][col]
                    .as_str()
                    .is_some_and(|s| s.to_lowercase().contains(&want))
            });
        let Some(i) = hit else {
            return false;
        };
        self.merge_record = Some(i);
        self.r3_run(vec![json!({ "cmd": "mergePreview", "record": i })])
    }

    /// Address Block from the Match Fields choices: title, first and last name; company;
    /// both address lines; city and state (MERGEFIELDs of the matched columns, §17.16.5.35).
    fn matched_address_block(&mut self) -> bool {
        let m = &self.r5.match_fields;
        let pick = |names: &[&str]| -> Vec<String> {
            names.iter().filter_map(|n| m.get(*n).cloned()).collect()
        };
        let lines: Vec<Vec<String>> = [
            pick(&[
                "Courtesy Title",
                "First Name",
                "Middle Name",
                "Last Name",
                "Suffix",
            ]),
            pick(&["Company"]),
            pick(&["Address 1"]),
            pick(&["Address 2"]),
            pick(&["City", "State"]),
        ]
        .into_iter()
        .filter(|l| !l.is_empty())
        .collect();
        let mut cmds = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                cmds.push(json!({ "cmd": "splitParagraph" }));
            }
            for (k, f) in line.iter().enumerate() {
                if k > 0 {
                    cmds.push(json!({ "cmd": "insertText", "text": " " }));
                }
                cmds.push(json!({ "cmd": "mergeInsertField", "name": f }));
            }
        }
        !cmds.is_empty() && self.r3_run(cmds)
    }
}

#[cfg(test)]
#[path = "doc_r5_tests.rs"]
mod tests;
