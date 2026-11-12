//! Style management (DOC-STYLE-001): modify / new / delete (reassign) / select all /
//! update to match, Style Inspector with clear buttons, Reveal Formatting sources,
//! the Organizer (copy with based-on chains, links and numbering; to a file) and the
//! round trip of style XML (ECMA-376 Part 1, §17.7).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_model::{
    AbstractNum, Block, Color, Document, HalfPoints, Level, Num, NumFormat, NumRef, Paragraph, Raw,
    Run, RunProps, Style, StyleKind, Twips,
};
use serde_json::{json, Value};

fn cmd(e: &mut Engine, v: &Value) -> Value {
    let r: Value = serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap();
    assert!(r.get("error").is_none(), "{v}: {:?}", r.get("error"));
    r
}

#[test]
fn modifying_a_gallery_builtin_adds_it() {
    let mut e = Engine::with_bundled_fonts();
    e.load(bo_engine::plain_text_document("một"), "n.docx")
        .unwrap();
    assert!(e.document().unwrap().style("Heading3").is_none());
    let r = cmd(&mut e, &json!({"cmd":"styleDetails","id":"Heading3"}));
    let mut spec = r["styleMgr"]["spec"].clone();
    assert_eq!(spec["bold"], true);
    spec["italic"] = json!(true);
    cmd(
        &mut e,
        &json!({"cmd":"styleModify","id":"Heading3","spec":spec}),
    );
    let d = e.document().unwrap();
    assert_eq!(d.style("Heading3").unwrap().rpr.italic, Some(true));
    assert!(d.style("Normal").is_some());
    cmd(&mut e, &json!({"cmd":"undo"}));
    assert!(e.document().unwrap().style("Heading3").is_none());
}

fn cmd_err(e: &mut Engine, v: &Value) -> String {
    let r: Value = serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap();
    r["error"].as_str().unwrap_or_default().to_owned()
}

fn para(text: &str, style: Option<&str>) -> Block {
    let mut p = Paragraph::default();
    p.props.style = style.map(str::to_owned);
    p.runs.push(Run::text(text, RunProps::default()));
    Block::Paragraph(p)
}

fn style(id: &str, name: &str, kind: StyleKind, based_on: Option<&str>) -> Style {
    let mut s = Style::new(id, name, kind);
    s.based_on = based_on.map(str::to_owned);
    s.quick = false;
    s
}

/// Normal (Arial, default) · Heading1 (bold, 16pt, linked Heading1Char) · Heading2
/// (with an unmodelled `w:rsid`) · Strong (character); docDefaults 12pt.
fn base_doc() -> Document {
    let mut d = bo_engine::plain_text_document("x");
    d.sections[0].blocks = vec![
        para("Một", Some("Heading1")),
        para("nội dung", None),
        para("Hai", Some("Heading1")),
        para("Ba", Some("Heading2")),
    ];
    d.default_run.size = Some(HalfPoints(24));
    let mut normal = style("Normal", "Normal", StyleKind::Paragraph, None);
    normal.is_default = true;
    normal.quick = true;
    normal.rpr.font = Some("Arial".into());
    let mut h1 = style(
        "Heading1",
        "heading 1",
        StyleKind::Paragraph,
        Some("Normal"),
    );
    h1.quick = true;
    h1.ui_priority = Some(9);
    h1.linked = Some("Heading1Char".into());
    h1.next = Some("Normal".into());
    h1.rpr.bold = Some(true);
    h1.rpr.size = Some(HalfPoints(32));
    let mut h1c = style("Heading1Char", "Heading 1 Char", StyleKind::Character, None);
    h1c.linked = Some("Heading1".into());
    h1c.rpr.bold = Some(true);
    let mut h2 = style(
        "Heading2",
        "heading 2",
        StyleKind::Paragraph,
        Some("Heading1"),
    );
    h2.rpr.size = Some(HalfPoints(28));
    h2.extra.push(Raw::new(r#"<w:rsid w:val="00AB12CD"/>"#, 0));
    let mut strong = style("Strong", "Strong", StyleKind::Character, None);
    strong.rpr.bold = Some(true);
    strong.semi_hidden = true;
    strong.unhide_when_used = true;
    d.styles.styles = vec![normal, h1, h1c, h2, strong];
    d
}

fn engine(d: Document) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.load(d, "styles.docx").unwrap();
    e
}

fn reopen(e: &Engine) -> Engine {
    let bytes = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "x.docx").unwrap();
    f
}

fn caret(e: &mut Engine, p: usize, o: usize) {
    cmd(
        e,
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":p,"o":o},"focus":{"p":p,"o":o}}}),
    );
}

fn reveal(e: &mut Engine, key: &str) -> Value {
    let r = cmd(e, &json!({"cmd":"revealFormatting"}));
    r["styleMgr"]["reveal"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["key"] == key && i["group"] != "section")
        .cloned()
        .unwrap_or(Value::Null)
}

#[test]
fn modify_heading1_colour_changes_every_heading_and_round_trips() {
    let mut e = engine(base_doc());
    let r = cmd(&mut e, &json!({"cmd":"styleDetails","id":"Heading1"}));
    let mut spec = r["styleMgr"]["spec"].clone();
    assert_eq!(spec["name"], "heading 1");
    assert_eq!(spec["size"], 32);
    assert_eq!(spec["font"], "Arial", "inherited from Normal");
    spec["color"] = json!("C00000");
    spec["spaceBefore"] = json!(480);
    cmd(
        &mut e,
        &json!({"cmd":"styleModify","id":"Heading1","spec":spec}),
    );
    for p in [0, 2] {
        caret(&mut e, p, 1);
        let c = reveal(&mut e, "color");
        assert_eq!(c["value"], "C00000");
        assert_eq!(c["source"], "paragraphStyle");
        assert_eq!(c["style"], "heading 1");
    }
    // Heading 2 is based on Heading 1: it follows.
    caret(&mut e, 3, 0);
    assert_eq!(reveal(&mut e, "color")["value"], "C00000");
    // Only the changed fields became the style's own: the font stays inherited.
    let h1 = e.document().unwrap().style("Heading1").unwrap().clone();
    assert_eq!(h1.rpr.font, None);
    assert_eq!(h1.ppr.space_before, Some(Twips(480)));
    // Save and reopen: the colour is kept, Heading 2 keeps its unmodelled rsid.
    let mut f = reopen(&e);
    let d = f.document().unwrap();
    assert_eq!(
        d.style("Heading1").unwrap().rpr.color,
        Color::from_hex("C00000")
    );
    assert_eq!(
        d.style("Heading1").unwrap().linked.as_deref(),
        Some("Heading1Char")
    );
    assert_eq!(d.style("Heading1").unwrap().ui_priority, Some(9));
    let h2 = d.style("Heading2").unwrap();
    assert!(
        h2.extra.iter().any(|r| r.xml.contains("00AB12CD")),
        "{:?}",
        h2.extra
    );
    caret(&mut f, 2, 0);
    assert_eq!(reveal(&mut f, "color")["value"], "C00000");
    // One undo step.
    cmd(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(
        e.document().unwrap().style("Heading1").unwrap().rpr.color,
        None
    );
}

#[test]
fn unchanged_styles_are_written_verbatim() {
    let mut e = engine(base_doc());
    let f = reopen(&e);
    let before = f
        .document()
        .unwrap()
        .style("Heading2")
        .unwrap()
        .src
        .clone()
        .unwrap();
    let mut g = engine(f.document().unwrap().clone());
    let r = cmd(&mut g, &json!({"cmd":"styleDetails","id":"Heading1"}));
    let mut spec = r["styleMgr"]["spec"].clone();
    spec["italic"] = json!(true);
    cmd(
        &mut g,
        &json!({"cmd":"styleModify","id":"Heading1","spec":spec}),
    );
    let h = reopen(&g);
    let after = h
        .document()
        .unwrap()
        .style("Heading2")
        .unwrap()
        .src
        .clone()
        .unwrap();
    assert_eq!(before.xml, after.xml);
    assert_eq!(
        h.document().unwrap().style("Heading1").unwrap().rpr.italic,
        Some(true)
    );
    cmd(&mut e, &json!({"cmd":"styleDetails","id":"Heading1"}));
}

#[test]
fn new_style_from_selection_takes_the_formatting_and_applies() {
    let mut d = base_doc();
    if let Block::Paragraph(p) = &mut d.sections[0].blocks[1] {
        p.props.align = Some(bo_model::Alignment::Center);
        p.runs[0].props.italic = Some(true);
        p.runs[0].props.size = Some(HalfPoints(30));
    }
    let mut e = engine(d);
    caret(&mut e, 1, 2);
    let r = cmd(
        &mut e,
        &json!({"cmd":"styleNew","spec":{"name":"Ghi chu","type":"paragraph","basedOn":"Normal","addToGallery":true},"fromSelection":true}),
    );
    let id = r["styleMgr"]["created"].as_str().unwrap().to_owned();
    assert_eq!(id, "Ghichu");
    let doc = e.document().unwrap();
    let st = doc.style(&id).unwrap();
    assert!(st.custom && st.quick);
    assert_eq!(st.rpr.italic, Some(true));
    assert_eq!(st.ppr.align, Some(bo_model::Alignment::Center));
    let Block::Paragraph(p) = &doc.sections[0].blocks[1] else {
        panic!()
    };
    assert_eq!(p.props.style.as_deref(), Some("Ghichu"));
    assert_eq!(
        p.props.align, None,
        "direct formatting moved into the style"
    );
    assert!(r["styles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == "Ghichu"));
    // Same name again is refused.
    let err = cmd_err(
        &mut e,
        &json!({"cmd":"styleNew","spec":{"name":"ghi chu"},"fromSelection":false}),
    );
    assert_eq!(err, "nameTaken");
    cmd(&mut e, &json!({"cmd":"undo"}));
    assert!(e.document().unwrap().style("Ghichu").is_none());
}

#[test]
fn delete_reassigns_to_based_on_and_rebases_children() {
    let mut d = base_doc();
    let mut quote = style("Quote", "Quote", StyleKind::Paragraph, Some("Heading1"));
    quote.rpr.color = Color::from_hex("336699");
    let mut child = style(
        "QuoteChild",
        "Quote child",
        StyleKind::Paragraph,
        Some("Quote"),
    );
    child.rpr.italic = Some(true);
    d.styles.styles.push(quote);
    d.styles.styles.push(child);
    d.sections[0].blocks.push(para("trích", Some("Quote")));
    d.sections[0].blocks.push(para("con", Some("QuoteChild")));
    let mut e = engine(d);
    cmd(&mut e, &json!({"cmd":"styleDelete","id":"Quote"}));
    let doc = e.document().unwrap();
    assert!(doc.style("Quote").is_none());
    let Block::Paragraph(p) = &doc.sections[0].blocks[4] else {
        panic!()
    };
    assert_eq!(p.props.style.as_deref(), Some("Heading1"));
    let c = doc.style("QuoteChild").unwrap();
    assert_eq!(c.based_on.as_deref(), Some("Heading1"));
    assert_eq!(c.rpr.color, Color::from_hex("336699"), "looks the same");
    // A character style without a base: its runs lose rStyle.
    assert_eq!(
        cmd_err(&mut e, &json!({"cmd":"styleDelete","id":"Normal"})),
        "defaultStyle"
    );
    assert_eq!(
        cmd_err(&mut e, &json!({"cmd":"styleDelete","id":"Nope"})),
        "noStyle"
    );
    cmd(&mut e, &json!({"cmd":"undo"}));
    assert!(e.document().unwrap().style("Quote").is_some());
}

#[test]
fn select_all_instances_and_update_to_match() {
    let mut d = base_doc();
    if let Block::Paragraph(p) = &mut d.sections[0].blocks[2] {
        p.runs[0].props.color = Color::from_hex("00AA00");
        p.props.space_after = Some(Twips(360));
    }
    let mut e = engine(d);
    let r = cmd(&mut e, &json!({"cmd":"styleSelectAll","id":"Heading1"}));
    let m = r["matches"].as_array().unwrap();
    assert_eq!(m.len(), 2);
    assert_eq!(m[0]["p"], 0);
    assert_eq!(m[1]["p"], 2);
    assert_eq!(r["selection"]["focus"]["p"], 0);
    assert_eq!(r["highlights"].as_array().unwrap().len(), 2);
    // The default paragraph style covers paragraphs that name none.
    let r = cmd(&mut e, &json!({"cmd":"styleSelectAll","id":"Normal"}));
    assert_eq!(r["matches"].as_array().unwrap().len(), 1);
    caret(&mut e, 2, 1);
    cmd(&mut e, &json!({"cmd":"styleUpdateToMatch","id":"Heading1"}));
    let h1 = e.document().unwrap().style("Heading1").unwrap().clone();
    assert_eq!(h1.rpr.color, Color::from_hex("00AA00"));
    assert_eq!(h1.ppr.space_after, Some(Twips(360)));
    caret(&mut e, 0, 1);
    assert_eq!(reveal(&mut e, "color")["value"], "00AA00");
}

#[test]
fn inspector_levels_and_clear_buttons() {
    let mut d = base_doc();
    if let Block::Paragraph(p) = &mut d.sections[0].blocks[1] {
        p.props.align = Some(bo_model::Alignment::Right);
        p.runs[0].props.style = Some("Strong".into());
        p.runs[0].props.italic = Some(true);
    }
    let mut e = engine(d);
    caret(&mut e, 1, 2);
    let r = cmd(&mut e, &json!({"cmd":"styleInspector"}));
    let i = &r["styleMgr"]["inspector"];
    assert_eq!(i["paragraphStyle"]["id"], "Normal");
    assert_eq!(
        i["paragraphDirect"],
        json!([{"key":"align","value":"right"}])
    );
    assert_eq!(i["characterStyle"]["name"], "Strong");
    assert_eq!(
        i["characterDirect"],
        json!([{"key":"italic","value":"true"}])
    );
    let r = cmd(
        &mut e,
        &json!({"cmd":"styleClear","level":"characterDirect"}),
    );
    let i = &r["styleMgr"]["inspector"];
    assert_eq!(i["characterDirect"], json!([]));
    assert_eq!(i["characterStyle"]["id"], "Strong");
    let r = cmd(
        &mut e,
        &json!({"cmd":"styleClear","level":"characterStyle"}),
    );
    assert!(r["styleMgr"]["inspector"]["characterStyle"].is_null());
    let r = cmd(
        &mut e,
        &json!({"cmd":"styleClear","level":"paragraphDirect"}),
    );
    assert_eq!(r["styleMgr"]["inspector"]["paragraphDirect"], json!([]));
    caret(&mut e, 0, 0);
    let r = cmd(
        &mut e,
        &json!({"cmd":"styleClear","level":"paragraphStyle"}),
    );
    assert_eq!(r["styleMgr"]["inspector"]["paragraphStyle"]["id"], "Normal");
    // Each clear is one undo step.
    cmd(&mut e, &json!({"cmd":"undo"}));
    caret(&mut e, 0, 0);
    let r = cmd(&mut e, &json!({"cmd":"styleInspector"}));
    assert_eq!(
        r["styleMgr"]["inspector"]["paragraphStyle"]["id"],
        "Heading1"
    );
}

#[test]
fn reveal_formatting_names_the_source_of_each_value() {
    let mut d = base_doc();
    if let Block::Paragraph(p) = &mut d.sections[0].blocks[3] {
        p.runs[0].props.italic = Some(true);
    }
    let mut e = engine(d);
    caret(&mut e, 3, 1);
    // Heading2 → Heading1 → Normal; docDefaults 12pt.
    let size = reveal(&mut e, "size");
    assert_eq!(
        (size["value"].as_str(), size["source"].as_str()),
        (Some("14pt"), Some("paragraphStyle"))
    );
    assert_eq!(size["style"], "heading 2");
    let bold = reveal(&mut e, "bold");
    assert_eq!(bold["style"], "heading 1");
    let font = reveal(&mut e, "font");
    assert_eq!(
        (font["value"].as_str(), font["style"].as_str()),
        (Some("Arial"), Some("Normal"))
    );
    assert_eq!(reveal(&mut e, "italic")["source"], "direct");
    caret(&mut e, 1, 0);
    let size = reveal(&mut e, "size");
    assert_eq!(
        (size["value"].as_str(), size["source"].as_str()),
        (Some("12pt"), Some("default"))
    );
    let r = cmd(&mut e, &json!({"cmd":"revealFormatting"}));
    let sec: Vec<&Value> = r["styleMgr"]["reveal"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["group"] == "section")
        .collect();
    assert!(sec
        .iter()
        .any(|i| i["key"] == "marginLeft" && i["source"] == "section"));
}

#[test]
fn style_list_marks_recommended_in_use_and_shows_linked_once() {
    let mut e = engine(base_doc());
    let r = cmd(&mut e, &json!({"cmd":"styleList"}));
    let l = r["styles"].as_array().unwrap();
    assert!(
        !l.iter().any(|s| s["id"] == "Heading1Char"),
        "linked pair listed once"
    );
    let h1 = l.iter().find(|s| s["id"] == "Heading1").unwrap();
    assert_eq!(h1["linked"], "Heading1Char");
    assert_eq!(h1["uiPriority"], 9);
    assert_eq!(h1["inUse"], true);
    let strong = l.iter().find(|s| s["id"] == "Strong").unwrap();
    assert_eq!(
        (strong["recommended"].as_bool(), strong["inUse"].as_bool()),
        (Some(false), Some(false))
    );
    let normal = l.iter().find(|s| s["id"] == "Normal").unwrap();
    assert_eq!(
        (normal["inUse"].as_bool(), normal["isDefault"].as_bool()),
        (Some(true), Some(true))
    );
}

/// Other file: Base ← Mid ← Top (numbered, linked TopChar) plus numbering 1.
fn other_doc() -> Document {
    let mut d = bo_engine::plain_text_document("khác");
    let mut normal = style("Normal", "Normal", StyleKind::Paragraph, None);
    normal.is_default = true;
    let mut base = style("Base", "Base", StyleKind::Paragraph, Some("Normal"));
    base.rpr.color = Color::from_hex("AA0000");
    let mut mid = style("Mid", "Mid", StyleKind::Paragraph, Some("Base"));
    mid.rpr.size = Some(HalfPoints(40));
    let mut top = style("Top", "Top", StyleKind::Paragraph, Some("Mid"));
    top.linked = Some("TopChar".into());
    top.ppr.num = Some(NumRef { num_id: 1, ilvl: 0 });
    let mut top_c = style("TopChar", "Top Char", StyleKind::Character, None);
    top_c.linked = Some("Top".into());
    let mut h1 = style(
        "Heading1",
        "heading 1",
        StyleKind::Paragraph,
        Some("Normal"),
    );
    h1.rpr.color = Color::from_hex("0000FF");
    d.styles.styles = vec![normal, base, mid, top, top_c, h1];
    d.numbering.abstracts.push(AbstractNum {
        id: 0,
        levels: vec![Level {
            ilvl: 0,
            start: 1,
            format: NumFormat::UpperRoman,
            text: "%1.".into(),
            align: bo_model::Alignment::Left,
            ppr: bo_model::ParaProps::default(),
            rpr: RunProps::default(),
            restart: None,
            suffix: "tab".into(),
            pstyle: Some("Top".into()),
            is_lgl: false,
        }],
    });
    d.numbering.nums.push(Num {
        id: 1,
        abstract_id: 0,
        start_overrides: Vec::new(),
    });
    d
}

fn b64(bytes: &[u8]) -> String {
    bo_engine::ole::encode(bytes)
}

#[test]
fn organizer_copies_with_based_on_chain_links_and_numbering() {
    let other = bo_docx::write_docx(&other_doc()).unwrap();
    let file = b64(&other);
    let mut d = base_doc();
    // The current document already uses numId 1 / abstractNumId 0.
    d.numbering = other_doc().numbering;
    d.numbering.abstracts[0].levels[0].format = NumFormat::Decimal;
    let mut e = engine(d);
    let r = cmd(&mut e, &json!({"cmd":"styleOrganizer","file":file}));
    let o = &r["styleMgr"]["organizer"];
    assert!(o["file"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == "Top"));
    assert!(o["current"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == "Heading2"));
    let r = cmd(
        &mut e,
        &json!({"cmd":"styleOrganizerApply","file":file,"action":"copy","ids":["Top","Heading1"]}),
    );
    let copied: Vec<&str> = r["styleMgr"]["organizer"]["copied"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for id in ["Top", "Mid", "Base", "TopChar", "Heading1"] {
        assert!(copied.contains(&id), "{copied:?}");
    }
    let doc = e.document().unwrap();
    assert_eq!(doc.style("Top").unwrap().based_on.as_deref(), Some("Mid"));
    assert_eq!(doc.style("Mid").unwrap().based_on.as_deref(), Some("Base"));
    // Heading 1 is replaced (blue), Normal kept (Arial, still the default).
    assert_eq!(
        doc.style("Heading1").unwrap().rpr.color,
        Color::from_hex("0000FF")
    );
    assert_eq!(
        doc.style("Normal").unwrap().rpr.font.as_deref(),
        Some("Arial")
    );
    // The numbering came along under a new id; the old list is untouched.
    let n = doc.style("Top").unwrap().ppr.num.unwrap();
    assert_ne!(n.num_id, 1);
    let lvl = doc.numbering.level(n).unwrap();
    assert_eq!(lvl.format, NumFormat::UpperRoman);
    assert_eq!(
        doc.numbering
            .level(NumRef { num_id: 1, ilvl: 0 })
            .unwrap()
            .format,
        NumFormat::Decimal
    );
    // Resolves like in the source: red from Base, 20pt from Mid.
    let (_, rp) = doc.styles.resolve_paragraph("Top");
    assert_eq!(
        (rp.color, rp.size),
        (Color::from_hex("AA0000"), Some(HalfPoints(40)))
    );
    // Round trip.
    let f = reopen(&e);
    assert!(f
        .document()
        .unwrap()
        .style("Top")
        .unwrap()
        .ppr
        .num
        .is_some());
    // One undo step.
    cmd(&mut e, &json!({"cmd":"undo"}));
    assert!(e.document().unwrap().style("Top").is_none());
    assert_eq!(e.document().unwrap().numbering.nums.len(), 1);
}

#[test]
fn organizer_writes_the_other_file() {
    let file = b64(&bo_docx::write_docx(&other_doc()).unwrap());
    let mut e = engine(base_doc());
    let r = cmd(
        &mut e,
        &json!({"cmd":"styleOrganizerApply","file":file,"action":"copy","ids":["Heading2"],"toFile":true}),
    );
    let out = r["styleMgr"]["organizer"]["output"].as_str().unwrap();
    let bytes = bo_engine::merge::base64_decode(out).unwrap();
    let d = bo_docx::read_docx(&bytes).unwrap();
    // Heading 2 overwrote nothing; its base Heading 1 already exists there (kept blue).
    assert_eq!(
        d.style("Heading2").unwrap().based_on.as_deref(),
        Some("Heading1")
    );
    assert_eq!(
        d.style("Heading1").unwrap().rpr.color,
        Color::from_hex("0000FF")
    );
    assert!(d
        .style("Heading2")
        .unwrap()
        .extra
        .iter()
        .any(|r| r.xml.contains("00AB12CD")));
    // The current document did not change.
    assert_eq!(r["modified"], false);
    // Rename and delete in the file.
    let file2 = out.to_owned();
    let r = cmd(
        &mut e,
        &json!({"cmd":"styleOrganizerApply","file":file2,"action":"rename","ids":["Base"],"toFile":true,"newName":"Nền"}),
    );
    let out = r["styleMgr"]["organizer"]["output"]
        .as_str()
        .unwrap()
        .to_owned();
    let d = bo_docx::read_docx(&bo_engine::merge::base64_decode(&out).unwrap()).unwrap();
    assert_eq!(d.style("Base").unwrap().name, "Nền");
    let r = cmd(
        &mut e,
        &json!({"cmd":"styleOrganizerApply","file":out,"action":"delete","ids":["Mid"],"toFile":true}),
    );
    let out = r["styleMgr"]["organizer"]["output"]
        .as_str()
        .unwrap()
        .to_owned();
    let d = bo_docx::read_docx(&bo_engine::merge::base64_decode(&out).unwrap()).unwrap();
    assert!(d.style("Mid").is_none());
    assert_eq!(d.style("Top").unwrap().based_on.as_deref(), Some("Base"));
    assert_eq!(d.style("Top").unwrap().rpr.size, Some(HalfPoints(40)));
    // Rename in the current document (one undo step).
    let r = cmd(
        &mut e,
        &json!({"cmd":"styleOrganizerApply","file":out,"action":"rename","ids":["Heading2"],"newName":"Tiêu đề 2"}),
    );
    assert_eq!(r["modified"], true);
    assert_eq!(
        e.document().unwrap().style("Heading2").unwrap().name,
        "Tiêu đề 2"
    );
    assert_eq!(
        cmd_err(&mut e, &json!({"cmd":"styleOrganizer","file":"!!"})),
        "badFile"
    );
}

#[test]
fn corpus_styles_survive_modify_and_round_trip() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/doc");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut n = 0;
    for ent in entries.flatten() {
        let path = ent.path();
        if path.extension().and_then(|x| x.to_str()) != Some("docx") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let mut e = Engine::with_bundled_fonts();
        if e.open(&bytes, "c.docx").is_err() {
            continue;
        }
        let before = e.document().unwrap().styles.clone();
        let Some(target) = before
            .styles
            .iter()
            .find(|s| s.kind == StyleKind::Paragraph)
            .map(|s| s.id.clone())
        else {
            continue;
        };
        let r = cmd(&mut e, &json!({"cmd":"styleDetails","id":target}));
        let mut spec = r["styleMgr"]["spec"].clone();
        spec["color"] = json!("123456");
        // Protected (forms-only, read-only) documents refuse the edit.
        if !cmd_err(
            &mut e,
            &json!({"cmd":"styleModify","id":target,"spec":spec}),
        )
        .is_empty()
        {
            continue;
        }
        let f = reopen(&e);
        let after = &f.document().unwrap().styles;
        assert_eq!(before.styles.len(), after.styles.len(), "{path:?}");
        for (a, b) in before.styles.iter().zip(&after.styles) {
            assert_eq!(a.id, b.id, "{path:?}");
            assert_eq!(a.extra.len(), b.extra.len(), "{path:?} {}", a.id);
            if a.id != target {
                assert_eq!(a.ppr, b.ppr, "{path:?} {}", a.id);
                assert_eq!(a.rpr, b.rpr, "{path:?} {}", a.id);
            }
        }
        let t = after.styles.iter().find(|s| s.id == target).unwrap();
        assert_eq!(t.rpr.color, Color::from_hex("123456"));
        n += 1;
    }
    assert!(n > 0);
}
