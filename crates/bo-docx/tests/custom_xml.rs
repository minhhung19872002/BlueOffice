//! Custom XML parts (ECMA-376 Part 1, §15.2.4 / §15.2.5) and content control data binding
//! (§17.5.2.6): reading, refreshing bound controls on open, round trip and rewriting.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_docx::{read_docx, write_docx, CT_DOCUMENT};
use bo_model::custom_xml::binding::{set_node_value, sync, SyncMode};
use bo_model::custom_xml::CustomXmlPart;
use bo_model::{Block, ContentControl, ControlKind, Document, RunContent};
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};

const REL_CX: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";
const REL_CXP: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps";
const CT_CXP: &str = "application/vnd.openxmlformats-officedocument.customXmlProperties+xml";
const ID: &str = "{0D6B1C4E-5A2F-4B7C-9E10-3F2A6B8C9D01}";

const ITEM: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n<f:form xmlns:f=\"urn:bluestar:test-form\">\n  <f:name>Bluestar</f:name>\n  <f:agree>true</f:agree>\n  <f:due>2026-11-20</f:due>\n  <f:size>m</f:size>\n</f:form>";

fn props(id: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n<ds:datastoreItem ds:itemID=\"{id}\" xmlns:ds=\"http://schemas.openxmlformats.org/officeDocument/2006/customXml\"><ds:schemaRefs><ds:schemaRef ds:uri=\"urn:bluestar:test-form\"/></ds:schemaRefs></ds:datastoreItem>")
}

fn binding(xpath: &str) -> String {
    format!("<w:dataBinding w:prefixMappings=\"xmlns:ns0='urn:bluestar:test-form'\" w:xpath=\"{xpath}\" w:storeItemID=\"{ID}\"/>")
}

fn body() -> String {
    let name = binding("/ns0:form[1]/ns0:name[1]");
    let agree = binding("/ns0:form[1]/ns0:agree[1]");
    let due = binding("/ns0:form[1]/ns0:due[1]");
    let size = binding("/ns0:form[1]/ns0:size[1]");
    let missing = binding("/ns0:form[1]/ns0:nothere[1]");
    format!(
        "<w:p><w:r><w:t xml:space=\"preserve\">Name: </w:t></w:r><w:sdt><w:sdtPr><w:rPr><w:b/></w:rPr><w:alias w:val=\"Name\"/><w:id w:val=\"1\"/>{name}<w:text/></w:sdtPr><w:sdtContent><w:r><w:rPr><w:b/></w:rPr><w:t>stale</w:t></w:r></w:sdtContent></w:sdt></w:p>\
<w:p><w:sdt><w:sdtPr><w:id w:val=\"2\"/>{name}<w:text/></w:sdtPr><w:sdtContent><w:r><w:t>stale too</w:t></w:r></w:sdtContent></w:sdt></w:p>\
<w:p><w:sdt><w:sdtPr><w:id w:val=\"3\"/>{agree}<w14:checkbox><w14:checked w14:val=\"0\"/><w14:checkedState w14:val=\"2612\"/><w14:uncheckedState w14:val=\"2610\"/></w14:checkbox></w:sdtPr><w:sdtContent><w:r><w:t>\u{2610}</w:t></w:r></w:sdtContent></w:sdt></w:p>\
<w:p><w:sdt><w:sdtPr><w:id w:val=\"4\"/>{due}<w:date><w:dateFormat w:val=\"dd/MM/yyyy\"/><w:lid w:val=\"vi-VN\"/><w:storeMappedDataAs w:val=\"date\"/><w:calendar w:val=\"gregorian\"/></w:date></w:sdtPr><w:sdtContent><w:r><w:t>01/01/2000</w:t></w:r></w:sdtContent></w:sdt></w:p>\
<w:p><w:sdt><w:sdtPr><w:id w:val=\"5\"/>{size}<w:dropDownList><w:listItem w:displayText=\"Small\" w:value=\"s\"/><w:listItem w:displayText=\"Medium\" w:value=\"m\"/></w:dropDownList></w:sdtPr><w:sdtContent><w:r><w:t>Small</w:t></w:r></w:sdtContent></w:sdt></w:p>\
<w:p><w:sdt><w:sdtPr><w:id w:val=\"6\"/>{missing}<w:text/></w:sdtPr><w:sdtContent><w:r><w:t>cached</w:t></w:r></w:sdtContent></w:sdt></w:p>\
<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/></w:sectPr>"
    )
}

/// A package with the main part, one custom XML item and its properties.
fn sample() -> Vec<u8> {
    let mut p = Package::new();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="w14"><w:body>{}</w:body></w:document>"#,
        body()
    );
    p.add_part("/word/document.xml", CT_DOCUMENT, xml.into_bytes())
        .unwrap();
    p.add_part(
        "/customXml/item1.xml",
        "application/xml",
        ITEM.as_bytes().to_vec(),
    )
    .unwrap();
    p.add_part("/customXml/itemProps1.xml", CT_CXP, props(ID).into_bytes())
        .unwrap();
    p.set_relationships(
        "/",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_OFFICE_DOCUMENT.into(),
            target: "word/document.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    p.set_relationships(
        "/word/document.xml",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_CX.into(),
            target: "../customXml/item1.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    p.set_relationships(
        "/customXml/item1.xml",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_CXP.into(),
            target: "itemProps1.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    p.write().unwrap()
}

fn texts(doc: &Document) -> Vec<String> {
    doc.sections[0]
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(bo_model::Paragraph::plain_text)
        .collect()
}

fn controls(doc: &Document) -> Vec<ContentControl> {
    doc.sections[0]
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .flat_map(|p| p.runs.iter().flat_map(|r| &r.content))
        .filter_map(|c| match c {
            RunContent::ControlStart(cc) => Some((**cc).clone()),
            _ => None,
        })
        .collect()
}

fn part_text(bytes: &[u8], name: &str) -> Option<String> {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    pkg.part(name)
        .map(|p| String::from_utf8_lossy(&p.data).into_owned())
}

#[test]
fn reads_parts_and_refreshes_bound_controls() {
    let doc = read_docx(&sample()).unwrap();
    assert_eq!(doc.custom_xml.len(), 1);
    let part = &doc.custom_xml[0];
    assert_eq!(part.part_name, "/customXml/item1.xml");
    assert_eq!(part.item_id.as_deref(), Some(ID));
    assert_eq!(part.schema_refs, ["urn:bluestar:test-form"]);
    assert_eq!(
        part.props_part.as_deref(),
        Some("/customXml/itemProps1.xml")
    );
    assert_eq!(
        part.root(),
        Some(("f:form".to_owned(), "urn:bluestar:test-form".to_owned()))
    );
    // Both controls bound to the name show the node; the check box is checked; the date is
    // shown in its format; the list shows the display text of value `m`; the control whose
    // node is missing keeps its cached text.
    assert_eq!(
        texts(&doc),
        [
            "Name: Bluestar",
            "Bluestar",
            "\u{2612}",
            "20/11/2026",
            "Medium",
            "cached"
        ]
    );
    let cc = controls(&doc);
    assert!(matches!(
        cc[2].kind,
        ControlKind::Checkbox { checked: true, .. }
    ));
    assert!(matches!(
        &cc[3].kind,
        ControlKind::Date { full_date: Some(d), store_as: Some(s), .. }
            if d.starts_with("2026-11-20") && s == "date"
    ));
    let b = cc[0].data_binding.as_ref().unwrap();
    assert_eq!(b.xpath, "/ns0:form[1]/ns0:name[1]");
    assert_eq!(b.prefix_mappings, "xmlns:ns0='urn:bluestar:test-form'");
    assert_eq!(b.store_item_id.as_deref(), Some(ID));
    // The refreshed run keeps the control's formatting.
    let Block::Paragraph(p) = &doc.sections[0].blocks[0] else {
        panic!("paragraph")
    };
    assert!(p
        .runs
        .iter()
        .any(|r| r.props.bold == Some(true) && r.content == [RunContent::Text("Bluestar".into())]));
}

#[test]
fn unchanged_parts_round_trip_byte_for_byte() {
    let src = sample();
    let doc = read_docx(&src).unwrap();
    let out = write_docx(&doc).unwrap();
    for name in [
        "/customXml/item1.xml",
        "/customXml/itemProps1.xml",
        "/customXml/_rels/item1.xml.rels",
        "/word/_rels/document.xml.rels",
    ] {
        assert_eq!(part_text(&out, name), part_text(&src, name), "{name}");
    }
    let main = part_text(&out, "/word/document.xml").unwrap();
    // The refreshed controls keep their binding and their other properties.
    assert_eq!(main.matches("<w:dataBinding").count(), 6);
    assert!(main.contains("<w:rPr><w:b/></w:rPr><w:alias w:val=\"Name\"/>"));
    let back = read_docx(&out).unwrap();
    assert_eq!(texts(&back), texts(&doc));
    assert_eq!(back.custom_xml, doc.custom_xml);
}

#[test]
fn edited_node_is_written_and_shown() {
    let mut doc = read_docx(&sample()).unwrap();
    set_node_value(
        &mut doc.custom_xml,
        0,
        "/ns0:form/ns0:name",
        "xmlns:ns0='urn:bluestar:test-form'",
        "Sao Xanh & Co",
    )
    .unwrap()
    .unwrap();
    let out = sync(&mut doc, SyncMode::FromParts, true);
    assert_eq!(out.edits.len(), 2);
    let bytes = write_docx(&doc).unwrap();
    let item = part_text(&bytes, "/customXml/item1.xml").unwrap();
    assert!(
        item.contains("<f:name>Sao Xanh &amp; Co</f:name>"),
        "{item}"
    );
    // Only the edited node differs from the source.
    assert_eq!(item.replace("Sao Xanh &amp; Co", "Bluestar"), ITEM);
    // The properties part is untouched.
    assert_eq!(
        part_text(&bytes, "/customXml/itemProps1.xml"),
        Some(props(ID))
    );
    let back = read_docx(&bytes).unwrap();
    assert_eq!(&texts(&back)[..2], ["Name: Sao Xanh & Co", "Sao Xanh & Co"]);
}

#[test]
fn removed_part_goes_with_its_relationships() {
    let mut doc = read_docx(&sample()).unwrap();
    doc.custom_xml.clear();
    let bytes = write_docx(&doc).unwrap();
    for name in [
        "/customXml/item1.xml",
        "/customXml/itemProps1.xml",
        "/customXml/_rels/item1.xml.rels",
    ] {
        assert!(part_text(&bytes, name).is_none(), "{name}");
    }
    let rels = part_text(&bytes, "/word/_rels/document.xml.rels").unwrap_or_default();
    assert!(!rels.contains("customXml"));
    let ct = part_text(&bytes, "/[Content_Types].xml").unwrap();
    assert!(!ct.contains("itemProps1"));
    let back = read_docx(&bytes).unwrap();
    assert_eq!(back.custom_xml.len(), 0);
    // The controls keep the text they showed (now unbound).
    assert_eq!(texts(&back)[0], "Name: Bluestar");
}

#[test]
fn new_document_with_a_part() {
    let mut doc = Document::default();
    doc.sections
        .push(bo_model::Section::new(vec![Block::Paragraph(
            bo_model::Paragraph::default(),
        )]));
    doc.custom_xml.push(CustomXmlPart {
        part_name: "/customXml/item1.xml".into(),
        xml: "<data xmlns=\"urn:x\"><v>1</v></data>".into(),
        item_id: Some(ID.into()),
        schema_refs: vec![],
        props_part: Some("/customXml/itemProps1.xml".into()),
    });
    let bytes = write_docx(&doc).unwrap();
    let back = read_docx(&bytes).unwrap();
    assert_eq!(back.custom_xml, doc.custom_xml);
    let rels = part_text(&bytes, "/customXml/_rels/item1.xml.rels").unwrap();
    assert!(rels.contains("Target=\"itemProps1.xml\""));
    let main_rels = part_text(&bytes, "/word/_rels/document.xml.rels").unwrap();
    assert!(main_rels.contains("Target=\"../customXml/item1.xml\""));
}

#[test]
fn utf16_part_is_read() {
    let src = sample();
    let mut pkg = Package::read(&src, &Limits::default()).unwrap();
    let text = ITEM.replace("UTF-8", "UTF-16");
    let mut bytes = vec![0xFF, 0xFE];
    for u in text.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    pkg.put_part("/customXml/item1.xml", bytes.clone());
    let mut doc = read_docx(&pkg.write().unwrap()).unwrap();
    assert_eq!(texts(&doc)[1], "Bluestar");
    // Unchanged: the UTF-16 bytes stay.
    let out = write_docx(&doc).unwrap();
    let again = Package::read(&out, &Limits::default()).unwrap();
    assert_eq!(again.part("/customXml/item1.xml").unwrap().data, bytes);
    // Changed: written as UTF-8 with a matching declaration.
    set_node_value(
        &mut doc.custom_xml,
        0,
        "/ns0:form/ns0:size",
        "xmlns:ns0='urn:bluestar:test-form'",
        "s",
    )
    .unwrap();
    let out = write_docx(&doc).unwrap();
    let item = part_text(&out, "/customXml/item1.xml").unwrap();
    assert!(item.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\""));
    assert!(item.contains("<f:size>s</f:size>"));
}
