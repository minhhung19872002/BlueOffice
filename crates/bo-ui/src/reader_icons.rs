//! BlueReader icons: our own line drawings on a 24-unit grid (clean-room; sizes and stroke
//! weight follow the reference reader's toolbar by measurement, the shapes are ours). Strokes
//! use the ink colour, highlights the accent.
use crate::Painter;
use bo_render::{PathSeg, Rgba};

/// Colours of a reader icon.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ink {
    pub ink: Rgba,
    pub accent: Rgba,
}

pub(crate) enum Part {
    /// Stroked outline in ink.
    S(&'static str),
    /// Stroked outline in the accent.
    A(&'static str),
    /// Filled in ink.
    F(&'static str),
    /// Filled in the accent.
    FA(&'static str),
    /// Stroked in the text-markup colour (the reference's marks are yellow-orange), grey when
    /// the icon is drawn disabled.
    Y(&'static str),
}

use Part::{A, F, FA, S, Y};

/// Colour of text-markup marks in the toolbar (measured on the reference: 249, 177, 10).
const MARK: Rgba = Rgba(249, 177, 10, 255);
/// Ink of a disabled icon.
const DISABLED: Rgba = Rgba(175, 175, 176, 255);

fn shape(name: &str) -> &'static [Part] {
    match name {
        "hand" => &[S(
            "M8 13V5.5a1.5 1.5 0 0 1 3 0V11 M11 11V3.5a1.5 1.5 0 0 1 3 0V11 M14 11V4.5a1.5 1.5 0 0 1 3 0V12 M17 12V7.5a1.5 1.5 0 0 1 3 0V15c0 4-2.5 6.5-6.5 6.5h-1c-2.5 0-4-1-5.5-3L4 13.5a1.6 1.6 0 0 1 2.4-2.1L8 13",
        )],
        "select" => &[
            S("M4 4h10 M9 4v16 M7 20h4"),
            A("M14 12l7 3.5-3 1 2 3.5-1.6 0.9-2-3.5-2.4 2z"),
        ],
        "snapshot" => &[
            S("M6 2v16h16 M2 6h16v16"),
            A("M8 16l3.5-4 2.5 2.5 2-2 2 3.5"),
            FA("M9 8.5a1.2 1.2 0 1 0 0.01 0z"),
        ],
        "clipboard" => &[
            S("M8 4H5.5v18h13V4H16 M8 3h8v3H8z"),
            S("M8.5 10h7 M8.5 13.5h7 M8.5 17h7"),
        ],
        // Clipboard menu (ours: blade pair over two rings, stacked sheets, sheet on a board,
        // dashed frame round text lines).
        "cut" => &[
            S("M7 18a2.8 2.8 0 1 0 0.01 0z M17 18a2.8 2.8 0 1 0 0.01 0z"),
            S("M8.8 15.8L16 3 M15.2 15.8L8 3"),
            FA("M12 9.6a1 1 0 1 0 0.01 0z"),
        ],
        "copy" => &[S("M5 16.5V3.5h9.5"), S("M8.5 7h10.5v14H8.5z"), A("M11 11h6 M11 14h6 M11 17h4")],
        "paste" => &[
            S("M8 4.5H4.5v16H10 M8 3h6v3H8z M14 4.5h3.5V9"),
            A("M11.5 10.5h9v10.5h-9z M13.5 14h5 M13.5 17h5"),
        ],
        "selectAll" => &[
            S("M3 3h4 M10 3h4 M17 3h4 M3 21h4 M10 21h4 M17 21h4 M3 3v4 M3 10v4 M3 17v4 M21 3v4 M21 10v4 M21 17v4"),
            A("M7.5 8h9 M7.5 12h9 M7.5 16h6"),
        ],
        "zoom" => &[
            S("M10.5 3a7.5 7.5 0 1 0 0.01 0z M16 16l5.5 5.5"),
            A("M10.5 7v7 M7 10.5h7"),
        ],
        "zoomOut" => &[
            S("M10.5 3a7.5 7.5 0 1 0 0.01 0z M16 16l5.5 5.5"),
            A("M7 10.5h7"),
        ],
        "fit" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M12 8v9 M9.5 10.5L12 8l2.5 2.5 M9.5 14.5L12 17l2.5-2.5")],
        "fitWidth" => &[S("M5 2.5h14v19H5z"), A("M7.5 12h9 M10 9.5L7.5 12l2.5 2.5 M14 9.5l2.5 2.5-2.5 2.5")],
        "actual" => &[S("M5 2.5h14v19H5z"), A("M10 8.5l2-1.5v10 M10 17h4")],
        "rotate" => &[
            S("M3 9.5h11v12H3z M8 3.5h11v11"),
            A("M17 18.5h4.5v-4 M21.5 18.5c-1 0-2 0-2.5-.5"),
        ],
        "rotateLeft" => &[S("M7 9h12v12H7z"), A("M3 11a8 8 0 0 1 8-8h3 M11.5 0.5L14 3l-2.5 2.5")],
        "rotateRight" => &[S("M5 9h12v12H5z"), A("M21 11a8 8 0 0 0-8-8h-3 M12.5 0.5L10 3l2.5 2.5")],
        "typewriter" => &[S("M3 4h11 M8.5 4v16 M6 20h5"), A("M18 4v16 M16 4h4 M16 20h4")],
        "highlight" => &[S("M3 5.5h18 M3 18.5h18"), Y("M3 10.5h18 M3 12h18 M3 13.5h18")],
        "underline" => &[S("M6 17L12 3l6 14 M8.2 12h7.6"), Y("M4 21h16")],
        "strikeout" => &[S("M6 19L12 3l6 16 M8.6 15h6.8"), Y("M3 10.5h18")],
        "squiggly" => &[S("M6 17L12 3l6 14 M8.2 12h7.6"), Y("M3 21l2-1.5 2 1.5 2-1.5 2 1.5 2-1.5 2 1.5 2-1.5 2 1.5")],
        "note" => &[
            S("M3 4h18v13H11l-5 4.5V17H3z"),
            A("M7 8.5h10 M7 12.5h7"),
        ],
        "textbox" => &[S("M2.5 5h19v14h-19z"), A("M8 15.5l4-8 4 8 M9.3 13h5.4")],
        "rectangle" => &[A("M3.5 6h17v12h-17z")],
        "ellipse" => &[A("M12 6c5 0 8.5 2.7 8.5 6s-3.5 6-8.5 6-8.5-2.7-8.5-6 3.5-6 8.5-6z")],
        "pencil" => &[S("M15.5 4.5l4 4L9 19l-5 1 1-5z"), A("M3 22c3-2 5 1 8-1")],
        "file" | "blankPage" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4")],
        "bookmark" => &[S("M6 3h12v18l-6-4.5L6 21z")],
        "pages" => &[S("M8 2.5h9l3 3v13H8z M17 2.5v3h3 M5 6v15.5h11")],
        "comments" => &[
            S("M3.5 4.5h17v12H12l-4.5 4v-4h-4z"),
            F("M8 10.5a1 1 0 1 0 .01 0z M12 10.5a1 1 0 1 0 .01 0z M16 10.5a1 1 0 1 0 .01 0z"),
        ],
        "attach" => &[S(
            "M17 7.5l-7.5 7.5a2 2 0 0 0 2.8 2.8l8-8a4 4 0 0 0-5.6-5.6l-8 8a6 6 0 0 0 8.5 8.5l6.5-6.5",
        )],
        // Round 8 panels (ours: stacked plates, clip with a plus, tree rows with a plus /
        // minus box, framed arrows, bars by length, a funnel).
        "layers" => &[S("M12 3.5l8.5 4.5-8.5 4.5-8.5-4.5z"), A("M3.5 12.5l8.5 4.5 8.5-4.5"), S("M3.5 16.5l8.5 4.5 8.5-4.5")],
        "attachAdd" => &[
            S("M13.5 7l-6 6a1.8 1.8 0 0 0 2.5 2.5l6.5-6.5a3.5 3.5 0 0 0-5-5L5 10.5a5.2 5.2 0 0 0 7.4 7.4"),
            A("M18.5 14.5v7 M15 18h7"),
        ],
        "expandAll" => &[S("M3 4h8v8H3z M14 8h7 M3 16h18 M3 20.5h18"), A("M5 8h4 M7 6v4")],
        "collapseAll" => &[S("M3 4h8v8H3z M14 8h7 M3 16h18 M3 20.5h18"), A("M5 8h4")],
        "prevItem" => &[S("M4 4h16v16H4z"), A("M14 8l-4 4 4 4")],
        "nextItem" => &[S("M4 4h16v16H4z"), A("M10 8l4 4-4 4")],
        "sortAZ" => &[S("M3 6h11 M3 11h8 M3 16h5"), A("M18 4v16 M15 17l3 3 3-3")],
        "exitFull" => &[S("M9 3v6H3 M15 3v6h6 M9 21v-6H3 M15 21v-6h6")],
        "mail" => &[S("M3 5.5h18v13H3z"), A("M3.5 6l8.5 7 8.5-7")],
        "funnel" => &[S("M3.5 4.5h17l-6.5 8v6.5l-4 2v-8.5z")],
        "first" => &[S("M12 6l-6 6 6 6 M18 6l-6 6 6 6")],
        "prev" => &[S("M15 5l-7 7 7 7")],
        "next" => &[S("M9 5l7 7-7 7")],
        "last" => &[S("M6 6l6 6-6 6 M12 6l6 6-6 6")],
        "prevView" => &[S("M7 4h10v4 M7 4v12h4"), S("M13 10h8v11h-8z"), A("M3 14l3-3 3 3 M6 11v8")],
        "nextView" => &[S("M3 4h10v12H3z"), S("M17 8h4v13H11v-3"), A("M15 7l3 3-3 3 M8 10h10")],
        "single" => &[S("M6 3h12v18H6z"), S("M9 7.5h6 M9 11h6 M9 14.5h6")],
        "continuous" => &[S("M6 1v5h12V1 M6 23v-13h12v13"), S("M9 13.5h6 M9 17h6")],
        "facing" => &[S("M2.5 4h8.5v16H2.5z M13 4h8.5v16H13z"), S("M5 8h4 M5 11h4 M15 8h4 M15 11h4")],
        "contFacing" => &[S("M3 2v6h8V2 M13 2v6h8V2 M3 22v-10h8v10 M13 22v-10h8v10")],
        "fullscreen" => &[S("M3 9V3h6 M15 3h6v6 M21 15v6h-6 M9 21H3v-6")],
        "readMode" => &[S("M2.5 5.5c3.2-1.5 6.5-1.3 9.5 1 3-2.3 6.3-2.5 9.5-1v13c-3.2-1.5-6.5-1.3-9.5 1-3-2.3-6.3-2.5-9.5-1z"), A("M12 6.5v13")],
        "search" => &[S("M10.5 3.5a7 7 0 1 0 0.01 0z M15.5 15.5l5 5")],
        // Comment tab drawing tools (round 3).
        "callout" => &[S("M9 3.5h12v9H9z"), A("M9 10l-6 9 M3 19l1-3.5 M3 19l3.4-1.3"), S("M11.5 6.5h7 M11.5 9.5h5")],
        "line" => &[A("M4 20L20 4")],
        "arrow" => &[A("M4 20L19 5 M12 4.5h7.5V12")],
        "polygon" => &[A("M12 3l8.5 6.5-3.5 10.5h-10L3.5 9.5z")],
        "polyline" => &[A("M3 18l5-9 5 6 4-10 4 5")],
        "stamp" => &[S("M9.5 3.5h5l-1 8h-3z M4 12.5h16v4H4z"), A("M5 20.5h14")],
        "measure" => &[S("M2.5 15.5L15.5 2.5l6 6-13 13z"), A("M6.5 11.5l2 2 M9.5 8.5l2 2 M12.5 5.5l2 2")],
        // View tab additions (round 3).
        "reverseView" => &[S("M3 5h7.5v14H3z M13.5 5H21v14h-7.5z"), A("M8 12h8 M14 9.5l2.5 2.5-2.5 2.5")],
        "wordCount" => &[S("M3.5 4h17v16h-17z"), A("M6.5 8.5h5 M6.5 12h8 M6.5 15.5h4"), S("M16 14v4.5 M14 16.2h4")],
        "viewSetting" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M12 11.5a2.5 2.5 0 1 0 .01 0z M12 8.5v1 M12 15.5v1 M8.5 13h1 M14.5 13h1")],
        "night" => &[F("M14 3a9 9 0 1 0 7 13.5A7.5 7.5 0 0 1 14 3z")],
        "textViewer" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M8 10h8 M8 13h8 M8 16h5")],
        "read" => &[S("M3.5 9h4l5-4.5v15l-5-4.5h-4z"), A("M15.5 9a4 4 0 0 1 0 6 M18 6.5a7.5 7.5 0 0 1 0 11")],
        "split" => &[S("M3.5 3.5h17v17h-17z"), A("M12 3.5v17 M3.5 12h17")],
        "replaceText" => &[S("M4 17L9.5 3 15 17 M6.3 13h6.4"), Y("M2.5 8.5h14 M14.5 21h7 M19 18.5l2.5 2.5-2.5 2.5")],
        "insertText" => &[S("M4 17L9.5 3 15 17 M6.3 13h6.4"), Y("M18.5 15.5v7 M15 19h7")],
        // Bookmarks panel tools (round 5).
        "bmOptions" => &[S("M3.5 4.5h17v15h-17z M7 9h10 M7 12.5h10 M7 16h6")],
        "bmFind" => &[S("M4.5 3h9v16l-4.5-3.5L4.5 19z"), A("M16.5 12a3 3 0 1 0 .01 0z M18.7 14.2l2.8 2.8")],
        "textLarger" => &[S("M2.5 19L7.5 5l5 14 M4.5 14.5h6"), A("M18 5v6 M15 8h6")],
        "textSmaller" => &[S("M2.5 19L7.5 5l5 14 M4.5 14.5h6"), A("M15 8h6")],
        // Fill & Sign marks (round 5).
        "check" => &[S("M4 12.5l5 5L20 6.5")],
        "cross" => &[S("M5.5 5.5l13 13 M18.5 5.5l-13 13")],
        "dot" => &[F("M12 8.5a3.5 3.5 0 1 0 .01 0z")],
        // Comment exchange rows and the small icon column (round 5).
        "importComments" => &[S("M3.5 6.5h17v10H11l-4.5 4v-4h-3z"), A("M12 1.5v9 M9 7.5l3 3 3-3")],
        "exportComments" => &[S("M3.5 6.5h17v10H11l-4.5 4v-4h-3z"), A("M12 12V2 M9 4.5l3-3 3 3")],
        "commentList" => &[S("M3.5 3.5h17v12H11l-4.5 4v-4h-3z"), S("M7 7.5h10 M7 11h7")],
        "commentOptions" => &[S("M3.5 3.5h17v12H11l-4.5 4v-4h-3z"), A("M7 7.5h10 M7 11.5h10"), F("M13 6.5h2v2h-2z M9 10.5h2v2H9z")],
        "pin" => &[S("M12 3.5a3.5 3.5 0 1 0 .01 0z M12 10.5v11")],
        "searchHighlight" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M11 9.5a3 3 0 1 0 .01 0z M13.2 14.7l3 3")],
        "summarize" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M8 10l1.5 1.5L12 9 M8 15l1.5 1.5L12 14 M13.5 11h3 M13.5 16h3")],
        "eraser" => &[S("M9 20.5L3 14.5 13.5 4l7 7-9.5 9.5z M20.5 20.5H9"), A("M8 9.5l6.5 6.5")],
        "areaHighlight" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M8 9h8v7H8z")],
        "customStamp" => &[S("M9.5 3.5h5l-1 8h-3z M4 12.5h16v4H4z"), A("M17 17.5a3 3 0 1 0 .01 0z M17 19v2 M16 20h2")],
        "formToSheet" => &[S("M3.5 3.5h11v13h-11z M3.5 8h11 M9 3.5v13"), A("M14 15l3 3-3 3 M10 18h7")],
        "import" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M2 13h9 M8 10l3 3-3 3")],
        "export" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M10 13h11 M18 10l3 3-3 3")],
        "timestamp" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M12 11a4 4 0 1 0 .01 0z M12 13v2l1.5 1")],
        "validate" => &[S("M14 3.5l4 4-8 8H6v-4z"), A("M12 18l2.5 2.5 5-5")],
        "reflow" => &[S("M5 2.5h14v19H5z"), A("M8 7h8 M8 10.5h8 M8 14h5.5 M13.5 14l2 2-2 2 M8 18h7.5")],
        "autoScroll" => &[S("M7 2.5h10v19H7z"), A("M12 6v9 M9 12l3 3 3-3 M9 18h6")],
        "cover" => &[S("M2.5 7h6v11h-6z M10 4h5.5v14H10z M15.5 4H21v14h-5.5")],
        "ruler" => &[S("M2.5 7.5h19v9h-19z"), A("M6 7.5v3 M9.5 7.5v4.5 M13 7.5v3 M16.5 7.5v4.5 M20 7.5v3")],
        "grid" => &[S("M3.5 3.5h17v17h-17z"), A("M9.2 3.5v17 M14.8 3.5v17 M3.5 9.2h17 M3.5 14.8h17")],
        "close" => &[S("M5 5l14 14 M19 5L5 19")],
        "arrowDown" => &[F("M7 10h10l-5 5z")],
        "arrowRight" => &[F("M10 7v10l5-5z")],
        "chevronDown" => &[S("M6 9l6 6 6-6")],
        "chevronRight" => &[S("M9 6l6 6-6 6")],
        "chevronUp" => &[S("M6 15l6-6 6 6")],
        "minus" => &[S("M5 12h14")],
        "plus" => &[S("M5 12h14 M12 5v14")],
        "open" => &[S("M2.5 5.5v14h16l3-9H7l-3 9 M2.5 5.5h6l2 2h7v3")],
        "save" => &[S("M4 3.5h13l3 3v14H4z M7.5 3.5v5h8v-5 M7.5 20.5v-6h9v6")],
        "print" => &[S("M6.5 9V3.5h11V9 M6.5 17H3.5V9h17v8h-3 M6.5 14h11v7h-11z")],
        "undo" => &[S("M9 5L4 10l5 5 M4 10h10a6 6 0 0 1 0 12h-3")],
        "redo" => &[S("M15 5l5 5-5 5 M20 10H10a6 6 0 0 0 0 12h3")],
        "properties" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), S("M8 11h8 M8 14.5h8 M8 18h5")],
        "bell" => &[S("M6 17V11a6 6 0 0 1 12 0v6l2 2H4z M10 21h4")],
        "info" => &[S("M12 2.5a9.5 9.5 0 1 0 0.01 0z M12 10.5v7"), F("M12 6.5a1.1 1.1 0 1 0 .01 0z")],
        // Help tab (round 9): a console window with a prompt; a page with a star (default).
        "cmdLine" => &[S("M2.5 4.5h19v15h-19z M2.5 8h19"), A("M6 11.5l3 2.5-3 2.5 M11 16.5h5")],
        "defaultApp" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M12 9.5l1.5 3 3.2.4-2.4 2.2.7 3.2-3-1.7-3 1.7.7-3.2-2.4-2.2 3.2-.4z")],
        "app" | "appEditor" => &[],
        "editText" => &[A("M3 4.5h18v15H3z"), S("M7.5 8.5h9 M12 8.5v8 M10 16.5h4")],
        "editObject" => &[S("M3.5 3.5h11v11h-11z"), A("M12 12l8.5 3.5-3.5 1.2 3.5 3.5-1.4 1.4-3.5-3.5-1.2 3.5z")],
        "insertPage" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M12 10v8 M8 14h8")],
        "deletePage" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M9 11l6 6 M15 11l-6 6")],
        "extractPage" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M12 9v9 M8.5 14.5L12 18l3.5-3.5")],
        "reversePages" => &[S("M3.5 3h8v11h-8z M12.5 10h8v11h-8z"), A("M15 3.5h3.5V7 M18.5 3.5L14 8 M9 20.5H5.5V17 M5.5 20.5L10 16")],
        "splitPages" => &[S("M5 2.5h14v7H5z M5 14.5h14v7H5z"), A("M2 12h3 M8 12h3 M13 12h3 M19 12h3")],
        "combine" => &[S("M3 3h7.5v10H3z M13.5 3H21v10h-7.5z"), A("M6.5 13v3h11v-3 M12 16v5 M9.5 18.5L12 21l2.5-2.5")],
        "createPdf" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M8.5 12.5h7 M8.5 15.5h7 M8.5 18.5h4")],
        "toWord" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M7.5 10.5l1.5 7 3-5 3 5 1.5-7")],
        "toExcel" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M8 10h8v8H8z M8 14h8 M12 10v8")],
        "toSlides" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M7.5 10h9v6h-9z M12 16v2.5 M10 18.5h4")],
        "toImage" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M7.5 18l3-3.5 2 2 1.5-1.5 2.5 3"), FA("M10 10.5a1.2 1.2 0 1 0 .01 0z")],
        "addText" => &[S("M4 5h12 M10 5v14 M7.5 19h5"), A("M18.5 14v7 M15 17.5h7")],
        "addImage" => &[S("M2.5 4.5h15v12h-15z M4.5 14.5l3.5-4 2.5 2.5 2-2 2.5 3.5"), A("M19 15v7 M15.5 18.5h7")],
        "redact" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), FA("M7.5 10h9v2.5h-9z M7.5 14.5h6v2.5h-6z")],
        "applyRedact" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), FA("M7.5 10h9v2.5h-9z"), A("M8 16.5l2.5 2.5 5.5-5.5")],
        "sign" => &[S("M3 20.5h18"), A("M4 16.5c2.5-7 5-10 6-8.5s-2.5 7.5 0 8.5 3.5-4.5 5-4.5 1 3 4 3")],
        "fillSign" => &[S("M14.5 3.5l5 5L9 19H4v-5z M12 6l5 5"), A("M3 22h18")],
        "secure" => &[S("M5.5 10.5h13v11h-13z M8.5 10.5V7a3.5 3.5 0 0 1 7 0v3.5"), FA("M12 14.5a1.6 1.6 0 1 0 .01 0z")],
        "unsecure" => &[S("M5.5 10.5h13v11h-13z M8.5 10.5V7a3.5 3.5 0 0 1 6.8-1.2"), A("M10 16h4")],
        "flatten" => &[S("M3 8l9-5 9 5-9 5z"), A("M3 12l9 5 9-5 M3 16l9 5 9-5")],
        "resetForm" => &[S("M5 2.5h10l4 4v15H5z M15 2.5v4h4"), A("M8.5 15a3.5 3.5 0 1 0 1-2.5 M8.5 10.5v2.5H11")],
        "formFields" => &[S("M3 5h18v5H3z M3 14h18v5H3z"), FA("M5 6.5h7v2H5z M5 15.5h4v2H5z")],
        "organize" => &[S("M3 3h7v8H3z M14 3h7v8h-7z M3 13h7v8H3z"), A("M14 13h7v8h-7z")],
        _ => crate::editor_icons::shape(name),
    }
}

/// Draw reader icon `name` into a `size`-pixel square at (`x`, `y`).
pub(crate) fn draw(p: &mut dyn Painter, name: &str, x: f32, y: f32, size: f32, c: Ink) {
    draw_weight(p, name, x, y, size, c, 1.5);
}

/// Menu icon of the reader (`popup::IconFn`): toolbar ink and accent, grey when `dim`.
pub(crate) fn menu_icon(p: &mut dyn Painter, name: &str, x: f32, y: f32, size: f32, dim: bool) {
    let c = if dim {
        Ink {
            ink: DISABLED,
            accent: DISABLED,
        }
    } else {
        Ink {
            ink: Rgba(51, 51, 51, 255),
            accent: crate::theme::Theme::light("reader").accent,
        }
    };
    draw(p, name, x, y, size, c);
}

/// [`draw`] with strokes `stroke` grid units wide (at least one device pixel).
pub(crate) fn draw_weight(
    p: &mut dyn Painter,
    name: &str,
    x: f32,
    y: f32,
    size: f32,
    c: Ink,
    stroke: f32,
) {
    let k = size / 24.0;
    // About the 1.5 / 24 stroke of the reference toolbar, at least one device pixel.
    let width = (stroke * k).round().max(1.0);
    let (x, y) = (x.round(), y.round());
    // Odd widths sit on pixel centres so straight lines stay solid.
    let off = if (width as i32) % 2 == 1 { 0.5 } else { 0.0 };
    if name == "app" {
        app_mark(p, x, y, size, c);
        return;
    }
    if name == "appEditor" {
        editor_mark(p, x, y, size, c);
        return;
    }
    for part in shape(name) {
        let (d, fill, color) = match part {
            S(d) => (*d, false, c.ink),
            A(d) => (*d, false, c.accent),
            F(d) => (*d, true, c.ink),
            FA(d) => (*d, true, c.accent),
            Y(d) => (*d, false, if c.ink == DISABLED { DISABLED } else { MARK }),
        };
        let segs: Vec<PathSeg> = crate::icons::parse_path(d)
            .into_iter()
            .map(|s| place(s, x, y, k, if fill { 0.0 } else { off }))
            .collect();
        if fill {
            p.path(&segs, Some(color), None);
        } else {
            p.path(&segs, None, Some((color, width)));
        }
    }
}

fn place(s: PathSeg, x: f32, y: f32, k: f32, off: f32) -> PathSeg {
    let t = |a: f32, b: f32| ((x + a * k).round() + off, (y + b * k).round() + off);
    let f = |a: f32, b: f32| (x + a * k + off, y + b * k + off);
    match s {
        PathSeg::M(a, b) => {
            let (u, v) = t(a, b);
            PathSeg::M(u, v)
        }
        PathSeg::L(a, b) => {
            let (u, v) = t(a, b);
            PathSeg::L(u, v)
        }
        PathSeg::Q(a, b, c, d) => {
            let (u, v) = f(a, b);
            let (w, z) = f(c, d);
            PathSeg::Q(u, v, w, z)
        }
        PathSeg::C(a, b, c, d, e, g) => {
            let (u, v) = f(a, b);
            let (w, z) = f(c, d);
            let (m, n) = f(e, g);
            PathSeg::C(u, v, w, z, m, n)
        }
        PathSeg::Z => PathSeg::Z,
    }
}

/// BlueReader's own app mark: an accent rounded square with a white page and folded corner.
fn app_mark(p: &mut dyn Painter, x: f32, y: f32, size: f32, c: Ink) {
    let k = size / 24.0;
    let r = 4.0 * k;
    let (w, h) = (size, size);
    let rounded = vec![
        PathSeg::M(x + r, y),
        PathSeg::L(x + w - r, y),
        PathSeg::Q(x + w, y, x + w, y + r),
        PathSeg::L(x + w, y + h - r),
        PathSeg::Q(x + w, y + h, x + w - r, y + h),
        PathSeg::L(x + r, y + h),
        PathSeg::Q(x, y + h, x, y + h - r),
        PathSeg::L(x, y + r),
        PathSeg::Q(x, y, x + r, y),
        PathSeg::Z,
    ];
    p.path(&rounded, Some(c.accent), None);
    let white = Rgba(255, 255, 255, 255);
    let page: Vec<PathSeg> = crate::icons::parse_path("M7 4.5h7l3.5 3.5v11.5H7z")
        .into_iter()
        .map(|s| place(s, x, y, k, 0.0))
        .collect();
    p.path(&page, Some(white), None);
    let fold: Vec<PathSeg> = crate::icons::parse_path("M14 4.5V8h3.5z")
        .into_iter()
        .map(|s| place(s, x, y, k, 0.0))
        .collect();
    p.path(&fold, Some(c.accent), None);
    let lines: Vec<PathSeg> = crate::icons::parse_path("M9.5 11.5h5.5 M9.5 14.5h5.5 M9.5 17h3.5")
        .into_iter()
        .map(|s| place(s, x, y, k, 0.5))
        .collect();
    p.path(&lines, None, Some((c.accent, (1.4 * k).round().max(1.0))));
}

/// BlueEditor's app mark: a white rounded square with an accent page and pen (for a coloured
/// title bar).
fn editor_mark(p: &mut dyn Painter, x: f32, y: f32, size: f32, c: Ink) {
    let k = size / 24.0;
    let r = 4.0 * k;
    let (w, h) = (size, size);
    let rounded = vec![
        PathSeg::M(x + r, y),
        PathSeg::L(x + w - r, y),
        PathSeg::Q(x + w, y, x + w, y + r),
        PathSeg::L(x + w, y + h - r),
        PathSeg::Q(x + w, y + h, x + w - r, y + h),
        PathSeg::L(x + r, y + h),
        PathSeg::Q(x, y + h, x, y + h - r),
        PathSeg::L(x, y + r),
        PathSeg::Q(x, y, x + r, y),
        PathSeg::Z,
    ];
    p.path(&rounded, Some(Rgba(255, 255, 255, 255)), None);
    let page: Vec<PathSeg> = crate::icons::parse_path("M6.5 4.5h7.5l3.5 3.5v11.5h-11z")
        .into_iter()
        .map(|s| place(s, x, y, k, 0.0))
        .collect();
    p.path(&page, Some(c.accent), None);
    let pen: Vec<PathSeg> = crate::icons::parse_path("M15.5 9.5l2.5 2.5-6 6H9.5v-2.5z")
        .into_iter()
        .map(|s| place(s, x, y, k, 0.0))
        .collect();
    p.path(&pen, Some(Rgba(255, 255, 255, 255)), None);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Count(usize);
    impl Painter for Count {
        fn clear(&mut self, _: Rgba) {}
        fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba) {}
        fn page(&mut self, _: usize, _: i32, _: i32) {}
        fn text(&mut self, _: &str, _: f32, _: f32, _: crate::TextStyle, _: Rgba) {}
        fn path(&mut self, segs: &[PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {
            assert_ne!(segs.len(), 0);
            self.0 += 1;
        }
        fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn pop_clip(&mut self) {}
    }

    #[test]
    fn every_icon_draws() {
        let ink = Ink {
            ink: Rgba(0, 0, 0, 255),
            accent: Rgba(200, 0, 0, 255),
        };
        for name in [
            "editText",
            "editObject",
            "insertPage",
            "deletePage",
            "extractPage",
            "reversePages",
            "splitPages",
            "combine",
            "blankPage",
            "createPdf",
            "toWord",
            "toExcel",
            "toSlides",
            "toImage",
            "addText",
            "addImage",
            "redact",
            "applyRedact",
            "sign",
            "fillSign",
            "secure",
            "unsecure",
            "flatten",
            "resetForm",
            "formFields",
            "organize",
            "check",
            "cross",
            "dot",
            "importComments",
            "exportComments",
            "commentList",
            "commentOptions",
            "pin",
            "squiggly",
            "replaceText",
            "insertText",
            "hand",
            "select",
            "snapshot",
            "clipboard",
            "cut",
            "copy",
            "paste",
            "selectAll",
            "zoom",
            "fit",
            "rotate",
            "typewriter",
            "highlight",
            "underline",
            "strikeout",
            "note",
            "textbox",
            "rectangle",
            "bookmark",
            "pages",
            "comments",
            "attach",
            "first",
            "prev",
            "next",
            "last",
            "single",
            "continuous",
            "facing",
            "contFacing",
            "fullscreen",
            "search",
            "open",
            "save",
            "print",
            "undo",
            "redo",
            "app",
        ] {
            let mut c = Count(0);
            draw(&mut c, name, 0.0, 0.0, 24.0, ink);
            assert!(c.0 > 0, "{name}");
        }
    }
}
