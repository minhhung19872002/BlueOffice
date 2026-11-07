//! Format detection and conversion between the formats the core can read
//! and write. Each mapping is a small function so that richer models
//! (BlueSheet / BlueSlide) can replace the intermediate types later.

use bo_binary::{LegacyKind, PptDeck, TextKind, Workbook, Worksheet};
use bo_model::Document;
use bo_odf::{Presentation, Slide, SpreadsheetDoc, TextFrame};
use bo_opc::{Limits, Package};

pub type Error = Box<dyn std::error::Error>;

/// What an input file turned out to be.
// One value per CLI invocation: the size difference between variants does not matter.
#[allow(clippy::large_enum_variant)]
pub enum Loaded {
    /// A word-processing document; `source` is the OPC package it came from
    /// (kept so VBA projects can be carried over on save).
    Document {
        doc: Document,
        source: Option<Package>,
    },
    Workbook(Workbook),
    Spreadsheet(SpreadsheetDoc),
    Deck(Presentation),
    /// A PresentationML package (`.pptx`, plain bytes after decryption).
    Pptx(Vec<u8>),
}

/// Input facts reported to the user.
#[derive(Debug, Default)]
pub struct Facts {
    pub format: &'static str,
    pub encrypted: bool,
    pub has_macros: bool,
}

/// Detect and read `bytes`. Encrypted OOXML needs `password`.
pub fn load(bytes: &[u8], password: Option<&str>) -> Result<(Loaded, Facts), Error> {
    let mut facts = Facts {
        has_macros: bo_binary::has_macros(bytes),
        ..Facts::default()
    };
    if let Some(kind) = bo_binary::detect(bytes) {
        let loaded = match kind {
            LegacyKind::Doc => {
                facts.format = "doc";
                Loaded::Document {
                    doc: bo_binary::read_doc(bytes)?,
                    source: None,
                }
            }
            LegacyKind::Xls => {
                facts.format = "xls";
                Loaded::Workbook(bo_binary::read_xls(bytes)?)
            }
            LegacyKind::Ppt => {
                facts.format = "ppt";
                Loaded::Deck(deck_to_presentation(&bo_binary::read_ppt(bytes)?))
            }
            LegacyKind::EncryptedOoxml => {
                let plain = bo_crypto::open_package(bytes, password)?;
                let (loaded, inner) = load(&plain, None)?;
                facts.format = inner.format;
                facts.has_macros = inner.has_macros;
                facts.encrypted = true;
                return Ok((loaded, facts));
            }
        };
        return Ok((loaded, facts));
    }
    if let Some(mime) = bo_odf::detect_mimetype(bytes) {
        let loaded = match mime.as_str() {
            bo_odf::MIME_ODT => {
                facts.format = "odt";
                Loaded::Document {
                    doc: bo_odf::read_odt(bytes)?,
                    source: None,
                }
            }
            bo_odf::MIME_ODS => {
                facts.format = "ods";
                Loaded::Spreadsheet(bo_odf::read_ods(bytes)?)
            }
            bo_odf::MIME_ODP => {
                facts.format = "odp";
                Loaded::Deck(bo_odf::read_odp(bytes)?)
            }
            other => return Err(format!("unsupported OpenDocument type {other}").into()),
        };
        return Ok((loaded, facts));
    }
    let pkg = Package::read(bytes, &Limits::default())?;
    let main = pkg
        .related_part("/", bo_opc::REL_OFFICE_DOCUMENT)?
        .ok_or("not an Office document (no officeDocument relationship)")?;
    let lower = main.to_ascii_lowercase();
    if lower.ends_with(".bin") {
        facts.format = "xlsb";
        Ok((Loaded::Workbook(bo_binary::read_xlsb_package(&pkg)?), facts))
    } else if lower.starts_with("/word/") {
        facts.format = "docx";
        let doc = bo_docx::read_docx(bytes)?;
        Ok((
            Loaded::Document {
                doc,
                source: Some(pkg),
            },
            facts,
        ))
    } else if lower.starts_with("/ppt/") {
        facts.format = "pptx";
        Ok((Loaded::Pptx(bytes.to_vec()), facts))
    } else {
        Err(format!("unsupported package (main part {main})").into())
    }
}

/// Map a `.ppt` deck to the ODF presentation text model.
pub fn deck_to_presentation(deck: &PptDeck) -> Presentation {
    Presentation {
        slides: deck
            .slides
            .iter()
            .enumerate()
            .map(|(i, s)| Slide {
                name: format!("Slide {}", i + 1),
                frames: s
                    .texts
                    .iter()
                    .map(|t| TextFrame {
                        class: match t.kind {
                            TextKind::Title | TextKind::CenterTitle => Some("title".into()),
                            TextKind::Body | TextKind::HalfBody | TextKind::QuarterBody => {
                                Some("outline".into())
                            }
                            TextKind::CenterBody => Some("subtitle".into()),
                            TextKind::Notes => Some("notes".into()),
                            TextKind::Other => None,
                        },
                        paragraphs: t.paragraphs.clone(),
                    })
                    .collect(),
                notes: s.notes.clone(),
            })
            .collect(),
    }
}

/// Map a binary workbook to the ODF spreadsheet model (values only; formula
/// text is A1 syntax and is not translated to OpenFormula yet).
pub fn workbook_to_ods(wb: &Workbook) -> SpreadsheetDoc {
    use bo_binary::CellValue as B;
    use bo_odf::CellValue as O;
    SpreadsheetDoc {
        sheets: wb
            .sheets
            .iter()
            .map(|s| bo_odf::Sheet {
                name: s.name.clone(),
                cells: s
                    .cells
                    .iter()
                    .filter_map(|c| {
                        let value = match &c.value {
                            B::Empty => return None,
                            B::Number(v) => O::Float(*v),
                            B::Text(t) => O::String(t.clone()),
                            B::Bool(b) => O::Boolean(*b),
                            B::Error(e) => O::String(e.clone()),
                        };
                        Some(bo_odf::Cell {
                            row: c.row,
                            col: c.col,
                            value,
                            formula: None,
                        })
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Map an ODF sheet to the simple worksheet model (for CSV export).
pub fn ods_sheet_to_worksheet(s: &bo_odf::Sheet) -> Worksheet {
    use bo_binary::CellValue as B;
    use bo_odf::CellValue as O;
    Worksheet {
        name: s.name.clone(),
        hidden: false,
        cells: s
            .cells
            .iter()
            .map(|c| bo_binary::Cell {
                row: c.row,
                col: c.col,
                value: match &c.value {
                    O::Float(v) | O::Percentage(v) | O::Currency { value: v, .. } => B::Number(*v),
                    O::Date(t) | O::Time(t) | O::String(t) => B::Text(t.clone()),
                    O::Boolean(b) => B::Bool(*b),
                },
                formula: c.formula.clone(),
                number_format: None,
            })
            .collect(),
    }
}

/// Plain text of a presentation (slides separated by blank lines).
pub fn presentation_text(p: &Presentation) -> String {
    p.slides
        .iter()
        .map(|s| {
            let mut lines: Vec<String> = s
                .frames
                .iter()
                .flat_map(|f| f.paragraphs.iter().cloned())
                .collect();
            lines.extend(s.notes.iter().map(|n| format!("> {n}")));
            lines.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Serialize `doc` as `.docx`, carrying a VBA project over from `source`.
/// Returns (bytes, macros carried).
pub fn document_to_docx(
    doc: &Document,
    source: Option<&Package>,
) -> Result<(Vec<u8>, bool), Error> {
    let bytes = bo_docx::write_docx(doc)?;
    let Some(src) = source.filter(|p| bo_binary::vba::package_has_macros(p)) else {
        return Ok((bytes, false));
    };
    let mut out = Package::read(&bytes, &Limits::default())?;
    let carried = bo_binary::vba::carry_over(src, &mut out)?;
    Ok((out.write()?, carried))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workbook_maps_to_ods_and_csv() {
        let wb = Workbook {
            sheets: vec![Worksheet {
                name: "S".into(),
                hidden: false,
                cells: vec![
                    bo_binary::Cell {
                        row: 0,
                        col: 0,
                        value: bo_binary::CellValue::Number(1.5),
                        formula: None,
                        number_format: None,
                    },
                    bo_binary::Cell {
                        row: 1,
                        col: 1,
                        value: bo_binary::CellValue::Text("a,\"b\"".into()),
                        formula: Some("A1".into()),
                        number_format: None,
                    },
                ],
            }],
            ..Workbook::default()
        };
        let ods = workbook_to_ods(&wb);
        assert_eq!(ods.sheets[0].cells.len(), 2);
        let back = bo_odf::read_ods(&bo_odf::write_ods(&ods).unwrap()).unwrap();
        let ws = ods_sheet_to_worksheet(&back.sheets[0]);
        assert_eq!(ws.to_csv(), "1.5\n,\"a,\"\"b\"\"\"\n");
    }

    #[test]
    fn docx_round_trip_with_password() {
        let doc = bo_engine::plain_text_document("Mật khẩu bảo vệ");
        let (docx, carried) = document_to_docx(&doc, None).unwrap();
        assert!(!carried);
        let enc = bo_crypto::save_package(&docx, Some("1234")).unwrap();
        assert!(load(&enc, None).is_err());
        let (loaded, facts) = load(&enc, Some("1234")).unwrap();
        assert!(facts.encrypted);
        assert_eq!(facts.format, "docx");
        let Loaded::Document { doc: back, .. } = loaded else {
            panic!("expected a document");
        };
        assert_eq!(back.plain_text(), "Mật khẩu bảo vệ");
    }
}
