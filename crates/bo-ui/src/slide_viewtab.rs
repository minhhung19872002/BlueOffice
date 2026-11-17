//! BlueSlide's View tab (N6, round 4), groups as on the reference presentation app:
//! Presentation Views (Normal, Outline View, Slide Sorter, Notes Page, Reading View), Master
//! Views (Slide Master, with its contextual tab and Close Master View), Show (Gridlines,
//! Guides, Notes) and Zoom (Zoom, Fit to Window).
use super::SlideWindow;
use crate::ribbon::{Group, Item, RibbonHit, Tab};
use crate::spin_box::paint_check;
use crate::{Painter, TextMeasure, TextStyle};
use bo_engine::slide_edit::master::MasterCommand;
use bo_render::Rgba;

/// Commands of the tab.
pub const COMMANDS: [&str; 8] = [
    "view.gridSettings",
    "view.outline",
    "view.notesPage",
    "view.slideMaster",
    "view.closeMaster",
    "view.gridlines",
    "view.guides",
    "view.zoom",
];

/// Editing-area views the window draws itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Pane {
    /// Thumbnails and the slide.
    #[default]
    Normal,
    /// The outline in place of the thumbnails.
    Outline,
    /// The notes page in place of the slide.
    NotesPage,
}

pub(super) fn view_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "slideEditor.viewTab",
        groups: vec![
            Group {
                label: "slideNative.presentationViews",
                large: vec![
                    L("view.normal"),
                    L("view.outline"),
                    L("view.sorter"),
                    L("view.notesPage"),
                    L("view.reading"),
                ],
                rows: vec![],
            },
            super::views5::master_views_group(),
            // Check boxes first, then Notes, as the reference.
            Group {
                label: "slideNative.showGroup",
                large: vec![L("view.notes")],
                rows: vec![
                    vec![S("view.ruler")],
                    vec![S("view.gridlines")],
                    vec![S("view.guides")],
                ],
            },
            Group {
                label: "ribbon.groups.zoom",
                large: vec![L("view.zoom"), L("view.fitSlide")],
                rows: vec![],
            },
            super::views5::color_group(),
            super::views5::window_group(),
        ],
    }
}

/// The Slide Master contextual tab.
pub(super) fn master_tab() -> Tab {
    Tab {
        label: "slideNative.slideMasterTab",
        groups: vec![Group {
            label: "slideNative.close",
            large: vec![Item::Large("view.closeMaster")],
            rows: vec![],
        }],
    }
}

pub(super) fn install_strings(s: &mut crate::strings::Strings) {
    for (id, key) in [
        ("view.outline", "outlineView"),
        ("view.notesPage", "notesPage"),
        ("view.slideMaster", "slideMaster"),
        ("view.closeMaster", "closeMaster"),
        ("view.gridlines", "gridlines"),
        ("view.guides", "guides"),
        ("view.zoom", "zoom"),
        ("view.fitSlide", "fitToWindow"),
        ("view.reading", "readingView"),
        ("view.sorter", "slideSorter"),
    ] {
        let v = s.get(&format!("slideNative.{key}")).to_owned();
        s.set(&format!("commands.{id}"), &v);
    }
}

pub(super) fn install_icons(icons: &mut crate::icons::Icons) {
    let white = Rgba(255, 255, 255, 255);
    let blue = Rgba(0x44, 0x80, 0xc0, 255);
    let orange = Rgba(0xed, 0x7d, 0x31, 255);
    icons.add_solid(
        "view.outline",
        &[
            ("M3 4h18v16h-18Z", Some(white)),
            ("M3 4h18v16h-18Z", None),
            ("M5 7h3v2h-3Z", Some(orange)),
            ("M10 7.5h9M10 11.5h9M10 15.5h7", None),
        ],
    );
    icons.add_solid(
        "view.notesPage",
        &[
            ("M6 2h12v20h-12Z", Some(white)),
            ("M6 2h12v20h-12Z", None),
            ("M8 4h8v6h-8Z", Some(blue)),
            ("M8 13h8M8 16h8M8 19h5", None),
        ],
    );
    icons.add_solid(
        "view.slideMaster",
        &[
            ("M2 5h20v14h-20Z", Some(white)),
            ("M2 5h20v14h-20Z", None),
            ("M5 8h14v2h-14Z", Some(blue)),
            ("M5 12h14M5 15h10", None),
        ],
    );
    icons.add_solid(
        "view.closeMaster",
        &[
            ("M3 4h18v16h-18Z", Some(white)),
            ("M3 4h18v16h-18Z", None),
            ("M8 8l8 8M16 8l-8 8", None),
            ("M14 14l6 6", None),
        ],
    );
    icons.alias("view.zoom", "edit.find");
}

/// The outline's font: the interface family at `size` px with a run's emphasis.
fn outline_font(size: f32, bold: bool, italic: bool, underline: bool) -> crate::Font<'static> {
    crate::Font {
        family: "Segoe UI",
        size,
        weight: if bold { 700 } else { 400 },
        italic,
        underline,
    }
}

impl SlideWindow {
    /// Run a View tab command; `None` when `id` is not one.
    pub(super) fn view_action(&mut self, id: &str) -> Option<bool> {
        if let Some(r) = self.views5_action(id) {
            return Some(r);
        }
        Some(match id {
            "view.outline" | "view.notesPage" => {
                let want = if id == "view.outline" {
                    Pane::Outline
                } else {
                    Pane::NotesPage
                };
                self.view.set_mode(crate::slide_view::ViewMode::Normal);
                self.pane = want;
                // Both take the whole view on the reference: no thumbnails or notes pane.
                self.view.set_full_page(true);
                self.relayout_pending = true;
                true
            }
            "view.normal" | "view.sorter" | "view.reading" => {
                // The window's own panes give way to the view's modes.
                if self.pane == Pane::Normal {
                    return None;
                }
                self.pane = Pane::Normal;
                self.view.set_full_page(false);
                self.relayout_pending = true;
                self.action(id);
                true
            }
            "view.slideMaster" | "view.closeMaster" => {
                let on = id == "view.slideMaster";
                let done = self.view.run_master(MasterCommand::MasterView { on });
                if done {
                    self.master_view = on;
                    self.context = super::ins::Context::None;
                    let tabs = self.fixed_tabs();
                    self.base_tabs = tabs.len();
                    self.ribbon.set_tabs(tabs);
                    self.ribbon
                        .set_active_tab(if on { 0 } else { self.base_tabs - 1 });
                    self.relayout_pending = true;
                }
                done
            }
            "view.gridlines" => {
                self.gridlines = !self.gridlines;
                true
            }
            "view.guides" => {
                self.guides = !self.guides;
                true
            }
            "view.zoom" => self.open_form8(super::forms8::Dlg::Zoom),
            "view.gridSettings" => self.open_form8(super::forms8::Dlg::Grid),
            _ => return None,
        })
    }

    /// Menu choices of the tab.
    pub(super) fn choose_view(&mut self, kind: &str, v: &str) -> Option<bool> {
        if kind == "pm" {
            return Some(self.choose_pm(v));
        }
        (kind == "zoom").then(|| {
            if v == "fit" {
                self.view.set_zoom(None)
            } else {
                let z = v.parse::<f32>().unwrap_or(100.0) / 100.0;
                self.view.set_zoom(Some(z))
            }
        })
    }

    /// Commands shown as on (the current view, gridlines, guides).
    pub(super) fn view_toggled(&self, on: &mut Vec<&'static str>) {
        use crate::slide_view::ViewMode;
        match self.pane {
            Pane::Outline => on.push("view.outline"),
            Pane::NotesPage => on.push("view.notesPage"),
            // The current view shows pressed, as the reference.
            Pane::Normal => on.push(match self.view.mode() {
                ViewMode::Sorter => "view.sorter",
                ViewMode::Reading => "view.reading",
                ViewMode::Normal => "view.normal",
            }),
        }
        if self.master_view {
            on.push("view.slideMaster");
        }
        self.views5_toggled(on);
    }

    /// A press in the outline pane goes to that slide.
    pub(super) fn pane_press(&mut self, x: f32, y: f32) -> Option<bool> {
        match self.pane {
            Pane::Outline => {
                let area = self.outline_area();
                if !crate::spin_box::inside(area, x, y) {
                    return None;
                }
                // Presses on the outline never reach the slide canvas under it.
                let Some(hit) = self
                    .outline_rows
                    .iter()
                    .rev()
                    .find(|(top, _)| y >= *top)
                    .map(|(_, slide)| *slide)
                else {
                    return Some(false);
                };
                // The line clicked is chosen for promoting / demoting (round 17).
                self.outline_sel = self.outline_line_at(y);
                // The caret goes to the character nearest the press (round 19).
                self.outline9.click_x = Some(x);
                self.view.go_to(hit);
                Some(true)
            }
            Pane::NotesPage => {
                // The page is drawn over the canvas: presses there do not edit the slide.
                let c = self.view.canvas_rect();
                crate::spin_box::inside(c, x, y).then_some(false)
            }
            Pane::Normal => None,
        }
    }

    /// Paint the window's own panes and the gridlines / guides over the slide.
    pub(super) fn paint_view_extras(&self, p: &mut dyn Painter) {
        if self.view.mode() != crate::slide_view::ViewMode::Normal || self.view.showing() {
            return;
        }
        match self.pane {
            Pane::Outline => {
                self.paint_outline(p);
                self.paint_outline_sel(p);
            }
            Pane::NotesPage => self.paint_notes_page(p),
            Pane::Normal => {}
        }
        if self.pane != Pane::NotesPage && (self.gridlines || self.guides) {
            self.paint_grid(p);
        }
        if self.pane == Pane::Normal {
            self.paint_ruler(p);
        }
    }

    fn paint_grid(&self, p: &mut dyn Painter) {
        let (ox, oy) = self.view.slide_origin();
        let k = self.view.scale();
        let (sw, sh) = self.view.slide_points();
        let (w, h) = (sw * k, sh * k);
        let c = self.view.canvas_rect();
        p.push_clip(c[0], c[1], c[2], c[3]);
        if self.gridlines {
            // Dotted lines every half inch (36 pt), grey as the reference draws them; a grid
            // spacing of 1/4" or more (Grid and Guides) is drawn at its own spacing.
            let dot = Rgba(0x9a, 0x9a, 0x9a, 255);
            let sp = self.view.grid_spacing();
            let step = if sp >= 18.0 { sp as f32 } else { 36.0 } * k;
            let d = (2.0 * self.scale).round().max(2.0);
            let mut x = ox + step;
            while x < ox + w - 0.5 {
                let mut y = oy;
                while y < oy + h {
                    p.fill_rect(x.round(), y.round(), 1.0, 1.0, dot);
                    y += d * 2.0;
                }
                x += step;
            }
            let mut y = oy + step;
            while y < oy + h - 0.5 {
                let mut x = ox;
                while x < ox + w {
                    p.fill_rect(x.round(), y.round(), 1.0, 1.0, dot);
                    x += d * 2.0;
                }
                y += step;
            }
        }
        if self.guides {
            // The default guides through the slide's centre, dashed.
            let ink = Rgba(0x7f, 0x7f, 0x7f, 255);
            let dash = (4.0 * self.scale).round();
            let (cx, cy) = ((ox + w / 2.0).round(), (oy + h / 2.0).round());
            let mut t = 0.0;
            while t < h {
                p.fill_rect(cx, oy + t, 1.0, dash.min(h - t), ink);
                t += dash * 2.0;
            }
            let mut t = 0.0;
            while t < w {
                p.fill_rect(ox + t, cy, dash.min(w - t), 1.0, ink);
                t += dash * 2.0;
            }
        }
        p.pop_clip();
    }

    /// Outline View's area: the whole view (round 18 — the reference draws the outline
    /// over the window, no slide beside it).
    pub(super) fn outline_area(&self) -> [f32; 4] {
        let c = self.view.canvas_rect();
        // The canvas plus its scroll bar column (the outline has no scroll bar of its own
        // while it fits).
        [c[0], c[1], c[2] + self.px(15.0), c[3]]
    }

    /// Outline text size and line pitch (reference at 125 %: 28–29 px lines).
    pub(super) fn outline_metrics(&self) -> (f32, f32) {
        let size = (self.text_size * 1.3).round();
        (size, (size * 1.45).round().max(1.0))
    }

    /// Lay out the outline rows (slide number, title, paragraphs) for hit testing, and the
    /// pieces of each paragraph line (number, runs with their emphasis) measured for drawing.
    pub(super) fn layout_outline(&mut self, text: &dyn TextMeasure) {
        self.outline_rows.clear();
        self.outline_lines.clear();
        if self.pane != Pane::Outline {
            return;
        }
        let area = self.outline_area();
        let (size, line) = self.outline_metrics();
        let mut y = area[1] + self.px(13.0);
        let Some(pres) = self.view.engine().presentation() else {
            return;
        };
        let tx = area[0] + self.px(45.6);
        let mut rows = Vec::new();
        let mut lines = Vec::new();
        let mut hits = Vec::new();
        let caret_line = self.outline_sel;
        for (i, s) in pres.slides.iter().enumerate() {
            rows.push((y, i));
            let o = bo_engine::slide_edit::outline::slide_outline(s);
            // Numbers inherited from the layout / master list styles too (round 19).
            let paras = bo_engine::slide_edit::outline::outline_paras(pres, i);
            hits.push(super::outline9::Hit {
                line: (i, None),
                y,
                pieces: vec![(o.title.replace('\n', " "), tx, true, false, false)],
            });
            let mut ly = y + line;
            for (k, b) in o.bullets.iter().enumerate() {
                // Empty paragraphs are not listed (reference) — but the one being typed in.
                if b.text.trim().is_empty() && caret_line != Some((i, Some(k))) {
                    continue;
                }
                let mut x = tx + f32::from(b.level) * self.px(15.2);
                let mut pieces = Vec::new();
                let para = paras.get(k);
                if let Some(p) = para.filter(|p| p.number.is_some()) {
                    let n = p.number.clone().unwrap_or_default();
                    let f = outline_font(size, false, false, false);
                    let w = text.measure_font(&n, &f).0;
                    lines.push((ly, vec![(n, x, false, false, false)]));
                    // The text starts after the number, or at the paragraph's hanging
                    // indent when wider (reference: "1. Bước" with a 0.5" hang, "1.One"
                    // with the master's 0.25"; 0.5" = one level step, 15.2 px).
                    let space = text.measure_font(" ", &f).0;
                    let hang = self.px(15.2 * p.hang / 36.0);
                    x += if hang >= w + space { hang } else { w };
                }
                match para {
                    Some(p) if !p.runs.is_empty() => {
                        for r in &p.runs {
                            let t = r.text.replace('\u{b}', " ");
                            // Body text is never bold in the outline (reference: a run's
                            // b="1" is dropped, italic and underline are kept; titles are
                            // bold).
                            let f = outline_font(size, false, r.italic, r.underline);
                            let w = text.measure_font(&t, &f).0;
                            pieces.push((t, x, false, r.italic, r.underline));
                            x += w;
                        }
                    }
                    _ => pieces.push((b.text.replace('\u{b}', " "), x, false, false, false)),
                }
                hits.push(super::outline9::Hit {
                    line: (i, Some(k)),
                    y: ly,
                    pieces: pieces.clone(),
                });
                lines.push((ly, pieces));
                ly += line;
            }
            y = ly + self.px(8.8);
        }
        self.outline_rows = rows;
        self.outline_lines = lines;
        self.outline9.hits = hits;
        self.place_outline_caret(text, size, line);
    }

    /// The outline as the reference draws it: grey ground, slide number, a framed slide
    /// icon (accent frame on the current slide), bold title, body paragraphs indented by
    /// level with their automatic numbers and run emphasis, without bullet marks, in the
    /// reference's grey 68.
    fn paint_outline(&self, p: &mut dyn Painter) {
        let area = self.outline_area();
        p.fill_rect(
            area[0],
            area[1],
            area[2],
            area[3],
            Rgba(0xe6, 0xe6, 0xe6, 255),
        );
        let Some(pres) = self.view.engine().presentation() else {
            return;
        };
        let (size, _) = self.outline_metrics();
        let style = TextStyle { size, bold: false };
        let ink = Rgba(0x44, 0x44, 0x44, 255);
        p.push_clip(area[0], area[1], area[2], area[3]);
        for ((top, i), s) in self.outline_rows.iter().zip(&pres.slides) {
            let o = bo_engine::slide_edit::outline::slide_outline(s);
            let current = *i == self.view.current();
            p.text(
                &(i + 1).to_string(),
                area[0] + self.px(6.0),
                *top,
                style,
                ink,
            );
            // The slide icon: 22 × 18 logical px with a 4 px frame (accent when current).
            // Its top 3 px over the title's capitals (reference at 125 %).
            let (ix, iy) = (area[0] + self.px(17.0), *top + self.px(2.4));
            let (iw, ih, fw) = (self.px(21.6), self.px(18.4), self.px(4.0));
            let frame = if current {
                self.theme.accent
            } else {
                Rgba(0xd2, 0xd5, 0xd8, 255)
            };
            p.fill_rect(ix, iy, iw, ih, frame);
            p.fill_rect(
                ix + fw,
                iy + fw,
                iw - 2.0 * fw,
                ih - 2.0 * fw,
                Rgba(255, 255, 255, 255),
            );
            let tx = area[0] + self.px(45.6);
            p.text_font(
                &o.title,
                tx,
                *top,
                &outline_font(size, true, false, false),
                ink,
            );
        }
        for (y, pieces) in &self.outline_lines {
            for (t, x, b, it, u) in pieces {
                p.text_font(t, *x, *y, &outline_font(size, *b, *it, *u), ink);
            }
        }
        p.pop_clip();
    }

    /// The Notes Page's 7.5 × 10 inch page fitted in the canvas with a margin (reference:
    /// 11 px at 125 %).
    pub(super) fn notes_page_rect(&self) -> [f32; 4] {
        let c = self.view.canvas_rect();
        let m = self.px(9.0);
        let ph = (c[3] - 2.0 * m).max(10.0);
        let pw = (ph * 0.75).min(c[2] - 2.0 * m).max(1.0);
        let ph = pw / 0.75;
        [
            (c[0] + (c[2] - pw) / 2.0).round(),
            (c[1] + m).round(),
            pw,
            ph,
        ]
    }

    /// The Notes Page's zoom: its page width over 7.5 inches at 100 % (the status bar's
    /// figure, as the reference's "67%").
    pub(super) fn notes_page_zoom(&self) -> f32 {
        self.notes_page_rect()[2] / (540.0 * self.scale * 96.0 / 72.0)
    }

    /// Notes Page: a portrait page on the grey canvas with the slide above and the notes
    /// under it (ECMA-376 §19.3.1.26 notes page layout).
    fn paint_notes_page(&self, p: &mut dyn Painter) {
        let c = self.view.canvas_rect();
        p.fill_rect(c[0], c[1], c[2], c[3], Rgba(0xe6, 0xe6, 0xe6, 255));
        let [px0, py0, pw, ph] = self.notes_page_rect();
        p.fill_rect(
            px0 - 1.0,
            py0 - 1.0,
            pw + 2.0,
            ph + 2.0,
            Rgba(0xc6, 0xc6, 0xc6, 255),
        );
        p.fill_rect(px0, py0, pw, ph, Rgba(255, 255, 255, 255));
        let (sw, sh) = self.view.slide_points();
        let iw = pw * 0.8;
        let ih = iw * sh / sw;
        let (ix, iy) = ((px0 + (pw - iw) / 2.0).round(), (py0 + ph * 0.084).round());
        p.fill_rect(
            ix - 1.0,
            iy - 1.0,
            iw + 2.0,
            ih + 2.0,
            // A black 1 px frame round the slide image, as the reference.
            Rgba(0, 0, 0, 255),
        );
        self.view
            .paint_slide_into(p, [ix, iy, iw.round(), ih.round()]);
        // The notes placeholder as the reference frames it (round 17): a dashed box from
        // 46 % to 92 % of the page under the slide, "Click to add text" while empty.
        let (nx, ny) = (ix, (py0 + ph * 0.46).round());
        let (nw, nh) = (iw.round(), (ph * 0.46).round());
        let dash = Rgba(0x80, 0x80, 0x80, 255);
        let d = self.px(3.2);
        let mut t = 0.0;
        while t < nw {
            let l = d.min(nw - t);
            p.fill_rect(nx + t, ny, l, 1.0, dash);
            p.fill_rect(nx + t, ny + nh, l, 1.0, dash);
            t += 2.0 * d;
        }
        let mut t = 0.0;
        while t < nh {
            let l = d.min(nh - t);
            p.fill_rect(nx, ny + t, 1.0, l, dash);
            p.fill_rect(nx + nw, ny + t, 1.0, l, dash);
            t += 2.0 * d;
        }
        let small = TextStyle {
            size: (self.text_size * 0.85).round(),
            bold: false,
        };
        let line = (small.size * 1.4).round();
        let (tx, mut y) = (nx + self.px(8.0), ny + self.px(6.0));
        p.push_clip(nx, ny, nw, nh);
        let notes = self.view.notes();
        if notes.trim().is_empty() {
            p.text(
                &self.t("slideNative.clickToAddText", "Click to add text"),
                tx,
                y,
                small,
                Rgba(0, 0, 0, 255),
            );
        }
        for para in notes.lines() {
            p.text(para, tx, y, small, Rgba(0, 0, 0, 255));
            y += line;
        }
        p.pop_clip();
    }

    /// Check boxes of Gridlines and Guides over their slots' icon place.
    pub(super) fn paint_view_checks(&self, p: &mut dyn Painter) {
        self.paint_views5_checks(p);
        for (id, on) in [
            ("view.gridlines", self.gridlines),
            ("view.guides", self.guides),
        ] {
            let Some(r) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
                continue;
            };
            let b = self.px(14.4);
            let (x, y) = (r[0] + self.px(4.0), (r[1] + (r[3] - b) / 2.0).round());
            paint_check(p, [x, y, b, b], on, true, self.theme.text);
        }
    }
}
