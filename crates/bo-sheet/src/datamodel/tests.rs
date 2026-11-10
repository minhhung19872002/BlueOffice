//! Data model tests. Expected values are computed independently in the tests: a naive
//! nested-loop join of the source rows, filtered and summed by plain Rust code.

use super::eval::{check, Context, Evaluator, Kind, V};
use super::expr::{parse, Expr, Op};
use super::pivot::{create_model_pivot, field_list, FieldKind};
use super::*;
use crate::numfmt::Locale;
use crate::pivot::{PivotLabels, PivotSpec};
use bo_model::sheet::{
    Cell, CellError, CellRange, CellRef, PivotDataField, PivotFunction, PivotLayout, Workbook,
};

// ---------------------------------------------------------------- source data

/// (Id, Name, Category, Price)
const PRODUCTS: [(f64, &str, &str, f64); 5] = [
    (1.0, "Pen", "Office", 2.0),
    (2.0, "Book", "Office", 10.0),
    (3.0, "Apple", "Food", 1.0),
    (4.0, "Bread", "Food", 3.0),
    (5.0, "Lamp", "Home", 25.0),
];

/// (OrderId, ProductId, Qty, Region); product 9 does not exist.
const SALES: [(f64, f64, f64, &str); 8] = [
    (1.0, 1.0, 10.0, "North"),
    (2.0, 1.0, 5.0, "South"),
    (3.0, 2.0, 2.0, "North"),
    (4.0, 3.0, 20.0, "South"),
    (5.0, 3.0, 7.0, "North"),
    (6.0, 4.0, 4.0, "North"),
    (7.0, 9.0, 1.0, "South"),
    (8.0, 2.0, 3.0, "South"),
];

fn text(s: &str) -> CellValue {
    CellValue::Text(s.into())
}

fn num(n: f64) -> CellValue {
    CellValue::Number(n)
}

/// A workbook with the tables `Products` (A1:D6) and `Sales` (F1:I9) on sheet 0.
fn workbook() -> Workbook {
    let mut wb = Workbook::new();
    let ws = &mut wb.sheets[0];
    for (c, h) in ["Id", "Name", "Category", "Price"].iter().enumerate() {
        ws.set_cell(0, c as u32, Cell::new(text(h)));
    }
    for (i, (id, name, cat, price)) in PRODUCTS.iter().enumerate() {
        let r = i as u32 + 1;
        ws.set_cell(r, 0, Cell::new(num(*id)));
        ws.set_cell(r, 1, Cell::new(text(name)));
        ws.set_cell(r, 2, Cell::new(text(cat)));
        ws.set_cell(r, 3, Cell::new(num(*price)));
    }
    for (c, h) in ["OrderId", "ProductId", "Qty", "Region"].iter().enumerate() {
        ws.set_cell(0, 5 + c as u32, Cell::new(text(h)));
    }
    for (i, (o, p, q, reg)) in SALES.iter().enumerate() {
        let r = i as u32 + 1;
        ws.set_cell(r, 5, Cell::new(num(*o)));
        ws.set_cell(r, 6, Cell::new(num(*p)));
        ws.set_cell(r, 7, Cell::new(num(*q)));
        ws.set_cell(r, 8, Cell::new(text(reg)));
    }
    let loc = Locale::default();
    let a = crate::table::create_table(&mut wb, 0, range("A1:D6"), &loc).unwrap();
    crate::table::rename_table(&mut wb, 0, a, "Products").unwrap();
    let b = crate::table::create_table(&mut wb, 0, range("F1:I9"), &loc).unwrap();
    crate::table::rename_table(&mut wb, 0, b, "Sales").unwrap();
    wb.sheets.push(bo_model::sheet::Worksheet::new("Report"));
    wb
}

fn range(s: &str) -> CellRange {
    CellRange::parse(s).unwrap()
}

fn measure(name: &str, table: &str, expression: &str) -> Measure {
    Measure {
        name: name.into(),
        table: table.into(),
        expression: expression.into(),
        format: None,
    }
}

/// The test model: both tables, Sales[ProductId] → Products[Id], a calculated column and
/// the measures used below.
fn model() -> DataModel {
    DataModel {
        tables: vec![
            ModelTable {
                name: "Products".into(),
            },
            ModelTable {
                name: "Sales".into(),
            },
        ],
        relationships: vec![Relationship {
            from_table: "Sales".into(),
            from_column: "ProductId".into(),
            to_table: "Products".into(),
            to_column: "Id".into(),
            active: true,
        }],
        columns: vec![CalcColumn {
            table: "Sales".into(),
            name: "Amount".into(),
            expression: "Sales[Qty] * RELATED(Products[Price])".into(),
        }],
        measures: vec![
            measure("Total Qty", "Sales", "SUM(Sales[Qty])"),
            measure("Revenue", "Sales", "SUM(Sales[Amount])"),
            measure("Orders", "Sales", "COUNTROWS(Sales)"),
            measure("Rev per Order", "Sales", "DIVIDE([Revenue], [Orders], 0)"),
            measure(
                "North Qty",
                "Sales",
                "CALCULATE([Total Qty], Sales[Region] = \"North\")",
            ),
            measure("All Qty", "Sales", "CALCULATE([Total Qty], ALL(Products))"),
            measure("Share", "Sales", "DIVIDE([Total Qty], [All Qty])"),
            measure(
                "Big Orders",
                "Sales",
                "CALCULATE(COUNTROWS(Sales), FILTER(Sales, Sales[Qty] >= 5))",
            ),
            measure("Products Sold", "Sales", "DISTINCTCOUNT(Sales[ProductId])"),
            measure("Avg Price", "Products", "AVERAGE(Products[Price])"),
            measure("Max Qty", "Sales", "MAX(Sales[Qty])"),
            measure("Min Qty", "Sales", "MIN(Sales[Qty])"),
            measure("Qty Count", "Sales", "COUNT(Sales[Qty])"),
            measure(
                "Revenue X",
                "Sales",
                "SUMX(Sales, Sales[Qty] * RELATED(Products[Price]))",
            ),
        ],
        pivots: Vec::new(),
    }
}

fn loaded() -> (Workbook, ModelData) {
    let mut wb = workbook();
    model().write(&mut wb);
    let md = ModelData::load(&wb);
    assert!(md.problems.is_empty(), "{:?}", md.problems);
    (wb, md)
}

/// One sale joined with its product (if any), by a nested loop.
#[derive(Debug, Clone)]
struct Joined {
    pid: f64,
    qty: f64,
    region: &'static str,
    product: Option<(f64, &'static str, &'static str, f64)>,
}

fn naive() -> Vec<Joined> {
    SALES
        .iter()
        .map(|(_, p, q, r)| Joined {
            pid: *p,
            qty: *q,
            region: r,
            product: PRODUCTS.iter().find(|x| x.0 == *p).copied(),
        })
        .collect()
}

fn cat(j: &Joined) -> &'static str {
    j.product.map_or("", |p| p.2)
}

fn amount(j: &Joined) -> f64 {
    j.qty * j.product.map_or(0.0, |p| p.3)
}

fn blank_or(n: f64, count: usize) -> V {
    if count == 0 {
        V::Blank
    } else {
        V::Num(n)
    }
}

/// Expected value of measure `name` over the sales `rows` (already filtered by the
/// category / region of the context) — written directly over the naive join.
fn expected(
    name: &str,
    rows: &[Joined],
    products: &[(f64, &str, &str, f64)],
    cat_filter: Option<&str>,
    region_filter: Option<&str>,
) -> V {
    let all = naive();
    let qty: f64 = rows.iter().map(|j| j.qty).sum();
    let rev: f64 = rows.iter().map(amount).sum();
    let n = rows.len();
    // Amounts of sales without a product are blank (qty * blank).
    let priced = rows.iter().filter(|j| j.product.is_some()).count();
    match name {
        "Total Qty" => blank_or(qty, n),
        "Revenue" | "Revenue X" => blank_or(rev, priced),
        "Orders" | "Qty Count" => blank_or(n as f64, n),
        "Rev per Order" => {
            if n == 0 {
                V::Num(0.0)
            } else if priced == 0 {
                V::Blank
            } else {
                V::Num(rev / n as f64)
            }
        }
        "North Qty" => {
            // CALCULATE replaces the region filter: the category's rows in North.
            let north: Vec<&Joined> = all
                .iter()
                .filter(|j| cat_filter.is_none_or(|c| cat(j) == c) && j.region == "North")
                .collect();
            blank_or(north.iter().map(|j| j.qty).sum(), north.len())
        }
        // ALL(Products) removes the category filter, not the region one.
        "All Qty" | "Share" => {
            let total: f64 = all
                .iter()
                .filter(|j| region_filter.is_none_or(|r| j.region == r))
                .map(|j| j.qty)
                .sum();
            if name == "All Qty" {
                return V::Num(total);
            }
            if n == 0 {
                V::Blank
            } else {
                V::Num(qty / total)
            }
        }
        "Big Orders" => {
            let k = rows.iter().filter(|j| j.qty >= 5.0).count();
            blank_or(k as f64, k)
        }
        "Products Sold" => {
            let mut ids: Vec<u64> = rows.iter().map(|j| j.pid.to_bits()).collect();
            ids.sort_unstable();
            ids.dedup();
            blank_or(ids.len() as f64, ids.len())
        }
        "Avg Price" => blank_or(
            products.iter().map(|p| p.3).sum::<f64>() / products.len().max(1) as f64,
            products.len(),
        ),
        "Max Qty" => blank_or(rows.iter().map(|j| j.qty).fold(f64::MIN, f64::max), n),
        "Min Qty" => blank_or(rows.iter().map(|j| j.qty).fold(f64::MAX, f64::min), n),
        other => panic!("no expectation for {other}"),
    }
}

fn close(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::Num(x), V::Num(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    }
}

// ---------------------------------------------------------------- parsing

#[test]
fn parses_the_grammar() {
    let col = |t: &str, c: &str| Expr::Col {
        table: t.into(),
        column: c.into(),
    };
    let cases: Vec<(&str, Expr)> = vec![
        (
            "1 + 2 * 3",
            Expr::Bin(
                Op::Add,
                Box::new(Expr::Num(1.0)),
                Box::new(Expr::Bin(
                    Op::Mul,
                    Box::new(Expr::Num(2.0)),
                    Box::new(Expr::Num(3.0)),
                )),
            ),
        ),
        (
            "=(1 + 2) * 3",
            Expr::Bin(
                Op::Mul,
                Box::new(Expr::Bin(
                    Op::Add,
                    Box::new(Expr::Num(1.0)),
                    Box::new(Expr::Num(2.0)),
                )),
                Box::new(Expr::Num(3.0)),
            ),
        ),
        (
            "sum(Sales[Qty])",
            Expr::Call("SUM".into(), vec![col("Sales", "Qty")]),
        ),
        ("'Bán hàng'[Số lượng]", col("Bán hàng", "Số lượng")),
        ("[Revenue]", Expr::Bracket("Revenue".into())),
        (
            "COUNTROWS(Sales)",
            Expr::Call("COUNTROWS".into(), vec![Expr::Table("Sales".into())]),
        ),
        (
            "-[A] & \"x\"\"y\"",
            Expr::Bin(
                Op::Concat,
                Box::new(Expr::Neg(Box::new(Expr::Bracket("A".into())))),
                Box::new(Expr::Str("x\"y".into())),
            ),
        ),
        (
            "1 <> 2 && TRUE || FALSE",
            Expr::Bin(
                Op::Or,
                Box::new(Expr::Bin(
                    Op::And,
                    Box::new(Expr::Bin(
                        Op::Ne,
                        Box::new(Expr::Num(1.0)),
                        Box::new(Expr::Num(2.0)),
                    )),
                    Box::new(Expr::Bool(true)),
                )),
                Box::new(Expr::Bool(false)),
            ),
        ),
        ("BLANK()", Expr::Call("BLANK".into(), vec![])),
        ("[a]]b]", Expr::Bracket("a]b".into())),
        ("1.5e2", Expr::Num(150.0)),
    ];
    for (src, want) in cases {
        assert_eq!(parse(src).unwrap(), want, "{src}");
    }
}

#[test]
fn syntax_errors_have_positions() {
    // (source, expected start of the error in characters)
    let cases = [
        ("SUM(Sales[Qty]", 14),
        ("1 +", 3),
        ("SUM(Sales[Qty]) 2", 16),
        ("\"open", 0),
        ("Sales[Qty", 5),
        ("1 # 2", 2),
        ("", 0),
        ("  = (1", 6),
        ("[ ]", 0),
    ];
    for (src, at) in cases {
        let e = parse(src).unwrap_err();
        assert_eq!(e.start, at, "{src}: {e:?}");
    }
}

#[test]
fn check_reports_unknown_names_and_cycles() {
    let (_, md) = loaded();
    assert!(check(&md, "SUM(Sales[Qty])", Kind::Measure, Some("New")).is_ok());
    let bad = [
        ("SUM(Sales[Nope])", "unknown column"),
        ("SUM(Nope[Qty])", "unknown table"),
        ("FOO(1)", "unknown function"),
        ("DIVIDE(1)", "wrong number"),
        ("[Missing]", "unknown measure"),
        ("[Total Qty] + [Self]", "itself"),
    ];
    for (src, msg) in bad {
        let e = check(&md, src, Kind::Measure, Some("Self")).unwrap_err();
        assert!(e.message.contains(msg), "{src}: {}", e.message);
    }
    // Indirectly: Revenue → Rev per Order → Revenue.
    let e = check(&md, "[Rev per Order] * 2", Kind::Measure, Some("Revenue")).unwrap_err();
    assert!(e.message.contains("itself"));
    // In a calculated column, [Qty] is a column of the row's table.
    let sales = md.table("Sales").unwrap();
    assert!(check(&md, "[Qty] * 2", Kind::Column(sales), None).is_ok());
}

// ---------------------------------------------------------------- evaluation

#[test]
fn literals_and_operators() {
    let (_, md) = loaded();
    let ev = Evaluator::new(&md);
    let ctx = Context::default();
    let cases: Vec<(&str, V)> = vec![
        ("1 + 2 * 3", V::Num(7.0)),
        ("10 / 4", V::Num(2.5)),
        ("1 / 0", V::Err(CellError::Div0)),
        ("DIVIDE(1, 0)", V::Blank),
        ("DIVIDE(1, 0, -1)", V::Num(-1.0)),
        ("DIVIDE(6, 3)", V::Num(2.0)),
        ("BLANK() + BLANK()", V::Blank),
        ("BLANK() + 2", V::Num(2.0)),
        ("BLANK() * 2", V::Blank),
        ("\"a\" & 1 & TRUE", V::Text("a1TRUE".into())),
        ("\"ABC\" = \"abc\"", V::Bool(true)),
        ("2 >= 3 || 1 < 2", V::Bool(true)),
        ("IF(1 > 2, \"y\", \"n\")", V::Text("n".into())),
        ("IF(FALSE, 1)", V::Blank),
        ("AND(TRUE, NOT(FALSE))", V::Bool(true)),
        ("ISBLANK(BLANK())", V::Bool(true)),
        ("ABS(-3)", V::Num(3.0)),
        ("ROUND(2.345, 2)", V::Num(2.35)),
        ("-(2 - 5)", V::Num(3.0)),
        ("\"x\" + 1", V::Err(CellError::Value)),
        ("Sales[Qty]", V::Err(CellError::Value)),
    ];
    for (src, want) in cases {
        let e = parse(src).unwrap();
        let got = ev.value(&e, &ctx);
        assert!(close(&got, &want), "{src}: {got:?} != {want:?}");
    }
}

#[test]
fn calculated_column_follows_the_relationship() {
    let (_, md) = loaded();
    let sales = md.table("Sales").unwrap();
    let amount = md.tables[sales].column("Amount").unwrap();
    for (i, j) in naive().iter().enumerate() {
        let want = j.product.map_or(0.0, |p| p.3 * j.qty);
        // An unmatched product reads RELATED as blank: qty * blank = blank.
        match &md.tables[sales].rows[i][amount] {
            CellValue::Number(n) => assert!((n - want).abs() < 1e-9, "row {i}"),
            CellValue::Empty => assert!(j.product.is_none(), "row {i}"),
            other => panic!("row {i}: {other:?}"),
        }
    }
}

/// Every measure in every context of (category, region) filters against the naive join.
#[test]
fn measures_in_filter_contexts() {
    let (_, md) = loaded();
    let ev = Evaluator::new(&md);
    let products = md.table("Products").unwrap();
    let sales = md.table("Sales").unwrap();
    let cat_col = md.tables[products].column("Category").unwrap();
    let reg_col = md.tables[sales].column("Region").unwrap();
    let cats: [Option<&str>; 5] = [None, Some("Office"), Some("Food"), Some("Home"), Some("")];
    let regions: [Option<&str>; 3] = [None, Some("North"), Some("South")];
    let all = naive();
    for c in cats {
        for r in regions {
            let mut ctx = Context::default();
            if let Some(c) = c {
                let v = if c.is_empty() {
                    CellValue::Empty
                } else {
                    text(c)
                };
                ctx.keep(products, cat_col, &[v]);
            }
            if let Some(r) = r {
                ctx.keep(sales, reg_col, &[text(r)]);
            }
            let rows: Vec<Joined> = all
                .iter()
                .filter(|j| c.is_none_or(|c| cat(j) == c))
                .filter(|j| r.is_none_or(|r| j.region == r))
                .cloned()
                .collect();
            // Products visible: filtered by category only (sales filters do not flow up).
            let prods: Vec<(f64, &str, &str, f64)> = PRODUCTS
                .iter()
                .filter(|p| c.is_none_or(|c| p.2 == c))
                .copied()
                .collect();
            for (mi, m) in md.def.measures.iter().enumerate() {
                let got = ev.measure(mi, &ctx);
                let want = expected(&m.name, &rows, &prods, c, r);
                assert!(
                    close(&got, &want),
                    "{} with category {c:?}, region {r:?}: {got:?} != {want:?}",
                    m.name
                );
            }
        }
    }
}

#[test]
fn inactive_relationship_and_userelationship() {
    let mut wb = workbook();
    let mut m = model();
    // A second, inactive relationship on OrderId → Products[Id].
    m.relationships.push(Relationship {
        from_table: "Sales".into(),
        from_column: "OrderId".into(),
        to_table: "Products".into(),
        to_column: "Id".into(),
        active: false,
    });
    m.measures.push(measure(
        "Qty by Order",
        "Sales",
        "CALCULATE([Total Qty], USERELATIONSHIP(Sales[OrderId], Products[Id]))",
    ));
    m.write(&mut wb);
    let md = ModelData::load(&wb);
    let ev = Evaluator::new(&md);
    let products = md.table("Products").unwrap();
    let cat_col = md.tables[products].column("Category").unwrap();
    let mut ctx = Context::default();
    ctx.keep(products, cat_col, &[text("Food")]);
    // Active path: products 3, 4 → orders 4, 5, 6 → 20 + 7 + 4.
    let tq = md.def.measure("Total Qty").unwrap();
    assert_eq!(ev.measure(tq, &ctx), V::Num(31.0));
    // Through OrderId: orders 3 and 4 are "Food" (ids 3, 4) → 2 + 20.
    let by_order = md.def.measure("Qty by Order").unwrap();
    let want: f64 = SALES
        .iter()
        .filter(|s| PRODUCTS.iter().any(|p| p.0 == s.0 && p.2 == "Food"))
        .map(|s| s.2)
        .sum();
    assert_eq!(ev.measure(by_order, &ctx), V::Num(want));
}

#[test]
fn excluded_values_and_circular_measures() {
    let mut wb = workbook();
    let mut m = model();
    m.measures.push(measure("Loop A", "Sales", "[Loop B] + 1"));
    m.measures.push(measure("Loop B", "Sales", "[Loop A] + 1"));
    m.write(&mut wb);
    let md = ModelData::load(&wb);
    let ev = Evaluator::new(&md);
    let a = md.def.measure("Loop A").unwrap();
    assert_eq!(ev.measure(a, &Context::default()), V::Err(CellError::Ref));
    let sales = md.table("Sales").unwrap();
    let reg = md.tables[sales].column("Region").unwrap();
    let mut ctx = Context::default();
    ctx.drop_values(sales, reg, &[text("north")]);
    let tq = md.def.measure("Total Qty").unwrap();
    let want: f64 = SALES.iter().filter(|s| s.3 != "North").map(|s| s.2).sum();
    assert_eq!(ev.measure(tq, &ctx), V::Num(want));
}

#[test]
fn cardinality_detection() {
    let (_, md) = loaded();
    let p = md.table("Products").unwrap();
    let s = md.table("Sales").unwrap();
    let col = |t: usize, c: &str| -> Vec<CellValue> {
        let ci = md.tables[t].column(c).unwrap();
        md.tables[t].rows.iter().map(|r| r[ci].clone()).collect()
    };
    let (card, unmatched) = detect(&col(s, "ProductId"), &col(p, "Id"));
    assert_eq!(card, Cardinality::ManyToOne);
    assert_eq!(unmatched, 1); // product 9
    assert_eq!(
        detect(&col(p, "Id"), &col(s, "ProductId")).0,
        Cardinality::OneToMany
    );
    assert_eq!(
        detect(&col(p, "Id"), &col(s, "OrderId")).0,
        Cardinality::OneToOne
    );
    assert_eq!(
        detect(&col(s, "Region"), &col(p, "Category")).0,
        Cardinality::ManyToMany
    );
}

// ---------------------------------------------------------------- persistence

#[test]
fn definitions_round_trip() {
    let mut m = model();
    m.measures.push(Measure {
        name: "Odd <name> & \"quotes\"".into(),
        table: "Sales".into(),
        expression: "IF(1 < 2,\n  \"a&b\", 'x'[y])".into(),
        format: Some("#,##0.00\" đ\"".into()),
    });
    m.pivots = vec![3, 7];
    let xml = to_xml(&m);
    assert!(is_defs_part(xml.as_bytes()));
    assert_eq!(parse_defs(xml.as_bytes()), m);
    // Through the workbook part, then an .xlsx file.
    let mut wb = workbook();
    m.write(&mut wb);
    assert_eq!(DataModel::read(&wb), m);
    let bytes = bo_xlsx::write_xlsx(&wb).unwrap();
    let back = bo_xlsx::read_xlsx(&bytes).unwrap();
    assert_eq!(DataModel::read(&back), m);
    // Writing an empty model removes the part and its relationship.
    let mut wb2 = back;
    DataModel::default().write(&mut wb2);
    assert!(DataModel::read(&wb2).is_empty());
    assert!(!wb2
        .opaque
        .workbook_rels
        .iter()
        .any(|r| r.rel_type.ends_with("/customXml")));
}

fn parse_defs(data: &[u8]) -> DataModel {
    super::parse(data)
}

// ---------------------------------------------------------------- pivot tables

fn labels() -> PivotLabels {
    PivotLabels::default()
}

fn field(wb: &Workbook, cache_id: u32, name: &str) -> u32 {
    wb.pivot_caches
        .iter()
        .find(|c| c.cache_id == cache_id)
        .unwrap()
        .fields
        .iter()
        .position(|f| f.name == name)
        .unwrap_or_else(|| panic!("no field {name}")) as u32
}

fn value_field(field: u32, function: PivotFunction) -> PivotDataField {
    PivotDataField {
        field,
        function,
        ..PivotDataField::default()
    }
}

/// Body cells of a pivot table by row: label of column 0 → values of the other columns.
fn report(wb: &Workbook, sheet: usize) -> Vec<(String, Vec<CellValue>)> {
    let pt = &wb.sheets[sheet].pivots[0];
    let loc = pt.location;
    let ws = &wb.sheets[sheet];
    (loc.first.row + pt.first_data_row..=loc.last.row)
        .map(|r| {
            let label = ws
                .cell(r, loc.first.col)
                .map(|c| crate::pivot::value_text(&c.value))
                .unwrap_or_default();
            let vals = (loc.first.col + pt.first_data_col..=loc.last.col)
                .map(|c| ws.cell(r, c).map_or(CellValue::Empty, |x| x.value.clone()))
                .collect();
            (label, vals)
        })
        .collect()
}

fn n(v: &CellValue) -> f64 {
    match v {
        CellValue::Number(x) => *x,
        CellValue::Empty => 0.0,
        other => panic!("{other:?}"),
    }
}

/// Rows from the dimension, values from the fact table and a measure: totals match a
/// naive join (unmatched sales under the blank item).
#[test]
fn multi_table_pivot_matches_a_naive_join() {
    let mut wb = workbook();
    model().write(&mut wb);
    let index = create_model_pivot(
        &mut wb,
        1,
        CellRef::new(0, 0),
        &PivotSpec::default(),
        &labels(),
    )
    .unwrap();
    let cache_id = wb.sheets[1].pivots[index].cache_id;
    assert!(super::pivot::is_model_cache(&wb, cache_id));
    let spec = PivotSpec {
        rows: vec![field(&wb, cache_id, "Products[Category]")],
        values: vec![
            value_field(field(&wb, cache_id, "Sales[Qty]"), PivotFunction::Sum),
            value_field(field(&wb, cache_id, "[Revenue]"), PivotFunction::Sum),
            value_field(field(&wb, cache_id, "[Rev per Order]"), PivotFunction::Sum),
            value_field(field(&wb, cache_id, "[All Qty]"), PivotFunction::Sum),
        ],
        layout: PivotLayout::Tabular,
        ..PivotSpec::default()
    };
    crate::pivot::update_pivot(&mut wb, 1, index, &spec, &labels()).unwrap();
    let rows = report(&wb, 1);
    let all = naive();
    // Categories in item order (text ascending), blank last, then the grand total.
    let mut cats: Vec<&str> = all.iter().map(cat).collect();
    cats.sort_by_key(|c| (c.is_empty(), c.to_lowercase()));
    cats.dedup();
    assert_eq!(rows.len(), cats.len() + 1, "{rows:?}");
    let total_qty: f64 = all.iter().map(|j| j.qty).sum();
    for (k, c) in cats.iter().enumerate() {
        let sub: Vec<&Joined> = all.iter().filter(|j| cat(j) == *c).collect();
        let qty: f64 = sub.iter().map(|j| j.qty).sum();
        let rev: f64 = sub.iter().map(|j| amount(j)).sum();
        let (label, vals) = &rows[k];
        let want_label = if c.is_empty() { "(blank)" } else { c };
        assert_eq!(label, want_label);
        assert!((n(&vals[0]) - qty).abs() < 1e-9, "{c} qty");
        assert!((n(&vals[1]) - rev).abs() < 1e-9, "{c} revenue");
        assert!(
            (n(&vals[2]) - rev / sub.len() as f64).abs() < 1e-9,
            "{c} per order"
        );
        assert!((n(&vals[3]) - total_qty).abs() < 1e-9, "{c} all");
    }
    let (label, vals) = rows.last().unwrap();
    assert_eq!(label, "Grand Total");
    let total_rev: f64 = all.iter().map(amount).sum();
    assert!((n(&vals[0]) - total_qty).abs() < 1e-9);
    assert!((n(&vals[1]) - total_rev).abs() < 1e-9);
    assert!((n(&vals[2]) - total_rev / all.len() as f64).abs() < 1e-9);
    // Captions of measure value fields are the measure names.
    let pt = &wb.sheets[1].pivots[index];
    assert_eq!(pt.values[1].name.as_deref(), Some("Revenue"));
}

#[test]
fn pivot_columns_filters_and_hidden_items() {
    let mut wb = workbook();
    model().write(&mut wb);
    let index = create_model_pivot(
        &mut wb,
        1,
        CellRef::new(0, 0),
        &PivotSpec::default(),
        &labels(),
    )
    .unwrap();
    let cid = wb.sheets[1].pivots[index].cache_id;
    let cat_f = field(&wb, cid, "Products[Category]");
    let reg_f = field(&wb, cid, "Sales[Region]");
    let mut spec = PivotSpec {
        rows: vec![cat_f],
        cols: vec![reg_f],
        values: vec![value_field(
            field(&wb, cid, "[North Qty]"),
            PivotFunction::Sum,
        )],
        layout: PivotLayout::Tabular,
        ..PivotSpec::default()
    };
    // Hide the Food category.
    spec.fields = vec![bo_model::sheet::PivotFieldOptions::default(); cat_f as usize + 1];
    spec.fields[cat_f as usize].hidden = vec![text("Food")];
    crate::pivot::update_pivot(&mut wb, 1, index, &spec, &labels()).unwrap();
    let rows = report(&wb, 1);
    assert!(rows.iter().all(|(l, _)| l != "Food"), "{rows:?}");
    // Office: North column = 10 + 2; South column: CALCULATE replaces the region filter
    // with North, so it is also 12 (the measure ignores the column's region).
    let office = rows.iter().find(|(l, _)| l == "Office").unwrap();
    let north_office: f64 = naive()
        .iter()
        .filter(|j| cat(j) == "Office" && j.region == "North")
        .map(|j| j.qty)
        .sum();
    assert!((n(&office.1[0]) - north_office).abs() < 1e-9, "{office:?}");
    assert!((n(&office.1[1]) - north_office).abs() < 1e-9, "{office:?}");
    // Grand total: hidden Food left out.
    let grand = rows.last().unwrap();
    let want: f64 = naive()
        .iter()
        .filter(|j| cat(j) != "Food" && j.region == "North")
        .map(|j| j.qty)
        .sum();
    assert!(
        (n(grand.1.last().unwrap()) - want).abs() < 1e-9,
        "{grand:?}"
    );
}

#[test]
fn model_pivot_survives_a_file_round_trip_and_refresh() {
    let mut wb = workbook();
    model().write(&mut wb);
    let index = create_model_pivot(
        &mut wb,
        1,
        CellRef::new(0, 0),
        &PivotSpec::default(),
        &labels(),
    )
    .unwrap();
    let cid = wb.sheets[1].pivots[index].cache_id;
    let spec = PivotSpec {
        rows: vec![field(&wb, cid, "Products[Name]")],
        values: vec![value_field(
            field(&wb, cid, "[Total Qty]"),
            PivotFunction::Sum,
        )],
        ..PivotSpec::default()
    };
    crate::pivot::update_pivot(&mut wb, 1, index, &spec, &labels()).unwrap();
    let before = report(&wb, 1);
    let bytes = bo_xlsx::write_xlsx(&wb).unwrap();
    // The cache is written without refreshOnLoad, with its records.
    let pkg = bo_opc_parts(&bytes);
    let def = pkg
        .iter()
        .find(|(n, _)| n.contains("pivotCacheDefinition"))
        .map(|(_, d)| String::from_utf8_lossy(d).into_owned())
        .unwrap();
    assert!(!def.contains("refreshOnLoad"), "{def}");
    assert!(def.contains("Products[Name]"));
    let mut back = bo_xlsx::read_xlsx(&bytes).unwrap();
    assert_eq!(report(&back, 1), before);
    // Refreshing rebuilds the cache from the model.
    crate::pivot::refresh_all(&mut back, &labels()).unwrap();
    assert_eq!(report(&back, 1), before);
    assert!(back.pivot_caches[0].model);
    // Grouping is refused over the model.
    let err = crate::pivot::group_field(
        &mut back,
        1,
        0,
        0,
        &crate::pivot::GroupRequest::Numbers {
            start: None,
            end: None,
            interval: 1.0,
        },
        &labels(),
    );
    assert!(err.is_err());
}

/// Names and bytes of the parts of a package.
fn bo_opc_parts(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
    pkg.parts()
        .iter()
        .map(|p| (p.name.clone(), p.data.clone()))
        .collect()
}

#[test]
fn field_list_has_every_column_and_measure() {
    let (_, md) = loaded();
    let list = field_list(&md);
    let names: Vec<&str> = list.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        &names[..4],
        &[
            "Products[Id]",
            "Products[Name]",
            "Products[Category]",
            "Products[Price]"
        ]
    );
    assert!(names.contains(&"Sales[Amount]"));
    assert!(list
        .iter()
        .any(|(n, k)| n == "[Revenue]" && matches!(k, FieldKind::Measure(_))));
}

#[test]
fn foreign_model_parts_are_listed() {
    let mut wb = workbook();
    wb.opaque.parts.push(bo_model::sheet::OpaquePart {
        name: "/xl/model/item.data".into(),
        content_type: None,
        data: vec![1, 2, 3, 4],
    });
    wb.opaque.parts.push(bo_model::sheet::OpaquePart {
        name: "/xl/connections.xml".into(),
        content_type: None,
        data: br#"<connections><connection id="1" name="ThisWorkbookDataModel" type="5" model="1"/></connections>"#.to_vec(),
    });
    let list = foreign_parts(&wb);
    assert_eq!(list.len(), 2, "{list:?}");
    assert_eq!(list[0].kind, "model");
    assert_eq!(list[0].size, 4);
    assert_eq!(list[1].name, "ThisWorkbookDataModel");
}
