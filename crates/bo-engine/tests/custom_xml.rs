//! Custom XML parts and content control data binding through the JSON command bridge
//! (ECMA-376 Part 1, §15.2.4, §15.2.5, §17.5.2.6), on the corpus form
//! `controls-03-data-binding.docx`.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use serde_json::{json, Value};

#[allow(clippy::needless_pass_by_value)] // test helper: called with `json!` temporaries
fn cmd(e: &mut Engine, v: Value) -> Value {
    serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap()
}

fn lines(e: &Engine) -> Vec<String> {
    e.document()
        .unwrap()
        .plain_text()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn caret(e: &mut Engine, p: usize, o: usize) -> Value {
    cmd(
        e,
        json!({"cmd": "setSelection", "selection": {"anchor": {"p": p, "o": o}, "focus": {"p": p, "o": o}}}),
    )
}

const MAPS: &str = "xmlns:ns0='urn:bluestar:corpus:don-hang'";

fn open() -> Engine {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/doc/controls-03-data-binding.docx");
    let bytes = std::fs::read(path).unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "controls-03-data-binding.docx").unwrap();
    e
}

#[test]
fn mapping_pane_lists_parts_and_nodes() {
    let mut e = open();
    let r = cmd(&mut e, json!({"cmd": "customXmlParts"}));
    let parts = r["customXml"].as_array().unwrap();
    assert_eq!(parts.len(), 1);
    let p = &parts[0];
    assert_eq!(p["partName"], "/customXml/item1.xml");
    assert_eq!(p["itemId"], "{3B0F6A52-1C7D-4E8A-9B21-6D4C0E5F7A13}");
    assert_eq!(p["root"], "dh:donHang");
    assert_eq!(p["namespace"], "urn:bluestar:corpus:don-hang");
    assert_eq!(p["prefixMappings"], MAPS);
    // Two text controls on the customer, the order code, the check box, the date and the
    // payment list; the note's node does not exist.
    assert_eq!(p["boundControls"], 6);
    let paths: Vec<&str> = p["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["xpath"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        [
            "/ns0:donHang[1]",
            "/ns0:donHang[1]/@ma",
            "/ns0:donHang[1]/ns0:khachHang[1]",
            "/ns0:donHang[1]/ns0:giaoTanNoi[1]",
            "/ns0:donHang[1]/ns0:ngayGiao[1]",
            "/ns0:donHang[1]/ns0:thanhToan[1]",
        ]
    );
    // The control at the caret reports its binding; the note is unbound.
    let r = caret(&mut e, 1, 13);
    assert_eq!(r["control"]["binding"], "/ns0:donHang[1]/ns0:khachHang[1]");
    assert_eq!(r["control"]["bound"], true);
    let r = caret(&mut e, 4, 12);
    assert_eq!(r["control"]["bound"], false);
}

#[test]
fn node_edits_reach_every_bound_control_and_save() {
    let mut e = open();
    let set = |e: &mut Engine, xpath: &str, value: &str| {
        cmd(
            e,
            json!({"cmd": "customXmlSetValue", "index": 0, "xpath": xpath, "prefixMappings": MAPS, "value": value}),
        )
    };
    set(
        &mut e,
        "/ns0:donHang[1]/ns0:khachHang[1]",
        "Công ty Bình Minh",
    );
    set(&mut e, "/ns0:donHang[1]/@ma", "DH-0100");
    set(&mut e, "/ns0:donHang[1]/ns0:giaoTanNoi[1]", "false");
    set(&mut e, "/ns0:donHang[1]/ns0:ngayGiao[1]", "2027-01-02");
    let r = set(&mut e, "/ns0:donHang[1]/ns0:thanhToan[1]", "TM");
    assert!(r["error"].is_null(), "{r}");
    let r = set(&mut e, "/ns0:donHang[1]", "x");
    assert_eq!(r["error"], "notLeaf");
    assert_eq!(
        lines(&e)[..5],
        [
            "Khách hàng: Công ty Bình Minh   Mã đơn: DH-0100",
            "Kính gửi Công ty Bình Minh,",
            "\u{2610} Giao hàng tận nơi",
            "Ngày giao: 02/01/2027   Hình thức: Tiền mặt",
            "Ghi chú: (không có nút này trong dữ liệu)",
        ]
    );
    let saved = e.save_docx().unwrap();
    if let Ok(dir) = std::env::var("BO_CUSTOM_XML_OUT") {
        std::fs::write(
            std::path::Path::new(&dir).join("custom-xml-edited.docx"),
            &saved,
        )
        .unwrap();
    }
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    assert_eq!(lines(&f)[..5], lines(&e)[..5]);
    let r = cmd(&mut f, json!({"cmd": "customXmlParts"}));
    let khach = r["customXml"][0]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["xpath"] == "/ns0:donHang[1]/ns0:ngayGiao[1]")
        .unwrap()
        .clone();
    // storeMappedDataAs date: xsd:date.
    assert_eq!(khach["value"], "2027-01-02");
}

#[test]
fn typing_in_a_control_updates_its_twin_in_one_undo_step() {
    let mut e = open();
    // "Kính gửi " is 12 bytes; the caret goes after the "C" of the customer.
    caret(&mut e, 1, 13);
    cmd(&mut e, json!({"cmd": "insertText", "text": "X"}));
    assert_eq!(
        lines(&e)[0],
        "Khách hàng: CXông ty Sao Xanh   Mã đơn: DH-0042"
    );
    assert_eq!(lines(&e)[1], "Kính gửi CXông ty Sao Xanh,");
    cmd(&mut e, json!({"cmd": "undo"}));
    assert_eq!(
        lines(&e)[0],
        "Khách hàng: Công ty Sao Xanh   Mã đơn: DH-0042"
    );
    assert_eq!(lines(&e)[1], "Kính gửi Công ty Sao Xanh,");
    // Picking a list item stores the item's value.
    caret(&mut e, 3, 40);
    let r = cmd(
        &mut e,
        json!({"cmd": "setControlValue", "value": "Tiền mặt"}),
    );
    assert!(r["error"].is_null(), "{r}");
    let r = cmd(&mut e, json!({"cmd": "customXmlParts"}));
    let pay = r["customXml"][0]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["xpath"] == "/ns0:donHang[1]/ns0:thanhToan[1]")
        .unwrap()
        .clone();
    assert_eq!(pay["value"], "TM");
}

#[test]
fn add_bind_unbind_and_remove() {
    let mut e = open();
    let r = cmd(
        &mut e,
        json!({"cmd": "customXmlAdd", "xml": "<ghi xmlns=\"urn:note\"><noiDung>Gọi trước khi giao</noiDung></ghi>"}),
    );
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(r["customXml"].as_array().unwrap().len(), 2);
    assert_eq!(r["customXml"][1]["partName"], "/customXml/item2.xml");
    // Bind the unbound note control to the new part.
    caret(&mut e, 4, 12);
    let r = cmd(
        &mut e,
        json!({"cmd": "customXmlBind", "index": 1, "xpath": "/ns0:ghi[1]/ns0:noiDung[1]", "prefixMappings": "xmlns:ns0='urn:note'"}),
    );
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(r["control"]["bound"], true);
    assert_eq!(lines(&e)[4], "Ghi chú: Gọi trước khi giao");
    let r = cmd(&mut e, json!({"cmd": "customXmlUnbind"}));
    assert!(r["control"]["binding"].is_null());
    let r = cmd(&mut e, json!({"cmd": "customXmlAdd", "xml": "<a><b></a>"}));
    assert_eq!(r["error"], "badXml");
    let r = cmd(&mut e, json!({"cmd": "customXmlRemove", "index": 0}));
    assert_eq!(r["customXml"].as_array().unwrap().len(), 1);
    // The controls keep what they showed.
    assert_eq!(lines(&e)[1], "Kính gửi Công ty Sao Xanh,");
    let saved = e.save_docx().unwrap();
    if let Ok(dir) = std::env::var("BO_CUSTOM_XML_OUT") {
        std::fs::write(
            std::path::Path::new(&dir).join("custom-xml-replaced.docx"),
            &saved,
        )
        .unwrap();
    }
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    let r = cmd(&mut f, json!({"cmd": "customXmlParts"}));
    assert_eq!(r["customXml"][0]["root"], "ghi");
    assert_eq!(r["customXml"][0]["partName"], "/customXml/item2.xml");
}
