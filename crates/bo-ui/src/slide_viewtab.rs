//! BlueSlide's View tab (N6, round 4), groups as on the reference presentation app:
//! Presentation Views (Normal, Outline View, Slide Sorter, Notes Page, Reading View), Master
//! Views (Slide Master, with its contextual tab and Close Master View), Show (Gridlines,
//! Guides, Notes) and Zoom (Zoom, Fit to Window).
use super::SlideWindow;
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::{Group, Item, RibbonHit, Tab};
use crate::spin_box::paint_check;
use crate::{Painter, TextStyle};
use bo_engine::slide_edit::master::MasterCommand;
use bo_render::Rgba;

/// Commands of the tab.
pub const COMMANDS: [&str; 7] = [
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
            Group {
                label: "slideNative.masterViews",
                large: vec![L("view.slideMaster")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.show",
                large: vec![L("view.notes")],
                rows: vec![vec![S("view.gridlines")], vec![S("view.guides")]],
            },
            Group {
                label: "ribbon.groups.zoom",
                large: vec![L("view.zoom"), L("view.fitSlide")],
                rows: vec![],
            },
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

impl SlideWindow {
    /// Run a View tab command; `None` when `id` is not one.
    pub(super) fn view_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "view.outline" | "view.notesPage" => {
                let want = if id == "view.outline" {
                    Pane::Outline
                } else {
                    Pane::NotesPage
                };
                self.view.set_mode(crate::slide_view::ViewMode::Normal);
                self.pane = want;
                self.relayout_pending = true;
                true
            }
            "view.normal" | "view.sorter" | "view.reading" => {
                // The window's own panes give way to the view's modes.
                if self.pane == Pane::Normal {
                    return None;
                }
                self.pane = Pane::Normal;
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
            "view.zoom" => {
                self.open_zoom_menu();
                true
            }
            _ => return None,
        })
    }

    fn open_zoom_menu(&mut self) {
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command("view.zoom")) else {
            return;
        };
        let cur = (self.view.zoom() * 100.0).round() as u32;
        let mut items: Vec<MenuItem> = [400u32, 200, 100, 66, 50, 33]
            .iter()
            .map(|z| MenuItem {
                label: format!("{z}%"),
                choice: Choice::Command(format!("zoom:{z}")),
                checked: cur == *z,
                dim: false,
            })
            .collect();
        items.push(MenuItem {
            label: self.strings.get("commands.view.fitSlide").to_owned(),
            choice: Choice::Command("zoom:fit".into()),
            checked: self.view.fits(),
            dim: false,
        });
        self.menu_kind = super::MenuKind::Duration;
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    /// Menu choices of the tab.
    pub(super) fn choose_view(&mut self, kind: &str, v: &str) -> Option<bool> {
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
        match self.pane {
            Pane::Outline => on.push("view.outline"),
            Pane::NotesPage => on.push("view.notesPage"),
            Pane::Normal => {}
        }
        if self.master_view {
            on.push("view.slideMaster");
        }
    }

    /// A press in the outline pane goes to that slide.
    pub(super) fn pane_press(&mut self, x: f32, y: f32) -> Option<bool> {
        match self.pane {
            Pane::Outline => {
                let area = self.view.thumbs_area();
                if !crate::spin_box::inside(area, x, y) {
                    return None;
                }
                let hit = self
                    .outline_rows
                    .iter()
                    .rev()
                    .find(|(top, _)| y >= *top)
                    .map(|(_, slide)| *slide)?;
                Some(self.view.go_to(hit))
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
            Pane::Outline => self.paint_outline(p),
            Pane::NotesPage => self.paint_notes_page(p),
            Pane::Normal => {}
        }
        if self.pane != Pane::NotesPage && (self.gridlines || self.guides) {
            self.paint_grid(p);
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
            // Dotted lines every half inch (36 pt), grey as the reference draws them.
            let dot = Rgba(0x9a, 0x9a, 0x9a, 255);
            let step = 36.0 * k;
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

    /// Lay out the outline rows (slide number, title, bullets) for hit testing.
    pub(super) fn layout_outline(&mut self, text_size: f32) {
        self.outline_rows.clear();
        if self.pane != Pane::Outline {
            return;
        }
        let area = self.view.thumbs_area();
        let line = (text_size * 1.5).round();
        let mut y = area[1] + self.px(8.0);
        let Some(pres) = self.view.engine().presentation() else {
            return;
        };
        for (i, s) in pres.slides.iter().enumerate() {
            self.outline_rows.push((y, i));
            let o = bo_engine::slide_edit::outline::slide_outline(s);
            y += line * (1 + o.bullets.len()) as f32 + self.px(6.0);
        }
    }

    fn paint_outline(&self, p: &mut dyn Painter) {
        let area = self.view.thumbs_area();
        p.fill_rect(area[0], area[1], area[2], area[3], Rgba(255, 255, 255, 255));
        let Some(pres) = self.view.engine().presentation() else {
            return;
        };
        let size = self.text_size;
        let style = TextStyle { size, bold: false };
        let bold = TextStyle {
            bold: true,
            ..style
        };
        let line = (size * 1.5).round();
        let ink = self.theme.text;
        p.push_clip(area[0], area[1], area[2], area[3]);
        for ((top, i), s) in self.outline_rows.iter().zip(&pres.slides) {
            let o = bo_engine::slide_edit::outline::slide_outline(s);
            let current = *i == self.view.current();
            if current {
                p.fill_rect(
                    area[0],
                    *top - self.px(2.0),
                    area[2],
                    line + self.px(4.0),
                    Rgba(0xd2, 0xd2, 0xd2, 255),
                );
            }
            p.text(
                &(i + 1).to_string(),
                area[0] + self.px(6.0),
                *top,
                style,
                ink,
            );
            // A small slide icon, then the title.
            let ix = area[0] + self.px(22.0);
            p.fill_rect(
                ix,
                *top + self.px(3.0),
                self.px(16.0),
                self.px(11.0),
                Rgba(0x80, 0x80, 0x80, 255),
            );
            p.fill_rect(
                ix + 1.0,
                *top + self.px(3.0) + 1.0,
                self.px(16.0) - 2.0,
                self.px(11.0) - 2.0,
                Rgba(255, 255, 255, 255),
            );
            p.text(&o.title, ix + self.px(22.0), *top, bold, ink);
            let mut y = *top + line;
            for b in &o.bullets {
                let x = ix + self.px(22.0) + f32::from(b.level) * self.px(18.0);
                p.text(&format!("•  {}", b.text), x, y, style, ink);
                y += line;
            }
        }
        p.pop_clip();
    }

    /// Notes Page: a portrait page on the grey canvas with the slide above and the notes
    /// under it (ECMA-376 §19.3.1.26 notes page layout).
    fn paint_notes_page(&self, p: &mut dyn Painter) {
        let c = self.view.canvas_rect();
        p.fill_rect(c[0], c[1], c[2], c[3], Rgba(0xe6, 0xe6, 0xe6, 255));
        // 7.5 × 10 inch page fitted with a margin.
        let m = self.px(16.0);
        let ph = (c[3] - 2.0 * m).max(10.0);
        let pw = (ph * 0.75).min(c[2] - 2.0 * m);
        let ph = pw / 0.75;
        let (px0, py0) = ((c[0] + (c[2] - pw) / 2.0).round(), (c[1] + m).round());
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
        let (ix, iy) = ((px0 + (pw - iw) / 2.0).round(), (py0 + ph * 0.09).round());
        p.fill_rect(
            ix - 1.0,
            iy - 1.0,
            iw + 2.0,
            ih + 2.0,
            Rgba(0x80, 0x80, 0x80, 255),
        );
        self.view
            .paint_slide_into(p, [ix, iy, iw.round(), ih.round()]);
        let style = TextStyle {
            size: self.text_size,
            bold: false,
        };
        let mut y = iy + ih + ph * 0.06;
        let line = (self.text_size * 1.5).round();
        let tx = px0 + pw * 0.1;
        p.push_clip(tx, y, pw * 0.8, (py0 + ph - y - ph * 0.08).max(0.0));
        for para in self.view.notes().lines() {
            p.text(para, tx, y, style, self.theme.text);
            y += line;
        }
        p.pop_clip();
    }

    /// Check boxes of Gridlines and Guides over their slots' icon place.
    pub(super) fn paint_view_checks(&self, p: &mut dyn Painter) {
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
