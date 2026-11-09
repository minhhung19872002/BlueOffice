//! CORE-SEC-001: mark as final (`_MarkAsFinal`, ECMA-376 Part 1, §22.3),
//! read-only recommended / password to modify (§17.15.1.93 writeProtection,
//! §18.2.12 fileSharing, §19.2.1.19 modifyVerifier), restricted editing with
//! editable regions (§17.15.1.29 documentProtection, §17.13.7 permStart /
//! permEnd), protected ranges (§18.3.1.71) and sensitivity labels.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::{Engine, SheetEngine};
use bo_opc::{Limits, Package, Relationship, TargetMode};
use serde_json::{json, Value};

fn cmd(e: &mut Engine, v: &Value) -> Value {
    serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap()
}

fn slide(e: &mut Engine, v: &Value) -> Value {
    serde_json::from_str(&e.slide_command_json(&v.to_string()).unwrap()).unwrap()
}

fn sheet(e: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap()
}

fn part(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    Package::read(bytes, &Limits::default())
        .unwrap()
        .part(name)
        .map(|p| p.data.clone())
}

fn text(bytes: &[u8], name: &str) -> String {
    String::from_utf8(part(bytes, name).unwrap_or_default()).unwrap()
}

fn doc(text: &str) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, &json!({"cmd": "insertText", "text": text}));
    let bytes = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "a.docx").unwrap();
    f
}

fn reopen(e: &Engine) -> Engine {
    let bytes = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "a.docx").unwrap();
    f
}

fn plain(e: &Engine) -> String {
    e.document().unwrap().plain_text()
}

const CUSTOM: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="MSIP_Label_11111111-2222-3333-4444-555555555555_Enabled"><vt:lpwstr>true</vt:lpwstr></property><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="3" name="MSIP_Label_11111111-2222-3333-4444-555555555555_Name"><vt:lpwstr>Confidential-Test</vt:lpwstr></property><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="4" name="MSIP_Label_11111111-2222-3333-4444-555555555555_Method"><vt:lpwstr>Standard</vt:lpwstr></property><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="5" name="Dự án"><vt:lpwstr>BlueOffice</vt:lpwstr></property></Properties>"#;
const LABEL_INFO: &str = r#"<?xml version="1.0" encoding="utf-8" standalone="yes"?><clbl:labelList xmlns:clbl="http://schemas.microsoft.com/office/2020/mipLabelMetadata"><clbl:label id="{11111111-2222-3333-4444-555555555555}" enabled="1" method="Standard" siteId="{99999999-8888-7777-6666-555555555555}" removed="0"/></clbl:labelList>"#;

/// A package with custom properties (another product's label) and a label
/// metadata part.
fn with_labels(bytes: &[u8]) -> Vec<u8> {
    let mut pkg = Package::read(bytes, &Limits::default()).unwrap();
    pkg.add_part(
        "/docProps/custom.xml",
        bo_opc::custom::CT_CUSTOM_PROPERTIES,
        CUSTOM.as_bytes().to_vec(),
    )
    .unwrap();
    pkg.add_part(
        "/docMetadata/LabelInfo.xml",
        "application/vnd.ms-office.classificationlabels+xml",
        LABEL_INFO.as_bytes().to_vec(),
    )
    .unwrap();
    let mut rels = pkg.relationships("/").unwrap();
    rels.push(Relationship {
        id: "rIdC".into(),
        rel_type: bo_opc::custom::REL_CUSTOM_PROPERTIES.into(),
        target: "docProps/custom.xml".into(),
        mode: TargetMode::Internal,
    });
    rels.push(Relationship {
        id: "rIdL".into(),
        rel_type: "http://schemas.microsoft.com/office/2020/02/relationships/classificationlabels"
            .into(),
        target: "docMetadata/LabelInfo.xml".into(),
        mode: TargetMode::Internal,
    });
    pkg.set_relationships("/", &rels);
    pkg.write().unwrap()
}

#[test]
fn untouched_labels_and_properties_round_trip_byte_for_byte() {
    let src = with_labels(&doc("Văn bản có nhãn").save_docx().unwrap());
    let mut e = Engine::with_bundled_fonts();
    e.open(&src, "a.docx").unwrap();
    let r = cmd(&mut e, &json!({"cmd": "security"}));
    let s = &r["security"];
    assert_eq!(s["readOnly"], false);
    assert_eq!(s["foreignLabels"][0]["name"], "Confidential-Test");
    assert_eq!(s["foreignLabels"][0]["enabled"], true);
    assert_eq!(s["foreignLabels"][0]["method"], "Standard");
    assert!(s["label"].is_null());
    let out = e.save_docx().unwrap();
    for name in [
        "/docProps/custom.xml",
        "/docMetadata/LabelInfo.xml",
        "/word/settings.xml",
        "/word/document.xml",
    ] {
        assert_eq!(part(&out, name), part(&src, name), "{name}");
    }
    // Typing does not touch the package-level parts either.
    cmd(&mut e, &json!({"cmd": "insertText", "text": "!"}));
    let out = e.save_docx().unwrap();
    assert_eq!(
        part(&out, "/docProps/custom.xml"),
        part(&src, "/docProps/custom.xml")
    );
    assert_eq!(
        part(&out, "/word/settings.xml"),
        part(&src, "/word/settings.xml")
    );
}

#[test]
fn mark_as_final_opens_read_only_until_edit_anyway() {
    let src = with_labels(&doc("Bản cuối").save_docx().unwrap());
    let mut e = Engine::with_bundled_fonts();
    e.open(&src, "a.docx").unwrap();
    let r = cmd(&mut e, &json!({"cmd": "markFinal", "on": true}));
    assert_eq!(r["security"]["markedFinal"], true);
    assert_eq!(r["security"]["readOnly"], true);
    assert_eq!(r["modified"], true);
    let saved = e.save_docx().unwrap();
    let custom = text(&saved, "/docProps/custom.xml");
    assert!(custom.contains(r#"name="_MarkAsFinal"><vt:bool>true</vt:bool>"#));
    // The other properties keep their XML.
    assert!(custom.contains(r#"pid="5" name="Dự án"><vt:lpwstr>BlueOffice</vt:lpwstr>"#));

    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    let r = cmd(&mut f, &json!({"cmd": "security"}));
    assert_eq!(r["security"]["readOnly"], true);
    let r = cmd(&mut f, &json!({"cmd": "insertText", "text": "X"}));
    assert_eq!(r["error"], "read only");
    assert_eq!(plain(&f), "Bản cuối");
    let r = cmd(
        &mut f,
        &json!({"cmd": "editHeaderFooter", "page": 0, "header": true}),
    );
    assert_eq!(r["error"], "read only");
    // Edit anyway: the mark goes on the next save.
    let r = cmd(&mut f, &json!({"cmd": "markFinal", "on": false}));
    assert_eq!(r["security"]["readOnly"], false);
    cmd(&mut f, &json!({"cmd": "insertText", "text": "X"}));
    assert_eq!(plain(&f), "XBản cuối");
    let out = f.save_docx().unwrap();
    let custom = text(&out, "/docProps/custom.xml");
    assert!(!custom.contains("_MarkAsFinal"));
    assert!(custom.contains("MSIP_Label_11111111-2222-3333-4444-555555555555_Name"));
}

#[test]
fn read_only_recommended_and_password_to_modify_in_docx() {
    let mut e = doc("Khuyến nghị");
    cmd(
        &mut e,
        &json!({"cmd": "setReadOnlyRecommended", "on": true}),
    );
    let out = e.save_docx().unwrap();
    let settings = text(&out, "/word/settings.xml");
    // §17.15.1.93: first child of w:settings.
    let start = settings.find("<w:settings").unwrap();
    let after = &settings[settings[start..].find('>').unwrap() + start + 1..];
    assert!(
        after.starts_with(r#"<w:writeProtection w:recommended="1"/>"#),
        "{settings}"
    );
    let mut f = Engine::with_bundled_fonts();
    f.open(&out, "a.docx").unwrap();
    let r = cmd(&mut f, &json!({"cmd": "security"}));
    assert_eq!(r["security"]["readOnlyRecommended"], true);
    assert_eq!(r["security"]["readOnly"], true);
    assert_eq!(
        cmd(&mut f, &json!({"cmd": "insertText", "text": "a"}))["error"],
        "read only"
    );
    let r = cmd(&mut f, &json!({"cmd": "setReadOnly", "on": false}));
    assert_eq!(r["security"]["readOnly"], false);
    cmd(&mut f, &json!({"cmd": "insertText", "text": "a"}));
    assert_eq!(plain(&f), "aKhuyến nghị");

    // Password to modify (hash only, SHA-512).
    cmd(
        &mut f,
        &json!({"cmd": "setReadOnlyRecommended", "on": false}),
    );
    cmd(
        &mut f,
        &json!({"cmd": "setWritePassword", "password": "sửa-được"}),
    );
    let out = f.save_docx().unwrap();
    let settings = text(&out, "/word/settings.xml");
    assert!(settings.contains("<w:writeProtection w:algorithmName=\"SHA-512\""));
    assert!(!settings.contains("sửa-được"));
    let mut g = Engine::with_bundled_fonts();
    g.open(&out, "a.docx").unwrap();
    let r = cmd(&mut g, &json!({"cmd": "setReadOnly", "on": false}));
    assert_eq!(r["error"], "write password required");
    let r = cmd(&mut g, &json!({"cmd": "unlockWrite", "password": "sai"}));
    assert_eq!(r["error"], "wrong protection password");
    let r = cmd(
        &mut g,
        &json!({"cmd": "unlockWrite", "password": "sửa-được"}),
    );
    assert_eq!(r["security"]["readOnly"], false);
    assert!(r.get("error").is_none());
    // Removing it drops the element.
    cmd(&mut g, &json!({"cmd": "setWritePassword", "password": ""}));
    let out = g.save_docx().unwrap();
    assert!(!text(&out, "/word/settings.xml").contains("writeProtection"));
}

fn select(e: &mut Engine, a: usize, b: usize) {
    cmd(
        e,
        &json!({"cmd": "setSelection", "selection": {"anchor": {"p": 0, "o": a}, "focus": {"p": 0, "o": b}}}),
    );
}

#[test]
fn editable_regions_limit_edits_of_a_protected_document() {
    let mut e = doc("Đầu Giữa Cuối");
    let start = "Đầu ".len();
    let end = start + "Giữa".len();
    select(&mut e, start, end);
    let r = cmd(&mut e, &json!({"cmd": "addEditableRegion"}));
    assert_eq!(r["security"]["regions"][0]["group"], "everyone");
    assert_eq!(r["security"]["regions"][0]["mine"], true);
    // Undoable while the document is not protected.
    cmd(&mut e, &json!({"cmd": "undo"}));
    assert!(cmd(&mut e, &json!({"cmd": "security"}))["security"]["regions"].is_null());
    cmd(&mut e, &json!({"cmd": "redo"}));
    // A region for one user only.
    select(&mut e, 0, "Đầu".len());
    cmd(
        &mut e,
        &json!({"cmd": "addEditableRegion", "editor": "an@bluestar.vn"}),
    );
    let r = cmd(
        &mut e,
        &json!({"cmd": "restrictEditing", "password": "khoá"}),
    );
    assert_eq!(r["security"]["protection"]["edit"], "readOnly");
    assert_eq!(r["security"]["protection"]["enforced"], true);
    assert_eq!(
        r["security"]["regions"][0]["mine"], false,
        "{}",
        r["security"]
    );
    assert_eq!(
        r["security"]["regions"][1]["end"]["o"], end,
        "{}",
        r["security"]
    );

    // Outside: refused, nothing changes.
    let len = plain(&e).len();
    select(&mut e, len, len);
    let r = cmd(&mut e, &json!({"cmd": "insertText", "text": "!"}));
    assert_eq!(
        r["error"],
        "document protected",
        "{} {}",
        plain(&e),
        r["selection"]
    );
    assert_eq!(plain(&e), "Đầu Giữa Cuối");
    // Formatting across the boundary is refused too.
    select(&mut e, 0, len);
    let r = cmd(
        &mut e,
        &json!({"cmd": "formatText", "format": {"kind": "bold"}}),
    );
    assert_eq!(r["error"], "document protected");
    // The other user's region is not mine.
    select(&mut e, 1, 1);
    assert_eq!(
        cmd(&mut e, &json!({"cmd": "insertText", "text": "x"}))["error"],
        "document protected"
    );

    // "Find next region I can edit" selects the everyone region.
    select(&mut e, 0, 0);
    let r = cmd(&mut e, &json!({"cmd": "nextEditableRegion"}));
    assert_eq!(r["selection"]["anchor"]["o"], start);
    assert_eq!(r["selection"]["focus"]["o"], end);
    // Inside: typing replaces the region's text, at its start too.
    let r = cmd(&mut e, &json!({"cmd": "insertText", "text": "Mới"}));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(plain(&e), "Đầu Mới Cuối");
    select(&mut e, start, start);
    cmd(&mut e, &json!({"cmd": "insertText", "text": "["}));
    assert_eq!(plain(&e), "Đầu [Mới Cuối");
    let r = cmd(&mut e, &json!({"cmd": "deleteBackward"}));
    assert!(r.get("error").is_none());
    // Backspace at the region start would delete protected text.
    let r = cmd(&mut e, &json!({"cmd": "deleteBackward"}));
    assert_eq!(r["error"], "document protected");
    assert_eq!(plain(&e), "Đầu Mới Cuối");
    // Undo of the accepted edits still works.
    cmd(&mut e, &json!({"cmd": "undo"}));
    assert_eq!(plain(&e), "Đầu [Mới Cuối");

    // The named user may edit their region.
    cmd(
        &mut e,
        &json!({"cmd": "setIdentity", "names": ["Nguyễn An", "an@bluestar.vn"]}),
    );
    select(&mut e, 1, 1);
    let r = cmd(&mut e, &json!({"cmd": "insertText", "text": "x"}));
    assert!(r.get("error").is_none(), "{r}");
    // Regions cannot be changed while protected.
    let r = cmd(&mut e, &json!({"cmd": "removeEditableRegion", "id": "1"}));
    assert_eq!(r["error"], "document protected");
    // Highlight rectangles of my regions.
    let r = cmd(&mut e, &json!({"cmd": "showEditableRegions", "on": true}));
    assert!(r["security"]["regionRects"].as_array().unwrap().len() >= 2);

    // Round trip: markers in the body, protection in the settings.
    let out = e.save_docx().unwrap();
    let body = text(&out, "/word/document.xml");
    assert!(
        body.contains(r#"<w:permStart w:id="1" w:edGrp="everyone"/>"#),
        "{body}"
    );
    assert!(body.contains(r#"<w:permEnd w:id="1"/>"#));
    assert!(body.contains(r#"<w:permStart w:id="2" w:ed="an@bluestar.vn"/>"#));
    let settings = text(&out, "/word/settings.xml");
    assert!(settings.contains(
        r#"<w:documentProtection w:edit="readOnly" w:enforcement="1" w:algorithmName="SHA-512""#
    ));
    let mut f = Engine::with_bundled_fonts();
    f.open(&out, "a.docx").unwrap();
    let r = cmd(&mut f, &json!({"cmd": "security"}));
    assert_eq!(r["security"]["protection"]["enforced"], true);
    assert_eq!(r["security"]["protection"]["hasPassword"], true);
    let flen = plain(&f).len();
    select(&mut f, flen, flen);
    assert_eq!(
        cmd(&mut f, &json!({"cmd": "insertText", "text": "!"}))["error"],
        "document protected"
    );
    let r = cmd(
        &mut f,
        &json!({"cmd": "stopRestrictEditing", "password": "sai"}),
    );
    assert_eq!(r["error"], "wrong protection password");
    cmd(
        &mut f,
        &json!({"cmd": "stopRestrictEditing", "password": "khoá"}),
    );
    let r = cmd(&mut f, &json!({"cmd": "insertText", "text": "!"}));
    assert!(r.get("error").is_none());
    let out = f.save_docx().unwrap();
    assert!(!text(&out, "/word/settings.xml").contains("documentProtection"));
}

#[test]
fn blueoffice_label_with_header_marking() {
    let src = with_labels(&doc("Có nhãn").save_docx().unwrap());
    let mut e = Engine::with_bundled_fonts();
    e.open(&src, "a.docx").unwrap();
    let r = cmd(
        &mut e,
        &json!({"cmd": "setLabel", "label": {"id": "noi-bo", "name": "Nội bộ", "marking": "TÀI LIỆU NỘI BỘ"}}),
    );
    assert_eq!(r["security"]["label"]["name"], "Nội bộ");
    let headers = &e.document().unwrap().sections[0].headers;
    let h = headers.default.as_ref().unwrap();
    assert_eq!(h[0].as_paragraph().unwrap().plain_text(), "TÀI LIỆU NỘI BỘ");
    let out = e.save_docx().unwrap();
    let custom = text(&out, "/docProps/custom.xml");
    assert!(custom.contains(r#"name="BO_Label_Name"><vt:lpwstr>Nội bộ</vt:lpwstr>"#));
    assert!(custom.contains(r#"name="BO_Label_Id"><vt:lpwstr>noi-bo</vt:lpwstr>"#));
    // The other product's label stays as it was.
    assert!(custom.contains("MSIP_Label_11111111-2222-3333-4444-555555555555_Name"));
    assert_eq!(
        part(&out, "/docMetadata/LabelInfo.xml"),
        part(&src, "/docMetadata/LabelInfo.xml")
    );
    let mut f = Engine::with_bundled_fonts();
    f.open(&out, "a.docx").unwrap();
    let r = cmd(&mut f, &json!({"cmd": "security"}));
    assert_eq!(r["security"]["label"]["marking"], "TÀI LIỆU NỘI BỘ");
    // Changing the label replaces the marking; removing it removes both.
    cmd(
        &mut f,
        &json!({"cmd": "setLabel", "label": {"name": "Mật", "marking": "MẬT"}}),
    );
    let h = f.document().unwrap().sections[0]
        .headers
        .default
        .clone()
        .unwrap();
    assert_eq!(h[0].as_paragraph().unwrap().plain_text(), "MẬT");
    assert!(h.iter().all(|b| b
        .as_paragraph()
        .is_none_or(|p| p.plain_text() != "TÀI LIỆU NỘI BỘ")));
    cmd(&mut f, &json!({"cmd": "setLabel"}));
    let out = f.save_docx().unwrap();
    assert!(!text(&out, "/docProps/custom.xml").contains("BO_Label"));
}

#[test]
fn new_document_gets_a_custom_properties_part() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("n.docx").unwrap();
    cmd(&mut e, &json!({"cmd": "markFinal", "on": true}));
    let out = e.save_docx().unwrap();
    let pkg = Package::read(&out, &Limits::default()).unwrap();
    let props = bo_opc::custom::read_custom_properties(&pkg).unwrap();
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "_MarkAsFinal");
    let _ = reopen(&e);
}

#[test]
fn sheet_protected_ranges_and_file_sharing() {
    let mut s = SheetEngine::new();
    s.new_workbook("a.xlsx").unwrap();
    let r = sheet(
        &mut s,
        &json!({"cmd": "addProtectedRange", "sheet": 0, "name": "Nhập liệu", "sqref": "B2:C3"}),
    );
    assert_eq!(r["ranges"][0]["sqref"], "B2:C3");
    sheet(
        &mut s,
        &json!({"cmd": "addProtectedRange", "sheet": 0, "name": "Kho", "sqref": "E5", "password": "kho"}),
    );
    sheet(
        &mut s,
        &json!({"cmd": "protectSheet", "sheet": 0, "password": ""}),
    );
    // Ranges cannot change while the sheet is protected.
    let r = sheet(
        &mut s,
        &json!({"cmd": "removeProtectedRange", "sheet": 0, "name": "Kho"}),
    );
    assert_eq!(r["error"], "sheet protected");
    // B2 is editable, A1 is not, E5 only after its password.
    s.set_inputs(0, &[(1, 1, "12".into())]).unwrap();
    assert!(s.set_inputs(0, &[(0, 0, "x".into())]).is_err());
    assert!(s.set_inputs(0, &[(4, 4, "1".into())]).is_err());
    let r = sheet(
        &mut s,
        &json!({"cmd": "unlockProtectedRange", "sheet": 0, "name": "Kho", "password": "sai"}),
    );
    assert_eq!(r["error"], "wrong protection password");
    sheet(
        &mut s,
        &json!({"cmd": "unlockProtectedRange", "sheet": 0, "name": "Kho", "password": "kho"}),
    );
    s.set_inputs(0, &[(4, 4, "1".into())]).unwrap();

    // fileSharing: read-only recommended, written at its schema slot.
    sheet(
        &mut s,
        &json!({"cmd": "setReadOnlyRecommended", "on": true}),
    );
    sheet(&mut s, &json!({"cmd": "markFinal", "on": true}));
    let out = s.save("xlsx", 0).unwrap();
    let wb = text(&out, "/xl/workbook.xml");
    let fs = wb
        .find(r#"<fileSharing readOnlyRecommended="1"/>"#)
        .unwrap();
    assert!(fs < wb.find("<workbookPr").unwrap_or(usize::MAX), "{wb}");
    assert!(fs < wb.find("<sheets").unwrap());
    let ws = text(&out, "/xl/worksheets/sheet1.xml");
    assert!(
        ws.contains(r#"<protectedRange sqref="B2:C3" name="Nhập liệu"/>"#),
        "{ws}"
    );
    assert!(ws.contains(r#"name="Kho" "#) || ws.contains(r#"name="Kho"/>"#));
    assert!(text(&out, "/docProps/custom.xml").contains("_MarkAsFinal"));

    let mut t = SheetEngine::new();
    t.open(&out, "a.xlsx").unwrap();
    let r = sheet(&mut t, &json!({"cmd": "security"}));
    assert_eq!(r["security"]["readOnly"], true);
    assert_eq!(r["security"]["markedFinal"], true);
    assert_eq!(r["security"]["readOnlyRecommended"], true);
    assert!(t.set_inputs(0, &[(1, 1, "1".into())]).is_err());
    assert!(t
        .exec_json(&json!({"cmd": "format", "sheet": 0, "range": {"r0": 1, "c0": 1, "r1": 1, "c1": 1}, "patch": {"bold": true}}).to_string())
        .is_err());
    sheet(&mut t, &json!({"cmd": "markFinal", "on": false}));
    t.set_inputs(0, &[(1, 1, "1".into())]).unwrap();
    let r = sheet(&mut t, &json!({"cmd": "protectedRanges", "sheet": 0}));
    assert_eq!(r["ranges"].as_array().unwrap().len(), 2);
    assert_eq!(r["ranges"][1]["hasPassword"], true);
    assert_eq!(r["ranges"][1]["unlocked"], false);

    // Password to modify, undoable (a workbook model change).
    sheet(
        &mut t,
        &json!({"cmd": "setReadOnlyRecommended", "on": false}),
    );
    sheet(
        &mut t,
        &json!({"cmd": "setWritePassword", "password": "pw"}),
    );
    let out = t.save("xlsx", 0).unwrap();
    assert!(text(&out, "/xl/workbook.xml").contains("<fileSharing algorithmName=\"SHA-512\""));
    let mut u = SheetEngine::new();
    u.open(&out, "a.xlsx").unwrap();
    let r = sheet(&mut u, &json!({"cmd": "unlockWrite", "password": "pw"}));
    assert_eq!(r["security"]["readOnly"], false);
    assert!(u.undo().unwrap() || !u.can_undo());
}

#[test]
fn untouched_workbook_custom_properties_stay_byte_identical() {
    let mut s = SheetEngine::new();
    s.new_workbook("a.xlsx").unwrap();
    let src = with_labels(&s.save("xlsx", 0).unwrap());
    let mut t = SheetEngine::new();
    t.open(&src, "a.xlsx").unwrap();
    let r = sheet(&mut t, &json!({"cmd": "security"}));
    assert_eq!(
        r["security"]["foreignLabels"][0]["name"],
        "Confidential-Test"
    );
    let out = t.save("xlsx", 0).unwrap();
    assert_eq!(
        part(&out, "/docProps/custom.xml"),
        part(&src, "/docProps/custom.xml")
    );
    assert_eq!(
        part(&out, "/docMetadata/LabelInfo.xml"),
        part(&src, "/docMetadata/LabelInfo.xml")
    );
    sheet(
        &mut t,
        &json!({"cmd": "setLabel", "label": {"name": "Công khai"}}),
    );
    let out = t.save("xlsx", 0).unwrap();
    assert!(text(&out, "/docProps/custom.xml").contains("Công khai"));
}

fn deck() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/slide/slide-text-bullets.pptx"),
    )
    .unwrap()
}

#[test]
fn presentation_password_to_modify_and_final_mark() {
    let src = with_labels(&deck());
    let mut e = Engine::with_bundled_fonts();
    e.open(&src, "a.pptx").unwrap();
    let r = slide(&mut e, &json!({"cmd": "security"}));
    assert_eq!(
        r["security"]["foreignLabels"][0]["name"],
        "Confidential-Test"
    );
    // Untouched: package parts unchanged.
    let out = e.save_pptx().unwrap();
    for n in [
        "/docProps/custom.xml",
        "/docMetadata/LabelInfo.xml",
        "/ppt/presentation.xml",
    ] {
        assert_eq!(part(&out, n), part(&src, n), "{n}");
    }
    // PresentationML has no read-only recommendation.
    let r = slide(
        &mut e,
        &json!({"cmd": "setReadOnlyRecommended", "on": true}),
    );
    assert_eq!(r["error"], "unsupported");
    let r = slide(
        &mut e,
        &json!({"cmd": "setWritePassword", "password": "trình chiếu"}),
    );
    assert_eq!(r["modified"], true);
    slide(&mut e, &json!({"cmd": "markFinal", "on": true}));
    let out = e.save_pptx().unwrap();
    let p = text(&out, "/ppt/presentation.xml");
    assert!(
        p.contains("<p:modifyVerifier algorithmName=\"SHA-512\""),
        "{p}"
    );
    assert!(p.contains("spinValue=\"100000\""));
    // §19.2.1.26: modifyVerifier comes after defaultTextStyle, before extLst.
    if let Some(d) = p.find("</p:defaultTextStyle>") {
        assert!(d < p.find("<p:modifyVerifier").unwrap());
    }

    let mut f = Engine::with_bundled_fonts();
    f.open(&out, "a.pptx").unwrap();
    let r = slide(&mut f, &json!({"cmd": "security"}));
    assert_eq!(r["security"]["readOnly"], true);
    assert_eq!(r["security"]["writePassword"], true);
    let r = slide(&mut f, &json!({"cmd": "addSlide"}));
    assert_eq!(r["error"], "read only");
    let r = slide(
        &mut f,
        &json!({"cmd": "unlockWrite", "password": "trình chiếu"}),
    );
    // Still final until "edit anyway".
    assert_eq!(r["security"]["readOnly"], true);
    let r = slide(&mut f, &json!({"cmd": "markFinal", "on": false}));
    assert_eq!(r["security"]["readOnly"], false);
    let n = f.slide_count();
    let r = slide(&mut f, &json!({"cmd": "addSlide"}));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(f.slide_count(), n + 1);
}

/// `BO_SECURITY_OUT=<dir>`: writes one protected file per format for the
/// OpenXmlValidator (`bo-cli validate <dir> --openxml require`).
#[test]
fn write_protected_samples_for_validation() {
    let Some(dir) = std::env::var_os("BO_SECURITY_OUT") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut e = doc("Đầu Giữa Cuối");
    select(&mut e, "Đầu ".len(), "Đầu Giữa".len());
    cmd(&mut e, &json!({"cmd": "addEditableRegion"}));
    select(&mut e, 0, "Đầu".len());
    cmd(
        &mut e,
        &json!({"cmd": "addEditableRegion", "editor": "an@bluestar.vn"}),
    );
    cmd(
        &mut e,
        &json!({"cmd": "restrictEditing", "password": "khoá"}),
    );
    cmd(
        &mut e,
        &json!({"cmd": "setReadOnlyRecommended", "on": true}),
    );
    cmd(
        &mut e,
        &json!({"cmd": "setWritePassword", "password": "sửa"}),
    );
    cmd(
        &mut e,
        &json!({"cmd": "setLabel", "label": {"name": "Mật", "marking": "MẬT"}}),
    );
    cmd(&mut e, &json!({"cmd": "markFinal", "on": true}));
    std::fs::write(dir.join("sec-doc.docx"), e.save_docx().unwrap()).unwrap();

    let mut s = SheetEngine::new();
    s.new_workbook("a.xlsx").unwrap();
    sheet(
        &mut s,
        &json!({"cmd": "addProtectedRange", "sheet": 0, "name": "Nhap", "sqref": "B2:C3", "password": "x"}),
    );
    sheet(
        &mut s,
        &json!({"cmd": "protectSheet", "sheet": 0, "password": ""}),
    );
    sheet(
        &mut s,
        &json!({"cmd": "setReadOnlyRecommended", "on": true}),
    );
    sheet(
        &mut s,
        &json!({"cmd": "setWritePassword", "password": "pw"}),
    );
    sheet(
        &mut s,
        &json!({"cmd": "setLabel", "label": {"name": "Nội bộ"}}),
    );
    sheet(&mut s, &json!({"cmd": "markFinal", "on": true}));
    std::fs::write(dir.join("sec-sheet.xlsx"), s.save("xlsx", 0).unwrap()).unwrap();

    let mut p = Engine::with_bundled_fonts();
    p.open(&deck(), "a.pptx").unwrap();
    slide(
        &mut p,
        &json!({"cmd": "setWritePassword", "password": "pw"}),
    );
    slide(
        &mut p,
        &json!({"cmd": "setLabel", "label": {"name": "Công khai"}}),
    );
    slide(&mut p, &json!({"cmd": "markFinal", "on": true}));
    std::fs::write(dir.join("sec-slide.pptx"), p.save_pptx().unwrap()).unwrap();
}
