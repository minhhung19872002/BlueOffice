//! Track changes for tables, pictures and page setup (ECMA-376 Part 1, §17.13.5):
//! row insertion/deletion (§17.13.5.19, §17.13.5.12 in trPr), cell and table property
//! changes (§17.13.5.36 tcPrChange, §17.13.5.34 tblPrChange), pictures as run
//! insertions/deletions and page setup changes (§17.13.5.32 sectPrChange).

use super::*;
use crate::section::{PageSetupSpec, SetupScope};
use crate::table::{BorderPreset, TableEdit};
use bo_model::{Cell, Image, Run, Table};
use proptest::prelude::*;

/// First difference between two documents (debug output is too long to compare).
fn same(a: &Document, b: &Document) -> std::result::Result<(), String> {
    let (x, y) = (format!("{a:?}"), format!("{b:?}"));
    if x == y {
        return Ok(());
    }
    let i = x.bytes().zip(y.bytes()).take_while(|(p, q)| p == q).count();
    let lo = x[..i].char_indices().rev().nth(200).map_or(0, |c| c.0);
    let end = |s: &str| s[i..].char_indices().nth(160).map_or(s.len(), |c| i + c.0);
    Err(format!(
        "…{}
  vs  …{}",
        &x[lo..end(&x)],
        &y[lo..end(&y)]
    ))
}

fn para(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run::text(text, RunProps::default())],
    ))
}

/// "Mở đầu", a 3×2 table with text in every cell, "Kết" with a picture.
fn doc() -> Document {
    let mut t = Table::simple(3, 2, Twips(9000));
    for (ri, row) in t.rows.iter_mut().enumerate() {
        for (ci, c) in row.cells.iter_mut().enumerate() {
            *c = Cell {
                props: c.props.clone(),
                blocks: vec![para(&format!("Ô {ri}{ci}"))],
            };
        }
    }
    let img = Image {
        media: "/word/media/image1.png".into(),
        width: bo_model::Emu(914_400),
        height: bo_model::Emu(914_400),
        anchor: None,
        crop: None,
        rotation: 0,
        description: String::new(),
        src: None,
    };
    let last = Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![
                RunContent::Text("Kết ".into()),
                RunContent::Image(Box::new(img)),
            ],
        }],
    ));
    let mut d = Document {
        sections: vec![Section::new(vec![para("Mở đầu"), Block::Table(t), last])],
        ..Default::default()
    };
    d.media
        .insert("/word/media/image1.png".into(), Arc::new(vec![1, 2, 3]));
    d
}

#[derive(Debug, Clone)]
enum Act {
    /// Caret in body paragraph `n` (0 = before the table, 1..=6 cells, last = picture).
    Caret(usize),
    InsertRow(bool),
    DeleteRow,
    DeleteTable,
    Shading(u8),
    Borders(u8),
    Align(u8),
    PageSetup(i32),
    TitlePage(bool),
    Picture,
    /// Delete the whole paragraph at the caret (deletes the picture in the last one).
    DeleteParagraphText,
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        (0usize..12).prop_map(Act::Caret),
        any::<bool>().prop_map(Act::InsertRow),
        Just(Act::DeleteRow),
        Just(Act::DeleteTable),
        (0u8..3).prop_map(Act::Shading),
        (0u8..4).prop_map(Act::Borders),
        (0u8..3).prop_map(Act::Align),
        (0i32..3).prop_map(Act::PageSetup),
        any::<bool>().prop_map(Act::TitlePage),
        Just(Act::Picture),
        Just(Act::DeleteParagraphText),
    ]
}

fn run(s: &mut Session, a: &Act) {
    let _ = match a {
        Act::Caret(n) => {
            let n = (*n).min(s.paragraph_count().saturating_sub(1));
            s.set_selection(Selection::caret(Pos::new(n, 0)));
            Ok(s.selection())
        }
        Act::InsertRow(below) => s.edit_table(&TableEdit::InsertRow { below: *below }),
        Act::DeleteRow => s.edit_table(&TableEdit::DeleteRow),
        Act::DeleteTable => s.edit_table(&TableEdit::DeleteTable),
        Act::Shading(k) => s.edit_table(&TableEdit::Shading {
            hex: ["FFEE00", "00AAFF", ""]
                .get(usize::from(*k))
                .filter(|h| !h.is_empty())
                .map(|h| (*h).to_owned()),
        }),
        Act::Borders(k) => s.edit_table(&TableEdit::Borders {
            preset: [
                BorderPreset::All,
                BorderPreset::None,
                BorderPreset::Outside,
                BorderPreset::Inside,
            ][usize::from(*k)],
        }),
        Act::Align(k) => s.edit_table(&TableEdit::Properties {
            width_pct: Some(80),
            width: None,
            align: Some([Alignment::Left, Alignment::Center, Alignment::Right][usize::from(*k)]),
        }),
        Act::PageSetup(k) => {
            let w = [11906, 16838, 12240][*k as usize];
            let spec = PageSetupSpec {
                width: w,
                height: 16838,
                margin_top: 1134 + k * 100,
                margin_bottom: 1134,
                margin_left: 1701,
                margin_right: 850,
                header: 720,
                footer: 720,
                columns: 1 + u16::try_from(*k).unwrap_or(0) % 2,
                column_space: 720,
            };
            s.set_page_setup(spec, SetupScope::Section)
                .map(|()| s.selection())
        }
        Act::TitlePage(on) => s
            .set_header_options(0, Some(*on), None)
            .map(|()| s.selection()),
        Act::Picture => s.insert_image(vec![9, 9, 9], "png", 457_200, 457_200),
        Act::DeleteParagraphText => {
            let p = s.selection().focus.para;
            let len = s.para_text(p).map_or(0, |t| t.len());
            s.set_selection(Selection {
                anchor: Pos::new(p, 0),
                focus: Pos::new(p, len),
            });
            s.delete_backward()
        }
    };
}

#[test]
fn tracked_row_insert_delete_and_reject() {
    let original = doc();
    let mut s = Session::new(original.clone());
    s.set_track_changes(true);
    s.set_author("Người duyệt");
    s.set_selection(Selection::caret(Pos::new(1, 0)));
    s.edit_table(&TableEdit::InsertRow { below: false })
        .unwrap();
    let Block::Table(t) = &s.doc().sections[0].blocks[1] else {
        panic!("table")
    };
    assert_eq!(t.rows.len(), 4);
    assert!(t.rows[0].props.ins.is_some());
    s.set_selection(Selection::caret(Pos::new(5, 0)));
    s.edit_table(&TableEdit::DeleteRow).unwrap();
    s.edit_table(&TableEdit::Shading {
        hex: Some("FFEE00".into()),
    })
    .unwrap();
    let kinds: Vec<_> = s.revisions().iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&RevKind::RowInsert), "{kinds:?}");
    assert!(kinds.contains(&RevKind::RowDelete), "{kinds:?}");
    assert!(kinds.contains(&RevKind::CellFormat), "{kinds:?}");
    // The deleted row is still there until accepted.
    let Block::Table(t) = &s.doc().sections[0].blocks[1] else {
        panic!("table")
    };
    assert_eq!(t.rows.len(), 4);
    let mut accepted = Session::new(s.doc().clone());
    accepted.resolve_revisions(None, true).unwrap();
    let Block::Table(t) = &accepted.doc().sections[0].blocks[1] else {
        panic!("table")
    };
    assert_eq!(t.rows.len(), 3);
    assert_eq!(accepted.revisions().len(), 0);
    s.resolve_revisions(None, false).unwrap();
    same(s.doc(), &original).unwrap();
}

#[test]
fn tracked_picture_and_page_setup() {
    let original = doc();
    let mut s = Session::new(original.clone());
    s.set_track_changes(true);
    s.set_selection(Selection::caret(Pos::new(0, 0)));
    s.insert_image(vec![7], "png", 457_200, 457_200).unwrap();
    assert_eq!(s.doc().media.len(), 2);
    run(&mut s, &Act::PageSetup(1));
    assert!(s.doc().sections[0].change.is_some());
    let kinds: Vec<_> = s.revisions().iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&RevKind::Insert), "{kinds:?}");
    assert!(kinds.contains(&RevKind::SectionFormat), "{kinds:?}");
    s.resolve_revisions(None, false).unwrap();
    same(s.doc(), &original).unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    /// Random tracked table, picture and page setup edits: rejecting all of them
    /// restores the original document exactly, accepting all leaves no revision, and
    /// undoing everything restores the original.
    #[test]
    fn tracked_structure_reject_all_restores_original(acts in proptest::collection::vec(act(), 1..20)) {
        let original = doc();
        let mut s = Session::new(original.clone());
        s.set_track_changes(true);
        s.set_author("Người duyệt");
        for a in &acts {
            run(&mut s, a);
            let sel = s.selection();
            prop_assert_eq!(s.clamp(sel.anchor), sel.anchor);
            prop_assert_eq!(s.clamp(sel.focus), sel.focus);
        }
        let edited = s.doc().clone();
        let mut accepted = Session::new(edited.clone());
        accepted.resolve_revisions(None, true).unwrap();
        prop_assert!(accepted.revisions().is_empty());
        s.resolve_revisions(None, false).unwrap();
        prop_assert!(s.revisions().is_empty());
        if let Err(e) = same(s.doc(), &original) { prop_assert!(false, "{}", e); }
        while s.undo() {}
        prop_assert_eq!(s.doc(), &original);
    }
}
