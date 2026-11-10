//! Get & Transform data for BlueSheet ("Lấy & biến đổi dữ liệu").
//!
//! A [`Query`] is a [`Source`] and an ordered list of [`Step`]s written in BlueOffice's own
//! step language (JSON, see `docs/spec-notes/sheet-queries.md`). [`Evaluator`] turns it into
//! a [`QTable`] from the bytes the host provides through a [`Resolver`]: the engine never
//! opens files or the network itself (the host reads files through its platform layer and
//! fetches web pages through it as well).
//!
//! Sources: delimited text ([`text`]), JSON ([`json_src`]), XML ([`xml_src`]), a sheet or
//! table of another workbook ([`workbook_src`]), the files of a folder combined, an HTML
//! table of a web page ([`html`]) and another query. [`persist`] keeps the definitions in a
//! workbook and writes the standard connection and query table parts.
#![forbid(unsafe_code)]

pub mod def;
pub mod eval;
pub mod formula;
pub mod html;
pub mod json_src;
pub mod persist;
pub mod steps;
pub mod text;
pub mod value;
pub mod workbook_src;
pub mod xml_src;

pub use def::{
    Aggregate, ColumnType, Condition, ItemKind, JoinKind, LoadTarget, Query, Rename, SortKey,
    Source, Step,
};
pub use eval::{suggest_steps, Evaluator, Resolver, StepError};
pub use value::{ColType, QTable, Val};

/// Why a query could not be evaluated. The `Display` text starts with a stable code
/// (`missingSource: data.csv`) the user interface translates.
#[derive(Debug, Clone, PartialEq)]
pub enum QueryError {
    /// The host has not provided the bytes of this source key.
    MissingSource(String),
    /// The source could not be read.
    Source(String),
    /// No column of this name.
    Column(String),
    /// A step's settings are wrong.
    Step(String),
    /// The custom column formula does not parse.
    Formula(String),
    /// No query of this name.
    Query(String),
    /// The query reads itself (through `merge`, `append` or a query source).
    Cycle(String),
    /// A query read by this one failed.
    Dependency(String, Box<QueryError>),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::MissingSource(k) => write!(f, "missingSource: {k}"),
            QueryError::Source(m) => write!(f, "source: {m}"),
            QueryError::Column(c) => write!(f, "column: {c}"),
            QueryError::Step(m) => write!(f, "step: {m}"),
            QueryError::Formula(m) => write!(f, "formula: {m}"),
            QueryError::Query(q) => write!(f, "query: {q}"),
            QueryError::Cycle(q) => write!(f, "cycle: {q}"),
            QueryError::Dependency(q, e) => write!(f, "dependency: {q}: {e}"),
        }
    }
}

impl std::error::Error for QueryError {}

impl QueryError {
    /// The stable code of the error (`missingSource`, `column`, …).
    pub fn code(&self) -> &'static str {
        match self {
            QueryError::MissingSource(_) => "missingSource",
            QueryError::Source(_) => "source",
            QueryError::Column(_) => "column",
            QueryError::Step(_) => "step",
            QueryError::Formula(_) => "formula",
            QueryError::Query(_) => "query",
            QueryError::Cycle(_) => "cycle",
            QueryError::Dependency(_, e) => e.code(),
        }
    }

    /// The source keys the host must provide again (missing sources, also of dependencies).
    pub fn missing(&self) -> Option<&str> {
        match self {
            QueryError::MissingSource(k) => Some(k),
            QueryError::Dependency(_, e) => e.missing(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
