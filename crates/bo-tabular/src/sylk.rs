//! SYLK — the "symbolic link" spreadsheet interchange format (`.slk`).
//!
//! A SYLK file is text, one record per line. A record is a record type followed
//! by fields; every field starts with `;` and a one-letter field type, and a
//! literal semicolon inside a field is doubled (`;;`). Written from the publicly
//! documented record descriptions of the format; the records understood here:
//!
//! | record | fields |
//! |---|---|
//! | `ID` | first record of every file (`;P` program) |
//! | `P`  | `;P` a number format (picture), numbered from 0 in file order; `;F` / `;E` a font (`;M` size in 1/20 pt, `;S` style `B` bold, `I` italic) |
//! | `F`  | format: `;X` `;Y` the cell, `;P` picture index, `;F` format / digits / alignment (`L` `C` `R`), `;S` style (`D` bold, `I` italic, `M`n font index), `;W` c1 c2 n column widths in characters, `;M` row height in 1/20 pt with `;R` row, `;R` / `;C` default format of a row / column |
//! | `B`  | bounds `;Y` rows `;X` columns (written; not relied on when reading) |
//! | `O`  | options: `;V4` the 1904 date system |
//! | `NN` | a defined name: `;N` name, `;E` expression |
//! | `C`  | a cell: `;X` `;Y`, `;K` value, `;E` expression, `;S` with `;R` `;C` an expression shared from another cell, `;M` a matrix expression |
//! | `E`  | end of file |
//!
//! `;X` / `;Y` are 1-based and stay in force for the following records.
//! Expressions use R1C1 references (`R[-1]C`, `R1C1`); they are converted to and
//! from the A1 references of the model. Text values are quoted with `"`; a line
//! break inside a text is the three characters ESC, space, `:`.
//!
//! Files are read as UTF-8 when valid, Windows-1252 otherwise, and written as
//! Windows-1252 when every character fits, UTF-8 otherwise.

use crate::text::{chars_to_width, lines, number_text, width_to_chars};
use crate::{Result, TabularError};
use bo_formula::{format_with, parse_with, ParseOptions, RefStyle};
use bo_model::sheet::{
    Cell, CellAlignment, CellError, CellValue, CellXf, ColInfo, DefinedName, Formula, FormulaKind,
    Workbook, Worksheet, MAX_COLS, MAX_ROWS,
};
use bo_sheet::numfmt::Locale;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

/// The line break inside a text value.
const LINE_BREAK: &str = "\u{1b} :";
/// Longest expression converted (longer ones keep only their value).
const MAX_EXPR: usize = 8192;

/// The bytes start with a SYLK `ID` record.
#[must_use]
pub fn is_sylk(bytes: &[u8]) -> bool {
    bytes
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(bytes)
        .starts_with(b"ID;")
}

/// Fields of a record: split at single `;`, `;;` being a literal semicolon.
fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ';' {
            if chars.peek() == Some(&';') {
                chars.next();
                if let Some(last) = out.last_mut() {
                    last.push(';');
                }
            } else {
                out.push(String::new());
            }
        } else if let Some(last) = out.last_mut() {
            last.push(c);
        }
    }
    out
}

/// A field as (type letter, value).
fn split_field(f: &str) -> Option<(char, &str)> {
    let mut chars = f.chars();
    let t = chars.next()?;
    Some((t, chars.as_str()))
}

/// R1C1 expression → A1 formula text seen from the cell (`row`, `col`).
fn to_a1(expr: &str, row: u32, col: u32) -> Option<String> {
    let text = expr.trim();
    let text = text.strip_prefix('=').unwrap_or(text);
    if text.is_empty() || text.len() > MAX_EXPR {
        return None;
    }
    let opts = ParseOptions {
        style: RefStyle::R1C1,
        origin_row: row,
        origin_col: col,
    };
    let parsed = parse_with(text, &opts).ok()?;
    Some(format_with(&parsed, &ParseOptions::default()))
}

/// A1 formula text of the cell (`row`, `col`) → R1C1 expression.
fn to_r1c1(formula: &str, row: u32, col: u32) -> Option<String> {
    if formula.is_empty() || formula.len() > MAX_EXPR {
        return None;
    }
    let parsed = parse_with(formula, &ParseOptions::default()).ok()?;
    let opts = ParseOptions {
        style: RefStyle::R1C1,
        origin_row: row,
        origin_col: col,
    };
    Some(format_with(&parsed, &opts))
}

/// The value of a `;K` field.
fn parse_value(v: &str) -> CellValue {
    if let Some(inner) = v.strip_prefix('"') {
        let inner = inner.strip_suffix('"').unwrap_or(inner);
        if inner.is_empty() {
            return CellValue::Empty;
        }
        return CellValue::Text(inner.replace("\"\"", "\"").replace(LINE_BREAK, "\n"));
    }
    match v {
        "" => CellValue::Empty,
        "TRUE" => CellValue::Bool(true),
        "FALSE" => CellValue::Bool(false),
        _ if v.starts_with('#') => CellValue::Error(CellError::parse(v)),
        _ => match v.trim().parse::<f64>() {
            Ok(n) if n.is_finite() => CellValue::Number(n),
            _ => CellValue::Text(v.to_owned()),
        },
    }
}

/// Formatting gathered from `F` records before it becomes a cell format.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
struct RawFmt {
    /// Index of the `P;P` picture.
    picture: Option<usize>,
    bold: bool,
    italic: bool,
    /// Index of the `P;F` / `P;E` font.
    font: Option<usize>,
    /// `L`, `C` or `R`.
    align: Option<char>,
}

impl RawFmt {
    fn is_default(&self) -> bool {
        *self == RawFmt::default()
    }

    /// The fields of one `F` record laid over this format.
    fn merge(&mut self, other: &RawFmt) {
        if other.picture.is_some() {
            self.picture = other.picture;
        }
        if other.font.is_some() {
            self.font = other.font;
        }
        if other.align.is_some() {
            self.align = other.align;
        }
        self.bold |= other.bold;
        self.italic |= other.italic;
    }
}

#[derive(Debug, Clone, Default)]
struct RawFont {
    name: String,
    size: Option<f64>,
    bold: bool,
    italic: bool,
}

/// What a column range carries.
#[derive(Debug, Clone, Default, PartialEq)]
struct ColAttr {
    width: Option<f64>,
    fmt: Option<RawFmt>,
}

/// Non-overlapping column ranges keyed by their first column (zero-based,
/// inclusive ends).
#[derive(Default)]
struct ColMap(BTreeMap<u32, (u32, ColAttr)>);

impl ColMap {
    /// Make `at` the start of a range if a range crosses it.
    fn split_at(&mut self, at: u32) {
        let crossing = self
            .0
            .range(..at)
            .next_back()
            .filter(|(_, (end, _))| *end >= at)
            .map(|(s, _)| *s);
        if let Some(start) = crossing {
            if let Some((end, attr)) = self.0.remove(&start) {
                self.0.insert(start, (at - 1, attr.clone()));
                self.0.insert(at, (end, attr));
            }
        }
    }

    /// Apply `f` to the columns `min..=max`, creating ranges where none exist.
    fn apply(&mut self, min: u32, max: u32, f: impl Fn(&mut ColAttr)) {
        self.split_at(min);
        if max < u32::MAX {
            self.split_at(max + 1);
        }
        let existing: Vec<(u32, u32)> = self
            .0
            .range(min..=max)
            .map(|(s, (e, _))| (*s, *e))
            .collect();
        let mut next = min;
        for (s, e) in existing {
            if s > next {
                self.0.insert(next, (s - 1, ColAttr::default()));
            }
            next = e.saturating_add(1);
        }
        if next <= max {
            self.0.insert(next, (max, ColAttr::default()));
        }
        for (_, (_, attr)) in self.0.range_mut(min..=max) {
            f(attr);
        }
    }
}

/// One-based coordinate of an `;X` / `;Y` / `;R` / `;C` field → zero-based index.
fn coord(v: &str, limit: u32, what: &str) -> Result<Option<u32>> {
    match v.trim().parse::<u32>() {
        Ok(0) | Err(_) => Ok(None),
        Ok(n) if n > limit => Err(TabularError::Limit(format!("{what} {n}"))),
        Ok(n) => Ok(Some(n - 1)),
    }
}

#[derive(Default)]
struct Reader {
    ws: Worksheet,
    wb_names: Vec<DefinedName>,
    date1904: bool,
    row: u32,
    col: u32,
    pictures: Vec<String>,
    fonts: Vec<RawFont>,
    cell_fmts: BTreeMap<(u32, u32), RawFmt>,
    row_fmts: BTreeMap<u32, RawFmt>,
    cols: ColMap,
    /// R1C1 text of each formula cell, for shared expressions.
    exprs: HashMap<(u32, u32), String>,
}

impl Reader {
    fn position(&mut self, t: char, v: &str) -> Result<bool> {
        match t {
            'X' => {
                if let Some(c) = coord(v, MAX_COLS, "column")? {
                    self.col = c;
                }
                Ok(true)
            }
            'Y' => {
                if let Some(r) = coord(v, MAX_ROWS, "row")? {
                    self.row = r;
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// `P`: a number format or a font.
    fn picture(&mut self, rec: &[String]) {
        let mut font: Option<RawFont> = None;
        for f in rec.iter().skip(1) {
            let Some((t, v)) = split_field(f) else {
                continue;
            };
            match (t, font.as_mut()) {
                ('P', None) => {
                    self.pictures.push(v.to_owned());
                    return;
                }
                ('F' | 'E', None) => {
                    font = Some(RawFont {
                        name: v.to_owned(),
                        ..RawFont::default()
                    });
                }
                ('M', Some(font)) => {
                    font.size = v
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|s| (20.0..=8180.0).contains(s))
                        .map(|s| s / 20.0);
                }
                ('S', Some(font)) => {
                    font.bold = v.contains('B');
                    font.italic = v.contains('I');
                }
                _ => {}
            }
        }
        if let Some(font) = font {
            self.fonts.push(font);
        }
    }

    /// `F`: cell, row and column formatting.
    fn format(&mut self, rec: &[String]) -> Result<()> {
        let mut fmt = RawFmt::default();
        let (mut row_of, mut col_of) = (None, None);
        let (mut placed, mut defaults) = (false, false);
        let mut height = None;
        for f in rec.iter().skip(1) {
            let Some((t, v)) = split_field(f) else {
                continue;
            };
            if self.position(t, v)? {
                placed = true;
                continue;
            }
            match t {
                'P' => fmt.picture = v.trim().parse().ok(),
                // Format character, digits, alignment character.
                'F' => {
                    fmt.align = v
                        .chars()
                        .skip(1)
                        .find(|c| !c.is_ascii_digit())
                        .filter(|c| matches!(c, 'L' | 'C' | 'R'));
                }
                'S' => {
                    let mut chars = v.chars().peekable();
                    while let Some(c) = chars.next() {
                        match c {
                            'D' => fmt.bold = true,
                            'I' => fmt.italic = true,
                            'M' => {
                                let mut n = String::new();
                                while let Some(d) = chars.peek().filter(|d| d.is_ascii_digit()) {
                                    n.push(*d);
                                    chars.next();
                                }
                                fmt.font = n.parse().ok();
                            }
                            _ => {}
                        }
                    }
                }
                'W' => {
                    let mut p = v.split_whitespace();
                    let c1 = p.next().and_then(|x| x.parse::<u32>().ok());
                    let c2 = p.next().and_then(|x| x.parse::<u32>().ok());
                    let w = p.next().and_then(|x| x.parse::<f64>().ok());
                    if let (Some(c1), Some(c2), Some(w)) = (c1, c2, w) {
                        if c1 >= 1 && c1 <= c2 && c1 <= MAX_COLS && (0.0..=255.0).contains(&w) {
                            let width = chars_to_width(w);
                            self.cols.apply(c1 - 1, c2.min(MAX_COLS) - 1, |a| {
                                a.width = Some(width);
                            });
                        }
                    }
                    return Ok(());
                }
                'M' => {
                    height = v
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|h| (0.0..=8180.0).contains(h))
                        .map(|h| h / 20.0);
                }
                'R' => row_of = coord(v, MAX_ROWS, "row")?,
                'C' => col_of = coord(v, MAX_COLS, "column")?,
                // Defaults of the whole sheet (`;D`): not a cell format.
                'D' => defaults = true,
                _ => {}
            }
        }
        if let Some(r) = row_of {
            let info = self.ws.rows.entry(r).or_default();
            if let Some(h) = height {
                info.height = Some(h);
                info.custom_height = true;
            }
            if !fmt.is_default() {
                self.row_fmts.entry(r).or_default().merge(&fmt);
            }
        } else if let Some(c) = col_of {
            if !fmt.is_default() {
                self.cols.apply(c, c, |a| {
                    a.fmt.get_or_insert_with(RawFmt::default).merge(&fmt);
                });
            }
        } else if placed && !defaults && !fmt.is_default() {
            self.cell_fmts
                .entry((self.row, self.col))
                .or_default()
                .merge(&fmt);
        }
        Ok(())
    }

    /// `C`: a cell value and its expression.
    fn cell(&mut self, rec: &[String]) -> Result<()> {
        let mut value = None;
        let mut expr: Option<String> = None;
        let mut shared = false;
        let (mut from_row, mut from_col) = (None, None);
        for f in rec.iter().skip(1) {
            let Some((t, v)) = split_field(f) else {
                continue;
            };
            if self.position(t, v)? {
                continue;
            }
            match t {
                'K' => value = Some(parse_value(v)),
                'E' | 'M' => expr = Some(v.to_owned()),
                'S' => shared = true,
                'R' => from_row = coord(v, MAX_ROWS, "row")?,
                'C' => from_col = coord(v, MAX_COLS, "column")?,
                _ => {}
            }
        }
        let at = (self.row, self.col);
        if shared && expr.is_none() {
            // The expression of another cell, relative references and all.
            if let (Some(r), Some(c)) = (from_row, from_col) {
                expr = self.exprs.get(&(r, c)).cloned();
            }
        }
        let formula = expr.as_deref().and_then(|e| to_a1(e, at.0, at.1));
        if let (Some(e), Some(_)) = (expr, &formula) {
            self.exprs.insert(at, e);
        }
        let value = value.filter(|v| *v != CellValue::Empty);
        if value.is_none() && formula.is_none() {
            return Ok(());
        }
        self.ws.cells.insert(
            at,
            Cell {
                value: value.unwrap_or_default(),
                formula: formula.as_deref().map(Formula::normal),
                ..Cell::default()
            },
        );
        Ok(())
    }

    /// `NN`: a defined name.
    fn name(&mut self, rec: &[String]) {
        let (mut name, mut expr) = (None, None);
        for f in rec.iter().skip(1) {
            match split_field(f) {
                Some(('N', v)) => name = Some(v.to_owned()),
                Some(('E', v)) => expr = Some(v),
                _ => {}
            }
        }
        let (Some(name), Some(expr)) = (name, expr) else {
            return;
        };
        let valid = name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '\\')
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '\\' | '?'))
            && name.chars().count() <= 255;
        if !valid
            || self
                .wb_names
                .iter()
                .any(|n| n.name.eq_ignore_ascii_case(&name))
        {
            return;
        }
        if let Some(formula) = to_a1(expr, 0, 0) {
            self.wb_names.push(DefinedName {
                name,
                formula,
                ..DefinedName::default()
            });
        }
    }

    fn finish(mut self) -> Workbook {
        let mut wb = Workbook::new();
        wb.date1904 = self.date1904;
        let mut cache: HashMap<RawFmt, u32> = HashMap::new();
        let (pictures, fonts) = (&self.pictures, &self.fonts);
        let mut xf_of = |fmt: &RawFmt, wb: &mut Workbook| -> u32 {
            if let Some(i) = cache.get(fmt) {
                return *i;
            }
            let i = build_xf(fmt, pictures, fonts, wb);
            cache.insert(fmt.clone(), i);
            i
        };
        for (at, fmt) in &self.cell_fmts {
            let style = xf_of(fmt, &mut wb);
            if style != 0 {
                self.ws.cells.entry(*at).or_default().style = style;
            }
        }
        for (r, fmt) in &self.row_fmts {
            let style = xf_of(fmt, &mut wb);
            if style != 0 {
                self.ws.rows.entry(*r).or_default().style = Some(style);
            }
        }
        for (min, (max, attr)) in &self.cols.0 {
            let style = attr
                .fmt
                .as_ref()
                .map(|f| xf_of(f, &mut wb))
                .filter(|s| *s != 0);
            if attr.width.is_none() && style.is_none() {
                continue;
            }
            self.ws.cols.push(ColInfo {
                min: *min,
                max: *max,
                width: attr.width,
                custom_width: attr.width.is_some(),
                style,
                ..ColInfo::default()
            });
        }
        self.ws.sheet_id = 1;
        wb.sheets = vec![self.ws];
        wb.defined_names = self.wb_names;
        wb
    }
}

/// The cell format (index into `cellXfs`) of gathered formatting.
fn build_xf(fmt: &RawFmt, pictures: &[String], fonts: &[RawFont], wb: &mut Workbook) -> u32 {
    let st = &mut wb.styles;
    let num_fmt_id = match fmt.picture.and_then(|i| pictures.get(i)) {
        Some(code) if !code.is_empty() && !code.eq_ignore_ascii_case("General") => {
            st.add_num_fmt(code)
        }
        _ => 0,
    };
    let base = st.fonts.first().cloned().unwrap_or_default();
    let raw = fmt.font.and_then(|i| fonts.get(i));
    let mut font = base.clone();
    if let Some(raw) = raw {
        if !raw.name.is_empty() {
            font.name = Some(raw.name.clone());
            font.scheme = None;
        }
        if raw.size.is_some() {
            font.size = raw.size;
        }
        font.bold = raw.bold;
        font.italic = raw.italic;
    }
    font.bold |= fmt.bold;
    font.italic |= fmt.italic;
    let font_id = if font == base { 0 } else { st.add_font(font) };
    let alignment = fmt.align.map(|a| CellAlignment {
        horizontal: Some(
            match a {
                'L' => "left",
                'C' => "center",
                _ => "right",
            }
            .to_owned(),
        ),
        ..CellAlignment::default()
    });
    if num_fmt_id == 0 && font_id == 0 && alignment.is_none() {
        return 0;
    }
    st.add_xf(CellXf {
        num_fmt_id,
        font_id,
        xf_id: Some(0),
        apply_number_format: (num_fmt_id != 0).then_some(true),
        apply_font: (font_id != 0).then_some(true),
        apply_alignment: alignment.is_some().then_some(true),
        alignment,
        ..CellXf::default()
    })
}

/// Read a SYLK file into a workbook with one sheet.
pub fn read_sylk(bytes: &[u8]) -> Result<Workbook> {
    if !is_sylk(bytes) {
        return Err(TabularError::NotSylk);
    }
    let text = bo_encoding::codepage::decode_text(bytes);
    let mut r = Reader {
        ws: Worksheet::new("Sheet1"),
        ..Reader::default()
    };
    for line in lines(&text) {
        if line.is_empty() {
            continue;
        }
        let rec = fields(line);
        match rec.first().map(String::as_str) {
            Some("C") => r.cell(&rec)?,
            Some("F") => r.format(&rec)?,
            Some("P") => r.picture(&rec),
            Some("NN") => r.name(&rec),
            Some("O") => {
                // `;V` value 4: serial dates count from 1904.
                if rec.iter().skip(1).any(|f| f == "V4") {
                    r.date1904 = true;
                }
            }
            Some("E") => break,
            // ID, B (bounds are taken from the cells), W (window) and others.
            _ => {}
        }
    }
    Ok(r.finish())
}

/// A text value as a `;K` field.
fn quote(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\"\""),
            ';' => out.push_str(";;"),
            '\n' => out.push_str(LINE_BREAK),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A field value with its semicolons doubled; control characters are dropped.
fn escape(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .replace(';', ";;")
}

/// What the writer says about a cell format.
#[derive(Default, Clone)]
struct OutFmt {
    picture: usize,
    bold: bool,
    italic: bool,
    align: Option<char>,
}

impl OutFmt {
    fn is_default(&self) -> bool {
        self.picture == 0 && !self.bold && !self.italic && self.align.is_none()
    }

    fn fields(&self, out: &mut String) {
        let _ = write!(out, ";P{}", self.picture);
        if let Some(a) = self.align {
            let _ = write!(out, ";FG0{a}");
        }
        if self.bold || self.italic {
            out.push_str(";S");
            if self.bold {
                out.push('D');
            }
            if self.italic {
                out.push('I');
            }
        }
    }
}

/// Serialise worksheet `ws` of `wb` as SYLK: values, formulas, number formats,
/// bold / italic, horizontal alignment, column widths, row heights and the
/// workbook's defined names.
#[must_use]
pub fn write_sylk(wb: &Workbook, ws: &Worksheet) -> Vec<u8> {
    let locale = Locale::EN_US;
    let mut pictures: Vec<String> = vec!["General".into()];
    let mut by_xf: HashMap<u32, OutFmt> = HashMap::new();
    let mut fmt_of = |style: u32, pictures: &mut Vec<String>| -> OutFmt {
        if let Some(f) = by_xf.get(&style) {
            return f.clone();
        }
        let mut f = OutFmt::default();
        if let Some(xf) = wb.styles.cell_xfs.get(style as usize) {
            let code = bo_sheet::numfmt::format_code(&wb.styles, xf.num_fmt_id, &locale);
            f.picture = pictures.iter().position(|p| *p == code).unwrap_or_else(|| {
                pictures.push(code);
                pictures.len() - 1
            });
            if let Some(font) = wb.styles.fonts.get(xf.font_id as usize) {
                f.bold = font.bold;
                f.italic = font.italic;
            }
            f.align = xf
                .alignment
                .as_ref()
                .and_then(|a| a.horizontal.as_deref())
                .and_then(|h| match h {
                    "left" => Some('L'),
                    "center" => Some('C'),
                    "right" => Some('R'),
                    _ => None,
                });
        }
        by_xf.insert(style, f.clone());
        f
    };
    // Shared-formula groups: the text lives on the master cell (§18.3.1.40 `f`).
    // In R1C1 notation every cell of a group has the master's expression.
    let mut masters: HashMap<u32, String> = HashMap::new();
    for ((row, col), cell) in &ws.cells {
        if let Some(f) = &cell.formula {
            if let (FormulaKind::Shared { index, .. }, false) = (&f.kind, f.text.is_empty()) {
                if let Some(e) = to_r1c1(&f.text, *row, *col) {
                    masters.entry(*index).or_insert(e);
                }
            }
        }
    }

    let mut body = String::new();
    for ((row, col), cell) in &ws.cells {
        let fmt = fmt_of(cell.style, &mut pictures);
        if !fmt.is_default() {
            body.push('F');
            fmt.fields(&mut body);
            let _ = write!(body, ";Y{};X{}\r\n", row + 1, col + 1);
        }
        let expr = cell.formula.as_ref().and_then(|f| match &f.kind {
            FormulaKind::Shared { index, .. } if f.text.is_empty() => masters.get(index).cloned(),
            _ => to_r1c1(&f.text, *row, *col),
        });
        if cell.value == CellValue::Empty && expr.is_none() {
            continue;
        }
        let _ = write!(body, "C;Y{};X{};K", row + 1, col + 1);
        match &cell.value {
            CellValue::Empty => body.push_str("\"\""),
            CellValue::Number(n) if n.is_finite() => body.push_str(&number_text(*n)),
            CellValue::Number(_) => body.push_str("#NUM!"),
            CellValue::Bool(b) => body.push_str(if *b { "TRUE" } else { "FALSE" }),
            CellValue::Error(e) => body.push_str(&escape(e.as_str())),
            CellValue::Date(d) => quote(d, &mut body),
            CellValue::Text(_) | CellValue::RichText(_) => {
                quote(&cell.value.as_text().unwrap_or_default(), &mut body);
            }
        }
        if let Some(e) = expr {
            let _ = write!(body, ";E{}", escape(&e));
        }
        body.push_str("\r\n");
    }

    // Column widths and formats, row heights and formats. Formats of columns
    // beyond the used cells are not written.
    let last_col = ws.used_range().map(|r| r.last.col);
    let mut layout = String::new();
    for c in &ws.cols {
        if c.min > c.max {
            continue;
        }
        if let Some(w) = c.width {
            let _ = write!(
                layout,
                "F;W{} {} {}\r\n",
                c.min + 1,
                c.max + 1,
                width_to_chars(w)
            );
        }
        let fmt = c.style.map(|s| fmt_of(s, &mut pictures));
        if let (Some(fmt), Some(last)) = (fmt.filter(|f| !f.is_default()), last_col) {
            for col in c.min..=c.max.min(last) {
                layout.push('F');
                fmt.fields(&mut layout);
                let _ = write!(layout, ";C{}\r\n", col + 1);
            }
        }
    }
    for (r, info) in &ws.rows {
        let height = info.height.filter(|_| info.custom_height);
        let fmt = info
            .style
            .map(|s| fmt_of(s, &mut pictures))
            .filter(|f| !f.is_default());
        if height.is_none() && fmt.is_none() {
            continue;
        }
        layout.push('F');
        if let Some(fmt) = fmt {
            fmt.fields(&mut layout);
        }
        if let Some(h) = height {
            let _ = write!(layout, ";M{}", (h * 20.0).round());
        }
        let _ = write!(layout, ";R{}\r\n", r + 1);
    }

    let mut out = String::from("ID;PBLUEOFFICE;N;E\r\n");
    for p in &pictures {
        let _ = write!(out, "P;P{}\r\n", escape(p));
    }
    if wb.date1904 {
        out.push_str("O;V4\r\n");
    }
    out.push_str(&layout);
    if let Some(range) = ws.used_range() {
        let _ = write!(out, "B;Y{};X{}\r\n", range.last.row + 1, range.last.col + 1);
    }
    for n in &wb.defined_names {
        if n.local_sheet.is_some() || n.name.starts_with("_xlnm.") {
            continue;
        }
        if let Some(e) = to_r1c1(&n.formula, 0, 0) {
            let _ = write!(out, "NN;N{};E{}\r\n", escape(&n.name), escape(&e));
        }
    }
    out.push_str(&body);
    out.push_str("E\r\n");
    bo_encoding::codepage::encode_text(&out)
}
