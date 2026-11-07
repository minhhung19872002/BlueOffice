//! BlueSheet core on top of the workbook model ([`bo_model::sheet`]).
//!
//! * [`numfmt`] — number format codes (ECMA-376 Part 1, §18.8.30–31): parse and render.
//! * [`color`] — `CT_Color` resolution (rgb / theme + tint / indexed).
//! * [`geometry`] — column widths / row heights, offsets, hidden, merges, hit-testing.
#![forbid(unsafe_code)]

pub mod color;
pub mod geometry;
pub mod numfmt;
