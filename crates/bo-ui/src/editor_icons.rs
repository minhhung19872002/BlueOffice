//! BlueEditor icons not shared with BlueReader: our own line drawings on the reader's 24-unit
//! grid (clean-room; the shapes are ours). Strokes use the ink colour, highlights the accent.
use crate::reader_icons::Part::{self, A, F, FA, S};

/// Shape of editor icon `name` (a plain square when unknown).
pub(crate) fn shape(name: &str) -> &'static [Part] {
    match name {
        // Format tab.
        "font" => &[S("M4 20l6-16h1l6 16 M6.5 14h8"), A("M17 20h4")],
        "fontSize" => &[S("M3 20l5-14h1l5 14 M5 15h7"), A("M18 4v9 M15.5 6.5L18 4l2.5 2.5 M15.5 10.5L18 13l2.5-2.5")],
        "fontColor" => &[S("M6 17l5.5-14h1L18 17 M8 12h8"), FA("M4 19h16v3H4z")],
        "bold" => &[F("M6 3h7a4.5 4.5 0 0 1 2.7 8.1A5 5 0 0 1 14 21H6z M9 6v4.5h4a2.25 2.25 0 0 0 0-4.5z M9 13.5V18h5a2.25 2.25 0 0 0 0-4.5z")],
        "italic" => &[S("M10 4h8 M6 20h8 M14 4l-4 16")],
        "alignLeft" => &[S("M3 5h18 M3 9h12 M3 13h18 M3 17h12"), A("M3 21h8")],
        "alignCenter" => &[S("M3 5h18 M6 9h12 M3 13h18 M6 17h12"), A("M8 21h8")],
        "alignRight" => &[S("M3 5h18 M9 9h12 M3 13h18 M9 17h12"), A("M13 21h8")],
        "alignJustify" => &[S("M3 5h18 M3 9h18 M3 13h18 M3 17h18"), A("M3 21h10")],
        "done" => &[A("M4 12.5l5 5L20 6.5")],
        "fmtUnderline" => &[S("M7 3v8a5 5 0 0 0 10 0V3"), A("M5 21h14")],
        "fmtStrike" => &[S("M17 6.5C16 4.5 14.5 4 12 4c-3 0-5 1.5-5 3.8 0 4.5 10 3.4 10 8.2 0 2.5-2 4-5 4-2.7 0-4.5-1-5.5-3"), A("M3 12h18")],
        // Comment tab.
        "callout" => &[S("M9 3.5h12v9H9z"), A("M9 10L3 19.5")],
        "drawing" => &[S("M3 4h9v7H3z"), A("M16.5 7.5a4 4 0 1 0 .01 0z M4 20l6-5 M14 21l7-7")],
        "stamp" => &[S("M9.5 3.5h5v6.5l4 2.5v3h-13v-3l4-2.5z"), A("M4 20h16")],
        // Protect tab.
        "signPanel" => &[S("M4 3.5h11l4 4v13H4z M15 3.5v4h4"), A("M7 13.5c1.5-3 3-4.5 3.5-3.5s-1.5 4 0 4.5 2-2 3-2 .5 1.5 2.5 1.5 M7 17.5h10")],
        _ => &[S("M5 5h14v14H5z")],
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn editor_icons_are_drawn() {
        for name in [
            "font",
            "fontSize",
            "fontColor",
            "bold",
            "italic",
            "alignLeft",
            "alignCenter",
            "alignRight",
            "alignJustify",
            "done",
            "signPanel",
            "fmtUnderline",
            "fmtStrike",
            "callout",
            "drawing",
            "stamp",
        ] {
            let parts = super::shape(name);
            assert!(!parts.is_empty(), "{name}");
            assert!(!std::ptr::eq(parts, super::shape("?")), "{name} is drawn");
        }
    }
}
