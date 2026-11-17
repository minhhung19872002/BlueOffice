//! BlueSlide dialogs of round 17 (N6), laid out on the reference presentation app's dialogs
//! measured at 125 % (`docs/ui/REPORT-slide.md` round 17) with BlueSheet's self-drawn form
//! controls (`sheet_form.rs`): Set Up Show (Slide Show tab), Custom Shows and Define Custom
//! Show, Zoom and Grid and Guides (View tab). Option buttons are the form's check boxes drawn
//! as Windows 11 option buttons (one chosen per group), in BlueSlide's accent; labels carry
//! the reference's access keys; the text is the reference's larger dialog size.
use super::sform::{Event, Form, Kind};
use super::SlideWindow;
use crate::{Input, Painter};
use bo_engine::slide_edit::present::{self, ShowType, SlideRange};
use bo_engine::slide_edit::SlideCommand;
use bo_render::{PathSeg, Rgba};

/// Which dialog is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dlg {
    SetUp,
    CustomShows,
    /// Define Custom Show: a new show, or editing show `id`.
    DefineShow(Option<u32>),
    Zoom,
    Grid,
    /// More Entrance / Emphasis / Exit Effects, More Motion Paths: every effect of a class
    /// (`entr`, `emph`, `exit`, `path`) by group; with `add`, the choice is added.
    MoreEffects(&'static str, bool),
}

/// The catalog's effects by class and group (heading key, engine ids), our own grouping in
/// the reference's Basic / Subtle / Moderate / Exciting manner.
const MORE_GROUPS: &[(&str, &str, &[&str])] = &[
    (
        "entr",
        "fxBasic",
        &[
            "appear",
            "blinds",
            "box",
            "checkerboard",
            "circle",
            "diamond",
            "dissolve",
            "flyIn",
            "peek",
            "plus",
            "randomBars",
            "split",
            "strips",
            "wedge",
            "wheel",
            "wipe",
        ],
    ),
    ("entr", "fxSubtle", &["expand", "fade", "swivel", "zoom"]),
    ("entr", "fxModerate", &["float", "growTurn", "crawl"]),
    ("entr", "fxExciting", &["bounce", "spinner"]),
    (
        "emph",
        "fxBasic",
        &[
            "fillColor",
            "fontColor",
            "growShrink",
            "lineColor",
            "spin",
            "transparency",
        ],
    ),
    (
        "emph",
        "fxSubtle",
        &[
            "boldFlash",
            "brushColor",
            "complementary",
            "darken",
            "desaturate",
            "lighten",
            "objectColor",
            "pulse",
            "underline",
        ],
    ),
    (
        "emph",
        "fxModerate",
        &["colorPulse", "teeter", "boldReveal"],
    ),
    ("emph", "fxExciting", &["blink", "waveText"]),
    (
        "path",
        "fxBasic",
        &["circlePath", "diamondPath", "squarePath", "triangle"],
    ),
    (
        "path",
        "fxLinesCurves",
        &[
            "arcDown",
            "arcUp",
            "lineDown",
            "lineLeft",
            "lineRight",
            "lineUp",
            "turnDown",
            "wave",
        ],
    ),
    ("path", "fxSpecial", &["loop"]),
];

/// An option-button group: the options' control ids, top-left of the first (reference px),
/// their labels, the chosen one, the row pitch (reference px), enabled options.
#[derive(Debug, Clone)]
pub(super) struct Radios {
    pub(super) ids: &'static [&'static str],
    pub(super) at: [f32; 2],
    pub(super) labels: Vec<String>,
    pub(super) chosen: usize,
    pub(super) pitch: f32,
    pub(super) enabled: Vec<bool>,
}

/// The open dialog.
pub(super) struct Open {
    pub(super) form: Form,
    pub(super) kind: Dlg,
    pub(super) radios: Vec<Radios>,
    /// Custom shows listed (ids), Define Custom Show's slides in the show (0-based).
    pub(super) ids: Vec<u32>,
    pub(super) chosen_slides: Vec<usize>,
    /// More … Effects: the effect of each list row (`None` on group headings).
    pub(super) fx: Vec<Option<&'static str>>,
}

/// Access keys of the dialogs' labels (control, letter index), as the reference underlines
/// them in English: Presented, Browsed, kiosk, Loop, narration, Show without animation, Pen
/// color, Laser pointer, All, From, To, Custom show, Manually, Using timings.
const ACCESS: &[(&str, usize, char)] = &[
    ("t_speaker", 0, 'P'),
    ("t_browse", 0, 'B'),
    ("t_kiosk", 13, 'k'),
    ("loop", 0, 'L'),
    ("nonarr", 13, 'n'),
    ("noanim", 0, 'S'),
    ("penL", 1, 'e'),
    ("laserL", 4, 'r'),
    ("s_all", 0, 'A'),
    ("s_from", 0, 'F'),
    ("toL", 0, 'T'),
    ("s_custom", 0, 'C'),
    ("a_manual", 0, 'M'),
    ("a_timings", 0, 'U'),
    ("z0", 0, 'F'),
    ("pctL", 0, 'P'),
    ("snap", 0, 'S'),
    ("grid", 0, 'D'),
    ("guides", 1, 'i'),
];

/// Zoom levels of the Zoom dialog (0 = fit).
const ZOOMS: [u32; 7] = [0, 400, 200, 100, 66, 50, 33];

/// Grid spacings offered (points; reference 1/12" … 2").
const SPACINGS: [(f64, &str); 6] = [
    (6.0, "1/12\""),
    (9.0, "1/8\""),
    (18.0, "1/4\""),
    (36.0, "1/2\""),
    (72.0, "1\""),
    (144.0, "2\""),
];

/// Colours offered by the pen colour buttons: ten hues in six rows of tints, then ten
/// standard colours (70, as the form's palette expects).
fn palette() -> Vec<String> {
    let base = [
        0xFF_FFFF, 0x00_0000, 0xE7_E6E6, 0x44_546A, 0x44_72C4, 0xED_7D31, 0xA5_A5A5, 0xFF_C000,
        0x5B_9BD5, 0x70_AD47,
    ];
    let mut out = Vec::with_capacity(70);
    for t in [0.0f32, 0.8, 0.6, 0.4, -0.25, -0.5] {
        for b in base {
            let c = crate::slide_pickers::rgb(b);
            out.push(crate::slide_pickers::hex(if t == 0.0 {
                c
            } else {
                crate::slide_pickers::tint(c, t)
            }));
        }
    }
    for s in [
        0xC0_0000, 0xFF_0000, 0xFF_C000, 0xFF_FF00, 0x92_D050, 0x00_B050, 0x00_B0F0, 0x00_70C0,
        0x00_2060, 0x70_30A0,
    ] {
        out.push(crate::slide_pickers::hex(crate::slide_pickers::rgb(s)));
    }
    out
}

fn edit(v: &str) -> Kind {
    Kind::Edit {
        value: v.chars().collect(),
        caret: v.chars().count(),
    }
}

impl SlideWindow {
    fn l8(&self, key: &str, fallback: &str) -> String {
        self.t(&format!("slideNative.{key}"), fallback)
    }

    /// Open one of these dialogs.
    pub(super) fn open_form8(&mut self, kind: Dlg) -> bool {
        let ok = self.l8("ok", "OK");
        let cancel = self.l8("cancel", "Cancel");
        let mut radios = Vec::new();
        let mut ids = Vec::new();
        let mut chosen_slides = Vec::new();
        let mut fx = Vec::new();
        let form = match kind {
            Dlg::MoreEffects(class, add) => {
                let (title_key, title) = match (class, add) {
                    ("emph", false) => ("changeEmphasis", "Change Emphasis Effect"),
                    ("emph", true) => ("addEmphasis", "Add Emphasis Effect"),
                    ("exit", false) => ("changeExit", "Change Exit Effect"),
                    ("exit", true) => ("addExit", "Add Exit Effect"),
                    ("path", false) => ("changePath", "Change Motion Path"),
                    ("path", true) => ("addPath", "Add Motion Path"),
                    (_, false) => ("changeEntrance", "Change Entrance Effect"),
                    (_, true) => ("addEntrance", "Add Entrance Effect"),
                };
                let mut f = Form::new(&self.l8(title_key, title), (420.0, 600.0), Vec::new());
                let groups = if class == "exit" { "entr" } else { class };
                let mut items = Vec::new();
                for (c, head, ids) in MORE_GROUPS {
                    if *c != groups {
                        continue;
                    }
                    items.push(self.l8(head, head));
                    fx.push(None);
                    for id in *ids {
                        items.push(format!("      {}", self.t(&format!("slideFx.{id}"), id)));
                        fx.push(Some(*id));
                    }
                }
                f.add(
                    "list",
                    None,
                    [16.0, 50.0, 388.0, 482.0],
                    Kind::List {
                        items,
                        sel: Some(1),
                        top: 0,
                    },
                );
                f.add(
                    "ok",
                    None,
                    [178.0, 550.0, 106.0, 33.0],
                    Kind::Button {
                        label: ok,
                        default: true,
                    },
                );
                f.add(
                    "cancel",
                    None,
                    [298.0, 550.0, 106.0, 33.0],
                    Kind::Button {
                        label: cancel,
                        default: false,
                    },
                );
                f
            }
            Dlg::SetUp => {
                let setup = self
                    .view
                    .engine()
                    .presentation()
                    .map(present::show_setup)
                    .unwrap_or_default();
                let shows = self
                    .view
                    .engine()
                    .presentation()
                    .map(present::custom_shows)
                    .unwrap_or_default();
                let n = self.view.slide_count() as i32;
                let has_show_pr = self.view.engine().presentation().is_some_and(|p| {
                    p.part_data("/ppt/presProps.xml")
                        .is_some_and(|d| d.windows(7).any(|w| w == b"showPr " || w == b"showPr>"))
                });
                let mut f = Form::new(
                    &self.l8("setUpShowTitle", "Set Up Show"),
                    (702.0, 587.0),
                    Vec::new(),
                );
                f.add(
                    "g_type",
                    None,
                    [16.0, 47.0, 327.0, 138.0],
                    Kind::Group(self.l8("showType", "Show type")),
                );
                radios.push(Radios {
                    ids: &["t_speaker", "t_browse", "t_kiosk"],
                    at: [30.0, 87.0],
                    labels: vec![
                        self.l8("typeSpeaker", "Presented by a speaker (full screen)"),
                        self.l8("typeBrowse", "Browsed by an individual (window)"),
                        self.l8("typeKiosk", "Browsed at a kiosk (full screen)"),
                    ],
                    chosen: match setup.show_type {
                        ShowType::Speaker => 0,
                        ShowType::Browse => 1,
                        ShowType::Kiosk => 2,
                    },
                    pitch: 31.0,
                    enabled: vec![true; 3],
                });
                f.add(
                    "g_opts",
                    None,
                    [16.0, 192.0, 327.0, 242.0],
                    Kind::Group(self.l8("showOptions", "Show options")),
                );
                let checks = [
                    (
                        "loop",
                        "loopShow",
                        "Loop continuously until 'Esc'",
                        setup.looped,
                    ),
                    (
                        "nonarr",
                        "noNarration",
                        "Show without narration",
                        // A deck without show settings plays its narrations on the
                        // reference (the box is clear); the attribute's default is off.
                        !setup.narration && has_show_pr,
                    ),
                    (
                        "noanim",
                        "noAnimation",
                        "Show without animation",
                        !setup.animation,
                    ),
                    (
                        "nohw",
                        "noHardware",
                        "Disable hardware graphics acceleration",
                        self.no_hw_accel,
                    ),
                ];
                for (i, (id, k, d, on)) in checks.iter().enumerate() {
                    f.add(
                        id,
                        None,
                        [30.0, 228.0 + i as f32 * 31.0, 300.0, 20.0],
                        Kind::Check {
                            label: self.l8(k, d),
                            on: *on,
                        },
                    );
                }
                f.add(
                    "penL",
                    None,
                    [31.0, 357.0, 0.0, 0.0],
                    Kind::Label(self.l8("penColorShow", "Pen color:")),
                );
                f.add(
                    "pen",
                    None,
                    [180.0, 350.0, 56.0, 32.0],
                    Kind::ColorPick {
                        hex: setup.pen_color.clone().unwrap_or_else(|| "FF0000".into()),
                        palette: palette(),
                    },
                );
                f.add(
                    "laserL",
                    None,
                    [31.0, 396.0, 0.0, 0.0],
                    Kind::Label(self.l8("laserColor", "Laser pointer color:")),
                );
                f.add(
                    "laser",
                    None,
                    [180.0, 389.0, 56.0, 32.0],
                    Kind::ColorPick {
                        hex: "FF0000".into(),
                        palette: palette(),
                    },
                );
                f.enable("laser", false);
                f.add(
                    "g_slides",
                    None,
                    [358.0, 47.0, 328.0, 173.0],
                    Kind::Group(self.l8("showSlides", "Show slides")),
                );
                let (from, to, custom) = match setup.range {
                    SlideRange::All => (1, n, 0),
                    SlideRange::Range { from, to } => (from as i32, to as i32, 1),
                    SlideRange::Custom { .. } => (1, n, 2),
                };
                radios.push(Radios {
                    ids: &["s_all", "s_from", "s_custom"],
                    at: [372.0, 87.0],
                    labels: vec![
                        self.l8("allSlides", "All"),
                        self.l8("fromSlideShort", "From:"),
                        self.l8("customShowRadio", "Custom show:"),
                    ],
                    chosen: custom,
                    pitch: 33.0,
                    enabled: vec![true, true, !shows.is_empty()],
                });
                f.add(
                    "from",
                    None,
                    [454.0, 117.0, 84.0, 28.0],
                    Kind::Spin {
                        value: from,
                        min: 1,
                        max: n.max(1),
                    },
                );
                f.add(
                    "toL",
                    None,
                    [553.0, 121.0, 0.0, 0.0],
                    Kind::Label(self.l8("toShort", "To:")),
                );
                f.add(
                    "to",
                    None,
                    [588.0, 117.0, 84.0, 28.0],
                    Kind::Spin {
                        value: to,
                        min: 1,
                        max: n.max(1),
                    },
                );
                let cur = match setup.range {
                    SlideRange::Custom { id } => shows.iter().position(|s| s.id == id).unwrap_or(0),
                    _ => 0,
                };
                ids = shows.iter().map(|s| s.id).collect();
                f.add(
                    "custom",
                    None,
                    [397.0, 181.0, 275.0, 27.0],
                    Kind::Combo {
                        items: shows.iter().map(|s| s.name.clone()).collect(),
                        sel: cur,
                    },
                );
                f.add(
                    "g_adv",
                    None,
                    [358.0, 228.0, 328.0, 107.0],
                    Kind::Group(self.l8("advanceSlides", "Advance slides")),
                );
                radios.push(Radios {
                    ids: &["a_manual", "a_timings"],
                    at: [372.0, 267.0],
                    labels: vec![
                        self.l8("manually", "Manually"),
                        self.l8("usingTimings", "Using timings, if present"),
                    ],
                    chosen: usize::from(setup.use_timings),
                    pitch: 31.0,
                    enabled: vec![true; 2],
                });
                f.add(
                    "g_mon",
                    None,
                    [358.0, 343.0, 328.0, 184.0],
                    Kind::Group(self.l8("multiMonitors", "Multiple monitors")),
                );
                f.add(
                    "monL",
                    None,
                    [373.0, 376.0, 0.0, 0.0],
                    Kind::Label(self.l8("showMonitor", "Slide show monitor:")),
                );
                f.add(
                    "monitor",
                    None,
                    [372.0, 400.0, 300.0, 27.0],
                    Kind::Combo {
                        items: vec![
                            self.l8("automatic", "Automatic"),
                            self.l8("primaryMonitor", "Primary Monitor"),
                        ],
                        sel: 0,
                    },
                );
                f.add(
                    "resL",
                    None,
                    [373.0, 433.0, 0.0, 0.0],
                    Kind::Label(self.l8("resolution", "Resolution:")),
                );
                f.add(
                    "res",
                    None,
                    [372.0, 457.0, 300.0, 27.0],
                    Kind::Combo {
                        items: vec![self.l8("currentResolution", "Use Current Resolution")],
                        sel: 0,
                    },
                );
                f.enable("resL", false);
                f.enable("res", false);
                f.add(
                    "presenter",
                    None,
                    [372.0, 491.0, 300.0, 20.0],
                    Kind::Check {
                        label: self.l8("usePresenter", "Use Presenter View"),
                        on: self.use_presenter,
                    },
                );
                f.add(
                    "ok",
                    None,
                    [457.0, 534.0, 106.0, 33.0],
                    Kind::Button {
                        label: ok,
                        default: true,
                    },
                );
                f.add(
                    "cancel",
                    None,
                    [578.0, 534.0, 107.0, 33.0],
                    Kind::Button {
                        label: cancel,
                        default: false,
                    },
                );
                f
            }
            Dlg::CustomShows => {
                let shows = self
                    .view
                    .engine()
                    .presentation()
                    .map(present::custom_shows)
                    .unwrap_or_default();
                ids = shows.iter().map(|s| s.id).collect();
                let mut f = Form::new(
                    &self.l8("customShowsTitle", "Custom Shows"),
                    (500.0, 260.0),
                    Vec::new(),
                );
                f.add(
                    "list",
                    None,
                    [16.0, 47.0, 345.0, 154.0],
                    Kind::List {
                        items: shows.iter().map(|s| s.name.clone()).collect(),
                        sel: (!shows.is_empty()).then_some(0),
                        top: 0,
                    },
                );
                for (i, (id, k, d)) in [
                    ("new", "csNew", "New..."),
                    ("edit", "csEdit", "Edit..."),
                    ("remove", "csRemove", "Remove"),
                    ("copy", "csCopy", "Copy"),
                    ("close", "csClose", "Close"),
                ]
                .iter()
                .enumerate()
                {
                    f.add(
                        id,
                        None,
                        [376.0, 47.0 + i as f32 * 40.0, 107.0, 34.0],
                        Kind::Button {
                            label: self.l8(k, d),
                            default: i == 0,
                        },
                    );
                }
                f.add(
                    "show",
                    None,
                    [255.0, 207.0, 106.0, 34.0],
                    Kind::Button {
                        label: self.l8("csShow", "Show"),
                        default: false,
                    },
                );
                for id in ["edit", "remove", "copy", "show"] {
                    f.enable(id, !shows.is_empty());
                }
                f
            }
            Dlg::DefineShow(id) => {
                let shows = self
                    .view
                    .engine()
                    .presentation()
                    .map(present::custom_shows)
                    .unwrap_or_default();
                let existing = id.and_then(|id| shows.iter().find(|s| s.id == id));
                let name = existing.map_or_else(
                    || {
                        format!(
                            "{} {}",
                            self.l8("customShowBase", "Custom Show"),
                            shows.len() + 1
                        )
                    },
                    |s| s.name.clone(),
                );
                chosen_slides = existing.map(|s| s.slides.clone()).unwrap_or_default();
                let mut f = Form::new(
                    &self.l8("defineCustomShow", "Define Custom Show"),
                    (760.0, 480.0),
                    Vec::new(),
                );
                f.add(
                    "nameL",
                    None,
                    [16.0, 56.0, 0.0, 0.0],
                    Kind::Label(self.l8("customShowNameShort", "Slide show name:")),
                );
                f.add("name", None, [160.0, 52.0, 584.0, 27.0], edit(&name));
                f.add(
                    "leftL",
                    None,
                    [16.0, 96.0, 0.0, 0.0],
                    Kind::Label(self.l8("slidesInPres", "Slides in presentation:")),
                );
                f.add(
                    "rightL",
                    None,
                    [430.0, 96.0, 0.0, 0.0],
                    Kind::Label(self.l8("slidesInShow", "Slides in custom show:")),
                );
                f.add(
                    "left",
                    None,
                    [16.0, 120.0, 300.0, 290.0],
                    Kind::List {
                        items: self.slide_titles8(),
                        sel: Some(0),
                        top: 0,
                    },
                );
                f.add(
                    "addB",
                    None,
                    [326.0, 220.0, 94.0, 32.0],
                    Kind::Button {
                        label: self.l8("csAdd", "Add >>"),
                        default: false,
                    },
                );
                f.add(
                    "right",
                    None,
                    [430.0, 120.0, 270.0, 290.0],
                    Kind::List {
                        items: Vec::new(),
                        sel: None,
                        top: 0,
                    },
                );
                f.add(
                    "up",
                    None,
                    [708.0, 200.0, 36.0, 32.0],
                    Kind::Button {
                        label: "\u{2191}".into(),
                        default: false,
                    },
                );
                f.add(
                    "remB",
                    None,
                    [708.0, 240.0, 36.0, 32.0],
                    Kind::Button {
                        label: "\u{2715}".into(),
                        default: false,
                    },
                );
                f.add(
                    "down",
                    None,
                    [708.0, 280.0, 36.0, 32.0],
                    Kind::Button {
                        label: "\u{2193}".into(),
                        default: false,
                    },
                );
                f.add(
                    "ok",
                    None,
                    [520.0, 430.0, 106.0, 33.0],
                    Kind::Button {
                        label: ok,
                        default: true,
                    },
                );
                f.add(
                    "cancel",
                    None,
                    [638.0, 430.0, 107.0, 33.0],
                    Kind::Button {
                        label: cancel,
                        default: false,
                    },
                );
                f
            }
            Dlg::Zoom => {
                let mut f = Form::new(&self.l8("zoomTitle", "Zoom"), (280.0, 359.0), Vec::new());
                f.add(
                    "g_zoom",
                    None,
                    [16.0, 50.0, 248.0, 0.0],
                    Kind::Group(self.l8("zoomTo", "Zoom to")),
                );
                let pct = (self.view.zoom() * 100.0).round() as i32;
                radios.push(Radios {
                    ids: &["z0", "z1", "z2", "z3", "z4", "z5", "z6"],
                    at: [28.0, 84.0],
                    labels: ZOOMS
                        .iter()
                        .map(|z| {
                            if *z == 0 {
                                self.l8("zoomFit", "Fit")
                            } else {
                                format!("{z}%")
                            }
                        })
                        .collect(),
                    chosen: if self.view.zoom_is_fit() {
                        0
                    } else {
                        ZOOMS
                            .iter()
                            .position(|z| *z as i32 == pct)
                            .unwrap_or(usize::MAX)
                    },
                    pitch: 31.0,
                    enabled: vec![true; ZOOMS.len()],
                });
                f.add(
                    "pctL",
                    None,
                    [110.0, 84.0, 0.0, 0.0],
                    Kind::Label(self.l8("percent", "Percent:")),
                );
                f.add(
                    "pct",
                    None,
                    [179.0, 79.0, 80.0, 28.0],
                    Kind::Spin {
                        value: pct,
                        min: 10,
                        max: 400,
                    },
                );
                f.add(
                    "ok",
                    None,
                    [35.0, 305.0, 105.0, 35.0],
                    Kind::Button {
                        label: ok,
                        default: true,
                    },
                );
                f.add(
                    "cancel",
                    None,
                    [157.0, 305.0, 105.0, 35.0],
                    Kind::Button {
                        label: cancel,
                        default: false,
                    },
                );
                f
            }
            Dlg::Grid => {
                let mut f = Form::new(
                    &self.l8("gridTitle", "Grid and Guides"),
                    (395.0, 368.0),
                    Vec::new(),
                );
                f.add(
                    "g_snap",
                    None,
                    [16.0, 50.0, 363.0, 0.0],
                    Kind::Group(self.l8("snapTo", "Snap to")),
                );
                f.add(
                    "snap",
                    None,
                    [28.0, 79.0, 300.0, 20.0],
                    Kind::Check {
                        label: self.l8("snapGrid", "Snap objects to grid"),
                        on: self.view.grid_snap().is_some(),
                    },
                );
                f.add(
                    "g_grid",
                    None,
                    [16.0, 115.0, 363.0, 0.0],
                    Kind::Group(self.l8("gridSettings", "Grid settings")),
                );
                f.add(
                    "spL",
                    None,
                    [30.0, 150.0, 0.0, 0.0],
                    Kind::Label(self.l8("spacing", "Spacing:")),
                );
                let sp = self.view.grid_spacing();
                let cur = SPACINGS
                    .iter()
                    .position(|(v, _)| (v - sp).abs() < 0.01)
                    .unwrap_or(0);
                f.add(
                    "spacing",
                    None,
                    [103.0, 145.0, 84.0, 27.0],
                    Kind::Combo {
                        items: SPACINGS.iter().map(|(_, s)| (*s).to_owned()).collect(),
                        sel: cur,
                    },
                );
                f.add(
                    "grid",
                    None,
                    [28.0, 182.0, 300.0, 20.0],
                    Kind::Check {
                        label: self.l8("displayGrid", "Display grid on screen"),
                        on: self.gridlines,
                    },
                );
                f.add(
                    "g_guides",
                    None,
                    [16.0, 216.0, 363.0, 0.0],
                    Kind::Group(self.l8("guideSettings", "Guide settings")),
                );
                f.add(
                    "guides",
                    None,
                    [28.0, 249.0, 340.0, 20.0],
                    Kind::Check {
                        label: self.l8("displayGuides", "Display drawing guides on screen"),
                        on: self.guides,
                    },
                );
                f.add(
                    "smart",
                    None,
                    [28.0, 280.0, 350.0, 20.0],
                    Kind::Check {
                        label: self.l8(
                            "smartGuides",
                            "Display smart guides when shapes are aligned",
                        ),
                        on: true,
                    },
                );
                f.enable("smart", false);
                f.add(
                    "ok",
                    None,
                    [150.0, 316.0, 106.0, 32.0],
                    Kind::Button {
                        label: ok,
                        default: true,
                    },
                );
                f.add(
                    "cancel",
                    None,
                    [272.0, 316.0, 106.0, 32.0],
                    Kind::Button {
                        label: cancel,
                        default: false,
                    },
                );
                f
            }
        };
        let mut form = form;
        // Option buttons: check boxes drawn as Windows 11 option buttons.
        let mut all: Vec<&'static str> = Vec::new();
        for g in &radios {
            for (k, (id, label)) in g.ids.iter().zip(&g.labels).enumerate() {
                let w = if *id == "s_from" { 76.0 } else { 300.0 };
                form.add(
                    id,
                    None,
                    [g.at[0], g.at[1] + k as f32 * g.pitch, w, 20.0],
                    Kind::Check {
                        label: label.clone(),
                        on: k == g.chosen,
                    },
                );
                form.enable(id, g.enabled.get(k).copied().unwrap_or(true));
            }
            all.extend(g.ids.iter().copied());
        }
        form.set_radios(&all);
        // BlueSlide's accent (#C2185B) for checked boxes and the chosen option buttons, the
        // reference's larger dialog text and its access keys.
        form.set_accent(Rgba(0xc2, 0x18, 0x5b, 255), Rgba(0x9c, 0x13, 0x49, 255));
        form.set_font_scale(1.1);
        // Only where the label has the reference's letter there (English labels).
        for (id, at, ch) in ACCESS {
            let has = form.get(id).is_some_and(|c| match &c.kind {
                Kind::Check { label, .. } | Kind::Label(label) => {
                    label.chars().nth(*at) == Some(*ch)
                }
                _ => false,
            });
            if has {
                form.set_access(id, *at);
            }
        }
        let mut open = Open {
            form,
            kind,
            radios,
            ids,
            chosen_slides,
            fx,
        };
        if kind == Dlg::SetUp {
            sync_setup(&mut open);
        }
        if let Dlg::DefineShow(_) = kind {
            let titles = self.slide_titles8();
            let items = open
                .chosen_slides
                .iter()
                .filter_map(|i| titles.get(*i).cloned())
                .collect();
            open.form.set_items("right", items, None);
        }
        self.menu = None;
        self.form8 = Some(open);
        self.form8_layout = true;
        true
    }

    /// "1. Title" for every slide (Define Custom Show).
    fn slide_titles8(&self) -> Vec<String> {
        let n = self.view.slide_count();
        (0..n)
            .map(|i| {
                let t = self.view.slide_title(i).unwrap_or_default();
                let t = if t.is_empty() {
                    self.l8("slideN", "Slide")
                } else {
                    t
                };
                format!("{}. {t}", i + 1)
            })
            .collect()
    }

    /// Input while one of these dialogs is open: `Some(redraw)` when the dialog took it.
    pub(super) fn form8_input(&mut self, input: &Input) -> Option<bool> {
        let open = self.form8.as_mut()?;
        let f = &mut open.form;
        let ev = match input {
            Input::Text(s) => f.type_text(s),
            Input::Key(k, m) => f.key(*k, *m),
            Input::Move { x, y } => {
                self.form8_ptr = (*x, *y);
                return Some(f.hover(*x, *y));
            }
            Input::Press { x, y, count, .. } => {
                self.form8_ptr = (*x, *y);
                f.press(*x, *y, *count)
            }
            Input::Wheel(d) => {
                let (x, y) = self.form8_ptr;
                return Some(f.wheel(x, y, *d));
            }
            Input::Resize { .. } | Input::Scale(_) => {
                self.form8_layout = true;
                return None;
            }
            Input::Blink => return Some(true),
            Input::Drag { .. }
            | Input::Release { .. }
            | Input::Context { .. }
            | Input::Preedit(_) => return Some(false),
            _ => return None,
        };
        Some(self.form8_event(&ev))
    }

    pub(super) fn form8_event(&mut self, ev: &Event) -> bool {
        let Some(open) = self.form8.as_mut() else {
            return false;
        };
        let kind = open.kind;
        match ev {
            Event::Changed(id) if open.radios.iter().any(|g| g.ids.contains(id)) => {
                for g in &mut open.radios {
                    if let Some(k) = g.ids.iter().position(|x| x == id) {
                        g.chosen = k;
                        for (j, other) in g.ids.iter().enumerate() {
                            open.form.set_on(other, j == k);
                        }
                    }
                }
                match kind {
                    Dlg::SetUp => sync_setup(open),
                    Dlg::Zoom => {
                        let z = open.radios[0].chosen;
                        if let Some(v) = ZOOMS.get(z).filter(|v| **v > 0) {
                            open.form.set_num("pct", *v as i32);
                        }
                    }
                    _ => {}
                }
                true
            }
            Event::Changed("pct") => {
                open.radios[0].chosen = usize::MAX;
                for id in open.radios[0].ids {
                    open.form.set_on(id, false);
                }
                true
            }
            Event::Cancel => {
                self.form8 = None;
                true
            }
            Event::Clicked(c) | Event::Changed(c) if matches!(kind, Dlg::CustomShows) => {
                let c: &str = c;
                self.custom_shows_click(c)
            }
            Event::Clicked("list") if matches!(kind, Dlg::MoreEffects(..)) => self.form8_ok(),
            Event::Clicked(c) if matches!(kind, Dlg::DefineShow(_)) => {
                let c: &str = c;
                self.define_show_click(c)
            }
            Event::Ok => self.form8_ok(),
            _ => true,
        }
    }

    fn custom_shows_click(&mut self, c: &str) -> bool {
        let Some(open) = self.form8.as_ref() else {
            return false;
        };
        let sel = open.form.sel("list").and_then(|i| open.ids.get(i).copied());
        match c {
            "new" => self.open_form8(Dlg::DefineShow(None)),
            "edit" | "list" if sel.is_some() && c == "edit" => {
                self.open_form8(Dlg::DefineShow(sel))
            }
            "remove" => {
                if let Some(id) = sel {
                    self.view.run(SlideCommand::DeleteCustomShow { id });
                }
                self.open_form8(Dlg::CustomShows)
            }
            "copy" => {
                if let Some(s) = sel.and_then(|id| {
                    self.view
                        .engine()
                        .presentation()
                        .map(present::custom_shows)
                        .and_then(|v| v.into_iter().find(|s| s.id == id))
                }) {
                    let name = format!("{} {}", self.l8("copyOf", "Copy of"), s.name);
                    self.view.run(SlideCommand::SetCustomShow {
                        id: None,
                        name,
                        slides: s.slides,
                    });
                }
                self.open_form8(Dlg::CustomShows)
            }
            "close" => {
                self.form8 = None;
                true
            }
            "show" => {
                self.form8 = None;
                let Some(id) = sel else {
                    return true;
                };
                let mut plan = self.view.show_plan(Some(id));
                plan.presenter = self.use_presenter;
                let from = plan.order.first().copied().unwrap_or(0);
                self.view.start_show_with(from, plan)
            }
            _ => true,
        }
    }

    fn define_show_click(&mut self, c: &str) -> bool {
        let titles = self.slide_titles8();
        let Some(open) = self.form8.as_mut() else {
            return false;
        };
        let right = open.form.sel("right");
        match c {
            "addB" | "left" => {
                if let Some(i) = open.form.sel("left") {
                    open.chosen_slides.push(i);
                }
            }
            "remB" => {
                if let Some(i) = right.filter(|i| *i < open.chosen_slides.len()) {
                    open.chosen_slides.remove(i);
                }
            }
            "up" => {
                if let Some(i) = right.filter(|i| *i > 0 && *i < open.chosen_slides.len()) {
                    open.chosen_slides.swap(i, i - 1);
                    let items = open
                        .chosen_slides
                        .iter()
                        .filter_map(|k| titles.get(*k).cloned())
                        .collect();
                    open.form.set_items("right", items, Some(i - 1));
                    return true;
                }
            }
            "down" => {
                if let Some(i) = right.filter(|i| i + 1 < open.chosen_slides.len()) {
                    open.chosen_slides.swap(i, i + 1);
                    let items = open
                        .chosen_slides
                        .iter()
                        .filter_map(|k| titles.get(*k).cloned())
                        .collect();
                    open.form.set_items("right", items, Some(i + 1));
                    return true;
                }
            }
            _ => return true,
        }
        let items: Vec<String> = open
            .chosen_slides
            .iter()
            .filter_map(|k| titles.get(*k).cloned())
            .collect();
        let sel = (!items.is_empty()).then(|| items.len() - 1);
        open.form.set_items("right", items, sel);
        true
    }

    fn form8_ok(&mut self) -> bool {
        let Some(open) = self.form8.take() else {
            return false;
        };
        let f = &open.form;
        match open.kind {
            Dlg::SetUp => {
                let Some(mut setup) = self.view.engine().presentation().map(present::show_setup)
                else {
                    return true;
                };
                setup.show_type = match open.radios[0].chosen {
                    1 => ShowType::Browse,
                    2 => ShowType::Kiosk,
                    _ => ShowType::Speaker,
                };
                setup.looped = f.on("loop");
                setup.narration = !f.on("nonarr");
                setup.animation = !f.on("noanim");
                let pen = f.color("pen");
                setup.pen_color = (pen.len() == 6).then_some(pen);
                setup.range = match open.radios[1].chosen {
                    1 => {
                        let (a, b) = (f.num("from").max(1) as u32, f.num("to").max(1) as u32);
                        SlideRange::Range {
                            from: a.min(b),
                            to: a.max(b),
                        }
                    }
                    2 => match f.sel("custom").and_then(|i| open.ids.get(i)) {
                        Some(id) => SlideRange::Custom { id: *id },
                        None => SlideRange::All,
                    },
                    _ => SlideRange::All,
                };
                setup.use_timings = open.radios[2].chosen == 1;
                self.use_presenter = f.on("presenter");
                // Kept for the session (the show is drawn by the same device either way).
                self.no_hw_accel = f.on("nohw");
                self.view.run(SlideCommand::SetShowSetup { setup });
            }
            Dlg::DefineShow(id) => {
                let name = f.text("name");
                if !open.chosen_slides.is_empty() {
                    self.view.run(SlideCommand::SetCustomShow {
                        id,
                        name,
                        slides: open.chosen_slides.clone(),
                    });
                }
                return self.open_form8(Dlg::CustomShows);
            }
            Dlg::CustomShows => {}
            Dlg::MoreEffects(class, add) => {
                if let Some(Some(id)) = f.sel("list").and_then(|i| open.fx.get(i)) {
                    self.pick_effect(id, class, add);
                }
            }
            Dlg::Zoom => {
                if open.radios[0].chosen == 0 {
                    self.view.set_zoom(None);
                } else {
                    let pct = f.num("pct").clamp(10, 400);
                    self.view.set_zoom(Some(pct as f32 / 100.0));
                }
            }
            Dlg::Grid => {
                let sp = f
                    .sel("spacing")
                    .and_then(|i| SPACINGS.get(i))
                    .map_or(6.0, |s| s.0);
                self.view.set_grid(f.on("snap"), sp);
                self.gridlines = f.on("grid");
                self.guides = f.on("guides");
            }
        }
        true
    }

    /// Draw the open dialog; the pen / laser colour buttons show a paint bucket over a bar
    /// of their colour, as the reference's.
    pub(super) fn paint_form8(&self, p: &mut dyn Painter) {
        let Some(open) = self.form8.as_ref() else {
            return;
        };
        let f = &open.form;
        f.paint(p, self.size, true, &mut |_, _, _| {});
        for id in ["pen", "laser"] {
            let Some(c) = f.get(id) else {
                continue;
            };
            let enabled = c.enabled;
            let r = f.dev(c.rect);
            let k = f.dev([0.0, 0.0, 1.0, 1.0])[2];
            let bw = (22.0 * k).round();
            let sr = [
                r[0] + (3.0 * k).round(),
                r[1] + (3.0 * k).round(),
                r[2] - bw - (6.0 * k).round(),
                r[3] - (6.0 * k).round(),
            ];
            p.fill_rect(sr[0], sr[1], sr[2], sr[3], Rgba(255, 255, 255, 255));
            let hex = f.color(id);
            let col = u32::from_str_radix(&hex, 16).map_or(Rgba(0xff, 0, 0, 255), |v| {
                Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
            });
            paint_bucket(p, sr, col, enabled);
        }
    }
}

/// A tilted paint bucket over a colour bar (own drawing) in `r`.
fn paint_bucket(p: &mut dyn Painter, r: [f32; 4], col: Rgba, enabled: bool) {
    let ink = if enabled {
        Rgba(0x40, 0x40, 0x40, 255)
    } else {
        Rgba(0xa0, 0xa0, 0xa0, 255)
    };
    let s = (r[3] * 0.72).min(r[2]);
    let (x, y) = (r[0] + (r[2] - s) / 2.0, r[1]);
    let k = s / 24.0;
    let pt = |a: f32, b: f32| (x + a * k, y + b * k);
    let poly = |pts: &[(f32, f32)]| -> Vec<PathSeg> {
        let mut v: Vec<PathSeg> = pts
            .iter()
            .enumerate()
            .map(|(i, &(a, b))| {
                let (px, py) = pt(a, b);
                if i == 0 {
                    PathSeg::M(px, py)
                } else {
                    PathSeg::L(px, py)
                }
            })
            .collect();
        v.push(PathSeg::Z);
        v
    };
    // Bucket body (a diamond tilted to the right), its handle and a drop.
    p.path(
        &poly(&[(4.0, 11.0), (11.0, 4.0), (19.0, 12.0), (12.0, 19.0)]),
        Some(Rgba(255, 255, 255, 255)),
        Some((ink, 1.0)),
    );
    p.path(
        &poly(&[(8.0, 7.0), (6.0, 2.0), (9.0, 1.5), (10.5, 5.5)]),
        None,
        Some((ink, 1.0)),
    );
    p.path(
        &poly(&[(20.0, 13.0), (21.5, 16.0), (20.0, 17.5), (18.5, 16.0)]),
        Some(ink),
        None,
    );
    let bar = (r[3] * 0.2).max(3.0).round();
    p.fill_rect(r[0] + 2.0, r[1] + r[3] - bar, r[2] - 4.0, bar, col);
}

/// Enable the Set Up Show controls that depend on the option buttons (From / To with
/// "From", the custom show box with "Custom show").
fn sync_setup(open: &mut Open) {
    let slides = open.radios.get(1).map_or(0, |r| r.chosen);
    for id in ["from", "to", "toL"] {
        open.form.enable(id, slides == 1);
    }
    open.form.enable("custom", slides == 2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_has_seventy_colours() {
        let p = palette();
        assert_eq!(p.len(), 70);
        assert!(p.iter().all(|c| c.len() == 6));
    }
}
