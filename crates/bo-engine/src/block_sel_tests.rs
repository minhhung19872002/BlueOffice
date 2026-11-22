use super::*;

/// Three equal lines "abcdef" with the caret after the first "a".
fn engine() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").expect("new");
    for (i, line) in ["abcdef", "abcdef", "abcdef"].iter().enumerate() {
        if i > 0 {
            e.command(Command::SplitParagraph).expect("enter");
        }
        e.command(Command::InsertText {
            text: (*line).to_owned(),
        })
        .expect("type");
    }
    e.command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(0, 1),
            focus: Pos::new(0, 1),
        },
    })
    .expect("caret");
    e
}

fn keys(e: &mut Engine, motions: &[Motion]) -> EditResult {
    let mut r = None;
    for &motion in motions {
        r = Some(e.command(Command::BlockMove { motion }).expect("move"));
    }
    r.expect("a motion")
}

fn para(e: &Engine, i: usize) -> String {
    e.session
        .as_ref()
        .and_then(|s| s.para_text(i))
        .unwrap_or_default()
}

#[test]
fn keyboard_block_covers_the_same_columns_on_each_line() {
    let mut e = engine();
    let r = keys(
        &mut e,
        &[Motion::Down, Motion::Down, Motion::Right, Motion::Right],
    );
    assert!(e.block_active());
    assert_eq!(e.block_text(), ["bc", "bc", "bc"]);
    // One rectangle down the three lines (spacing included), as the reference draws it.
    assert_eq!(r.rects.len(), 1);
    let pieces = e.block_pieces();
    let last = pieces[2].rect;
    assert!((r.rects[0].y - pieces[0].rect.y).abs() < 0.01);
    assert!((r.rects[0].y + r.rects[0].h - (last.y + last.h)).abs() < 0.01);
    // A query keeps the block; a caret move ends it.
    let r = e.command(Command::State).expect("state");
    assert_eq!(r.rects.len(), 1);
    e.command(Command::Move {
        motion: Motion::Right,
        extend: false,
    })
    .expect("move");
    assert!(!e.block_active());
}

#[test]
fn copy_delete_and_bold_act_on_every_piece_in_one_step() {
    let mut e = engine();
    keys(
        &mut e,
        &[Motion::Down, Motion::Down, Motion::Right, Motion::Right],
    );
    let clip = e.command(Command::Copy).expect("copy").clip.expect("clip");
    assert_eq!(clip.text, "bc\nbc\nbc");
    assert!(clip.html.contains("bc"));
    assert!(e.block_active());
    e.command(Command::FormatText {
        format: CharFormat::Bold,
    })
    .expect("bold");
    assert!(e.block_active());
    for p in e.block_pieces() {
        let s = e.session.as_mut().expect("session");
        s.set_selection(Selection {
            anchor: p.from,
            focus: p.to,
        });
        assert!(s.format_state().bold);
    }
    e.command(Command::DeleteBackward).expect("delete");
    assert!(!e.block_active());
    for i in 0..3 {
        assert_eq!(para(&e, i), "adef");
    }
    e.command(Command::Undo).expect("undo");
    assert_eq!(para(&e, 2), "abcdef");
    e.command(Command::Undo).expect("undo bold");
    let s = e.session.as_mut().expect("session");
    s.set_selection(Selection {
        anchor: Pos::new(1, 1),
        focus: Pos::new(1, 3),
    });
    assert!(!s.format_state().bold);
}

#[test]
fn mixed_bold_pieces_all_turn_bold() {
    let mut e = engine();
    e.command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(0, 1),
            focus: Pos::new(0, 3),
        },
    })
    .expect("sel");
    e.command(Command::FormatText {
        format: CharFormat::Bold,
    })
    .expect("bold one");
    e.command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(0, 1),
            focus: Pos::new(0, 1),
        },
    })
    .expect("caret");
    keys(&mut e, &[Motion::Down, Motion::Right, Motion::Right]);
    e.command(Command::FormatText {
        format: CharFormat::Bold,
    })
    .expect("bold block");
    for p in e.block_pieces() {
        let s = e.session.as_mut().expect("session");
        s.set_selection(Selection {
            anchor: p.from,
            focus: p.to,
        });
        assert!(s.format_state().bold, "{p:?}");
    }
}

#[test]
fn mouse_block_and_typing_replace_the_pieces() {
    let mut e = engine();
    let layout = e.layout.document();
    let a = nav::caret_rect(layout, Pos::new(0, 2)).expect("a");
    let b = nav::caret_rect(layout, Pos::new(1, 4)).expect("b");
    e.command(Command::BlockSelect {
        page: a.page,
        x: a.x,
        y: a.y + a.h / 2.0,
        extend: false,
    })
    .expect("press");
    let r = e
        .command(Command::BlockSelect {
            page: b.page,
            x: b.x,
            y: b.y + b.h / 2.0,
            extend: true,
        })
        .expect("drag");
    assert_eq!(e.block_text(), ["cd", "cd"]);
    assert_eq!(r.rects.len(), 1);
    assert!((r.rects[0].x - a.x).abs() < 0.01 && (r.rects[0].x + r.rects[0].w - b.x).abs() < 0.01);
    e.command(Command::InsertText { text: "X".into() })
        .expect("type");
    assert_eq!(para(&e, 0), "abXef");
    assert_eq!(para(&e, 1), "abef");
    assert!(!e.block_active());
    e.command(Command::BlockMove {
        motion: Motion::Down,
    })
    .expect("start again");
    assert!(e.block_active());
    e.command(Command::BlockClear).expect("clear");
    assert!(!e.block_active());
}

#[test]
fn a_block_inside_a_table_cell_stays_in_that_cell_and_leaving_it_selects_on() {
    // Measured on the reference (COM ColumnSelectMode): a block started in a cell covers only
    // that cell's lines; moving its corner past the cell turns it into an ordinary selection.
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").expect("new");
    e.command(Command::InsertTable { rows: 2, cols: 2 })
        .expect("table");
    let p = e.session.as_ref().expect("session").selection().focus.para;
    for (i, line) in ["abcdef", "abcdef", "abcdef"].iter().enumerate() {
        if i > 0 {
            e.command(Command::SplitParagraph).expect("enter");
        }
        e.command(Command::InsertText {
            text: (*line).to_owned(),
        })
        .expect("type");
    }
    // The cell beside it holds text at the same heights.
    let at = |para, offset| Selection::caret(Pos::new(para, offset));
    e.command(Command::SetSelection {
        selection: at(p + 3, 0),
    })
    .expect("next cell");
    e.command(Command::InsertText {
        text: "uvwxyz".into(),
    })
    .expect("type");
    e.command(Command::SetSelection {
        selection: at(p, 1),
    })
    .expect("caret");
    keys(
        &mut e,
        &[Motion::Down, Motion::Down, Motion::Right, Motion::Right],
    );
    assert_eq!(e.block_text(), ["bc", "bc", "bc"]);
    // Down past the cell's last line: no block, an ordinary selection from the anchor.
    keys(&mut e, &[Motion::Down]);
    assert!(!e.block_active());
    let sel = e.session.as_ref().expect("session").selection();
    assert_eq!(sel.anchor, Pos::new(p, 1));
    assert!(sel.focus.para > p + 2);
}
