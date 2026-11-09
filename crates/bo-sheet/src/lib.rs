//! BlueSheet core on top of the workbook model ([`bo_model::sheet`]).
//!
//! * [`numfmt`] — number format codes (ECMA-376 Part 1, §18.8.30–31): parse and render.
//! * [`color`] — `CT_Color` resolution (rgb / theme + tint / indexed).
//! * [`formula`] — the [`calc`] boundary implemented with `bo-formula` (recalculation,
//!   reference adjustment for insert/delete, AutoFill and sort).
//! * [`geometry`] — column widths / row heights, offsets, hidden, merges, hit-testing.
#![forbid(unsafe_code)]

pub mod calc;
pub mod cells;
pub mod cf;
pub mod chart;
pub mod color;
pub mod fill;
pub mod filter;
pub mod formula;
pub mod geometry;
pub mod icons;
pub mod live;
pub mod numfmt;
pub mod ops;
pub mod pivot;
pub mod render;
pub mod rowcol;
pub mod sort;
pub mod style;
pub mod validation;
pub mod viewport;
pub mod whatif;
