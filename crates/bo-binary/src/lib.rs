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
//! * VBA — [MS-OVBA]: macro detection, module listing and preservation on
//!   save ([`vba`]); macros are never executed.
//!
//! All parsers are read-only and bounds-checked: malformed input yields an
//! error, never a panic. Compound files are opened with the `cfb` crate ([MS-CFB]).
#![forbid(unsafe_code)]

mod bytes;
#[cfg(test)]
mod corpus_gen;
pub mod doc;
pub mod ppt;
mod ppt_draw;
pub mod sheet;
pub mod vba;
// The project writer builds generated test and corpus files; the browser build
// (WASM) only reads.
#[cfg(not(target_arch = "wasm32"))]
pub mod vba_write;
pub mod xls;
pub mod xls_formula;
pub mod xlsb;

pub use doc::{read_doc, read_doc_with, DocOptions};
pub use ppt::{read_ppt, PptDeck, PptSlide, PptText, TextKind};
pub use ppt_draw::{
    preset_for, PptParagraph, PptPicture, PptPlaceholder, PptRichText, PptRun, PptShape,
};
pub use sheet::{Cell, CellValue, DefinedName, Workbook, Worksheet};
pub use vba::{
    has_macros, read_vba_from_file, read_vba_project, VbaModule, VbaModuleKind, VbaProject,
};
pub use xls::read_xls;
pub use xlsb::{read_xlsb, read_xlsb_package};

use std::io::{Cursor, Read};

/// Errors produced by the binary readers.
#[derive(Debug, thiserror::Error)]
pub enum BinaryError {
    /// The compound file (CFB) container could not be read.
    #[error("compound file error: {0}")]
    Cfb(#[from] std::io::Error),
    /// A required stream, storage or part is absent.
    #[error("missing stream or part: {0}")]
    Missing(String),
    /// The binary structures are inconsistent or truncated.
    #[error("malformed file: {0}")]
    Malformed(String),
    /// The file is encrypted; decryption must happen first.
    #[error("the file is encrypted or password protected")]
    Encrypted,
    /// The file uses a feature or version this crate does not read.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Error from the OPC package layer.
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

    pub(crate) fn has_storage(&self, name: &str) -> bool {
        self.inner.is_storage(name)
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

    /// [`Self::read`] refusing streams longer than `cap` bytes.
    pub(crate) fn read_capped(&mut self, name: &str, cap: u64) -> Result<Vec<u8>> {
        if !self.inner.is_stream(name) {
            return Err(BinaryError::Missing(name.to_owned()));
        }
        let len = self.inner.entry(name)?.len();
        if len > cap {
            return Err(malformed(format!("stream {name} is {len} bytes")));
        }
        self.read(name)
    }
}

/// Whether `bytes` starts with the compound file signature ([MS-CFB] §2.2).
pub fn is_compound_file(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
}

/// Legacy format of a compound file, by the streams it contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyKind {
    /// Word 97-2003 document (`WordDocument` stream, [MS-DOC]).
    Doc,
    /// Excel 97-2003 workbook (`Workbook`/`Book` stream, [MS-XLS]).
    Xls,
    /// PowerPoint 97-2003 presentation (`PowerPoint Document` stream, [MS-PPT]).
    Ppt,
    /// An encrypted OOXML package ([MS-OFFCRYPTO] `EncryptionInfo` + `EncryptedPackage`).
    EncryptedOoxml,
}

/// Identify a compound file's format.
pub fn detect(bytes: &[u8]) -> Option<LegacyKind> {
    if !is_compound_file(bytes) {
        return None;
    }
    let cfb = Cfb::open(bytes).ok()?;
    if cfb.has("/EncryptedPackage") {
        Some(LegacyKind::EncryptedOoxml)
    } else if cfb.has("/WordDocument") {
        Some(LegacyKind::Doc)
    } else if cfb.has("/Workbook") || cfb.has("/Book") {
        Some(LegacyKind::Xls)
    } else if cfb.has("/PowerPoint Document") {
        Some(LegacyKind::Ppt)
    } else {
        None
    }
}

#[cfg(test)]
pub(crate) mod test_util {
    use std::io::Write;

    /// Build a compound file with the given streams (parent storages are
    /// created automatically) and extra empty storages.
    pub fn cfb_file(streams: &[(&str, Vec<u8>)], storages: &[&str]) -> Vec<u8> {
        let mut cf = cfb::CompoundFile::create(std::io::Cursor::new(Vec::new())).unwrap();
        let ensure = |cf: &mut cfb::CompoundFile<std::io::Cursor<Vec<u8>>>, path: &str| {
            let mut cur = String::new();
            for seg in path.trim_start_matches('/').split('/') {
                cur.push('/');
                cur.push_str(seg);
                if !cf.is_storage(&cur) {
                    cf.create_storage(&cur).unwrap();
                }
            }
        };
        for s in storages {
            ensure(&mut cf, s);
        }
        for (name, data) in streams {
            if let Some((parent, _)) = name.rsplit_once('/') {
                if !parent.is_empty() {
                    ensure(&mut cf, parent);
                }
            }
            let mut st = cf.create_stream(name).unwrap();
            st.write_all(data).unwrap();
        }
        cf.flush().unwrap();
        cf.into_inner().into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_kinds() {
        let f = |n: &str| test_util::cfb_file(&[(n, vec![0; 8])], &[]);
        assert_eq!(detect(&f("/WordDocument")), Some(LegacyKind::Doc));
        assert_eq!(detect(&f("/Workbook")), Some(LegacyKind::Xls));
        assert_eq!(detect(&f("/PowerPoint Document")), Some(LegacyKind::Ppt));
        assert_eq!(
            detect(&f("/EncryptedPackage")),
            Some(LegacyKind::EncryptedOoxml)
        );
        assert_eq!(detect(&f("/Other")), None);
        assert_eq!(detect(b"PK"), None);
    }
}
