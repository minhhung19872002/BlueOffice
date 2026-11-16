//! BlueSlide View tab, round 5 (N6): Handout Master and Notes Master views with their own
//! contextual tab (placeholders, orientation, slides per page, Close), the Ruler over the
//! editing area, and the Color / Grayscale / Black and White modes — as the reference
//! presentation app's View tab at 1500 px.
use super::SlideWindow;
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::{Group, Item, RibbonHit, Tab};
use crate::slide_view::colormode::ColorMode;
use crate::spin_box::paint_check;
use crate::{Painter, TextStyle};
use bo_engine::slide_edit::page_master::{PageKind, PageMasterCommand};
use bo_render::Rgba;

/// Commands of this round's View tab additions.
pub const COMMANDS: [&str; 20] = [
    "pm.themes",
    "win.new",
    "win.switch",
    "win.arrange",
    "win.cascade",
    "view.handoutMaster",
    "view.notesMaster",
    "view.closePageMaster",
    "view.ruler",
    "view.color",
    "view.grayscale",
    "view.blackWhite",
    "pm.orientation",
    "pm.perPage",
    "pm.header",
    "pm.date",
    "pm.footer",
    "pm.number",
    "pm.slideImage",
    "pm.body",
];

/// Placeholder check boxes of the master tabs: command, flag index.
const PLACEHOLDERS: [(&str, usize); 6] = [
    ("pm.header", 0),
    ("pm.date", 1),
    ("pm.footer", 2),
    ("pm.number", 3),
    ("pm.slideImage", 4),
    ("pm.body", 5),
];

/// Master Views group (Slide, Handout, Notes Master).
pub(super) fn master_views_group() -> Group {
    use Item::Large as L;
    Group {
        label: "slideNative.masterViews",
        large: vec![
            L("view.slideMaster"),
            L("view.handoutMaster"),
            L("view.notesMaster"),
        ],
        rows: vec![],
    }
}

/// Color/Grayscale group: three rows, the mode in use highlighted.
pub(super) fn color_group() -> Group {
    use Item::Labeled as S;
    Group {
        label: "slideNative.colorGrayscale",
        large: vec![],
        rows: vec![
            vec![S("view.color")],
            vec![S("view.grayscale")],
            vec![S("view.blackWhite")],
        ],
    }
}

/// Window group: New Window, Arrange All, Cascade, Switch Windows.
pub(super) fn window_group() -> Group {
    use Item::{Labeled as S, Large as L};
    Group {
        label: "slideNative.window",
        large: vec![L("win.new"), L("win.switch")],
        rows: vec![vec![S("win.arrange")], vec![S("win.cascade")]],
    }
}

/// A window operation the host carries out (View > Window).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRequest {
    /// Another window on the same presentation.
    NewWindow,
    /// Tile the BlueSlide windows side by side.
    Arrange,
    /// Cascade them.
    Cascade,
    /// List them: the host answers with [`SlideWindow::show_windows`].
    List,
    /// Bring window `n` of the last list to the front.
    Activate(usize),
}

/// The Handout Master / Notes Master contextual tab.
pub(super) fn page_master_tab(kind: PageKind) -> Tab {
    use Item::{Labeled as S, Large as L};
    // Page Setup as the reference: orientation, Slide Size, then slides per page.
    let mut setup = vec![L("pm.orientation"), L("design.slideSize")];
    if kind == PageKind::Handout {
        setup.push(L("pm.perPage"));
    }
    let rows = if kind == PageKind::Handout {
        vec![
            vec![S("pm.header"), S("pm.date")],
            vec![S("pm.footer"), S("pm.number")],
        ]
    } else {
        vec![
            vec![S("pm.header"), S("pm.slideImage")],
            vec![S("pm.date"), S("pm.body")],
            vec![S("pm.footer"), S("pm.number")],
        ]
    };
    Tab {
        label: if kind == PageKind::Handout {
            "slideNative.handoutMasterTab"
        } else {
            "slideNative.notesMasterTab"
        },
        groups: vec![
            Group {
                label: "slideNative.pageSetup",
                large: setup,
                rows: vec![],
            },
            Group {
                label: "slideNative.placeholders",
                large: vec![],
                rows,
            },
            // Edit Theme and Background (reference), over the Design tab's commands.
            Group {
                label: "slideNative.editTheme",
                large: vec![L("pm.themes")],
                rows: vec![],
            },
            Group {
                label: "slideNative.bgGroup",
                large: vec![],
                rows: vec![
                    vec![S("design.colors"), S("design.background")],
                    vec![S("design.fonts"), S("pm.hideBg")],
                    vec![S("design.effects")],
                ],
            },
            Group {
                label: "slideNative.close",
                large: vec![L("view.closePageMaster")],
                rows: vec![],
            },
        ],
    }
}

pub(super) fn install_strings(s: &mut crate::strings::Strings) {
    for (id, key) in [
        ("view.handoutMaster", "handoutMaster"),
        ("view.notesMaster", "notesMaster"),
        ("view.closePageMaster", "closeMaster"),
        ("view.ruler", "ruler"),
        ("view.color", "colorMode"),
        ("view.grayscale", "grayscale"),
        ("view.blackWhite", "blackWhite"),
        ("pm.orientation", "orientation"),
        ("pm.perPage", "slidesPerPage"),
        ("pm.header", "phHeader"),
        ("pm.date", "phDate"),
        ("pm.footer", "phFooter"),
        ("pm.number", "phNumber"),
        ("pm.slideImage", "phSlideImage"),
        ("pm.body", "phBody"),
        ("win.new", "newWindow"),
        ("win.switch", "switchWindows"),
        ("win.arrange", "arrangeAll"),
        ("win.cascade", "cascade"),
        ("pm.themes", "themes"),
        ("pm.hideBg", "hideBgGraphics"),
    ] {
        let v = s.get(&format!("slideNative.{key}")).to_owned();
        s.set(&format!("commands.{id}"), &v);
    }
}

pub(super) fn install_icons(icons: &mut crate::icons::Icons) {
    let white = Rgba(255, 255, 255, 255);
    let blue = Rgba(0x44, 0x80, 0xc0, 255);
    let grey = Rgba(0x80, 0x80, 0x80, 255);
    // Handout Master: a page with three slide frames; Notes Master: a page with a slide
    // and lines (own drawings).
    icons.add_solid(
        "view.handoutMaster",
        &[
            ("M5 2h14v20h-14Z", Some(white)),
            ("M5 2h14v20h-14Z", None),
            ("M7 4h10v4h-10ZM7 10h10v4h-10ZM7 16h10v4h-10Z", Some(blue)),
        ],
    );
    icons.add_solid(
        "view.notesMaster",
        &[
            ("M5 2h14v20h-14Z", Some(white)),
            ("M5 2h14v20h-14Z", None),
            ("M7 4h10v6h-10Z", Some(blue)),
            ("M7 13h10M7 16h10M7 19h6", None),
        ],
    );
    icons.alias("view.closePageMaster", "view.closeMaster");
    // Themes (Edit Theme): a page with a large and a small letter shape over a colour strip
    // (own drawing).
    icons.add_solid(
        "pm.themes",
        &[
            ("M3 2h18v20h-18Z", Some(white)),
            ("M3 2h18v20h-18Z", None),
            (
                "M6 14l3-9h1.6l3 9h-1.8l-0.7-2.2h-2.6l-0.7 2.2ZM8.8 10.4h1.8l-0.9-2.9Z",
                Some(Rgba(0x40, 0x40, 0x40, 255)),
            ),
            ("M5 17h3v3h-3Z", Some(blue)),
            ("M8 17h3v3h-3Z", Some(Rgba(0xed, 0x7d, 0x31, 255))),
            ("M11 17h3v3h-3Z", Some(Rgba(0xa5, 0xa5, 0xa5, 255))),
            ("M14 17h3v3h-3Z", Some(Rgba(0xff, 0xc0, 0x00, 255))),
        ],
    );
    // Window group: a new window, tiled / cascaded windows, a stack to switch (own drawings).
    let frame = |x: f32, y: f32, w: f32, h: f32| format!("M{x} {y}h{w}v{h}h-{w}Z");
    let a = frame(3.0, 5.0, 14.0, 12.0);
    let b = frame(7.0, 9.0, 14.0, 12.0);
    icons.add_solid(
        "win.new",
        &[
            (a.as_str(), Some(white)),
            (a.as_str(), None),
            (b.as_str(), Some(white)),
            (b.as_str(), None),
            ("M7 9h14v2h-14Z", Some(blue)),
        ],
    );
    icons.add_solid(
        "win.arrange",
        &[
            ("M2 4h9v16h-9ZM13 4h9v16h-9Z", Some(white)),
            ("M2 4h9v16h-9ZM13 4h9v16h-9Z", None),
            ("M2 4h9v2h-9ZM13 4h9v2h-9Z", Some(blue)),
        ],
    );
    icons.add_solid(
        "win.cascade",
        &[
            (
                "M2 2h12v10h-12ZM6 6h12v10h-12ZM10 10h12v10h-12Z",
                Some(white),
            ),
            ("M2 2h12v10h-12ZM6 6h12v10h-12ZM10 10h12v10h-12Z", None),
        ],
    );
    icons.add_solid(
        "win.switch",
        &[
            ("M4 3h14v11h-14Z", Some(white)),
            ("M4 3h14v11h-14Z", None),
            ("M7 8h14v11h-14Z", Some(white)),
            ("M7 8h14v11h-14Z", None),
            ("M7 8h14v2h-14Z", Some(blue)),
        ],
    );
    // Color: three coloured squares; Grayscale / Black and White: grey and black ones.
    icons.add_solid(
        "view.color",
        &[
            ("M3 3h9v9h-9Z", Some(Rgba(0xd2, 0x4a, 0x3c, 255))),
            ("M12 3h9v9h-9Z", Some(Rgba(0x3f, 0xa3, 0x4d, 255))),
            ("M3 12h9v9h-9Z", Some(blue)),
            ("M12 12h9v9h-9Z", Some(Rgba(0xf5, 0xb8, 0x2e, 255))),
        ],
    );
    icons.add_solid(
        "view.grayscale",
        &[
            ("M3 3h9v9h-9Z", Some(Rgba(0x40, 0x40, 0x40, 255))),
            ("M12 3h9v9h-9Z", Some(grey)),
            ("M3 12h9v9h-9Z", Some(Rgba(0xb0, 0xb0, 0xb0, 255))),
            ("M12 12h9v9h-9Z", Some(Rgba(0xd8, 0xd8, 0xd8, 255))),
        ],
    );
    icons.add_solid(
        "view.blackWhite",
        &[
            ("M3 3h18v18h-18Z", Some(white)),
            ("M3 3h18v18h-18Z", None),
            ("M3 3h9v9h-9ZM12 12h9v9h-9Z", Some(Rgba(0, 0, 0, 255))),
        ],
    );
    icons.add_solid(
        "pm.orientation",
        &[
            ("M3 6h12v16h-12Z", Some(white)),
            ("M3 6h12v16h-12Z", None),
            ("M9 2h13v10h-13Z", Some(Rgba(0xdd, 0xec, 0xf8, 255))),
            ("M9 2h13v10h-13Z", None),
        ],
    );
    icons.add_solid(
        "pm.perPage",
        &[
            ("M5 2h14v20h-14Z", Some(white)),
            ("M5 2h14v20h-14Z", None),
            (
                "M7 4h4v4h-4ZM13 4h4v4h-4ZM7 10h4v4h-4ZM13 10h4v4h-4ZM7 16h4v4h-4ZM13 16h4v4h-4Z",
                Some(blue),
            ),
        ],
    );
}

impl SlideWindow {
    /// The status bar's text in a master view (the reference names the view where the
    /// slide counter usually is); `None` outside them.
    pub(super) fn status_mode_text(&self) -> Option<String> {
        let (key, def) = match self.page_master {
            Some(PageKind::Handout) => ("slideNative.handoutMaster", "Handout Master"),
            Some(PageKind::Notes) => ("slideNative.notesMaster", "Notes Master"),
            None if self.master_view => ("slideNative.slideMaster", "Slide Master"),
            None => return None,
        };
        Some(self.t(key, def))
    }

    /// The page master tab now shown, if any.
    pub(super) fn page_master_tab(&self) -> Option<Tab> {
        self.page_master.map(page_master_tab)
    }

    /// Run one of this round's View commands; `None` when `id` is not one.
    pub(super) fn views5_action(&mut self, id: &str) -> Option<bool> {
        if let Some((_, i)) = PLACEHOLDERS.iter().find(|(c, _)| *c == id) {
            let on = !self.pm_flags[*i];
            let set = |k: usize| (k == *i).then_some(on);
            let done = self
                .view
                .run_page_master(PageMasterCommand::PageMasterElements {
                    header: set(0),
                    date: set(1),
                    footer: set(2),
                    number: set(3),
                    slide_image: set(4),
                    body: set(5),
                });
            if done {
                self.pm_flags[*i] = on;
            }
            return Some(done);
        }
        Some(match id {
            "view.handoutMaster" | "view.notesMaster" => {
                let kind = if id == "view.handoutMaster" {
                    PageKind::Handout
                } else {
                    PageKind::Notes
                };
                if self.page_master.is_some() || self.master_view {
                    return Some(false);
                }
                let done = self
                    .view
                    .run_page_master(PageMasterCommand::PageMasterView { kind, on: true });
                if done {
                    // "Handout Orientation" / "Notes Page Orientation" (reference).
                    let (key, def) = if kind == PageKind::Handout {
                        ("slideNative.handoutOrientation", "Handout Orientation")
                    } else {
                        ("slideNative.notesOrientation", "Notes Page Orientation")
                    };
                    let label = self.t(key, def);
                    self.strings.set("commands.pm.orientation", &label);
                    self.page_master = Some(kind);
                    self.pm_flags = [true; 6];
                    self.reset_fixed_tabs(true);
                }
                done
            }
            "view.closePageMaster" => {
                let done = self
                    .view
                    .run_page_master(PageMasterCommand::PageMasterView {
                        kind: PageKind::Notes,
                        on: false,
                    });
                if done {
                    self.page_master = None;
                    self.reset_fixed_tabs(false);
                }
                done
            }
            "pm.themes" => {
                self.open_menu(super::MenuKind::Themes, RibbonHit::Command("pm.themes"));
                true
            }
            "win.new" => self.window_req(WindowRequest::NewWindow),
            "win.arrange" => self.window_req(WindowRequest::Arrange),
            "win.cascade" => self.window_req(WindowRequest::Cascade),
            "win.switch" => self.window_req(WindowRequest::List),
            "view.ruler" => {
                self.ruler = !self.ruler;
                true
            }
            "view.color" => self.view.set_color_mode(ColorMode::Color),
            "view.grayscale" => self.view.set_color_mode(ColorMode::Grayscale),
            "view.blackWhite" => self.view.set_color_mode(ColorMode::BlackWhite),
            "pm.orientation" => {
                self.open_pm_menu(
                    "pm.orientation",
                    vec![
                        (
                            self.t("slideNative.portrait", "Portrait"),
                            "pm:portrait".into(),
                        ),
                        (
                            self.t("slideNative.landscape", "Landscape"),
                            "pm:landscape".into(),
                        ),
                    ],
                );
                true
            }
            "pm.perPage" => {
                let label = self.t("slideNative.perPageN", "{{n}} Slides");
                let items = [1u8, 2, 3, 4, 6, 9]
                    .iter()
                    .map(|n| (label.replace("{{n}}", &n.to_string()), format!("pm:n{n}")))
                    .collect();
                self.open_pm_menu("pm.perPage", items);
                true
            }
            _ => return None,
        })
    }

    fn open_pm_menu(&mut self, anchor: &'static str, items: Vec<(String, String)>) {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command(anchor)) else {
            return;
        };
        let items = items
            .into_iter()
            .map(|(label, c)| MenuItem {
                label,
                choice: Choice::Command(c),
                checked: false,
                dim: false,
            })
            .collect();
        self.menu_kind = super::MenuKind::Duration;
        self.menu = Some(Menu::open(items, r, self.size, self.scale));
    }

    fn window_req(&mut self, r: WindowRequest) -> bool {
        self.window_request = Some(r);
        true
    }

    /// A window operation for the host (taken once).
    pub fn take_window_request(&mut self) -> Option<WindowRequest> {
        self.window_request.take()
    }

    /// The host's answer to [`WindowRequest::List`]: the open windows' titles (this one
    /// `current`) shown as Switch Windows' menu.
    pub fn show_windows(&mut self, titles: &[String], current: usize) {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command("win.switch")) else {
            return;
        };
        let items = titles
            .iter()
            .enumerate()
            .map(|(i, t)| MenuItem {
                label: format!("{} {}", i + 1, t),
                choice: Choice::Command(format!("pm:w{i}")),
                checked: i == current,
                dim: false,
            })
            .collect();
        self.menu_kind = super::MenuKind::Duration;
        self.menu = Some(Menu::open(items, r, self.size, self.scale));
    }

    /// Menu choices of the master tabs (`pm:…`).
    pub(super) fn choose_pm(&mut self, v: &str) -> bool {
        if let Some(n) = v.strip_prefix('w').and_then(|n| n.parse().ok()) {
            return self.window_req(WindowRequest::Activate(n));
        }
        if let Some(n) = v.strip_prefix('n').and_then(|n| n.parse().ok()) {
            return self
                .view
                .run_page_master(PageMasterCommand::HandoutSlides { per_page: n });
        }
        self.view
            .run_page_master(PageMasterCommand::PageOrientation {
                portrait: v == "portrait",
            })
    }

    /// Rebuild the fixed tabs after a master view opened (its tab first and active) or
    /// closed (back to the View tab).
    fn reset_fixed_tabs(&mut self, opened: bool) {
        self.context = super::ins::Context::None;
        let tabs = self.fixed_tabs();
        self.base_tabs = tabs.len();
        self.ribbon.set_tabs(tabs);
        self.ribbon
            .set_active_tab(if opened { 0 } else { self.base_tabs - 1 });
        self.view.set_zoom(None);
        self.relayout_pending = true;
    }

    /// Commands shown as on.
    pub(super) fn views5_toggled(&self, on: &mut Vec<&'static str>) {
        match self.page_master {
            Some(PageKind::Handout) => on.push("view.handoutMaster"),
            Some(PageKind::Notes) => on.push("view.notesMaster"),
            None => {}
        }
        on.push(match self.view.color_mode() {
            ColorMode::Color => "view.color",
            ColorMode::Grayscale => "view.grayscale",
            ColorMode::BlackWhite => "view.blackWhite",
        });
    }

    /// Check boxes: Ruler and the master placeholders.
    pub(super) fn paint_views5_checks(&self, p: &mut dyn Painter) {
        let mut checks = vec![("view.ruler", self.ruler, true)];
        for (id, i) in PLACEHOLDERS {
            checks.push((id, self.pm_flags[i], true));
        }
        // Hide Background Graphics: greyed on the handout / notes master (reference).
        checks.push(("pm.hideBg", false, false));
        for (id, on, enabled) in checks {
            let Some(r) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
                continue;
            };
            let b = self.px(14.4);
            let (x, y) = (r[0] + self.px(4.0), (r[1] + (r[3] - b) / 2.0).round());
            p.fill_rect(r[0], r[1], self.px(24.0), r[3], Rgba(0xf3, 0xf3, 0xf3, 255));
            paint_check(p, [x, y, b, b], on, enabled, self.theme.text);
        }
    }

    /// The rulers along the top and left of the editing area, in inches from the slide's
    /// centre as the reference draws them (white over the slide's extent, ticks every
    /// eighth of an inch, numbers each inch).
    pub(super) fn paint_ruler(&self, p: &mut dyn Painter) {
        if !self.ruler
            || self.view.mode() != crate::slide_view::ViewMode::Normal
            || self.view.showing()
        {
            return;
        }
        let c = self.view.canvas_rect();
        let (ox, oy) = self.view.slide_origin();
        let k = self.view.scale();
        let (sw, sh) = self.view.slide_points();
        let t = self.px(17.6);
        let bg = Rgba(0xe6, 0xe6, 0xe6, 255);
        let white = Rgba(255, 255, 255, 255);
        let ink = Rgba(0x60, 0x60, 0x60, 255);
        let style = TextStyle {
            size: self.px(9.6),
            bold: false,
        };
        // Top ruler.
        p.fill_rect(c[0], c[1], c[2], t, bg);
        p.fill_rect(c[0], c[1] + t - 1.0, c[2], 1.0, Rgba(0xc6, 0xc6, 0xc6, 255));
        p.fill_rect(ox.max(c[0]), c[1] + 2.0, (sw * k).min(c[2]), t - 4.0, white);
        // Left ruler.
        p.fill_rect(c[0], c[1] + t, t, c[3] - t, bg);
        p.fill_rect(
            c[0] + t - 1.0,
            c[1] + t,
            1.0,
            c[3] - t,
            Rgba(0xc6, 0xc6, 0xc6, 255),
        );
        p.fill_rect(c[0] + 2.0, oy.max(c[1] + t), t - 4.0, sh * k, white);
        p.push_clip(c[0], c[1], c[2], c[3]);
        let inch = 72.0 * k;
        let (cx, cy) = (ox + sw * k / 2.0, oy + sh * k / 2.0);
        let half_w = (sw / 72.0 / 2.0).ceil() as i32;
        let half_h = (sh / 72.0 / 2.0).ceil() as i32;
        for i in -half_w * 8..=half_w * 8 {
            let x = (cx + i as f32 * inch / 8.0).round();
            if x < ox - 0.5 || x > ox + sw * k + 0.5 {
                continue;
            }
            let len = if i % 8 == 0 {
                0.0
            } else if i % 4 == 0 {
                t * 0.4
            } else {
                t * 0.18
            };
            if i % 8 == 0 {
                let n = (i / 8).abs().to_string();
                let w = crate::slide_pickers::estimate(&n, style);
                p.text(
                    &n,
                    (x - w / 2.0).round(),
                    c[1] + (t - style.size * 1.33) / 2.0,
                    style,
                    ink,
                );
            } else {
                p.fill_rect(x, (c[1] + (t - len) / 2.0).round(), 1.0, len.round(), ink);
            }
        }
        for i in -half_h * 8..=half_h * 8 {
            let y = (cy + i as f32 * inch / 8.0).round();
            if y < oy - 0.5 || y > oy + sh * k + 0.5 || y < c[1] + t {
                continue;
            }
            let len = if i % 4 == 0 { t * 0.4 } else { t * 0.18 };
            if i % 8 == 0 {
                let n = (i / 8).abs().to_string();
                let w = crate::slide_pickers::estimate(&n, style);
                p.text(
                    &n,
                    (c[0] + (t - w) / 2.0).round(),
                    (y - style.size * 0.66).round(),
                    style,
                    ink,
                );
            } else {
                p.fill_rect((c[0] + (t - len) / 2.0).round(), y, len.round(), 1.0, ink);
            }
        }
        p.pop_clip();
    }
}
