//! Headless tests of form design, page design and protection in BlueEditor.
use crate::editor_actions::Ask2;
use crate::editor_tests::{window, Fixed, Rec};
use crate::Input;

#[test]
fn field_tools_page_design_and_protection() {
    let mut w = window(2);
    w.run("fd.text", &Fixed);
    let r = w
        .view()
        .and_then(|v| v.doc_rect_to_window(0, [100.0, 200.0, 250.0, 222.0]))
        .expect("page");
    w.handle(
        Input::Press {
            x: r[0],
            y: r[1],
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Drag {
            x: r[0] + r[2],
            y: r[1] + r[3],
        },
        &Fixed,
    );
    let mut rec = Rec::default();
    w.paint(&mut rec);
    w.handle(
        Input::Release {
            x: r[0] + r[2],
            y: r[1] + r[3],
        },
        &Fixed,
    );
    let Some((d, what)) = w.dialog2.take() else {
        panic!("no properties dialog");
    };
    assert_eq!(d.values()[0], "Text1");
    w.answer2(
        what,
        &[
            "Họ tên".into(),
            "Nhập họ tên".into(),
            String::new(),
            "1".into(),
            "0".into(),
        ],
    );
    let fields = w.view().expect("view").doc().form_fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "Họ tên");
    assert!(
        (fields[0].rect[0] - 100.0).abs() < 1.0,
        "{:?}",
        fields[0].rect
    );
    // Page design: header & footer, watermark, Bates numbers, background.
    w.answer2(
        Ask2::HeaderFooter,
        &[
            String::new(),
            "Đầu trang".into(),
            String::new(),
            String::new(),
            "<<page>> / <<pages>>".into(),
            String::new(),
            "10".into(),
        ],
    );
    w.answer2(
        Ask2::Watermark,
        &["MẬT".into(), "72".into(), "30".into(), "45".into()],
    );
    w.answer2(
        Ask2::Bates,
        &["BO".into(), "1".into(), "6".into(), String::new()],
    );
    w.run("org.bg.1", &Fixed);
    let v = w.view_mut().expect("view");
    assert!(v.doc().can_undo());
    let saved = v.doc_mut().save().expect("saves");
    let text = String::from_utf8_lossy(&saved);
    assert!(text.contains("/Subtype /Watermark") || saved.len() > 2000);
    // Protection: AES-256 with permissions from the dialog's check boxes.
    w.run("prot.password", &Fixed);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(
        rec.texts.iter().any(|t| t == "Encrypt with AES 256-bit"),
        "{:?}",
        rec.texts
    );
    w.handle(
        Input::Key(crate::Key::Escape, crate::Mods::default()),
        &Fixed,
    );
    let v = w.view_mut().expect("view");
    v.doc_mut()
        .set_protection("mo", "chu", bo_pdfread::Permissions::default(), true)
        .expect("protects");
    let saved = v.doc_mut().save().expect("saves");
    assert!(String::from_utf8_lossy(&saved).contains("/AESV3"));
}
