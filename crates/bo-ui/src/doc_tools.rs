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
const MENUS: [&str; 17] = [
    "review.reviewingPane",
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
    "design.themes",
    "design.colors",
    "design.fonts",
    "design.paragraphSpacing",
    "design.effects",
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
    /// The outline view is on (Outlining tab).
    pub(crate) outline: bool,
    /// A drawing shape is selected (Shape Format).
    pub(crate) shape: bool,
    /// An equation is selected or edited (Equation).
    pub(crate) equation: bool,
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
            outline: false,
            shape: false,
            equation: false,
        }
    }
}

/// Contextual tabs for `c`, in the reference's order.
pub(crate) fn tabs(c: Context) -> Vec<Tab> {
    let mut out = Vec::new();
    if c.outline {
        out.push(outlining_tab());
    }
    if c.story.is_some() {
        out.push(crate::doc_tabs7::header_footer_tab());
    }
    if c.table {
        out.push(crate::doc_tabs7::table_design_tab());
        out.push(crate::doc_tabs7::table_layout_tab());
    }
    if c.picture {
        out.push(crate::doc_tabs7::picture_tab());
    }
    if c.shape {
        out.push(crate::doc_tabs7::shape_tab());
    }
    if c.equation {
        out.push(crate::doc_tabs7::equation_tab());
    }
    out
}

/// Tab label key of the Outlining tab.
pub(crate) const OUTLINING: &str = "docR3.tabOutlining";

/// The Outlining tab of the outline view.
fn outlining_tab() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    Tab {
        label: OUTLINING,
        groups: vec![
            // « ← [Level ▾] → », ▲ ▼ + −, then Show Level / Show Text Formatting / Show
            // First Line Only, as the reference's Outline Tools.
            Group {
                label: "docR3.groupOutlineTools",
                large: vec![],
                rows: vec![
                    vec![
                        B("outline.promoteHeading"),
                        B("outline.promote"),
                        S("outline.level"),
                        B("outline.demote"),
                        B("outline.demoteBody"),
                    ],
                    vec![
                        B("outline.moveUp"),
                        B("outline.moveDown"),
                        B("outline.expand"),
                        B("outline.collapse"),
                    ],
                ],
            },
            Group {
                label: "docR3.groupOutlineShow",
                large: vec![],
                rows: vec![
                    vec![S("outline.showLevel")],
                    vec![S("outline.showFormatting")],
                    vec![S("outline.firstLine")],
                ],
            },
            Group {
                label: "docR3.groupMaster",
                large: vec![L("outline.showDocument"), L("outline.collapseSubdocs")],
                rows: vec![],
            },
            Group {
                label: "docR3.groupClose",
                large: vec![L("outline.close")],
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
            e("table.sortAsc", "docTable.sortAsc"),
            e("table.sortDesc", "docTable.sortDesc"),
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
        // The reference's five entries each (round 6).
        "review.acceptMenu" => vec![
            e("review.acceptNext", "docR6.rv.acceptNext"),
            e("review.accept", "docR6.rv.acceptThis"),
            e("review.acceptAllShown", "docR6.rv.acceptShown"),
            e("review.acceptAll", "docR6.rv.acceptAll"),
            e("review.acceptAllStop", "docR6.rv.acceptStop"),
        ],
        "review.rejectMenu" => vec![
            e("review.rejectNext", "docR6.rv.rejectNext"),
            e("review.reject", "docR6.rv.rejectThis"),
            e("review.rejectAllShown", "docR6.rv.rejectShown"),
            e("review.rejectAll", "docR6.rv.rejectAll"),
            e("review.rejectAllStop", "docR6.rv.rejectStop"),
        ],
        // Track Changes split button (its menu is opened by the window, round 6) and the
        // Reviewing Pane's placements.
        "review.trackChanges" => vec![
            e("review.trackChanges", "docR6.rv.forEveryone"),
            e("review.lockTracking", "docR6.rv.lock"),
        ],
        "review.reviewingPane" => vec![
            e("review.reviewingPane.vertical", "docR6.rv.paneVertical"),
            e("review.reviewingPane.horizontal", "docR6.rv.paneHorizontal"),
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
        _ => crate::doc_r3_menus::menu(id),
    }
}

/// The `&'static str` of a contextual menu id (menus anchor on their ribbon button).
pub(crate) fn static_id(id: &str) -> Option<&'static str> {
    // The Track Changes split button (round 6) anchors its menu too.
    MENUS
        .iter()
        .chain(["review.trackChanges"].iter())
        .find(|m| **m == id)
        .copied()
        .or_else(|| crate::doc_tabs7::static_id(id))
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
        "table.splitTable" => table(&json!({ "kind": "splitTable" })),
        // Sort the rows below the header row by the caret's column, as text.
        "table.sortAsc" | "table.sortDesc" => {
            let column = f.table.as_ref().map_or(0, |t| t.col);
            table(&json!({
                "kind": "sort",
                "keys": [{ "column": column, "kind": "text", "descending": id == "table.sortDesc" }],
                "header": f.table.as_ref().is_some_and(|t| t.header_row),
            }))
        }
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
    if let Some(on) = crate::doc_tabs7::enabled(id, f) {
        return Some(on);
    }
    let in_table = f.table.is_some();
    Some(match id {
        "table.mergeCells" => f.table.as_ref().is_some_and(|t| t.multi_cell),
        "picture.group" | "outline.showDocument" | "outline.collapseSubdocs" => false,
        _ if id.starts_with("outline.") => true,
        _ if id.starts_with("table.") => in_table,
        _ if id.starts_with("picture.") => f.picture.is_some() || f.picture_tools,
        _ if id.starts_with("hf.") || id.starts_with("review.markup") => true,
        "review.showMarkup"
        | "review.acceptMenu"
        | "review.rejectMenu"
        | "review.accept"
        | "review.reject"
        | "review.showComments"
        | "design.themes"
        | "design.colors"
        | "design.fonts"
        | "design.paragraphSpacing"
        | "design.styleSetMore"
        | "design.pageBorders"
        | "design.effects"
        | "design.setDefault"
        | "references.markCitation"
        | "references.insertToa"
        | "references.updateToa"
        | "references.updateIndex" => true,
        _ if id.starts_with("design.styleSet.") => true,
        _ if crate::doc_design::UNAVAILABLE.contains(&id) => false,
        _ if crate::doc_r3_menus::static_id(id).is_some() => true,
        // Arrange acts on the selected picture or shape (round 4, as the reference).
        _ if crate::doc_r4_fit::ARRANGE.contains(&id) => f.picture.is_some() || f.picture_tools,
        _ if crate::doc_r4_fit::UNAVAILABLE.contains(&id) => false,
        _ if crate::doc_r4_fit::static_id(id).is_some() => true,
        _ if crate::doc_review16::static_id(id).is_some() => true,
        _ if crate::doc_r4_menus::enabled(id).is_some() => {
            crate::doc_r4_menus::enabled(id).unwrap_or(false)
        }
        _ => return None,
    })
}

/// Ids drawn as on: repeat header row, header/footer options.
pub(crate) fn on_ids(f: &FormatState, story: Option<(bool, bool, bool)>) -> Vec<&'static str> {
    let mut on = Vec::new();
    // Page Movement › Vertical is the only movement: shown pressed, as the reference.
    on.push("view.vertical");
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
    crate::shell::install_round6_strings(s);
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
        ("view.readMode", "docTable.readMode"),
        ("review.showComments", "slideNative.showComments"),
        ("design.colors", "docTable.designColors"),
        ("design.fonts", "docTable.designFonts"),
        ("design.paragraphSpacing", "docTable.paragraphSpacing"),
        ("design.effects", "docTable.effects"),
        ("design.setDefault", "docTable.setDefault"),
        ("design.pageBorders", "docTable.pageBorders"),
        ("design.pageColor", "docTable.pageColor"),
        ("design.styleSetMore", "docTable.moreStyleSets"),
        ("design.styleSet.standard", "docTable.styleSet.standard"),
        ("design.styleSet.capital", "docTable.styleSet.capital"),
        ("design.styleSet.lotus", "docTable.styleSet.lotus"),
        ("design.styleSet.river", "docTable.styleSet.river"),
        ("design.styleSet.harbor", "docTable.styleSet.harbor"),
        ("design.styleSet.quiet", "docTable.styleSet.quiet"),
        ("design.styleSet.bold", "docTable.styleSet.bold"),
    ] {
        copy(s, id, key);
    }
    // Group names in the reference's casing (Document Formatting, Page Background).
    for (group, key) in [
        (
            "ribbon.groups.documentFormatting",
            "docTable.groupDocumentFormatting",
        ),
        (
            "ribbon.groups.pageBackground",
            "docTable.groupPageBackground",
        ),
    ] {
        let v = s.get(key).to_owned();
        if v != key {
            s.set(group, &v);
        }
    }
}

/// Icons of the contextual commands (Bluestar's own drawings, shared icon set).
pub(crate) fn install_icons(icons: &mut Icons) {
    crate::shell::install_round6_icons(icons);
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
        ("table.splitTable", "table.split"),
        ("table.properties", "table.bordersAll"),
        ("table.cellMargins", "layout.margins"),
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
        ("view.readMode", "view.reading"),
        ("view.webLayout", "view.normal"),
        ("view.draft", "format.justify"),
        ("review.showMarkup", "review.comments"),
        ("review.showComments", "review.comments"),
        ("design.themes", "view.toggleTheme"),
        ("design.colors", "format.color"),
        ("design.fonts", "format.fontDialog"),
        ("design.paragraphSpacing", "format.lineSpacing"),
        ("design.effects", "format.fill"),
        ("design.setDefault", "review.accept"),
        ("design.pageBorders", "format.borders"),
        ("design.styleSetMore", "style.title"),
        ("design.styleSet.standard", "style.title"),
        ("design.styleSet.capital", "style.title"),
        ("design.styleSet.lotus", "style.title"),
        ("design.styleSet.river", "style.title"),
        ("design.styleSet.harbor", "style.title"),
        ("design.styleSet.quiet", "style.title"),
        ("design.styleSet.bold", "style.title"),
        ("picture.crop", "picture.position"),
        ("picture.fill", "view.fitWidth"),
        ("picture.fit", "view.fitWidth"),
    ] {
        if !icons.has(id) {
            icons.alias(id, existing);
        }
    }
    // Design tab, filled like the rest of BlueDoc's ribbon (own shapes): Themes an "Aa"
    // tile over four accent bars, Colors four swatches of a BlueOffice theme, Fonts a solid
    // A, Watermark a page with a pale diagonal band, Page Color a tinted page and a drop.
    let rgb = |r, g, b| Some(bo_render::Rgba(r, g, b, 255));
    let page = "M4 3h16v18h-16Z";
    icons.add_solid(
        "design.themes",
        &[
            (page, rgb(255, 255, 255)),
            (
                "M5.5 13L8.6 4.5h1.6L13.3 13h-1.7l-0.7-2h-3l-0.7 2Z",
                rgb(38, 38, 38),
            ),
            ("M8.7 9.6h1.9L9.6 6.6Z", rgb(255, 255, 255)),
            ("M14.2 8.5h3.6v4.5h-3.6Z", rgb(38, 38, 38)),
            ("M15.3 9.7h1.4v2.1h-1.4Z", rgb(255, 255, 255)),
            ("M6 16h3v3h-3Z", rgb(0x1f, 0x6f, 0xb2)),
            ("M9.3 16h3v3h-3Z", rgb(0x2b, 0xa3, 0xa0)),
            ("M12.6 16h3v3h-3Z", rgb(0xf2, 0xa5, 0x41)),
            ("M15.9 16h2.1v3h-2.1Z", rgb(0xd9, 0x53, 0x4f)),
            (page, None),
        ],
    );
    icons.add_solid(
        "design.colors",
        &[
            ("M3 3h9v9h-9Z", rgb(0x1b, 0x3a, 0x5c)),
            ("M12 3h9v9h-9Z", rgb(0xe7, 0xee, 0xf5)),
            ("M3 12h9v9h-9Z", rgb(0x1f, 0x6f, 0xb2)),
            ("M12 12h9v9h-9Z", rgb(0xf2, 0xa5, 0x41)),
            ("M3 3h18v18h-18Z", None),
        ],
    );
    icons.add_solid(
        "design.fonts",
        &[
            (page, rgb(255, 255, 255)),
            (
                "M6.5 19L11 5h2l4.5 14h-2.4l-1.1-3.4h-4l-1.1 3.4Z",
                rgb(20, 20, 20),
            ),
            ("M10.6 13.6h2.8L12 9Z", rgb(255, 255, 255)),
            (page, None),
        ],
    );
    icons.add_solid(
        "design.watermark",
        &[
            (page, rgb(255, 255, 255)),
            ("M6 16.5L15.5 5.5h2.5L8.5 16.5Z", rgb(0xf0, 0xb4, 0xb4)),
            ("M6 18.5h12v1h-12Z", rgb(150, 150, 150)),
            (page, None),
        ],
    );
    icons.add_solid(
        "design.pageColor",
        &[
            ("M3 3h14v18h-14Z", rgb(0xc8, 0xde, 0xf5)),
            ("M3 3h14v18h-14Z", None),
            (
                "M18.5 11L21.5 15.5L21 18L18.5 19.5L16 18L15.5 15.5Z",
                rgb(0x1f, 0x6f, 0xb2),
            ),
        ],
    );
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
