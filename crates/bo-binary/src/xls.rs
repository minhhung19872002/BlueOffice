//! Excel 97–2003 workbooks ([MS-XLS] BIFF8).
//!
//! The `Workbook` stream holds the globals substream (§2.1.7.20.3: BoundSheet8,
//! SST, Format, XF, Date1904, SupBook/ExternSheet/ExternName, Lbl) followed by
//! one substream per sheet (§2.1.7.20.5) located through `BoundSheet8.lbPlyPos`.

use crate::bytes::{f64_at, latin1, slice, u16_at, u32_at, u8_at, utf16};
use crate::sheet::{
    builtin_format, error_text, rk_to_f64, Cell, CellValue, DefinedName, Workbook, Worksheet,
};
use crate::xls_formula::{builtin_name, formula_to_text, ptg_exp, FormulaCtx};
use crate::{malformed, BinaryError, Cfb, Result};
use std::collections::HashMap;

/// Maximum number of cells read from one workbook (guards decompression-bomb style inputs).
pub const MAX_CELLS: usize = 20_000_000;
/// Maximum number of shared strings.
const MAX_SST: usize = 10_000_000;
/// Maximum nesting of BOF/EOF substreams inside a sheet (embedded charts).
const MAX_SUBSTREAM_DEPTH: usize = 8;

// [MS-XLS] §2.3 record numbers.
const BOF: u16 = 0x0809;
const EOF: u16 = 0x000A;
const CONTINUE: u16 = 0x003C;
const FILEPASS: u16 = 0x002F;
const BOUNDSHEET8: u16 = 0x0085;
const SST: u16 = 0x00FC;
const FORMAT: u16 = 0x041E;
const XF: u16 = 0x00E0;
const DATE1904: u16 = 0x0022;
const SUPBOOK: u16 = 0x01AE;
const EXTERNNAME: u16 = 0x0023;
const EXTERNSHEET: u16 = 0x0017;
const LBL: u16 = 0x0018;
const NUMBER: u16 = 0x0203;
const RK: u16 = 0x027E;
const MULRK: u16 = 0x00BD;
const LABELSST: u16 = 0x00FD;
const LABEL: u16 = 0x0204;
const RSTRING: u16 = 0x00D6;
const BOOLERR: u16 = 0x0205;
const FORMULA: u16 = 0x0006;
const STRING: u16 = 0x0207;
const SHRFMLA: u16 = 0x04BC;
const ARRAY: u16 = 0x0221;

/// Read an `.xls` (BIFF8) workbook.
pub fn read_xls(bytes: &[u8]) -> Result<Workbook> {
    let mut cfb = Cfb::open(bytes)?;
    let has_vba = cfb.has_storage("/_VBA_PROJECT_CUR");
    let stream = if cfb.has("/Workbook") {
        cfb.read("/Workbook")?
    } else if cfb.has("/Book") {
        return Err(BinaryError::Unsupported(
            "BIFF5/BIFF7 workbooks (Excel 5.0/95)".into(),
        ));
    } else {
        return Err(BinaryError::Missing("Workbook".into()));
    };
    let mut wb = parse_workbook_stream(&stream)?;
    wb.has_macros |= has_vba;
    Ok(wb)
}

/// One BIFF record.
#[derive(Debug, Clone, Copy)]
struct Rec<'a> {
    typ: u16,
    data: &'a [u8],
}

/// Iterate records from `pos`; stops at the first truncated header.
fn records(stream: &[u8], pos: usize) -> impl Iterator<Item = Rec<'_>> + '_ {
    let mut p = pos;
    std::iter::from_fn(move || {
        let typ = u16_at(stream, p).ok()?;
        let len = u16_at(stream, p + 2).ok()? as usize;
        let data = stream.get(p + 4..p + 4 + len)?;
        p += 4 + len;
        Some(Rec { typ, data })
    })
}

/// A reader over a record followed by its CONTINUE records, implementing the
/// string-splitting rule of [MS-XLS] §2.1.7.20.2 (each continued character
/// array starts with a fresh `fHighByte` flag byte).
struct Segments<'a> {
    segs: Vec<&'a [u8]>,
    si: usize,
    pos: usize,
}

impl Segments<'_> {
    fn advance(&mut self) -> bool {
        while self.si < self.segs.len() && self.pos >= self.segs[self.si].len() {
            self.si += 1;
            self.pos = 0;
        }
        self.si < self.segs.len()
    }

    fn byte(&mut self) -> Result<u8> {
        if !self.advance() {
            return Err(malformed("SST truncated"));
        }
        let b = self.segs[self.si][self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes([self.byte()?, self.byte()?]))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes([
            self.byte()?,
            self.byte()?,
            self.byte()?,
            self.byte()?,
        ]))
    }

    fn skip(&mut self, mut n: usize) -> Result<()> {
        while n > 0 {
            if !self.advance() {
                return Err(malformed("SST truncated"));
            }
            let avail = self.segs[self.si].len() - self.pos;
            let k = avail.min(n);
            self.pos += k;
            n -= k;
        }
        Ok(())
    }

    /// Characters of an XLUnicodeRichExtendedString (§2.5.293).
    fn chars(&mut self, cch: usize, mut high: bool) -> Result<String> {
        let mut units: Vec<u16> = Vec::with_capacity(cch.min(32_768));
        while units.len() < cch {
            if self.pos >= self.segs.get(self.si).map_or(0, |s| s.len()) {
                // Character data continues in the next CONTINUE record.
                self.si += 1;
                self.pos = 0;
                let seg = self
                    .segs
                    .get(self.si)
                    .ok_or_else(|| malformed("SST string truncated"))?;
                let flag = *seg.first().ok_or_else(|| malformed("empty CONTINUE"))?;
                high = flag & 1 != 0;
                self.pos = 1;
                continue;
            }
            let seg = self.segs[self.si];
            if high {
                let lo = *seg.get(self.pos).ok_or_else(|| malformed("SST"))?;
                let hi = *seg.get(self.pos + 1).ok_or_else(|| malformed("SST"))?;
                units.push(u16::from_le_bytes([lo, hi]));
                self.pos += 2;
            } else {
                units.push(u16::from(seg[self.pos]));
                self.pos += 1;
            }
        }
        Ok(String::from_utf16_lossy(&units))
    }

    /// One XLUnicodeRichExtendedString.
    fn rich_string(&mut self) -> Result<String> {
        let cch = self.u16()? as usize;
        let flags = self.byte()?;
        let high = flags & 0x01 != 0;
        let rich = if flags & 0x08 != 0 { self.u16()? } else { 0 };
        let ext = if flags & 0x04 != 0 { self.u32()? } else { 0 };
        let s = self.chars(cch, high)?;
        self.skip(rich as usize * 4)?;
        self.skip(ext as usize)?;
        Ok(s)
    }
}

/// §2.4.265 SST.
fn parse_sst(segs: Vec<&[u8]>) -> Result<Vec<String>> {
    let mut r = Segments {
        segs,
        si: 0,
        pos: 0,
    };
    let _total = r.u32()?;
    let unique = r.u32()? as usize;
    let mut out = Vec::with_capacity(unique.min(65_536));
    for _ in 0..unique.min(MAX_SST) {
        match r.rich_string() {
            Ok(s) => out.push(s),
            Err(_) => break,
        }
    }
    Ok(out)
}

/// §2.5.294 XLUnicodeString (cch u16, flags, characters).
fn xl_unicode_string(d: &[u8], off: usize) -> Result<(String, usize)> {
    let cch = u16_at(d, off)? as usize;
    let flags = u8_at(d, off + 2)?;
    if flags & 1 != 0 {
        Ok((utf16(slice(d, off + 3, cch * 2)?), 3 + cch * 2))
    } else {
        Ok((latin1(slice(d, off + 3, cch)?), 3 + cch))
    }
}

/// §2.5.240 ShortXLUnicodeString (cch u8, flags, characters).
fn short_xl_unicode_string(d: &[u8], off: usize) -> Result<String> {
    let cch = u8_at(d, off)? as usize;
    let flags = u8_at(d, off + 1)?;
    if flags & 1 != 0 {
        Ok(utf16(slice(d, off + 2, cch * 2)?))
    } else {
        Ok(latin1(slice(d, off + 2, cch)?))
    }
}

#[derive(Debug)]
struct BoundSheet {
    pos: u32,
    hidden: bool,
    kind: u8,
    name: String,
}

#[derive(Debug)]
struct RawName {
    name: String,
    itab: u16,
    rgce: Vec<u8>,
    rgcb: Vec<u8>,
}

#[derive(Debug, Default)]
struct Globals {
    sheets: Vec<BoundSheet>,
    sst: Vec<String>,
    formats: HashMap<u16, String>,
    xf_fmt: Vec<u16>,
    date1904: bool,
    ctx: FormulaCtx,
    names: Vec<RawName>,
}

impl Globals {
    fn number_format(&self, ixfe: u16) -> Option<String> {
        let ifmt = *self.xf_fmt.get(ixfe as usize)?;
        if ifmt == 0 {
            return None;
        }
        self.formats
            .get(&ifmt)
            .cloned()
            .or_else(|| builtin_format(ifmt).map(str::to_owned))
    }
}

/// Parse a whole BIFF8 `Workbook` stream.
pub(crate) fn parse_workbook_stream(stream: &[u8]) -> Result<Workbook> {
    let g = parse_globals(stream)?;
    let mut wb = Workbook {
        date1904: g.date1904,
        ..Workbook::default()
    };
    let mut total_cells = 0usize;
    for bs in &g.sheets {
        match bs.kind {
            // worksheet / dialog sheet
            0 => {}
            // macro sheet (XLM) or VBA module
            1 | 6 => {
                wb.has_macros = true;
                continue;
            }
            _ => continue,
        }
        let mut ws = Worksheet {
            name: bs.name.clone(),
            hidden: bs.hidden,
            cells: Vec::new(),
        };
        // A damaged sheet substream yields an empty sheet rather than failing the book.
        if let Ok(cells) = parse_sheet(stream, bs.pos as usize, &g, MAX_CELLS - total_cells) {
            total_cells += cells.len();
            ws.cells = cells;
        }
        wb.sheets.push(ws);
    }
    for n in &g.names {
        let formula = formula_to_text(&n.rgce, &n.rgcb, &g.ctx, (0, 0)).ok();
        wb.names.push(DefinedName {
            name: n.name.clone(),
            formula,
            sheet: n.itab.checked_sub(1).map(u32::from),
        });
    }
    Ok(wb)
}

fn parse_globals(stream: &[u8]) -> Result<Globals> {
    let mut g = Globals::default();
    let mut it = records(stream, 0).peekable();
    let first = it
        .next()
        .ok_or_else(|| malformed("empty Workbook stream"))?;
    if first.typ != BOF {
        return Err(malformed("Workbook stream does not start with BOF"));
    }
    // §2.4.21 BOF: vers 0x0600 = BIFF8, dt 0x0005 = workbook globals.
    let vers = u16_at(first.data, 0)?;
    if vers != 0x0600 {
        return Err(BinaryError::Unsupported(format!(
            "BIFF version 0x{vers:04X}"
        )));
    }
    while let Some(rec) = it.next() {
        let d = rec.data;
        match rec.typ {
            EOF => break,
            // §2.4.117 FilePass: RC4 / XOR obfuscation is not implemented.
            FILEPASS => return Err(BinaryError::Encrypted),
            // §2.4.28 BoundSheet8
            BOUNDSHEET8 => {
                g.sheets.push(BoundSheet {
                    pos: u32_at(d, 0)?,
                    hidden: u8_at(d, 4)? & 0x03 != 0,
                    kind: u8_at(d, 5)?,
                    name: short_xl_unicode_string(d, 6)?,
                });
            }
            SST => {
                let mut segs = vec![d];
                while let Some(next) = it.peek() {
                    if next.typ != CONTINUE {
                        break;
                    }
                    segs.push(next.data);
                    it.next();
                }
                g.sst = parse_sst(segs)?;
            }
            // §2.4.126 Format
            FORMAT => {
                if let (Ok(id), Ok((s, _))) = (u16_at(d, 0), xl_unicode_string(d, 2)) {
                    g.formats.insert(id, s);
                }
            }
            // §2.4.353 XF: ifmt at offset 2
            XF => g.xf_fmt.push(u16_at(d, 2)?),
            // §2.4.77 Date1904
            DATE1904 => g.date1904 = u16_at(d, 0)? == 1,
            // §2.4.271 SupBook: cch 0x0401 = self-reference, 0x3A01 = add-in functions
            SUPBOOK => {
                let cch = u16_at(d, 2).unwrap_or(0);
                g.ctx.supbook_self.push(cch == 0x0401);
                g.ctx.extern_names.push(Vec::new());
            }
            // §2.4.105 ExternName: name is a ShortXLUnicodeString at offset 6
            EXTERNNAME => {
                let name = short_xl_unicode_string(d, 6).unwrap_or_default();
                if let Some(list) = g.ctx.extern_names.last_mut() {
                    list.push(name);
                }
            }
            // §2.4.106 ExternSheet / §2.5.?? XTI
            EXTERNSHEET => {
                let n = u16_at(d, 0)? as usize;
                for k in 0..n {
                    let o = 2 + 6 * k;
                    let (Ok(sup), Ok(first), Ok(last)) =
                        (u16_at(d, o), u16_at(d, o + 2), u16_at(d, o + 4))
                    else {
                        break;
                    };
                    g.ctx.xti.push((
                        u32::from(sup),
                        i32::from(first as i16),
                        i32::from(last as i16),
                    ));
                }
            }
            // §2.4.150 Lbl
            LBL => {
                if let Ok(n) = parse_lbl(d) {
                    g.ctx.names.push(n.name.clone());
                    g.names.push(n);
                } else {
                    g.ctx.names.push("#NAME?".into());
                }
            }
            _ => {}
        }
    }
    g.ctx.sheet_names = g.sheets.iter().map(|s| s.name.clone()).collect();
    Ok(g)
}

fn parse_lbl(d: &[u8]) -> Result<RawName> {
    let flags = u16_at(d, 0)?;
    let cch = u8_at(d, 3)? as usize;
    let cce = u16_at(d, 4)? as usize;
    let itab = u16_at(d, 8)?;
    // Name: XLUnicodeStringNoCch at offset 14
    let high = u8_at(d, 14)? & 1 != 0;
    let (raw, used) = if high {
        (utf16(slice(d, 15, cch * 2)?), cch * 2)
    } else {
        (latin1(slice(d, 15, cch)?), cch)
    };
    let name = if flags & 0x0020 != 0 {
        // fBuiltin: the name is one character holding the built-in index
        let code = raw.chars().next().map_or(0xFF, |c| c as u32 as u8);
        builtin_name(code).to_owned()
    } else {
        raw
    };
    let f = 15 + used;
    let rgce = slice(d, f, cce)?.to_vec();
    let rgcb = d.get(f + cce..).unwrap_or_default().to_vec();
    Ok(RawName {
        name,
        itab,
        rgce,
        rgcb,
    })
}

/// A formula cell waiting for a SHRFMLA / ARRAY record.
struct Pending {
    cell: usize,
    key: (u32, u32),
}

/// Parse one worksheet substream starting at its BOF.
fn parse_sheet(stream: &[u8], pos: usize, g: &Globals, budget: usize) -> Result<Vec<Cell>> {
    let mut cells: Vec<Cell> = Vec::new();
    let mut it = records(stream, pos);
    let first = it.next().ok_or_else(|| malformed("sheet BOF missing"))?;
    if first.typ != BOF {
        return Err(malformed("sheet substream does not start with BOF"));
    }
    let mut depth = 1usize;
    let mut string_for: Option<usize> = None;
    let mut shared: HashMap<(u32, u32), (Vec<u8>, Vec<u8>)> = HashMap::new();
    let mut pending: Vec<Pending> = Vec::new();

    let add = |cells: &mut Vec<Cell>, row: u16, col: u16, ixfe: u16, value| -> Result<usize> {
        if cells.len() >= budget {
            return Err(malformed("too many cells"));
        }
        cells.push(Cell {
            row: u32::from(row),
            col: u32::from(col),
            value,
            formula: None,
            number_format: g.number_format(ixfe),
        });
        Ok(cells.len() - 1)
    };

    for rec in it {
        let d = rec.data;
        if depth > 1 {
            // Inside an embedded chart substream: track nesting only.
            match rec.typ {
                BOF => depth += 1,
                EOF => depth -= 1,
                _ => {}
            }
            continue;
        }
        let rc =
            || -> Result<(u16, u16, u16)> { Ok((u16_at(d, 0)?, u16_at(d, 2)?, u16_at(d, 4)?)) };
        match rec.typ {
            EOF => break,
            BOF => {
                depth += 1;
                if depth > MAX_SUBSTREAM_DEPTH {
                    return Err(malformed("substreams nested too deeply"));
                }
            }
            // §2.4.180 Number
            NUMBER => {
                let (r, c, x) = rc()?;
                add(&mut cells, r, c, x, CellValue::Number(f64_at(d, 6)?))?;
            }
            // §2.4.220 RK
            RK => {
                let (r, c, x) = rc()?;
                add(
                    &mut cells,
                    r,
                    c,
                    x,
                    CellValue::Number(rk_to_f64(u32_at(d, 6)?)),
                )?;
            }
            // §2.4.175 MulRk
            MULRK => {
                let r = u16_at(d, 0)?;
                let c0 = u16_at(d, 2)?;
                let n = d.len().saturating_sub(6) / 6;
                for k in 0..n {
                    let o = 4 + 6 * k;
                    let x = u16_at(d, o)?;
                    let v = rk_to_f64(u32_at(d, o + 2)?);
                    add(
                        &mut cells,
                        r,
                        c0.saturating_add(k as u16),
                        x,
                        CellValue::Number(v),
                    )?;
                }
            }
            // §2.4.149 LabelSst
            LABELSST => {
                let (r, c, x) = rc()?;
                let s = g
                    .sst
                    .get(u32_at(d, 6)? as usize)
                    .cloned()
                    .unwrap_or_default();
                add(&mut cells, r, c, x, CellValue::Text(s))?;
            }
            // §2.4.148 Label / §2.4.233 RString
            LABEL | RSTRING => {
                let (r, c, x) = rc()?;
                let (s, _) = xl_unicode_string(d, 6)?;
                add(&mut cells, r, c, x, CellValue::Text(s))?;
            }
            // §2.4.24 BoolErr
            BOOLERR => {
                let (r, c, x) = rc()?;
                let v = u8_at(d, 6)?;
                let value = if u8_at(d, 7)? != 0 {
                    CellValue::Error(error_text(v))
                } else {
                    CellValue::Bool(v != 0)
                };
                add(&mut cells, r, c, x, value)?;
            }
            // §2.4.127 Formula
            FORMULA => {
                let (r, c, x) = rc()?;
                let val = slice(d, 6, 8)?;
                let mut wants_string = false;
                let value = if val[6] == 0xFF && val[7] == 0xFF {
                    // §2.5.133 FormulaValue
                    match val[0] {
                        0 => {
                            wants_string = true;
                            CellValue::Text(String::new())
                        }
                        1 => CellValue::Bool(val[2] != 0),
                        2 => CellValue::Error(error_text(val[2])),
                        _ => CellValue::Text(String::new()),
                    }
                } else {
                    CellValue::Number(f64_at(d, 6)?)
                };
                let idx = add(&mut cells, r, c, x, value)?;
                if wants_string {
                    string_for = Some(idx);
                }
                // CellParsedFormula at offset 20
                let cce = u16_at(d, 20)? as usize;
                let rgce = slice(d, 22, cce)?;
                let rgcb = d.get(22 + cce..).unwrap_or_default();
                if let Some(key) = ptg_exp(rgce, crate::xls_formula::Dialect::Biff8) {
                    pending.push(Pending { cell: idx, key });
                } else {
                    cells[idx].formula =
                        formula_to_text(rgce, rgcb, &g.ctx, (u32::from(r), u32::from(c))).ok();
                }
            }
            // §2.4.268 String: cached text result of the preceding formula
            STRING => {
                if let Some(idx) = string_for.take() {
                    if let Ok((s, _)) = xl_unicode_string(d, 0) {
                        cells[idx].value = CellValue::Text(s);
                    }
                }
            }
            // §2.4.260 ShrFmla: RefU (6) + reserved + cUse, then CellParsedFormula
            SHRFMLA => {
                let key = (u32::from(u16_at(d, 0)?), u32::from(u8_at(d, 4)?));
                let cce = u16_at(d, 8)? as usize;
                let rgce = slice(d, 10, cce)?.to_vec();
                let rgcb = d.get(10 + cce..).unwrap_or_default().to_vec();
                shared.insert(key, (rgce, rgcb));
            }
            // §2.4.4 Array: RefU (6) + flags (2) + chn (4), then the formula
            ARRAY => {
                let key = (u32::from(u16_at(d, 0)?), u32::from(u8_at(d, 4)?));
                let cce = u16_at(d, 12)? as usize;
                let rgce = slice(d, 14, cce)?.to_vec();
                let rgcb = d.get(14 + cce..).unwrap_or_default().to_vec();
                shared.insert(key, (rgce, rgcb));
            }
            _ => {}
        }
    }
    for p in pending {
        if let Some((rgce, rgcb)) = shared.get(&p.key) {
            let base = (cells[p.cell].row, cells[p.cell].col);
            cells[p.cell].formula = formula_to_text(rgce, rgcb, &g.ctx, base).ok();
        }
    }
    Ok(cells)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Build one BIFF record.
    pub fn rec(typ: u16, data: &[u8]) -> Vec<u8> {
        let mut v = typ.to_le_bytes().to_vec();
        v.extend((data.len() as u16).to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    fn bof(dt: u16) -> Vec<u8> {
        let mut d = 0x0600u16.to_le_bytes().to_vec();
        d.extend(dt.to_le_bytes());
        d.extend([0u8; 12]);
        rec(BOF, &d)
    }

    fn short_str(s: &str) -> Vec<u8> {
        let mut v = vec![s.chars().count() as u8, 1];
        for u in s.encode_utf16() {
            v.extend(u.to_le_bytes());
        }
        v
    }

    fn cell_hdr(r: u16, c: u16, x: u16) -> Vec<u8> {
        let mut v = r.to_le_bytes().to_vec();
        v.extend(c.to_le_bytes());
        v.extend(x.to_le_bytes());
        v
    }

    fn formula_rec(r: u16, c: u16, value: [u8; 8], rgce: &[u8]) -> Vec<u8> {
        let mut d = cell_hdr(r, c, 0);
        d.extend(value);
        d.extend([0, 0, 0, 0, 0, 0]); // flags + chn
        d.extend((rgce.len() as u16).to_le_bytes());
        d.extend_from_slice(rgce);
        rec(FORMULA, &d)
    }

    /// A small BIFF8 workbook stream: two sheets, SST split across CONTINUE,
    /// a custom number format, numbers, strings, booleans, errors, formulas
    /// (plain, string result, shared, 3-D) and a defined name.
    pub fn sample_stream() -> Vec<u8> {
        let mut globals = bof(0x0005);
        // Format 164 "0.000" and XF 0 (General), XF 1 (fmt 164), XF 2 (builtin 14)
        let mut f = 164u16.to_le_bytes().to_vec();
        f.extend(5u16.to_le_bytes());
        f.push(0);
        f.extend(b"0.000");
        globals.extend(rec(FORMAT, &f));
        for ifmt in [0u16, 164, 14] {
            let mut x = 0u16.to_le_bytes().to_vec();
            x.extend(ifmt.to_le_bytes());
            x.extend([0u8; 16]);
            globals.extend(rec(XF, &x));
        }
        globals.extend(rec(DATE1904, &1u16.to_le_bytes()));
        // SupBook (self, 2 sheets) + ExternSheet with XTI (0, 1, 1)
        globals.extend(rec(SUPBOOK, &[2, 0, 0x01, 0x04]));
        globals.extend(rec(EXTERNSHEET, &[1, 0, 0, 0, 1, 0, 1, 0]));
        // Lbl "Rate" → Sheet2!$A$1 (PtgRef3d)
        let mut l = vec![0, 0, 0, 4];
        l.extend(7u16.to_le_bytes()); // cce
        l.extend([0, 0, 0, 0, 0, 0, 0, 0]);
        l.push(0);
        l.extend(b"Rate");
        l.extend([0x3A, 0, 0, 0, 0, 0, 0]);
        globals.extend(rec(LBL, &l));
        // SST: "Xin chào" (UTF-16), "plain"; the second string's characters
        // continue in a CONTINUE record with a fresh flag byte.
        let mut sst = 3u32.to_le_bytes().to_vec();
        sst.extend(2u32.to_le_bytes());
        sst.extend(8u16.to_le_bytes());
        sst.push(1);
        for u in "Xin chào".encode_utf16() {
            sst.extend(u.to_le_bytes());
        }
        sst.extend(5u16.to_le_bytes());
        sst.push(0);
        sst.extend(b"pl");
        globals.extend(rec(SST, &sst));
        globals.extend(rec(CONTINUE, &[1, b'a', 0, b'i', 0, b'n', 0]));
        // BoundSheet8 placeholders; positions are patched below.
        let bs_at = globals.len();
        let mut bs1 = vec![0, 0, 0, 0, 0, 0];
        bs1.extend(short_str("Trang 1"));
        globals.extend(rec(BOUNDSHEET8, &bs1));
        let bs2_at = globals.len();
        let mut bs2 = vec![0, 0, 0, 0, 1, 0];
        bs2.extend(short_str("Sheet2"));
        globals.extend(rec(BOUNDSHEET8, &bs2));
        globals.extend(rec(EOF, &[]));

        let mut s1 = bof(0x0010);
        let mut n = cell_hdr(0, 0, 1);
        n.extend(1.25f64.to_le_bytes());
        s1.extend(rec(NUMBER, &n));
        let mut rk = cell_hdr(0, 1, 2);
        rk.extend(((45_000u32) << 2 | 2).to_le_bytes());
        s1.extend(rec(RK, &rk));
        let mut ls = cell_hdr(1, 0, 0);
        ls.extend(0u32.to_le_bytes());
        s1.extend(rec(LABELSST, &ls));
        let mut ls = cell_hdr(1, 1, 0);
        ls.extend(1u32.to_le_bytes());
        s1.extend(rec(LABELSST, &ls));
        let mut be = cell_hdr(2, 0, 0);
        be.extend([1, 0]);
        s1.extend(rec(BOOLERR, &be));
        let mut be = cell_hdr(2, 1, 0);
        be.extend([0x07, 1]);
        s1.extend(rec(BOOLERR, &be));
        // MulRk row 3: C4=1, D4=2
        let mut mr = 3u16.to_le_bytes().to_vec();
        mr.extend(2u16.to_le_bytes());
        for v in [1u32, 2] {
            mr.extend(0u16.to_le_bytes());
            mr.extend((v << 2 | 2).to_le_bytes());
        }
        mr.extend(3u16.to_le_bytes());
        s1.extend(rec(MULRK, &mr));
        // C1 = A1*2 → 2.5
        s1.extend(formula_rec(
            0,
            2,
            2.5f64.to_le_bytes(),
            &[0x24, 0, 0, 0, 0xC0, 0x1E, 2, 0, 0x05],
        ));
        // D1 = "a"&"b" with a string result in the following STRING record
        s1.extend(formula_rec(
            0,
            3,
            [0, 0, 0, 0, 0, 0, 0xFF, 0xFF],
            &[0x17, 1, 0, b'a', 0x17, 1, 0, b'b', 0x08],
        ));
        s1.extend(rec(STRING, &[2, 0, 0, b'a', b'b']));
        // E1:E2 shared formula = Sheet2!A1 + A(row)  (PtgRef3d + PtgRefN)
        let exp = [0x01, 0, 0, 4, 0];
        s1.extend(formula_rec(0, 4, 3.0f64.to_le_bytes(), &exp));
        let shr_rgce = [0x3A, 0, 0, 0, 0, 0, 0, 0x2C, 0, 0, 0xFC, 0xC0, 0x03];
        let mut shr = vec![0, 0, 1, 0, 4, 4, 0, 2];
        shr.extend((shr_rgce.len() as u16).to_le_bytes());
        shr.extend(shr_rgce);
        s1.extend(rec(SHRFMLA, &shr));
        s1.extend(formula_rec(1, 4, 4.0f64.to_le_bytes(), &exp));
        // an embedded chart substream is skipped
        s1.extend(bof(0x0020));
        let mut n = cell_hdr(9, 9, 0);
        n.extend(9.0f64.to_le_bytes());
        s1.extend(rec(NUMBER, &n));
        s1.extend(rec(EOF, &[]));
        s1.extend(rec(EOF, &[]));

        let mut s2 = bof(0x0010);
        let mut n = cell_hdr(0, 0, 0);
        n.extend(0.1f64.to_le_bytes());
        s2.extend(rec(NUMBER, &n));
        s2.extend(rec(EOF, &[]));

        let p1 = globals.len() as u32;
        let p2 = p1 + s1.len() as u32;
        globals[bs_at + 4..bs_at + 8].copy_from_slice(&p1.to_le_bytes());
        globals[bs2_at + 4..bs2_at + 8].copy_from_slice(&p2.to_le_bytes());
        let mut out = globals;
        out.extend(s1);
        out.extend(s2);
        out
    }

    pub fn sample_xls() -> Vec<u8> {
        crate::test_util::cfb_file(&[("/Workbook", sample_stream())], &[])
    }

    #[test]
    fn reads_sample_workbook() {
        let wb = read_xls(&sample_xls()).unwrap();
        assert!(wb.date1904);
        assert!(!wb.has_macros);
        assert_eq!(wb.sheets.len(), 2);
        let s = &wb.sheets[0];
        assert_eq!(s.name, "Trang 1");
        assert!(wb.sheets[1].hidden);
        let get = |r, c| s.cells.iter().find(|x| x.row == r && x.col == c).unwrap();
        assert_eq!(get(0, 0).value, CellValue::Number(1.25));
        assert_eq!(get(0, 0).number_format.as_deref(), Some("0.000"));
        assert_eq!(get(0, 1).value, CellValue::Number(45_000.0));
        assert_eq!(get(0, 1).number_format.as_deref(), Some("mm-dd-yy"));
        assert_eq!(get(1, 0).value, CellValue::Text("Xin chào".into()));
        assert_eq!(get(1, 1).value, CellValue::Text("plain".into()));
        assert_eq!(get(2, 0).value, CellValue::Bool(true));
        assert_eq!(get(2, 1).value, CellValue::Error("#DIV/0!".into()));
        assert_eq!(get(3, 3).value, CellValue::Number(2.0));
        assert_eq!(get(0, 2).formula.as_deref(), Some("A1*2"));
        assert_eq!(get(0, 3).formula.as_deref(), Some("\"a\"&\"b\""));
        assert_eq!(get(0, 3).value, CellValue::Text("ab".into()));
        assert_eq!(get(0, 4).formula.as_deref(), Some("Sheet2!$A$1+A1"));
        assert_eq!(get(1, 4).formula.as_deref(), Some("Sheet2!$A$1+A2"));
        assert!(
            s.cells.iter().all(|c| c.row != 9),
            "chart substream skipped"
        );
        assert_eq!(wb.names.len(), 1);
        assert_eq!(wb.names[0].name, "Rate");
        assert_eq!(wb.names[0].formula.as_deref(), Some("Sheet2!$A$1"));
        assert_eq!(wb.sheets[1].cells[0].value, CellValue::Number(0.1));
    }

    #[test]
    fn filepass_is_reported() {
        let mut s = bof(0x0005);
        s.extend(rec(FILEPASS, &[0, 0]));
        s.extend(rec(EOF, &[]));
        assert!(matches!(
            parse_workbook_stream(&s),
            Err(BinaryError::Encrypted)
        ));
    }

    #[test]
    fn vba_storage_flags_macros() {
        let f = crate::test_util::cfb_file(
            &[
                ("/Workbook", sample_stream()),
                ("/_VBA_PROJECT_CUR/VBA/dir", vec![1, 2, 3]),
            ],
            &["/_VBA_PROJECT_CUR", "/_VBA_PROJECT_CUR/VBA"],
        );
        assert!(read_xls(&f).unwrap().has_macros);
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]
        #[test]
        fn mutated_stream_never_panics(pos in 0usize..2048, val in proptest::num::u8::ANY, cut in 0usize..2048) {
            let mut s = sample_stream();
            let i = pos % s.len();
            s[i] = val;
            let _ = parse_workbook_stream(&s);
            s.truncate(cut % (s.len() + 1));
            let _ = parse_workbook_stream(&s);
        }

        #[test]
        fn random_stream_never_panics(data in proptest::collection::vec(proptest::num::u8::ANY, 0..1024)) {
            let _ = parse_workbook_stream(&data);
            let _ = read_xls(&data);
        }
    }
}
