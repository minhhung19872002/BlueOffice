//! BlueDoc's contextual ribbon tabs, shown while the selection needs them as on a desktop
//! word processor: Table Design and Table Layout with the caret in a table, Header & Footer
//! while a header or footer is edited, Picture Format with a picture selected. Commands
//! go to the engine through the JSON command bridge (ADR-0008).
use crate::catalog::Entry;
use crate::icons::Icons;
use crate::ribbon::{Group, Item, Tab};
use crate::strings::Strings;
use bo_edit::FormatState;
use bo_engine::edit::Command;
use bo_engine::markup::MarkupOptions;
use serde_json::{json, Value};

/// Tab label keys of the contextual tabs.
pub(crate) const TABLE_DESIGN: &str = "ribbon.tabs.tableDesign";
pub(crate) const TABLE_LAYOUT: &str = "ribbon.tabs.tableLayout";
pub(crate) const HEADER_FOOTER: &str = "ribbon.tabs.headerFooter";
pub(crate) const PICTURE_FORMAT: &str = "ribbon.tabs.pictureFormat";

/// Menu buttons of the contextual tabs.
const MENUS: [&str; 11] = [
    "table.select",
    "table.data",
    "review.markupMode",
    "review.showMarkup",
    "review.acceptMenu",
    "review.rejectMenu",
    "table.shadingMenu",
    "table.borders",
    "table.delete",
    "table.autoFit",
    "picture.crop",
];

/// What the selection needs: header/footer editing, a table, a picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Context {
    /// A header (`Some(true)`) or footer (`Some(false)`) is being edited.
    pub(crate) story: Option<bool>,
    /// The caret is in a table.
    pub(crate) table: bool,
    /// A picture is selected.
    pub(crate) picture: bool,
}

impl Context {
    /// Context of the formatting state and the edited story kind (`header`, `footer`, …).
    pub(crate) fn of(f: &FormatState, story: Option<&str>) -> Self {
        Self {
            story: match story {
                Some("header") => Some(true),
                Some("footer") => Some(false),
                _ => None,
            },
            table: f.table.is_some(),
            picture: f.picture.is_some() || f.picture_tools,
        }
    }
}

/// Contextual tabs for `c`, in the reference's order.
pub(crate) fn tabs(c: Context) -> Vec<Tab> {
    let mut out = Vec::new();
    if c.story.is_some() {
        out.push(header_footer_tab());
    }
    if c.table {
        out.push(table_design_tab());
        out.push(table_layout_tab());
    }
    if c.picture {
        out.push(picture_tab());
    }
    out
}

fn table_design_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    let mut looks: Vec<Item> = crate::doc_table::LOOK_TILES
        .iter()
        .map(|(id, _)| L(id))
        .collect();
    looks.push(L("table.shadingMenu"));
    Tab {
        label: TABLE_DESIGN,
        groups: vec![
            // Check boxes in three rows of two, as on the reference.
            Group {
                label: "ribbon.groups.tableStyleOptions",
                large: vec![],
                rows: vec![
                    vec![S("table.opt.headerRow"), S("table.opt.firstColumn")],
                    vec![S("table.opt.totalRow"), S("table.opt.lastColumn")],
                    vec![S("table.opt.bandedRows"), S("table.opt.bandedColumns")],
                ],
            },
            Group {
                label: "ribbon.groups.tableStyles",
                large: looks,
                rows: vec![],
            },
            Group {
                label: "sheetEditor.borders",
                large: vec![L("table.borders")],
                rows: vec![],
            },
        ],
    }
}

fn table_layout_tab() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    let a = |i: usize| B(crate::doc_table::ALIGN[i].0);
    Tab {
        label: TABLE_LAYOUT,
        groups: vec![
            Group {
                label: "ribbon.groups.table",
                large: vec![],
                rows: vec![vec![S("table.select")]],
            },
            Group {
                label: "ribbon.groups.rowsColumns",
                large: vec![L("table.delete"), L("table.insertRowAbove")],
                rows: vec![
                    vec![S("table.insertRowBelow")],
                    vec![S("table.insertColumnLeft")],
                    vec![S("table.insertColumnRight")],
                ],
            },
            Group {
                label: "ribbon.groups.merge",
                large: vec![],
                rows: vec![vec![S("table.mergeCells")], vec![S("table.splitCell")]],
            },
            Group {
                label: "ribbon.groups.cellSize",
                large: vec![L("table.autoFit")],
                rows: vec![
                    vec![S("table.distributeRows")],
                    vec![S("table.distributeColumns")],
                ],
            },
            // Nine alignment buttons in three rows, then Text Direction (reference order).
            Group {
                label: "ribbon.groups.cellAlignment",
                large: vec![L("table.textDirection")],
                rows: vec![
                    vec![a(0), a(1), a(2)],
                    vec![a(3), a(4), a(5)],
                    vec![a(6), a(7), a(8)],
                ],
            },
            // One Data menu, as on the reference.
            Group {
                label: "ribbon.groups.tableData",
                large: vec![L("table.data")],
                rows: vec![],
            },
        ],
    }
}

fn header_footer_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: HEADER_FOOTER,
        groups: vec![
            Group {
                label: "ribbon.groups.headerFooter",
                large: vec![L("insert.pageNumber")],
                rows: vec![],
            },
            Group {
                label: "ribbon.tabs.insert",
                large: vec![L("insert.dateTime")],
                rows: vec![],
            },
            Group {
                label: "hf.navigation",
                large: vec![L("hf.gotoHeader"), L("hf.gotoFooter")],
                rows: vec![vec![S("hf.linkToPrevious")]],
            },
            Group {
                label: "hf.options",
                large: vec![],
                rows: vec![vec![S("hf.differentFirst")], vec![S("hf.differentOddEven")]],
            },
            Group {
                label: "slideEditor.close",
                large: vec![L("hf.close")],
                rows: vec![],
            },
        ],
    }
}

fn picture_tab() -> Tab {
    use Item::Large as L;
    Tab {
        label: PICTURE_FORMAT,
        groups: vec![
            Group {
                label: "pictureTools.groupAdjust",
                large: vec![L("picture.reset")],
                rows: vec![],
            },
            Group {
                label: "pictureTools.groupSize",
                large: vec![L("picture.crop")],
                rows: vec![],
            },
        ],
    }
}

const fn e(id: &'static str, label: &'static str) -> Entry {
    Entry { id, label }
}

/// Entries of the contextual menu button `id` (labels are i18n keys).
pub(crate) fn menu(id: &str) -> Vec<Entry> {
    match id {
        // §17.4.33 shd of the selected cells.
        "table.shadingMenu" => vec![
            e("table.shade.none", "No color"),
            e("table.shade.D9D9D9", "Light gray"),
            e("table.shade.DDEBF7", "Light blue"),
            e("table.shade.E2EFDA", "Light green"),
            e("table.shade.FFF2CC", "Light yellow"),
            e("table.shade.FCE4D6", "Light orange"),
        ],
        // §17.4.39 tblBorders presets.
        "table.borders" => vec![
            e("table.bordersAll", "commands.table.bordersAll"),
            e("table.bordersOutside", "commands.table.bordersOutside"),
            e("table.bordersInside", "commands.table.bordersInside"),
            e("table.bordersNone", "commands.table.bordersNone"),
        ],
        "table.data" => vec![
            e("table.headerRow", "docTable.repeatHeaderRows"),
            e("table.toText", "docTable.convertToText"),
        ],
        "table.select" => crate::doc_table::SELECT
            .iter()
            .map(|(id, label)| e(id, label))
            .collect(),
        "table.delete" => vec![
            e("table.deleteCell", "commands.table.deleteCell"),
            e("table.deleteColumn", "commands.table.deleteColumn"),
            e("table.deleteRow", "commands.table.deleteRow"),
            e("table.deleteTable", "commands.table.deleteTable"),
        ],
        // §17.4.53 tblLayout, §17.4.63 tblW.
        "table.autoFit" => vec![
            e("table.autoFitContents", "commands.table.autoFitContents"),
            e("table.autoFitWindow", "commands.table.autoFitWindow"),
            e("table.autoFitFixed", "commands.table.autoFitFixed"),
        ],
        // Review display (CORE markup modes) and marks shown.
        "review.markupMode" => vec![
            e("review.markup.simple", "commands.review.markupSimple"),
            e("review.markup.all", "commands.review.markupAll"),
            e("review.markup.none", "commands.review.markupNone"),
            e("review.markup.original", "commands.review.markupOriginal"),
        ],
        "review.showMarkup" => vec![
            e("review.markup.comments", "commands.review.markupComments"),
            e("review.markup.insDel", "commands.review.markupInsDel"),
            e(
                "review.markup.formatting",
                "commands.review.markupFormatting",
            ),
            e("review.markup.balloons", "commands.review.markupBalloons"),
        ],
        // §17.13.5 revisions: the one at the selection, or all of them.
        "review.acceptMenu" => vec![
            e("review.accept", "commands.review.accept"),
            e("review.acceptAll", "commands.review.acceptAll"),
        ],
        "review.rejectMenu" => vec![
            e("review.reject", "commands.review.reject"),
            e("review.rejectAll", "commands.review.rejectAll"),
        ],
        // §20.1.8.55 srcRect / §20.1.9.18 prstGeom.
        "picture.crop" => vec![
            e("picture.cropAspect.1:1", "1:1"),
            e("picture.cropAspect.4:3", "4:3"),
            e("picture.cropAspect.3:2", "3:2"),
            e("picture.cropAspect.16:9", "16:9"),
            e("picture.cropShape.ellipse", "commands.insert.ellipse"),
            e("picture.cropShape.roundRect", "commands.insert.rect"),
            e("picture.fill", "pictureTools.fill"),
            e("picture.fit", "pictureTools.fit"),
        ],
        _ => Vec::new(),
    }
}

/// The `&'static str` of a contextual menu id (menus anchor on their ribbon button).
pub(crate) fn static_id(id: &str) -> Option<&'static str> {
    MENUS.iter().find(|m| **m == id).copied()
}

/// What a contextual command runs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Tool {
    /// An editing command.
    Edit(Command),
    /// A picture tools command (`{"cmd": "picture…"}`).
    Picture(Value),
}

fn table(edit: &Value) -> Option<Tool> {
    crate::catalog::from_json(json!({ "cmd": "tableEdit", "edit": edit })).map(Tool::Edit)
}

/// The command of contextual button or menu entry `id`, given the formatting state and the
/// open header/footer (`(title page, even and odd, linked)` when one is edited).
pub(crate) fn command(
    id: &str,
    f: &FormatState,
    story: Option<(bool, bool, bool)>,
    page: usize,
    markup: &MarkupOptions,
) -> Option<Tool> {
    if let Some(cmd) = crate::doc_table::command(id) {
        return Some(Tool::Edit(cmd));
    }
    if let Some(hex) = id.strip_prefix("table.shade.") {
        let hex = (hex != "none").then(|| hex.to_owned());
        return table(&json!({ "kind": "shading", "hex": hex }));
    }
    if let Some(ratio) = id.strip_prefix("picture.cropAspect.") {
        return Some(Tool::Picture(
            json!({ "cmd": "pictureCropAspect", "ratio": ratio }),
        ));
    }
    if let Some(preset) = id.strip_prefix("picture.cropShape.") {
        return Some(Tool::Picture(
            json!({ "cmd": "pictureCropShape", "preset": preset }),
        ));
    }
    let edit = |v: Value| crate::catalog::from_json(v).map(Tool::Edit);
    if let Some(mode) = id.strip_prefix("review.markup.") {
        let v = match mode {
            "simple" | "all" | "none" | "original" => json!({ "cmd": "setMarkup", "mode": mode }),
            _ => {
                let key = match mode {
                    "insDel" => "insertionsDeletions",
                    "formatting" => "formatting",
                    "comments" => "comments",
                    "balloons" => "deletionBalloons",
                    _ => return None,
                };
                let mut v = json!({ "cmd": "setMarkup" });
                v[key] = json!(!markup_flag(markup, key));
                v
            }
        };
        return edit(v);
    }
    match id {
        "review.accept" => edit(json!({ "cmd": "acceptRevision" })),
        "review.reject" => edit(json!({ "cmd": "rejectRevision" })),
        "table.insertRowAbove" => table(&json!({ "kind": "insertRow", "below": false })),
        "table.insertRowBelow" => table(&json!({ "kind": "insertRow", "below": true })),
        "table.insertColumnLeft" => table(&json!({ "kind": "insertColumn", "right": false })),
        "table.insertColumnRight" => table(&json!({ "kind": "insertColumn", "right": true })),
        "table.deleteRow" => table(&json!({ "kind": "deleteRow" })),
        "table.deleteColumn" => table(&json!({ "kind": "deleteColumn" })),
        "table.deleteCell" => table(&json!({ "kind": "deleteCell" })),
        "table.deleteTable" => table(&json!({ "kind": "deleteTable" })),
        "table.mergeCells" => table(&json!({ "kind": "mergeCells" })),
        "table.splitCell" => table(&json!({ "kind": "splitCell" })),
        "table.bordersAll" => table(&json!({ "kind": "borders", "preset": "all" })),
        "table.bordersNone" => table(&json!({ "kind": "borders", "preset": "none" })),
        "table.bordersOutside" => table(&json!({ "kind": "borders", "preset": "outside" })),
        "table.bordersInside" => table(&json!({ "kind": "borders", "preset": "inside" })),
        "table.distributeRows" => table(&json!({ "kind": "distributeRows" })),
        "table.distributeColumns" => table(&json!({ "kind": "distributeColumns" })),
        "table.autoFitContents" => table(&json!({ "kind": "autoFit", "mode": "contents" })),
        "table.autoFitWindow" => table(&json!({ "kind": "autoFit", "mode": "window" })),
        "table.autoFitFixed" => table(&json!({ "kind": "autoFit", "mode": "fixed" })),
        "table.toText" => table(&json!({ "kind": "toText", "separator": "\t" })),
        "table.headerRow" => {
            let on = !f.table.as_ref().is_some_and(|t| t.header_row);
            table(&json!({ "kind": "headerRow", "on": on }))
        }
        // Each press turns the text a quarter (§17.18.93 ST_TextDirection), as the
        // reference's Text Direction button cycles.
        "table.textDirection" => {
            let dir = match f.table.as_ref().and_then(|t| t.direction) {
                None | Some(bo_model::TextDirection::LrTb) => Some("tbRl"),
                Some(bo_model::TextDirection::TbRl) => Some("btLr"),
                Some(_) => None,
            };
            table(&json!({ "kind": "textDirection", "dir": dir }))
        }
        "hf.close" => Some(Tool::Edit(Command::CloseHeaderFooter)),
        "hf.gotoHeader" | "hf.gotoFooter" => edit(json!({
            "cmd": "editHeaderFooter", "page": page, "header": id == "hf.gotoHeader",
        })),
        "hf.linkToPrevious" => {
            let linked = story.is_some_and(|s| s.2);
            edit(json!({ "cmd": "setHeaderLink", "linked": !linked }))
        }
        // §17.10.6 titlePg, §17.15.1.35 evenAndOddHeaders.
        "hf.differentFirst" => {
            let on = story.is_some_and(|s| s.0);
            edit(json!({ "cmd": "setHeaderOptions", "titlePage": !on }))
        }
        "hf.differentOddEven" => {
            let on = story.is_some_and(|s| s.1);
            edit(json!({ "cmd": "setHeaderOptions", "evenAndOdd": !on }))
        }
        "picture.reset" => Some(Tool::Picture(
            json!({ "cmd": "pictureReset", "size": false }),
        )),
        "picture.fill" => Some(Tool::Picture(json!({ "cmd": "pictureFill" }))),
        "picture.fit" => Some(Tool::Picture(json!({ "cmd": "pictureFit" }))),
        _ => None,
    }
}

/// A markup option by its JSON name.
fn markup_flag(m: &MarkupOptions, key: &str) -> bool {
    match key {
        "insertionsDeletions" => m.insertions_deletions,
        "formatting" => m.formatting,
        "comments" => m.comments,
        _ => m.deletion_balloons,
    }
}

/// Menu entries shown ticked: the markup mode and the marks shown.
pub(crate) fn checked(id: &str, m: &MarkupOptions) -> bool {
    use bo_engine::markup::MarkupMode;
    match id {
        "review.markup.simple" => m.mode == MarkupMode::Simple,
        "review.markup.all" => m.mode == MarkupMode::All,
        "review.markup.none" => m.mode == MarkupMode::None,
        "review.markup.original" => m.mode == MarkupMode::Original,
        "review.markup.insDel" => m.insertions_deletions,
        "review.markup.formatting" => m.formatting,
        "review.markup.comments" => m.comments,
        "review.markup.balloons" => m.deletion_balloons,
        _ => false,
    }
}

/// Whether contextual command `id` can run (`None`: not a contextual command).
pub(crate) fn enabled(id: &str, f: &FormatState) -> Option<bool> {
    let in_table = f.table.is_some();
    Some(match id {
        "table.mergeCells" => f.table.as_ref().is_some_and(|t| t.multi_cell),
        _ if id.starts_with("table.") => in_table,
        _ if id.starts_with("picture.") => f.picture.is_some() || f.picture_tools,
        _ if id.starts_with("hf.") || id.starts_with("review.markup") => true,
        "review.showMarkup" | "review.acceptMenu" | "review.rejectMenu" | "review.accept"
        | "review.reject" => true,
        _ => return None,
    })
}

/// Ids drawn as on: repeat header row, header/footer options.
pub(crate) fn on_ids(f: &FormatState, story: Option<(bool, bool, bool)>) -> Vec<&'static str> {
    let mut on = Vec::new();
    on.extend(crate::doc_table::align_on(f));
    if f.table.as_ref().is_some_and(|t| t.header_row) {
        on.push("table.headerRow");
    }
    if let Some((first, odd_even, linked)) = story {
        if first {
            on.push("hf.differentFirst");
        }
        if odd_even {
            on.push("hf.differentOddEven");
        }
        if linked {
            on.push("hf.linkToPrevious");
        }
    }
    on
}

/// Names of the contextual commands, from strings the shared interface already has.
pub(crate) fn install_strings(s: &mut Strings) {
    let copy = |s: &mut Strings, id: &str, key: &str| {
        let v = s.get(key).to_owned();
        if v != key {
            s.set(&format!("commands.{id}"), &v);
        }
    };
    for (id, key) in [
        ("table.shadingMenu", "ribbon.docTools.shading"),
        ("table.borders", "ribbon.docTools.borders"),
        ("table.delete", "ribbon.docTools.delete"),
        ("table.autoFit", "ribbon.docTools.autoFit"),
        ("table.textDirection", "ribbon.docTools.textDirection"),
        ("table.mergeCells", "ribbon.docTools.mergeCells"),
        ("table.splitCell", "ribbon.docTools.splitCells"),
        ("hf.close", "hf.close"),
        ("hf.gotoHeader", "hf.gotoHeader"),
        ("hf.gotoFooter", "hf.gotoFooter"),
        ("hf.linkToPrevious", "hf.linkToPrevious"),
        ("hf.differentFirst", "hf.differentFirst"),
        ("hf.differentOddEven", "hf.differentOddEven"),
        ("picture.reset", "pictureTools.reset"),
        ("review.spellingDialog", "ribbon.docTools.spelling"),
        ("review.acceptMenu", "commands.review.accept"),
        ("review.rejectMenu", "commands.review.reject"),
        ("picture.crop", "ribbon.docTools.crop"),
        ("picture.fill", "pictureTools.fill"),
        ("picture.fit", "pictureTools.fit"),
    ] {
        copy(s, id, key);
    }
}

/// Icons of the contextual commands (Bluestar's own drawings, shared icon set).
pub(crate) fn install_icons(icons: &mut Icons) {
    for (id, existing) in [
        ("table.shadingMenu", "table.fill"),
        ("table.borders", "table.bordersAll"),
        ("table.delete", "table.deleteTable"),
        ("table.autoFit", "view.fitWidth"),
        ("table.distributeRows", "table.insertRowBelow"),
        ("table.distributeColumns", "table.insertColumnRight"),
        ("table.textDirection", "format.wrap"),
        ("table.toText", "table.split"),
        ("table.data", "data.sortAsc"),
        ("hf.gotoHeader", "insert.header"),
        ("hf.gotoFooter", "insert.footer"),
        ("hf.linkToPrevious", "insert.link"),
        ("hf.differentFirst", "insert.pageNumber"),
        ("hf.differentOddEven", "insert.pageNumber"),
        ("picture.reset", "edit.undo"),
        ("review.acceptMenu", "review.accept"),
        ("review.rejectMenu", "review.reject"),
        ("review.markupMode", "review.viewMarkup"),
        ("view.printLayout", "layout.margins"),
        ("view.webLayout", "view.normal"),
        ("view.draft", "format.justify"),
        ("review.showMarkup", "review.comments"),
        ("picture.crop", "picture.position"),
        ("picture.fill", "view.fitWidth"),
        ("picture.fit", "view.fitWidth"),
    ] {
        if !icons.has(id) {
            icons.alias(id, existing);
        }
    }
    // Navigation pane: a page with a filled list column on its left (own drawing).
    icons.add_solid(
        "view.navigationPane",
        &[
            ("M3 4h18v16h-18Z", Some(bo_render::Rgba(255, 255, 255, 255))),
            (
                "M3 4h7v16h-7Z",
                Some(bo_render::Rgba(0xb9, 0xd2, 0xee, 255)),
            ),
            (
                "M3 4h18v16h-18ZM10 4v16M5 8h3M5 11h3M5 14h3M13 8h5M13 11h5M13 14h5",
                None,
            ),
        ],
    );
    // Close: a white cross on a warm red square (own drawing; the reference's close
    // button is a red square, 216,99,68 measured).
    icons.add_solid(
        "hf.close",
        &[
            ("M4 4h16v16h-16Z", Some(bo_render::Rgba(216, 99, 68, 255))),
            (
                "M7.5 8.9L8.9 7.5L12 10.6L15.1 7.5L16.5 8.9L13.4 12L16.5 15.1L15.1 16.5L12 13.4L8.9 16.5L7.5 15.1L10.6 12Z",
                Some(bo_render::Rgba(255, 255, 255, 255)),
            ),
        ],
    );
}
