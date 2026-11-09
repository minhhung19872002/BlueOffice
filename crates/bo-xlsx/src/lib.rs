//! SpreadsheetML (`.xlsx`) and CSV support for the BlueOffice workbook model
//! ([`bo_model::sheet`]).
//!
//! * [`read_xlsx`] / [`write_xlsx`] — ECMA-376 Part 1, §18 SpreadsheetML:
//!   workbook (§18.2), worksheets (§18.3), shared strings (§18.4), tables (§18.5),
//!   comments (§18.7), styles (§18.8) and defined names (§18.2.5).
//! * Lossless round trip: package parts the model does not understand (theme,
//!   drawings, charts, pivot tables, VML, printer settings, custom XML, …) are kept
//!   byte-for-byte; unknown child elements of `workbook`, `worksheet`, `styleSheet`,
//!   `table`, `sheetView` and `cfRule` (e.g. `extLst`, `mc:AlternateContent`,
//!   `sheetProtection`, `autoFilter`, `drawing`) are kept as raw XML and written
//!   back at their schema position; relationships pointing at opaque parts keep
//!   their ids so raw fragments stay valid.
//! * Untrusted input is protected by `bo-opc` (zip-bomb limits, XML depth limit).
//! * [`read_csv`] / [`write_csv`] — RFC 4180 CSV with UTF-8 BOM handling and a
//!   configurable delimiter (`;` for vi-VN).
#![forbid(unsafe_code)]

mod autofilter;
mod chart;
mod chart_format;
pub mod chart_write;
pub mod chartex;
mod csv;
pub mod formctl;
mod pivot;
pub mod pivot_write;
mod read;
mod scenario;
pub mod slicer;
pub mod sparkline;
mod styles;
mod threaded;
pub mod timeline;
mod write;
mod xml;

pub use csv::{read_csv, write_csv, CsvOptions};
pub use read::{read_xlsx, read_xlsx_with_limits};
pub use write::write_xlsx;
pub use xml::canonicalize_xml;

/// Errors produced while reading or writing spreadsheets.
#[derive(Debug, thiserror::Error)]
pub enum XlsxError {
    /// Error from the OPC package layer.
    #[error(transparent)]
    Opc(#[from] bo_opc::OpcError),
    /// The package has no workbook part.
    #[error("package has no workbook part (officeDocument relationship)")]
    NoWorkbook,
    /// A referenced part is absent.
    #[error("missing part: {0}")]
    MissingPart(String),
    /// CSV input is not valid UTF-8.
    #[error("CSV input is not valid UTF-8")]
    CsvEncoding,
    /// CSV input exceeds a size limit.
    #[error("CSV input exceeds limits: {0}")]
    CsvLimit(String),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, XlsxError>;

/// Workbook part content type (ECMA-376 Part 1, §12.3.23).
pub const CT_WORKBOOK: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
/// Worksheet part content type (§12.3.24).
pub const CT_WORKSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
/// Styles part content type (§12.3.20).
pub const CT_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml";
/// Shared string table content type (§12.3.15).
pub const CT_SHARED_STRINGS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml";
/// Table part content type (§12.3.21).
pub const CT_TABLE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml";
/// Comments part content type (§12.3.3).
pub const CT_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml";

/// Worksheet relationship type (§12.3.24).
pub const REL_WORKSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
/// Shared strings relationship type (§12.3.15).
pub const REL_SHARED_STRINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings";
/// Table relationship type (§12.3.21).
pub const REL_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/table";
/// Comments relationship type (§12.3.3).
pub const REL_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
/// Hyperlink relationship type (§15.3).
pub const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

/// Relationship classification by the last path segment of the type URI, so that
/// both Transitional and Strict (`purl.oclc.org`) URIs are recognized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelKind {
    Worksheet,
    OtherSheet,
    Styles,
    SharedStrings,
    CalcChain,
    Theme,
    Table,
    Comments,
    Hyperlink,
    Other,
}

pub(crate) fn rel_kind(rel_type: &str) -> RelKind {
    match rel_type.rsplit('/').next().unwrap_or("") {
        "worksheet" => RelKind::Worksheet,
        "chartsheet" | "dialogsheet" | "xlMacrosheet" | "xlIntlMacrosheet" => RelKind::OtherSheet,
        "styles" => RelKind::Styles,
        "sharedStrings" => RelKind::SharedStrings,
        "calcChain" => RelKind::CalcChain,
        "theme" => RelKind::Theme,
        "table" => RelKind::Table,
        "comments" => RelKind::Comments,
        "hyperlink" => RelKind::Hyperlink,
        _ => RelKind::Other,
    }
}

/// Child sequence of `workbook` (§18.2.27 CT_Workbook).
pub(crate) const WORKBOOK_SCHEMA: &[&str] = &[
    "fileVersion",
    "fileSharing",
    "workbookPr",
    "workbookProtection",
    "bookViews",
    "sheets",
    "functionGroups",
    "externalReferences",
    "definedNames",
    "calcPr",
    "oleSize",
    "customWorkbookViews",
    "pivotCaches",
    "smartTagPr",
    "smartTagTypes",
    "webPublishing",
    "fileRecoveryPr",
    "webPublishObjects",
    "extLst",
];

/// Child sequence of `worksheet` (§18.3.1.99 CT_Worksheet).
pub(crate) const SHEET_SCHEMA: &[&str] = &[
    "sheetPr",
    "dimension",
    "sheetViews",
    "sheetFormatPr",
    "cols",
    "sheetData",
    "sheetCalcPr",
    "sheetProtection",
    "protectedRanges",
    "scenarios",
    "autoFilter",
    "sortState",
    "dataConsolidate",
    "customSheetViews",
    "mergeCells",
    "phoneticPr",
    "conditionalFormatting",
    "dataValidations",
    "hyperlinks",
    "printOptions",
    "pageMargins",
    "pageSetup",
    "headerFooter",
    "rowBreaks",
    "colBreaks",
    "customProperties",
    "cellWatches",
    "ignoredErrors",
    "smartTags",
    "drawing",
    "legacyDrawing",
    "legacyDrawingHF",
    "drawingHF",
    "picture",
    "oleObjects",
    "controls",
    "webPublishItems",
    "tableParts",
    "extLst",
];

/// Child sequence of `table` (§18.5.1.2 CT_Table).
pub(crate) const TABLE_SCHEMA: &[&str] = &[
    "autoFilter",
    "sortState",
    "tableColumns",
    "tableStyleInfo",
    "extLst",
];
