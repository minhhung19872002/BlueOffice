//! BlueDoc's contextual tabs, round 7 (docs/ui/REPORT.md rounds 93+): Table Design with the
//! Borders group (Border Styles, line style / weight boxes, Pen Color, Borders ▾, Border
//! Painter), Table Layout with View Gridlines / Draw / Data ▾ Formula, Picture Format in the
//! reference's groups (Adjust, Picture Styles, Accessibility, Arrange, Size), Shape Format,
//! the full Header & Footer tab and the Equation tab — laid out as the reference desktop word
//! processor shows them at 1500 px (125 %), measured black-box. Icons are BlueOffice's own
//! drawings. Commands run in `doc_r7` (the shell's round-7 module).
use crate::catalog::Entry;
use crate::icons::Icons;
use crate::ribbon::{Group, Item, Tab};
use crate::strings::Strings;
use bo_edit::FormatState;
use bo_render::Rgba;

/// Tab label keys of the round-7 contextual tabs.
pub(crate) const SHAPE_FORMAT: &str = "ribbon.tabs.shapeFormat";
pub(crate) const EQUATION: &str = "ribbon.tabs.equation";

const fn e(id: &'static str, label: &'static str) -> Entry {
    Entry { id, label }
}

/// Menu buttons added in this round (their `'static` ids anchor the menus).
pub(crate) const MENUS: [&str; 27] = [
    "table.borderStyles",
    "table.penStyle",
    "table.penWeight",
    "table.penColor",
    "picture.corrections",
    "picture.color",
    "picture.artisticEffects",
    "picture.change",
    "picture.resetMenu",
    "picture.quickStyles",
    "picture.border",
    "picture.effects",
    "picture.layout",
    "hf.documentInfo",
    "eq.equation",
    "eq.convert",
    "eq.fraction",
    "eq.script",
    "eq.radical",
    "eq.integral",
    "eq.largeOperator",
    "eq.bracket",
    "eq.function",
    "eq.accent",
    "eq.limitLog",
    "eq.operator",
    "eq.matrix",
];

/// Commands shown but not available (greyed, as the reference greys what does not apply or
/// what BlueDoc does not have yet).
pub(crate) const GREYED: [&str; 18] = [
    "table.drawTable",
    "table.eraser",
    "picture.artisticEffects",
    "picture.quickStyles",
    "picture.border",
    "picture.effects",
    "picture.layout",
    "shape.edit",
    "shape.wordArt",
    "shape.style.0",
    "shape.style.1",
    "shape.style.2",
    "shape.fill",
    "shape.outline",
    "shape.effects",
    "hf.onlinePictures",
    "hf.alignmentTab",
    "eq.ink",
];

/// The pen of the Borders group: line style, width (eighths of a point), colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pen {
    pub(crate) style: &'static str,
    pub(crate) size: u32,
    pub(crate) color: Option<&'static str>,
}

impl Default for Pen {
    /// The reference's default pen: single line, ½ pt, automatic colour.
    fn default() -> Self {
        Pen {
            style: "single",
            size: 4,
            color: None,
        }
    }
}

/// Line styles of the line style box (§17.18.2 ST_Border).
pub(crate) const PEN_STYLES: [(&str, &str); 8] = [
    ("single", "Single"),
    ("dotted", "Dotted"),
    ("dashSmallGap", "Dashed (small gap)"),
    ("dashed", "Dashed"),
    ("dotDash", "Dot dash"),
    ("double", "Double"),
    ("triple", "Triple"),
    ("thickThinSmallGap", "Thick-thin"),
];

/// Line weights (eighths of a point) and their names, as the reference lists them.
pub(crate) const PEN_WEIGHTS: [(u32, &str); 9] = [
    (2, "¼ pt"),
    (4, "½ pt"),
    (6, "¾ pt"),
    (8, "1 pt"),
    (12, "1½ pt"),
    (18, "2¼ pt"),
    (24, "3 pt"),
    (36, "4½ pt"),
    (48, "6 pt"),
];

/// Pen colours: Automatic, then the standard colours.
pub(crate) const PEN_COLORS: [(&str, &str); 11] = [
    ("auto", "Automatic"),
    ("C00000", "Dark Red"),
    ("FF0000", "Red"),
    ("FFC000", "Orange"),
    ("FFFF00", "Yellow"),
    ("92D050", "Light Green"),
    ("00B050", "Green"),
    ("00B0F0", "Light Blue"),
    ("0070C0", "Blue"),
    ("002060", "Dark Blue"),
    ("7030A0", "Purple"),
];

/// Border Styles presets: style, size, label.
pub(crate) const BORDER_STYLES: [(&str, u32, &str); 6] = [
    ("single", 4, "Single solid line, ½ pt"),
    ("single", 12, "Single solid line, 1½ pt"),
    ("double", 4, "Double solid lines, ½ pt"),
    ("dotted", 4, "Dotted line, ½ pt"),
    ("dashed", 4, "Dashed line, ½ pt"),
    ("thickThinSmallGap", 24, "Thick-thin double line, 3 pt"),
];

/// Borders ▾ entries: id, engine preset, label key (reference order).
pub(crate) const BORDERS: [(&str, &str, &str); 10] = [
    ("table.border.bottom", "bottom", "docR7.b.bottom"),
    ("table.border.top", "top", "docR7.b.top"),
    ("table.border.left", "left", "docR7.b.left"),
    ("table.border.right", "right", "docR7.b.right"),
    ("table.bordersNone", "none", "docR7.b.none"),
    ("table.bordersAll", "all", "docR7.b.all"),
    ("table.bordersOutside", "outside", "docR7.b.outside"),
    ("table.bordersInside", "inside", "docR7.b.inside"),
    ("table.border.insideH", "insideH", "docR7.b.insideH"),
    ("table.border.insideV", "insideV", "docR7.b.insideV"),
];

/// Equation symbols of the Symbols gallery (the reference's Basic Math set, in its order).
pub(crate) const SYMBOLS: [char; 16] = [
    '±', '∞', '=', '≠', '~', '×', '÷', '!', '∝', '<', '≪', '>', '≫', '≤', '≥', '∓',
];

/// One structure: id suffix, label, linear text inserted.
type Structure = (&'static str, &'static str, &'static str);

/// Equation structures: menu id and its entries (id suffix, label, linear text inserted).
pub(crate) const STRUCTURES: [(&str, &[Structure]); 11] = [
    (
        "eq.fraction",
        &[
            ("stacked", "Stacked Fraction", "(□)/(□)"),
            ("skewed", "Skewed Fraction", "□⊘□"),
            ("linear", "Linear Fraction", "□∕□"),
            ("dydx", "dy/dx", "(dy)/(dx)"),
        ],
    ),
    (
        "eq.script",
        &[
            ("sup", "Superscript", "□^□"),
            ("sub", "Subscript", "□_□"),
            ("subsup", "Subscript-Superscript", "□_□^□"),
            ("xsq", "x squared", "x^2"),
        ],
    ),
    (
        "eq.radical",
        &[
            ("sqrt", "Square Root", "√(□)"),
            ("nroot", "Radical with Degree", "√(□&□)"),
            ("cbrt", "Cube Root", "∛(□)"),
        ],
    ),
    (
        "eq.integral",
        &[
            ("int", "Integral", "∫□"),
            ("intlim", "Integral with Limits", "∫_□^□ □"),
            ("dint", "Double Integral", "∬□"),
            ("oint", "Contour Integral", "∮□"),
        ],
    ),
    (
        "eq.largeOperator",
        &[
            ("sum", "Summation", "∑_(□)^□ □"),
            ("prod", "Product", "∏_(□)^□ □"),
            ("union", "Union", "⋃_(□)^□ □"),
            ("inter", "Intersection", "⋂_(□)^□ □"),
        ],
    ),
    (
        "eq.bracket",
        &[
            ("paren", "Parentheses", "(□)"),
            ("square", "Brackets", "[□]"),
            ("brace", "Braces", "{□}"),
            ("abs", "Vertical Bars", "|□|"),
        ],
    ),
    (
        "eq.function",
        &[
            ("sin", "Sine Function", "sin⁡□"),
            ("cos", "Cosine Function", "cos⁡□"),
            ("tan", "Tangent Function", "tan⁡□"),
            ("ln", "Natural Logarithm", "ln⁡□"),
        ],
    ),
    (
        "eq.accent",
        &[
            ("dot", "Dot", "□̇"),
            ("hat", "Hat", "□̂"),
            ("bar", "Bar", "□̅"),
            ("vec", "Vector", "□⃗"),
        ],
    ),
    (
        "eq.limitLog",
        &[
            ("lim", "Limit", "lim┬(□→□)⁡□"),
            ("log", "Logarithm", "log⁡□"),
            ("logn", "Logarithm base n", "log_□⁡□"),
            ("max", "Maximum", "max┬□⁡□"),
        ],
    ),
    (
        "eq.operator",
        &[
            ("def", "Equal by Definition", "≝"),
            ("colon", "Colon Equal", "≔"),
            ("arrow", "Right Arrow", "→"),
            ("delta", "Delta Equal To", "≜"),
        ],
    ),
    (
        "eq.matrix",
        &[
            ("m11", "1×2 Empty Matrix", "[■(□&□)]"),
            ("m21", "2×1 Empty Matrix", "[■(□@□)]"),
            ("m22", "2×2 Empty Matrix", "[■(□&□@□&□)]"),
            ("id2", "2×2 Identity Matrix", "[■(1&0@0&1)]"),
        ],
    ),
];

/// The linear text of structure entry `id` (`eq.fraction.stacked`).
pub(crate) fn structure_text(id: &str) -> Option<&'static str> {
    let (menu, item) = id.rsplit_once('.')?;
    STRUCTURES
        .iter()
        .find(|(m, _)| *m == menu)?
        .1
        .iter()
        .find(|(k, ..)| *k == item)
        .map(|(_, _, t)| *t)
}

/// Table Design as the reference at 1500 px: Table Style Options, the quick table looks
/// with Shading, then Borders (Border Styles, line style / weight boxes over Pen Color,
/// Borders ▾ and Border Painter).
pub(crate) fn table_design_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    let mut looks: Vec<Item> = crate::doc_table::LOOK_TILES
        .iter()
        .map(|(id, _)| L(id))
        .collect();
    looks.push(L("table.shadingMenu"));
    Tab {
        label: crate::doc_tools::TABLE_DESIGN,
        groups: vec![
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
                large: vec![
                    L("table.borderStyles"),
                    L("table.borders"),
                    L("table.borderPainter"),
                ],
                rows: vec![
                    vec![S("table.penStyle")],
                    vec![S("table.penWeight")],
                    vec![S("table.penColor")],
                ],
            },
        ],
    }
}

/// Table Layout as the reference at 1500 px.
pub(crate) fn table_layout_tab() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    let a = |i: usize| B(crate::doc_table::ALIGN[i].0);
    Tab {
        label: crate::doc_tools::TABLE_LAYOUT,
        groups: vec![
            Group {
                label: "ribbon.groups.table",
                large: vec![],
                rows: vec![
                    vec![S("table.select")],
                    vec![S("table.viewGridlines")],
                    vec![S("table.properties")],
                ],
            },
            Group {
                label: "docR7.g.draw",
                large: vec![],
                rows: vec![vec![S("table.drawTable")], vec![S("table.eraser")]],
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
                rows: vec![
                    vec![S("table.mergeCells")],
                    vec![S("table.splitCell")],
                    vec![S("table.splitTable")],
                ],
            },
            Group {
                label: "ribbon.groups.cellSize",
                large: vec![],
                rows: vec![
                    vec![S("table.rowHeight"), B("table.distributeRows")],
                    vec![S("table.colWidth"), B("table.distributeColumns")],
                    vec![S("table.autoFit")],
                ],
            },
            Group {
                label: "ribbon.groups.cellAlignment",
                large: vec![L("table.textDirection"), L("table.cellMargins")],
                rows: vec![
                    vec![a(0), a(1), a(2)],
                    vec![a(3), a(4), a(5)],
                    vec![a(6), a(7), a(8)],
                ],
            },
            Group {
                label: "ribbon.groups.tableData",
                large: vec![L("table.data")],
                rows: vec![],
            },
        ],
    }
}

/// The Arrange group shared by Picture Format and Shape Format (the Layout tab's commands).
fn arrange_group() -> Group {
    use Item::{Button as B, Labeled as S, Large as L};
    Group {
        label: "docR3.groupArrange",
        large: vec![L("picture.position"), L("picture.wrap")],
        rows: vec![
            vec![S("picture.bringForward"), B("picture.align")],
            vec![S("picture.sendBackward"), B("picture.group")],
            vec![S("layout.selectionPane"), B("picture.rotate")],
        ],
    }
}

fn accessibility_group() -> Group {
    Group {
        label: "docR7.g.accessibility",
        large: vec![Item::Large("picture.altText")],
        rows: vec![],
    }
}

/// Picture Format as the reference at 1500 px.
pub(crate) fn picture_tab() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    Tab {
        label: crate::doc_tools::PICTURE_FORMAT,
        groups: vec![
            Group {
                label: "pictureTools.groupAdjust",
                large: vec![L("picture.removeBackground"), L("picture.corrections")],
                rows: vec![
                    vec![S("picture.color"), B("picture.change")],
                    vec![S("picture.artisticEffects"), B("picture.resetMenu")],
                    vec![B("picture.compress")],
                ],
            },
            Group {
                label: "docR7.g.pictureStyles",
                large: vec![L("picture.quickStyles")],
                rows: vec![
                    vec![B("picture.border")],
                    vec![B("picture.effects")],
                    vec![B("picture.layout")],
                ],
            },
            accessibility_group(),
            arrange_group(),
            Group {
                label: "pictureTools.groupSize",
                large: vec![L("picture.crop")],
                rows: vec![vec![S("picture.height")], vec![S("picture.width")]],
            },
        ],
    }
}

/// Shape Format as the reference at 1500 px.
pub(crate) fn shape_tab() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    Tab {
        label: SHAPE_FORMAT,
        groups: vec![
            Group {
                label: "docR7.g.insertShapes",
                large: vec![L("insert.shapes")],
                rows: vec![vec![B("shape.edit")], vec![B("insert.textBox")]],
            },
            Group {
                label: "docR7.g.shapeStyles",
                large: vec![L("shape.style.0"), L("shape.style.1"), L("shape.style.2")],
                rows: vec![
                    vec![B("shape.fill")],
                    vec![B("shape.outline")],
                    vec![B("shape.effects")],
                ],
            },
            Group {
                label: "docR7.g.wordArt",
                large: vec![L("shape.wordArt")],
                rows: vec![],
            },
            accessibility_group(),
            arrange_group(),
            Group {
                label: "pictureTools.groupSize",
                large: vec![],
                rows: vec![vec![S("picture.height")], vec![S("picture.width")]],
            },
        ],
    }
}

/// Header & Footer as the reference at 1500 px.
pub(crate) fn header_footer_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: crate::doc_tools::HEADER_FOOTER,
        groups: vec![
            Group {
                label: "ribbon.groups.headerFooter",
                large: vec![],
                rows: vec![
                    vec![S("insert.header")],
                    vec![S("insert.footer")],
                    vec![S("insert.pageNumber")],
                ],
            },
            Group {
                label: "ribbon.tabs.insert",
                large: vec![L("insert.dateTime"), L("hf.documentInfo")],
                rows: vec![
                    vec![S("insert.quickParts")],
                    vec![S("hf.pictures")],
                    vec![S("hf.onlinePictures")],
                ],
            },
            Group {
                label: "hf.navigation",
                large: vec![L("hf.gotoHeader"), L("hf.gotoFooter")],
                rows: vec![
                    vec![S("hf.previous")],
                    vec![S("hf.next")],
                    vec![S("hf.linkToPrevious")],
                ],
            },
            Group {
                label: "hf.options",
                large: vec![],
                rows: vec![
                    vec![S("hf.differentFirst")],
                    vec![S("hf.differentOddEven")],
                    vec![S("hf.showDocumentText")],
                ],
            },
            Group {
                label: "docR7.g.position",
                large: vec![],
                rows: vec![
                    vec![S("hf.headerPos")],
                    vec![S("hf.footerPos")],
                    vec![Item::Button("hf.alignmentTab")],
                ],
            },
            Group {
                label: "slideEditor.close",
                large: vec![L("hf.close")],
                rows: vec![],
            },
        ],
    }
}

/// Equation as the reference at 1500 px: Tools, Conversions, the Symbols gallery and the
/// Structures.
pub(crate) fn equation_tab() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    let sym = |i: usize| B(SYMBOL_IDS[i]);
    Tab {
        label: EQUATION,
        groups: vec![
            Group {
                label: "docR7.g.tools",
                large: vec![],
                rows: vec![vec![S("eq.equation")], vec![S("eq.ink")]],
            },
            Group {
                label: "docR7.g.conversions",
                large: vec![L("eq.convert")],
                rows: vec![
                    vec![S("eq.unicode")],
                    vec![S("eq.latex")],
                    vec![S("eq.text")],
                ],
            },
            Group {
                label: "ribbon.groups.symbols",
                large: vec![],
                rows: vec![(0..8).map(sym).collect(), (8..16).map(sym).collect()],
            },
            Group {
                label: "docR7.g.structures",
                large: vec![
                    L("eq.fraction"),
                    L("eq.script"),
                    L("eq.radical"),
                    L("eq.integral"),
                    L("eq.largeOperator"),
                ],
                rows: vec![
                    vec![S("eq.bracket"), S("eq.limitLog")],
                    vec![S("eq.function"), S("eq.operator")],
                    vec![S("eq.accent"), S("eq.matrix")],
                ],
            },
        ],
    }
}

/// Command ids of the Symbols gallery.
pub(crate) const SYMBOL_IDS: [&str; 16] = [
    "eq.sym.0",
    "eq.sym.1",
    "eq.sym.2",
    "eq.sym.3",
    "eq.sym.4",
    "eq.sym.5",
    "eq.sym.6",
    "eq.sym.7",
    "eq.sym.8",
    "eq.sym.9",
    "eq.sym.10",
    "eq.sym.11",
    "eq.sym.12",
    "eq.sym.13",
    "eq.sym.14",
    "eq.sym.15",
];

/// Entries of the round-7 menus (labels are i18n keys or plain text).
pub(crate) fn menu(id: &str) -> Vec<Entry> {
    if let Some((_, list)) = STRUCTURES.iter().find(|(m, _)| *m == id) {
        return list
            .iter()
            .map(|(k, label, _)| Entry {
                id: structure_id(id, k),
                label,
            })
            .collect();
    }
    match id {
        "table.borderStyles" => BORDER_STYLES_IDS
            .iter()
            .zip(BORDER_STYLES.iter())
            .map(|(i, (.., l))| e(i, l))
            .collect(),
        "table.penStyle" => PEN_STYLE_IDS
            .iter()
            .zip(PEN_STYLES.iter())
            .map(|(i, (_, l))| e(i, l))
            .collect(),
        "table.penWeight" => PEN_WEIGHT_IDS
            .iter()
            .zip(PEN_WEIGHTS.iter())
            .map(|(i, (_, l))| e(i, l))
            .collect(),
        "table.penColor" => PEN_COLOR_IDS
            .iter()
            .zip(PEN_COLORS.iter())
            .map(|(i, (_, l))| e(i, l))
            .collect(),
        // The reference's Borders menu (the edges apply to the table here).
        "table.borders" => BORDERS
            .iter()
            .map(|(i, _, l)| e(i, l))
            .chain([
                e("table.viewGridlines", "docR7.cmd.viewGridlines"),
                // Round 12: the Borders and Shading dialog.
                e("table.bordersShading", "doc12.bsMenu"),
            ])
            .collect(),
        "table.data" => vec![
            e("table.sortAsc", "docTable.sortAsc"),
            e("table.sortDesc", "docTable.sortDesc"),
            e("table.headerRow", "docTable.repeatHeaderRows"),
            e("table.toText", "docTable.convertToText"),
            e("table.formula", "docR7.cmd.formula"),
        ],
        "picture.change" => vec![e("picture.change.file", "docR7.m.fromFile")],
        // Galleries open under these; one entry marks them as drop-downs.
        "picture.corrections" | "picture.color" => vec![e(id_static(id), "docR7.m.options")],
        "picture.artisticEffects"
        | "picture.quickStyles"
        | "picture.border"
        | "picture.effects"
        | "picture.layout" => vec![e("eq.none", "docR7.m.none")],
        "picture.resetMenu" => vec![
            e("picture.reset", "docR7.m.resetPicture"),
            e("picture.resetSize", "docR7.m.resetPictureSize"),
        ],
        "hf.documentInfo" => vec![
            e("insert.field.AUTHOR", "docR7.m.author"),
            e("insert.field.FILENAME", "docR7.m.fileName"),
            e("insert.field.FILENAME \\p", "docR7.m.filePath"),
            e("insert.field.TITLE", "docR7.m.docTitle"),
        ],
        "eq.equation" => crate::doc_r4_menus::menu("insert.equation"),
        "eq.convert" => vec![
            e("eq.convert.professional", "docR7.m.professional"),
            e("eq.convert.linear", "docR7.m.linear"),
        ],
        _ => crate::doc_review16::menu(id),
    }
}

const BORDER_STYLES_IDS: [&str; 6] = [
    "table.pen.preset.0",
    "table.pen.preset.1",
    "table.pen.preset.2",
    "table.pen.preset.3",
    "table.pen.preset.4",
    "table.pen.preset.5",
];
const PEN_STYLE_IDS: [&str; 8] = [
    "table.pen.style.0",
    "table.pen.style.1",
    "table.pen.style.2",
    "table.pen.style.3",
    "table.pen.style.4",
    "table.pen.style.5",
    "table.pen.style.6",
    "table.pen.style.7",
];
const PEN_WEIGHT_IDS: [&str; 9] = [
    "table.pen.weight.0",
    "table.pen.weight.1",
    "table.pen.weight.2",
    "table.pen.weight.3",
    "table.pen.weight.4",
    "table.pen.weight.5",
    "table.pen.weight.6",
    "table.pen.weight.7",
    "table.pen.weight.8",
];
const PEN_COLOR_IDS: [&str; 11] = [
    "table.pen.color.0",
    "table.pen.color.1",
    "table.pen.color.2",
    "table.pen.color.3",
    "table.pen.color.4",
    "table.pen.color.5",
    "table.pen.color.6",
    "table.pen.color.7",
    "table.pen.color.8",
    "table.pen.color.9",
    "table.pen.color.10",
];

/// The `'static` id of structure entry `item` of menu `menu`.
fn structure_id(menu: &str, item: &str) -> &'static str {
    STRUCTURE_IDS
        .iter()
        .find(|s| {
            s.strip_prefix(menu)
                .and_then(|r| r.strip_prefix('.'))
                .is_some_and(|r| r == item)
        })
        .copied()
        .unwrap_or("eq.none")
}

const STRUCTURE_IDS: [&str; 43] = [
    "eq.fraction.stacked",
    "eq.fraction.skewed",
    "eq.fraction.linear",
    "eq.fraction.dydx",
    "eq.script.sup",
    "eq.script.sub",
    "eq.script.subsup",
    "eq.script.xsq",
    "eq.radical.sqrt",
    "eq.radical.nroot",
    "eq.radical.cbrt",
    "eq.integral.int",
    "eq.integral.intlim",
    "eq.integral.dint",
    "eq.integral.oint",
    "eq.largeOperator.sum",
    "eq.largeOperator.prod",
    "eq.largeOperator.union",
    "eq.largeOperator.inter",
    "eq.bracket.paren",
    "eq.bracket.square",
    "eq.bracket.brace",
    "eq.bracket.abs",
    "eq.function.sin",
    "eq.function.cos",
    "eq.function.tan",
    "eq.function.ln",
    "eq.accent.dot",
    "eq.accent.hat",
    "eq.accent.bar",
    "eq.accent.vec",
    "eq.limitLog.lim",
    "eq.limitLog.log",
    "eq.limitLog.logn",
    "eq.limitLog.max",
    "eq.operator.def",
    "eq.operator.colon",
    "eq.operator.arrow",
    "eq.operator.delta",
    "eq.matrix.m11",
    "eq.matrix.m21",
    "eq.matrix.m22",
    "eq.matrix.id2",
];

fn id_static(id: &str) -> &'static str {
    static_id(id).unwrap_or("eq.none")
}

/// Commands of the round-7 tabs that the window runs (`doc_r7`), or the Insert / Layout
/// tabs' commands they reuse (`doc_r4`).
#[cfg(test)]
pub(crate) fn window_command(id: &str) -> bool {
    const IDS: [&str; 14] = [
        "table.borderStyles",
        "table.borderPainter",
        "table.viewGridlines",
        "table.formula",
        "table.penStyle",
        "table.penWeight",
        "table.penColor",
        "picture.removeBackground",
        "picture.corrections",
        "picture.color",
        "picture.compress",
        "picture.altText",
        "picture.height",
        "picture.width",
    ];
    IDS.contains(&id)
        || GREYED.contains(&id)
        || MENUS.contains(&id)
        || [
            "eq.",
            "shape.",
            "layout.",
            "insert.",
            "hf.",
            "table.pen.",
            "table.border.",
        ]
        .iter()
        .any(|p| id.starts_with(p))
}

/// The `'static` id of a round-7 menu button.
pub(crate) fn static_id(id: &str) -> Option<&'static str> {
    MENUS.iter().find(|m| **m == id).copied()
}

/// The pen a menu entry sets (`None`: not a pen entry).
pub(crate) fn pen_entry(id: &str, pen: &Pen) -> Option<Pen> {
    let rest = id.strip_prefix("table.pen.")?;
    let (kind, n) = rest.rsplit_once('.')?;
    let i: usize = n.parse().ok()?;
    let mut p = pen.clone();
    match kind {
        "preset" => {
            let (style, size, _) = BORDER_STYLES.get(i)?;
            p.style = style;
            p.size = *size;
        }
        "style" => p.style = PEN_STYLES.get(i)?.0,
        "weight" => p.size = PEN_WEIGHTS.get(i)?.0,
        "color" => {
            let c = PEN_COLORS.get(i)?.0;
            p.color = (c != "auto").then_some(c);
        }
        _ => return None,
    }
    Some(p)
}

/// The name of the pen's weight (`½ pt`).
pub(crate) fn weight_name(size: u32) -> String {
    PEN_WEIGHTS.iter().find(|(s, _)| *s == size).map_or_else(
        || format!("{} pt", f64::from(size) / 8.0),
        |(_, n)| (*n).to_owned(),
    )
}

/// Whether round-7 command `id` can run (`None`: not a round-7 command).
pub(crate) fn enabled(id: &str, f: &FormatState) -> Option<bool> {
    if GREYED.contains(&id) {
        return Some(false);
    }
    let in_table = f.table.is_some();
    let picture = f.picture.is_some() || f.picture_tools;
    Some(match id {
        _ if id.starts_with("table.pen") || id.starts_with("table.border.") => in_table,
        "table.borderStyles" | "table.borderPainter" | "table.viewGridlines" | "table.formula" => {
            in_table
        }
        "picture.removeBackground"
        | "picture.corrections"
        | "picture.color"
        | "picture.compress"
        | "picture.change"
        | "picture.resetMenu"
        | "picture.resetSize"
        | "picture.altText"
        | "picture.height"
        | "picture.width" => picture,
        _ if id.starts_with("picture.change.") => picture,
        _ if id.starts_with("shape.") => picture,
        _ if id.starts_with("hf.") => true,
        _ if id.starts_with("eq.") => true,
        _ => return None,
    })
}

/// Group label and command names (English fallbacks; translations in `docR7`).
pub(crate) fn install_strings(s: &mut Strings) {
    // `commands.<id>` from `docR7.cmd.<name>`.
    for (id, key) in [
        ("table.borderStyles", "borderStyles"),
        ("table.penStyle", "penStyle"),
        ("table.penWeight", "penWeight"),
        ("table.penColor", "penColor"),
        ("table.borderPainter", "borderPainter"),
        ("table.viewGridlines", "viewGridlines"),
        ("table.drawTable", "drawTable"),
        ("table.eraser", "eraser"),
        ("table.formula", "formula"),
        ("picture.removeBackground", "removeBackground"),
        ("picture.corrections", "corrections"),
        ("picture.color", "color"),
        ("picture.artisticEffects", "artisticEffects"),
        ("picture.compress", "compress"),
        ("picture.change", "change"),
        ("picture.resetMenu", "resetPicture"),
        ("picture.quickStyles", "quickStyles"),
        ("picture.border", "pictureBorder"),
        ("picture.effects", "pictureEffects"),
        ("picture.layout", "pictureLayout"),
        ("picture.altText", "altText"),
        ("picture.height", "height"),
        ("picture.width", "width"),
        ("shape.edit", "editShape"),
        ("shape.fill", "shapeFill"),
        ("shape.outline", "shapeOutline"),
        ("shape.effects", "shapeEffects"),
        ("shape.wordArt", "quickStyles"),
        ("shape.style.0", "styleOutline"),
        ("shape.style.1", "styleDark"),
        ("shape.style.2", "styleAccent"),
        ("hf.documentInfo", "documentInfo"),
        ("hf.pictures", "pictures"),
        ("hf.onlinePictures", "onlinePictures"),
        ("hf.previous", "previous"),
        ("hf.next", "next"),
        ("hf.showDocumentText", "showDocumentText"),
        ("hf.headerPos", "headerPos"),
        ("hf.footerPos", "footerPos"),
        ("hf.alignmentTab", "alignmentTab"),
        ("hf.close", "closeHeaderFooter"),
        ("eq.equation", "equation"),
        ("eq.ink", "inkEquation"),
        ("eq.unicode", "unicode"),
        ("eq.latex", "latex"),
        ("eq.text", "text"),
        ("eq.convert", "convert"),
        ("eq.fraction", "fraction"),
        ("eq.script", "script"),
        ("eq.radical", "radical"),
        ("eq.integral", "integral"),
        ("eq.largeOperator", "largeOperator"),
        ("eq.bracket", "bracket"),
        ("eq.function", "function"),
        ("eq.accent", "accent"),
        ("eq.limitLog", "limitLog"),
        ("eq.operator", "operator"),
        ("eq.matrix", "matrix"),
        ("review.checkAccessibility", "checkAccessibility"),
        ("hf.gotoHeader", "gotoHeader"),
        ("hf.gotoFooter", "gotoFooter"),
        ("hf.differentFirst", "differentFirst"),
        ("hf.differentOddEven", "differentOddEven"),
        ("hf.linkToPrevious", "linkToPrevious"),
        ("insert.header", "header"),
        ("insert.footer", "footer"),
        ("insert.pageNumber", "pageNumber"),
        ("insert.dateTime", "dateTime"),
        ("insert.quickParts", "quickParts"),
    ] {
        let k = format!("docR7.cmd.{key}");
        let v = s.get(&k).to_owned();
        if v != k {
            s.set(&format!("commands.{id}"), &v);
        }
    }
    for (i, c) in SYMBOLS.iter().enumerate() {
        s.set(&format!("commands.{}", SYMBOL_IDS[i]), &c.to_string());
    }
    for (key, name) in [
        (crate::doc_tools::HEADER_FOOTER, "headerFooter"),
        (SHAPE_FORMAT, "shapeFormat"),
        (EQUATION, "equation"),
        ("docR7.g.draw", "draw"),
        ("docR7.g.accessibility", "accessibility"),
        ("docR7.g.pictureStyles", "pictureStyles"),
        ("docR7.g.insertShapes", "insertShapes"),
        ("docR7.g.shapeStyles", "shapeStyles"),
        ("docR7.g.wordArt", "wordArtStyles"),
        ("docR7.g.position", "position"),
        ("docR7.g.tools", "tools"),
        ("docR7.g.conversions", "conversions"),
        ("docR7.g.structures", "structures"),
    ] {
        let k = format!("docR7.grp.{name}");
        let v = s.get(&k).to_owned();
        if v != k {
            s.set(key, &v);
        }
    }
}

/// Fixed widths (logical px) of the round-7 labeled items: the pen boxes (reference 140 px
/// at 125 %), the size and position boxes, the check box.
pub(crate) fn labeled_w(id: &str) -> Option<f32> {
    match id {
        "table.penStyle" | "table.penWeight" => Some(106.0),
        "picture.height" | "picture.width" | "hf.headerPos" | "hf.footerPos" => Some(104.0),
        "hf.showDocumentText" => Some(180.0),
        _ => None,
    }
}

/// Large items of a group placed after its rows: Borders ▾ and Border Painter follow the
/// pen boxes; Convert follows the Unicode / LaTeX / Text column.
pub(crate) fn trailing(id: &str) -> bool {
    matches!(id, "table.borders" | "table.borderPainter" | "eq.convert")
}

/// Width (logical px) of the shape style tiles (reference 66 px at 125 %).
pub(crate) fn large_w(id: &str) -> Option<f32> {
    id.starts_with("shape.style.").then_some(66.0)
}

/// Icons (BlueOffice's own drawings in the shared filled style).
pub(crate) fn install_icons(icons: &mut Icons) {
    let rgb = |r, g, b| Some(Rgba(r, g, b, 255));
    let white = rgb(255, 255, 255);
    let blue = Some(crate::icons::ICON_BLUE);
    let sky = rgb(0xcf, 0xe4, 0xf6);
    let hill = rgb(0x3f, 0x8f, 0x4d);
    // Border Styles: a cell with a thick blue lower edge over a paint drop.
    icons.add_solid(
        "table.borderStyles",
        &[
            ("M3 5h18v12h-18Z", white),
            ("M3 5h18v12h-18Z", None),
            ("M3 15h18v3h-18Z", blue),
        ],
    );
    // Borders: a 2 × 2 grid with a bold outline.
    icons.add_solid(
        "table.borders",
        &[
            ("M3 3h18v18h-18Z", white),
            ("M12 4v16M4 12h16", None),
            (
                "M2 2h20v2h-20ZM2 20h20v2h-20ZM2 2h2v20h-2ZM20 2h2v20h-2Z",
                rgb(60, 60, 60),
            ),
        ],
    );
    // Border Painter: a brush crossing a cell corner.
    icons.add_solid(
        "table.borderPainter",
        &[
            ("M3 9h11v12h-11Z", white),
            ("M3 9h11v12h-11Z", None),
            ("M14 3l6 6l-7 7l-3-3Z", rgb(0x9a, 0xb7, 0xd8)),
            ("M14 3l6 6l-7 7l-3-3Z", None),
            ("M10 13l3 3l-4 3l-2-2Z", blue),
        ],
    );
    // Pen Color: a pencil over a colour bar (the bar is drawn in the pen's colour).
    icons.add_solid(
        "table.penColor",
        &[
            ("M16 2l4 4l-10 10l-5 1l1-5Z", white),
            ("M16 2l4 4l-10 10l-5 1l1-5Z", None),
            ("M3 19h18v3h-18Z", rgb(20, 20, 20)),
        ],
    );
    for (id, existing) in [
        ("table.viewGridlines", "view.gridlines"),
        ("table.formula", "insert.equation"),
        ("picture.artisticEffects", "insert.picture"),
        ("picture.quickStyles", "insert.picture"),
        ("picture.border", "format.color"),
        ("picture.effects", "format.textEffects"),
        ("picture.layout", "insert.smartArt"),
        ("picture.resetMenu", "edit.undo"),
        ("picture.height", "layout.size"),
        ("picture.width", "layout.size"),
        ("shape.edit", "insert.shapes"),
        ("shape.fill", "format.fillColor"),
        ("shape.outline", "format.color"),
        ("shape.effects", "format.textEffects"),
        ("shape.wordArt", "insert.wordArt"),
        ("hf.pictures", "insert.picture"),
        ("hf.onlinePictures", "insert.picture"),
        ("hf.previous", "insert.header"),
        ("hf.next", "insert.footer"),
        ("hf.alignmentTab", "format.alignLeft"),
        ("eq.equation", "insert.equation"),
        ("eq.ink", "insert.equation"),
    ] {
        if !icons.has(id) {
            icons.alias(id, existing);
        }
    }
    // Draw Table: a pencil over a cell grid; Eraser: a tilted block (own drawings).
    icons.add_solid(
        "table.drawTable",
        &[
            ("M3 6h12v12h-12Z", white),
            ("M3 6h12v12h-12ZM9 6v12M3 12h12", None),
            ("M19 3l2 2l-8 8l-3 1l1-3Z", rgb(0xf2, 0xa5, 0x41)),
        ],
    );
    icons.add_solid(
        "table.eraser",
        &[
            ("M9 20l-5-5l9-9l7 7l-7 7Z", rgb(0xe6, 0x9a, 0xb4)),
            ("M4 15l5 5h4l-4-4Z", white),
            ("M9 20l-5-5l9-9l7 7l-7 7ZM4 21h16", None),
        ],
    );
    let frame = "M3 4h18v16h-18Z";
    // Remove Background: a picture whose left half is a checkerboard (transparent).
    icons.add_solid(
        "picture.removeBackground",
        &[
            ("M12 4h9v16h-9Z", sky),
            ("M12 20l4-6l5 6Z", hill),
            (
                "M3 4h3v3h-3ZM9 4h3v3h-3ZM6 7h3v3h-3ZM3 10h3v3h-3ZM9 10h3v3h-3ZM6 13h3v3h-3ZM3 16h3v3h-3ZM9 16h3v3h-3Z",
                rgb(0xc8, 0xc8, 0xc8),
            ),
            (frame, None),
        ],
    );
    // Corrections: a sun.
    icons.add_solid(
        "picture.corrections",
        &[
            ("M12 7a5 5 0 1 0 0.01 0Z", rgb(0xf5, 0xb8, 0x2e)),
            (
                "M11 1h2v4h-2ZM11 19h2v4h-2ZM1 11h4v2h-4ZM19 11h4v2h-4ZM4.2 5.6l1.4-1.4l2.8 2.8l-1.4 1.4ZM15.6 17l1.4-1.4l2.8 2.8l-1.4 1.4ZM4.2 18.4l2.8-2.8l1.4 1.4l-2.8 2.8ZM15.6 7l2.8-2.8l1.4 1.4l-2.8 2.8Z",
                rgb(0xf5, 0xb8, 0x2e),
            ),
        ],
    );
    // Color: a picture in three bands of colour.
    icons.add_solid(
        "picture.color",
        &[
            ("M3 4h6v16h-6Z", rgb(0xd2, 0x4a, 0x3c)),
            ("M9 4h6v16h-6Z", rgb(0x3f, 0xa3, 0x4d)),
            ("M15 4h6v16h-6Z", blue),
            (frame, None),
        ],
    );
    // Compress Pictures: a picture with arrows pressing inwards.
    icons.add_solid(
        "picture.compress",
        &[
            ("M6 7h12v10h-12Z", sky),
            ("M6 17l4-5l3 3l2-2l3 4Z", hill),
            ("M6 7h12v10h-12Z", None),
            ("M1 1l5 5M23 1l-5 5M1 23l5-5M23 23l-5-5", None),
        ],
    );
    // Change Picture: a picture with a turning arrow.
    icons.add_solid(
        "picture.change",
        &[
            ("M3 4h14v12h-14Z", sky),
            ("M3 16l5-6l3 3l2-2l4 5Z", hill),
            ("M3 4h14v12h-14Z", None),
            ("M14 20a5 5 0 0 0 8-4", None),
            ("M22 13l1.5 4h-4Z", blue),
        ],
    );
    // Alt Text: a picture with a caption box under its corner.
    icons.add_solid(
        "picture.altText",
        &[
            ("M2 3h15v12h-15Z", sky),
            ("M2 15l5-6l4 4l3-2l3 4Z", hill),
            ("M2 3h15v12h-15Z", None),
            ("M10 14h12v8h-12Z", white),
            ("M10 14h12v8h-12ZM12 17h8M12 19.5h6", None),
        ],
    );
    // Shape style tiles: drawn by the window (white placeholder here).
    for id in ["shape.style.0", "shape.style.1", "shape.style.2"] {
        icons.add_solid(id, &[("M2 5h20v14h-20Z", white)]);
    }
    // Check Accessibility: a page with a person badge and a tick (own drawing).
    icons.add_solid(
        "review.checkAccessibility",
        &[
            ("M4 2h11l4 4v16h-15Z", white),
            ("M4 2h11l4 4v16h-15ZM15 2v4h4M7 10h6M7 13h4", None),
            ("M16 12a5 5 0 1 0 0.01 0Z", blue),
            ("M14 17l1.5 1.5l3-3", None),
        ],
    );
    // Document Info: a page with an "i" badge.
    icons.add_solid(
        "hf.documentInfo",
        &[
            ("M4 2h11l4 4v16h-15Z", white),
            ("M4 2h11l4 4v16h-15ZM15 2v4h4M7 10h9M7 13h9M7 16h5", None),
            ("M17 14a5 5 0 1 0 0.01 0Z", blue),
            ("M16.4 17h1.2v5h-1.2ZM16.4 15.2h1.2v1.2h-1.2Z", white),
        ],
    );
    // Equation conversions and structures: own letterforms on a page-less ground.
    icons.add_solid(
        "eq.convert",
        &[
            ("M3 3h12v14h-12Z", white),
            ("M3 3h12v14h-12ZM6 8h6M6 12h6", None),
            ("M14 17a6 6 0 0 0 8-6", None),
            ("M22 8l1.5 4h-4Z", blue),
        ],
    );
    let structure = |icons: &mut Icons, id: &str, d: &str| {
        icons.add_solid(id, &[(d, None)]);
    };
    structure(icons, "eq.fraction", "M8 3h8v6h-8ZM4 12h16M8 15h8v6h-8Z");
    structure(icons, "eq.script", "M3 9h10v12h-10ZM15 3h6v6h-6Z");
    structure(icons, "eq.radical", "M2 13l3-1l4 9l5-18h8M12 8h8v10h-8Z");
    structure(
        icons,
        "eq.integral",
        "M14 3c-3 0-3 3-3 6v8c0 3 0 4-3 4M16 8h5v8h-5Z",
    );
    structure(
        icons,
        "eq.largeOperator",
        "M5 4h13l-7 8l7 8h-13M15 2h3M15 22h3",
    );
    structure(
        icons,
        "eq.bracket",
        "M7 3c-4 4-4 14 0 18M17 3c4 4 4 14 0 18M10 9h4v6h-4Z",
    );
    structure(
        icons,
        "eq.function",
        "M3 18c3 0 3-12 6-12M3 11h6M13 9h8v8h-8Z",
    );
    structure(icons, "eq.accent", "M8 8h8v12h-8ZM9 4h6M12 2v4");
    structure(
        icons,
        "eq.limitLog",
        "M3 4v10h5M10 6v8M10 3v1M13 8v6M13 10c0-3 5-3 5 0v4M7 18h10v4h-10Z",
    );
    structure(icons, "eq.operator", "M5 9h14M5 15h14M12 3l3 3l-3 3");
    structure(
        icons,
        "eq.matrix",
        "M5 3h-2v18h2M19 3h2v18h-2M7 6h4v4h-4ZM13 6h4v4h-4ZM7 14h4v4h-4ZM13 14h4v4h-4Z",
    );
    // Unicode / LaTeX / Text: their marks are lettered by the window over a blank icon.
    for id in ["eq.unicode", "eq.latex", "eq.text"] {
        icons.add_solid(id, &[("M12 12h0.01Z", None)]);
    }
    for id in SYMBOL_IDS {
        icons.add_solid(id, &[("M12 12h0.01Z", None)]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_has_entries_and_every_structure_inserts_text() {
        for id in MENUS {
            assert!(!menu(id).is_empty(), "{id}");
        }
        for (m, list) in STRUCTURES {
            for (k, ..) in list {
                let id = format!("{m}.{k}");
                assert_eq!(structure_id(m, k), id, "static id of {id}");
                assert!(structure_text(&id).is_some(), "{id}");
            }
        }
    }

    #[test]
    fn pen_entries_change_one_property() {
        let p = Pen::default();
        assert_eq!(
            pen_entry("table.pen.style.5", &p).map(|p| p.style),
            Some("double")
        );
        assert_eq!(
            pen_entry("table.pen.weight.4", &p).map(|p| p.size),
            Some(12)
        );
        let c = pen_entry("table.pen.color.2", &p).expect("red");
        assert_eq!((c.color, c.style, c.size), (Some("FF0000"), "single", 4));
        assert_eq!(
            pen_entry("table.pen.color.0", &c).and_then(|p| p.color),
            None
        );
        let s = pen_entry("table.pen.preset.5", &p).expect("thick-thin");
        assert_eq!((s.style, s.size), ("thickThinSmallGap", 24));
        assert_eq!(weight_name(12), "1½ pt");
    }
}
