//! BlueSheet editing commands over the JSON bridge (ADR-0002, ADR-0008):
//! formatting, rows/columns, sort & filter, fill, conditional formatting,
//! data validation, charts, find/replace, clipboard and page setup.
//!
//! Hosts send one [`SheetCommand`] as JSON (`{"cmd": "format", …}`) and get a
//! JSON value back. Every command that changes the workbook is one undo step.

use super::pivot_cmd::{
    data_field, group_request, pivot_json, updated_spec, PivotFilterSpec, PivotSettings,
    PivotValueSpec,
};
use super::{date_input, input_to_cell, number_input, SheetEngine, UndoEntry, MAX_COLS, MAX_ROWS};
use crate::{EngineError, Result};
use bo_model::sheet::PivotFilter;
use bo_model::sheet::{
    AutoFilter, Cell, CellRange, CellRef, CellValue, ColumnFilter, CustomFilter, FilterColumn,
    PageMargins, Workbook, Worksheet,
};
use bo_sheet::calc::FormulaAdjuster;
use bo_sheet::cf::CfRuleSpec;
use bo_sheet::pivot::{PivotLabels, PivotSpec};
use bo_sheet::style::{BorderSpec, StylePatch};
use bo_sheet::validation::ValidationSpec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A rectangular range from the UI (zero-based, inclusive, any corner order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    pub r0: u32,
    pub c0: u32,
    pub r1: u32,
    pub c1: u32,
}

impl Rng {
    pub(super) fn range(self) -> CellRange {
        let cl = |r: u32, c: u32| CellRef::new(r.min(MAX_ROWS - 1), c.min(MAX_COLS - 1));
        CellRange::new(cl(self.r0, self.c0), cl(self.r1, self.c1))
    }

    pub(super) fn of(r: CellRange) -> Rng {
        Rng {
            r0: r.first.row,
            c0: r.first.col,
            r1: r.last.row,
            c1: r.last.col,
        }
    }
}

/// One sort level from the sort dialog.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortKeySpec {
    /// Absolute column (row when sorting left to right).
    pub col: u32,
    #[serde(default)]
    pub descending: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    /// `value` (default), `cellColor` or `fontColor` (§18.18.79 `ST_SortBy`).
    #[serde(default)]
    pub by: Option<String>,
    /// `RRGGBB` of a colour key.
    #[serde(default)]
    pub color: Option<String>,
}

impl SortKeySpec {
    fn key(&self) -> bo_sheet::sort::SortKey {
        use bo_sheet::sort::SortBy;
        let color = self.color.as_deref().and_then(bo_sheet::color::parse_hex);
        let by = match (self.by.as_deref(), color) {
            (Some("cellColor"), Some(c)) => SortBy::CellColor(c),
            (Some("fontColor"), Some(c)) => SortBy::FontColor(c),
            _ => SortBy::Value,
        };
        bo_sheet::sort::SortKey {
            col: self.col,
            descending: self.descending,
            case_sensitive: self.case_sensitive,
            by,
        }
    }
}

/// A custom AutoFilter condition (§18.3.2.2 `customFilter`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CondSpec {
    /// `equal`, `notEqual`, `greaterThan`, `lessThan`, … or the text helpers
    /// `contains`, `beginsWith`, `endsWith` (written with `*` wildcards).
    pub operator: String,
    pub value: String,
}

/// A filter criterion of one column.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FilterSpec {
    /// Displayed values to keep (`""` = blanks).
    pub values: Option<Vec<String>>,
    pub conditions: Vec<CondSpec>,
    /// Join two conditions with AND (default OR).
    pub and: bool,
    /// Dates to keep, by year or month (§18.3.2.4 `dateGroupItem`), with `values`.
    pub dates: Option<Vec<DateSel>>,
    /// Relative date or average filter (§18.18.26 `ST_DynamicFilterType`).
    pub dynamic: Option<String>,
    /// Keep cells filled with this colour (`RRGGBB`).
    pub cell_color: Option<String>,
    /// Keep cells whose text has this colour (`RRGGBB`).
    pub font_color: Option<String>,
}

/// A year, or a month of a year, of a date filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateSel {
    pub year: u32,
    #[serde(default)]
    pub month: Option<u32>,
}

/// What a paste brings over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PasteMode {
    /// Values, formulas and formats.
    #[default]
    All,
    /// Displayed values only (formulas become their results).
    Values,
    /// Cell formats only.
    Formats,
    /// Formulas (or constants) without formats.
    Formulas,
}

/// Internal clipboard: cell records relative to the copied range.
#[derive(Debug, Clone)]
pub struct Clip {
    /// 0-based index of the sheet the range was copied from.
    pub sheet: usize,
    /// The copied range.
    pub range: CellRange,
    /// `(row offset, column offset, cell)`.
    pub cells: Vec<(u32, u32, Cell)>,
    /// Cut rather than copy: the source is cleared on paste.
    pub cut: bool,
}

/// A cell address in any sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct At {
    pub sheet: usize,
    pub row: u32,
    pub col: u32,
}

/// A sheet editing command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SheetCommand {
    /// Number formats, cell styles and chart types offered by the UI.
    Presets,
    /// Format state (font, fill, number format, …) of one cell for the ribbon.
    FormatState {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based row.
        row: u32,
        /// 0-based column.
        col: u32,
    },
    /// Applies a style patch (font, fill, alignment, number format, …) to a range.
    Format {
        /// 0-based sheet index.
        sheet: usize,
        /// Target range.
        range: Rng,
        /// Style properties to change; absent fields are kept.
        patch: StylePatch,
    },
    /// Applies cell borders to a range.
    Borders {
        /// 0-based sheet index.
        sheet: usize,
        /// Target range.
        range: Rng,
        /// Which borders to set and their line style / colour.
        spec: BorderSpec,
    },
    /// Applies a named cell style to a range.
    CellStyle {
        /// 0-based sheet index.
        sheet: usize,
        /// Target range.
        range: Rng,
        /// Cell style name (one of the presets).
        name: String,
    },
    /// Resets a range to the default format (values are kept).
    ClearFormats {
        /// 0-based sheet index.
        sheet: usize,
        /// Target range.
        range: Rng,
    },
    /// Format Painter: styles of `from` tiled over `to`.
    PaintFormat {
        /// 0-based sheet index.
        sheet: usize,
        /// Range whose formats are copied.
        from: Rng,
        /// Range the formats are painted over.
        to: Rng,
    },
    /// Row height / column width in points.
    Resize {
        /// 0-based sheet index.
        sheet: usize,
        /// `true` = rows, `false` = columns.
        rows: bool,
        /// First row / column (0-based).
        first: u32,
        /// Last row / column (0-based, inclusive).
        last: u32,
        /// New height / width in points.
        size: f64,
    },
    /// Fits row heights / column widths to their content.
    Autofit {
        /// 0-based sheet index.
        sheet: usize,
        /// `true` = rows, `false` = columns.
        rows: bool,
        /// First row / column (0-based).
        first: u32,
        /// Last row / column (0-based, inclusive).
        last: u32,
    },
    /// Hides or unhides rows / columns.
    Hide {
        /// 0-based sheet index.
        sheet: usize,
        /// `true` = rows, `false` = columns.
        rows: bool,
        /// First row / column (0-based).
        first: u32,
        /// Last row / column (0-based, inclusive).
        last: u32,
        /// `true` hides, `false` unhides.
        hidden: bool,
    },
    /// Freezes panes above / left of the given row / column counts (`0, 0` unfreezes).
    Freeze {
        /// 0-based sheet index.
        sheet: usize,
        /// Number of frozen rows at the top.
        rows: u32,
        /// Number of frozen columns at the left.
        cols: u32,
    },
    /// Merges a range into one cell.
    Merge {
        /// 0-based sheet index.
        sheet: usize,
        /// Range to merge.
        range: Rng,
        /// Merge each row of the range separately (Merge Across).
        #[serde(default)]
        across: bool,
        /// Also center the content horizontally (Merge & Center).
        #[serde(default)]
        center: bool,
    },
    /// Splits merged cells overlapping a range.
    Unmerge {
        /// 0-based sheet index.
        sheet: usize,
        /// Range to unmerge.
        range: Rng,
    },
    /// Sorts a range by one or more keys.
    Sort {
        /// 0-based sheet index.
        sheet: usize,
        /// Range to sort.
        range: Rng,
        /// Sort levels, most significant first.
        keys: Vec<SortKeySpec>,
        /// The first row (column) is a header and stays in place.
        #[serde(default)]
        has_header: bool,
        /// Sort columns left to right by key rows (§18.3.1.92 `columnSort`).
        #[serde(default)]
        left_to_right: bool,
    },
    /// Fill and font colours present in a key column (row) of the sort range.
    SortColors {
        /// 0-based sheet index.
        sheet: usize,
        /// Sort range.
        range: Rng,
        /// Key column (row when `left_to_right`), absolute 0-based.
        key: u32,
        /// Sort columns left to right (key is a row).
        #[serde(default)]
        left_to_right: bool,
    },
    /// The range sort / filter / chart would use for a selection (a single
    /// cell expands to its current region).
    Region {
        /// 0-based sheet index.
        sheet: usize,
        /// Selected range.
        range: Rng,
    },
    /// Turns the AutoFilter on (over the selection's region) or off.
    ToggleFilter {
        /// 0-based sheet index.
        sheet: usize,
        /// Selected range.
        range: Rng,
    },
    /// Distinct displayed values of an AutoFilter column (filter drop-down list).
    FilterValues {
        /// 0-based sheet index.
        sheet: usize,
        /// Absolute 0-based column of the AutoFilter range.
        col: u32,
    },
    /// Sets or clears (`None`) the filter criterion of an AutoFilter column.
    SetFilter {
        /// 0-based sheet index.
        sheet: usize,
        /// Absolute 0-based column of the AutoFilter range.
        col: u32,
        /// Criterion; `None` shows every row of the column.
        #[serde(default)]
        filter: Option<FilterSpec>,
    },
    /// Fill handle: extends the series / pattern of `source` over `target`.
    Autofill {
        /// 0-based sheet index.
        sheet: usize,
        /// Range the fill starts from.
        source: Rng,
        /// Whole range after filling (contains `source`).
        target: Rng,
    },
    /// Ctrl+D (`down`) / Ctrl+R (`right`): copies the first row / column.
    FillDir {
        /// 0-based sheet index.
        sheet: usize,
        /// Range to fill.
        range: Rng,
        /// `down` or `right`.
        dir: String,
    },
    /// Adds a conditional formatting rule to a range.
    AddCf {
        /// 0-based sheet index.
        sheet: usize,
        /// Range the rule applies to.
        range: Rng,
        /// The rule (type, operator, formulas, format).
        rule: CfRuleSpec,
    },
    /// Removes the conditional formatting rules over a range.
    ClearCf {
        /// 0-based sheet index.
        sheet: usize,
        /// Range to clear.
        range: Rng,
    },
    /// Data validation rule of one cell (drop-down list, input message).
    Validation {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based row.
        row: u32,
        /// 0-based column.
        col: u32,
    },
    /// Sets (or clears) the data validation rule of a range.
    SetValidation {
        /// 0-based sheet index.
        sheet: usize,
        /// Range the rule applies to.
        range: Rng,
        /// The validation rule.
        spec: ValidationSpec,
    },
    /// Inputs as one undo step, checked against data validation.
    SetInputs {
        /// 0-based sheet index.
        sheet: usize,
        /// `(row, column, input text)` triples, 0-based.
        cells: Vec<(u32, u32, String)>,
        /// Reject inputs that violate the cells' data validation.
        #[serde(default)]
        validate: bool,
    },
    /// Inserts a chart plotting a range.
    InsertChart {
        /// 0-based sheet index.
        sheet: usize,
        /// Data range (series and categories).
        range: Rng,
        /// Chart type (`column`, `bar`, `line`, `pie`, …).
        kind: String,
    },
    /// Charts of a sheet with their properties.
    Charts {
        /// 0-based sheet index.
        sheet: usize,
    },
    /// Changes a chart's type, title, legend or axis titles.
    EditChart {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the chart in the sheet.
        index: usize,
        /// New chart type.
        #[serde(default)]
        kind: Option<String>,
        /// `""` removes the title.
        #[serde(default)]
        title: Option<String>,
        /// `r` / `l` / `t` / `b` or `none`.
        #[serde(default)]
        legend: Option<String>,
        /// Category (horizontal) axis title; `""` removes it.
        #[serde(default)]
        x_title: Option<String>,
        /// Value (vertical) axis title; `""` removes it.
        #[serde(default)]
        y_title: Option<String>,
    },
    /// Removes a chart (charts read from the file lose their drawing anchor and parts).
    DeleteChart {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the chart in the sheet.
        index: usize,
    },
    /// Moves / resizes a chart: top-left cell `from` and the cell holding its
    /// bottom-right corner `to` (`[row, col]`, zero-based).
    MoveChart {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the chart in the sheet.
        index: usize,
        /// Top-left anchor cell `[row, col]`.
        from: [u32; 2],
        /// Bottom-right anchor cell `[row, col]`.
        to: [u32; 2],
    },
    /// Finds cells matching a query.
    Find {
        /// Text to look for.
        query: String,
        /// Distinguish upper and lower case.
        #[serde(default)]
        match_case: bool,
        /// Match the whole cell content only.
        #[serde(default)]
        whole_cell: bool,
        /// Search formulas (input text) instead of displayed values.
        #[serde(default)]
        in_formulas: bool,
        /// `None` = every sheet.
        #[serde(default)]
        sheet: Option<usize>,
    },
    /// Replaces matches of a query.
    Replace {
        /// Text to look for.
        query: String,
        /// Replacement text.
        with: String,
        /// Distinguish upper and lower case.
        #[serde(default)]
        match_case: bool,
        /// Match the whole cell content only.
        #[serde(default)]
        whole_cell: bool,
        /// Sheet to search; `None` = every sheet.
        #[serde(default)]
        sheet: Option<usize>,
        /// Only this cell (Replace), else every match (Replace All).
        #[serde(default)]
        at: Option<At>,
    },
    /// Defined names (Go To list).
    Names,
    /// Resolves `B7`, `Sheet2!A1:C3` or a defined name (Go To).
    Resolve {
        /// 0-based sheet index the reference is relative to.
        sheet: usize,
        /// Cell reference, range reference or defined name.
        text: String,
    },
    /// Copies (or cuts) a range to the internal clipboard.
    Copy {
        /// 0-based sheet index.
        sheet: usize,
        /// Range to copy.
        range: Rng,
        /// Cut instead of copy.
        #[serde(default)]
        cut: bool,
    },
    /// Pastes the internal clipboard at a cell.
    Paste {
        /// 0-based sheet index of the destination.
        sheet: usize,
        /// Destination top-left row (0-based).
        row: u32,
        /// Destination top-left column (0-based).
        col: u32,
        /// What to paste (all, values, formats, formulas).
        #[serde(default)]
        mode: PasteMode,
        /// Swap rows and columns.
        #[serde(default)]
        transpose: bool,
        /// Do not overwrite destination cells with blank source cells.
        #[serde(default)]
        skip_blanks: bool,
        /// Selected target range: a copy is repeated to fill it when its size
        /// is a multiple of the copied block.
        #[serde(default)]
        range: Option<Rng>,
    },
    /// Page setup of a sheet (orientation, paper, scale, margins, print area).
    PageSetup {
        /// 0-based sheet index.
        sheet: usize,
    },
    /// Source range (current region of the selection) and field names for
    /// the "Insert pivot table" dialog.
    PivotSource {
        /// 0-based sheet index.
        sheet: usize,
        /// Selected range.
        range: Rng,
    },
    /// Creates a pivot table over `range` (header row included). `dest` = top-left
    /// on an existing sheet; `None` = a new sheet.
    PivotCreate {
        /// 0-based sheet index of the source data.
        sheet: usize,
        /// Source range including the header row.
        range: Rng,
        /// Top-left cell of the report; `None` = a new sheet.
        #[serde(default)]
        dest: Option<At>,
        /// Source columns (0-based within `range`) used as row fields.
        #[serde(default)]
        rows: Vec<u32>,
        /// Source columns (0-based within `range`) used as column fields.
        #[serde(default)]
        cols: Vec<u32>,
        /// Value fields with their summary function.
        #[serde(default)]
        values: Vec<PivotValueSpec>,
        /// Report filter fields.
        #[serde(default)]
        filters: Vec<PivotFilterSpec>,
        /// Localized labels (Grand Total, …) for the generated cells.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Pivot tables of a sheet with their field lists.
    Pivots {
        /// 0-based sheet index.
        sheet: usize,
    },
    /// Moves fields between the areas of a pivot table; optional report
    /// settings (layout, totals, field settings, filters, style) change too.
    PivotUpdate {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the pivot table in the sheet.
        index: usize,
        /// Row fields (0-based source field indices).
        #[serde(default)]
        rows: Vec<u32>,
        /// Column fields (0-based source field indices).
        #[serde(default)]
        cols: Vec<u32>,
        /// Value fields with their summary function.
        #[serde(default)]
        values: Vec<PivotValueSpec>,
        /// Report filter fields.
        #[serde(default)]
        filters: Vec<PivotFilterSpec>,
        /// Localized labels for the generated cells.
        #[serde(default)]
        labels: Option<PivotLabels>,
        /// Optional report settings.
        #[serde(flatten)]
        settings: PivotSettings,
    },
    /// Groups a field by date parts (`by: "dates"`), numeric ranges
    /// (`"numbers"`) or removes the grouping (`"none"`).
    PivotGroup {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the pivot table in the sheet.
        index: usize,
        /// 0-based source field index.
        field: u32,
        /// `dates`, `numbers` or `none`.
        by: String,
        /// Date grouping: by years.
        #[serde(default)]
        years: bool,
        /// Date grouping: by quarters.
        #[serde(default)]
        quarters: bool,
        /// Date grouping: by months.
        #[serde(default)]
        months: bool,
        /// Date grouping: by days.
        #[serde(default)]
        days: bool,
        /// Numeric grouping: first value (`None` = the field minimum).
        #[serde(default)]
        start: Option<f64>,
        /// Numeric grouping: last value (`None` = the field maximum).
        #[serde(default)]
        end: Option<f64>,
        /// Numeric grouping: size of each range.
        #[serde(default)]
        interval: Option<f64>,
        /// Localized labels for the generated cells.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Adds (`field` absent), changes or removes a calculated field.
    PivotCalcField {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the pivot table in the sheet.
        index: usize,
        /// Field to change or remove; `None` adds a new one.
        #[serde(default)]
        field: Option<u32>,
        /// Field name.
        #[serde(default)]
        name: String,
        /// Formula over other fields (e.g. `=Sales*0.1`).
        #[serde(default)]
        formula: String,
        /// Remove the field instead of adding / changing it.
        #[serde(default)]
        remove: bool,
        /// Localized labels for the generated cells.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Deletes a pivot table (its cells and parts).
    PivotDelete {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the pivot table in the sheet.
        index: usize,
    },
    /// Moves a pivot table so its top-left is `dest`.
    PivotMove {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the pivot table in the sheet.
        index: usize,
        /// New top-left cell.
        dest: At,
        /// Localized labels for the generated cells.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Points the cache at another source range (`Sheet1!A1:D20`) and refreshes.
    PivotChangeSource {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the pivot table in the sheet.
        index: usize,
        /// New source range reference (`Sheet1!A1:D20`).
        source: String,
        /// Localized labels for the generated cells.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Refreshes every pivot table of the workbook.
    PivotRefreshAll {
        /// Localized labels for the generated cells.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Recomputes a pivot table (and others sharing its cache) from the source.
    PivotRefresh {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the pivot table in the sheet.
        index: usize,
        /// Localized labels for the generated cells.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Adds, changes (`old` = its current name) or removes a calculated item
    /// of a row / column field (§18.10.1.8).
    PivotCalcItem {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the pivot table in the sheet.
        index: usize,
        /// 0-based source field index the item belongs to.
        field: u32,
        /// Current name of the item to change or remove; `None` adds one.
        #[serde(default)]
        old: Option<String>,
        /// Item name.
        #[serde(default)]
        name: String,
        /// Formula over other items of the field.
        #[serde(default)]
        formula: String,
        /// Remove the item instead of adding / changing it.
        #[serde(default)]
        remove: bool,
        /// Localized labels for the generated cells.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Groups the selected items of a field (`items`: indexes into its items)
    /// into a manual group, or ungroups them (§18.10.1.26 `discretePr`).
    PivotGroupItems {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the pivot table in the sheet.
        index: usize,
        /// 0-based source field index.
        field: u32,
        /// 0-based indices into the field's items.
        items: Vec<usize>,
        /// Ungroup instead of grouping.
        #[serde(default)]
        ungroup: bool,
        /// Localized labels for the generated cells.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Inserts a pivot chart plotting a pivot table (§21.2.2.142 `pivotSource`).
    PivotChart {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the pivot table in the sheet.
        index: usize,
        /// Chart type (default column).
        #[serde(default)]
        kind: Option<String>,
    },
    /// Changes the page setup of a sheet; absent fields are kept.
    SetPageSetup {
        /// 0-based sheet index.
        sheet: usize,
        /// `portrait` or `landscape`.
        #[serde(default)]
        orientation: Option<String>,
        /// Paper size code (§18.3.1.63 `paperSize`, 9 = A4).
        #[serde(default)]
        paper_size: Option<u32>,
        /// Print scale in percent.
        #[serde(default)]
        scale: Option<u32>,
        /// Fit to `[width, height]` pages (0 = no limit); `null` turns fitting off.
        #[serde(default)]
        fit: Option<Option<[u32; 2]>>,
        /// Margins in inches: left, right, top, bottom.
        #[serde(default)]
        margins: Option<[f64; 4]>,
        /// `null` clears the print area.
        #[serde(default)]
        print_area: Option<Option<Rng>>,
        /// Rows repeated at the top `[first, last]`; `null` clears.
        #[serde(default)]
        title_rows: Option<Option<[u32; 2]>>,
        /// Print gridlines.
        #[serde(default)]
        grid_lines: Option<bool>,
    },
    /// Password for the next `.xlsx` saves ([MS-OFFCRYPTO]); empty/absent removes it.
    SetSavePassword {
        /// The password; `None` or empty saves unencrypted.
        #[serde(default)]
        password: Option<String>,
    },
    /// Sheet and workbook protection state (§18.3.1.85, §18.2.29).
    Protection {
        /// 0-based sheet index.
        sheet: usize,
    },
    /// Protects a sheet; `allow` lists what stays permitted.
    ProtectSheet {
        /// 0-based sheet index.
        sheet: usize,
        /// Password (hashed with SHA-512 + salt, §18.3.1.85 hashValue); empty = none.
        #[serde(default)]
        password: String,
        /// Actions still permitted on the protected sheet.
        #[serde(default)]
        allow: super::protect::SheetPermissions,
    },
    /// Removes the protection of a sheet; fails when the password does not match.
    UnprotectSheet {
        /// 0-based sheet index.
        sheet: usize,
        /// Password given when the sheet was protected (empty when it had none).
        #[serde(default)]
        password: String,
    },
    /// Locks the workbook structure (`lockStructure`).
    ProtectWorkbook {
        /// Password (hashed, §18.2.29 workbookHashValue); empty = none.
        #[serde(default)]
        password: String,
    },
    /// Removes workbook protection (§18.2.29); fails when the password does not match.
    UnprotectWorkbook {
        /// Password given when the workbook was protected (empty when it had none).
        #[serde(default)]
        password: String,
    },
    /// Moves sheet `from` to position `to` (sheet tabs drag and drop).
    MoveSheet {
        /// Current 0-based index of the sheet.
        from: usize,
        /// 0-based index the sheet ends up at.
        to: usize,
    },
    /// `sheetPr/tabColor` (§18.3.1.93) as `RRGGBB`; `None`/empty removes it.
    SetTabColor {
        /// 0-based sheet index.
        sheet: usize,
        /// Tab colour `RRGGBB`; `None` or empty removes it.
        #[serde(default)]
        color: Option<String>,
    },
    /// Several commands as one undo step (e.g. formatting every range of a
    /// multi-range selection); results are returned in order.
    Batch {
        /// The commands, run in order.
        cmds: Vec<SheetCommand>,
    },
    /// Sum, count and average of the numbers in several ranges (status bar of a
    /// multi-range selection); cells covered twice count once.
    RangeStats {
        /// 0-based sheet index.
        sheet: usize,
        /// The ranges of the selection.
        ranges: Vec<Rng>,
    },
}

/// Recalculation needed after an edit.
#[derive(Clone, Copy)]
pub(super) enum Recalc {
    No,
    All,
    Range(usize, CellRange),
}

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

fn non_empty(c: &Cell) -> bool {
    !matches!(c.value, CellValue::Empty) || c.formula.is_some()
}

/// The contiguous block of data around a single cell (Ctrl+A / sort / filter).
pub(crate) fn current_region(ws: &Worksheet, r: CellRange) -> CellRange {
    if r.rows() > 1 || r.cols() > 1 {
        return r;
    }
    let filled = |row: u32, c0: u32, c1: u32| {
        ws.cells
            .range((row, c0)..=(row, c1))
            .any(|(_, c)| non_empty(c))
    };
    let (mut r0, mut c0, mut r1, mut c1) = (r.first.row, r.first.col, r.last.row, r.last.col);
    for _ in 0..100_000 {
        let lc = c0.saturating_sub(1);
        let rc = (c1 + 1).min(MAX_COLS - 1);
        let mut grown = false;
        if r0 > 0 && filled(r0 - 1, lc, rc) {
            r0 -= 1;
            grown = true;
        }
        if r1 + 1 < MAX_ROWS && filled(r1 + 1, lc, rc) {
            r1 += 1;
            grown = true;
        }
        if c0 > 0 && (r0.saturating_sub(1)..=r1 + 1).any(|row| filled(row, c0 - 1, c0 - 1)) {
            c0 -= 1;
            grown = true;
        }
        if c1 + 1 < MAX_COLS
            && (r0.saturating_sub(1)..=r1 + 1).any(|row| filled(row, c1 + 1, c1 + 1))
        {
            c1 += 1;
            grown = true;
        }
        if !grown {
            break;
        }
    }
    CellRange::new(CellRef::new(r0, c0), CellRef::new(r1, c1))
}

fn tsv_field(s: &str) -> String {
    if s.contains(['\t', '\n', '\r', '"']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// `customFilter` from a UI condition (§18.18.31 `ST_FilterOperator`).
fn custom_of(c: &CondSpec) -> CustomFilter {
    let (op, val) = match c.operator.as_str() {
        "contains" => ("equal", format!("*{}*", c.value)),
        "notContains" => ("notEqual", format!("*{}*", c.value)),
        "beginsWith" => ("equal", format!("{}*", c.value)),
        "endsWith" => ("equal", format!("*{}", c.value)),
        other => (other, c.value.clone()),
    };
    CustomFilter {
        operator: (op != "equal").then(|| op.to_owned()),
        val,
    }
}

fn replace_text(hay: &str, needle: &str, with: &str, match_case: bool) -> (String, usize) {
    if needle.is_empty() {
        return (hay.to_owned(), 0);
    }
    if match_case {
        let n = hay.matches(needle).count();
        return (hay.replace(needle, with), n);
    }
    let low_h = hay.to_lowercase();
    let low_n = needle.to_lowercase();
    // Lower-casing can change byte lengths (rare); fall back to exact matching then.
    if low_h.len() != hay.len() || low_n.len() != needle.len() {
        let n = hay.matches(needle).count();
        return (hay.replace(needle, with), n);
    }
    let mut out = String::with_capacity(hay.len());
    let mut i = 0;
    let mut n = 0;
    while let Some(p) = low_h[i..].find(&low_n) {
        out.push_str(&hay[i..i + p]);
        out.push_str(with);
        i += p + needle.len();
        n += 1;
    }
    out.push_str(&hay[i..]);
    (out, n)
}

impl SheetEngine {
    /// Runs a command given as JSON; returns the JSON result.
    pub fn exec_json(&mut self, json: &str) -> Result<String> {
        let cmd: SheetCommand =
            serde_json::from_str(json).map_err(|e| bad(format!("bad command: {e}")))?;
        Ok(self.exec(cmd)?.to_string())
    }

    /// Refuses a command that sheet or workbook protection forbids
    /// (ECMA-376 Part 1, §18.3.1.85 `sheetProtection`, §18.2.29 `workbookProtection`).
    /// Content changes to locked cells are refused in [`SheetEngine::edit`].
    fn guard(&self, cmd: &SheetCommand) -> Result<()> {
        use super::protect::{check, check_range, Action as A};
        use SheetCommand as C;
        let Some(wb) = self.wb.as_ref() else {
            return Ok(());
        };
        let ws = |i: usize| wb.sheets.get(i).ok_or(EngineError::PageOutOfRange(i));
        let rows_or_cols = |rows: bool| {
            if rows {
                A::FormatRows
            } else {
                A::FormatColumns
            }
        };
        match cmd {
            C::Format { sheet, .. }
            | C::Borders { sheet, .. }
            | C::CellStyle { sheet, .. }
            | C::ClearFormats { sheet, .. }
            | C::PaintFormat { sheet, .. }
            | C::Merge { sheet, .. }
            | C::Unmerge { sheet, .. }
            | C::AddCf { sheet, .. }
            | C::ClearCf { sheet, .. }
            | C::SetValidation { sheet, .. } => check(ws(*sheet)?, A::FormatCells),
            C::Resize { sheet, rows, .. }
            | C::Autofit { sheet, rows, .. }
            | C::Hide { sheet, rows, .. } => check(ws(*sheet)?, rows_or_cols(*rows)),
            C::Sort { sheet, range, .. } => {
                let w = ws(*sheet)?;
                check(w, A::Sort)?;
                check_range(wb, w, range.r0, range.c0, range.r1, range.c1)
            }
            C::ToggleFilter { sheet, .. } | C::SetFilter { sheet, .. } => {
                check(ws(*sheet)?, A::AutoFilter)
            }
            C::InsertChart { sheet, .. }
            | C::EditChart { sheet, .. }
            | C::DeleteChart { sheet, .. }
            | C::MoveChart { sheet, .. } => check(ws(*sheet)?, A::Objects),
            C::PivotCreate { sheet, .. }
            | C::PivotUpdate { sheet, .. }
            | C::PivotGroup { sheet, .. }
            | C::PivotCalcField { sheet, .. }
            | C::PivotDelete { sheet, .. }
            | C::PivotMove { sheet, .. }
            | C::PivotChangeSource { sheet, .. }
            | C::PivotRefresh { sheet, .. } => check(ws(*sheet)?, A::PivotTables),
            C::MoveSheet { .. } => super::protect::check_structure(wb),
            _ => Ok(()),
        }
    }

    /// One mutating edit: snapshot for undo, apply, refresh geometry and values.
    pub(super) fn edit<R>(
        &mut self,
        sheet: usize,
        recalc: Recalc,
        f: impl FnOnce(&mut Workbook, &bo_sheet::formula::BoFormula) -> Result<R>,
    ) -> Result<R> {
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        if sheet >= wb.sheets.len() {
            return Err(EngineError::PageOutOfRange(sheet));
        }
        let before = Box::new(wb.clone());
        let out = match f(wb, &self.adjuster) {
            Ok(out) => out,
            Err(e) => {
                // A failed edit leaves the workbook as it was.
                *wb = *before;
                return Err(e);
            }
        };
        if let Err(e) = locked_cells_unchanged(&before, wb) {
            *wb = *before;
            return Err(e);
        }
        match recalc {
            Recalc::No => {}
            Recalc::All => self.live.rebuild(wb),
            Recalc::Range(si, r) => {
                let cells: Vec<(u32, u32)> = (r.first.row..=r.last.row)
                    .flat_map(|row| (r.first.col..=r.last.col).map(move |c| (row, c)))
                    .collect();
                self.live.update(wb, si, &cells);
            }
        }
        self.invalidate(Some(sheet));
        self.push_undo(UndoEntry::Workbook(before));
        Ok(out)
    }

    /// Executes a command.
    #[allow(clippy::too_many_lines)]
    pub fn exec(&mut self, cmd: SheetCommand) -> Result<Value> {
        use SheetCommand as C;
        let locale = self.locale;
        self.guard(&cmd)?;
        match cmd {
            C::SetSavePassword { password } => {
                self.set_save_password(password.as_deref());
                Ok(json!({ "ok": true }))
            }
            C::Protection { sheet } => {
                let ws = self.sheet(sheet)?;
                Ok(json!({
                    "sheet": super::protect::sheet_info(ws),
                    "workbook": super::protect::workbook_info(self.wb()?),
                }))
            }
            C::ProtectSheet {
                sheet,
                password,
                allow,
            } => self.edit(sheet, Recalc::No, |wb, _| {
                super::protect::protect_sheet(&mut wb.sheets[sheet], &password, &allow)?;
                Ok(json!({ "ok": true }))
            }),
            C::UnprotectSheet { sheet, password } => self.edit(sheet, Recalc::No, |wb, _| {
                super::protect::unprotect_sheet(&mut wb.sheets[sheet], &password)?;
                Ok(json!({ "ok": true }))
            }),
            C::ProtectWorkbook { password } => self.edit(0, Recalc::No, |wb, _| {
                super::protect::protect_workbook(wb, &password)?;
                Ok(json!({ "ok": true }))
            }),
            C::UnprotectWorkbook { password } => self.edit(0, Recalc::No, |wb, _| {
                super::protect::unprotect_workbook(wb, &password)?;
                Ok(json!({ "ok": true }))
            }),
            C::MoveSheet { from, to } => {
                let n = self.wb()?.sheets.len();
                if from >= n || to >= n {
                    return Err(EngineError::PageOutOfRange(from.max(to)));
                }
                if from == to {
                    return Ok(json!({ "ok": true }));
                }
                let out = self.edit(from, Recalc::All, |wb, _| {
                    move_sheet(wb, from, to);
                    Ok(json!({ "ok": true }))
                })?;
                self.invalidate(None);
                Ok(out)
            }
            C::SetTabColor { sheet, color } => self.edit(sheet, Recalc::No, |wb, _| {
                let hex = color
                    .as_deref()
                    .map(|c| c.trim().trim_start_matches('#').to_ascii_uppercase())
                    .filter(|c| !c.is_empty());
                let ws = &mut wb.sheets[sheet];
                ws.props.tab_color = match hex {
                    Some(h) if h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()) => {
                        // §18.3.1.93 tabColor (ARGB, `rgb` adds the alpha).
                        Some(bo_model::sheet::SheetColor::rgb(&h))
                    }
                    Some(h) => return Err(bad(format!("bad colour '{h}'"))),
                    None => None,
                };
                Ok(json!({ "ok": true }))
            }),
            C::Batch { cmds } => self.batch(cmds),
            C::RangeStats { sheet, ranges } => {
                let ws = self.sheet(sheet)?;
                Ok(range_stats(ws, &ranges))
            }
            C::Presets => Ok(json!({
                "numberFormats": bo_sheet::style::NUMBER_PRESETS
                    .iter()
                    .map(|(id, code)| json!({ "id": id, "code": code }))
                    .collect::<Vec<_>>(),
                "cellStyles": bo_sheet::style::NAMED_STYLES.iter().map(|s| s.name).collect::<Vec<_>>(),
                "chartTypes": bo_sheet::chart::CHART_TYPES,
                "pivotStyles": bo_sheet::pivot::style::STYLE_NAMES,
            })),
            C::FormatState { sheet, row, col } => {
                self.sheet(sheet)?;
                let st = bo_sheet::style::format_state(self.wb()?, sheet, row, col, &locale);
                Ok(serde_json::to_value(st).unwrap_or(Value::Null))
            }
            C::Format {
                sheet,
                range,
                patch,
            } => self.edit(sheet, Recalc::No, |wb, _| {
                bo_sheet::style::apply_patch(wb, sheet, range.range(), &patch)
                    .then_some(json!({ "ok": true }))
                    .ok_or_else(|| bad("range too large"))
            }),
            C::Borders { sheet, range, spec } => self.edit(sheet, Recalc::No, |wb, _| {
                bo_sheet::style::apply_borders(wb, sheet, range.range(), &spec)
                    .then_some(json!({ "ok": true }))
                    .ok_or_else(|| bad("range too large"))
            }),
            C::CellStyle { sheet, range, name } => self.edit(sheet, Recalc::No, |wb, _| {
                bo_sheet::style::apply_named_style(wb, sheet, range.range(), &name)
                    .then_some(json!({ "ok": true }))
                    .ok_or_else(|| bad(format!("unknown cell style '{name}'")))
            }),
            C::ClearFormats { sheet, range } => self.edit(sheet, Recalc::No, |wb, _| {
                bo_sheet::style::clear_formats(wb, sheet, range.range());
                Ok(json!({ "ok": true }))
            }),
            C::PaintFormat { sheet, from, to } => self.edit(sheet, Recalc::No, |wb, _| {
                bo_sheet::style::copy_formats(wb, sheet, from.range(), to.range())
                    .then_some(json!({ "ok": true }))
                    .ok_or_else(|| bad("range too large"))
            }),
            C::Resize {
                sheet,
                rows,
                first,
                last,
                size,
            } => {
                let metrics = self.geometry(sheet)?.metrics;
                self.edit(sheet, Recalc::No, |wb, _| {
                    let ws = &mut wb.sheets[sheet];
                    let (a, b) = (first.min(last), first.max(last));
                    if rows {
                        bo_sheet::rowcol::set_row_height(ws, a, b, size);
                    } else {
                        bo_sheet::rowcol::set_col_width(ws, &metrics, a, b, size);
                    }
                    Ok(json!({ "ok": true }))
                })
            }
            C::Autofit {
                sheet,
                rows,
                first,
                last,
            } => self.autofit(sheet, rows, first.min(last), first.max(last)),
            C::Hide {
                sheet,
                rows,
                first,
                last,
                hidden,
            } => self.edit(sheet, Recalc::All, |wb, _| {
                bo_sheet::rowcol::set_hidden(
                    &mut wb.sheets[sheet],
                    rows,
                    first.min(last),
                    first.max(last),
                    hidden,
                );
                Ok(json!({ "ok": true }))
            }),
            C::Freeze { sheet, rows, cols } => self.edit(sheet, Recalc::No, |wb, _| {
                bo_sheet::rowcol::freeze(
                    &mut wb.sheets[sheet],
                    rows.min(MAX_ROWS - 1),
                    cols.min(MAX_COLS - 1),
                );
                Ok(json!({ "ok": true }))
            }),
            C::Merge {
                sheet,
                range,
                across,
                center,
            } => self.edit(sheet, Recalc::All, |wb, _| {
                let r = range.range();
                bo_sheet::rowcol::merge(&mut wb.sheets[sheet], r, across);
                if center {
                    bo_sheet::style::apply_patch(
                        wb,
                        sheet,
                        r,
                        &StylePatch {
                            h_align: Some("center".into()),
                            ..StylePatch::default()
                        },
                    );
                }
                Ok(json!({ "ok": true }))
            }),
            C::Unmerge { sheet, range } => self.edit(sheet, Recalc::No, |wb, _| {
                let n = bo_sheet::rowcol::unmerge(&mut wb.sheets[sheet], range.range());
                Ok(json!({ "removed": n }))
            }),
            C::Region { sheet, range } => {
                let ws = self.sheet(sheet)?;
                Ok(
                    serde_json::to_value(Rng::of(current_region(ws, range.range())))
                        .unwrap_or(Value::Null),
                )
            }
            C::Sort {
                sheet,
                range,
                keys,
                has_header,
                left_to_right,
            } => {
                let r = current_region(self.sheet(sheet)?, range.range());
                self.edit(sheet, Recalc::All, |wb, adj| {
                    let keys: Vec<bo_sheet::sort::SortKey> =
                        keys.iter().map(SortKeySpec::key).collect();
                    bo_sheet::sort::sort_lines(wb, sheet, r, &keys, has_header, left_to_right, adj);
                    Ok(serde_json::to_value(Rng::of(r)).unwrap_or(Value::Null))
                })
            }
            C::SortColors {
                sheet,
                range,
                key,
                left_to_right,
            } => {
                let r = current_region(self.sheet(sheet)?, range.range());
                let (fills, fonts) =
                    bo_sheet::sort::line_colors(self.wb()?, sheet, r, key, left_to_right);
                Ok(json!({ "cell": fills, "font": fonts }))
            }
            C::ToggleFilter { sheet, range } => {
                let r = current_region(self.sheet(sheet)?, range.range());
                self.edit(sheet, Recalc::All, |wb, _| {
                    let ws = &mut wb.sheets[sheet];
                    if let Some(af) = ws.auto_filter.take() {
                        for row in af.range.first.row..=af.range.last.row {
                            bo_sheet::rowcol::set_hidden(ws, true, row, row, false);
                        }
                        return Ok(json!({ "active": false }));
                    }
                    ws.auto_filter = Some(AutoFilter {
                        range: r,
                        ..AutoFilter::default()
                    });
                    Ok(json!({ "active": true, "range": Rng::of(r) }))
                })
            }
            C::FilterValues { sheet, col } => {
                let wb = self.wb()?;
                let ws = self.sheet(sheet)?;
                let af = ws
                    .auto_filter
                    .as_ref()
                    .ok_or_else(|| bad("no AutoFilter"))?;
                if col < af.range.first.col || col > af.range.last.col {
                    return Err(bad("column outside the AutoFilter"));
                }
                let col_id = col - af.range.first.col;
                let values = bo_sheet::filter::column_values(wb, ws, af, col_id, &locale);
                let current = af
                    .columns
                    .iter()
                    .find(|c| c.col_id == col_id)
                    .map(|c| &c.filter);
                let checked = |v: &str| match current {
                    Some(ColumnFilter::Values { values, blank }) => {
                        if v.is_empty() {
                            *blank
                        } else {
                            values.iter().any(|x| x.to_lowercase() == v.to_lowercase())
                        }
                    }
                    _ => true,
                };
                let kind = match current {
                    None | Some(ColumnFilter::None) => "none",
                    Some(ColumnFilter::Values { .. }) => "values",
                    Some(ColumnFilter::Custom { .. }) => "custom",
                    Some(ColumnFilter::DateGroups { .. }) => "dates",
                    Some(ColumnFilter::Dynamic { .. }) => "dynamic",
                    Some(ColumnFilter::Color { .. }) => "color",
                    Some(_) => "other",
                };
                let dates = bo_sheet::filter::column_dates(wb, ws, af, col_id);
                let picked = |y: i32, m: Option<u32>| match current {
                    Some(ColumnFilter::DateGroups { groups, .. }) => groups.iter().any(|g| {
                        i64::from(g.year) == i64::from(y)
                            && (g.month.is_none() || m.is_none() || g.month == m)
                    }),
                    _ => true,
                };
                let data = CellRange::new(
                    CellRef::new(af.range.first.row + 1, af.range.first.col),
                    af.range.last,
                );
                let (cell_colors, font_colors) =
                    bo_sheet::sort::line_colors(wb, sheet, data, col, false);
                Ok(json!({
                    "kind": kind,
                    "values": values
                        .iter()
                        .map(|v| json!({ "text": v, "checked": checked(v) }))
                        .collect::<Vec<_>>(),
                    "dates": dates.iter().map(|(y, months)| json!({
                        "year": y,
                        "checked": picked(*y, None),
                        "months": months.iter().map(|m| json!({
                            "month": m,
                            "checked": picked(*y, Some(*m)),
                        })).collect::<Vec<_>>(),
                    })).collect::<Vec<_>>(),
                    "dynamic": match current {
                        Some(ColumnFilter::Dynamic { kind, .. }) => Value::from(kind.clone()),
                        _ => Value::Null,
                    },
                    "colors": { "cell": cell_colors, "font": font_colors },
                }))
            }
            C::SetFilter { sheet, col, filter } => {
                let now = self.live.now();
                self.edit(sheet, Recalc::All, |wb, _| {
                    let range = wb.sheets[sheet]
                        .auto_filter
                        .as_ref()
                        .ok_or_else(|| bad("no AutoFilter"))?
                        .range;
                    if col < range.first.col || col > range.last.col {
                        return Err(bad("column outside the AutoFilter"));
                    }
                    let col_id = col - range.first.col;
                    let color_dxf = |wb: &mut Workbook, hex: &str, cell: bool| -> Option<u32> {
                        let c = bo_model::sheet::SheetColor::rgb(hex.trim_start_matches('#'));
                        let dxf = if cell {
                            bo_model::sheet::Dxf {
                                fill: Some(bo_model::sheet::Fill::Pattern(
                                    bo_model::sheet::PatternFill {
                                        pattern: Some("solid".into()),
                                        fg: None,
                                        bg: Some(c),
                                    },
                                )),
                                ..bo_model::sheet::Dxf::default()
                            }
                        } else {
                            bo_model::sheet::Dxf {
                                font: Some(bo_model::sheet::Font {
                                    color: Some(c),
                                    ..bo_model::sheet::Font::default()
                                }),
                                ..bo_model::sheet::Dxf::default()
                            }
                        };
                        let dxfs = &mut wb.styles.dxfs;
                        let i = dxfs.iter().position(|d| *d == dxf).unwrap_or_else(|| {
                            dxfs.push(dxf);
                            dxfs.len() - 1
                        });
                        Some(i as u32)
                    };
                    let crit = match filter {
                        Some(FilterSpec {
                            dynamic: Some(kind),
                            ..
                        }) => Some(ColumnFilter::Dynamic {
                            kind,
                            val: None,
                            max_val: None,
                        }),
                        Some(FilterSpec {
                            cell_color: Some(hex),
                            ..
                        }) => Some(ColumnFilter::Color {
                            dxf_id: color_dxf(wb, &hex, true),
                            cell_color: true,
                        }),
                        Some(FilterSpec {
                            font_color: Some(hex),
                            ..
                        }) => Some(ColumnFilter::Color {
                            dxf_id: color_dxf(wb, &hex, false),
                            cell_color: false,
                        }),
                        Some(FilterSpec {
                            values,
                            dates: Some(dates),
                            ..
                        }) => {
                            let values = values.unwrap_or_default();
                            let blank = values.iter().any(String::is_empty);
                            Some(ColumnFilter::DateGroups {
                                values: values.into_iter().filter(|v| !v.is_empty()).collect(),
                                blank,
                                groups: dates
                                    .iter()
                                    .map(|d| bo_model::sheet::DateGroup {
                                        year: d.year,
                                        month: d.month,
                                        grouping: if d.month.is_some() { "month" } else { "year" }
                                            .into(),
                                        ..bo_model::sheet::DateGroup::default()
                                    })
                                    .collect(),
                            })
                        }
                        Some(FilterSpec {
                            values: Some(values),
                            ..
                        }) => {
                            let blank = values.iter().any(String::is_empty);
                            Some(ColumnFilter::Values {
                                values: values.into_iter().filter(|v| !v.is_empty()).collect(),
                                blank,
                            })
                        }
                        Some(FilterSpec {
                            conditions, and, ..
                        }) if !conditions.is_empty() => Some(ColumnFilter::Custom {
                            and,
                            conditions: conditions.iter().take(2).map(custom_of).collect(),
                        }),
                        _ => None,
                    };
                    let Some(af) = wb.sheets[sheet].auto_filter.as_mut() else {
                        return Err(bad("no AutoFilter"));
                    };
                    af.columns.retain(|c| c.col_id != col_id);
                    if let Some(filter) = crit {
                        af.columns.push(FilterColumn {
                            col_id,
                            hidden_button: false,
                            show_button: None,
                            filter,
                        });
                        af.columns.sort_by_key(|c| c.col_id);
                    }
                    let hidden = bo_sheet::filter::apply_auto_filter_at(wb, sheet, &locale, now);
                    Ok(json!({ "hidden": hidden }))
                })
            }
            C::Autofill {
                sheet,
                source,
                target,
            } => {
                let t = target.range();
                self.edit(sheet, Recalc::Range(sheet, t), |wb, adj| {
                    let done = bo_sheet::fill::autofill(wb, sheet, source.range(), t, &locale, adj);
                    Ok(json!({ "ok": done }))
                })
            }
            C::FillDir { sheet, range, dir } => {
                let r = range.range();
                self.edit(sheet, Recalc::Range(sheet, r), |wb, adj| {
                    let ws = &mut wb.sheets[sheet];
                    let down = dir != "right";
                    let (n, lanes) = if down {
                        (r.rows(), r.first.col..=r.last.col)
                    } else {
                        (r.cols(), r.first.row..=r.last.row)
                    };
                    if n < 2 {
                        return Ok(json!({ "ok": false }));
                    }
                    for lane in lanes {
                        let src = if down {
                            (r.first.row, lane)
                        } else {
                            (lane, r.first.col)
                        };
                        let cell = ws.cell(src.0, src.1).cloned();
                        for i in 1..n {
                            let dst = if down {
                                (r.first.row + i, lane)
                            } else {
                                (lane, r.first.col + i)
                            };
                            match &cell {
                                Some(c) => {
                                    let mut c = c.clone();
                                    if let Some(f) = c.formula.as_mut() {
                                        let (dr, dc) = if down {
                                            (i64::from(i), 0)
                                        } else {
                                            (0, i64::from(i))
                                        };
                                        if let Some(t) = adj.translate(&f.text, dr, dc) {
                                            f.text = t;
                                        }
                                        f.kind = bo_model::sheet::FormulaKind::Normal;
                                    }
                                    ws.cells.insert(dst, c);
                                }
                                None => {
                                    ws.cells.remove(&dst);
                                }
                            }
                        }
                    }
                    Ok(json!({ "ok": true }))
                })
            }
            C::AddCf { sheet, range, rule } => self.edit(sheet, Recalc::No, |wb, _| {
                bo_sheet::cf::add_rule(wb, sheet, range.range(), &rule)
                    .then_some(json!({ "ok": true }))
                    .ok_or_else(|| bad("invalid rule"))
            }),
            C::ClearCf { sheet, range } => self.edit(sheet, Recalc::No, |wb, _| {
                Ok(json!({ "removed": bo_sheet::cf::clear_rules(wb, sheet, range.range()) }))
            }),
            C::Validation { sheet, row, col } => {
                let wb = self.wb()?;
                self.sheet(sheet)?;
                let Some(dv) = bo_sheet::validation::rule_at(wb, sheet, row, col) else {
                    return Ok(json!({ "list": null }));
                };
                let list = bo_sheet::validation::has_dropdown(dv)
                    .then(|| bo_sheet::validation::list_items(wb, sheet, dv, &locale));
                Ok(json!({
                    "kind": dv.kind,
                    "list": list,
                    "prompt": dv.prompt,
                    "promptTitle": dv.prompt_title,
                }))
            }
            C::SetValidation { sheet, range, spec } => self.edit(sheet, Recalc::No, |wb, _| {
                bo_sheet::validation::set_rule(wb, sheet, range.range(), &spec)
                    .then_some(json!({ "ok": true }))
                    .ok_or_else(|| bad("invalid validation rule"))
            }),
            C::SetInputs {
                sheet,
                cells,
                validate,
            } => {
                let r = self.set_inputs_checked(sheet, &cells, validate)?;
                let blocked = r
                    .as_ref()
                    .is_some_and(bo_sheet::validation::Rejection::blocks);
                Ok(json!({
                    "rejected": if blocked { serde_json::to_value(&r).unwrap_or(Value::Null) } else { Value::Null },
                    "warning": if blocked { Value::Null } else { serde_json::to_value(&r).unwrap_or(Value::Null) },
                }))
            }
            C::InsertChart { sheet, range, kind } => {
                let r = current_region(self.sheet(sheet)?, range.range());
                let chart = bo_sheet::chart::new_chart(self.wb()?, sheet, r, &kind)
                    .ok_or_else(|| bad("no data for a chart"))?;
                self.edit(sheet, Recalc::No, |wb, _| {
                    let charts = &mut wb.sheets[sheet].charts;
                    charts.push(chart);
                    Ok(json!({ "index": charts.len() - 1 }))
                })
            }
            C::Charts { sheet } => {
                let ws = self.sheet(sheet)?;
                Ok(Value::Array(
                    ws.charts
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            let p = c.plots.first();
                            let kind = match (
                                c.plots.len(),
                                p.map(|p| (&p.kind, p.bar_dir.as_deref(), p.grouping.as_deref())),
                            ) {
                                (n, _) if n > 1 => "combo",
                                (_, Some((bo_model::sheet::ChartKind::Bar, Some("bar"), _))) => {
                                    "bar"
                                }
                                (
                                    _,
                                    Some((
                                        bo_model::sheet::ChartKind::Bar,
                                        _,
                                        Some("stacked" | "percentStacked"),
                                    )),
                                ) => "stackedColumn",
                                (_, Some((bo_model::sheet::ChartKind::Line, ..))) => "line",
                                (_, Some((bo_model::sheet::ChartKind::Area, ..))) => "area",
                                (_, Some((bo_model::sheet::ChartKind::Pie, ..))) => "pie",
                                (_, Some((bo_model::sheet::ChartKind::Doughnut, ..))) => "doughnut",
                                (_, Some((bo_model::sheet::ChartKind::Scatter, ..))) => "scatter",
                                _ => "column",
                            };
                            json!({
                                "index": i,
                                "kind": kind,
                                "title": c.title,
                                "legend": c.legend,
                                "xTitle": c.x_title,
                                "yTitle": c.y_title,
                                "anchor": c.anchor,
                                "pivot": c.pivot_source,
                                "from": [c.from.row, c.from.col],
                                "to": c.to.map_or([c.from.row + 15, c.from.col + 8], |t| [t.row, t.col]),
                                "series": c.plots.iter().map(|p| p.series.len()).sum::<usize>(),
                                "editable": true,
                            })
                        })
                        .collect(),
                ))
            }
            C::EditChart {
                sheet,
                index,
                kind,
                title,
                legend,
                x_title,
                y_title,
            } => self.edit(sheet, Recalc::No, |wb, _| {
                let c = wb.sheets[sheet]
                    .charts
                    .get_mut(index)
                    .ok_or_else(|| bad("no such chart"))?;
                let opt = |s: String| (!s.trim().is_empty()).then(|| s.trim().to_owned());
                if let Some(k) = kind {
                    bo_sheet::chart::set_chart_type(c, &k);
                }
                if let Some(t) = title {
                    c.title = opt(t);
                }
                if let Some(l) = legend {
                    c.legend = (l != "none" && !l.is_empty()).then_some(l);
                }
                if let Some(t) = x_title {
                    c.x_title = opt(t);
                }
                if let Some(t) = y_title {
                    c.y_title = opt(t);
                }
                c.dirty = true;
                Ok(json!({ "ok": true }))
            }),
            C::DeleteChart { sheet, index } => self.edit(sheet, Recalc::No, |wb, _| {
                if bo_xlsx::chart_write::remove_chart(wb, sheet, index) {
                    Ok(json!({ "ok": true }))
                } else {
                    Err(bad("no such chart"))
                }
            }),
            C::MoveChart {
                sheet,
                index,
                from,
                to,
            } => {
                let cl = |[r, c]: [u32; 2]| CellRef::new(r.min(MAX_ROWS - 1), c.min(MAX_COLS - 1));
                let (from, to) = (cl(from), cl(to));
                // At least one cell wide and tall.
                let to = CellRef::new(to.row.max(from.row + 1), to.col.max(from.col + 1));
                // One-cell / absolute anchors store EMU (12 700 per point, §20.5.2.14).
                let g = self.geometry(sheet)?;
                let emu = |pt: f64| (pt * 12_700.0).round() as i64;
                let (x0, y0) = (g.col_x(from.col), g.row_y(from.row));
                let rect = Some((
                    emu(x0),
                    emu(y0),
                    emu(g.col_x(to.col) - x0),
                    emu(g.row_y(to.row) - y0),
                ));
                self.edit(sheet, Recalc::No, |wb, _| {
                    if bo_xlsx::chart_write::move_chart(wb, sheet, index, from, to, rect) {
                        Ok(json!({ "ok": true }))
                    } else {
                        Err(bad("this chart cannot be moved"))
                    }
                })
            }
            C::Find {
                query,
                match_case,
                whole_cell,
                in_formulas,
                sheet,
            } => Ok(json!({
                "matches": self.find(&query, match_case, whole_cell, in_formulas, sheet)?,
            })),
            C::Replace {
                query,
                with,
                match_case,
                whole_cell,
                sheet,
                at,
            } => self.replace(&query, &with, match_case, whole_cell, sheet, at),
            C::Names => {
                let wb = self.wb()?;
                Ok(Value::Array(
                    wb.defined_names
                        .iter()
                        .filter(|d| !d.hidden && !d.name.starts_with("_xlnm."))
                        .map(|d| json!({ "name": d.name, "ref": d.formula }))
                        .collect(),
                ))
            }
            C::Resolve { sheet, text } => {
                let wb = self.wb()?;
                Ok(match bo_sheet::cells::resolve_ref(wb, sheet, &text) {
                    Some((si, r)) => json!({ "sheet": si, "range": Rng::of(r) }),
                    None => Value::Null,
                })
            }
            C::Copy { sheet, range, cut } => self.copy(sheet, range.range(), cut),
            C::Paste {
                sheet,
                row,
                col,
                mode,
                transpose,
                skip_blanks,
                range,
            } => self.paste(sheet, row, col, mode, transpose, skip_blanks, range),
            C::PageSetup { sheet } => {
                let ws = self.sheet(sheet)?;
                let p = &ws.page_setup;
                let m = p.margins.unwrap_or_default();
                let (rows, _) = ws
                    .print_titles
                    .as_deref()
                    .map_or((None, None), super::print::parse_titles);
                Ok(json!({
                    "orientation": p.orientation.clone().unwrap_or_else(|| "portrait".into()),
                    "paperSize": p.paper_size.unwrap_or(9),
                    "scale": p.scale.unwrap_or(100),
                    "fit": (ws.props.fit_to_page == Some(true))
                        .then(|| [p.fit_to_width.unwrap_or(1), p.fit_to_height.unwrap_or(1)]),
                    "margins": [m.left, m.right, m.top, m.bottom],
                    "printArea": ws.print_area,
                    "titleRows": rows.map(|(a, b)| [a, b]),
                    "gridLines": p.print_grid_lines,
                }))
            }
            C::PivotSource { sheet, range } => {
                let ws = self.sheet(sheet)?;
                let r = current_region(ws, range.range());
                let (names, records) =
                    bo_sheet::pivot::source_data(ws, r, &PivotLabels::default()).map_err(bad)?;
                Ok(json!({ "range": Rng::of(r), "fields": names, "records": records.len() }))
            }
            C::PivotCreate {
                sheet,
                range,
                dest,
                rows,
                cols,
                values,
                filters,
                labels,
            } => {
                let r = current_region(self.sheet(sheet)?, range.range());
                let labels = self.pivot_labels(labels);
                let out = self.edit(sheet, Recalc::All, |wb, _| {
                    let spec = PivotSpec {
                        rows,
                        cols,
                        values: values
                            .into_iter()
                            .map(|v| data_field(&mut wb.styles, v))
                            .collect(),
                        filters: filters
                            .iter()
                            .map(|f| PivotFilter {
                                field: f.field,
                                selected: Vec::new(),
                            })
                            .collect(),
                        ..PivotSpec::default()
                    };
                    let (dest_sheet, at) = if let Some(a) = dest {
                        if a.sheet >= wb.sheets.len() {
                            return Err(bad("no such sheet"));
                        }
                        (a.sheet, CellRef::new(a.row, a.col))
                    } else {
                        {
                            let mut n = wb.sheets.len() + 1;
                            let name = loop {
                                let cand = format!("Sheet{n}");
                                if wb
                                    .sheets
                                    .iter()
                                    .all(|s| !s.name.eq_ignore_ascii_case(&cand))
                                {
                                    break cand;
                                }
                                n += 1;
                            };
                            let mut ws = Worksheet::new(&name);
                            ws.sheet_id =
                                wb.sheets.iter().map(|s| s.sheet_id).max().unwrap_or(0) + 1;
                            wb.sheets.push(ws);
                            (wb.sheets.len() - 1, CellRef::new(2, 0))
                        }
                    };
                    let index =
                        bo_sheet::pivot::create_pivot(wb, sheet, r, dest_sheet, at, &spec, &labels)
                            .map_err(bad)?;
                    Ok(json!({ "sheet": dest_sheet, "index": index }))
                })?;
                // A new sheet needs its own geometry slot.
                self.invalidate(None);
                Ok(out)
            }
            C::Pivots { sheet } => {
                let wb = self.wb()?;
                let ws = self.sheet(sheet)?;
                Ok(Value::Array(
                    ws.pivots
                        .iter()
                        .enumerate()
                        .map(|(i, p)| pivot_json(wb, i, p, &locale))
                        .collect(),
                ))
            }
            C::PivotUpdate {
                sheet,
                index,
                rows,
                cols,
                values,
                filters,
                labels,
                settings,
            } => {
                let labels = self.pivot_labels(labels);
                self.sheet(sheet)?
                    .pivots
                    .get(index)
                    .ok_or_else(|| bad("no such pivot table"))?;
                self.edit(sheet, Recalc::All, |wb, _| {
                    let pt = wb.sheets[sheet].pivots[index].clone();
                    let spec = updated_spec(wb, &pt, (rows, cols, values, filters), settings);
                    bo_sheet::pivot::update_pivot(wb, sheet, index, &spec, &labels).map_err(bad)?;
                    Ok(json!({ "ok": true }))
                })
            }
            C::PivotGroup {
                sheet,
                index,
                field,
                by,
                years,
                quarters,
                months,
                days,
                start,
                end,
                interval,
                labels,
            } => {
                let labels = self.pivot_labels(labels);
                let req =
                    group_request(&by, [years, quarters, months, days], start, end, interval)?;
                self.pivot_edit(sheet, |wb| {
                    bo_sheet::pivot::group_field(wb, sheet, index, field, &req, &labels)?;
                    Ok(json!({ "ok": true }))
                })
            }
            C::PivotCalcField {
                sheet,
                index,
                field,
                name,
                formula,
                remove,
                labels,
            } => {
                let labels = self.pivot_labels(labels);
                self.pivot_edit(sheet, |wb| {
                    if remove {
                        let f = field.ok_or("no field to remove")?;
                        bo_sheet::pivot::remove_calculated_field(wb, sheet, index, f, &labels)?;
                        return Ok(json!({ "ok": true }));
                    }
                    let f = bo_sheet::pivot::set_calculated_field(
                        wb, sheet, index, field, &name, &formula, &labels,
                    )?;
                    Ok(json!({ "field": f }))
                })
            }
            C::PivotDelete { sheet, index } => self.pivot_edit(sheet, |wb| {
                bo_sheet::pivot::delete_pivot(wb, sheet, index)?;
                Ok(json!({ "ok": true }))
            }),
            C::PivotMove {
                sheet,
                index,
                dest,
                labels,
            } => self.pivot_move(sheet, index, dest, &self.pivot_labels(labels)),
            C::PivotChangeSource {
                sheet,
                index,
                source,
                labels,
            } => self.pivot_change_source(sheet, index, &source, &self.pivot_labels(labels)),
            C::PivotRefreshAll { labels } => {
                let labels = self.pivot_labels(labels);
                self.pivot_edit(0, |wb| {
                    let n = bo_sheet::pivot::refresh_all(wb, &labels)?;
                    Ok(json!({ "count": n }))
                })
            }
            C::PivotRefresh {
                sheet,
                index,
                labels,
            } => {
                let labels = self.pivot_labels(labels);
                let out = self.edit(sheet, Recalc::All, |wb, _| {
                    bo_sheet::pivot::refresh_pivot(wb, sheet, index, &labels).map_err(bad)?;
                    Ok(json!({ "ok": true }))
                })?;
                // Pivots on other sheets may share the cache.
                self.invalidate(None);
                Ok(out)
            }
            C::PivotCalcItem {
                sheet,
                index,
                field,
                old,
                name,
                formula,
                remove,
                labels,
            } => {
                let labels = self.pivot_labels(labels);
                self.pivot_edit(sheet, |wb| {
                    if remove {
                        let n = old.as_deref().unwrap_or(&name);
                        bo_sheet::pivot::remove_calculated_item(
                            wb, sheet, index, field, n, &labels,
                        )?;
                    } else {
                        bo_sheet::pivot::set_calculated_item(
                            wb,
                            sheet,
                            index,
                            field,
                            old.as_deref(),
                            &name,
                            &formula,
                            &labels,
                        )?;
                    }
                    Ok(json!({ "ok": true }))
                })
            }
            C::PivotGroupItems {
                sheet,
                index,
                field,
                items,
                ungroup,
                labels,
            } => {
                let labels = self.pivot_labels(labels);
                self.pivot_edit(sheet, |wb| {
                    if ungroup {
                        bo_sheet::pivot::ungroup_items(wb, sheet, index, field, &items, &labels)?;
                        Ok(json!({ "ok": true }))
                    } else {
                        let g =
                            bo_sheet::pivot::group_items(wb, sheet, index, field, &items, &labels)?;
                        Ok(json!({ "field": g }))
                    }
                })
            }
            C::PivotChart { sheet, index, kind } => {
                let kind = kind.unwrap_or_else(|| "column".into());
                if !bo_sheet::chart::CHART_TYPES.contains(&kind.as_str()) {
                    return Err(bad(format!("unknown chart type '{kind}'")));
                }
                self.edit(sheet, Recalc::No, |wb, _| {
                    let chart = bo_sheet::chart::new_pivot_chart(wb, sheet, index, &kind)
                        .ok_or_else(|| bad("no such pivot table"))?;
                    let ws = &mut wb.sheets[sheet];
                    ws.charts.push(chart);
                    Ok(json!({ "index": ws.charts.len() - 1 }))
                })
            }
            C::SetPageSetup {
                sheet,
                orientation,
                paper_size,
                scale,
                fit,
                margins,
                print_area,
                title_rows,
                grid_lines,
            } => self.edit(sheet, Recalc::No, |wb, _| {
                let ws = &mut wb.sheets[sheet];
                let name = bo_sheet::cells::quote_sheet(&ws.name);
                let p = &mut ws.page_setup;
                if let Some(o) = orientation {
                    p.orientation = Some(o);
                }
                if let Some(s) = paper_size {
                    p.paper_size = Some(s);
                }
                if let Some(s) = scale {
                    p.scale = Some(s.clamp(10, 400));
                }
                if let Some(f) = fit {
                    if let Some([w, h]) = f {
                        ws.props.fit_to_page = Some(true);
                        ws.props.present = true;
                        p.fit_to_width = Some(w);
                        p.fit_to_height = Some(h);
                    } else {
                        ws.props.fit_to_page = None;
                        p.fit_to_width = None;
                        p.fit_to_height = None;
                    }
                }
                if let Some([l, r, t, b]) = margins {
                    let d = PageMargins::default();
                    let base = p.margins.unwrap_or(d);
                    p.margins = Some(PageMargins {
                        left: l.clamp(0.0, 10.0),
                        right: r.clamp(0.0, 10.0),
                        top: t.clamp(0.0, 10.0),
                        bottom: b.clamp(0.0, 10.0),
                        ..base
                    });
                }
                if let Some(g) = grid_lines {
                    p.print_grid_lines = g;
                    p.has_print_options |= g;
                }
                if let Some(pa) = print_area {
                    ws.print_area = pa.map(|r| bo_sheet::cells::abs_ref(&ws.name, r.range()));
                }
                if let Some(tr) = title_rows {
                    ws.print_titles =
                        tr.map(|[a, b]| format!("{name}!${}:${}", a.min(b) + 1, a.max(b) + 1));
                }
                Ok(json!({ "ok": true }))
            }),
        }
    }

    fn autofit(&mut self, sheet: usize, rows: bool, first: u32, last: u32) -> Result<Value> {
        let locale = self.locale;
        let metrics = self.geometry(sheet)?.metrics;
        let wb = self.wb.as_ref().ok_or(EngineError::NoDocument)?;
        let ws = wb
            .sheets
            .get(sheet)
            .ok_or(EngineError::PageOutOfRange(sheet))?;
        let mut sizes: Vec<(u32, f64)> = Vec::new();
        let in_merge = |r: u32, c: u32| ws.merge_at(CellRef::new(r, c)).is_some();
        if rows {
            for r in first..=last.min(MAX_ROWS - 1) {
                let mut h: f64 = 0.0;
                for (&(rr, cc), cell) in ws.cells.range((r, 0)..=(r, MAX_COLS)) {
                    if non_empty(cell) && !in_merge(rr, cc) {
                        h = h.max(
                            bo_sheet::render::measure_cell(wb, cell, &mut self.fonts, &locale).1,
                        );
                    }
                }
                sizes.push((
                    r,
                    if h > 0.0 {
                        h.max(metrics.default_row_height_pt)
                    } else {
                        metrics.default_row_height_pt
                    },
                ));
                if sizes.len() > 100_000 {
                    break;
                }
            }
        } else {
            let mut widths = std::collections::HashMap::<u32, f64>::new();
            for (&(rr, cc), cell) in &ws.cells {
                if cc >= first && cc <= last && non_empty(cell) && !in_merge(rr, cc) {
                    let w = bo_sheet::render::measure_cell(wb, cell, &mut self.fonts, &locale).0;
                    let e = widths.entry(cc).or_insert(0.0);
                    *e = e.max(w);
                }
            }
            let default = metrics.default_col_pt(ws);
            for c in first..=last.min(MAX_COLS - 1) {
                sizes.push((
                    c,
                    widths
                        .get(&c)
                        .copied()
                        .unwrap_or(default)
                        .clamp(6.0, 1_500.0),
                ));
                if sizes.len() > 16_384 {
                    break;
                }
            }
        }
        self.edit(sheet, Recalc::No, |wb, _| {
            let ws = &mut wb.sheets[sheet];
            for (i, s) in &sizes {
                if rows {
                    bo_sheet::rowcol::set_row_height(ws, *i, *i, *s);
                } else {
                    bo_sheet::rowcol::set_col_width(ws, &metrics, *i, *i, *s);
                }
            }
            Ok(json!({ "ok": true, "sizes": sizes }))
        })
    }

    /// Input text of a cell (formula bar form).
    fn input_of(&self, cell: &Cell) -> String {
        match (&cell.formula, &cell.value) {
            (Some(f), _) if !f.text.is_empty() => format!("={}", f.text),
            (_, CellValue::Number(n)) => self
                .wb
                .as_ref()
                .and_then(|wb| {
                    let fmt_id = wb.styles.xf(cell.style).map_or(0, |x| x.num_fmt_id);
                    date_input(*n, &wb.styles, fmt_id, &self.locale, wb.date1904)
                })
                .unwrap_or_else(|| number_input(*n, &self.locale)),
            (_, CellValue::Bool(b)) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
            (_, CellValue::Error(e)) => e.as_str().to_owned(),
            (_, v) => v.as_text().unwrap_or_default(),
        }
    }

    fn find(
        &self,
        query: &str,
        match_case: bool,
        whole_cell: bool,
        in_formulas: bool,
        sheet: Option<usize>,
    ) -> Result<Vec<At>> {
        let wb = self.wb()?;
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let q = if match_case {
            query.to_owned()
        } else {
            query.to_lowercase()
        };
        let mut out = Vec::new();
        for (si, ws) in wb.sheets.iter().enumerate() {
            if sheet.is_some_and(|s| s != si) {
                continue;
            }
            for (&(row, col), cell) in &ws.cells {
                let text = if in_formulas {
                    self.input_of(cell)
                } else {
                    bo_sheet::render::format_cell(wb, cell, &self.locale).text
                };
                let t = if match_case {
                    text
                } else {
                    text.to_lowercase()
                };
                let hit = if whole_cell { t == q } else { t.contains(&q) };
                if hit {
                    out.push(At {
                        sheet: si,
                        row,
                        col,
                    });
                    if out.len() >= 10_000 {
                        return Ok(out);
                    }
                }
            }
        }
        Ok(out)
    }

    fn replace(
        &mut self,
        query: &str,
        with: &str,
        match_case: bool,
        whole_cell: bool,
        sheet: Option<usize>,
        at: Option<At>,
    ) -> Result<Value> {
        let targets = match at {
            Some(a) => vec![a],
            None => self.find(query, match_case, whole_cell, true, sheet)?,
        };
        let mut by_sheet: std::collections::BTreeMap<usize, Vec<(u32, u32, String)>> =
            std::collections::BTreeMap::new();
        let mut count = 0;
        {
            let wb = self.wb()?;
            for t in &targets {
                let Some(cell) = wb.sheets.get(t.sheet).and_then(|s| s.cell(t.row, t.col)) else {
                    continue;
                };
                let input = self.input_of(cell);
                let new = if whole_cell {
                    let same = if match_case {
                        input == query
                    } else {
                        input.to_lowercase() == query.to_lowercase()
                    };
                    same.then(|| with.to_owned())
                } else {
                    let (s, n) = replace_text(&input, query, with, match_case);
                    (n > 0).then_some(s)
                };
                if let Some(n) = new {
                    count += 1;
                    by_sheet.entry(t.sheet).or_default().push((t.row, t.col, n));
                }
            }
        }
        for (si, cells) in by_sheet {
            self.set_inputs(si, &cells)?;
        }
        Ok(json!({ "count": count }))
    }

    fn copy(&mut self, sheet: usize, range: CellRange, cut: bool) -> Result<Value> {
        let ws = self.sheet(sheet)?;
        if u64::from(range.rows()) * u64::from(range.cols()) > 1_000_000 {
            return Err(bad("selection too large to copy"));
        }
        let wb = self.wb()?;
        let mut cells = Vec::new();
        let mut lines = Vec::with_capacity(range.rows() as usize);
        for r in range.first.row..=range.last.row {
            let mut line = Vec::with_capacity(range.cols() as usize);
            for c in range.first.col..=range.last.col {
                match ws.cell(r, c) {
                    Some(cell) => {
                        cells.push((r - range.first.row, c - range.first.col, cell.clone()));
                        let text = if cell.formula.is_some() {
                            bo_sheet::render::format_cell(wb, cell, &self.locale).text
                        } else {
                            self.input_of(cell)
                        };
                        line.push(tsv_field(&text));
                    }
                    None => line.push(String::new()),
                }
            }
            lines.push(line.join("\t"));
        }
        let text = lines.join("\r\n");
        self.clip = Some(super::Clip {
            sheet,
            range,
            cells,
            cut,
        });
        Ok(json!({ "text": text }))
    }

    #[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
    fn paste(
        &mut self,
        sheet: usize,
        row: u32,
        col: u32,
        mode: PasteMode,
        transpose: bool,
        skip_blanks: bool,
        target: Option<Rng>,
    ) -> Result<Value> {
        let Some(clip) = self.clip.clone() else {
            return Ok(Value::Null);
        };
        let (h, w) = if transpose {
            (clip.range.cols(), clip.range.rows())
        } else {
            (clip.range.rows(), clip.range.cols())
        };
        // A copy fills a larger selection whose size is a multiple of the block.
        let (tiles_r, tiles_c) = match target.map(Rng::range) {
            Some(t)
                if !clip.cut
                    && t.first == CellRef::new(row, col)
                    && t.rows() % h == 0
                    && t.cols() % w == 0 =>
            {
                (t.rows() / h, t.cols() / w)
            }
            _ => (1, 1),
        };
        if u64::from(tiles_r) * u64::from(tiles_c) * u64::from(h) * u64::from(w) > 4_000_000 {
            return Err(bad("paste area too large"));
        }
        let last_r = u64::from(row) + u64::from(h) * u64::from(tiles_r) - 1;
        let last_c = u64::from(col) + u64::from(w) * u64::from(tiles_c) - 1;
        if last_r >= u64::from(MAX_ROWS) || last_c >= u64::from(MAX_COLS) {
            return Err(bad("paste area outside the sheet"));
        }
        let target = CellRange::new(
            CellRef::new(row, col),
            CellRef::new(last_r as u32, last_c as u32),
        );
        let cut = clip.cut;
        let recalc = if cut {
            // References elsewhere follow the moved cells.
            Recalc::All
        } else {
            Recalc::Range(sheet, target)
        };
        let out = self.edit(sheet, recalc, |wb, adj| {
            // Cut: clear the source first (cells under the target are written again).
            if cut {
                if let Some(src) = wb.sheets.get_mut(clip.sheet) {
                    for (dr, dc, _) in &clip.cells {
                        src.cells
                            .remove(&(clip.range.first.row + dr, clip.range.first.col + dc));
                    }
                }
            }
            let by_pos: std::collections::HashMap<(u32, u32), &Cell> = clip
                .cells
                .iter()
                .map(|(r, c, cell)| ((*r, *c), cell))
                .collect();
            for ti in 0..tiles_r {
                for tj in 0..tiles_c {
                    let (row, col) = (row + ti * h, col + tj * w);
                    let ws = &mut wb.sheets[sheet];
                    for dr in 0..clip.range.rows() {
                        for dc in 0..clip.range.cols() {
                            let (tr, tc) = if transpose {
                                (row + dc, col + dr)
                            } else {
                                (row + dr, col + dc)
                            };
                            let src = by_pos.get(&(dr, dc)).copied();
                            if skip_blanks && src.is_none_or(|c| !non_empty(c)) {
                                continue;
                            }
                            let old = ws.cells.get(&(tr, tc)).cloned();
                            let moved_formula = |c: &Cell| {
                                c.formula.as_ref().map(|f| {
                                    let mut f = f.clone();
                                    // A cut keeps its formulas; references are
                                    // re-pointed below. A copy shifts relative ones.
                                    if !cut {
                                        let d_r =
                                            i64::from(tr) - i64::from(clip.range.first.row + dr);
                                        let d_c =
                                            i64::from(tc) - i64::from(clip.range.first.col + dc);
                                        if let Some(t) = adj.translate(&f.text, d_r, d_c) {
                                            f.text = t;
                                        }
                                    }
                                    f.kind = bo_model::sheet::FormulaKind::Normal;
                                    f
                                })
                            };
                            let new = match (mode, src) {
                                (PasteMode::All, Some(c)) => Some(Cell {
                                    formula: moved_formula(c),
                                    ..c.clone()
                                }),
                                (PasteMode::All, None) => None,
                                (PasteMode::Values, s) => {
                                    let mut n = old.clone().unwrap_or_default();
                                    n.formula = None;
                                    n.value = s.map_or(CellValue::Empty, |c| c.value.clone());
                                    Some(n)
                                }
                                (PasteMode::Formulas, s) => {
                                    let mut n = old.clone().unwrap_or_default();
                                    n.formula = s.and_then(moved_formula);
                                    n.value = s.map_or(CellValue::Empty, |c| c.value.clone());
                                    Some(n)
                                }
                                (PasteMode::Formats, s) => {
                                    let mut n = old.clone().unwrap_or_default();
                                    n.style = s.map_or(0, |c| c.style);
                                    Some(n)
                                }
                            };
                            match new {
                                Some(c) if non_empty(&c) || c.style != 0 => {
                                    ws.cells.insert((tr, tc), c);
                                }
                                _ => {
                                    ws.cells.remove(&(tr, tc));
                                }
                            }
                        }
                    }
                }
            }
            if cut && !transpose {
                let mv = bo_sheet::calc::CellMove {
                    from_sheet: wb
                        .sheets
                        .get(clip.sheet)
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                    to_sheet: wb.sheets[sheet].name.clone(),
                    first_row: clip.range.first.row,
                    first_col: clip.range.first.col,
                    last_row: clip.range.last.row,
                    last_col: clip.range.last.col,
                    d_rows: i64::from(row) - i64::from(clip.range.first.row),
                    d_cols: i64::from(col) - i64::from(clip.range.first.col),
                };
                for ws in &mut wb.sheets {
                    let name = ws.name.clone();
                    for cell in ws.cells.values_mut() {
                        if let Some(f) = cell.formula.as_mut().filter(|f| !f.text.is_empty()) {
                            if let Some(t) = adj.moved(&f.text, &name, &mv) {
                                f.text = t;
                            }
                        }
                    }
                }
                for d in &mut wb.defined_names {
                    if let Some(t) = adj.moved(&d.formula, "", &mv) {
                        d.formula = t;
                    }
                }
            }
            Ok(json!({ "range": Rng::of(target) }))
        })?;
        if cut {
            // A cut is pasted once.
            self.clip = None;
        }
        Ok(out)
    }
    /// Current internal clipboard (tests, hosts).
    pub fn clipboard(&self) -> Option<&Clip> {
        self.clip.as_ref()
    }

    /// Input conversion for host-side checks (e.g. the formula bar preview).
    pub fn parse_input(&self, input: &str) -> Option<Cell> {
        input_to_cell(input, None, &self.locale)
    }
}

/// Content (value or formula) of locked cells of protected sheets must not
/// change (ECMA-376 Part 1, §18.3.1.85 with §18.8.33 `protection/@locked`).
fn locked_cells_unchanged(before: &Workbook, after: &Workbook) -> Result<()> {
    for (si, old) in before.sheets.iter().enumerate() {
        if !super::protect::sheet_info(old).protected {
            continue;
        }
        let Some(new) = after.sheets.get(si) else {
            continue;
        };
        let same = |a: Option<&Cell>, b: Option<&Cell>| match (a, b) {
            (Some(a), Some(b)) => a.value == b.value && a.formula == b.formula,
            (None, None) => true,
            (Some(c), None) | (None, Some(c)) => !non_empty(c),
        };
        let keys = old.cells.keys().chain(new.cells.keys());
        for &(r, c) in keys {
            if !same(old.cells.get(&(r, c)), new.cells.get(&(r, c)))
                && super::protect::cell_locked(before, old, r, c)
            {
                return Err(EngineError::Sheet(super::protect::SHEET_PROTECTED.into()));
            }
        }
    }
    Ok(())
}

/// Moves a sheet, keeping sheet-scoped names (`localSheetId`, §18.2.5) and the
/// pivot caches pointing at the same sheets.
fn move_sheet(wb: &mut Workbook, from: usize, to: usize) {
    let ws = wb.sheets.remove(from);
    wb.sheets.insert(to, ws);
    let map = |i: usize| -> usize {
        if i == from {
            to
        } else if from < to && i > from && i <= to {
            i - 1
        } else if to < from && i >= to && i < from {
            i + 1
        } else {
            i
        }
    };
    for dn in &mut wb.defined_names {
        if let Some(s) = dn.local_sheet {
            if let Ok(i) = usize::try_from(s) {
                dn.local_sheet = u32::try_from(map(i)).ok();
            }
        }
    }
}

/// Status-bar figures of a multi-range selection.
fn range_stats(ws: &Worksheet, ranges: &[Rng]) -> Value {
    let mut seen = std::collections::BTreeSet::new();
    let (mut sum, mut numbers, mut count) = (0.0_f64, 0_u64, 0_u64);
    for r in ranges {
        let r = r.range();
        for row in r.first.row..=r.last.row {
            for (&(rr, cc), cell) in ws.cells.range((row, r.first.col)..=(row, r.last.col)) {
                if !seen.insert((rr, cc)) || !non_empty(cell) {
                    continue;
                }
                count += 1;
                if let CellValue::Number(n) = cell.value {
                    sum += n;
                    numbers += 1;
                }
            }
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let average = (numbers > 0).then(|| sum / numbers as f64);
    json!({ "sum": sum, "count": count, "numbers": numbers, "average": average })
}

impl SheetEngine {
    /// Runs `cmds` as one undo step: on error the workbook is restored.
    fn batch(&mut self, cmds: Vec<SheetCommand>) -> Result<Value> {
        let before = Box::new(self.wb()?.clone());
        let depth = self.undo.len();
        let mut out = Vec::with_capacity(cmds.len());
        for c in cmds {
            if matches!(c, SheetCommand::Batch { .. }) {
                return Err(bad("nested batch"));
            }
            match self.exec(c) {
                Ok(v) => out.push(v),
                Err(e) => {
                    self.undo.truncate(depth);
                    if let Some(wb) = self.wb.as_mut() {
                        *wb = *before;
                        self.live.rebuild(wb);
                    }
                    self.invalidate(None);
                    return Err(e);
                }
            }
        }
        if self.undo.len() > depth {
            self.undo.truncate(depth);
            self.undo.push(UndoEntry::Workbook(before));
        }
        Ok(Value::Array(out))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{HitInfo, ViewRequest};
    use super::*;

    fn engine() -> SheetEngine {
        let mut e = SheetEngine::with_bundled_fonts();
        e.new_workbook("Book1.xlsx").unwrap();
        e
    }

    #[allow(clippy::needless_pass_by_value)]
    fn run(e: &mut SheetEngine, v: Value) -> Value {
        let s = e.exec_json(&v.to_string()).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    fn rng(r0: u32, c0: u32, r1: u32, c1: u32) -> Value {
        json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 })
    }

    fn reopen(e: &mut SheetEngine) -> SheetEngine {
        let bytes = e.save("xlsx", 0).unwrap();
        let mut back = SheetEngine::with_bundled_fonts();
        back.open(&bytes, "x.xlsx").unwrap();
        back
    }

    // source: ECMA-376 Part 1, §18.17.4.1 (serial 45351 = 2024-02-29), §18.8.30 (ids 15, 17)
    #[test]
    fn typed_dates_with_month_names() {
        let mut e = engine();
        e.set_input(0, 0, 0, "29 tháng 2 năm 2024").unwrap();
        e.set_input(0, 1, 0, "tháng 2/2024").unwrap();
        e.set_input(0, 2, 0, "29 tháng 2").unwrap();
        e.set_locale("en-US");
        e.set_input(0, 3, 0, "February 29, 2024").unwrap();
        let wb = e.workbook().unwrap();
        let ws = &wb.sheets[0];
        let got = |r: u32| {
            let c = ws.cell(r, 0).unwrap();
            (c.value.clone(), wb.styles.xf(c.style).unwrap().num_fmt_id)
        };
        assert_eq!(got(0), (CellValue::Number(45351.0), 15));
        assert_eq!(got(1), (CellValue::Number(45323.0), 17));
        // No year: left as text.
        assert_eq!(got(2).0, CellValue::Text("29 tháng 2".into()));
        assert_eq!(got(3), (CellValue::Number(45351.0), 15));
        assert_eq!(e.cell(0, 3, 0).unwrap().display, "29-Feb-24");
        // The formula bar shows the date (not the serial) for these formats
        // and for a custom date format; confirming it keeps value and format.
        assert_eq!(e.cell(0, 3, 0).unwrap().input, "2/29/2024");
        e.set_locale("vi-VN");
        assert_eq!(e.cell(0, 0, 0).unwrap().input, "29/02/2024");
        assert_eq!(e.cell(0, 1, 0).unwrap().input, "01/02/2024");
        e.set_input(0, 0, 0, "29/02/2024").unwrap();
        let wb = e.workbook().unwrap();
        let c = wb.sheets[0].cell(0, 0).unwrap();
        assert_eq!(c.value, CellValue::Number(45351.0));
        assert_eq!(wb.styles.xf(c.style).unwrap().num_fmt_id, 15);
        run(
            &mut e,
            json!({ "cmd": "format", "sheet": 0, "range": rng(5, 0, 5, 0), "patch": { "numFmt": "d mmm yyyy" } }),
        );
        e.set_input(0, 5, 0, "45351,5").unwrap();
        assert_eq!(e.cell(0, 5, 0).unwrap().input, "29/02/2024 12:00:00");
        e.set_input(0, 5, 0, "0,25").unwrap();
        assert_eq!(e.cell(0, 5, 0).unwrap().input, "6:00:00");
    }

    // source: ECMA-376 Part 1, §18.17.4.1 (serial 45351 = 2024-02-29), §18.8.30 (ids 14, 20, 22)
    #[test]
    fn typed_dates_become_serials() {
        let mut e = engine();
        e.set_input(0, 0, 0, "29/02/2024").unwrap();
        e.set_input(0, 1, 0, "2024-02-29 18:00").unwrap();
        e.set_input(0, 2, 0, "18:30").unwrap();
        e.set_input(0, 3, 0, "=DATE(2024;2;29)").unwrap();
        e.set_input(0, 4, 0, "'29/02/2024").unwrap();
        e.set_input(0, 5, 0, "1/2").unwrap();
        e.set_input(0, 6, 0, "2/29/2024").unwrap();
        let wb = e.workbook().unwrap();
        let ws = &wb.sheets[0];
        let fmt = |r: u32| {
            wb.styles
                .xf(ws.cell(r, 0).unwrap().style)
                .unwrap()
                .num_fmt_id
        };
        assert_eq!(ws.cell(0, 0).unwrap().value, CellValue::Number(45351.0));
        assert_eq!(fmt(0), 14);
        assert_eq!(ws.cell(1, 0).unwrap().value, CellValue::Number(45351.75));
        assert_eq!(fmt(1), 22);
        assert_eq!(fmt(2), 20);
        assert!(ws.cell(3, 0).unwrap().formula.is_some());
        assert_eq!(
            ws.cell(4, 0).unwrap().value,
            CellValue::Text("29/02/2024".into())
        );
        assert_eq!(ws.cell(5, 0).unwrap().value, CellValue::Text("1/2".into()));
        // Month-first is not a valid vi-VN date: kept as text.
        assert_eq!(
            ws.cell(6, 0).unwrap().value,
            CellValue::Text("2/29/2024".into())
        );
        let d = e.cell(0, 0, 0).unwrap();
        assert_eq!(
            (d.display.as_str(), d.input.as_str()),
            ("29/02/2024", "29/02/2024")
        );
        assert_eq!(e.cell(0, 1, 0).unwrap().input, "29/02/2024 18:00:00");
        // Re-confirming the formula-bar text keeps the value and format.
        e.set_input(0, 1, 0, "29/02/2024 18:00:00").unwrap();
        assert_eq!(
            e.workbook().unwrap().sheets[0].cell(1, 0).unwrap().value,
            CellValue::Number(45351.75)
        );
        // A date format chosen by the user is kept; a text format keeps text.
        run(
            &mut e,
            json!({ "cmd": "format", "sheet": 0, "range": rng(8, 0, 9, 0), "patch": { "numFmt": "d mmm yyyy" } }),
        );
        run(
            &mut e,
            json!({ "cmd": "format", "sheet": 0, "range": rng(10, 0, 10, 0), "patch": { "numFmt": "@" } }),
        );
        e.set_input(0, 8, 0, "1/3/2024").unwrap();
        e.set_input(0, 10, 0, "1/3/2024").unwrap();
        assert_eq!(e.cell(0, 8, 0).unwrap().display, "1 Thg 3 2024");
        assert_eq!(
            e.workbook().unwrap().sheets[0].cell(10, 0).unwrap().value,
            CellValue::Text("1/3/2024".into())
        );
        // en-US reads month first.
        e.set_locale("en-US");
        e.set_input(0, 12, 0, "2/29/2024").unwrap();
        e.set_input(0, 13, 0, "29/2/2024").unwrap();
        let ws = &e.workbook().unwrap().sheets[0];
        assert_eq!(ws.cell(12, 0).unwrap().value, CellValue::Number(45351.0));
        assert_eq!(
            ws.cell(13, 0).unwrap().value,
            CellValue::Text("29/2/2024".into())
        );
        // Survives a save.
        let back = reopen(&mut e);
        assert_eq!(
            back.workbook().unwrap().sheets[0].cell(0, 0).unwrap().value,
            CellValue::Number(45351.0)
        );
    }

    #[test]
    fn format_cells_survive_save() {
        let mut e = engine();
        e.set_input(0, 0, 0, "1234567").unwrap();
        run(
            &mut e,
            json!({ "cmd": "format", "sheet": 0, "range": rng(0, 0, 1, 1),
            "patch": { "bold": true, "fillColor": "FFEE00", "numFmt": "#,##0 [$₫-42A]", "hAlign": "center" } }),
        );
        run(
            &mut e,
            json!({ "cmd": "borders", "sheet": 0, "range": rng(0, 0, 1, 1),
            "spec": { "preset": "outline", "style": "medium" } }),
        );
        run(
            &mut e,
            json!({ "cmd": "cellStyle", "sheet": 0, "range": rng(3, 0, 3, 0), "name": "Bad" }),
        );
        assert_eq!(e.cell(0, 0, 0).unwrap().display, "1.234.567 ₫");
        let n = e.workbook().unwrap().styles.cell_xfs.len();
        let mut back = reopen(&mut e);
        let st = run(
            &mut back,
            json!({ "cmd": "formatState", "sheet": 0, "row": 0, "col": 0 }),
        );
        assert_eq!(st["bold"], true);
        assert_eq!(st["fillColor"], "FFEE00");
        assert_eq!(st["hAlign"], "center");
        assert_eq!(st["borders"][0], "medium");
        assert_eq!(st["numFmt"], "#,##0 [$₫-42A]");
        let st = run(
            &mut back,
            json!({ "cmd": "formatState", "sheet": 0, "row": 3, "col": 0 }),
        );
        assert_eq!(st["styleName"], "Bad");
        assert_eq!(
            back.workbook().unwrap().styles.cell_xfs.len(),
            n,
            "xfs deduplicated"
        );
        // Format painter + undo.
        run(
            &mut back,
            json!({ "cmd": "paintFormat", "sheet": 0, "from": rng(0, 0, 0, 0), "to": rng(5, 5, 6, 6) }),
        );
        assert_eq!(
            run(
                &mut back,
                json!({ "cmd": "formatState", "sheet": 0, "row": 6, "col": 6 })
            )["bold"],
            true
        );
        assert!(back.undo().unwrap());
        assert_eq!(
            run(
                &mut back,
                json!({ "cmd": "formatState", "sheet": 0, "row": 6, "col": 6 })
            )["bold"],
            false
        );
        assert!(back.exec_json("{\"cmd\":\"nope\"}").is_err());
    }

    #[test]
    fn rows_columns_freeze_merge() {
        let mut e = engine();
        e.set_input(0, 0, 0, "Một chuỗi văn bản khá dài để tự giãn cột")
            .unwrap();
        run(
            &mut e,
            json!({ "cmd": "autofit", "sheet": 0, "rows": false, "first": 0, "last": 0 }),
        );
        let w = e.geometry(0).unwrap().col_width(0);
        assert!(w > 150.0, "autofit width {w}");
        run(
            &mut e,
            json!({ "cmd": "resize", "sheet": 0, "rows": true, "first": 2, "last": 3, "size": 30 }),
        );
        assert_eq!(e.geometry(0).unwrap().row_height(3), 30.0);
        run(
            &mut e,
            json!({ "cmd": "hide", "sheet": 0, "rows": true, "first": 5, "last": 6, "hidden": true }),
        );
        assert_eq!(e.info().unwrap().sheets[0].hidden_rows, vec![(5, 6)]);
        run(
            &mut e,
            json!({ "cmd": "freeze", "sheet": 0, "rows": 1, "cols": 1 }),
        );
        assert_eq!(e.info().unwrap().sheets[0].frozen_rows, 1);
        e.set_input(0, 10, 0, "x").unwrap();
        e.set_input(0, 10, 1, "y").unwrap();
        run(
            &mut e,
            json!({ "cmd": "merge", "sheet": 0, "range": rng(10, 0, 11, 2), "center": true }),
        );
        assert_eq!(e.info().unwrap().sheets[0].merges, vec![[10, 0, 11, 2]]);
        assert_eq!(e.cell(0, 10, 1).unwrap().input, "");
        let mut back = reopen(&mut e);
        let info = back.info().unwrap();
        assert_eq!(
            (info.sheets[0].frozen_rows, info.sheets[0].frozen_cols),
            (1, 1)
        );
        assert_eq!(info.sheets[0].merges.len(), 1);
        assert_eq!(info.sheets[0].hidden_rows, vec![(5, 6)]);
        assert!(back.geometry(0).unwrap().col_width(0) > 150.0);
        // Header edges hit-test as resize handles.
        let req = ViewRequest {
            sheet: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            width: 600.0,
            height: 300.0,
        };
        let x = 30.0 + back.geometry(0).unwrap().col_width(0);
        assert_eq!(
            back.hit_test(&req, x - 1.0, 5.0).unwrap(),
            HitInfo::ColumnEdge { col: 0 }
        );
        assert_eq!(
            back.hit_test(&req, x + 20.0, 5.0).unwrap(),
            HitInfo::ColumnHeader { col: 1 }
        );
        assert_eq!(
            back.hit_test(&req, 5.0, 29.5).unwrap(),
            HitInfo::RowEdge { row: 0 }
        );
        run(
            &mut back,
            json!({ "cmd": "unmerge", "sheet": 0, "range": rng(10, 1, 10, 1) }),
        );
        assert_eq!(
            back.info().unwrap().sheets[0].merges,
            Vec::<[u32; 4]>::new()
        );
    }

    #[test]
    fn sort_filter_and_fill() {
        let mut e = engine();
        let rows = [
            ("Tên", "Điểm"),
            ("Lan", "7"),
            ("An", "9"),
            ("Bình", "7"),
            ("Chi", "5"),
        ];
        for (i, (a, b)) in rows.iter().enumerate() {
            e.set_input(0, i as u32, 0, a).unwrap();
            e.set_input(0, i as u32, 1, b).unwrap();
        }
        e.set_input(0, 1, 2, "=B2*10").unwrap();
        run(
            &mut e,
            json!({ "cmd": "sort", "sheet": 0, "range": rng(0, 0, 4, 1), "hasHeader": true,
            "keys": [{ "col": 1, "descending": true }, { "col": 0 }] }),
        );
        let col = |e: &SheetEngine, c| {
            (1..5)
                .map(|r| e.cell(0, r, c).unwrap().input)
                .collect::<Vec<_>>()
        };
        assert_eq!(col(&e, 0), vec!["An", "Bình", "Lan", "Chi"]);
        // Single-cell selection expands to the current region.
        let region = run(
            &mut e,
            json!({ "cmd": "region", "sheet": 0, "range": rng(2, 1, 2, 1) }),
        );
        assert_eq!(region, rng(0, 0, 4, 2));
        run(
            &mut e,
            json!({ "cmd": "toggleFilter", "sheet": 0, "range": rng(0, 0, 0, 0) }),
        );
        assert_eq!(e.info().unwrap().sheets[0].auto_filter, Some([0, 0, 4, 2]));
        let vals = run(
            &mut e,
            json!({ "cmd": "filterValues", "sheet": 0, "col": 1 }),
        );
        assert_eq!(vals["values"].as_array().unwrap().len(), 3);
        run(
            &mut e,
            json!({ "cmd": "setFilter", "sheet": 0, "col": 1, "filter": { "values": ["7"] } }),
        );
        assert_eq!(
            e.info().unwrap().sheets[0].hidden_rows,
            vec![(1, 1), (4, 4)]
        );
        let vals = run(
            &mut e,
            json!({ "cmd": "filterValues", "sheet": 0, "col": 1 }),
        );
        assert_eq!(vals["kind"], "values");
        run(
            &mut e,
            json!({ "cmd": "setFilter", "sheet": 0, "col": 1,
            "filter": { "conditions": [{ "operator": "greaterThan", "value": "6" }] } }),
        );
        assert_eq!(e.info().unwrap().sheets[0].hidden_rows, vec![(4, 4)]);
        let req = ViewRequest {
            sheet: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            width: 600.0,
            height: 300.0,
        };
        let r = e.range_rect(&req, (0, 1), (0, 1)).unwrap().unwrap();
        assert_eq!(
            e.hit_test(&req, r.x + r.w - 4.0, r.y + r.h / 2.0).unwrap(),
            HitInfo::FilterButton { row: 0, col: 1 }
        );
        run(
            &mut e,
            json!({ "cmd": "toggleFilter", "sheet": 0, "range": rng(0, 0, 0, 0) }),
        );
        assert_eq!(
            e.info().unwrap().sheets[0].hidden_rows,
            Vec::<(u32, u32)>::new()
        );
        // Fill: series + formula translation, Ctrl+D.
        e.set_input(0, 10, 0, "1").unwrap();
        e.set_input(0, 11, 0, "3").unwrap();
        e.set_input(0, 10, 1, "=A11*2").unwrap();
        run(
            &mut e,
            json!({ "cmd": "autofill", "sheet": 0, "source": rng(10, 0, 11, 0), "target": rng(10, 0, 13, 0) }),
        );
        assert_eq!(e.cell(0, 13, 0).unwrap().input, "7");
        run(
            &mut e,
            json!({ "cmd": "fillDir", "sheet": 0, "range": rng(10, 1, 13, 1), "dir": "down" }),
        );
        assert_eq!(e.cell(0, 13, 1).unwrap().input, "=A14*2");
        assert_eq!(e.cell(0, 13, 1).unwrap().display, "14");
    }

    #[test]
    fn conditional_formats_and_validation() {
        let mut e = engine();
        for r in 0..5 {
            e.set_input(0, r, 0, &(r * 10).to_string()).unwrap();
        }
        run(
            &mut e,
            json!({ "cmd": "addCf", "sheet": 0, "range": rng(0, 0, 4, 0),
            "rule": { "kind": "cellIs", "operator": "greaterThan", "formula1": "25", "style": { "fill": "FF0000" } } }),
        );
        run(
            &mut e,
            json!({ "cmd": "addCf", "sheet": 0, "range": rng(0, 0, 4, 0),
            "rule": { "kind": "iconSet", "set": "3Arrows" } }),
        );
        let req = ViewRequest {
            sheet: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            width: 300.0,
            height: 200.0,
        };
        let page = e.render(&req).unwrap();
        let red = bo_render::Rgba(255, 0, 0, 255);
        assert!(page
            .commands
            .iter()
            .any(|c| matches!(c, bo_render::Command::FillRect { color, .. } if *color == red)));
        let mut back = reopen(&mut e);
        assert_eq!(
            back.workbook().unwrap().sheets[0].conditional_formats.len(),
            2
        );
        run(
            &mut back,
            json!({ "cmd": "clearCf", "sheet": 0, "range": rng(0, 0, 0, 0) }),
        );
        assert_eq!(
            back.workbook().unwrap().sheets[0].conditional_formats.len(),
            0
        );
        // Validation list.
        run(
            &mut e,
            json!({ "cmd": "setValidation", "sheet": 0, "range": rng(0, 2, 9, 2),
            "spec": { "kind": "list", "formula1": "Có,Không", "error": "Chọn trong danh sách" } }),
        );
        let v = run(
            &mut e,
            json!({ "cmd": "validation", "sheet": 0, "row": 3, "col": 2 }),
        );
        assert_eq!(v["list"], json!(["Có", "Không"]));
        let r = run(
            &mut e,
            json!({ "cmd": "setInputs", "sheet": 0, "cells": [[3, 2, "Có lẽ"]], "validate": true }),
        );
        assert_eq!(r["rejected"]["message"], "Chọn trong danh sách");
        assert_eq!(e.cell(0, 3, 2).unwrap().input, "");
        let r = run(
            &mut e,
            json!({ "cmd": "setInputs", "sheet": 0, "cells": [[3, 2, "không"]], "validate": true }),
        );
        assert!(r["rejected"].is_null());
        assert_eq!(e.cell(0, 3, 2).unwrap().input, "không");
        let back = reopen(&mut e);
        assert_eq!(back.workbook().unwrap().sheets[0].data_validations.len(), 1);
    }

    #[test]
    fn charts_insert_edit_save() {
        let mut e = engine();
        let data = [
            ["Tỉnh", "2024", "2025"],
            ["HN", "10", "12"],
            ["HUE", "5", "4"],
        ];
        for (r, row) in data.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                e.set_input(0, r as u32, c as u32, v).unwrap();
            }
        }
        let i = run(
            &mut e,
            json!({ "cmd": "insertChart", "sheet": 0, "range": rng(0, 0, 2, 2), "kind": "column" }),
        );
        assert_eq!(i["index"], 0);
        run(
            &mut e,
            json!({ "cmd": "editChart", "sheet": 0, "index": 0, "title": "Doanh thu",
            "legend": "b", "xTitle": "Tỉnh", "yTitle": "Tỷ" }),
        );
        let req = ViewRequest {
            sheet: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            width: 900.0,
            height: 400.0,
        };
        let page = e.render(&req).unwrap();
        assert!(page.commands.iter().any(|c| matches!(c, bo_render::Command::FillRect { color, .. } if *color == bo_sheet::chart::PALETTE[1])));
        let r = e.range_rect(&req, (2, 5), (2, 5)).unwrap().unwrap();
        assert_eq!(
            e.hit_test(&req, r.x + 2.0, r.y + 2.0).unwrap(),
            HitInfo::Chart {
                index: 0,
                row: 2,
                col: 5
            }
        );
        let mut back = reopen(&mut e);
        let list = run(&mut back, json!({ "cmd": "charts", "sheet": 0 }));
        assert_eq!(list[0]["title"], "Doanh thu");
        assert_eq!(list[0]["legend"], "b");
        assert_eq!(list[0]["yTitle"], "Tỷ");
        assert_eq!(list[0]["kind"], "column");
        run(
            &mut back,
            json!({ "cmd": "editChart", "sheet": 0, "index": 0, "kind": "line", "title": "" }),
        );
        let again = reopen(&mut back);
        let c = &again.workbook().unwrap().sheets[0].charts[0];
        assert_eq!(c.plots[0].kind, bo_model::sheet::ChartKind::Line);
        assert!(c.title.is_none());
        // Charts read from the file move, resize and delete too.
        let mut again = again;
        run(
            &mut again,
            json!({ "cmd": "moveChart", "sheet": 0, "index": 0, "from": [5, 1], "to": [12, 6] }),
        );
        let list = run(&mut again, json!({ "cmd": "charts", "sheet": 0 }));
        assert_eq!(
            (list[0]["from"].clone(), list[0]["to"].clone()),
            (json!([5, 1]), json!([12, 6]))
        );
        let mut moved = reopen(&mut again);
        let list = run(&mut moved, json!({ "cmd": "charts", "sheet": 0 }));
        assert_eq!(list[0]["to"], json!([12, 6]));
        run(
            &mut moved,
            json!({ "cmd": "deleteChart", "sheet": 0, "index": 0 }),
        );
        let mut gone = reopen(&mut moved);
        assert_eq!(gone.workbook().unwrap().sheets[0].charts.len(), 0);
        assert!(gone
            .exec(SheetCommand::DeleteChart { sheet: 0, index: 0 })
            .is_err());
    }

    #[test]
    fn find_replace_goto_and_paste_special() {
        let mut e = engine();
        e.set_input(0, 0, 0, "Hà Nội").unwrap();
        e.set_input(0, 1, 0, "hà nam").unwrap();
        e.set_input(0, 0, 1, "5").unwrap();
        e.set_input(0, 1, 1, "=B1*2").unwrap();
        let m = run(&mut e, json!({ "cmd": "find", "query": "hà" }));
        assert_eq!(m["matches"].as_array().unwrap().len(), 2);
        let m = run(
            &mut e,
            json!({ "cmd": "find", "query": "hà", "matchCase": true }),
        );
        assert_eq!(m["matches"].as_array().unwrap().len(), 1);
        let m = run(
            &mut e,
            json!({ "cmd": "find", "query": "B1", "inFormulas": true }),
        );
        assert_eq!(m["matches"][0], json!({ "sheet": 0, "row": 1, "col": 1 }));
        let m = run(
            &mut e,
            json!({ "cmd": "find", "query": "10", "wholeCell": true }),
        );
        assert_eq!(
            m["matches"].as_array().unwrap().len(),
            1,
            "values of formulas"
        );
        let r = run(
            &mut e,
            json!({ "cmd": "replace", "query": "hà", "with": "Thành phố" }),
        );
        assert_eq!(r["count"], 2);
        assert_eq!(e.cell(0, 0, 0).unwrap().input, "Thành phố Nội");
        assert!(e.undo().unwrap());
        assert_eq!(e.cell(0, 0, 0).unwrap().input, "Hà Nội");
        let g = run(
            &mut e,
            json!({ "cmd": "resolve", "sheet": 0, "text": "Sheet1!B2:C3" }),
        );
        assert_eq!(g["range"], rng(1, 1, 2, 2));
        // Copy B1:B2, paste at D5: relative references follow.
        let t = run(
            &mut e,
            json!({ "cmd": "copy", "sheet": 0, "range": rng(0, 1, 1, 1) }),
        );
        assert_eq!(t["text"], "5\r\n10");
        run(
            &mut e,
            json!({ "cmd": "paste", "sheet": 0, "row": 4, "col": 3 }),
        );
        assert_eq!(e.cell(0, 5, 3).unwrap().input, "=D5*2");
        assert_eq!(e.cell(0, 5, 3).unwrap().display, "10");
        run(
            &mut e,
            json!({ "cmd": "paste", "sheet": 0, "row": 8, "col": 0, "mode": "values", "transpose": true }),
        );
        assert_eq!(e.cell(0, 8, 1).unwrap().input, "10");
        assert_eq!(e.cell(0, 8, 0).unwrap().input, "5");
        run(
            &mut e,
            json!({ "cmd": "format", "sheet": 0, "range": rng(0, 1, 0, 1), "patch": { "bold": true } }),
        );
        run(
            &mut e,
            json!({ "cmd": "copy", "sheet": 0, "range": rng(0, 1, 0, 1) }),
        );
        run(
            &mut e,
            json!({ "cmd": "paste", "sheet": 0, "row": 0, "col": 5, "mode": "formats" }),
        );
        assert_eq!(
            run(
                &mut e,
                json!({ "cmd": "formatState", "sheet": 0, "row": 0, "col": 5 })
            )["bold"],
            true
        );
        assert_eq!(e.cell(0, 0, 5).unwrap().input, "");
        // Cut moves.
        run(
            &mut e,
            json!({ "cmd": "copy", "sheet": 0, "range": rng(0, 0, 0, 0), "cut": true }),
        );
        run(
            &mut e,
            json!({ "cmd": "paste", "sheet": 0, "row": 20, "col": 0 }),
        );
        assert_eq!(e.cell(0, 20, 0).unwrap().input, "Hà Nội");
        assert_eq!(e.cell(0, 0, 0).unwrap().input, "");
        assert!(run(
            &mut e,
            json!({ "cmd": "paste", "sheet": 0, "row": 0, "col": 0 })
        )
        .is_null());
    }

    #[test]
    fn page_setup_drives_pdf_pages() {
        let mut e = engine();
        e.set_input(0, 0, 0, "Tiêu đề").unwrap();
        for r in 1..120 {
            e.set_input(0, r, 0, &r.to_string()).unwrap();
            e.set_input(0, r, 12, "x").unwrap();
        }
        let portrait = e.print_pages(0).unwrap();
        assert!(portrait.len() >= 4, "{} pages", portrait.len());
        run(
            &mut e,
            json!({ "cmd": "setPageSetup", "sheet": 0, "orientation": "landscape",
            "titleRows": [0, 0], "margins": [0.5, 0.5, 0.5, 0.5] }),
        );
        let pages = e.print_pages(0).unwrap();
        assert_eq!((pages[0].width, pages[0].height), (842.0, 595.0));
        // The title row is repeated on every page.
        let has_title = |p: &bo_render::PageList| {
            p.commands.iter().any(
                |c| matches!(c, bo_render::Command::Glyphs { text, .. } if text.contains("Tiêu")),
            )
        };
        assert!(pages.iter().all(has_title), "titles on every page");
        run(
            &mut e,
            json!({ "cmd": "setPageSetup", "sheet": 0, "fit": [1, 1] }),
        );
        assert_eq!(e.print_pages(0).unwrap().len(), 1, "fit to one page");
        run(
            &mut e,
            json!({ "cmd": "setPageSetup", "sheet": 0, "fit": null, "printArea": rng(0, 0, 9, 0) }),
        );
        assert_eq!(e.print_pages(0).unwrap().len(), 1);
        let ps = run(&mut e, json!({ "cmd": "pageSetup", "sheet": 0 }));
        assert_eq!(ps["orientation"], "landscape");
        assert_eq!(ps["titleRows"], json!([0, 0]));
        assert_eq!(ps["printArea"], "Sheet1!$A$1:$A$10");
        let back = reopen(&mut e);
        assert_eq!(
            back.workbook().unwrap().sheets[0].print_titles.as_deref(),
            Some("Sheet1!$1:$1")
        );
        assert!(e.export_pdf().unwrap().starts_with(b"%PDF"));
    }

    /// 100 000 formulas; editing one input recalculates only its dependents.
    #[test]
    fn live_recalc_touches_only_dependents() {
        let mut e = engine();
        let mut cells = Vec::new();
        for r in 0..20_000u32 {
            cells.push((r, 0, r.to_string()));
            for c in 1..6u32 {
                cells.push((r, c, format!("=A{}*{c}+1", r + 1)));
            }
        }
        e.set_inputs(0, &cells).unwrap();
        e.set_input(0, 0, 7, "=SUM(B1:B20000)").unwrap();
        let t = std::time::Instant::now();
        e.set_input(0, 100, 0, "1000").unwrap();
        let took = t.elapsed();
        eprintln!("edit one input of a 100k-formula sheet: {took:?}");
        assert_eq!(e.live.last_evaluations(), 6, "5 row formulas + the SUM");
        assert_eq!(e.cell(0, 100, 1).unwrap().display, "1001");
        // Target (BlueOffice-PROMPT §5): < 100 ms, met even by debug builds.
        assert!(took.as_millis() < 100, "{took:?}");
    }

    #[test]
    fn pivot_create_update_save_reopen_refresh() {
        let mut e = engine();
        let data = [
            ("Vùng", "Sản phẩm", "Doanh thu"),
            ("Bắc", "Gạo", "10"),
            ("Nam", "Gạo", "5"),
            ("Bắc", "Chè", "7"),
            ("Nam", "Chè", "1"),
        ];
        let mut cells = Vec::new();
        for (r, (a, b, c)) in data.iter().enumerate() {
            let r = r as u32;
            cells.push((r, 0, (*a).to_owned()));
            cells.push((r, 1, (*b).to_owned()));
            cells.push((r, 2, (*c).to_owned()));
        }
        e.set_inputs(0, &cells).unwrap();
        let src = run(
            &mut e,
            json!({ "cmd": "pivotSource", "sheet": 0, "range": rng(1, 1, 1, 1) }),
        );
        assert_eq!(src["fields"], json!(["Vùng", "Sản phẩm", "Doanh thu"]));
        assert_eq!(src["range"], rng(0, 0, 4, 2));
        let labels = json!({ "grandTotal": "Tổng cộng", "values": "Giá trị", "name": "BTH" });
        let made = run(
            &mut e,
            json!({ "cmd": "pivotCreate", "sheet": 0, "range": rng(0, 0, 0, 0),
                "rows": [0], "values": [{ "field": 2, "function": "sum", "name": "Tổng Doanh thu" }],
                "labels": labels }),
        );
        let ps = made["sheet"].as_u64().unwrap() as usize;
        assert_eq!(ps, 1, "new sheet");
        assert_eq!(e.info().unwrap().sheets[1].pivots, 1);
        assert_eq!(e.cell(1, 2, 1).unwrap().display, "Tổng Doanh thu");
        assert_eq!(e.cell(1, 3, 0).unwrap().display, "Bắc");
        assert_eq!(e.cell(1, 3, 1).unwrap().display, "17");
        assert_eq!(e.cell(1, 5, 0).unwrap().display, "Tổng cộng");
        assert_eq!(e.cell(1, 5, 1).unwrap().display, "23");
        // Change the aggregation to Average and add a column field.
        run(
            &mut e,
            json!({ "cmd": "pivotUpdate", "sheet": 1, "index": 0, "rows": [0], "cols": [1],
                "values": [{ "field": 2, "function": "average" }], "labels": labels }),
        );
        let info = run(&mut e, json!({ "cmd": "pivots", "sheet": 1 }));
        assert_eq!(info[0]["values"][0]["function"], "average");
        assert_eq!(info[0]["values"][0]["name"], "Average of Doanh thu");
        assert_eq!(info[0]["fields"][1]["items"], json!(["Chè", "Gạo"]));
        assert_eq!(info[0]["source"], "Sheet1!A1:C5");
        // Header rows: caption, column items; Bắc row: Chè 7, Gạo 10, total 8.5.
        assert_eq!(e.cell(1, 3, 1).unwrap().display, "Chè");
        assert_eq!(e.cell(1, 4, 0).unwrap().display, "Bắc");
        assert_eq!(e.cell(1, 4, 3).unwrap().display, "8,5");
        // Undo restores the Sum layout.
        assert!(e.undo().unwrap());
        assert_eq!(e.cell(1, 3, 1).unwrap().display, "17");
        assert!(e.redo().unwrap());
        // Save, reopen: the pivot is read back from its parts.
        let mut back = reopen(&mut e);
        let info = run(&mut back, json!({ "cmd": "pivots", "sheet": 1 }));
        assert_eq!(info[0]["cols"], json!([1]));
        assert_eq!(info[0]["values"][0]["function"], "average");
        assert_eq!(back.cell(1, 4, 3).unwrap().display, "8,5");
        // Edit the source, refresh the reopened pivot (records reloaded from the source).
        back.set_input(0, 1, 2, "30").unwrap();
        run(
            &mut back,
            json!({ "cmd": "pivotRefresh", "sheet": 1, "index": 0, "labels": labels }),
        );
        assert_eq!(back.cell(1, 4, 3).unwrap().display, "18,5");
        // Filter to Gạo only through the side panel's item indices.
        run(
            &mut back,
            json!({ "cmd": "pivotUpdate", "sheet": 1, "index": 0, "rows": [0],
                "values": [{ "field": 2, "function": "max" }],
                "filters": [{ "field": 1, "items": [1] }], "labels": labels }),
        );
        let info = run(&mut back, json!({ "cmd": "pivots", "sheet": 1 }));
        assert_eq!(info[0]["filters"], json!([{ "field": 1, "items": [1] }]));
        assert_eq!(back.cell(1, 0, 1).unwrap().display, "Gạo");
        assert_eq!(back.cell(1, 2, 1).unwrap().display, "Max of Doanh thu");
        assert_eq!(back.cell(1, 3, 1).unwrap().display, "30");
        let again = reopen(&mut back);
        assert_eq!(again.cell(1, 3, 1).unwrap().display, "30");
        let info = run(&mut back, json!({ "cmd": "pivots", "sheet": 1 }));
        assert_eq!(info[0]["filters"][0]["items"], json!([1]));
    }

    // source: ECMA-376 Part 1, §21.2.2.142 pivotSource, §18.10.1.8 calculatedItem,
    // §18.10.1.26 discretePr, §18.18.60 date filters, §18.10.1.73 dataOnRows
    #[test]
    fn pivot_charts_items_groups_date_filters() {
        let mut e = engine();
        // Dates typed as text in vi-VN become serial dates.
        let data = [
            ("Ngày", "Vùng", "Doanh thu"),
            ("15/01/2024", "Bắc", "10"),
            ("20/01/2024", "Nam", "30"),
            ("10/02/2024", "Bắc", "20"),
            ("05/03/2024", "Nam", "40"),
        ];
        let mut cells = Vec::new();
        for (r, (a, b, c)) in data.iter().enumerate() {
            let r = r as u32;
            cells.push((r, 0, (*a).to_owned()));
            cells.push((r, 1, (*b).to_owned()));
            cells.push((r, 2, (*c).to_owned()));
        }
        e.set_inputs(0, &cells).unwrap();
        let labels = json!({ "grandTotal": "Tổng cộng", "group": "Nhóm {n}" });
        run(
            &mut e,
            json!({ "cmd": "pivotCreate", "sheet": 0, "range": rng(0, 0, 0, 0),
                "rows": [1], "values": [{ "field": 2 }], "labels": labels }),
        );
        let d = |e: &SheetEngine, r: u32, c: u32| e.cell(1, r, c).unwrap().display;
        assert_eq!((d(&e, 3, 0), d(&e, 3, 1)), ("Bắc".into(), "30".into()));
        // Pivot chart from the pivot table.
        let c = run(
            &mut e,
            json!({ "cmd": "pivotChart", "sheet": 1, "index": 0, "kind": "column" }),
        );
        assert_eq!(c["index"], 0);
        let charts = run(&mut e, json!({ "cmd": "charts", "sheet": 1 }));
        assert_eq!(charts[0]["pivot"], "Sheet2!Pivot1");
        let cats = |e: &SheetEngine| {
            e.workbook().unwrap().sheets[1].charts[0].plots[0].series[0]
                .categories
                .clone()
        };
        assert_eq!(cats(&e), vec!["Bắc", "Nam"]);
        // A calculated item; the chart follows.
        run(
            &mut e,
            json!({ "cmd": "pivotCalcItem", "sheet": 1, "index": 0, "field": 1,
                "name": "Cả hai", "formula": "Bắc+Nam", "labels": labels }),
        );
        assert_eq!((d(&e, 5, 0), d(&e, 5, 1)), ("Cả hai".into(), "100".into()));
        assert_eq!(cats(&e), vec!["Bắc", "Nam", "Cả hai"]);
        let info = run(&mut e, json!({ "cmd": "pivots", "sheet": 1 }));
        assert_eq!(info[0]["calcItems"][0]["formula"], "Bắc+Nam");
        // Grouping needs the calculated item gone.
        let err = e.exec_json(
            &json!({ "cmd": "pivotGroupItems", "sheet": 1, "index": 0, "field": 1, "items": [0, 1] })
                .to_string(),
        );
        assert!(err.is_err());
        run(
            &mut e,
            json!({ "cmd": "pivotCalcItem", "sheet": 1, "index": 0, "field": 1,
                "old": "Cả hai", "remove": true, "labels": labels }),
        );
        let g = run(
            &mut e,
            json!({ "cmd": "pivotGroupItems", "sheet": 1, "index": 0, "field": 1,
                "items": [0, 1], "labels": labels }),
        );
        assert_eq!(g["field"], 3);
        assert_eq!(d(&e, 3, 0), "Nhóm 1");
        run(
            &mut e,
            json!({ "cmd": "pivotGroupItems", "sheet": 1, "index": 0, "field": 3,
                "items": [0], "ungroup": true, "labels": labels }),
        );
        assert_eq!(d(&e, 3, 0), "Bắc");
        // Date filter "this month" with the clock on 2024-02-15 (serial 45337).
        e.set_now_ms((45_337 - 25_569) * 86_400_000);
        run(
            &mut e,
            json!({ "cmd": "pivotUpdate", "sheet": 1, "index": 0, "rows": [0],
                "values": [{ "field": 2 }], "labels": labels,
                "fieldFilters": [{ "field": 0, "kind": "thisMonth" }] }),
        );
        assert_eq!(d(&e, 3, 0), "10/02/2024");
        assert_eq!(
            (d(&e, 4, 0), d(&e, 4, 1)),
            ("Tổng cộng".into(), "20".into())
        );
        run(
            &mut e,
            json!({ "cmd": "pivotUpdate", "sheet": 1, "index": 0, "rows": [0],
                "values": [{ "field": 2 }], "labels": labels,
                "fieldFilters": [{ "field": 0, "kind": "dateBetween", "value1": "2024-01-01", "value2": "2024-01-31" }] }),
        );
        assert_eq!(
            (d(&e, 5, 0), d(&e, 5, 1)),
            ("Tổng cộng".into(), "40".into())
        );
        // Two value fields down the rows.
        run(
            &mut e,
            json!({ "cmd": "pivotUpdate", "sheet": 1, "index": 0, "rows": [1],
                "values": [{ "field": 2 }, { "field": 2, "function": "count" }],
                "dataOnRows": true, "fieldFilters": [], "labels": labels }),
        );
        let info = run(&mut e, json!({ "cmd": "pivots", "sheet": 1 }));
        assert_eq!(info[0]["dataOnRows"], true);
        assert_eq!(
            (d(&e, 3, 1), d(&e, 4, 1)),
            ("Sum of Doanh thu".into(), "Count of Doanh thu".into())
        );
        assert_eq!((d(&e, 3, 2), d(&e, 4, 2)), ("30".into(), "2".into()));
        // Save and reopen: the chart is still a pivot chart of the table.
        let back = reopen(&mut e);
        let charts = back.workbook().unwrap().sheets[1].charts.clone();
        assert_eq!(charts[0].pivot_source.as_deref(), Some("Sheet2!Pivot1"));
        assert!(back.workbook().unwrap().sheets[1].pivots[0].data_on_rows);
    }

    // source: ECMA-376 Part 1, §20.5.2.1 absoluteAnchor, §20.5.2.24 oneCellAnchor
    #[test]
    fn absolute_and_one_cell_charts_move_and_resize() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/sheet");
        for (file, kind) in [
            ("chart-anchors-1.xlsx", "oneCell"),
            ("chart-anchors-2.xlsx", "absolute"),
        ] {
            let Ok(bytes) = std::fs::read(dir.join(file)) else {
                continue;
            };
            let mut e = SheetEngine::with_bundled_fonts();
            e.open(&bytes, file).unwrap();
            let list = run(&mut e, json!({ "cmd": "charts", "sheet": 0 }));
            assert_eq!(list[0]["anchor"], kind);
            run(
                &mut e,
                json!({ "cmd": "moveChart", "sheet": 0, "index": 0, "from": [6, 2], "to": [16, 9] }),
            );
            let mut back = reopen(&mut e);
            let list = run(&mut back, json!({ "cmd": "charts", "sheet": 0 }));
            assert_eq!(list[0]["anchor"], kind, "{file}");
            assert_eq!(list[0]["from"], json!([6, 2]), "{file}");
            assert_eq!(list[0]["to"], json!([16, 9]), "{file}");
            let c = &back.workbook().unwrap().sheets[0].charts[0];
            // Default column width 48 pt (8.43 chars) and row height 15 pt → EMU.
            let (cx, cy) = c.ext.unwrap();
            assert!(cx > 0 && cy == 10 * 15 * 12_700, "{file}: {cx} {cy}");
            if kind == "absolute" {
                assert_eq!(c.pos.unwrap().1, 6 * 15 * 12_700);
            }
        }
    }

    #[test]
    fn pivot_grouping_layout_show_as_and_management() {
        let mut e = engine();
        let data = [
            ("Ngày", "Vùng", "Doanh thu"),
            ("=DATE(2024,1,15)", "Bắc", "10"),
            ("=DATE(2024,1,20)", "Nam", "30"),
            ("=DATE(2024,2,10)", "Bắc", "20"),
            ("=DATE(2024,3,5)", "Nam", "40"),
        ];
        let mut cells = Vec::new();
        for (r, (a, b, c)) in data.iter().enumerate() {
            let r = r as u32;
            cells.push((r, 0, (*a).to_owned()));
            cells.push((r, 1, (*b).to_owned()));
            cells.push((r, 2, (*c).to_owned()));
        }
        e.set_inputs(0, &cells).unwrap();
        let months: Vec<String> = (1..=12).map(|m| format!("Thg {m}")).collect();
        let labels =
            json!({ "grandTotal": "Tổng cộng", "months": months, "subtotal": "Tổng {item}" });
        run(
            &mut e,
            json!({ "cmd": "pivotCreate", "sheet": 0, "range": rng(0, 0, 0, 0),
                "rows": [0], "values": [{ "field": 2 }], "labels": labels }),
        );
        // Group the dates by month: Jan 40, Feb 20, Mar 40.
        run(
            &mut e,
            json!({ "cmd": "pivotGroup", "sheet": 1, "index": 0, "field": 0, "by": "dates",
                "months": true, "labels": labels }),
        );
        let d = |e: &SheetEngine, r: u32, c: u32| e.cell(1, r, c).unwrap().display;
        assert_eq!((d(&e, 3, 0), d(&e, 3, 1)), ("Thg 1".into(), "40".into()));
        assert_eq!((d(&e, 5, 0), d(&e, 6, 1)), ("Thg 3".into(), "100".into()));
        // Outline layout, values as % of the grand total.
        run(
            &mut e,
            json!({ "cmd": "pivotUpdate", "sheet": 1, "index": 0, "rows": [0],
                "values": [{ "field": 2, "showAs": "percentOfTotal" }],
                "layout": "outline", "labels": labels }),
        );
        assert_eq!(d(&e, 3, 1), "40,00%");
        assert_eq!(d(&e, 4, 1), "20,00%");
        let info = run(&mut e, json!({ "cmd": "pivots", "sheet": 1 }));
        assert_eq!(info[0]["layout"], "outline");
        assert_eq!(info[0]["values"][0]["showAs"], "percentOfTotal");
        assert_eq!(info[0]["fields"][0]["group"]["by"], "dates");
        assert_eq!(info[0]["fields"][0]["group"]["months"], true);
        assert_eq!(info[0]["fields"][0]["items"][1], "Thg 1");
        // Save and reopen.
        let mut e = reopen(&mut e);
        assert_eq!(d(&e, 3, 1), "40,00%");
        let info = run(&mut e, json!({ "cmd": "pivots", "sheet": 1 }));
        assert_eq!(info[0]["layout"], "outline");
        assert_eq!(info[0]["fields"][0]["group"]["months"], true);
        // A calculated field (10 % of the revenue): Jan 4.
        let f = run(
            &mut e,
            json!({ "cmd": "pivotCalcField", "sheet": 1, "index": 0, "name": "Thuế",
                "formula": "='Doanh thu'*0.1", "labels": labels }),
        );
        assert_eq!(f["field"], 3);
        assert_eq!(d(&e, 2, 2), "Sum of Thuế");
        assert_eq!(d(&e, 3, 2), "4");
        // A failing edit changes nothing.
        let err = e.exec_json(
            &json!({ "cmd": "pivotCalcField", "sheet": 1, "index": 0, "name": "X",
                "formula": "Giá*2" })
            .to_string(),
        );
        assert!(err.is_err());
        assert_eq!(d(&e, 3, 2), "4");
        // Field settings: number format, sort descending by value.
        run(
            &mut e,
            json!({ "cmd": "pivotUpdate", "sheet": 1, "index": 0, "rows": [0],
                "values": [{ "field": 2, "numFmt": "#,##0.00" }],
                "fieldOptions": [{ "field": 0, "sort": "descending", "sortBy": 0 }],
                "rowGrandTotals": false, "colGrandTotals": false,
                "style": { "name": "PivotStyleMedium2", "rowStripes": true }, "labels": labels }),
        );
        assert_eq!((d(&e, 3, 0), d(&e, 3, 1)), ("Thg 1".into(), "40,00".into()));
        assert_eq!((d(&e, 5, 0), d(&e, 5, 1)), ("Thg 2".into(), "20,00".into()));
        assert_eq!(d(&e, 6, 0), "", "no grand total row");
        let info = run(&mut e, json!({ "cmd": "pivots", "sheet": 1 }));
        assert_eq!(info[0]["values"][0]["numFmt"], "#,##0.00");
        assert_eq!(info[0]["fieldOptions"][0]["sort"], "descending");
        assert_eq!(info[0]["style"]["name"], "PivotStyleMedium2");
        assert_eq!(info[0]["colGrandTotals"], false);
        // Move to F11, change the source to January only, refresh all, delete.
        let moved = run(
            &mut e,
            json!({ "cmd": "pivotMove", "sheet": 1, "index": 0,
                "dest": { "sheet": 1, "row": 10, "col": 5 }, "labels": labels }),
        );
        assert_eq!(moved["index"], 0);
        assert_eq!(d(&e, 3, 1), "");
        assert_eq!(
            (d(&e, 10, 5), d(&e, 11, 6)),
            ("Ngày".into(), "40,00".into())
        );
        run(
            &mut e,
            json!({ "cmd": "pivotChangeSource", "sheet": 1, "index": 0,
                "source": "Sheet1!A1:C3", "labels": labels }),
        );
        assert_eq!(d(&e, 12, 5), "", "only January is left");
        e.set_input(0, 1, 2, "15").unwrap();
        let n = run(
            &mut e,
            json!({ "cmd": "pivotRefreshAll", "labels": labels }),
        );
        assert_eq!(n["count"], 1);
        assert_eq!(d(&e, 11, 6), "45,00");
        run(
            &mut e,
            json!({ "cmd": "pivotDelete", "sheet": 1, "index": 0 }),
        );
        assert_eq!(e.info().unwrap().sheets[1].pivots, 0);
        assert_eq!(d(&e, 11, 6), "");
        assert!(e.undo().unwrap());
        assert_eq!(e.info().unwrap().sheets[1].pivots, 1);
        let presets = run(&mut e, json!({ "cmd": "presets" }));
        assert!(presets["pivotStyles"]
            .as_array()
            .unwrap()
            .contains(&json!("PivotStyleLight16")));
    }

    #[test]
    fn cut_paste_moves_references_and_paste_fills_selection() {
        let mut e = engine();
        e.set_input(0, 0, 0, "5").unwrap(); // A1
        e.set_input(0, 1, 0, "7").unwrap(); // A2
        e.set_input(0, 0, 2, "=A1*2").unwrap(); // C1
        e.set_input(0, 1, 2, "=SUM(A1:A2)").unwrap(); // C2
        e.set_input(0, 2, 2, "=$A$1+1").unwrap(); // C3
        e.add_sheet(Some("Khác")).unwrap();
        e.set_input(1, 0, 0, "=Sheet1!A2").unwrap();
        // Cut A1:A2 → E5:E6.
        run(
            &mut e,
            json!({ "cmd": "copy", "sheet": 0, "range": rng(0, 0, 1, 0), "cut": true }),
        );
        run(
            &mut e,
            json!({ "cmd": "paste", "sheet": 0, "row": 4, "col": 4 }),
        );
        assert_eq!(e.cell(0, 0, 2).unwrap().input, "=E5*2");
        assert_eq!(e.cell(0, 1, 2).unwrap().input, "=SUM(E5:E6)");
        assert_eq!(e.cell(0, 2, 2).unwrap().input, "=$E$5+1");
        assert_eq!(e.cell(1, 0, 0).unwrap().input, "=Sheet1!E6");
        assert_eq!(e.cell(0, 1, 2).unwrap().display, "12");
        // Copy one cell, paste into a 2×3 selection: repeated (relative refs shift).
        e.set_input(0, 9, 0, "1").unwrap();
        e.set_input(0, 9, 1, "=A10+1").unwrap();
        run(
            &mut e,
            json!({ "cmd": "copy", "sheet": 0, "range": rng(9, 0, 9, 1) }),
        );
        let r = run(
            &mut e,
            json!({ "cmd": "paste", "sheet": 0, "row": 11, "col": 0, "range": rng(11, 0, 12, 3) }),
        );
        assert_eq!(r["range"], rng(11, 0, 12, 3));
        assert_eq!(e.cell(0, 12, 2).unwrap().input, "1");
        assert_eq!(e.cell(0, 12, 3).unwrap().input, "=C13+1");
        // Not a multiple: pasted once.
        let r = run(
            &mut e,
            json!({ "cmd": "paste", "sheet": 0, "row": 20, "col": 0, "range": rng(20, 0, 20, 2) }),
        );
        assert_eq!(r["range"], rng(20, 0, 20, 1));
    }

    #[test]
    fn sort_left_to_right_and_filters_by_date_and_colour() {
        let mut e = engine();
        e.set_now_ms(1_736_935_200_000); // 2025-01-15 10:00 UTC
        for (c, v) in ["Mã", "30", "10", "20"].iter().enumerate() {
            e.set_input(0, 0, c as u32, v).unwrap();
        }
        run(
            &mut e,
            json!({ "cmd": "sort", "sheet": 0, "range": rng(0, 0, 0, 3), "hasHeader": true,
                "leftToRight": true, "keys": [{ "col": 0 }] }),
        );
        assert_eq!(e.cell(0, 0, 1).unwrap().input, "10");
        assert_eq!(e.cell(0, 0, 3).unwrap().input, "30");
        // Dates: 2024-03-05, 2025-01-02, 2025-01-14 in B2:B4 with a fill on B3.
        e.set_input(0, 1, 5, "Ngày").unwrap();
        for (r, d) in ["45356", "45659", "45671"].iter().enumerate() {
            e.set_input(0, r as u32 + 2, 5, d).unwrap();
        }
        run(
            &mut e,
            json!({ "cmd": "format", "sheet": 0, "range": rng(2, 5, 4, 5), "patch": { "numFmt": "dd/mm/yyyy" } }),
        );
        run(
            &mut e,
            json!({ "cmd": "format", "sheet": 0, "range": rng(3, 5, 3, 5), "patch": { "fillColor": "FFEE00" } }),
        );
        run(
            &mut e,
            json!({ "cmd": "toggleFilter", "sheet": 0, "range": rng(1, 5, 4, 5) }),
        );
        let fv = run(
            &mut e,
            json!({ "cmd": "filterValues", "sheet": 0, "col": 5 }),
        );
        assert_eq!(fv["dates"][0]["year"], 2024);
        assert_eq!(fv["dates"][1]["months"][0]["month"], 1);
        assert_eq!(fv["colors"]["cell"], json!(["FFEE00"]));
        let hidden = |e: &mut SheetEngine| e.info().unwrap().sheets[0].hidden_rows.clone();
        run(
            &mut e,
            json!({ "cmd": "setFilter", "sheet": 0, "col": 5, "filter": { "dates": [{ "year": 2025 }] } }),
        );
        assert_eq!(hidden(&mut e), vec![(2, 2)]);
        run(
            &mut e,
            json!({ "cmd": "setFilter", "sheet": 0, "col": 5, "filter": { "dynamic": "thisWeek" } }),
        );
        assert_eq!(hidden(&mut e), vec![(2, 3)]);
        run(
            &mut e,
            json!({ "cmd": "setFilter", "sheet": 0, "col": 5, "filter": { "cellColor": "FFEE00" } }),
        );
        assert_eq!(hidden(&mut e), vec![(2, 2), (4, 4)]);
        let fv = run(
            &mut e,
            json!({ "cmd": "filterValues", "sheet": 0, "col": 5 }),
        );
        assert_eq!(fv["kind"], "color");
        // The criterion survives a save.
        let back = reopen(&mut e);
        assert!(matches!(
            back.workbook().unwrap().sheets[0]
                .auto_filter
                .as_ref()
                .unwrap()
                .columns[0]
                .filter,
            ColumnFilter::Color { .. }
        ));
    }
}
