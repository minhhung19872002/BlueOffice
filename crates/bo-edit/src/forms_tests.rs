//! Legacy form fields and forms protection (`forms.rs`).
use crate::forms::{parse_date, FormsAccess};
use crate::{fields, para, Pos, Selection, Session};
use bo_model::form::{
    DocProtection, FormCheckBox, FormDropDown, FormField, FormFieldKind, FormTextInput,
    TextInputType, EMPTY_TEXT_RESULT,
};
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{Block, Document, ParaProps, Paragraph, Run, RunContent, RunProps, Section};

fn doc(paras: &[&str]) -> Document {
    let p = |t: &str| {
        Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run::text(t, RunProps::default())],
        ))
    };
    Document {
        sections: vec![Section::new(paras.iter().map(|t| p(t)).collect())],
        ..Default::default()
    }
}

fn plain(s: &Session, id: usize) -> String {
    let path = body_paragraphs(s.doc())[id].clone();
    para_at(s.doc(), &path)
        .map(Paragraph::plain_text)
        .unwrap_or_default()
}

fn caret(s: &mut Session, p: usize, o: usize) {
    s.set_selection(Selection::caret(Pos::new(p, o)));
}

/// Change the field at the caret.
fn modify(s: &mut Session, f: impl FnOnce(&mut FormField)) {
    let mut d = s.form_field_info().expect("field").field;
    f(&mut d);
    s.set_form_field(&d).expect("set");
}

fn text_input(s: &mut Session, t: FormTextInput) {
    modify(s, |f| f.kind = FormFieldKind::Text(t));
}

fn end_of(s: &Session, id: usize) -> usize {
    s.para_text(id).unwrap_or_default().len()
}

fn markers(s: &Session) -> usize {
    para::flatten(para_at(s.doc(), &body_paragraphs(s.doc())[0]).expect("para"))
        .iter()
        .filter(|a| matches!(a.1, RunContent::FormEnd | RunContent::FormField(_)))
        .count()
}

#[test]
fn insert_each_kind_with_bookmarks_and_undo() {
    let mut s = Session::new(doc(&["Name: "]));
    caret(&mut s, 0, 6);
    s.insert_form_field("text").unwrap();
    s.insert_form_field("checkBox").unwrap();
    s.insert_form_field("dropDown").unwrap();
    s.insert_form_field("checkBox").unwrap();
    assert_eq!(
        plain(&s, 0),
        format!("Name: {EMPTY_TEXT_RESULT}\u{2610}{EMPTY_TEXT_RESULT}\u{2610}")
    );
    let names: Vec<Option<String>> = s.form_fields().into_iter().map(|f| f.field.name).collect();
    assert_eq!(
        names,
        ["Text1", "Check1", "Dropdown1", "Check2"].map(|n| Some(n.to_owned()))
    );
    let marks: Vec<(String, String)> = fields::bookmarks(s.doc())
        .into_iter()
        .map(|b| (b.name, b.text))
        .collect();
    assert_eq!(marks[0], ("Text1".to_owned(), EMPTY_TEXT_RESULT.to_owned()));
    assert_eq!(marks[1], ("Check1".to_owned(), "\u{2610}".to_owned()));
    assert!(s.insert_form_field("button").is_err());
    for _ in 0..4 {
        assert!(s.undo());
    }
    assert_eq!(plain(&s, 0), "Name: ");
    assert!(s.redo());
    assert_eq!(s.form_fields().len(), 1);
}

#[test]
fn check_box_toggles_and_list_selects() {
    let mut s = Session::new(doc(&[""]));
    s.insert_form_field("checkBox").unwrap();
    assert!(s.set_form_value("").unwrap());
    assert_eq!(plain(&s, 0), "\u{2612}");
    assert!(s.set_form_value("0").unwrap());
    assert_eq!(plain(&s, 0), "\u{2610}");
    assert!(s.undo());
    assert_eq!(plain(&s, 0), "\u{2612}");
    // The current state wins over the default; without one the default shows.
    modify(&mut s, |f| {
        f.kind = FormFieldKind::CheckBox(FormCheckBox {
            size: Some(24),
            default: false,
            checked: None,
        });
    });
    assert_eq!(plain(&s, 0), "\u{2610}");

    let mut s = Session::new(doc(&[""]));
    s.insert_form_field("dropDown").unwrap();
    modify(&mut s, |f| {
        f.kind = FormFieldKind::DropDown(FormDropDown {
            result: None,
            default: None,
            entries: vec!["Hà Nội".into(), "Huế".into(), "Cần Thơ".into()],
        });
    });
    assert_eq!(plain(&s, 0), "Hà Nội");
    assert!(s.set_form_value("Cần Thơ").unwrap());
    assert_eq!(plain(&s, 0), "Cần Thơ");
    assert!(s.set_form_value("1").unwrap());
    assert_eq!(plain(&s, 0), "Huế");
    assert!(!s.set_form_value("Đà Lạt").unwrap());
    assert!(!s.set_form_value("7").unwrap());
    // Removing entries drops a selection past the end.
    modify(&mut s, |f| {
        if let FormFieldKind::DropDown(d) = &mut f.kind {
            d.entries.truncate(1);
        }
    });
    assert_eq!(plain(&s, 0), "Hà Nội");
    // The type of a field cannot change.
    let mut d = s.form_field_info().unwrap().field;
    d.kind = FormFieldKind::Text(FormTextInput::default());
    assert!(s.set_form_field(&d).is_err());
}

#[test]
fn typing_respects_max_length_and_placeholder() {
    let mut s = Session::new(doc(&["A "]));
    caret(&mut s, 0, 2);
    s.insert_form_field("text").unwrap();
    text_input(
        &mut s,
        FormTextInput {
            max_length: 4,
            ..FormTextInput::default()
        },
    );
    // The caret is at the end of the placeholder: typing replaces it.
    assert!(s.form_type("Hanoi").unwrap());
    assert_eq!(plain(&s, 0), "A Hano");
    assert_eq!(s.selection(), Selection::caret(Pos::new(0, 6)));
    assert!(!s.form_type("x").unwrap());
    assert!(s.form_delete(true).unwrap());
    assert_eq!(plain(&s, 0), "A Han");
    // Nothing is deleted past the result's ends.
    assert!(!s.form_delete(false).unwrap());
    caret(&mut s, 0, 2);
    assert!(!s.form_delete(true).unwrap());
    assert!(s.form_type("X").unwrap());
    assert_eq!(plain(&s, 0), "A XHan");
    // One undo step per keystroke.
    assert!(s.undo());
    assert_eq!(plain(&s, 0), "A Han");

    // A default text shows while the result still is the placeholder.
    let mut s = Session::new(doc(&[""]));
    s.insert_form_field("text").unwrap();
    text_input(
        &mut s,
        FormTextInput {
            default: "Họ tên".into(),
            ..FormTextInput::default()
        },
    );
    assert_eq!(plain(&s, 0), "Họ tên");
}

#[test]
fn number_and_date_fields_validate_and_format() {
    let mut s = Session::new(doc(&[""]));
    s.insert_form_field("text").unwrap();
    text_input(
        &mut s,
        FormTextInput {
            input_type: TextInputType::Number,
            format: "#,##0.00".into(),
            ..FormTextInput::default()
        },
    );
    // Letters are not number characters.
    assert!(s.form_type("12a34.5").unwrap());
    assert_eq!(plain(&s, 0), "1234.5");
    let span = s.form_at(s.selection().focus).unwrap();
    assert_eq!(s.form_exit(&span), Ok(false));
    assert_eq!(plain(&s, 0), "1,234.50");

    let mut s = Session::new(doc(&[""]));
    s.insert_form_field("text").unwrap();
    text_input(
        &mut s,
        FormTextInput {
            input_type: TextInputType::Date,
            format: "dd/MM/yyyy".into(),
            ..FormTextInput::default()
        },
    );
    s.form_type("2026-3-5").unwrap();
    let span = s.form_at(s.selection().focus).unwrap();
    s.form_exit(&span).unwrap();
    assert_eq!(plain(&s, 0), "05/03/2026");
    s.set_form_value("31/02/2026").unwrap();
    let span = s.form_at(s.selection().focus).unwrap();
    assert!(s.form_exit(&span).is_err());
    assert_eq!(plain(&s, 0), "31/02/2026");

    // A case format of a regular field.
    let mut s = Session::new(doc(&[""]));
    s.insert_form_field("text").unwrap();
    text_input(
        &mut s,
        FormTextInput {
            format: "TITLE CASE".into(),
            ..FormTextInput::default()
        },
    );
    s.form_type("nguyễn văn an").unwrap();
    let span = s.form_at(s.selection().focus).unwrap();
    s.form_exit(&span).unwrap();
    assert_eq!(plain(&s, 0), "Nguyễn Văn An");
}

#[test]
fn protection_limits_edits_to_fields_and_unprotected_sections() {
    let mut d = doc(&["Intro ", "Free"]);
    // The second paragraph in a section of its own, left editable.
    let second = d.sections[0].blocks.remove(1);
    let mut free = Section::new(vec![second]);
    free.form_unprotected = true;
    d.sections.push(free);
    let mut s = Session::new(d);
    caret(&mut s, 0, 6);
    s.insert_form_field("text").unwrap();
    assert_eq!(s.forms_access(), FormsAccess::Free);
    s.set_protection(Some(DocProtection {
        edit: "forms".into(),
        enforced: true,
        ..DocProtection::default()
    }));
    assert!(s.forms_locked());
    caret(&mut s, 0, 2);
    assert_eq!(s.forms_access(), FormsAccess::Denied);
    caret(&mut s, 0, 7);
    assert!(matches!(s.forms_access(), FormsAccess::Text(_)));
    caret(&mut s, 1, 2);
    assert_eq!(s.forms_access(), FormsAccess::Free);
    // A selection from protected text into the field is not inside the field.
    s.set_selection(Selection {
        anchor: Pos::new(0, 1),
        focus: Pos::new(0, 7),
    });
    assert_eq!(s.forms_access(), FormsAccess::Denied);
    // A disabled field cannot be filled in.
    caret(&mut s, 0, 7);
    modify(&mut s, |f| f.enabled = false);
    assert_eq!(s.forms_access(), FormsAccess::Denied);
    assert!(!s.set_form_value("x").unwrap());
    // Section exceptions are section properties (undoable).
    s.set_unprotected_sections(&[]).unwrap();
    caret(&mut s, 1, 2);
    assert_eq!(s.forms_access(), FormsAccess::Denied);
    assert!(s.undo());
    caret(&mut s, 1, 2);
    assert_eq!(s.forms_access(), FormsAccess::Free);
    // Not enforced: no restriction.
    s.set_protection(Some(DocProtection {
        edit: "forms".into(),
        enforced: false,
        ..DocProtection::default()
    }));
    caret(&mut s, 0, 1);
    assert_eq!(s.forms_access(), FormsAccess::Free);
}

#[test]
fn tab_order_skips_disabled_fields_and_wraps() {
    let mut s = Session::new(doc(&["a ", "b "]));
    caret(&mut s, 0, 2);
    s.insert_form_field("text").unwrap();
    s.insert_form_field("checkBox").unwrap();
    let p1 = end_of(&s, 1);
    caret(&mut s, 1, p1);
    s.insert_form_field("dropDown").unwrap();
    s.insert_form_field("text").unwrap();
    modify(&mut s, |f| f.enabled = false);
    caret(&mut s, 0, 0);
    let order: Vec<String> = (0..4)
        .map(|_| {
            s.next_form_field(false)
                .and_then(|f| f.field.name)
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(order, ["Text1", "Check1", "Dropdown1", "Text1"]);
    // The text field's whole result is selected.
    assert_eq!(
        s.selection(),
        Selection {
            anchor: Pos::new(0, 2),
            focus: Pos::new(0, 2 + EMPTY_TEXT_RESULT.len())
        }
    );
    let back = s.next_form_field(true).and_then(|f| f.field.name);
    assert_eq!(back.as_deref(), Some("Dropdown1"));
    let back = s.next_form_field(true).and_then(|f| f.field.name);
    assert_eq!(back.as_deref(), Some("Check1"));
}

#[test]
fn ref_fields_and_calculated_fields_update() {
    let mut s = Session::new(doc(&["", "", ""]));
    s.insert_form_field("text").unwrap();
    modify(&mut s, |f| f.name = Some("Qty".into()));
    s.set_form_value("21").unwrap();
    caret(&mut s, 1, 0);
    s.insert_form_field("text").unwrap();
    modify(&mut s, |f| {
        f.calc_on_exit = true;
        f.kind = FormFieldKind::Text(FormTextInput {
            input_type: TextInputType::Calculated,
            default: "=Qty*2".into(),
            ..FormTextInput::default()
        });
    });
    caret(&mut s, 2, 0);
    s.insert_field("REF Qty", "").unwrap();
    // The bookmark follows the renamed field.
    let names: Vec<String> = fields::bookmarks(s.doc())
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert_eq!(names, ["Qty", "Text1"]);
    s.update_fields(&mut |_| Vec::new()).unwrap();
    assert_eq!(plain(&s, 1), "42");
    assert_eq!(plain(&s, 2), "21");
    // A calculated field is not filled in by hand.
    caret(&mut s, 1, 1);
    assert!(!s.set_form_value("5").unwrap());
    // Leaving a calcOnExit field asks for a recalculation.
    caret(&mut s, 0, 1);
    s.set_form_value("5").unwrap();
    let span = s.form_at(Pos::new(1, 1)).unwrap();
    assert_eq!(s.form_exit(&span), Ok(true));
    s.update_fields(&mut |_| Vec::new()).unwrap();
    assert_eq!(plain(&s, 1), "10");
    assert_eq!(plain(&s, 2), "5");
}

#[test]
fn deleting_a_field_removes_its_markers() {
    let mut s = Session::new(doc(&["x"]));
    caret(&mut s, 0, 1);
    s.insert_form_field("checkBox").unwrap();
    // Typing after a check box goes past its end character.
    s.insert_text("y").unwrap();
    assert_eq!(plain(&s, 0), "x\u{2610}y");
    assert_eq!(s.form_fields().len(), 1);
    // Delete the object only: no end character is left behind.
    s.set_selection(Selection {
        anchor: Pos::new(0, 1),
        focus: Pos::new(0, 1 + bo_model::index::OBJECT.len_utf8()),
    });
    s.delete_backward().unwrap();
    assert_eq!(plain(&s, 0), "xy");
    assert_eq!(markers(&s), 0);

    // A text field inside a deleted range goes with its markers.
    let mut s = Session::new(doc(&["ab"]));
    caret(&mut s, 0, 1);
    s.insert_form_field("text").unwrap();
    s.set_selection(Selection {
        anchor: Pos::new(0, 0),
        focus: Pos::new(0, end_of(&s, 0)),
    });
    s.delete_backward().unwrap();
    assert_eq!(markers(&s), 0);
    assert_eq!(plain(&s, 0), "");
}

#[test]
fn dates_parse() {
    let d = |s: &str| parse_date(s).map(|d| (d.year, d.month, d.day));
    assert_eq!(d("5/3/2026"), Some((2026, 3, 5)));
    assert_eq!(d("29.02.24"), Some((2024, 2, 29)));
    assert_eq!(d("2026-12-01"), Some((2026, 12, 1)));
    assert_eq!(d("29/02/2025"), None);
    assert_eq!(d("hôm nay"), None);
}

#[test]
fn tab_after_typing_goes_to_the_next_field() {
    let mut s = Session::new(doc(&["Họ tên: "]));
    let end = end_of(&s, 0);
    caret(&mut s, 0, end);
    s.insert_form_field("text").unwrap();
    s.insert_text(" Đồng ý: ").unwrap();
    s.insert_form_field("checkBox").unwrap();
    s.split_paragraph().unwrap();
    s.insert_form_field("dropDown").unwrap();
    assert_eq!(
        plain(&s, 0),
        format!("Họ tên: {EMPTY_TEXT_RESULT} Đồng ý: \u{2610}")
    );
    caret(&mut s, 0, 0);
    let name = |f: Option<crate::forms::FormSpan>| f.and_then(|f| f.field.name);
    assert_eq!(name(s.next_form_field(false)).as_deref(), Some("Text1"));
    s.form_type("Lan").unwrap();
    // Splitting after the check box kept its end character in its paragraph.
    let kinds: Vec<&str> = para::flatten(para_at(s.doc(), &body_paragraphs(s.doc())[0]).unwrap())
        .iter()
        .map(|a| match &a.1 {
            RunContent::FormField(_) => "FF",
            RunContent::FormEnd => "FE",
            RunContent::Markup(_) => "M",
            _ => "T",
        })
        .collect();
    assert_eq!(
        kinds,
        ["T", "M", "FF", "T", "FE", "M", "T", "M", "FF", "FE", "M"]
    );
    assert_eq!(name(s.next_form_field(false)).as_deref(), Some("Check1"));
    assert_eq!(name(s.next_form_field(false)).as_deref(), Some("Dropdown1"));
}
