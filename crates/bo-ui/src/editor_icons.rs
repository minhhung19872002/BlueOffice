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
        "fmtSuper" => &[S("M3 8l9 12 M12 8l-9 12"), A("M15 5.5a2 2 0 1 1 3.5 1.5L15 11h4.5")],
        "fmtSub" => &[S("M3 4l9 12 M12 4L3 16"), A("M15 15a2 2 0 1 1 3.5 1.5L15 20.5h4.5")],
        "fmtIndent" => &[S("M3 4h18 M11 9h10 M11 14h10 M3 19h18 M3 8.5l4 3-4 3z")],
        "fmtOutdent" => &[S("M3 4h18 M11 9h10 M11 14h10 M3 19h18 M7 8.5l-4 3 4 3z")],
        "bullets" => &[S("M9 6h12 M9 12h12 M9 18h12"), F("M4.5 4.5a1.5 1.5 0 1 0 .01 0z M4.5 10.5a1.5 1.5 0 1 0 .01 0z M4.5 16.5a1.5 1.5 0 1 0 .01 0z")],
        "numbering" => &[S("M10 6h11 M10 12h11 M10 18h11 M4 4.5l1.5-1v5 M3 13.5a1.5 1.5 0 1 1 2.5 1.2L3 17h3 M3 19.5h3l-1.5 1.5a1.5 1.5 0 1 1-1.5 1.5")],
        "fmtBaseline" => &[S("M5 4h14 M12 4v11 M3 20h18 M8 17l4 3 4-3")],
        "lineSpacing" => &[S("M10 5h11 M10 10h11 M10 15h11 M10 20h11 M5 4v16 M3 6l2-2 2 2 M3 18l2 2 2-2")],
        "paraSpacing" => &[S("M3 3h18 M3 21h18 M8 8h13 M8 12h13 M8 16h13 M5 7v10")],
        "charScale" => &[S("M3 12h18 M3 6v12 M21 6v12 M6 9l-3 3 3 3 M18 9l3 3-3 3")],
        "charSpacing" => &[S("M3 14l4-10h1l4 10 M4.5 10.5h6 M14 14V4h3.5a2.5 2.5 0 0 1 0 5H14 M14 9h4a2.5 2.5 0 0 1 0 5h-4 M3 18h18 M3 16v4 M21 16v4")],
        "wordSpacing" => &[S("M3 6h6 M15 6h6 M3 10h6 M15 10h6 M3 18h18 M3 16v4 M21 16v4 M10 14l2-2 2 2")],
        // Edit tab.
        "linkJoin" => &[S("M4 4h7 M7.5 4v9 M13 4h7 M16.5 4v9"), A("M6 20h5a2 2 0 0 0 0-4H9 M18 16h-5a2 2 0 0 0 0 4h2")],
        "spell" => &[S("M3 12l3-8h1l3 8 M4 9h5 M12 12V4h3a2 2 0 0 1 0 4h-3 M12 8h3.5a2 2 0 0 1 0 4H12"), A("M8 18l3 3 7-7")],
        "searchReplace" => &[S("M3 4l3-1v6 M13 14l4-10h1l4 10 M14.5 10.5h6"), A("M4 14v3a3 3 0 0 0 3 3h5 M10 18l2 2-2 2")],
        "addShapes" => &[S("M3 3.5h10v10H3z"), A("M16.5 10.5a5 5 0 1 0 .01 0z")],
        "reflow" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4 M8 11h8 M8 14h6"), A("M8 17h8 M14 15.5l2 1.5-2 1.5")],
        "articleBox" => &[S("M3 5h18v14H3z M12 5v14"), A("M7 9l-2 2 2 2 M17 9l2 2-2 2")],
        "webLinks" => &[S("M12 3a9 9 0 1 0 .01 0z M3 12h18 M12 3c3 3 3 15 0 18 M12 3c-3 3-3 15 0 18"), A("M15 17h4 M17 15v4")],
        "link" => &[A("M10 14l4-4 M8.5 12.5l-2 2a3 3 0 0 0 4.2 4.2l2-2 M15.5 11.5l2-2a3 3 0 0 0-4.2-4.2l-2 2")],
        "crossRef" => &[S("M4 4h9v9H4z M11 11h9v9h-9z"), A("M8 15v4h3 M16 9V5h-3")],
        "imageAnnot" => &[S("M3 4.5h15v11H3z M5 13l3.5-4 2.5 2.5 2-2 2.5 3.5"), A("M14 15.5v5l3-2.5h4")],
        "media" => &[S("M3 7h12v10H3z"), A("M15 10l6-3v10l-6-3")],
        "add3d" => &[S("M12 3l8 4.5v9L12 21l-8-4.5v-9z M4 7.5l8 4.5 8-4.5 M12 12v9")],
        // Comment tab.
        "callout" => &[S("M9 3.5h12v9H9z"), A("M9 10L3 19.5")],
        "drawing" => &[S("M3 4h9v7H3z"), A("M16.5 7.5a4 4 0 1 0 .01 0z M4 20l6-5 M14 21l7-7")],
        "stamp" => &[S("M9.5 3.5h5v6.5l4 2.5v3h-13v-3l4-2.5z"), A("M4 20h16")],
        // Form tab.
        "properties" => &[S("M4 4h16v16H4z M8 9h8 M8 13h8 M8 17h5"), A("M7 4v3 M12 4v3 M17 4v3")],
        "fieldButton" => &[S("M3 7h18v10H3z"), A("M8 12h8")],
        "fieldCheck" => &[S("M4 4h16v16H4z"), A("M7.5 12.5l3 3 6-7")],
        "fieldRadio" => &[S("M12 3.5a8.5 8.5 0 1 0 .01 0z"), FA("M12 8.5a3.5 3.5 0 1 0 .01 0z")],
        "fieldDate" => &[S("M3.5 5h17v15h-17z M3.5 9.5h17 M8 3v4 M16 3v4"), A("M7 13h2 M11 13h2 M15 13h2 M7 16.5h2 M11 16.5h2")],
        "fieldText" => &[S("M2.5 7h19v10h-19z"), A("M6 9.5v5 M5 9.5h2 M5 14.5h2")],
        "fieldCombo" => &[S("M2.5 7h19v10h-19z M16 7v10"), A("M17.5 11l1.5 1.5 1.5-1.5")],
        "fieldList" => &[S("M3 3.5h18v17H3z M6 8h12 M6 16h12"), A("M6 12h12")],
        "fieldBarcode" => &[S("M4 5v14 M7 5v14 M9 5v14 M13 5v14 M15 5v14 M19 5v14 M20.5 5v14")],
        "fieldImage" => &[S("M3 4.5h18v15H3z M5 17l4.5-5 3 3 2.5-2.5 4 4.5"), A("M9 8.5a1.5 1.5 0 1 0 .01 0z")],
        "fieldSignature" => &[S("M3 18h18"), A("M5 15c2-5 4-7.5 5-6.5s-2 5.5 0 6.5 3-3.5 4.5-3.5 1 2 3.5 2")],
        // Organize tab: page design.
        "headerFooter" => &[S("M5 2.5h14v19H5z M8 10.5h8 M8 13.5h8"), A("M8 6h8 M8 18h8")],
        "watermark" => &[S("M5 2.5h14v19H5z"), A("M8 17l8-10 M9.5 8.5l1.5-1.5 M13 17l1.5-1.5")],
        "background" => &[S("M5 2.5h14v19H5z"), A("M8 19l8-8 M8 14l8-8")],
        "bates" => &[S("M5 2.5h14v19H5z M8 6h8 M8 9h8"), A("M12 14l-.5 5 M15 14l-.5 5 M10.5 15.5h5.5 M10 17.5h5.5")],
        // Protect tab.
        "signPanel" => &[S("M4 3.5h11l4 4v13H4z M15 3.5v4h4"), A("M7 13.5c1.5-3 3-4.5 3.5-3.5s-1.5 4 0 4.5 2-2 3-2 .5 1.5 2.5 1.5 M7 17.5h10")],
        "cropPages" => &[S("M6 2v16h16 M2 6h16v16"), A("M9 9h6v6H9z")],
        "summary" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M8 11h8 M8 14h8 M8 17h5")],
        "whiteout" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M8 12h8v5H8z")],
        // Home tab: grey drawings with the accent on one detail, as the reference's.
        "edEditText" => &[S("M3 4.5h18v15H3z M7.5 8.5h9 M12 8.5v8 M10 16.5h4"), A("M1.5 3h3v3h-3z M19.5 3h3v3h-3z M1.5 18h3v3h-3z M19.5 18h3v3h-3z")],
        "edRotatePages" => &[S("M5 6.5h10l4 4v11H5z M15 6.5v4h4"), A("M8 4a6 6 0 0 1 9-1.5 M17.5 0.5v2.5H15")],
        "edInsertPage" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M2 14h9 M8.5 11.5L11 14l-2.5 2.5")],
        "scanner" => &[S("M3 13h18v6H3z M5 13l2-9h10l2 9 M6 16h8")],
        "ocr" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4 M8 12h8 M8 15h8 M8 18h5")],
        // Organize tab, round 4 (row buttons are drawn at 22 px: few, plain strokes).
        "movePages" => &[S("M4 3h9v12H4z"), A("M16 9h5 M19 7l2 2-2 2 M12 19h9 M19 17l2 2-2 2")],
        "duplicatePages" => &[S("M3 3h10v13H3z M8 8h10v13H8z"), A("M13 12v6 M10 15h6")],
        "replacePages" => &[S("M3 3h8v10H3z M13 11h8v10h-8z"), A("M15 3h3a2 2 0 0 1 2 2v3 M18 6.5l2 2 2-2 M9 21H6a2 2 0 0 1-2-2v-3 M2 17.5l2-2 2 2")],
        "swapPages" => &[S("M3 5h7v14H3z M14 5h7v14h-7z"), A("M8 2.5h8 M14.5 1l1.5 1.5L14.5 4 M16 21.5H8 M9.5 20L8 21.5 9.5 23")],
        "interleave" => &[S("M3 3h7v8H3z M14 13h7v8h-7z"), A("M3 14h7v7H3z M14 3h7v7h-7z")],
        "rearrange" => &[S("M3 4h7v16H3z M14 4h7v16h-7z"), A("M6.5 8v8 M17.5 8v8")],
        "resizePages" => &[S("M5 2.5h14v19H5z"), A("M8 8h3 M8 8v3 M16 16h-3 M16 16v-3 M8 8l8 8")],
        "pageNumbers" => &[S("M5 2.5h14v19H5z M8 6h8 M8 9h8 M8 12h5"), A("M11 16.5l1.5-1.5v4.5")],
        // Convert tab, round 4.
        "fromClipboard" => &[S("M5 4h14v18H5z M9 2.5h6v3H9z"), A("M8.5 16l2.5-3 2 2 2.5-3 M8 18.5h8")],
        "fromWeb" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M12 10a4 4 0 1 0 .01 0z M8 14h8 M12 10c1.5 1.5 1.5 6.5 0 8")],
        "template" => &[S("M4 3h16v18H4z M4 8h16 M10 8v13"), A("M13 12h4 M13 15h4")],
        "exportImages" => &[S("M3 5h14v11H3z M5 14l3.5-4 2.5 2.5 2-2 2.5 3.5"), A("M14 20h7 M19 18l2 2-2 2")],
        "toOffice" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M8 11h8v8H8z M8 15h8 M12 11v8")],
        "toHtml" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M10 12l-2.5 2.5L10 17 M14 12l2.5 2.5L14 17")],
        "toOther" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M8.5 15h.01 M12 15h.01 M15.5 15h.01")],
        "recognizeText" => &[S("M3 7V3h4 M17 3h4v4 M21 17v4h-4 M7 21H3v-4"), A("M8 16l3.5-9h1L16 16 M9.5 13h5")],
        "suspect" => &[S("M3 7V3h4 M17 3h4v4 M21 17v4h-4 M7 21H3v-4"), A("M12 7v6 M12 16v.5")],
        "portfolio" => &[S("M3 7h18v13H3z M9 7V4h6v3"), A("M3 12h18")],
        "formPdf" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4 M8 11h8v3H8z"), A("M8 17h5")],
        // Comment / View / Protect / Form tabs, round 4.
        "transitions" => &[S("M3 5h8v14H3z M13 5h8v14h-8z"), A("M9 12h6 M13 10l2 2-2 2")],
        "calculator" => &[S("M5 2.5h14v19H5z M8 5.5h8v4H8z"), A("M8.5 13h.01 M12 13h.01 M15.5 13h.01 M8.5 17h.01 M12 17h.01 M15.5 17h.01")],
        "createStamp" => &[S("M9.5 5.5h5v5l4 2v3h-13v-3l4-2z M4 20h16"), A("M18 2v5 M15.5 4.5h5")],
        "compare" => &[S("M3 3h7v18H3z M14 3h7v18h-7z"), A("M5.5 8h2 M5.5 12h2 M16.5 10h2 M16.5 14h2")],
        "hiddenData" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M7.5 14c2-3 7-3 9 0-2 3-7 3-9 0z M12 13.5v1 M7 19l10-10")],
        "restrictAccess" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M9 15h6v4.5H9z M10.5 15v-1.5a1.5 1.5 0 0 1 3 0V15")],
        "calcOrder" => &[S("M4 4h12v16H4z M7 8h6 M7 12h6 M7 16h3"), A("M19 6v12 M17 16l2 2 2-2")],
        "tooltip" => &[S("M3 4h18v12H11l-4 4v-4H3z"), A("M12 7.5v.5 M12 10v4")],
        "javascript" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M11 11v5a1.5 1.5 0 0 1-3 0 M16 11.5c-.5-.5-1-.5-1.5-.5-1 0-1.5.7-1.5 1.3 0 1.7 3 1 3 3 0 .8-.7 1.7-1.7 1.7-.6 0-1.3-.3-1.6-.8")],
        // Format pane with Edit Object: Arrange (round 5; a guide line and plain bars).
        "objCenterDown" => &[S("M3 12h18 M7 5h10v14H7z")],
        "objCenterAcross" => &[S("M12 3v18 M5 7h14v10H5z")],
        "objCenterBoth" => &[S("M3 3h18v18H3z M8 8h8v8H8z M12 5v2 M12 17v2 M5 12h2 M17 12h2")],
        "objAlignLeft" => &[S("M4 3v18 M7 6h13v4H7z M7 14h8v4H7z")],
        "objAlignCenter" => &[S("M12 3v18 M5 6h14v4H5z M8 14h8v4H8z")],
        "objAlignRight" => &[S("M20 3v18 M4 6h13v4H4z M9 14h8v4H9z")],
        "objAlignTop" => &[S("M3 4h18 M6 7h4v13H6z M14 7h4v8h-4z")],
        "objAlignMiddle" => &[S("M3 12h18 M6 5h4v14H6z M14 8h4v8h-4z")],
        "objAlignBottom" => &[S("M3 20h18 M6 4h4v13H6z M14 9h4v8h-4z")],
        "objDistAcross" => &[S("M3 4v16 M21 4v16 M6 8h3v8H6z M10.5 6h3v12h-3z M15 9h3v6h-3z")],
        "objDistDown" => &[S("M4 3h16 M4 21h16 M8 6h8v3H8z M6 10.5h12v3H6z M9 15h6v3H9z")],
        "objRotateRight" => &[S("M5 10h9v10H5z M9 4.5a7 7 0 0 1 9.5 4 M19 5v4h-4")],
        "objRotateLeft" => &[S("M10 10h9v10h-9z M15 4.5a7 7 0 0 0-9.5 4 M5 5v4h4")],
        "objFlipAcross" => &[S("M12 2v20 M9.5 6L3 18h6.5z M14.5 6L21 18h-6.5z")],
        "objFlipDown" => &[S("M2 12h20 M6 9.5L18 3v6.5z M6 14.5L18 21v-6.5z")],
        // File page.
        "clock" => &[S("M12 3a9 9 0 1 0 .01 0z"), A("M12 7v5l3.5 2")],
        "computer" => &[S("M3 4h18v12H3z M9 20h6 M12 16v4")],
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
            "fieldButton",
            "fieldCheck",
            "fieldRadio",
            "fieldText",
            "fieldCombo",
            "fieldList",
            "headerFooter",
            "watermark",
            "background",
            "bates",
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
