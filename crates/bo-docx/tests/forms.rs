//! Legacy form fields (ECMA-376 Part 1, §17.16.17 ffData) and forms protection
//! (§17.15.1.29 documentProtection, §17.6.6 formProt): reading and lossless writing.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_docx::{read_docx, write_docx, CT_DOCUMENT};
use bo_model::form::{FormFieldKind, TextInputType};
use bo_model::{Block, Document, Paragraph, RunContent};
use bo_opc::{Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const CT_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";

const TEXT_FF: &str = "<w:ffData><w:name w:val=\"Name\"/><w:tabIndex w:val=\"2\"/><w:enabled/><w:calcOnExit/><w:entryMacro w:val=\"OnEnter\"/><w:exitMacro w:val=\"OnExit\"/><w:helpText w:type=\"text\" w:val=\"Your full name\"/><w:statusText w:type=\"text\" w:val=\"Name\"/><w:textInput><w:type w:val=\"regular\"/><w:default w:val=\"Nguyen\"/><w:maxLength w:val=\"10\"/><w:format w:val=\"UPPERCASE\"/></w:textInput></w:ffData>";
const CHECK_FF: &str = "<w:ffData><w:name w:val=\"Agree\"/><w:enabled/><w:calcOnExit w:val=\"0\"/><w:checkBox><w:size w:val=\"28\"/><w:default w:val=\"0\"/><w:checked/></w:checkBox></w:ffData>";
const LIST_FF: &str = "<w:ffData><w:name w:val=\"City\"/><w:enabled/><w:calcOnExit w:val=\"0\"/><w:ddList><w:result w:val=\"1\"/><w:listEntry w:val=\"Hà Nội\"/><w:listEntry w:val=\"Huế\"/><w:listEntry w:val=\"Đà Nẵng\"/></w:ddList></w:ffData>";

fn body() -> String {
    format!(
        "<w:p><w:r><w:t xml:space=\"preserve\">Name: </w:t></w:r><w:bookmarkStart w:id=\"0\" w:name=\"Name\"/><w:r><w:fldChar w:fldCharType=\"begin\">{TEXT_FF}</w:fldChar></w:r><w:r><w:instrText xml:space=\"preserve\"> FORMTEXT </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>Lan</w:t></w:r><w:r><w:t>h</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r><w:bookmarkEnd w:id=\"0\"/><w:r><w:t xml:space=\"preserve\"> ok </w:t></w:r><w:r><w:fldChar w:fldCharType=\"begin\">{CHECK_FF}</w:fldChar></w:r><w:r><w:instrText xml:space=\"preserve\"> FORMCHECKBOX </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>\
<w:p><w:pPr><w:sectPr><w:type w:val=\"continuous\"/><w:formProt w:val=\"0\"/></w:sectPr></w:pPr><w:r><w:t>Free text</w:t></w:r></w:p>\
<w:p><w:r><w:fldChar w:fldCharType=\"begin\">{LIST_FF}</w:fldChar></w:r><w:r><w:instrText xml:space=\"preserve\"> FORMDROPDOWN </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p><w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/></w:sectPr>"
    )
}

const SETTINGS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:settings xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:zoom w:percent=\"100\"/><w:documentProtection w:edit=\"forms\" w:enforcement=\"1\"/><w:defaultTabStop w:val=\"720\"/><w:compat/></w:settings>";

fn docx() -> Vec<u8> {
    let mut p = Package::new();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="{W}"><w:body>{}</w:body></w:document>"#,
        body()
    );
    p.add_part("/word/document.xml", CT_DOCUMENT, xml.into_bytes())
        .unwrap();
    p.add_part(
        "/word/settings.xml",
        CT_SETTINGS,
        SETTINGS.as_bytes().to_vec(),
    )
    .unwrap();
    p.set_relationships(
        "/",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_OFFICE_DOCUMENT.into(),
            target: "/word/document.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    p.set_relationships(
        "/word/document.xml",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_SETTINGS.into(),
            target: "settings.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    p.write().unwrap()
}

fn part(bytes: &[u8], name: &str) -> String {
    let pkg = Package::read(bytes, &bo_opc::Limits::default()).unwrap();
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

fn paras(doc: &Document) -> Vec<&Paragraph> {
    doc.sections
        .iter()
        .flat_map(|s| &s.blocks)
        .filter_map(|b| match b {
            Block::Paragraph(p) => Some(p),
            _ => None,
        })
        .collect()
}

fn items(p: &Paragraph) -> Vec<&RunContent> {
    p.runs.iter().flat_map(|r| &r.content).collect()
}

fn fields(doc: &Document) -> Vec<bo_model::FormField> {
    paras(doc)
        .iter()
        .flat_map(|p| items(p))
        .filter_map(|c| match c {
            RunContent::FormField(f) => Some((**f).clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn reads_every_ff_data_child() {
    let doc = read_docx(&docx()).unwrap();
    let f = fields(&doc);
    assert_eq!(f.len(), 3);
    let t = &f[0];
    assert_eq!(t.name.as_deref(), Some("Name"));
    assert!(t.enabled && t.calc_on_exit);
    assert_eq!(t.entry_macro.as_deref(), Some("OnEnter"));
    assert_eq!(t.exit_macro.as_deref(), Some("OnExit"));
    assert_eq!(
        t.help_text.as_ref().map(|h| h.text.as_str()),
        Some("Your full name")
    );
    assert_eq!(
        t.status_text.as_ref().map(|h| h.text.as_str()),
        Some("Name")
    );
    assert_eq!(t.unknown, vec!["<w:tabIndex w:val=\"2\"/>".to_owned()]);
    assert_eq!(t.instr, " FORMTEXT ");
    assert!(t.separate);
    let FormFieldKind::Text(ti) = &t.kind else {
        panic!("text field")
    };
    assert_eq!(ti.input_type, TextInputType::Regular);
    assert_eq!(
        (ti.default.as_str(), ti.max_length, ti.format.as_str()),
        ("Nguyen", 10, "UPPERCASE")
    );
    // Check box without a separator; checked.
    assert!(!f[1].separate);
    assert_eq!(f[1].display().as_deref(), Some("\u{2612}"));
    let FormFieldKind::CheckBox(c) = &f[1].kind else {
        panic!("check box")
    };
    assert_eq!(c.size, Some(28));
    // Drop-down: the second entry is selected.
    assert_eq!(f[2].display().as_deref(), Some("Huế"));

    // The text field's result is ordinary text between the markers, its formatting kept.
    let p = paras(&doc)[0];
    assert_eq!(p.plain_text(), "Name: Lanh ok \u{2612}");
    let it = items(p);
    let start = it
        .iter()
        .position(|c| matches!(c, RunContent::FormField(_)))
        .unwrap();
    assert_eq!(*it[start + 1], RunContent::Text("Lan".into()));
    assert_eq!(*it[start + 2], RunContent::Text("h".into()));
    assert_eq!(*it[start + 3], RunContent::FormEnd);
    assert_eq!(
        p.runs
            .iter()
            .find(|r| r.content == [RunContent::Text("Lan".into())])
            .and_then(|r| r.props.bold),
        Some(true)
    );

    // Protection and the unprotected section.
    let prot = doc.protection.as_ref().unwrap();
    assert!(prot.forms_enforced());
    assert!(doc.sections[0].form_unprotected);
    assert!(!doc.sections[1].form_unprotected);
}

#[test]
fn unchanged_document_is_byte_identical() {
    let src = docx();
    let doc = read_docx(&src).unwrap();
    let out = write_docx(&doc).unwrap();
    assert_eq!(
        part(&out, "/word/document.xml"),
        part(&src, "/word/document.xml")
    );
    assert_eq!(part(&out, "/word/settings.xml"), SETTINGS);
}

#[test]
fn edited_paragraph_keeps_untouched_ff_data_verbatim() {
    let mut doc = read_docx(&docx()).unwrap();
    // Uncheck the box: its paragraph is written again.
    let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] else {
        panic!("paragraph")
    };
    for r in &mut p.runs {
        for c in &mut r.content {
            if let RunContent::FormField(f) = c {
                if let FormFieldKind::CheckBox(cb) = &mut f.kind {
                    cb.checked = Some(false);
                }
            }
        }
    }
    let out = write_docx(&doc).unwrap();
    let xml = part(&out, "/word/document.xml");
    // The text field's ffData (with the unknown tabIndex) is written as read.
    assert!(xml.contains(TEXT_FF));
    assert!(!xml.contains(CHECK_FF));
    assert!(xml.contains("<w:checked w:val=\"0\"/>"));
    // The bookmark around the text field stays.
    assert!(xml.contains("<w:bookmarkStart w:id=\"0\" w:name=\"Name\"/>"));
    let back = read_docx(&out).unwrap();
    let f = fields(&back);
    assert_eq!(f[1].display().as_deref(), Some("\u{2610}"));
    assert_eq!(f[0].data(), fields(&doc)[0].data());
    assert_eq!(paras(&back)[0].plain_text(), "Name: Lanh ok \u{2610}");
}

#[test]
fn regenerated_document_reads_back_equal() {
    let mut doc = read_docx(&docx()).unwrap();
    let before = fields(&doc);
    let text: Vec<String> = paras(&doc).iter().map(|p| p.plain_text()).collect();
    let prot = doc.protection.clone();
    doc.source = None;
    for s in &mut doc.sections {
        s.src = None;
        for b in &mut s.blocks {
            if let Block::Paragraph(p) = b {
                p.src = None;
            }
        }
    }
    let out = write_docx(&doc).unwrap();
    let xml = part(&out, "/word/document.xml");
    // §17.6.6 formProt written for the unprotected section.
    assert!(xml.contains("<w:formProt w:val=\"0\"/>"));
    let settings = part(&out, "/word/settings.xml");
    assert!(settings.contains("<w:documentProtection w:edit=\"forms\" w:enforcement=\"1\"/>"));
    let back = read_docx(&out).unwrap();
    let after = fields(&back);
    assert_eq!(after.len(), 3);
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(a.data(), b.data());
    }
    let text2: Vec<String> = paras(&back).iter().map(|p| p.plain_text()).collect();
    assert_eq!(text, text2);
    assert_eq!(back.protection, prot);
    assert!(back.sections[0].form_unprotected);
}

#[test]
fn protection_change_patches_settings_only() {
    let src = docx();
    let mut doc = read_docx(&src).unwrap();
    if let Some(p) = doc.protection.as_mut() {
        p.enforced = false;
    }
    let out = write_docx(&doc).unwrap();
    assert_eq!(
        part(&out, "/word/document.xml"),
        part(&src, "/word/document.xml")
    );
    let s = part(&out, "/word/settings.xml");
    assert_eq!(
        s,
        SETTINGS.replace("w:enforcement=\"1\"", "w:enforcement=\"0\"")
    );
    // Removing it drops the element; adding one back puts it before defaultTabStop.
    doc.protection = None;
    let s = part(&write_docx(&doc).unwrap(), "/word/settings.xml");
    assert!(!s.contains("documentProtection"));
    doc.protection = Some(bo_model::DocProtection {
        edit: "forms".into(),
        enforced: true,
        formatting: false,
        attrs: vec![("spinCount".into(), "100000".into())],
    });
    let s = part(&write_docx(&doc).unwrap(), "/word/settings.xml");
    let a = s.find("<w:documentProtection").unwrap();
    assert!(a < s.find("<w:defaultTabStop").unwrap());
    assert!(a > s.find("<w:zoom").unwrap());
}

#[test]
fn incomplete_field_stays_opaque() {
    // A begin with ffData but no end in the paragraph is not collapsed.
    let body = format!(
        "<w:p><w:r><w:fldChar w:fldCharType=\"begin\">{CHECK_FF}</w:fldChar></w:r><w:r><w:instrText> FORMCHECKBOX </w:instrText></w:r></w:p><w:p><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>"
    );
    let mut p = Package::new();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="{W}"><w:body>{body}</w:body></w:document>"#
    );
    p.add_part("/word/document.xml", CT_DOCUMENT, xml.into_bytes())
        .unwrap();
    p.set_relationships(
        "/",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_OFFICE_DOCUMENT.into(),
            target: "/word/document.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    let doc = read_docx(&p.write().unwrap()).unwrap();
    assert_eq!(fields(&doc), vec![]);
}
