//! Paste options (CORE-PASTE-001): merge formatting, text only and link paste,
//! checked against hand-derived runs.

use super::*;
use bo_model::{Color, HalfPoints, NumRef, Run, Underline};

/// Destination formatting: Carlito 12 pt, dark blue.
fn dest_props() -> RunProps {
    RunProps {
        font: Some("Carlito".into()),
        size: Some(HalfPoints(24)),
        color: Color::from_hex("1F3864"),
        ..RunProps::default()
    }
}

fn doc_with(paras: Vec<Paragraph>) -> Document {
    Document {
        sections: vec![Section::new(
            paras.into_iter().map(Block::Paragraph).collect(),
        )],
        ..Default::default()
    }
}

fn dest_doc() -> Document {
    doc_with(vec![Paragraph::new(
        ParaProps::default(),
        vec![Run::text("đích", dest_props())],
    )])
}

/// `(text, props)` of every run of body paragraph `i`.
fn runs(s: &Session, i: usize) -> Vec<(String, RunProps)> {
    let paths = bo_model::index::body_paragraphs(s.doc());
    let p = bo_model::index::para_at(s.doc(), &paths[i]).expect("paragraph");
    p.runs
        .iter()
        .map(|r| (r.plain_text(), r.props.clone()))
        .collect()
}

fn para_props(s: &Session, i: usize) -> ParaProps {
    let paths = bo_model::index::body_paragraphs(s.doc());
    bo_model::index::para_at(s.doc(), &paths[i])
        .expect("paragraph")
        .props
        .clone()
}

fn caret_end(s: &mut Session) {
    let n = s.para_text(0).unwrap_or_default().len();
    s.set_selection(Selection::caret(Pos::new(0, n)));
}

const SOURCE: &str = "<p><b style=\"font-family:Arial;color:#00ff00\">A</b>\
    <i style=\"font-size:20pt\">B</i><u>C</u> D</p>";

#[test]
fn merge_keeps_emphasis_on_destination_formatting() {
    let mut s = Session::new(dest_doc());
    caret_end(&mut s);
    s.paste_with(PasteMode::Merge, None, Some(SOURCE), Some("ABC D"))
        .unwrap();
    let d = dest_props();
    // Font, size and colour come from the destination; b / i / u from the source.
    let expected = vec![
        ("đích".to_owned(), d.clone()),
        (
            "A".to_owned(),
            RunProps {
                bold: Some(true),
                ..d.clone()
            },
        ),
        (
            "B".to_owned(),
            RunProps {
                italic: Some(true),
                ..d.clone()
            },
        ),
        (
            "C".to_owned(),
            RunProps {
                underline: Some(Underline::Single),
                ..d.clone()
            },
        ),
        (" D".to_owned(), d),
    ];
    assert_eq!(runs(&s, 0), expected);
    // One undo step.
    assert!(s.undo());
    assert_eq!(s.doc(), &dest_doc());
}

#[test]
fn text_only_takes_destination_formatting() {
    let mut s = Session::new(dest_doc());
    caret_end(&mut s);
    s.paste_with(PasteMode::Text, None, Some(SOURCE), None)
        .unwrap();
    // A single run: the pasted text joins the destination run (same properties).
    assert_eq!(runs(&s, 0), vec![("đíchABC D".to_owned(), dest_props())]);
    assert!(s.undo());
    assert_eq!(s.doc(), &dest_doc());
}

#[test]
fn text_only_splits_lines_into_paragraphs() {
    let mut s = Session::new(dest_doc());
    caret_end(&mut s);
    s.paste_with(PasteMode::Text, None, None, Some("một\r\nhai"))
        .unwrap();
    assert_eq!(runs(&s, 0), vec![("đíchmột".to_owned(), dest_props())]);
    assert_eq!(runs(&s, 1), vec![("hai".to_owned(), dest_props())]);
    assert_eq!(s.selection(), Selection::caret(Pos::new(1, "hai".len())));
}

#[test]
fn merge_keeps_lists_of_an_internal_copy() {
    // Source: a destination paragraph and a list item; copy both.
    let mut n = bo_model::Numbering::default();
    let num = n.add_simple_list(true);
    let item = Paragraph::new(
        ParaProps {
            num: Some(NumRef {
                num_id: num,
                ilvl: 0,
            }),
            ..ParaProps::default()
        },
        vec![Run::text(
            "mục",
            RunProps {
                bold: Some(true),
                font: Some("Arial".into()),
                ..RunProps::default()
            },
        )],
    );
    let plain = Paragraph::new(
        ParaProps::default(),
        vec![Run::text("đầu", RunProps::default())],
    );
    let mut src = doc_with(vec![plain, item]);
    src.numbering = n.clone();
    let clip = {
        let mut s = Session::new(src);
        s.set_selection(Selection {
            anchor: Pos::new(0, 0),
            focus: Pos::new(1, "mục".len()),
        });
        s.copy()
    };
    let mut d = dest_doc();
    d.numbering = n;
    let mut s = Session::new(d);
    caret_end(&mut s);
    s.paste_with(
        PasteMode::Merge,
        Some(&clip.internal),
        None,
        Some(&clip.text),
    )
    .unwrap();
    // The list item keeps its numbering; its run is bold in the destination font.
    assert_eq!(
        para_props(&s, 1).num,
        Some(NumRef {
            num_id: num,
            ilvl: 0
        })
    );
    assert_eq!(
        runs(&s, 1),
        vec![(
            "mục".to_owned(),
            RunProps {
                bold: Some(true),
                ..dest_props()
            }
        )]
    );
    // Text only drops the list (the destination paragraph has none).
    assert!(s.undo());
    caret_end(&mut s);
    s.paste_with(PasteMode::Text, Some(&clip.internal), None, None)
        .unwrap();
    assert_eq!(para_props(&s, 1).num, None);
    assert_eq!(runs(&s, 1), vec![("mục".to_owned(), dest_props())]);
}

#[test]
fn link_paste_makes_a_hyperlink_run() {
    let mut s = Session::new(dest_doc());
    caret_end(&mut s);
    s.paste_with(
        PasteMode::Link,
        None,
        None,
        Some("  https://bluestar.vn/a  "),
    )
    .unwrap();
    let link = RunProps {
        link: Some("https://bluestar.vn/a".into()),
        color: Color::from_hex("0563C1"),
        underline: Some(Underline::Single),
        ..dest_props()
    };
    assert_eq!(
        runs(&s, 0),
        vec![
            ("đích".to_owned(), dest_props()),
            ("https://bluestar.vn/a".to_owned(), link),
        ]
    );
    // An e-mail address links with mailto:.
    s.paste_with(PasteMode::Link, None, None, Some("ai@bluestar.vn"))
        .unwrap();
    assert_eq!(
        runs(&s, 0)[2].1.link.as_deref(),
        Some("mailto:ai@bluestar.vn")
    );
    // Not a link: refused, nothing changes.
    let before = s.doc().clone();
    assert!(s
        .paste_with(PasteMode::Link, None, None, Some("hai từ"))
        .is_err());
    assert_eq!(s.doc(), &before);
    assert_eq!(
        crate::paste::clipboard_link("www.bluestar.vn").as_deref(),
        Some("http://www.bluestar.vn")
    );
}
