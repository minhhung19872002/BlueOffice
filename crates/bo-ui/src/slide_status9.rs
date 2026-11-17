//! BlueSlide's status bar, round 19 (N6): the items the reference presentation app shows
//! after the slide counter, measured at 125 % (`show/slide-5x`, device px from the counter's
//! end): the proofing icon (+29), the language of the text at the caret or the
//! presentation's default (+23 after the icon; click → Language dialog), and
//! "Accessibility: Investigate / Good to go" in bold after its own icon (+23; click → the
//! Accessibility pane); on the right, Comments after Notes (Normal view). Outline View and
//! Notes Page show the left items too; Reading View and the slide show do not.
use super::viewtab::Pane;
use super::SlideWindow;
use crate::slide_view::ViewMode;
use crate::{Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};

/// Laid-out items: rectangles (proofing, language, accessibility, comments) and texts.
#[derive(Debug, Clone, Default)]
pub(super) struct Status9 {
    pub rects: [[f32; 4]; 4],
    pub lang: String,
    pub a11y: String,
    /// Accessibility issues found (status icon).
    pub issues: usize,
    /// Reading View's Previous, Menu and Next buttons.
    pub nav: [[f32; 4]; 3],
}

/// The Slide Show view button: a projector screen with a bar on top, a line on the screen
/// and a stand with its foot (own drawing), centred in `r`.
pub(super) fn paint_screen(p: &mut dyn Painter, r: [f32; 4], scale: f32, ink: bo_render::Rgba) {
    let k = scale / 1.25;
    let (cx, cy) = ((r[0] + r[2] / 2.0).round(), (r[1] + r[3] / 2.0).round());
    let pen = Some((ink, k.max(1.0)));
    let (w, h) = (16.0 * k, 9.0 * k);
    let top = cy - 9.0 * k;
    p.fill_rect(
        (cx - w / 2.0 - k).round(),
        top,
        (w + 2.0 * k).round(),
        (1.5 * k).max(1.0).round(),
        ink,
    );
    p.path(
        &[
            PathSeg::M(cx - w / 2.0 + 1.0, top + 1.5 * k),
            PathSeg::L(cx - w / 2.0 + 1.0, top + 1.5 * k + h),
            PathSeg::L(cx + w / 2.0 - 1.0, top + 1.5 * k + h),
            PathSeg::L(cx + w / 2.0 - 1.0, top + 1.5 * k),
            PathSeg::M(cx - 4.0 * k, top + 6.0 * k),
            PathSeg::L(cx + 4.0 * k, top + 6.0 * k),
            PathSeg::M(cx, top + 1.5 * k + h),
            PathSeg::L(cx, cy + 7.0 * k),
            PathSeg::M(cx - 4.0 * k, cy + 7.0 * k),
            PathSeg::L(cx + 4.0 * k, cy + 7.0 * k),
        ],
        None,
        pen,
    );
}

/// Display name of a BCP 47 tag (the Language dialog's list), else the tag.
pub(super) fn language_name(tag: &str) -> String {
    crate::doc_forms::LANGUAGES
        .iter()
        .find(|(t, _)| t.eq_ignore_ascii_case(tag))
        .map_or_else(|| tag.to_owned(), |(_, n)| (*n).to_owned())
}

impl SlideWindow {
    /// The language the status bar names: the run at the outline caret or of the shape
    /// whose text is edited, else the presentation's default text language (§21.1.2.3.9
    /// rPr@lang; `defaultTextStyle`), else English (United States).
    pub(super) fn status_lang_tag(&self) -> String {
        use bo_model::slide::{ShapeNode, TextRun};
        let Some(pres) = self.view.engine().presentation() else {
            return "en-US".into();
        };
        let first_lang = |tb: &bo_model::slide::TextBody, para: Option<usize>| {
            let paras: Vec<&bo_model::slide::Paragraph> = match para {
                Some(k) => tb.paragraphs.get(k).into_iter().collect(),
                None => tb.paragraphs.iter().collect(),
            };
            paras.into_iter().find_map(|p| {
                p.runs
                    .iter()
                    .find_map(|r| match r {
                        TextRun::Run { props, .. } => props.lang.clone(),
                        _ => None,
                    })
                    .or_else(|| p.end_props.as_ref().and_then(|e| e.lang.clone()))
            })
        };
        let shape_text = |slide: usize, pick: &dyn Fn(&bo_model::slide::Shape) -> bool| {
            pres.slides.get(slide).and_then(|s| {
                s.data.shapes.iter().find_map(|n| match n {
                    ShapeNode::Shape(sh) if pick(sh) => sh.text.as_ref(),
                    _ => None,
                })
            })
        };
        let found = if self.pane == Pane::Outline {
            self.outline_sel.and_then(|(slide, b)| {
                let is_title = |sh: &bo_model::slide::Shape| {
                    sh.nv
                        .placeholder
                        .as_ref()
                        .is_some_and(|p| matches!(p.kind.as_deref(), Some("title" | "ctrTitle")))
                };
                let is_body = |sh: &bo_model::slide::Shape| {
                    sh.nv.placeholder.as_ref().is_some_and(|p| {
                        matches!(p.kind.as_deref(), None | Some("body" | "subTitle"))
                    })
                };
                match b {
                    None => shape_text(slide, &is_title).and_then(|t| first_lang(t, None)),
                    Some(k) => shape_text(slide, &is_body).and_then(|t| first_lang(t, Some(k))),
                }
            })
        } else {
            self.view.text_edit().and_then(|te| {
                let id = te.id;
                shape_text(self.view.current(), &|sh| sh.nv.id == id)
                    .and_then(|t| first_lang(t, None))
            })
        };
        // Nothing edited: the editing language of the interface, as the reference shows
        // its own ("English (United States)" over a Vietnamese deck).
        found.unwrap_or_else(|| {
            if self.metric {
                "vi-VN".into()
            } else {
                "en-US".into()
            }
        })
    }

    /// Whether the status bar shows the proofing / language / accessibility items.
    fn status9_shown(&self) -> bool {
        self.view.mode() != ViewMode::Reading && self.page_master.is_none()
    }

    /// Lay the items out (after each input: the language follows the caret, the
    /// accessibility status the deck).
    pub(super) fn layout_status9(&mut self, text: &dyn TextMeasure) {
        let style = self.status_style();
        let bold = TextStyle {
            size: style.size,
            bold: true,
        };
        let sh = self.status_h();
        let y = self.size.1 - sh + 1.0;
        let h = sh - 1.0;
        let counter_end = self.px(11.0) + text.measure(&self.status_counter(), style).0;
        let k = self.scale / 1.25;
        let issues = self.a9_status_count();
        let lang = language_name(&self.status_lang_tag());
        let a11y = if issues > 0 {
            self.t(
                "slideNative.acc.statusInvestigate",
                "Accessibility: Investigate",
            )
        } else {
            self.t("slideNative.acc.statusGood", "Accessibility: Good to go")
        };
        let proof = [(counter_end + 29.0 * k).round(), y, (22.0 * k).round(), h];
        let lx = proof[0] + proof[2] + (23.0 * k).round();
        let lw = text.measure(&lang, style).0;
        let ax = (lx + lw + 23.0 * k).round();
        let aw = 27.0 * k + text.measure(&a11y, bold).0;
        // Comments ends 25 px before the view buttons, Notes 25 px before Comments
        // (reference at 125 %).
        let cl = self.t("slideNative.commentsStatus", "Comments");
        let cw = 25.0 * k + text.measure(&cl, style).0;
        let cx = (self.status[6][0] - 25.0 * k - cw).round();
        if self.view.mode() == ViewMode::Normal {
            let nw = self.status[0][2];
            self.status[0][0] = (cx - 25.0 * k - nw + self.px(6.0)).round();
        }
        self.status9 = Status9 {
            rects: [
                proof,
                [lx - 4.0 * k, y, lw + 8.0 * k, h],
                [ax, y, aw, h],
                [cx, y, cw, h],
            ],
            lang,
            a11y,
            issues,
            nav: [[-1000.0, 0.0, 0.0, 0.0]; 3],
        };
        if self.view.mode() == ViewMode::Reading {
            self.layout_reading_status();
        }
    }

    /// Reading View's status bar as the reference's (round 19, device px at 125 %): no
    /// zoom; the four view buttons (54 px) end 17 px from the window's right edge; Previous,
    /// Menu and Next centred 126, 82 and 38 px before them.
    fn layout_reading_status(&mut self) {
        let k = self.scale / 1.25;
        let (w, _) = self.size;
        let y = self.status[6][1];
        let h = self.status[6][3];
        let bw = (54.0 * k).round();
        let end = (w - 17.0 * k).round();
        for i in 0..4 {
            self.status[6 + i] = [end - (4 - i) as f32 * bw, y, bw, h];
        }
        // Zoom items leave the bar.
        for i in 1..6 {
            self.status[i] = [-1000.0, y, 0.0, h];
        }
        let start = self.status[6][0];
        let half = (18.0 * k).round();
        for (n, c) in [126.0, 82.0, 38.0].iter().enumerate() {
            self.status9.nav[n] = [(start - c * k - half).round(), y, 2.0 * half, h];
        }
    }

    /// Reading View's Previous / Menu / Next.
    fn reading_nav_press(&mut self, x: f32, y: f32) -> bool {
        let hit = self
            .status9
            .nav
            .iter()
            .position(|[rx, ry, rw, rh]| x >= *rx && x < rx + rw && y >= *ry && y < ry + rh);
        let key = |w: &mut Self, k: crate::Key| {
            w.view.handle(crate::Input::Key(k, crate::Mods::default()))
        };
        match hit {
            Some(0) => key(self, crate::Key::Left),
            Some(2) => key(self, crate::Key::Right),
            Some(1) => {
                let r = self.status9.nav[1];
                let items = [
                    ("slideNative.read.next", "Next", "rv:readnext"),
                    ("slideNative.read.prev", "Previous", "rv:readprev"),
                    ("slideNative.read.edit", "Edit View", "rv:readedit"),
                ]
                .iter()
                .map(|(key, d, c)| crate::popup::MenuItem {
                    label: self.t(key, d),
                    choice: crate::popup::Choice::Command((*c).to_owned()),
                    checked: false,
                    dim: false,
                })
                .collect();
                self.menu_kind = super::MenuKind::Duration;
                self.menu = Some(crate::popup::Menu::open(items, r, self.size, self.scale));
                true
            }
            _ => false,
        }
    }

    /// Reading View menu choices (`rv:read…`).
    pub(super) fn choose_reading(&mut self, v: &str) -> bool {
        let k = match v {
            "readnext" => crate::Key::Right,
            "readprev" => crate::Key::Left,
            _ => return self.action("view.normal"),
        };
        self.view
            .handle(crate::Input::Key(k, crate::Mods::default()))
    }

    /// The status bar's slide counter text (what `paint_status` draws first).
    fn status_counter(&self) -> String {
        match self.status_mode_text() {
            Some(t) => t,
            None if self.pane == Pane::Outline => self.t("slideNative.statusOutline", "Outline"),
            None => self
                .strings
                .get(if self.pane == Pane::NotesPage {
                    "slideNative.notesOf"
                } else {
                    "slideEditor.slideOf"
                })
                .replace(
                    "{{n}}",
                    &(self.view.show_slide().unwrap_or(self.view.current()) + 1).to_string(),
                )
                .replace("{{total}}", &self.view.slide_count().to_string()),
        }
    }

    /// A press on the status bar outside the zoom / view items.
    pub(super) fn status9_press(&mut self, x: f32, y: f32) -> bool {
        if self.view.mode() == ViewMode::Reading {
            return self.reading_nav_press(x, y);
        }
        if !self.status9_shown() {
            return false;
        }
        let hit = self
            .status9
            .rects
            .iter()
            .position(|[rx, ry, rw, rh]| x >= *rx && x < rx + rw && y >= *ry && y < ry + rh);
        match hit {
            Some(0) => self.start_spelling(),
            Some(1) => self.open_language_dialog(),
            Some(2) => self.open_a11y(),
            Some(3) if self.comments_toggle_shown() => self.action("review.comments"),
            _ => false,
        }
    }

    fn comments_toggle_shown(&self) -> bool {
        self.view.mode() == ViewMode::Normal && self.pane == Pane::Normal
    }

    /// Previous / Next: arrows in rings; Menu: a page with lines (own drawings).
    fn paint_reading_nav(&self, p: &mut dyn Painter) {
        let ink = self.theme.text;
        let k = self.scale / 1.25;
        let pen = Some((ink, k.max(1.0)));
        for (n, r) in self.status9.nav.iter().enumerate() {
            let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
            let rr = 8.0 * k;
            if n == 1 {
                let (x0, y0, w, h) = (cx - 7.0 * k, cy - 9.0 * k, 14.0 * k, 18.0 * k);
                p.path(
                    &[
                        PathSeg::M(x0, y0),
                        PathSeg::L(x0 + w, y0),
                        PathSeg::L(x0 + w, y0 + h),
                        PathSeg::L(x0, y0 + h),
                        PathSeg::Z,
                    ],
                    None,
                    pen,
                );
                for i in 0..4 {
                    let ly = (y0 + (4.0 + 3.5 * i as f32) * k).round();
                    p.fill_rect((x0 + 4.0 * k).round(), ly, (7.0 * k).round(), 1.0, ink);
                }
                p.fill_rect(x0 + 2.0 * k, y0, 1.0, h, ink);
                continue;
            }
            let c = 0.552 * rr;
            p.path(
                &[
                    PathSeg::M(cx, cy - rr),
                    PathSeg::C(cx + c, cy - rr, cx + rr, cy - c, cx + rr, cy),
                    PathSeg::C(cx + rr, cy + c, cx + c, cy + rr, cx, cy + rr),
                    PathSeg::C(cx - c, cy + rr, cx - rr, cy + c, cx - rr, cy),
                    PathSeg::C(cx - rr, cy - c, cx - c, cy - rr, cx, cy - rr),
                    PathSeg::Z,
                ],
                None,
                pen,
            );
            let d = if n == 0 { -1.0 } else { 1.0 };
            let t = 3.5 * k;
            p.path(
                &[
                    PathSeg::M(cx - d * t * 0.6, cy - t),
                    PathSeg::L(cx + d * t * 0.9, cy),
                    PathSeg::L(cx - d * t * 0.6, cy + t),
                    PathSeg::Z,
                ],
                Some(ink),
                None,
            );
        }
    }

    /// Draw the items.
    pub(super) fn paint_status9(&self, p: &mut dyn Painter) {
        if self.view.mode() == ViewMode::Reading && !self.view.showing() {
            self.paint_reading_nav(p);
            return;
        }
        if !self.status9_shown() || self.view.showing() {
            return;
        }
        let style = self.status_style();
        let bold = TextStyle {
            size: style.size,
            bold: true,
        };
        let ink = self.theme.text;
        let k = self.scale / 1.25;
        let [proof, lang, a11y, comments] = self.status9.rects;
        let ty = (proof[1] + (proof[3] - style.size * 1.33) / 2.0).round();
        let cy = proof[1] + proof[3] / 2.0;
        // Proofing: a page with a folded corner and a tick (own drawing).
        let (px, py) = (proof[0], (cy - 9.0 * k).round());
        let (pw, ph) = (14.0 * k, 18.0 * k);
        let f = 5.0 * k;
        let pen = Some((ink, k.max(1.0)));
        p.path(
            &[
                PathSeg::M(px + 0.5, py + 0.5),
                PathSeg::L(px + pw - f, py + 0.5),
                PathSeg::L(px + pw, py + f),
                PathSeg::L(px + pw, py + ph),
                PathSeg::L(px + 0.5, py + ph),
                PathSeg::Z,
                PathSeg::M(px + pw - f, py + 0.5),
                PathSeg::L(px + pw - f, py + f),
                PathSeg::L(px + pw, py + f),
            ],
            None,
            pen,
        );
        p.path(
            &[
                PathSeg::M(px + 9.0 * k, py + 12.0 * k),
                PathSeg::L(px + 13.0 * k, py + 16.0 * k),
                PathSeg::L(px + 21.0 * k, py + 6.0 * k),
            ],
            None,
            Some((ink, (1.4 * k).max(1.0))),
        );
        p.text(&self.status9.lang, lang[0] + 4.0 * k, ty, style, ink);
        // Accessibility: a figure in a ring with a small magnifier (own drawing).
        let (ax, ay) = (a11y[0] + 10.0 * k, cy - 1.0 * k);
        let r = 8.0 * k;
        let c = 0.552 * r;
        p.path(
            &[
                PathSeg::M(ax, ay - r),
                PathSeg::C(ax + c, ay - r, ax + r, ay - c, ax + r, ay),
                PathSeg::C(ax + r, ay + c, ax + c, ay + r, ax, ay + r),
                PathSeg::C(ax - c, ay + r, ax - r, ay + c, ax - r, ay),
                PathSeg::C(ax - r, ay - c, ax - c, ay - r, ax, ay - r),
                PathSeg::Z,
                PathSeg::M(ax - 4.0 * k, ay - 2.0 * k),
                PathSeg::L(ax + 4.0 * k, ay - 2.0 * k),
                PathSeg::M(ax, ay - 2.0 * k),
                PathSeg::L(ax, ay + 2.0 * k),
                PathSeg::L(ax - 3.0 * k, ay + 6.0 * k),
                PathSeg::M(ax, ay + 2.0 * k),
                PathSeg::L(ax + 3.0 * k, ay + 6.0 * k),
            ],
            None,
            pen,
        );
        p.fill_rect(ax - 1.0 * k, ay - 5.5 * k, 2.0 * k, 2.0 * k, ink);
        if self.status9.issues > 0 {
            // A small cross on the ring's lower right: something to look at.
            let (xx, xy) = (ax + 6.0 * k, ay + 4.0 * k);
            p.path(
                &[
                    PathSeg::M(xx, xy),
                    PathSeg::L(xx + 6.0 * k, xy + 6.0 * k),
                    PathSeg::M(xx + 6.0 * k, xy),
                    PathSeg::L(xx, xy + 6.0 * k),
                ],
                None,
                Some((ink, (1.4 * k).max(1.0))),
            );
        }
        p.text(&self.status9.a11y, a11y[0] + 27.0 * k, ty, bold, ink);
        if self.comments_toggle_shown() {
            // Comments: a filled speech bubble (own drawing) and the label.
            let (bx, by) = (comments[0], (cy - 7.0 * k).round());
            let (bw, bh) = (19.0 * k, 12.0 * k);
            p.path(
                &[
                    PathSeg::M(bx, by),
                    PathSeg::L(bx + bw, by),
                    PathSeg::L(bx + bw, by + bh),
                    PathSeg::L(bx + 8.0 * k, by + bh),
                    PathSeg::L(bx + 4.0 * k, by + bh + 4.0 * k),
                    PathSeg::L(bx + 4.0 * k, by + bh),
                    PathSeg::L(bx, by + bh),
                    PathSeg::Z,
                ],
                Some(if self.comments.shown {
                    self.theme.accent
                } else {
                    Rgba(0x44, 0x44, 0x44, 255)
                }),
                None,
            );
            p.text(
                &self.t("slideNative.commentsStatus", "Comments"),
                comments[0] + 25.0 * k,
                ty,
                style,
                ink,
            );
        }
    }
}
