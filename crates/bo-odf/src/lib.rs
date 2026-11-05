//! OpenDocument Format (OASIS ODF 1.3) support.
//!
//! * `.odt` — read into / write from [`bo_model::Document`] ([`read_odt`], [`write_odt`]):
//!   paragraphs and headings, spans with automatic styles (bold, italic,
//!   underline, strike-through, size, font, colour), paragraph alignment,
//!   indents, spacing, line height, page breaks and page layout per section.
//! * `.ods` — cells of every table into [`SpreadsheetDoc`] and back
//!   ([`read_ods`], [`write_ods`]).
//! * `.odp` — text frames of every slide into [`Presentation`] ([`read_odp`]).
//!
//! Packages follow ODF 1.3 Part 2 (Packages): the `mimetype` entry is written
//! first and uncompressed, and `META-INF/manifest.xml` lists every file.
#![forbid(unsafe_code)]

mod dom;
mod odp;
mod ods;
mod odt_read;
mod odt_write;
mod package;
mod units;

pub use odp::{read_odp, Presentation, Slide, TextFrame};
pub use ods::{read_ods, write_ods, Cell, CellValue, Sheet, SpreadsheetDoc};
pub use odt_read::read_odt;
pub use odt_write::write_odt;
pub use package::{detect_mimetype, OdfPackage};

/// `mimetype` of text documents (ODF 1.3 Part 1, Appendix C).
pub const MIME_ODT: &str = "application/vnd.oasis.opendocument.text";
/// `mimetype` of spreadsheets.
pub const MIME_ODS: &str = "application/vnd.oasis.opendocument.spreadsheet";
/// `mimetype` of presentations.
pub const MIME_ODP: &str = "application/vnd.oasis.opendocument.presentation";

/// Errors produced by this crate.
#[derive(Debug, thiserror::Error)]
pub enum OdfError {
    #[error("not a valid ZIP container: {0}")]
    Zip(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("package limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("missing package entry: {0}")]
    Missing(String),
    #[error("malformed XML in {part}: {message}")]
    Xml { part: String, message: String },
    #[error("unexpected document type: {0}")]
    WrongType(String),
}

impl From<zip::result::ZipError> for OdfError {
    fn from(e: zip::result::ZipError) -> Self {
        OdfError::Zip(e.to_string())
    }
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, OdfError>;

/// XML namespaces (ODF 1.3 Part 3, §1.5 / Part 2 §4).
pub mod ns {
    pub const OFFICE: &str = "urn:oasis:names:tc:opendocument:xmlns:office:1.0";
    pub const STYLE: &str = "urn:oasis:names:tc:opendocument:xmlns:style:1.0";
    pub const TEXT: &str = "urn:oasis:names:tc:opendocument:xmlns:text:1.0";
    pub const TABLE: &str = "urn:oasis:names:tc:opendocument:xmlns:table:1.0";
    pub const DRAW: &str = "urn:oasis:names:tc:opendocument:xmlns:drawing:1.0";
    pub const FO: &str = "urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0";
    pub const SVG: &str = "urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0";
    pub const PRESENTATION: &str = "urn:oasis:names:tc:opendocument:xmlns:presentation:1.0";
    pub const META: &str = "urn:oasis:names:tc:opendocument:xmlns:meta:1.0";
    pub const MANIFEST: &str = "urn:oasis:names:tc:opendocument:xmlns:manifest:1.0";
    pub const OF: &str = "urn:oasis:names:tc:opendocument:xmlns:of:1.2";
}
