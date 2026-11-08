//! WordprocessingML (`.docx`) support.
//!
//! Phase 1 implements a reader for the subset needed by the first layout engine
//! (paragraphs, runs, basic run/paragraph properties, page setup, document defaults)
//! and a minimal writer used to produce test documents. Full styles, numbering,
//! tables, drawings and lossless round-trip of unknown elements arrive in Phase 2.
#![forbid(unsafe_code)]

mod reader;
mod writer;

pub use reader::read_docx;
pub use writer::write_docx;

/// Errors produced while reading a `.docx`.
#[derive(Debug, thiserror::Error)]
pub enum DocxError {
    /// Error from the OPC package layer.
    #[error(transparent)]
    Opc(#[from] bo_opc::OpcError),
    /// The package has no main document part.
    #[error("package has no main document part (officeDocument relationship)")]
    NoMainDocument,
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, DocxError>;

/// Content type of the main document part (ECMA-376 Part 1, §11.3.10).
pub const CT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
/// Content type of the styles part (ECMA-376 Part 1, §11.3.12).
pub const CT_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
