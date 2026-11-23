//! Ribbon scaling of BlueDoc's contextual tabs (round 19, docs/ui/REPORT.md "Vòng 109"):
//! the order in which their groups collapse to one drop-down button as the window narrows,
//! measured on the reference with UI Automation at 1500…600 px (50 px steps, 125 %):
//!
//! * Picture Format — Arrange (1350), Size (1000), Adjust (900); Picture Styles and
//!   Accessibility never collapse.
//! * Table Design — the table looks gallery loses tiles first, then Borders (1150), Table
//!   Style Options (900), Table Styles (650).
//! * Shape Format — Size, Arrange (1400), WordArt Styles (1000), Insert Shapes (950), Shape
//!   Styles (900).
//! * Equation — Structures (1250), Tools and Conversions (750); the Symbols gallery only
//!   loses columns.
//! * Header & Footer — Options (1300), Position (1100), Navigation (1000), Insert (950).
//!
//! Builds on round 18's machinery (`doc_scale18`): the buttons here join its list.

use crate::doc_tabs7::{EQUATION, SHAPE_FORMAT};
use crate::doc_tools::{HEADER_FOOTER, PICTURE_FORMAT, TABLE_DESIGN};

/// Collapsed-group buttons of the contextual tabs: (button id, tab, group label, icon of).
pub(crate) const GROUPS: [(&str, &str, &str, &str); 13] = [
    (
        "r19g.arrange",
        PICTURE_FORMAT,
        "docR3.groupArrange",
        "picture.position",
    ),
    (
        "r19g.size",
        PICTURE_FORMAT,
        "pictureTools.groupSize",
        "picture.crop",
    ),
    (
        "r19g.adjust",
        PICTURE_FORMAT,
        "pictureTools.groupAdjust",
        "picture.corrections",
    ),
    (
        "r19g.borders",
        TABLE_DESIGN,
        "sheetEditor.borders",
        "table.borders",
    ),
    (
        "r19g.styleOptions",
        TABLE_DESIGN,
        "ribbon.groups.tableStyleOptions",
        "table.opt.headerRow",
    ),
    (
        "r19g.tableStyles",
        TABLE_DESIGN,
        "ribbon.groups.tableStyles",
        "table.shadingMenu",
    ),
    (
        "r19g.wordArt",
        SHAPE_FORMAT,
        "docR7.g.wordArt",
        "shape.wordArt",
    ),
    (
        "r19g.insertShapes",
        SHAPE_FORMAT,
        "docR7.g.insertShapes",
        "insert.shapes",
    ),
    (
        "r19g.shapeStyles",
        SHAPE_FORMAT,
        "docR7.g.shapeStyles",
        "shape.fill",
    ),
    (
        "r19g.structures",
        EQUATION,
        "docR7.g.structures",
        "eq.fraction",
    ),
    ("r19g.tools", EQUATION, "docR7.g.tools", "eq.equation"),
    (
        "r19g.conversions",
        EQUATION,
        "docR7.g.conversions",
        "eq.convert",
    ),
    (
        "r19g.hfOptions",
        HEADER_FOOTER,
        "hf.options",
        "hf.differentFirst",
    ),
];

/// Header & Footer's other collapsed groups (kept apart so the table above stays readable).
pub(crate) const GROUPS_HF: [(&str, &str, &str, &str); 3] = [
    (
        "r19g.hfPosition",
        HEADER_FOOTER,
        "docR7.g.position",
        "hf.headerPos",
    ),
    (
        "r19g.hfNavigation",
        HEADER_FOOTER,
        "hf.navigation",
        "hf.gotoHeader",
    ),
    (
        "r19g.hfInsert",
        HEADER_FOOTER,
        "ribbon.tabs.insert",
        "insert.dateTime",
    ),
];

/// All the buttons of this module.
pub(crate) fn all(
) -> impl Iterator<Item = &'static (&'static str, &'static str, &'static str, &'static str)> {
    GROUPS.iter().chain(GROUPS_HF.iter())
}

/// Collapse steps of a contextual tab, in the reference's order (`None`: not one).
pub(crate) fn scaling(tab: &str) -> Option<&'static [&'static str]> {
    Some(match tab {
        PICTURE_FORMAT => &[
            "docR3.groupArrange",
            "pictureTools.groupSize",
            "pictureTools.groupAdjust",
        ],
        TABLE_DESIGN => &[
            "sheetEditor.borders",
            "ribbon.groups.tableStyleOptions",
            "ribbon.groups.tableStyles",
        ],
        SHAPE_FORMAT => &[
            "pictureTools.groupSize",
            "docR3.groupArrange",
            "docR7.g.wordArt",
            "docR7.g.insertShapes",
            "docR7.g.shapeStyles",
        ],
        EQUATION => &["docR7.g.structures", "docR7.g.tools", "docR7.g.conversions"],
        HEADER_FOOTER => &[
            "hf.options",
            "docR7.g.position",
            "hf.navigation",
            "ribbon.tabs.insert",
        ],
        _ => return None,
    })
}

/// The contextual tabs, for the drop-down panel of a collapsed group.
pub(crate) fn tabs() -> Vec<crate::ribbon::Tab> {
    vec![
        crate::doc_tabs7::picture_tab(),
        crate::doc_tabs7::table_design_tab(),
        crate::doc_tabs7::table_layout_tab(),
        crate::doc_tabs7::shape_tab(),
        crate::doc_tabs7::header_footer_tab(),
        crate::doc_tabs7::equation_tab(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_contextual_button_names_a_group_of_its_tab_in_the_order() {
        for (id, tab, label, _) in all() {
            assert!(crate::ribbon::doc_group(tab, label).is_some(), "{id}");
            assert!(scaling(tab).is_some_and(|s| s.contains(label)), "{id}");
            assert!(crate::doc_scale18::is_group(id), "{id}");
            let last = crate::doc_scale18::shrink(label, 1).expect("stage");
            assert_eq!(last.large.len(), 1, "{id}");
        }
        assert_eq!(scaling("ribbon.tabs.home"), None);
    }
}
