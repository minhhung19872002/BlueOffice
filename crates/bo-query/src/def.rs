//! The query definition: a source and an ordered list of steps (BlueOffice's own step
//! language, stored as JSON; see `docs/sheet/queries.md`).

use crate::value::ColType;
use serde::{Deserialize, Serialize};

/// A saved query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    /// Name, unique in the workbook (case-insensitive).
    pub name: String,
    /// Where the data comes from.
    pub source: Source,
    /// Steps applied in order.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// Where the result is loaded (absent: connection only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load: Option<LoadTarget>,
    /// Free description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// The table a query's result is loaded to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadTarget {
    /// Name of the table (§18.5.1.2 `table/@name`) holding the result.
    pub table: String,
}

/// Where a query reads its data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Source {
    /// Delimited text (RFC 4180).
    Csv {
        /// Source key: file name (web) or path (desktop).
        file: String,
        /// Field delimiter (absent: detected).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delimiter: Option<String>,
        /// `utf-8`, `utf-16le`, `utf-16be`, `windows-1252`, `tcvn3`, `vni` (absent: detected).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        encoding: Option<String>,
    },
    /// JSON (RFC 8259): an array of records, possibly below `path`.
    Json {
        /// Source key.
        file: String,
        /// Dotted path to the array (`data.items`, `rows[0]`); absent: the first array found.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    /// XML: one row per repeating element.
    Xml {
        /// Source key.
        file: String,
        /// Local name of the repeating element (absent: detected).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        element: Option<String>,
    },
    /// A sheet or table of another workbook (.xlsx, .xlsm, .xls, .xlsb, .ods, .csv).
    Workbook {
        /// Source key.
        file: String,
        /// Sheet or table name.
        item: String,
        /// `sheet` or `table`.
        #[serde(default)]
        item_kind: ItemKind,
    },
    /// Every file of one kind in a folder, combined (one table per file, appended, with a
    /// `Source.Name` column).
    Folder {
        /// Source key: folder name (web) or path (desktop).
        folder: String,
        /// `csv`, `json`, `xml` or `workbook`.
        file_kind: String,
        /// Delimiter of CSV files (absent: detected per file).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delimiter: Option<String>,
        /// JSON path / XML element / workbook sheet or table, as for single files.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item: Option<String>,
        /// The first row of each file holds the column names (CSV and sheets).
        #[serde(default = "yes")]
        promote_headers: bool,
    },
    /// An HTML `<table>` of a web page.
    Web {
        /// Page URL (also the source key).
        url: String,
        /// Zero-based index of the table on the page.
        #[serde(default)]
        table: usize,
    },
    /// The result of another query of the workbook.
    Query {
        /// Name of the query.
        query: String,
    },
}

fn yes() -> bool {
    true
}

/// What a workbook source reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    /// The used range of a sheet.
    #[default]
    Sheet,
    /// A table (with its column names).
    Table,
}

impl Source {
    /// The key under which the host provides the source's bytes (`None`: another query).
    pub fn key(&self) -> Option<&str> {
        match self {
            Source::Csv { file, .. }
            | Source::Json { file, .. }
            | Source::Xml { file, .. }
            | Source::Workbook { file, .. } => Some(file),
            Source::Folder { folder, .. } => Some(folder),
            Source::Web { url, .. } => Some(url),
            Source::Query { .. } => None,
        }
    }

    /// `file`, `folder` or `web`: how the host gets the bytes again on refresh.
    pub fn access(&self) -> &'static str {
        match self {
            Source::Folder { .. } => "folder",
            Source::Web { .. } => "web",
            Source::Query { .. } => "query",
            _ => "file",
        }
    }
}

/// One step of a query (BlueOffice step language; `op` names the step).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Step {
    /// The first row becomes the column names.
    PromoteHeaders,
    /// The column names become the first row.
    DemoteHeaders,
    /// Change the type of columns.
    ChangeType {
        /// Column → type.
        columns: Vec<ColumnType>,
        /// Decimal separator of numbers written as text (`.` by default).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decimal: Option<String>,
        /// Dates written as text are month first (`01/31/2024`).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        month_first: bool,
    },
    /// Keep the rows matching a condition.
    FilterRows {
        /// Column tested.
        column: String,
        /// The condition.
        condition: Condition,
        /// Compared value (text; read as a number or date for those columns).
        #[serde(default)]
        value: String,
    },
    /// Remove columns.
    RemoveColumns {
        /// Columns removed.
        columns: Vec<String>,
    },
    /// Keep only these columns (in this order).
    KeepColumns {
        /// Columns kept.
        columns: Vec<String>,
    },
    /// Rename columns.
    RenameColumns {
        /// Old → new names.
        renames: Vec<Rename>,
    },
    /// Put columns in this order (columns not listed follow, in their order).
    ReorderColumns {
        /// The order.
        columns: Vec<String>,
    },
    /// Split a text column at a delimiter into `column.1`, `column.2`, ….
    SplitColumn {
        /// Column split.
        column: String,
        /// Delimiter.
        delimiter: String,
        /// Number of parts (absent: as many as the longest value has).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parts: Option<usize>,
    },
    /// Replace values.
    ReplaceValues {
        /// Columns (empty: all).
        #[serde(default)]
        columns: Vec<String>,
        /// Text looked for.
        find: String,
        /// Replacement.
        replace: String,
        /// Replace only cells whose whole text matches.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        match_entire: bool,
    },
    /// Group rows by key columns with aggregates.
    GroupBy {
        /// Key columns.
        keys: Vec<String>,
        /// Aggregates (new columns).
        aggregates: Vec<Aggregate>,
    },
    /// Sort rows.
    Sort {
        /// Sort keys, most significant first.
        keys: Vec<SortKey>,
    },
    /// Join with another query.
    Merge {
        /// The other query.
        query: String,
        /// Key columns of this table.
        left_keys: Vec<String>,
        /// Key columns of the other query.
        right_keys: Vec<String>,
        /// Join kind.
        #[serde(default)]
        kind: JoinKind,
        /// Columns of the other query brought in (empty: all but its keys).
        #[serde(default)]
        columns: Vec<String>,
    },
    /// Append the rows of other queries (columns matched by name).
    Append {
        /// The other queries.
        queries: Vec<String>,
    },
    /// Turn columns into attribute / value rows.
    Unpivot {
        /// Columns unpivoted (or kept, with `other`).
        columns: Vec<String>,
        /// Unpivot every column but `columns`.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        other: bool,
        /// Name of the attribute column.
        #[serde(default = "attribute")]
        attribute: String,
        /// Name of the value column.
        #[serde(default = "value_name")]
        value: String,
    },
    /// Add a column computed by a BlueSheet formula per row (`=[@Price]*[@Qty]`).
    AddColumn {
        /// Name of the new column.
        name: String,
        /// The formula (structured references to the query's columns).
        formula: String,
    },
    /// Remove rows repeating the values of earlier rows.
    RemoveDuplicates {
        /// Columns compared (empty: all).
        #[serde(default)]
        columns: Vec<String>,
    },
    /// Keep the first `count` rows.
    KeepTop {
        /// Rows kept.
        count: usize,
    },
    /// Remove the first `count` rows.
    RemoveTop {
        /// Rows removed.
        count: usize,
    },
    /// Remove rows whose cells are all blank.
    RemoveBlankRows,
}

fn attribute() -> String {
    "Attribute".into()
}

fn value_name() -> String {
    "Value".into()
}

/// A column and its type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnType {
    /// Column.
    pub column: String,
    /// Type.
    #[serde(rename = "type")]
    pub ty: ColType,
}

/// A rename.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rename {
    /// Old name.
    pub from: String,
    /// New name.
    pub to: String,
}

/// A filter condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Condition {
    /// `=`
    Equals,
    /// `<>`
    NotEquals,
    /// `>`
    GreaterThan,
    /// `>=`
    GreaterOrEqual,
    /// `<`
    LessThan,
    /// `<=`
    LessOrEqual,
    /// Text contains (case-insensitive).
    Contains,
    /// Text does not contain.
    NotContains,
    /// Text begins with.
    BeginsWith,
    /// Text ends with.
    EndsWith,
    /// Blank.
    IsBlank,
    /// Not blank.
    IsNotBlank,
}

/// An aggregate of a group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aggregate {
    /// Name of the new column.
    pub name: String,
    /// `sum`, `average`, `min`, `max`, `count` (rows), `countDistinct`, `median`.
    pub op: String,
    /// Column aggregated (not needed by `count`).
    #[serde(default)]
    pub column: String,
}

/// A sort key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortKey {
    /// Column.
    pub column: String,
    /// Descending order.
    #[serde(default)]
    pub descending: bool,
}

/// A join kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JoinKind {
    /// Every row of this table, matching rows of the other.
    #[default]
    LeftOuter,
    /// Every row of the other table, matching rows of this one.
    RightOuter,
    /// Every row of both.
    FullOuter,
    /// Only matching rows.
    Inner,
    /// Rows of this table without a match.
    LeftAnti,
    /// Rows of the other table without a match.
    RightAnti,
}
