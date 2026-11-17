//! Ribbon scaling of BlueDoc's Insert, Layout and View tabs (round 4): as the window
//! narrows, groups collapse one stage at a time in the reference's order — large buttons
//! to labeled rows, labeled rows to icon-only buttons, and at last the whole group to one
//! drop-down button — instead of the tab scrolling. At 1500 px (125 %) the tabs show the
//! collapsed layout the reference shows at that width.
use crate::catalog::Entry;
use crate::ribbon::{Group, Item};

const fn e(id: &'static str, label: &'static str) -> Entry {
    Entry { id, label }
}

/// Group buttons a collapsed group shows (one drop-down with the group's commands).
pub(crate) const GROUP_MENUS: [&str; 6] = [
    "insert.pagesGroup",
    "insert.linksGroup",
    "insert.symbolsGroup",
    "insert.headerFooterGroup",
    "layout.arrangeGroup",
    "view.windowGroup",
];

/// Collapse steps per tab, in order (a group named twice collapses twice).
pub(crate) fn scaling(tab: &str) -> &'static [&'static str] {
    match tab {
        "ribbon.tabs.insert" => &[
            "ribbon.groups.pages",
            "ribbon.groups.links",
            "docR4.groupText",
            "ribbon.groups.symbols",
            "ribbon.groups.illustrations",
            "ribbon.groups.headerFooter",
            "ribbon.groups.headerFooter",
        ],
        "ribbon.tabs.layout" => &[
            "docR4.groupArrange",
            "ribbon.groups.pageSetup",
            "docR4.groupArrange",
            "docR4.groupArrange",
        ],
        "ribbon.tabs.view" => &[
            "ribbon.groups.zoom",
            "docR4.groupWindow",
            "ribbon.groups.views",
            "docR4.groupWindow",
        ],
        _ => &[],
    }
}

/// Group `label` at collapse stage `stage`.
pub(crate) fn shrink(label: &str, stage: u8) -> Option<Group> {
    use Item::{Button as B, Labeled as S, Large as L};
    let g = |label: &'static str, large: Vec<Item>, rows: Vec<Vec<Item>>| {
        Some(Group { label, large, rows })
    };
    match (label, stage) {
        ("ribbon.groups.pages", _) => {
            g("ribbon.groups.pages", vec![L("insert.pagesGroup")], vec![])
        }
        ("ribbon.groups.links", _) => {
            g("ribbon.groups.links", vec![L("insert.linksGroup")], vec![])
        }
        ("ribbon.groups.symbols", _) => g(
            "ribbon.groups.symbols",
            vec![L("insert.symbolsGroup")],
            vec![],
        ),
        ("docR4.groupText", _) => g(
            "docR4.groupText",
            vec![L("insert.textBox")],
            vec![
                vec![B("insert.quickParts"), B("insert.signatureMenu")],
                vec![B("insert.wordArt"), B("insert.dateTime")],
                vec![B("insert.dropCap"), B("insert.object")],
            ],
        ),
        ("ribbon.groups.illustrations", _) => g(
            "ribbon.groups.illustrations",
            vec![L("insert.picture")],
            vec![
                vec![B("insert.shapes"), B("insert.smartArt")],
                vec![B("insert.icons"), B("insert.chart")],
                vec![B("insert.models3d"), B("insert.screenshot")],
            ],
        ),
        ("ribbon.groups.headerFooter", 1) => g(
            "ribbon.groups.headerFooter",
            vec![],
            vec![
                vec![B("insert.header")],
                vec![B("insert.footer")],
                vec![B("insert.pageNumber")],
            ],
        ),
        ("ribbon.groups.headerFooter", _) => g(
            "ribbon.groups.headerFooter",
            vec![L("insert.headerFooterGroup")],
            vec![],
        ),
        // Arrange: the reference's middle layout, then icon rows, then one button.
        ("docR4.groupArrange", 1) => g(
            "docR4.groupArrange",
            vec![L("layout.position"), L("layout.wrapText")],
            vec![
                vec![S("layout.bringForward"), S("layout.align")],
                vec![S("layout.sendBackward"), S("layout.group")],
                vec![S("layout.selectionPane"), S("layout.rotate")],
            ],
        ),
        ("docR4.groupArrange", 2) => g(
            "docR4.groupArrange",
            vec![L("layout.position"), L("layout.wrapText")],
            vec![
                vec![B("layout.bringForward"), B("layout.align")],
                vec![B("layout.sendBackward"), B("layout.group")],
                vec![B("layout.selectionPane"), B("layout.rotate")],
            ],
        ),
        ("docR4.groupArrange", _) => {
            g("docR4.groupArrange", vec![L("layout.arrangeGroup")], vec![])
        }
        ("ribbon.groups.pageSetup", _) => g(
            "ribbon.groups.pageSetup",
            vec![
                L("layout.margins"),
                L("layout.orientation"),
                L("layout.size"),
                L("layout.columns"),
            ],
            vec![
                vec![B("layout.breaks")],
                vec![B("layout.lineNumbers")],
                vec![B("layout.hyphenation")],
            ],
        ),
        // Zoom: One Page / Multiple Pages / Page Width as icons (the reference at 1500 px).
        ("ribbon.groups.zoom", _) => g(
            "ribbon.groups.zoom",
            vec![L("view.zoomDialog"), L("view.zoomReset")],
            vec![
                vec![B("view.onePage")],
                vec![B("view.multiplePages")],
                vec![B("view.pageWidth")],
            ],
        ),
        ("docR4.groupWindow", 1) => g(
            "docR4.groupWindow",
            vec![L("view.switchWindows")],
            vec![
                vec![B("view.newWindow"), B("view.sideBySide")],
                vec![B("view.arrangeAll"), B("view.syncScroll")],
                vec![B("view.split"), B("view.resetWindow")],
            ],
        ),
        ("docR4.groupWindow", _) => g("docR4.groupWindow", vec![L("view.windowGroup")], vec![]),
        ("ribbon.groups.views", _) => g(
            "ribbon.groups.views",
            vec![
                L("view.readMode"),
                L("view.printLayout"),
                L("view.webLayout"),
            ],
            vec![vec![B("view.outline")], vec![B("view.draft")]],
        ),
        _ => None,
    }
}

/// Fixed widths (logical px) of labeled row buttons, so the icon column after them lines
/// up as on the reference (Window: New Window / Arrange All / Split).
pub(crate) fn row_w(id: &str) -> Option<f32> {
    matches!(id, "view.newWindow" | "view.arrangeAll" | "view.split").then_some(112.0)
}

/// Entries of a collapsed group's drop-down (and of the round-4 menus added for fitting).
pub(crate) fn menu(id: &str) -> Vec<Entry> {
    match id {
        "insert.pagesGroup" => vec![
            e("insert.coverPage", "Cover Page"),
            e("insert.blankPage", "Blank Page"),
            e("insert.pageBreak", "Page Break"),
        ],
        "insert.linksGroup" => vec![
            e("insert.link", "Link"),
            e("insert.bookmarkDialog", "Bookmark"),
            e("references.crossReference", "Cross-reference"),
        ],
        "insert.symbolsGroup" => vec![
            e("insert.equation", "Equation"),
            e("insert.symbol", "Symbol"),
        ],
        "insert.headerFooterGroup" => vec![
            e("insert.header", "Header"),
            e("insert.footer", "Footer"),
            e("insert.pageNumber", "Page Number"),
        ],
        "layout.arrangeGroup" => vec![
            e("layout.position", "Position"),
            e("layout.wrapText", "Wrap Text"),
            e("layout.bringForward", "Bring Forward"),
            e("layout.sendBackward", "Send Backward"),
            e("layout.selectionPane", "Selection Pane"),
            e("layout.align", "Align"),
            e("layout.group", "Group"),
            e("layout.rotate", "Rotate"),
        ],
        "view.windowGroup" => vec![
            e("view.newWindow", "New Window"),
            e("view.arrangeAll", "Arrange All"),
            e("view.split", "Split"),
            e("view.sideBySide", "View Side by Side"),
            e("view.syncScroll", "Synchronous Scrolling"),
            e("view.resetWindow", "Reset Window Position"),
            e("view.switchWindows", "Switch Windows"),
        ],
        "view.switchWindows" => vec![e("view.switchWindows.current", "1 Document")],
        "view.macros" => vec![
            e("view.macros.view", "View Macros"),
            e("view.macros.record", "Record Macro..."),
            e("view.macros.pause", "Pause Recording"),
        ],
        "insert.picture" => vec![
            e("insert.picture.device", "This Device..."),
            e("insert.picture.stock", "Stock Images..."),
            e("insert.picture.online", "Online Pictures..."),
        ],
        "layout.bringForward" => vec![
            e("picture.order.forward", "Bring Forward"),
            e("picture.order.front", "Bring to Front"),
            e("picture.wrap.inFront", "Bring in Front of Text"),
        ],
        "layout.sendBackward" => vec![
            e("picture.order.backward", "Send Backward"),
            e("picture.order.back", "Send to Back"),
            e("picture.wrap.behind", "Send Behind Text"),
        ],
        _ => Vec::new(),
    }
}

/// Menu entries shown dimmed: features BlueDoc does not have.
pub(crate) const UNAVAILABLE: [&str; 9] = [
    "view.syncScroll",
    "view.resetWindow",
    "view.switchWindows.current",
    "view.macros.view",
    "view.macros.record",
    "view.macros.pause",
    "insert.picture.stock",
    "insert.picture.online",
    "insert.bookmarkDialog",
];

/// Ids of this module that are buttons or menus (`&'static` for the ribbon).
pub(crate) const IDS: [&str; 13] = [
    "insert.picture",
    "insert.pagesGroup",
    "insert.linksGroup",
    "insert.symbolsGroup",
    "insert.headerFooterGroup",
    "layout.arrangeGroup",
    "view.windowGroup",
    "view.switchWindows",
    "view.syncScroll",
    "view.resetWindow",
    "insert.comment",
    "layout.indentHeading",
    "layout.spacingHeading",
];

/// Labels (id, English) of the ids above.
pub(crate) const LABELS: [(&str, &str); 12] = [
    ("insert.pagesGroup", "Pages"),
    ("insert.linksGroup", "Links"),
    ("insert.symbolsGroup", "Symbols"),
    ("insert.headerFooterGroup", "Header & Footer"),
    ("layout.arrangeGroup", "Arrange"),
    ("view.windowGroup", "Window"),
    ("view.switchWindows", "Switch Windows"),
    ("view.syncScroll", "Synchronous Scrolling"),
    ("view.resetWindow", "Reset Window Position"),
    ("insert.comment", "Comment"),
    ("layout.indentHeading", "Indent"),
    ("layout.spacingHeading", "Spacing"),
];

/// Layout › Paragraph column headings (text drawn by the window, not buttons).
pub(crate) const HEADINGS: [&str; 2] = ["layout.indentHeading", "layout.spacingHeading"];

/// Arrange commands that act on the selected picture or shape (dimmed without one, as on
/// the reference; Selection Pane stays available).
pub(crate) const ARRANGE: [&str; 7] = [
    "layout.position",
    "layout.wrapText",
    "layout.bringForward",
    "layout.sendBackward",
    "layout.align",
    "layout.group",
    "layout.rotate",
];

/// The `&'static` id of a fitting id.
pub(crate) fn static_id(id: &str) -> Option<&'static str> {
    IDS.iter().find(|m| **m == id).copied()
}

/// The collapsed group button that holds command `id`, if any (menus and galleries of a
/// collapsed group open under it).
pub(crate) fn group_of(id: &str) -> Option<&'static str> {
    GROUP_MENUS
        .iter()
        .find(|g| menu(g).iter().any(|e| e.id == id))
        .copied()
}

/// English labels into `commands.<id>` (translations under `docR4.cmd.*`).
pub(crate) fn install_strings(s: &mut crate::strings::Strings) {
    let english = s.get("ribbon.tabs.home") == "Home";
    for (id, en) in LABELS {
        let key = format!("docR4.cmd.{}", id.replace('.', "_"));
        let v = s.get(&key).to_owned();
        if v != key {
            s.set(&format!("commands.{id}"), &v);
        } else if english {
            s.set(&format!("commands.{id}"), en);
        }
    }
}

/// Icons of the fitting ids: the icon of the group's first command (own drawings).
pub(crate) fn install_icons(icons: &mut crate::icons::Icons) {
    for (id, of) in [
        ("insert.pagesGroup", "insert.coverPage"),
        ("insert.linksGroup", "insert.link"),
        ("insert.symbolsGroup", "insert.symbol"),
        ("insert.headerFooterGroup", "insert.header"),
        ("layout.arrangeGroup", "layout.position"),
        ("view.windowGroup", "view.newWindow"),
        ("view.switchWindows", "view.newWindow"),
        ("view.syncScroll", "view.sideBySide"),
        ("view.resetWindow", "view.sideBySide"),
        ("insert.comment", "review.newComment"),
        ("layout.indentHeading", "format.indent"),
        ("layout.spacingHeading", "format.lineSpacing"),
    ] {
        icons.alias(id, of);
    }
}
