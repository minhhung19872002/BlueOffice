//! Excel binary workbooks (`.xlsb`, [MS-XLSB]).
//!
//! An OPC package whose parts are BIFF12 record streams (§2.1.4): the
//! workbook part (BrtBundleSh, BrtWbProp, BrtExternSheet, BrtName), shared
//! strings (BrtSSTItem), styles (BrtFmt, BrtXF inside BrtBeginCellXFs) and one
//! part per worksheet (BrtRowHdr + BrtCell* / BrtFmla* records).

use crate::bytes::{f64_at, slice, u16_at, u32_at, u8_at, utf16};
use crate::sheet::{
    builtin_format, error_text, rk_to_f64, Cell, CellValue, DefinedName, Workbook, Worksheet,
};
use crate::xls_formula::{builtin_name, formula_to_text_with, ptg_exp, Dialect, FormulaCtx};
use crate::{malformed, BinaryError, Result};
use bo_opc::{Limits, Package};
use std::collections::HashMap;

/// Maximum number of cells read from one workbook.
pub const MAX_CELLS: usize = 20_000_000;

// [MS-XLSB] §2.3 record types.
const BRT_ROW_HDR: u32 = 0;
const BRT_CELL_BLANK: u32 = 1;
const BRT_CELL_RK: u32 = 2;
const BRT_CELL_ERROR: u32 = 3;
const BRT_CELL_BOOL: u32 = 4;
const BRT_CELL_REAL: u32 = 5;
const BRT_CELL_ST: u32 = 6;
const BRT_CELL_ISST: u32 = 7;
const BRT_FMLA_STRING: u32 = 8;
const BRT_FMLA_NUM: u32 = 9;
const BRT_FMLA_BOOL: u32 = 10;
const BRT_FMLA_ERROR: u32 = 11;
const BRT_SST_ITEM: u32 = 19;
const BRT_NAME: u32 = 39;
const BRT_FMT: u32 = 44;
const BRT_XF: u32 = 47;
const BRT_WB_PROP: u32 = 153;
const BRT_BUNDLE_SH: u32 = 156;
const BRT_EXTERN_SHEET: u32 = 362;
const BRT_ARR_FMLA: u32 = 426;
const BRT_SHR_FMLA: u32 = 427;
const BRT_BEGIN_CELL_XFS: u32 = 617;
const BRT_END_CELL_XFS: u32 = 618;

/// One BIFF12 record.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Rec<'a> {
    pub typ: u32,
    pub data: &'a [u8],
}

/// Iterate BIFF12 records (§2.1.4: variable-length type and size, 7 bits per byte).
pub(crate) fn records(b: &[u8]) -> impl Iterator<Item = Rec<'_>> + '_ {
    let mut p = 0usize;
    std::iter::from_fn(move || {
        let mut typ = 0u32;
        for k in 0..2 {
            let byte = *b.get(p)?;
            p += 1;
            typ |= u32::from(byte & 0x7F) << (7 * k);
            if byte & 0x80 == 0 {
                break;
            }
        }
        let mut len = 0usize;
        for k in 0..4 {
            let byte = *b.get(p)?;
            p += 1;
            len |= usize::from(byte & 0x7F) << (7 * k);
            if byte & 0x80 == 0 {
                break;
            }
        }
        let data = b.get(p..p.checked_add(len)?)?;
        p += len;
        Some(Rec { typ, data })
    })
}

/// §2.5.166 XLWideString: cch (4 bytes) + UTF-16 characters. Returns (text, bytes used).
fn wide_string(d: &[u8], off: usize) -> Result<(String, usize)> {
    let cch = u32_at(d, off)? as usize;
    if cch == 0xFFFF_FFFF {
        // XLNullableWideString null value
        return Ok((String::new(), 4));
    }
    let bytes = cch
        .checked_mul(2)
        .ok_or_else(|| malformed("string length overflow"))?;
    Ok((utf16(slice(d, off + 4, bytes)?), 4 + bytes))
}

/// Read an `.xlsb` workbook.
pub fn read_xlsb(bytes: &[u8]) -> Result<Workbook> {
    let pkg = Package::read(bytes, &Limits::default())?;
    read_xlsb_package(&pkg)
}

/// Read an `.xlsb` workbook from an opened package.
pub fn read_xlsb_package(pkg: &Package) -> Result<Workbook> {
    let wb_name = pkg
        .related_part("/", bo_opc::REL_OFFICE_DOCUMENT)?
        .ok_or_else(|| BinaryError::Missing("workbook part".into()))?;
    let wb_data = &pkg
        .part(&wb_name)
        .ok_or_else(|| BinaryError::Missing(wb_name.clone()))?
        .data;
    let rels = pkg.relationships(&wb_name)?;
    let target_of = |id: &str| {
        rels.iter()
            .find(|r| r.id == id)
            .map(|r| bo_opc::resolve_target(&wb_name, &r.target))
    };
    let by_type = |suffix: &str| {
        rels.iter()
            .find(|r| r.rel_type.ends_with(suffix))
            .and_then(|r| pkg.part(&bo_opc::resolve_target(&wb_name, &r.target)))
    };

    let mut wb = Workbook {
        has_macros: pkg
            .parts()
            .iter()
            .any(|p| p.name.to_ascii_lowercase().ends_with("vbaproject.bin")),
        ..Workbook::default()
    };
    let mut ctx = FormulaCtx::default();
    // (name, hidden, part name)
    let mut sheets: Vec<(String, bool, Option<String>)> = Vec::new();
    let mut raw_names: Vec<RawName> = Vec::new();
    for rec in records(wb_data) {
        let d = rec.data;
        match rec.typ {
            // §2.4.304 BrtWbProp: f1904 is bit 0
            BRT_WB_PROP => wb.date1904 = u32_at(d, 0).unwrap_or(0) & 1 != 0,
            // §2.4.302 BrtBundleSh
            BRT_BUNDLE_SH => {
                let hs = u32_at(d, 0)?;
                let (rel_id, used) = wide_string(d, 8)?;
                let (name, _) = wide_string(d, 8 + used)?;
                sheets.push((name, hs != 0, target_of(&rel_id)));
            }
            // §2.4.? BrtExternSheet: cXti then Xti (iSupBook, firstSheet, lastSheet)
            BRT_EXTERN_SHEET => {
                let n = u32_at(d, 0)? as usize;
                for k in 0..n.min(65_536) {
                    let o = 4 + 12 * k;
                    let (Ok(s), Ok(a), Ok(b)) = (u32_at(d, o), u32_at(d, o + 4), u32_at(d, o + 8))
                    else {
                        break;
                    };
                    ctx.xti.push((s, a as i32, b as i32));
                }
            }
            // §2.4.? BrtName: flags (4), chKey (1), itab (4), name, NameParsedFormula
            BRT_NAME => match parse_name(d) {
                Ok(n) => {
                    ctx.names.push(n.0.clone());
                    raw_names.push(n);
                }
                Err(_) => ctx.names.push("#NAME?".into()),
            },
            _ => {}
        }
    }
    ctx.sheet_names = sheets.iter().map(|s| s.0.clone()).collect();

    let sst: Vec<String> = by_type("/sharedStrings")
        .map(|p| {
            records(&p.data)
                .filter(|r| r.typ == BRT_SST_ITEM)
                // §2.5.121 RichStr: flags (1) + XLWideString
                .map(|r| wide_string(r.data, 1).map(|s| s.0).unwrap_or_default())
                .collect()
        })
        .unwrap_or_default();
    let (formats, xf_fmt) = by_type("/styles")
        .map(|p| styles(&p.data))
        .unwrap_or_default();
    let number_format = |style: u32| -> Option<String> {
        let ifmt = *xf_fmt.get(style as usize)?;
        if ifmt == 0 {
            return None;
        }
        formats
            .get(&ifmt)
            .cloned()
            .or_else(|| builtin_format(ifmt).map(str::to_owned))
    };

    let mut total = 0usize;
    for (name, hidden, target) in sheets {
        let mut ws = Worksheet {
            name,
            hidden,
            cells: Vec::new(),
        };
        // Chart sheets and missing parts produce an empty sheet.
        if let Some(part) = target.as_deref().and_then(|t| pkg.part(t)) {
            if let Ok(cells) =
                parse_sheet(&part.data, &ctx, &sst, &number_format, MAX_CELLS - total)
            {
                total += cells.len();
                ws.cells = cells;
            }
        }
        wb.sheets.push(ws);
    }
    for (name, sheet, rgce, rgcb) in raw_names {
        wb.names.push(DefinedName {
            formula: formula_to_text_with(Dialect::Biff12, &rgce, &rgcb, &ctx, (0, 0)).ok(),
            name,
            sheet,
        });
    }
    Ok(wb)
}

type RawName = (String, Option<u32>, Vec<u8>, Vec<u8>);

fn parse_name(d: &[u8]) -> Result<RawName> {
    let flags = u32_at(d, 0)?;
    let itab = u32_at(d, 5)?;
    let (raw, used) = wide_string(d, 9)?;
    let name = if flags & 0x20 != 0 {
        let code = raw.chars().next().map_or(0xFF, |c| c as u32 as u8);
        builtin_name(code).to_owned()
    } else {
        raw
    };
    let f = 9 + used;
    let cce = u32_at(d, f)? as usize;
    let rgce = slice(d, f + 4, cce)?.to_vec();
    let rgcb = match u32_at(d, f + 4 + cce) {
        Ok(cb) => slice(d, f + 8 + cce, cb as usize)
            .map(<[u8]>::to_vec)
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    Ok((name, (itab != 0xFFFF_FFFF).then_some(itab), rgce, rgcb))
}

/// §2.4.? BrtFmt and the cell XFs of the styles part.
fn styles(b: &[u8]) -> (HashMap<u16, String>, Vec<u16>) {
    let mut formats = HashMap::new();
    let mut xfs = Vec::new();
    let mut in_cell_xfs = false;
    for rec in records(b) {
        match rec.typ {
            BRT_FMT => {
                if let (Ok(id), Ok((s, _))) = (u16_at(rec.data, 0), wide_string(rec.data, 2)) {
                    formats.insert(id, s);
                }
            }
            BRT_BEGIN_CELL_XFS => in_cell_xfs = true,
            BRT_END_CELL_XFS => in_cell_xfs = false,
            // BrtXF: ixfeParent (2), iFmt (2), …
            BRT_XF if in_cell_xfs => xfs.push(u16_at(rec.data, 2).unwrap_or(0)),
            _ => {}
        }
    }
    (formats, xfs)
}

/// A rectangle from BrtShrFmla / BrtArrFmla (RfX: rwFirst, rwLast, colFirst, colLast).
struct SharedFormula {
    r1: u32,
    r2: u32,
    c1: u32,
    c2: u32,
    rgce: Vec<u8>,
    rgcb: Vec<u8>,
}

/// §2.5.? CellParsedFormula (BIFF12): cce (4), rgce, cb (4), rgcb.
fn parsed_formula(d: &[u8], off: usize) -> Result<(&[u8], &[u8])> {
    let cce = u32_at(d, off)? as usize;
    let rgce = slice(d, off + 4, cce)?;
    let rgcb = match u32_at(d, off + 4 + cce) {
        Ok(cb) => slice(d, off + 8 + cce, cb as usize).unwrap_or_default(),
        Err(_) => &[],
    };
    Ok((rgce, rgcb))
}

fn parse_sheet(
    b: &[u8],
    ctx: &FormulaCtx,
    sst: &[String],
    number_format: &dyn Fn(u32) -> Option<String>,
    budget: usize,
) -> Result<Vec<Cell>> {
    let mut cells: Vec<Cell> = Vec::new();
    let mut row = 0u32;
    let mut shared: Vec<SharedFormula> = Vec::new();
    let mut pending: Vec<(usize, u32)> = Vec::new();
    for rec in records(b) {
        let d = rec.data;
        if rec.typ == BRT_ROW_HDR {
            row = u32_at(d, 0)?;
            continue;
        }
        if matches!(rec.typ, BRT_SHR_FMLA | BRT_ARR_FMLA) {
            let (r1, r2, c1, c2) = (u32_at(d, 0)?, u32_at(d, 4)?, u32_at(d, 8)?, u32_at(d, 12)?);
            // BrtArrFmla has a 1-byte flags field before the formula.
            let off = if rec.typ == BRT_ARR_FMLA { 17 } else { 16 };
            if let Ok((rgce, rgcb)) = parsed_formula(d, off) {
                shared.push(SharedFormula {
                    r1,
                    r2,
                    c1,
                    c2,
                    rgce: rgce.to_vec(),
                    rgcb: rgcb.to_vec(),
                });
            }
            continue;
        }
        if !(BRT_CELL_BLANK..=BRT_FMLA_ERROR).contains(&rec.typ) {
            continue;
        }
        // §2.5.9 Cell: column (4) + iStyleRef (24 bits) + flags (8 bits)
        let col = u32_at(d, 0)?;
        let style = u32_at(d, 4)? & 0x00FF_FFFF;
        let (value, formula_at) = match rec.typ {
            BRT_CELL_BLANK => continue,
            BRT_CELL_RK => (CellValue::Number(rk_to_f64(u32_at(d, 8)?)), None),
            BRT_CELL_ERROR => (CellValue::Error(error_text(u8_at(d, 8)?)), None),
            BRT_CELL_BOOL => (CellValue::Bool(u8_at(d, 8)? != 0), None),
            BRT_CELL_REAL => (CellValue::Number(f64_at(d, 8)?), None),
            BRT_CELL_ST => (CellValue::Text(wide_string(d, 8)?.0), None),
            BRT_CELL_ISST => (
                CellValue::Text(sst.get(u32_at(d, 8)? as usize).cloned().unwrap_or_default()),
                None,
            ),
            BRT_FMLA_STRING => {
                let (s, used) = wide_string(d, 8)?;
                (CellValue::Text(s), Some(8 + used + 2))
            }
            BRT_FMLA_NUM => (CellValue::Number(f64_at(d, 8)?), Some(18)),
            BRT_FMLA_BOOL => (CellValue::Bool(u8_at(d, 8)? != 0), Some(11)),
            _ => (CellValue::Error(error_text(u8_at(d, 8)?)), Some(11)),
        };
        if cells.len() >= budget {
            return Err(malformed("too many cells"));
        }
        let mut cell = Cell {
            row,
            col,
            value,
            formula: None,
            number_format: number_format(style),
        };
        if let Some(off) = formula_at {
            if let Ok((rgce, rgcb)) = parsed_formula(d, off) {
                if let Some((r, _)) = ptg_exp(rgce, Dialect::Biff12) {
                    pending.push((cells.len(), r));
                } else {
                    cell.formula =
                        formula_to_text_with(Dialect::Biff12, rgce, rgcb, ctx, (row, col)).ok();
                }
            }
        }
        cells.push(cell);
    }
    for (idx, r) in pending {
        let (row, col) = (cells[idx].row, cells[idx].col);
        if let Some(f) = shared
            .iter()
            .find(|f| f.r1 == r && (f.r1..=f.r2).contains(&row) && (f.c1..=f.c2).contains(&col))
        {
            cells[idx].formula =
                formula_to_text_with(Dialect::Biff12, &f.rgce, &f.rgcb, ctx, (row, col)).ok();
        }
    }
    Ok(cells)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Encode one BIFF12 record.
    pub fn brt(typ: u32, data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        if typ < 0x80 {
            v.push(typ as u8);
        } else {
            v.push((typ & 0x7F) as u8 | 0x80);
            v.push((typ >> 7) as u8);
        }
        let mut len = data.len();
        loop {
            let b = (len & 0x7F) as u8;
            len >>= 7;
            if len == 0 {
                v.push(b);
                break;
            }
            v.push(b | 0x80);
        }
        v.extend_from_slice(data);
        v
    }

    pub fn ws(s: &str) -> Vec<u8> {
        let units: Vec<u16> = s.encode_utf16().collect();
        let mut v = (units.len() as u32).to_le_bytes().to_vec();
        for u in units {
            v.extend(u.to_le_bytes());
        }
        v
    }

    fn cell(col: u32, style: u32) -> Vec<u8> {
        let mut v = col.to_le_bytes().to_vec();
        v.extend(style.to_le_bytes());
        v
    }

    fn fmla(rgce: &[u8]) -> Vec<u8> {
        let mut v = (rgce.len() as u32).to_le_bytes().to_vec();
        v.extend_from_slice(rgce);
        v.extend(0u32.to_le_bytes());
        v
    }

    pub fn sample_xlsb() -> Vec<u8> {
        let mut wbb = brt(BRT_WB_PROP, &1u32.to_le_bytes());
        let mut bs = 0u32.to_le_bytes().to_vec();
        bs.extend(1u32.to_le_bytes());
        bs.extend(ws("rId1"));
        bs.extend(ws("Dữ liệu"));
        wbb.extend(brt(BRT_BUNDLE_SH, &bs));
        let mut bs = 1u32.to_le_bytes().to_vec();
        bs.extend(2u32.to_le_bytes());
        bs.extend(ws("rId9"));
        bs.extend(ws("Ẩn"));
        wbb.extend(brt(BRT_BUNDLE_SH, &bs));
        let mut xs = 1u32.to_le_bytes().to_vec();
        xs.extend([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        wbb.extend(brt(BRT_EXTERN_SHEET, &xs));

        let mut sst = Vec::new();
        for s in ["xin chào", "two"] {
            let mut d = vec![0u8];
            d.extend(ws(s));
            sst.extend(brt(BRT_SST_ITEM, &d));
        }

        let mut sty = Vec::new();
        let mut f = 170u16.to_le_bytes().to_vec();
        f.extend(ws("#,##0.0"));
        sty.extend(brt(BRT_FMT, &f));
        sty.extend(brt(BRT_BEGIN_CELL_XFS, &1u32.to_le_bytes()));
        for ifmt in [0u16, 170] {
            let mut x = 0u16.to_le_bytes().to_vec();
            x.extend(ifmt.to_le_bytes());
            x.extend([0u8; 12]);
            sty.extend(brt(BRT_XF, &x));
        }
        sty.extend(brt(BRT_END_CELL_XFS, &[]));

        let mut sh = brt(BRT_ROW_HDR, &[0; 17]);
        let mut c = cell(0, 1);
        c.extend(1234.5f64.to_le_bytes());
        sh.extend(brt(BRT_CELL_REAL, &c));
        let mut c = cell(1, 0);
        c.extend(0u32.to_le_bytes());
        sh.extend(brt(BRT_CELL_ISST, &c));
        let mut c = cell(2, 0);
        c.extend(((7u32 << 2) | 2).to_le_bytes());
        sh.extend(brt(BRT_CELL_RK, &c));
        let mut row = 4u32.to_le_bytes().to_vec();
        row.extend([0u8; 13]);
        sh.extend(brt(BRT_ROW_HDR, &row));
        let mut c = cell(0, 0);
        c.push(1);
        sh.extend(brt(BRT_CELL_BOOL, &c));
        let mut c = cell(1, 0);
        c.push(0x2A);
        sh.extend(brt(BRT_CELL_ERROR, &c));
        let mut c = cell(2, 0);
        c.extend(ws("inline"));
        sh.extend(brt(BRT_CELL_ST, &c));
        // D5 = SUM(A1:C1) → BrtFmlaNum
        let mut rgce = vec![0x25];
        for v in [0u32, 0] {
            rgce.extend(v.to_le_bytes());
        }
        rgce.extend(0xC000u16.to_le_bytes());
        rgce.extend(0xC002u16.to_le_bytes());
        rgce.extend([0x42, 1, 4, 0]);
        let mut c = cell(3, 0);
        c.extend(1241.5f64.to_le_bytes());
        c.extend(0u16.to_le_bytes());
        c.extend(fmla(&rgce));
        sh.extend(brt(BRT_FMLA_NUM, &c));
        // E5 = "a"&B1 → BrtFmlaString
        let mut rgce = vec![0x17, 1, 0, b'a', 0];
        rgce.push(0x24);
        rgce.extend(0u32.to_le_bytes());
        rgce.extend(0xC001u16.to_le_bytes());
        rgce.push(0x08);
        let mut c = cell(4, 0);
        c.extend(ws("axin chào"));
        c.extend(0u16.to_le_bytes());
        c.extend(fmla(&rgce));
        sh.extend(brt(BRT_FMLA_STRING, &c));

        let mut pkg = Package::new();
        let ct = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="bin" ContentType="application/vnd.ms-excel.sheet.binary.macroEnabled.main"/></Types>"#;
        pkg.put_part("/[Content_Types].xml", ct.as_bytes().to_vec());
        pkg.put_part("/_rels/.rels", br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.bin"/></Relationships>"#.to_vec());
        pkg.put_part("/xl/_rels/workbook.bin.rels", br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.bin"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.bin"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.bin"/></Relationships>"#.to_vec());
        pkg.put_part("/xl/workbook.bin", wbb);
        pkg.put_part("/xl/sharedStrings.bin", sst);
        pkg.put_part("/xl/styles.bin", sty);
        pkg.put_part("/xl/worksheets/sheet1.bin", sh);
        pkg.write().unwrap()
    }

    #[test]
    fn record_header_encoding() {
        let mut b = brt(BRT_BEGIN_CELL_XFS, &[9; 200]);
        b.extend(brt(3, &[]));
        let recs: Vec<_> = records(&b).collect();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].typ, BRT_BEGIN_CELL_XFS);
        assert_eq!(recs[0].data.len(), 200);
        assert_eq!(recs[1].typ, 3);
        // truncated stream ends the iteration
        assert_eq!(records(&b[..10]).count(), 0);
    }

    #[test]
    fn reads_sample() {
        let wb = read_xlsb(&sample_xlsb()).unwrap();
        assert!(wb.date1904);
        assert!(!wb.has_macros);
        assert_eq!(wb.sheets.len(), 2);
        assert_eq!(wb.sheets[0].name, "Dữ liệu");
        assert!(!wb.sheets[0].hidden);
        assert!(wb.sheets[1].hidden);
        assert_eq!(wb.sheets[1].cells, Vec::new());
        let s = &wb.sheets[0];
        let get = |r, c| s.cells.iter().find(|x| x.row == r && x.col == c).unwrap();
        assert_eq!(get(0, 0).value, CellValue::Number(1234.5));
        assert_eq!(get(0, 0).number_format.as_deref(), Some("#,##0.0"));
        assert_eq!(get(0, 1).value, CellValue::Text("xin chào".into()));
        assert_eq!(get(0, 2).value, CellValue::Number(7.0));
        assert_eq!(get(4, 0).value, CellValue::Bool(true));
        assert_eq!(get(4, 1).value, CellValue::Error("#N/A".into()));
        assert_eq!(get(4, 2).value, CellValue::Text("inline".into()));
        assert_eq!(get(4, 3).formula.as_deref(), Some("SUM(A1:C1)"));
        assert_eq!(get(4, 3).value, CellValue::Number(1241.5));
        assert_eq!(get(4, 4).formula.as_deref(), Some("\"a\"&B1"));
        assert_eq!(get(4, 4).value, CellValue::Text("axin chào".into()));
    }

    proptest::proptest! {
        #[test]
        fn random_records_never_panic(data in proptest::collection::vec(proptest::num::u8::ANY, 0..2048)) {
            let ctx = FormulaCtx::default();
            let _ = parse_sheet(&data, &ctx, &[], &|_| None, 1000);
            let _ = styles(&data);
            let _ = read_xlsb(&data);
        }
    }
}
