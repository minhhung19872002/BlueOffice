//! Track changes for tables, pictures and page setup (ECMA-376 Part 1, §17.13.5):
//! row insertion/deletion (§17.13.5.19, §17.13.5.12 in trPr), cell insertion/deletion
//! (§17.13.5.4 cellIns, §17.13.5.3 cellDel: columns and cells), merges and splits
//! (§17.13.5.6 cellMerge, tcPrChange spans), row properties (§17.13.5.37 trPrChange),
//! cell and table property changes (§17.13.5.36 tcPrChange, §17.13.5.34 tblPrChange),
//! grid changes (§17.13.5.33 tblGridChange), pictures as run insertions/deletions,
//! floating picture moves (§17.13.5.22 moveFrom / §17.13.5.25 moveTo), page setup
//! changes (§17.13.5.32 sectPrChange) and changes inside headers and notes.

use super::*;
use crate::revisions::StoryRef;
use crate::section::{PageSetupSpec, SetupScope};
use crate::table::{BorderPreset, TableEdit};
use bo_model::{Anchor, Cell, Image, Run, Table, VMerge};
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

/// "Mở đầu" with a footnote, a 3×2 table with text in every cell, "Kết" with an
/// inline and a floating picture; a default header and the footnote's text.
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
        shape: None,
        format: None,
        src: None,
        ole: None,
    };
    let floating = Image {
        anchor: Some(Anchor {
            h_relative: "column".into(),
            v_relative: "paragraph".into(),
            x: bo_model::Emu(127_000),
            y: bo_model::Emu(0),
            ..Anchor::default()
        }),
        ..img.clone()
    };
    let last = Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![
                RunContent::Text("Kết ".into()),
                RunContent::Image(Box::new(img)),
                RunContent::Image(Box::new(floating)),
            ],
        }],
    ));
    let first = Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![
                RunContent::Text("Mở đầu".into()),
                RunContent::FootnoteRef(1),
            ],
        }],
    ));
    let mut sec = Section::new(vec![first, Block::Table(t), last]);
    sec.headers.default = Some(vec![para("Đầu trang")]);
    let mut d = Document {
        sections: vec![sec],
        ..Default::default()
    };
    d.footnotes.insert(1, vec![para("Chú thích một")]);
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
    /// Select from the start of body paragraph `a` to the start of `b`.
    Select(usize, usize),
    InsertColumn(bool),
    DeleteColumn,
    InsertCell(bool),
    DeleteCell,
    Merge,
    Split,
    HeaderRow(bool),
    /// Move the floating picture of the caret's paragraph.
    MovePicture(i32),
    /// Edit the header (or the footnote) in its own tracked session: type at the end
    /// (`true`) or delete the last character.
    Story {
        note: bool,
        typing: bool,
    },
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
        (0usize..12, 0usize..12).prop_map(|(a, b)| Act::Select(a, b)),
        any::<bool>().prop_map(Act::InsertColumn),
        Just(Act::DeleteColumn),
        any::<bool>().prop_map(Act::InsertCell),
        Just(Act::DeleteCell),
        Just(Act::Merge),
        Just(Act::Split),
        any::<bool>().prop_map(Act::HeaderRow),
        (0i32..4).prop_map(Act::MovePicture),
        (any::<bool>(), any::<bool>()).prop_map(|(note, typing)| Act::Story { note, typing }),
    ]
}

/// Edit a story (the default header of section 0, or footnote 1) the way the engine
/// does: a tracked session over its blocks, then one change in the main session.
fn edit_story(s: &mut Session, note: bool, typing: bool) {
    let blocks = if note {
        s.doc().footnotes.get(&1).cloned()
    } else {
        s.doc().sections[0].headers.default.clone()
    };
    let Some(blocks) = blocks else { return };
    let mut sub = Session::new(Document {
        sections: vec![Section::new(blocks)],
        ..Document::default()
    });
    sub.set_track_changes(true);
    sub.set_author(s.author());
    sub.set_revision_floor(revisions::max_id(s.doc()));
    let last = sub.paragraph_count().saturating_sub(1);
    let end = sub.para_text(last).map_or(0, |t| t.len());
    sub.set_selection(Selection::caret(Pos::new(last, end)));
    let _ = if typing {
        sub.insert_text(" mới")
    } else {
        sub.delete_backward()
    };
    let new = sub.doc().sections[0].blocks.clone();
    let _ = if note {
        s.set_note_blocks(false, 1, new)
    } else {
        s.set_header_footer(0, true, crate::section::HfVariant::Default, new)
    };
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
        Act::Select(a, b) => {
            let n = s.paragraph_count().saturating_sub(1);
            s.set_selection(Selection {
                anchor: Pos::new((*a).min(n), 0),
                focus: Pos::new((*b).min(n), 0),
            });
            Ok(s.selection())
        }
        Act::InsertColumn(right) => s.edit_table(&TableEdit::InsertColumn { right: *right }),
        Act::DeleteColumn => s.edit_table(&TableEdit::DeleteColumn),
        Act::InsertCell(right) => s.edit_table(&TableEdit::InsertCell { right: *right }),
        Act::DeleteCell => s.edit_table(&TableEdit::DeleteCell),
        Act::Merge => s.edit_table(&TableEdit::MergeCells),
        Act::Split => s.edit_table(&TableEdit::SplitCell),
        Act::HeaderRow(on) => s.edit_table(&TableEdit::HeaderRow { on: *on }),
        Act::MovePicture(k) => s.set_picture_position(10.0 + *k as f32 * 20.0, *k as f32 * 5.0),
        Act::Story { note, typing } => {
            edit_story(s, *note, *typing);
            Ok(s.selection())
        }
    };
}

fn table0(s: &Session) -> Table {
    match &s.doc().sections[0].blocks[1] {
        Block::Table(t) => t.clone(),
        _ => panic!("table"),
    }
}

/// Body paragraph id of the first paragraph of cell (`r`, `c`) of the table.
fn cell_para(s: &Session, r: usize, c: usize) -> usize {
    crate::id_of(s.doc(), &[0, 1, r, c, 0]).expect("cell paragraph")
}

#[test]
fn tracked_columns_and_cells() {
    let original = doc();
    let mut s = Session::new(original.clone());
    s.set_track_changes(true);
    s.set_author("Người duyệt");
    s.set_selection(Selection::caret(Pos::new(cell_para(&s, 0, 0), 0)));
    s.edit_table(&TableEdit::InsertColumn { right: true })
        .unwrap();
    let t = table0(&s);
    assert_eq!(t.grid.len(), 3);
    assert!(t.grid_change.is_some());
    assert!(t.rows.iter().all(|r| r.cells[1].props.ins.is_some()));
    let kinds: Vec<_> = s.revisions().iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&RevKind::CellInsert), "{kinds:?}");
    // The widths changed by the insertion belong to the same revision.
    assert!(!kinds.contains(&RevKind::CellFormat), "{kinds:?}");
    assert!(!kinds.contains(&RevKind::TableGrid), "{kinds:?}");
    // Delete the last column: cells marked deleted, grid unchanged until accepted.
    s.set_selection(Selection::caret(Pos::new(cell_para(&s, 1, 2), 0)));
    s.edit_table(&TableEdit::DeleteColumn).unwrap();
    let t = table0(&s);
    assert!(t.rows.iter().all(|r| r.cells[2].props.del.is_some()));
    assert_eq!(t.grid.len(), 3);
    let mut accepted = Session::new(s.doc().clone());
    accepted.resolve_revisions(None, true).unwrap();
    assert_eq!(accepted.revisions().len(), 0);
    let t = table0(&accepted);
    assert_eq!(t.grid.len(), 2, "the deleted column is gone");
    assert!(t.rows.iter().all(|r| r.cells.len() == 2), "{:?}", t.rows);
    s.resolve_revisions(None, false).unwrap();
    same(s.doc(), &original).unwrap();

    // Delete one cell (shift left): accepted, the row is one cell short.
    s.set_selection(Selection::caret(Pos::new(cell_para(&s, 2, 0), 0)));
    s.edit_table(&TableEdit::DeleteCell).unwrap();
    assert!(table0(&s).rows[2].cells[0].props.del.is_some());
    let mut accepted = Session::new(s.doc().clone());
    accepted.resolve_revisions(None, true).unwrap();
    let t = table0(&accepted);
    assert_eq!((t.grid.len(), t.rows[2].cells.len()), (2, 1));
    s.resolve_revisions(None, false).unwrap();
    same(s.doc(), &original).unwrap();

    // Insert one cell (shift right): the grid grows, rejecting restores it.
    s.set_selection(Selection::caret(Pos::new(cell_para(&s, 0, 0), 0)));
    s.edit_table(&TableEdit::InsertCell { right: false })
        .unwrap();
    let t = table0(&s);
    assert_eq!(t.rows[0].cells.len(), 3);
    assert!(t.rows[0].cells[0].props.ins.is_some());
    assert_eq!(t.grid.len(), 3);
    s.resolve_revisions(None, false).unwrap();
    same(s.doc(), &original).unwrap();
}

#[test]
fn tracked_merge_split_and_header_row() {
    let original = doc();
    let mut s = Session::new(original.clone());
    s.set_track_changes(true);
    // Merge the 2×2 block of the first two rows.
    s.set_selection(Selection {
        anchor: Pos::new(cell_para(&s, 0, 0), 0),
        focus: Pos::new(cell_para(&s, 1, 1), 0),
    });
    s.edit_table(&TableEdit::MergeCells).unwrap();
    let t = table0(&s);
    assert_eq!(t.rows[0].cells[0].props.grid_span, 2);
    assert!(t.rows[0].cells[1].props.del.is_some());
    assert_eq!(t.rows[1].cells[0].props.v_merge, Some(VMerge::Continue));
    assert!(t.rows[1].cells[0].props.merge.is_some());
    // Markup keeps every cell inside the grid.
    let g = t.geometry(bo_model::TableView::Markup);
    assert!(g.rows[0].iter().all(|c| c.start + c.span <= 2), "{g:?}");
    let kinds: Vec<_> = s.revisions().iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&RevKind::CellMerge), "{kinds:?}");
    assert!(kinds.contains(&RevKind::CellDelete), "{kinds:?}");
    let mut accepted = Session::new(s.doc().clone());
    accepted.resolve_revisions(None, true).unwrap();
    let t = table0(&accepted);
    assert_eq!(t.rows[0].cells.len(), 1);
    assert_eq!(t.rows[0].cells[0].props.grid_span, 2);
    let text = accepted.doc().plain_text();
    assert!(text.contains("Ô 00") && text.contains("Ô 11"), "{text}");
    assert_eq!(text.matches("Ô 11").count(), 1, "{text}");
    s.resolve_revisions(None, false).unwrap();
    same(s.doc(), &original).unwrap();

    // Split a cell into two columns, then mark the first row as a header row.
    let mut s = Session::new(original.clone());
    s.set_track_changes(true);
    s.set_selection(Selection::caret(Pos::new(cell_para(&s, 0, 0), 0)));
    s.edit_table(&TableEdit::SplitCell).unwrap();
    s.edit_table(&TableEdit::HeaderRow { on: true }).unwrap();
    let t = table0(&s);
    assert!(t.rows[0].cells[1].props.ins.is_some());
    assert!(t.rows[0].props.header);
    assert!(t.rows[0].props.change.is_some());
    let kinds: Vec<_> = s.revisions().iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&RevKind::RowFormat), "{kinds:?}");
    s.resolve_revisions(None, false).unwrap();
    same(s.doc(), &original).unwrap();
}

#[test]
fn tracked_picture_move() {
    let original = doc();
    let mut s = Session::new(original.clone());
    s.set_track_changes(true);
    let last = s.paragraph_count() - 1;
    s.set_selection(Selection::caret(Pos::new(last, 0)));
    assert!(s.format_state().picture.is_some());
    s.set_picture_position(72.0, 36.0).unwrap();
    let kinds: Vec<_> = s.revisions().iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&RevKind::MoveFrom), "{kinds:?}");
    assert!(kinds.contains(&RevKind::MoveTo), "{kinds:?}");
    // Moving the moved picture again changes the moveTo copy in place.
    s.set_picture_position(80.0, 36.0).unwrap();
    assert_eq!(s.revisions().len(), 2);
    let pic = s.format_state().picture.unwrap();
    assert!((pic.x - 80.0).abs() < 0.01, "{pic:?}");
    let mut accepted = Session::new(s.doc().clone());
    accepted.resolve_revisions(None, true).unwrap();
    accepted.set_selection(Selection::caret(Pos::new(last, 0)));
    assert!((accepted.format_state().picture.unwrap().x - 80.0).abs() < 0.01);
    s.resolve_revisions(None, false).unwrap();
    same(s.doc(), &original).unwrap();
}

#[test]
fn tracked_header_and_note_changes() {
    let original = doc();
    let mut s = Session::new(original.clone());
    s.set_track_changes(true);
    s.set_selection(Selection::caret(Pos::new(0, 0)));
    s.insert_text("A").unwrap();
    edit_story(&mut s, false, true);
    edit_story(&mut s, true, false);
    let revs = s.revisions();
    let stories: Vec<_> = revs.iter().filter_map(|r| r.story).collect();
    assert!(stories.contains(&StoryRef::Header {
        section: 0,
        variant: crate::section::HfVariant::Default
    }));
    assert!(stories.contains(&StoryRef::Footnote { id: 1 }));
    // Ids are unique across the main text and the stories.
    let mut ids: Vec<i32> = revs.iter().map(|r| r.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), revs.len(), "{revs:?}");
    // Accept the header change only.
    let hid = revs
        .iter()
        .find(|r| matches!(r.story, Some(StoryRef::Header { .. })))
        .map(|r| r.id)
        .unwrap();
    s.resolve_revisions(Some(&[hid]), true).unwrap();
    assert_eq!(s.revisions().len(), revs.len() - 1);
    let h = &s.doc().sections[0].headers.default.as_ref().unwrap()[0];
    assert_eq!(h.as_paragraph().unwrap().plain_text(), "Đầu trang mới");
    s.resolve_revisions(None, false).unwrap();
    let mut expected = original.clone();
    expected.sections[0].headers.default = Some(vec![Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run::text("Đầu trang mới", RunProps::default())],
    ))]);
    same(s.doc(), &expected).unwrap();
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
    #![proptest_config(ProptestConfig::with_cases(128))]
    /// Random tracked table (rows, columns, cells, merges, splits, header rows),
    /// picture (insert, delete, move), page setup, header and footnote edits: rejecting all of them
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
        prop_assert_eq!(accepted.revisions().len(), 0);
        s.resolve_revisions(None, false).unwrap();
        prop_assert_eq!(s.revisions().len(), 0);
        if let Err(e) = same(s.doc(), &original) { prop_assert!(false, "{}", e); }
        while s.undo() {}
        prop_assert_eq!(s.doc(), &original);
    }
}

/// Shrunk case of `tracked_structure_reject_all_restores_original`: merge two
/// cells, insert a row above, shade, then reject everything.
#[test]
fn tracked_merge_insert_row_shade_reject_restores_original() {
    let original = doc();
    let mut s = Session::new(original.clone());
    s.set_track_changes(true);
    s.set_author("Người duyệt");
    for a in [
        Act::Select(2, 1),
        Act::Merge,
        Act::InsertRow(false),
        Act::Shading(0),
    ] {
        run(&mut s, &a);
    }
    s.resolve_revisions(None, false).unwrap();
    same(s.doc(), &original).unwrap();
}

// source: ECMA-376 Part 1, §17.13.5.14 del / §17.13.5.18 ins on an Office Math zone (§22.1.2.77);
// ADR-0015 addendum (an equation edited in place is one revision of the whole equation).
#[test]
fn tracked_equation_replacement_is_one_revision() {
    const M: &str = r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math""#;
    let old = format!("<m:oMath {M}><m:r><m:t>a</m:t></m:r></m:oMath>");
    let original = Document {
        sections: vec![Section::new(vec![para("x ")])],
        ..Document::default()
    };
    let mut s = Session::new(original);
    s.set_selection(Selection::caret(Pos::new(0, 2)));
    s.insert_equation(&old).unwrap();
    let base = s.doc().clone();
    s.set_track_changes(true);
    s.set_author("An");
    let at = Pos::new(0, 2);
    // Each in-place edit: the first marks the original deleted and inserts the edit after it,
    // the next ones change that (own) insertion.
    let mut cur = at;
    for t in ["ab", "abc", "abcd"] {
        cur = s
            .replace_equation(
                cur,
                &format!("<m:oMath {M}><m:r><m:t>{t}</m:t></m:r></m:oMath>"),
                true,
            )
            .unwrap();
    }
    assert_eq!(cur, Pos::new(0, 2 + 3));
    assert_eq!(s.equation_at(at).as_deref(), Some(old.as_str()));
    assert!(s.equation_deleted(at));
    assert!(!s.equation_deleted(cur));
    assert!(s.equation_at(cur).unwrap().contains("abcd"));
    let revs = s.revisions();
    assert_eq!(revs.len(), 1, "{revs:?}");
    assert_eq!(revs[0].kind, RevKind::Equation);
    assert_eq!((revs[0].start, revs[0].end), (at, Pos::new(0, 8)));
    // A deleted equation is not edited.
    assert!(s.replace_equation(at, &old, false).is_err());
    // Resolving the one listed id resolves both halves.
    let id = revs[0].id;
    let mut accepted = Session::new(s.doc().clone());
    assert_eq!(accepted.resolve_revisions(Some(&[id]), true).unwrap(), 1);
    assert_eq!(accepted.revisions().len(), 0);
    assert!(accepted.equation_at(at).unwrap().contains("abcd"));
    assert_eq!(
        accepted.para_text(0).unwrap().matches('\u{FFFC}').count(),
        1
    );
    s.resolve_revisions(Some(&[id]), false).unwrap();
    assert_eq!(s.revisions().len(), 0);
    same(s.doc(), &base).unwrap();
    // A deletion and an insertion that are not one zone each are listed apart.
    let mut s = Session::new(base);
    s.set_track_changes(true);
    s.set_selection(Selection {
        anchor: Pos::new(0, 0),
        focus: Pos::new(0, 5),
    });
    s.insert_text("y").unwrap();
    let kinds: Vec<_> = s.revisions().iter().map(|r| r.kind).collect();
    assert!(!kinds.contains(&RevKind::Equation), "{kinds:?}");
}

/// Tracked table, row, cell and page setup property changes list what changed
/// (§17.13.5.34 tblPrChange, §17.13.5.37 trPrChange, §17.13.5.36 tcPrChange,
/// §17.13.5.32 sectPrChange).
#[test]
fn tracked_property_changes_list_their_details() {
    let mut s = Session::new(doc());
    s.set_track_changes(true);
    s.set_selection(Selection::caret(Pos::new(cell_para(&s, 0, 0), 0)));
    s.edit_table(&TableEdit::Properties {
        width_pct: Some(80),
        width: None,
        align: Some(Alignment::Center),
    })
    .unwrap();
    s.edit_table(&TableEdit::HeaderRow { on: true }).unwrap();
    s.edit_table(&TableEdit::Shading {
        hex: Some("FFEE00".into()),
    })
    .unwrap();
    run(&mut s, &Act::PageSetup(1));
    let revs = s.revisions();
    let detail = |k: RevKind| {
        revs.iter()
            .find(|r| r.kind == k)
            .map_or_else(|| panic!("{k:?} missing: {revs:?}"), |r| r.detail.clone())
    };
    let t = detail(RevKind::TableFormat);
    assert!(t.contains(&"width") && t.contains(&"align"), "{t:?}");
    assert_eq!(detail(RevKind::RowFormat), ["rowHeader"]);
    assert!(detail(RevKind::CellFormat).contains(&"borders"));
    let p = detail(RevKind::SectionFormat);
    assert!(p.contains(&"margins") && p.contains(&"columns"), "{p:?}");
    assert!(
        p.contains(&"pageSize") && !p.contains(&"titlePage"),
        "{p:?}"
    );
}
