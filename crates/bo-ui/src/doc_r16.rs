//! BlueDoc parity round 16 (docs/ui/REPORT.md "Vòng 106"): the Review tab as the reference
//! word processor's — collapsed Comments / Tracking groups whose drop-down shows the whole
//! group, the markup display box, Delete ▾ (Delete All Comments Shown / in Document), Show
//! Markup ▾ with its Balloons ▸ and Specific People ▸ sub-menus. A child module of the shell.
use super::DocWindow;
use crate::doc_grouppop16::GroupPopup;
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use crate::{Input, Painter, TextMeasure, TextStyle};
use bo_engine::markup::MarkupMode;
use bo_render::Rgba;
use serde_json::json;

#[cfg(test)]
#[path = "doc_r16_tests.rs"]
mod tests;

/// Round-16 state of the window.
#[derive(Default)]
pub(super) struct R16 {
    /// The open drop-down of a collapsed ribbon group.
    pub(super) pop: Option<GroupPopup>,
    /// Reviewers listed in the open Specific People sub-menu (row order).
    people: Vec<String>,
    /// Row rectangles of the last Show Markup menu (its sub-menus open beside them).
    markup_rows: Vec<[f32; 4]>,
    /// The Thesaurus pane, and whether it opened since the last layout (it then closes the
    /// other panes at the right; one opened later closes it).
    pub(super) thes: Option<crate::doc_thes16::ThesPane>,
    thes_fresh: bool,
    /// The word whose Insert / Copy menu is open in the Thesaurus pane.
    thes_word: Option<String>,
    /// Actions for the window the last merge opens.
    new_actions: Option<String>,
    /// Where a menu opened from the group drop-down goes (its button there).
    pop_anchor: Option<[f32; 4]>,
    /// The text of the last Check for Errors report.
    report: String,
    /// Read Aloud in progress (its bar shown).
    pub(super) ra: Option<crate::doc_speech16::ReadAloud>,
    /// The Preview Results record box being typed in (its text).
    pub(super) record_edit: Option<String>,
    /// Home › Font Color: the open palette, the colour the button applies (`RRGGBB`, empty
    /// until one is picked: red), More Colors… asked from it.
    pub(super) font_pal: Option<Box<crate::doc_wm14::PagePalette>>,
    pub(super) font_color: String,
    pub(super) font_more: bool,
}

/// Border of the markup display box (#646464), its arrow separator (#949494) and arrow
/// (#6D6D6D), measured on the reference at 125 %.
const BOX_EDGE: Rgba = Rgba(0x64, 0x64, 0x64, 255);
const BOX_SEP: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const BOX_ARROW: Rgba = Rgba(0x6D, 0x6D, 0x6D, 255);

impl DocWindow {
    /// A round-16 command; `None` when `id` is not one.
    #[inline(never)]
    pub(super) fn r16_action(&mut self, id: &str) -> Option<bool> {
        if self.ribbon.extra_dimmed(id) {
            // Dimmed for the merge state (round 5 answers).
            return None;
        }
        if let Some(i) = id.strip_prefix("review.people.") {
            return Some(self.r16_toggle_person(i));
        }
        if let Some(spec) = id.strip_prefix("r16.merge:") {
            return Some(self.r16_merge(spec));
        }
        if let Some(hex) = id.strip_prefix("r16.fontColor:") {
            return Some(self.r16_font_color(Some(hex)));
        }
        if let Some(g) = id.strip_prefix("r16.gradient:") {
            return Some(self.r16_gradient(g));
        }
        if let Some(n) = id.strip_prefix("r16.ra.speed:") {
            let rate = n.parse::<i32>().unwrap_or(0).clamp(-10, 10);
            if let Some(ra) = self.r16.ra.as_mut() {
                ra.rate = rate;
            }
            self.requests.push(super::Request::SpeechControl(10 + rate));
            return Some(true);
        }
        if let Some(mode) = id.strip_prefix("r16.checkErrors:") {
            return Some(self.r16_check_errors(mode));
        }
        // Track Changes Options › Reviewing Pane: Off / Vertical / Horizontal.
        if let Some(pane) = id.strip_prefix("r16.pane:") {
            let now = self.r6.review.as_ref().map(|p| p.horizontal);
            let want = match pane {
                "vertical" => Some(false),
                "horizontal" => Some(true),
                _ => None,
            };
            return Some(match (now, want) {
                (a, b) if a == b => false,
                (Some(_), None) => self.r6_toggle_review(None),
                (_, Some(h)) => self.r6_toggle_review(Some(h)),
                (None, None) => false,
            });
        }
        Some(match id {
            "review.thesaurus" => self.r16_toggle_thesaurus(),
            "r16.thes.size" => {
                if let Some(t) = self.r16.thes.as_mut() {
                    t.sizing = true;
                }
                true
            }
            "r16.thes.close" => {
                self.r16.thes = None;
                self.relayout_pending = true;
                true
            }
            "review.readAloud" => self.r16_toggle_read_aloud(),
            "mailings.highlightFields" => {
                self.view.highlight_fields = !self.view.highlight_fields;
                self.view.refresh_comment_marks();
                true
            }
            "mailings.finish" => self.r16_finish_menu(),
            "review.deleteMenu" => self.r16_delete_menu(),
            // The Font Color button applies the last colour picked (red at first).
            "format.color" => {
                let hex = if self.r16.font_color.is_empty() {
                    "FF0000".to_owned()
                } else {
                    self.r16.font_color.clone()
                };
                self.r16_font_color(Some(&hex))
            }
            "format.colorMenu" => self.r16_open_font_palette(),
            // The record box takes a typed record number (Enter goes to it).
            "mailings.recordBox" => {
                if self.merge_fields().1 == 0 {
                    return Some(false);
                }
                let n = self.merge_record.or(self.r5.record_box).unwrap_or(0) + 1;
                self.r16.record_edit = Some(n.to_string());
                true
            }
            "mailings.recipients" => self.r16_recipients_menu(),
            "mailings.recipients.new" => self.r3_open_form(crate::doc_forms::DocFormKind::NewList),
            "mailings.checkErrors" => self.r3_open_form(crate::doc_forms::DocFormKind::CheckErrors),
            "mailings.finish.edit" => self.r3_open_form(crate::doc_forms::DocFormKind::MergeNew),
            "mailings.finish.print" => self.r3_open_form(crate::doc_forms::DocFormKind::MergePrint),
            "review.trackingOptions" => {
                self.r3_open_form(crate::doc_forms::DocFormKind::TrackOptions)
            }
            "review.thes.insert" => {
                let (Some(w), Some(t)) = (
                    self.r16.thes_word.take(),
                    self.r16.thes.as_ref().and_then(|t| t.target),
                ) else {
                    return Some(false);
                };
                let ok = self.view.replace_range(t.0, t.1, t.2, &w);
                if let Some(th) = self.r16.thes.as_mut() {
                    th.target = Some((t.0, t.1, t.1 + w.len()));
                }
                ok
            }
            "review.thes.copy" => {
                if let Some(w) = self.r16.thes_word.take() {
                    self.requests.push(super::Request::SetClipboard {
                        text: w,
                        html: String::new(),
                        internal: String::new(),
                    });
                }
                false
            }
            "review.commentsGroup" | "review.trackingGroup" => self.r16_open_group(id),
            "review.showMarkup" => self.r16_show_markup_menu(),
            "review.markup.balloonsSub" => self.r16_balloons_menu(),
            "review.markup.peopleSub" => self.r16_people_menu(),
            "review.balloons.revisions" => self.r16_set_markup(json!({
                "cmd": "setMarkup", "deletionBalloons": true, "inline": false,
            })),
            "review.balloons.inline" => self.r16_set_markup(json!({
                "cmd": "setMarkup", "deletionBalloons": false, "inline": true,
            })),
            "review.balloons.commentsFormatting" => self.r16_set_markup(json!({
                "cmd": "setMarkup", "deletionBalloons": false, "inline": false,
            })),
            "review.deleteShown" | "review.deleteAll" => {
                let shown = id == "review.deleteShown";
                let m = self.view.markup().clone();
                let ids: Vec<i32> = self
                    .view
                    .comments()
                    .iter()
                    .filter(|c| c.parent.is_none())
                    .filter(|c| {
                        !shown
                            || (m.comments
                                && !matches!(m.mode, MarkupMode::None | MarkupMode::Original)
                                && !m.hidden_reviewers.contains(&c.author))
                    })
                    .map(|c| c.id)
                    .collect();
                !ids.is_empty() && self.r3_run(vec![json!({ "cmd": "deleteComments", "ids": ids })])
            }
            _ => return None,
        })
    }

    /// The word to look up: the selection when it is one word in one paragraph, else the
    /// word at the caret; with its range (body paragraph, byte start, byte end).
    fn r16_caret_word(&self) -> Option<(String, (usize, usize, usize))> {
        let s = self.view.engine().session()?;
        let sel = s.selection();
        let (a, b) = sel.ordered();
        let text = s.para_text(a.para)?;
        if a.para == b.para && a.offset < b.offset {
            let w = text.get(a.offset..b.offset)?.trim();
            if !w.is_empty() && !w.contains(char::is_whitespace) {
                return Some((w.to_owned(), (a.para, a.offset, b.offset)));
            }
        }
        let r = bo_spell::words(&text)
            .into_iter()
            .find(|r| r.start <= a.offset && a.offset <= r.end)?;
        let w = text.get(r.clone())?.to_owned();
        Some((w, (a.para, r.start, r.end)))
    }

    /// Review › Thesaurus (Shift+F7): open the pane on the word at the caret, or close it.
    pub(super) fn r16_toggle_thesaurus(&mut self) -> bool {
        if self.r16.thes.take().is_some() {
            self.relayout_pending = true;
            return true;
        }
        let (word, target) = self
            .r16_caret_word()
            .map_or((String::new(), None), |(w, t)| (w, Some(t)));
        self.r16.thes = Some(crate::doc_thes16::ThesPane::new(&word, target));
        self.r16.thes_fresh = true;
        self.relayout_pending = true;
        true
    }

    /// Width the round-16 pane takes at the right.
    pub(super) fn r16_pane_w(&self) -> f32 {
        self.r16
            .thes
            .as_ref()
            .map_or(0.0, |t| t.pane_width(self.scale))
    }

    /// Paint the Thesaurus pane (with the other panes at the right).
    pub(super) fn r16_paint_panes(&self, p: &mut dyn Painter) {
        if let Some(t) = &self.r16.thes {
            let labels = [
                self.t("doc16.thesaurus", "Thesaurus"),
                self.t("doc16.noResults", "No results found."),
                self.t("doc16.englishUs", "English (United States)"),
            ];
            t.paint(p, &labels);
        }
    }

    /// Input to the Thesaurus pane (`None`: not for it).
    fn r16_thes_input(&mut self, input: &Input) -> Option<bool> {
        if let Input::Key(crate::Key::F(7), m) = input {
            if m.shift {
                return Some(self.r16_toggle_thesaurus());
            }
        }
        let pointer = self.pointer;
        let (right, scale) = (self.size.0, self.scale);
        let t = self.r16.thes.as_mut()?;
        if t.key(input) {
            return Some(true);
        }
        // Size: the left edge follows the pointer until a press.
        if t.sizing {
            match *input {
                Input::Move { x, .. } | Input::Drag { x, .. } => {
                    t.size_to(right, x, scale);
                    self.relayout_pending = true;
                    return Some(true);
                }
                Input::Press { .. } | Input::Key(crate::Key::Escape, _) => {
                    t.sizing = false;
                    return Some(true);
                }
                _ => {}
            }
        }
        match *input {
            Input::Move { x, y } if t.contains(x, y) => Some(t.hover(x, y)),
            Input::Wheel(dy) if t.contains(pointer.0, pointer.1) => Some(t.wheel(-dy)),
            Input::Release { x, y } | Input::Drag { x, y } if t.contains(x, y) => Some(false),
            Input::Press { x, y, .. } if t.contains(x, y) => Some(match t.press(x, y) {
                crate::doc_thes16::ThesAction::None => false,
                crate::doc_thes16::ThesAction::Redraw => {
                    self.relayout_pending = true;
                    true
                }
                crate::doc_thes16::ThesAction::Close => {
                    self.r16.thes = None;
                    self.relayout_pending = true;
                    true
                }
                crate::doc_thes16::ThesAction::PaneMenu(r) => {
                    let items = [
                        ("r16.thes.size", "doc16.size", "Size"),
                        ("r16.thes.close", "doc16.close", "Close"),
                    ]
                    .iter()
                    .map(|(id, k, en)| MenuItem {
                        label: self.t(k, en),
                        choice: Choice::Command((*id).to_owned()),
                        checked: false,
                        dim: false,
                    })
                    .collect();
                    self.menu = Some(Menu::open(items, r, self.size, self.scale));
                    true
                }
                crate::doc_thes16::ThesAction::Menu(word, r) => {
                    let no_target = self.r16.thes.as_ref().is_none_or(|t| t.target.is_none());
                    self.r16.thes_word = Some(word);
                    let items = [
                        ("review.thes.insert", "doc16.insert", "Insert"),
                        ("review.thes.copy", "doc16.copy", "Copy"),
                    ]
                    .iter()
                    .map(|(id, k, en)| MenuItem {
                        label: self.t(k, en),
                        choice: Choice::Command((*id).to_owned()),
                        checked: false,
                        dim: *id == "review.thes.insert" && no_target,
                    })
                    .collect();
                    self.menu = Some(Menu::open(items, r, self.size, self.scale));
                    true
                }
            }),
            _ => None,
        }
    }

    /// The `extra["r16"]` context of the round-16 dialogs: the review display options and
    /// where the Reviewing Pane is.
    pub(super) fn r16_form_extra(&mut self) -> serde_json::Value {
        let pane = match self.r6.review.as_ref().map(|p| p.horizontal) {
            None => "off",
            Some(false) => "vertical",
            Some(true) => "horizontal",
        };
        json!({
            "markup": serde_json::to_value(self.view.markup()).unwrap_or_default(),
            "pane": pane,
            "records": self.merge_fields().1,
            "report": self.r16.report,
        })
    }

    /// Delete ▾: Delete / Delete All Comments Shown / Delete All Comments in Document (the
    /// first dim without a comment at the caret, the others without comments).
    fn r16_delete_menu(&mut self) -> bool {
        let anchor = self
            .r16
            .pop_anchor
            .take()
            .or_else(|| self.ribbon.rect_of(RibbonHit::Command("review.deleteMenu")))
            .unwrap_or([0.0, self.ribbon.bottom(), 1.0, 0.0]);
        let at_caret = self.view.comment_at_caret().is_some();
        let any = !self.view.comments().is_empty();
        let rows = [
            ("review.deleteComment", "doc16.delete", "Delete", !at_caret),
            (
                "review.deleteShown",
                "doc16.deleteShown",
                "Delete All Comments Shown",
                !any,
            ),
            (
                "review.deleteAll",
                "doc16.deleteAll",
                "Delete All Comments in Document",
                !any,
            ),
        ];
        let items = rows
            .iter()
            .map(|(id, k, en, dim)| MenuItem {
                label: self.t(k, en),
                choice: Choice::Command((*id).to_owned()),
                checked: false,
                dim: *dim,
            })
            .collect();
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        true
    }

    /// Select Recipients ▾: Type a New List… / Use an Existing List… / Choose from Outlook
    /// Contacts… (dim: no Outlook address book on this machine's BlueDoc).
    fn r16_recipients_menu(&mut self) -> bool {
        let anchor = self
            .ribbon
            .rect_of(RibbonHit::Command("mailings.recipients"))
            .unwrap_or([0.0, self.ribbon.bottom(), 1.0, 0.0]);
        let rows = [
            (
                "mailings.recipients.new",
                "docR5.typeNewList",
                "Type a New List...",
                false,
            ),
            (
                "mailings.recipients.existing",
                "docR5.useExistingList",
                "Use an Existing List...",
                false,
            ),
            (
                "mailings.recipients.contacts",
                "docR5.contactsList",
                "Choose from Outlook Contacts...",
                true,
            ),
        ];
        let items = rows
            .iter()
            .map(|(id, k, en, dim)| MenuItem {
                label: self.t(k, en),
                choice: Choice::Command((*id).to_owned()),
                checked: false,
                dim: *dim,
            })
            .collect();
        let icons = vec![
            Some("mailings.recipients.new"),
            Some("mailings.recipients.existing"),
            Some("mailings.recipients.contacts"),
        ];
        self.menu =
            Some(Menu::open(items, anchor, self.size, self.scale).with_decor(icons, Vec::new()));
        true
    }

    /// Finish & Merge ▾: Edit Individual Documents… / Print Documents… / Send Email
    /// Messages… (dim: the native window has no mail provider).
    fn r16_finish_menu(&mut self) -> bool {
        let anchor = self
            .ribbon
            .rect_of(RibbonHit::Command("mailings.finish"))
            .unwrap_or([0.0, self.ribbon.bottom(), 1.0, 0.0]);
        let rows = [
            (
                "mailings.finish.edit",
                "doc16.editIndividual",
                "Edit Individual Documents...",
                false,
            ),
            (
                "mailings.finish.print",
                "doc16.printDocuments",
                "Print Documents...",
                false,
            ),
            (
                "mailings.finish.email",
                "doc16.sendEmail",
                "Send Email Messages...",
                true,
            ),
        ];
        let items = rows
            .iter()
            .map(|(id, k, en, dim)| MenuItem {
                label: self.t(k, en),
                choice: Choice::Command((*id).to_owned()),
                checked: false,
                dim: *dim,
            })
            .collect();
        // With their icons, as the reference's menu.
        let icons = vec![
            Some("mailings.mergeDocument"),
            Some("file.print"),
            Some("mailings.finish.email"),
        ];
        self.menu =
            Some(Menu::open(items, anchor, self.size, self.scale).with_decor(icons, Vec::new()));
        true
    }

    /// Merge `edit|print:all|cur|a-b`: the merged document opens in a new window (with the
    /// Print page for Print Documents); this document keeps its merge fields.
    fn r16_merge(&mut self, spec: &str) -> bool {
        let (what, range) = spec.split_once(':').unwrap_or((spec, "all"));
        let range = match range {
            "all" => None,
            "cur" => {
                let r = self.merge_record.unwrap_or(0) + 1;
                Some((r, r))
            }
            r => r
                .split_once('-')
                .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))),
        };
        let Ok(bytes) = self.view.engine().merged_docx(range) else {
            return false;
        };
        self.r3.new_document = Some(bytes);
        self.r16.new_actions = (what == "print").then(|| "file.print".to_owned());
        self.requests.push(super::Request::OpenNewDocument);
        true
    }

    /// Merge fields (§17.16.5.35 MERGEFIELD) of the document whose names are not fields of
    /// the data source.
    fn r16_merge_errors(&mut self) -> Vec<String> {
        let fields = self.merge_fields().0;
        let Some(doc) = self.view.engine().document() else {
            return Vec::new();
        };
        let mut out: Vec<String> = Vec::new();
        for path in bo_model::index::body_paragraphs(doc) {
            let Some(p) = bo_model::index::para_at(doc, &path) else {
                continue;
            };
            for (_, c) in bo_edit::para::flatten(p) {
                if let bo_model::RunContent::Field { instr, .. } = &c {
                    if bo_edit::fields::field_kind(instr) != "MERGEFIELD" {
                        continue;
                    }
                    // The first argument after the field's kind, without quotes.
                    let name = bo_edit::fields::field_args(instr)
                        .into_iter()
                        .find(|a| !a.eq_ignore_ascii_case("MERGEFIELD") && !a.starts_with('\\'))
                        .unwrap_or_default()
                        .trim_matches('"')
                        .to_owned();
                    let known = fields.iter().any(|f| f.eq_ignore_ascii_case(&name));
                    if !known && !out.contains(&name) {
                        out.push(name);
                    }
                }
            }
        }
        out
    }

    /// Check for Errors: `sim` reports only; `pause` / `report` also complete the merge into a
    /// new window (`pause` stops at the first error, as the reference pauses on it).
    fn r16_check_errors(&mut self, mode: &str) -> bool {
        let errors = self.r16_merge_errors();
        self.r16.report = if errors.is_empty() {
            self.t("doc16.noErrors", "No mail merge errors have been found.")
        } else {
            let head = self.t(
                "doc16.errorsFound",
                "These merge fields are not in the data source:",
            );
            std::iter::once(head)
                .chain(errors.iter().map(|e| format!("  «{e}»")))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let merge = match mode {
            "pause" => errors.is_empty(),
            "report" => true,
            _ => false,
        };
        if merge {
            self.r16_merge("edit:all");
        }
        if mode == "sim" || !errors.is_empty() {
            return self.r3_open_form(crate::doc_forms::DocFormKind::MergeReport);
        }
        true
    }

    /// Interface commands the new window of the last merge runs first (Print Documents opens
    /// the Print page there).
    pub fn take_new_document_actions(&mut self) -> Option<String> {
        self.r16.new_actions.take()
    }

    /// Ribbon ids drawn as on (Highlight Merge Fields, Read Aloud).
    pub(super) fn r16_on_ids(&self) -> Vec<&'static str> {
        let mut on = Vec::new();
        if self.view.highlight_fields {
            on.push("mailings.highlightFields");
        }
        if self.r16.ra.is_some() {
            on.push("review.readAloud");
        }
        on
    }

    fn r16_set_markup(&mut self, cmd: serde_json::Value) -> bool {
        self.r3_run(vec![cmd])
    }

    /// Open (or close) the drop-down of a collapsed group under its button.
    fn r16_open_group(&mut self, id: &str) -> bool {
        let Some(sid) = crate::doc_review16::GROUP_IDS
            .iter()
            .find(|g| **g == id)
            .copied()
        else {
            return false;
        };
        if self.r16.pop.as_ref().is_some_and(|p| p.id == sid) {
            self.r16.pop = None;
            return true;
        }
        let Some(group) = crate::doc_review16::full_group(sid) else {
            return false;
        };
        let anchor = self.ribbon.rect_of(RibbonHit::Command(sid)).unwrap_or([
            0.0,
            self.ribbon.bottom(),
            1.0,
            0.0,
        ]);
        self.menu = None;
        self.r16.pop = Some(GroupPopup::new(sid, "ribbon.tabs.review", group, anchor));
        true
    }

    /// Show Markup ▾: the marks shown, then the Balloons and Specific People sub-menus.
    fn r16_show_markup_menu(&mut self) -> bool {
        let m = self.view.markup().clone();
        let row = |w: &Self, id: &str, key: &str, checked: bool| MenuItem {
            label: w.strings.get(key).to_owned(),
            choice: Choice::Command(id.to_owned()),
            checked,
            dim: false,
        };
        let items = vec![
            row(
                self,
                "review.markup.comments",
                "commands.review.markupComments",
                m.comments,
            ),
            row(
                self,
                "review.markup.insDel",
                "commands.review.markupInsDel",
                m.insertions_deletions,
            ),
            row(
                self,
                "review.markup.formatting",
                "commands.review.markupFormatting",
                m.formatting,
            ),
            row(
                self,
                "review.markup.balloonsSub",
                "commands.review.markupBalloons",
                false,
            ),
            MenuItem {
                label: self.t("doc16.specificPeople", "Specific People"),
                choice: Choice::Command("review.markup.peopleSub".to_owned()),
                checked: false,
                dim: false,
            },
        ];
        let anchor = self
            .ribbon
            .rect_of(RibbonHit::Command("review.showMarkup"))
            .unwrap_or([0.0, self.ribbon.bottom(), 1.0, 0.0]);
        let menu = Menu::open(items, anchor, self.size, self.scale)
            .with_arrows(vec![false, false, false, true, true], self.theme.hover);
        self.r16.markup_rows = (0..5).map(|i| menu.row_rect(i)).collect();
        self.menu = Some(menu);
        true
    }

    /// The anchor of a sub-menu beside row `row` of the Show Markup menu.
    fn r16_sub_anchor(&self, row: usize) -> [f32; 4] {
        match self.r16.markup_rows.get(row) {
            Some(r) => [r[0] + r[2] - 2.0 * self.scale, r[1], 1.0, 0.0],
            None => self
                .ribbon
                .rect_of(RibbonHit::Command("review.showMarkup"))
                .unwrap_or([0.0, self.ribbon.bottom(), 1.0, 0.0]),
        }
    }

    /// A sub-menu beside Show Markup's row `row`: right of it, or left when it would not fit
    /// (as the reference's cascading menus flip).
    fn r16_open_sub(&mut self, items: Vec<MenuItem>, row: usize) {
        let a = self.r16_sub_anchor(row);
        let m = Menu::open(items.clone(), a, self.size, self.scale);
        let r = m.rect();
        let row_rect = self.r16.markup_rows.get(row).copied();
        self.menu = Some(match row_rect {
            Some(rr) if r[0] < a[0] - 1.0 => Menu::open(
                items,
                [rr[0] - r[2] + 2.0 * self.scale, rr[1], 1.0, 0.0],
                self.size,
                self.scale,
            ),
            _ => m,
        });
    }

    /// Balloons ▸: Show Revisions in Balloons / Show All Revisions Inline / Show Only
    /// Comments and Formatting in Balloons.
    fn r16_balloons_menu(&mut self) -> bool {
        let m = self.view.markup().clone();
        let rows = [
            (
                "review.balloons.revisions",
                "doc16.balloonsRevisions",
                "Show Revisions in Balloons",
                m.deletion_balloons && !m.inline,
            ),
            (
                "review.balloons.inline",
                "doc16.balloonsInline",
                "Show All Revisions Inline",
                m.inline,
            ),
            (
                "review.balloons.commentsFormatting",
                "doc16.balloonsComments",
                "Show Only Comments and Formatting in Balloons",
                !m.deletion_balloons && !m.inline,
            ),
        ];
        let items = rows
            .iter()
            .map(|(id, key, en, on)| MenuItem {
                label: self.t(key, en),
                choice: Choice::Command((*id).to_owned()),
                checked: *on,
                dim: false,
            })
            .collect();
        self.r16_open_sub(items, 3);
        true
    }

    /// Reviewers of the document (comment and revision authors), in first-seen order.
    fn r16_reviewers(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for a in self
            .view
            .comments()
            .iter()
            .map(|c| c.author.clone())
            .chain(self.view.revisions().iter().map(|r| r.author.clone()))
        {
            if !a.is_empty() && !out.contains(&a) {
                out.push(a);
            }
        }
        out
    }

    /// Specific People ▸: All Reviewers, then each reviewer (ticked when shown).
    fn r16_people_menu(&mut self) -> bool {
        let m = self.view.markup().clone();
        let people = self.r16_reviewers();
        let mut items = vec![MenuItem {
            label: self.t("doc16.allReviewers", "All Reviewers"),
            choice: Choice::Command("review.people.all".to_owned()),
            checked: people.iter().all(|p| !m.hidden_reviewers.contains(p)),
            dim: false,
        }];
        for (i, p) in people.iter().enumerate() {
            items.push(MenuItem {
                label: p.clone(),
                choice: Choice::Command(format!("review.people.{i}")),
                checked: !m.hidden_reviewers.contains(p),
                dim: false,
            });
        }
        self.r16.people = people;
        self.r16_open_sub(items, 4);
        true
    }

    /// Show or hide one reviewer's marks (`i`), or everyone's (`all`).
    fn r16_toggle_person(&mut self, i: &str) -> bool {
        let mut hidden = self.view.markup().hidden_reviewers.clone();
        let people = if self.r16.people.is_empty() {
            self.r16_reviewers()
        } else {
            self.r16.people.clone()
        };
        if i == "all" {
            let all_shown = people.iter().all(|p| !hidden.contains(p));
            if all_shown {
                hidden.extend(people.iter().cloned());
            } else {
                hidden.retain(|h| !people.contains(h));
            }
        } else {
            let Some(p) = i.parse::<usize>().ok().and_then(|n| people.get(n)) else {
                return false;
            };
            if let Some(k) = hidden.iter().position(|h| h == p) {
                hidden.remove(k);
            } else {
                hidden.push(p.clone());
            }
        }
        self.r16_set_markup(json!({ "cmd": "setMarkup", "hiddenReviewers": hidden }))
    }

    /// Layout pass: the open group drop-down measures its buttons; the Thesaurus pane takes
    /// the window's right side alone (the newest pane wins).
    pub(super) fn r16_layout(&mut self, text: &dyn TextMeasure) {
        if let Some(p) = self.r16.pop.as_mut() {
            p.layout(text, &self.strings, self.size, self.scale);
        }
        if self.r16.thes.is_some() {
            let others = self.spell_pane.is_some()
                || self.comments_pane.is_some()
                || self.r3.styles_pane.is_some()
                || self.r3.restrict.is_some();
            if self.r16.thes_fresh {
                self.r16.thes_fresh = false;
                if others {
                    self.spell_pane = None;
                    self.comments_pane = None;
                    self.r3.styles_pane = None;
                    self.r3.restrict = None;
                    self.relayout_pending = true;
                }
            } else if others {
                self.r16.thes = None;
                self.relayout_pending = true;
            }
        }
        let (top, bottom, right, scale) = (
            self.chrome_bottom(),
            self.size.1 - self.status_h(),
            self.size.0,
            self.scale,
        );
        if let Some(t) = self.r16.thes.as_mut() {
            t.layout(right, top, bottom, scale);
        }
        let frame = self.view.frame_rect();
        if let Some(ra) = self.r16.ra.as_mut() {
            ra.layout(frame, scale);
        }
    }

    /// Review › Read Aloud: start reading from the caret, or stop.
    fn r16_toggle_read_aloud(&mut self) -> bool {
        if self.r16.ra.take().is_some() {
            self.requests.push(super::Request::SpeechControl(0));
            return true;
        }
        let Some(s) = self.view.engine().session() else {
            return false;
        };
        let at = s.selection().ordered().0;
        self.r16.ra = Some(crate::doc_speech16::ReadAloud::default());
        self.r16_ra_read(at.para, at.offset, 1)
    }

    /// Hand paragraph `para` (from byte `from`) to the voice, or the next paragraph with text
    /// in direction `dir` (1 / −1); past the end Read Aloud stops.
    fn r16_ra_read(&mut self, para: usize, from: usize, dir: i64) -> bool {
        let Some(s) = self.view.engine().session() else {
            return false;
        };
        let count = bo_model::index::body_paragraphs(s.doc()).len();
        let mut p = para as i64;
        let mut from = from;
        while p >= 0 && (p as usize) < count {
            let text = s.para_text(p as usize).unwrap_or_default();
            let rest = text.get(from.min(text.len())..).unwrap_or_default();
            if !rest.trim().is_empty() {
                let rate = self.r16.ra.as_ref().map_or(0, |r| r.rate);
                if let Some(ra) = self.r16.ra.as_mut() {
                    ra.para = p as usize;
                    ra.from = from.min(text.len());
                    ra.text = rest.to_owned();
                    ra.paused = false;
                    ra.waiting = 0;
                    ra.seen = false;
                    ra.word = None;
                }
                self.requests.push(super::Request::Speak {
                    text: rest.to_owned(),
                    rate,
                });
                return true;
            }
            p += dir;
            from = 0;
        }
        // Nothing more to read.
        self.requests.push(super::Request::SpeechControl(0));
        if let Some(ra) = self.r16.ra.as_mut() {
            ra.paused = true;
            ra.seen = false;
        }
        true
    }

    /// Read Aloud is on (the host polls the voice each tick).
    pub fn reading_aloud(&self) -> bool {
        self.r16.ra.as_ref().is_some_and(|r| !r.paused)
    }

    /// The word the voice is speaking (UTF-16 offset and length in the text handed over), or
    /// `None` when it is silent: the word is selected; a finished paragraph moves on to the
    /// next one. `true` when the window changed.
    pub fn speech_word(&mut self, word: Option<(usize, usize)>) -> bool {
        let Some(ra) = self.r16.ra.as_mut() else {
            return false;
        };
        if ra.paused {
            return false;
        }
        let Some((pos, len)) = word else {
            ra.waiting += 1;
            // Done with this paragraph (or the voice never started: about 3 s).
            if ra.seen || ra.waiting > 30 {
                let next = ra.para + 1;
                return self.r16_ra_read(next, 0, 1);
            }
            return false;
        };
        ra.seen = true;
        let a = crate::doc_speech16::byte_of_utf16(&ra.text, pos);
        let b = crate::doc_speech16::byte_of_utf16(&ra.text, pos + len);
        let w = (ra.from + a, ra.from + b);
        if ra.word == Some(w) {
            return false;
        }
        ra.word = Some(w);
        let para = ra.para;
        self.view
            .run_command(bo_engine::edit::Command::SetSelection {
                selection: bo_edit::Selection {
                    anchor: bo_edit::Pos::new(para, w.0),
                    focus: bo_edit::Pos::new(para, w.1),
                },
            })
    }

    /// Keys typed into the record box: digits, Backspace, Enter (go to that record), Escape;
    /// a press elsewhere leaves it.
    fn r16_record_input(&mut self, input: &Input) -> Option<bool> {
        let text = self.r16.record_edit.as_mut()?;
        match input {
            Input::Text(t) => {
                text.extend(t.chars().filter(char::is_ascii_digit));
                Some(true)
            }
            Input::Key(crate::Key::Backspace, _) => {
                text.pop();
                Some(true)
            }
            Input::Key(crate::Key::Escape, _) => {
                self.r16.record_edit = None;
                Some(true)
            }
            Input::Key(crate::Key::Enter, _) => {
                let n = text.parse::<usize>().unwrap_or(1).max(1);
                self.r16.record_edit = None;
                let count = self.merge_fields().1;
                let record = (n - 1).min(count.saturating_sub(1));
                self.merge_record = Some(record);
                Some(self.r3_run(vec![json!({ "cmd": "mergePreview", "record": record })]))
            }
            Input::Press { .. } => {
                self.r16.record_edit = None;
                None
            }
            _ => None,
        }
    }

    /// Font colour `hex` (`None`: Automatic) on the selection; it becomes the button's colour.
    fn r16_font_color(&mut self, hex: Option<&str>) -> bool {
        if let Some(h) = hex {
            self.r16.font_color = h.to_ascii_uppercase();
        }
        self.r3_run(vec![
            json!({ "cmd": "formatText", "format": { "kind": "color", "hex": hex } }),
        ])
    }

    /// Gradient ▸ of the Font Color palette: `none`, or `light|dark:right|down|left|up` — a
    /// w14 gradient fill from a tint (light) or shade (dark) of the button's colour to it.
    fn r16_gradient(&mut self, g: &str) -> bool {
        if g == "none" {
            return self.r3_run(vec![json!({ "cmd": "formatText",
                "format": { "kind": "advanced", "patch": { "gradient": null } } })]);
        }
        let base = if self.r16.font_color.is_empty() {
            "4472C4".to_owned()
        } else {
            self.r16.font_color.clone()
        };
        let (tone, dir) = g.split_once(':').unwrap_or(("light", "right"));
        let rgb = u32::from_str_radix(&base, 16).unwrap_or(0x0044_72C4);
        let mix = |c: u32, k: f32, to: f32| ((c as f32) * (1.0 - k) + to * k).round() as u32;
        let (r, gg, b) = ((rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255);
        let other = if tone == "dark" {
            (mix(r, 0.5, 0.0), mix(gg, 0.5, 0.0), mix(b, 0.5, 0.0))
        } else {
            (mix(r, 0.6, 255.0), mix(gg, 0.6, 255.0), mix(b, 0.6, 255.0))
        };
        let other = format!("{:02X}{:02X}{:02X}", other.0, other.1, other.2);
        let angle = match dir {
            "down" => 90.0,
            "left" => 180.0,
            "up" => 270.0,
            _ => 0.0,
        };
        self.r3_run(vec![
            json!({ "cmd": "formatText", "format": { "kind": "advanced",
            "patch": { "gradient": { "from": other, "to": base, "angle": angle } } } }),
        ])
    }

    /// Home › Font Color ▾: Automatic, theme / standard / recent colours, More Colors…,
    /// Gradient ▸ (the Page Color palette's look).
    fn r16_open_font_palette(&mut self) -> bool {
        self.menu = None;
        let anchor = self
            .ribbon
            .rect_of(RibbonHit::Command("format.color"))
            .unwrap_or([self.size.0 / 3.0, 120.0, 28.0, 28.0]);
        let doc = self.view.engine().document();
        let theme = doc
            .map(|d| d.design.theme_colors.clone())
            .filter(|t| !t.is_empty())
            .or_else(|| {
                doc.and_then(|d| d.design.theme_xml.clone())
                    .map(|x| bo_engine::theme_file::theme_colors(x.as_bytes()))
            })
            .unwrap_or_default();
        let labels = [
            self.t("doc14.pcHighContrast", "High-contrast only"),
            self.t("doc14.pcTheme", "Theme Colors"),
            self.t("doc14.pcStandard", "Standard Colors"),
            self.t("doc16.automatic", "Automatic"),
            self.t("doc14.pcMore", "More Colors..."),
            self.t("doc16.gradient", "Gradient"),
        ];
        let current = Some(self.r16.font_color.clone()).filter(|c| !c.is_empty());
        let mut pal = crate::doc_wm14::PagePalette::open(
            anchor,
            self.size,
            self.scale,
            super::r12::palette(&theme),
            labels,
            current,
        );
        pal.recent.clone_from(&self.r15.recent);
        pal.recent_label = self.t("doc15.recentColors", "Recent Colors");
        self.r16.font_pal = Some(Box::new(pal));
        true
    }

    /// Gradient ▸ beside the palette's last row.
    fn r16_gradient_menu(&mut self, at: [f32; 4]) -> bool {
        let rows = [
            ("none", "doc16.noGradient", "No Gradient"),
            (
                "light:right",
                "doc16.lightRight",
                "Light Variations: Linear Right",
            ),
            (
                "light:down",
                "doc16.lightDown",
                "Light Variations: Linear Down",
            ),
            (
                "light:left",
                "doc16.lightLeft",
                "Light Variations: Linear Left",
            ),
            ("light:up", "doc16.lightUp", "Light Variations: Linear Up"),
            (
                "dark:right",
                "doc16.darkRight",
                "Dark Variations: Linear Right",
            ),
            (
                "dark:down",
                "doc16.darkDown",
                "Dark Variations: Linear Down",
            ),
            (
                "dark:left",
                "doc16.darkLeft",
                "Dark Variations: Linear Left",
            ),
            ("dark:up", "doc16.darkUp", "Dark Variations: Linear Up"),
        ];
        let items = rows
            .iter()
            .map(|(id, k, en)| MenuItem {
                label: self.t(k, en),
                choice: Choice::Command(format!("r16.gradient:{id}")),
                checked: false,
                dim: false,
            })
            .collect();
        self.menu = Some(Menu::open(items, at, self.size, self.scale));
        true
    }

    /// Input to the Font Color palette (`None`: not open / not for it), and the press on
    /// the Font Color button's arrow part that opens it.
    fn r16_font_input(&mut self, input: &Input) -> Option<bool> {
        use crate::doc_wm14::PcPick;
        if self.r16.font_pal.is_none() {
            if let Input::Press { x, y, .. } = *input {
                let r = self.ribbon.rect_of(RibbonHit::Command("format.color"))?;
                let arrow =
                    x >= r[0] + r[2] * 0.62 && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
                if arrow && self.ribbon.active_label() == "ribbon.tabs.home" {
                    return Some(self.r16_open_font_palette());
                }
            }
            return None;
        }
        let p = self.r16.font_pal.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => p.hover(x, y),
            Input::Press { x, y, .. } => match p.press(x, y) {
                Some(PcPick::Nothing) => true,
                Some(PcPick::Color(hex)) => {
                    self.r16.font_pal = None;
                    self.r16_font_color(Some(&hex))
                }
                Some(PcPick::None) => {
                    self.r16.font_pal = None;
                    self.r16_font_color(None)
                }
                Some(PcPick::More) => {
                    self.r16.font_pal = None;
                    self.r16.font_more = true;
                    self.r3_open_form(crate::doc_forms::DocFormKind::Colors)
                }
                Some(PcPick::Fill) => {
                    self.r16.font_pal = None;
                    self.r16_gradient_menu([x, y, 1.0, 0.0])
                }
                None => {
                    self.r16.font_pal = None;
                    true
                }
            },
            Input::Key(crate::Key::Escape, _) => {
                self.r16.font_pal = None;
                true
            }
            Input::Text(_) | Input::Key(..) | Input::Release { .. } | Input::Wheel(_) => true,
            _ => return None,
        })
    }

    /// Input to the Read Aloud bar.
    fn r16_ra_input(&mut self, input: &Input) -> Option<bool> {
        use crate::doc_speech16::RaButton;
        let ra = self.r16.ra.as_mut()?;
        match *input {
            Input::Move { x, y } => {
                let h = ra.contains(x, y).then(|| ra.hit(x, y)).flatten();
                let changed = h != ra.hover;
                ra.hover = h;
                if ra.contains(x, y) {
                    Some(changed)
                } else if changed {
                    Some(true)
                } else {
                    None
                }
            }
            Input::Release { x, y } | Input::Drag { x, y } if ra.contains(x, y) => Some(false),
            Input::Press { x, y, .. } if ra.contains(x, y) => {
                let b = ra.hit(x, y)?;
                let (para, paused) = (ra.para, ra.paused);
                Some(match b {
                    RaButton::Play => {
                        if paused {
                            if ra.seen || ra.word.is_some() {
                                ra.paused = false;
                                self.requests.push(super::Request::SpeechControl(2));
                                true
                            } else {
                                let from = ra.from;
                                self.r16_ra_read(para, from, 1)
                            }
                        } else {
                            ra.paused = true;
                            self.requests.push(super::Request::SpeechControl(1));
                            true
                        }
                    }
                    RaButton::Next => self.r16_ra_read(para + 1, 0, 1),
                    RaButton::Prev => {
                        let back = if para == 0 { 0 } else { para - 1 };
                        self.r16_ra_read(back, 0, -1)
                    }
                    RaButton::Settings => {
                        let anchor = ra.settings_rect();
                        let rate = ra.rate;
                        let items = crate::doc_speech16::SPEEDS
                            .iter()
                            .map(|(k, en, r)| MenuItem {
                                label: self.t(k, en),
                                choice: Choice::Command(format!("r16.ra.speed:{r}")),
                                checked: *r == rate,
                                dim: false,
                            })
                            .collect();
                        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
                        true
                    }
                    RaButton::Close => {
                        self.r16.ra = None;
                        self.requests.push(super::Request::SpeechControl(0));
                        true
                    }
                })
            }
            _ => None,
        }
    }

    /// Input to the open group drop-down (`None`: not open or not for it).
    pub(super) fn r16_input(&mut self, input: &Input) -> Option<bool> {
        // A menu opened from the drop-down takes the input first.
        if self.menu.is_some() {
            return None;
        }
        if let Some(r) = self.r16_ra_input(input) {
            return Some(r);
        }
        if let Some(r) = self.r16_record_input(input) {
            return Some(r);
        }
        if let Some(r) = self.r16_font_input(input) {
            return Some(r);
        }
        if self.r16.pop.is_none() {
            return self.r16_thes_input(input);
        }
        let pop = self.r16.pop.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => {
                if pop.contains(x, y) {
                    pop.hover(x, y)
                } else {
                    pop.hover(x, y);
                    return None;
                }
            }
            Input::Press { x, y, .. } => {
                if pop.contains(x, y) {
                    pop.press(x, y)
                } else {
                    // A press outside closes it; a press on its own button only closes.
                    let own = self
                        .ribbon
                        .rect_of(RibbonHit::Command(pop.id))
                        .is_some_and(|r| {
                            x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
                        });
                    self.r16.pop = None;
                    if own {
                        return Some(true);
                    }
                    return None;
                }
            }
            Input::Release { x, y, .. } => {
                if !pop.contains(x, y) {
                    return None;
                }
                let Some(id) = pop.release(x, y) else {
                    return Some(true);
                };
                let anchor = pop.rect_of(id);
                self.r16.pop = None;
                if crate::catalog::is_menu(id) && id != "review.trackChanges" {
                    if let Some(a) = anchor {
                        if id == "review.deleteMenu" {
                            self.r16.pop_anchor = Some(a);
                            return Some(self.r16_delete_menu());
                        }
                        self.open_command_menu(id, a);
                        return Some(true);
                    }
                }
                self.action(id);
                true
            }
            Input::Key(crate::Key::Escape, _) => {
                self.r16.pop = None;
                true
            }
            _ => return None,
        })
    }

    /// Ribbon overlays of round 16: the markup display box, the open group drop-down.
    pub(super) fn r16_paint(&self, p: &mut dyn Painter) {
        self.r16_paint_markup_box(p);
        // The Font Color button's arrow part (split button, as the reference).
        if self.ribbon.active_label() == "ribbon.tabs.home" {
            if let Some(r) = self.ribbon.rect_of(RibbonHit::Command("format.color")) {
                let k = self.scale / 1.25;
                let (cx, cy, a) = (r[0] + r[2] - 5.0 * k, r[1] + r[3] / 2.0, 2.5 * k);
                p.path(
                    &[
                        bo_render::PathSeg::M(cx - a, cy - a / 2.0),
                        bo_render::PathSeg::L(cx + a, cy - a / 2.0),
                        bo_render::PathSeg::L(cx, cy + a / 2.0),
                        bo_render::PathSeg::Z,
                    ],
                    Some(self.theme.text),
                    None,
                );
            }
        }
        if let Some(fp) = &self.r16.font_pal {
            fp.paint(p);
        }
        if let (Some(t), Some(r)) = (
            &self.r16.record_edit,
            self.ribbon
                .rect_of(RibbonHit::Command("mailings.recordBox")),
        ) {
            // The box being typed in: a blue frame, the digits and a caret.
            let px = |v: f32| (v * self.scale).round();
            let b = [
                r[0] + px(2.0),
                r[1] + px(3.0),
                r[2] - px(4.0),
                r[3] - px(6.0),
            ];
            p.fill_rect(b[0], b[1], b[2], b[3], self.theme.accent);
            p.fill_rect(
                b[0] + 1.0,
                b[1] + 1.0,
                b[2] - 2.0,
                b[3] - 2.0,
                Rgba(255, 255, 255, 255),
            );
            let style = TextStyle {
                size: px(12.0),
                bold: false,
            };
            let y = (b[1] + (b[3] - style.size * 1.33) / 2.0).round();
            p.text(t, b[0] + px(6.0), y, style, self.theme.text);
            let cx = b[0] + px(6.0) + t.chars().count() as f32 * style.size * 0.55;
            p.fill_rect(
                cx.round(),
                b[1] + px(3.0),
                1.0,
                b[3] - px(6.0),
                self.theme.text,
            );
        }
        if let Some(ra) = &self.r16.ra {
            ra.paint(p);
        }
        if let Some(pop) = &self.r16.pop {
            let on = self.on_ids();
            pop.paint(
                p,
                &self.theme,
                &self.icons,
                &self.strings,
                self.view.format(),
                &on,
            );
        }
    }

    /// The markup display box (Simple Markup / All Markup / No Markup / Original), drawn as
    /// the reference's: its icon, a box with a 1 px #646464 frame on the ribbon grey, the
    /// mode's name and an arrow part behind a #949494 rule.
    fn r16_paint_markup_box(&self, p: &mut dyn Painter) {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command("review.markupMode")) else {
            return;
        };
        let k = self.scale / 1.25;
        let px = |v: f32| (v * k).round();
        let icon = px(24.0).min(r[3]);
        self.icons.draw(
            p,
            "review.markupMode",
            r[0] + px(2.0),
            r[1] + (r[3] - icon) / 2.0,
            icon,
            crate::icons::IconColors {
                ink: self.theme.text,
                accent: self.theme.accent,
                on: self.theme.accent_on,
                tinted: true,
            },
        );
        let bx = r[0] + px(27.0);
        let bw = (r[0] + r[2] - bx).max(px(40.0));
        // The box fills its row below a 2 px gap (the reference: 32 px from the row's top).
        let by = (r[1] + px(2.0)).round();
        let bh = (r[3] - px(2.0)).max(px(16.0)).round();
        let hover = self.ribbon.hovered() == Some(RibbonHit::Command("review.markupMode"));
        p.fill_rect(bx, by, bw, bh, BOX_EDGE);
        p.fill_rect(
            bx + 1.0,
            by + 1.0,
            bw - 2.0,
            bh - 2.0,
            if hover {
                self.theme.hover
            } else {
                self.theme.ribbon
            },
        );
        let arrow = px(17.0);
        p.fill_rect(bx + bw - arrow, by + 1.0, 1.0, bh - 2.0, BOX_SEP);
        let (cx, cy) = ((bx + bw - arrow / 2.0).round(), (by + bh / 2.0).round());
        let a = px(3.0);
        p.path(
            &[
                bo_render::PathSeg::M(cx - a, cy - a / 2.0),
                bo_render::PathSeg::L(cx + a, cy - a / 2.0),
                bo_render::PathSeg::L(cx, cy + a / 2.0),
                bo_render::PathSeg::Z,
            ],
            Some(BOX_ARROW),
            None,
        );
        let key = match self.view.markup().mode {
            MarkupMode::Simple => "commands.review.markupSimple",
            MarkupMode::All => "commands.review.markupAll",
            MarkupMode::None => "commands.review.markupNone",
            MarkupMode::Original => "commands.review.markupOriginal",
        };
        let style = TextStyle {
            size: (13.0 * self.scale).round(),
            bold: false,
        };
        p.push_clip(bx + 1.0, by, bw - arrow - 2.0, bh);
        p.text(
            self.strings.get(key),
            bx + px(3.0),
            (by + (bh - style.size * 1.33) / 2.0).round(),
            style,
            self.theme.text,
        );
        p.pop_clip();
    }
}
