//! BlueOffice document model.
//!
//! The model stores values in the units used by the file-format specifications
//! (twips, half-points, EMU). Conversion to device pixels happens only in the
//! rendering layer (see `bo-render`).
//!
//! Phase 1 covers the word-processing subset needed to lay out simple documents:
//! sections with page setup, paragraphs and runs with basic formatting.
#![forbid(unsafe_code)]

pub mod units;
pub mod write;

pub use units::{Emu, HalfPoints, Twips};
pub use write::{
    Alignment, Block, Color, Document, LineSpacing, PageSetup, ParaProps, Paragraph, Run,
    RunContent, RunProps, Section, Underline,
};
