//! BlueSheet core on top of the workbook model ([`bo_model::sheet`]).
//!
//! * [`numfmt`] — number format codes (ECMA-376 Part 1, §18.8.30–31): parse and render.
//! * [`color`] — `CT_Color` resolution (rgb / theme + tint / indexed).
//! * [`formula`] — the [`calc`] boundary implemented with `bo-formula` (recalculation,
//!   reference adjustment for insert/delete, AutoFill and sort).
//! * [`flashfill`] — Flash Fill: learn a text pattern from examples and fill a column.
//! * [`geometry`] — column widths / row heights, offsets, hidden, merges, hit-testing.
#![forbid(unsafe_code)]

pub mod advfilter;
pub mod analysis;
pub mod calc;
pub mod cells;
pub mod cf;
pub mod chart;
pub mod chart_style;
pub mod color;
pub mod datatools;
pub mod fill;
pub mod filter;
pub mod flashfill;
pub mod formctl;
pub mod formula;
pub mod geometry;
pub mod headerfooter;
pub mod icons;
pub mod live;
pub mod numfmt;
pub mod ops;
pub mod outline;
pub mod pivot;
pub mod render;
pub mod rowcol;
pub mod scenario;
pub mod slicer;
pub mod solver;
pub(crate) mod solver_ga;
pub(crate) mod solver_nl;
pub mod solver_report;
pub mod sort;
pub mod sparkline;
pub mod style;
pub mod table;
pub mod timeline;
pub mod validation;
pub mod viewport;
pub mod whatif;
