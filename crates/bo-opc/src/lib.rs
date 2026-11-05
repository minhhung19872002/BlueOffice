//! Open Packaging Conventions (OPC) package reader/writer.
//!
//! ECMA-376 Part 2 (Open Packaging Conventions). Every part — including ones
//! BlueOffice does not understand — is kept as raw bytes and written back
//! unchanged, in the original order, so a read → write round trip is lossless.
//!
//! Untrusted input is guarded by [`Limits`] (zip-bomb protection).
#![forbid(unsafe_code)]

mod content_types;
mod package;
mod rels;
pub mod xml;

pub use content_types::ContentTypes;
pub use package::{Limits, Package, Part};
pub use rels::{rels_part_name, resolve_target, Relationship, TargetMode};

/// Relationship type of the main office document part.
/// ECMA-376 Part 1, §15.2.12.1 / Part 2 §9.3.
pub const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
/// Relationship type of the styles part (ECMA-376 Part 1, §15.2.12.1 / §11.3.12).
pub const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";

/// Errors produced while reading or writing a package.
#[derive(Debug, thiserror::Error)]
pub enum OpcError {
    #[error("not a valid ZIP container: {0}")]
    Zip(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("package limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("malformed XML in part {part}: {message}")]
    Xml { part: String, message: String },
    #[error("missing required part: {0}")]
    MissingPart(String),
    #[error("invalid part name: {0}")]
    InvalidPartName(String),
}

impl From<zip::result::ZipError> for OpcError {
    fn from(e: zip::result::ZipError) -> Self {
        OpcError::Zip(e.to_string())
    }
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, OpcError>;
