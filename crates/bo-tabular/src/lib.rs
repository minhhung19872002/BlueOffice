//! Legacy tabular data formats for the workbook model ([`bo_model::sheet`]),
//! CORE-FMT-004:
//!
//! * [`sylk`] — SYLK, the "symbolic link" interchange format (`.slk`): values,
//!   formulas, number formats, bold / italic, alignment, column widths, names;
//! * [`dif`] — Data Interchange Format (`.dif`): values only;
//! * [`prntext`] — formatted text, space delimited (`.prn`): the displayed text of
//!   every cell in fixed-width columns; reading is a best-effort column detection;
//! * [`dbf`] — dBASE III / IV and FoxPro tables (`.dbf`), read only.
//!
//! All readers take untrusted bytes: nothing here panics on malformed input, sizes
//! come from the data actually present (never from counts a header claims) and
//! cells stay inside the sheet limits.
#![forbid(unsafe_code)]

pub mod dbf;
pub mod dif;
pub mod prntext;
pub mod sylk;
mod text;

pub use dbf::{is_dbf, read_dbf};
pub use dif::{is_dif, read_dif, write_dif};
pub use prntext::{read_prn, write_prn};
pub use sylk::{is_sylk, read_sylk, write_sylk};

/// Errors of the readers.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TabularError {
    /// The text does not start with a SYLK `ID` record.
    #[error("not a SYLK file (no ID record)")]
    NotSylk,
    /// The text does not start with a DIF `TABLE` header.
    #[error("not a DIF file (no TABLE header)")]
    NotDif,
    /// The dBASE table header is damaged.
    #[error("invalid dBASE table: {0}")]
    Dbf(String),
    /// The data does not fit a worksheet.
    #[error("the data exceeds the sheet limits: {0}")]
    Limit(String),
}

/// Result alias of this crate.
pub type Result<T> = std::result::Result<T, TabularError>;

/// A tabular format told from the file content or, failing that, its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// SYLK (`.slk`, `.sylk`).
    Sylk,
    /// Data Interchange Format (`.dif`).
    Dif,
    /// Formatted text, space delimited (`.prn`).
    Prn,
    /// dBASE table (`.dbf`).
    Dbf,
}

impl Kind {
    /// Short format name (`slk`, `dif`, `prn`, `dbf`) — also the file extension.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Kind::Sylk => "slk",
            Kind::Dif => "dif",
            Kind::Prn => "prn",
            Kind::Dbf => "dbf",
        }
    }

    /// The format of a file extension (without the dot, any case).
    #[must_use]
    pub fn from_ext(ext: &str) -> Option<Kind> {
        match ext.to_ascii_lowercase().as_str() {
            "slk" | "sylk" => Some(Kind::Sylk),
            "dif" => Some(Kind::Dif),
            "prn" => Some(Kind::Prn),
            "dbf" => Some(Kind::Dbf),
            _ => None,
        }
    }
}

/// Recognise a tabular format: SYLK and DIF by their first record, then by the
/// extension of `name`. A `.dbf` name must come with a plausible table header; a
/// `.prn` file is any text (binary printer output is refused).
#[must_use]
pub fn detect(bytes: &[u8], name: &str) -> Option<Kind> {
    if is_sylk(bytes) {
        return Some(Kind::Sylk);
    }
    if is_dif(bytes) {
        return Some(Kind::Dif);
    }
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
    match Kind::from_ext(ext)? {
        Kind::Dbf => is_dbf(bytes).then_some(Kind::Dbf),
        Kind::Prn => prntext::is_text(bytes).then_some(Kind::Prn),
        // A `.slk` / `.dif` name without the format's first record.
        Kind::Sylk | Kind::Dif => None,
    }
}

/// Read `bytes` as the given format into a workbook with one sheet. `decimal` is
/// the decimal separator of numbers in `.prn` text.
pub fn read(bytes: &[u8], kind: Kind, decimal: char) -> Result<bo_model::sheet::Workbook> {
    match kind {
        Kind::Sylk => read_sylk(bytes),
        Kind::Dif => read_dif(bytes),
        Kind::Prn => read_prn(bytes, decimal),
        Kind::Dbf => read_dbf(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_by_content_then_name() {
        assert_eq!(detect(b"ID;PWXL;N;E\r\nE\r\n", "a.txt"), Some(Kind::Sylk));
        assert_eq!(
            detect(b"TABLE\r\n0,1\r\n\"EXCEL\"\r\n", "a.txt"),
            Some(Kind::Dif)
        );
        assert_eq!(detect(b"a  b\r\n", "a.PRN"), Some(Kind::Prn));
        assert_eq!(detect(b"\x1b%-12345X@PJL", "job.prn"), None);
        assert_eq!(detect(b"a  b\r\n", "a.txt"), None);
        assert_eq!(detect(b"not sylk", "a.slk"), None);
        assert_eq!(detect(b"x", "a.dbf"), None);
        assert_eq!(Kind::from_ext("SYLK"), Some(Kind::Sylk));
        assert_eq!(Kind::Dbf.name(), "dbf");
    }
}
