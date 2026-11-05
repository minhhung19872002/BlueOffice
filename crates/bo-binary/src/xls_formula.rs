//! BIFF8 parsed formulas (`Rgce`, [MS-XLS] §2.5.198) → A1-style formula text.

use crate::bytes::{f64_at, i16_at, latin1, u16_at, u32_at, u8_at, utf16, Rd};
use crate::sheet::{col_name, error_text};
use crate::{malformed, Result};

/// Context needed to print names and 3-D references.
#[derive(Debug, Default)]
pub struct FormulaCtx {
    /// Worksheet names by sheet index (BoundSheet8 order).
    pub sheet_names: Vec<String>,
    /// EXTERNSHEET XTI entries: (iSupBook, itabFirst, itabLast).
    pub xti: Vec<(u16, i16, i16)>,
    /// SUPBOOK kinds by index: true for the self-referencing book.
    pub supbook_self: Vec<bool>,
    /// Defined names (Lbl records, 1-based in PtgName).
    pub names: Vec<String>,
}

/// [MS-XLS] §2.5.198.17 Ftab: function index → (name, fixed argument count or -1).
pub fn ftab(i: u16) -> Option<(&'static str, i8)> {
    Some(match i {
        0 => ("COUNT", -1),
        1 => ("IF", -1),
        2 => ("ISNA", 1),
        3 => ("ISERROR", 1),
        4 => ("SUM", -1),
        5 => ("AVERAGE", -1),
        6 => ("MIN", -1),
        7 => ("MAX", -1),
        8 => ("ROW", -1),
        9 => ("COLUMN", -1),
        10 => ("NA", 0),
        11 => ("NPV", -1),
        12 => ("STDEV", -1),
        13 => ("DOLLAR", -1),
        14 => ("FIXED", -1),
        15 => ("SIN", 1),
        16 => ("COS", 1),
        17 => ("TAN", 1),
        18 => ("ATAN", 1),
        19 => ("PI", 0),
        20 => ("SQRT", 1),
        21 => ("EXP", 1),
        22 => ("LN", 1),
        23 => ("LOG10", 1),
        24 => ("ABS", 1),
        25 => ("INT", 1),
        26 => ("SIGN", 1),
        27 => ("ROUND", 2),
        28 => ("LOOKUP", -1),
        29 => ("INDEX", -1),
        30 => ("REPT", 2),
        31 => ("MID", 3),
        32 => ("LEN", 1),
        33 => ("VALUE", 1),
        34 => ("TRUE", 0),
        35 => ("FALSE", 0),
        36 => ("AND", -1),
        37 => ("OR", -1),
        38 => ("NOT", 1),
        39 => ("MOD", 2),
        40 => ("DCOUNT", 3),
        41 => ("DSUM", 3),
        42 => ("DAVERAGE", 3),
        43 => ("DMIN", 3),
        44 => ("DMAX", 3),
        45 => ("DSTDEV", 3),
        46 => ("VAR", -1),
        47 => ("DVAR", 3),
        48 => ("TEXT", 2),
        49 => ("LINEST", -1),
        50 => ("TREND", -1),
        51 => ("LOGEST", -1),
        52 => ("GROWTH", -1),
        56 => ("PV", -1),
        57 => ("FV", -1),
        58 => ("NPER", -1),
        59 => ("PMT", -1),
        60 => ("RATE", -1),
        61 => ("MIRR", 3),
        62 => ("IRR", -1),
        63 => ("RAND", 0),
        64 => ("MATCH", -1),
        65 => ("DATE", 3),
        66 => ("TIME", 3),
        67 => ("DAY", 1),
        68 => ("MONTH", 1),
        69 => ("YEAR", 1),
        70 => ("WEEKDAY", -1),
        71 => ("HOUR", 1),
        72 => ("MINUTE", 1),
        73 => ("SECOND", 1),
        74 => ("NOW", 0),
        75 => ("AREAS", 1),
        76 => ("ROWS", 1),
        77 => ("COLUMNS", 1),
        78 => ("OFFSET", -1),
        82 => ("SEARCH", -1),
        83 => ("TRANSPOSE", 1),
        86 => ("TYPE", 1),
        97 => ("ATAN2", 2),
        98 => ("ASIN", 1),
        99 => ("ACOS", 1),
        100 => ("CHOOSE", -1),
        101 => ("HLOOKUP", -1),
        102 => ("VLOOKUP", -1),
        105 => ("ISREF", 1),
        109 => ("LOG", -1),
        111 => ("CHAR", 1),
        112 => ("LOWER", 1),
        113 => ("UPPER", 1),
        114 => ("PROPER", 1),
        115 => ("LEFT", -1),
        116 => ("RIGHT", -1),
        117 => ("EXACT", 2),
        118 => ("TRIM", 1),
        119 => ("REPLACE", 4),
        120 => ("SUBSTITUTE", -1),
        121 => ("CODE", 1),
        124 => ("FIND", -1),
        125 => ("CELL", -1),
        126 => ("ISERR", 1),
        127 => ("ISTEXT", 1),
        128 => ("ISNUMBER", 1),
        129 => ("ISBLANK", 1),
        130 => ("T", 1),
        131 => ("N", 1),
        140 => ("DATEVALUE", 1),
        141 => ("TIMEVALUE", 1),
        142 => ("SLN", 3),
        143 => ("SYD", 4),
        144 => ("DDB", -1),
        148 => ("INDIRECT", -1),
        162 => ("CLEAN", 1),
        163 => ("MDETERM", 1),
        164 => ("MINVERSE", 1),
        165 => ("MMULT", 2),
        167 => ("IPMT", -1),
        168 => ("PPMT", -1),
        169 => ("COUNTA", -1),
        183 => ("PRODUCT", -1),
        184 => ("FACT", 1),
        189 => ("DPRODUCT", 3),
        190 => ("ISNONTEXT", 1),
        193 => ("STDEVP", -1),
        194 => ("VARP", -1),
        195 => ("DSTDEVP", 3),
        196 => ("DVARP", 3),
        197 => ("TRUNC", -1),
        198 => ("ISLOGICAL", 1),
        199 => ("DCOUNTA", 3),
        212 => ("ROUNDUP", 2),
        213 => ("ROUNDDOWN", 2),
        216 => ("RANK", -1),
        219 => ("ADDRESS", -1),
        220 => ("DAYS360", -1),
        221 => ("TODAY", 0),
        222 => ("VDB", -1),
        227 => ("MEDIAN", -1),
        228 => ("SUMPRODUCT", -1),
        229 => ("SINH", 1),
        230 => ("COSH", 1),
        231 => ("TANH", 1),
        232 => ("ASINH", 1),
        233 => ("ACOSH", 1),
        234 => ("ATANH", 1),
        235 => ("DGET", 3),
        244 => ("INFO", 1),
        247 => ("DB", -1),
        252 => ("FREQUENCY", 2),
        261 => ("ERROR.TYPE", 1),
        269 => ("AVEDEV", -1),
        270 => ("BETADIST", -1),
        271 => ("GAMMALN", 1),
        272 => ("BETAINV", -1),
        273 => ("BINOMDIST", 4),
        274 => ("CHIDIST", 2),
        275 => ("CHIINV", 2),
        276 => ("COMBIN", 2),
        277 => ("CONFIDENCE", 3),
        278 => ("CRITBINOM", 3),
        279 => ("EVEN", 1),
        280 => ("EXPONDIST", 3),
        281 => ("FDIST", 3),
        282 => ("FINV", 3),
        283 => ("FISHER", 1),
        284 => ("FISHERINV", 1),
        285 => ("FLOOR", 2),
        286 => ("GAMMADIST", 4),
        287 => ("GAMMAINV", 3),
        288 => ("CEILING", 2),
        289 => ("HYPGEOMDIST", 4),
        290 => ("LOGNORMDIST", 3),
        291 => ("LOGINV", 3),
        292 => ("NEGBINOMDIST", 3),
        293 => ("NORMDIST", 4),
        294 => ("NORMSDIST", 1),
        295 => ("NORMINV", 3),
        296 => ("NORMSINV", 1),
        297 => ("STANDARDIZE", 3),
        298 => ("ODD", 1),
        299 => ("PERMUT", 2),
        300 => ("POISSON", 3),
        301 => ("TDIST", 3),
        302 => ("WEIBULL", 4),
        303 => ("SUMXMY2", 2),
        304 => ("SUMX2MY2", 2),
        305 => ("SUMX2PY2", 2),
        306 => ("CHITEST", 2),
        307 => ("CORREL", 2),
        308 => ("COVAR", 2),
        309 => ("FORECAST", 3),
        310 => ("FTEST", 2),
        311 => ("INTERCEPT", 2),
        312 => ("PEARSON", 2),
        313 => ("RSQ", 2),
        314 => ("STEYX", 2),
        315 => ("SLOPE", 2),
        316 => ("TTEST", 4),
        317 => ("PROB", -1),
        318 => ("DEVSQ", -1),
        319 => ("GEOMEAN", -1),
        320 => ("HARMEAN", -1),
        321 => ("SUMSQ", -1),
        322 => ("KURT", -1),
        323 => ("SKEW", -1),
        324 => ("ZTEST", -1),
        325 => ("LARGE", 2),
        326 => ("SMALL", 2),
        327 => ("QUARTILE", 2),
        328 => ("PERCENTILE", 2),
        329 => ("PERCENTRANK", -1),
        330 => ("MODE", -1),
        331 => ("TRIMMEAN", 2),
        332 => ("TINV", 2),
        336 => ("CONCATENATE", -1),
        337 => ("POWER", 2),
        342 => ("RADIANS", 1),
        343 => ("DEGREES", 1),
        344 => ("SUBTOTAL", -1),
        345 => ("SUMIF", -1),
        346 => ("COUNTIF", 2),
        347 => ("COUNTBLANK", 1),
        350 => ("ISPMT", 4),
        351 => ("DATEDIF", 3),
        354 => ("ROMAN", -1),
        358 => ("GETPIVOTDATA", -1),
        359 => ("HYPERLINK", -1),
        361 => ("AVERAGEA", -1),
        362 => ("MAXA", -1),
        363 => ("MINA", -1),
        364 => ("STDEVPA", -1),
        365 => ("VARPA", -1),
        366 => ("STDEVA", -1),
        367 => ("VARA", -1),
        _ => return None,
    })
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

/// One cell reference with its relative flags (BIFF8 `RgceLoc` / `ColRelU`).
fn cell_ref(row: u16, col_field: u16) -> String {
    let col = u32::from(col_field & 0x3FFF);
    let col_rel = col_field & 0x4000 != 0;
    let row_rel = col_field & 0x8000 != 0;
    format!(
        "{}{}{}{}",
        if col_rel { "" } else { "$" },
        col_name(col),
        if row_rel { "" } else { "$" },
        u32::from(row) + 1
    )
}

fn area_ref(r1: u16, r2: u16, c1: u16, c2: u16) -> String {
    let (col1, col2) = (c1 & 0x3FFF, c2 & 0x3FFF);
    if r1 == 0 && r2 == 0xFFFF {
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
    if col1 == 0 && col2 == 0xFF {
        // whole rows
        let a = |r: u16, f: u16| format!("{}{}", if f & 0x8000 != 0 { "" } else { "$" }, u32::from(r) + 1);
        return format!("{}:{}", a(r1, c1), a(r2, c2));
    }
    format!("{}:{}", cell_ref(r1, c1), cell_ref(r2, c2))
}

/// Apply a relative offset for `PtgRefN` / `PtgAreaN` (§2.5.198.88, §2.5.198.30).
fn rel_n(row: u16, col: u16, base: (u32, u32)) -> (u16, u16) {
    let r = if col & 0x8000 != 0 {
        (base.0 as u16).wrapping_add(row)
    } else {
        row
    };
    let c = if col & 0x4000 != 0 {
        ((base.1 as u16).wrapping_add(col & 0xFF)) & 0xFF
    } else {
        col & 0x3FFF
    };
    (r, (col & 0xC000) | c)
}

impl FormulaCtx {
    fn sheet_prefix(&self, ixti: u16) -> Option<String> {
        let &(sup, first, last) = self.xti.get(ixti as usize)?;
        if !self.supbook_self.get(sup as usize).copied().unwrap_or(true) {
            return None;
        }
        let name = |i: i16| self.sheet_names.get(usize::try_from(i).ok()?).cloned();
        let a = name(first)?;
        if last == first {
            Some(format!("{}!", sheet_ref(&a)))
        } else {
            let b = name(last)?;
            Some(format!("{}!", sheet_ref(&format!("{a}:{b}"))))
        }
    }
}

/// Built-in defined names (§2.5.? Lbl.fBuiltin, chKey index).
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

fn pop(stack: &mut Vec<String>) -> Result<String> {
    stack.pop().ok_or_else(|| malformed("formula stack underflow"))
}

fn pop_n(stack: &mut Vec<String>, n: usize) -> Result<Vec<String>> {
    if n > stack.len() {
        return Err(malformed("formula stack underflow"));
    }
    Ok(stack.split_off(stack.len() - n))
}

/// Read a `PtgArray` constant from `rgcb` (§2.5.198.8 PtgArray, §2.5.198.? SerAr).
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
        }
    }
    out.push('}');
    Ok(out)
}

/// Convert `rgce` (+ `rgcb` extra data) to formula text. `base` is the cell
/// the formula belongs to (for relative `PtgRefN`/`PtgAreaN` tokens).
pub fn formula_to_text(rgce: &[u8], rgcb: &[u8], ctx: &FormulaCtx, base: (u32, u32)) -> Result<String> {
    let mut stack: Vec<String> = Vec::new();
    let mut extra = Rd::new(rgcb);
    let mut i = 0usize;
    while i < rgce.len() {
        let ptg = rgce[i];
        i += 1;
        let d = &rgce[i..];
        let mut used = 0usize;
        match ptg {
            0x01 | 0x02 => return Err(malformed("PtgExp/PtgTbl outside cell context")),
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
                stack.push(format!("{a}{op}{b}"));
            }
            0x12 => {
                let a = pop(&mut stack)?;
                stack.push(format!("+{a}"));
            }
            0x13 => {
                let a = pop(&mut stack)?;
                stack.push(format!("-{a}"));
            }
            0x14 => {
                let a = pop(&mut stack)?;
                stack.push(format!("{a}%"));
            }
            0x15 => {
                let a = pop(&mut stack)?;
                stack.push(format!("({a})"));
            }
            0x16 => stack.push(String::new()),
            // PtgStr: ShortXLUnicodeString
            0x17 => {
                let cch = u8_at(d, 0)? as usize;
                let high = u8_at(d, 1)? & 1 != 0;
                let s = if high {
                    used = 2 + cch * 2;
                    utf16(crate::bytes::slice(d, 2, cch * 2)?)
                } else {
                    used = 2 + cch;
                    latin1(crate::bytes::slice(d, 2, cch)?)
                };
                stack.push(format!("\"{}\"", s.replace('"', "\"\"")));
            }
            // PtgAttr
            0x19 => {
                let t = u8_at(d, 0)?;
                used = 3;
                if t & 0x04 != 0 {
                    // attrChoose: jump table of (cOffset + 1) words
                    let n = u16_at(d, 1)? as usize;
                    used += (n + 1) * 2;
                } else if t & 0x10 != 0 {
                    // attrSum: SUM with one argument
                    let a = pop(&mut stack)?;
                    stack.push(format!("SUM({a})"));
                }
                // attrSemi, attrIf, attrGoto, attrBaxcel, attrSpace: no text effect
            }
            0x1C => {
                stack.push(error_text(u8_at(d, 0)?));
                used = 1;
            }
            0x1D => {
                stack.push(if u8_at(d, 0)? != 0 { "TRUE" } else { "FALSE" }.into());
                used = 1;
            }
            0x1E => {
                stack.push(u16_at(d, 0)?.to_string());
                used = 2;
            }
            0x1F => {
                stack.push(fmt_number(f64_at(d, 0)?));
                used = 8;
            }
            0x20..=0x7F => match ptg & 0x1F {
                // PtgArray
                0x00 => {
                    used = 7;
                    stack.push(array_constant(&mut extra)?);
                }
                // PtgFunc
                0x01 => {
                    let iftab = u16_at(d, 0)?;
                    used = 2;
                    let (name, argc) =
                        ftab(iftab).ok_or_else(|| malformed(format!("function {iftab}")))?;
                    if argc < 0 {
                        return Err(malformed(format!("{name} needs PtgFuncVar")));
                    }
                    let args = pop_n(&mut stack, argc as usize)?;
                    stack.push(format!("{name}({})", args.join(",")));
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
                        ftab(iftab)
                            .map(|f| f.0.to_owned())
                            .ok_or_else(|| malformed(format!("function {iftab}")))?
                    };
                    stack.push(format!("{name}({})", args.join(",")));
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
                    stack.push(name);
                }
                // PtgRef
                0x04 => {
                    stack.push(cell_ref(u16_at(d, 0)?, u16_at(d, 2)?));
                    used = 4;
                }
                // PtgArea
                0x05 => {
                    stack.push(area_ref(
                        u16_at(d, 0)?,
                        u16_at(d, 2)?,
                        u16_at(d, 4)?,
                        u16_at(d, 6)?,
                    ));
                    used = 8;
                }
                // PtgMemArea: its PtgExtraMem sits in rgcb.
                0x06 => {
                    used = 6;
                    let n = extra.u16()? as usize;
                    extra.skip(n * 8)?;
                }
                // PtgMemErr, PtgMemNoMem
                0x07 | 0x08 => used = 6,
                // PtgMemFunc
                0x09 => used = 2,
                // PtgRefErr / PtgAreaErr
                0x0A => {
                    stack.push("#REF!".into());
                    used = 4;
                }
                0x0B => {
                    stack.push("#REF!".into());
                    used = 8;
                }
                // PtgRefN
                0x0C => {
                    let (r, c) = rel_n(u16_at(d, 0)?, u16_at(d, 2)?, base);
                    stack.push(cell_ref(r, c));
                    used = 4;
                }
                // PtgAreaN
                0x0D => {
                    let (r1, c1) = rel_n(u16_at(d, 0)?, u16_at(d, 4)?, base);
                    let (r2, c2) = rel_n(u16_at(d, 2)?, u16_at(d, 6)?, base);
                    stack.push(area_ref(r1, r2, c1, c2));
                    used = 8;
                }
                // PtgNameX (external / add-in name)
                0x19 => {
                    stack.push("#NAME?".into());
                    used = 6;
                }
                // PtgRef3d
                0x1A => {
                    let ixti = u16_at(d, 0)?;
                    let r = cell_ref(u16_at(d, 2)?, u16_at(d, 4)?);
                    stack.push(match ctx.sheet_prefix(ixti) {
                        Some(p) => format!("{p}{r}"),
                        None => "#REF!".into(),
                    });
                    used = 6;
                }
                // PtgArea3d
                0x1B => {
                    let ixti = u16_at(d, 0)?;
                    let a = area_ref(
                        u16_at(d, 2)?,
                        u16_at(d, 4)?,
                        u16_at(d, 6)?,
                        u16_at(d, 8)?,
                    );
                    stack.push(match ctx.sheet_prefix(ixti) {
                        Some(p) => format!("{p}{a}"),
                        None => "#REF!".into(),
                    });
                    used = 10;
                }
                // PtgRefErr3d / PtgAreaErr3d
                0x1C => {
                    stack.push("#REF!".into());
                    used = 6;
                }
                0x1D => {
                    stack.push("#REF!".into());
                    used = 10;
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

/// Read a little-endian i16 (re-exported for the record parser).
pub fn _i16(b: &[u8], off: usize) -> Result<i16> {
    i16_at(b, off)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(rgce: &[u8]) -> String {
        formula_to_text(rgce, &[], &FormulaCtx::default(), (0, 0)).unwrap()
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
        assert_eq!(t(&[0x25, 0, 0, 9, 0, 0, 0xC0, 0, 0xC0, 0x19, 0x10, 0, 0]), "SUM(A1:A10)");
    }

    #[test]
    fn whole_columns_and_errors() {
        assert_eq!(t(&[0x25, 0, 0, 0xFF, 0xFF, 0, 0xC0, 1, 0xC0]), "A:B");
        assert_eq!(t(&[0x1C, 0x07]), "#DIV/0!");
        assert!(formula_to_text(&[0x03], &[], &FormulaCtx::default(), (0, 0)).is_err());
        assert!(formula_to_text(&[0x24, 0], &[], &FormulaCtx::default(), (0, 0)).is_err());
    }

    #[test]
    fn three_d_and_names() {
        let ctx = FormulaCtx {
            sheet_names: vec!["Sheet1".into(), "Bảng 2".into()],
            xti: vec![(0, 1, 1), (0, 0, 1)],
            supbook_self: vec![true],
            names: vec!["Thuế".into()],
        };
        let f = |v: &[u8]| formula_to_text(v, &[], &ctx, (0, 0)).unwrap();
        assert_eq!(f(&[0x3A, 0, 0, 4, 0, 2, 0xC0]), "'Bảng 2'!C5");
        assert_eq!(f(&[0x3A, 1, 0, 0, 0, 0, 0x00]), "'Sheet1:Bảng 2'!$A$1");
        assert_eq!(f(&[0x23, 1, 0, 0, 0]), "Thuế");
    }

    #[test]
    fn shared_relative() {
        // PtgRefN with row offset -1 (0xFFFF) and col offset +1, base (5, 2) → D5
        let v = [0x2C, 0xFF, 0xFF, 0x01, 0xC0];
        assert_eq!(
            formula_to_text(&v, &[], &FormulaCtx::default(), (5, 2)).unwrap(),
            "D5"
        );
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
}
