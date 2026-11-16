//! BlueSlide's Slide Show tab (N6, round 4), groups as on the reference presentation app:
//! Start Slide Show (From Beginning, From Current Slide, Custom Slide Show), Set Up (Set Up
//! Slide Show dialog, Hide Slide, Rehearse Timings, Play Narrations / Use Timings / Show
//! Media Controls check boxes) and Monitors (Use Presenter View).
use super::SlideWindow;
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::{Group, Item, RibbonHit, Tab};
use crate::slide_view::ShowPlan;
use crate::spin_box::paint_check;
use crate::Painter;
use bo_engine::slide_edit::present::{self, SlideRange};
use bo_engine::slide_edit::SlideCommand;

/// Dialogs of the tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ShowDialog {
    SetUp,
    CustomShow,
    KeepTimings,
}

/// Commands of the tab.
pub const COMMANDS: [&str; 9] = [
    "show.record",
    "show.custom",
    "show.setUp",
    "show.rehearse",
    "show.narrations",
    "show.useTimings",
    "show.mediaControls",
    "show.usePresenter",
    "show.presenterView",
];

pub(super) fn show_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "slideEditor.showTab",
        groups: vec![
            Group {
                label: "slideEditor.groupShow",
                large: vec![L("show.fromStart"), L("show.fromCurrent"), L("show.custom")],
                rows: vec![],
            },
            Group {
                label: "slideNative.setUp",
                large: vec![
                    L("show.setUp"),
                    L("show.hideSlide"),
                    L("show.rehearse"),
                    L("show.record"),
                ],
                rows: vec![
                    vec![S("show.narrations")],
                    vec![S("show.useTimings")],
                    vec![S("show.mediaControls")],
                ],
            },
            Group {
                label: "slideNative.monitors",
                large: vec![L("show.presenterView")],
                rows: vec![vec![S("show.usePresenter")]],
            },
        ],
    }
}

/// Labels (`slideNative.*` keys) of the tab's commands.
pub(super) fn install_strings(s: &mut crate::strings::Strings) {
    for (id, key) in [
        ("show.fromStart", "fromBeginning"),
        ("show.fromCurrent", "fromCurrent"),
        ("show.custom", "customShow"),
        ("show.setUp", "setUpShow"),
        ("show.rehearse", "rehearse"),
        ("show.narrations", "playNarrations"),
        ("show.useTimings", "useTimings"),
        ("show.mediaControls", "mediaControls"),
        ("show.usePresenter", "usePresenter"),
        ("show.presenterView", "presenterView"),
        ("show.record", "record"),
    ] {
        let v = s.get(&format!("slideNative.{key}")).to_owned();
        s.set(&format!("commands.{id}"), &v);
    }
}

pub(super) fn install_icons(icons: &mut crate::icons::Icons) {
    use bo_render::Rgba;
    let screen = "M3 4h18v12h-18Z";
    let white = Rgba(255, 255, 255, 255);
    let blue = Rgba(0x44, 0x80, 0xc0, 255);
    icons.add_solid(
        "show.custom",
        &[
            (screen, Some(white)),
            (screen, None),
            ("M6 7h5v3h-5Z", Some(blue)),
            ("M13 7h5v3h-5Z", Some(blue)),
            ("M6 11h5v3h-5Z", Some(blue)),
            ("M12 16v5M8 21h8", None),
        ],
    );
    icons.add_solid(
        "show.setUp",
        &[
            (screen, Some(white)),
            (screen, None),
            ("M6 7h4v6h-4Z", Some(blue)),
            ("M12 8h6M12 11h6", None),
            ("M12 16v5M8 21h8", None),
        ],
    );
    icons.add_solid(
        "show.rehearse",
        &[
            (screen, Some(white)),
            (screen, None),
            ("M16 12a5 5 0 1 0 0.01 0Z", Some(white)),
            ("M16 12a5 5 0 1 0 0.01 0Z", None),
            ("M16 14v3h2.5", None),
        ],
    );
    icons.alias("show.presenterView", "show.presenter");
    // Record: a screen with a red dot (own drawing).
    icons.add_solid(
        "show.record",
        &[
            (screen, Some(white)),
            (screen, None),
            (
                "M14 14a4.5 4.5 0 1 0 9 0a4.5 4.5 0 1 0 -9 0Z",
                Some(Rgba(0xd8, 0x3b, 0x2f, 255)),
            ),
            ("M12 16v5M8 21h4", None),
        ],
    );
}

impl SlideWindow {
    /// Which check boxes are ticked.
    fn show_checks(&self) -> [(&'static str, bool); 4] {
        let setup = self
            .view
            .engine()
            .presentation()
            .map(present::show_setup)
            .unwrap_or_default();
        [
            ("show.narrations", setup.narration),
            ("show.useTimings", setup.use_timings),
            ("show.mediaControls", self.media_controls),
            ("show.usePresenter", self.use_presenter),
        ]
    }

    /// Run a Slide Show tab command; `None` when `id` is not one.
    pub(super) fn show_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "show.fromStart" | "show.fromCurrent" | "show.presenterView" => {
                let mut plan = self.view.show_plan(None);
                plan.presenter = self.use_presenter || id == "show.presenterView";
                let from = if id == "show.fromStart" {
                    plan.order.first().copied().unwrap_or(0)
                } else {
                    self.view.current()
                };
                self.view.start_show_with(from, plan)
            }
            "show.rehearse" => {
                let mut plan = self.view.show_plan(None);
                plan.rehearse = true;
                plan.use_timings = false;
                let from = plan.order.first().copied().unwrap_or(0);
                self.view.start_show_with(from, plan)
            }
            "show.custom" => {
                self.open_custom_menu();
                true
            }
            "show.record" => {
                self.open_record_menu();
                true
            }
            "show.setUp" => self.open_form8(super::forms8::Dlg::SetUp),
            "show.narrations" | "show.useTimings" => {
                let Some(mut setup) = self.view.engine().presentation().map(present::show_setup)
                else {
                    return Some(false);
                };
                if id == "show.narrations" {
                    setup.narration = !setup.narration;
                } else {
                    setup.use_timings = !setup.use_timings;
                }
                self.view.run(SlideCommand::SetShowSetup { setup })
            }
            "show.mediaControls" => {
                self.media_controls = !self.media_controls;
                true
            }
            "show.usePresenter" => {
                self.use_presenter = !self.use_presenter;
                true
            }
            _ => return None,
        })
    }

    /// Record ▾ as on the reference: recording (narration / camera) is not available here
    /// and greyed; Clear removes the slides' automatic timings.
    fn open_record_menu(&mut self) {
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command("show.record")) else {
            return;
        };
        let items = [
            ("recordCurrent", "rec:cur", true),
            ("recordBeginning", "rec:start", true),
            ("clearTimingSlide", "rec:clear1", false),
            ("clearTimingsAll", "rec:clearall", false),
        ]
        .iter()
        .map(|(k, c, dim)| MenuItem {
            label: self.t(&format!("slideNative.{k}"), k),
            choice: Choice::Command((*c).to_owned()),
            checked: false,
            dim: *dim,
        })
        .collect();
        self.menu_kind = super::MenuKind::Duration;
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    fn open_custom_menu(&mut self) {
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command("show.custom")) else {
            return;
        };
        let shows = self
            .view
            .engine()
            .presentation()
            .map(present::custom_shows)
            .unwrap_or_default();
        let mut items: Vec<MenuItem> = shows
            .into_iter()
            .map(|s| MenuItem {
                label: s.name,
                choice: Choice::Command(format!("cshow:{}", s.id)),
                checked: false,
                dim: false,
            })
            .collect();
        items.push(MenuItem {
            label: self.t("slideNative.customShows", "Custom Shows..."),
            choice: Choice::Command("cshow:new".into()),
            checked: false,
            dim: false,
        });
        self.menu_kind = super::MenuKind::Duration;
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    /// Menu choices of the tab.
    pub(super) fn choose_show(&mut self, kind: &str, v: &str) -> Option<bool> {
        if kind == "rec" {
            let slides: Vec<usize> = match v {
                "clear1" => vec![self.view.current()],
                "clearall" => (0..self.view.slide_count()).collect(),
                _ => return Some(false),
            };
            let timings = slides
                .into_iter()
                .map(|slide| present::SlideTiming { slide, ms: None })
                .collect();
            return Some(self.view.run(SlideCommand::SetTimings {
                timings,
                use_timings: None,
            }));
        }
        (kind == "cshow").then(|| {
            if v == "new" {
                return self.open_form8(super::forms8::Dlg::CustomShows);
            }
            let Ok(id) = v.parse::<u32>() else {
                return false;
            };
            let mut plan = self.view.show_plan(Some(id));
            plan.presenter = self.use_presenter;
            let from = plan.order.first().copied().unwrap_or(0);
            self.view.start_show_with(from, plan)
        })
    }

    pub(super) fn open_show_dialog(&mut self, kind: ShowDialog) {
        let l = |w: &Self, k: &str| w.strings.get(&format!("slideNative.{k}")).to_owned();
        let buttons = |w: &Self, ok: &str, cancel: &str| {
            vec![
                (l(w, ok), DialogButton::Ok),
                (l(w, cancel), DialogButton::Cancel),
            ]
        };
        let d = match kind {
            ShowDialog::SetUp => {
                let setup = self
                    .view
                    .engine()
                    .presentation()
                    .map(present::show_setup)
                    .unwrap_or_default();
                let (from, to) = match setup.range {
                    SlideRange::Range { from, to } => (from.to_string(), to.to_string()),
                    _ => (String::new(), String::new()),
                };
                Dialog::new(
                    DialogKind::Comment,
                    l(self, "setUpShowTitle"),
                    vec![(l(self, "fromSlide"), from), (l(self, "toSlide"), to)],
                    buttons(self, "ok", "cancel"),
                    Vec::new(),
                )
                .with_checks(vec![
                    (l(self, "loopShow"), setup.looped),
                    (l(self, "noNarration"), !setup.narration),
                    (l(self, "noAnimation"), !setup.animation),
                    (l(self, "advanceTimings"), setup.use_timings),
                ])
            }
            ShowDialog::CustomShow => Dialog::new(
                DialogKind::Comment,
                l(self, "defineCustomShow"),
                vec![
                    (l(self, "customShowName"), l(self, "customShowDefault")),
                    (l(self, "customShowSlides"), String::new()),
                ],
                buttons(self, "ok", "cancel"),
                Vec::new(),
            ),
            ShowDialog::KeepTimings => Dialog::new(
                DialogKind::WordCount,
                l(self, "rehearse"),
                Vec::new(),
                buttons(self, "yes", "no"),
                vec![l(self, "keepTimings")],
            ),
        };
        self.menu = None;
        self.dialog = Some((d, super::ins::SlideDialog::Show(kind)));
        self.dialog_layout = true;
    }

    /// A Slide Show dialog's button.
    pub(super) fn show_dialog_button(
        &mut self,
        kind: ShowDialog,
        b: DialogButton,
        values: &[String],
    ) {
        self.dialog = None;
        if b != DialogButton::Ok {
            if kind == ShowDialog::KeepTimings {
                self.pending_timings = None;
            }
            return;
        }
        match kind {
            ShowDialog::SetUp => {
                let Some(mut setup) = self.view.engine().presentation().map(present::show_setup)
                else {
                    return;
                };
                let num = |i: usize| values.get(i).and_then(|v| v.trim().parse::<u32>().ok());
                let on = |i: usize| values.get(i).is_some_and(|v| v == "1");
                setup.range = match (num(0), num(1)) {
                    (Some(from), Some(to)) if from >= 1 && to >= from => {
                        SlideRange::Range { from, to }
                    }
                    _ => SlideRange::All,
                };
                setup.looped = on(2);
                setup.narration = !on(3);
                setup.animation = !on(4);
                setup.use_timings = on(5);
                self.view.run(SlideCommand::SetShowSetup { setup });
            }
            ShowDialog::CustomShow => {
                let name = values.first().cloned().unwrap_or_default();
                let n = self.view.slide_count();
                let slides: Vec<usize> = values
                    .get(1)
                    .map(|v| {
                        v.split([',', ' ', ';'])
                            .filter_map(|s| s.trim().parse::<usize>().ok())
                            .filter(|i| (1..=n).contains(i))
                            .map(|i| i - 1)
                            .collect()
                    })
                    .unwrap_or_default();
                let slides = if slides.is_empty() {
                    self.view.selected_slides()
                } else {
                    slides
                };
                self.view.run(SlideCommand::SetCustomShow {
                    id: None,
                    name,
                    slides,
                });
            }
            ShowDialog::KeepTimings => {
                if let Some(timings) = self.pending_timings.take() {
                    self.view.run(SlideCommand::SetTimings {
                        timings,
                        use_timings: Some(true),
                    });
                }
            }
        }
    }

    /// After a rehearsal: ask whether to keep its timings, as the reference does.
    pub(super) fn after_rehearsal(&mut self) -> bool {
        let Some(t) = self.view.take_rehearsal() else {
            return false;
        };
        self.pending_timings = Some(t);
        self.open_show_dialog(ShowDialog::KeepTimings);
        true
    }

    /// Check boxes of the Set Up and Monitors groups, over their slots' icon place.
    pub(super) fn paint_show_checks(&self, p: &mut dyn Painter) {
        let checks = self.show_checks();
        for (id, on) in checks {
            let Some(r) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
                continue;
            };
            let b = self.px(14.4);
            let x = r[0] + self.px(4.0);
            let y = (r[1] + (r[3] - b) / 2.0).round();
            p.fill_rect(
                x - 1.0,
                y - 1.0,
                b + 2.0,
                b + 2.0,
                bo_render::Rgba(0xf3, 0xf3, 0xf3, 255),
            );
            paint_check(p, [x, y, b, b], on, true, self.theme.text);
        }
    }

    /// Start a show with `plan` (automation and tests).
    pub fn start_show_plan(&mut self, from: usize, plan: ShowPlan) -> bool {
        self.view.start_show_with(from, plan)
    }
}
