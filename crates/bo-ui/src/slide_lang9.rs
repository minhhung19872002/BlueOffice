//! The Language dialog, round 19 (N6): opened by the status bar's language and Review ›
//! Language › Set Proofing Language…, laid out as the reference's (454 × 416 at 125 %,
//! `show/slide-5x`): "Update What?" with Document / Slide / Selected Text, "Mark selected
//! text as:" over the list (the language in use first, then the others by name), the note
//! on dictionaries, "Do not check spelling", Default… / OK / Cancel. OK marks the text
//! (§21.1.2.3.9 rPr@lang, `noProof`) of the selection, the slide or every slide, in one
//! undoable step.
use super::forms8::{Dlg, Open, Radios};
use super::sform::{Form, Kind};
use super::SlideWindow;
use bo_engine::slide_edit::text::TextFormat;
use bo_engine::slide_edit::SlideCommand;
use bo_model::slide::ShapeNode;

/// The list's languages: the one in use first, then the rest by name (indices into
/// `doc_forms::LANGUAGES`).
pub(super) fn language_order(current: &str) -> Vec<usize> {
    let all = &crate::doc_forms::LANGUAGES;
    let mut rest: Vec<usize> = (0..all.len())
        .filter(|&i| !all[i].0.eq_ignore_ascii_case(current))
        .collect();
    rest.sort_by(|a, b| all[*a].1.cmp(all[*b].1));
    let mut out: Vec<usize> = (0..all.len())
        .filter(|&i| all[i].0.eq_ignore_ascii_case(current))
        .collect();
    out.extend(rest);
    out
}

/// Move the option buttons onto one row, as the reference's.
pub(super) fn place_language_radios(open: &mut Open) {
    for (id, x, w) in [
        ("lw_doc", 131.0, 100.0),
        ("lw_slide", 245.0, 66.0),
        ("lw_sel", 321.0, 122.0),
    ] {
        if let Some(c) = open.form.get_mut(id) {
            c.rect = [x, 49.0, w, 20.0];
        }
    }
}

impl SlideWindow {
    /// Open the Language dialog.
    pub(super) fn open_language_dialog(&mut self) -> bool {
        self.open_form8(Dlg::Language)
    }

    /// The dialog's form and its option buttons.
    pub(super) fn language_form(&self, ok: &str, cancel: &str) -> (Form, Vec<Radios>) {
        let mut f = Form::new(
            &self.t("slideNative.lang.title", "Language"),
            (454.0, 416.0),
            Vec::new(),
        );
        f.add(
            "l_what",
            None,
            [16.0, 49.0, 110.0, 20.0],
            Kind::Label(self.t("slideNative.lang.updateWhat", "Update What?")),
        );
        f.add(
            "l_mark",
            None,
            [16.0, 90.0, 300.0, 20.0],
            Kind::Label(self.t("slideNative.lang.markAs", "Mark selected text as:")),
        );
        let current = self.status_lang_tag();
        let order = language_order(&current);
        let items = order
            .iter()
            .map(|i| crate::doc_forms::LANGUAGES[*i].1.to_owned())
            .collect();
        f.add(
            "list",
            None,
            [16.0, 113.0, 422.0, 165.0],
            Kind::List {
                items,
                sel: Some(0),
                top: 0,
            },
        );
        f.add(
            "l_note1",
            None,
            [16.0, 288.0, 422.0, 20.0],
            Kind::Label(self.t(
                "slideNative.lang.note1",
                "The speller automatically uses dictionaries of the selected",
            )),
        );
        f.add(
            "l_note2",
            None,
            [16.0, 309.0, 422.0, 20.0],
            Kind::Label(self.t("slideNative.lang.note2", "language, if available.")),
        );
        f.add(
            "noproof",
            None,
            [18.0, 334.0, 300.0, 20.0],
            Kind::Check {
                label: self.t("slideNative.lang.noProof", "Do not check spelling"),
                on: false,
            },
        );
        f.add(
            "default",
            None,
            [16.0, 363.0, 108.0, 34.0],
            Kind::Button {
                label: self.t("slideNative.lang.default", "Default..."),
                default: false,
            },
        );
        // Changing the presentation's default language is not offered yet.
        f.enable("default", false);
        f.add(
            "ok",
            None,
            [209.0, 363.0, 120.0, 34.0],
            Kind::Button {
                label: ok.to_owned(),
                default: true,
            },
        );
        f.add(
            "cancel",
            None,
            [331.0, 363.0, 107.0, 34.0],
            Kind::Button {
                label: cancel.to_owned(),
                default: false,
            },
        );
        let has_sel = self.view.text_edit().is_some() || !self.view.selection().is_empty();
        let radios = vec![Radios {
            ids: &["lw_doc", "lw_slide", "lw_sel"],
            at: [131.0, 49.0],
            labels: vec![
                self.t("slideNative.lang.document", "Document"),
                self.t("slideNative.lang.slide", "Slide"),
                self.t("slideNative.lang.selected", "Selected Text"),
            ],
            chosen: if has_sel { 2 } else { 0 },
            pitch: 0.0,
            enabled: vec![true, true, has_sel],
        }];
        (f, radios)
    }

    /// Select every top-level shape of the current slide.
    fn select_all_shapes9(&mut self) {
        let ids: Vec<u32> = self
            .view
            .engine()
            .presentation()
            .and_then(|p| p.slides.get(self.view.current()))
            .map(|s| {
                s.data
                    .shapes
                    .iter()
                    .filter_map(ShapeNode::nv)
                    .map(|nv| nv.id)
                    .collect()
            })
            .unwrap_or_default();
        if !ids.is_empty() {
            self.view.run(SlideCommand::Select { ids });
        }
    }

    /// OK: mark the text of the scope chosen.
    pub(super) fn language_ok(&mut self, open: &Open) {
        let f = &open.form;
        let current = self.status_lang_tag();
        let order = language_order(&current);
        let lang = if f.on("noproof") {
            None
        } else {
            f.sel("list")
                .and_then(|i| order.get(i))
                .map(|i| crate::doc_forms::LANGUAGES[*i].0.to_owned())
        };
        let fmt = TextFormat::Lang { lang };
        match open.radios.first().map_or(0, |r| r.chosen) {
            2 => {
                self.view.format_text(fmt);
            }
            1 => {
                self.select_all_shapes9();
                self.view.format_text(fmt);
            }
            _ => {
                let back = self.view.current();
                let since = self.view.history_mark();
                for i in 0..self.view.slide_count() {
                    self.view.go_to(i);
                    self.select_all_shapes9();
                    self.view.format_text(fmt.clone());
                }
                self.view.go_to(back);
                self.view.merge_history(since);
            }
        }
        self.relayout_pending = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_language_first_then_by_name() {
        let o = language_order("vi-VN");
        let all = &crate::doc_forms::LANGUAGES;
        assert_eq!(all[o[0]].0, "vi-VN");
        assert_eq!(o.len(), all.len());
        let names: Vec<&str> = o[1..].iter().map(|i| all[*i].1).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }
}
