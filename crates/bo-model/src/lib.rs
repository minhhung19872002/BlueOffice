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
pub mod chart_part;
pub mod chart_style;
pub mod custom_xml;
pub mod diagram;
pub mod field;
pub mod form;
pub mod glossary;
pub mod index;
pub mod mention;
pub mod ole;
pub mod picture;
pub mod run_fx;
pub mod sheet;
pub mod sigline;
pub mod slide;
pub mod units;
pub mod write;

pub use form::{DocProtection, FormField, FormFieldKind};
pub use sigline::{SignatureLine, SignedLine};
pub use units::{Emu, HalfPoints, Twips};
pub use write::{
    fingerprint, format_number, AbstractNum, Alignment, Anchor, Block, Border, Borders, Cell,
    CellMerge, CellPlace, CellProps, CellPropsChange, CellVAlign, Color, ColumnDef, Columns,
    Comment, ContentControl, ControlKind, DocDesign, Document, DropCap, FrameProps, GridChange,
    HeaderFooter, HeightRule, HfParts, Hyphenation, Image, Lang, Level, LineNumberRestart,
    LineNumbering, LineSpacing, NotePos, NoteProps, NoteRestart, Num, NumFormat, NumRef, Numbering,
    PageBorder, PageBorderDisplay, PageBorders, PageSetup, ParaProps, ParaPropsChange, Paragraph,
    Raw, RevMark, RevisionInfo, Row, RowProps, RowPropsChange, Run, RunContent, RunProps,
    RunPropsChange, Section, SectionBreak, SectionChange, Shape, ShapeArt, ShapeLine,
    SourcePackage, Style, StyleKind, StyleSheet, TabAlign, TabLeader, TabStop, Table, TableFloat,
    TableGeometry, TableProps, TablePropsChange, TableView, TableWidth, TextAnchor, TextDirection,
    TextPath, ThemeFonts, Underline, VMerge, VertAlign, Wrap, WrapText,
};
