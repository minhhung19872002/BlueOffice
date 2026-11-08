//! PresentationML (.pptx) ⇄ [`bo_model::slide::Presentation`] and slide rendering.
//!
//! ECMA-376 Part 1, §19 (PresentationML) and §13.3 (PresentationML parts).
//!
//! * [`read_pptx`] / [`write_pptx`]: lossless round trip — unmodelled XML is kept
//!   as opaque nodes at its original position, unmodelled parts byte-for-byte.
//! * [`inherit`]: placeholder inheritance master → layout → slide (§19.3.1.36).
//! * [`render_slide`] / [`render_thumbnail`]: slide → [`bo_render::PageList`].
#![forbid(unsafe_code)]

pub mod anim;
pub mod build;
mod chart_render;
pub mod inherit;
mod pml;
mod read;
mod render;
pub mod table_style;
mod text_layout;
mod write;

pub use pml::{parse_node, visit, write_node, URI_TABLE};
pub use read::read_pptx;
pub use render::{
    render_slide, render_thumbnail, render_with, shape_text_geometry, table_cell_text_geometry,
    RenderOptions, ShapeRange,
};
pub use text_layout::{CaretLine, TextGeometry};
pub use write::write_pptx;

/// Relationship type namespace (transitional), ECMA-376 Part 1, §15.2.
pub(crate) const REL_NS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";

/// Errors produced by the PresentationML reader/writer.
#[derive(Debug, thiserror::Error)]
pub enum PptxError {
    /// Error from the OPC package layer.
    #[error(transparent)]
    Opc(#[from] bo_opc::OpcError),
    /// The package has no presentation part.
    #[error("no presentation part in package")]
    NoPresentation,
    /// A referenced part is absent.
    #[error("missing part {0}")]
    MissingPart(String),
    /// The slide index is out of range.
    #[error("slide index {0} out of range")]
    NoSuchSlide(usize),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, PptxError>;

/// Is `bytes` a PresentationML package (`.pptx`, `.pptm`, `.potx`, `.ppsx`)?
/// Decided by the content type of the main part (ECMA-376 Part 1, §13.3.6).
pub fn is_pptx(bytes: &[u8]) -> bool {
    let Ok(pkg) = bo_opc::Package::read(bytes, &bo_opc::Limits::default()) else {
        return false;
    };
    let Ok(Some(main)) = pkg.related_part("/", bo_opc::REL_OFFICE_DOCUMENT) else {
        return false;
    };
    pkg.content_types()
        .ok()
        .and_then(|ct| ct.get(&main).map(|t| t.contains("presentationml")))
        .unwrap_or(false)
}

/// Wrap an unmodelled child of `p:presentation` (e.g. `p:notesMasterIdLst`)
/// so the writer puts it at its schema position (§19.2.1.26 CT_Presentation).
pub fn presentation_extra(xml: bo_model::slide::XmlElement) -> bo_model::slide::Extra {
    let key = bo_drawingml::dom::key_for(read::PRES, &xml.name, 0);
    bo_model::slide::Extra { key, xml }
}

/// Relationship type namespace, for parts created by editors.
pub const RELATIONSHIP_NS: &str = REL_NS;

/// Does a relationship type end with `/kind` (transitional or strict namespace)?
pub(crate) fn rel_is(rel_type: &str, kind: &str) -> bool {
    rel_type.rsplit('/').next().is_some_and(|last| last == kind)
}
