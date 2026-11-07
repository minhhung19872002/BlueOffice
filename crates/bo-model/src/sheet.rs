//! Spreadsheet (workbook) model.
//!
//! Mirrors ECMA-376 Part 1, §18 SpreadsheetML closely enough that a
//! read → write round trip of `.xlsx` is lossless for everything the model
//! understands, while anything it does not understand is carried as opaque
//! data ([`RawXml`] fragments anchored at their original position, opaque
//! relationships and opaque package parts — see [`Opaque`]).
//!
//! Conventions:
//! * Row and column indexes are **zero-based** in the model (`A1` = `(0, 0)`).
//!   The XML uses one-based indexes; conversion happens in `bo-xlsx`.
//! * Sizes keep the file units: column widths in "characters" (§18.3.1.13),
//!   row heights and margins in points / inches as in the spec.
//! * Formulas are stored as text (without the leading `=`); evaluation lives
//!   in `bo-formula`. Cached values are stored in [`Cell::value`].
// File-format records mirror the many boolean attributes of the spec.
#![allow(clippy::struct_excessive_bools)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Maximum number of rows in a worksheet (ECMA-376 Part 1, §18.3.1.73: `r` ≤ 1048576).
pub const MAX_ROWS: u32 = 1_048_576;
/// Maximum number of columns in a worksheet (16384 = `XFD`).
pub const MAX_COLS: u32 = 16_384;

// ---------------------------------------------------------------------------
// A1 references
// ---------------------------------------------------------------------------

/// Convert a zero-based column index to letters (`0` → `A`, `26` → `AA`).
pub fn col_to_letters(col: u32) -> String {
    let mut n = col + 1;
    let mut buf = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        buf.push(b'A' + rem as u8);
        n = (n - 1) / 26;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap_or_default()
}

/// Convert column letters (case-insensitive) to a zero-based index. `None` for junk
/// or columns beyond [`MAX_COLS`].
pub fn letters_to_col(s: &str) -> Option<u32> {
    if s.is_empty() || s.len() > 3 {
        return None;
    }
    let mut n: u32 = 0;
    for b in s.bytes() {
        if !b.is_ascii_alphabetic() {
            return None;
        }
        n = n * 26 + u32::from(b.to_ascii_uppercase() - b'A' + 1);
    }
    (1..=MAX_COLS).contains(&n).then(|| n - 1)
}

/// A cell reference (zero-based), e.g. `B3` = `CellRef { row: 2, col: 1 }`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct CellRef {
    pub row: u32,
    pub col: u32,
}

impl CellRef {
    /// Construct a reference.
    pub const fn new(row: u32, col: u32) -> Self {
        CellRef { row, col }
    }

    /// Parse an A1 reference (`$` markers are accepted and ignored).
    /// ECMA-376 Part 1, §18.18.62 `ST_Ref` / §18.18.7 `ST_CellRef`.
    pub fn parse(s: &str) -> Option<CellRef> {
        let s = s.trim();
        let s: String = s.chars().filter(|c| *c != '$').collect();
        let split = s.find(|c: char| c.is_ascii_digit())?;
        let (letters, digits) = s.split_at(split);
        let col = letters_to_col(letters)?;
        if !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let row: u32 = digits.parse().ok()?;
        (1..=MAX_ROWS)
            .contains(&row)
            .then(|| CellRef { row: row - 1, col })
    }
}

impl fmt::Display for CellRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", col_to_letters(self.col), self.row + 1)
    }
}

/// A rectangular range (inclusive, zero-based), e.g. `A1:C3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CellRange {
    pub first: CellRef,
    pub last: CellRef,
}

impl CellRange {
    /// Construct a normalized range (first ≤ last on both axes).
    pub fn new(a: CellRef, b: CellRef) -> Self {
        CellRange {
            first: CellRef::new(a.row.min(b.row), a.col.min(b.col)),
            last: CellRef::new(a.row.max(b.row), a.col.max(b.col)),
        }
    }

    /// Single-cell range.
    pub fn single(c: CellRef) -> Self {
        CellRange { first: c, last: c }
    }

    /// Parse `A1:B2` or `A1` (ECMA-376 Part 1, §18.18.62 `ST_Ref`).
    pub fn parse(s: &str) -> Option<CellRange> {
        match s.split_once(':') {
            Some((a, b)) => Some(CellRange::new(CellRef::parse(a)?, CellRef::parse(b)?)),
            None => CellRef::parse(s).map(CellRange::single),
        }
    }

    /// Parse a space-separated list of ranges (`ST_Sqref`, §18.18.76).
    pub fn parse_sqref(s: &str) -> Vec<CellRange> {
        s.split_whitespace().filter_map(CellRange::parse).collect()
    }

    /// Does the range contain `c`?
    pub fn contains(&self, c: CellRef) -> bool {
        (self.first.row..=self.last.row).contains(&c.row)
            && (self.first.col..=self.last.col).contains(&c.col)
    }

    /// Number of rows.
    pub fn rows(&self) -> u32 {
        self.last.row - self.first.row + 1
    }

    /// Number of columns.
    pub fn cols(&self) -> u32 {
        self.last.col - self.first.col + 1
    }
}

impl fmt::Display for CellRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.first == self.last {
            write!(f, "{}", self.first)
        } else {
            write!(f, "{}:{}", self.first, self.last)
        }
    }
}

/// Format a list of ranges as `ST_Sqref`.
pub fn sqref_to_string(ranges: &[CellRange]) -> String {
    ranges
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Opaque (unknown) content
// ---------------------------------------------------------------------------

/// An XML fragment the model does not understand, kept verbatim and re-emitted
/// at schema slot `after`: after the known element of that slot (if any) and before
/// the next slot. Elements that are part of the parent's schema sequence anchor at their
/// own name; extension elements (`mc:AlternateContent`, …) at the slot of the previous
/// sibling. Empty = at the start of the parent.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RawXml {
    /// Schema slot (local element name) after which the fragment is written (or `""`).
    pub after: String,
    /// The serialized element(s), including any namespace prefixes.
    pub xml: String,
}

/// A relationship the model does not own (drawing, printer settings, pivot …).
/// The target is stored as an absolute part name for internal relationships.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OpaqueRel {
    pub id: String,
    pub rel_type: String,
    pub target: String,
    pub external: bool,
}

/// A package part kept byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OpaquePart {
    /// Absolute part name, e.g. `/xl/theme/theme1.xml`.
    pub name: String,
    /// Content type override from `[Content_Types].xml`, if any.
    pub content_type: Option<String>,
    pub data: Vec<u8>,
}

/// Package-level content preserved for lossless writing.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Opaque {
    /// Parts the writer does not regenerate (theme, docProps, drawings, charts, pivots, VML…).
    pub parts: Vec<OpaquePart>,
    /// `Default` content types (extension, type) from the source package.
    pub content_type_defaults: Vec<(String, String)>,
    /// Package relationships other than the main workbook (`/_rels/.rels`).
    pub package_rels: Vec<OpaqueRel>,
    /// Workbook relationships other than sheets/styles/sharedStrings/calcChain.
    pub workbook_rels: Vec<OpaqueRel>,
    /// Content type of the workbook part (e.g. macro-enabled), when not the default.
    pub workbook_content_type: Option<String>,
}

/// An attribute kept verbatim (qualified name, unescaped value).
pub type RawAttr = (String, String);

// ---------------------------------------------------------------------------
// Workbook
// ---------------------------------------------------------------------------

/// A workbook (ECMA-376 Part 1, §18.2.27 `workbook`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Workbook {
    pub sheets: Vec<Worksheet>,
    /// Styles part (§18.8 Styles).
    pub styles: Stylesheet,
    /// Defined names other than the per-sheet print area / print titles (§18.2.5).
    pub defined_names: Vec<DefinedName>,
    /// `workbookPr/@date1904` (§18.2.28): serial dates use the 1904 base.
    pub date1904: bool,
    /// Theme colour scheme parsed from the theme part (read-only; the part itself is opaque).
    pub theme: Option<ThemeColors>,
    /// Attributes of the root `workbook` element (namespace declarations, `mc:Ignorable`, …).
    pub root_attrs: Vec<RawAttr>,
    /// Attributes of `workbookPr` other than `date1904`.
    pub workbook_pr_attrs: Vec<RawAttr>,
    /// Unknown children of `workbook` (fileVersion, bookViews, calcPr, pivotCaches, extLst, …).
    pub unknown: Vec<RawXml>,
    /// Preserved package content.
    pub opaque: Opaque,
}

impl Workbook {
    /// New workbook with one empty sheet and the default stylesheet.
    pub fn new() -> Self {
        Workbook {
            sheets: vec![Worksheet::new("Sheet1")],
            styles: Stylesheet::default_styles(),
            ..Workbook::default()
        }
    }

    /// Find a sheet by name (case-insensitive, as sheet names are, §18.2.19).
    pub fn sheet(&self, name: &str) -> Option<&Worksheet> {
        self.sheets
            .iter()
            .find(|s| s.name.to_lowercase() == name.to_lowercase())
    }

    /// Mutable sheet lookup by name.
    pub fn sheet_mut(&mut self, name: &str) -> Option<&mut Worksheet> {
        let lname = name.to_lowercase();
        self.sheets
            .iter_mut()
            .find(|s| s.name.to_lowercase() == lname)
    }
}

/// Theme colour scheme (ECMA-376 Part 1, §20.1.6.2 `clrScheme`), as `RRGGBB` strings
/// in document order: dk1, lt1, dk2, lt2, accent1–6, hlink, folHlink.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ThemeColors {
    pub colors: Vec<String>,
}

/// A defined name (ECMA-376 Part 1, §18.2.5 `definedName`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DefinedName {
    pub name: String,
    /// Formula text (e.g. `Sheet1!$A$1:$B$5`).
    pub formula: String,
    /// `localSheetId`: sheet scope (index into [`Workbook::sheets`]); `None` = workbook scope.
    pub local_sheet: Option<u32>,
    pub hidden: bool,
    /// Other attributes (comment, function, vbProcedure, xlm, …).
    pub attrs: Vec<RawAttr>,
}

/// Visibility of a sheet (§18.18.68 `ST_SheetState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SheetState {
    #[default]
    Visible,
    Hidden,
    VeryHidden,
}

/// What kind of sheet this is.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SheetKind {
    /// A worksheet (§18.3) modelled by [`Worksheet`].
    #[default]
    Worksheet,
    /// Chartsheet / dialogsheet / macro sheet kept as an opaque part.
    Opaque {
        rel_type: String,
        /// Absolute part name of the (opaque) sheet part.
        part: String,
    },
}

// ---------------------------------------------------------------------------
// Worksheet
// ---------------------------------------------------------------------------

/// A worksheet (ECMA-376 Part 1, §18.3.1.99 `worksheet`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Worksheet {
    /// Sheet name (§18.2.19 `sheet/@name`).
    pub name: String,
    /// `sheet/@sheetId` (stable id, unique within the workbook).
    pub sheet_id: u32,
    pub state: SheetState,
    pub kind: SheetKind,
    /// Sparse cells keyed by zero-based `(row, col)`.
    #[serde(with = "cell_map")]
    pub cells: BTreeMap<(u32, u32), Cell>,
    /// Row properties keyed by zero-based row (§18.3.1.73 `row`).
    pub rows: BTreeMap<u32, RowInfo>,
    /// Column ranges (§18.3.1.13 `col`), sorted by `min`, non-overlapping.
    pub cols: Vec<ColInfo>,
    /// `sheetFormatPr` (§18.3.1.81).
    pub format: SheetFormat,
    /// `sheetPr` (§18.3.1.82): tab colour, outline settings, fit-to-page.
    pub props: SheetProps,
    /// `sheetViews/sheetView` (§18.3.1.87): freeze/split panes, zoom, gridlines.
    pub views: Vec<SheetView>,
    /// `mergeCells` (§18.3.1.55).
    pub merges: Vec<CellRange>,
    /// `hyperlinks` (§18.3.1.47).
    pub hyperlinks: Vec<Hyperlink>,
    /// Comments / notes from the comments part (§18.7).
    pub comments: Vec<Comment>,
    /// `dataValidations` (§18.3.1.33).
    pub data_validations: Vec<DataValidation>,
    /// Other `dataValidations` attributes (disablePrompts, xWindow, yWindow).
    pub data_validations_attrs: Vec<RawAttr>,
    /// `conditionalFormatting` blocks (§18.3.1.18).
    pub conditional_formats: Vec<ConditionalFormat>,
    /// Tables (ListObjects, §18.5).
    pub tables: Vec<Table>,
    /// Sheet AutoFilter (§18.3.1.2).
    pub auto_filter: Option<AutoFilter>,
    /// Charts found in the sheet's drawing (read-only, see [`Chart`]).
    pub charts: Vec<Chart>,
    /// Print area formula (defined name `_xlnm.Print_Area`, §18.2.6).
    pub print_area: Option<String>,
    /// Print titles formula (defined name `_xlnm.Print_Titles`).
    pub print_titles: Option<String>,
    /// `printOptions`, `pageMargins`, `pageSetup`, `headerFooter`, breaks.
    pub page_setup: PageSetup,
    /// Attributes of the root `worksheet` element.
    pub root_attrs: Vec<RawAttr>,
    /// Unknown children of `worksheet` (sheetProtection, autoFilter, drawing, extLst, …).
    pub unknown: Vec<RawXml>,
    /// Relationships of the sheet part that point at opaque content.
    pub rels: Vec<OpaqueRel>,
}

impl Worksheet {
    /// A new empty worksheet.
    pub fn new(name: &str) -> Self {
        Worksheet {
            name: name.to_owned(),
            sheet_id: 1,
            ..Worksheet::default()
        }
    }

    /// Cell at zero-based `(row, col)`.
    pub fn cell(&self, row: u32, col: u32) -> Option<&Cell> {
        self.cells.get(&(row, col))
    }

    /// Cell at an A1 reference.
    pub fn cell_a1(&self, a1: &str) -> Option<&Cell> {
        let r = CellRef::parse(a1)?;
        self.cell(r.row, r.col)
    }

    /// Insert or replace a cell.
    pub fn set_cell(&mut self, row: u32, col: u32, cell: Cell) {
        self.cells.insert((row, col), cell);
    }

    /// Set a value at an A1 reference (keeps an existing style). Ignores invalid references.
    pub fn set_a1(&mut self, a1: &str, value: CellValue) {
        if let Some(r) = CellRef::parse(a1) {
            self.cells.entry((r.row, r.col)).or_default().value = value;
        }
    }

    /// Bounding range of all cells (`None` for an empty sheet).
    pub fn used_range(&self) -> Option<CellRange> {
        let mut it = self.cells.keys();
        let &(r0, c0) = it.next()?;
        let (mut r1, mut c1, mut rmin, mut cmin) = (r0, c0, r0, c0);
        for &(r, c) in self.cells.keys() {
            rmin = rmin.min(r);
            cmin = cmin.min(c);
            r1 = r1.max(r);
            c1 = c1.max(c);
        }
        Some(CellRange::new(
            CellRef::new(rmin, cmin),
            CellRef::new(r1, c1),
        ))
    }

    /// Column info covering zero-based `col`, if any.
    pub fn col_info(&self, col: u32) -> Option<&ColInfo> {
        self.cols.iter().find(|c| c.min <= col && col <= c.max)
    }

    /// Merged range containing `c`, if any.
    pub fn merge_at(&self, c: CellRef) -> Option<&CellRange> {
        self.merges.iter().find(|m| m.contains(c))
    }

    /// The first sheet view's frozen pane, if any.
    pub fn frozen(&self) -> Option<&Pane> {
        self.views
            .first()
            .and_then(|v| v.pane.as_ref())
            .filter(|p| p.state != PaneState::Split)
    }
}

mod cell_map {
    use super::Cell;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S: Serializer>(
        m: &BTreeMap<(u32, u32), Cell>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let v: Vec<(&(u32, u32), &Cell)> = m.iter().collect();
        v.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<(u32, u32), Cell>, D::Error> {
        let v: Vec<((u32, u32), Cell)> = Vec::deserialize(d)?;
        Ok(v.into_iter().collect())
    }
}

/// Sheet-wide row/column defaults (§18.3.1.81 `sheetFormatPr`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SheetFormat {
    /// `defaultRowHeight` in points.
    pub default_row_height: Option<f64>,
    /// `defaultColWidth` in characters.
    pub default_col_width: Option<f64>,
    /// `baseColWidth` in characters (default 8).
    pub base_col_width: Option<u32>,
    pub custom_height: bool,
    pub zero_height: bool,
    pub outline_level_row: u8,
    pub outline_level_col: u8,
    /// Other attributes (thickTop, thickBottom, x14ac:dyDescent, …).
    pub attrs: Vec<RawAttr>,
}

/// `sheetPr` (§18.3.1.82).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SheetProps {
    /// `tabColor` (§18.3.1.93).
    pub tab_color: Option<SheetColor>,
    /// `outlinePr/@summaryBelow` (§18.3.1.61), default true.
    pub summary_below: Option<bool>,
    /// `outlinePr/@summaryRight`, default true.
    pub summary_right: Option<bool>,
    /// `pageSetUpPr/@fitToPage` (§18.3.1.65).
    pub fit_to_page: Option<bool>,
    /// Whether a `sheetPr` element was present at all.
    pub present: bool,
    /// Attributes of `sheetPr` (codeName, filterMode, …).
    pub attrs: Vec<RawAttr>,
    /// Other attributes of `outlinePr`.
    pub outline_attrs: Vec<RawAttr>,
    /// Other attributes of `pageSetUpPr`.
    pub page_setup_attrs: Vec<RawAttr>,
}

/// Row properties (§18.3.1.73 `row`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RowInfo {
    /// `ht` in points.
    pub height: Option<f64>,
    pub custom_height: bool,
    pub hidden: bool,
    pub outline_level: u8,
    pub collapsed: bool,
    /// Row style (`s` with `customFormat="1"`).
    pub style: Option<u32>,
    pub thick_top: bool,
    pub thick_bot: bool,
    /// Other attributes (x14ac:dyDescent, ph, …).
    pub attrs: Vec<RawAttr>,
}

/// A column range (§18.3.1.13 `col`), zero-based inclusive.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ColInfo {
    pub min: u32,
    pub max: u32,
    /// Width in characters.
    pub width: Option<f64>,
    pub custom_width: bool,
    pub hidden: bool,
    pub best_fit: bool,
    pub outline_level: u8,
    pub collapsed: bool,
    pub style: Option<u32>,
}

/// One cell (§18.3.1.4 `c`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Cell {
    /// Value (cached result for formula cells).
    pub value: CellValue,
    pub formula: Option<Formula>,
    /// Index into [`Stylesheet::cell_xfs`] (`s`, default 0).
    pub style: u32,
    /// Cell metadata index (`cm`, used e.g. by dynamic arrays).
    pub cm: Option<u32>,
    /// Value metadata index (`vm`).
    pub vm: Option<u32>,
    /// `ph` show phonetic.
    pub show_phonetic: bool,
}

impl Cell {
    /// A cell holding `value` with the default style.
    pub fn new(value: CellValue) -> Self {
        Cell {
            value,
            ..Cell::default()
        }
    }

    /// Builder: set the style index.
    #[must_use]
    pub fn with_style(mut self, style: u32) -> Self {
        self.style = style;
        self
    }

    /// Builder: set a normal formula.
    #[must_use]
    pub fn with_formula(mut self, text: &str) -> Self {
        self.formula = Some(Formula::normal(text));
        self
    }
}

/// Cell value (§18.18.11 `ST_CellType`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum CellValue {
    #[default]
    Empty,
    Number(f64),
    Text(String),
    /// Rich text (shared string with runs, §18.4.4 `r`).
    RichText(Vec<TextRun>),
    Bool(bool),
    Error(CellError),
    /// ISO 8601 date (`t="d"`), kept as text.
    Date(String),
}

impl CellValue {
    /// Plain text content of text / rich-text values.
    pub fn as_text(&self) -> Option<String> {
        match self {
            CellValue::Text(s) => Some(s.clone()),
            CellValue::RichText(runs) => Some(runs.iter().map(|r| r.text.as_str()).collect()),
            _ => None,
        }
    }
}

/// Error values (§18.18.11 / §3.17.7 of Part 1 formulas).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CellError {
    Null,
    Div0,
    Value,
    Ref,
    Name,
    Num,
    NA,
    GettingData,
    Spill,
    Calc,
    /// Any other error literal.
    Other(String),
}

impl CellError {
    /// Parse an error literal (`#DIV/0!` …).
    pub fn parse(s: &str) -> CellError {
        match s {
            "#NULL!" => CellError::Null,
            "#DIV/0!" => CellError::Div0,
            "#VALUE!" => CellError::Value,
            "#REF!" => CellError::Ref,
            "#NAME?" => CellError::Name,
            "#NUM!" => CellError::Num,
            "#N/A" => CellError::NA,
            "#GETTING_DATA" => CellError::GettingData,
            "#SPILL!" => CellError::Spill,
            "#CALC!" => CellError::Calc,
            other => CellError::Other(other.to_owned()),
        }
    }

    /// The error literal.
    pub fn as_str(&self) -> &str {
        match self {
            CellError::Null => "#NULL!",
            CellError::Div0 => "#DIV/0!",
            CellError::Value => "#VALUE!",
            CellError::Ref => "#REF!",
            CellError::Name => "#NAME?",
            CellError::Num => "#NUM!",
            CellError::NA => "#N/A",
            CellError::GettingData => "#GETTING_DATA",
            CellError::Spill => "#SPILL!",
            CellError::Calc => "#CALC!",
            CellError::Other(s) => s,
        }
    }
}

/// A run of rich text (§18.4.4 `r` with `rPr` §18.4.7).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TextRun {
    pub text: String,
    /// Run properties; `None` = inherit the cell font.
    pub font: Option<Font>,
}

/// A cell formula (§18.3.1.40 `f`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Formula {
    /// Formula text without `=`. Empty for shared-formula followers.
    pub text: String,
    pub kind: FormulaKind,
    /// `ca` — calculate cell always.
    pub calc_always: bool,
    /// Other attributes (aca, dt2D, dtr, del1, del2, r1, r2, bx).
    pub attrs: Vec<RawAttr>,
}

impl Formula {
    /// A normal formula.
    pub fn normal(text: &str) -> Self {
        Formula {
            text: text.to_owned(),
            ..Formula::default()
        }
    }
}

/// Formula type (§18.18.6 `ST_CellFormulaType`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FormulaKind {
    #[default]
    Normal,
    /// Shared formula: `index` = `si`; `range` is set on the master cell only.
    Shared { index: u32, range: Option<String> },
    /// Array formula over `range`.
    Array { range: String },
    /// Data table formula over `range`.
    DataTable { range: String },
}

// ---------------------------------------------------------------------------
// Sheet views, hyperlinks, comments, validation, conditional formatting
// ---------------------------------------------------------------------------

/// `sheetView` (§18.3.1.87).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SheetView {
    pub workbook_view_id: u32,
    pub tab_selected: bool,
    /// `showGridLines` (default true).
    pub show_grid_lines: Option<bool>,
    /// `showRowColHeaders` (default true).
    pub show_headers: Option<bool>,
    pub zoom_scale: Option<u32>,
    pub right_to_left: bool,
    pub top_left_cell: Option<String>,
    pub pane: Option<Pane>,
    /// `selection` elements (§18.3.1.78), kept as raw attributes.
    pub selections: Vec<Vec<RawAttr>>,
    /// Other attributes.
    pub attrs: Vec<RawAttr>,
    /// Unknown children (pivotSelection, extLst).
    pub unknown: Vec<RawXml>,
}

/// Pane state (§18.18.53 `ST_PaneState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PaneState {
    #[default]
    Split,
    Frozen,
    FrozenSplit,
}

/// Freeze or split pane (§18.3.1.66 `pane`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Pane {
    /// Frozen: number of columns; split: horizontal position in 1/20 pt.
    pub x_split: f64,
    /// Frozen: number of rows; split: vertical position in 1/20 pt.
    pub y_split: f64,
    pub top_left_cell: Option<String>,
    pub active_pane: Option<String>,
    pub state: PaneState,
}

/// Hyperlink (§18.3.1.47).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Hyperlink {
    pub range: CellRange,
    /// External target (from the relationship), e.g. a URL.
    pub target: Option<String>,
    /// In-document location (`location`), e.g. `Sheet2!A1`.
    pub location: Option<String>,
    pub display: Option<String>,
    pub tooltip: Option<String>,
}

impl Default for CellRange {
    fn default() -> Self {
        CellRange::single(CellRef::new(0, 0))
    }
}

/// A comment / note (§18.7.3 `comment`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Comment {
    pub cell: CellRef,
    pub author: String,
    pub text: Vec<TextRun>,
}

/// Data validation (§18.3.1.32 `dataValidation`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DataValidation {
    pub sqref: Vec<CellRange>,
    /// `type` (§18.18.21): none, whole, decimal, list, date, time, textLength, custom.
    pub kind: Option<String>,
    /// `operator` (§18.18.20).
    pub operator: Option<String>,
    pub allow_blank: bool,
    pub show_input_message: bool,
    pub show_error_message: bool,
    /// `showDropDown` (note: true *hides* the in-cell dropdown, per spec).
    pub show_drop_down: bool,
    pub error_style: Option<String>,
    pub error_title: Option<String>,
    pub error: Option<String>,
    pub prompt_title: Option<String>,
    pub prompt: Option<String>,
    pub formula1: Option<String>,
    pub formula2: Option<String>,
    /// Other attributes (imeMode, xr:uid, …).
    pub attrs: Vec<RawAttr>,
}

/// A `conditionalFormatting` block (§18.3.1.18).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ConditionalFormat {
    pub sqref: Vec<CellRange>,
    pub pivot: bool,
    pub rules: Vec<CfRule>,
}

/// A conditional formatting rule (§18.3.1.10 `cfRule`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CfRule {
    /// `type` (§18.18.12): cellIs, expression, colorScale, dataBar, iconSet, top10, …
    pub kind: String,
    pub priority: i32,
    pub dxf_id: Option<u32>,
    pub operator: Option<String>,
    pub stop_if_true: bool,
    pub text: Option<String>,
    /// `formula` children (§18.3.1.43).
    pub formulas: Vec<String>,
    pub color_scale: Option<ColorScale>,
    pub data_bar: Option<DataBar>,
    pub icon_set: Option<IconSet>,
    /// Other attributes (rank, percent, bottom, aboveAverage, timePeriod, …).
    pub attrs: Vec<RawAttr>,
    /// Unknown children (extLst).
    pub unknown: Vec<RawXml>,
}

/// Conditional format value object (§18.3.1.11 `cfvo`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Cfvo {
    /// `type` (§18.18.13): num, percent, max, min, formula, percentile.
    pub kind: String,
    pub val: Option<String>,
    /// `gte` (default true).
    pub gte: Option<bool>,
}

/// Colour scale (§18.3.1.16).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ColorScale {
    pub cfvos: Vec<Cfvo>,
    pub colors: Vec<SheetColor>,
}

/// Data bar (§18.3.1.28).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DataBar {
    pub cfvos: Vec<Cfvo>,
    pub color: Option<SheetColor>,
    pub min_length: Option<u32>,
    pub max_length: Option<u32>,
    pub show_value: Option<bool>,
}

/// Icon set (§18.3.1.49).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IconSet {
    /// `iconSet` (§18.18.42), default `3TrafficLights1`.
    pub icon_set: Option<String>,
    pub cfvos: Vec<Cfvo>,
    pub show_value: Option<bool>,
    pub percent: Option<bool>,
    pub reverse: Option<bool>,
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

/// A table / ListObject (§18.5.1.2 `table`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Table {
    pub id: u32,
    pub name: String,
    pub display_name: String,
    pub range: CellRange,
    /// `headerRowCount` (default 1).
    pub header_row_count: Option<u32>,
    /// `totalsRowCount` (default 0).
    pub totals_row_count: Option<u32>,
    /// `totalsRowShown`.
    pub totals_row_shown: Option<bool>,
    pub columns: Vec<TableColumn>,
    /// `tableStyleInfo` (§18.5.1.5).
    pub style: Option<TableStyleInfo>,
    /// `autoFilter` element kept verbatim (§18.3.1.2).
    pub auto_filter: Option<String>,
    /// Other root attributes (namespace declarations, xr:uid, …).
    pub attrs: Vec<RawAttr>,
    /// Unknown children (sortState, extLst).
    pub unknown: Vec<RawXml>,
}

/// A table column (§18.5.1.3 `tableColumn`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TableColumn {
    pub id: u32,
    pub name: String,
    pub totals_row_function: Option<String>,
    pub totals_row_label: Option<String>,
    /// `calculatedColumnFormula` (§18.5.1.1).
    pub calculated_formula: Option<String>,
    /// `totalsRowFormula`.
    pub totals_row_formula: Option<String>,
    /// Other attributes (dataDxfId, xr3:uid, …).
    pub attrs: Vec<RawAttr>,
    /// Unknown children (xmlColumnPr, extLst).
    pub unknown: Vec<RawXml>,
}

/// `tableStyleInfo` (§18.5.1.5).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TableStyleInfo {
    pub name: Option<String>,
    pub show_first_column: bool,
    pub show_last_column: bool,
    pub show_row_stripes: bool,
    pub show_column_stripes: bool,
}

// ---------------------------------------------------------------------------
// AutoFilter
// ---------------------------------------------------------------------------

/// Worksheet AutoFilter (ECMA-376 Part 1, §18.3.1.2 `autoFilter`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AutoFilter {
    /// `ref`: the filtered range, header row included.
    pub range: CellRange,
    /// `filterColumn` children (§18.3.2.7).
    pub columns: Vec<FilterColumn>,
    /// Other attributes (xr:uid, …).
    pub attrs: Vec<RawAttr>,
    /// Unknown children (sortState §18.3.1.92, extLst), anchored like other raw XML.
    pub unknown: Vec<RawXml>,
}

/// One filtered column (§18.3.2.7 `filterColumn`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FilterColumn {
    /// `colId`: zero-based column offset inside [`AutoFilter::range`].
    pub col_id: u32,
    /// `hiddenButton`.
    pub hidden_button: bool,
    /// `showButton` (default true).
    pub show_button: Option<bool>,
    pub filter: ColumnFilter,
}

/// The criterion of a filter column.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum ColumnFilter {
    /// No criterion (button only).
    #[default]
    None,
    /// `filters` (§18.3.2.8): show rows whose displayed text equals one of `values`
    /// (case-insensitive); `blank` also shows empty cells.
    Values { values: Vec<String>, blank: bool },
    /// `customFilters` (§18.3.2.3): one or two `customFilter` conditions (§18.3.2.2)
    /// joined with AND (`and = true`) or OR.
    Custom {
        and: bool,
        conditions: Vec<CustomFilter>,
    },
    /// `top10` (§18.3.2.10).
    Top10 {
        top: bool,
        percent: bool,
        val: f64,
        /// `filterVal`: the cut-off value cached by the writing application.
        filter_val: Option<f64>,
    },
    /// Any other criterion (dynamicFilter, colorFilter, iconFilter, dateGroupItem…)
    /// kept as the verbatim child element.
    Raw(String),
}

/// `customFilter` (§18.3.2.2): `operator` (§18.18.31 `ST_FilterOperator`, default
/// `equal`) and `val`, which may use the `*` / `?` wildcards.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CustomFilter {
    pub operator: Option<String>,
    pub val: String,
}

// ---------------------------------------------------------------------------
// Charts (read-only view of DrawingML chart parts)
// ---------------------------------------------------------------------------

/// A chart placed on a worksheet, parsed from its drawing (ECMA-376 Part 1,
/// §20.5 SpreadsheetML Drawing) and chart part (§21.2 DrawingML Charts, `c:chartSpace`).
///
/// This is a **read-only** summary for rendering: the drawing and chart parts stay
/// opaque and are written back byte-for-byte, so edits to this struct are not saved.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Chart {
    /// Absolute name of the chart part, e.g. `/xl/charts/chart1.xml`.
    pub part: String,
    /// Anchor cells (zero-based) of the drawing anchor (§20.5.2.33 twoCellAnchor).
    pub from: CellRef,
    pub to: Option<CellRef>,
    /// Title text (rich text runs concatenated, or the cached `strRef` value).
    pub title: Option<String>,
    /// Plot groups inside `c:plotArea` (several = combo chart).
    pub plots: Vec<ChartPlot>,
    /// `c:legend/c:legendPos` (§21.2.2.94): `r`, `l`, `t`, `b`, `tr`; `None` = no legend.
    pub legend: Option<String>,
}

/// Chart family (`c:barChart` §21.2.2.16, `c:lineChart` §21.2.2.97, …).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ChartKind {
    /// Bar/column; `c:barDir` `col` = vertical columns, `bar` = horizontal bars.
    #[default]
    Bar,
    Line,
    Area,
    Pie,
    Doughnut,
    Scatter,
    Radar,
    Bubble,
    /// Any other plot element (stock, surface, ofPie, 3-D variants…), by local name.
    Other(String),
}

/// One plot group of a chart.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ChartPlot {
    pub kind: ChartKind,
    /// `c:barDir` for bar charts (`bar` / `col`).
    pub bar_dir: Option<String>,
    /// `c:grouping` (§21.2.2.76): clustered, stacked, percentStacked, standard.
    pub grouping: Option<String>,
    pub series: Vec<ChartSeries>,
}

/// A data series (`c:ser`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ChartSeries {
    /// Series name (cached text of `c:tx`).
    pub name: Option<String>,
    /// Formula of the name reference (`c:tx/c:strRef/c:f`).
    pub name_ref: Option<String>,
    /// Category (or scatter X) reference formula (`c:cat` / `c:xVal`).
    pub cat_ref: Option<String>,
    /// Value (or scatter Y) reference formula (`c:val` / `c:yVal`).
    pub val_ref: Option<String>,
    /// Cached categories.
    pub categories: Vec<String>,
    /// Cached values (`None` = missing point).
    pub values: Vec<Option<f64>>,
}

// ---------------------------------------------------------------------------
// Page setup
// ---------------------------------------------------------------------------

/// Printing related sheet settings (§18.3.1.70 printOptions, §18.3.1.62 pageMargins,
/// §18.3.1.63 pageSetup, §18.3.1.46 headerFooter, §18.3.1.74 rowBreaks / colBreaks).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PageSetup {
    pub print_grid_lines: bool,
    pub print_headings: bool,
    pub horizontal_centered: bool,
    pub vertical_centered: bool,
    /// Whether `printOptions` was present.
    pub has_print_options: bool,
    /// Margins in inches: left, right, top, bottom, header, footer.
    pub margins: Option<PageMargins>,
    /// `paperSize` (§18.3.1.63; 1 = Letter, 9 = A4).
    pub paper_size: Option<u32>,
    /// `orientation`: `portrait` / `landscape` / `default`.
    pub orientation: Option<String>,
    /// `scale` percent (10–400).
    pub scale: Option<u32>,
    pub fit_to_width: Option<u32>,
    pub fit_to_height: Option<u32>,
    /// Other `pageSetup` attributes (r:id, horizontalDpi, firstPageNumber, …).
    pub attrs: Vec<RawAttr>,
    /// Whether `pageSetup` was present.
    pub has_page_setup: bool,
    pub odd_header: Option<String>,
    pub odd_footer: Option<String>,
    /// Whole `headerFooter` element when it has more than odd header/footer.
    pub header_footer_raw: Option<String>,
    /// Manual row breaks (zero-based row *after which* a page breaks).
    pub row_breaks: Vec<u32>,
    /// Manual column breaks.
    pub col_breaks: Vec<u32>,
}

/// Page margins in inches (§18.3.1.62).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PageMargins {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
    pub header: f64,
    pub footer: f64,
}

impl Default for PageMargins {
    /// The "normal" margins written by common spreadsheet applications.
    fn default() -> Self {
        PageMargins {
            left: 0.7,
            right: 0.7,
            top: 0.75,
            bottom: 0.75,
            header: 0.3,
            footer: 0.3,
        }
    }
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

/// A colour reference (§18.3.1.15 / §18.8.19 `CT_Color`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SheetColor {
    /// `rgb` as `AARRGGBB`.
    pub rgb: Option<String>,
    pub theme: Option<u32>,
    pub indexed: Option<u32>,
    pub tint: Option<f64>,
    pub auto: bool,
}

impl SheetColor {
    /// Colour from `RRGGBB` (opaque).
    pub fn rgb(hex: &str) -> Self {
        SheetColor {
            rgb: Some(format!("FF{}", hex.trim_start_matches('#'))),
            ..SheetColor::default()
        }
    }
}

/// The styles part (§18.8.39 `styleSheet`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Stylesheet {
    /// Custom number formats (§18.8.30 `numFmt`): (id, format code).
    pub num_fmts: Vec<NumFmt>,
    pub fonts: Vec<Font>,
    pub fills: Vec<Fill>,
    pub borders: Vec<Border>,
    /// `cellStyleXfs` (§18.8.9): formatting records of named cell styles.
    pub cell_style_xfs: Vec<CellXf>,
    /// `cellXfs` (§18.8.10): formatting records referenced by cells (`c/@s`).
    pub cell_xfs: Vec<CellXf>,
    /// `cellStyles` (§18.8.8).
    pub cell_styles: Vec<CellStyle>,
    /// `dxfs` (§18.8.15): differential formats for conditional formatting / tables.
    pub dxfs: Vec<Dxf>,
    /// Root attributes (namespaces, mc:Ignorable).
    pub root_attrs: Vec<RawAttr>,
    /// Unknown children (tableStyles, colors, extLst).
    pub unknown: Vec<RawXml>,
}

impl Stylesheet {
    /// The minimal stylesheet every workbook needs: one font (Calibri 11), the two
    /// mandatory fills (none, gray125), one empty border, one style xf and one cell xf.
    pub fn default_styles() -> Self {
        Stylesheet {
            fonts: vec![Font {
                name: Some("Calibri".into()),
                size: Some(11.0),
                family: Some(2),
                scheme: Some("minor".into()),
                ..Font::default()
            }],
            fills: vec![
                Fill::Pattern(PatternFill {
                    pattern: Some("none".into()),
                    ..PatternFill::default()
                }),
                Fill::Pattern(PatternFill {
                    pattern: Some("gray125".into()),
                    ..PatternFill::default()
                }),
            ],
            borders: vec![Border::default()],
            cell_style_xfs: vec![CellXf::default()],
            cell_xfs: vec![CellXf {
                xf_id: Some(0),
                ..CellXf::default()
            }],
            cell_styles: vec![CellStyle {
                name: "Normal".into(),
                xf_id: 0,
                builtin_id: Some(0),
                ..CellStyle::default()
            }],
            ..Stylesheet::default()
        }
    }

    /// Format code for a number format id: custom formats only (built-ins live in `bo-sheet`).
    pub fn custom_num_fmt(&self, id: u32) -> Option<&str> {
        self.num_fmts
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.code.as_str())
    }

    /// Cell format by index (falls back to the first one).
    pub fn xf(&self, index: u32) -> Option<&CellXf> {
        self.cell_xfs
            .get(index as usize)
            .or_else(|| self.cell_xfs.first())
    }

    /// Add a cell format (deduplicated) and return its index.
    pub fn add_xf(&mut self, xf: CellXf) -> u32 {
        if let Some(i) = self.cell_xfs.iter().position(|x| *x == xf) {
            return i as u32;
        }
        self.cell_xfs.push(xf);
        (self.cell_xfs.len() - 1) as u32
    }

    /// Add (or reuse) a custom number format and return its id (custom ids start at 164).
    pub fn add_num_fmt(&mut self, code: &str) -> u32 {
        if let Some(f) = self.num_fmts.iter().find(|f| f.code == code) {
            return f.id;
        }
        let id = self
            .num_fmts
            .iter()
            .map(|f| f.id + 1)
            .max()
            .unwrap_or(164)
            .max(164);
        self.num_fmts.push(NumFmt {
            id,
            code: code.to_owned(),
        });
        id
    }

    /// Add (or reuse) a font and return its index.
    pub fn add_font(&mut self, font: Font) -> u32 {
        if let Some(i) = self.fonts.iter().position(|f| *f == font) {
            return i as u32;
        }
        self.fonts.push(font);
        (self.fonts.len() - 1) as u32
    }

    /// Add (or reuse) a fill and return its index.
    pub fn add_fill(&mut self, fill: Fill) -> u32 {
        if let Some(i) = self.fills.iter().position(|f| *f == fill) {
            return i as u32;
        }
        self.fills.push(fill);
        (self.fills.len() - 1) as u32
    }

    /// Add (or reuse) a border and return its index.
    pub fn add_border(&mut self, border: Border) -> u32 {
        if let Some(i) = self.borders.iter().position(|b| *b == border) {
            return i as u32;
        }
        self.borders.push(border);
        (self.borders.len() - 1) as u32
    }
}

/// A custom number format (§18.8.30 `numFmt`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NumFmt {
    pub id: u32,
    pub code: String,
}

/// Font (§18.8.22 `font`; also run properties §18.4.7 `rPr`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Font {
    pub name: Option<String>,
    /// Size in points (`sz`).
    pub size: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    /// `u` (§18.18.82): single, double, singleAccounting, doubleAccounting.
    pub underline: Option<String>,
    pub strike: bool,
    pub color: Option<SheetColor>,
    pub family: Option<u32>,
    pub charset: Option<u32>,
    /// `scheme`: major / minor / none.
    pub scheme: Option<String>,
    /// `vertAlign`: superscript / subscript / baseline.
    pub vert_align: Option<String>,
    pub outline: bool,
    pub shadow: bool,
    pub condense: bool,
    pub extend: bool,
}

/// Fill (§18.8.20 `fill`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Fill {
    /// `patternFill` (§18.8.32).
    Pattern(PatternFill),
    /// `gradientFill` (§18.8.24).
    Gradient(GradientFill),
}

impl Default for Fill {
    fn default() -> Self {
        Fill::Pattern(PatternFill::default())
    }
}

impl Fill {
    /// A solid fill of `RRGGBB`.
    pub fn solid(hex: &str) -> Self {
        Fill::Pattern(PatternFill {
            pattern: Some("solid".into()),
            fg: Some(SheetColor::rgb(hex)),
            bg: Some(SheetColor {
                indexed: Some(64),
                ..SheetColor::default()
            }),
        })
    }
}

/// Pattern fill (§18.8.32).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PatternFill {
    /// `patternType` (§18.18.55): none, solid, gray125, darkGray, …
    pub pattern: Option<String>,
    pub fg: Option<SheetColor>,
    pub bg: Option<SheetColor>,
}

/// Gradient fill (§18.8.24).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GradientFill {
    /// `type`: linear (default) / path.
    pub kind: Option<String>,
    pub degree: Option<f64>,
    pub left: Option<f64>,
    pub right: Option<f64>,
    pub top: Option<f64>,
    pub bottom: Option<f64>,
    /// Gradient stops (position 0..1, colour).
    pub stops: Vec<(f64, SheetColor)>,
}

/// One border edge (§18.8.43 `CT_BorderPr`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BorderEdge {
    /// `style` (§18.18.3): thin, medium, thick, dashed, dotted, double, hair, …
    pub style: Option<String>,
    pub color: Option<SheetColor>,
}

/// Border (§18.8.4 `border`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Border {
    pub left: BorderEdge,
    pub right: BorderEdge,
    pub top: BorderEdge,
    pub bottom: BorderEdge,
    pub diagonal: BorderEdge,
    pub vertical: Option<BorderEdge>,
    pub horizontal: Option<BorderEdge>,
    pub diagonal_up: bool,
    pub diagonal_down: bool,
    pub outline: Option<bool>,
}

/// Cell alignment (§18.8.1 `alignment`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CellAlignment {
    /// `horizontal` (§18.18.40): general, left, center, right, fill, justify, centerContinuous, distributed.
    pub horizontal: Option<String>,
    /// `vertical` (§18.18.88): top, center, bottom, justify, distributed.
    pub vertical: Option<String>,
    pub wrap_text: bool,
    pub shrink_to_fit: bool,
    pub indent: Option<u32>,
    /// `textRotation` (0–180, 255 = vertical).
    pub text_rotation: Option<u32>,
    pub reading_order: Option<u32>,
    pub justify_last_line: bool,
    pub relative_indent: Option<i32>,
}

/// Cell protection (§18.8.33 `protection`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CellProtection {
    /// `locked` (default true).
    pub locked: Option<bool>,
    /// `hidden` (default false).
    pub hidden: Option<bool>,
}

/// A cell format record (§18.8.45 `xf`) — the "style" of a cell:
/// number format, font, fill, border, alignment, protection.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CellXf {
    pub num_fmt_id: u32,
    pub font_id: u32,
    pub fill_id: u32,
    pub border_id: u32,
    /// Parent cell style (`xfId`, index into `cellStyleXfs`); only in `cellXfs`.
    pub xf_id: Option<u32>,
    pub apply_number_format: Option<bool>,
    pub apply_font: Option<bool>,
    pub apply_fill: Option<bool>,
    pub apply_border: Option<bool>,
    pub apply_alignment: Option<bool>,
    pub apply_protection: Option<bool>,
    pub quote_prefix: bool,
    pub pivot_button: bool,
    pub alignment: Option<CellAlignment>,
    pub protection: Option<CellProtection>,
}

/// Alias used by the rest of the code base: a cell "style" is an `xf` record.
pub type Style = CellXf;

/// A named cell style (§18.8.7 `cellStyle`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CellStyle {
    pub name: String,
    pub xf_id: u32,
    pub builtin_id: Option<u32>,
    pub i_level: Option<u32>,
    pub hidden: bool,
    pub custom_builtin: bool,
}

/// Differential format (§18.8.14 `dxf`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Dxf {
    pub font: Option<Font>,
    pub num_fmt: Option<NumFmt>,
    pub fill: Option<Fill>,
    pub alignment: Option<CellAlignment>,
    pub border: Option<Border>,
    pub protection: Option<CellProtection>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_letters() {
        assert_eq!(col_to_letters(0), "A");
        assert_eq!(col_to_letters(25), "Z");
        assert_eq!(col_to_letters(26), "AA");
        assert_eq!(col_to_letters(701), "ZZ");
        assert_eq!(col_to_letters(702), "AAA");
        assert_eq!(col_to_letters(MAX_COLS - 1), "XFD");
        for c in [0, 1, 25, 26, 27, 51, 52, 701, 702, 16383] {
            assert_eq!(letters_to_col(&col_to_letters(c)), Some(c));
        }
        assert_eq!(letters_to_col("xfd"), Some(16383));
        assert_eq!(letters_to_col("XFE"), None);
        assert_eq!(letters_to_col("A1"), None);
    }

    #[test]
    fn refs_and_ranges() {
        assert_eq!(CellRef::parse("B3"), Some(CellRef::new(2, 1)));
        assert_eq!(CellRef::parse("$C$10"), Some(CellRef::new(9, 2)));
        assert_eq!(CellRef::parse("A0"), None);
        assert_eq!(CellRef::parse("A1048577"), None);
        assert_eq!(CellRef::parse("1A"), None);
        let r = CellRange::parse("C3:A1").unwrap();
        assert_eq!(r.to_string(), "A1:C3");
        assert!(r.contains(CellRef::new(1, 1)));
        assert_eq!((r.rows(), r.cols()), (3, 3));
        assert_eq!(
            sqref_to_string(&CellRange::parse_sqref("A1 B2:C3")),
            "A1 B2:C3"
        );
    }

    #[test]
    fn workbook_helpers() {
        let mut wb = Workbook::new();
        let ws = wb.sheet_mut("sheet1").unwrap();
        ws.set_a1("B2", CellValue::Number(1.0));
        ws.set_a1("D5", CellValue::Text("x".into()));
        assert_eq!(ws.used_range().unwrap().to_string(), "B2:D5");
        let id = wb.styles.add_num_fmt("0.000");
        assert_eq!(id, 164);
        assert_eq!(wb.styles.add_num_fmt("0.000"), 164);
        assert_eq!(wb.styles.add_num_fmt("#,##0.0"), 165);
        assert_eq!(CellError::parse("#N/A").as_str(), "#N/A");
    }
}
