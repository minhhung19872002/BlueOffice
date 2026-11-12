//! BlueSheet checkboxes in cells, pictures placed in cells and `IMAGE` pictures
//! through the command bridge (SHEET-CELL-001): one undo step per edit, the
//! two-step Delete of checkbox cells, saving and reopening, and the asynchronous
//! fetch of `IMAGE` pictures with an injected (fake) host.
#![allow(clippy::unwrap_used, clippy::needless_pass_by_value)]

use base64::Engine as _;
use bo_engine::sheet::ViewRequest;
use bo_engine::SheetEngine;
use bo_render::Command;
use serde_json::{json, Value};

/// 2×1 PNG (see `bo-xlsx/tests/cell_media.rs`).
const PNG_2X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x7b, 0x40, 0xe8,
    0xdd, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0x00, 0x04,
    0xff, 0x01, 0x07, 0x00, 0x01, 0xff, 0xe2, 0x23, 0x9e, 0x59, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn run(e: &mut SheetEngine, v: Value) -> Value {
    let s = e.exec_json(&v.to_string()).unwrap();
    serde_json::from_str(&s).unwrap_or(Value::Null)
}

fn shown(e: &SheetEngine, row: u32, col: u32) -> String {
    e.cell(0, row, col).unwrap().display
}

fn new_engine() -> SheetEngine {
    let mut e = SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    e
}

fn range(r0: u32, c0: u32, r1: u32, c1: u32) -> Value {
    json!({"r0": r0, "c0": c0, "r1": r1, "c1": c1})
}

fn render(e: &mut SheetEngine) -> Vec<Command> {
    e.render(&ViewRequest {
        sheet: 0,
        scroll_x: 0.0,
        scroll_y: 0.0,
        width: 500.0,
        height: 300.0,
    })
    .unwrap()
    .commands
}

#[test]
fn checkboxes_insert_toggle_and_undo() {
    let mut e = new_engine();
    e.set_input(0, 1, 0, "TRUE").unwrap();
    e.set_input(0, 2, 0, "ghi chú").unwrap();
    run(
        &mut e,
        json!({"cmd": "insertCheckbox", "sheet": 0, "range": range(0, 0, 2, 0)}),
    );
    // Empty A1 becomes FALSE; A2 keeps TRUE; text in A3 stays.
    assert_eq!(shown(&e, 0, 0), "FALSE");
    assert_eq!(shown(&e, 1, 0), "TRUE");
    assert_eq!(shown(&e, 2, 0), "ghi chú");
    // Space on A1:A2 with A1 active: A1 was FALSE → both TRUE.
    let on = run(
        &mut e,
        json!({"cmd": "toggleCheckbox", "sheet": 0, "range": range(0, 0, 1, 0), "row": 0, "col": 0}),
    );
    assert_eq!(on, json!(true));
    assert_eq!(
        (shown(&e, 0, 0), shown(&e, 1, 0)),
        ("TRUE".into(), "TRUE".into())
    );
    // A formula reading the checkbox follows it.
    e.set_input(0, 0, 1, "=IF(A1,\"Xong\",\"Chưa\")").unwrap();
    assert_eq!(shown(&e, 0, 1), "Xong");
    run(
        &mut e,
        json!({"cmd": "toggleCheckbox", "sheet": 0, "range": range(0, 0, 0, 0), "row": 0, "col": 0}),
    );
    assert_eq!(shown(&e, 0, 1), "Chưa");
    // One undo step per toggle.
    assert!(e.undo().unwrap());
    assert_eq!(shown(&e, 0, 0), "TRUE");
    assert_eq!(shown(&e, 0, 1), "Xong");
    // Toggling a cell without a checkbox does nothing.
    let r = run(
        &mut e,
        json!({"cmd": "toggleCheckbox", "sheet": 0, "range": range(5, 5, 5, 5), "row": 5, "col": 5}),
    );
    assert_eq!(r, Value::Null);
}

#[test]
fn delete_clears_values_then_the_control() {
    let mut e = new_engine();
    run(
        &mut e,
        json!({"cmd": "insertCheckbox", "sheet": 0, "range": range(0, 0, 1, 0)}),
    );
    let first = run(
        &mut e,
        json!({"cmd": "deleteCheckboxes", "sheet": 0, "range": range(0, 0, 1, 0)}),
    );
    assert_eq!(first["removed"], "values");
    assert_eq!(shown(&e, 0, 0), "");
    // Still a checkbox: Space checks it.
    run(
        &mut e,
        json!({"cmd": "toggleCheckbox", "sheet": 0, "range": range(0, 0, 0, 0), "row": 0, "col": 0}),
    );
    assert_eq!(shown(&e, 0, 0), "TRUE");
    run(
        &mut e,
        json!({"cmd": "deleteCheckboxes", "sheet": 0, "range": range(0, 0, 1, 0)}),
    );
    let second = run(
        &mut e,
        json!({"cmd": "deleteCheckboxes", "sheet": 0, "range": range(0, 0, 1, 0)}),
    );
    assert_eq!(second["removed"], "checkbox");
    let none = run(
        &mut e,
        json!({"cmd": "deleteCheckboxes", "sheet": 0, "range": range(0, 0, 1, 0)}),
    );
    assert_eq!(none["handled"], false);
    // Without the control a typed TRUE is plain text again in the grid.
    e.set_input(0, 0, 0, "TRUE").unwrap();
    let wb = e.workbook().unwrap();
    let style = wb.sheets[0].cell(0, 0).unwrap().style;
    assert!(!wb.styles.xf(style).is_some_and(|x| x.checkbox));
}

#[test]
fn checkboxes_and_pictures_survive_saving() {
    let mut e = new_engine();
    run(
        &mut e,
        json!({"cmd": "insertCheckbox", "sheet": 0, "range": range(0, 0, 0, 0)}),
    );
    run(
        &mut e,
        json!({"cmd": "toggleCheckbox", "sheet": 0, "range": range(0, 0, 0, 0), "row": 0, "col": 0}),
    );
    run(
        &mut e,
        json!({"cmd": "placeImageInCell", "sheet": 0, "row": 2, "col": 1, "data": b64(PNG_2X1), "ext": "png", "alt": "Logo"}),
    );
    // The picture hides the #VALUE! fallback and is drawn.
    let cmds = render(&mut e);
    assert_eq!(
        cmds.iter()
            .filter(|c| matches!(c, Command::Image { .. }))
            .count(),
        1
    );
    // PDF export embeds the picture (an image XObject).
    let pdf = e.export_pdf().unwrap();
    assert!(pdf.windows(15).any(|w| w == b"/Subtype /Image"));
    let bytes = e.save("xlsx", 0).unwrap();
    let mut back = SheetEngine::new();
    back.open(&bytes, "t.xlsx").unwrap();
    assert_eq!(shown(&back, 0, 0), "TRUE");
    let wb = back.workbook().unwrap();
    assert!(
        wb.styles
            .xf(wb.sheets[0].cell(0, 0).unwrap().style)
            .unwrap()
            .checkbox
    );
    let info = run(
        &mut back,
        json!({"cmd": "cellImageInfo", "sheet": 0, "row": 2, "col": 1}),
    );
    assert_eq!(info["kind"], "local");
    assert_eq!(info["alt"], "Logo");
    // Alt text change, decorative flag.
    run(
        &mut back,
        json!({"cmd": "cellImageAlt", "sheet": 0, "row": 2, "col": 1, "alt": "Biểu trưng", "decorative": true}),
    );
    let info = run(
        &mut back,
        json!({"cmd": "cellImageInfo", "sheet": 0, "row": 2, "col": 1}),
    );
    assert_eq!(
        (info["alt"].clone(), info["decorative"].clone()),
        (json!("Biểu trưng"), json!(true))
    );
    // The display-list id resolves to the bytes.
    let id = back.workbook().unwrap().sheets[0]
        .cell(2, 1)
        .unwrap()
        .image
        .as_ref()
        .unwrap()
        .media_id();
    let media = run(&mut back, json!({"cmd": "cellMedia", "id": id}));
    assert_eq!(media, json!(b64(PNG_2X1)));
    // Typing over the picture replaces it (one undo step brings it back).
    back.set_input(0, 2, 1, "42").unwrap();
    assert_eq!(
        run(
            &mut back,
            json!({"cmd": "cellImageInfo", "sheet": 0, "row": 2, "col": 1})
        ),
        Value::Null
    );
    back.undo().unwrap();
    assert_eq!(
        run(
            &mut back,
            json!({"cmd": "cellImageInfo", "sheet": 0, "row": 2, "col": 1})
        )["kind"],
        "local"
    );
}

#[test]
fn image_function_is_fetched_by_the_host() {
    let mut e = new_engine();
    e.set_input(0, 0, 0, "=IMAGE(\"https://example.vn/a.png\",\"Bản đồ\")")
        .unwrap();
    e.set_input(0, 1, 0, "=IMAGE(\"https://example.vn/b.png\")")
        .unwrap();
    // The engine asks once per URL.
    let urls = run(&mut e, json!({"cmd": "imageRequests"}));
    assert_eq!(
        urls,
        json!(["https://example.vn/a.png", "https://example.vn/b.png"])
    );
    assert_eq!(run(&mut e, json!({"cmd": "imageRequests"})), json!([]));
    assert!(render(&mut e)
        .iter()
        .all(|c| !matches!(c, Command::Image { .. })));
    // Fake host: a.png arrives, b.png is blocked.
    let r = run(
        &mut e,
        json!({"cmd": "imageLoaded", "url": "https://example.vn/a.png", "data": b64(PNG_2X1)}),
    );
    assert_eq!(r["ready"], true);
    run(
        &mut e,
        json!({"cmd": "imageLoaded", "url": "https://example.vn/b.png", "error": "blocked"}),
    );
    let cmds = render(&mut e);
    let ids: Vec<&str> = cmds
        .iter()
        .filter_map(|c| match c {
            Command::Image { image, .. } => Some(image.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(ids, vec!["url:https://example.vn/a.png"]);
    let texts: Vec<&str> = cmds
        .iter()
        .filter_map(|c| match c {
            Command::Glyphs { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    // Glyph runs need fonts; without them only the absence of #VALUE! is checked.
    assert!(!texts.contains(&"#VALUE!"));
    let info = run(
        &mut e,
        json!({"cmd": "cellImageInfo", "sheet": 0, "row": 1, "col": 0}),
    );
    assert_eq!(
        (info["kind"].clone(), info["state"].clone()),
        (json!("web"), json!("failed"))
    );
    assert_eq!(
        run(
            &mut e,
            json!({"cmd": "cellMedia", "id": "url:https://example.vn/a.png"})
        ),
        json!(b64(PNG_2X1))
    );
    // Not a picture: #VALUE!.
    e.set_input(0, 2, 0, "=IMAGE(\"https://example.vn/c.txt\")")
        .unwrap();
    run(&mut e, json!({"cmd": "imageRequests"}));
    let r = run(
        &mut e,
        json!({"cmd": "imageLoaded", "url": "https://example.vn/c.txt", "data": b64(b"hello")}),
    );
    assert_eq!(r["ready"], false);
}

/// [MS-XLSX] §2.3.6.1.11: an `IMAGE` result is saved as a `_webimage` rich value
/// (formula `_xlfn.IMAGE` + `vm`), and the picture is back on opening the file,
/// before any recalculation.
#[test]
fn image_function_results_are_saved_as_web_pictures() {
    let mut e = new_engine();
    e.set_input(
        0,
        0,
        0,
        "=IMAGE(\"https://example.vn/a.png\",\"Bản đồ\",3,30,50)",
    )
    .unwrap();
    let bytes = e.save("xlsx", 0).unwrap();
    let wb = bo_xlsx::read_xlsx(&bytes).unwrap();
    let cell = wb.sheets[0].cell(0, 0).unwrap();
    assert!(cell.vm.is_some());
    assert!(wb.opaque.parts.iter().any(|p| p
        .name
        .to_ascii_lowercase()
        .ends_with("rdrichvaluewebimage.xml")));
    let mut back = SheetEngine::new();
    back.open(&bytes, "a.xlsx").unwrap();
    let info = run(
        &mut back,
        json!({"cmd": "cellImageInfo", "sheet": 0, "row": 0, "col": 0}),
    );
    assert_eq!(info["kind"], "web");
    assert_eq!(info["url"], "https://example.vn/a.png");
    assert_eq!(info["alt"], "Bản đồ");
    assert_eq!(info["sizing"], 3);
    // Saving the reopened file again keeps the same rich value.
    let again = bo_xlsx::read_xlsx(&back.save("xlsx", 0).unwrap()).unwrap();
    assert_eq!(again.sheets[0].cell(0, 0).unwrap().vm, cell.vm);
}

#[test]
fn bad_pictures_are_refused() {
    let mut e = new_engine();
    let r = e.exec_json(
        &json!({"cmd": "placeImageInCell", "sheet": 0, "row": 0, "col": 0, "data": b64(b"not a picture"), "ext": "png"})
            .to_string(),
    );
    assert!(r.is_err());
    assert!(!e.can_undo());
}

/// The bridge tries these commands before `SheetCommand`, `AuditCommand` and
/// `SecurityCommand`; a shared `cmd` name would go to the wrong handler.
#[test]
fn command_names_do_not_shadow_other_commands() {
    fn names(src: &str, name: &str) -> Vec<String> {
        let head = format!("pub enum {name} {{");
        let start = src.find(&head).unwrap() + head.len();
        let mut depth = 1usize;
        let mut out = Vec::new();
        for line in src[start..].lines() {
            if depth == 1 {
                if let Some(rest) = line.strip_prefix("    ") {
                    let ident: String = rest
                        .chars()
                        .take_while(char::is_ascii_alphanumeric)
                        .collect();
                    if ident.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                        let mut c = ident.chars();
                        let first = c.next().unwrap().to_ascii_lowercase();
                        out.push(std::iter::once(first).chain(c).collect());
                    }
                }
            }
            depth += line.matches('{').count();
            depth -= line.matches('}').count();
            if depth == 0 {
                break;
            }
        }
        out
    }
    let mine = names(
        include_str!("../src/sheet/cell_media_cmd.rs"),
        "CellMediaCommand",
    );
    assert!(mine.contains(&"insertCheckbox".to_owned()) && mine.len() >= 11);
    for (src, e) in [
        (include_str!("../src/sheet/cmd.rs"), "SheetCommand"),
        (include_str!("../src/sheet/audit_cmd.rs"), "AuditCommand"),
        (include_str!("../src/security.rs"), "SecurityCommand"),
    ] {
        let other = names(src, e);
        let shared: Vec<&String> = mine.iter().filter(|n| other.contains(n)).collect();
        assert!(shared.is_empty(), "{e}: {shared:?}");
    }
}
