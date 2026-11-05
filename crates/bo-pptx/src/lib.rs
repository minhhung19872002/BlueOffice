//! PresentationML (.pptx) ⇄ [`bo_model::slide::Presentation`] and slide rendering.
//!
//! ECMA-376 Part 1, §19 (PresentationML) and §13.3 (PresentationML parts).
//!
//! * [`read_pptx`] / [`write_pptx`]: lossless round trip — unmodelled XML is kept
//!   as opaque nodes at its original position, unmodelled parts byte-for-byte.
//! * [`inherit`]: placeholder inheritance master → layout → slide (§19.3.1.36).
//! * [`render_slide`] / [`render_thumbnail`]: slide → [`bo_render::PageList`].
#![forbid(unsafe_code)]

pub mod inherit;
mod pml;
mod read;
mod render;
mod text_layout;
mod write;

pub use pml::{parse_node, write_node, URI_TABLE};
pub use read::read_pptx;
pub use render::{render_slide, render_thumbnail, render_with, RenderOptions};
pub use write::write_pptx;

/// Relationship type namespace (transitional), ECMA-376 Part 1, §15.2.
pub(crate) const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";

/// Errors produced by the PresentationML reader/writer.
#[derive(Debug, thiserror::Error)]
pub enum PptxError {
    #[error(transparent)]
    Opc(#[from] bo_opc::OpcError),
    #[error("no presentation part in package")]
    NoPresentation,
    #[error("missing part {0}")]
    MissingPart(String),
    #[error("slide index {0} out of range")]
    NoSuchSlide(usize),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, PptxError>;

/// Does a relationship type end with `/kind` (transitional or strict namespace)?
pub(crate) fn rel_is(rel_type: &str, kind: &str) -> bool {
    rel_type
        .rsplit('/')
        .next()
        .is_some_and(|last| last == kind)
}
