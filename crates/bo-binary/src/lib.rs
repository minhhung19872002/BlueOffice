//! Readers for the legacy binary Office formats.
//!
//! * `.doc` — [MS-DOC] Word 97–2003: FIB, piece table (CLX), character and
//!   paragraph properties through FKPs, stylesheet, font table and section
//!   page setup → [`bo_model::Document`] ([`doc::read_doc`]).
//! * `.xls` — [MS-XLS] BIFF8: workbook globals (sheets, SST, formats, XF,
//!   1904 flag, names, external sheets) and cell records including formulas
//!   rendered back to text ([`xls::read_xls`]).
//! * `.xlsb` — [MS-XLSB]: shared strings, styles and cell records
//!   ([`xlsb::read_xlsb`]).
//! * `.ppt` — [MS-PPT]: slide text from `SlideListWithText` and slide drawings
//!   ([`ppt::read_ppt`]).
//!
//! All parsers are read-only and bounds-checked: malformed input yields an
//! error, never a panic. Compound files are opened with the `cfb` crate ([MS-CFB]).
#![forbid(unsafe_code)]

mod bytes;
pub mod doc;
pub mod ppt;
pub mod sheet;
pub mod xls;
mod xls_formula;
pub mod xlsb;

pub use doc::{read_doc, read_doc_with, DocOptions};
pub use ppt::{read_ppt, PptDeck, PptSlide};
pub use sheet::{Cell, CellValue, Workbook, Worksheet};
pub use xls::{read_xls, XlsWorkbook};
pub use xlsb::read_xlsb;

use std::io::{Cursor, Read};

/// Errors produced by the binary readers.
#[derive(Debug, thiserror::Error)]
pub enum BinaryError {
    #[error("compound file error: {0}")]
    Cfb(#[from] std::io::Error),
    #[error("missing stream or part: {0}")]
    Missing(String),
    #[error("malformed file: {0}")]
    Malformed(String),
    #[error("the file is encrypted or password protected")]
    Encrypted,
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Opc(#[from] bo_opc::OpcError),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, BinaryError>;

/// Maximum size of one compound-file stream read into memory (512 MiB).
pub const MAX_STREAM: u64 = 512 * 1024 * 1024;

pub(crate) fn malformed(msg: impl Into<String>) -> BinaryError {
    BinaryError::Malformed(msg.into())
}

/// An opened compound file.
pub(crate) struct Cfb<'a> {
    inner: cfb::CompoundFile<Cursor<&'a [u8]>>,
}

impl<'a> Cfb<'a> {
    pub(crate) fn open(bytes: &'a [u8]) -> Result<Self> {
        Ok(Cfb {
            inner: cfb::CompoundFile::open(Cursor::new(bytes))?,
        })
    }

    pub(crate) fn has(&self, name: &str) -> bool {
        self.inner.is_stream(name)
    }

    /// Read a stream fully (bounded by [`MAX_STREAM`]).
    pub(crate) fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        if !self.inner.is_stream(name) {
            return Err(BinaryError::Missing(name.to_owned()));
        }
        let len = self.inner.entry(name)?.len();
        if len > MAX_STREAM {
            return Err(malformed(format!("stream {name} is {len} bytes")));
        }
        let mut s = self.inner.open_stream(name)?;
        let mut out = Vec::with_capacity(len.min(64 << 20) as usize);
        (&mut s).take(MAX_STREAM).read_to_end(&mut out)?;
        Ok(out)
    }
}

/// Whether `bytes` starts with the compound file signature ([MS-CFB] §2.2).
pub fn is_compound_file(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
}
