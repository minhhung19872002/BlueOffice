//! Embedded and linked BlueSheet workbooks in BlueDoc and BlueSlide (CORE-EMBED-001).
//!
//! An object is shown through a preview picture drawn by the BlueSheet renderer from
//! the area of the workbook it shows ([`crate::sheet::SheetEngine::embed_area`]); the
//! workbook itself is an embedded SpreadsheetML package (ProgID
//! [`bo_model::ole::PROG_ID_XLSX`]) or an external file a linked object names. Objects
//! read from files keep their parts byte-for-byte until they are edited; nothing in an
//! object is ever run.
//!
//! The commands ([`OleCommand`], `cmd` starting with `ole`) are shared by BlueDoc
//! ([`crate::Engine::doc_ole_command`]) and BlueSlide
//! ([`crate::Engine::slide_ole_command`]).

use crate::sheet::SheetEngine;
use crate::{EngineError, Result};
use base64::Engine as _;
use bo_model::sheet::{CellRange, CellRef};
use bo_text::FontManager;
use serde::{Deserialize, Serialize};

/// Pixels per point of a preview picture.
pub const PREVIEW_SCALE: f32 = 2.0;
/// Largest number of edited rows / columns a command may send.
const MAX_GRID: usize = 200;

/// A linked file's current bytes, read by the host.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkData {
    /// Source as listed by `oleLinks`.
    pub source: String,
    /// File bytes, base64.
    pub data: String,
}

/// An embedded-object command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum OleCommand {
    /// State of the object at the selection.
    OleInfo {
        /// Include the cell inputs of the shown area.
        #[serde(default)]
        cells: bool,
        /// Include the embedded workbook (base64).
        #[serde(default)]
        data: bool,
    },
    /// Insert a workbook object.
    OleInsert {
        /// Workbook bytes (base64); `None` = a new empty workbook.
        #[serde(default)]
        data: Option<String>,
        /// Area to show (`Sheet1!A1:C5`); `None` = the workbook's print area or used cells.
        #[serde(default)]
        range: Option<String>,
        /// Cell inputs written at the top-left of the area first (e.g. pasted cells).
        #[serde(default)]
        cells: Option<Vec<Vec<String>>>,
        /// Link to this file instead of embedding (desktop path).
        #[serde(default)]
        link: Option<String>,
        /// A linked object updates automatically.
        #[serde(default)]
        auto_update: bool,
    },
    /// Write cell inputs into the shown area of the selected embedded workbook,
    /// re-embed it and draw its preview again.
    OleEdit {
        /// Inputs row by row from the area's top-left cell; the area grows or shrinks to them.
        cells: Vec<Vec<String>>,
    },
    /// Replace the selected object's embedded workbook (edited elsewhere).
    OleSetData {
        /// Workbook bytes, base64.
        data: String,
    },
    /// Linked files of the document.
    OleLinks,
    /// Refresh linked objects from their files' current bytes.
    OleUpdateLinks {
        /// One entry per source.
        sources: Vec<LinkData>,
    },
}

impl OleCommand {
    /// Parses a JSON command when it is an object command (`cmd` starts with `ole`).
    pub fn parse(v: &serde_json::Value) -> Option<OleCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !c.starts_with("ole") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// The object at the selection as seen by the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OleInfo {
    /// ProgID (a file-format identifier; not for display).
    pub prog_id: String,
    /// The object holds a workbook BlueSheet can open.
    pub workbook: bool,
    /// Linked (else embedded).
    pub linked: bool,
    /// Linked file (path for desktop hosts).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Item of the linked file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
    /// A linked object updates automatically.
    pub auto_update: bool,
    /// Shown size in points.
    pub width: f64,
    /// Shown size in points.
    pub height: f64,
    /// Sheet of the shown area (embedded workbooks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
    /// Shown area (`A1:C5`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<String>,
    /// Cell inputs of the area, row by row (`oleInfo` with `cells`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cells: Option<Vec<Vec<String>>>,
    /// Embedded workbook, base64 (`oleInfo` with `data`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// A link of the document (`oleLinks`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkOut {
    /// Source as stored (relationship target or path).
    pub source: String,
    /// File path the host reads (a `file:` URI turned into a path).
    pub path: String,
    /// Item inside the file.
    pub item: String,
    /// Objects linked to it.
    pub count: usize,
}

/// A workbook object built for embedding or after an edit.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    /// The workbook (`.xlsx`).
    pub xlsx: Vec<u8>,
    /// Preview picture (PNG at [`PREVIEW_SCALE`]).
    pub png: Vec<u8>,
    /// Size of the shown area (points).
    pub size: (f64, f64),
    /// Size of the area before the edit (points), when a workbook was given.
    pub old_size: Option<(f64, f64)>,
    /// Sheet name of the shown area.
    pub sheet: String,
    /// Shown area.
    pub range: CellRange,
}

/// Decode base64 command data.
pub fn decode(data: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| EngineError::BadCommand("base64".into()))
}

/// Encode bytes as base64.
pub fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// `Sheet1!R1C1:R3C4` (a LINK item, §17.16.5.32) → `Sheet1!A1:D3`; A1 items are kept.
pub fn a1_item(item: &str) -> String {
    let (sheet, refs) = match item.rsplit_once('!') {
        Some((s, r)) => (Some(s), r),
        None => (None, item),
    };
    let cell = |s: &str| -> Option<CellRef> {
        let s = s.trim().to_ascii_uppercase();
        let rest = s.strip_prefix('R')?;
        let (r, c) = rest.split_once('C')?;
        let (r, c): (u32, u32) = (r.parse().ok()?, c.parse().ok()?);
        (r > 0 && c > 0).then(|| CellRef::new(r - 1, c - 1))
    };
    let parts: Vec<Option<CellRef>> = refs.split(':').map(cell).collect();
    let a1 = match parts.as_slice() {
        [Some(a)] => Some(a.to_string()),
        [Some(a), Some(b)] => Some(CellRange::new(*a, *b).to_string()),
        _ => None,
    };
    match (sheet, a1) {
        (Some(s), Some(r)) => format!("{s}!{r}"),
        (None, Some(r)) => r,
        _ => item.to_owned(),
    }
}

/// A `file:` URI as a local path (`file:///C:/a%20b.xlsx` → `C:/a b.xlsx`); other
/// sources are kept.
pub fn source_path(source: &str) -> String {
    let Some(rest) = source.strip_prefix("file://") else {
        return source.to_owned();
    };
    let rest = rest
        .strip_prefix('/')
        .filter(|r| r.as_bytes().get(1) == Some(&b':'))
        .unwrap_or(rest);
    let mut out = Vec::new();
    let b = rest.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&rest[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Build a workbook object: open `src` (else a new workbook), set the shown area to
/// `range`, write `cells` at its top-left (the area follows their extent), draw the
/// preview and save the workbook.
pub fn build(
    fonts: &mut FontManager,
    src: Option<&[u8]>,
    range: Option<&str>,
    cells: Option<&[Vec<String>]>,
) -> Result<Built> {
    SheetEngine::with_fonts(fonts, |e| {
        match src {
            Some(b) => {
                e.open(b, "object.xlsx")?;
            }
            None => {
                e.new_workbook("object.xlsx")?;
            }
        }
        if let Some(r) = range.filter(|r| !r.trim().is_empty()) {
            e.set_embed_area(r)?;
        }
        let (sheet, mut area) = e.embed_area()?;
        let old_size = match src {
            Some(_) => Some(e.area_size(sheet, area)?),
            None => None,
        };
        if let Some(cells) = cells {
            let rows = cells.len().min(MAX_GRID);
            let cols = cells.iter().map(Vec::len).max().unwrap_or(0).min(MAX_GRID);
            if rows == 0 || cols == 0 {
                return Err(EngineError::BadCommand("noCells".into()));
            }
            let mut inputs = Vec::new();
            for (i, row) in cells.iter().take(rows).enumerate() {
                for j in 0..cols {
                    let v = row.get(j).cloned().unwrap_or_default();
                    let (r, c) = (area.first.row + i as u32, area.first.col + j as u32);
                    // Unchanged cells keep their content (formats, rich text).
                    if e.cell(sheet, r, c)?.input != v {
                        inputs.push((r, c, v));
                    }
                }
            }
            // Cells of the old area left out of the grid are cleared.
            for r in area.first.row..=area.last.row {
                for c in area.first.col..=area.last.col {
                    let inside =
                        (r - area.first.row) < rows as u32 && (c - area.first.col) < cols as u32;
                    if !inside && !e.cell(sheet, r, c)?.input.is_empty() {
                        inputs.push((r, c, String::new()));
                    }
                }
            }
            if !inputs.is_empty() {
                e.set_inputs(sheet, &inputs)?;
            }
            let last = CellRef::new(
                area.first.row + rows as u32 - 1,
                area.first.col + cols as u32 - 1,
            );
            let name = e.info()?.sheets[sheet].name.clone();
            let next = CellRange::new(area.first, last);
            if next != area {
                area = e
                    .set_embed_area(&format!("'{}'!{next}", name.replace('\'', "''")))?
                    .1;
            }
        }
        let pic = e.range_picture(sheet, area, PREVIEW_SCALE)?;
        let xlsx = e.save("xlsx", sheet)?;
        Ok(Built {
            xlsx,
            png: pic.png,
            size: (pic.width, pic.height),
            old_size,
            sheet: e.info()?.sheets[sheet].name.clone(),
            range: area,
        })
    })
}

/// Shown area and cell inputs of a workbook: (sheet name, area, inputs).
pub fn area_of(
    fonts: &mut FontManager,
    xlsx: &[u8],
) -> Result<(String, CellRange, Vec<Vec<String>>)> {
    SheetEngine::with_fonts(fonts, |e| {
        e.open(xlsx, "object.xlsx")?;
        let (sheet, area) = e.embed_area()?;
        let cells = e.range_inputs(sheet, area)?;
        Ok((e.info()?.sheets[sheet].name.clone(), area, cells))
    })
}

/// Shown size of an edited object (points): the shown/natural ratio it had is kept
/// (`old` natural size known), else its width is kept and the height follows the new
/// area's proportions.
pub fn edited_size(shown: (f64, f64), old: Option<(f64, f64)>, new: (f64, f64)) -> (f64, f64) {
    match old {
        Some((ow, oh)) if ow > 0.0 && oh > 0.0 => (new.0 * shown.0 / ow, new.1 * shown.1 / oh),
        _ if new.0 > 0.0 => (shown.0, shown.0 * new.1 / new.0),
        _ => shown,
    }
}

/// Fit a natural size (points) into `max_w` × `max_h`, keeping proportions.
pub fn fit((w, h): (f64, f64), max_w: f64, max_h: f64) -> (f64, f64) {
    let k = (max_w / w.max(1.0)).min(max_h / h.max(1.0)).min(1.0);
    (w * k, h * k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r1c1_items_become_a1() {
        assert_eq!(a1_item("Sheet1!R1C1:R3C4"), "Sheet1!A1:D3");
        assert_eq!(a1_item("R2C2"), "B2");
        assert_eq!(a1_item("Sheet1!A1:B2"), "Sheet1!A1:B2");
        assert_eq!(a1_item(""), "");
    }

    #[test]
    fn file_uris_become_paths() {
        assert_eq!(source_path("file:///C:/a%20b/c.xlsx"), "C:/a b/c.xlsx");
        assert_eq!(source_path("file:///home/u/c.xlsx"), "/home/u/c.xlsx");
        assert_eq!(source_path(r"D:\x.xlsx"), r"D:\x.xlsx");
    }

    #[test]
    fn edited_size_keeps_the_scale() {
        // Shown at half size: a twice-as-wide area stays at half size.
        assert_eq!(
            edited_size((50.0, 20.0), Some((100.0, 40.0)), (200.0, 40.0)),
            (100.0, 20.0)
        );
        // Unknown natural size: width kept, height from the new proportions.
        assert_eq!(
            edited_size((100.0, 10.0), None, (50.0, 25.0)),
            (100.0, 50.0)
        );
        assert_eq!(fit((200.0, 100.0), 100.0, 100.0), (100.0, 50.0));
        assert_eq!(fit((20.0, 10.0), 100.0, 100.0), (20.0, 10.0));
    }

    #[test]
    fn builds_and_edits_a_workbook_object() {
        let mut fonts = FontManager::new();
        for f in crate::bundled::FONTS.iter().take(8) {
            fonts.add_font(f.to_vec());
        }
        let cells = vec![
            vec!["Tên".to_owned(), "Số".to_owned()],
            vec!["a".to_owned(), "2".to_owned()],
            vec!["b".to_owned(), "=B2*3".to_owned()],
        ];
        let b = build(&mut fonts, None, None, Some(&cells)).unwrap();
        assert_eq!(b.range.to_string(), "A1:B3");
        assert!(b.png.starts_with(b"\x89PNG") && b.old_size.is_none());
        let (sheet, area, inputs) = area_of(&mut fonts, &b.xlsx).unwrap();
        assert_eq!(
            (sheet.as_str(), area.to_string().as_str()),
            ("Sheet1", "A1:B3")
        );
        assert_eq!(inputs, cells);
        // Edit: one row fewer, one column more.
        let next = vec![
            vec!["x".to_owned(), "y".to_owned(), "z".to_owned()],
            vec!["1".to_owned(), "2".to_owned(), "=A2+B2".to_owned()],
        ];
        let e = build(&mut fonts, Some(&b.xlsx), None, Some(&next)).unwrap();
        assert_eq!(e.range.to_string(), "A1:C2");
        assert_eq!(e.old_size, Some(b.size));
        let (_, area, inputs) = area_of(&mut fonts, &e.xlsx).unwrap();
        assert_eq!(area.to_string(), "A1:C2");
        assert_eq!(inputs, next);
        // The cleared third row is gone from the workbook.
        let wb = bo_xlsx::read_xlsx(&e.xlsx).unwrap();
        assert!(wb.sheets[0]
            .cell(2, 0)
            .is_none_or(|c| matches!(c.value, bo_model::sheet::CellValue::Empty)));
        // The fonts are given back.
        assert_eq!(fonts.len(), 8);
    }
}
