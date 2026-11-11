//! Spreadsheet formula engine for BlueSheet.
//!
//! Clean-room implementation based on ECMA-376 Part 1, §18.17 (Formulas):
//!
//! * [`parse`] / [`parse_with`] turn formula text (A1 or R1C1) into an [`Expr`];
//!   [`format()`] / [`format_with`] turn it back into text (round-trip stable).
//! * [`adjust_for_change`] and [`offset_relative`] rewrite references for
//!   inserted/deleted rows and columns and for copy/paste.
//! * [`evaluate`] evaluates an expression against any [`CellProvider`].
//! * [`Workbook`] is an in-memory provider with a dependency graph, incremental
//!   recalculation, circular-reference detection (optionally iterative),
//!   volatile functions and dynamic-array spilling.
//! * [`date`] implements serial dates for the 1900 (with the fictitious
//!   1900-02-29) and 1904 date systems.
//!
//! ```
//! use bo_formula::{Value, Workbook};
//!
//! let mut wb = Workbook::with_sheets(&["Sheet1"]);
//! wb.set_a1(0, "A1", "2").unwrap();
//! wb.set_a1(0, "A2", "=A1*21").unwrap();
//! wb.recalculate();
//! assert_eq!(wb.value_a1(0, "A2"), Value::Number(42.0));
//! ```
#![forbid(unsafe_code)]

pub mod ast;
pub mod criteria;
pub mod date;
mod eval;
mod format;
mod functions;
mod fxhash;
mod input;
mod lexer;
mod lookupindex;
mod numfmt;
mod parser;
pub mod provider;
mod rangeindex;
pub mod reference;
mod shift;
pub mod stat;
mod storage;
mod trace;
pub mod value;
mod workbook;

pub use ast::{BinaryOp, Expr, StructItem, StructRef, UnaryOp};
pub use date::DateSystem;
pub use eval::{evaluate, EvalConfig, FormatInput, TextFormatter};
pub use format::{format, format_reference, format_with};
pub use functions::{function_names, is_volatile_function, linear_fit};
pub use input::localized_input;
pub use lookupindex::ExactIndex;
pub use numfmt::{format_number, format_text};
pub use parser::{
    parse, parse_with, ParseError, ParseOptions, RefStyle, MAX_FORMULA_CHARS, MAX_NESTING,
    MAX_TREE_DEPTH,
};
pub use provider::{CellProvider, TableInfo, WebError, WebProvider};
pub use reference::{CellAddr, RefKind, Reference, SheetRef};
pub use shift::{
    adjust_for_change, adjust_for_move, offset_relative, MovedBlock, StructuralChange,
};
pub use storage::{from_storage, storage_prefix, to_storage};
pub use trace::{trace_formula, FormulaTrace, TraceStep};
pub use value::{Area, Array, ErrorKind, ImageValue, Lambda, Value};
pub use workbook::{CellId, IterativeConfig, Workbook};
