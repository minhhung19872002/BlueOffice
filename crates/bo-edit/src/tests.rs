use super::*;
use bo_model::{Run, StyleSheet};
use proptest::prelude::*;

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

#[test]
fn typing_vietnamese_and_undo_by_word() {
    let mut s = Session::new(doc(&[""]));
    for c in "Xin chào Việt Nam".chars() {
        s.insert_text(&c.to_string()).unwrap();
    }
    assert_eq!(texts(&s), vec!["Xin chào Việt Nam"]);
    assert_eq!(s.selection().focus, Pos::new(0, "Xin chào Việt Nam".len()));
    // Typing is grouped per word: undo removes "Nam".
    assert!(s.undo());
    assert_eq!(texts(&s), vec!["Xin chào Việt "]);
    while s.undo() {}
    assert_eq!(texts(&s), vec![""]);
    while s.redo() {}
    assert_eq!(texts(&s), vec!["Xin chào Việt Nam"]);
}

#[test]
fn delete_by_grapheme_and_merge() {
    let mut s = Session::new(doc(&["ab", "Việt"]));
    caret(&mut s, 1, "Việt".len());
    s.delete_backward().unwrap();
    assert_eq!(texts(&s)[1], "Việ");
    s.delete_backward().unwrap();
    assert_eq!(texts(&s)[1], "Vi");
    caret(&mut s, 1, 0);
    s.delete_backward().unwrap();
    assert_eq!(texts(&s), vec!["abVi"]);
    assert_eq!(s.selection().focus, Pos::new(0, 2));
    caret(&mut s, 0, 0);
    s.delete_backward().unwrap(); // nothing before the first paragraph
    s.delete_forward().unwrap();
    assert_eq!(texts(&s), vec!["bVi"]);
    let mut s = Session::new(doc(&["a", "b"]));
    caret(&mut s, 0, 1);
    s.delete_forward().unwrap();
    assert_eq!(texts(&s), vec!["ab"]);
}

#[test]
fn enter_splits_and_selection_delete() {
    let mut s = Session::new(doc(&["Hai đoạn"]));
    caret(&mut s, 0, 3);
    s.split_paragraph().unwrap();
    assert_eq!(texts(&s), vec!["Hai", " đoạn"]);
    assert_eq!(s.selection().focus, Pos::new(1, 0));
    select(&mut s, (0, 1), (1, 2));
    s.insert_text("X").unwrap();
    assert_eq!(texts(&s), vec!["HXđoạn"]);
    s.insert_text("1\n2").unwrap();
    assert_eq!(texts(&s), vec!["HX1", "2đoạn"]);
}

#[test]
fn character_formatting_toggles() {
    let mut s = Session::new(doc(&["đậm nhạt"]));
    select(&mut s, (0, 0), (0, "đậm".len()));
    s.format_text(&CharFormat::Bold).unwrap();
    let p = bo_model::index::para_at(s.doc(), &[0, 0]).unwrap().clone();
    assert_eq!(p.runs[0].props.bold, Some(true));
    assert_eq!(p.runs[0].plain_text(), "đậm");
    assert!(s.format_state().bold);
    s.format_text(&CharFormat::Bold).unwrap();
    assert!(!s.format_state().bold);
    s.format_text(&CharFormat::Color {
        hex: Some("C00000".into()),
    })
    .unwrap();
    s.format_text(&CharFormat::Size { half_points: 28 })
        .unwrap();
    s.format_text(&CharFormat::Font {
        name: "Arial".into(),
    })
    .unwrap();
    let st = s.format_state();
    assert_eq!(st.color.as_deref(), Some("C00000"));
    assert_eq!(st.size_half_points, Some(28));
    assert_eq!(st.font.as_deref(), Some("Arial"));
    // Collapsed selection: formatting applies to the next typed text.
    caret(&mut s, 0, "đậm nhạt".len());
    s.format_text(&CharFormat::Italic).unwrap();
    s.insert_text("!").unwrap();
    let p = bo_model::index::para_at(s.doc(), &[0, 0]).unwrap();
    assert_eq!(p.runs.last().unwrap().props.italic, Some(true));
    select(&mut s, (0, 0), (0, 3));
    s.format_text(&CharFormat::Clear).unwrap();
    assert_eq!(
        bo_model::index::para_at(s.doc(), &[0, 0]).unwrap().runs[0].props,
        RunProps::default()
    );
}

#[test]
fn paragraph_formatting_lists_and_styles() {
    let mut d = doc(&["một", "hai"]);
    let mut h = bo_model::Style::new("Heading1", "heading 1", StyleKind::Paragraph);
    h.next = Some("Normal".into());
    h.rpr.bold = Some(true);
    d.styles = StyleSheet {
        styles: vec![
            bo_model::Style::new("Normal", "Normal", StyleKind::Paragraph),
            h,
        ],
        latent: None,
    };
    let mut s = Session::new(d);
    select(&mut s, (0, 0), (1, 1));
    s.format_paragraphs(&ParaFormat::Align {
        align: Alignment::Center,
    })
    .unwrap();
    assert_eq!(s.format_state().align, Alignment::Center);
    s.format_paragraphs(&ParaFormat::Bullets).unwrap();
    assert!(s.format_state().bullets);
    assert_eq!(s.doc().numbering.nums.len(), 1);
    s.format_paragraphs(&ParaFormat::IncreaseIndent).unwrap();
    let p = bo_model::index::para_at(s.doc(), &[0, 1]).unwrap();
    assert_eq!(p.props.num.unwrap().ilvl, 1);
    s.format_paragraphs(&ParaFormat::Bullets).unwrap();
    assert!(!s.format_state().bullets);
    s.format_paragraphs(&ParaFormat::Numbering).unwrap();
    assert!(s.format_state().numbering);
    caret(&mut s, 0, 0);
    s.apply_style("Heading1").unwrap();
    assert_eq!(s.format_state().style.as_deref(), Some("Heading1"));
    assert!(s.format_state().bold);
    // Enter at the end of a heading switches to the next style.
    caret(&mut s, 0, "một".len());
    s.split_paragraph().unwrap();
    let p = bo_model::index::para_at(s.doc(), &[0, 1]).unwrap();
    assert_eq!(p.props.style.as_deref(), Some("Normal"));
    assert!(s.apply_style("Missing").is_err());
    // Built-in styles are created on first use and removed again by undo.
    let mut s = Session::new(doc(&["x"]));
    s.apply_style("Heading2").unwrap();
    assert!(s.doc().style("Heading2").is_some());
    assert!(s.format_state().bold);
    s.undo();
    assert_eq!(s.doc().styles.styles.len(), 0);
}

#[test]
fn insert_table_image_breaks() {
    let mut s = Session::new(doc(&["trước sau"]));
    caret(&mut s, 0, "trước".len());
    s.insert_table(2, 3).unwrap();
    // Paragraph split around the table; caret in the first cell.
    let ids = body_paragraphs(s.doc());
    assert_eq!(ids.len(), 2 + 6);
    assert_eq!(s.selection().focus, Pos::new(1, 0));
    s.insert_text("ô").unwrap();
    assert_eq!(texts(&s)[1], "ô");
    assert_eq!(texts(&s).last().unwrap(), " sau");
    caret(&mut s, 0, 0);
    s.insert_image(vec![1, 2, 3], "png", 914_400, 457_200)
        .unwrap();
    assert_eq!(s.doc().media.len(), 1);
    assert!(texts(&s)[0].starts_with('\u{FFFC}'));
    s.insert_page_break().unwrap();
    assert_eq!(texts(&s)[0], "\u{FFFC}\u{000C}");
    let n = s.paragraph_count();
    caret(&mut s, n - 1, 0);
    s.insert_section_break(SectionBreak::NextPage).unwrap();
    assert_eq!(s.doc().sections.len(), 2);
    assert_eq!(s.doc().sections[1].break_type, SectionBreak::NextPage);
    // A section break inside a table cell is refused and leaves no trace.
    let before = s.doc().clone();
    caret(&mut s, 2, 0);
    assert!(s.insert_section_break(SectionBreak::Continuous).is_err());
    assert_eq!(*s.doc(), before);
    while s.undo() {}
    assert_eq!(texts(&s), vec!["trước sau"]);
    assert!(s.doc().media.is_empty());
}

#[test]
fn find_and_replace() {
    let mut s = Session::new(doc(&["Hà Nội và hà nội", "HÀ NỘI, hànội"]));
    let o = FindOptions::default();
    assert_eq!(s.find("hà nội", &o).unwrap().len(), 3);
    let cs = FindOptions {
        match_case: true,
        ..o
    };
    assert_eq!(s.find("hà nội", &cs).unwrap().len(), 1);
    let ww = FindOptions {
        whole_word: true,
        ..o
    };
    assert_eq!(s.find("hà", &ww).unwrap().len(), 3);
    let re = FindOptions { regex: true, ..o };
    assert_eq!(s.find(r"h[àÀ]\s?n", &re).unwrap().len(), 4);
    assert!(s.find("(", &re).is_err());
    let n = s.replace_all("hà nội", &o, "HN").unwrap();
    assert_eq!(n, 3);
    assert_eq!(texts(&s), vec!["HN và HN", "HN, hànội"]);
    let n = s.replace_all(r"(\w+)nội", &re, "[$1]").unwrap();
    assert_eq!(n, 1);
    assert_eq!(texts(&s)[1], "HN, [hà]");
    s.undo();
    s.undo();
    assert_eq!(texts(&s), vec!["Hà Nội và hà nội", "HÀ NỘI, hànội"]);
    let m = s.find("và", &o).unwrap()[0];
    s.replace(m, "&").unwrap();
    assert_eq!(texts(&s)[0], "Hà Nội & hà nội");
}

#[test]
fn clipboard_internal_html_text() {
    let mut s = Session::new(doc(&["một hai", "ba bốn"]));
    select(&mut s, (0, "một ".len()), (1, 2));
    s.format_text(&CharFormat::Bold).unwrap();
    let clip = s.copy();
    assert_eq!(clip.text, "hai\nba");
    assert!(clip.html.contains("font-weight:bold"));
    caret(&mut s, 1, "ba bốn".len());
    s.paste_internal(&clip.internal).unwrap();
    assert_eq!(texts(&s), vec!["một hai", "ba bốnhai", "ba"]);
    let p = bo_model::index::para_at(s.doc(), &[0, 1]).unwrap();
    assert_eq!(p.runs.last().unwrap().props.bold, Some(true));
    s.paste_text("x\ny").unwrap();
    assert_eq!(texts(&s), vec!["một hai", "ba bốnhai", "bax", "y"]);
    s.paste_html("<p>p1</p><p><i>p2</i></p>").unwrap();
    assert_eq!(texts(&s)[3..], ["yp1".to_owned(), "p2".to_owned()]);
    assert!(s.paste_internal("not json").is_err());
    select(&mut s, (0, 0), (0, "một".len()));
    let c = s.cut().unwrap();
    assert_eq!(c.text, "một");
    assert_eq!(texts(&s)[0], " hai");
}

#[test]
fn navigation_helpers() {
    let mut s = Session::new(doc(&["Xin chào bạn", "dòng hai"]));
    let w = s.word_at(Pos::new(0, 5));
    assert_eq!((w.anchor.offset, w.focus.offset), (4, 4 + "chào".len()));
    let p = s.paragraph_at(Pos::new(1, 2));
    assert_eq!(p.focus.offset, "dòng hai".len());
    assert_eq!(s.word_boundary(Pos::new(0, 0), true), Pos::new(0, 4));
    assert_eq!(s.word_boundary(Pos::new(0, 4), false), Pos::new(0, 0));
    assert_eq!(
        s.char_step(Pos::new(0, "Xin chào bạn".len()), true),
        Pos::new(1, 0)
    );
    assert_eq!(
        s.char_step(Pos::new(1, 0), false),
        Pos::new(0, "Xin chào bạn".len())
    );
    assert_eq!(s.char_step(Pos::new(0, 6), true), Pos::new(0, 7 + 1));
    let all = s.select_all();
    assert_eq!(all.focus, Pos::new(1, "dòng hai".len()));
    // Clamping to char boundaries and existing paragraphs.
    assert_eq!(s.clamp(Pos::new(9, 99)), Pos::new(1, "dòng hai".len()));
    assert_eq!(s.clamp(Pos::new(0, 7)), Pos::new(0, 6));
}

#[test]
fn ops_are_checked_and_invertible() {
    let mut d = doc(&["a"]);
    let op = Op::Blocks {
        path: vec![0],
        index: 0,
        remove: vec![para("zzz")],
        insert: vec![],
    };
    assert_eq!(op.apply(&mut d), Err(EditError::Conflict));
    let op = Op::Blocks {
        path: vec![7],
        index: 0,
        remove: vec![],
        insert: vec![],
    };
    assert_eq!(op.apply(&mut d), Err(EditError::BadPath));
    let op = Op::Blocks {
        path: vec![0],
        index: 1,
        remove: vec![],
        insert: vec![para("b")],
    };
    let orig = d.clone();
    op.apply(&mut d).unwrap();
    op.inverse().apply(&mut d).unwrap();
    assert_eq!(d, orig);
}

#[derive(Debug, Clone)]
enum Action {
    Caret(usize, usize),
    Select(usize, usize, usize, usize),
    Type(String),
    Backspace,
    Delete,
    Enter,
    Bold,
    Align(u8),
    Bullets,
    Table(usize, usize),
    PageBreak,
    Paste,
    Replace,
    Style,
    TableTool(u8),
}

/// Table tools (sort, distribute, AutoFit, direction, wrap, text ↔ table).
fn table_tool(k: u8) -> TableEdit {
    use crate::table_tools::{AutoFit, SortKey, SortKind};
    match k {
        0 => TableEdit::Sort {
            keys: vec![SortKey {
                column: 0,
                kind: SortKind::Text,
                descending: true,
            }],
            header: false,
        },
        1 => TableEdit::DistributeRows {
            heights: vec![200, 400, 300],
        },
        2 => TableEdit::DistributeColumns,
        3 => TableEdit::AutoFit {
            mode: AutoFit::Window,
            widths: Vec::new(),
        },
        4 => TableEdit::TextDirection {
            dir: Some(bo_model::TextDirection::TbRl),
        },
        5 => TableEdit::Wrap { around: true },
        6 => TableEdit::ToText {
            separator: "\t".into(),
        },
        _ => TableEdit::FromText {
            separator: " ".into(),
            cols: None,
        },
    }
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        (0usize..20, 0usize..40).prop_map(|(p, o)| Action::Caret(p, o)),
        (0usize..20, 0usize..40, 0usize..20, 0usize..40)
            .prop_map(|(a, b, c, d)| Action::Select(a, b, c, d)),
        "[a-zàáạảãăắâđêôơư \\n]{1,6}".prop_map(Action::Type),
        Just(Action::Backspace),
        Just(Action::Delete),
        Just(Action::Enter),
        Just(Action::Bold),
        (0u8..4).prop_map(Action::Align),
        Just(Action::Bullets),
        (1usize..3, 1usize..3).prop_map(|(r, c)| Action::Table(r, c)),
        Just(Action::PageBreak),
        Just(Action::Paste),
        Just(Action::Replace),
        Just(Action::Style),
        (0u8..8).prop_map(Action::TableTool),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]
    /// prompt §7: random edits followed by undoing everything restore the original document,
    /// and redoing everything reproduces the edited one.
    #[test]
    fn undo_all_restores_original(actions in proptest::collection::vec(action(), 1..30)) {
        let mut d = doc(&["Căn cứ Luật", "Tổ chức Chính phủ", ""]);
        d.styles.styles.push(bo_model::Style::new("Heading1", "heading 1", StyleKind::Paragraph));
        let original = d.clone();
        let mut s = Session::new(d);
        for a in &actions {
            let _ = match a {
                Action::Caret(p, o) => { caret(&mut s, *p, *o); Ok(s.selection()) }
                Action::Select(a, b, c, d) => { select(&mut s, (*a, *b), (*c, *d)); Ok(s.selection()) }
                Action::Type(t) => s.insert_text(t),
                Action::Backspace => s.delete_backward(),
                Action::Delete => s.delete_forward(),
                Action::Enter => s.split_paragraph(),
                Action::Bold => s.format_text(&CharFormat::Bold),
                Action::Align(k) => s.format_paragraphs(&ParaFormat::Align { align: [Alignment::Left, Alignment::Center, Alignment::Right, Alignment::Justify][usize::from(*k)] }),
                Action::Bullets => s.format_paragraphs(&ParaFormat::Bullets),
                Action::Table(r, c) => s.insert_table(*r, *c),
                Action::PageBreak => s.insert_page_break(),
                Action::Paste => { let c = s.copy(); s.paste_internal(&c.internal) }
                Action::Replace => s.replace_all("a", &FindOptions::default(), "ă").map(|_| s.selection()),
                Action::Style => s.apply_style("Heading1"),
                Action::TableTool(k) => s.edit_table(&table_tool(*k)),
            };
            // The selection always addresses existing text.
            let sel = s.selection();
            prop_assert_eq!(s.clamp(sel.anchor), sel.anchor);
            prop_assert_eq!(s.clamp(sel.focus), sel.focus);
        }
        let edited = s.doc().clone();
        while s.undo() {}
        prop_assert_eq!(s.doc(), &original);
        while s.redo() {}
        prop_assert_eq!(s.doc(), &edited);
    }

    /// Track changes (ECMA-376 Part 1, §17.13.5): text edits recorded as revisions are
    /// undone exactly, rejecting all of them restores the original text and accepting all
    /// of them gives the text shown by the "final" view.
    #[test]
    fn tracked_edits_reject_restores_text(actions in proptest::collection::vec(action(), 1..30)) {
        let d = doc(&["Căn cứ Luật", "Tổ chức Chính phủ", ""]);
        let original = d.clone();
        let mut s = Session::new(d);
        s.set_track_changes(true);
        s.set_author("Người duyệt");
        for a in &actions {
            let _ = match a {
                Action::Caret(p, o) => { caret(&mut s, *p, *o); Ok(s.selection()) }
                Action::Select(a, b, c, d) => { select(&mut s, (*a, *b), (*c, *d)); Ok(s.selection()) }
                Action::Type(t) => s.insert_text(t),
                Action::Backspace => s.delete_backward(),
                Action::Delete => s.delete_forward(),
                Action::Enter => s.split_paragraph(),
                Action::Bold => s.format_text(&CharFormat::Bold),
                Action::Align(k) => s.format_paragraphs(&ParaFormat::Align { align: [Alignment::Left, Alignment::Center, Alignment::Right, Alignment::Justify][usize::from(*k)] }),
                Action::Paste => { let c = s.copy(); s.paste_internal(&c.internal) }
                Action::Replace => s.replace_all("a", &FindOptions::default(), "ă").map(|_| s.selection()),
                _ => Ok(s.selection()),
            };
            let sel = s.selection();
            prop_assert_eq!(s.clamp(sel.anchor), sel.anchor);
            prop_assert_eq!(s.clamp(sel.focus), sel.focus);
        }
        let edited = s.doc().clone();
        let final_text = edited.plain_text();
        let mut accepted = Session::new(edited.clone());
        accepted.resolve_revisions(None, true).unwrap();
        prop_assert!(accepted.revisions().is_empty());
        prop_assert_eq!(accepted.doc().plain_text(), final_text);
        let n = s.resolve_revisions(None, false).unwrap();
        prop_assert!(s.revisions().is_empty());
        prop_assert_eq!(s.doc().plain_text(), original.plain_text());
        // Accepting/rejecting is one undo step.
        if n > 0 {
            prop_assert!(s.undo());
            prop_assert_eq!(s.doc(), &edited);
        }
        while s.undo() {}
        prop_assert_eq!(s.doc(), &original);
    }
}

#[test]
fn comments_threads_resolve_delete_and_undo() {
    let mut s = Session::new(doc(&["Điều 1. Phạm vi điều chỉnh", "Điều 2."]));
    s.set_author("Nguyễn Văn An");
    s.set_clock(Some("2026-11-08T10:00:00Z".into()));
    select(&mut s, (0, "Điều 1. ".len()), (0, "Điều 1. Phạm vi".len()));
    let id = s.add_comment("Làm rõ phạm vi").unwrap();
    // Marks are zero-width: the text is unchanged.
    assert_eq!(texts(&s)[0], "Điều 1. Phạm vi điều chỉnh");
    let c = &s.comments()[0];
    assert_eq!(c.id, id);
    assert_eq!(c.text, "Làm rõ phạm vi");
    assert_eq!(c.quote, "Phạm vi");
    assert_eq!(c.initials.as_deref(), Some("NVA"));
    assert_eq!(c.date.as_deref(), Some("2026-11-08T10:00:00Z"));
    assert_eq!(c.start, Some(Pos::new(0, "Điều 1. ".len())));
    // A reply is threaded under its parent ([MS-DOCX] paraIdParent).
    s.set_author("Trần Thị Bình");
    let reply = s.reply_comment(id, "Đã sửa").unwrap();
    let list = s.comments();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1].parent, Some(id));
    assert_eq!(list[1].quote, "Phạm vi");
    s.resolve_comment(id, true).unwrap();
    assert!(s.comments()[0].done);
    // Collapsed selection: the word at the caret.
    caret(&mut s, 1, 2);
    let second = s.add_comment("Thiếu nội dung").unwrap();
    assert_eq!(s.comments()[2].quote, "Điều");
    caret(&mut s, 0, 0);
    assert_eq!(s.next_comment(true), Some(id));
    assert_eq!(s.next_comment(true), Some(second));
    // Deleting a thread removes the replies and every mark.
    s.delete_comment(id).unwrap();
    assert_eq!(s.comments().len(), 1);
    assert!(s.select_comment(second));
    assert!(!s.select_comment(reply));
    while s.undo() {}
    assert_eq!(s.doc().comments.len(), 0);
    assert_eq!(texts(&s), vec!["Điều 1. Phạm vi điều chỉnh", "Điều 2."]);
    assert_eq!(s.doc(), &doc(&["Điều 1. Phạm vi điều chỉnh", "Điều 2."]));
}

fn tracked(paras: &[&str]) -> Session {
    let mut s = Session::new(doc(paras));
    s.set_track_changes(true);
    s.set_author("An");
    s.set_clock(Some("2026-11-08T09:00:00Z".into()));
    s
}

#[test]
fn track_changes_records_insertions_and_deletions() {
    let mut s = tracked(&["Xin chào"]);
    caret(&mut s, 0, 3);
    for c in "XYZ".chars() {
        s.insert_text(&c.to_string()).unwrap();
    }
    // One insertion revision per typing burst, attributed to the author.
    let revs = s.revisions();
    assert_eq!(revs.len(), 1);
    assert_eq!(revs[0].kind, RevKind::Insert);
    assert_eq!(revs[0].author, "An");
    assert_eq!(revs[0].date.as_deref(), Some("2026-11-08T09:00:00Z"));
    assert_eq!(revs[0].text, "XYZ");
    // Backspace inside one's own insertion really removes it.
    s.delete_backward().unwrap();
    assert_eq!(texts(&s), vec!["XinXY chào"]);
    // Deleting original text keeps it, marked deleted; the caret moves before it.
    caret(&mut s, 0, "XinXY ".len());
    s.delete_forward().unwrap();
    assert_eq!(texts(&s), vec!["XinXY chào"]);
    assert_eq!(s.doc().plain_text(), "XinXY hào");
    assert_eq!(s.selection().focus, Pos::new(0, "XinXY c".len()));
    let kinds: Vec<RevKind> = s.revisions().iter().map(|r| r.kind).collect();
    assert_eq!(kinds, vec![RevKind::Insert, RevKind::Delete]);
    // Accept one (the deletion), reject the other.
    let del_id = s.revisions()[1].id;
    s.resolve_revisions(Some(&[del_id]), true).unwrap();
    assert_eq!(texts(&s), vec!["XinXY hào"]);
    s.resolve_revisions(None, false).unwrap();
    assert_eq!(texts(&s), vec!["Xin hào"]);
    assert_eq!(s.revisions().len(), 0);
}

#[test]
fn track_changes_paragraph_marks_and_formatting() {
    let mut s = tracked(&["Một hai", "ba"]);
    caret(&mut s, 0, "Một".len());
    s.split_paragraph().unwrap();
    assert_eq!(texts(&s), vec!["Một", " hai", "ba"]);
    let r = s.revisions();
    assert_eq!(r[0].kind, RevKind::ParagraphInsert);
    // Rejecting the inserted mark joins the paragraphs again.
    s.resolve_revisions(None, false).unwrap();
    assert_eq!(texts(&s), vec!["Một hai", "ba"]);
    // Backspace at a paragraph start marks the previous mark deleted; accepting merges.
    caret(&mut s, 1, 0);
    s.delete_backward().unwrap();
    assert_eq!(s.paragraph_count(), 2);
    assert_eq!(s.selection().focus, Pos::new(0, "Một hai".len()));
    assert_eq!(s.revisions()[0].kind, RevKind::ParagraphDelete);
    s.resolve_revisions(None, true).unwrap();
    assert_eq!(texts(&s), vec!["Một haiba"]);
    // Formatting is recorded with the previous properties (rPrChange / pPrChange).
    select(&mut s, (0, 0), (0, "Một".len()));
    s.format_text(&CharFormat::Bold).unwrap();
    s.format_paragraphs(&ParaFormat::Align {
        align: Alignment::Center,
    })
    .unwrap();
    let kinds: Vec<RevKind> = s.revisions().iter().map(|r| r.kind).collect();
    assert_eq!(kinds, vec![RevKind::ParagraphFormat, RevKind::Format]);
    s.resolve_revisions(None, false).unwrap();
    let p = s.doc().paragraphs().next().unwrap();
    assert_eq!(p.props.align, None);
    assert!(p.runs.iter().all(|r| r.props.bold.is_none()));
    // Without track changes, typing next to an insertion is not tracked.
    let mut s = tracked(&["ab"]);
    caret(&mut s, 0, 1);
    s.insert_text("x").unwrap();
    s.set_track_changes(false);
    s.insert_text("y").unwrap();
    assert_eq!(s.revisions()[0].text, "x");
}
