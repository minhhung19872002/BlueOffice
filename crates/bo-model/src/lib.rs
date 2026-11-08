//! BlueOffice document model.
//!
//! The model stores values in the units used by the file-format specifications
//! (twips, half-points, EMU). Conversion to device pixels happens only in the
//! rendering layer (see `bo-render`).
//!
//! Phase 1 covers the word-processing subset needed to lay out simple documents:
//! sections with page setup, paragraphs and runs with basic formatting.
#![forbid(unsafe_code)]

pub mod change;
pub mod index;
pub mod sheet;
pub mod slide;
pub mod units;
pub mod write;

pub use units::{Emu, HalfPoints, Twips};
pub use write::{
    fingerprint, format_number, AbstractNum, Alignment, Anchor, Block, Border, Borders, Cell,
    CellMerge, CellPlace, CellProps, CellPropsChange, CellVAlign, Color, Columns, Comment,
    Document, GridChange, HeaderFooter, HeightRule, Image, Level, LineSpacing, NotePos, NoteProps,
    NoteRestart, Num, NumFormat, NumRef, Numbering, PageSetup, ParaProps, ParaPropsChange,
    Paragraph, Raw, RevMark, RevisionInfo, Row, RowProps, RowPropsChange, Run, RunContent,
    RunProps, RunPropsChange, Section, SectionBreak, SectionChange, SourcePackage, Style,
    StyleKind, StyleSheet, TabAlign, TabLeader, TabStop, Table, TableGeometry, TableProps,
    TablePropsChange, TableView, TableWidth, Underline, VMerge, VertAlign, Wrap, WrapText,
};
