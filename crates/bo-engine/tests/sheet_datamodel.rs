//! The workbook data model in BlueSheet (SHEET-DM-001) through the command bridge: tables,
//! relationships (cardinality detection), measures and calculated columns, pivot tables over
//! the model (rows from a dimension, values from the fact table and measures), table slicers
//! filtering them, undo, the definitions part in the file and the data model parts of other
//! software kept byte for byte.
//!
//! `BO_WRITE_CORPUS=1 cargo test -p bo-engine --test sheet_datamodel` writes
//! `tests/corpus/sheet/datamodel-{1,2,3}.xlsx`.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used, clippy::needless_pass_by_value)]

use bo_engine::SheetEngine;
use serde_json::{json, Value};
use std::path::PathBuf;

fn exec(e: &mut SheetEngine, v: &Value) -> bo_engine::Result<Value> {
    e.exec_json(&v.to_string())
        .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null))
}

fn run(e: &mut SheetEngine, v: Value) -> Value {
    exec(e, &v).unwrap_or_else(|err| panic!("{v}: {err}"))
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/sheet")
        .join(name)
}

fn shown(e: &SheetEngine, sheet: usize, row: u32, col: u32) -> String {
    e.cell(sheet, row, col).unwrap().display
}

/// Products (A1:C6), Regions (E1:F3) and Sales (H1:K11) on sheet 0, each made a table.
const PRODUCTS: [(u32, &str, &str); 5] = [
    (1, "Bút", "Văn phòng"),
    (2, "Sổ", "Văn phòng"),
    (3, "Táo", "Thực phẩm"),
    (4, "Bánh", "Thực phẩm"),
    (5, "Đèn", "Gia dụng"),
];
const REGIONS: [(&str, &str); 2] = [("N", "Bắc"), ("S", "Nam")];
/// (Order, Product, Region, Qty)
const SALES: [(u32, u32, &str, f64); 10] = [
    (1, 1, "N", 10.0),
    (2, 1, "S", 5.0),
    (3, 2, "N", 2.0),
    (4, 3, "S", 20.0),
    (5, 3, "N", 7.0),
    (6, 4, "N", 4.0),
    (7, 2, "S", 3.0),
    (8, 4, "S", 6.0),
    (9, 1, "N", 1.0),
    (10, 3, "S", 9.0),
];

fn setup(e: &mut SheetEngine) -> (String, String, String) {
    e.new_workbook("dm.xlsx").unwrap();
    let mut cells: Vec<(u32, u32, String)> = Vec::new();
    for (c, h) in ["Mã", "Tên", "Nhóm"].iter().enumerate() {
        cells.push((0, c as u32, (*h).into()));
    }
    for (i, (id, n, g)) in PRODUCTS.iter().enumerate() {
        let r = i as u32 + 1;
        cells.push((r, 0, id.to_string()));
        cells.push((r, 1, (*n).into()));
        cells.push((r, 2, (*g).into()));
    }
    for (c, h) in ["Mã vùng", "Vùng"].iter().enumerate() {
        cells.push((0, 4 + c as u32, (*h).into()));
    }
    for (i, (k, n)) in REGIONS.iter().enumerate() {
        cells.push((i as u32 + 1, 4, (*k).into()));
        cells.push((i as u32 + 1, 5, (*n).into()));
    }
    for (c, h) in ["Đơn", "Mã SP", "Mã vùng", "SL"].iter().enumerate() {
        cells.push((0, 7 + c as u32, (*h).into()));
    }
    for (i, (o, p, r, q)) in SALES.iter().enumerate() {
        let row = i as u32 + 1;
        cells.push((row, 7, o.to_string()));
        cells.push((row, 8, p.to_string()));
        cells.push((row, 9, (*r).into()));
        cells.push((row, 10, q.to_string()));
    }
    e.set_inputs(0, &cells).unwrap();
    for (c0, c1, r1) in [(0, 2, 5), (4, 5, 2), (7, 10, 10)] {
        run(
            e,
            json!({"cmd": "createTable", "sheet": 0, "range": {"r0": 0, "c0": c0, "r1": r1, "c1": c1}}),
        );
    }
    let info = run(e, json!({"cmd": "modelInfo"}));
    let names: Vec<String> = info["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names.len(), 3, "{info}");
    let mut it = names.into_iter();
    let (p, r, s) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
    for n in [&p, &r, &s] {
        run(e, json!({"cmd": "modelAddTable", "name": n}));
    }
    (p, r, s)
}

fn relate(e: &mut SheetEngine, a: (&str, &str), b: (&str, &str)) -> Value {
    run(
        e,
        json!({"cmd": "modelSetRelationship", "fromTable": a.0, "fromColumn": a.1,
               "toTable": b.0, "toColumn": b.1}),
    )
}

fn measure(e: &mut SheetEngine, name: &str, table: &str, expr: &str) {
    run(
        e,
        json!({"cmd": "modelSetMeasure", "name": name, "table": table, "expression": expr}),
    );
}

/// Index of a field of pivot 0 on `sheet` by name.
fn field(e: &mut SheetEngine, sheet: usize, name: &str) -> usize {
    let p = run(e, json!({"cmd": "pivots", "sheet": sheet}));
    p[0]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .position(|f| f["name"] == name)
        .unwrap_or_else(|| panic!("no field {name}: {p}"))
}

/// Body of pivot 0 on `sheet` as shown text, row by row.
fn body(e: &mut SheetEngine, sheet: usize) -> Vec<Vec<String>> {
    let p = run(e, json!({"cmd": "pivots", "sheet": sheet}));
    let loc = &p[0]["location"];
    let (r0, c0, r1, c1) = (
        loc["r0"].as_u64().unwrap() as u32,
        loc["c0"].as_u64().unwrap() as u32,
        loc["r1"].as_u64().unwrap() as u32,
        loc["c1"].as_u64().unwrap() as u32,
    );
    (r0..=r1)
        .map(|r| (c0..=c1).map(|c| shown(e, sheet, r, c)).collect())
        .collect()
}

/// Expected quantity of `group` (all groups: `None`) in `region` (all: `None`) by a nested
/// loop over the source rows.
fn qty(group: Option<&str>, region: Option<&str>) -> f64 {
    SALES
        .iter()
        .filter(|s| {
            group.is_none_or(|g| PRODUCTS.iter().any(|p| p.0 == s.1 && p.2 == g))
                && region.is_none_or(|r| s.2 == r)
        })
        .map(|s| s.3)
        .sum()
}

fn fmt(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{n:.0}")
    } else {
        format!("{n}")
    }
}

fn build_first(e: &mut SheetEngine) -> (String, String, String) {
    let (p, r, s) = setup(e);
    // Detected one-to-many, stored many-to-one.
    let d = run(
        e,
        json!({"cmd": "modelDetect", "fromTable": p, "fromColumn": "Mã", "toTable": s, "toColumn": "Mã SP"}),
    );
    assert_eq!(d["cardinality"], "oneToMany");
    let info = relate(e, (&p, "Mã"), (&s, "Mã SP"));
    assert_eq!(info["relationships"][0]["fromTable"], json!(s));
    relate(e, (&s, "Mã vùng"), (&r, "Mã vùng"));
    measure(e, "Tổng SL", &s, &format!("SUM({s}[SL])"));
    measure(e, "Số đơn", &s, &format!("COUNTROWS({s})"));
    measure(e, "SL mỗi đơn", &s, "DIVIDE([Tổng SL], [Số đơn], 0)");
    (p, r, s)
}

#[test]
fn model_pivot_by_dimension_with_measures_and_undo() {
    let mut e = SheetEngine::new();
    let (p, _, s) = build_first(&mut e);
    // Errors: syntax with a position, unknown names, many-to-many.
    let c = run(
        &mut e,
        json!({"cmd": "modelCheck", "expression": "SUM(Sales[SL]"}),
    );
    assert_eq!(c["ok"], false);
    assert!(c["error"]["start"].as_u64().is_some());
    let c = run(
        &mut e,
        json!({"cmd": "modelCheck", "expression": "[Không có]"}),
    );
    assert!(c["error"]["message"].as_str().unwrap().contains("unknown"));
    assert!(exec(
        &mut e,
        &json!({"cmd": "modelSetRelationship", "fromTable": p, "fromColumn": "Nhóm", "toTable": s, "toColumn": "Mã vùng"})
    )
    .is_err());
    let v = run(
        &mut e,
        json!({"cmd": "modelEvaluate", "expression": "[Tổng SL]"}),
    );
    assert_eq!(v["value"], json!(qty(None, None)));

    let out = run(
        &mut e,
        json!({"cmd": "modelPivotCreate", "dest": {"sheet": 0, "row": 14, "col": 0}}),
    );
    assert_eq!(out["index"], 0);
    let pivots = run(&mut e, json!({"cmd": "pivots", "sheet": 0}));
    assert_eq!(pivots[0]["model"], true);
    let group = field(&mut e, 0, &format!("{p}[Nhóm]"));
    let total = field(&mut e, 0, "[Tổng SL]");
    let per = field(&mut e, 0, "[SL mỗi đơn]");
    let plain = field(&mut e, 0, &format!("{s}[SL]"));
    let f = &run(&mut e, json!({"cmd": "pivots", "sheet": 0}))[0]["fields"];
    assert_eq!(f[total]["measure"], true);
    assert_eq!(f[group]["table"], json!(p));
    run(
        &mut e,
        json!({"cmd": "pivotUpdate", "sheet": 0, "index": 0, "rows": [group],
               "values": [{"field": total}, {"field": plain, "function": "sum"}, {"field": per}],
               "settings": {"layout": "tabular"}}),
    );
    let b = body(&mut e, 0);
    // Header, the groups with sales (text order; a group without sales has no fact rows),
    // grand total.
    let mut groups: Vec<&str> = PRODUCTS
        .iter()
        .map(|p| p.2)
        .filter(|g| qty(Some(g), None) > 0.0)
        .collect();
    groups.sort_by_key(|g| g.to_lowercase());
    groups.dedup();
    assert_eq!(b.len(), groups.len() + 2, "{b:?}");
    assert_eq!(b[0][0], "Nhóm");
    assert_eq!(b[0][1], "Tổng SL");
    for (k, g) in groups.iter().enumerate() {
        let row = &b[k + 1];
        assert_eq!(row[0], *g);
        let q = qty(Some(g), None);
        assert_eq!(row[1], fmt(q), "{g}");
        assert_eq!(row[2], fmt(q), "{g}");
    }
    let last = b.last().unwrap();
    assert_eq!(last[1], fmt(qty(None, None)));
    assert_eq!(last[2], fmt(qty(None, None)));
    // Undo of a model change restores the report.
    let before = body(&mut e, 0);
    run(
        &mut e,
        json!({"cmd": "modelSetMeasure", "original": "Tổng SL", "name": "Tổng SL", "table": s,
               "expression": format!("SUM({s}[SL]) * 10")}),
    );
    assert_eq!(
        body(&mut e, 0).last().unwrap()[1],
        fmt(qty(None, None) * 10.0)
    );
    e.undo().unwrap();
    assert_eq!(body(&mut e, 0), before);
    // Round trip: definitions, report and refresh.
    let bytes = e.save("xlsx", 0).unwrap();
    let mut f = SheetEngine::new();
    f.open(&bytes, "dm.xlsx").unwrap();
    let info = run(&mut f, json!({"cmd": "modelInfo"}));
    assert_eq!(info["measures"].as_array().unwrap().len(), 3);
    assert_eq!(info["relationships"].as_array().unwrap().len(), 2);
    assert_eq!(body(&mut f, 0), before);
    run(&mut f, json!({"cmd": "pivotRefreshAll"}));
    assert_eq!(body(&mut f, 0), before);
    if std::env::var_os("BO_WRITE_CORPUS").is_some() {
        std::fs::write(corpus("datamodel-1.xlsx"), &bytes).unwrap();
    }
}

#[test]
fn model_pivot_rows_and_columns_with_calculated_column() {
    let mut e = SheetEngine::new();
    let (p, r, s) = build_first(&mut e);
    run(
        &mut e,
        json!({"cmd": "modelSetColumn", "table": s, "name": "Nhóm SP",
               "expression": format!("RELATED({p}[Nhóm])")}),
    );
    measure(
        &mut e,
        "SL Bắc",
        &s,
        &format!("CALCULATE([Tổng SL], {r}[Vùng] = \"Bắc\")"),
    );
    run(&mut e, json!({"cmd": "modelPivotCreate"}));
    let sheet = 1;
    let group = field(&mut e, sheet, &format!("{s}[Nhóm SP]"));
    let region = field(&mut e, sheet, &format!("{r}[Vùng]"));
    let total = field(&mut e, sheet, "[Tổng SL]");
    run(
        &mut e,
        json!({"cmd": "pivotUpdate", "sheet": sheet, "index": 0, "rows": [group], "cols": [region],
               "values": [{"field": total}], "settings": {"layout": "tabular"}}),
    );
    let b = body(&mut e, sheet);
    // Header rows, groups with sales, grand total; columns: label, Bắc, Nam, total.
    let header = b
        .iter()
        .position(|row| row.iter().any(|c| c == "Bắc"))
        .unwrap();
    let bac = b[header].iter().position(|c| c == "Bắc").unwrap();
    let nam = b[header].iter().position(|c| c == "Nam").unwrap();
    for row in &b[header + 1..b.len() - 1] {
        let g = row[0].as_str();
        assert_eq!(row[bac], fmt(qty(Some(g), Some("N"))), "{g}");
        assert_eq!(row[nam], fmt(qty(Some(g), Some("S"))), "{g}");
        assert_eq!(row.last().unwrap(), &fmt(qty(Some(g), None)), "{g}");
    }
    let grand = b.last().unwrap();
    assert_eq!(grand.last().unwrap(), &fmt(qty(None, None)));
    if std::env::var_os("BO_WRITE_CORPUS").is_some() {
        std::fs::write(corpus("datamodel-2.xlsx"), e.save("xlsx", 0).unwrap()).unwrap();
    }
}

#[test]
fn table_slicer_filters_model_pivots() {
    let mut e = SheetEngine::new();
    let (p, _, s) = build_first(&mut e);
    run(
        &mut e,
        json!({"cmd": "modelPivotCreate", "dest": {"sheet": 0, "row": 14, "col": 0}}),
    );
    let name = field(&mut e, 0, &format!("{p}[Tên]"));
    let total = field(&mut e, 0, "[Tổng SL]");
    run(
        &mut e,
        json!({"cmd": "pivotUpdate", "sheet": 0, "index": 0, "rows": [name],
               "values": [{"field": total}], "settings": {"layout": "tabular"}}),
    );
    // A slicer on Products[Nhóm] keeping "Thực phẩm".
    run(
        &mut e,
        json!({"cmd": "insertSlicer", "sheet": 0, "row": 1, "col": 2}),
    );
    run(
        &mut e,
        json!({"cmd": "slicerSelect", "sheet": 0, "index": 0, "items": ["Thực phẩm"]}),
    );
    let b = body(&mut e, 0);
    let names: Vec<&str> = b[1..b.len() - 1].iter().map(|r| r[0].as_str()).collect();
    assert_eq!(names, ["Bánh", "Táo"], "{b:?}");
    assert_eq!(b.last().unwrap()[1], fmt(qty(Some("Thực phẩm"), None)));
    // An inactive relationship used by one measure (orders by their number as product).
    relate(&mut e, (&s, "Đơn"), (&p, "Mã"));
    let info = run(&mut e, json!({"cmd": "modelInfo"}));
    let rels = info["relationships"].as_array().unwrap();
    assert_eq!(rels.len(), 3);
    // The new relationship between Sales and Products is active; the old one is not.
    assert_eq!(rels[0]["active"], false);
    assert_eq!(rels[2]["active"], true);
    run(
        &mut e,
        json!({"cmd": "modelSetRelationship", "index": 2, "fromTable": s, "fromColumn": "Đơn",
               "toTable": p, "toColumn": "Mã", "active": false}),
    );
    run(
        &mut e,
        json!({"cmd": "modelSetRelationship", "index": 0, "fromTable": s, "fromColumn": "Mã SP",
               "toTable": p, "toColumn": "Mã", "active": true}),
    );
    measure(
        &mut e,
        "SL theo đơn",
        &s,
        &format!("CALCULATE([Tổng SL], USERELATIONSHIP({s}[Đơn], {p}[Mã]))"),
    );
    let v = run(
        &mut e,
        json!({"cmd": "modelEvaluate", "expression": "[SL theo đơn]"}),
    );
    // Without filters, every sale counts once whichever relationship is used.
    assert_eq!(v["value"], json!(qty(None, None)));
    if std::env::var_os("BO_WRITE_CORPUS").is_some() {
        std::fs::write(corpus("datamodel-3.xlsx"), e.save("xlsx", 0).unwrap()).unwrap();
    }
}

#[test]
fn foreign_data_model_parts_are_kept_byte_for_byte() {
    let mut e = SheetEngine::new();
    build_first(&mut e);
    let bytes = e.save("xlsx", 0).unwrap();
    // Add the parts another application writes for its own data model.
    let mut pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
    let model: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
    pkg.add_part(
        "/xl/model/item.data",
        "application/vnd.openxmlformats-officedocument.model+data",
        model.clone(),
    )
    .unwrap();
    let conns = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<connections xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><connection id="1" keepAlive="1" name="ThisWorkbookDataModel" description="Data Model" type="5" refreshedVersion="6" minRefreshableVersion="5" background="1"><dbPr connection="Data Model Connection" command="Model" commandType="1"/></connection></connections>"#.to_vec();
    pkg.add_part(
        "/xl/connections.xml",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.connections+xml",
        conns.clone(),
    )
    .unwrap();
    let rels_name = "/xl/_rels/workbook.xml.rels";
    let rels = String::from_utf8(pkg.part(rels_name).unwrap().data.clone()).unwrap();
    let rels = rels.replace(
        "</Relationships>",
        "<Relationship Id=\"rIdModel9\" Type=\"http://schemas.microsoft.com/office/2006/relationships/model\" Target=\"model/item.data\"/><Relationship Id=\"rIdConn9\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/connections\" Target=\"connections.xml\"/></Relationships>",
    );
    pkg.put_part(rels_name, rels.into_bytes());
    let with_model = pkg.write().unwrap();
    let mut f = SheetEngine::new();
    f.open(&with_model, "dm.xlsx").unwrap();
    let info = run(&mut f, json!({"cmd": "modelInfo"}));
    let foreign = info["foreign"].as_array().unwrap();
    assert!(
        foreign
            .iter()
            .any(|x| x["kind"] == "model" && x["size"] == 4096),
        "{info}"
    );
    assert!(
        foreign.iter().any(|x| x["name"] == "ThisWorkbookDataModel"),
        "{info}"
    );
    // Our own model still works and both are saved side by side.
    let s = info["tables"][2]["name"].as_str().unwrap().to_owned();
    measure(&mut f, "Lớn nhất", &s, &format!("MAX({s}[SL])"));
    let out = f.save("xlsx", 0).unwrap();
    let back = bo_opc::Package::read(&out, &bo_opc::Limits::default()).unwrap();
    assert_eq!(back.part("/xl/model/item.data").unwrap().data, model);
    let c = String::from_utf8_lossy(&back.part("/xl/connections.xml").unwrap().data).into_owned();
    assert!(c.contains("ThisWorkbookDataModel"), "{c}");
    let mut g = SheetEngine::new();
    g.open(&out, "dm.xlsx").unwrap();
    let info = run(&mut g, json!({"cmd": "modelInfo"}));
    assert_eq!(info["measures"].as_array().unwrap().len(), 4);
}
