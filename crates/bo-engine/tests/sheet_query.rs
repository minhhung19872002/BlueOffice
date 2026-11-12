//! Get & Transform data in BlueSheet (SHEET-PQ-001) through the command bridge: sources the
//! host provides, preview, Close & Load as a table, refresh (one undo step), the definitions
//! kept in the file with the standard connection / query table parts, and [MS-QDEFF]
//! DataMashup parts of other software kept byte for byte.
//!
//! `BO_WRITE_CORPUS=1 cargo test -p bo-engine --test sheet_query` writes
//! `tests/corpus/sheet/query-{1,2,3}.xlsx`.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::SheetEngine;
use serde_json::{json, Value};
use std::io::Write as _;
use std::path::PathBuf;

fn exec(e: &mut SheetEngine, v: &Value) -> bo_engine::Result<Value> {
    e.exec_json(&v.to_string())
        .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null))
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/sheet")
        .join(name)
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(corpus("query").join(name)).unwrap()
}

fn b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn provide(e: &mut SheetEngine, key: &str, bytes: &[u8]) {
    exec(
        e,
        &json!({"cmd": "querySource", "key": key, "access": "file",
                "files": [{"name": key, "data": b64(bytes)}]}),
    )
    .unwrap();
}

fn shown(e: &SheetEngine, sheet: usize, a1: &str) -> String {
    let c = bo_model::sheet::CellRef::parse(a1).unwrap();
    e.cell(sheet, c.row, c.col).unwrap().display
}

fn input(e: &SheetEngine, sheet: usize, a1: &str) -> String {
    let c = bo_model::sheet::CellRef::parse(a1).unwrap();
    e.cell(sheet, c.row, c.col).unwrap().input
}

/// The sales query: CSV, the suggested steps, then a filter and a removed column.
fn sales_query(e: &mut SheetEngine) -> Value {
    let r = exec(
        e,
        &json!({"cmd": "queryInspect", "source": {"kind": "csv", "file": "sales.csv"}}),
    )
    .unwrap();
    assert_eq!(r["csv"]["delimiter"], ";");
    assert_eq!(r["csv"]["encoding"], "utf-8");
    let mut steps = r["suggested"].as_array().unwrap().clone();
    assert_eq!(steps[0]["op"], "promoteHeaders");
    steps.push(json!({"op": "filterRows", "column": "Khu vực", "condition": "notEquals", "value": "Trung"}));
    steps.push(json!({"op": "removeColumns", "columns": ["Đơn giá"]}));
    json!({"name": "Doanh số", "source": {"kind": "csv", "file": "sales.csv"}, "steps": steps})
}

fn wb_parts(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut v = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut v).unwrap();
            (f.name().unwrap().into_owned(), v)
        })
        .collect()
}

fn part(parts: &[(String, Vec<u8>)], name: &str) -> Option<String> {
    parts
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, d)| String::from_utf8_lossy(d).into_owned())
}

#[test]
fn csv_query_load_refresh_undo_and_round_trip() {
    let mut e = SheetEngine::new();
    e.new_workbook("q.xlsx").unwrap();
    // Without the bytes, the source is missing.
    let q = json!({"name": "X", "source": {"kind": "csv", "file": "sales.csv"}, "steps": []});
    let r = exec(&mut e, &json!({"cmd": "queryPreview", "query": q})).unwrap();
    assert_eq!(r["error"]["code"], "missingSource");
    assert_eq!(r["error"]["missing"], "sales.csv");
    provide(&mut e, "sales.csv", &fixture("sales.csv"));
    let q = sales_query(&mut e);
    let r = exec(
        &mut e,
        &json!({"cmd": "queryPreview", "query": q, "upto": 1}),
    )
    .unwrap();
    assert_eq!(r["rowCount"], 5);
    let r = exec(&mut e, &json!({"cmd": "queryPreview", "query": q})).unwrap();
    assert_eq!(r["rowCount"], 4);
    assert_eq!(
        r["columns"],
        json!(["Mã", "Sản phẩm", "Khu vực", "Số lượng", "Ngày"])
    );
    assert_eq!(r["types"][3], "integer");
    assert_eq!(r["rows"][0][3], 10.0);

    // Close & Load: a new sheet with a table, one undo step.
    let depth = e.history_len();
    let r = exec(
        &mut e,
        &json!({"cmd": "querySave", "query": q, "load": "newSheet"}),
    )
    .unwrap();
    assert_eq!(e.history_len(), depth + 1);
    assert_eq!(r["table"], "Doanh_số");
    assert_eq!(r["rows"], 4);
    let s = r["sheet"].as_u64().unwrap() as usize;
    assert_eq!(e.info().unwrap().sheets[s].name, "Doanh số");
    assert_eq!(shown(&e, s, "A1"), "Mã");
    assert_eq!(shown(&e, s, "B4"), "Cam; loại 1");
    assert_eq!(shown(&e, s, "D2"), "10");
    assert_eq!(shown(&e, s, "E2"), "05/01/2024");
    assert_eq!(shown(&e, s, "A6"), "");
    // Formulas can use the loaded table.
    e.set_input(0, 0, 0, "=SUM(Doanh_số[Số lượng])").unwrap();
    assert_eq!(shown(&e, 0, "A1"), "24");
    let list = exec(&mut e, &json!({"cmd": "queryList"})).unwrap();
    assert_eq!(list["queries"][0]["name"], "Doanh số");
    assert_eq!(list["queries"][0]["table"], "Doanh_số");
    assert_eq!(list["queries"][0]["rows"], 4);
    assert_eq!(list["queries"][0]["available"], true);
    // A second query with the same name is refused.
    assert!(exec(&mut e, &json!({"cmd": "querySave", "query": q})).is_err());

    // The source changes: refresh rewrites the table (one undo step), formulas follow.
    let changed = String::from_utf8(fixture("sales.csv"))
        .unwrap()
        .replace("A02;Lê;Nam;4", "A02;Lê;Nam;40")
        + "A05;Xoài;Nam;6;9;2024-03-01\r\n";
    provide(&mut e, "sales.csv", changed.as_bytes());
    let depth = e.history_len();
    let r = exec(&mut e, &json!({"cmd": "queryRefresh"})).unwrap();
    assert_eq!(r["refreshed"][0]["rows"], 5);
    assert_eq!(e.history_len(), depth + 1);
    assert_eq!(shown(&e, s, "D3"), "40");
    assert_eq!(shown(&e, s, "A6"), "A05");
    assert_eq!(shown(&e, 0, "A1"), "66");
    assert!(e.undo().unwrap());
    assert_eq!(shown(&e, s, "D3"), "4");
    assert_eq!(shown(&e, s, "A6"), "");
    assert_eq!(shown(&e, 0, "A1"), "24");
    assert!(e.redo().unwrap());
    assert_eq!(shown(&e, s, "A6"), "A05");

    // Save → reopen: the definition, the table and the standard parts are there.
    let bytes = e.save("xlsx", 0).unwrap();
    let parts = wb_parts(&bytes);
    let conns = part(&parts, "xl/connections.xml").unwrap();
    assert!(conns.contains("type=\"6\"") && conns.contains("sourceFile=\"sales.csv\""));
    assert!(conns.contains("semicolon=\"1\"") || conns.contains("delimiter="));
    let qt = parts
        .iter()
        .find(|(n, _)| n.starts_with("xl/queryTables/"))
        .unwrap();
    assert!(String::from_utf8_lossy(&qt.1).contains("connectionId=\"1\""));
    let table = parts
        .iter()
        .find(|(n, _)| n.starts_with("xl/tables/table") && n.ends_with(".xml"))
        .map(|(_, d)| String::from_utf8_lossy(d).into_owned())
        .unwrap();
    assert!(table.contains("tableType=\"queryTable\""));
    assert!(table.contains("queryTableFieldId=\"5\""));
    assert!(parts.iter().any(|(n, _)| n.starts_with("xl/tables/_rels/")));
    let defs = parts
        .iter()
        .find(|(n, d)| {
            n.starts_with("customXml/item")
                && String::from_utf8_lossy(d).contains("blueoffice/2026/queries")
        })
        .unwrap();
    assert!(String::from_utf8_lossy(&defs.1).contains("Doanh số"));
    // Saving again writes the same parts (BlueOffice's own elements are replaced, not added).
    let again = e.save("xlsx", 0).unwrap();
    let parts2 = wb_parts(&again);
    assert_eq!(part(&parts2, "xl/connections.xml").unwrap(), conns);
    assert_eq!(
        parts2
            .iter()
            .filter(|(n, _)| n.starts_with("xl/queryTables/"))
            .count(),
        1
    );

    let mut f = SheetEngine::new();
    f.open(&bytes, "q.xlsx").unwrap();
    let list = exec(&mut f, &json!({"cmd": "queryList"})).unwrap();
    assert_eq!(list["queries"][0]["name"], "Doanh số");
    assert_eq!(list["queries"][0]["available"], false);
    // Refresh without the source: reported, nothing changes.
    let r = exec(&mut f, &json!({"cmd": "queryRefresh"})).unwrap();
    assert_eq!(r["errors"][0]["missing"], "sales.csv");
    assert_eq!(r["refreshed"], json!([]));
    // With the same source: the same table.
    let before: Vec<String> = ["A1", "A6", "D3", "E6", "C5"]
        .iter()
        .map(|a| shown(&f, s, a))
        .collect();
    provide(&mut f, "sales.csv", changed.as_bytes());
    exec(
        &mut f,
        &json!({"cmd": "queryRefresh", "names": ["doanh số"]}),
    )
    .unwrap();
    let after: Vec<String> = ["A1", "A6", "D3", "E6", "C5"]
        .iter()
        .map(|a| shown(&f, s, a))
        .collect();
    assert_eq!(before, after);
    assert_eq!(input(&f, 0, "A1"), "=SUM(Doanh_số[Số lượng])");
    // Saving the reopened file keeps one connection and one query table.
    let third = wb_parts(&f.save("xlsx", 0).unwrap());
    assert_eq!(
        part(&third, "xl/connections.xml")
            .unwrap()
            .matches("<connection ")
            .count(),
        1
    );

    // Edit the query: it keeps its table; delete it: the table stays a plain table.
    let mut q2 = exec(&mut f, &json!({"cmd": "queryGet", "name": "Doanh số"})).unwrap();
    q2["steps"]
        .as_array_mut()
        .unwrap()
        .push(json!({"op": "keepTop", "count": 2}));
    let r = exec(
        &mut f,
        &json!({"cmd": "querySave", "query": q2, "original": "Doanh số"}),
    )
    .unwrap();
    assert_eq!(r["table"], "Doanh_số");
    assert_eq!(r["sheet"], s);
    assert_eq!(shown(&f, s, "A3"), "A02");
    assert_eq!(shown(&f, s, "A4"), "");
    exec(&mut f, &json!({"cmd": "queryDelete", "name": "Doanh số"})).unwrap();
    assert_eq!(
        exec(&mut f, &json!({"cmd": "queryList"})).unwrap()["queries"],
        json!([])
    );
    assert_eq!(shown(&f, s, "A3"), "A02");
    let plain = wb_parts(&f.save("xlsx", 0).unwrap());
    assert!(part(&plain, "xl/connections.xml").is_none());
    assert!(!plain.iter().any(|(n, _)| n.starts_with("xl/queryTables/")));
    assert!(!plain
        .iter()
        .any(|(_, d)| String::from_utf8_lossy(d).contains("blueoffice/2026/queries")));
    let t = plain
        .iter()
        .find(|(n, _)| n.starts_with("xl/tables/table") && n.ends_with(".xml"))
        .map(|(_, d)| String::from_utf8_lossy(d).into_owned())
        .unwrap();
    assert!(!t.contains("queryTable"));
    // Undo brings the query back.
    assert!(f.undo().unwrap());
    assert_eq!(
        exec(&mut f, &json!({"cmd": "queryList"})).unwrap()["queries"][0]["name"],
        "Doanh số"
    );
}

#[test]
fn other_sources_merge_existing_sheet_and_errors() {
    let mut e = SheetEngine::new();
    e.new_workbook("q.xlsx").unwrap();
    provide(&mut e, "sales.csv", &fixture("sales.csv"));
    provide(&mut e, "products.json", &fixture("products.json"));
    provide(&mut e, "orders.xml", &fixture("orders.xml"));
    provide(
        &mut e,
        "https://example.test/prices",
        &fixture("prices.html"),
    );
    provide(
        &mut e,
        "features.xls",
        &std::fs::read(corpus("binary-xls-01-features.xls")).unwrap(),
    );
    let folder: Vec<Value> = ["2024-01.csv", "2024-02.csv", "notes.md"]
        .iter()
        .map(|n| json!({"name": n, "data": b64(&std::fs::read(corpus("query/folder").join(n)).unwrap())}))
        .collect();
    exec(
        &mut e,
        &json!({"cmd": "querySource", "key": "Nhập hàng", "access": "folder", "files": folder}),
    )
    .unwrap();

    let r = exec(
        &mut e,
        &json!({"cmd": "queryInspect", "source": {"kind": "json", "file": "products.json"}}),
    )
    .unwrap();
    assert_eq!(r["jsonPaths"][0], "data.products");
    let r = exec(
        &mut e,
        &json!({"cmd": "queryInspect", "source": {"kind": "xml", "file": "orders.xml"}}),
    )
    .unwrap();
    assert_eq!(r["xmlElements"][0], json!({"name": "order", "count": 3}));
    assert_eq!(r["preview"]["columns"][0], "id");
    let r = exec(&mut e, &json!({"cmd": "queryInspect", "source": {"kind": "web", "url": "https://example.test/prices", "table": 1}})).unwrap();
    assert_eq!(r["tables"][1]["caption"], "Giá bán lẻ");
    assert_eq!(r["tables"][1]["header"], true);
    assert_eq!(r["suggested"][0]["op"], "promoteHeaders");
    let r = exec(&mut e, &json!({"cmd": "queryInspect", "source": {"kind": "workbook", "file": "features.xls", "item": ""}})).unwrap();
    let items = r["items"].as_array().unwrap();
    assert_ne!(items.len(), 0);
    assert_eq!(items[0]["kind"], "sheet");
    let first_sheet = items[0]["name"].as_str().unwrap().to_owned();
    let r = exec(&mut e, &json!({"cmd": "queryPreview", "query": {"name": "X", "source": {"kind": "workbook", "file": "features.xls", "item": first_sheet}}})).unwrap();
    assert!(r["rowCount"].as_u64().unwrap() > 0, "{r}");
    let r = exec(&mut e, &json!({"cmd": "queryInspect", "source": {"kind": "folder", "folder": "Nhập hàng", "fileKind": "csv"}})).unwrap();
    assert_eq!(r["files"], json!(["2024-01.csv", "2024-02.csv"]));
    assert_eq!(r["preview"]["rowCount"], 4);

    // Products (connection only) merged into a query loaded on an existing sheet.
    exec(
        &mut e,
        &json!({"cmd": "querySave", "load": "none",
                "query": {"name": "Sản phẩm", "source": {"kind": "json", "file": "products.json"}}}),
    )
    .unwrap();
    let sales = sales_query(&mut e);
    let mut steps = sales["steps"].as_array().unwrap().clone();
    steps.push(json!({"op": "merge", "query": "Sản phẩm", "leftKeys": ["Mã"], "rightKeys": ["code"], "columns": ["price"]}));
    steps.push(
        json!({"op": "addColumn", "name": "Thành tiền", "formula": "=[@[Số lượng]]*[@price]"}),
    );
    steps.push(json!({"op": "groupBy", "keys": ["Khu vực"], "aggregates": [{"name": "Tổng", "op": "sum", "column": "Thành tiền"}]}));
    steps.push(json!({"op": "sort", "keys": [{"column": "Tổng", "descending": true}]}));
    let r = exec(
        &mut e,
        &json!({"cmd": "querySave", "load": "existing", "sheet": 0, "row": 2, "col": 1,
                "query": {"name": "Theo vùng", "source": sales["source"], "steps": steps}}),
    )
    .unwrap();
    assert_eq!(r["sheet"], 0);
    assert_eq!(shown(&e, 0, "B3"), "Khu vực");
    assert_eq!(shown(&e, 0, "B4"), "Bắc");
    assert_eq!(shown(&e, 0, "C4"), "230");
    assert_eq!(shown(&e, 0, "B5"), "Nam");
    assert_eq!(shown(&e, 0, "C5"), "117,5");
    // Loading on top of the table again is refused.
    assert!(exec(
        &mut e,
        &json!({"cmd": "querySave", "load": "existing", "sheet": 0, "row": 3, "col": 2,
                "query": {"name": "Đè", "source": {"kind": "query", "query": "Sản phẩm"}}}),
    )
    .is_err());
    // Folder, XML and web queries; the web one writes a web connection.
    for (name, source) in [
        (
            "Nhập",
            json!({"kind": "folder", "folder": "Nhập hàng", "fileKind": "csv"}),
        ),
        ("Đơn", json!({"kind": "xml", "file": "orders.xml"})),
        (
            "Bảng giá",
            json!({"kind": "web", "url": "https://example.test/prices", "table": 1}),
        ),
    ] {
        let r = exec(&mut e, &json!({"cmd": "queryInspect", "source": source})).unwrap();
        exec(
            &mut e,
            &json!({"cmd": "querySave", "query": {"name": name, "source": source, "steps": r["suggested"]}}),
        )
        .unwrap();
    }
    let list = exec(&mut e, &json!({"cmd": "queryList"})).unwrap();
    assert_eq!(list["queries"].as_array().unwrap().len(), 5);
    assert_eq!(list["queries"][2]["kind"], "folder");
    assert_eq!(list["queries"][2]["access"], "folder");
    let parts = wb_parts(&e.save("xlsx", 0).unwrap());
    let conns = part(&parts, "xl/connections.xml").unwrap();
    assert!(conns.contains("url=\"https://example.test/prices\""));
    // Text and web sources have standard connections; the folder and XML ones an OLE DB
    // connection naming the BlueOffice provider (no standard form, kept by other software).
    assert_eq!(conns.matches("<connection ").count(), 4);
    assert_eq!(conns.matches("Provider=BlueOffice.Query").count(), 2);
    // A query step error is reported with the step.
    let r = exec(
        &mut e,
        &json!({"cmd": "queryPreview", "query": {"name": "E", "source": {"kind": "csv", "file": "sales.csv"},
                "steps": [{"op": "promoteHeaders"}, {"op": "removeColumns", "columns": ["Không có"]}]}}),
    )
    .unwrap();
    assert_eq!(r["error"]["step"], 1);
    assert_eq!(r["error"]["code"], "column");
    assert_eq!(r["rowCount"], 5);
    // Bad commands.
    assert!(exec(&mut e, &json!({"cmd": "queryNope"})).is_err());
    assert!(exec(&mut e, &json!({"cmd": "queryDelete", "name": "Không có"})).is_err());
}

/// A DataMashup part ([MS-QDEFF] §2.2) as other software writes it: UTF-16 XML holding the
/// base64 of Version, PackagePartsLength, an OPC package with `Formulas/Section1.m`, and
/// empty permissions / metadata / bindings.
fn data_mashup(m: &str) -> Vec<u8> {
    let mut zipped = Vec::new();
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut zipped));
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file("[Content_Types].xml", opts).unwrap();
        z.write_all(b"<?xml version=\"1.0\" encoding=\"utf-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"text/xml\"/><Default Extension=\"m\" ContentType=\"application/x-ms-m\"/></Types>").unwrap();
        z.start_file("Formulas/Section1.m", opts).unwrap();
        z.write_all(m.as_bytes()).unwrap();
        z.finish().unwrap();
    }
    let mut raw = Vec::new();
    raw.extend_from_slice(&0u32.to_le_bytes());
    raw.extend_from_slice(&(zipped.len() as u32).to_le_bytes());
    raw.extend_from_slice(&zipped);
    for _ in 0..3 {
        raw.extend_from_slice(&0u32.to_le_bytes());
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-16\"?><DataMashup xmlns=\"http://schemas.microsoft.com/DataMashup\">{}</DataMashup>",
        b64(&raw)
    );
    let mut out = vec![0xFF, 0xFE];
    for u in xml.encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

/// A workbook as other software leaves it after a query: a DataMashup part and an OLE DB
/// connection of its own.
fn foreign_workbook() -> Vec<u8> {
    use bo_model::sheet::{OpaquePart, OpaqueRel};
    let mut wb = bo_model::sheet::Workbook::new();
    wb.sheets[0].cells.insert(
        (0, 0),
        bo_model::sheet::Cell {
            value: bo_model::sheet::CellValue::Text("Dữ liệu".into()),
            ..Default::default()
        },
    );
    wb.opaque.parts.push(OpaquePart {
        name: "/customXml/item1.xml".into(),
        content_type: None,
        data: data_mashup(
            "section Section1;\r\nshared Nguon = Csv.Document(File.Contents(\"a.csv\"));",
        ),
    });
    wb.opaque.parts.push(OpaquePart {
        name: "/customXml/itemProps1.xml".into(),
        content_type: Some("application/vnd.openxmlformats-officedocument.customXmlProperties+xml".into()),
        data: b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\r\n<ds:datastoreItem ds:itemID=\"{11111111-2222-3333-4444-555555555555}\" xmlns:ds=\"http://schemas.openxmlformats.org/officeDocument/2006/customXml\"><ds:schemaRefs/></ds:datastoreItem>".to_vec(),
    });
    wb.opaque.parts.push(OpaquePart {
        name: "/customXml/_rels/item1.xml.rels".into(),
        content_type: None,
        data: b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps\" Target=\"itemProps1.xml\"/></Relationships>".to_vec(),
    });
    wb.opaque.workbook_rels.push(OpaqueRel {
        id: "rId90".into(),
        rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml"
            .into(),
        target: "/customXml/item1.xml".into(),
        external: false,
    });
    wb.opaque.parts.push(OpaquePart {
        name: "/xl/connections.xml".into(),
        content_type: Some("application/vnd.openxmlformats-officedocument.spreadsheetml.connections+xml".into()),
        data: b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<connections xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><connection id=\"1\" keepAlive=\"1\" name=\"Query - Nguon\" description=\"Connection to the 'Nguon' query in the workbook.\" type=\"5\" refreshedVersion=\"8\" background=\"1\"><dbPr connection=\"Provider=Microsoft.Mashup.OleDb.1;Data Source=$Workbook$;Location=Nguon;\" command=\"SELECT * FROM [Nguon]\"/></connection></connections>".to_vec(),
    });
    wb.opaque.workbook_rels.push(OpaqueRel {
        id: "rId91".into(),
        rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/connections"
            .into(),
        target: "/xl/connections.xml".into(),
        external: false,
    });
    bo_xlsx::write_xlsx(&wb).unwrap()
}

#[test]
fn foreign_data_mashup_and_connections_are_kept() {
    let src = foreign_workbook();
    let src_parts = wb_parts(&src);
    let mashup = src_parts
        .iter()
        .find(|(n, _)| n == "customXml/item1.xml")
        .unwrap()
        .1
        .clone();
    let mut e = SheetEngine::new();
    e.open(&src, "foreign.xlsx").unwrap();
    let r = exec(&mut e, &json!({"cmd": "queryMashup"})).unwrap();
    assert_eq!(r["present"], true);
    assert!(r["formulas"]
        .as_str()
        .unwrap()
        .contains("shared Nguon = Csv.Document"));
    assert_eq!(
        exec(&mut e, &json!({"cmd": "queryList"})).unwrap()["queries"],
        json!([])
    );
    // Add a BlueOffice query next to theirs.
    provide(&mut e, "sales.csv", &fixture("sales.csv"));
    let q = sales_query(&mut e);
    exec(&mut e, &json!({"cmd": "querySave", "query": q})).unwrap();
    for round in 0..2 {
        let out = e.save("xlsx", 0).unwrap();
        let parts = wb_parts(&out);
        // Their DataMashup part: byte for byte.
        assert_eq!(
            parts
                .iter()
                .find(|(n, _)| n == "customXml/item1.xml")
                .unwrap()
                .1,
            mashup
        );
        // Their connection is kept as it was; ours follows with the next id.
        let conns = part(&parts, "xl/connections.xml").unwrap();
        assert!(conns.contains("<connection id=\"1\" keepAlive=\"1\" name=\"Query - Nguon\" description=\"Connection to the 'Nguon' query in the workbook.\" type=\"5\" refreshedVersion=\"8\" background=\"1\"><dbPr connection=\"Provider=Microsoft.Mashup.OleDb.1;Data Source=$Workbook$;Location=Nguon;\" command=\"SELECT * FROM [Nguon]\"/></connection>"), "{conns}");
        assert!(
            conns.contains("<connection id=\"2\" name=\"Doanh số\" type=\"6\""),
            "round {round}: {conns}"
        );
        assert_eq!(conns.matches("<connection ").count(), 2);
        // Our definitions in a part of their own.
        assert!(parts.iter().any(|(n, d)| n == "customXml/item2.xml"
            && String::from_utf8_lossy(d).contains("blueoffice")));
        e = SheetEngine::new();
        e.open(&out, "foreign.xlsx").unwrap();
        provide(&mut e, "sales.csv", &fixture("sales.csv"));
    }
    if std::env::var_os("BO_WRITE_CORPUS").is_some() {
        std::fs::write(corpus("query-3.xlsx"), e.save("xlsx", 0).unwrap()).unwrap();
    }
}

#[test]
fn corpus_files_reopen_with_their_queries() {
    if std::env::var_os("BO_WRITE_CORPUS").is_some() {
        // query-1: a CSV query loaded on its own sheet.
        let mut e = SheetEngine::new();
        e.new_workbook("query-1.xlsx").unwrap();
        provide(&mut e, "sales.csv", &fixture("sales.csv"));
        let q = sales_query(&mut e);
        exec(&mut e, &json!({"cmd": "querySave", "query": q})).unwrap();
        std::fs::write(corpus("query-1.xlsx"), e.save("xlsx", 0).unwrap()).unwrap();
        // query-2: a web table and a JSON query merged into it, on the first sheet.
        let mut e = SheetEngine::new();
        e.new_workbook("query-2.xlsx").unwrap();
        provide(
            &mut e,
            "https://example.test/prices",
            &fixture("prices.html"),
        );
        provide(&mut e, "products.json", &fixture("products.json"));
        exec(
            &mut e,
            &json!({"cmd": "querySave", "load": "none",
            "query": {"name": "Sản phẩm", "source": {"kind": "json", "file": "products.json"}}}),
        )
        .unwrap();
        let src = json!({"kind": "web", "url": "https://example.test/prices", "table": 1});
        let r = exec(&mut e, &json!({"cmd": "queryInspect", "source": src})).unwrap();
        let mut steps = r["suggested"].as_array().unwrap().clone();
        steps.push(json!({"op": "merge", "query": "Sản phẩm", "leftKeys": ["Mã"], "rightKeys": ["code"], "columns": ["group.name"]}));
        steps.push(
            json!({"op": "renameColumns", "renames": [{"from": "group.name", "to": "Nhóm"}]}),
        );
        exec(
            &mut e,
            &json!({"cmd": "querySave", "load": "existing", "sheet": 0, "row": 0, "col": 0,
            "query": {"name": "Bảng giá", "source": src, "steps": steps}}),
        )
        .unwrap();
        std::fs::write(corpus("query-2.xlsx"), e.save("xlsx", 0).unwrap()).unwrap();
    }
    for (file, n) in [
        ("query-1.xlsx", 1),
        ("query-2.xlsx", 2),
        ("query-3.xlsx", 1),
        ("query-4.xlsx", 2),
    ] {
        let bytes = std::fs::read(corpus(file)).unwrap();
        let mut e = SheetEngine::new();
        e.open(&bytes, file).unwrap();
        let list = exec(&mut e, &json!({"cmd": "queryList"})).unwrap();
        assert_eq!(list["queries"].as_array().unwrap().len(), n, "{file}");
    }
}

/// Columns added to a loaded query table survive a refresh (a calculated column fills the new
/// rows, a plain one keeps its values row by row), a renamed table keeps its query, and a JSON
/// query is saved with an OLE DB connection and a query table whose added columns are not bound
/// to the query (§18.12.4 `dataBound="0"`, `fillFormulas`).
///
/// `BO_WRITE_CORPUS=1` writes `tests/corpus/sheet/query-4.xlsx`.
#[test]
fn user_columns_survive_refresh_and_rename_keeps_the_query() {
    let mut e = SheetEngine::new();
    e.new_workbook("q.xlsx").unwrap();
    provide(&mut e, "sales.csv", &fixture("sales.csv"));
    let q = sales_query(&mut e);
    let r = exec(
        &mut e,
        &json!({"cmd": "querySave", "query": q, "load": "newSheet"}),
    )
    .unwrap();
    let s = r["sheet"].as_u64().unwrap() as usize;
    // A1:E5 (5 columns, 4 rows). Type a header right of it (the table grows), then a formula
    // (a calculated column), and a plain column of notes.
    e.set_input(s, 0, 5, "Gấp đôi").unwrap();
    e.set_input(s, 1, 5, "=[@[Số lượng]]*2").unwrap();
    e.set_input(s, 0, 6, "Ghi chú").unwrap();
    e.set_input(s, 1, 6, "kiểm").unwrap();
    e.set_input(s, 3, 6, "đủ").unwrap();
    let t = &e.workbook().unwrap().sheets[s].tables[0];
    assert_eq!(t.columns.len(), 7, "{:?}", t.columns);
    // The reference is qualified with the table name as it is typed.
    assert!(
        t.columns[5]
            .calculated_formula
            .as_deref()
            .is_some_and(|f| f.ends_with("[Số lượng]]*2")),
        "{:?}",
        t.columns[5]
    );
    assert_eq!(shown(&e, s, "F5"), "6");

    // The source grows by a row and changes a value: the query's columns are rewritten, the
    // added columns stay where they were, the formula fills the new row.
    let changed = String::from_utf8(fixture("sales.csv"))
        .unwrap()
        .replace("A02;Lê;Nam;4", "A02;Lê;Nam;40")
        + "A05;Xoài;Nam;6;9;2024-03-01\r\n";
    provide(&mut e, "sales.csv", changed.as_bytes());
    exec(&mut e, &json!({"cmd": "queryRefresh"})).unwrap();
    assert_eq!(shown(&e, s, "F1"), "Gấp đôi");
    assert_eq!(shown(&e, s, "F3"), "80");
    assert_eq!(shown(&e, s, "A6"), "A05");
    assert_eq!(shown(&e, s, "F6"), "12");
    assert!(input(&e, s, "F6").ends_with("[Số lượng]]*2"), "{}", input(&e, s, "F6"));
    assert_eq!(shown(&e, s, "G2"), "kiểm");
    assert_eq!(shown(&e, s, "G4"), "đủ");
    assert_eq!(shown(&e, s, "G6"), "");
    let t = &e.workbook().unwrap().sheets[s].tables[0];
    assert_eq!(t.range.to_string(), "A1:G6");

    // The query drops a column: the user's columns move left with the table.
    let mut q2 = exec(&mut e, &json!({"cmd": "queryGet", "name": "Doanh số"})).unwrap();
    q2["steps"]
        .as_array_mut()
        .unwrap()
        .push(json!({"op": "removeColumns", "columns": ["Ngày"]}));
    exec(
        &mut e,
        &json!({"cmd": "querySave", "query": q2, "original": "Doanh số"}),
    )
    .unwrap();
    assert_eq!(shown(&e, s, "E1"), "Gấp đôi");
    assert_eq!(shown(&e, s, "E6"), "12");
    assert_eq!(shown(&e, s, "F2"), "kiểm");
    assert_eq!(shown(&e, s, "G1"), "");

    // Rename the table: the query follows it and refreshes into it.
    exec(
        &mut e,
        &json!({"cmd": "tableUpdate", "sheet": s, "table": 0, "name": "BanHang"}),
    )
    .unwrap();
    let list = exec(&mut e, &json!({"cmd": "queryList"})).unwrap();
    assert_eq!(list["queries"][0]["table"], "BanHang");
    let r = exec(&mut e, &json!({"cmd": "queryRefresh"})).unwrap();
    assert_eq!(r["refreshed"][0]["rows"], 5);
    // Undo of the rename takes the link back too.
    assert!(e.undo().unwrap());
    assert!(e.undo().unwrap());
    let list = exec(&mut e, &json!({"cmd": "queryList"})).unwrap();
    assert_eq!(list["queries"][0]["table"], "Doanh_số");
    assert!(e.redo().unwrap());

    // A JSON query loaded as a table: an OLE DB connection only BlueOffice refreshes.
    provide(&mut e, "products.json", &fixture("products.json"));
    exec(
        &mut e,
        &json!({"cmd": "querySave", "load": "newSheet",
                "query": {"name": "Sản phẩm", "source": {"kind": "json", "file": "products.json"}}}),
    )
    .unwrap();
    let bytes = e.save("xlsx", 0).unwrap();
    let parts = wb_parts(&bytes);
    let conns = part(&parts, "xl/connections.xml").unwrap();
    assert!(conns.contains("type=\"6\""), "{conns}");
    assert!(
        conns.contains("type=\"5\"")
            && conns.contains(
                "Provider=BlueOffice.Query;Location=Sản phẩm;Source=json;Data Source=products.json"
            ),
        "{conns}"
    );
    let qts: Vec<String> = parts
        .iter()
        .filter(|(n, _)| n.starts_with("xl/queryTables/"))
        .map(|(_, d)| String::from_utf8_lossy(d).into_owned())
        .collect();
    assert_eq!(qts.len(), 2);
    let sales_qt = qts.iter().find(|x| x.contains("fillFormulas")).unwrap();
    assert_eq!(sales_qt.matches("dataBound=\"0\"").count(), 2, "{sales_qt}");
    // Reopened: the added columns are still the user's on the next refresh.
    let mut f = SheetEngine::new();
    f.open(&bytes, "q.xlsx").unwrap();
    provide(&mut f, "sales.csv", &fixture("sales.csv"));
    exec(
        &mut f,
        &json!({"cmd": "queryRefresh", "names": ["Doanh số"]}),
    )
    .unwrap();
    assert_eq!(shown(&f, s, "E1"), "Gấp đôi");
    assert_eq!(shown(&f, s, "E3"), "8");
    assert_eq!(shown(&f, s, "E6"), "");
    assert_eq!(shown(&f, s, "F2"), "kiểm");
    if std::env::var_os("BO_WRITE_CORPUS").is_some() {
        std::fs::write(corpus("query-4.xlsx"), &bytes).unwrap();
    }
}
