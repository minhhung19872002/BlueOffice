//! Parsed formulas → A1-style formula text.
//!
//! BIFF8 `Rgce` ([MS-XLS] §2.5.198) and the BIFF12 variant used by `.xlsb`
//! ([MS-XLSB] §2.5.97) share the token set; they differ in the width of row
//! and column fields and in the string encoding, captured by [`Dialect`].

use crate::bytes::{f64_at, latin1, slice, u16_at, u32_at, u8_at, utf16, Rd};
use crate::sheet::{col_name, error_text};
use crate::{malformed, Result};

/// Which binary formula grammar a token stream uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    /// [MS-XLS] BIFF8: 16-bit rows, 8-bit columns, `ShortXLUnicodeString`.
    #[default]
    Biff8,
    /// [MS-XLSB] BIFF12: 20-bit rows (stored in 32 bits), 14-bit columns, `XLWideString`.
    Biff12,
}

impl Dialect {
    fn max_row(self) -> u32 {
        match self {
            Dialect::Biff8 => 0xFFFF,
            Dialect::Biff12 => 0xF_FFFF,
        }
    }
    fn max_col(self) -> u32 {
        match self {
            Dialect::Biff8 => 0xFF,
            Dialect::Biff12 => 0x3FFF,
        }
    }
    /// Bytes of one row field.
    fn rw(self) -> usize {
        match self {
            Dialect::Biff8 => 2,
            Dialect::Biff12 => 4,
        }
    }
    fn read_row(self, d: &[u8], off: usize) -> Result<u32> {
        match self {
            Dialect::Biff8 => u16_at(d, off).map(u32::from),
            Dialect::Biff12 => u32_at(d, off),
        }
    }
}

/// Context needed to print names and 3-D references.
#[derive(Debug, Default, Clone)]
pub struct FormulaCtx {
    /// Worksheet names by sheet index (BoundSheet8 / BrtBundleSh order).
    pub sheet_names: Vec<String>,
    /// EXTERNSHEET XTI entries: (iSupBook, itabFirst, itabLast).
    pub xti: Vec<(u32, i32, i32)>,
    /// SUPBOOK kinds by index: true for the self-referencing book.
    pub supbook_self: Vec<bool>,
    /// External names (EXTERNNAME, 1-based in `PtgNameX`) per SUPBOOK.
    pub extern_names: Vec<Vec<String>>,
    /// Defined names (Lbl / BrtName records, 1-based in `PtgName`).
    pub names: Vec<String>,
}

include!("ftab.rs");

/// Function name for an `iftab` (`None` for unassigned ids).
pub fn function_name(iftab: u16) -> Option<&'static str> {
    FTAB.get(iftab as usize).copied().filter(|n| !n.is_empty())
}

/// Format a number literal.
pub fn fmt_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Quote a sheet name if needed (`'My Sheet'!A1`).
fn sheet_ref(name: &str) -> String {
    let simple = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    if simple {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

/// One cell reference with its relative flags (`ColRelU` / `ColRelShort`:
/// bit 14 = column relative, bit 15 = row relative).
fn cell_ref(row: u32, col_field: u16) -> String {
    let col = u32::from(col_field & 0x3FFF);
    let col_rel = col_field & 0x4000 != 0;
    let row_rel = col_field & 0x8000 != 0;
    format!(
        "{}{}{}{}",
        if col_rel { "" } else { "$" },
        col_name(col),
        if row_rel { "" } else { "$" },
        u64::from(row) + 1
    )
}

fn area_ref(dialect: Dialect, r1: u32, r2: u32, c1: u16, c2: u16) -> String {
    let (col1, col2) = (u32::from(c1 & 0x3FFF), u32::from(c2 & 0x3FFF));
    if r1 == 0 && r2 == dialect.max_row() {
        // whole columns
        let a = |c: u16| {
            format!(
                "{}{}",
                if c & 0x4000 != 0 { "" } else { "$" },
                col_name(u32::from(c & 0x3FFF))
            )
        };
        return format!("{}:{}", a(c1), a(c2));
    }
    if col1 == 0 && col2 == dialect.max_col() {
        // whole rows
        let a = |r: u32, f: u16| {
            format!(
                "{}{}",
                if f & 0x8000 != 0 { "" } else { "$" },
                u64::from(r) + 1
            )
        };
        return format!("{}:{}", a(r1, c1), a(r2, c2));
    }
    format!("{}:{}", cell_ref(r1, c1), cell_ref(r2, c2))
}

/// Resolve a `PtgRefN` / `PtgAreaN` location against the formula's cell
/// ([MS-XLS] §2.5.198.88 PtgRefN, §2.5.198.31 PtgAreaN; [MS-XLSB] §2.5.97.86).
fn rel_n(dialect: Dialect, row: u32, col: u16, base: (u32, u32)) -> (u32, u16) {
    let rows = i64::from(dialect.max_row()) + 1;
    let cols = i64::from(dialect.max_col()) + 1;
    let r = if col & 0x8000 != 0 {
        // signed offset of the row field's width
        let off = match dialect {
            Dialect::Biff8 => i64::from(row as u16 as i16),
            Dialect::Biff12 => i64::from(row as i32),
        };
        (i64::from(base.0) + off).rem_euclid(rows) as u32
    } else {
        row
    };
    let c = if col & 0x4000 != 0 {
        let off = match dialect {
            Dialect::Biff8 => i64::from((col & 0xFF) as u8 as i8),
            // 14-bit signed
            Dialect::Biff12 => i64::from(((col & 0x3FFF) << 2) as i16 >> 2),
        };
        (i64::from(base.1) + off).rem_euclid(cols) as u16
    } else {
        col & 0x3FFF
    };
    (r, (col & 0xC000) | c)
}

impl FormulaCtx {
    fn xti_entry(&self, ixti: u16) -> Option<(u32, i32, i32)> {
        self.xti.get(ixti as usize).copied()
    }

    fn sheet_prefix(&self, ixti: u16) -> Option<String> {
        let (sup, first, last) = self.xti_entry(ixti)?;
        if !self.supbook_self.get(sup as usize).copied().unwrap_or(true) {
            return None;
        }
        let name = |i: i32| self.sheet_names.get(usize::try_from(i).ok()?).cloned();
        let a = name(first)?;
        if last == first {
            Some(format!("{}!", sheet_ref(&a)))
        } else {
            let b = name(last)?;
            Some(format!("{}!", sheet_ref(&format!("{a}:{b}"))))
        }
    }

    fn extern_name(&self, ixti: u16, idx: u32) -> Option<String> {
        let (sup, _, _) = self.xti_entry(ixti)?;
        let list = self.extern_names.get(sup as usize)?;
        list.get(usize::try_from(idx).ok()?.checked_sub(1)?)
            .cloned()
    }
}

/// Built-in defined names ([MS-XLS] §2.4.150 Lbl, `fBuiltin` + chKey).
pub fn builtin_name(i: u8) -> &'static str {
    match i {
        0 => "Consolidate_Area",
        1 => "Auto_Open",
        2 => "Auto_Close",
        3 => "Extract",
        4 => "Database",
        5 => "Criteria",
        6 => "Print_Area",
        7 => "Print_Titles",
        8 => "Recorder",
        9 => "Data_Form",
        10 => "Auto_Activate",
        11 => "Auto_Deactivate",
        12 => "Sheet_Title",
        13 => "_FilterDatabase",
        _ => "_Unknown",
    }
}

/// Maximum operand stack depth (guards pathological token streams).
const MAX_STACK: usize = 4096;
/// Maximum length of the produced formula text.
const MAX_TEXT: usize = 1 << 20;

fn push(stack: &mut Vec<String>, s: String) -> Result<()> {
    if stack.len() >= MAX_STACK || s.len() > MAX_TEXT {
        return Err(malformed("formula too complex"));
    }
    stack.push(s);
    Ok(())
}

fn pop(stack: &mut Vec<String>) -> Result<String> {
    stack
        .pop()
        .ok_or_else(|| malformed("formula stack underflow"))
}

fn pop_n(stack: &mut Vec<String>, n: usize) -> Result<Vec<String>> {
    if n > stack.len() {
        return Err(malformed("formula stack underflow"));
    }
    Ok(stack.split_off(stack.len() - n))
}

/// Read a BIFF8 array constant from `rgcb` ([MS-XLS] §2.5.198.8 PtgArray,
/// §2.5.198.59 PtgExtraArray, §2.5.192 SerAr).
fn array_constant(rgcb: &mut Rd<'_>) -> Result<String> {
    let cols = usize::from(rgcb.u8()?) + 1;
    let rows = usize::from(rgcb.u16()?) + 1;
    if cols * rows > 65_536 {
        return Err(malformed("array constant too large"));
    }
    let mut out = String::from("{");
    for r in 0..rows {
        if r > 0 {
            out.push(';');
        }
        for c in 0..cols {
            if c > 0 {
                out.push(',');
            }
            match rgcb.u8()? {
                0x00 => rgcb.skip(8)?,
                0x01 => out.push_str(&fmt_number(rgcb.f64()?)),
                0x02 => {
                    let cch = rgcb.u16()? as usize;
                    let high = rgcb.u8()? & 1 != 0;
                    let s = if high {
                        utf16(rgcb.take(cch * 2)?)
                    } else {
                        latin1(rgcb.take(cch)?)
                    };
                    out.push('"');
                    out.push_str(&s.replace('"', "\"\""));
                    out.push('"');
                }
                0x04 => {
                    let b = rgcb.u8()?;
                    rgcb.skip(7)?;
                    out.push_str(if b != 0 { "TRUE" } else { "FALSE" });
                }
                0x10 => {
                    let e = rgcb.u8()?;
                    rgcb.skip(7)?;
                    out.push_str(&error_text(e));
                }
                t => return Err(malformed(format!("SerAr type {t}"))),
            }
            if out.len() > MAX_TEXT {
                return Err(malformed("array constant too large"));
            }
        }
    }
    out.push('}');
    Ok(out)
}

/// The cell a `PtgExp` (shared / array formula pointer) refers to, if the
/// token stream starts with one ([MS-XLS] §2.5.198.58 PtgExp). For BIFF12
/// only the row is stored ([MS-XLSB] §2.5.97.37); the column is `u32::MAX`.
pub fn ptg_exp(rgce: &[u8], dialect: Dialect) -> Option<(u32, u32)> {
    if rgce.first() != Some(&0x01) {
        return None;
    }
    match dialect {
        Dialect::Biff8 => Some((
            u32::from(u16_at(rgce, 1).ok()?),
            u32::from(u16_at(rgce, 3).ok()?),
        )),
        Dialect::Biff12 => Some((u32_at(rgce, 1).ok()?, u32::MAX)),
    }
}

/// Convert BIFF8 `rgce` (+ `rgcb` extra data) to formula text. `base` is the
/// cell the formula belongs to (for relative `PtgRefN`/`PtgAreaN` tokens).
pub fn formula_to_text(
    rgce: &[u8],
    rgcb: &[u8],
    ctx: &FormulaCtx,
    base: (u32, u32),
) -> Result<String> {
    formula_to_text_with(Dialect::Biff8, rgce, rgcb, ctx, base)
}

/// Convert a parsed formula of the given dialect to formula text.
pub fn formula_to_text_with(
    dialect: Dialect,
    rgce: &[u8],
    rgcb: &[u8],
    ctx: &FormulaCtx,
    base: (u32, u32),
) -> Result<String> {
    let mut stack: Vec<String> = Vec::new();
    let mut extra = Rd::new(rgcb);
    let rw = dialect.rw();
    // RgceLoc and RgceArea sizes.
    let loc = rw + 2;
    let area = 2 * rw + 4;
    let mut i = 0usize;
    while i < rgce.len() {
        let ptg = rgce[i];
        i += 1;
        let d = &rgce[i..];
        let mut used = 0usize;
        match ptg {
            // PtgExp / PtgTbl
            0x01 | 0x02 => return Err(malformed("PtgExp/PtgTbl outside cell context")),
            // §2.5.198.26 PtgAdd … §2.5.198.83 PtgRange: binary operators
            0x03..=0x11 => {
                let b = pop(&mut stack)?;
                let a = pop(&mut stack)?;
                let op = match ptg {
                    0x03 => "+",
                    0x04 => "-",
                    0x05 => "*",
                    0x06 => "/",
                    0x07 => "^",
                    0x08 => "&",
                    0x09 => "<",
                    0x0A => "<=",
                    0x0B => "=",
                    0x0C => ">=",
                    0x0D => ">",
                    0x0E => "<>",
                    0x0F => " ",
                    0x10 => ",",
                    _ => ":",
                };
                push(&mut stack, format!("{a}{op}{b}"))?;
            }
            // PtgUplus, PtgUminus, PtgPercent, PtgParen
            0x12 => {
                let a = pop(&mut stack)?;
                push(&mut stack, format!("+{a}"))?;
            }
            0x13 => {
                let a = pop(&mut stack)?;
                push(&mut stack, format!("-{a}"))?;
            }
            0x14 => {
                let a = pop(&mut stack)?;
                push(&mut stack, format!("{a}%"))?;
            }
            0x15 => {
                let a = pop(&mut stack)?;
                push(&mut stack, format!("({a})"))?;
            }
            // PtgMissArg
            0x16 => push(&mut stack, String::new())?,
            // PtgStr
            0x17 => {
                let s = match dialect {
                    // ShortXLUnicodeString
                    Dialect::Biff8 => {
                        let cch = u8_at(d, 0)? as usize;
                        let high = u8_at(d, 1)? & 1 != 0;
                        if high {
                            used = 2 + cch * 2;
                            utf16(slice(d, 2, cch * 2)?)
                        } else {
                            used = 2 + cch;
                            latin1(slice(d, 2, cch)?)
                        }
                    }
                    // [MS-XLSB] §2.5.97.81 PtgStr: cch (2 bytes) + UTF-16 characters
                    Dialect::Biff12 => {
                        let cch = u16_at(d, 0)? as usize;
                        used = 2 + cch * 2;
                        utf16(slice(d, 2, cch * 2)?)
                    }
                };
                push(&mut stack, format!("\"{}\"", s.replace('"', "\"\"")))?;
            }
            // §2.5.198.34 PtgAttr*
            0x19 => {
                let t = u8_at(d, 0)?;
                used = 3;
                if t & 0x04 != 0 {
                    // PtgAttrChoose: jump table of (cOffset + 1) words
                    let n = u16_at(d, 1)? as usize;
                    used += (n + 1) * 2;
                } else if t & 0x10 != 0 {
                    // PtgAttrSum: SUM with one argument
                    let a = pop(&mut stack)?;
                    push(&mut stack, format!("SUM({a})"))?;
                }
                // attrSemi, attrIf, attrGoto, attrBaxcel, attrSpace: no text effect
            }
            // PtgErr, PtgBool, PtgInt, PtgNum
            0x1C => {
                push(&mut stack, error_text(u8_at(d, 0)?))?;
                used = 1;
            }
            0x1D => {
                let b = if u8_at(d, 0)? != 0 { "TRUE" } else { "FALSE" };
                push(&mut stack, b.into())?;
                used = 1;
            }
            0x1E => {
                push(&mut stack, u16_at(d, 0)?.to_string())?;
                used = 2;
            }
            0x1F => {
                push(&mut stack, fmt_number(f64_at(d, 0)?))?;
                used = 8;
            }
            0x20..=0x7F => match ptg & 0x1F {
                // PtgArray
                0x00 => {
                    if dialect == Dialect::Biff12 {
                        return Err(malformed("BIFF12 array constants not supported"));
                    }
                    used = 7;
                    let s = array_constant(&mut extra)?;
                    push(&mut stack, s)?;
                }
                // PtgFunc
                0x01 => {
                    let iftab = u16_at(d, 0)?;
                    used = 2;
                    let name = function_name(iftab)
                        .ok_or_else(|| malformed(format!("function {iftab}")))?;
                    let argc = fixed_argc(iftab)
                        .ok_or_else(|| malformed(format!("{name} needs PtgFuncVar")))?;
                    let args = pop_n(&mut stack, argc)?;
                    push(&mut stack, format!("{name}({})", args.join(",")))?;
                }
                // PtgFuncVar
                0x02 => {
                    let argc = u8_at(d, 0)? as usize & 0x7F;
                    let tab = u16_at(d, 1)?;
                    used = 3;
                    let mut args = pop_n(&mut stack, argc)?;
                    let iftab = tab & 0x7FFF;
                    let name = if iftab == 255 && tab & 0x8000 == 0 {
                        // user-defined / add-in function: first argument is its name
                        if args.is_empty() {
                            return Err(malformed("UDF without name"));
                        }
                        args.remove(0)
                    } else {
                        function_name(iftab)
                            .map(str::to_owned)
                            .ok_or_else(|| malformed(format!("function {iftab}")))?
                    };
                    push(&mut stack, format!("{name}({})", args.join(",")))?;
                }
                // PtgName
                0x03 => {
                    let idx = u32_at(d, 0)? as usize;
                    used = 4;
                    let name = idx
                        .checked_sub(1)
                        .and_then(|k| ctx.names.get(k))
                        .cloned()
                        .unwrap_or_else(|| "#NAME?".into());
                    push(&mut stack, name)?;
                }
                // PtgRef
                0x04 => {
                    let s = cell_ref(dialect.read_row(d, 0)?, u16_at(d, rw)?);
                    push(&mut stack, s)?;
                    used = loc;
                }
                // PtgArea
                0x05 => {
                    let s = area_ref(
                        dialect,
                        dialect.read_row(d, 0)?,
                        dialect.read_row(d, rw)?,
                        u16_at(d, 2 * rw)?,
                        u16_at(d, 2 * rw + 2)?,
                    );
                    push(&mut stack, s)?;
                    used = area;
                }
                // PtgMemArea: its PtgExtraMem sits in rgcb.
                0x06 => {
                    used = 6;
                    if dialect == Dialect::Biff8 {
                        let n = extra.u16()? as usize;
                        extra.skip(n * 8)?;
                    }
                }
                // PtgMemErr, PtgMemNoMem
                0x07 | 0x08 => used = 6,
                // PtgMemFunc
                0x09 => used = 2,
                // PtgRefErr / PtgAreaErr
                0x0A => {
                    push(&mut stack, "#REF!".into())?;
                    used = loc;
                }
                0x0B => {
                    push(&mut stack, "#REF!".into())?;
                    used = area;
                }
                // PtgRefN
                0x0C => {
                    let (r, c) = rel_n(dialect, dialect.read_row(d, 0)?, u16_at(d, rw)?, base);
                    push(&mut stack, cell_ref(r, c))?;
                    used = loc;
                }
                // PtgAreaN
                0x0D => {
                    let (r1, c1) =
                        rel_n(dialect, dialect.read_row(d, 0)?, u16_at(d, 2 * rw)?, base);
                    let (r2, c2) = rel_n(
                        dialect,
                        dialect.read_row(d, rw)?,
                        u16_at(d, 2 * rw + 2)?,
                        base,
                    );
                    push(&mut stack, area_ref(dialect, r1, r2, c1, c2))?;
                    used = area;
                }
                // PtgNameX (external / add-in name)
                0x19 => {
                    let ixti = u16_at(d, 0)?;
                    let idx = match dialect {
                        Dialect::Biff8 => u32::from(u16_at(d, 2)?),
                        Dialect::Biff12 => u32_at(d, 2)?,
                    };
                    let name = ctx
                        .extern_name(ixti, idx)
                        .unwrap_or_else(|| "#NAME?".into());
                    push(&mut stack, name)?;
                    used = 6;
                }
                // PtgRef3d
                0x1A => {
                    let ixti = u16_at(d, 0)?;
                    let r = cell_ref(dialect.read_row(d, 2)?, u16_at(d, 2 + rw)?);
                    let s = match ctx.sheet_prefix(ixti) {
                        Some(p) => format!("{p}{r}"),
                        None => "#REF!".into(),
                    };
                    push(&mut stack, s)?;
                    used = 2 + loc;
                }
                // PtgArea3d
                0x1B => {
                    let ixti = u16_at(d, 0)?;
                    let a = area_ref(
                        dialect,
                        dialect.read_row(d, 2)?,
                        dialect.read_row(d, 2 + rw)?,
                        u16_at(d, 2 + 2 * rw)?,
                        u16_at(d, 4 + 2 * rw)?,
                    );
                    let s = match ctx.sheet_prefix(ixti) {
                        Some(p) => format!("{p}{a}"),
                        None => "#REF!".into(),
                    };
                    push(&mut stack, s)?;
                    used = 2 + area;
                }
                // PtgRefErr3d / PtgAreaErr3d
                0x1C => {
                    push(&mut stack, "#REF!".into())?;
                    used = 2 + loc;
                }
                0x1D => {
                    push(&mut stack, "#REF!".into())?;
                    used = 2 + area;
                }
                other => return Err(malformed(format!("unsupported ptg 0x{other:02X}"))),
            },
            other => return Err(malformed(format!("unsupported ptg 0x{other:02X}"))),
        }
        if used > d.len() {
            return Err(malformed("ptg data truncated"));
        }
        i += used;
    }
    let out = pop(&mut stack)?;
    if !stack.is_empty() {
        return Err(malformed("formula leaves extra operands"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(rgce: &[u8]) -> String {
        formula_to_text(rgce, &[], &FormulaCtx::default(), (0, 0)).unwrap()
    }

    #[test]
    fn ftab_spot_checks() {
        // [MS-XLS] §2.5.198.17 Ftab values.
        assert_eq!(function_name(0), Some("COUNT"));
        assert_eq!(function_name(4), Some("SUM"));
        assert_eq!(function_name(102), Some("VLOOKUP"));
        assert_eq!(function_name(216), Some("RANK"));
        assert_eq!(function_name(219), Some("ADDRESS"));
        assert_eq!(function_name(255), None);
        assert_eq!(function_name(336), Some("CONCATENATE"));
        assert_eq!(function_name(367), Some("VARA"));
        assert_eq!(function_name(379), Some("RTD"));
        // [MS-XLSB] extension
        assert_eq!(function_name(449), Some("EDATE"));
        assert_eq!(function_name(480), Some("IFERROR"));
        assert_eq!(function_name(484), Some("AVERAGEIFS"));
        assert_eq!(function_name(485), None);
        assert_eq!(FTAB.len(), 485);
        // every fixed-arg id names a function
        for i in 0..600u16 {
            if fixed_argc(i).is_some() {
                assert!(function_name(i).is_some(), "{i}");
            }
        }
    }

    #[test]
    fn arithmetic_and_refs() {
        // A1 + $B$2 * 2  → PtgRef(A1 rel), PtgRef($B$2), PtgInt 2, PtgMul, PtgAdd
        let mut v = vec![0x24, 0, 0, 0x00, 0xC0];
        v.extend([0x24, 1, 0, 1, 0x00]);
        v.extend([0x1E, 2, 0, 0x05, 0x03]);
        assert_eq!(t(&v), "A1+$B$2*2");
    }

    #[test]
    fn functions_and_strings() {
        // SUM(A1:B3) via PtgArea + PtgFuncVar(1, 4)
        let mut v = vec![0x25, 0, 0, 2, 0, 0x00, 0xC0, 0x01, 0xC0];
        v.extend([0x42, 1, 4, 0]);
        assert_eq!(t(&v), "SUM(A1:B3)");
        // IF(TRUE,"a""b",1.5) with PtgAttr if/goto noise
        let mut v = vec![0x1D, 1, 0x19, 0x02, 4, 0, 0x17, 3, 0, b'a', b'"', b'b'];
        v.extend([0x1F]);
        v.extend(1.5f64.to_le_bytes());
        v.extend([0x19, 0x08, 3, 0, 0x42, 3, 1, 0]);
        assert_eq!(t(&v), "IF(TRUE,\"a\"\"b\",1.5)");
        // ROUND(PI(),2) via PtgFunc
        assert_eq!(t(&[0x21, 19, 0, 0x1E, 2, 0, 0x41, 27, 0]), "ROUND(PI(),2)");
        // attrSum
        assert_eq!(
            t(&[0x25, 0, 0, 9, 0, 0, 0xC0, 0, 0xC0, 0x19, 0x10, 0, 0]),
            "SUM(A1:A10)"
        );
        // A PtgFunc with a variadic id is rejected, not guessed.
        assert!(formula_to_text(&[0x21, 4, 0], &[], &FormulaCtx::default(), (0, 0)).is_err());
    }

    #[test]
    fn whole_columns_and_errors() {
        assert_eq!(t(&[0x25, 0, 0, 0xFF, 0xFF, 0, 0xC0, 1, 0xC0]), "A:B");
        assert_eq!(t(&[0x25, 0, 0, 4, 0, 0, 0x80, 0xFF, 0x80]), "1:5");
        assert_eq!(t(&[0x1C, 0x07]), "#DIV/0!");
        assert!(formula_to_text(&[0x03], &[], &FormulaCtx::default(), (0, 0)).is_err());
        assert!(formula_to_text(&[0x24, 0], &[], &FormulaCtx::default(), (0, 0)).is_err());
    }

    #[test]
    fn three_d_and_names() {
        let ctx = FormulaCtx {
            sheet_names: vec!["Sheet1".into(), "Bảng 2".into()],
            xti: vec![(0, 1, 1), (0, 0, 1), (1, -2, -2)],
            supbook_self: vec![true, false],
            extern_names: vec![Vec::new(), vec!["_xlfn.IFERROR".into()]],
            names: vec!["Thuế".into()],
        };
        let f = |v: &[u8]| formula_to_text(v, &[], &ctx, (0, 0)).unwrap();
        assert_eq!(f(&[0x3A, 0, 0, 4, 0, 2, 0xC0]), "'Bảng 2'!C5");
        assert_eq!(f(&[0x3A, 1, 0, 0, 0, 0, 0x00]), "'Sheet1:Bảng 2'!$A$1");
        assert_eq!(f(&[0x23, 1, 0, 0, 0]), "Thuế");
        // _xlfn.IFERROR(A1,0): PtgNameX, PtgRef, PtgInt, PtgFuncVar(3, 255)
        let v = [
            0x39, 2, 0, 1, 0, 0, 0, 0x24, 0, 0, 0, 0xC0, 0x1E, 0, 0, 0x42, 3, 0xFF, 0,
        ];
        assert_eq!(f(&v), "_xlfn.IFERROR(A1,0)");
        // reference into an external book is not printable → #REF!
        assert_eq!(f(&[0x3A, 2, 0, 0, 0, 0, 0]), "#REF!");
    }

    #[test]
    fn shared_relative() {
        // PtgRefN with row offset -1 (0xFFFF) and col offset +1, base (5, 2) → D5
        let v = [0x2C, 0xFF, 0xFF, 0x01, 0xC0];
        assert_eq!(
            formula_to_text(&v, &[], &FormulaCtx::default(), (5, 2)).unwrap(),
            "D5"
        );
        // negative column offset wraps inside the sheet: base col 0, offset -1 → IV
        let v = [0x2C, 0, 0, 0xFF, 0xC0];
        assert_eq!(
            formula_to_text(&v, &[], &FormulaCtx::default(), (0, 0)).unwrap(),
            "IV1"
        );
        assert_eq!(ptg_exp(&[0x01, 2, 0, 3, 0], Dialect::Biff8), Some((2, 3)));
        assert_eq!(ptg_exp(&[0x24, 2, 0, 3, 0], Dialect::Biff8), None);
    }

    #[test]
    fn array_constants() {
        let mut cb = vec![1u8, 1, 0]; // 2 cols, 2 rows
        cb.push(0x01);
        cb.extend(1.0f64.to_le_bytes());
        cb.extend([0x02, 1, 0, 0, b'x']);
        cb.extend([0x04, 1, 0, 0, 0, 0, 0, 0, 0]);
        cb.extend([0x10, 0x2A, 0, 0, 0, 0, 0, 0, 0]);
        let v = [0x60, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            formula_to_text(&v, &cb, &FormulaCtx::default(), (0, 0)).unwrap(),
            "{1,\"x\";TRUE,#N/A}"
        );
    }

    #[test]
    fn biff12_tokens() {
        let ctx = FormulaCtx {
            sheet_names: vec!["Data".into()],
            xti: vec![(0, 0, 0)],
            ..FormulaCtx::default()
        };
        let f = |v: &[u8], base| formula_to_text_with(Dialect::Biff12, v, &[], &ctx, base).unwrap();
        // PtgRef row 99999 col XFD (relative)
        let mut v = vec![0x24];
        v.extend(99_999u32.to_le_bytes());
        v.extend((0x3FFFu16 | 0xC000).to_le_bytes());
        assert_eq!(f(&v, (0, 0)), "XFD100000");
        // SUM(Data!$A:$A) & "ok"
        let mut v = vec![0x3B, 0, 0];
        v.extend(0u32.to_le_bytes());
        v.extend(0xF_FFFFu32.to_le_bytes());
        v.extend(0u16.to_le_bytes());
        v.extend(0u16.to_le_bytes());
        v.extend([0x42, 1, 4, 0]);
        v.extend([0x17, 2, 0, b'o', 0, b'k', 0, 0x08]);
        assert_eq!(f(&v, (0, 0)), "SUM(Data!$A:$A)&\"ok\"");
        // PtgRefN: row -2, col -1 from (10, 10) → J9
        let mut v = vec![0x2C];
        v.extend((-2i32).to_le_bytes());
        v.extend((0xC000u16 | 0x3FFF).to_le_bytes());
        assert_eq!(f(&v, (10, 10)), "J9");
        assert_eq!(
            ptg_exp(&[0x01, 5, 0, 0, 0], Dialect::Biff12),
            Some((5, u32::MAX))
        );
    }

    proptest::proptest! {
        #[test]
        fn random_tokens_never_panic(
            rgce in proptest::collection::vec(proptest::num::u8::ANY, 0..256),
            rgcb in proptest::collection::vec(proptest::num::u8::ANY, 0..64),
        ) {
            let ctx = FormulaCtx::default();
            let _ = formula_to_text(&rgce, &rgcb, &ctx, (3, 3));
            let _ = formula_to_text_with(Dialect::Biff12, &rgce, &rgcb, &ctx, (3, 3));
        }
    }
}
