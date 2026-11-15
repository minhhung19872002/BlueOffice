//! BlueSlide's Design, Transitions, Animations and Review tabs (N6, round 2): their commands,
//! labels (from the presentation editor's existing strings), own icons and the theme tiles
//! drawn over the ribbon, laid out like the reference presentation app's tabs.
use crate::icons::Icons;
use crate::ribbon::{Group, Item, Tab};
use crate::strings::Strings;
use crate::Painter;
use bo_render::Rgba;

/// Transitions offered in the ribbon gallery: command id and the engine's transition kind
/// (`bo_pptx::transitions` catalog), in the reference app's gallery order (None, Morph, Fade,
/// Push, Wipe, Split in the ribbon; the rest in its menu).
pub const TRANSITIONS: [(&str, &str); 27] = [
    ("transition.none", "none"),
    ("transition.morph", "morph"),
    ("transition.fade", "fade"),
    ("transition.push", "push"),
    ("transition.wipe", "wipe"),
    ("transition.split", "split"),
    ("transition.reveal", "reveal"),
    ("transition.cut", "cut"),
    ("transition.randomBar", "randomBar"),
    ("transition.cover", "cover"),
    ("transition.pull", "pull"),
    ("transition.zoom", "zoom"),
    ("transition.dissolve", "dissolve"),
    ("transition.blinds", "blinds"),
    ("transition.checker", "checker"),
    ("transition.comb", "comb"),
    ("transition.circle", "circle"),
    ("transition.diamond", "diamond"),
    ("transition.plus", "plus"),
    ("transition.wedge", "wedge"),
    ("transition.strips", "strips"),
    ("transition.wheel", "wheel"),
    ("transition.newsflash", "newsflash"),
    ("transition.flash", "flash"),
    ("transition.vortex", "vortex"),
    ("transition.switch", "switch"),
    ("transition.flip", "flip"),
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

/// Tiles of the animation gallery: four at this width on the reference (its Advanced
/// Animation and Timing groups are wider).
pub const ANIM_TILES: usize = 4;

/// Other commands of these tabs.
const OWN: [&str; 39] = [
    "show.hideSlide",
    "review.deleteComment",
    "review.prevComment",
    "review.nextComment",
    "anim.trigger",
    "anim.painter",
    "anim.add",
    "anim.start",
    "anim.duration",
    "anim.delay",
    "anim.reorder",
    "anim.earlier",
    "anim.later",
    "transition.sound",
    "transition.advance",
    "transition.onClick",
    "transition.after",
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
    CURRENT_THEME,
    "design.more",
    "design.variants",
    "design.effects",
    "design.layout",
];

/// Whether `id` is one of the commands of these tabs.
pub fn is_command(id: &str) -> bool {
    OWN.contains(&id)
        || DRAW_TOOLS.contains(&id)
        || id == "view.normal"
        || id == "draw.inkToShape"
        || TRANSITIONS.iter().any(|(c, _)| *c == id)
        || EFFECTS.iter().any(|(c, _)| *c == id)
        || THEMES.iter().any(|(c, _)| *c == id)
}

/// BlueSlide-only buttons drawn with the drop-down mark (they open a menu or a picker).
pub fn has_menu_mark(id: &str) -> bool {
    matches!(
        id,
        "sl.table"
            | "sl.shapes"
            | "sl.arrange"
            | "sl.quickStyles"
            | "sl.fill"
            | "sl.outline"
            | "sl.effects"
            | "sl.editing"
            | "sl.symbols"
            | "sl.media"
            | "sl.align"
            | "sl.group"
            | "sl.rotate"
            | "sl.crop"
            | "sl.picBorder"
            | "sl.shading"
            | "sl.tblDelete"
            | "design.variants"
            | "design.colors"
            | "design.fonts"
            | "design.effects"
            | "design.background"
            | "design.layout"
            | "design.slideSize"
            | "show.custom"
            | "sl.smartart"
            | "sl.links"
            | "sl.textStyles"
    )
}

/// Commands that open a menu (Slide Size, Effect Options, …).
pub fn is_menu(id: &str) -> bool {
    matches!(
        id,
        "design.colors"
            | "design.fonts"
            | "design.variants"
            | "design.effects"
            | "design.layout"
            | "design.more"
            | "design.slideSize"
            | "design.background"
            | "transition.effect"
            | "transition.more"
            | "anim.effect"
            | "anim.add"
            | "anim.trigger"
            | "anim.more"
    )
}

/// Width (logical px) of gallery tiles, measured on the reference at 125 %: theme tiles
/// 130 device px, transition and effect tiles 124.
pub fn large_width(id: &str) -> Option<f32> {
    // Measured at 125 %: the Preview group's divider 92 px from the left, gallery pitch
    // 124 px (121 + the ribbon's 3 px gap), the "more" column 36 px framed, Effect Options
    // reaching the next divider at 985.
    match id {
        "transition.preview" | "anim.preview" => return Some(48.8),
        "transition.more" | "anim.more" | "design.more" => return Some(29.6),
        // Design tab, measured on the reference at 125 %: Variants 66, Colors 58, Fonts 62,
        // Effects 60, Background Styles 100, Layout 66, Slide Size 60 device px.
        "design.variants" | "design.layout" => return Some(52.8),
        "design.colors" => return Some(46.4),
        "design.fonts" => return Some(49.6),
        "design.effects" | "design.slideSize" => return Some(48.0),
        "design.background" => return Some(80.0),
        CURRENT_THEME => return Some(109.6),
        "transition.effect" | "anim.effect" => return Some(72.0),
        // Add Animation: reference x 745-850.
        "anim.add" => return Some(89.6),
        _ => {}
    }
    if THEMES.iter().any(|(c, _)| *c == id) {
        // Reference pitch 140 px with the ribbon's 2 px gap.
        Some(109.6)
    } else if TRANSITIONS.iter().any(|(c, _)| *c == id) || EFFECTS.iter().any(|(c, _)| *c == id) {
        Some(96.8)
    } else {
        None
    }
}

/// Draw tab: Select and the drawing tools (pens, highlighter, eraser).
pub(crate) fn draw_tab() -> Tab {
    use Item::Large as L;
    Tab {
        label: "slideNative.draw",
        groups: vec![
            Group {
                label: "slideNative.tools",
                large: vec![L("draw.eraser"), L("draw.select"), L("draw.lasso")],
                rows: vec![],
            },
            Group {
                label: "slideNative.pens",
                large: vec![
                    L("draw.pen"),
                    L("draw.red"),
                    L("draw.highlight"),
                    L("draw.blue"),
                    L("draw.green"),
                ],
                rows: vec![],
            },
            Group {
                label: "slideNative.convert",
                large: vec![L("draw.inkToShape")],
                rows: vec![],
            },
        ],
    }
}

/// Commands of the Draw tab.
pub const DRAW_TOOLS: [&str; 8] = [
    "draw.select",
    "draw.pen",
    "draw.red",
    "draw.highlight",
    "draw.eraser",
    "draw.lasso",
    "draw.blue",
    "draw.green",
];

/// Design tab as the reference shows it at 1500 px: six theme cards (this presentation's
/// theme first) and the gallery's "more" column; Customize (Variants, Colors, Fonts, Effects,
/// Background Styles); Slides (Layout, Slide Size).
pub(crate) fn design_tab() -> Tab {
    use Item::Large as L;
    Tab {
        label: "slideNative.design",
        groups: vec![
            Group {
                label: "slideEditor.design.groupThemes",
                large: std::iter::once(L(CURRENT_THEME))
                    .chain(THEMES.iter().map(|(c, _)| L(c)))
                    .chain([L("design.more")])
                    .collect(),
                rows: vec![],
            },
            Group {
                label: "slideNative.customize",
                large: vec![
                    L("design.variants"),
                    L("design.colors"),
                    L("design.fonts"),
                    L("design.effects"),
                    L("design.background"),
                ],
                rows: vec![],
            },
            Group {
                label: "slideEditor.groupSlides",
                large: vec![L("design.layout"), L("design.slideSize")],
                rows: vec![],
            },
        ],
    }
}

/// The first theme card: this presentation's own theme.
pub const CURRENT_THEME: &str = "design.theme.current";

pub(crate) fn transitions_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "slideEditor.transitionTab",
        groups: vec![
            Group {
                label: "slideEditor.groupPreview",
                large: vec![L("transition.preview")],
                rows: vec![],
            },
            // Tiles, the "more" column and Effect Options share one group, as on the
            // reference ribbon.
            Group {
                label: "slideNative.transitionGallery",
                large: TRANSITIONS
                    .iter()
                    .take(GALLERY_TILES)
                    .map(|(c, _)| L(c))
                    .chain([L("transition.more"), L("transition.effect")])
                    .collect(),
                rows: vec![],
            },
            // Two columns of three rows: sound, duration, apply to all; advance slide, on
            // mouse click, after (the window draws the boxes over these slots).
            Group {
                label: "slideNative.timing",
                large: vec![],
                rows: vec![
                    vec![S("transition.sound"), S("transition.advance")],
                    vec![S("transition.duration"), S("transition.onClick")],
                    vec![S("transition.applyAll"), S("transition.after")],
                ],
            },
        ],
    }
}

/// Fixed widths (logical px) of the timing slots: the reference's two columns at 125 %
/// (first 237 device px to its separator at x 1235, second 188 to the group's edge).
pub fn row_width(id: &str) -> Option<f32> {
    match id {
        "transition.sound" | "transition.duration" | "transition.applyAll" => Some(189.6),
        "transition.advance" | "transition.onClick" | "transition.after" => Some(150.4),
        // Reference at 125 %: first column 1046-1272, second to the edge at 1438.
        "anim.start" | "anim.duration" | "anim.delay" => Some(180.8),
        "anim.reorder" | "anim.earlier" | "anim.later" => Some(124.0),
        _ => None,
    }
}

pub(crate) fn animations_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
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
                    .take(ANIM_TILES)
                    .map(|(c, _)| L(c))
                    .chain([L("anim.more"), L("anim.effect")])
                    .collect(),
                rows: vec![],
            },
            Group {
                label: "slideNative.advanced",
                large: vec![L("anim.add")],
                rows: vec![
                    vec![S("anim.pane")],
                    vec![S("anim.trigger")],
                    vec![S("anim.painter")],
                ],
            },
            // Start, duration and delay of the chosen effect; reorder it (drawn controls).
            Group {
                label: "slideNative.timing",
                large: vec![],
                rows: vec![
                    vec![S("anim.start"), S("anim.reorder")],
                    vec![S("anim.duration"), S("anim.earlier")],
                    vec![S("anim.delay"), S("anim.later")],
                ],
            },
        ],
    }
}

pub(crate) fn review_tab() -> Tab {
    use Item::Large as L;
    Tab {
        label: "ribbon.tabs.review",
        groups: vec![Group {
            label: "ribbon.groups.comments",
            large: vec![
                L("review.newComment"),
                L("review.deleteComment"),
                L("review.prevComment"),
                L("review.nextComment"),
                L("review.comments"),
            ],
            rows: vec![],
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
    s.set(&format!("commands.{CURRENT_THEME}"), "");
    let this = s.get("slideNative.thisPresentation").to_owned();
    s.set(&format!("commands.tip.{CURRENT_THEME}"), &this);
    s.set("commands.design.more", "");
    for (id, theme) in THEMES {
        // Theme tiles show a preview, not a label; the name is the tooltip.
        copy(
            s,
            &format!("tip.{id}"),
            &format!("slideEditor.design.theme.{theme}"),
        );
        s.set(&format!("commands.{id}"), "");
    }
    // Column headings are drawn by the window: no label of the ribbon's own under them.
    for id in ["transition.advance", "anim.reorder"] {
        s.set(&format!("commands.{id}"), "");
    }
    for (id, key) in [
        ("design.slideSize", "slideNative.slideSize"),
        ("design.background", "slideNative.backgroundStyles"),
        ("design.colors", "slideNative.colors"),
        ("design.fonts", "slideNative.fonts"),
        ("design.variants", "slideEditor.design.groupVariants"),
        ("design.effects", "slideNative.themeEffects"),
        ("design.layout", "slideNative.layout"),
        ("transition.preview", "slideEditor.preview"),
        ("anim.preview", "slideEditor.preview"),
        ("transition.effect", "slideNative.effectOptions"),
        ("anim.effect", "slideNative.effectOptions"),
        ("transition.duration", "slideNative.duration"),
        ("transition.applyAll", "slideNative.applyToAll"),
        ("transition.sound", "slideNative.sound"),
        ("anim.start", "slideNative.start"),
        ("anim.duration", "slideNative.duration"),
        ("anim.delay", "slideNative.delay"),
        ("anim.earlier", "slideNative.moveEarlier"),
        ("anim.later", "slideNative.moveLater"),
        ("transition.onClick", "slideNative.onMouseClick"),
        ("transition.after", "slideNative.after"),
        ("anim.pane", "slideNative.animationPane"),
        ("transition.more", "slideEditor.transitionGroup.subtle"),
        ("anim.more", "slideEditor.addEffect"),
        ("anim.add", "slideNative.addAnimation"),
        ("anim.trigger", "slideNative.trigger"),
        ("anim.painter", "slideNative.animationPainter"),
        ("draw.select", "slideNative.select"),
        ("draw.pen", "slideNative.pen"),
        ("draw.red", "slideNative.redPen"),
        ("draw.highlight", "slideNative.highlighter"),
        ("draw.eraser", "slideNative.eraser"),
        ("draw.lasso", "slideNative.lasso"),
        ("draw.blue", "slideNative.bluePen"),
        ("draw.green", "slideNative.greenPen"),
        ("draw.inkToShape", "slideNative.inkToShape"),
        ("review.newComment", "slideEditor.comments.new"),
        ("review.comments", "slideNative.showComments"),
        ("review.deleteComment", "slideEditor.comments.delete"),
        ("review.prevComment", "slideNative.previous"),
        ("review.nextComment", "slideNative.next"),
        ("show.hideSlide", "slideNative.hideSlide"),
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
const ENTR: Rgba = Rgba(0x68, 0xa4, 0x90, 255);
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
    // Timing group (own drawings): a speaker with sound waves and a blue arrow, a clock, a
    // stack of slides with the arrow.
    let blue = Rgba(0x44, 0x80, 0xc0, 255);
    let grey = Rgba(0x72, 0x72, 0x72, 255);
    icons.add_solid(
        "transition.sound",
        &[
            ("M3 8h4l5-4.5v15l-5-4.5h-4Z", Some(grey)),
            ("M15 8.5c1.3 1.2 1.3 5.8 0 7", None),
            ("M17.5 6c2.4 2.2 2.4 10 0 12", None),
            ("M10 21h7v-1.8l3 2.8l-3 2.8v-1.8h-7Z", Some(blue)),
        ],
    );
    icons.add_solid(
        "transition.clock",
        &[
            ("M12 3a9 9 0 1 0 0.01 0Z", Some(blue)),
            (
                "M12 4.5a7.5 7.5 0 1 0 0.01 0Z",
                Some(Rgba(255, 255, 255, 255)),
            ),
            (
                "M11.3 7h1.4v5.3h4v1.4h-5.4Z",
                Some(Rgba(0x40, 0x40, 0x40, 255)),
            ),
        ],
    );
    icons.add_solid(
        "anim.start",
        &[
            (
                "M6 3l10 10h-5l3 7l-2.5 1l-3-7l-2.5 3Z",
                Some(Rgba(255, 255, 255, 255)),
            ),
            ("M6 3l10 10h-5l3 7l-2.5 1l-3-7l-2.5 3Z", None),
        ],
    );
    icons.alias("anim.delay", "transition.clock");
    icons.alias("draw.select", "anim.start");
    icons.add_solid(
        "anim.trigger",
        &[(
            "M13 2l-8 11h6l-2 9l9-12h-6Z",
            Some(Rgba(0xf0, 0xb4, 0x1e, 255)),
        )],
    );
    icons.add_solid(
        "anim.painter",
        &[
            (STAR, Some(Rgba(0x44, 0x80, 0xc0, 255))),
            ("M14 13l7 7l-2 2l-7-7Z", Some(Rgba(0xd8, 0x9a, 0x3a, 255))),
        ],
    );
    // Preview of effects: a star with a green play button (own drawing).
    icons.add_solid(
        "anim.preview",
        &[
            (STAR, Some(Rgba(0x44, 0x80, 0xc0, 255))),
            (
                "M17 12a6 6 0 1 0 0.01 0Z",
                Some(Rgba(0x3f, 0xa3, 0x4d, 255)),
            ),
            (
                "M15.5 14.5l4.5 3.5l-4.5 3.5Z",
                Some(Rgba(255, 255, 255, 255)),
            ),
        ],
    );
    // Pens: a slanted barrel with a coloured tip and cap (own drawings).
    let pen = |icons: &mut Icons, id: &str, tip: Rgba, wide: bool| {
        let body = if wide {
            "M14 3l6 6l-9 9h-6v-6Z"
        } else {
            "M16 3l5 5l-11 11l-5 1l1-5Z"
        };
        icons.add_solid(
            id,
            &[
                (body, Some(Rgba(0xf4, 0xf4, 0xf4, 255))),
                (body, None),
                (
                    if wide {
                        "M5 12v6h6Z"
                    } else {
                        "M5 15l4 4l-5 1Z"
                    },
                    Some(tip),
                ),
                ("M3 21h18v2h-18Z", Some(tip)),
            ],
        );
    };
    pen(icons, "draw.pen", Rgba(0x20, 0x20, 0x20, 255), false);
    pen(icons, "draw.red", Rgba(0xe7, 0x12, 0x24, 255), false);
    pen(icons, "draw.highlight", Rgba(0xff, 0xe0, 0x00, 255), true);
    pen(icons, "draw.blue", Rgba(0x0f, 0x5e, 0xaa, 255), false);
    pen(icons, "draw.green", Rgba(0x1e, 0x8a, 0x3c, 255), false);
    // Lasso: a dashed loop with a plus; Ink to Shape: a stroke turning into a square.
    icons.add_solid(
        "draw.lasso",
        &[
            ("M6 4c5-2 12 0 12 5s-6 7-11 6s-5-9-1-11Z", None),
            (
                "M17 15h2v3h3v2h-3v3h-2v-3h-3v-2h3Z",
                Some(Rgba(0x44, 0x80, 0xc0, 255)),
            ),
        ],
    );
    icons.add_solid(
        "draw.inkToShape",
        &[
            ("M3 10c2-4 4 4 6 0", None),
            ("M11 9h10v12h-10Z", Some(Rgba(0xcf, 0xe0, 0xf3, 255))),
            ("M11 9h10v12h-10Z", None),
            ("M7 15l3 3l-3 3Z", Some(Rgba(0x44, 0x80, 0xc0, 255))),
        ],
    );
    icons.add_solid(
        "draw.eraser",
        &[
            (
                "M9 20l-5-5l10-10l7 7l-8 8Z",
                Some(Rgba(0xf4, 0xb8, 0xc8, 255)),
            ),
            ("M4 15l5 5h11", None),
            ("M9 20l-5-5l10-10l7 7l-8 8Z", None),
        ],
    );
    icons.add_solid(
        "anim.earlier",
        &[("M12 4l7 8h-4.5v8h-5v-8h-4.5Z", Some(blue))],
    );
    icons.add_solid(
        "anim.later",
        &[("M12 20l7-8h-4.5v-8h-5v8h-4.5Z", Some(blue))],
    );
    icons.add_solid(
        "transition.applyAll",
        &[
            ("M2 3h15v11h-15Z", Some(Rgba(0xf4, 0xf4, 0xf4, 255))),
            ("M2 3h15v11h-15Z", None),
            ("M5 6h15v11h-15Z", Some(Rgba(255, 255, 255, 255))),
            ("M5 6h15v11h-15Z", None),
            ("M7 8.5h11v2h-11Z", Some(Rgba(0xbf, 0xbf, 0xbf, 255))),
            ("M10 21h7v-1.8l3 2.8l-3 2.8v-1.8h-7Z", Some(blue)),
        ],
    );
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
    // Add Animation: a star with a plus (own drawing).
    icons.add_solid(
        "anim.add",
        &[
            (STAR, Some(Rgba(0x44, 0x80, 0xc0, 255))),
            (
                "M3 2h2v3h3v2h-3v3h-2v-3h-3v-2h3Z",
                Some(Rgba(0x3f, 0xa3, 0x4d, 255)),
            ),
        ],
    );
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
    // Variants: a card with a brush; Effects: a bevelled tile (own drawings).
    icons.add_solid(
        "design.variants",
        &[
            (
                "M4 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-14a2 2 0 0 1-2-2v-12a2 2 0 0 1 2-2Z",
                Some(Rgba(255, 255, 255, 255)),
            ),
            (
                "M4 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-14a2 2 0 0 1-2-2v-12a2 2 0 0 1 2-2Z",
                None,
            ),
            (
                "M12 16l9-9l1.5 1.5l-9 9Z",
                Some(Rgba(0x5b, 0x8f, 0xd0, 255)),
            ),
        ],
    );
    icons.add_solid(
        "design.effects",
        &[
            (
                "M5 6h12l3 3v10h-12l-3-3Z",
                Some(Rgba(0x80, 0x80, 0x80, 255)),
            ),
            ("M5 6h12v10h-12Z", Some(Rgba(0xe6, 0xe6, 0xe6, 255))),
            ("M5 6h12v10h-12Z", None),
        ],
    );
    icons.alias("design.layout", "layout.pageSetup");
    icons.alias("design.more", "transition.more");
    icons.alias("design.slideSize", "layout.size");
    icons.alias("design.background", "format.fillColor");
    icons.alias("review.comments", "review.pane");
    // Comment commands: a speech bubble with a red cross / blue arrows (own drawings).
    let bubble = "M3 4h18v12h-10l-5 4v-4h-3Z";
    let paper = Rgba(0xff, 0xff, 0xff, 255);
    icons.add_solid(
        "review.deleteComment",
        &[
            (bubble, Some(paper)),
            (bubble, None),
            (
                "M8 7l1-1l3 3l3-3l1 1l-3 3l3 3l-1 1l-3-3l-3 3l-1-1l3-3Z",
                Some(Rgba(0xd2, 0x4a, 0x3c, 255)),
            ),
        ],
    );
    icons.add_solid(
        "review.prevComment",
        &[
            (bubble, Some(paper)),
            (bubble, None),
            ("M7 10l4-3.5v2h6v3h-6v2Z", Some(Rgba(0x44, 0x80, 0xc0, 255))),
        ],
    );
    icons.add_solid(
        "review.nextComment",
        &[
            (bubble, Some(paper)),
            (bubble, None),
            (
                "M17 10l-4-3.5v2h-6v3h6v2Z",
                Some(Rgba(0x44, 0x80, 0xc0, 255)),
            ),
        ],
    );
    icons.add_solid(
        "show.hideSlide",
        &[
            ("M3 5h18v13h-18Z", Some(paper)),
            ("M3 5h18v13h-18Z", None),
            ("M5 19l14-15", None),
        ],
    );
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

// Transition tile colours, measured on the reference's gallery at 125 %: a slide-like card
// framed grey 151 then 196 with a light inner bevel, filled pale blue, motifs in a
// darker blue (often shaded across) and grey arrows outlined in white.
const TILE_OUTER: Rgba = Rgba(0x97, 0x96, 0x96, 255);
const TILE_CORNER: Rgba = Rgba(0xc4, 0xc3, 0xc2, 255);
const TILE_INNER: Rgba = Rgba(0xc4, 0xc4, 0xc4, 255);
const TILE_BEVEL: Rgba = Rgba(0xec, 0xf2, 0xf8, 255);
const TILE_FILL: Rgba = Rgba(0xc9, 0xd9, 0xea, 255);
const TILE_BLUE: Rgba = Rgba(0x4d, 0x82, 0xb8, 255);
const TILE_LIGHT: Rgba = Rgba(0xb1, 0xc7, 0xde, 255);
const TILE_ARROW: Rgba = Rgba(0x72, 0x72, 0x72, 255);
const TILE_DIM: Rgba = Rgba(0xd9, 0xd9, 0xd9, 255);

fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Rgba(l(a.0, b.0), l(a.1, b.1), l(a.2, b.2), 255)
}

/// Fill `[x, y, w, h]` shading from `a` (left) to `b` (right), one device column at a time.
fn shade_across(p: &mut dyn Painter, r: [f32; 4], a: Rgba, b: Rgba) {
    let n = r[2].round().max(1.0) as usize;
    for i in 0..n {
        let t = if n > 1 {
            i as f32 / (n - 1) as f32
        } else {
            1.0
        };
        p.fill_rect(r[0] + i as f32, r[1], 1.0, r[3], mix(a, b, t));
    }
}

/// A grey arrow outlined in white (`pts` in device px).
fn tile_arrow(p: &mut dyn Painter, pts: &[(f32, f32)], color: Rgba) {
    let mut segs: Vec<bo_render::PathSeg> = pts
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            if i == 0 {
                bo_render::PathSeg::M(x, y)
            } else {
                bo_render::PathSeg::L(x, y)
            }
        })
        .collect();
    segs.push(bo_render::PathSeg::Z);
    p.path(&segs, None, Some((Rgba(255, 255, 255, 255), 2.0)));
    p.path(&segs, Some(color), None);
}

/// Picture of a transition in its gallery tile (own drawings in the reference's style): a
/// 40 × 36 logical px card at (`x`, `y`) with the effect's motif; `enabled` false greys it.
pub(crate) fn paint_transition_tile(
    p: &mut dyn Painter,
    kind: &str,
    x: f32,
    y: f32,
    scale: f32,
    enabled: bool,
) {
    let (w, h) = ((40.0 * scale).round(), (36.0 * scale).round());
    let g = |c: Rgba| {
        if enabled {
            c
        } else {
            mix(c, TILE_DIM, 0.7)
        }
    };
    // Rounded frame: the outer ring without its corner pixels.
    p.fill_rect(x + 1.0, y, w - 2.0, h, g(TILE_OUTER));
    p.fill_rect(x, y + 1.0, w, h - 2.0, g(TILE_OUTER));
    for (cx, cy) in [
        (x, y),
        (x + w - 1.0, y),
        (x, y + h - 1.0),
        (x + w - 1.0, y + h - 1.0),
    ] {
        p.fill_rect(cx, cy, 1.0, 1.0, g(TILE_CORNER));
    }
    p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, g(TILE_INNER));
    p.fill_rect(x + 2.0, y + 2.0, w - 4.0, h - 4.0, g(TILE_BEVEL));
    let i = [x + 3.0, y + 3.0, w - 6.0, h - 6.0];
    p.fill_rect(i[0], i[1], i[2], i[3], g(TILE_FILL));
    // Interior point at fractions (u, v).
    let at = |u: f32, v: f32| ((i[0] + u * i[2]).round(), (i[1] + v * i[3]).round());
    let arrow = g(TILE_ARROW);
    match kind {
        "morph" => {
            // A square turning into a smaller, rotated one along a diagonal.
            let (ax, ay) = at(0.1, 0.12);
            let (bx, by) = at(0.46, 0.55);
            p.fill_rect(ax, ay, bx - ax, by - ay, g(TILE_BLUE));
            let (cx, cy) = at(0.68, 0.62);
            let k = 0.17 * i[2];
            p.path(
                &[
                    bo_render::PathSeg::M(cx, cy - k),
                    bo_render::PathSeg::L(cx + k, cy),
                    bo_render::PathSeg::L(cx, cy + k),
                    bo_render::PathSeg::L(cx - k, cy),
                    bo_render::PathSeg::Z,
                ],
                Some(g(Rgba(0x8d, 0xb0, 0xd7, 255))),
                None,
            );
            let (dx, dy) = at(0.52, 0.35);
            p.fill_rect(
                dx,
                dy,
                (0.2 * i[2]).round(),
                (0.2 * i[3]).round(),
                g(TILE_LIGHT),
            );
        }
        "fade" => {
            // A slide half way through appearing over the old one.
            let (ax, ay) = at(0.08, 0.1);
            let (bx, by) = at(0.7, 0.72);
            p.fill_rect(ax, ay, bx - ax, by - ay, g(Rgba(0xb4, 0xca, 0xe2, 255)));
            let (cx, cy) = at(0.3, 0.3);
            let (dx, dy) = at(0.94, 0.92);
            p.fill_rect(cx, cy, dx - cx, dy - cy, g(Rgba(0x8f, 0xb2, 0xd8, 255)));
            p.fill_rect(cx, cy, bx - cx, by - cy, g(Rgba(0x7d, 0xa4, 0xcf, 255)));
        }
        "push" => {
            let (_, my) = at(0.0, 0.52);
            p.fill_rect(i[0], my, i[2], i[1] + i[3] - my, g(TILE_BLUE));
            let (cx, top) = at(0.5, 0.06);
            let s = 0.13 * i[2];
            let neck = my + 0.12 * i[3];
            tile_arrow(
                p,
                &[
                    (cx, top),
                    (cx + 1.6 * s, top + 1.5 * s),
                    (cx + 0.6 * s, top + 1.5 * s),
                    (cx + 0.6 * s, neck),
                    (cx - 0.6 * s, neck),
                    (cx - 0.6 * s, top + 1.5 * s),
                    (cx - 1.6 * s, top + 1.5 * s),
                ],
                arrow,
            );
        }
        "wipe" => {
            let (sx, _) = at(0.62, 0.0);
            shade_across(
                p,
                [sx, i[1], i[0] + i[2] - sx, i[3]],
                g(TILE_LIGHT),
                g(TILE_BLUE),
            );
            let (lx, cy) = at(0.14, 0.5);
            let s = 0.11 * i[2];
            let tail = sx - 0.04 * i[2];
            tile_arrow(
                p,
                &[
                    (lx, cy),
                    (lx + 1.7 * s, cy - 1.7 * s),
                    (lx + 1.7 * s, cy - 0.6 * s),
                    (tail, cy - 0.6 * s),
                    (tail, cy + 0.6 * s),
                    (lx + 1.7 * s, cy + 0.6 * s),
                    (lx + 1.7 * s, cy + 1.7 * s),
                ],
                arrow,
            );
        }
        "split" => {
            let (mx, _) = at(0.5, 0.0);
            let half = (0.16 * i[2]).round();
            shade_across(
                p,
                [mx - half, i[1], half, i[3]],
                g(TILE_FILL),
                g(Rgba(0x86, 0xa9, 0xcc, 255)),
            );
            shade_across(
                p,
                [mx, i[1], half, i[3]],
                g(Rgba(0x86, 0xa9, 0xcc, 255)),
                g(TILE_FILL),
            );
            let cy = at(0.0, 0.5).1;
            let s = 0.09 * i[2];
            for dir in [-1.0f32, 1.0] {
                let tip = mx + dir * 0.42 * i[2];
                let base = mx + dir * 0.06 * i[2];
                tile_arrow(
                    p,
                    &[
                        (tip, cy),
                        (tip - dir * 1.6 * s, cy - 1.6 * s),
                        (tip - dir * 1.6 * s, cy - 0.55 * s),
                        (base, cy - 0.55 * s),
                        (base, cy + 0.55 * s),
                        (tip - dir * 1.6 * s, cy + 0.55 * s),
                        (tip - dir * 1.6 * s, cy + 1.6 * s),
                    ],
                    arrow,
                );
            }
        }
        _ => {}
    }
}
