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

mod cell_media;
mod custom_view;
mod form_control;
pub use cell_media::{
    CellImage, CellImageSource, ImageSizing, CALC_ORIGIN_DECORATIVE, CALC_ORIGIN_STANDALONE,
};
pub use custom_view::{CustomSheetView, CustomWorkbookView, SheetViewType};
pub use form_control::{CheckState, ControlAnchor, FormControl, FormControlKind};

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
    /// Zero-based row index.
    pub row: u32,
    /// Zero-based column index.
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
        if s.contains('$') {
            let plain: String = s.chars().filter(|c| *c != '$').collect();
            return Self::parse_plain(&plain);
        }
        Self::parse_plain(s)
    }

    /// [`CellRef::parse`] of a reference without `$` (no allocation: every cell of a
    /// worksheet has one).
    fn parse_plain(s: &str) -> Option<CellRef> {
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
    /// Top-left cell.
    pub first: CellRef,
    /// Bottom-right cell (inclusive).
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
    /// Relationship id (`Id`, e.g. `rId3`).
    pub id: String,
    /// Relationship type URI (`Type`).
    pub rel_type: String,
    /// Target: an absolute part name, or the URI of an external target.
    pub target: String,
    /// `TargetMode="External"`.
    pub external: bool,
}

/// A package part kept byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OpaquePart {
    /// Absolute part name, e.g. `/xl/theme/theme1.xml`.
    pub name: String,
    /// Content type override from `[Content_Types].xml`, if any.
    pub content_type: Option<String>,
    /// Raw part bytes.
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
    /// Sheets in workbook order (`sheets/sheet`).
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
    /// Unknown children of `workbook` (fileVersion, bookViews, calcPr, extLst, …).
    pub unknown: Vec<RawXml>,
    /// Pivot caches (`pivotCaches`, §18.2.22), in file order.
    #[serde(default)]
    pub pivot_caches: Vec<PivotCache>,
    /// Authors of threaded comments ([MS-XLSX] person part, `CT_Person`).
    #[serde(default)]
    pub persons: Vec<Person>,
    /// `customWorkbookViews` (§18.2.3): named custom views.
    #[serde(default)]
    pub custom_views: Vec<CustomWorkbookView>,
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
    /// Scheme colours as `RRGGBB` strings, in the order listed above.
    pub colors: Vec<String>,
}

/// A defined name (ECMA-376 Part 1, §18.2.5 `definedName`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DefinedName {
    /// Name (`name`), e.g. `Tax_Rate` or a built-in `_xlnm.*` name.
    pub name: String,
    /// Formula text (e.g. `Sheet1!$A$1:$B$5`).
    pub formula: String,
    /// `localSheetId`: sheet scope (index into [`Workbook::sheets`]); `None` = workbook scope.
    pub local_sheet: Option<u32>,
    /// `hidden`: the name is not shown in the user interface.
    pub hidden: bool,
    /// Other attributes (comment, function, vbProcedure, xlm, …).
    pub attrs: Vec<RawAttr>,
}

/// Visibility of a sheet (§18.18.68 `ST_SheetState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SheetState {
    /// Shown normally.
    #[default]
    Visible,
    /// Hidden; the user can unhide it.
    Hidden,
    /// Hidden; only unhidable programmatically (`veryHidden`).
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
        /// Relationship type of the sheet part.
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
    /// Visibility (`sheet/@state`).
    pub state: SheetState,
    /// Worksheet or opaque sheet kind.
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
    /// `authors` of the comments part in file order (§18.7.2), so author ids and
    /// unused authors survive a round trip. Authors missing here are appended on write.
    pub comment_authors: Vec<String>,
    /// Threaded comments of the sheet ([MS-XLSX] threaded comments part,
    /// `CT_ThreadedComment`), in file order; replies follow their thread.
    #[serde(default)]
    pub threaded_comments: Vec<ThreadedComment>,
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
    /// Pivot tables of the sheet (see [`PivotTable`]).
    #[serde(default)]
    pub pivots: Vec<PivotTable>,
    /// SmartArt graphic frames of the sheet's drawing, shown through their
    /// drawing fallback (read-only, see [`SheetDiagram`]).
    #[serde(default)]
    pub diagrams: Vec<SheetDiagram>,
    /// Slicers of the sheet ([MS-XLSX] slicers and slicer caches).
    #[serde(default)]
    pub slicers: Vec<Slicer>,
    /// Timelines of the sheet ([MS-XLSX] timelines and timeline caches).
    #[serde(default)]
    pub timelines: Vec<Timeline>,
    /// Form controls and ActiveX controls of the sheet ([MS-XLSX] control properties parts,
    /// §18.3.1.21 `controls`, legacy VML `x:ClientData`), in drawing order.
    #[serde(default)]
    pub form_controls: Vec<FormControl>,
    /// Sparkline groups of the sheet ([MS-XLSX] `x14:sparklineGroups` in the worksheet `extLst`).
    #[serde(default)]
    pub sparkline_groups: Vec<SparklineGroup>,
    /// What-if scenarios of the sheet (ECMA-376 Part 1, §18.3.1 `scenarios`, CT_Scenarios).
    #[serde(default)]
    pub scenarios: Option<Scenarios>,
    /// Print area formula (defined name `_xlnm.Print_Area`, §18.2.6).
    pub print_area: Option<String>,
    /// Print titles formula (defined name `_xlnm.Print_Titles`).
    pub print_titles: Option<String>,
    /// `printOptions`, `pageMargins`, `pageSetup`, `headerFooter`, breaks.
    pub page_setup: PageSetup,
    /// `customSheetViews` (§18.3.1.26): this sheet's half of the custom views.
    #[serde(default)]
    pub custom_views: Vec<CustomSheetView>,
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
    /// `customHeight`: `default_row_height` was set explicitly.
    pub custom_height: bool,
    /// `zeroHeight`: rows are hidden by default.
    pub zero_height: bool,
    /// `outlineLevelRow`: highest row outline level in use.
    pub outline_level_row: u8,
    /// `outlineLevelCol`: highest column outline level in use.
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
    /// `customHeight`: the height was set manually.
    pub custom_height: bool,
    /// `hidden`.
    pub hidden: bool,
    /// `outlineLevel` (0 = not grouped).
    pub outline_level: u8,
    /// `collapsed`: the outline group below is collapsed.
    pub collapsed: bool,
    /// Row style (`s` with `customFormat="1"`).
    pub style: Option<u32>,
    /// `thickTop`: a cell of the row has a thick or double top border.
    pub thick_top: bool,
    /// `thickBot`: a cell of the row has a thick or double bottom border.
    pub thick_bot: bool,
    /// Other attributes (x14ac:dyDescent, ph, …).
    pub attrs: Vec<RawAttr>,
}

/// A column range (§18.3.1.13 `col`), zero-based inclusive.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ColInfo {
    /// First column, zero-based (`min` − 1).
    pub min: u32,
    /// Last column, zero-based inclusive (`max` − 1).
    pub max: u32,
    /// Width in characters.
    pub width: Option<f64>,
    /// `customWidth`: the width was set manually.
    pub custom_width: bool,
    /// `hidden`.
    pub hidden: bool,
    /// `bestFit`: the width was auto-fitted to the contents.
    pub best_fit: bool,
    /// `outlineLevel` (0 = not grouped).
    pub outline_level: u8,
    /// `collapsed`: the outline group is collapsed.
    pub collapsed: bool,
    /// Default style of the column's cells (`style`, index into `cellXfs`).
    pub style: Option<u32>,
}

/// One cell (§18.3.1.4 `c`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Cell {
    /// Value (cached result for formula cells).
    pub value: CellValue,
    /// Formula (`f`), if any.
    pub formula: Option<Formula>,
    /// Index into [`Stylesheet::cell_xfs`] (`s`, default 0).
    pub style: u32,
    /// Cell metadata index (`cm`, used e.g. by dynamic arrays).
    pub cm: Option<u32>,
    /// Value metadata index (`vm`).
    pub vm: Option<u32>,
    /// `ph` show phonetic.
    pub show_phonetic: bool,
    /// Picture shown in the cell: a `_localImage` rich value read through `vm`
    /// ([MS-XLSX] §2.3.6.1.10) or the result of `IMAGE` (§2.3.6.1.11). The value
    /// is then the fallback `#VALUE!`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<Box<CellImage>>,
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
    /// No value.
    #[default]
    Empty,
    /// A number (`t="n"`, the default type).
    Number(f64),
    /// Plain text (`t="s"` shared, `t="str"` formula string or `t="inlineStr"`).
    Text(String),
    /// Rich text (shared string with runs, §18.4.4 `r`).
    RichText(Vec<TextRun>),
    /// A boolean (`t="b"`).
    Bool(bool),
    /// An error value (`t="e"`).
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
    /// `#NULL!`
    Null,
    /// `#DIV/0!`
    Div0,
    /// `#VALUE!`
    Value,
    /// `#REF!`
    Ref,
    /// `#NAME?`
    Name,
    /// `#NUM!`
    Num,
    /// `#N/A`
    NA,
    /// `#GETTING_DATA`
    GettingData,
    /// `#SPILL!`
    Spill,
    /// `#CALC!`
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
    /// Run text (`t`).
    pub text: String,
    /// Run properties; `None` = inherit the cell font.
    pub font: Option<Font>,
}

/// A cell formula (§18.3.1.40 `f`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Formula {
    /// Formula text without `=`. Empty for shared-formula followers.
    pub text: String,
    /// Formula type (`t`) and its shared / array range.
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
    /// An ordinary single-cell formula.
    #[default]
    Normal,
    /// Shared formula: `index` = `si`; `range` is set on the master cell only.
    Shared {
        /// `si`: shared formula group index.
        index: u32,
        /// `ref`: range of the group (master cell only).
        range: Option<String>,
    },
    /// Array formula over `range`.
    Array {
        /// `ref`: the cells the array formula fills.
        range: String,
    },
    /// Data table formula over `range`.
    DataTable {
        /// `ref`: the cells of the data table.
        range: String,
    },
}

// ---------------------------------------------------------------------------
// Sheet views, hyperlinks, comments, validation, conditional formatting
// ---------------------------------------------------------------------------

/// `sheetView` (§18.3.1.87).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SheetView {
    /// `workbookViewId`: the `bookViews/workbookView` this view belongs to.
    pub workbook_view_id: u32,
    /// `tabSelected`: the sheet tab is selected.
    pub tab_selected: bool,
    /// `showGridLines` (default true).
    pub show_grid_lines: Option<bool>,
    /// `showRowColHeaders` (default true).
    pub show_headers: Option<bool>,
    /// `zoomScale` percent (default 100).
    pub zoom_scale: Option<u32>,
    /// `rightToLeft`: columns run right to left.
    pub right_to_left: bool,
    /// `topLeftCell`: A1 reference of the top-left visible cell.
    pub top_left_cell: Option<String>,
    /// `pane` (§18.3.1.66): freeze or split pane, if any.
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
    /// Panes are split, not frozen.
    #[default]
    Split,
    /// Panes are frozen.
    Frozen,
    /// Panes are frozen, and unfreezing leaves a split.
    FrozenSplit,
}

/// Freeze or split pane (§18.3.1.66 `pane`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Pane {
    /// Frozen: number of columns; split: horizontal position in 1/20 pt.
    pub x_split: f64,
    /// Frozen: number of rows; split: vertical position in 1/20 pt.
    pub y_split: f64,
    /// `topLeftCell` of the bottom-right pane (A1 reference).
    pub top_left_cell: Option<String>,
    /// `activePane`: bottomRight, topRight, bottomLeft or topLeft.
    pub active_pane: Option<String>,
    /// `state`.
    pub state: PaneState,
}

/// Hyperlink (§18.3.1.47).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Hyperlink {
    /// Cells covered by the link (`ref`).
    pub range: CellRange,
    /// External target (from the relationship), e.g. a URL.
    pub target: Option<String>,
    /// In-document location (`location`), e.g. `Sheet2!A1`.
    pub location: Option<String>,
    /// `display`: text shown in the cell.
    pub display: Option<String>,
    /// `tooltip`.
    pub tooltip: Option<String>,
}

impl Default for CellRange {
    fn default() -> Self {
        CellRange::single(CellRef::new(0, 0))
    }
}

/// One comment of a thread ([MS-XLSX] `CT_ThreadedComment`): the thread's
/// first comment has no `parent_id`; replies name it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadedComment {
    /// `id` (GUID in braces).
    pub id: String,
    /// `ref`: the anchor cell.
    pub cell: CellRef,
    /// `personId`: a [`Person::id`].
    pub person_id: String,
    /// `dT`: xsd:dateTime.
    pub date: Option<String>,
    /// `parentId`: the thread's first comment.
    pub parent_id: Option<String>,
    /// `text`.
    pub text: String,
    /// `done`: the thread is resolved.
    pub done: bool,
}

/// An author of threaded comments ([MS-XLSX] `CT_Person`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    /// `id` (GUID) referenced by `personId`.
    pub id: String,
    /// `displayName`.
    pub display_name: String,
    /// `userId` (identity at the provider).
    pub user_id: Option<String>,
    /// `providerId` (e.g. `AD`, `None`).
    pub provider_id: Option<String>,
}

/// A comment / note (§18.7.3 `comment`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Comment {
    /// Anchor cell (`ref`).
    pub cell: CellRef,
    /// Author name (resolved from `authorId`).
    pub author: String,
    /// Comment text as rich text runs.
    pub text: Vec<TextRun>,
}

/// The scenarios of a worksheet (ECMA-376 Part 1, §18.3.1 `scenarios`, CT_Scenarios).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Scenarios {
    /// Index of the scenario shown last (`current`).
    pub current: Option<u32>,
    /// Index of the scenario selected in the list (`show`).
    pub show: Option<u32>,
    /// Result cells chosen for the summary report (`sqref`).
    pub sqref: Vec<CellRange>,
    /// The scenarios (`scenario`, at least one in a valid file).
    pub items: Vec<Scenario>,
}

/// One named set of input values (CT_Scenario, `scenario`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Scenario {
    /// `name` (required).
    pub name: String,
    /// `locked`: cannot be changed while the sheet's scenarios are protected.
    pub locked: bool,
    /// `hidden`: not listed while the sheet's scenarios are protected.
    pub hidden: bool,
    /// `user`: who last changed it.
    pub user: Option<String>,
    /// `comment`.
    pub comment: Option<String>,
    /// The changing cells with their values (`inputCells`, at least one).
    pub cells: Vec<ScenarioCell>,
}

/// A changing cell of a scenario (CT_InputCells, `inputCells`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ScenarioCell {
    /// `r`: the cell.
    pub cell: CellRef,
    /// `val`: the value as text (a number in invariant form, or a string).
    pub value: String,
    /// `deleted`: the cell was deleted.
    pub deleted: bool,
    /// `undone`: the change was undone.
    pub undone: bool,
    /// `numFmtId`: number format of the value.
    pub num_fmt_id: Option<u32>,
}

/// Data validation (§18.3.1.32 `dataValidation`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DataValidation {
    /// Cells the rule applies to (`sqref`).
    pub sqref: Vec<CellRange>,
    /// `type` (§18.18.21): none, whole, decimal, list, date, time, textLength, custom.
    pub kind: Option<String>,
    /// `operator` (§18.18.20).
    pub operator: Option<String>,
    /// `allowBlank`: empty cells pass validation.
    pub allow_blank: bool,
    /// `showInputMessage`: show the prompt when the cell is selected.
    pub show_input_message: bool,
    /// `showErrorMessage`: show the error alert on invalid input.
    pub show_error_message: bool,
    /// `showDropDown` (note: true *hides* the in-cell dropdown, per spec).
    pub show_drop_down: bool,
    /// `errorStyle`: stop, warning or information.
    pub error_style: Option<String>,
    /// `errorTitle`.
    pub error_title: Option<String>,
    /// `error`: error alert message.
    pub error: Option<String>,
    /// `promptTitle`.
    pub prompt_title: Option<String>,
    /// `prompt`: input message.
    pub prompt: Option<String>,
    /// `formula1`: first criterion (list source, minimum, …), without `=`.
    pub formula1: Option<String>,
    /// `formula2`: second criterion (maximum for between / notBetween).
    pub formula2: Option<String>,
    /// Other attributes (imeMode, xr:uid, …).
    pub attrs: Vec<RawAttr>,
}

/// A `conditionalFormatting` block (§18.3.1.18).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ConditionalFormat {
    /// Cells the block applies to (`sqref`).
    pub sqref: Vec<CellRange>,
    /// `pivot`: the formatting belongs to a pivot table.
    pub pivot: bool,
    /// Rules of the block (`cfRule`).
    pub rules: Vec<CfRule>,
    /// The block is stored only in the worksheet extension
    /// `x14:conditionalFormattings` ([MS-XLSX] x14 `CT_ConditionalFormattings`,
    /// ext uri `{78C0D931-6437-407d-A8EE-F0AAD7539E65}`): rules the 2006 schema cannot
    /// express (icon sets 3Stars / 3Triangles / 5Boxes, custom icons, `x14:dxf`, …).
    #[serde(default)]
    pub ext: bool,
}

/// A conditional formatting rule (§18.3.1.10 `cfRule`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CfRule {
    /// `type` (§18.18.12): cellIs, expression, colorScale, dataBar, iconSet, top10, …
    pub kind: String,
    /// `priority` (lower = evaluated first).
    pub priority: i32,
    /// `dxfId`: differential format applied when the rule matches.
    pub dxf_id: Option<u32>,
    /// `operator` (§18.18.15) for `cellIs` rules.
    pub operator: Option<String>,
    /// `stopIfTrue`: lower-priority rules are skipped when this one matches.
    pub stop_if_true: bool,
    /// `text` for text rules (containsText, beginsWith, …).
    pub text: Option<String>,
    /// `formula` children (§18.3.1.43).
    pub formulas: Vec<String>,
    /// `colorScale` of a colour scale rule.
    pub color_scale: Option<ColorScale>,
    /// `dataBar` of a data bar rule.
    pub data_bar: Option<DataBar>,
    /// `iconSet` of an icon set rule.
    pub icon_set: Option<IconSet>,
    /// Other attributes (rank, percent, bottom, aboveAverage, timePeriod, …).
    pub attrs: Vec<RawAttr>,
    /// Unknown children (extLst).
    pub unknown: Vec<RawXml>,
    /// `id` of the matching `x14:cfRule` ([MS-XLSX] `CT_CfRule/@id`, a GUID), referenced
    /// from `cfRule/extLst/ext/x14:id` for rules that also have a 2010 extension.
    #[serde(default)]
    pub ext_id: Option<String>,
}

impl CfRule {
    /// Icon set names only defined by the 2010 extension ([MS-XLSX] x14 `ST_IconSetType`).
    pub const X14_ICON_SETS: [&'static str; 4] = ["3Stars", "3Triangles", "5Boxes", "NoIcons"];

    /// The rule cannot be written as a 2006 `cfRule` (§18.3.1.10) and is stored only in
    /// the `x14:conditionalFormattings` extension: an icon set from the 2010 list,
    /// custom icons (`x14:cfIcon`) or `autoMin` / `autoMax` thresholds.
    #[must_use]
    pub fn needs_ext(&self) -> bool {
        self.icon_set.as_ref().is_some_and(|i| {
            i.custom
                || i.icon_set
                    .as_deref()
                    .is_some_and(|s| Self::X14_ICON_SETS.contains(&s))
                || i.cfvos
                    .iter()
                    .any(|c| matches!(c.kind.as_str(), "autoMin" | "autoMax"))
        })
    }
}

/// Conditional format value object (§18.3.1.11 `cfvo`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Cfvo {
    /// `type` (§18.18.13): num, percent, max, min, formula, percentile.
    pub kind: String,
    /// `val`: the number, percent or formula of the threshold.
    pub val: Option<String>,
    /// `gte` (default true).
    pub gte: Option<bool>,
}

/// Colour scale (§18.3.1.16).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ColorScale {
    /// Thresholds (`cfvo`), two or three.
    pub cfvos: Vec<Cfvo>,
    /// Colours matching the thresholds.
    pub colors: Vec<SheetColor>,
}

/// Data bar (§18.3.1.28).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DataBar {
    /// Minimum and maximum thresholds (`cfvo`).
    pub cfvos: Vec<Cfvo>,
    /// Bar colour.
    pub color: Option<SheetColor>,
    /// `minLength`: shortest bar in percent of the cell width.
    pub min_length: Option<u32>,
    /// `maxLength`: longest bar in percent of the cell width.
    pub max_length: Option<u32>,
    /// `showValue` (default true): also show the cell value.
    pub show_value: Option<bool>,
    /// The 2010 extension (`x14:dataBar`): negative bars, axis, border, gradient.
    #[serde(default)]
    pub x14: Option<DataBarX14>,
}

/// `x14:dataBar` ([MS-XLSX] x14 `CT_DataBar`, extension of §18.3.1.28 `dataBar`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DataBarX14 {
    /// `minLength` (default 10): shortest bar in percent.
    pub min_length: Option<u32>,
    /// `maxLength` (default 90): longest bar in percent.
    pub max_length: Option<u32>,
    /// `border` (default false): draw a border around the bar.
    pub border: bool,
    /// `gradient` (default true): fade the bar fill towards its end.
    pub gradient: Option<bool>,
    /// `direction` (`ST_DataBarDirection`: context, leftToRight, rightToLeft).
    pub direction: Option<String>,
    /// `negativeBarColorSameAsPositive` (default false).
    pub negative_same_color: bool,
    /// `negativeBarBorderColorSameAsPositive` (default true).
    pub negative_same_border: Option<bool>,
    /// `axisPosition` (`ST_DataBarAxisPosition`: automatic, middle, none).
    pub axis_position: Option<String>,
    /// Minimum and maximum thresholds (`x14:cfvo`, types include autoMin / autoMax;
    /// the value is the `xm:f` child).
    pub cfvos: Vec<Cfvo>,
    /// `fillColor`: bar colour of positive values.
    pub fill_color: Option<SheetColor>,
    /// `borderColor`: bar border colour of positive values.
    pub border_color: Option<SheetColor>,
    /// `negativeFillColor`.
    pub negative_fill_color: Option<SheetColor>,
    /// `negativeBorderColor`.
    pub negative_border_color: Option<SheetColor>,
    /// `axisColor`.
    pub axis_color: Option<SheetColor>,
}

/// One icon of a custom icon set (`x14:cfIcon`, [MS-XLSX] `CT_CfIcon`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CfIcon {
    /// `iconSet`: the set the icon is taken from (`NoIcons` = no icon).
    pub set: String,
    /// `iconId`: zero-based index of the icon in that set.
    pub id: u32,
}

/// Icon set (§18.3.1.49).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IconSet {
    /// `iconSet` (§18.18.42), default `3TrafficLights1`.
    pub icon_set: Option<String>,
    /// Thresholds (`cfvo`), one per icon.
    pub cfvos: Vec<Cfvo>,
    /// `showValue` (default true): also show the cell value.
    pub show_value: Option<bool>,
    /// `percent` (default true): thresholds are percentages.
    pub percent: Option<bool>,
    /// `reverse`: icons in reverse order.
    pub reverse: Option<bool>,
    /// `custom` ([MS-XLSX] `x14:iconSet/@custom`): `icons` replace the set's icons.
    #[serde(default)]
    pub custom: bool,
    /// `x14:cfIcon`: icon of each threshold of a custom set.
    #[serde(default)]
    pub icons: Vec<CfIcon>,
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

/// A table / ListObject (§18.5.1.2 `table`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Table {
    /// `id`, unique within the workbook.
    pub id: u32,
    /// `name`.
    pub name: String,
    /// `displayName`: the name used in formulas (structured references).
    pub display_name: String,
    /// `ref`: the table range, header and totals rows included.
    pub range: CellRange,
    /// `headerRowCount` (default 1).
    pub header_row_count: Option<u32>,
    /// `totalsRowCount` (default 0).
    pub totals_row_count: Option<u32>,
    /// `totalsRowShown`.
    pub totals_row_shown: Option<bool>,
    /// `tableColumns/tableColumn`.
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
    /// `id`, unique within the table.
    pub id: u32,
    /// `name`: column header text.
    pub name: String,
    /// `totalsRowFunction` (§18.18.86): sum, average, count, custom, ….
    pub totals_row_function: Option<String>,
    /// `totalsRowLabel`: text shown in the totals row.
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
    /// `name`: table style name, e.g. `TableStyleMedium2`.
    pub name: Option<String>,
    /// `showFirstColumn`.
    pub show_first_column: bool,
    /// `showLastColumn`.
    pub show_last_column: bool,
    /// `showRowStripes`.
    pub show_row_stripes: bool,
    /// `showColumnStripes`.
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
    /// Unknown children (an unparsed sortState §18.3.1.92, extLst), anchored like other raw XML.
    pub unknown: Vec<RawXml>,
    /// `sortState` (§18.3.1.92) the model represents exactly; written in its schema slot.
    pub sort_state: Option<SortState>,
}

/// Sort settings remembered with a range (ECMA-376 Part 1, §18.3.1.92 `sortState`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SortState {
    /// `ref`: the sorted range (without the header).
    pub range: CellRange,
    /// `columnSort`: columns were sorted left to right.
    pub column_sort: bool,
    /// `caseSensitive`.
    pub case_sensitive: bool,
    /// `sortMethod` (§18.18.80 `ST_SortMethod`: `stroke`, `pinYin`, `none`).
    pub sort_method: Option<String>,
    /// `sortCondition` children (§18.3.1.91), most significant first.
    pub conditions: Vec<SortCondition>,
}

/// One sort level (ECMA-376 Part 1, §18.3.1.91 `sortCondition`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SortCondition {
    /// `descending`.
    pub descending: bool,
    /// `sortBy` (§18.18.79 `ST_SortBy`: `value` (default), `cellColor`, `fontColor`, `icon`).
    pub sort_by: Option<String>,
    /// `ref`: the key column (row with `columnSort`) inside the sorted range.
    pub range: CellRange,
    /// `customList`: comma-separated custom sort order.
    pub custom_list: Option<String>,
    /// `dxfId`: differential format holding the colour of a colour sort.
    pub dxf_id: Option<u32>,
    /// `iconSet` (§18.18.42 `ST_IconSetType`) of an icon sort.
    pub icon_set: Option<String>,
    /// `iconId`: zero-based icon of `icon_set`.
    pub icon_id: Option<u32>,
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
    /// The column's filter criterion.
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
    Values {
        /// Shown text values (`filter/@val`).
        values: Vec<String>,
        /// `blank`: also show empty cells.
        blank: bool,
    },
    /// `customFilters` (§18.3.2.3): one or two `customFilter` conditions (§18.3.2.2)
    /// joined with AND (`and = true`) or OR.
    Custom {
        /// `customFilters/@and`: conditions are joined with AND.
        and: bool,
        /// The `customFilter` conditions.
        conditions: Vec<CustomFilter>,
    },
    /// `top10` (§18.3.2.10).
    Top10 {
        /// `top`: top items (false = bottom).
        top: bool,
        /// `percent`: `val` is a percentage of the items.
        percent: bool,
        /// `val`: number (or percent) of items to show.
        val: f64,
        /// `filterVal`: the cut-off value cached by the writing application.
        filter_val: Option<f64>,
    },
    /// `filters` with `dateGroupItem` children (§18.3.2.4): rows whose date falls
    /// into one of `groups`, or whose text is one of `values`.
    DateGroups {
        /// Shown text values (`filter/@val`).
        values: Vec<String>,
        /// `blank`: also show empty cells.
        blank: bool,
        /// Shown date groups (`dateGroupItem`).
        groups: Vec<DateGroup>,
    },
    /// `dynamicFilter` (§18.3.2.5): `kind` is an `ST_DynamicFilterType` (§18.18.26)
    /// such as `today`, `thisMonth`, `lastYear`, `Q1`, `M12`, `aboveAverage`.
    Dynamic {
        /// `type`: the dynamic filter type token.
        kind: String,
        /// `val`: cached reference value (e.g. the average).
        val: Option<f64>,
        /// `maxVal`: cached upper bound of a date-range filter.
        max_val: Option<f64>,
    },
    /// `colorFilter` (§18.3.2.1): cells whose fill (`cell_color`, default) or font
    /// colour is the one of differential format `dxf_id`.
    Color {
        /// `dxfId`: differential format holding the colour.
        dxf_id: Option<u32>,
        /// `cellColor` (default true): match the fill colour instead of the font colour.
        cell_color: bool,
    },
    /// `iconFilter` (§18.3.2.9): cells showing icon `icon_id` of conditional-format
    /// icon set `icon_set`; without `icon_id`, cells showing no icon.
    Icon {
        /// `iconSet` (§18.18.42 `ST_IconSetType`).
        icon_set: String,
        /// `iconId`: zero-based icon of the set; `None` = no icon.
        icon_id: Option<u32>,
    },
    /// Any other criterion (extension elements, unknown attributes…)
    /// kept as the verbatim child element.
    Raw(String),
}

/// A date group of an AutoFilter (§18.3.2.4 `dateGroupItem`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DateGroup {
    /// `year`.
    pub year: u32,
    /// `month` (1–12).
    pub month: Option<u32>,
    /// `day` (1–31).
    pub day: Option<u32>,
    /// `hour` (0–23).
    pub hour: Option<u32>,
    /// `minute` (0–59).
    pub minute: Option<u32>,
    /// `second` (0–59).
    pub second: Option<u32>,
    /// `dateTimeGrouping` (§18.18.20): year, month, day, hour, minute, second.
    pub grouping: String,
}

/// `customFilter` (§18.3.2.2): `operator` (§18.18.31 `ST_FilterOperator`, default
/// `equal`) and `val`, which may use the `*` / `?` wildcards.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CustomFilter {
    /// `operator`; `None` = `equal`.
    pub operator: Option<String>,
    /// `val`: the comparison value.
    pub val: String,
}

// ---------------------------------------------------------------------------
// Charts (read-only view of DrawingML chart parts)
// ---------------------------------------------------------------------------

/// A chart placed on a worksheet, parsed from its drawing (ECMA-376 Part 1,
/// §20.5 SpreadsheetML Drawing) and chart part (§21.2 DrawingML Charts, `c:chartSpace`).
///
/// Charts read from a file keep their drawing and chart parts opaque (written back
/// byte-for-byte). A chart with an empty `part` (inserted by the user) or with
/// `dirty` set (edited) is regenerated as a `c:chartSpace` part on write.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Chart {
    /// Absolute name of the chart part, e.g. `/xl/charts/chart1.xml`; empty for a new chart.
    pub part: String,
    /// Regenerate the chart part on write (title, legend, axis titles or type changed).
    #[serde(default)]
    pub dirty: bool,
    /// Category (horizontal) axis title (§21.2.2.211 `title` of `c:catAx` / `c:valAx`).
    #[serde(default)]
    pub x_title: Option<String>,
    /// Value (vertical) axis title.
    #[serde(default)]
    pub y_title: Option<String>,
    /// Anchor cells (zero-based) of the drawing anchor (§20.5.2.33 twoCellAnchor).
    pub from: CellRef,
    /// Bottom-right anchor cell (zero-based); `None` for one-cell and absolute anchors.
    pub to: Option<CellRef>,
    /// Title text (rich text runs concatenated, or the cached `strRef` value).
    pub title: Option<String>,
    /// Plot groups inside `c:plotArea` (several = combo chart).
    pub plots: Vec<ChartPlot>,
    /// `c:legend/c:legendPos` (§21.2.2.94): `r`, `l`, `t`, `b`, `tr`; `None` = no legend.
    pub legend: Option<String>,
    /// Kind of the drawing anchor holding the chart (§20.5.2.33 twoCellAnchor,
    /// §20.5.2.24 oneCellAnchor, §20.5.2.1 absoluteAnchor).
    #[serde(default)]
    pub anchor: ChartAnchor,
    /// `xdr:pos` of an absolute anchor (§20.5.2.25), EMU `(x, y)`.
    #[serde(default)]
    pub pos: Option<(i64, i64)>,
    /// `xdr:ext` of a one-cell or absolute anchor (§20.5.2.14), EMU `(cx, cy)`.
    #[serde(default)]
    pub ext: Option<(i64, i64)>,
    /// A pivot chart: `c:pivotSource/c:name` (§21.2.2.142), the pivot table the
    /// chart plots, as `[file]Sheet!PivotName` or `Sheet!PivotName`.
    #[serde(default)]
    pub pivot_source: Option<String>,
    /// `c:pivotSource/c:fmtId` (§21.2.2.73): the `chartFormat` of the pivot table.
    #[serde(default)]
    pub pivot_fmt_id: u32,
    /// Show the pivot field buttons on the chart ([MS-ODRAWXML] `c14:pivotOptions`).
    #[serde(default)]
    pub pivot_buttons: bool,
    /// Axes and table of the plot area (CORE-CHART-005).
    #[serde(default)]
    pub axes: ChartAxes,
    /// Style, colours and quick-layout formatting (CORE-CHART-007).
    #[serde(default)]
    pub format: crate::chart_style::ChartFormat,
}

/// Value / category axis options and the data table of a chart (ECMA-376 Part 1, §21.2.2).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ChartAxes {
    /// §21.2.2.100 logBase of the (primary) value axis.
    #[serde(default)]
    pub log_base: Option<f64>,
    /// §21.2.2.111 min / §21.2.2.106 max of the value axis.
    #[serde(default)]
    pub min: Option<f64>,
    /// Maximum of the value axis.
    #[serde(default)]
    pub max: Option<f64>,
    /// The category axis is a date axis (§21.2.2.39 dateAx): serial-number categories shown as dates.
    #[serde(default)]
    pub date: bool,
    /// §21.2.2.54 dTable: a data table under the plot.
    #[serde(default)]
    pub data_table: bool,
    /// §21.2.2.228 view3D of a 3-D chart.
    #[serde(default)]
    pub view3d: Option<View3D>,
}

/// §21.2.2.228 view3D: how a 3-D chart is seen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct View3D {
    /// §21.2.2.158 rotX: elevation in degrees (−90..=90).
    pub rot_x: i32,
    /// §21.2.2.159 rotY: turn about the vertical axis in degrees (0..=360).
    pub rot_y: u32,
    /// §21.2.2.43 depthPercent: depth as a percentage of the width (20..=2000).
    pub depth_percent: u32,
    /// §21.2.2.79 hPercent: height as a percentage of the width (5..=500); `None` = automatic.
    pub h_percent: Option<u32>,
    /// §21.2.2.157 rAngAx: right-angle axes (oblique drawing, no perspective).
    pub r_ang_ax: bool,
    /// §21.2.2.135 perspective: field of view (0..=240; 30 by default).
    pub perspective: u32,
}

impl Default for View3D {
    /// The defaults of the schema (rotX 0, rotY 0, depth 100 %, perspective 30, rAngAx on); a new 3-D
    /// chart uses [`View3D::new_chart`].
    fn default() -> Self {
        View3D {
            rot_x: 0,
            rot_y: 0,
            depth_percent: 100,
            h_percent: None,
            r_ang_ax: true,
            perspective: 30,
        }
    }
}

impl View3D {
    /// The view of a new 3-D chart: seen 15° from above and turned 20°.
    pub fn new_chart(pie: bool) -> Self {
        View3D {
            rot_x: if pie { 30 } else { 15 },
            rot_y: if pie { 0 } else { 20 },
            r_ang_ax: !pie,
            ..View3D::default()
        }
    }
}

/// §21.2.2.211 trendline.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Trendline {
    /// §21.2.2.212 trendlineType: `linear`, `exp`, `log`, `poly`, `power`, `movingAvg`.
    pub kind: String,
    /// Polynomial order (§21.2.2.130) or moving-average period (§21.2.2.136).
    pub order: u32,
    /// §21.2.2.51 dispEq.
    pub disp_eq: bool,
    /// §21.2.2.50 dispRSqr.
    pub disp_r2: bool,
}

/// §21.2.2.58 errBars.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ErrorBars {
    /// §21.2.2.58 errBarType: `both`, `plus`, `minus`.
    pub kind: String,
    /// §21.2.2.59 errValType: `fixedVal`, `percentage`, `stdDev`, `stdErr`, `cust`.
    pub value_type: String,
    /// §21.2.2.229 val.
    pub value: f64,
}

/// A SmartArt graphic frame on a sheet (`xdr:graphicFrame` with
/// `dgm:relIds`, ECMA-376 Part 1, §20.5.2.16 and §21.4.2.6). The pre-rendered drawing
/// part ([MS-ODRAWXML] `dsp:drawing`) named here is drawn over the cells; without one,
/// `part` is the data model part and the diagram is laid out from it. The drawing and
/// diagram parts stay verbatim in the package.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetDiagram {
    /// Anchor cells (zero-based) of the drawing anchor (§20.5.2.33 twoCellAnchor).
    pub from: CellRef,
    /// End anchor cell (`None`: one-cell anchor).
    pub to: Option<CellRef>,
    /// Absolute part name of the diagram drawing part (or of the data model part).
    pub part: String,
    /// `cNvPr@name` of the frame.
    pub name: String,
}

/// A slicer: a box of item buttons that filters a pivot table field or a table column
/// ([MS-XLSX] 2.4.x `x14:slicer` in a slicers part, its `slicerCacheDefinition`, and the
/// drawing frame `sle:slicer` that places it).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slicer {
    /// `slicer/@name` (also the drawing frame's `sle:slicer/@name`).
    pub name: String,
    /// `slicer/@caption`, the header text.
    pub caption: String,
    /// `slicer/@cache`: the slicer cache's name (a defined name of the workbook).
    pub cache: String,
    /// What the slicer filters.
    pub source: SlicerSource,
    /// `slicer/@columnCount` (default 1).
    pub columns: u32,
    /// `slicer/@style`.
    pub style: Option<String>,
    /// `slicer/@rowHeight` in EMU.
    pub row_height: Option<i64>,
    /// Anchor cells of the drawing frame (§20.5.2.33).
    pub from: CellRef,
    /// End anchor cell.
    pub to: Option<CellRef>,
    /// Absolute part name of the slicer cache definition (empty for a new slicer).
    pub cache_part: String,
    /// Absolute part name of the slicers part holding this slicer (empty for a new slicer).
    pub part: String,
    /// The selection changed since the file was read: the cache part is rewritten on save.
    pub dirty: bool,
}

/// A timeline: a date filter over a pivot table's date field, shown as a strip of periods
/// ([MS-XLSX] `x15:timeline` in a timelines part, its `x15:timelineCacheDefinition` and the
/// drawing frame `tsle:timeslicer`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    /// `timeline/@name`.
    pub name: String,
    /// `timeline/@caption`.
    pub caption: String,
    /// `timeline/@cache`: the timeline cache's name.
    pub cache: String,
    /// `timelineCacheDefinition/@sourceName`: the pivot cache date field.
    pub field: String,
    /// Connected pivot tables (`pivotTables/pivotTable/@name`).
    pub pivots: Vec<String>,
    /// `timeline/@level`: 0 years, 1 quarters, 2 months, 3 days.
    pub level: u32,
    /// `state/selection/@startDate` (ISO date-time), `None` when nothing is filtered.
    pub start: Option<String>,
    /// `state/selection/@endDate`.
    pub end: Option<String>,
    /// Anchor cells of the drawing frame.
    pub from: CellRef,
    /// End anchor cell.
    pub to: Option<CellRef>,
    /// Absolute part name of the timeline cache definition.
    pub cache_part: String,
    /// Absolute part name of the timelines part.
    pub part: String,
    /// The selection changed since the file was read.
    pub dirty: bool,
}

/// Kind of a sparkline group ([MS-XLSX] `ST_SparklineType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SparklineType {
    /// `line` (the schema default).
    #[default]
    Line,
    /// `column`.
    Column,
    /// `stacked`: win/loss bars.
    Stacked,
}

/// How empty cells are drawn ([MS-XLSX] `displayEmptyCellsAs`, ECMA-376 Part 1, §21.2.3.10
/// `ST_DispBlanksAs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SparklineEmpty {
    /// `gap`: leave a gap.
    Gap,
    /// `zero`: treat as zero (the schema default).
    #[default]
    Zero,
    /// `span`: connect the neighbouring points.
    Span,
}

/// Vertical axis bound of a sparkline group ([MS-XLSX] `ST_SparklineAxisMinMax`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SparklineAxis {
    /// `individual`: each sparkline uses its own data (the schema default).
    #[default]
    Individual,
    /// `group`: the same bound for every sparkline of the group.
    Group,
    /// `custom`: `manualMin` / `manualMax`.
    Custom,
}

/// Colours of a sparkline group ([MS-XLSX] `x14:colorSeries` … `x14:colorLow`, `CT_Color`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SparklineColors {
    /// `colorSeries`: line / bars.
    pub series: Option<SheetColor>,
    /// `colorNegative`: negative points.
    pub negative: Option<SheetColor>,
    /// `colorAxis`: horizontal axis.
    pub axis: Option<SheetColor>,
    /// `colorMarkers`: line markers.
    pub markers: Option<SheetColor>,
    /// `colorFirst`.
    pub first: Option<SheetColor>,
    /// `colorLast`.
    pub last: Option<SheetColor>,
    /// `colorHigh`.
    pub high: Option<SheetColor>,
    /// `colorLow`.
    pub low: Option<SheetColor>,
}

/// One sparkline ([MS-XLSX] `x14:sparkline`): its data (`xm:f`) drawn in one cell (`xm:sqref`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sparkline {
    /// `xm:f`: the data range (e.g. `Sheet1!A2:D2`); `None` draws nothing.
    pub formula: Option<String>,
    /// `xm:sqref`: the cell the sparkline is drawn in.
    pub location: CellRef,
}

/// A group of sparklines sharing their settings ([MS-XLSX] `x14:sparklineGroup`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SparklineGroup {
    /// `type`.
    #[serde(rename = "type")]
    pub kind: SparklineType,
    /// `x14:sparklines`.
    pub sparklines: Vec<Sparkline>,
    /// `displayEmptyCellsAs`.
    pub empty_as: SparklineEmpty,
    /// `markers`: a marker on every point (line).
    pub markers: bool,
    /// `high`: highlight the highest point.
    pub high: bool,
    /// `low`: highlight the lowest point.
    pub low: bool,
    /// `first`: highlight the first point.
    pub first: bool,
    /// `last`: highlight the last point.
    pub last: bool,
    /// `negative`: highlight negative points.
    pub negative: bool,
    /// `displayXAxis`: draw the horizontal axis.
    pub display_x_axis: bool,
    /// `displayHidden`: include data in hidden rows and columns.
    pub display_hidden: bool,
    /// `rightToLeft`: plot the points from right to left.
    pub right_to_left: bool,
    /// `minAxisType`.
    pub min_axis: SparklineAxis,
    /// `maxAxisType`.
    pub max_axis: SparklineAxis,
    /// `manualMin` (used with [`SparklineAxis::Custom`]).
    pub manual_min: Option<f64>,
    /// `manualMax` (used with [`SparklineAxis::Custom`]).
    pub manual_max: Option<f64>,
    /// `lineWeight` in points (schema default 0.75).
    pub line_weight: Option<f64>,
    /// `dateAxis`: place the points by the dates in [`Self::date_range`].
    pub date_axis: bool,
    /// Group `xm:f`: the date range of a date axis.
    pub date_range: Option<String>,
    /// Colours.
    pub colors: SparklineColors,
    /// Other attributes of `sparklineGroup` (e.g. `xr2:uid`), kept as read.
    pub attrs: Vec<RawAttr>,
}

/// The data a slicer filters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SlicerSource {
    /// A field of one or more pivot tables sharing a pivot cache.
    Pivot {
        /// `slicerCacheDefinition/@sourceName`: the pivot cache field's name.
        field: String,
        /// `pivotTables/pivotTable/@name` connected to the slicer.
        pivots: Vec<String>,
        /// `tabular/@pivotCacheId`.
        cache_id: u32,
        /// `tabular/items/i`: shared item index and whether it is selected (`s`).
        items: Vec<(u32, bool)>,
    },
    /// A column of a table (`x15:tableSlicerCache`).
    Table {
        /// `tableSlicerCache/@tableId`.
        table_id: u32,
        /// `tableSlicerCache/@column`: 1-based table column id.
        column: u32,
        /// Values the table's filter shows for the column (`None`: not filtered, all shown).
        shown: Option<Vec<String>>,
    },
}

impl Default for SlicerSource {
    fn default() -> Self {
        SlicerSource::Table {
            table_id: 0,
            column: 1,
            shown: None,
        }
    }
}

/// Drawing anchor of a chart (ECMA-376 Part 1, §20.5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChartAnchor {
    /// §20.5.2.33: moves and sizes with the cells.
    #[default]
    TwoCell,
    /// §20.5.2.24: the top-left follows a cell, the size is fixed (`ext`).
    OneCell,
    /// §20.5.2.1: position and size in EMU (`pos`, `ext`).
    Absolute,
}

impl Chart {
    /// The sheet and pivot table names of [`Chart::pivot_source`]
    /// (`[Book1.xlsx]Sheet 1!PivotTable1` → `("Sheet 1", "PivotTable1")`).
    pub fn pivot_ref(&self) -> Option<(String, String)> {
        let src = self.pivot_source.as_deref()?.trim();
        let (sheet, name) = src.rsplit_once('!')?;
        let sheet = sheet.trim().trim_matches('\'');
        let sheet = match sheet.strip_prefix('[') {
            Some(rest) => rest.split_once(']').map_or(rest, |(_, r)| r),
            None => sheet,
        };
        Some((sheet.replace("''", "'"), name.trim().to_owned()))
    }
}

/// Chart family (`c:barChart` §21.2.2.16, `c:lineChart` §21.2.2.97, …).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ChartKind {
    /// Bar/column; `c:barDir` `col` = vertical columns, `bar` = horizontal bars.
    #[default]
    Bar,
    /// Line chart (`c:lineChart`).
    Line,
    /// Area chart (`c:areaChart`).
    Area,
    /// Pie chart (`c:pieChart`).
    Pie,
    /// Doughnut chart (`c:doughnutChart`).
    Doughnut,
    /// XY scatter chart (`c:scatterChart`).
    Scatter,
    /// Radar chart (`c:radarChart`).
    Radar,
    /// Bubble chart (`c:bubbleChart`).
    Bubble,
    /// Stock chart (`c:stockChart`, §21.2.2.198): high-low(-close) series, optional up/down bars.
    Stock,
    /// Surface or contour chart (`c:surfaceChart` / `c:surface3DChart`, §21.2.2.203).
    Surface,
    /// Pie of pie / bar of pie (`c:ofPieChart`, §21.2.2.126).
    OfPie,
    /// Chartex waterfall ([MS-ODRAWXML] 2.24, `cx:series@layoutId="waterfall"`).
    Waterfall,
    /// Chartex treemap (`treemap`): hierarchical categories, sizes.
    Treemap,
    /// Chartex sunburst (`sunburst`).
    Sunburst,
    /// Chartex histogram (`clusteredColumn` with `cx:binning`).
    Histogram,
    /// Chartex pareto (histogram / sorted columns with the `paretoLine` series).
    Pareto,
    /// Chartex box & whisker (`boxWhisker`).
    BoxWhisker,
    /// Chartex funnel (`funnel`).
    Funnel,
    /// Any other plot element, by local name.
    Other(String),
}

impl ChartKind {
    /// Stored in a chartex part ([MS-ODRAWXML] `cx:chartSpace`) rather than a `c:chartSpace`.
    #[must_use]
    pub fn is_ex(&self) -> bool {
        matches!(
            self,
            ChartKind::Waterfall
                | ChartKind::Treemap
                | ChartKind::Sunburst
                | ChartKind::Histogram
                | ChartKind::Pareto
                | ChartKind::BoxWhisker
                | ChartKind::Funnel
        )
    }
}

/// A colour of a chart element (§20.1.2.3.32 srgbClr / §20.1.2.3.29 schemeClr), resolved by the host
/// (a theme colour needs the document theme).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartColor {
    /// sRGB `RRGGBB`.
    Rgb(u32),
    /// Theme colour name (`accent1` …).
    Scheme(String),
}

/// One plot group of a chart.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ChartPlot {
    /// Plot element type.
    pub kind: ChartKind,
    /// `c:barDir` for bar charts (`bar` / `col`).
    pub bar_dir: Option<String>,
    /// `c:grouping` (§21.2.2.76): clustered, stacked, percentStacked, standard.
    pub grouping: Option<String>,
    /// Data series of the group (`c:ser`).
    pub series: Vec<ChartSeries>,
    /// §21.2.2.227 varyColors: one colour per data point.
    #[serde(default)]
    pub vary_colors: bool,
    /// §21.2.2.156 radarStyle (`standard` | `marker` | `filled`) or §21.2.2.127 ofPieType (`pie` | `bar`).
    #[serde(default)]
    pub style: Option<String>,
    /// Stock chart up/down bars (§21.2.2.218 upDownBars); surface chart wireframe (§21.2.2.230).
    #[serde(default)]
    pub flag: bool,
    /// §21.2.2.53 dropLines (line / area charts).
    #[serde(default)]
    pub drop_lines: bool,
    /// §21.2.2.80 hiLowLines (line charts).
    #[serde(default)]
    pub hi_low_lines: bool,
    /// The group is plotted on the secondary value axis (the second `c:valAx`).
    #[serde(default)]
    pub secondary: bool,
    /// A 3-D chart group (`c:bar3DChart`, `c:line3DChart`, `c:area3DChart`, `c:pie3DChart`,
    /// `c:surface3DChart`); a bar group keeps its §21.2.2.177 shape in `style`.
    #[serde(default)]
    pub three_d: bool,
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
    /// Bubble sizes (`c:bubbleSize`, §21.2.2.22), cached.
    #[serde(default)]
    pub sizes: Vec<Option<f64>>,
    /// Bubble size reference formula.
    #[serde(default)]
    pub size_ref: Option<String>,
    /// Series fill (`c:spPr/a:solidFill`).
    #[serde(default)]
    pub fill: Option<ChartColor>,
    /// Series line (`c:spPr/a:ln/a:solidFill`).
    #[serde(default)]
    pub line: Option<ChartColor>,
    /// Data point fills (`c:dPt`, §21.2.2.52): (point index, colour).
    #[serde(default)]
    pub point_fills: Vec<(usize, ChartColor)>,
    /// §21.2.2.65 explosion of pie slices (percent of the radius).
    #[serde(default)]
    pub explosion: u32,
    /// Hierarchical category labels (chartex `cx:strDim` levels): `levels[0]` the innermost (leaf)
    /// level, then the parents outwards; one label per point.
    #[serde(default)]
    pub levels: Vec<Vec<String>>,
    /// Waterfall points drawn as totals (`cx:subtotals/cx:idx`).
    #[serde(default)]
    pub subtotals: Vec<usize>,
    /// Trend lines of the series.
    #[serde(default)]
    pub trendlines: Vec<Trendline>,
    /// Error bars (value direction).
    #[serde(default)]
    pub err_bars: Option<ErrorBars>,
}

// ---------------------------------------------------------------------------
// PivotTables (summary of the pivot parts, ECMA-376 Part 1, §18.10)
// ---------------------------------------------------------------------------

/// A pivot cache (§18.10.1.67 `pivotCacheDefinition`, referenced from the
/// workbook's `pivotCaches/pivotCache` §18.2.21).
///
/// Caches read from a file keep their definition and records parts opaque
/// (written back byte-for-byte). A cache with an empty `part` (created by the
/// user) or with `dirty` set (refreshed) is regenerated from [`PivotCache::fields`]
/// and [`PivotCache::records`] on write.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PivotCache {
    /// `pivotCache/@cacheId`, unique in the workbook.
    pub cache_id: u32,
    /// Workbook relationship id (`pivotCache/@r:id`); empty for a new cache.
    pub rel_id: String,
    /// Absolute name of the definition part; empty for a new cache.
    pub part: String,
    /// Absolute name of the records part (§18.10.1.68), if any.
    pub records_part: Option<String>,
    /// `worksheetSource/@sheet` (§18.10.1 `cacheSource/worksheetSource`).
    pub source_sheet: Option<String>,
    /// `worksheetSource/@ref`: the source range including the header row.
    pub source_ref: Option<String>,
    /// `worksheetSource/@name`: a defined name or table used as the source.
    pub source_name: Option<String>,
    /// `cacheFields/cacheField` (§18.10.1.3) with their shared items.
    pub fields: Vec<PivotCacheField>,
    /// Source records (one value per field) of a new or refreshed cache; empty
    /// for caches read from a file (their records part stays opaque).
    pub records: Vec<Vec<CellValue>>,
    /// Regenerate the cache parts on write.
    pub dirty: bool,
    /// `calculatedItems` (§18.10.1.9) of the cache definition.
    #[serde(default)]
    pub calculated_items: Vec<PivotCalcItem>,
}

/// A cache field (§18.10.1.3 `cacheField`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotCacheField {
    /// `name`: the field caption (source column header).
    pub name: String,
    /// Shared items (§18.10.1.90 `sharedItems`) in file order; empty when the
    /// field's values are stored inline in the records.
    pub items: Vec<CellValue>,
    /// `fieldGroup` (§18.10.1.27): date / number range or manual grouping.
    pub group: Option<PivotFieldGroup>,
    /// `formula` of a calculated field (§18.10.1.3 `cacheField/@formula`,
    /// without `=`), evaluated over the sums of the fields it names.
    pub formula: Option<String>,
    /// `databaseField="0"`: the field has no column in the records (a
    /// calculated field or a grouping field built on another field).
    pub derived: bool,
    /// `numFmtId` (§18.10.1.3), `0` = General.
    pub num_fmt_id: u32,
}

/// `ST_GroupBy` (§18.18.36): what a range group divides its base values by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PivotGroupBy {
    /// Numeric ranges of `groupInterval`.
    #[default]
    Range,
    /// Group date/times by second.
    Seconds,
    /// Group date/times by minute.
    Minutes,
    /// Group date/times by hour.
    Hours,
    /// Group dates by day.
    Days,
    /// Group dates by month.
    Months,
    /// Group dates by quarter.
    Quarters,
    /// Group dates by year.
    Years,
}

impl PivotGroupBy {
    /// The `ST_GroupBy` token.
    pub fn as_str(self) -> &'static str {
        match self {
            PivotGroupBy::Range => "range",
            PivotGroupBy::Seconds => "seconds",
            PivotGroupBy::Minutes => "minutes",
            PivotGroupBy::Hours => "hours",
            PivotGroupBy::Days => "days",
            PivotGroupBy::Months => "months",
            PivotGroupBy::Quarters => "quarters",
            PivotGroupBy::Years => "years",
        }
    }

    /// Parse a token (unknown → `range`, the default).
    pub fn parse(s: &str) -> PivotGroupBy {
        match s {
            "seconds" => PivotGroupBy::Seconds,
            "minutes" => PivotGroupBy::Minutes,
            "hours" => PivotGroupBy::Hours,
            "days" => PivotGroupBy::Days,
            "months" => PivotGroupBy::Months,
            "quarters" => PivotGroupBy::Quarters,
            "years" => PivotGroupBy::Years,
            _ => PivotGroupBy::Range,
        }
    }
}

/// `rangePr` (§18.10.1.77): grouping of numbers or dates into ranges.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotRangeGroup {
    /// `groupBy`: range size unit.
    pub group_by: PivotGroupBy,
    /// `autoStart` / `autoEnd` (default true): bounds follow the data on refresh.
    pub auto_start: bool,
    /// See [`PivotRangeGroup::auto_start`].
    pub auto_end: bool,
    /// `startNum` / `endNum`, or the serial of `startDate` / `endDate` when
    /// [`PivotRangeGroup::dates`] is set.
    pub start: f64,
    /// Upper bound of the grouped range (see [`PivotRangeGroup::start`]).
    pub end: f64,
    /// `groupInterval` (default 1).
    pub interval: f64,
    /// The bounds are written as `startDate` / `endDate` (date grouping).
    pub dates: bool,
}

/// `fieldGroup` (§18.10.1.27) of a cache field.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotFieldGroup {
    /// `par`: the next coarser grouping field (e.g. Years above Months).
    pub par: Option<u32>,
    /// `base`: the field whose values are grouped (itself for the base field).
    pub base: Option<u32>,
    /// `rangePr` (§18.10.1.77).
    pub range: Option<PivotRangeGroup>,
    /// `discretePr` (§18.10.1.26): group index of every base item (manual grouping).
    pub discrete: Vec<u32>,
    /// `groupItems` (§18.10.1.37): the group captions.
    pub items: Vec<CellValue>,
}

/// Grouping key of a pivot item: values that compare equal (numbers by value,
/// text case-insensitively) share one item (§18.10.1.90 `sharedItems`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PivotItemKey {
    /// A number, by its bit pattern (`-0.0` and `0.0` share a key).
    Number(u64),
    /// Text, lower-cased.
    Text(String),
    /// A boolean.
    Bool(bool),
    /// An error literal.
    Error(String),
    /// An empty value.
    Blank,
}

impl PivotItemKey {
    /// Key of a cell value (rich text and dates by their text).
    pub fn of(v: &CellValue) -> PivotItemKey {
        match v {
            CellValue::Empty => PivotItemKey::Blank,
            CellValue::Number(n) => PivotItemKey::Number(if *n == 0.0 { 0 } else { n.to_bits() }),
            CellValue::Bool(b) => PivotItemKey::Bool(*b),
            CellValue::Error(e) => PivotItemKey::Error(e.as_str().to_owned()),
            CellValue::Date(d) => PivotItemKey::Text(d.to_lowercase()),
            other => PivotItemKey::Text(other.as_text().unwrap_or_default().to_lowercase()),
        }
    }
}

/// Display order of pivot items: numbers ascending, then text (case-insensitive),
/// booleans, errors and blanks last.
pub fn pivot_item_cmp(a: &CellValue, b: &CellValue) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let rank = |k: &PivotItemKey| match k {
        PivotItemKey::Number(_) => 0,
        PivotItemKey::Text(_) => 1,
        PivotItemKey::Bool(_) => 2,
        PivotItemKey::Error(_) => 3,
        PivotItemKey::Blank => 4,
    };
    let (ka, kb) = (PivotItemKey::of(a), PivotItemKey::of(b));
    match (&ka, &kb, a, b) {
        (
            PivotItemKey::Number(_),
            PivotItemKey::Number(_),
            CellValue::Number(x),
            CellValue::Number(y),
        ) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (PivotItemKey::Text(x), PivotItemKey::Text(y), ..)
        | (PivotItemKey::Error(x), PivotItemKey::Error(y), ..) => x.cmp(y),
        (PivotItemKey::Bool(x), PivotItemKey::Bool(y), ..) => x.cmp(y),
        _ => rank(&ka).cmp(&rank(&kb)),
    }
}

/// Aggregation of a data field (§18.18.17 `ST_DataConsolidateFunction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PivotFunction {
    /// Sum of the values.
    #[default]
    Sum,
    /// Number of non-empty values.
    Count,
    /// Arithmetic mean.
    Average,
    /// Largest value.
    Max,
    /// Smallest value.
    Min,
    /// Product of the values.
    Product,
    /// Number of numeric values.
    CountNums,
    /// Sample standard deviation.
    StdDev,
    /// Population standard deviation.
    StdDevp,
    /// Sample variance.
    Var,
    /// Population variance.
    Varp,
}

impl PivotFunction {
    /// The `ST_DataConsolidateFunction` token.
    pub fn as_str(self) -> &'static str {
        match self {
            PivotFunction::Sum => "sum",
            PivotFunction::Count => "count",
            PivotFunction::Average => "average",
            PivotFunction::Max => "max",
            PivotFunction::Min => "min",
            PivotFunction::Product => "product",
            PivotFunction::CountNums => "countNums",
            PivotFunction::StdDev => "stdDev",
            PivotFunction::StdDevp => "stdDevp",
            PivotFunction::Var => "var",
            PivotFunction::Varp => "varp",
        }
    }

    /// Parse a token (unknown tokens fall back to `sum`, the default).
    pub fn parse(s: &str) -> PivotFunction {
        match s {
            "count" => PivotFunction::Count,
            "average" => PivotFunction::Average,
            "max" => PivotFunction::Max,
            "min" => PivotFunction::Min,
            "product" => PivotFunction::Product,
            "countNums" => PivotFunction::CountNums,
            "stdDev" => PivotFunction::StdDev,
            "stdDevp" => PivotFunction::StdDevp,
            "var" => PivotFunction::Var,
            "varp" => PivotFunction::Varp,
            _ => PivotFunction::Sum,
        }
    }
}

/// "Show values as" of a data field: `ST_ShowDataAs` (§18.18.70) plus the
/// later calculations of `x14:dataField/@pivotShowAs` ([MS-XLSX] `x14:dataField`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PivotShowAs {
    /// The value itself (no calculation).
    #[default]
    Normal,
    /// Value minus the base item's value.
    Difference,
    /// Value / base item's value.
    Percent,
    /// (Value − base) / base.
    PercentDiff,
    /// Running total along the base field.
    RunTotal,
    /// Value / row grand total.
    PercentOfRow,
    /// Value / column grand total.
    PercentOfCol,
    /// Value / grand total.
    PercentOfTotal,
    /// (Value × grand total) / (row total × column total).
    Index,
    /// Value / parent row item's value (x14).
    PercentOfParentRow,
    /// Value / parent column item's value (x14).
    PercentOfParentCol,
    /// Value / the value of the enclosing base field item (x14).
    PercentOfParent,
    /// Running total / total along the base field (x14).
    PercentOfRunningTotal,
    /// Rank smallest-first among the base field's items (x14).
    RankAscending,
    /// Rank largest-first (x14).
    RankDescending,
}

impl PivotShowAs {
    /// The token (`ST_ShowDataAs` or `x14` `ST_PivotShowAs`).
    pub fn as_str(self) -> &'static str {
        match self {
            PivotShowAs::Normal => "normal",
            PivotShowAs::Difference => "difference",
            PivotShowAs::Percent => "percent",
            PivotShowAs::PercentDiff => "percentDiff",
            PivotShowAs::RunTotal => "runTotal",
            PivotShowAs::PercentOfRow => "percentOfRow",
            PivotShowAs::PercentOfCol => "percentOfCol",
            PivotShowAs::PercentOfTotal => "percentOfTotal",
            PivotShowAs::Index => "index",
            PivotShowAs::PercentOfParentRow => "percentOfParentRow",
            PivotShowAs::PercentOfParentCol => "percentOfParentCol",
            PivotShowAs::PercentOfParent => "percentOfParent",
            PivotShowAs::PercentOfRunningTotal => "percentOfRunningTotal",
            PivotShowAs::RankAscending => "rankAscending",
            PivotShowAs::RankDescending => "rankDescending",
        }
    }

    /// Parse a token (unknown → `normal`).
    pub fn parse(s: &str) -> PivotShowAs {
        match s {
            "difference" => PivotShowAs::Difference,
            "percent" => PivotShowAs::Percent,
            "percentDiff" => PivotShowAs::PercentDiff,
            "runTotal" => PivotShowAs::RunTotal,
            "percentOfRow" => PivotShowAs::PercentOfRow,
            "percentOfCol" => PivotShowAs::PercentOfCol,
            "percentOfTotal" => PivotShowAs::PercentOfTotal,
            "index" => PivotShowAs::Index,
            "percentOfParentRow" => PivotShowAs::PercentOfParentRow,
            "percentOfParentCol" => PivotShowAs::PercentOfParentCol,
            "percentOfParent" => PivotShowAs::PercentOfParent,
            "percentOfRunningTotal" => PivotShowAs::PercentOfRunningTotal,
            "rankAscending" => PivotShowAs::RankAscending,
            "rankDescending" => PivotShowAs::RankDescending,
            _ => PivotShowAs::Normal,
        }
    }

    /// Only expressible through the `x14:dataField` extension.
    pub fn is_x14(self) -> bool {
        matches!(
            self,
            PivotShowAs::PercentOfParentRow
                | PivotShowAs::PercentOfParentCol
                | PivotShowAs::PercentOfParent
                | PivotShowAs::PercentOfRunningTotal
                | PivotShowAs::RankAscending
                | PivotShowAs::RankDescending
        )
    }

    /// Whether the result is a ratio (shown as a percentage by default).
    pub fn is_percentage(self) -> bool {
        matches!(
            self,
            PivotShowAs::Percent
                | PivotShowAs::PercentDiff
                | PivotShowAs::PercentOfRow
                | PivotShowAs::PercentOfCol
                | PivotShowAs::PercentOfTotal
                | PivotShowAs::PercentOfParentRow
                | PivotShowAs::PercentOfParentCol
                | PivotShowAs::PercentOfParent
                | PivotShowAs::PercentOfRunningTotal
        )
    }

    /// Whether the calculation uses `baseField` / `baseItem`.
    pub fn uses_base(self) -> bool {
        matches!(
            self,
            PivotShowAs::Difference
                | PivotShowAs::Percent
                | PivotShowAs::PercentDiff
                | PivotShowAs::RunTotal
                | PivotShowAs::PercentOfParent
                | PivotShowAs::PercentOfRunningTotal
                | PivotShowAs::RankAscending
                | PivotShowAs::RankDescending
        )
    }
}

/// `baseItem` meaning "the previous item" (§18.10.1.21: 1048828).
pub const PIVOT_BASE_PREVIOUS: u32 = 1_048_828;
/// `baseItem` meaning "the next item" (1048829).
pub const PIVOT_BASE_NEXT: u32 = 1_048_829;

/// A data (values) field of a pivot table (§18.10.1.21 `dataField`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotDataField {
    /// Cache field index (`fld`).
    pub field: u32,
    /// `subtotal` (default `sum`).
    pub function: PivotFunction,
    /// Caption (`name`), e.g. "Sum of Sales".
    pub name: Option<String>,
    /// `showDataAs` (or the x14 `pivotShowAs`).
    pub show_as: PivotShowAs,
    /// `baseField`: cache field index the calculation is relative to.
    pub base_field: u32,
    /// `baseItem`: item index of the base field, or [`PIVOT_BASE_PREVIOUS`] /
    /// [`PIVOT_BASE_NEXT`].
    pub base_item: u32,
    /// `numFmtId`: number format of the value cells (`None` = from the source).
    pub num_fmt_id: Option<u32>,
}

/// Report layout (`compact` / `outline` attributes of §18.10.1.73 and §18.10.1.69).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PivotLayout {
    /// Every row field in one indented column.
    Compact,
    /// One column per row field, an item's children start on the next row.
    Outline,
    /// One column per row field, children on the item's row.
    #[default]
    Tabular,
}

/// `sortType` of a pivot field (§18.18.28 `ST_FieldSortType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PivotSort {
    /// Item order of the field (`manual`, the default).
    #[default]
    Manual,
    /// Items sorted ascending by label (or by `autoSortScope`).
    Ascending,
    /// Items sorted descending by label (or by `autoSortScope`).
    Descending,
}

impl PivotSort {
    /// The `ST_FieldSortType` token.
    pub fn as_str(self) -> &'static str {
        match self {
            PivotSort::Manual => "manual",
            PivotSort::Ascending => "ascending",
            PivotSort::Descending => "descending",
        }
    }

    /// Parse a token (unknown → `manual`, the default).
    pub fn parse(s: &str) -> PivotSort {
        match s {
            "ascending" => PivotSort::Ascending,
            "descending" => PivotSort::Descending,
            _ => PivotSort::Manual,
        }
    }
}

/// Settings of one pivot field (§18.10.1.69 `pivotField`), by cache field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotFieldOptions {
    /// `defaultSubtotal` (default true): subtotal of the field's items.
    pub subtotal: bool,
    /// `subtotalTop` (default true): the subtotal is on the item's first row
    /// (compact / outline layouts; tabular always shows it below).
    pub subtotal_top: bool,
    /// `insertBlankRow`: a blank row after each item.
    pub blank_row: bool,
    /// `x14:pivotField/@fillDownLabels`: repeat item labels on every row.
    pub repeat_labels: bool,
    /// `sortType`.
    pub sort: PivotSort,
    /// `autoSortScope` (§18.10.1.6): sort by this data field (index into
    /// [`PivotTable::values`]) instead of by label.
    pub sort_by: Option<u32>,
    /// Hidden items (`item/@h`), by value.
    pub hidden: Vec<CellValue>,
}

impl Default for PivotFieldOptions {
    fn default() -> Self {
        PivotFieldOptions {
            subtotal: true,
            subtotal_top: true,
            blank_row: false,
            repeat_labels: false,
            sort: PivotSort::Manual,
            sort_by: None,
            hidden: Vec::new(),
        }
    }
}

/// `ST_PivotFilterType` (§18.18.60) of a label, value or top-N filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PivotFilterType {
    /// Top / bottom N items.
    #[default]
    Count,
    /// Top / bottom items making up N percent.
    Percent,
    /// Top / bottom items whose sum reaches N.
    Sum,
    /// Caption equals `value1`.
    CaptionEqual,
    /// Caption does not equal `value1`.
    CaptionNotEqual,
    /// Caption begins with `value1`.
    CaptionBeginsWith,
    /// Caption does not begin with `value1`.
    CaptionNotBeginsWith,
    /// Caption ends with `value1`.
    CaptionEndsWith,
    /// Caption does not end with `value1`.
    CaptionNotEndsWith,
    /// Caption contains `value1`.
    CaptionContains,
    /// Caption does not contain `value1`.
    CaptionNotContains,
    /// Caption is greater than `value1`.
    CaptionGreaterThan,
    /// Caption is greater than or equal to `value1`.
    CaptionGreaterThanOrEqual,
    /// Caption is less than `value1`.
    CaptionLessThan,
    /// Caption is less than or equal to `value1`.
    CaptionLessThanOrEqual,
    /// Caption is between `value1` and `value2`.
    CaptionBetween,
    /// Caption is not between `value1` and `value2`.
    CaptionNotBetween,
    /// Value equals `value1`.
    ValueEqual,
    /// Value does not equal `value1`.
    ValueNotEqual,
    /// Value is greater than `value1`.
    ValueGreaterThan,
    /// Value is greater than or equal to `value1`.
    ValueGreaterThanOrEqual,
    /// Value is less than `value1`.
    ValueLessThan,
    /// Value is less than or equal to `value1`.
    ValueLessThanOrEqual,
    /// Value is between `value1` and `value2`.
    ValueBetween,
    /// Value is not between `value1` and `value2`.
    ValueNotBetween,
    /// Date filters (§18.18.60): the item (a date) compared with `value1` /
    /// `value2` (serials of the workbook's date system).
    DateEqual,
    /// Date differs from `value1`.
    DateNotEqual,
    /// Date is before `value1`.
    DateOlderThan,
    /// Date is on or before `value1`.
    DateOlderThanOrEqual,
    /// Date is after `value1`.
    DateNewerThan,
    /// Date is on or after `value1`.
    DateNewerThanOrEqual,
    /// Date is between `value1` and `value2`.
    DateBetween,
    /// Date is not between `value1` and `value2`.
    DateNotBetween,
    /// Relative dates, evaluated against today.
    Tomorrow,
    /// Today.
    Today,
    /// Yesterday.
    Yesterday,
    /// Next calendar week.
    NextWeek,
    /// Current calendar week.
    ThisWeek,
    /// Previous calendar week.
    LastWeek,
    /// Next calendar month.
    NextMonth,
    /// Current calendar month.
    ThisMonth,
    /// Previous calendar month.
    LastMonth,
    /// Next calendar quarter.
    NextQuarter,
    /// Current calendar quarter.
    ThisQuarter,
    /// Previous calendar quarter.
    LastQuarter,
    /// Next calendar year.
    NextYear,
    /// Current calendar year.
    ThisYear,
    /// Previous calendar year.
    LastYear,
    /// From January 1 of the current year to today.
    YearToDate,
    /// A quarter / month of any year.
    #[serde(rename = "Q1")]
    Q1,
    /// Second quarter.
    #[serde(rename = "Q2")]
    Q2,
    /// Third quarter.
    #[serde(rename = "Q3")]
    Q3,
    /// Fourth quarter.
    #[serde(rename = "Q4")]
    Q4,
    /// January.
    #[serde(rename = "M1")]
    M1,
    /// February.
    #[serde(rename = "M2")]
    M2,
    /// March.
    #[serde(rename = "M3")]
    M3,
    /// April.
    #[serde(rename = "M4")]
    M4,
    /// May.
    #[serde(rename = "M5")]
    M5,
    /// June.
    #[serde(rename = "M6")]
    M6,
    /// July.
    #[serde(rename = "M7")]
    M7,
    /// August.
    #[serde(rename = "M8")]
    M8,
    /// September.
    #[serde(rename = "M9")]
    M9,
    /// October.
    #[serde(rename = "M10")]
    M10,
    /// November.
    #[serde(rename = "M11")]
    M11,
    /// December.
    #[serde(rename = "M12")]
    M12,
}

impl PivotFilterType {
    const ALL: [(PivotFilterType, &'static str); 65] = [
        (PivotFilterType::Count, "count"),
        (PivotFilterType::Percent, "percent"),
        (PivotFilterType::Sum, "sum"),
        (PivotFilterType::CaptionEqual, "captionEqual"),
        (PivotFilterType::CaptionNotEqual, "captionNotEqual"),
        (PivotFilterType::CaptionBeginsWith, "captionBeginsWith"),
        (
            PivotFilterType::CaptionNotBeginsWith,
            "captionNotBeginsWith",
        ),
        (PivotFilterType::CaptionEndsWith, "captionEndsWith"),
        (PivotFilterType::CaptionNotEndsWith, "captionNotEndsWith"),
        (PivotFilterType::CaptionContains, "captionContains"),
        (PivotFilterType::CaptionNotContains, "captionNotContains"),
        (PivotFilterType::CaptionGreaterThan, "captionGreaterThan"),
        (
            PivotFilterType::CaptionGreaterThanOrEqual,
            "captionGreaterThanOrEqual",
        ),
        (PivotFilterType::CaptionLessThan, "captionLessThan"),
        (
            PivotFilterType::CaptionLessThanOrEqual,
            "captionLessThanOrEqual",
        ),
        (PivotFilterType::CaptionBetween, "captionBetween"),
        (PivotFilterType::CaptionNotBetween, "captionNotBetween"),
        (PivotFilterType::ValueEqual, "valueEqual"),
        (PivotFilterType::ValueNotEqual, "valueNotEqual"),
        (PivotFilterType::ValueGreaterThan, "valueGreaterThan"),
        (
            PivotFilterType::ValueGreaterThanOrEqual,
            "valueGreaterThanOrEqual",
        ),
        (PivotFilterType::ValueLessThan, "valueLessThan"),
        (
            PivotFilterType::ValueLessThanOrEqual,
            "valueLessThanOrEqual",
        ),
        (PivotFilterType::ValueBetween, "valueBetween"),
        (PivotFilterType::ValueNotBetween, "valueNotBetween"),
        (PivotFilterType::DateEqual, "dateEqual"),
        (PivotFilterType::DateNotEqual, "dateNotEqual"),
        (PivotFilterType::DateOlderThan, "dateOlderThan"),
        (
            PivotFilterType::DateOlderThanOrEqual,
            "dateOlderThanOrEqual",
        ),
        (PivotFilterType::DateNewerThan, "dateNewerThan"),
        (
            PivotFilterType::DateNewerThanOrEqual,
            "dateNewerThanOrEqual",
        ),
        (PivotFilterType::DateBetween, "dateBetween"),
        (PivotFilterType::DateNotBetween, "dateNotBetween"),
        (PivotFilterType::Tomorrow, "tomorrow"),
        (PivotFilterType::Today, "today"),
        (PivotFilterType::Yesterday, "yesterday"),
        (PivotFilterType::NextWeek, "nextWeek"),
        (PivotFilterType::ThisWeek, "thisWeek"),
        (PivotFilterType::LastWeek, "lastWeek"),
        (PivotFilterType::NextMonth, "nextMonth"),
        (PivotFilterType::ThisMonth, "thisMonth"),
        (PivotFilterType::LastMonth, "lastMonth"),
        (PivotFilterType::NextQuarter, "nextQuarter"),
        (PivotFilterType::ThisQuarter, "thisQuarter"),
        (PivotFilterType::LastQuarter, "lastQuarter"),
        (PivotFilterType::NextYear, "nextYear"),
        (PivotFilterType::ThisYear, "thisYear"),
        (PivotFilterType::LastYear, "lastYear"),
        (PivotFilterType::YearToDate, "yearToDate"),
        (PivotFilterType::Q1, "Q1"),
        (PivotFilterType::Q2, "Q2"),
        (PivotFilterType::Q3, "Q3"),
        (PivotFilterType::Q4, "Q4"),
        (PivotFilterType::M1, "M1"),
        (PivotFilterType::M2, "M2"),
        (PivotFilterType::M3, "M3"),
        (PivotFilterType::M4, "M4"),
        (PivotFilterType::M5, "M5"),
        (PivotFilterType::M6, "M6"),
        (PivotFilterType::M7, "M7"),
        (PivotFilterType::M8, "M8"),
        (PivotFilterType::M9, "M9"),
        (PivotFilterType::M10, "M10"),
        (PivotFilterType::M11, "M11"),
        (PivotFilterType::M12, "M12"),
    ];

    /// The `ST_PivotFilterType` token.
    pub fn as_str(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(t, _)| *t == self)
            .map_or("count", |(_, s)| s)
    }

    /// Parse a token; `None` for types not modelled (dates, OLAP).
    pub fn parse(s: &str) -> Option<PivotFilterType> {
        Self::ALL.iter().find(|(_, n)| *n == s).map(|(t, _)| *t)
    }

    /// Top / bottom filters (`top10` in the filter's AutoFilter).
    pub fn is_top10(self) -> bool {
        matches!(
            self,
            PivotFilterType::Count | PivotFilterType::Percent | PivotFilterType::Sum
        )
    }

    /// Filters comparing item captions.
    pub fn is_caption(self) -> bool {
        self.as_str().starts_with("caption")
    }

    /// Date filters (fixed dates, relative periods, quarters and months).
    pub fn is_date(self) -> bool {
        self as u32 >= PivotFilterType::DateEqual as u32
    }

    /// Date filters relative to today (`today`, `thisMonth`, `yearToDate`, …).
    pub fn is_relative_date(self) -> bool {
        (PivotFilterType::Tomorrow as u32..=PivotFilterType::YearToDate as u32)
            .contains(&(self as u32))
    }

    /// Date filters comparing with one or two dates (`dateEqual` … `dateNotBetween`).
    pub fn is_fixed_date(self) -> bool {
        self.is_date() && (self as u32) < PivotFilterType::Tomorrow as u32
    }
}

/// A calculated item (§18.10.1.8 `calculatedItem`): an extra item of a cache
/// field whose values are a formula over other items of the same field
/// (`='North'+'South'`), evaluated over their sums in every report cell.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotCalcItem {
    /// Cache field index.
    pub field: u32,
    /// Caption (also a shared item of the field in the cache definition).
    pub name: String,
    /// Formula without `=`; item names bare or quoted (`'Hà Nội'`).
    pub formula: String,
}

/// A label, value or top-N filter of a row / column field (§18.10.1.35 `filter`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotFieldFilter {
    /// Cache field index (`fld`).
    pub field: u32,
    /// `type`: the filter type.
    pub kind: PivotFilterType,
    /// `iMeasureFld`: data field index for value and top-N filters.
    pub measure: u32,
    /// `stringValue1` / `stringValue2` (caption filters) or the numbers of
    /// value filters as text.
    pub value1: String,
    /// Second value (`stringValue2` or numeric upper bound) of between filters.
    pub value2: String,
    /// Top-N: `top10/@top` (false = bottom) and `top10/@val`.
    pub top: bool,
    /// Top-N count, percent or sum (`top10/@val`).
    pub count: f64,
}

/// A pivot table `conditionalFormat` (§18.10.1.15): links the worksheet `cfRule` with
/// the same `priority` (in a `conditionalFormatting pivot="1"` block) to pivot areas.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotConditionalFormat {
    /// `priority` of the linked `cfRule`.
    pub priority: i32,
    /// `scope` (`ST_Scope`: selection (default), data, field).
    pub scope: Option<String>,
    /// `type` (`ST_Type`: none (default), all, row, column).
    pub kind: Option<String>,
    /// `pivotAreas/pivotArea` (§18.3.1.68), serialized.
    pub areas: Vec<String>,
}

/// `pivotTableStyleInfo` (§18.10.1 `CT_PivotTableStyle`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotStyleInfo {
    /// Built-in or custom style name, e.g. `PivotStyleLight16`.
    pub name: String,
    /// `showRowHeaders`.
    pub show_row_headers: bool,
    /// `showColHeaders`.
    pub show_col_headers: bool,
    /// `showRowStripes`.
    pub show_row_stripes: bool,
    /// `showColStripes`.
    pub show_col_stripes: bool,
    /// `showLastColumn`.
    pub show_last_column: bool,
}

/// One line of `rowItems` / `colItems` (§18.10.1.44 `i`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotAxisItem {
    /// `t` (`ST_ItemType`): empty = `data`; `default` (subtotal), `grand`, `blank`, …
    pub kind: String,
    /// `r`: leading members repeated from the previous line.
    pub repeat: u32,
    /// `i`: data field index.
    pub data_index: u32,
    /// `x/@v`: item indexes (into the pivot field's items).
    pub items: Vec<u32>,
}

/// A report filter (page field, §18.10.1 `pageField` in `pageFields`) and its selection.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PivotFilter {
    /// Cache field index.
    pub field: u32,
    /// Shown items (by value); empty = all items.
    pub selected: Vec<CellValue>,
}

/// A pivot table placed on a worksheet (§18.10.1.73 `pivotTableDefinition`).
///
/// The rendered report itself consists of ordinary cells of the sheet. Pivot
/// tables read from a file keep their part opaque unless edited ([`PivotTable::dirty`]);
/// new ones (empty `part`) are written from this summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PivotTable {
    /// Absolute part name; empty for a new pivot table.
    pub part: String,
    /// `name` of the pivot table.
    pub name: String,
    /// `cacheId`: the [`PivotCache`] the table reads from.
    pub cache_id: u32,
    /// `location/@ref` (§18.10.1.49): the report body, page fields excluded.
    pub location: CellRange,
    /// `location/@firstHeaderRow`, `@firstDataRow`, `@firstDataCol` (relative).
    pub first_header_row: u32,
    /// See [`PivotTable::first_header_row`].
    pub first_data_row: u32,
    /// See [`PivotTable::first_header_row`].
    pub first_data_col: u32,
    /// `rowFields/field/@x` (cache field indices; the values pseudo-field excluded).
    pub rows: Vec<u32>,
    /// `colFields/field/@x`.
    pub cols: Vec<u32>,
    /// The values pseudo-field (`x = -2`) is on the row axis (`dataOnRows`).
    pub data_on_rows: bool,
    /// `dataFields/dataField`.
    pub values: Vec<PivotDataField>,
    /// Report filters (`pageFields/pageField`).
    pub filters: Vec<PivotFilter>,
    /// `dataCaption` (required, e.g. "Values").
    pub data_caption: String,
    /// Per cache field settings (`pivotFields`); missing entries use the defaults.
    pub fields: Vec<PivotFieldOptions>,
    /// Report layout (`compact` / `outline`).
    pub layout: PivotLayout,
    /// `rowGrandTotals` (default true): the grand total column at the right
    /// (totals of every row).
    pub row_grand_totals: bool,
    /// `colGrandTotals` (default true): the grand total row at the bottom.
    pub col_grand_totals: bool,
    /// Label, value and top-N filters (§18.10.1 `filters`).
    pub field_filters: Vec<PivotFieldFilter>,
    /// `pivotTableStyleInfo`.
    pub style: Option<PivotStyleInfo>,
    /// `rowItems` / `colItems` (§18.10.1.84 / §18.10.1.10): the report lines.
    pub row_items: Vec<PivotAxisItem>,
    /// Column axis lines (`colItems`); see [`PivotTable::row_items`].
    pub col_items: Vec<PivotAxisItem>,
    /// `conditionalFormats` (§18.10.1.16): scope of the sheet's pivot rules.
    pub conditional_formats: Vec<PivotConditionalFormat>,
    /// Regenerate the part on write.
    pub dirty: bool,
}

impl Default for PivotTable {
    fn default() -> Self {
        PivotTable {
            part: String::new(),
            name: String::new(),
            cache_id: 0,
            location: CellRange::default(),
            first_header_row: 0,
            first_data_row: 0,
            first_data_col: 0,
            rows: Vec::new(),
            cols: Vec::new(),
            data_on_rows: false,
            values: Vec::new(),
            filters: Vec::new(),
            data_caption: String::new(),
            fields: Vec::new(),
            layout: PivotLayout::Tabular,
            row_grand_totals: true,
            col_grand_totals: true,
            field_filters: Vec::new(),
            style: None,
            row_items: Vec::new(),
            col_items: Vec::new(),
            conditional_formats: Vec::new(),
            dirty: false,
        }
    }
}

impl PivotTable {
    /// Settings of cache field `f` (defaults when not set).
    pub fn field(&self, f: u32) -> PivotFieldOptions {
        self.fields.get(f as usize).cloned().unwrap_or_default()
    }

    /// Mutable settings of cache field `f`, growing [`PivotTable::fields`].
    pub fn field_mut(&mut self, f: u32) -> &mut PivotFieldOptions {
        let i = f as usize;
        if self.fields.len() <= i {
            self.fields.resize(i + 1, PivotFieldOptions::default());
        }
        &mut self.fields[i]
    }
}

// ---------------------------------------------------------------------------
// Page setup
// ---------------------------------------------------------------------------

/// Printing related sheet settings (§18.3.1.70 printOptions, §18.3.1.62 pageMargins,
/// §18.3.1.63 pageSetup, §18.3.1.46 headerFooter, §18.3.1.74 rowBreaks / colBreaks).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PageSetup {
    /// `printOptions/@gridLines`.
    pub print_grid_lines: bool,
    /// `printOptions/@headings`: print row and column headings.
    pub print_headings: bool,
    /// `printOptions/@horizontalCentered`.
    pub horizontal_centered: bool,
    /// `printOptions/@verticalCentered`.
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
    /// `fitToWidth`: pages wide when fitting to page.
    pub fit_to_width: Option<u32>,
    /// `fitToHeight`: pages tall when fitting to page.
    pub fit_to_height: Option<u32>,
    /// Other `pageSetup` attributes (r:id, horizontalDpi, firstPageNumber, …).
    pub attrs: Vec<RawAttr>,
    /// Whether `pageSetup` was present.
    pub has_page_setup: bool,
    /// `oddHeader` text with its `&` codes.
    pub odd_header: Option<String>,
    /// `oddFooter` text with its `&` codes.
    pub odd_footer: Option<String>,
    /// `evenHeader` (§18.3.1.40), used on even pages when `different_odd_even`.
    #[serde(default)]
    pub even_header: Option<String>,
    /// `evenFooter` (§18.3.1.39).
    #[serde(default)]
    pub even_footer: Option<String>,
    /// `firstHeader` (§18.3.1.42), used on the first page when `different_first`.
    #[serde(default)]
    pub first_header: Option<String>,
    /// `firstFooter` (§18.3.1.41).
    #[serde(default)]
    pub first_footer: Option<String>,
    /// `headerFooter/@differentOddEven` (§18.3.1.46).
    #[serde(default)]
    pub different_odd_even: bool,
    /// `headerFooter/@differentFirst` (§18.3.1.46).
    #[serde(default)]
    pub different_first: bool,
    /// Other `headerFooter` attributes (`scaleWithDoc`, `alignWithMargins`).
    #[serde(default)]
    pub header_footer_attrs: Vec<RawAttr>,
    /// Whole `headerFooter` element when it has children this model does not know.
    pub header_footer_raw: Option<String>,
    /// Manual row breaks (zero-based row *after which* a page breaks).
    pub row_breaks: Vec<u32>,
    /// Manual column breaks.
    pub col_breaks: Vec<u32>,
}

/// Page margins in inches (§18.3.1.62).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PageMargins {
    /// Left margin, inches.
    pub left: f64,
    /// Right margin, inches.
    pub right: f64,
    /// Top margin, inches.
    pub top: f64,
    /// Bottom margin, inches.
    pub bottom: f64,
    /// Header margin, inches.
    pub header: f64,
    /// Footer margin, inches.
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
    /// `theme`: index into the theme colour scheme.
    pub theme: Option<u32>,
    /// `indexed`: index into the legacy colour palette.
    pub indexed: Option<u32>,
    /// `tint` (−1.0 to 1.0): darken (< 0) or lighten (> 0).
    pub tint: Option<f64>,
    /// `auto`: automatic (system) colour.
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
    /// `fonts` (§18.8.23).
    pub fonts: Vec<Font>,
    /// `fills` (§18.8.21).
    pub fills: Vec<Fill>,
    /// `borders` (§18.8.5).
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
    /// `numFmtId` (custom ids start at 164).
    pub id: u32,
    /// `formatCode`.
    pub code: String,
}

/// Font (§18.8.22 `font`; also run properties §18.4.7 `rPr`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Font {
    /// `name`: typeface name.
    pub name: Option<String>,
    /// Size in points (`sz`).
    pub size: Option<f64>,
    /// `b`: bold.
    pub bold: bool,
    /// `i`: italic.
    pub italic: bool,
    /// `u` (§18.18.82): single, double, singleAccounting, doubleAccounting.
    pub underline: Option<String>,
    /// `strike`: strikethrough.
    pub strike: bool,
    /// `color`.
    pub color: Option<SheetColor>,
    /// `family`: font family class (2 = swiss, …).
    pub family: Option<u32>,
    /// `charset`: character set.
    pub charset: Option<u32>,
    /// `scheme`: major / minor / none.
    pub scheme: Option<String>,
    /// `vertAlign`: superscript / subscript / baseline.
    pub vert_align: Option<String>,
    /// `outline`.
    pub outline: bool,
    /// `shadow`.
    pub shadow: bool,
    /// `condense`.
    pub condense: bool,
    /// `extend`.
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
    /// `fgColor`: pattern (foreground) colour; the fill colour of a solid fill.
    pub fg: Option<SheetColor>,
    /// `bgColor`: background colour of the pattern.
    pub bg: Option<SheetColor>,
}

/// Gradient fill (§18.8.24).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GradientFill {
    /// `type`: linear (default) / path.
    pub kind: Option<String>,
    /// `degree`: angle of a linear gradient.
    pub degree: Option<f64>,
    /// `left`: path gradient focus (0..1).
    pub left: Option<f64>,
    /// `right`: path gradient focus (0..1).
    pub right: Option<f64>,
    /// `top`: path gradient focus (0..1).
    pub top: Option<f64>,
    /// `bottom`: path gradient focus (0..1).
    pub bottom: Option<f64>,
    /// Gradient stops (position 0..1, colour).
    pub stops: Vec<(f64, SheetColor)>,
}

/// One border edge (§18.8.43 `CT_BorderPr`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BorderEdge {
    /// `style` (§18.18.3): thin, medium, thick, dashed, dotted, double, hair, …
    pub style: Option<String>,
    /// Edge colour.
    pub color: Option<SheetColor>,
}

/// Border (§18.8.4 `border`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Border {
    /// Left edge.
    pub left: BorderEdge,
    /// Right edge.
    pub right: BorderEdge,
    /// Top edge.
    pub top: BorderEdge,
    /// Bottom edge.
    pub bottom: BorderEdge,
    /// Diagonal line(s).
    pub diagonal: BorderEdge,
    /// Inner vertical edges (formats applied to ranges, e.g. dxfs).
    pub vertical: Option<BorderEdge>,
    /// Inner horizontal edges.
    pub horizontal: Option<BorderEdge>,
    /// `diagonalUp`: draw the bottom-left to top-right diagonal.
    pub diagonal_up: bool,
    /// `diagonalDown`: draw the top-left to bottom-right diagonal.
    pub diagonal_down: bool,
    /// `outline`: apply the edges to the outside of a range.
    pub outline: Option<bool>,
}

/// Cell alignment (§18.8.1 `alignment`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CellAlignment {
    /// `horizontal` (§18.18.40): general, left, center, right, fill, justify, centerContinuous, distributed.
    pub horizontal: Option<String>,
    /// `vertical` (§18.18.88): top, center, bottom, justify, distributed.
    pub vertical: Option<String>,
    /// `wrapText`.
    pub wrap_text: bool,
    /// `shrinkToFit`.
    pub shrink_to_fit: bool,
    /// `indent`, in indent steps.
    pub indent: Option<u32>,
    /// `textRotation` (0–180, 255 = vertical).
    pub text_rotation: Option<u32>,
    /// `readingOrder`: 0 = context, 1 = left-to-right, 2 = right-to-left.
    pub reading_order: Option<u32>,
    /// `justifyLastLine`.
    pub justify_last_line: bool,
    /// `relativeIndent`.
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
    /// `numFmtId`: built-in or custom number format id.
    pub num_fmt_id: u32,
    /// `fontId`: index into [`Stylesheet::fonts`].
    pub font_id: u32,
    /// `fillId`: index into [`Stylesheet::fills`].
    pub fill_id: u32,
    /// `borderId`: index into [`Stylesheet::borders`].
    pub border_id: u32,
    /// Parent cell style (`xfId`, index into `cellStyleXfs`); only in `cellXfs`.
    pub xf_id: Option<u32>,
    /// `applyNumberFormat`.
    pub apply_number_format: Option<bool>,
    /// `applyFont`.
    pub apply_font: Option<bool>,
    /// `applyFill`.
    pub apply_fill: Option<bool>,
    /// `applyBorder`.
    pub apply_border: Option<bool>,
    /// `applyAlignment`.
    pub apply_alignment: Option<bool>,
    /// `applyProtection`.
    pub apply_protection: Option<bool>,
    /// `quotePrefix`: the value was entered with a leading `'`.
    pub quote_prefix: bool,
    /// `pivotButton`: the cell shows a pivot field button.
    pub pivot_button: bool,
    /// `alignment` child.
    pub alignment: Option<CellAlignment>,
    /// `protection` child.
    pub protection: Option<CellProtection>,
    /// The format shows a checkbox cell control ([MS-XLSX] §2.3.9.4 Checkbox,
    /// reached from `extLst/ext/xfpb:xfComplement` §2.6.238).
    #[serde(default)]
    pub checkbox: bool,
    /// `xfpb:xfComplement/@i` as read ([MS-XLSX] §2.6.238), kept for complements
    /// other than a checkbox.
    #[serde(default)]
    pub complement: Option<u32>,
}

/// Alias used by the rest of the code base: a cell "style" is an `xf` record.
pub type Style = CellXf;

/// A named cell style (§18.8.7 `cellStyle`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CellStyle {
    /// `name`, e.g. `Normal`.
    pub name: String,
    /// `xfId`: index into `cellStyleXfs`.
    pub xf_id: u32,
    /// `builtinId`: id of a built-in style (0 = Normal).
    pub builtin_id: Option<u32>,
    /// `iLevel`: outline level of RowLevel / ColLevel built-in styles.
    pub i_level: Option<u32>,
    /// `hidden`: not shown in the style gallery.
    pub hidden: bool,
    /// `customBuiltin`: a built-in style that was customised.
    pub custom_builtin: bool,
}

/// Differential format (§18.8.14 `dxf`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Dxf {
    /// Font changes.
    pub font: Option<Font>,
    /// Number format change.
    pub num_fmt: Option<NumFmt>,
    /// Fill change.
    pub fill: Option<Fill>,
    /// Alignment change.
    pub alignment: Option<CellAlignment>,
    /// Border changes.
    pub border: Option<Border>,
    /// Protection change.
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
