//! Legacy form fields and "filling in forms" protection through the JSON command bridge
//! (ECMA-376 Part 1, §17.16.17 ffData, §17.15.1.29 documentProtection).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use serde_json::{json, Value};

#[allow(clippy::needless_pass_by_value)] // test helper: called with `json!` temporaries
fn cmd(e: &mut Engine, v: Value) -> Value {
    serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap()
}

fn text(e: &Engine) -> String {
    e.document().unwrap().plain_text()
}

/// Change the field at the caret with `f` applied to its JSON.
fn set_props(e: &mut Engine, f: impl FnOnce(&mut Value)) -> Value {
    let r = cmd(e, json!({"cmd": "state"}));
    let mut field = r["formField"]["field"].clone();
    assert!(field.is_object(), "no form field at the caret: {r}");
    f(&mut field);
    cmd(e, json!({"cmd": "setFormField", "field": field}))
}

/// "Họ tên: [text]  Đồng ý: [check]" / "Thành phố: [list]".
fn form() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("mẫu.docx").unwrap();
    cmd(&mut e, json!({"cmd": "insertText", "text": "Họ tên: "}));
    cmd(&mut e, json!({"cmd": "insertFormField", "kind": "text"}));
    cmd(&mut e, json!({"cmd": "insertText", "text": " Đồng ý: "}));
    cmd(
        &mut e,
        json!({"cmd": "insertFormField", "kind": "checkBox"}),
    );
    cmd(&mut e, json!({"cmd": "splitParagraph"}));
    cmd(&mut e, json!({"cmd": "insertText", "text": "Thành phố: "}));
    cmd(
        &mut e,
        json!({"cmd": "insertFormField", "kind": "dropDown"}),
    );
    let r = set_props(&mut e, |f| {
        f["entries"] = json!(["Hà Nội", "Huế", "Đà Nẵng"]);
    });
    assert!(r.get("error").is_none(), "{r}");
    e
}

const EMPTY: &str = "\u{2002}\u{2002}\u{2002}\u{2002}\u{2002}";

#[test]
fn fill_in_a_protected_form_and_save() {
    let mut e = form();
    assert_eq!(
        text(&e),
        format!("Họ tên: {EMPTY} Đồng ý: \u{2610}\nThành phố: Hà Nội")
    );
    let r = cmd(
        &mut e,
        json!({"cmd": "protectDocument", "password": "bí mật"}),
    );
    assert_eq!(r["protection"]["edit"], "forms");
    assert_eq!(r["protection"]["enforced"], true);
    assert_eq!(r["protection"]["hasPassword"], true);

    // Outside the fields nothing changes.
    cmd(
        &mut e,
        json!({"cmd": "setSelection", "selection": {"anchor": {"p": 0, "o": 0}, "focus": {"p": 0, "o": 0}}}),
    );
    let r = cmd(&mut e, json!({"cmd": "insertText", "text": "X"}));
    assert_eq!(r["error"], "forms:protected");
    let r = cmd(
        &mut e,
        json!({"cmd": "formatText", "format": {"kind": "bold"}}),
    );
    assert_eq!(r["error"], "forms:protected");
    let r = cmd(&mut e, json!({"cmd": "insertFormField", "kind": "text"}));
    assert_eq!(r["error"], "forms:protected");
    assert!(text(&e).starts_with("Họ tên: "));

    // Tab selects the text field's result; typing replaces it.
    let r = cmd(&mut e, json!({"cmd": "tab"}));
    assert_eq!(r["formField"]["field"]["name"], "Text1");
    let start = "Họ tên: ".len();
    assert_eq!(r["selection"]["anchor"]["o"], start);
    assert_eq!(r["selection"]["focus"]["o"], start + EMPTY.len());
    for c in ["L", "a", "n"] {
        cmd(&mut e, json!({"cmd": "insertText", "text": c}));
    }
    let r = cmd(&mut e, json!({"cmd": "deleteBackward"}));
    assert_eq!(r["formField"]["value"], "La");
    cmd(&mut e, json!({"cmd": "insertText", "text": "n"}));
    // Enter is refused inside a form field.
    let r = cmd(&mut e, json!({"cmd": "splitParagraph"}));
    assert_eq!(r["error"], "forms:protected");

    // The check box: Space toggles it.
    let r = cmd(&mut e, json!({"cmd": "tab"}));
    assert_eq!(
        r["formField"]["field"]["name"],
        "Check1",
        "{} {}",
        r["selection"],
        text(&e)
    );
    let r = cmd(&mut e, json!({"cmd": "insertText", "text": " "}));
    assert_eq!(r["formField"]["value"], "\u{2612}");
    // The list: an entry is picked.
    let r = cmd(&mut e, json!({"cmd": "tab"}));
    assert_eq!(r["formField"]["field"]["name"], "Dropdown1");
    assert_eq!(
        r["formField"]["field"]["entries"],
        json!(["Hà Nội", "Huế", "Đà Nẵng"])
    );
    cmd(&mut e, json!({"cmd": "setFormFieldValue", "value": "Huế"}));
    // Wraps around; Shift+Tab goes back.
    let r = cmd(&mut e, json!({"cmd": "tab"}));
    assert_eq!(r["formField"]["field"]["name"], "Text1");
    let r = cmd(&mut e, json!({"cmd": "tab", "back": true}));
    assert_eq!(r["formField"]["field"]["name"], "Dropdown1");
    assert_eq!(text(&e), "Họ tên: Lan Đồng ý: \u{2612}\nThành phố: Huế");

    // Saved and opened again: still protected, filled in.
    let saved = e.save_docx().unwrap();
    // For OpenXmlValidator: `BO_FORMS_OUT=<dir> cargo test -p bo-engine --test forms`, then
    // `bo-cli validate <dir> --openxml require`.
    if let Ok(dir) = std::env::var("BO_FORMS_OUT") {
        std::fs::write(std::path::Path::new(&dir).join("forms-engine.docx"), &saved).unwrap();
    }
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "mẫu.docx").unwrap();
    assert_eq!(text(&f), "Họ tên: Lan Đồng ý: \u{2612}\nThành phố: Huế");
    let r = cmd(&mut f, json!({"cmd": "state"}));
    assert_eq!(r["protection"]["enforced"], true);
    let r = cmd(&mut f, json!({"cmd": "insertText", "text": "X"}));
    assert_eq!(r["error"], "forms:protected");

    // Stopping needs the password.
    let r = cmd(
        &mut f,
        json!({"cmd": "unprotectDocument", "password": "sai"}),
    );
    assert_eq!(r["error"], "forms:password");
    assert_eq!(r["protection"]["enforced"], true);
    let r = cmd(
        &mut f,
        json!({"cmd": "unprotectDocument", "password": "bí mật"}),
    );
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(r["protection"]["enforced"], false);
    cmd(
        &mut f,
        json!({"cmd": "setSelection", "selection": {"anchor": {"p": 0, "o": 0}, "focus": {"p": 0, "o": 0}}}),
    );
    cmd(&mut f, json!({"cmd": "insertText", "text": "X"}));
    assert!(text(&f).starts_with("XHọ tên"));
}

#[test]
fn leaving_fields_formats_validates_and_recalculates() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, json!({"cmd": "insertText", "text": "SL: "}));
    cmd(&mut e, json!({"cmd": "insertFormField", "kind": "text"}));
    set_props(&mut e, |f| {
        f["name"] = json!("SoLuong");
        f["calcOnExit"] = json!(true);
        f["type"] = json!("number");
        f["format"] = json!("#,##0");
    });
    cmd(&mut e, json!({"cmd": "insertText", "text": " = "}));
    cmd(
        &mut e,
        json!({"cmd": "insertField", "instr": "REF SoLuong"}),
    );
    cmd(&mut e, json!({"cmd": "protectDocument"}));
    let r = cmd(&mut e, json!({"cmd": "tab"}));
    assert_eq!(r["formField"]["field"]["name"], "SoLuong");
    // Letters are not typed into a number field.
    cmd(&mut e, json!({"cmd": "insertText", "text": "12a500"}));
    // Leaving: formatted, and the REF field shows the new value.
    let r = cmd(&mut e, json!({"cmd": "tab"}));
    assert!(r.get("error").is_none(), "{r}");
    assert_eq!(text(&e), "SL: 12,500 = 12,500");
    // Not a number: the field keeps the selection and says why.
    cmd(
        &mut e,
        json!({"cmd": "setFormFieldValue", "value": "1.2.3"}),
    );
    let r = cmd(&mut e, json!({"cmd": "move", "motion": "docStart"}));
    assert_eq!(r["error"], "forms:number");
    assert_eq!(r["formField"]["field"]["name"], "SoLuong");
}

#[test]
fn unprotected_sections_stay_editable() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, json!({"cmd": "insertText", "text": "Mẫu"}));
    cmd(
        &mut e,
        json!({"cmd": "insertSectionBreak", "kind": "continuous"}),
    );
    cmd(&mut e, json!({"cmd": "insertText", "text": "Ghi chú"}));
    let r = cmd(
        &mut e,
        json!({"cmd": "protectDocument", "unprotectedSections": [1]}),
    );
    assert_eq!(r["protection"]["unprotectedSections"], json!([1]));
    assert_eq!(r["protection"]["sections"], 2);
    let r = cmd(&mut e, json!({"cmd": "insertText", "text": "!"}));
    assert!(r.get("error").is_none(), "{r}");
    assert!(text(&e).ends_with("Ghi chú!"));
    cmd(
        &mut e,
        json!({"cmd": "setSelection", "selection": {"anchor": {"p": 0, "o": 0}, "focus": {"p": 0, "o": 0}}}),
    );
    let r = cmd(&mut e, json!({"cmd": "insertText", "text": "!"}));
    assert_eq!(r["error"], "forms:protected");
    // The section exception is saved (§17.6.6 formProt).
    let saved = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    let r = cmd(&mut f, json!({"cmd": "state"}));
    assert_eq!(r["protection"]["unprotectedSections"], json!([1]));
}

#[test]
fn fill_in_the_corpus_form() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/doc/controls-02-legacy-forms.docx");
    let bytes = std::fs::read(path).unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "controls-02-legacy-forms.docx").unwrap();
    let r = cmd(&mut e, json!({"cmd": "state"}));
    assert_eq!(r["protection"]["enforced"], true);
    assert_eq!(r["protection"]["unprotectedSections"], json!([1]));
    // Name (title case on leaving), date (formatted), quantity (calcOnExit).
    let r = cmd(&mut e, json!({"cmd": "tab"}));
    assert_eq!(r["formField"]["field"]["name"], "HoTen");
    assert_eq!(r["formField"]["field"]["entryMacro"], "VaoHoTen");
    assert_eq!(
        r["formField"]["field"]["helpText"]["text"],
        "Ghi đầy đủ họ tên"
    );
    cmd(
        &mut e,
        json!({"cmd": "insertText", "text": "trần thị bình"}),
    );
    let r = cmd(&mut e, json!({"cmd": "tab"}));
    assert_eq!(r["formField"]["field"]["name"], "NgaySinh");
    cmd(&mut e, json!({"cmd": "insertText", "text": "5/3/1991"}));
    let r = cmd(&mut e, json!({"cmd": "tab"}));
    assert_eq!(r["formField"]["field"]["name"], "SoLuong");
    cmd(&mut e, json!({"cmd": "insertText", "text": "4"}));
    // The calculated (disabled) total is skipped.
    let r = cmd(&mut e, json!({"cmd": "tab"}));
    assert_eq!(r["formField"]["field"]["name"], "DongY");
    cmd(&mut e, json!({"cmd": "insertText", "text": " "}));
    let lines: Vec<String> = text(&e).lines().map(str::to_owned).collect();
    assert_eq!(
        lines[..6],
        [
            "Họ và tên: Trần Thị Bình",
            "Ngày sinh: 05/03/1991",
            "Số lượng: 4   Thành tiền: 600,000",
            "\u{2610} Đồng ý điều khoản   \u{2610} Nhận thư",
            "Tỉnh/thành: Huế",
            "Xác nhận: Trần Thị Bình",
        ]
    );
    // Saved: valid, every ffData child kept (macros too), read back the same.
    let saved = e.save_docx().unwrap();
    if let Ok(dir) = std::env::var("BO_FORMS_OUT") {
        std::fs::write(
            std::path::Path::new(&dir).join("forms-corpus-filled.docx"),
            &saved,
        )
        .unwrap();
    }
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    assert_eq!(text(&f), text(&e));
    let doc = f.document().unwrap();
    let fields: Vec<bo_model::FormField> = doc
        .paragraphs()
        .flat_map(|p| p.runs.iter().flat_map(|r| &r.content))
        .filter_map(|c| match c {
            bo_model::RunContent::FormField(f) => Some((**f).clone()),
            _ => None,
        })
        .collect();
    assert_eq!(fields.len(), 7);
    assert_eq!(fields[5].exit_macro.as_deref(), Some("RaNhanThu"));
    assert_eq!(
        fields[5].kind,
        bo_model::FormFieldKind::CheckBox(bo_model::form::FormCheckBox {
            size: Some(28),
            default: false,
            checked: None,
        })
    );
}
