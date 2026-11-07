//! Tests for page setup, headers/footers, notes, tables, Format Painter, Clear
//! Formatting, Paste Special, indents and tab stops.

use super::*;
use crate::section::HfVariant;
use bo_model::{Run, VMerge};

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

fn texts(s: &Session) -> Vec<String> {
    (0..s.paragraph_count())
        .map(|i| s.para_text(i).unwrap_or_default())
        .collect()
}

fn caret(s: &mut Session, p: usize, o: usize) {
    s.set_selection(Selection::caret(Pos::new(p, o)));
}

fn select(s: &mut Session, a: (usize, usize), b: (usize, usize)) {
    s.set_selection(Selection {
        anchor: Pos::new(a.0, a.1),
        focus: Pos::new(b.0, b.1),
    });
}

fn table(s: &Session) -> Table {
    s.doc().sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("table")
}

fn undo_all(s: &mut Session, original: &Document) {
    while s.undo() {}
    assert_eq!(s.doc(), original);
}

#[test]
fn indents_line_spacing_and_tabs() {
    let mut s = Session::new(doc(&["một", "hai"]));
    let original = s.doc().clone();
    select(&mut s, (0, 0), (1, 1));
    s.format_paragraphs(&ParaFormat::Indents {
        left: 720,
        right: 360,
        first_line: -360,
    })
    .unwrap();
    let f = s.format_state();
    assert_eq!(
        (f.indent_left, f.indent_right, f.first_line),
        (720, 360, -360)
    );
    // A hanging indent cannot pass the margin.
    s.format_paragraphs(&ParaFormat::Indents {
        left: 100,
        right: 0,
        first_line: -500,
    })
    .unwrap();
    assert_eq!(s.format_state().first_line, -100);
    s.format_paragraphs(&ParaFormat::LineSpacingRule {
        rule: LineRule::Exact,
        value: 360,
    })
    .unwrap();
    assert_eq!(
        s.format_state().line_spacing,
        LineSpacingState {
            rule: LineRule::Exact,
            value: 360
        }
    );
    let tabs = vec![
        TabSpec {
            pos: 1440,
            align: TabKind::Center,
            leader: LeaderKind::None,
        },
        TabSpec {
            pos: 4320,
            align: TabKind::Decimal,
            leader: LeaderKind::Dot,
        },
    ];
    s.format_paragraphs(&ParaFormat::Tabs { tabs: tabs.clone() })
        .unwrap();
    assert_eq!(s.format_state().tabs, tabs);
    s.format_paragraphs(&ParaFormat::Tabs {
        tabs: tabs[1..].to_vec(),
    })
    .unwrap();
    assert_eq!(s.format_state().tabs, tabs[1..].to_vec());
    undo_all(&mut s, &original);
}

#[test]
fn style_tabs_are_cleared_not_dropped() {
    let mut d = doc(&["a"]);
    let mut st = bo_model::Style::new("Tabbed", "Tabbed", StyleKind::Paragraph);
    st.ppr.tabs = Some(vec![TabStop {
        pos: Twips(2000),
        align: TabAlign::Left,
        leader: TabLeader::None,
    }]);
    d.styles.styles.push(st);
    if let Block::Paragraph(p) = &mut d.sections[0].blocks[0] {
        p.props.style = Some("Tabbed".into());
    }
    let mut s = Session::new(d);
    assert_eq!(s.format_state().tabs.len(), 1);
    s.format_paragraphs(&ParaFormat::Tabs { tabs: Vec::new() })
        .unwrap();
    assert_eq!(s.format_state().tabs, Vec::<TabSpec>::new());
    let direct = s.para_props(0).tabs.unwrap();
    assert_eq!(direct[0].align, TabAlign::Clear);
}

#[test]
fn page_setup_scopes_and_undo() {
    let mut d = doc(&["một", "hai"]);
    let mut second = d.sections[0].clone();
    second.blocks = vec![para("ba")];
    d.sections.push(second);
    let mut s = Session::new(d);
    let original = s.doc().clone();
    caret(&mut s, 2, 0);
    let st = s.section_state();
    assert_eq!((st.index, st.count), (1, 2));
    // Landscape A4 with Nghị định 30 margins and two columns, current section only.
    let spec = PageSetupSpec {
        width: 16_838,
        height: 11_906,
        margin_top: 1134,
        margin_bottom: 1134,
        margin_left: 1701,
        margin_right: 850,
        header: 567,
        footer: 567,
        columns: 2,
        column_space: 425,
    };
    s.set_page_setup(spec, SetupScope::Section).unwrap();
    assert!(s.doc().sections[1].page.is_landscape());
    assert!(!s.doc().sections[0].page.is_landscape());
    assert_eq!(s.doc().sections[1].columns.count, 2);
    s.set_page_setup(spec, SetupScope::Document).unwrap();
    assert!(s.doc().sections[0].page.is_landscape());
    // Absurd margins are clamped to keep a text area.
    s.set_page_setup(
        PageSetupSpec {
            margin_left: 99_999,
            ..spec
        },
        SetupScope::Section,
    )
    .unwrap();
    let p = s.doc().sections[1].page;
    assert!(p.width.0 - p.margin_left.0 - p.margin_right.0 >= 720);
    undo_all(&mut s, &original);
}

#[test]
fn header_footer_options_and_notes() {
    let mut s = Session::new(doc(&["thân bài"]));
    let original = s.doc().clone();
    s.set_header_options(0, Some(true), Some(true)).unwrap();
    assert!(s.doc().sections[0].title_page);
    assert!(s.doc().even_and_odd_headers);
    s.set_header_footer(0, true, HfVariant::First, vec![para("Trang đầu")])
        .unwrap();
    assert_eq!(
        section::effective_hf(s.doc(), 0, true, HfVariant::First)
            .map(|b| b[0].as_paragraph().map(Paragraph::plain_text)),
        Some(Some("Trang đầu".into()))
    );
    // Footnote: reference at the caret, note text editable.
    caret(&mut s, 0, "thân".len());
    let id = s.insert_note(false, "Chú thích").unwrap();
    assert_eq!(s.notes().len(), 1);
    assert_eq!(s.notes()[0].text, "Chú thích");
    s.set_note_text(false, id, "Chú thích mới\ndòng hai")
        .unwrap();
    assert_eq!(s.notes()[0].text, "Chú thích mới\ndòng hai");
    assert!(s.doc().footnotes[&id][0]
        .as_paragraph()
        .unwrap()
        .runs
        .iter()
        .any(|r| r.content.contains(&RunContent::NoteMark)));
    let e = s.insert_note(true, "Kết").unwrap();
    assert!(s.doc().endnotes.contains_key(&e));
    undo_all(&mut s, &original);
}

#[test]
fn table_rows_columns_merge_split_delete() {
    let mut s = Session::new(doc(&[""]));
    let original = s.doc().clone();
    s.insert_table(2, 2).unwrap();
    s.insert_text("A").unwrap();
    let ts = s.format_state().table.unwrap();
    assert_eq!((ts.rows, ts.cols, ts.row, ts.col), (2, 2, 0, 0));
    s.edit_table(&TableEdit::InsertRow { below: true }).unwrap();
    assert_eq!(table(&s).rows.len(), 3);
    assert_eq!(s.format_state().table.unwrap().row, 1);
    s.edit_table(&TableEdit::InsertColumn { right: true })
        .unwrap();
    let t = table(&s);
    assert_eq!(t.grid.len(), 3);
    assert!(t.rows.iter().all(|r| r.cells.len() == 3));
    // The table keeps its width.
    let w: i32 = t.grid.iter().map(|g| g.0).sum();
    assert!((w - 9355).abs() < 5, "{w}");
    // Merge the first row's three cells.
    let first = 0;
    let third = 2;
    select(&mut s, (first, 0), (third, 0));
    s.edit_table(&TableEdit::MergeCells).unwrap();
    let t = table(&s);
    assert_eq!(t.rows[0].cells.len(), 1);
    assert_eq!(t.rows[0].cells[0].props.grid_span, 3);
    assert_eq!(texts(&s)[0], "A");
    assert!(s.format_state().table.unwrap().merged);
    s.edit_table(&TableEdit::SplitCell).unwrap();
    assert_eq!(table(&s).rows[0].cells.len(), 3);
    // Vertical merge of the first column.
    let ids = body_paragraphs(s.doc());
    let below = ids
        .iter()
        .position(|p| p[2..4] == [1, 0])
        .expect("row 1 cell 0");
    select(&mut s, (0, 0), (below, 0));
    s.edit_table(&TableEdit::MergeCells).unwrap();
    let t = table(&s);
    assert_eq!(t.rows[0].cells[0].props.v_merge, Some(VMerge::Restart));
    assert_eq!(t.rows[1].cells[0].props.v_merge, Some(VMerge::Continue));
    caret(&mut s, 0, 0);
    s.edit_table(&TableEdit::DeleteRow).unwrap();
    let t = table(&s);
    assert_eq!(t.rows.len(), 2);
    assert_eq!(t.rows[0].cells[0].props.v_merge, Some(VMerge::Restart));
    s.edit_table(&TableEdit::DeleteColumn).unwrap();
    assert_eq!(table(&s).grid.len(), 2);
    s.edit_table(&TableEdit::Shading {
        hex: Some("DDEEFF".into()),
    })
    .unwrap();
    assert_eq!(
        s.format_state().table.unwrap().shading.as_deref(),
        Some("DDEEFF")
    );
    s.edit_table(&TableEdit::Borders {
        preset: BorderPreset::Outside,
    })
    .unwrap();
    let b = table(&s).props.borders.unwrap();
    assert!(b.top.unwrap().is_visible() && !b.inside_h.unwrap().is_visible());
    s.edit_table(&TableEdit::Properties {
        width_pct: Some(80),
        width: None,
        align: Some(Alignment::Center),
    })
    .unwrap();
    let ts = s.format_state().table.unwrap();
    assert_eq!((ts.width_pct, ts.align), (Some(80), Alignment::Center));
    s.edit_table(&TableEdit::HeaderRow { on: true }).unwrap();
    assert!(table(&s).rows[0].props.header);
    s.edit_table(&TableEdit::DeleteTable).unwrap();
    assert!(s.format_state().table.is_none());
    assert!(!s.doc().sections[0]
        .blocks
        .iter()
        .any(|b| matches!(b, Block::Table(_))));
    undo_all(&mut s, &original);
}

#[test]
fn tab_moves_between_cells_and_adds_a_row() {
    let mut s = Session::new(doc(&[""]));
    s.insert_table(1, 2).unwrap();
    s.insert_text("x").unwrap();
    assert!(s.next_cell(false).unwrap().is_some());
    assert_eq!(s.format_state().table.unwrap().col, 1);
    s.insert_text("y").unwrap();
    // Last cell: Tab adds a row.
    s.next_cell(false).unwrap();
    assert_eq!(table(&s).rows.len(), 2);
    let ts = s.format_state().table.unwrap();
    assert_eq!((ts.row, ts.col), (1, 0));
    s.next_cell(true).unwrap();
    assert_eq!(s.format_state().table.unwrap().row, 0);
    // Shift+Tab selected the previous cell's text.
    assert!(!s.selection().is_collapsed());
    // Outside a table: not handled.
    let mut plain = Session::new(doc(&["a"]));
    assert!(plain.next_cell(false).unwrap().is_none());
}

#[test]
fn format_painter_and_clear_formatting() {
    let mut s = Session::new(doc(&["đậm thường", "khác"]));
    select(&mut s, (0, 0), (0, "đậm".len()));
    s.format_text(&CharFormat::Bold).unwrap();
    s.format_text(&CharFormat::Font {
        name: "Arial".into(),
    })
    .unwrap();
    s.format_paragraphs(&ParaFormat::Align {
        align: Alignment::Center,
    })
    .unwrap();
    // Pick from a caret inside the bold word: character and paragraph formatting.
    caret(&mut s, 0, 1);
    let picked = s.pick_format();
    assert_eq!(picked.run.bold, Some(true));
    assert!(picked.para.is_some());
    // Apply to the word at the caret in the other paragraph.
    caret(&mut s, 1, 1);
    s.apply_picked_format().unwrap();
    let f = s.format_state();
    assert!(f.bold);
    assert_eq!(f.font.as_deref(), Some("Arial"));
    assert_eq!(f.align, Alignment::Center);
    assert_eq!(texts(&s), vec!["đậm thường", "khác"]);
    // Clear formatting removes both in one step.
    select(&mut s, (1, 0), (1, "khác".len()));
    s.clear_formatting().unwrap();
    let f = s.format_state();
    assert!(!f.bold);
    assert_eq!(f.align, Alignment::Left);
    s.undo();
    assert!(s.format_state().bold);
}

#[test]
fn paste_special_modes() {
    let mut s = Session::new(doc(&["đích"]));
    select(&mut s, (0, 0), (0, "đích".len()));
    s.format_text(&CharFormat::Font {
        name: "Carlito".into(),
    })
    .unwrap();
    caret(&mut s, 0, "đích".len());
    let html = "<p><b style=\"font-family:Arial\">nguồn</b> chữ</p>";
    s.paste_with(PasteMode::Merge, None, Some(html), Some("nguồn chữ"))
        .unwrap();
    assert_eq!(texts(&s), vec!["đíchnguồn chữ"]);
    // Merged: destination font, source bold kept.
    select(&mut s, (0, "đích".len()), (0, "đíchng".len()));
    let f = s.format_state();
    assert!(f.bold);
    assert_eq!(f.font.as_deref(), Some("Carlito"));
    caret(&mut s, 0, 0);
    s.paste_with(PasteMode::Text, None, Some(html), None)
        .unwrap();
    assert!(texts(&s)[0].starts_with("nguồn chữ"));
    select(&mut s, (0, 0), (0, 2));
    assert!(!s.format_state().bold);
    caret(&mut s, 0, 0);
    s.paste_with(PasteMode::Keep, None, Some(html), None)
        .unwrap();
    select(&mut s, (0, 0), (0, 2));
    assert!(s.format_state().bold);
}
