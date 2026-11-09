//! Check document (CORE-INSPECT-001): document inspector, accessibility checker
//! and compatibility checker for BlueDoc, BlueSheet and BlueSlide, on small
//! hand-built files whose expected counts follow from how they are built.
//! Removal → save → reopen verifies the parts are gone. With `BO_INSPECT_OUT`
//! set, the files are written there for `bo-cli validate --openxml require`.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::{Engine, SheetEngine};
use bo_model::sheet::{
    CellRange, CellRef, CellValue, ColInfo, Comment as SheetComment, RowInfo, SheetState, TextRun,
    Workbook, Worksheet,
};
use bo_model::{
    Block, Cell, Color, Comment, Document, Emu, Image, ParaProps, Paragraph, Row, Run, RunContent,
    RunProps, Section, Table,
};
use bo_opc::{Limits, Package, Relationship, TargetMode};
use serde_json::{json, Value};

const CT_CORE: &str = "application/vnd.openxmlformats-package.core-properties+xml";
const CT_APP: &str = "application/vnd.openxmlformats-officedocument.extended-properties+xml";
const CT_CUSTOM: &str = "application/vnd.openxmlformats-officedocument.custom-properties+xml";
const CT_ITEM_PROPS: &str = "application/vnd.openxmlformats-officedocument.customXmlProperties+xml";
const CT_VBA: &str = "application/vnd.ms-office.vbaProject";
const REL_ITEM_PROPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps";

fn out_dir() -> Option<std::path::PathBuf> {
    let d = std::env::var_os("BO_INSPECT_OUT")?;
    let d = std::path::PathBuf::from(d);
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

fn keep(name: &str, bytes: &[u8]) {
    if let Some(d) = out_dir() {
        std::fs::write(d.join(name), bytes).unwrap();
    }
}

fn rel(id: &str, ty: &str, target: &str) -> Relationship {
    Relationship {
        id: id.into(),
        rel_type: ty.into(),
        target: target.into(),
        mode: TargetMode::Internal,
    }
}

/// Add document properties, custom properties, two custom XML parts (one of
/// them server properties) and — with `macro_ct` — a VBA project to a package.
fn add_package_parts(bytes: &[u8], main: &str, macro_ct: Option<&str>) -> Vec<u8> {
    use bo_engine::inspect::package::{REL_APP, REL_CORE, REL_CUSTOM, REL_CUSTOM_XML, REL_VBA};
    let mut pkg = Package::read(bytes, &Limits::default()).unwrap();
    for n in [
        "/docProps/core.xml",
        "/docProps/app.xml",
        "/docProps/custom.xml",
    ] {
        pkg.remove_part(n);
    }
    let core = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>Ann Author</dc:creator><cp:lastModifiedBy>Bob Editor</cp:lastModifiedBy><cp:revision>3</cp:revision><dcterms:created xsi:type="dcterms:W3CDTF">2026-01-02T03:04:05Z</dcterms:created><dcterms:modified xsi:type="dcterms:W3CDTF">2026-01-03T03:04:05Z</dcterms:modified></cp:coreProperties>"#;
    let app = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>BlueOffice</Application><Company>Acme Ltd</Company></Properties>"#;
    let custom = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="Client"><vt:lpwstr>Contoso</vt:lpwstr></property><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="3" name="Project"><vt:lpwstr>X</vt:lpwstr></property></Properties>"#;
    pkg.add_part("/docProps/core.xml", CT_CORE, core.into())
        .unwrap();
    pkg.add_part("/docProps/app.xml", CT_APP, app.into())
        .unwrap();
    pkg.add_part("/docProps/custom.xml", CT_CUSTOM, custom.into())
        .unwrap();
    let mut root: Vec<Relationship> = pkg
        .relationships("/")
        .unwrap()
        .into_iter()
        .filter(|r| ![REL_CORE, REL_APP, REL_CUSTOM].contains(&r.rel_type.as_str()))
        .collect();
    root.push(rel("rIdCore", REL_CORE, "docProps/core.xml"));
    root.push(rel("rIdApp", REL_APP, "docProps/app.xml"));
    root.push(rel("rIdCustom", REL_CUSTOM, "docProps/custom.xml"));
    pkg.set_relationships("/", &root);
    // Custom XML data (Part 1, §15.2.4) with its properties part (§15.2.5).
    let items = [
        (
            "1",
            r#"<?xml version="1.0" encoding="UTF-8"?><data xmlns="urn:test:data"><v>1</v></data>"#,
        ),
        (
            "2",
            r#"<?xml version="1.0" encoding="UTF-8"?><p:properties xmlns:p="http://schemas.microsoft.com/office/2006/metadata/properties"><documentManagement/></p:properties>"#,
        ),
    ];
    let mut main_rels = pkg.relationships(main).unwrap();
    for (n, xml) in items {
        let item = format!("/customXml/item{n}.xml");
        pkg.put_part(&item, xml.as_bytes().to_vec());
        let props = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?><ds:datastoreItem ds:itemID="{{6C4A6C3E-1111-4B6B-9C00-00000000000{n}}}" xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml"><ds:schemaRefs/></ds:datastoreItem>"#
        );
        pkg.add_part(
            &format!("/customXml/itemProps{n}.xml"),
            CT_ITEM_PROPS,
            props.into_bytes(),
        )
        .unwrap();
        pkg.set_relationships(
            &item,
            &[rel("rId1", REL_ITEM_PROPS, &format!("itemProps{n}.xml"))],
        );
        main_rels.push(rel(
            &format!("rIdCx{n}"),
            REL_CUSTOM_XML,
            &format!("../customXml/item{n}.xml"),
        ));
    }
    if let Some(ct) = macro_ct {
        let dir = main.rsplit_once('/').map_or("", |(d, _)| d);
        pkg.add_part(
            &format!("{dir}/vbaProject.bin"),
            CT_VBA,
            vec![0xD0, 0xCF, 0x11, 0xE0],
        )
        .unwrap();
        main_rels.push(rel("rIdVba", REL_VBA, "vbaProject.bin"));
        let mut types = pkg.content_types().unwrap();
        types.set_override(main, ct);
        pkg.set_content_types(&types);
    }
    pkg.set_relationships(main, &main_rels);
    pkg.write().unwrap()
}

fn part_names(bytes: &[u8]) -> Vec<String> {
    Package::read(bytes, &Limits::default())
        .unwrap()
        .parts()
        .iter()
        .map(|p| p.name.to_ascii_lowercase())
        .collect()
}

fn part_text(bytes: &[u8], name: &str) -> String {
    Package::read(bytes, &Limits::default())
        .unwrap()
        .part(name)
        .map(|p| String::from_utf8_lossy(&p.data).into_owned())
        .unwrap_or_default()
}

fn count(report: &Value, category: &str) -> u64 {
    report["inspector"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["category"] == category)
        .unwrap_or_else(|| panic!("no category {category}"))["count"]
        .as_u64()
        .unwrap()
}

fn pending(report: &Value, category: &str) -> bool {
    report["inspector"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["category"] == category)
        .unwrap()["pending"]
        == true
}

fn issues<'a>(report: &'a Value, rule: &str) -> Vec<&'a Value> {
    report["accessibility"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["rule"] == rule)
        .collect()
}

fn compat(report: &Value, feature: &str) -> u64 {
    report["compatibility"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["feature"] == feature)
        .map_or(0, |c| c["count"].as_u64().unwrap())
}

// ------------------------------------------------------------------ BlueDoc

fn para(text: &str, props: RunProps) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run::text(text, props)],
    ))
}

fn tiny_png() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, 1, 1);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut w = e.write_header().unwrap();
        w.write_image_data(&[255, 0, 0, 255]).unwrap();
    }
    out
}

fn table(cells: &[&[&str]], header: bool) -> Block {
    let rows = cells
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut row = Row {
                cells: r
                    .iter()
                    .map(|t| Cell {
                        blocks: vec![if t.is_empty() {
                            Block::Paragraph(Paragraph::default())
                        } else {
                            para(t, RunProps::default())
                        }],
                        ..Cell::default()
                    })
                    .collect(),
                ..Row::default()
            };
            row.props.header = header && i == 0;
            row
        })
        .collect();
    Block::Table(Table {
        grid: vec![bo_model::Twips(2000); cells[0].len()],
        rows,
        ..Table::default()
    })
}

/// A document whose checks are known by construction (see `doc_report_counts`).
fn sample_doc() -> Document {
    let heading = |text: &str, level: u8| {
        Block::Paragraph(Paragraph::new(
            ParaProps {
                outline_level: Some(level),
                ..ParaProps::default()
            },
            vec![Run::text(text, RunProps::default())],
        ))
    };
    let hidden = Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![
            Run::text("Visible ", RunProps::default()),
            Run::text(
                "secret",
                RunProps {
                    hidden: Some(true),
                    ..RunProps::default()
                },
            ),
        ],
    ));
    let grey = para(
        "Light grey text",
        RunProps {
            color: Some(Color {
                r: 0x77,
                g: 0x77,
                b: 0x77,
            }),
            ..RunProps::default()
        },
    );
    let link = para(
        "https://example.com",
        RunProps {
            link: Some("https://example.com".into()),
            ..RunProps::default()
        },
    );
    let commented = Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![
            Run {
                props: RunProps::default(),
                content: vec![RunContent::CommentStart(1)],
            },
            Run::text("Reviewed text", RunProps::default()),
            Run {
                props: RunProps::default(),
                content: vec![RunContent::CommentEnd(1)],
            },
            Run {
                props: RunProps::default(),
                content: vec![RunContent::CommentRef(1)],
            },
        ],
    ));
    let picture = Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![RunContent::Image(Box::new(Image {
                media: "/word/media/image1.png".into(),
                width: Emu(914_400),
                height: Emu(914_400),
                anchor: None,
                crop: None,
                rotation: 0,
                description: String::new(),
                shape: None,
                format: None,
                src: None,
            }))],
        }],
    ));
    let mut section = Section::new(vec![
        heading("Introduction", 0),
        heading("Details", 2),
        hidden,
        grey,
        link,
        commented,
        picture,
        table(&[&["Name", "Value"], &["a", "1"]], false),
        table(&[&["Left", ""], &["x", ""]], true),
        para("End", RunProps::default()),
    ]);
    section.headers.default = Some(vec![para("Confidential", RunProps::default())]);
    let mut doc = Document {
        sections: vec![section],
        comments: vec![Comment {
            id: 1,
            author: "Ann Author".into(),
            initials: Some("AA".into()),
            date: Some("2026-01-02T03:04:05Z".into()),
            blocks: vec![para("Please check", RunProps::default())],
            ..Comment::default()
        }],
        ..Document::default()
    };
    doc.media.insert(
        "/word/media/image1.png".into(),
        std::sync::Arc::new(tiny_png()),
    );
    doc
}

fn doc_cmd(e: &mut Engine, v: &Value) -> Value {
    serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap()
}

fn doc_report(e: &mut Engine) -> Value {
    doc_cmd(e, &json!({"cmd": "inspect"}))["inspection"].clone()
}

const DOCM_CT: &str = "application/vnd.ms-word.document.macroEnabled.main+xml";

fn sample_docm() -> Vec<u8> {
    let bytes = bo_docx::write_docx(&sample_doc()).unwrap();
    add_package_parts(&bytes, "/word/document.xml", Some(DOCM_CT))
}

#[test]
fn doc_report_counts() {
    let bytes = sample_docm();
    keep("inspect-doc-before.docm", &bytes);
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "sample.docm").unwrap();
    let r = doc_report(&mut e);
    // Inspector.
    assert_eq!(count(&r, "comments"), 1);
    assert_eq!(count(&r, "revisions"), 0);
    assert_eq!(count(&r, "properties"), 3, "{}", r["inspector"]); // creator, lastModifiedBy, Company
    assert_eq!(count(&r, "customProperties"), 2);
    assert_eq!(count(&r, "customXml"), 1);
    assert_eq!(count(&r, "serverProperties"), 1);
    assert_eq!(count(&r, "macros"), 1);
    assert_eq!(count(&r, "hiddenText"), 1);
    assert_eq!(count(&r, "headersFooters"), 1);
    assert_eq!(count(&r, "watermarks"), 0);
    assert_eq!(count(&r, "invisibleObjects"), 0);
    assert_eq!(count(&r, "embedded"), 0);
    let comments = &r["inspector"][0];
    assert_eq!(comments["details"][0], "Ann Author");
    assert_eq!(comments["action"], "remove");
    // Accessibility.
    let alt = issues(&r, "altText");
    assert_eq!(alt.len(), 1);
    assert_eq!(alt[0]["severity"], "error");
    assert_eq!(alt[0]["target"]["kind"], "drawing");
    assert_eq!(alt[0]["target"]["para"], 6);
    assert_eq!(alt[0]["fix"], "altText");
    let order = issues(&r, "headingOrder");
    assert_eq!(order.len(), 1);
    assert_eq!(order[0]["target"]["para"], 1);
    let contrast = issues(&r, "contrast");
    assert_eq!(contrast.len(), 1);
    assert_eq!(contrast[0]["ratio"], 4.48);
    assert_eq!(contrast[0]["target"]["para"], 3);
    assert_eq!(issues(&r, "linkText").len(), 1);
    let header = issues(&r, "tableHeader");
    assert_eq!(header.len(), 1);
    assert_eq!(header[0]["target"], json!({"kind": "table", "para": 7}));
    let layout = issues(&r, "layoutTable");
    assert_eq!(layout.len(), 1);
    assert_eq!(layout[0]["target"], json!({"kind": "table", "para": 11}));
    assert_eq!(issues(&r, "documentTitle").len(), 1);
    assert_eq!(issues(&r, "noHeadings").len(), 0);
    assert_eq!(r["issueCount"], 7);
    // Errors first.
    assert_eq!(r["accessibility"][0]["severity"], "error");
    // Compatibility.
    assert_eq!(compat(&r, "tables"), 2);
    assert_eq!(compat(&r, "drawings"), 1);
    assert_eq!(compat(&r, "comments"), 1);
    assert_eq!(compat(&r, "headersFooters"), 1);
    assert_eq!(compat(&r, "contentControls"), 0);
    assert!(r["compatibility"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["count"].as_u64().unwrap() > 0));
}

#[test]
fn doc_fixes_are_undoable() {
    let mut e = Engine::with_bundled_fonts();
    e.open(&sample_docm(), "sample.docm").unwrap();
    let r = doc_report(&mut e);
    let alt = issues(&r, "altText")[0]["target"].clone();
    let table = issues(&r, "tableHeader")[0]["target"].clone();
    let res = doc_cmd(
        &mut e,
        &json!({"cmd": "inspectFix", "fix": {"kind": "altText", "target": alt, "text": "Red square"}}),
    );
    assert!(res.get("error").is_none(), "{res}");
    assert_eq!(issues(&res["inspection"], "altText").len(), 0);
    let res = doc_cmd(
        &mut e,
        &json!({"cmd": "inspectFix", "fix": {"kind": "headerRow", "target": table}}),
    );
    assert_eq!(issues(&res["inspection"], "tableHeader").len(), 0);
    assert!(
        e.document().unwrap().sections[0]
            .blocks
            .iter()
            .any(|b| matches!(b,
        Block::Table(t) if t.rows[0].props.header && t.rows[0].cells.len() == 2
            && t.rows[1].cells[1].blocks[0].as_paragraph().is_some_and(|p| p.plain_text() == "1")))
    );
    doc_cmd(&mut e, &json!({"cmd": "undo"}));
    assert_eq!(issues(&doc_report(&mut e), "tableHeader").len(), 1);
    doc_cmd(&mut e, &json!({"cmd": "undo"}));
    assert_eq!(issues(&doc_report(&mut e), "altText").len(), 1);
    doc_cmd(&mut e, &json!({"cmd": "redo"}));
    // The alternative text is written to `wp:docPr/@descr` and read back.
    let saved = e.save_docx().unwrap();
    assert!(part_text(&saved, "/word/document.xml").contains("descr=\"Red square\""));
    let mut again = Engine::with_bundled_fonts();
    again.open(&saved, "fixed.docx").unwrap();
    assert_eq!(issues(&doc_report(&mut again), "altText").len(), 0);
    // A fix for another app's target is refused without changing anything.
    let res = doc_cmd(
        &mut e,
        &json!({"cmd": "inspectFix", "fix": {"kind": "altText", "target": {"kind": "shape", "slide": 0, "id": 2}, "text": "x"}}),
    );
    assert!(res["error"].is_string());
}

#[test]
fn doc_remove_save_reopen() {
    let mut e = Engine::with_bundled_fonts();
    e.open(&sample_docm(), "sample.docm").unwrap();
    let all = [
        "comments",
        "hiddenText",
        "headersFooters",
        "properties",
        "customProperties",
        "customXml",
        "serverProperties",
        "macros",
    ];
    let res = doc_cmd(&mut e, &json!({"cmd": "inspectRemove", "categories": all}));
    let r = &res["inspection"];
    assert!(res.get("error").is_none(), "{res}");
    assert_eq!(count(r, "comments"), 0);
    assert_eq!(count(r, "hiddenText"), 0);
    assert_eq!(count(r, "headersFooters"), 0);
    // Package parts are queued until the next save.
    assert_eq!(count(r, "customXml"), 1);
    assert!(pending(r, "customXml") && pending(r, "macros") && pending(r, "properties"));
    assert_eq!(res["modified"], true);
    // One undo step brings the model content back, redo removes it again.
    doc_cmd(&mut e, &json!({"cmd": "undo"}));
    let back = doc_report(&mut e);
    assert_eq!(
        (
            count(&back, "comments"),
            count(&back, "hiddenText"),
            count(&back, "headersFooters")
        ),
        (1, 1, 1)
    );
    doc_cmd(&mut e, &json!({"cmd": "redo"}));
    // Keep, then queue again.
    let kept = doc_cmd(
        &mut e,
        &json!({"cmd": "inspectKeep", "categories": ["macros"]}),
    );
    assert!(!pending(&kept["inspection"], "macros"));
    doc_cmd(
        &mut e,
        &json!({"cmd": "inspectRemove", "categories": ["macros"]}),
    );
    let text = e.document().unwrap().plain_text();
    assert!(
        text.contains("Visible") && !text.contains("secret"),
        "{text}"
    );

    let saved = e.save_docx().unwrap();
    keep("inspect-doc-after.docx", &saved);
    let names = part_names(&saved);
    for gone in [
        "/docprops/custom.xml",
        "/customxml/item1.xml",
        "/customxml/item2.xml",
        "/customxml/itemprops1.xml",
        "/word/vbaproject.bin",
        "/word/comments.xml",
    ] {
        assert!(
            !names.contains(&gone.to_owned()),
            "{gone} still in {names:?}"
        );
    }
    let core = part_text(&saved, "/docProps/core.xml");
    assert!(
        !core.contains("Ann Author") && !core.contains("Bob Editor"),
        "{core}"
    );
    assert!(
        core.contains("2026-01-02T03:04:05Z"),
        "dates are kept: {core}"
    );
    assert!(!part_text(&saved, "/docProps/app.xml").contains("Acme"));
    let types = part_text(&saved, "/[Content_Types].xml");
    assert!(
        !types.contains("macroEnabled") && !types.contains("vbaProject"),
        "{types}"
    );
    assert!(!types.contains("customXml/itemProps"), "{types}");
    let rels = part_text(&saved, "/word/_rels/document.xml.rels");
    assert!(
        !rels.contains("customXml") && !rels.contains("vbaProject"),
        "{rels}"
    );
    let root = part_text(&saved, "/_rels/.rels");
    assert!(!root.contains("custom-properties"), "{root}");

    let mut again = Engine::with_bundled_fonts();
    again.open(&saved, "clean.docx").unwrap();
    let r = doc_report(&mut again);
    for c in [
        "comments",
        "properties",
        "customProperties",
        "customXml",
        "serverProperties",
        "macros",
        "hiddenText",
        "headersFooters",
    ] {
        assert_eq!(count(&r, c), 0, "{c}");
    }
}

#[test]
fn doc_watermarks_and_invisible_objects() {
    let mut doc = sample_doc();
    let wm = r##"<w:pict><v:shape id="PowerPlusWaterMarkObject1" type="#_x0000_t136"/></w:pict>"##;
    doc.sections[0].headers.default = Some(vec![Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![RunContent::Opaque(bo_model::Raw::new(wm, 0))],
        }],
    ))]);
    let mut e = Engine::with_bundled_fonts();
    e.load(doc, "w.docx").unwrap();
    let r = doc_report(&mut e);
    assert_eq!(count(&r, "watermarks"), 1);
    let res = doc_cmd(
        &mut e,
        &json!({"cmd": "inspectRemove", "categories": ["watermarks"]}),
    );
    assert_eq!(count(&res["inspection"], "watermarks"), 0);
    // The header itself stays (now empty of the watermark).
    assert!(e.document().unwrap().sections[0].headers.default.is_some());
}

#[test]
fn long_document_without_headings() {
    let words = "lorem ipsum dolor sit amet ".repeat(220);
    let doc = Document {
        sections: vec![Section::new(vec![para(&words, RunProps::default())])],
        ..Document::default()
    };
    let mut e = Engine::with_bundled_fonts();
    e.load(doc, "long.docx").unwrap();
    assert_eq!(issues(&doc_report(&mut e), "noHeadings").len(), 1);
}

// ---------------------------------------------------------------- BlueSheet

fn sheet_exec(s: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&s.exec_json(&v.to_string()).unwrap()).unwrap()
}

fn sample_workbook() -> Workbook {
    let mut wb = Workbook::new();
    let ws = &mut wb.sheets[0];
    for r in 0..6u32 {
        ws.set_a1(&format!("A{}", r + 1), CellValue::Number(f64::from(r)));
        ws.set_a1(&format!("C{}", r + 1), CellValue::Text(format!("c{r}")));
    }
    // Rows 2 and 3 (0-based 1, 2) and column B are hidden.
    for r in [1, 2] {
        ws.rows.insert(
            r,
            RowInfo {
                hidden: true,
                ..RowInfo::default()
            },
        );
    }
    ws.cols.push(ColInfo {
        min: 1,
        max: 1,
        hidden: true,
        ..ColInfo::default()
    });
    ws.merges
        .push(CellRange::new(CellRef::new(8, 0), CellRef::new(8, 2)));
    ws.comments.push(SheetComment {
        cell: CellRef::new(0, 0),
        author: "Ann Author".into(),
        text: vec![TextRun {
            text: "Check".into(),
            ..TextRun::default()
        }],
    });
    ws.page_setup.odd_header = Some("&CConfidential".into());
    ws.set_a1("E1", CellValue::Text("https://example.com".into()));
    ws.hyperlinks.push(bo_model::sheet::Hyperlink {
        range: CellRange::single(CellRef::new(0, 4)),
        target: Some("https://example.com".into()),
        location: None,
        display: None,
        tooltip: None,
    });
    let mut hidden = Worksheet::new("Rates");
    hidden.sheet_id = 2;
    hidden.state = SheetState::Hidden;
    hidden.set_a1("A1", CellValue::Number(0.1));
    wb.sheets.push(hidden);
    let mut named = Worksheet::new("Summary");
    named.sheet_id = 3;
    named.set_a1("A1", CellValue::Text("Total".into()));
    wb.sheets.push(named);
    wb
}

fn sample_xlsx() -> Vec<u8> {
    let mut s = SheetEngine::with_bundled_fonts();
    s.load(sample_workbook(), "book.xlsx").unwrap();
    let bytes = s.save("xlsx", 0).unwrap();
    add_package_parts(
        &bytes,
        "/xl/workbook.xml",
        Some("application/vnd.ms-excel.sheet.macroEnabled.main+xml"),
    )
}

#[test]
fn sheet_report_remove_save_reopen() {
    let bytes = sample_xlsx();
    keep("inspect-sheet-before.xlsm", &bytes);
    let mut s = SheetEngine::with_bundled_fonts();
    s.open(&bytes, "book.xlsm").unwrap();
    let r = sheet_exec(&mut s, &json!({"cmd": "inspect"}));
    assert_eq!(count(&r, "comments"), 1);
    assert_eq!(count(&r, "hiddenRows"), 2);
    assert_eq!(count(&r, "hiddenColumns"), 1);
    assert_eq!(count(&r, "hiddenSheets"), 1);
    assert_eq!(count(&r, "headersFooters"), 1);
    assert_eq!(count(&r, "properties"), 3);
    assert_eq!(count(&r, "customProperties"), 2);
    assert_eq!(count(&r, "customXml"), 1);
    assert_eq!(count(&r, "serverProperties"), 1);
    assert_eq!(count(&r, "macros"), 1);
    // Accessibility: one merge, one default sheet name (Sheet1), one bare-URL link.
    let merged = issues(&r, "mergedCells");
    assert_eq!(merged.len(), 1);
    assert_eq!(
        merged[0]["target"],
        json!({"kind": "range", "sheet": 0, "r0": 8, "c0": 0, "r1": 8, "c1": 2})
    );
    let names = issues(&r, "sheetName");
    assert_eq!(names.len(), 1);
    assert_eq!(names[0]["detail"], "Sheet1");
    assert_eq!(issues(&r, "linkText").len(), 1);
    assert_eq!(compat(&r, "comments"), 1);

    let res = sheet_exec(
        &mut s,
        &json!({"cmd": "inspectRemove", "categories": ["comments", "hiddenRows", "hiddenColumns", "hiddenSheets", "headersFooters", "properties", "customProperties", "customXml", "serverProperties", "macros"]}),
    );
    for c in [
        "comments",
        "hiddenRows",
        "hiddenColumns",
        "hiddenSheets",
        "headersFooters",
    ] {
        assert_eq!(count(&res, c), 0, "{c}");
    }
    let wb = s.workbook().unwrap();
    assert_eq!(wb.sheets.len(), 2);
    // Rows 2–3 deleted: A4 (value 3) moved up to A2; column B deleted: C → B.
    assert_eq!(
        wb.sheets[0].cell_a1("A2").unwrap().value,
        CellValue::Number(3.0)
    );
    assert_eq!(
        wb.sheets[0].cell_a1("B1").unwrap().value,
        CellValue::Text("c0".into())
    );
    // One undo step restores everything.
    assert!(s.undo().unwrap());
    assert_eq!(s.workbook().unwrap().sheets.len(), 3);
    assert!(s.redo().unwrap());

    let saved = s.save("xlsx", 0).unwrap();
    keep("inspect-sheet-after.xlsx", &saved);
    let names = part_names(&saved);
    assert!(
        !names.iter().any(|n| n.contains("vbaproject")
            || n.starts_with("/customxml/")
            || n == "/docprops/custom.xml"),
        "{names:?}"
    );
    assert!(!part_text(&saved, "/[Content_Types].xml").contains("macroEnabled"));
    let core = part_text(&saved, "/docProps/core.xml");
    assert!(!core.contains("Ann Author"), "{core}");
    let mut again = SheetEngine::with_bundled_fonts();
    again.open(&saved, "clean.xlsx").unwrap();
    let r = sheet_exec(&mut again, &json!({"cmd": "inspect"}));
    for c in [
        "comments",
        "properties",
        "customProperties",
        "customXml",
        "serverProperties",
        "macros",
        "hiddenRows",
        "hiddenColumns",
        "hiddenSheets",
    ] {
        assert_eq!(count(&r, c), 0, "{c}");
    }
}

// ---------------------------------------------------------------- BlueSlide

fn corpus(path: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus")
            .join(path),
    )
    .unwrap()
}

fn slide_cmd(e: &mut Engine, v: &Value) -> Value {
    serde_json::from_str(&e.slide_command_json(&v.to_string()).unwrap()).unwrap()
}

#[test]
fn slide_report_remove_save_reopen() {
    use bo_model::slide::ShapeNode;
    let mut e = Engine::with_bundled_fonts();
    e.open(&corpus("slide/slide-edit-slides.pptx"), "deck.pptx")
        .unwrap();
    let n = e.slide_count();
    assert!(n >= 3, "corpus deck has {n} slides");
    {
        let pres = e.presentation_mut().unwrap();
        // Slide 2 hidden, every speaker note of slide 1 set, one shape off the slide,
        // one shape invisible.
        pres.slides[1].show = Some(false);
        let cx = pres.slide_size.cx;
        let shapes = &mut pres.slides[0].data.shapes;
        let mut moved = 0;
        for node in shapes.iter_mut() {
            if let ShapeNode::Shape(s) = node {
                match moved {
                    0 => {
                        s.props.xfrm = Some(bo_model::slide::Xfrm {
                            off: Some((cx + 10, 0)),
                            ext: Some((914_400, 914_400)),
                            ..bo_model::slide::Xfrm::default()
                        });
                    }
                    1 => s.nv.hidden = Some(true),
                    _ => break,
                }
                moved += 1;
            }
        }
        assert_eq!(moved, 2, "slide 1 needs two shapes");
    }
    let saved = e.save_pptx().unwrap();
    let bytes = add_package_parts(&saved, "/ppt/presentation.xml", None);
    keep("inspect-slide-before.pptx", &bytes);
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "deck.pptx").unwrap();
    let r = slide_cmd(&mut e, &json!({"cmd": "inspect"}))["inspection"].clone();
    assert_eq!(count(&r, "hiddenSlides"), 1);
    assert_eq!(count(&r, "offSlide"), 1);
    assert_eq!(count(&r, "invisibleObjects"), 1);
    assert_eq!(count(&r, "properties"), 3);
    assert_eq!(count(&r, "customXml"), 1);
    let notes = count(&r, "notes");
    // Every slide without a title is an error; ids target slides.
    for i in issues(&r, "slideTitle") {
        assert_eq!(i["target"]["kind"], "slide");
    }

    let res = slide_cmd(
        &mut e,
        &json!({"cmd": "inspectRemove", "categories": ["hiddenSlides", "offSlide", "invisibleObjects", "notes", "properties", "customProperties", "customXml", "serverProperties"]}),
    );
    let r2 = &res["inspection"];
    for c in ["hiddenSlides", "offSlide", "invisibleObjects", "notes"] {
        assert_eq!(count(r2, c), 0, "{c}");
    }
    assert_eq!(e.slide_count(), n - 1);
    assert_eq!(res["canUndo"], true);
    let undone = slide_cmd(&mut e, &json!({"cmd": "undo"}));
    assert_eq!(e.slide_count(), n);
    assert_eq!(
        count(
            &slide_cmd(&mut e, &json!({"cmd": "inspect"}))["inspection"],
            "notes"
        ),
        notes
    );
    assert_eq!(undone["slidesChanged"], true);
    slide_cmd(&mut e, &json!({"cmd": "redo"}));

    let saved = e.save_pptx().unwrap();
    keep("inspect-slide-after.pptx", &saved);
    let names = part_names(&saved);
    assert!(
        !names
            .iter()
            .any(|n| n.starts_with("/customxml/") || n == "/docprops/custom.xml"),
        "{names:?}"
    );
    let mut again = Engine::with_bundled_fonts();
    again.open(&saved, "clean.pptx").unwrap();
    let r = slide_cmd(&mut again, &json!({"cmd": "inspect"}))["inspection"].clone();
    for c in [
        "hiddenSlides",
        "offSlide",
        "invisibleObjects",
        "notes",
        "properties",
        "customXml",
        "serverProperties",
        "customProperties",
    ] {
        assert_eq!(count(&r, c), 0, "{c}");
    }
}

#[test]
fn slide_alt_text_and_header_row_fixes() {
    let mut e = Engine::with_bundled_fonts();
    e.open(&corpus("slide/slide-edit-tables.pptx"), "t.pptx")
        .unwrap();
    let r = slide_cmd(&mut e, &json!({"cmd": "inspect"}))["inspection"].clone();
    let tables: Vec<Value> = issues(&r, "tableHeader").into_iter().cloned().collect();
    if let Some(t) = tables.first() {
        let res = slide_cmd(
            &mut e,
            &json!({"cmd": "inspectFix", "fix": {"kind": "headerRow", "target": t["target"]}}),
        );
        assert!(res.get("error").is_none(), "{res}");
        assert_eq!(
            issues(&res["inspection"], "tableHeader").len(),
            tables.len() - 1
        );
        slide_cmd(&mut e, &json!({"cmd": "undo"}));
        let back = slide_cmd(&mut e, &json!({"cmd": "inspect"}));
        assert_eq!(
            issues(&back["inspection"], "tableHeader").len(),
            tables.len()
        );
    }
    // Alternative text on the first top-level object of slide 1.
    let id = e.presentation().unwrap().slides[0].data.shapes[0]
        .nv()
        .unwrap()
        .id;
    let res = slide_cmd(
        &mut e,
        &json!({"cmd": "inspectFix", "fix": {"kind": "altText", "target": {"kind": "shape", "slide": 0, "id": id}, "text": "Sales table"}}),
    );
    assert!(res.get("error").is_none(), "{res}");
    assert_eq!(
        e.presentation().unwrap().slides[0].data.shapes[0]
            .nv()
            .unwrap()
            .descr
            .as_deref(),
        Some("Sales table")
    );
    let res = slide_cmd(
        &mut e,
        &json!({"cmd": "inspectFix", "fix": {"kind": "altText", "target": {"kind": "shape", "slide": 0, "id": 999_999}, "text": "x"}}),
    );
    assert_eq!(res["error"], "inspectTarget");
}

#[test]
fn slide_titles_and_contrast() {
    use bo_model::slide::{Color as DColor, Fill, ShapeNode, TextRun as DRun};
    let mut e = Engine::with_bundled_fonts();
    e.open(&corpus("slide/slide-edit-slides.pptx"), "deck.pptx")
        .unwrap();
    let pres = e.presentation_mut().unwrap();
    // Give every slide the same title text, and grey-on-white text on slide 1.
    let mut titled = 0;
    for s in &mut pres.slides {
        for node in &mut s.data.shapes {
            if let ShapeNode::Shape(sh) = node {
                let is_title = sh
                    .nv
                    .placeholder
                    .as_ref()
                    .and_then(|p| p.kind.as_deref())
                    .is_some_and(|k| k == "title" || k == "ctrTitle");
                if let (true, Some(body)) = (is_title, sh.text.as_mut()) {
                    if let Some(DRun::Run { text, .. }) =
                        body.paragraphs.first_mut().and_then(|p| p.runs.first_mut())
                    {
                        "Same title".clone_into(text);
                        titled += 1;
                    }
                }
            }
        }
    }
    let first_text = pres.slides[0].data.shapes.iter_mut().find_map(|n| match n {
        ShapeNode::Shape(sh) if sh.text.is_some() => Some(sh),
        _ => None,
    });
    let sh = first_text.unwrap();
    sh.props.fill = Some(Fill::Solid(DColor::srgb(0xFF_FF_FF)));
    if let Some(DRun::Run { props, .. }) = sh.text.as_mut().unwrap().paragraphs[0].runs.first_mut()
    {
        props.fill = Some(Fill::Solid(DColor::srgb(0xAA_AA_AA)));
        props.sz = Some(1200);
    }
    let r = slide_cmd(&mut e, &json!({"cmd": "inspect"}))["inspection"].clone();
    if titled >= 2 {
        assert_eq!(issues(&r, "duplicateTitle").len(), titled - 1);
    }
    let c = issues(&r, "contrast");
    assert_eq!(c.len(), 1, "{}", r["accessibility"]);
    // #AAAAAA on white ≈ 2.32:1.
    assert!((c[0]["ratio"].as_f64().unwrap() - 2.32).abs() < 0.01);
}
