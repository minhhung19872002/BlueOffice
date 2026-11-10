//! Open Packaging Conventions (OPC) package reader/writer.
//!
//! ECMA-376 Part 2 (Open Packaging Conventions). Every part — including ones
//! BlueOffice does not understand — is kept as raw bytes and written back
//! unchanged, in the original order, so a read → write round trip is lossless.
//!
//! A package may also be stored as one XML file ("Flat OPC", [`flat`]):
//! [`Package::read`] accepts both containers.
//!
//! Untrusted input is guarded by [`Limits`] (zip-bomb protection).
#![forbid(unsafe_code)]

mod content_types;
pub mod custom;
pub mod flat;
mod package;
mod rels;
pub mod xml;

pub use content_types::ContentTypes;
pub use package::{Limits, Package, Part};
pub use rels::{
    parse as parse_relationships, rels_part_name, resolve_target, Relationship, TargetMode,
};

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
    /// The ZIP container could not be read or written.
    #[error("not a valid ZIP container: {0}")]
    Zip(String),
    /// Reading or writing the stream failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A size or entry-count safety limit was hit.
    #[error("package limit exceeded: {0}")]
    LimitExceeded(String),
    /// An XML part is not well formed.
    #[error("malformed XML in part {part}: {message}")]
    Xml {
        /// Name of the offending part.
        part: String,
        /// Parser error message.
        message: String,
    },
    /// A required part (e.g. `[Content_Types].xml`) is absent.
    #[error("missing required part: {0}")]
    MissingPart(String),
    /// A part name violates the OPC part-name rules.
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
