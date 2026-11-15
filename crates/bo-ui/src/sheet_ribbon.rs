//! BlueSheet ribbon tabs and drop-down menus (ADR-0018, N6 round 2). The tab and group
//! layout follows a desktop spreadsheet observed black-box (Home, Insert, Formulas, Data,
//! View); the commands are BlueSheet's own engine commands (`SheetCommand`, `autoSum`).
use crate::ribbon::{Group, Item, Tab};

/// One menu entry: choice id (run by the window) and label (an i18n key when it holds a dot,
/// else a literal such as a style or function name).
pub(crate) type Entry = (String, String);

fn e(id: &str, label: &str) -> Entry {
    (id.to_owned(), label.to_owned())
}

/// Number format box entries: id suffix, label key, format code (`NUMBER_PRESETS` ids of
/// the engine; the desktop spreadsheet's list order).
pub(crate) const NUMBER_FORMATS: [(&str, &str); 11] = [
    ("general", "sheetMenu.numFmt.general"),
    ("number", "sheetMenu.numFmt.number"),
    ("currencyVnd", "sheetMenu.numFmt.currency"),
    ("accounting", "sheetMenu.numFmt.accounting"),
    ("date", "sheetMenu.numFmt.shortDate"),
    ("dateLong", "sheetMenu.numFmt.longDate"),
    ("time", "sheetMenu.numFmt.time"),
    ("percent2", "sheetMenu.numFmt.percentage"),
    ("fraction", "sheetMenu.numFmt.fraction"),
    ("scientific", "sheetMenu.numFmt.scientific"),
    ("text", "sheetMenu.numFmt.text"),
];

/// Format code of number format preset `id`.
pub(crate) fn number_code(id: &str) -> Option<&'static str> {
    bo_sheet::style::NUMBER_PRESETS
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, c)| *c)
}

/// Label key of the number format box for a cell's format `code` (its category).
pub(crate) fn number_category(code: &str) -> &'static str {
    let c = code.trim();
    if c.is_empty() || c.eq_ignore_ascii_case("general") {
        return "sheetMenu.numFmt.general";
    }
    if c == "@" {
        return "sheetMenu.numFmt.text";
    }
    // Tokens outside quotes and brackets ([$₫-42A] keeps its symbol).
    let mut bare = String::new();
    let mut chars = c.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                for n in chars.by_ref() {
                    if n == '"' {
                        break;
                    }
                }
            }
            '[' => {
                let inner: String = chars.by_ref().take_while(|n| *n != ']').collect();
                if let Some(sym) = inner.strip_prefix('$') {
                    bare.push_str(sym.split('-').next().unwrap_or(""));
                }
            }
            '\\' => {
                chars.next();
            }
            _ => bare.push(ch),
        }
    }
    let has = |p: &[char]| bare.chars().any(|ch| p.contains(&ch));
    if (c.contains("_-*") || c.contains("* ")) && has(&['0', '#']) {
        "sheetMenu.numFmt.accounting"
    } else if has(&['₫', '$', '€', '£', '¥']) {
        "sheetMenu.numFmt.currency"
    } else if bare.contains('%') {
        "sheetMenu.numFmt.percentage"
    } else if bare.contains("E+") || bare.contains("e+") || bare.contains("E-") {
        "sheetMenu.numFmt.scientific"
    } else if bare.contains("?/") {
        "sheetMenu.numFmt.fraction"
    } else if has(&['d', 'y', 'D', 'Y']) {
        let low = bare.to_ascii_lowercase();
        if low.contains("dddd") || low.contains("mmmm") {
            "sheetMenu.numFmt.longDate"
        } else {
            "sheetMenu.numFmt.shortDate"
        }
    } else if has(&['h', 's', 'H', 'S']) {
        "sheetMenu.numFmt.time"
    } else if has(&['0', '#']) {
        "sheetMenu.numFmt.number"
    } else {
        "sheetMenu.numFmt.custom"
    }
}

/// Function library categories (Formulas tab) with common worksheet functions; the menu
/// lists those the engine knows.
pub(crate) const FUNCTION_CATEGORIES: [(&str, &[&str]); 7] = [
    (
        "sheet.fnFinancial",
        &[
            "FV", "IPMT", "IRR", "NPER", "NPV", "PMT", "PPMT", "PV", "RATE", "SLN", "XIRR", "XNPV",
        ],
    ),
    (
        "sheet.fnLogical",
        &[
            "AND", "FALSE", "IF", "IFERROR", "IFNA", "IFS", "NOT", "OR", "SWITCH", "TRUE", "XOR",
        ],
    ),
    (
        "sheet.fnText",
        &[
            "CONCAT",
            "EXACT",
            "FIND",
            "LEFT",
            "LEN",
            "LOWER",
            "MID",
            "PROPER",
            "REPLACE",
            "RIGHT",
            "SEARCH",
            "SUBSTITUTE",
            "TEXT",
            "TEXTJOIN",
            "TRIM",
            "UPPER",
            "VALUE",
        ],
    ),
    (
        "sheet.fnDateTime",
        &[
            "DATE",
            "DATEDIF",
            "DATEVALUE",
            "DAY",
            "DAYS",
            "EDATE",
            "EOMONTH",
            "HOUR",
            "MINUTE",
            "MONTH",
            "NETWORKDAYS",
            "NOW",
            "SECOND",
            "TIME",
            "TODAY",
            "WEEKDAY",
            "WEEKNUM",
            "WORKDAY",
            "YEAR",
        ],
    ),
    (
        "sheet.fnLookup",
        &[
            "ADDRESS",
            "CHOOSE",
            "COLUMN",
            "COLUMNS",
            "HLOOKUP",
            "INDEX",
            "INDIRECT",
            "LOOKUP",
            "MATCH",
            "OFFSET",
            "ROW",
            "ROWS",
            "TRANSPOSE",
            "VLOOKUP",
            "XLOOKUP",
        ],
    ),
    (
        "sheet.fnMath",
        &[
            "ABS",
            "CEILING",
            "EXP",
            "FLOOR",
            "INT",
            "LN",
            "LOG10",
            "MOD",
            "PI",
            "POWER",
            "PRODUCT",
            "RAND",
            "RANDBETWEEN",
            "ROUND",
            "ROUNDDOWN",
            "ROUNDUP",
            "SQRT",
            "SUM",
            "SUMIF",
            "SUMIFS",
            "SUMPRODUCT",
            "TRUNC",
        ],
    ),
    (
        "sheet.fnMore",
        &[
            "AVERAGE",
            "AVERAGEIF",
            "COUNT",
            "COUNTA",
            "COUNTBLANK",
            "COUNTIF",
            "COUNTIFS",
            "LARGE",
            "MAX",
            "MEDIAN",
            "MIN",
            "MODE",
            "RANK",
            "SMALL",
            "STDEV",
            "VAR",
        ],
    ),
];

/// Entries of menu button `id` (empty: not a menu). `functions` are the engine's
/// worksheet function names (function library menus).
pub(crate) fn menu(id: &str, functions: &[String]) -> Vec<Entry> {
    let known = |name: &str| functions.is_empty() || functions.iter().any(|f| f == name);
    if let Some((_, names)) = FUNCTION_CATEGORIES.iter().find(|(k, _)| *k == id) {
        return names
            .iter()
            .filter(|n| known(n))
            .map(|n| e(&format!("fn.{n}"), n))
            .collect();
    }
    match id {
        "sheet.numberFormat" => NUMBER_FORMATS
            .iter()
            .map(|(k, label)| e(&format!("numfmt.{k}"), label))
            .chain([e("sheet.formatCellsDialog", "sheetMenu.numFmt.more")])
            .collect(),
        "sheet.autoSum" | "sheet.autoSumF" => vec![
            e("autosum.sum", "sheetMenu.autoSum.sum"),
            e("autosum.average", "sheetMenu.autoSum.average"),
            e("autosum.count", "sheetMenu.autoSum.count"),
            e("autosum.max", "sheetMenu.autoSum.max"),
            e("autosum.min", "sheetMenu.autoSum.min"),
            e("sheet.insertFunction", "sheetMenu.autoSum.more"),
        ],
        "sheet.fill" => vec![
            e("fill.down", "sheetMenu.fill.down"),
            e("fill.right", "sheetMenu.fill.right"),
            e("fill.up", "sheetMenu.fill.up"),
            e("fill.left", "sheetMenu.fill.left"),
            e("fill.series", "sheetMenu.fill.series"),
        ],
        "sheet.clear" => vec![
            e("clear.all", "sheetMenu.clear.all"),
            e("clear.formats", "sheetMenu.clear.formats"),
            e("clear.contents", "sheetMenu.clear.contents"),
        ],
        "sheet.sortFilter" => vec![
            e("sort.asc", "sheetMenu.sort.asc"),
            e("sort.desc", "sheetMenu.sort.desc"),
            e("filter.toggle", "sheetMenu.sort.filter"),
            e("filter.clear", "sheetMenu.sort.clear"),
        ],
        "sheet.findSelect" => vec![
            e("find.find", "sheetMenu.find.find"),
            e("find.replace", "sheetMenu.find.replace"),
            e("find.goto", "sheetMenu.find.goto"),
        ],
        "sheet.conditional" => vec![
            e("cf.greater", "sheetMenu.cf.greater"),
            e("cf.less", "sheetMenu.cf.less"),
            e("cf.duplicates", "sheetMenu.cf.duplicates"),
            e("cf.top10", "sheetMenu.cf.top10"),
            e("cf.bottom10", "sheetMenu.cf.bottom10"),
            e("cf.barBlue", "sheetMenu.cf.barBlue"),
            e("cf.barGreen", "sheetMenu.cf.barGreen"),
            e("cf.scaleGyr", "sheetMenu.cf.scaleGyr"),
            e("cf.scaleRwb", "sheetMenu.cf.scaleRwb"),
            e("cf.icons3Arrows", "sheetMenu.cf.icons3Arrows"),
            e("cf.icons3Traffic", "sheetMenu.cf.icons3Traffic"),
            e("cf.icons5Quarters", "sheetMenu.cf.icons5Quarters"),
            e("cf.clearSel", "sheetMenu.cf.clearSel"),
            e("cf.clearSheet", "sheetMenu.cf.clearSheet"),
        ],
        "sheet.formatTable" => vec![
            e("table.TableStyleLight9", "sheetMenu.table.light"),
            e("table.TableStyleMedium2", "sheetMenu.table.medium"),
            e("table.TableStyleDark1", "sheetMenu.table.dark"),
        ],
        "sheet.cellStyles" => bo_sheet::style::NAMED_STYLES
            .iter()
            .map(|s| e(&format!("style.{}", s.name), s.name))
            .collect(),
        "sheet.insertCells" => vec![
            e("cells.insertRow", "sheetMenu.cells.insertRows"),
            e("cells.insertCol", "sheetMenu.cells.insertCols"),
            e("sheet.addSheet", "sheetMenu.cells.insertSheet"),
        ],
        "sheet.deleteCells" => vec![
            e("cells.deleteRow", "sheetMenu.cells.deleteRows"),
            e("cells.deleteCol", "sheetMenu.cells.deleteCols"),
        ],
        "sheet.formatCells" => vec![
            e("cells.rowHeight", "sheetMenu.cells.autofitRow"),
            e("cells.autofitCol", "sheetMenu.cells.autofitCol"),
            e("cells.hideRows", "sheetMenu.cells.hideRows"),
            e("cells.hideCols", "sheetMenu.cells.hideCols"),
            e("cells.unhideRows", "sheetMenu.cells.unhideRows"),
            e("cells.unhideCols", "sheetMenu.cells.unhideCols"),
            e("sheet.formatCellsDialog", "sheetMenu.cells.formatCells"),
        ],
        "sheet.chartColumn" => vec![
            e("chart.column", "sheetMenu.chart.column"),
            e("chart.bar", "sheetMenu.chart.bar"),
        ],
        "sheet.chartLine" => vec![
            e("chart.line", "sheetMenu.chart.line"),
            e("chart.area", "sheetMenu.chart.area"),
        ],
        "sheet.chartPie" => vec![
            e("chart.pie", "sheetMenu.chart.pie"),
            e("chart.doughnut", "sheetMenu.chart.doughnut"),
        ],

        "sheet.freezePanes" => vec![
            e("freeze.panes", "sheetMenu.freeze.panes"),
            e("freeze.row", "sheetMenu.freeze.row"),
            e("freeze.col", "sheetMenu.freeze.col"),
            e("freeze.none", "sheetMenu.freeze.none"),
        ],
        _ => Vec::new(),
    }
}

/// Whether `id` is one of BlueSheet's menu buttons (the ribbon draws its drop-down arrow).
pub(crate) fn is_menu(id: &str) -> bool {
    id.starts_with("sheet.") && !menu(id, &[]).is_empty() && id != "sheet.numberFormat"
}

fn home() -> Tab {
    use Item::{Button as B, FontBox, Labeled as S, Large as L, SizeBox};
    Tab {
        label: "ribbon.tabs.home",
        groups: vec![
            Group {
                label: "ribbon.groups.clipboard",
                large: vec![L("edit.paste")],
                rows: vec![
                    vec![B("edit.cut")],
                    vec![B("edit.copy")],
                    vec![B("format.painter")],
                ],
            },
            Group {
                label: "sheetEditor.groupFont",
                large: vec![],
                rows: vec![
                    vec![
                        FontBox,
                        SizeBox,
                        B("format.growStep"),
                        B("format.shrinkStep"),
                    ],
                    vec![
                        B("format.bold"),
                        B("format.italic"),
                        B("format.underline"),
                        B("format.borders"),
                        B("format.fillColor"),
                        B("format.color"),
                    ],
                ],
            },
            Group {
                label: "sheetEditor.groupAlignment",
                large: vec![],
                rows: vec![
                    vec![
                        B("format.alignTop"),
                        B("format.alignMiddle"),
                        B("format.alignBottom"),
                        B("format.wrap"),
                    ],
                    vec![
                        B("format.alignLeft"),
                        B("format.alignCenter"),
                        B("format.alignRight"),
                        B("format.outdent"),
                        B("format.indent"),
                        B("format.mergeCenter"),
                    ],
                ],
            },
            Group {
                label: "sheetEditor.groupNumber",
                large: vec![],
                // Box, then currency / percent / thousands, then the decimal steps (three
                // rows, as on the reference spreadsheet's ribbon).
                rows: vec![
                    vec![S("sheet.numberFormat")],
                    vec![B("format.currency"), B("format.percent"), B("format.comma")],
                    vec![B("format.decimalInc"), B("format.decimalDec")],
                ],
            },
            Group {
                label: "sheetEditor.groupStyles",
                large: vec![],
                rows: vec![
                    vec![S("sheet.conditional")],
                    vec![S("sheet.formatTable")],
                    vec![S("sheet.cellStyles")],
                ],
            },
            Group {
                label: "sheetEditor.groupCells",
                large: vec![],
                rows: vec![
                    vec![S("sheet.insertCells")],
                    vec![S("sheet.deleteCells")],
                    vec![S("sheet.formatCells")],
                ],
            },
            Group {
                label: "sheetEditor.groupEditing",
                large: vec![],
                // Icon menu buttons in two columns.
                rows: vec![
                    vec![B("sheet.autoSum"), B("sheet.sortFilter")],
                    vec![B("sheet.fill"), B("sheet.findSelect")],
                    vec![B("sheet.clear")],
                ],
            },
        ],
    }
}

/// Home tab of a wide window (the reference at 1800 px and more): number format box 196 px
/// over one row of number buttons, large style buttons, and Sort & Filter / Find & Select as
/// large buttons after the editing icon column.
fn home_wide() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    let mut t = home();
    for g in &mut t.groups {
        match g.label {
            "sheetEditor.groupNumber" => {
                g.rows = vec![
                    vec![S("sheet.numberFormat")],
                    vec![
                        B("format.currency"),
                        B("format.percent"),
                        B("format.comma"),
                        B("format.decimalInc"),
                        B("format.decimalDec"),
                    ],
                ];
            }
            "sheetEditor.groupStyles" => {
                g.large = vec![
                    L("sheet.conditional"),
                    L("sheet.formatTable"),
                    L("sheet.cellStyles"),
                ];
                g.rows = Vec::new();
            }
            "sheetEditor.groupEditing" => {
                g.large = vec![L("sheet.sortFilter"), L("sheet.findSelect")];
                g.rows = vec![
                    vec![B("sheet.autoSum")],
                    vec![B("sheet.fill")],
                    vec![B("sheet.clear")],
                ];
            }
            _ => {}
        }
    }
    t
}

/// Number format box width (logical px): 121 px at 125 % on a compact ribbon, 196 px on a
/// wide one (measured).
pub(crate) fn number_box_width(wide: bool) -> f32 {
    if wide {
        156.8
    } else {
        96.8
    }
}

/// Large buttons placed after their group's rows.
pub(crate) fn trailing(id: &str) -> bool {
    matches!(id, "sheet.sortFilter" | "sheet.findSelect")
}

fn insert() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.insert",
        groups: vec![
            Group {
                label: "sheetEditor.groupTables",
                large: vec![L("sheet.table")],
                rows: vec![],
            },
            Group {
                label: "sheetEditor.groupCharts",
                large: vec![],
                rows: vec![
                    vec![S("sheet.chartColumn")],
                    vec![S("sheet.chartLine")],
                    vec![S("sheet.chartPie")],
                ],
            },
        ],
    }
}

fn formulas() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "sheetMenu.tabFormulas",

        groups: vec![
            Group {
                label: "sheetMenu.groupFunctions",
                large: vec![
                    L("sheet.insertFunction"),
                    L("sheet.autoSumF"),
                    L("sheet.fnFinancial"),
                    L("sheet.fnLogical"),
                    L("sheet.fnText"),
                    L("sheet.fnDateTime"),
                    L("sheet.fnLookup"),
                    L("sheet.fnMath"),
                    L("sheet.fnMore"),
                ],
                rows: vec![],
            },
            Group {
                label: "sheetMenu.groupCalculation",
                large: vec![],
                rows: vec![vec![S("sheet.calcNow")], vec![S("sheet.calcSheet")]],
            },
        ],
    }
}

fn data() -> Tab {
    use Item::{Button as B, Large as L};
    Tab {
        label: "sheetEditor.tabData",
        groups: vec![
            Group {
                label: "sheetEditor.groupSortFilter",
                large: vec![L("sheet.filter")],
                rows: vec![vec![B("sheet.sortAsc")], vec![B("sheet.sortDesc")]],
            },
            Group {
                label: "sheetEditor.groupDataTools",
                large: vec![L("sheet.textToColumns"), L("sheet.removeDuplicates")],
                rows: vec![],
            },
        ],
    }
}

fn view() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.view",
        groups: vec![
            Group {
                label: "ribbon.groups.show",
                large: vec![],
                rows: vec![
                    vec![S("sheet.gridlines")],
                    vec![S("sheet.headings")],
                    vec![S("sheet.formulaBar")],
                ],
            },
            Group {
                label: "ribbon.groups.zoom",
                large: vec![L("sheet.zoom100"), L("sheet.zoomSelection")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.window",
                large: vec![L("sheet.freezePanes")],
                rows: vec![],
            },
        ],
    }
}

/// The ribbon tabs of BlueSheet.
pub(crate) fn tabs() -> Vec<Tab> {
    vec![home(), insert(), formulas(), data(), view()]
}

/// The tabs of a wide window (Home with large buttons).
pub(crate) fn tabs_wide() -> Vec<Tab> {
    let mut t = tabs();
    t[0] = home_wide();
    t
}

/// Every command id on BlueSheet's tabs (enabled rule, label check).
pub(crate) fn tab_ids() -> Vec<&'static str> {
    let mut out = Vec::new();
    for t in tabs() {
        for g in &t.groups {
            for item in g.large.iter().chain(g.rows.iter().flatten()) {
                if let Item::Button(id) | Item::Large(id) | Item::Labeled(id) = item {
                    out.push(*id);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strings::Strings;

    #[test]
    fn number_categories_follow_the_format_codes() {
        assert_eq!(number_category("General"), "sheetMenu.numFmt.general");
        assert_eq!(number_category("#,##0.00"), "sheetMenu.numFmt.number");
        assert_eq!(number_category("0.00%"), "sheetMenu.numFmt.percentage");
        assert_eq!(
            number_category("#,##0 [$₫-42A]"),
            "sheetMenu.numFmt.currency"
        );
        assert_eq!(
            number_category(r#"_-* #,##0 [$₫-42A]_-;-* #,##0 [$₫-42A]_-;_-* "-" [$₫-42A]_-;_-@_-"#),
            "sheetMenu.numFmt.accounting"
        );
        assert_eq!(number_category("dd/mm/yyyy"), "sheetMenu.numFmt.shortDate");
        assert_eq!(
            number_category("dddd, dd mmmm yyyy"),
            "sheetMenu.numFmt.longDate"
        );
        assert_eq!(number_category("hh:mm:ss"), "sheetMenu.numFmt.time");
        assert_eq!(number_category("# ?/?"), "sheetMenu.numFmt.fraction");
        assert_eq!(number_category("0.00E+00"), "sheetMenu.numFmt.scientific");
        assert_eq!(number_category("@"), "sheetMenu.numFmt.text");
        for (id, _) in NUMBER_FORMATS {
            assert!(number_code(id).is_some(), "preset {id}");
        }
    }

    #[test]
    fn every_label_and_menu_entry_has_both_translations() {
        for locale in ["en-US", "vi-VN"] {
            let s = Strings::load(locale);
            for id in tab_ids().into_iter().filter(|i| i.starts_with("sheet.")) {
                let key = format!("commands.{id}");
                assert_ne!(s.get(&key), key, "{locale}: missing {key}");
                for (_, label) in menu(id, &[]) {
                    if label.contains('.') {
                        assert_ne!(s.get(&label), label, "{locale}: missing {label}");
                    }
                }
            }
            for t in tabs() {
                assert_ne!(s.get(t.label), t.label, "{locale}: missing {}", t.label);
                for g in &t.groups {
                    assert_ne!(s.get(g.label), g.label, "{locale}: missing {}", g.label);
                }
            }
        }
    }
}
