//! DrawingML (ECMA-376 Part 1, §20.1 and §21.1): shared by the presentation,
//! spreadsheet and word-processing crates.
//!
//! * [`dom`] — small DOM over `quick-xml` with helpers that keep unknown
//!   children in place for lossless round trips.
//! * [`color`] — colour models and transforms (§20.1.2.3) and their resolution.
//! * [`props`], [`text`], [`table`] — readers/writers for shape properties,
//!   text bodies and tables, mapping to [`bo_model::slide`].
//! * [`theme`] — theme parts (§20.1.6).
//! * [`geom`], [`presets`] — guide formulas, path evaluation and the preset
//!   shape definitions (§20.1.9).
//! * [`freeform`], [`boolean`] — editing outlines as custom geometry and
//!   polygon boolean operations for merging shapes.
//! * [`effects`], [`textwarp`] — reflection / soft-edge geometry and the preset
//!   text warps (WordArt) of §20.1.9.19.
#![forbid(unsafe_code)]

pub mod boolean;
pub mod chart;
pub mod color;
pub mod dom;
pub mod effects;
pub mod freeform;
pub mod geom;
pub mod handles;
pub mod ink;
pub mod picture;
pub mod presets;
pub mod props;
pub mod svg;
pub mod table;
pub mod text;
pub mod textwarp;
pub mod theme;

pub use color::{resolve as resolve_color, ColorContext, Rgba};
pub use geom::{evaluate as evaluate_geometry, Evaluated, FillMode, Seg, ShapePath};
pub use presets::{preset, preset_names};
