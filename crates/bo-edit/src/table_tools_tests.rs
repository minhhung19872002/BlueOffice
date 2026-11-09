//! Session tests of the table tools: every command is one undo step and undo/redo
//! restores the document exactly.

use super::*;
use crate::table_tools::{AutoFit, SortKey, SortKind};
use bo_model::{HeightRule, Run, TableWidth, TextDirection};

fn para(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run::text(text, RunProps::default())],
    ))
}

fn doc(paras: &[&str]) -> Document {
    Document {
        sections: vec![Section::new(paras.iter().map(|t| para(t)).collect())],
        ..Default::default()
    }
}

fn table(s: &Session) -> Table {
    s.doc().sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("a table")
}

fn column(s: &Session, c: usize) -> Vec<String> {
    table(s)
        .rows
        .iter()
        .map(|r| crate::table_tools::cell_text(&r.cells[c]))
        .collect()
}

fn select(s: &mut Session, a: (usize, usize), b: (usize, usize)) {
    s.set_selection(Selection {
        anchor: Pos::new(a.0, a.1),
        focus: Pos::new(b.0, b.1),
    });
}

/// Undo everything (back to `original`), redo everything (back to `edited`).
fn round_trip(s: &mut Session, original: &Document) {
    let edited = s.doc().clone();
    while s.undo() {}
    assert_eq!(s.doc(), original);
    while s.redo() {}
    assert_eq!(s.doc(), &edited);
}

/// Text → table (tabs) → sort → distribute → AutoFit → direction → wrap → text.
#[test]
fn table_tools_are_undoable() {
    let mut s = Session::new(doc(&[
        "Tên\tSố",
        "Đức\t10",
        "An\t9",
        "Ánh\t100",
        "sau bảng",
    ]));
    let original = s.doc().clone();
    // The selection ends at the start of the fifth paragraph: four rows.
    select(&mut s, (0, 0), (4, 0));
    s.edit_table(&TableEdit::FromText {
        separator: "\t".into(),
        cols: None,
    })
    .unwrap();
    let t = table(&s);
    assert_eq!((t.rows.len(), t.grid.len()), (4, 2));
    assert_eq!(column(&s, 0), ["Tên", "Đức", "An", "Ánh"]);
    // The paragraph after the selection stays after the table.
    assert!(matches!(s.doc().sections[0].blocks[1], Block::Paragraph(_)));

    // Caret in the table: sort the rows below the header by the second column, as numbers.
    s.set_selection(Selection::caret(Pos::new(0, 0)));
    s.edit_table(&TableEdit::Sort {
        keys: vec![SortKey {
            column: 1,
            kind: SortKind::Number,
            descending: false,
        }],
        header: true,
    })
    .unwrap();
    assert_eq!(column(&s, 1), ["Số", "9", "10", "100"]);
    s.edit_table(&TableEdit::Sort {
        keys: vec![SortKey {
            column: 0,
            kind: SortKind::Text,
            descending: true,
        }],
        header: true,
    })
    .unwrap();
    assert_eq!(column(&s, 0), ["Tên", "Đức", "Ánh", "An"]);

    s.edit_table(&TableEdit::DistributeRows {
        heights: vec![300, 300, 600, 400],
    })
    .unwrap();
    assert!(table(&s)
        .rows
        .iter()
        .all(|r| r.props.height == Some((Twips(400), HeightRule::AtLeast))));
    s.edit_table(&TableEdit::AutoFit {
        mode: AutoFit::Contents,
        widths: vec![1500, 900],
    })
    .unwrap();
    let t = table(&s);
    assert_eq!(t.grid, vec![Twips(1500), Twips(900)]);
    assert_eq!(t.props.width, Some(TableWidth::Auto));
    s.edit_table(&TableEdit::DistributeColumns).unwrap();
    assert_eq!(table(&s).grid, vec![Twips(1200), Twips(1200)]);
    s.edit_table(&TableEdit::AutoFit {
        mode: AutoFit::Window,
        widths: Vec::new(),
    })
    .unwrap();
    // The default section is 11906 twips wide with 1134 + 850 twips margins... whatever
    // its text width is, the columns fill it exactly.
    let page = &s.doc().sections[0].page;
    let text_w = page.width.0 - page.margin_left.0 - page.margin_right.0;
    assert_eq!(table(&s).grid.iter().map(|g| g.0).sum::<i32>(), text_w);
    s.edit_table(&TableEdit::TextDirection {
        dir: Some(TextDirection::BtLr),
    })
    .unwrap();
    assert_eq!(
        s.format_state().table.unwrap().direction,
        Some(TextDirection::BtLr)
    );
    s.edit_table(&TableEdit::Wrap { around: true }).unwrap();
    assert!(s.format_state().table.unwrap().wrap);
    s.edit_table(&TableEdit::ToText {
        separator: ";".into(),
    })
    .unwrap();
    assert!(s.format_state().table.is_none());
    let texts: Vec<String> = (0..s.paragraph_count())
        .map(|i| s.para_text(i).unwrap_or_default())
        .collect();
    assert_eq!(texts, ["Tên;Số", "Đức;10", "Ánh;100", "An;9", "sau bảng"]);
    round_trip(&mut s, &original);
}

#[test]
fn paragraphs_become_cells() {
    let mut s = Session::new(doc(&["một", "hai", "ba"]));
    let original = s.doc().clone();
    select(&mut s, (0, 0), (2, 2));
    s.edit_table(&TableEdit::FromText {
        separator: "\n".into(),
        cols: Some(2),
    })
    .unwrap();
    let t = table(&s);
    assert_eq!((t.rows.len(), t.grid.len()), (2, 2));
    assert_eq!(column(&s, 1), ["hai", ""]);
    // The table was the last block: an empty paragraph follows it.
    assert_eq!(s.doc().sections[0].blocks.len(), 2);
    round_trip(&mut s, &original);
}

#[test]
fn sort_refuses_merged_rows_and_tracking() {
    let mut s = Session::new(doc(&["a", "b"]));
    select(&mut s, (0, 0), (1, 1));
    s.edit_table(&TableEdit::FromText {
        separator: ",".into(),
        cols: None,
    })
    .unwrap();
    select(&mut s, (0, 0), (1, 0));
    s.edit_table(&TableEdit::MergeCells).unwrap();
    let sort = TableEdit::Sort {
        keys: vec![SortKey {
            column: 0,
            kind: SortKind::Text,
            descending: false,
        }],
        header: false,
    };
    assert!(matches!(
        s.edit_table(&sort),
        Err(EditError::Unsupported(_))
    ));
    s.set_track_changes(true);
    assert!(matches!(
        s.edit_table(&TableEdit::ToText {
            separator: "\t".into()
        }),
        Err(EditError::Unsupported(_))
    ));
}
