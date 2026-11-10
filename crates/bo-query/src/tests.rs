//! Sources and steps against the fixtures of `tests/corpus/sheet/query/`.

use super::*;
use crate::def::{Aggregate, ColumnType, Rename, SortKey};
use std::collections::HashMap;
use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/sheet/query")
}

#[derive(Default)]
struct Fixtures {
    files: HashMap<String, Vec<u8>>,
    folders: HashMap<String, Vec<(String, Vec<u8>)>>,
    queries: Vec<Query>,
}

impl Fixtures {
    fn new() -> Self {
        let mut f = Fixtures::default();
        for name in ["sales.csv", "products.json", "orders.xml", "prices.html"] {
            f.files.insert(
                name.into(),
                std::fs::read(fixture_dir().join(name)).unwrap(),
            );
        }
        f.files.insert(
            "tables-1.xlsx".into(),
            std::fs::read(fixture_dir().join("../tables-1.xlsx")).unwrap(),
        );
        let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(fixture_dir().join("folder"))
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        files.sort();
        f.folders.insert("folder".into(), files);
        f
    }
}

impl Resolver for Fixtures {
    fn file(&self, key: &str) -> Option<&[u8]> {
        self.files.get(key).map(Vec::as_slice)
    }
    fn folder(&self, key: &str) -> Option<Vec<(&str, &[u8])>> {
        self.folders
            .get(key)
            .map(|v| v.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect())
    }
    fn query(&self, name: &str) -> Option<Query> {
        self.queries
            .iter()
            .find(|q| q.name.eq_ignore_ascii_case(name))
            .cloned()
    }
}

fn q(name: &str, source: Source, steps: Vec<Step>) -> Query {
    Query {
        name: name.into(),
        source,
        steps,
        load: None,
        description: String::new(),
    }
}

fn csv() -> Source {
    Source::Csv {
        file: "sales.csv".into(),
        delimiter: None,
        encoding: None,
    }
}

fn texts(t: &QTable, col: &str) -> Vec<String> {
    let i = t.col(col).unwrap();
    t.rows.iter().map(|r| r[i].display()).collect()
}

fn sales(f: &Fixtures) -> QTable {
    let base = Evaluator::new(f).source(&csv()).unwrap();
    let mut steps = suggest_steps(&base, true);
    // `12,5`: the decimal comma is the user's choice (the column stays text when detecting).
    steps.push(Step::ChangeType {
        columns: vec![ColumnType {
            column: "Đơn giá".into(),
            ty: ColType::Number,
        }],
        decimal: Some(",".into()),
        month_first: false,
    });
    Evaluator::new(f)
        .run(&q("Sales", csv(), steps), None)
        .unwrap()
}

#[test]
fn csv_source_with_suggested_steps() {
    let f = Fixtures::new();
    let base = Evaluator::new(&f).source(&csv()).unwrap();
    assert_eq!(base.rows.len(), 6);
    let steps = suggest_steps(&base, true);
    assert_eq!(steps[0], Step::PromoteHeaders);
    let Step::ChangeType { columns, .. } = &steps[1] else {
        panic!("{steps:?}");
    };
    let names: Vec<(&str, ColType)> = columns.iter().map(|c| (c.column.as_str(), c.ty)).collect();
    assert_eq!(
        names,
        vec![("Số lượng", ColType::Integer), ("Ngày", ColType::Date)]
    );
    let t = sales(&f);
    assert_eq!(
        t.columns,
        vec!["Mã", "Sản phẩm", "Khu vực", "Số lượng", "Đơn giá", "Ngày"]
    );
    assert_eq!(t.rows[2][1], Val::Text("Cam; loại 1".into()));
    assert_eq!(t.rows[0][4], Val::Num(12.5));
    assert_eq!(t.rows[0][5], Val::Date(45296.0));
    assert_eq!(t.rows[4][3], Val::Null);
    // An explicit delimiter and encoding.
    let t = Evaluator::new(&f)
        .source(&Source::Csv {
            file: "sales.csv".into(),
            delimiter: Some(",".into()),
            encoding: Some("utf-8".into()),
        })
        .unwrap();
    assert_eq!(
        t.rows[0][0],
        Val::Text("Mã;Sản phẩm;Khu vực;Số lượng;Đơn giá;Ngày".into())
    );
}

#[test]
fn each_step() {
    let f = Fixtures::new();
    let t = sales(&f);
    let none = |_: &str| -> Result<QTable, QueryError> { Err(QueryError::Query("x".into())) };
    let run = |step: Step| steps::apply(&step, t.clone(), &none).unwrap();

    let r = run(Step::FilterRows {
        column: "Số lượng".into(),
        condition: Condition::GreaterOrEqual,
        value: "5".into(),
    });
    assert_eq!(texts(&r, "Mã"), vec!["A01", "A03"]);
    let r = run(Step::FilterRows {
        column: "Ngày".into(),
        condition: Condition::LessThan,
        value: "2024-02-01".into(),
    });
    assert_eq!(r.rows.len(), 2);
    let r = run(Step::FilterRows {
        column: "Sản phẩm".into(),
        condition: Condition::Contains,
        value: "CAM".into(),
    });
    assert_eq!(r.rows.len(), 1);
    let r = run(Step::FilterRows {
        column: "Số lượng".into(),
        condition: Condition::IsBlank,
        value: String::new(),
    });
    assert_eq!(texts(&r, "Mã"), vec!["A04"]);

    let r = run(Step::RemoveColumns {
        columns: vec!["Ngày".into(), "Khu vực".into()],
    });
    assert_eq!(r.columns, vec!["Mã", "Sản phẩm", "Số lượng", "Đơn giá"]);
    let r = run(Step::KeepColumns {
        columns: vec!["Ngày".into(), "Mã".into()],
    });
    assert_eq!(r.columns, vec!["Ngày", "Mã"]);
    let r = run(Step::ReorderColumns {
        columns: vec!["Ngày".into()],
    });
    assert_eq!(r.columns[0], "Ngày");
    assert_eq!(r.columns.len(), 6);
    let r = run(Step::RenameColumns {
        renames: vec![Rename {
            from: "Mã".into(),
            to: "Code".into(),
        }],
    });
    assert_eq!(r.columns[0], "Code");
    assert!(steps::apply(
        &Step::RenameColumns {
            renames: vec![Rename {
                from: "Mã".into(),
                to: "Ngày".into()
            }]
        },
        t.clone(),
        &none
    )
    .is_err());
    let r = run(Step::SplitColumn {
        column: "Sản phẩm".into(),
        delimiter: "; ".into(),
        parts: None,
    });
    assert_eq!(&r.columns[1..3], &["Sản phẩm.1", "Sản phẩm.2"]);
    assert_eq!(r.rows[2][2], Val::Text("loại 1".into()));
    assert_eq!(r.rows[0][2], Val::Null);
    let r = run(Step::ReplaceValues {
        columns: vec!["Khu vực".into()],
        find: "Bắc".into(),
        replace: "Miền Bắc".into(),
        match_entire: true,
    });
    assert_eq!(texts(&r, "Khu vực")[0], "Miền Bắc");
    let r = run(Step::ReplaceValues {
        columns: vec!["Số lượng".into()],
        find: String::new(),
        replace: "0".into(),
        match_entire: true,
    });
    assert_eq!(r.rows[4][3], Val::Num(0.0));
    let r = run(Step::GroupBy {
        keys: vec!["Khu vực".into()],
        aggregates: vec![
            Aggregate {
                name: "Tổng SL".into(),
                op: "sum".into(),
                column: "Số lượng".into(),
            },
            Aggregate {
                name: "Số dòng".into(),
                op: "count".into(),
                column: String::new(),
            },
            Aggregate {
                name: "Giá TB".into(),
                op: "average".into(),
                column: "Đơn giá".into(),
            },
            Aggregate {
                name: "Ngày đầu".into(),
                op: "min".into(),
                column: "Ngày".into(),
            },
        ],
    });
    assert_eq!(texts(&r, "Khu vực"), vec!["Bắc", "Nam", "Trung"]);
    assert_eq!(texts(&r, "Tổng SL"), vec!["17", "7", "0"]);
    assert_eq!(texts(&r, "Số dòng"), vec!["2", "2", "1"]);
    assert_eq!(texts(&r, "Giá TB"), vec!["13.75", "16.25", "30"]);
    assert_eq!(texts(&r, "Ngày đầu")[0], "2024-01-05");
    let r = run(Step::Sort {
        keys: vec![
            SortKey {
                column: "Khu vực".into(),
                descending: false,
            },
            SortKey {
                column: "Số lượng".into(),
                descending: true,
            },
        ],
    });
    assert_eq!(texts(&r, "Mã"), vec!["A01", "A03", "A02", "A01", "A04"]);
    let r = run(Step::Unpivot {
        columns: vec!["Số lượng".into(), "Đơn giá".into()],
        other: false,
        attribute: "Thuộc tính".into(),
        value: "Giá trị".into(),
    });
    assert_eq!(
        r.columns,
        vec!["Mã", "Sản phẩm", "Khu vực", "Ngày", "Thuộc tính", "Giá trị"]
    );
    // The blank quantity of A04 makes no row.
    assert_eq!(r.rows.len(), 9);
    let r = run(Step::AddColumn {
        name: "Thành tiền".into(),
        formula: "=[@[Số lượng]]*[@[Đơn giá]]".into(),
    });
    assert_eq!(
        texts(&r, "Thành tiền"),
        vec!["125", "80", "105", "37.5", "0"]
    );
    assert_eq!(r.types[6], ColType::Number);
    let r = run(Step::AddColumn {
        name: "Tỉ lệ".into(),
        formula: "=ROUND([@[Số lượng]]/SUM([Số lượng]),2)".into(),
    });
    assert_eq!(texts(&r, "Tỉ lệ")[0], "0.42");
    assert!(steps::apply(
        &Step::AddColumn {
            name: "x".into(),
            formula: "=((".into()
        },
        t.clone(),
        &none
    )
    .is_err());
    let r = run(Step::RemoveDuplicates {
        columns: vec!["Mã".into()],
    });
    assert_eq!(texts(&r, "Mã"), vec!["A01", "A02", "A03", "A04"]);
    let r = run(Step::RemoveDuplicates { columns: vec![] });
    assert_eq!(r.rows.len(), 5);
    let r = run(Step::KeepTop { count: 2 });
    assert_eq!(r.rows.len(), 2);
    let r = run(Step::RemoveTop { count: 4 });
    assert_eq!(texts(&r, "Mã"), vec!["A04"]);
    let r = run(Step::DemoteHeaders);
    assert_eq!(r.rows[0][0], Val::Text("Mã".into()));
    assert_eq!(r.columns[0], "Column1");
    let mut blank = t.clone();
    blank.rows.push(vec![Val::Null; 6]);
    let r = steps::apply(&Step::RemoveBlankRows, blank, &none).unwrap();
    assert_eq!(r.rows.len(), 5);
    // A missing column names itself.
    let e = steps::apply(
        &Step::RemoveColumns {
            columns: vec!["Nope".into()],
        },
        t,
        &none,
    )
    .unwrap_err();
    assert_eq!(e.to_string(), "column: Nope");
}

#[test]
fn merge_append_and_query_sources() {
    let mut f = Fixtures::new();
    let base = Evaluator::new(&f).source(&csv()).unwrap();
    f.queries
        .push(q("Sales", csv(), suggest_steps(&base, true)));
    let products = Source::Json {
        file: "products.json".into(),
        path: Some("data.products".into()),
    };
    f.queries.push(q("Products", products.clone(), vec![]));
    let merge = |kind: JoinKind| {
        Evaluator::new(&f)
            .run(
                &q(
                    "M",
                    Source::Query {
                        query: "Sales".into(),
                    },
                    vec![Step::Merge {
                        query: "Products".into(),
                        left_keys: vec!["Mã".into()],
                        right_keys: vec!["code".into()],
                        kind,
                        columns: vec!["price".into(), "group.name".into()],
                    }],
                ),
                None,
            )
            .unwrap()
    };
    let r = merge(JoinKind::LeftOuter);
    assert_eq!(r.rows.len(), 5);
    assert_eq!(texts(&r, "price"), vec!["12.5", "20", "15", "12.5", ""]);
    assert_eq!(merge(JoinKind::Inner).rows.len(), 4);
    let r = merge(JoinKind::FullOuter);
    assert_eq!(r.rows.len(), 6);
    assert_eq!(texts(&r, "Mã")[5], "B01");
    assert_eq!(merge(JoinKind::RightOuter).rows.len(), 5);
    let r = merge(JoinKind::LeftAnti);
    assert_eq!(texts(&r, "Mã"), vec!["A04"]);
    assert_eq!(r.columns.len(), 6);
    let r = merge(JoinKind::RightAnti);
    assert_eq!(texts(&r, "Mã"), vec!["B01"]);

    let r = Evaluator::new(&f)
        .run(
            &q(
                "A",
                Source::Query {
                    query: "sales".into(),
                },
                vec![Step::Append {
                    queries: vec!["Products".into()],
                }],
            ),
            None,
        )
        .unwrap();
    assert_eq!(r.rows.len(), 9);
    assert!(r.columns.contains(&"price".to_owned()));

    // Cycles and unknown queries.
    f.queries.push(q(
        "Loop",
        Source::Query {
            query: "Loop2".into(),
        },
        vec![],
    ));
    f.queries.push(q(
        "Loop2",
        Source::Query {
            query: "Loop".into(),
        },
        vec![],
    ));
    let e = Evaluator::new(&f)
        .run(&f.query("Loop").unwrap(), None)
        .unwrap_err();
    assert_eq!(e.error.code(), "cycle");
    let e = Evaluator::new(&f)
        .run(
            &q(
                "X",
                Source::Query {
                    query: "Nope".into(),
                },
                vec![],
            ),
            None,
        )
        .unwrap_err();
    assert_eq!(e.error.to_string(), "query: Nope");
    // Step errors carry the step.
    let e = Evaluator::new(&f)
        .run(
            &q(
                "Y",
                csv(),
                vec![
                    Step::PromoteHeaders,
                    Step::RemoveColumns {
                        columns: vec!["zz".into()],
                    },
                ],
            ),
            None,
        )
        .unwrap_err();
    assert_eq!(e.step, Some(1));
    // A missing source names its key.
    let e = Evaluator::new(&f)
        .run(
            &q(
                "Z",
                Source::Csv {
                    file: "gone.csv".into(),
                    delimiter: None,
                    encoding: None,
                },
                vec![],
            ),
            None,
        )
        .unwrap_err();
    assert_eq!(e.error.missing(), Some("gone.csv"));
    assert_eq!(e.step, None);
}

#[test]
fn json_xml_web_workbook_and_folder_sources() {
    let f = Fixtures::new();
    let ev = Evaluator::new(&f);
    let t = ev
        .source(&Source::Json {
            file: "products.json".into(),
            path: None,
        })
        .unwrap();
    assert_eq!(
        t.columns,
        vec![
            "code",
            "name",
            "group.id",
            "group.name",
            "price",
            "active",
            "tags"
        ]
    );
    assert_eq!(t.rows[3][4], Val::Num(18.25));
    assert_eq!(t.rows[2][6], Val::Text("[\"new\",\"sale\"]".into()));

    let t = ev
        .source(&Source::Xml {
            file: "orders.xml".into(),
            element: None,
        })
        .unwrap();
    assert_eq!(
        t.columns,
        vec![
            "id",
            "status",
            "customer",
            "date",
            "total",
            "ship.city",
            "ship.zip"
        ]
    );
    assert_eq!(t.rows[1][2], Val::Text("Minh & Cộng sự".into()));
    assert_eq!(t.rows[1][6], Val::Null);

    let html = crate::text::decode(f.file("prices.html").unwrap(), None).text;
    let tables = crate::html::tables(&html);
    assert_eq!(tables.len(), 2);
    assert_eq!(tables[1].caption, "Giá bán lẻ");
    assert!(tables[1].header);
    let t = ev
        .source(&Source::Web {
            url: "prices.html".into(),
            table: 1,
        })
        .unwrap();
    assert_eq!(t.rows[2][1], Val::Text("Lê & đào".into()));
    let steps = suggest_steps(&t, true);
    assert_eq!(steps[0], Step::PromoteHeaders);
    assert!(ev
        .source(&Source::Web {
            url: "prices.html".into(),
            table: 5
        })
        .is_err());

    let wb = bo_xlsx::read_xlsx(f.file("tables-1.xlsx").unwrap()).unwrap();
    let items = crate::workbook_src::items(&wb);
    let table = items.iter().find(|i| i.kind == ItemKind::Table).unwrap();
    let t = ev
        .source(&Source::Workbook {
            file: "tables-1.xlsx".into(),
            item: table.name.clone(),
            item_kind: ItemKind::Table,
        })
        .unwrap();
    let t0 = &wb.sheets.iter().flat_map(|s| &s.tables).next().unwrap();
    assert_eq!(t.columns[0], t0.columns[0].name);
    assert_ne!(t.rows, [] as [std::vec::Vec<value::Val>; 0]);
    let sheet = items.iter().find(|i| i.kind == ItemKind::Sheet).unwrap();
    let s = ev
        .source(&Source::Workbook {
            file: "tables-1.xlsx".into(),
            item: sheet.name.clone(),
            item_kind: ItemKind::Sheet,
        })
        .unwrap();
    assert!(s.rows.len() > t.rows.len());

    let t = ev
        .source(&Source::Folder {
            folder: "folder".into(),
            file_kind: "csv".into(),
            delimiter: None,
            item: None,
            promote_headers: true,
        })
        .unwrap();
    assert_eq!(t.columns, vec!["Source.Name", "Mã", "Số lượng", "Ghi chú"]);
    assert_eq!(t.rows.len(), 4);
    assert_eq!(texts(&t, "Source.Name")[3], "2024-02.csv");
    assert_eq!(t.rows[0][3], Val::Null);
}

#[test]
fn definitions_part_and_connections() {
    let f = Fixtures::new();
    let base = Evaluator::new(&f).source(&csv()).unwrap();
    let mut sales = q("Doanh số & <Q>", csv(), suggest_steps(&base, true));
    sales.load = Some(LoadTarget {
        table: "DoanhSo".into(),
    });
    let web = q(
        "Web",
        Source::Web {
            url: "https://example.com/a?b=1&c=2".into(),
            table: 1,
        },
        vec![],
    );
    let xml = persist::queries_xml(&[sales.clone(), web.clone()]);
    assert!(persist::is_queries_part(xml.as_bytes()));
    assert_eq!(
        persist::parse_queries(xml.as_bytes()),
        vec![sales.clone(), web.clone()]
    );
    assert!(!persist::is_queries_part(b"<root/>"));

    let c = persist::connection_xml(&sales, 3, "Doanh số", "").unwrap();
    assert!(c.contains("type=\"6\""));
    assert!(c.contains("semicolon") || c.contains("comma"));
    let w = persist::connection_xml(&web, 4, "Web", "").unwrap();
    assert!(w.contains("url=\"https://example.com/a?b=1&amp;c=2\""));
    assert!(persist::connection_xml(
        &q(
            "J",
            Source::Json {
                file: "a.json".into(),
                path: None
            },
            vec![]
        ),
        1,
        "J",
        ""
    )
    .is_none());
    // Other connections stay byte for byte; ours are replaced.
    let other = "<connection id=\"7\" name=\"Theirs\" type=\"5\" refreshedVersion=\"6\"><dbPr connection=\"x\" command=\"y\"/></connection>";
    let part = persist::connections_part(None, std::slice::from_ref(&c)).unwrap();
    let with_other = part.replace("<connection ", &format!("{other}<connection "));
    let stripped = persist::strip_connections(&with_other);
    assert_eq!(stripped.max_id, 7);
    assert_eq!(stripped.names, vec!["Theirs"]);
    assert!(stripped.xml.contains(other));
    assert!(!stripped.xml.contains(persist::EXT_URI));
    let again = persist::connections_part(Some(&stripped), &[w]).unwrap();
    assert!(again.contains(other) && again.contains("webPr"));
    assert!(!again.contains("textPr"));
    let qt = persist::query_table_xml(&sales, 3, &[(1, "Mã".into()), (2, "Ngày".into())]);
    assert!(qt.contains("connectionId=\"3\"") && qt.contains("tableColumnId=\"2\""));
    assert!(persist::is_own_query_table(qt.as_bytes()));
}
