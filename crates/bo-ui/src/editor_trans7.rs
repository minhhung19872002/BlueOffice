//! BlueEditor round 7: View › Page Transitions as the reference editor's dialog — Transition
//! (No Transition and the twelve PDF styles), Direction (the ones the style takes), Speed (Slow,
//! Medium, Fast) and the pages (all, or a range) — written as each page's `/Trans`
//! (`bo_pdfread::transitions`, one undo step).
use crate::dialog::{Dialog, DialogButton};
use crate::editor_five::FiveAsk;
use crate::editor_window::EditorWindow;
use bo_pdfread::{Transition, TransitionDirection as D, TransitionKind as K};

/// Speeds offered: label key and seconds (`/D`).
const SPEEDS: [(&str, f32); 3] = [
    ("pdfEditor7.speedSlow", 2.0),
    ("pdfEditor7.speedMedium", 1.0),
    ("pdfEditor7.speedFast", 0.5),
];

fn kind_key(k: K) -> &'static str {
    match k {
        K::Split => "pdfEditor7.trSplit",
        K::Blinds => "pdfEditor7.trBlinds",
        K::Box => "pdfEditor7.trBox",
        K::Wipe => "pdfEditor7.trWipe",
        K::Dissolve => "pdfEditor7.trDissolve",
        K::Glitter => "pdfEditor7.trGlitter",
        K::Push => "pdfEditor7.trPush",
        K::Cover => "pdfEditor7.trCover",
        K::Uncover => "pdfEditor7.trUncover",
        K::Fade => "pdfEditor7.trFade",
        K::Replace => "pdfEditor7.trReplace",
        K::Fly => "pdfEditor7.trFly",
    }
}

fn dir_key(d: D) -> &'static str {
    match d {
        D::HorizontalIn => "pdfEditor7.dirHorizontalIn",
        D::HorizontalOut => "pdfEditor7.dirHorizontalOut",
        D::VerticalIn => "pdfEditor7.dirVerticalIn",
        D::VerticalOut => "pdfEditor7.dirVerticalOut",
        D::Horizontal => "pdfEditor7.dirHorizontal",
        D::Vertical => "pdfEditor7.dirVertical",
        D::In => "pdfEditor7.dirIn",
        D::Out => "pdfEditor7.dirOut",
        D::LeftToRight => "pdfEditor7.dirLeftToRight",
        D::TopToBottom => "pdfEditor7.dirTopToBottom",
        D::RightToLeft => "pdfEditor7.dirRightToLeft",
        D::BottomToTop => "pdfEditor7.dirBottomToTop",
        D::TopLeftToBottomRight => "pdfEditor7.dirTopLeft",
    }
}

impl EditorWindow {
    /// Transition labels: No Transition first, then the styles.
    fn transition_labels(&self) -> Vec<String> {
        std::iter::once(self.t("pdfEditor7.trNone"))
            .chain(K::ALL.iter().map(|k| self.t(kind_key(*k))))
            .collect()
    }

    /// The style named `label` (`None`: No Transition).
    fn kind_of(&self, label: &str) -> Option<K> {
        K::ALL.into_iter().find(|k| self.t(kind_key(*k)) == label)
    }

    fn direction_labels(&self, k: Option<K>) -> Vec<String> {
        let dirs = k.map_or(&[][..], K::directions);
        if dirs.is_empty() {
            return vec![self.t("pdfEditor7.dirNone")];
        }
        dirs.iter().map(|d| self.t(dir_key(*d))).collect()
    }

    /// Open the Page Transitions dialog with the current page's transition.
    pub(crate) fn open_transitions(&mut self) {
        let page = self.r.view().map_or(0, crate::ReaderView::current_page);
        let now = self
            .with_doc(|d| Ok(d.page_transition(page)))
            .ok()
            .flatten();
        let kind = now.map(|t| t.kind);
        let kinds = self.transition_labels();
        let dirs = self.direction_labels(kind);
        let speeds: Vec<String> = SPEEDS.iter().map(|s| self.t(s.0)).collect();
        let kind_label = kind.map_or_else(|| kinds[0].clone(), |k| self.t(kind_key(k)));
        let dir_label = now
            .and_then(|t| t.direction)
            .map_or_else(|| dirs[0].clone(), |d| self.t(dir_key(d)));
        let speed = now.map_or(1, |t| {
            SPEEDS
                .iter()
                .position(|s| (s.1 - t.seconds).abs() < 0.26)
                .unwrap_or(1)
        });
        let fields = vec![
            (self.t("pdfEditor7.transition"), kind_label),
            (self.t("pdfEditor7.direction"), dir_label),
            (self.t("pdfEditor7.speed"), speeds[speed].clone()),
            (self.t("pdfEditor7.pages"), self.t("pdfEditor.cropAll")),
        ];
        let mut d = Dialog::new(
            crate::dialog::DialogKind::Comment,
            self.t("pdfEditor7.transitionsTitle"),
            fields,
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfReader.cancel"), DialogButton::Cancel),
            ],
            vec![self.t("pdfEditor7.pagesHint")],
        )
        .with_choices(0, kinds)
        .with_choices(1, dirs)
        .with_choices(2, speeds)
        .with_caption();
        d.layout(
            &crate::editor_window::Estimate,
            self.window_size(),
            self.ui_scale(),
        );
        self.dialog2 = Some((d, crate::editor_actions::Ask2::Five(FiveAsk::Transitions)));
    }

    /// Keep the Direction list in step with the chosen transition.
    pub(crate) fn transitions_changed(&mut self) {
        let Some((d, crate::editor_actions::Ask2::Five(FiveAsk::Transitions))) = &self.dialog2
        else {
            return;
        };
        let kind = d.values().first().and_then(|v| self.kind_of(v));
        let dirs = self.direction_labels(kind);
        if let Some((d, _)) = self.dialog2.as_mut() {
            d.set_choices(1, dirs);
        }
    }

    /// OK in the Page Transitions dialog.
    pub(crate) fn transitions_answer(&mut self, b: DialogButton, v: &[String]) {
        if b != DialogButton::Ok {
            return;
        }
        let get = |i: usize| v.get(i).cloned().unwrap_or_default();
        let kind = self.kind_of(&get(0));
        let n = self.r.view().map_or(0, crate::ReaderView::page_count);
        let all = self.t("pdfEditor.cropAll");
        let pages = crate::editor_more::parse_pages(&get(3), &all, n);
        if pages.is_empty() {
            let msg = self.t("pdfEditor7.noPages");
            self.set_status(&msg);
            return;
        }
        let t = kind.map(|k| {
            let direction = k
                .directions()
                .iter()
                .copied()
                .find(|d| self.t(dir_key(*d)) == get(1));
            let seconds = SPEEDS
                .iter()
                .find(|s| self.t(s.0) == get(2))
                .map_or(1.0, |s| s.1);
            Transition {
                kind: k,
                direction,
                seconds,
            }
        });
        match self.doc_edit(|d| d.set_page_transitions(&pages, t.as_ref())) {
            Ok(()) => self.after_doc_edit(),
            Err(e) => self.set_status(&e),
        }
    }

    /// Form › XFA: a static XFA form becomes a plain AcroForm form (its fields stay); a
    /// dynamic one is not offered.
    pub(crate) fn static_xfa(&mut self) {
        let kind = self
            .with_doc(|d| Ok(d.xfa_kind()))
            .unwrap_or(bo_pdfread::XfaKind::None);
        let key = match kind {
            bo_pdfread::XfaKind::None => "pdfEditor7.xfaNone",
            bo_pdfread::XfaKind::Dynamic => "pdfEditor7.xfaDynamic",
            bo_pdfread::XfaKind::Static => {
                match self.doc_edit(bo_pdfread::PdfDocument::drop_static_xfa) {
                    Ok(()) => {
                        self.after_doc_edit();
                        "pdfEditor7.xfaDone"
                    }
                    Err(e) => {
                        self.set_status(&e);
                        return;
                    }
                }
            }
        };
        let msg = self.t(key);
        self.set_status(&msg);
    }
}
