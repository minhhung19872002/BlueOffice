//! BlueSlide's Design, Transitions, Animations and Review tabs (N6, round 2): their commands,
//! labels (from the presentation editor's existing strings), own icons and the theme tiles
//! drawn over the ribbon, laid out like the reference presentation app's tabs.
use crate::icons::Icons;
use crate::ribbon::{Group, Item, Tab};
use crate::strings::Strings;
use crate::Painter;
use bo_render::Rgba;

/// Transitions offered in the ribbon gallery: command id and the engine's transition kind
/// (`bo_pptx::transitions` catalog), in the reference app's gallery order.
pub const TRANSITIONS: [(&str, &str); 9] = [
    ("transition.none", "none"),
    ("transition.fade", "fade"),
    ("transition.push", "push"),
    ("transition.wipe", "wipe"),
    ("transition.split", "split"),
    ("transition.cover", "cover"),
    ("transition.pull", "pull"),
    ("transition.cut", "cut"),
    ("transition.zoom", "zoom"),
];

/// Effects of the animation gallery: command id, engine effect id and class (`entr`, `emph`,
/// `exit`); `None` removes the selection's effects.
pub const EFFECTS: [(&str, Option<(&str, &str)>); 10] = [
    ("anim.none", None),
    ("anim.appear", Some(("appear", "entr"))),
    ("anim.fade", Some(("fade", "entr"))),
    ("anim.flyIn", Some(("flyIn", "entr"))),
    ("anim.wipe", Some(("wipe", "entr"))),
    ("anim.zoom", Some(("zoom", "entr"))),
    ("anim.pulse", Some(("pulse", "emph"))),
    ("anim.growShrink", Some(("growShrink", "emph"))),
    ("anim.fadeOut", Some(("fade", "exit"))),
    ("anim.flyOut", Some(("flyIn", "exit"))),
];

/// Built-in themes of the Design tab (ids of `bo_engine::slide_edit::master::THEMES`).
pub const THEMES: [(&str, &str); 5] = [
    ("design.theme.harbor", "harbor"),
    ("design.theme.lotus", "lotus"),
    ("design.theme.bamboo", "bamboo"),
    ("design.theme.ember", "ember"),
    ("design.theme.slate", "slate"),
];

/// Tiles shown in the ribbon galleries; the rest are in the gallery's "more" menu (the
/// reference shows six at this window width).
pub const GALLERY_TILES: usize = 6;

/// Other commands of these tabs.
const OWN: [&str; 17] = [
    "transition.more",
    "anim.more",
    "design.colors",
    "design.fonts",
    "design.slideSize",
    "design.background",
    "transition.preview",
    "transition.effect",
    "transition.duration",
    "transition.applyAll",
    "anim.preview",
    "anim.effect",
    "anim.pane",
    "review.newComment",
    "review.comments",
    "view.sorter",
    "view.reading",
];

/// Whether `id` is one of the commands of these tabs.
pub fn is_command(id: &str) -> bool {
    OWN.contains(&id)
        || id == "view.normal"
        || TRANSITIONS.iter().any(|(c, _)| *c == id)
        || EFFECTS.iter().any(|(c, _)| *c == id)
        || THEMES.iter().any(|(c, _)| *c == id)
}

/// Commands that open a menu (Slide Size, Effect Options, …).
pub fn is_menu(id: &str) -> bool {
    matches!(
        id,
        "design.colors"
            | "design.fonts"
            | "design.slideSize"
            | "design.background"
            | "transition.effect"
            | "transition.duration"
            | "transition.more"
            | "anim.effect"
            | "anim.more"
    )
}

/// Width (logical px) of gallery tiles, measured on the reference at 125 %: theme tiles
/// 130 device px, transition and effect tiles 124.
pub fn large_width(id: &str) -> Option<f32> {
    if THEMES.iter().any(|(c, _)| *c == id) {
        // Reference pitch 140 px with the ribbon's 2 px gap.
        Some(109.6)
    } else if TRANSITIONS.iter().any(|(c, _)| *c == id) || EFFECTS.iter().any(|(c, _)| *c == id) {
        Some(99.2)
    } else {
        None
    }
}

pub(crate) fn design_tab() -> Tab {
    use Item::Large as L;
    Tab {
        label: "slideNative.design",
        groups: vec![
            Group {
                label: "slideEditor.design.groupThemes",
                large: THEMES.iter().map(|(c, _)| L(c)).collect(),
                rows: vec![],
            },
            Group {
                label: "slideEditor.design.groupVariants",
                large: vec![L("design.colors"), L("design.fonts")],
                rows: vec![],
            },
            Group {
                label: "slideNative.customize",
                large: vec![L("design.slideSize"), L("design.background")],
                rows: vec![],
            },
        ],
    }
}

pub(crate) fn transitions_tab() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    Tab {
        label: "slideEditor.transitionTab",
        groups: vec![
            Group {
                label: "slideEditor.groupPreview",
                large: vec![L("transition.preview")],
                rows: vec![],
            },
            Group {
                label: "slideNative.transitionGallery",
                large: TRANSITIONS
                    .iter()
                    .take(GALLERY_TILES)
                    .map(|(c, _)| L(c))
                    .collect(),
                rows: vec![vec![B("transition.more")]],
            },
            Group {
                label: "slideNative.effectOptions",
                large: vec![L("transition.effect")],
                rows: vec![],
            },
            Group {
                label: "slideNative.timing",
                large: vec![],
                rows: vec![
                    vec![S("transition.duration")],
                    vec![S("transition.applyAll")],
                ],
            },
        ],
    }
}

pub(crate) fn animations_tab() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    Tab {
        label: "slideNative.animations",
        groups: vec![
            Group {
                label: "slideEditor.groupPreview",
                large: vec![L("anim.preview")],
                rows: vec![],
            },
            Group {
                label: "slideNative.animationGallery",
                large: EFFECTS
                    .iter()
                    .take(GALLERY_TILES)
                    .map(|(c, _)| L(c))
                    .collect(),
                rows: vec![vec![B("anim.more")]],
            },
            Group {
                label: "slideNative.effectOptions",
                large: vec![L("anim.effect")],
                rows: vec![],
            },
            Group {
                label: "slideNative.advanced",
                large: vec![],
                rows: vec![vec![S("anim.pane")]],
            },
        ],
    }
}

pub(crate) fn review_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.review",
        groups: vec![Group {
            label: "ribbon.groups.comments",
            large: vec![L("review.newComment")],
            rows: vec![vec![S("review.comments")]],
        }],
    }
}

/// Name the commands of these tabs from the presentation editor's strings.
pub(crate) fn install_strings(s: &mut Strings) {
    let copy = |s: &mut Strings, id: &str, key: &str| {
        let v = s.get(key).to_owned();
        if v != key {
            s.set(&format!("commands.{id}"), &v);
        }
    };
    for (id, kind) in TRANSITIONS {
        copy(s, id, &format!("slideEditor.transition.{kind}"));
    }
    for (id, effect) in EFFECTS {
        match effect {
            None => copy(s, id, "slideEditor.transition.none"),
            Some((e, "exit")) => copy(s, id, &format!("slideEditor.effectOut.{e}")),
            Some((e, _)) => copy(s, id, &format!("slideEditor.effect.{e}")),
        }
    }
    for (id, theme) in THEMES {
        // Theme tiles show a preview, not a label; the name is the tooltip.
        copy(
            s,
            &format!("tip.{id}"),
            &format!("slideEditor.design.theme.{theme}"),
        );
        s.set(&format!("commands.{id}"), "");
    }
    for (id, key) in [
        ("design.slideSize", "slideEditor.design.slideSize"),
        ("design.background", "slideNative.formatBackground"),
        ("design.colors", "slideEditor.smartart.colors"),
        ("design.fonts", "slideNative.fonts"),
        ("transition.preview", "slideEditor.preview"),
        ("anim.preview", "slideEditor.preview"),
        ("transition.effect", "slideNative.effectOptions"),
        ("anim.effect", "slideNative.effectOptions"),
        ("transition.duration", "slideEditor.durationSec"),
        ("transition.applyAll", "slideEditor.applyAll"),
        ("anim.pane", "slideEditor.animPane"),
        ("transition.more", "slideEditor.transitionGroup.subtle"),
        ("anim.more", "slideEditor.addEffect"),
        ("review.newComment", "slideEditor.comments.new"),
        ("review.comments", "slideEditor.comments.title"),
        ("view.normal", "slideEditor.normal"),
        ("view.sorter", "slideEditor.sorter"),
        ("view.reading", "slideEditor.reading"),
    ] {
        copy(s, id, key);
    }
}

const SLIDE_BLUE: Rgba = Rgba(0x5b, 0x8f, 0xd0, 255);
const SLIDE_PALE: Rgba = Rgba(0xc9, 0xdc, 0xf2, 255);
const SLIDE_MID: Rgba = Rgba(0x9b, 0xbc, 0xe4, 255);
const ARROW: Rgba = Rgba(0x2b, 0x57, 0x97, 255);
const ENTR: Rgba = Rgba(0x3f, 0xa3, 0x4d, 255);
const EMPH: Rgba = Rgba(0xe8, 0xb3, 0x1e, 255);
const EXIT: Rgba = Rgba(0xd2, 0x4a, 0x3c, 255);
const NONE: Rgba = Rgba(0xa6, 0xa6, 0xa6, 255);

/// A five-pointed star in the 24-unit icon grid (own drawing).
const STAR: &str =
    "M12 2.5L14.8 8.6L21.4 9.3L16.4 13.8L17.8 20.3L12 17L6.2 20.3L7.6 13.8L2.6 9.3L9.2 8.6Z";

/// Add BlueSlide's own icons (own drawings: slide tiles with a motif, coloured stars).
pub(crate) fn install_icons(icons: &mut Icons) {
    let frame = "M3 5h18v14h-18Z";
    let tile = |icons: &mut Icons, id: &str, extra: &[(&str, Option<Rgba>)]| {
        let mut parts: Vec<(&str, Option<Rgba>)> = vec![(frame, Some(SLIDE_PALE))];
        parts.extend_from_slice(extra);
        parts.push((frame, None));
        icons.add_solid(id, &parts);
    };
    tile(icons, "transition.none", &[]);
    tile(
        icons,
        "transition.fade",
        &[
            ("M7 8h11v9h-11Z", Some(SLIDE_MID)),
            ("M9 10h7v5h-7Z", Some(SLIDE_BLUE)),
        ],
    );
    tile(
        icons,
        "transition.push",
        &[
            ("M3 13h18v6h-18Z", Some(SLIDE_BLUE)),
            ("M12 6l4 4h-2.5v4h-3v-4h-2.5Z", Some(ARROW)),
        ],
    );
    tile(
        icons,
        "transition.wipe",
        &[
            ("M13 5h8v14h-8Z", Some(SLIDE_BLUE)),
            ("M6 12l4-4v2.5h5v3h-5v2.5Z", Some(ARROW)),
        ],
    );
    tile(
        icons,
        "transition.split",
        &[
            ("M9 5h6v14h-6Z", Some(SLIDE_BLUE)),
            ("M4 12l3-3v6Z", Some(ARROW)),
            ("M20 12l-3-3v6Z", Some(ARROW)),
        ],
    );
    tile(
        icons,
        "transition.cover",
        &[
            ("M8 8h13v11h-13Z", Some(SLIDE_BLUE)),
            ("M5 12l3-3v6Z", Some(ARROW)),
        ],
    );
    tile(
        icons,
        "transition.pull",
        &[
            ("M3 8h11v11h-11Z", Some(SLIDE_BLUE)),
            ("M19 12l-3-3v6Z", Some(ARROW)),
        ],
    );
    tile(
        icons,
        "transition.cut",
        &[("M3 5h9v14h-9Z", Some(SLIDE_BLUE))],
    );
    tile(
        icons,
        "transition.zoom",
        &[
            ("M9 9h6v6h-6Z", Some(SLIDE_BLUE)),
            ("M6 7h12v10h-12Z", None),
        ],
    );
    icons.alias("transition.preview", "show.fromCurrent");
    icons.add_solid(
        "transition.effect",
        &[
            ("M5 4h14v14h-14Z", Some(SLIDE_PALE)),
            ("M5 4h14v14h-14Z", None),
            ("M9 20l3 2.5l3-2.5Z", Some(ARROW)),
        ],
    );
    icons.alias("transition.duration", "view.reading");
    for id in ["transition.more", "anim.more"] {
        icons.add_solid(
            id,
            &[
                ("M7 7h10v1.5h-10Z", Some(Rgba(0x44, 0x44, 0x44, 255))),
                ("M7.5 11h9l-4.5 5Z", Some(Rgba(0x44, 0x44, 0x44, 255))),
            ],
        );
    }
    icons.alias("anim.effect", "transition.effect");
    for (id, effect) in EFFECTS {
        let color = match effect {
            None => NONE,
            Some((_, "emph")) => EMPH,
            Some((_, "exit")) => EXIT,
            _ => ENTR,
        };
        icons.add_solid(id, &[(STAR, Some(color))]);
    }
    icons.add_solid(
        "design.colors",
        &[
            ("M4 4h8v8h-8Z", Some(Rgba(0x2e, 0x4a, 0x7d, 255))),
            ("M12 4h8v8h-8Z", Some(Rgba(0x5b, 0x8f, 0xd0, 255))),
            ("M4 12h8v8h-8Z", Some(Rgba(0x3f, 0xa3, 0x4d, 255))),
            ("M12 12h8v8h-8Z", Some(Rgba(0xe8, 0x8a, 0x2e, 255))),
        ],
    );
    icons.alias("design.fonts", "format.fontDialog");
    icons.alias("design.slideSize", "layout.size");
    icons.alias("design.background", "format.fillColor");
    icons.alias("review.comments", "review.pane");
    icons.alias("anim.pane", "view.animationPane");
}

/// A theme tile of the Design tab in its gallery slot `r` (full band height): measured on
/// the reference at 125 %, a 130 x 74 px card framed dark grey (100) 9 px below the gallery
/// box's top, "Aa" in the theme's heading font, a row of its accents 57 px down; the current
/// theme sits on a light grey (210) band framed in 136.
pub(crate) fn paint_theme_tile(
    p: &mut dyn Painter,
    r: [f32; 4],
    box_top: f32,
    scale: f32,
    heading: &str,
    colors: &[Rgba],
    current: bool,
) {
    let px = |v: f32| (v * scale).round();
    // The current band starts 2 px left of the ribbon slot, the card 3 px right of it.
    let x = r[0] - px(1.6);
    if current {
        p.fill_rect(
            x,
            box_top + px(3.2),
            px(111.2),
            px(66.4),
            Rgba(0x88, 0x8a, 0x86, 255),
        );
        p.fill_rect(
            x + 1.0,
            box_top + px(3.2) + 1.0,
            px(111.2) - 2.0,
            px(66.4) - 2.0,
            Rgba(0xd2, 0xd0, 0xce, 255),
        );
    }
    let (cx, cy, cw, ch) = (x + px(4.0), box_top + px(7.2), px(104.0), px(59.2));
    p.fill_rect(cx, cy, cw, ch, Rgba(0x64, 0x64, 0x64, 255));
    p.fill_rect(
        cx + 1.0,
        cy + 1.0,
        cw - 2.0,
        ch - 2.0,
        Rgba(255, 255, 255, 255),
    );
    let font = crate::Font {
        family: heading,
        size: px(22.4),
        weight: 400,
        italic: false,
        underline: false,
    };
    p.text_font("Aa", cx + px(9.6), cy + px(4.0), &font, Rgba(0, 0, 0, 255));
    let bw = px(8.8);
    let gap = px(1.6);
    let by = cy + px(45.6);
    for (i, c) in colors.iter().take(6).enumerate() {
        p.fill_rect(cx + px(7.2) + i as f32 * (bw + gap), by, bw, px(6.4), *c);
    }
}
