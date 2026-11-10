//! Reader for the XML Spreadsheet 2003 format ("SpreadsheetML 2003", namespace
//! `urn:schemas-microsoft-com:office:spreadsheet`): a workbook as one XML file.
//!
//! Source: the public "XML Spreadsheet Reference" (elements `Workbook`, `Styles`,
//! `Style`, `Names`, `NamedRange`, `Worksheet`, `Table`, `Column`, `Row`, `Cell`,
//! `Data`, `WorksheetOptions` and their `ss:` attributes). Read here:
//!
//! * cells with their type (`ss:Type`: `Number`, `String`, `DateTime`, `Boolean`,
//!   `Error`), sparse addressing (`ss:Index`), merged cells (`ss:MergeAcross`,
//!   `ss:MergeDown`) and hyperlinks (`ss:HRef`);
//! * formulas — written in R1C1 notation, converted to A1 for the model
//!   (ECMA-376 Part 1, §18.17.2.3 describes both notations);
//! * styles with inheritance (`ss:Parent`): font, interior (fill), borders, number
//!   format and alignment;
//! * column widths and row heights (in points), hidden rows / columns and sheets;
//! * named ranges of the workbook and of a sheet; the 1904 date system.
//!
//! Untrusted input: element depth is limited by the XML reader, and the numbers of
//! sheets, cells, styles and names are capped.

use crate::xml::{for_children, lname, read_text, root};
use crate::{Result, XlsxError};
use bo_formula::{format_with, parse_with, ParseOptions, RefStyle};
use bo_model::sheet::{
    Border, BorderEdge, Cell, CellAlignment, CellError, CellRange, CellRef, CellValue, CellXf,
    ColInfo, DefinedName, Fill, Font, Formula, FormulaKind, Hyperlink, PatternFill, RowInfo,
    SheetColor, SheetState, Workbook, Worksheet,
};
use bo_opc::xml::{attr, XmlReader};
use quick_xml::events::BytesStart;
use std::borrow::Cow;
use std::collections::HashMap;

/// Namespace of the format's elements and `ss:` attributes.
pub const NS_SPREADSHEET: &str = "urn:schemas-microsoft-com:office:spreadsheet";

/// Limits of a worksheet (ECMA-376 Part 1, §18.3.1.73 row, §18.3.1.4 c).
const MAX_ROWS: u32 = 1_048_576;
const MAX_COLS: u32 = 16_384;
/// Caps on what one file may declare.
const MAX_SHEETS: usize = 1024;
const MAX_CELLS: usize = 8_000_000;
const MAX_STYLES: usize = 60_000;
const MAX_NAMES: usize = 20_000;
const MAX_MERGES: usize = 200_000;

/// XML text as UTF-8: UTF-16 with a byte order mark is transcoded.
fn decode(bytes: &[u8]) -> Cow<'_, str> {
    let utf16 = |be: bool, b: &[u8]| -> String {
        let units = b.chunks_exact(2).map(|c| {
            if be {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            }
        });
        char::decode_utf16(units)
            .map(|r| r.unwrap_or('\u{fffd}'))
            .collect()
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        Cow::Owned(utf16(false, rest))
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        Cow::Owned(utf16(true, rest))
    } else {
        String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes))
    }
}

/// The bytes are an XML Spreadsheet 2003 file: an XML document whose root element
/// is `Workbook` in the spreadsheet namespace (looked for in the first 4 KiB).
pub fn is_xml_spreadsheet(bytes: &[u8]) -> bool {
    if bytes.starts_with(b"PK") {
        return false;
    }
    let head = decode(&bytes[..bytes.len().min(4096) & !1]);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    if !head.starts_with('<') || !head.contains(NS_SPREADSHEET) {
        return false;
    }
    let mut rest = head;
    loop {
        let Some(at) = rest.find('<') else {
            return false;
        };
        rest = &rest[at + 1..];
        match rest.chars().next() {
            Some('?' | '!') => {}
            Some(_) => {
                let name = rest
                    .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                    .next()
                    .unwrap_or("");
                return name.rsplit(':').next() == Some("Workbook");
            }
            None => return false,
        }
    }
}

fn bad(r: &XmlReader<'_>, message: &str) -> XlsxError {
    XlsxError::Opc(r.err(message))
}

/// `1` / `0` attribute.
fn flag(e: &BytesStart<'_>, name: &[u8]) -> Option<bool> {
    attr(e, name).map(|v| v.trim() == "1" || v.trim().eq_ignore_ascii_case("true"))
}

fn num<T: std::str::FromStr>(e: &BytesStart<'_>, name: &[u8]) -> Option<T> {
    attr(e, name).and_then(|v| v.trim().parse().ok())
}

/// `#RRGGBB` → colour; `Automatic` and anything else → none.
fn color(v: &str) -> Option<SheetColor> {
    let hex = v.trim().strip_prefix('#')?;
    (hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| SheetColor::rgb(&hex.to_ascii_uppercase()))
}

/// One `Style` as declared: only what the element itself sets.
#[derive(Debug, Clone, Default)]
#[allow(clippy::option_option)]
struct RawStyle {
    parent: Option<String>,
    font_name: Option<String>,
    font_size: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<String>,
    strike: Option<bool>,
    font_color: Option<Option<SheetColor>>,
    vert_align: Option<String>,
    /// `Interior`: pattern, colour, pattern colour.
    pattern: Option<String>,
    fill_color: Option<SheetColor>,
    pattern_color: Option<SheetColor>,
    /// `Borders` was present: it replaces the inherited borders.
    borders: Option<Border>,
    num_format: Option<String>,
    horizontal: Option<String>,
    vertical: Option<String>,
    wrap: Option<bool>,
    shrink: Option<bool>,
    indent: Option<u32>,
    rotate: Option<i32>,
    vertical_text: Option<bool>,
    reading_order: Option<String>,
}

impl RawStyle {
    /// `self` laid over `base`.
    fn over(&self, base: &RawStyle) -> RawStyle {
        macro_rules! pick {
            ($($f:ident),*) => { RawStyle { parent: None, $($f: self.$f.clone().or_else(|| base.$f.clone())),* } };
        }
        pick!(
            font_name,
            font_size,
            bold,
            italic,
            underline,
            strike,
            font_color,
            vert_align,
            pattern,
            fill_color,
            pattern_color,
            borders,
            num_format,
            horizontal,
            vertical,
            wrap,
            shrink,
            indent,
            rotate,
            vertical_text,
            reading_order
        )
    }
}

/// `Border` line: `ss:LineStyle` and `ss:Weight` → `ST_BorderStyle` (§18.18.3).
fn border_style(line: &str, weight: u32) -> Option<&'static str> {
    Some(match (line, weight) {
        ("None", _) => return None,
        ("Double", _) => "double",
        ("Dot", _) => "dotted",
        ("Dash", 0 | 1) => "dashed",
        ("Dash", _) => "mediumDashed",
        ("DashDot", 0 | 1) => "dashDot",
        ("DashDot", _) => "mediumDashDot",
        ("DashDotDot", 0 | 1) => "dashDotDot",
        ("DashDotDot", _) => "mediumDashDotDot",
        ("SlantDashDot", _) => "slantDashDot",
        (_, 0) => "hair",
        (_, 1) => "thin",
        (_, 2) => "medium",
        _ => "thick",
    })
}

/// Format code of a named `ss:Format` (`None`: the General format).
fn format_code(v: &str) -> Option<String> {
    let code = match v {
        "" | "General" | "General Number" => return None,
        "General Date" => "m/d/yyyy h:mm",
        "Long Date" => "[$-F800]dddd, mmmm dd, yyyy",
        "Medium Date" => "d-mmm-yy",
        "Short Date" => "m/d/yyyy",
        "Long Time" => "h:mm:ss AM/PM",
        "Medium Time" => "h:mm AM/PM",
        "Short Time" => "h:mm",
        "Currency" => "\"$\"#,##0.00_);[Red]\\(\"$\"#,##0.00\\)",
        "Euro Currency" => "[$\u{20ac}-2] #,##0.00_);[Red]\\([$\u{20ac}-2] #,##0.00\\)",
        "Fixed" => "0.00",
        "Standard" => "#,##0.00",
        "Percent" => "0.00%",
        "Scientific" => "0.00E+00",
        "Yes/No" => "\"Yes\";\"Yes\";\"No\"",
        "True/False" => "\"True\";\"True\";\"False\"",
        "On/Off" => "\"On\";\"On\";\"Off\"",
        other => other,
    };
    (code.len() <= 255).then(|| code.to_owned())
}

fn parse_style<'a>(
    r: &mut XmlReader<'a>,
    e: &BytesStart<'a>,
    empty: bool,
) -> Result<(String, RawStyle)> {
    let id = attr(e, b"ID").unwrap_or_default();
    let mut s = RawStyle {
        parent: attr(e, b"Parent"),
        ..RawStyle::default()
    };
    if empty {
        return Ok((id, s));
    }
    for_children(r, |r, c, c_empty| {
        match lname(c).as_str() {
            "Font" => {
                s.font_name = attr(c, b"FontName").filter(|n| !n.is_empty() && n.len() <= 64);
                s.font_size =
                    num::<f64>(c, b"Size").filter(|v| v.is_finite() && (1.0..=409.0).contains(v));
                s.bold = flag(c, b"Bold");
                s.italic = flag(c, b"Italic");
                s.strike = flag(c, b"StrikeThrough");
                s.underline = attr(c, b"Underline");
                s.font_color = attr(c, b"Color").map(|v| color(&v));
                s.vert_align = attr(c, b"VerticalAlign");
            }
            "Interior" => {
                s.pattern = attr(c, b"Pattern").or_else(|| Some("None".into()));
                s.fill_color = attr(c, b"Color").and_then(|v| color(&v));
                s.pattern_color = attr(c, b"PatternColor").and_then(|v| color(&v));
            }
            "NumberFormat" => s.num_format = Some(attr(c, b"Format").unwrap_or_default()),
            "Alignment" => {
                s.horizontal = attr(c, b"Horizontal");
                s.vertical = attr(c, b"Vertical");
                s.wrap = flag(c, b"WrapText");
                s.shrink = flag(c, b"ShrinkToFit");
                s.indent = num::<u32>(c, b"Indent").map(|i| i.min(250));
                s.rotate = num::<i32>(c, b"Rotate").filter(|a| (-90..=90).contains(a));
                s.vertical_text = flag(c, b"VerticalText");
                s.reading_order = attr(c, b"ReadingOrder");
            }
            "Borders" => {
                let mut b = Border::default();
                if !c_empty {
                    for_children(r, |_, edge, _| {
                        if lname(edge) != "Border" {
                            return Ok(());
                        }
                        let line = attr(edge, b"LineStyle").unwrap_or_else(|| "Continuous".into());
                        let weight = num::<u32>(edge, b"Weight").unwrap_or(1);
                        let Some(style) = border_style(&line, weight) else {
                            return Ok(());
                        };
                        let e = BorderEdge {
                            style: Some(style.into()),
                            color: attr(edge, b"Color").and_then(|v| color(&v)),
                        };
                        match attr(edge, b"Position").as_deref() {
                            Some("Left") => b.left = e,
                            Some("Right") => b.right = e,
                            Some("Top") => b.top = e,
                            Some("Bottom") => b.bottom = e,
                            Some("DiagonalLeft") => {
                                b.diagonal = e;
                                b.diagonal_down = true;
                            }
                            Some("DiagonalRight") => {
                                b.diagonal = e;
                                b.diagonal_up = true;
                            }
                            _ => {}
                        }
                        Ok(())
                    })?;
                }
                s.borders = Some(b);
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok((id, s))
}

/// Builds `cellXfs` from the declared styles, on demand.
struct Styles {
    raw: HashMap<String, RawStyle>,
    xf: HashMap<String, u32>,
}

impl Styles {
    /// The style with everything it inherits (`ss:Parent` chain, then `Default`).
    fn resolved(&self, id: &str) -> RawStyle {
        let mut chain: Vec<&RawStyle> = Vec::new();
        let mut cur = self.raw.get(id);
        while let Some(s) = cur {
            chain.push(s);
            if chain.len() > 16 {
                break;
            }
            cur = s.parent.as_deref().and_then(|p| self.raw.get(p));
        }
        if id != "Default" {
            if let Some(d) = self.raw.get("Default") {
                chain.push(d);
            }
        }
        let mut out = RawStyle::default();
        for s in chain.iter().rev() {
            out = s.over(&out);
        }
        out
    }

    /// Index of the cell format of style `id` (0 for unknown ids).
    fn xf_of(&mut self, id: &str, wb: &mut Workbook) -> u32 {
        if let Some(&i) = self.xf.get(id) {
            return i;
        }
        if !self.raw.contains_key(id) || wb.styles.cell_xfs.len() >= MAX_STYLES {
            return 0;
        }
        let s = self.resolved(id);
        let xf = build_xf(&s, wb);
        let i = wb.styles.add_xf(xf);
        self.xf.insert(id.to_owned(), i);
        i
    }
}

fn build_font(s: &RawStyle, base: &Font) -> Font {
    let mut f = Font {
        scheme: None,
        ..base.clone()
    };
    if let Some(n) = &s.font_name {
        f.name = Some(n.clone());
    }
    if let Some(sz) = s.font_size {
        f.size = Some(sz);
    }
    f.bold = s.bold.unwrap_or(false);
    f.italic = s.italic.unwrap_or(false);
    f.strike = s.strike.unwrap_or(false);
    f.underline = match s.underline.as_deref() {
        Some("Single") => Some("single".into()),
        Some("Double") => Some("double".into()),
        Some("SingleAccounting") => Some("singleAccounting".into()),
        Some("DoubleAccounting") => Some("doubleAccounting".into()),
        _ => None,
    };
    f.color = s.font_color.clone().flatten();
    f.vert_align = match s.vert_align.as_deref() {
        Some("Superscript") => Some("superscript".into()),
        Some("Subscript") => Some("subscript".into()),
        _ => None,
    };
    f
}

fn build_xf(s: &RawStyle, wb: &mut Workbook) -> CellXf {
    let st = &mut wb.styles;
    let base = st.fonts.first().cloned().unwrap_or_default();
    let font = build_font(s, &base);
    // Unchanged from the workbook font: keep font 0 (with its theme scheme).
    let same = Font {
        scheme: base.scheme.clone(),
        ..font.clone()
    } == base;
    let font_id = if same { 0 } else { st.add_font(font) };
    let fill_id = match s.pattern.as_deref() {
        None | Some("None") => 0,
        Some(p) => {
            let pattern = match p {
                "Gray75" => "darkGray",
                "Gray50" => "mediumGray",
                "Gray25" => "lightGray",
                "Gray125" => "gray125",
                "Gray0625" => "gray0625",
                "HorzStripe" => "darkHorizontal",
                "VertStripe" => "darkVertical",
                "ReverseDiagStripe" => "darkDown",
                "DiagStripe" => "darkUp",
                "DiagCross" => "darkGrid",
                "ThickDiagCross" => "darkTrellis",
                "ThinHorzStripe" => "lightHorizontal",
                "ThinVertStripe" => "lightVertical",
                "ThinReverseDiagStripe" => "lightDown",
                "ThinDiagStripe" => "lightUp",
                "ThinHorzCross" => "lightGrid",
                "ThinDiagCross" => "lightTrellis",
                _ => "solid",
            };
            match (&s.fill_color, pattern) {
                (Some(c), "solid") => st.add_fill(Fill::Pattern(PatternFill {
                    pattern: Some("solid".into()),
                    fg: Some(c.clone()),
                    bg: Some(SheetColor {
                        indexed: Some(64),
                        ..SheetColor::default()
                    }),
                })),
                (None, "solid") => 0,
                // A patterned fill: the pattern colour over the interior colour.
                (c, pat) => st.add_fill(Fill::Pattern(PatternFill {
                    pattern: Some(pat.into()),
                    fg: s.pattern_color.clone(),
                    bg: c.clone(),
                })),
            }
        }
    };
    let border_id = s.borders.clone().map_or(0, |b| st.add_border(b));
    let num_fmt_id = s
        .num_format
        .as_deref()
        .and_then(format_code)
        .map_or(0, |code| st.add_num_fmt(&code));
    let a = CellAlignment {
        horizontal: match s.horizontal.as_deref() {
            Some("Left") => Some("left"),
            Some("Center") => Some("center"),
            Some("Right") => Some("right"),
            Some("Fill") => Some("fill"),
            Some("Justify") => Some("justify"),
            Some("CenterAcrossSelection") => Some("centerContinuous"),
            Some("Distributed" | "JustifyDistributed") => Some("distributed"),
            _ => None,
        }
        .map(str::to_owned),
        vertical: match s.vertical.as_deref() {
            Some("Top") => Some("top"),
            Some("Center") => Some("center"),
            Some("Justify") => Some("justify"),
            Some("Distributed" | "JustifyDistributed") => Some("distributed"),
            // Bottom is the default of a cell format (§18.8.1).
            _ => None,
        }
        .map(str::to_owned),
        wrap_text: s.wrap.unwrap_or(false),
        shrink_to_fit: s.shrink.unwrap_or(false),
        indent: s.indent.filter(|i| *i > 0),
        // `ss:Rotate` −90…90 degrees → `textRotation` 0…180 (§18.8.1); stacked text is 255.
        text_rotation: if s.vertical_text == Some(true) {
            Some(255)
        } else {
            s.rotate.filter(|r| *r != 0).map(|r| {
                if r >= 0 {
                    r.unsigned_abs()
                } else {
                    90 + r.unsigned_abs()
                }
            })
        },
        reading_order: match s.reading_order.as_deref() {
            Some("LeftToRight") => Some(1),
            Some("RightToLeft") => Some(2),
            _ => None,
        },
        ..CellAlignment::default()
    };
    let has_align = a != CellAlignment::default();
    CellXf {
        num_fmt_id,
        font_id,
        fill_id,
        border_id,
        xf_id: Some(0),
        apply_number_format: (num_fmt_id != 0).then_some(true),
        apply_font: (font_id != 0).then_some(true),
        apply_fill: (fill_id != 0).then_some(true),
        apply_border: (border_id != 0).then_some(true),
        apply_alignment: has_align.then_some(true),
        alignment: has_align.then_some(a),
        ..CellXf::default()
    }
}

/// Days from 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Serial date of an ISO 8601 `DateTime` value (`2024-03-15T13:30:00.000`) in the
/// 1900 date system — with its leap-year quirk, serial 60 being 29 February 1900
/// (ECMA-376 Part 1, §18.17.4.1) — or the 1904 date system.
fn serial_date(v: &str, date1904: bool) -> Option<f64> {
    let v = v.trim().trim_end_matches('Z');
    let (date, time) = v.split_once('T').unwrap_or((v, ""));
    let mut d = date.splitn(3, '-');
    let y: i64 = d.next()?.parse().ok()?;
    let m: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    let mut frac = 0.0;
    if !time.is_empty() {
        let mut t = time.splitn(3, ':');
        let h: f64 = t.next()?.parse().ok()?;
        let min: f64 = t.next().map_or(Some(0.0), |x| x.parse().ok())?;
        let sec: f64 = t.next().map_or(Some(0.0), |x| x.parse().ok())?;
        if !(0.0..24.0).contains(&h) || !(0.0..60.0).contains(&min) || !(0.0..61.0).contains(&sec) {
            return None;
        }
        frac = (h * 3600.0 + min * 60.0 + sec) / 86_400.0;
    }
    let days = days_from_civil(y, m, day);
    let serial = if date1904 {
        days - days_from_civil(1904, 1, 1)
    } else {
        // Day 0 is 1899-12-31; from 1 March 1900 on, the phantom leap day adds one.
        let n = days - days_from_civil(1899, 12, 31);
        if n >= 60 {
            n + 1
        } else {
            n
        }
    };
    (serial >= 0).then_some(serial as f64 + frac)
}

/// R1C1 formula text → A1 text, as seen from the cell at (`row`, `col`). `None`
/// when the formula does not parse.
pub fn r1c1_to_a1(formula: &str, row: u32, col: u32) -> Option<String> {
    let text = formula.trim();
    let text = text.strip_prefix('=').unwrap_or(text);
    if text.is_empty() || text.len() > 8192 {
        return None;
    }
    let expr = parse_with(
        text,
        &ParseOptions {
            style: RefStyle::R1C1,
            origin_row: row,
            origin_col: col,
        },
    )
    .ok()?;
    Some(format_with(&expr, &ParseOptions::default()))
}

/// What a `Table` gathers besides its cells.
#[derive(Default)]
struct SheetAcc {
    cells: usize,
}

struct Reader {
    wb: Workbook,
    styles: Styles,
    cells: usize,
}

impl Reader {
    fn names(&mut self, r: &mut XmlReader<'_>, local: Option<u32>) -> Result<()> {
        for_children(r, |_, e, _| {
            if lname(e) != "NamedRange" || self.wb.defined_names.len() >= MAX_NAMES {
                return Ok(());
            }
            let (Some(name), Some(refers)) = (attr(e, b"Name"), attr(e, b"RefersTo")) else {
                return Ok(());
            };
            // Print settings and the filter range are not names of the model.
            if name.is_empty()
                || matches!(
                    name.as_str(),
                    "Print_Area" | "Print_Titles" | "_FilterDatabase"
                )
            {
                return Ok(());
            }
            if let Some(formula) = r1c1_to_a1(&refers, 0, 0) {
                self.wb.defined_names.push(DefinedName {
                    name,
                    formula,
                    local_sheet: local,
                    hidden: flag(e, b"Hidden").unwrap_or(false),
                    attrs: Vec::new(),
                });
            }
            Ok(())
        })
    }

    #[allow(clippy::too_many_lines)]
    fn table<'a>(
        &mut self,
        r: &mut XmlReader<'a>,
        t: &BytesStart<'a>,
        ws: &mut Worksheet,
    ) -> Result<()> {
        let table_style = attr(t, b"StyleID");
        if let Some(w) = num::<f64>(t, b"DefaultColumnWidth").filter(|w| w.is_finite() && *w > 0.0)
        {
            ws.format.default_col_width = Some(points_to_chars(w));
        }
        if let Some(h) = num::<f64>(t, b"DefaultRowHeight")
            .filter(|h| h.is_finite() && (0.0..=409.5).contains(h))
        {
            ws.format.default_row_height = Some(h);
            ws.format.custom_height = true;
        }
        let table_xf = table_style
            .as_deref()
            .map(|id| self.styles.xf_of(id, &mut self.wb));
        // Style of each column that has one (zero-based column → format).
        let mut col_xf: HashMap<u32, u32> = HashMap::new();
        let mut next_col = 0u32;
        let mut next_row = 0u32;
        let mut acc = SheetAcc::default();
        for_children(r, |r, e, empty| {
            match lname(e).as_str() {
                "Column" => {
                    if let Some(i) = num::<u32>(e, b"Index").filter(|i| *i >= 1) {
                        next_col = (i - 1).max(next_col);
                    }
                    let span = num::<u32>(e, b"Span").unwrap_or(0).min(MAX_COLS);
                    if next_col >= MAX_COLS {
                        return Ok(());
                    }
                    let last = (next_col + span).min(MAX_COLS - 1);
                    let width = num::<f64>(e, b"Width")
                        .filter(|w| w.is_finite() && (0.0..=2000.0).contains(w));
                    let hidden = flag(e, b"Hidden").unwrap_or(false);
                    let style = attr(e, b"StyleID").map(|id| self.styles.xf_of(&id, &mut self.wb));
                    if (width.is_some() || hidden || style.is_some()) && ws.cols.len() < 4096 {
                        ws.cols.push(ColInfo {
                            min: next_col,
                            max: last,
                            width: width.map(points_to_chars),
                            custom_width: width.is_some(),
                            hidden,
                            style,
                            ..ColInfo::default()
                        });
                        if let Some(s) = style {
                            for c in next_col..=last.min(next_col + 1023) {
                                col_xf.insert(c, s);
                            }
                        }
                    }
                    next_col = last + 1;
                }
                "Row" => {
                    if let Some(i) = num::<u32>(e, b"Index").filter(|i| *i >= 1) {
                        next_row = (i - 1).max(next_row);
                    }
                    let row = next_row;
                    let span = num::<u32>(e, b"Span").unwrap_or(0).min(MAX_ROWS);
                    next_row = row.saturating_add(span).saturating_add(1);
                    if row >= MAX_ROWS {
                        return Ok(());
                    }
                    let height = num::<f64>(e, b"Height")
                        .filter(|h| h.is_finite() && (0.0..=409.5).contains(h));
                    let hidden = flag(e, b"Hidden").unwrap_or(false);
                    let row_xf = attr(e, b"StyleID").map(|id| self.styles.xf_of(&id, &mut self.wb));
                    if height.is_some() || hidden || row_xf.is_some() {
                        ws.rows.insert(
                            row,
                            RowInfo {
                                height,
                                custom_height: height.is_some(),
                                hidden,
                                style: row_xf,
                                ..RowInfo::default()
                            },
                        );
                    }
                    if !empty {
                        self.row(r, ws, row, row_xf, table_xf, &col_xf, &mut acc)?;
                    }
                }
                _ => {}
            }
            Ok(())
        })?;
        self.cells += acc.cells;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self,
        r: &mut XmlReader<'_>,
        ws: &mut Worksheet,
        row: u32,
        row_xf: Option<u32>,
        table_xf: Option<u32>,
        col_xf: &HashMap<u32, u32>,
        acc: &mut SheetAcc,
    ) -> Result<()> {
        let mut next_col = 0u32;
        for_children(r, |r, e, empty| {
            if lname(e) != "Cell" {
                return Ok(());
            }
            if let Some(i) = num::<u32>(e, b"Index").filter(|i| *i >= 1) {
                next_col = (i - 1).max(next_col);
            }
            let col = next_col;
            let across = num::<u32>(e, b"MergeAcross").unwrap_or(0).min(MAX_COLS);
            let down = num::<u32>(e, b"MergeDown").unwrap_or(0).min(MAX_ROWS);
            next_col = col.saturating_add(across).saturating_add(1);
            if col >= MAX_COLS || self.cells + acc.cells >= MAX_CELLS {
                return Ok(());
            }
            acc.cells += 1;
            if (across > 0 || down > 0) && ws.merges.len() < MAX_MERGES {
                ws.merges.push(CellRange::new(
                    CellRef { row, col },
                    CellRef {
                        row: (row + down).min(MAX_ROWS - 1),
                        col: (col + across).min(MAX_COLS - 1),
                    },
                ));
            }
            let style = attr(e, b"StyleID")
                .map(|id| self.styles.xf_of(&id, &mut self.wb))
                .or(row_xf)
                .or_else(|| col_xf.get(&col).copied())
                .or(table_xf)
                .unwrap_or(0);
            let formula_text = attr(e, b"Formula");
            let array_range = attr(e, b"ArrayRange");
            if let Some(href) = attr(e, b"HRef").filter(|h| !h.is_empty() && h.len() <= 2083) {
                if ws.hyperlinks.len() < 65_530 {
                    let (target, location) = match href.strip_prefix('#') {
                        Some(loc) => (None, Some(loc.to_owned())),
                        None => (Some(href), None),
                    };
                    ws.hyperlinks.push(Hyperlink {
                        range: CellRange::single(CellRef { row, col }),
                        target,
                        location,
                        display: None,
                        tooltip: attr(e, b"HRefScreenTip"),
                    });
                }
            }
            let mut value = CellValue::Empty;
            if !empty {
                for_children(r, |r, d, d_empty| {
                    if lname(d) != "Data" {
                        return Ok(());
                    }
                    let ty = attr(d, b"Type").unwrap_or_else(|| "String".into());
                    let text = if d_empty {
                        String::new()
                    } else {
                        read_text(r)?
                    };
                    value = match ty.as_str() {
                        "Number" => text
                            .trim()
                            .parse::<f64>()
                            .ok()
                            .filter(|n| n.is_finite())
                            .map_or(CellValue::Empty, CellValue::Number),
                        "Boolean" => CellValue::Bool(matches!(text.trim(), "1" | "true" | "TRUE")),
                        "DateTime" => serial_date(&text, self.wb.date1904)
                            .map_or_else(|| CellValue::Text(text.clone()), CellValue::Number),
                        "Error" => CellValue::Error(CellError::parse(text.trim())),
                        _ => CellValue::Text(text.chars().take(32_767).collect()),
                    };
                    Ok(())
                })?;
            }
            let formula = formula_text
                .as_deref()
                .and_then(|f| r1c1_to_a1(f, row, col))
                .map(|text| {
                    let kind = array_range
                        .as_deref()
                        .and_then(|a| r1c1_to_a1(a, row, col))
                        .filter(|a| CellRange::parse(a).is_some())
                        .map_or(FormulaKind::Normal, |range| FormulaKind::Array { range });
                    Formula {
                        text,
                        kind,
                        ..Formula::default()
                    }
                });
            if value != CellValue::Empty || formula.is_some() || style != 0 {
                ws.set_cell(
                    row,
                    col,
                    Cell {
                        value,
                        formula,
                        style,
                        ..Cell::default()
                    },
                );
            }
            Ok(())
        })
    }

    fn worksheet<'a>(&mut self, r: &mut XmlReader<'a>, e: &BytesStart<'a>) -> Result<()> {
        let index = self.wb.sheets.len();
        let name = sheet_name(
            &attr(e, b"Name").unwrap_or_default(),
            index,
            &self.wb.sheets,
        );
        let mut ws = Worksheet::new(&name);
        ws.sheet_id = index as u32 + 1;
        for_children(r, |r, c, empty| {
            match lname(c).as_str() {
                "Table" if !empty => self.table(r, c, &mut ws)?,
                "Names" if !empty => self.names(r, Some(index as u32))?,
                "WorksheetOptions" if !empty => {
                    for_children(r, |r, o, o_empty| {
                        if lname(o) == "Visible" && !o_empty {
                            ws.state = match read_text(r)?.trim() {
                                "SheetHidden" => SheetState::Hidden,
                                "SheetVeryHidden" => SheetState::VeryHidden,
                                _ => SheetState::Visible,
                            };
                        }
                        Ok(())
                    })?;
                }
                _ => {}
            }
            Ok(())
        })?;
        self.wb.sheets.push(ws);
        Ok(())
    }
}

/// Column width in points → characters of the default font (§18.3.1.13: a width of
/// `n` characters is `7n + 5` pixels; 1 px = 0.75 pt).
fn points_to_chars(pt: f64) -> f64 {
    let px = pt / 0.75;
    (((px - 5.0) / 7.0).max(0.0) * 100.0).round() / 100.0
}

/// A valid, unique sheet name (§18.2.19).
fn sheet_name(raw: &str, index: usize, sheets: &[Worksheet]) -> String {
    let clean: String = raw
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\') && !c.is_control())
        .take(31)
        .collect();
    let clean = clean.trim().trim_matches('\'').to_owned();
    let base = if clean.is_empty() {
        format!("Sheet{}", index + 1)
    } else {
        clean
    };
    let taken = |n: &str| sheets.iter().any(|s| s.name.eq_ignore_ascii_case(n));
    if !taken(&base) {
        return base;
    }
    (2..10_000)
        .map(|n| {
            let suffix = format!(" ({n})");
            let keep: String = base.chars().take(31 - suffix.len()).collect();
            format!("{keep}{suffix}")
        })
        .find(|n| !taken(n))
        .unwrap_or(base)
}

/// Read an XML Spreadsheet 2003 file into the sheet model.
pub fn read_xml_spreadsheet(bytes: &[u8]) -> Result<Workbook> {
    let text = decode(bytes);
    let mut r = XmlReader::new(text.as_bytes(), "XML Spreadsheet");
    let (e, empty) = root(&mut r)?;
    if lname(&e) != "Workbook" {
        return Err(bad(&r, "root element is not Workbook"));
    }
    let mut reader = Reader {
        wb: Workbook::new(),
        styles: Styles {
            raw: HashMap::new(),
            xf: HashMap::new(),
        },
        cells: 0,
    };
    reader.wb.sheets.clear();
    // Names can refer to sheets declared later: they are read after the sheets, so
    // the `Names` of the workbook are kept as a second pass over the same element.
    if !empty {
        for_children(&mut r, |r, c, c_empty| {
            match lname(c).as_str() {
                "ExcelWorkbook" if !c_empty => {
                    for_children(r, |_, o, _| {
                        if lname(o) == "Date1904" {
                            reader.wb.date1904 = true;
                        }
                        Ok(())
                    })?;
                }
                "Styles" if !c_empty => {
                    for_children(r, |r, s, s_empty| {
                        if lname(s) == "Style" && reader.styles.raw.len() < MAX_STYLES {
                            let (id, style) = parse_style(r, s, s_empty)?;
                            reader.styles.raw.entry(id).or_insert(style);
                        }
                        Ok(())
                    })?;
                    // The Default style is the workbook's base format (cellXfs[0]):
                    // its font becomes font 0.
                    if reader.styles.raw.contains_key("Default") {
                        let d = reader.styles.resolved("Default");
                        let base = reader.wb.styles.fonts.first().cloned().unwrap_or_default();
                        let font = build_font(&d, &base);
                        if let Some(f0) = reader.wb.styles.fonts.first_mut() {
                            *f0 = font;
                        }
                        let mut xf = build_xf(&d, &mut reader.wb);
                        xf.font_id = 0;
                        xf.apply_font = None;
                        if let Some(x0) = reader.wb.styles.cell_xfs.first_mut() {
                            *x0 = xf;
                        }
                        reader.styles.xf.insert("Default".into(), 0);
                    }
                }
                "Names" if !c_empty => reader.names(r, None)?,
                "Worksheet" if !c_empty && reader.wb.sheets.len() < MAX_SHEETS => {
                    reader.worksheet(r, c)?;
                }
                _ => {}
            }
            Ok(())
        })?;
    }
    if reader.wb.sheets.is_empty() {
        let mut ws = Worksheet::new("Sheet1");
        ws.sheet_id = 1;
        reader.wb.sheets.push(ws);
    }
    Ok(reader.wb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_formulas() {
        assert_eq!(serial_date("1900-01-01T00:00:00.000", false), Some(1.0));
        assert_eq!(serial_date("1900-02-28", false), Some(59.0));
        assert_eq!(serial_date("1900-03-01T00:00:00.000", false), Some(61.0));
        assert_eq!(
            serial_date("2024-03-15T12:00:00.000", false),
            Some(45_366.5)
        );
        assert_eq!(serial_date("1899-12-31T06:00:00.000", false), Some(0.25));
        assert_eq!(serial_date("1904-01-02T00:00:00", true), Some(1.0));
        for bad in [
            "",
            "x",
            "2024-13-01",
            "2024-01-01T25:00:00",
            "0000-01-01",
            "1800-01-01",
        ] {
            assert_eq!(serial_date(bad, false), None, "{bad}");
        }
        assert_eq!(
            r1c1_to_a1("=SUM(R[-2]C:R[-1]C)", 2, 1).as_deref(),
            Some("SUM(B1:B2)")
        );
        assert_eq!(
            r1c1_to_a1("=R1C1*RC[-1]+R[1]C4", 1, 2).as_deref(),
            Some("$A$1*B2+$D3")
        );
        assert_eq!(
            r1c1_to_a1("=Sheet2!R2C2:R3C3", 0, 0).as_deref(),
            Some("Sheet2!$B$2:$C$3")
        );
        assert_eq!(
            r1c1_to_a1("=IF(RC[1]>0,\"R1C1\",C[1])", 0, 0).as_deref(),
            Some("IF(B1>0,\"R1C1\",B:B)")
        );
        assert_eq!(r1c1_to_a1("=((", 0, 0), None);
        assert_eq!(points_to_chars(48.0), 8.43);
    }

    #[test]
    fn sniffing() {
        let ok = format!("<?xml version=\"1.0\"?>\n<?mso-application progid=\"Excel.Sheet\"?>\n<Workbook xmlns=\"{NS_SPREADSHEET}\"/>");
        assert!(is_xml_spreadsheet(ok.as_bytes()));
        assert!(is_xml_spreadsheet(
            format!("<ss:Workbook xmlns:ss=\"{NS_SPREADSHEET}\">").as_bytes()
        ));
        let mut utf16 = vec![0xFF, 0xFE];
        utf16.extend(ok.encode_utf16().flat_map(u16::to_le_bytes));
        assert!(is_xml_spreadsheet(&utf16));
        assert!(read_xml_spreadsheet(&utf16).is_ok());
        assert!(!is_xml_spreadsheet(b"<Workbook/>"));
        assert!(!is_xml_spreadsheet(
            format!("<workbook xmlns=\"{NS_SPREADSHEET}\"/>").as_bytes()
        ));
        assert!(!is_xml_spreadsheet(b"PK\x03\x04"));
        assert!(read_xml_spreadsheet(b"<a/>").is_err());
        assert!(read_xml_spreadsheet(b"").is_err());
    }
}
