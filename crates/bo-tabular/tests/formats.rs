//! SYLK, DIF, PRN and dBASE: hand-written files in the shape other applications
//! write them, with the values their specifications give, and round trips of the
//! writers.
#![allow(clippy::unwrap_used)]

use bo_model::sheet::{
    Cell, CellAlignment, CellError, CellValue, CellXf, ColInfo, DefinedName, Font, Formula,
    FormulaKind, RowInfo, Workbook, Worksheet,
};
use bo_sheet::numfmt::Locale;
use bo_tabular::{
    read_dbf, read_dif, read_prn, read_sylk, write_dif, write_prn, write_sylk, TabularError,
};

fn text(s: &str) -> CellValue {
    CellValue::Text(s.into())
}

fn value<'a>(wb: &'a Workbook, a1: &str) -> &'a CellValue {
    &wb.sheets[0]
        .cell_a1(a1)
        .unwrap_or_else(|| panic!("no cell {a1}"))
        .value
}

fn formula<'a>(wb: &'a Workbook, a1: &str) -> Option<&'a str> {
    wb.sheets[0]
        .cell_a1(a1)
        .and_then(|c| c.formula.as_ref())
        .map(|f| f.text.as_str())
}

fn num_fmt(wb: &Workbook, a1: &str) -> String {
    let c = wb.sheets[0].cell_a1(a1).unwrap();
    let id = wb.styles.xf(c.style).map_or(0, |x| x.num_fmt_id);
    bo_sheet::numfmt::format_code(&wb.styles, id, &Locale::EN_US)
}

fn font<'a>(wb: &'a Workbook, a1: &str) -> &'a Font {
    let c = wb.sheets[0].cell_a1(a1).unwrap();
    let xf = wb.styles.xf(c.style).unwrap();
    &wb.styles.fonts[xf.font_id as usize]
}

/// A file in the shape a spreadsheet application writes: pictures, fonts,
/// defaults, bounds, options, widths, a name, formats before their cells, `;X` /
/// `;Y` carried over from the record before.
const SYLK: &str = "ID;PWXL;N;E\r\n\
P;PGeneral\r\n\
P;P0\r\n\
P;P0.00\r\n\
P;P#,##0;;\\-#,##0\r\n\
P;FArial;M200\r\n\
P;FArial;M240;SB\r\n\
F;P0;DG0G8;M255\r\n\
B;Y6;X3;D0 0 5 2\r\n\
O;L;D;V0;K47;G100 0.001\r\n\
F;W1 1 12\r\n\
F;W3 3 20\r\n\
F;M600;R2\r\n\
NN;NTotal;ER4C2\r\n\
F;SM1;Y1;X1\r\n\
C;K\"Item\"\r\n\
F;SDI;X2\r\n\
C;K\"Qty\"\r\n\
C;Y2;X1;K\"Apple;; red\"\r\n\
C;X2;K3\r\n\
C;Y3;X1;K\"Say \"\"hi\"\"\"\r\n\
F;P2;FF2R;X2\r\n\
C;K4.5\r\n\
C;Y4;X1;KTRUE\r\n\
C;X2;K7.5;ESUM(R[-2]C:R[-1]C)\r\n\
F;P3;X3\r\n\
C;K-15000;ERC[-1]*-2000\r\n\
C;Y5;X2;K#DIV/0!;ER2C2/0\r\n\
C;Y6;X2;K12;S;R4;C2\r\n\
C;X3;K\"two\x1b :lines\"\r\n\
E\r\n";

#[test]
fn sylk_values_formulas_and_formats() {
    let wb = read_sylk(SYLK.as_bytes()).unwrap();
    assert_eq!(wb.sheets.len(), 1);
    assert!(!wb.date1904);
    assert_eq!(value(&wb, "A1"), &text("Item"));
    assert_eq!(value(&wb, "B1"), &text("Qty"));
    // `;;` is a semicolon, `""` a quotation mark.
    assert_eq!(value(&wb, "A2"), &text("Apple; red"));
    assert_eq!(value(&wb, "B2"), &CellValue::Number(3.0));
    assert_eq!(value(&wb, "A3"), &text("Say \"hi\""));
    assert_eq!(value(&wb, "B3"), &CellValue::Number(4.5));
    assert_eq!(value(&wb, "A4"), &CellValue::Bool(true));
    assert_eq!(value(&wb, "B4"), &CellValue::Number(7.5));
    assert_eq!(value(&wb, "C4"), &CellValue::Number(-15000.0));
    assert_eq!(value(&wb, "B5"), &CellValue::Error(CellError::Div0));
    assert_eq!(value(&wb, "C6"), &text("two\nlines"));
    // R1C1 expressions as A1 formulas of their cells.
    assert_eq!(formula(&wb, "B4"), Some("SUM(B2:B3)"));
    assert_eq!(formula(&wb, "C4"), Some("B4*-2000"));
    assert_eq!(formula(&wb, "B5"), Some("$B$2/0"));
    // `;S;R4;C2`: the expression of B4, seen from B6.
    assert_eq!(formula(&wb, "B6"), Some("SUM(B4:B5)"));
    assert_eq!(formula(&wb, "A1"), None);
    // Pictures by index; the picture's own `;;` is one semicolon.
    assert_eq!(num_fmt(&wb, "B3"), "0.00");
    assert_eq!(num_fmt(&wb, "C4"), "#,##0;\\-#,##0");
    assert_eq!(num_fmt(&wb, "B2"), "General");
    let b3 = wb.sheets[0].cell_a1("B3").unwrap();
    let align = wb.styles.xf(b3.style).unwrap().alignment.as_ref().unwrap();
    assert_eq!(align.horizontal.as_deref(), Some("right"));
    // Font 1 of the file: Arial 12 bold. `;SDI`: bold italic in the default font.
    let a1 = font(&wb, "A1");
    assert_eq!(
        (a1.name.as_deref(), a1.size, a1.bold, a1.italic),
        (Some("Arial"), Some(12.0), true, false)
    );
    let b1 = font(&wb, "B1");
    assert_eq!(
        (b1.name.as_deref(), b1.bold, b1.italic),
        (Some("Calibri"), true, true)
    );
    assert!(!font(&wb, "B2").bold);
    // Widths in characters (plus the cell padding, §18.3.1.13), row height in points.
    let ws = &wb.sheets[0];
    let width = |col: u32| ws.col_info(col).and_then(|c| c.width);
    assert!((width(0).unwrap() - (12.0 * 7.0 + 5.0) / 7.0).abs() < 0.01);
    assert_eq!(width(1), None);
    assert!((width(2).unwrap() - (20.0 * 7.0 + 5.0) / 7.0).abs() < 0.01);
    assert_eq!(ws.rows[&1].height, Some(30.0));
    assert_eq!(wb.defined_names.len(), 1);
    assert_eq!(wb.defined_names[0].name, "Total");
    assert_eq!(wb.defined_names[0].formula, "$B$4");
}

#[test]
fn sylk_round_trip() {
    let first = read_sylk(SYLK.as_bytes()).unwrap();
    let out = write_sylk(&first, &first.sheets[0]);
    let text = String::from_utf8(out.clone()).unwrap();
    assert!(text.starts_with("ID;PBLUEOFFICE;N;E\r\nP;PGeneral\r\n"));
    assert!(text.ends_with("\r\nE\r\n"));
    assert!(text.contains("C;Y4;X2;K7.5;ESUM(R[-2]C:R[-1]C)\r\n"));
    assert!(text.contains("C;Y2;X1;K\"Apple;; red\"\r\n"));
    assert!(text.contains("P;P#,##0;;\\-#,##0\r\n"));
    assert!(text.contains("F;W1 1 12\r\n"));
    assert!(text.contains("NN;NTotal;ER4C2\r\n"));
    let second = read_sylk(&out).unwrap();
    let (a, b) = (&first.sheets[0], &second.sheets[0]);
    assert_eq!(a.cells.len(), b.cells.len());
    for (at, cell) in &a.cells {
        let other = &b.cells[at];
        assert_eq!(cell.value, other.value, "{at:?}");
        assert_eq!(cell.formula, other.formula, "{at:?}");
    }
    for a1 in ["B3", "C4", "B2"] {
        assert_eq!(num_fmt(&first, a1), num_fmt(&second, a1));
    }
    assert!(font(&second, "A1").bold && !font(&second, "A1").italic);
    assert!(font(&second, "B1").bold && font(&second, "B1").italic);
    let b3 = second.sheets[0].cell_a1("B3").unwrap();
    assert_eq!(
        second
            .styles
            .xf(b3.style)
            .and_then(|x| x.alignment.as_ref())
            .and_then(|a| a.horizontal.as_deref()),
        Some("right")
    );
    assert_eq!(a.cols, b.cols);
    assert_eq!(b.rows[&1].height, Some(30.0));
    assert_eq!(first.defined_names, second.defined_names);
    // Writing what was read again gives the same bytes.
    assert_eq!(write_sylk(&second, &second.sheets[0]), out);
}

#[test]
fn sylk_writes_model_features() {
    let mut wb = Workbook::new();
    wb.date1904 = true;
    let bold = wb.styles.add_font(Font {
        bold: true,
        ..wb.styles.fonts[0].clone()
    });
    let pct = wb.styles.add_xf(CellXf {
        num_fmt_id: 10, // built-in `0.00%`
        font_id: bold,
        alignment: Some(CellAlignment {
            horizontal: Some("center".into()),
            ..CellAlignment::default()
        }),
        ..CellXf::default()
    });
    let ws = &mut wb.sheets[0];
    ws.set_cell(0, 0, Cell::new(CellValue::Number(0.125)).with_style(pct));
    // A shared formula: the text is on the master cell only (§18.3.1.40).
    ws.set_cell(
        1,
        0,
        Cell {
            value: CellValue::Number(1.0),
            formula: Some(Formula {
                text: "A1*2".into(),
                kind: FormulaKind::Shared {
                    index: 0,
                    range: Some("A2:B2".into()),
                },
                ..Formula::default()
            }),
            ..Cell::default()
        },
    );
    ws.set_cell(
        1,
        1,
        Cell {
            value: CellValue::Number(2.0),
            formula: Some(Formula {
                text: String::new(),
                kind: FormulaKind::Shared {
                    index: 0,
                    range: None,
                },
                ..Formula::default()
            }),
            ..Cell::default()
        },
    );
    ws.set_cell(2, 0, Cell::new(text("Việt Nam — “quoted”")));
    ws.set_cell(3, 0, Cell::new(CellValue::Empty).with_formula("NOW()"));
    ws.rows.insert(
        0,
        RowInfo {
            height: Some(24.0),
            custom_height: true,
            style: Some(pct),
            ..RowInfo::default()
        },
    );
    ws.cols.push(ColInfo {
        min: 0,
        max: 1,
        width: Some(15.71),
        custom_width: true,
        style: Some(pct),
        ..ColInfo::default()
    });
    wb.defined_names.push(DefinedName {
        name: "Rate".into(),
        formula: "Sheet1!$A$1".into(),
        ..DefinedName::default()
    });
    let out = write_sylk(&wb, &wb.sheets[0]);
    // Not representable in Windows-1252: the file is UTF-8.
    let text_out = String::from_utf8(out.clone()).unwrap();
    assert!(text_out.contains("O;V4\r\n"));
    assert!(text_out.contains("F;P1;FG0C;SD;Y1;X1\r\n"));
    assert!(text_out.contains("F;P1;FG0C;SD;C2\r\n"));
    assert!(text_out.contains("F;P1;FG0C;SD;M480;R1\r\n"));
    assert!(text_out.contains("F;W1 2 15\r\n"));
    assert!(text_out.contains("C;Y2;X2;K2;ER[-1]C*2\r\n"));
    let back = read_sylk(&out).unwrap();
    assert!(back.date1904);
    assert_eq!(value(&back, "A1"), &CellValue::Number(0.125));
    assert_eq!(num_fmt(&back, "A1"), "0.00%");
    assert!(font(&back, "A1").bold);
    assert_eq!(formula(&back, "A2"), Some("A1*2"));
    assert_eq!(formula(&back, "B2"), Some("B1*2"));
    assert_eq!(value(&back, "A3"), &text("Việt Nam — “quoted”"));
    assert_eq!(formula(&back, "A4"), Some("NOW()"));
    let ws = &back.sheets[0];
    assert_eq!(ws.rows[&0].height, Some(24.0));
    assert!(ws.rows[&0].style.is_some());
    assert_eq!(ws.cols.len(), 2, "{:?}", ws.cols);
    assert!(ws
        .cols
        .iter()
        .all(|c| c.style.is_some() && c.width.is_some()));
    assert_eq!(back.defined_names[0].formula, "Sheet1!$A$1");
}

#[test]
fn sylk_rejects_other_text_and_bad_positions() {
    assert_eq!(read_sylk(b"C;Y1;X1;K1\r\n"), Err(TabularError::NotSylk));
    assert!(matches!(
        read_sylk(b"ID;P\r\nC;Y1048577;X1;K1\r\n"),
        Err(TabularError::Limit(_))
    ));
    assert!(matches!(
        read_sylk(b"ID;P\r\nC;Y1;X16385;K1\r\n"),
        Err(TabularError::Limit(_))
    ));
    // Zero and unreadable coordinates keep the position; a bad expression keeps the value.
    let wb = read_sylk(b"ID;P\nC;Y0;Xq;K5;E((\nC;X2;K\"unterminated\nF;W9 2 5\nF;W1 99999 300\n")
        .unwrap();
    assert_eq!(value(&wb, "A1"), &CellValue::Number(5.0));
    assert_eq!(formula(&wb, "A1"), None);
    assert_eq!(value(&wb, "B1"), &text("unterminated"));
    assert_eq!(wb.sheets[0].cols.len(), 0);
    // Windows-1252 bytes (not valid UTF-8).
    let wb = read_sylk(b"ID;PWXL\r\nC;Y1;X1;K\"caf\xe9\"\r\nE\r\n").unwrap();
    assert_eq!(value(&wb, "A1"), &text("café"));
    // Overlapping width ranges stay sorted and disjoint; the last one wins.
    let wb = read_sylk(b"ID;P\r\nF;W1 10 5\r\nF;W3 4 9\r\nF;W4 12 2\r\nE\r\n").unwrap();
    let cols: Vec<(u32, u32, u32)> = wb.sheets[0]
        .cols
        .iter()
        .map(|c| (c.min, c.max, (c.width.unwrap() - 5.0 / 7.0).round() as u32))
        .collect();
    assert_eq!(
        cols,
        [(0, 1, 5), (2, 2, 9), (3, 3, 2), (4, 9, 2), (10, 11, 2)]
    );
}

/// A table as spreadsheet applications write it.
const DIF: &str = "TABLE\r\n0,1\r\n\"EXCEL\"\r\nVECTORS\r\n0,3\r\n\"\"\r\nTUPLES\r\n0,3\r\n\"\"\r\nDATA\r\n0,0\r\n\"\"\r\n\
-1,0\r\nBOT\r\n1,0\r\n\"Name\"\r\n1,0\r\n\"Amount\"\r\n1,0\r\n\"\"\r\n\
-1,0\r\nBOT\r\n1,0\r\n\"Say \"\"hi\"\"\"\r\n0,12.5\r\nV\r\n0,-1.5E+3\r\nV\r\n\
-1,0\r\nBOT\r\n0,1\r\nTRUE\r\n0,0\r\nNA\r\n0,0\r\nERROR\r\n\
-1,0\r\nEOD\r\n";

#[test]
fn dif_values() {
    let wb = read_dif(DIF.as_bytes()).unwrap();
    assert_eq!(value(&wb, "A1"), &text("Name"));
    assert_eq!(value(&wb, "B1"), &text("Amount"));
    assert!(wb.sheets[0].cell_a1("C1").is_none());
    assert_eq!(value(&wb, "A2"), &text("Say \"hi\""));
    assert_eq!(value(&wb, "B2"), &CellValue::Number(12.5));
    assert_eq!(value(&wb, "C2"), &CellValue::Number(-1500.0));
    assert_eq!(value(&wb, "A3"), &CellValue::Bool(true));
    assert_eq!(value(&wb, "B3"), &CellValue::Error(CellError::NA));
    assert_eq!(value(&wb, "C3"), &CellValue::Error(CellError::Value));
    assert_eq!(wb.sheets[0].cells.len(), 8);
    // The writer produces the same data part; counts are columns then rows.
    let out = String::from_utf8(write_dif(&wb.sheets[0]).unwrap()).unwrap();
    assert!(out.starts_with(
        "TABLE\r\n0,1\r\n\"Sheet1\"\r\nVECTORS\r\n0,3\r\n\"\"\r\nTUPLES\r\n0,3\r\n\"\"\r\nDATA\r\n0,0\r\n\"\"\r\n"
    ));
    let data = |s: &str| s[s.find("-1,0").unwrap()..].replace("-1.5E+3", "-1500");
    assert_eq!(data(&out), data(DIF));
    assert_eq!(
        read_dif(out.as_bytes()).unwrap().sheets[0].cells,
        wb.sheets[0].cells
    );
}

#[test]
fn dif_edge_cases() {
    assert_eq!(read_dif(b"VECTORS\r\n0,1\r\n"), Err(TabularError::NotDif));
    assert_eq!(read_dif(b"TABLE of contents"), Err(TabularError::NotDif));
    // A header with no data, and data cut off in the middle of a value.
    assert!(read_dif(b"TABLE\n0,1\n\"\"\n").unwrap().sheets[0]
        .cells
        .is_empty());
    let wb = read_dif(b"TABLE\n0,1\n\"\"\nDATA\n0,0\n\"\"\n-1,0\nBOT\n0,7\nV\n1,0\n").unwrap();
    assert_eq!(value(&wb, "A1"), &CellValue::Number(7.0));
    assert_eq!(wb.sheets[0].cells.len(), 1);
    // An empty sheet still is a DIF file.
    let empty = write_dif(&Worksheet::new("S")).unwrap();
    assert!(read_dif(&empty).unwrap().sheets[0].cells.is_empty());
    // A line break cannot be kept: a DIF string is one line.
    let mut ws = Worksheet::new("S");
    ws.set_cell(0, 0, Cell::new(text("a\nb")));
    ws.set_cell(2, 1, Cell::new(text("Tiếng Việt")));
    let back = read_dif(&write_dif(&ws).unwrap()).unwrap();
    assert_eq!(value(&back, "A1"), &text("a b"));
    assert_eq!(value(&back, "B3"), &text("Tiếng Việt"));
    assert_eq!(back.sheets[0].cells.len(), 2);
}

fn prn_sheet() -> Workbook {
    let mut wb = Workbook::new();
    let two = wb.styles.add_xf(CellXf {
        num_fmt_id: 2, // built-in `0.00`
        ..CellXf::default()
    });
    let ws = &mut wb.sheets[0];
    ws.set_cell(0, 0, Cell::new(text("Name")));
    ws.set_cell(0, 1, Cell::new(text("Qty")));
    ws.set_cell(1, 0, Cell::new(text("Apple")));
    ws.set_cell(1, 1, Cell::new(CellValue::Number(3.0)));
    ws.set_cell(2, 0, Cell::new(text("Kiwi")));
    ws.set_cell(2, 1, Cell::new(CellValue::Number(12.5)).with_style(two));
    wb
}

#[test]
fn prn_columns() {
    let wb = prn_sheet();
    let out = String::from_utf8(write_prn(&wb, &wb.sheets[0], &Locale::EN_US).unwrap()).unwrap();
    // Two columns of the default width (8 characters) and one space between them:
    // text starts at its column, numbers end at theirs, with their number format.
    assert_eq!(
        out,
        "Name     Qty\r\nApple           3\r\nKiwi        12.50\r\n"
    );
    let back = read_prn(out.as_bytes(), '.').unwrap();
    assert_eq!(value(&back, "A1"), &text("Name"));
    assert_eq!(value(&back, "B1"), &text("Qty"));
    assert_eq!(value(&back, "B2"), &CellValue::Number(3.0));
    assert_eq!(value(&back, "A3"), &text("Kiwi"));
    assert_eq!(value(&back, "B3"), &CellValue::Number(12.5));
    assert_eq!(back.sheets[0].cells.len(), 6);
    // The decimal separator of the locale.
    let vi = String::from_utf8(write_prn(&wb, &wb.sheets[0], &Locale::VI_VN).unwrap()).unwrap();
    assert!(vi.ends_with("Kiwi        12,50\r\n"));
    assert_eq!(
        value(&read_prn(vi.as_bytes(), ',').unwrap(), "B3"),
        &CellValue::Number(12.5)
    );
}

#[test]
fn prn_widths_gaps_and_best_effort_reading() {
    let mut wb = prn_sheet();
    {
        let ws = &mut wb.sheets[0];
        // A narrow column is widened to its longest text; an empty column keeps its width.
        ws.cols.push(ColInfo {
            min: 0,
            max: 0,
            width: Some(3.71),
            custom_width: true,
            ..ColInfo::default()
        });
        ws.cells.remove(&(2, 1));
        ws.set_cell(1, 3, Cell::new(text("x y")));
        ws.set_cell(4, 0, Cell::new(CellValue::Bool(true)));
    }
    let out = String::from_utf8(write_prn(&wb, &wb.sheets[0], &Locale::EN_US).unwrap()).unwrap();
    // Columns start at 0, 6, 15 and 24: widths 5 (widened), 8, 8 and 8.
    assert_eq!(
        out,
        format!(
            "Name  Qty\r\nApple{}3{}x y\r\nKiwi\r\n\r\nTRUE\r\n",
            " ".repeat(8),
            " ".repeat(10)
        )
    );
    let back = read_prn(out.as_bytes(), '.').unwrap();
    // A left-aligned heading over right-aligned numbers is one column.
    assert_eq!(value(&back, "B1"), &text("Qty"));
    assert_eq!(value(&back, "B2"), &CellValue::Number(3.0));
    // No line has a character between `x` and `y`: two columns (best effort);
    // the empty sheet column leaves no trace in the text.
    assert_eq!(value(&back, "C2"), &text("x"));
    assert_eq!(value(&back, "D2"), &text("y"));
    assert_eq!(value(&back, "A5"), &CellValue::Bool(true));
    assert!(back.sheets[0].cell_a1("A4").is_none());
    // Hand-written fixed-width text: tabs, a form feed, identifiers with leading
    // zeros, texts with spaces kept together by a neighbouring line.
    let src = b"Code  Full name      Amount\r\n007   Tran Thi B        1.5\r\n12    Le_Van_C_long   -20\r\n\x0c\x1a";
    let wb = read_prn(src, '.').unwrap();
    assert_eq!(value(&wb, "A2"), &text("007"));
    assert_eq!(value(&wb, "A3"), &CellValue::Number(12.0));
    assert_eq!(value(&wb, "B1"), &text("Full name"));
    assert_eq!(value(&wb, "B2"), &text("Tran Thi B"));
    assert_eq!(value(&wb, "C2"), &CellValue::Number(1.5));
    assert_eq!(value(&wb, "C3"), &CellValue::Number(-20.0));
    assert_eq!(wb.sheets[0].cells.len(), 9);
    // Column widths follow the fields: 4, 13 and 6 characters.
    let chars: Vec<u32> = wb.sheets[0]
        .cols
        .iter()
        .map(|c| (c.width.unwrap() - 5.0 / 7.0).round() as u32)
        .collect();
    assert_eq!(chars, [4, 13, 6]);
    assert!(read_prn(b"", '.').unwrap().sheets[0].cells.is_empty());
    let nothing = write_prn(&Workbook::new(), &Worksheet::new("S"), &Locale::EN_US).unwrap();
    assert_eq!(nothing.len(), 0);
}

/// A dBASE table built field by field from the file structure description.
struct Dbf {
    kind: u8,
    driver: u8,
    fields: Vec<(&'static str, u8, u16, u8)>,
    records: Vec<Vec<u8>>,
    backlink: usize,
}

impl Dbf {
    fn bytes(&self) -> Vec<u8> {
        let header = 32 + 32 * self.fields.len() + 1 + self.backlink;
        let record: usize = 1 + self.fields.iter().map(|f| f.2 as usize).sum::<usize>();
        let mut b = vec![0u8; 32];
        b[0] = self.kind;
        b[1..4].copy_from_slice(&[124, 3, 15]); // last update: 2024-03-15
        b[4..8].copy_from_slice(&(self.records.len() as u32).to_le_bytes());
        b[8..10].copy_from_slice(&(header as u16).to_le_bytes());
        b[10..12].copy_from_slice(&(record as u16).to_le_bytes());
        b[29] = self.driver;
        for (name, kind, len, dec) in &self.fields {
            let mut d = [0u8; 32];
            d[..name.len()].copy_from_slice(name.as_bytes());
            d[11] = *kind;
            d[16] = (*len & 0xFF) as u8;
            d[17] = if *kind == b'C' {
                (*len >> 8) as u8
            } else {
                *dec
            };
            b.extend_from_slice(&d);
        }
        b.push(0x0D);
        b.extend(std::iter::repeat_n(0u8, self.backlink));
        assert_eq!(b.len(), header);
        for r in &self.records {
            assert_eq!(r.len(), record, "record length");
            b.extend_from_slice(r);
        }
        b.push(0x1A);
        b
    }
}

fn record(flag: u8, parts: &[&[u8]]) -> Vec<u8> {
    let mut r = vec![flag];
    for p in parts {
        r.extend_from_slice(p);
    }
    r
}

#[test]
fn dbase3_table() {
    let dbf = Dbf {
        kind: 0x83,   // dBASE III with a memo file
        driver: 0x02, // code page 850
        fields: vec![
            ("NAME", b'C', 10, 0),
            ("PRICE", b'N', 8, 2),
            ("QTY", b'N', 4, 0),
            ("RATE", b'F', 10, 3),
            ("BORN", b'D', 8, 0),
            ("OK", b'L', 1, 0),
            ("NOTE", b'M', 10, 0),
        ],
        records: vec![
            record(
                b' ',
                &[
                    b"Caf\x82      ",
                    b"   12.50",
                    b"   7",
                    b"     1.250",
                    b"20240315",
                    b"T",
                    b"         1",
                ],
            ),
            record(
                b'*',
                &[
                    b"deleted   ",
                    b"    1.00",
                    b"   1",
                    b"     0.000",
                    b"20000101",
                    b"F",
                    b"          ",
                ],
            ),
            record(
                b' ',
                &[
                    b"\x9d\xb5        ",
                    b"        ",
                    b"  -3",
                    b"**********",
                    b"19000301",
                    b"n",
                    b"          ",
                ],
            ),
            record(
                b' ',
                &[
                    b"          ",
                    b"   -0.05",
                    b"    ",
                    b"      1e2 ",
                    b"        ",
                    b"?",
                    b"          ",
                ],
            ),
        ],
        backlink: 0,
    };
    let wb = read_dbf(&dbf.bytes()).unwrap();
    let ws = &wb.sheets[0];
    let names: Vec<_> = (0..7)
        .map(|c| ws.cell(0, c).unwrap().value.clone())
        .collect();
    assert_eq!(
        names,
        ["NAME", "PRICE", "QTY", "RATE", "BORN", "OK", "NOTE"].map(text)
    );
    // Code page 850: 0x82 é, 0x9D Ø, 0xB5 Á. Trailing blanks are padding.
    assert_eq!(value(&wb, "A2"), &text("Café"));
    assert_eq!(value(&wb, "B2"), &CellValue::Number(12.5));
    assert_eq!(num_fmt(&wb, "B2"), "0.00");
    assert_eq!(value(&wb, "C2"), &CellValue::Number(7.0));
    assert_eq!(num_fmt(&wb, "C2"), "General");
    assert_eq!(value(&wb, "D2"), &CellValue::Number(1.25));
    assert_eq!(num_fmt(&wb, "D2"), "0.000");
    // 2024-03-15 is serial 45366 in the 1900 date system; shown as a short date.
    assert_eq!(value(&wb, "E2"), &CellValue::Number(45366.0));
    assert_eq!(num_fmt(&wb, "E2"), Locale::EN_US.short_date);
    assert_eq!(value(&wb, "F2"), &CellValue::Bool(true));
    // The memo text is in another file.
    assert!(ws.cell_a1("G2").is_none());
    // The deleted record is skipped: the third record is row 3.
    assert_eq!(value(&wb, "A3"), &text("ØÁ"));
    assert!(ws.cell_a1("B3").is_none());
    assert_eq!(value(&wb, "C3"), &CellValue::Number(-3.0));
    assert!(ws.cell_a1("D3").is_none(), "an overflowed number is blank");
    // 1900-03-01 is serial 61 (serial 60 is the day that never was).
    assert_eq!(value(&wb, "E3"), &CellValue::Number(61.0));
    assert_eq!(value(&wb, "F3"), &CellValue::Bool(false));
    assert!(ws.cell_a1("A4").is_none());
    assert_eq!(value(&wb, "B4"), &CellValue::Number(-0.05));
    assert_eq!(value(&wb, "D4"), &CellValue::Number(100.0));
    assert!(ws.cell_a1("E4").is_none() && ws.cell_a1("F4").is_none());
    assert_eq!(ws.used_range().unwrap().last.row, 3);
    assert_eq!(ws.cols.len(), 7);
}

#[test]
fn visual_foxpro_table() {
    let mut stamp = Vec::new();
    stamp.extend_from_slice(&2_460_385u32.to_le_bytes()); // Julian day of 2024-03-15
    stamp.extend_from_slice(&43_200_000u32.to_le_bytes()); // 12:00:00
    let long = [b'x'; 300];
    let dbf = Dbf {
        kind: 0x30,
        driver: 0xC9, // Windows-1251
        fields: vec![
            ("ID", b'I', 4, 0),
            ("AMOUNT", b'Y', 8, 4),
            ("RATIO", b'B', 8, 2),
            ("AT", b'T', 8, 0),
            ("TEXT", b'C', 300, 0),
            ("_NullFlags", b'0', 1, 0),
            ("CITY", b'C', 6, 0),
        ],
        records: vec![record(
            b' ',
            &[
                &(-42i32).to_le_bytes(),
                &1_234_500i64.to_le_bytes(),
                &2.5f64.to_le_bytes(),
                &stamp,
                &long,
                &[0],
                b"\xcc\xee\xf1\xea\xe2\xe0",
            ],
        )],
        backlink: 263,
    };
    let wb = read_dbf(&dbf.bytes()).unwrap();
    assert_eq!(value(&wb, "A2"), &CellValue::Number(-42.0));
    assert_eq!(value(&wb, "B2"), &CellValue::Number(123.45));
    assert_eq!(num_fmt(&wb, "B2"), "0.0000");
    assert_eq!(value(&wb, "C2"), &CellValue::Number(2.5));
    assert_eq!(value(&wb, "D2"), &CellValue::Number(45366.5));
    assert_eq!(num_fmt(&wb, "D2"), "yyyy-mm-dd hh:mm:ss");
    // A character field longer than 255 bytes.
    assert_eq!(value(&wb, "E2"), &text(&"x".repeat(300)));
    // The null-flags field is not a column: CITY follows TEXT.
    assert_eq!(value(&wb, "F1"), &text("CITY"));
    assert_eq!(value(&wb, "F2"), &text("Москва"));
    assert!(wb.sheets[0].cell_a1("G1").is_none());
}

#[test]
fn dbf_damage_and_encodings() {
    let table = |driver: u8, cell: &'static [u8]| Dbf {
        kind: 0x03,
        driver,
        fields: vec![("T", b'C', 6, 0)],
        records: vec![record(b' ', &[cell])],
        backlink: 0,
    };
    // No language driver: UTF-8 when valid, else Windows-1252.
    let wb = read_dbf(&table(0, b"Vi\xe1\xbb\x87t").bytes()).unwrap();
    assert_eq!(value(&wb, "A2"), &text("Việt"));
    let wb = read_dbf(&table(0, b"caf\xe9  ").bytes()).unwrap();
    assert_eq!(value(&wb, "A2"), &text("café"));
    // A double-byte code page that is not supported falls back the same way.
    let wb = read_dbf(&table(0x7B, b"abc   ").bytes()).unwrap();
    assert_eq!(value(&wb, "A2"), &text("abc"));
    // The header claims more records than the file holds.
    let mut bytes = table(0x03, b"one   ").bytes();
    bytes[4..8].copy_from_slice(&1_000_000u32.to_le_bytes());
    let wb = read_dbf(&bytes).unwrap();
    assert_eq!(wb.sheets[0].cells.len(), 2);
    // A record cut short is not read.
    let cut = &bytes[..bytes.len() - 3];
    assert_eq!(read_dbf(cut).unwrap().sheets[0].cells.len(), 1);
    // Not tables.
    for bad in [
        &b""[..],
        b"\x03",
        &[0x03; 31],
        b"plain text, long enough to hold a header ..",
    ] {
        assert!(
            matches!(read_dbf(bad), Err(TabularError::Dbf(_))),
            "{bad:?}"
        );
    }
    // Fields longer than the record.
    let mut bytes = table(0x03, b"one   ").bytes();
    bytes[10] = 3;
    assert!(matches!(read_dbf(&bytes), Err(TabularError::Dbf(_))));
    // dBASE 7 (another header layout) is not claimed.
    let mut bytes = table(0x03, b"one   ").bytes();
    bytes[0] = 0x04;
    assert!(!bo_tabular::is_dbf(&bytes));
}

#[test]
fn sparse_sheets_do_not_explode() {
    // One cell in the last row and column: DIF would need 17 thousand million
    // values, formatted text a megabyte of line ends and one long line.
    let mut wb = Workbook::new();
    wb.sheets[0].set_cell(0, 0, Cell::new(text("a")));
    wb.sheets[0].set_cell(1_048_575, 16_383, Cell::new(CellValue::Number(1.0)));
    assert!(matches!(
        write_dif(&wb.sheets[0]),
        Err(TabularError::Limit(_))
    ));
    let prn = write_prn(&wb, &wb.sheets[0], &Locale::EN_US).unwrap();
    assert!(prn.starts_with(b"a\r\n\r\n"));
    assert!(prn.ends_with(b" 1\r\n"));
    // 16 383 columns of 8 characters and a space, then the last one.
    assert_eq!(prn.len(), 3 + 1_048_574 * 2 + 16_383 * 9 + 8 + 2);
    // SYLK is sparse by nature.
    let slk = write_sylk(&wb, &wb.sheets[0]);
    assert!(slk.len() < 200);
    assert_eq!(
        value(&read_sylk(&slk).unwrap(), "XFD1048576"),
        &CellValue::Number(1.0)
    );
    // The widest sheet DIF accepts: 8 000 000 values.
    let mut ws = Worksheet::new("S");
    ws.set_cell(7_999, 999, Cell::new(CellValue::Bool(true)));
    assert!(write_dif(&ws).is_ok());
    ws.set_cell(8_000, 999, Cell::new(CellValue::Bool(true)));
    assert!(write_dif(&ws).is_err());
}

mod random {
    use super::*;
    use proptest::prelude::*;

    fn cell_value() -> impl Strategy<Value = CellValue> {
        prop_oneof![
            (-1.0e12..1.0e12f64).prop_map(CellValue::Number),
            (-100_000i32..100_000).prop_map(|n| CellValue::Number(f64::from(n) / 8.0)),
            any::<bool>().prop_map(CellValue::Bool),
            // Texts with the characters the formats escape; never looking like
            // another type once quoted.
            "[a-zA-Z;\"'=, éệЖ€0-9#]{1,12}".prop_map(CellValue::Text),
            Just(CellValue::Error(CellError::NA)),
        ]
    }

    fn sheet() -> impl Strategy<Value = Worksheet> {
        proptest::collection::btree_map((0u32..12, 0u32..6), cell_value(), 0..40).prop_map(|m| {
            let mut ws = Worksheet::new("Sheet1");
            ws.sheet_id = 1;
            for (at, v) in m {
                ws.cells.insert(at, Cell::new(v));
            }
            ws
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn sylk_keeps_every_value(ws in sheet()) {
            let wb = Workbook { sheets: vec![ws.clone()], ..Workbook::new() };
            let back = read_sylk(&write_sylk(&wb, &ws)).unwrap();
            prop_assert_eq!(&back.sheets[0].cells, &ws.cells);
        }

        #[test]
        fn dif_keeps_every_value(ws in sheet()) {
            let back = read_dif(&write_dif(&ws).unwrap()).unwrap();
            prop_assert_eq!(&back.sheets[0].cells, &ws.cells);
        }

        #[test]
        fn readers_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..600)) {
            let mut sylk = b"ID;P\n".to_vec();
            sylk.extend_from_slice(&bytes);
            let _ = read_sylk(&sylk);
            let mut dif = b"TABLE\n0,1\n\"\"\nDATA\n0,0\n\"\"\n".to_vec();
            dif.extend_from_slice(&bytes);
            let _ = read_dif(&dif);
            let _ = read_prn(&bytes, '.');
            let _ = read_dbf(&bytes);
            let mut dbf = vec![0x03, 1, 1, 1, 9, 0, 0, 0, 65, 0, 20, 0];
            dbf.resize(32, 0);
            dbf.extend_from_slice(&bytes);
            let _ = read_dbf(&dbf);
        }

        #[test]
        fn sylk_records_never_panic(body in "((C|F|P|NN|O|B);[XYKESRCMPFWDN][ -~]{0,12}(;[XYKESRCMPFWDN][ -~]{0,8}){0,4}\n){0,12}") {
            let _ = read_sylk(format!("ID;P\n{body}").as_bytes());
        }
    }
}
