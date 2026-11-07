//! `.ods` spreadsheet cells (ODF 1.3 Part 3, §9.1 Tables, §19.385 office:value-type).

use crate::dom::{self, escape, Element, Node};
use crate::ns::{OFFICE, TABLE, TEXT};
use crate::odt_write::XMLNS;
use crate::package::{meta_xml, OdfPackage};
use crate::{OdfError, Result, MIME_ODS};

/// Maximum rows / columns addressed (ODF has no hard limit; these match
/// common spreadsheet applications and keep repeat counts sane).
pub const MAX_ROWS: u32 = 1 << 20;
pub const MAX_COLS: u32 = 1 << 14;
/// Maximum number of value cells produced by expanding repeated cells/rows.
const MAX_CELLS: usize = 2_000_000;

/// A whole spreadsheet (intermediate type; mapping to the sheet model is done
/// by the BlueSheet integration layer).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpreadsheetDoc {
    pub sheets: Vec<Sheet>,
}

/// One `table:table`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sheet {
    pub name: String,
    /// Non-empty cells in row-major order.
    pub cells: Vec<Cell>,
}

/// A cell with a value (0-based row / column).
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub row: u32,
    pub col: u32,
    pub value: CellValue,
    /// `table:formula` verbatim, including the namespace prefix (e.g. `of:=SUM([.A1:.A3])`).
    pub formula: Option<String>,
}

/// Cell value by `office:value-type` (§19.385).
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Float(f64),
    Percentage(f64),
    Currency {
        value: f64,
        currency: Option<String>,
    },
    /// `office:date-value` (xsd:date or xsd:dateTime) verbatim.
    Date(String),
    /// `office:time-value` (xsd:duration) verbatim.
    Time(String),
    Boolean(bool),
    String(String),
}

/// Collects the plain text of a paragraph applying the white-space rules of
/// ODF 1.3 Part 3, §6.1.2.
struct TextCollector {
    out: String,
    last_space: bool,
}

impl TextCollector {
    fn walk(&mut self, el: &Element, depth: usize) {
        if depth > dom::MAX_DEPTH {
            return;
        }
        for n in &el.children {
            match n {
                Node::Text(t) => {
                    for c in t.chars() {
                        if matches!(c, ' ' | '\t' | '\n' | '\r') {
                            if !self.last_space {
                                self.out.push(' ');
                                self.last_space = true;
                            }
                        } else {
                            self.out.push(c);
                            self.last_space = false;
                        }
                    }
                }
                Node::Element(e) if e.ns == TEXT => match e.name.as_str() {
                    // §6.1.3 text:s
                    "s" => {
                        let c: usize = e
                            .attr(TEXT, "c")
                            .and_then(|c| c.parse().ok())
                            .unwrap_or(1)
                            .clamp(1, 10_000);
                        self.out.extend(std::iter::repeat_n(' ', c));
                        self.last_space = false;
                    }
                    "tab" => {
                        self.out.push('\t');
                        self.last_space = false;
                    }
                    "line-break" => {
                        self.out.push('\n');
                        self.last_space = true;
                    }
                    "note" | "soft-page-break" => {}
                    _ => self.walk(e, depth + 1),
                },
                Node::Element(e) if e.is(OFFICE, "annotation") => {}
                Node::Element(e) => self.walk(e, depth + 1),
            }
        }
    }
}

/// Plain text of one `text:p` / `text:h`.
pub(crate) fn paragraph_text(p: &Element) -> String {
    let mut c = TextCollector {
        out: String::new(),
        last_space: true,
    };
    c.walk(p, 0);
    if c.last_space && c.out.ends_with(' ') {
        c.out.pop();
    }
    c.out
}

/// Text of all `text:p`/`text:h` children joined by `\n`.
pub(crate) fn paragraphs_text(el: &Element) -> String {
    el.elements()
        .filter(|e| e.is(TEXT, "p") || e.is(TEXT, "h"))
        .map(paragraph_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_f64(s: Option<&str>) -> Option<f64> {
    s?.trim().parse().ok()
}

fn cell_value(c: &Element) -> Option<CellValue> {
    let vt = c.attr(OFFICE, "value-type")?;
    let value = || parse_f64(c.attr(OFFICE, "value"));
    Some(match vt {
        "float" => CellValue::Float(value()?),
        "percentage" => CellValue::Percentage(value()?),
        "currency" => CellValue::Currency {
            value: value()?,
            currency: c.attr(OFFICE, "currency").map(str::to_owned),
        },
        "date" => CellValue::Date(c.attr(OFFICE, "date-value")?.to_owned()),
        "time" => CellValue::Time(c.attr(OFFICE, "time-value")?.to_owned()),
        "boolean" => CellValue::Boolean(matches!(
            c.attr(OFFICE, "boolean-value"),
            Some("true" | "1")
        )),
        "string" => CellValue::String(
            c.attr(OFFICE, "string-value")
                .map_or_else(|| paragraphs_text(c), str::to_owned),
        ),
        _ => return None,
    })
}

fn repeat(e: &Element, attr: &str) -> u32 {
    e.attr(TABLE, attr)
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(1)
        .max(1)
}

struct TableReader {
    row: u32,
    cells: Vec<Cell>,
}

impl TableReader {
    fn rows(&mut self, el: &Element, depth: usize) -> Result<()> {
        if depth > dom::MAX_DEPTH {
            return Ok(());
        }
        for e in el.elements() {
            if e.ns != TABLE {
                continue;
            }
            match e.name.as_str() {
                "table-row" => self.row(e)?,
                "table-rows" | "table-row-group" | "table-header-rows" => {
                    self.rows(e, depth + 1)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn row(&mut self, r: &Element) -> Result<()> {
        let reps = repeat(r, "number-rows-repeated");
        // Parse once; expand only if the row has values.
        let mut col = 0u32;
        let mut row_cells = Vec::new();
        for c in r.elements() {
            if !(c.is(TABLE, "table-cell") || c.is(TABLE, "covered-table-cell")) {
                continue;
            }
            let creps = repeat(c, "number-columns-repeated");
            if let Some(v) = cell_value(c) {
                let formula = c.attr(TABLE, "formula").map(str::to_owned);
                for k in 0..creps {
                    let cc = col.saturating_add(k);
                    if cc >= MAX_COLS {
                        break;
                    }
                    row_cells.push((cc, v.clone(), formula.clone()));
                    if row_cells.len() > MAX_CELLS {
                        return Err(OdfError::LimitExceeded("too many cells".into()));
                    }
                }
            }
            col = col.saturating_add(creps);
        }
        if row_cells.is_empty() {
            self.row = self.row.saturating_add(reps);
            return Ok(());
        }
        for _ in 0..reps {
            if self.row >= MAX_ROWS {
                break;
            }
            if self.cells.len() + row_cells.len() > MAX_CELLS {
                return Err(OdfError::LimitExceeded("too many cells".into()));
            }
            for (c, v, f) in &row_cells {
                self.cells.push(Cell {
                    row: self.row,
                    col: *c,
                    value: v.clone(),
                    formula: f.clone(),
                });
            }
            if self.cells.len() > MAX_CELLS {
                return Err(OdfError::LimitExceeded("too many cells".into()));
            }
            self.row += 1;
        }
        Ok(())
    }
}

/// Read an `.ods` package.
pub fn read_ods(bytes: &[u8]) -> Result<SpreadsheetDoc> {
    let pkg = OdfPackage::read(bytes)?;
    pkg.expect_mimetype(MIME_ODS)?;
    let content = dom::parse(pkg.require("content.xml")?, "content.xml")?;
    let ss = content
        .child(OFFICE, "body")
        .and_then(|b| b.child(OFFICE, "spreadsheet"))
        .ok_or_else(|| OdfError::Missing("office:body/office:spreadsheet".into()))?;
    let mut doc = SpreadsheetDoc::default();
    for t in ss.children_named(TABLE, "table") {
        let mut r = TableReader {
            row: 0,
            cells: Vec::new(),
        };
        r.rows(t, 0)?;
        doc.sheets.push(Sheet {
            name: t.attr(TABLE, "name").unwrap_or_default().to_owned(),
            cells: r.cells,
        });
    }
    Ok(doc)
}

/// xsd:double lexical form.
fn fmt_double(v: f64) -> String {
    if v.is_nan() {
        "NaN".into()
    } else if v.is_infinite() {
        if v > 0.0 { "INF" } else { "-INF" }.into()
    } else {
        format!("{v}")
    }
}

fn write_cell(out: &mut String, c: &Cell) {
    out.push_str("<table:table-cell");
    if let Some(f) = &c.formula {
        out.push_str(&format!(r#" table:formula="{}""#, escape(f)));
    }
    let text = match &c.value {
        CellValue::Float(v) => {
            out.push_str(&format!(
                r#" office:value-type="float" office:value="{}""#,
                fmt_double(*v)
            ));
            fmt_double(*v)
        }
        CellValue::Percentage(v) => {
            out.push_str(&format!(
                r#" office:value-type="percentage" office:value="{}""#,
                fmt_double(*v)
            ));
            format!("{}%", fmt_double(v * 100.0))
        }
        CellValue::Currency { value, currency } => {
            out.push_str(&format!(
                r#" office:value-type="currency" office:value="{}""#,
                fmt_double(*value)
            ));
            if let Some(cur) = currency {
                out.push_str(&format!(r#" office:currency="{}""#, escape(cur)));
            }
            fmt_double(*value)
        }
        CellValue::Date(d) => {
            out.push_str(&format!(
                r#" office:value-type="date" office:date-value="{}""#,
                escape(d)
            ));
            d.clone()
        }
        CellValue::Time(t) => {
            out.push_str(&format!(
                r#" office:value-type="time" office:time-value="{}""#,
                escape(t)
            ));
            t.clone()
        }
        CellValue::Boolean(b) => {
            out.push_str(&format!(
                r#" office:value-type="boolean" office:boolean-value="{b}""#
            ));
            if *b { "TRUE" } else { "FALSE" }.into()
        }
        CellValue::String(s) => {
            out.push_str(r#" office:value-type="string""#);
            s.clone()
        }
    };
    out.push('>');
    for line in text.split('\n') {
        out.push_str("<text:p>");
        write_text(out, line);
        out.push_str("</text:p>");
    }
    out.push_str("</table:table-cell>");
}

/// Text with spaces/tabs encoded so that they survive white-space collapsing.
pub(crate) fn write_text(out: &mut String, s: &str) {
    let chars: Vec<char> = s.chars().collect();
    let mut ws = true;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' {
            let mut n = 1;
            while chars.get(i + n) == Some(&' ') {
                n += 1;
            }
            let at_end = i + n == chars.len();
            let literal = usize::from(!ws && !at_end);
            if literal == 1 {
                out.push(' ');
            }
            let rest = n - literal;
            if rest == 1 {
                out.push_str("<text:s/>");
            } else if rest > 1 {
                out.push_str(&format!(r#"<text:s text:c="{rest}"/>"#));
            }
            i += n;
            ws = true;
            continue;
        }
        if c == '\t' {
            out.push_str("<text:tab/>");
        } else {
            out.push_str(&escape(&c.to_string()));
        }
        ws = false;
        i += 1;
    }
}

/// Serialize as an `.ods` package.
pub fn write_ods(doc: &SpreadsheetDoc) -> Result<Vec<u8>> {
    let mut content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-content {XMLNS}><office:body><office:spreadsheet>"
    );
    for sheet in &doc.sheets {
        content.push_str(&format!(
            r#"<table:table table:name="{}">"#,
            escape(&sheet.name)
        ));
        let mut cells: Vec<&Cell> = sheet
            .cells
            .iter()
            .filter(|c| c.row < MAX_ROWS && c.col < MAX_COLS)
            .collect();
        cells.sort_by_key(|c| (c.row, c.col));
        cells.dedup_by_key(|c| (c.row, c.col));
        let max_col = cells.iter().map(|c| c.col).max().map_or(1, |m| m + 1);
        content.push_str(&format!(
            r#"<table:table-column table:number-columns-repeated="{max_col}"/>"#
        ));
        if cells.is_empty() {
            content.push_str("<table:table-row><table:table-cell/></table:table-row>");
        }
        let mut next_row = 0u32;
        let mut i = 0;
        while i < cells.len() {
            let row = cells[i].row;
            if row > next_row {
                content.push_str(&format!(
                    r#"<table:table-row table:number-rows-repeated="{}"><table:table-cell table:number-columns-repeated="{max_col}"/></table:table-row>"#,
                    row - next_row
                ));
            }
            content.push_str("<table:table-row>");
            let mut next_col = 0u32;
            while i < cells.len() && cells[i].row == row {
                let c = cells[i];
                if c.col > next_col {
                    let gap = c.col - next_col;
                    if gap == 1 {
                        content.push_str("<table:table-cell/>");
                    } else {
                        content.push_str(&format!(
                            r#"<table:table-cell table:number-columns-repeated="{gap}"/>"#
                        ));
                    }
                }
                write_cell(&mut content, c);
                next_col = c.col + 1;
                i += 1;
            }
            content.push_str("</table:table-row>");
            next_row = row + 1;
        }
        content.push_str("</table:table>");
    }
    content.push_str("</office:spreadsheet></office:body></office:document-content>");
    let styles = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-styles {XMLNS}><office:styles/></office:document-styles>"
    );
    OdfPackage::write(
        MIME_ODS,
        &[
            ("content.xml", "text/xml", content.into_bytes()),
            ("styles.xml", "text/xml", styles.into_bytes()),
            ("meta.xml", "text/xml", meta_xml()),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> SpreadsheetDoc {
        let cell = |row, col, value, formula: Option<&str>| Cell {
            row,
            col,
            value,
            formula: formula.map(str::to_owned),
        };
        SpreadsheetDoc {
            sheets: vec![
                Sheet {
                    name: "Bảng lương".into(),
                    cells: vec![
                        cell(0, 0, CellValue::String("Họ và tên".into()), None),
                        cell(0, 1, CellValue::String("Lương".into()), None),
                        cell(1, 0, CellValue::String("  Nguyễn  Văn A ".into()), None),
                        cell(1, 1, CellValue::Float(12_500_000.5), None),
                        cell(
                            2,
                            0,
                            CellValue::String("dòng 1\ndòng 2\tTab <&>".into()),
                            None,
                        ),
                        cell(2, 1, CellValue::Float(-0.1), None),
                        cell(
                            5,
                            3,
                            CellValue::Float(12_499_999.6),
                            Some("of:=SUM([.B2:.B3])"),
                        ),
                        cell(6, 0, CellValue::Percentage(0.125), None),
                        cell(
                            6,
                            2,
                            CellValue::Currency {
                                value: 1500.0,
                                currency: Some("VND".into()),
                            },
                            None,
                        ),
                        cell(7, 0, CellValue::Date("2026-11-05".into()), None),
                        cell(7, 1, CellValue::Time("PT13H45M00S".into()), None),
                        cell(7, 5, CellValue::Boolean(true), Some("of:=TRUE()")),
                    ],
                },
                Sheet {
                    name: "Trống".into(),
                    cells: vec![],
                },
            ],
        }
    }

    #[test]
    fn round_trip() {
        let doc = sample();
        let bytes = write_ods(&doc).unwrap();
        assert_eq!(read_ods(&bytes).unwrap(), doc);
    }

    #[test]
    fn repeated_cells_and_rows() {
        let xml = format!(
            r#"<office:document-content {XMLNS}><office:body><office:spreadsheet><table:table table:name="S">
            <table:table-row table:number-rows-repeated="3"><table:table-cell table:number-columns-repeated="2" office:value-type="float" office:value="7"/></table:table-row>
            <table:table-row table:number-rows-repeated="1048000"><table:table-cell table:number-columns-repeated="16384"/></table:table-row>
            <table:table-row><table:covered-table-cell/><table:table-cell office:value-type="string"><text:p>a<text:s text:c="2"/>b</text:p></table:table-cell></table:table-row>
            </table:table></office:spreadsheet></office:body></office:document-content>"#
        );
        let bytes =
            OdfPackage::write(MIME_ODS, &[("content.xml", "text/xml", xml.into_bytes())]).unwrap();
        let doc = read_ods(&bytes).unwrap();
        let cells = &doc.sheets[0].cells;
        assert_eq!(cells.len(), 7);
        assert_eq!((cells[5].row, cells[5].col), (2, 1));
        assert_eq!(cells[6].row, 1_048_003);
        assert_eq!(cells[6].col, 1);
        assert_eq!(cells[6].value, CellValue::String("a  b".into()));
    }
}
