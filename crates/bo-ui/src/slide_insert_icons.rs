//! BlueSlide's Insert tab icons in the filled native style (round 16): solid colour areas
//! under grey outlines, the palette of BlueDoc's `icons.rs` (`SOLID`: light blue, warm
//! yellow, blue, green). The shapes are Bluestar's own drawings in the 24-unit grid — the
//! reference's icons were only looked at for which accent colour means what (blue table
//! header, yellow header / footer bands, blue letter A, green plus on a comment).
use crate::icons::{Icons, ICON_BLUE};
use bo_render::Rgba;

const WHITE: Rgba = Rgba(0xff, 0xff, 0xff, 255);
/// Light blue of filled areas (BlueDoc's `SOFT`).
const SOFT: Rgba = Rgba(0xb9, 0xd2, 0xee, 255);
/// Warm yellow (BlueDoc's `WARM`).
const WARM: Rgba = Rgba(0xec, 0xc6, 0x7e, 255);
/// Green of "add" marks (BlueDoc's `GREEN`).
const GREEN: Rgba = Rgba(0x1e, 0x8e, 0x3e, 255);
/// Lighter blue for the lit side of solids.
const MID: Rgba = Rgba(0x5b, 0x8f, 0xd0, 255);
/// Dark grey of solid parts (a camera body).
const DARK: Rgba = Rgba(0x3b, 0x44, 0x52, 255);

/// A green plus badge centred on (`cx`, `cy`).
fn plus(cx: f32, cy: f32) -> [(String, Option<Rgba>); 3] {
    [
        (
            format!(
                "M{} {}a3.6 3.6 0 1 0 7.2 0a3.6 3.6 0 1 0 -7.2 0Z",
                cx - 3.6,
                cy
            ),
            Some(GREEN),
        ),
        (
            format!("M{} {}h1.2v4.4h-1.2Z", cx - 0.6, cy - 2.2),
            Some(WHITE),
        ),
        (
            format!("M{} {}h4.4v1.2h-4.4Z", cx - 2.2, cy - 0.6),
            Some(WHITE),
        ),
    ]
}

fn add(icons: &mut Icons, id: &str, parts: &[(&str, Option<Rgba>)], badge: Option<(f32, f32)>) {
    let extra = badge.map(|(x, y)| plus(x, y));
    let mut all: Vec<(&str, Option<Rgba>)> = parts.to_vec();
    if let Some(b) = &extra {
        all.extend(b.iter().map(|(d, c)| (d.as_str(), *c)));
    }
    icons.add_solid(id, &all);
}

/// Replace the Insert tab's icons by the filled drawings.
pub(super) fn install(icons: &mut Icons) {
    // New Slide: a slide with a blue title band and a green plus.
    add(
        icons,
        "slide.new",
        &[
            ("M2 7h17v13H2Z", Some(WHITE)),
            ("M2 7h17v4H2Z", Some(SOFT)),
            ("M2 7h17v13H2Z", None),
            ("M5 14.5h11M5 17h7", None),
        ],
        Some((19.0, 6.0)),
    );
    // Table: blue header row.
    add(
        icons,
        "sl.table",
        &[
            ("M3 4h18v16H3Z", Some(WHITE)),
            ("M3 4h18v4.5H3Z", Some(ICON_BLUE)),
            ("M3 4h18v16H3Z", None),
            ("M3 8.5h18M3 14h18M9 8.5v11.5M15 8.5v11.5", None),
        ],
        None,
    );
    // Pictures: sky, sun and hills in a frame.
    add(
        icons,
        "sl.pictures",
        &[
            ("M3 4h18v16H3Z", Some(SOFT)),
            ("M3 20l6-8l4 5l3-3l5 6Z", Some(ICON_BLUE)),
            (
                "M15 7.5a1.8 1.8 0 1 0 3.6 0a1.8 1.8 0 1 0 -3.6 0Z",
                Some(WARM),
            ),
            ("M3 4h18v16H3Z", None),
        ],
        None,
    );
    // Photo Album: a framed picture over a second one.
    add(
        icons,
        "sl.photoAlbum",
        &[
            ("M6 3h15v13H6Z", Some(WHITE)),
            ("M6 3h15v13H6Z", None),
            ("M3 7h15v14H3Z", Some(SOFT)),
            ("M3 21l5-6l3 4l3-2l4 4Z", Some(ICON_BLUE)),
            ("M3 7h15v14H3Z", None),
        ],
        None,
    );
    // Screenshot: a window with a dark camera in its corner.
    add(
        icons,
        "sl.screenshot",
        &[
            ("M5 3h17v13H5Z", Some(WHITE)),
            ("M5 3h17v3H5Z", Some(SOFT)),
            ("M5 3h17v13H5Z", None),
            ("M2 14h9v7H2Z", Some(DARK)),
            ("M4.5 12.5h3v1.5h-3Z", Some(DARK)),
            (
                "M4.8 17.5a1.7 1.7 0 1 0 3.4 0a1.7 1.7 0 1 0 -3.4 0Z",
                Some(WHITE),
            ),
        ],
        None,
    );
    // Shapes: a light square under a blue circle.
    add(
        icons,
        "sl.shapes",
        &[
            ("M3 8h12v13H3Z", Some(SOFT)),
            ("M3 8h12v13H3Z", None),
            ("M11 8a6 6 0 1 0 12 0a6 6 0 1 0 -12 0Z", Some(ICON_BLUE)),
        ],
        None,
    );
    // Icons: a blue person in a light badge.
    add(
        icons,
        "sl.icons",
        &[
            ("M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0Z", Some(SOFT)),
            ("M9 9a3 3 0 1 0 6 0a3 3 0 1 0 -6 0Z", Some(ICON_BLUE)),
            ("M6 18.5c1-3.2 3.2-4.5 6-4.5s5 1.3 6 4.5Z", Some(ICON_BLUE)),
            ("M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0Z", None),
        ],
        None,
    );
    // 3D Models: a cube lit from the top.
    add(
        icons,
        "sl.model3d",
        &[
            ("M12 3l8 4.5l-8 4.5l-8-4.5Z", Some(SOFT)),
            ("M4 7.5l8 4.5v9l-8-4.5Z", Some(ICON_BLUE)),
            ("M20 7.5l-8 4.5v9l8-4.5Z", Some(MID)),
            ("M12 3l8 4.5v9l-8 4.5l-8-4.5v-9Z", None),
        ],
        None,
    );
    // SmartArt: three linked green boxes.
    add(
        icons,
        "sl.smartart",
        &[
            ("M9 2h6v5H9Z", Some(GREEN)),
            ("M2 16h7v5H2Z", Some(GREEN)),
            ("M15 16h7v5h-7Z", Some(GREEN)),
            ("M12 7v5M5.5 16v-4h13v4", None),
        ],
        None,
    );
    // Chart: blue, yellow and light bars.
    add(
        icons,
        "sl.chart",
        &[
            ("M4 10h4v11H4Z", Some(ICON_BLUE)),
            ("M10 5h4v16h-4Z", Some(WARM)),
            ("M16 13h4v8h-4Z", Some(SOFT)),
            ("M2 21h20", None),
        ],
        None,
    );
    // Links: two chain links over a light globe.
    add(
        icons,
        "sl.links",
        &[
            ("M5 12a7 7 0 1 0 14 0a7 7 0 1 0 -14 0Z", Some(SOFT)),
            (
                "M10 9.5H7a2.5 2.5 0 0 0 0 5h3M14 9.5h3a2.5 2.5 0 0 1 0 5h-3M9 12h6",
                None,
            ),
        ],
        None,
    );
    // Comment: a white bubble with a green plus.
    add(
        icons,
        "sl.comment",
        &[
            ("M3 6h18v11h-9l-5 4v-4H3Z", Some(WHITE)),
            ("M3 6h18v11h-9l-5 4v-4H3Z", None),
            ("M7 13h10", None),
        ],
        Some((6.0, 5.5)),
    );
    // Text Box: a page with a blue letter A.
    add(
        icons,
        "sl.textBox",
        &[
            ("M3 3h18v18H3Z", Some(WHITE)),
            ("M3 3h18v18H3Z", None),
            (
                "M5.5 13l3-8h1.6l3 8h-1.8l-.7-2h-3.6l-.7 2ZM8.6 9.5h2.4l-1.2-3.4Z",
                Some(ICON_BLUE),
            ),
            ("M14 8h4.5M14 11h4.5M6 15.5h12.5M6 18.5h9", None),
        ],
        None,
    );
    // Header & Footer: a page with yellow bands at the top and bottom.
    add(
        icons,
        "sl.headerFooter",
        &[
            ("M5 2h14v20H5Z", Some(WHITE)),
            ("M5 2h14v4H5Z", Some(WARM)),
            ("M5 18h14v4H5Z", Some(WARM)),
            ("M5 2h14v20H5Z", None),
            ("M8 9.5h8M8 12.5h8M8 15h5", None),
        ],
        None,
    );
    // Text Styles: a slanted blue A.
    add(
        icons,
        "sl.textStyles",
        &[(
            "M11 3h4l4 18h-4l-.9-4.5h-6.2l-2.6 4.5h-4ZM10.3 13h3.6l-1.2-6Z",
            Some(ICON_BLUE),
        )],
        None,
    );
    // Date & Time: a calendar page with a blue top.
    add(
        icons,
        "sl.dateTime",
        &[
            ("M3 5h18v16H3Z", Some(WHITE)),
            ("M3 5h18v4H3Z", Some(ICON_BLUE)),
            ("M3 5h18v16H3Z", None),
            (
                "M7 3v4M17 3v4M6.5 12.5h2M11 12.5h2M15.5 12.5h2M6.5 16.5h2M11 16.5h2",
                None,
            ),
        ],
        None,
    );
    // Slide Number: a slide with a blue number sign.
    add(
        icons,
        "sl.slideNumber",
        &[
            ("M3 4h18v16H3Z", Some(WHITE)),
            ("M3 4h18v16H3Z", None),
            ("M10 7.5l-1.5 9M15 7.5l-1.5 9M7.5 10.5h9.5M7 13.5h9.5", None),
            ("M3 17.5h18v2.5H3Z", Some(SOFT)),
        ],
        None,
    );
    // Object: a framed sheet holding a light grid.
    add(
        icons,
        "sl.object",
        &[
            ("M3 3h18v18H3Z", Some(WHITE)),
            ("M7 7h10v10H7Z", Some(SOFT)),
            ("M3 3h18v18H3Z", None),
            ("M7 7h10v10H7ZM7 10.5h10M7 13.8h10M10.5 7v10", None),
        ],
        None,
    );
    // Symbols: an omega on a light tile.
    add(
        icons,
        "sl.symbols",
        &[
            ("M3 3h18v18H3Z", Some(SOFT)),
            ("M3 3h18v18H3Z", None),
            ("M7 18h3.5v-1.6a5 5 0 1 1 3 0V18H17", None),
        ],
        None,
    );
    // Media: a blue speaker with sound waves.
    add(
        icons,
        "sl.media",
        &[
            ("M3 9h4l5-4v14l-5-4H3Z", Some(ICON_BLUE)),
            ("M15 9a4 4 0 0 1 0 6M17.5 6.5a7.5 7.5 0 0 1 0 11", None),
        ],
        None,
    );
}
