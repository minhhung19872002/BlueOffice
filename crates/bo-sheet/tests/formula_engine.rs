//! `bo-formula` behind the `calc` boundary: recalculation of the workbook model
//! and reference adjustment for insert/delete, AutoFill and sort.

use bo_model::sheet::{
    Cell, CellError, CellRange, CellValue, DefinedName, Formula, FormulaKind, Table, TableColumn,
    Workbook, Worksheet,
};
use bo_sheet::calc::{FormulaAdjuster, FormulaEngine};
use bo_sheet::fill::autofill;
use bo_sheet::formula::BoFormula;
use bo_sheet::numfmt::Locale;
use bo_sheet::ops::{delete_rows, insert_cols, insert_rows};
use bo_sheet::sort::{sort_range_with, SortKey};

fn num(n: f64) -> CellValue {
    CellValue::Number(n)
}

/// A workbook without the default sheet.
fn empty_book() -> Workbook {
    let mut wb = Workbook::new();
    wb.sheets.clear();
    wb
}

fn formula_cell(text: &str) -> Cell {
    Cell::new(CellValue::Empty).with_formula(text)
}

fn value(wb: &Workbook, sheet: usize, a1: &str) -> CellValue {
    wb.sheets[sheet]
        .cell_a1(a1)
        .map_or(CellValue::Empty, |c| c.value.clone())
}

fn text(wb: &Workbook, sheet: usize, a1: &str) -> String {
    wb.sheets[sheet]
        .cell_a1(a1)
        .and_then(|c| c.formula.as_ref())
        .map(|f| f.text.clone())
        .unwrap_or_default()
}

fn set_formula(ws: &mut Worksheet, a1: &str, f: &str) {
    let r = bo_model::sheet::CellRef::parse(a1).expect("cell");
    ws.set_cell(r.row, r.col, formula_cell(f));
}

fn book() -> Workbook {
    let mut wb = empty_book();
    let mut s1 = Worksheet::new("Sheet1");
    s1.set_a1("A1", num(1.0));
    s1.set_a1("A2", num(2.0));
    s1.set_a1("A3", num(3.0));
    set_formula(&mut s1, "B1", "SUM(A1:A3)");
    set_formula(&mut s1, "B2", "A2*Rate");
    set_formula(&mut s1, "B3", "'Data 2'!A1+1");
    let mut s2 = Worksheet::new("Data 2");
    s2.set_a1("A1", num(41.0));
    wb.sheets.push(s1);
    wb.sheets.push(s2);
    wb.defined_names.push(DefinedName {
        name: "Rate".into(),
        formula: "0.5".into(),
        ..DefinedName::default()
    });
    wb
}

#[test]
fn recalculates_cached_values() {
    let mut wb = book();
    let mut engine = BoFormula::new();
    engine.recalculate(&mut wb);
    assert_eq!(value(&wb, 0, "B1"), num(6.0));
    assert_eq!(value(&wb, 0, "B2"), num(1.0));
    assert_eq!(value(&wb, 0, "B3"), num(42.0));
    wb.sheets[0].set_a1("A1", num(10.0));
    set_formula(&mut wb.sheets[0], "C1", "1/0");
    engine.recalculate(&mut wb);
    assert_eq!(value(&wb, 0, "B1"), num(15.0));
    assert_eq!(value(&wb, 0, "C1"), CellValue::Error(CellError::Div0));
}

#[test]
fn shared_formulas_are_expanded() {
    let mut wb = empty_book();
    let mut ws = Worksheet::new("S");
    for r in 0..3 {
        ws.set_cell(r, 0, Cell::new(num(f64::from(r + 1))));
    }
    let master = Cell {
        formula: Some(Formula {
            text: "A1*10".into(),
            kind: FormulaKind::Shared {
                index: 0,
                range: Some("B1:B3".into()),
            },
            ..Formula::default()
        }),
        ..Cell::default()
    };
    ws.set_cell(0, 1, master);
    for r in 1..3 {
        ws.set_cell(
            r,
            1,
            Cell {
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
    }
    wb.sheets.push(ws);
    BoFormula::new().recalculate(&mut wb);
    assert_eq!(value(&wb, 0, "B3"), num(30.0));
}

#[test]
fn dynamic_arrays_spill_into_cells() {
    let mut wb = empty_book();
    let mut ws = Worksheet::new("S");
    set_formula(&mut ws, "A1", "SEQUENCE(3)");
    // Stale results of an earlier, larger array are cleared.
    ws.set_cell(
        0,
        0,
        Cell {
            formula: Some(Formula {
                text: "SEQUENCE(3)".into(),
                kind: FormulaKind::Array {
                    range: "A1:A5".into(),
                },
                ..Formula::default()
            }),
            ..Cell::default()
        },
    );
    ws.set_a1("A5", num(99.0));
    set_formula(&mut ws, "B1", "SUM(A1#)");
    wb.sheets.push(ws);
    BoFormula::new().recalculate(&mut wb);
    assert_eq!(value(&wb, 0, "A3"), num(3.0));
    assert_eq!(value(&wb, 0, "A5"), CellValue::Empty);
    assert_eq!(value(&wb, 0, "B1"), num(6.0));
    let f = wb.sheets[0].cell_a1("A1").and_then(|c| c.formula.clone());
    assert_eq!(
        f.map(|f| f.kind),
        Some(FormulaKind::Array {
            range: "A1:A3".into()
        })
    );
}

#[test]
fn tables_and_date_system() {
    let mut wb = empty_book();
    wb.date1904 = true;
    let mut ws = Worksheet::new("S");
    ws.set_a1("A1", CellValue::Text("Qty".into()));
    ws.set_a1("A2", num(2.0));
    ws.set_a1("A3", num(5.0));
    ws.tables.push(Table {
        name: "T".into(),
        display_name: "T".into(),
        range: CellRange::parse("A1:A3").unwrap(),
        columns: vec![TableColumn {
            id: 1,
            name: "Qty".into(),
            ..TableColumn::default()
        }],
        ..Table::default()
    });
    set_formula(&mut ws, "C1", "SUM(T[Qty])");
    set_formula(&mut ws, "C2", "DATE(1904,1,2)");
    wb.sheets.push(ws);
    BoFormula::new().recalculate(&mut wb);
    assert_eq!(value(&wb, 0, "C1"), num(7.0));
    assert_eq!(value(&wb, 0, "C2"), num(1.0));
}

#[test]
fn insert_delete_adjust_references() {
    let mut wb = book();
    set_formula(&mut wb.sheets[0], "C1", "A2*2");
    let adj = BoFormula::new();
    insert_rows(&mut wb, 0, 0, 2, &adj).unwrap();
    assert_eq!(text(&wb, 0, "B3"), "SUM(A3:A5)");
    assert_eq!(text(&wb, 0, "C3"), "A4*2");
    insert_cols(&mut wb, 1, 0, 1, &adj).unwrap();
    assert_eq!(text(&wb, 0, "B5"), "'Data 2'!B1+1");
    delete_rows(&mut wb, 0, 3, 1, &adj).unwrap();
    assert_eq!(text(&wb, 0, "B3"), "SUM(A3:A4)");
    assert_eq!(text(&wb, 0, "C3"), "#REF!*2");
    assert_eq!(text(&wb, 0, "B4"), "'Data 2'!B1+1");
    let mut engine = adj;
    engine.recalculate(&mut wb);
    assert_eq!(value(&wb, 0, "B3"), num(4.0));
    assert_eq!(value(&wb, 0, "C3"), CellValue::Error(CellError::Ref));
    assert_eq!(value(&wb, 0, "B4"), num(42.0));
    // Unchanged formulas report no rewrite.
    let change = bo_sheet::calc::StructuralChange {
        sheet: "Other".into(),
        dimension: bo_sheet::calc::Dimension::Rows,
        at: 0,
        count: 1,
        insert: true,
    };
    assert_eq!(adj.adjust("A1+1", Some("Sheet1"), &change), None);
}

#[test]
fn autofill_and_sort_translate_formulas() {
    let mut wb = empty_book();
    let mut ws = Worksheet::new("S");
    ws.set_a1("A1", num(3.0));
    ws.set_a1("A2", num(1.0));
    ws.set_a1("A3", num(2.0));
    set_formula(&mut ws, "B1", "A1*$D$1");
    ws.set_a1("D1", num(10.0));
    wb.sheets.push(ws);
    let adj = BoFormula::new();
    let ok = autofill(
        &mut wb,
        0,
        CellRange::parse("B1").unwrap(),
        CellRange::parse("B1:B3").unwrap(),
        &Locale::default(),
        &adj,
    );
    assert!(ok);
    assert_eq!(text(&wb, 0, "B3"), "A3*$D$1");
    sort_range_with(
        &mut wb.sheets[0],
        CellRange::parse("A1:B3").unwrap(),
        &[SortKey::asc(0)],
        false,
        &adj,
    );
    assert_eq!(text(&wb, 0, "B1"), "A1*$D$1");
    assert_eq!(value(&wb, 0, "A1"), num(1.0));
    let mut engine = adj;
    engine.recalculate(&mut wb);
    assert_eq!(value(&wb, 0, "B1"), num(10.0));
    assert_eq!(value(&wb, 0, "B3"), num(30.0));
}
