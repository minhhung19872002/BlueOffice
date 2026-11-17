//! BlueSlide's Review tab beyond comments (round 16), groups as on the reference: Proofing
//! (Spelling, Thesaurus), Accessibility (Check Accessibility ▾), Language (Translate,
//! Language ▾), Comments, Ink (Hide Ink ▾).
//!
//! - Check Accessibility opens a pane listing slides without a title and pictures, charts,
//!   diagrams, media and objects without alternative text (our own checker over
//!   `bo_engine::slide_edit::a11y::slide_shapes`); a row goes to the shape.
//! - Language sets the proofing language of the text (or leaves it unchecked), engine
//!   `TextFormat::Lang`.
//! - Thesaurus opens its pane on the word at the caret (`slide_thespane.rs`, round 18);
//!   Translate has no engine behind it yet and is greyed. Hide Ink is
//!   greyed while the deck holds no ink (as on the reference) and otherwise toggles the ink
//!   out of the slides, thumbnails and the show (round 17).
use super::selpane::{A11yRow, SideHit, SideKind};
use super::{MenuKind, SlideWindow};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use crate::Painter;
use bo_engine::slide_edit::text::TextFormat;
use bo_engine::slide_edit::SlideCommand;
use bo_render::Rgba;

/// Commands of these groups that work (Thesaurus and Translate are greyed; Hide Ink needs
/// ink in the deck).
pub const COMMANDS: [&str; 4] = ["sl.checkA11y", "sl.language", "sl.hideInk", "sl.thesaurus"];

/// Commands shown greyed.
pub const GREYED: [&str; 1] = ["sl.translate"];

/// Proofing languages of the Language menu (BCP 47 tag, `spell.lang.*` key); the last
/// entry leaves the text unchecked.
const LANGUAGES: [&str; 8] = [
    "en-US", "en-GB", "vi-VN", "fr-FR", "de-DE", "zh-CN", "ja-JP", "ko-KR",
];

/// Shape kinds that need alternative text.
const NEEDS_ALT: [&str; 6] = ["picture", "chart", "smartart", "media", "object", "model3d"];

/// Name the commands.
pub(super) fn install_strings(s: &mut crate::strings::Strings) {
    for (id, key) in [
        ("sl.thesaurus", "thesaurus"),
        ("sl.checkA11y", "checkA11y"),
        ("sl.translate", "translate"),
        ("sl.language", "language"),
        ("sl.hideInk", "hideInk"),
    ] {
        let v = s.get(&format!("slideNative.{key}")).to_owned();
        s.set(&format!("commands.{id}"), &v);
    }
}

/// Own filled drawings: Spelling (letters over a blue check), Thesaurus (an open book),
/// Check Accessibility (a page with a blue person badge), Translate (two speech tiles),
/// Language (a globe), Hide Ink (a pen stroke over a page).
pub(super) fn install_icons(icons: &mut crate::icons::Icons) {
    let blue = crate::icons::ICON_BLUE;
    let soft = Rgba(0xb9, 0xd2, 0xee, 255);
    let white = Rgba(0xff, 0xff, 0xff, 255);
    let green = Rgba(0x1e, 0x8e, 0x3e, 255);
    icons.add_solid(
        "review.spellingDialog",
        &[
            ("M2 10l2.5-7h1l2.5 7M3 7.5h4", None),
            (
                "M9.5 3v7h2.5a1.7 1.7 0 0 0 0-3.5h-2.5h2a1.7 1.7 0 0 0 0-3.5Z",
                None,
            ),
            ("M21.5 4a3 3.5 0 1 0 0 5", None),
            ("M4 16l3-3l4 4l9-9l3 3l-12 12Z", Some(blue)),
        ],
    );
    icons.add_solid(
        "sl.thesaurus",
        &[
            ("M2 5c4-1.5 7-1 10 1v15c-3-2-6-2.5-10-1Z", Some(white)),
            ("M22 5c-4-1.5-7-1-10 1v15c3-2 6-2.5 10-1Z", Some(soft)),
            (
                "M2 5c4-1.5 7-1 10 1v15c-3-2-6-2.5-10-1ZM22 5c-4-1.5-7-1-10 1v15c3-2 6-2.5 10-1Z",
                None,
            ),
        ],
    );
    icons.add_solid(
        "sl.checkA11y",
        &[
            ("M4 2h11l4 4v16H4Z", Some(white)),
            ("M4 2h11l4 4v16H4Z", None),
            ("M10 15a5 5 0 1 0 10 0a5 5 0 1 0 -10 0Z", Some(blue)),
            (
                "M13.8 12.3a1.2 1.2 0 1 0 2.4 0a1.2 1.2 0 1 0 -2.4 0Z",
                Some(white),
            ),
            ("M12.5 14.2h5v1h-1.8v3.5h-1.4v-3.5h-1.8Z", Some(white)),
        ],
    );
    icons.add_solid(
        "sl.translate",
        &[
            ("M2 3h12v10H2Z", Some(blue)),
            ("M10 11h12v10H10Z", Some(green)),
            ("M2 3h12v10H2ZM10 11h12v10H10Z", None),
        ],
    );
    icons.add_solid(
        "sl.language",
        &[
            ("M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0Z", Some(soft)),
            ("M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0ZM3 12h18M12 3c-3 3-3 15 0 18M12 3c3 3 3 15 0 18", None),
        ],
    );
    icons.add_solid(
        "sl.hideInk",
        &[
            ("M4 3h16v18H4Z", Some(white)),
            ("M4 3h16v18H4Z", None),
            ("M6 17c3-6 5 2 8-4s4 0 5-2", None),
            ("M3 21L21 3", None),
        ],
    );
}

impl SlideWindow {
    /// Run one of these commands; `None` when `id` is not one of them.
    pub(super) fn review_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "sl.thesaurus" => self.open_thesaurus(),
            "sl.checkA11y" => {
                let items = [
                    ("checkA11y", "rv:check"),
                    ("altText", "rv:alt"),
                    ("selectionPaneMenu", "rv:sel"),
                ]
                .iter()
                .map(|(k, c)| (self.t(&format!("slideNative.{k}"), k), (*c).to_owned()))
                .collect();
                self.review_menu(items, "sl.checkA11y");
                true
            }
            "sl.language" => {
                let mut items: Vec<(String, String)> = LANGUAGES
                    .iter()
                    .map(|l| {
                        (
                            self.t(&format!("spell.lang.{l}"), l),
                            format!("rv:lang:{l}"),
                        )
                    })
                    .collect();
                items.push((
                    self.t("spell.noProof", "Do not check spelling"),
                    "rv:noproof".into(),
                ));
                self.review_menu(items, "sl.language");
                true
            }
            "sl.hideInk" => {
                // As the reference's split button: Hide Ink (checked while hidden), Delete
                // All Ink on Slide, Delete All Ink in Presentation.
                let hidden = self.view.engine().slide_hide_ink();
                let items: Vec<MenuItem> = [
                    ("hideInk", "rv:hideink", hidden),
                    ("deleteInkSlide", "rv:delinkslide", false),
                    ("deleteInkAll", "rv:delinkall", false),
                ]
                .iter()
                .map(|(k, c, on)| MenuItem {
                    label: self.t(&format!("slideNative.{k}"), k),
                    choice: Choice::Command((*c).to_owned()),
                    checked: *on,
                    dim: false,
                })
                .collect();
                let Some(r) = self.ribbon.rect_of(RibbonHit::Command("sl.hideInk")) else {
                    return Some(false);
                };
                self.menu_kind = MenuKind::Duration;
                self.menu = Some(Menu::open(items, r, self.size, self.scale));
                true
            }
            id if GREYED.contains(&id) => false,
            _ => return None,
        })
    }

    fn review_menu(&mut self, items: Vec<(String, String)>, anchor: &'static str) {
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
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, r, self.size, self.scale));
    }

    /// A choice of these menus (`rv:…`).
    pub(super) fn choose_review(&mut self, v: &str) -> bool {
        match v {
            "check" => self.open_a11y(),
            "alt" => self.action("sl.altText"),
            "sel" => self.action("sl.selPane"),
            "noproof" => self.view.format_text(TextFormat::Lang { lang: None }),
            "hideink" => {
                let hide = !self.view.engine().slide_hide_ink();
                self.view.set_hide_ink(hide)
            }
            "delinkslide" => self.view.delete_ink(false),
            "delinkall" => self.view.delete_ink(true),
            _ => match v.strip_prefix("lang:") {
                Some(l) => self.view.format_text(TextFormat::Lang {
                    lang: Some(l.to_owned()),
                }),
                None => false,
            },
        }
    }

    /// Open (or refresh) the Accessibility pane.
    pub(super) fn open_a11y(&mut self) -> bool {
        if self.side.kind != SideKind::Accessibility {
            self.open_side(SideKind::Accessibility);
        }
        self.relayout_pending = true;
        true
    }

    /// The deck's accessibility issues: slides without a title, graphics without
    /// alternative text (slide order, then reading order).
    pub(super) fn a11y_rows(&self) -> Vec<A11yRow> {
        let Some(pres) = self.view.engine().presentation() else {
            return Vec::new();
        };
        let no_title = self.t("slideNative.a11yNoTitle", "Missing slide title");
        let no_alt = self.t("slideNative.a11yNoAlt", "Missing alternative text");
        let slide_word = self.t("slideNative.slideWord", "Slide");
        let mut alt = Vec::new();
        let mut title = Vec::new();
        for i in 0..pres.slides.len() {
            let shapes = bo_engine::slide_edit::a11y::slide_shapes(pres, i);
            let titled = shapes.iter().any(|s| {
                matches!(s.placeholder.as_deref(), Some("title" | "ctrTitle"))
                    && s.text.as_deref().is_some_and(|t| !t.trim().is_empty())
            });
            if !titled {
                title.push(A11yRow {
                    slide: i,
                    id: 0,
                    kind: no_title.clone(),
                    what: format!("{slide_word} {}", i + 1),
                });
            }
            for s in shapes.iter().filter(|s| NEEDS_ALT.contains(&s.kind)) {
                if s.alt.as_deref().is_none_or(|a| a.trim().is_empty()) {
                    alt.push(A11yRow {
                        slide: i,
                        id: s.id,
                        kind: no_alt.clone(),
                        what: format!("{} ({slide_word} {})", s.name, i + 1),
                    });
                }
            }
        }
        alt.extend(title);
        alt
    }

    /// A press on an Accessibility row: go to its slide and select the shape.
    pub(super) fn a11y_goto(&mut self, slide: usize, id: u32) -> bool {
        if slide != self.view.current() {
            self.view.go_to(slide);
        }
        if id != 0 {
            self.view.run(SlideCommand::Select { ids: vec![id] });
        }
        true
    }

    /// The Spelling / Accessibility pane hits handled here.
    pub(super) fn review_side_hit(&mut self, hit: &SideHit) -> Option<bool> {
        Some(match hit {
            SideHit::Goto(slide, id) => self.a11y_goto(*slide, *id),
            SideHit::PaneMenu => {
                let r = self
                    .side
                    .hits
                    .iter()
                    .find(|(_, h)| *h == SideHit::PaneMenu)
                    .map(|(r, _)| *r);
                if let Some(r) = r {
                    let items = vec![MenuItem {
                        label: self.t("slideNative.closePane", "Close"),
                        choice: Choice::Command("rv:closepane".into()),
                        checked: false,
                        dim: false,
                    }];
                    self.menu_kind = MenuKind::Duration;
                    self.menu = Some(Menu::open(items, r, self.size, self.scale));
                }
                true
            }
            SideHit::Speak => {
                let shown = self.spell_shown();
                self.speak = shown.suggestions.get(shown.chosen).cloned();
                self.speak.is_some()
            }
            _ => return None,
        })
    }

    /// The text the host should read aloud (Spelling pane's speaker button).
    pub fn take_speak(&mut self) -> Option<String> {
        self.speak.take()
    }

    /// Review tab: the thin line between Next and Show Comments (reference).
    pub(super) fn paint_review_extras(&self, p: &mut dyn Painter) {
        let (Some(a), Some(b)) = (
            self.ribbon
                .rect_of(RibbonHit::Command("review.nextComment")),
            self.ribbon.rect_of(RibbonHit::Command("review.comments")),
        ) else {
            return;
        };
        let x = ((a[0] + a[2] + b[0]) / 2.0).round();
        let pad = (6.0 * self.scale).round();
        p.fill_rect(
            x,
            b[1] + pad,
            1.0,
            b[3] - 2.0 * pad,
            Rgba(0xc8, 0xc8, 0xc8, 255),
        );
    }
}
