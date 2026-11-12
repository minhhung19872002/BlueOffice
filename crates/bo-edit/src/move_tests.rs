//! Tracked moves (ECMA-376 Part 1, §17.13.5.22–27), formatting change details
//! (§17.13.5.29 pPrChange, §17.13.5.31 rPrChange) and locked tracking (§17.15.1.29
//! documentProtection `w:edit="trackedChanges"`). Expected values are worked out by
//! hand from the specification's description of each element.

use super::*;
use bo_model::form::{DocProtection, PROTECT_TRACKED};
use bo_model::{RevMark, Run};

fn para(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run::text(text, RunProps::default())],
    ))
}

fn doc(texts: &[&str]) -> Document {
    Document {
        sections: vec![Section::new(texts.iter().map(|t| para(t)).collect())],
        ..Document::default()
    }
}

/// Text of every body paragraph as laid out in the markup view (deleted text kept).
fn texts(s: &Session) -> Vec<String> {
    (0..s.paragraph_count())
        .map(|i| s.para_text(i).unwrap_or_default())
        .collect()
}

/// Number of verbatim move range markers left in the body.
fn markers(d: &Document) -> usize {
    let mut n = 0;
    for b in &d.sections[0].blocks {
        if let Block::Paragraph(p) = b {
            for r in &p.runs {
                for c in &r.content {
                    if let RunContent::Markup(raw) = c {
                        if crate::moves::move_marker(&raw.xml).is_some() {
                            n += 1;
                        }
                    }
                }
            }
        }
    }
    n
}

fn tracked(texts: &[&str]) -> Session {
    let mut s = Session::new(doc(texts));
    s.set_author("An");
    s.set_track_changes(true);
    s
}

#[test]
fn move_down_records_a_pair_and_accepting_one_half_resolves_both() {
    let mut s = tracked(&["A", "B", "C"]);
    s.set_selection(Selection::caret(Pos::new(0, 0)));
    let sel = s.move_paragraphs(false).unwrap();
    // [A moved from, B, A moved to, C]: the copy (paragraph 2) is selected.
    assert_eq!(texts(&s), ["A", "B", "A", "C"]);
    assert_eq!(sel.ordered(), (Pos::new(2, 0), Pos::new(2, 1)));
    let revs = s.revisions();
    let kinds: Vec<RevKind> = revs.iter().map(|r| r.kind).collect();
    // Text and paragraph mark of each half form one revision.
    assert_eq!(kinds, [RevKind::MoveFrom, RevKind::MoveTo]);
    assert_ne!(revs[0].id, revs[1].id);
    assert_eq!(markers(s.doc()), 4);
    // Accepting the moveFrom half accepts the whole move.
    let n = s.resolve_revisions(Some(&[revs[0].id]), true).unwrap();
    assert_eq!(n, 2);
    assert_eq!(texts(&s), ["B", "A", "C"]);
    assert_eq!(s.revisions().len(), 0);
    assert_eq!(markers(s.doc()), 0);
}

#[test]
fn rejecting_a_move_restores_the_original() {
    let original = doc(&["A", "B", "C"]);
    let mut s = Session::new(original.clone());
    s.set_track_changes(true);
    s.set_selection(Selection::caret(Pos::new(2, 0)));
    s.move_paragraphs(true).unwrap();
    // [C moved to, A, B, C moved from]: moving up passes paragraph B.
    assert_eq!(texts(&s), ["A", "C", "B", "C"]);
    let to = s
        .revisions()
        .iter()
        .find(|r| r.kind == RevKind::MoveTo)
        .map(|r| r.id)
        .unwrap();
    s.resolve_revisions(Some(&[to]), false).unwrap();
    assert_eq!(s.doc().sections, original.sections);
}

#[test]
fn move_two_paragraphs_up_and_accept_all() {
    let mut s = tracked(&["A", "B", "C", "D"]);
    s.set_selection(Selection {
        anchor: Pos::new(1, 0),
        focus: Pos::new(2, 1),
    });
    s.move_paragraphs(true).unwrap();
    assert_eq!(texts(&s), ["B", "C", "A", "B", "C", "D"]);
    assert_eq!(s.revisions().len(), 2);
    s.resolve_revisions(None, true).unwrap();
    assert_eq!(texts(&s), ["B", "C", "A", "D"]);
    assert_eq!(markers(s.doc()), 0);
    // Undo brings the tracked move back.
    s.undo();
    assert_eq!(s.revisions().len(), 2);
}

#[test]
fn untracked_move_reorders_and_moves_to_the_end_of_the_text() {
    let mut s = Session::new(doc(&["A", "B", "C"]));
    s.set_selection(Selection::caret(Pos::new(0, 0)));
    s.move_paragraphs(false).unwrap();
    assert_eq!(texts(&s), ["B", "A", "C"]);
    assert_eq!(s.revisions().len(), 0);
    // At the top nothing moves.
    let original = doc(&["A", "B"]);
    let mut s = Session::new(original.clone());
    s.set_selection(Selection::caret(Pos::new(0, 0)));
    s.move_paragraphs(true).unwrap();
    assert_eq!(texts(&s), ["A", "B"]);
    // Tracked, the copy ends the text: B's mark becomes the inserted one.
    s.set_track_changes(true);
    s.move_paragraphs(false).unwrap();
    assert_eq!(texts(&s), ["A", "B", "A"]);
    let kinds: Vec<RevKind> = s.revisions().iter().map(|r| r.kind).collect();
    assert_eq!(kinds, [RevKind::MoveFrom, RevKind::MoveTo]);
    let mut accepted = s.clone();
    accepted.resolve_revisions(None, true).unwrap();
    assert_eq!(texts(&accepted), ["B", "A"]);
    s.resolve_revisions(None, false).unwrap();
    assert_eq!(s.doc().sections, original.sections);
    // Different paragraph properties at the end cannot be kept apart: refused.
    let mut d = doc(&["A", "B"]);
    if let Block::Paragraph(p) = &mut d.sections[0].blocks[1] {
        p.props.align = Some(Alignment::Center);
    }
    let mut s = Session::new(d);
    s.set_track_changes(true);
    assert!(s.move_paragraphs(false).is_err());
}

#[test]
fn file_moves_pair_by_range_name() {
    // Two halves with their own ids, tied by moveFromRangeStart / moveToRangeStart
    // sharing w:name="move1" (§17.13.5.23, §17.13.5.27).
    let raw = |x: &str| {
        (
            RunProps::default(),
            RunContent::Markup(bo_model::Raw::new(x, 0)),
        )
    };
    let mark = |id: i32| RevMark {
        info: bo_model::RevisionInfo {
            id,
            author: "B".into(),
            date: None,
        },
        moved: true,
    };
    let from = RunProps {
        del: Some(mark(8)),
        ..RunProps::default()
    };
    let to = RunProps {
        ins: Some(mark(10)),
        ..RunProps::default()
    };
    let p1 = para::with_atoms(
        &Paragraph::default(),
        vec![
            raw(r#"<w:moveFromRangeStart w:id="7" w:author="B" w:name="move1"/>"#),
            (from, RunContent::Text("X".into())),
            raw(r#"<w:moveFromRangeEnd w:id="7"/>"#),
            (RunProps::default(), RunContent::Text(" Y".into())),
        ],
    );
    let p2 = para::with_atoms(
        &Paragraph::default(),
        vec![
            (RunProps::default(), RunContent::Text("Z ".into())),
            raw(r#"<w:moveToRangeStart w:id="9" w:author="B" w:name="move1"/>"#),
            (to, RunContent::Text("X".into())),
            raw(r#"<w:moveToRangeEnd w:id="9"/>"#),
        ],
    );
    let d = Document {
        sections: vec![Section::new(vec![
            Block::Paragraph(p1),
            Block::Paragraph(p2),
        ])],
        ..Document::default()
    };
    assert_eq!(revisions::max_id(&d), 10);
    let mut s = Session::new(d.clone());
    // Rejecting the moveTo half (id 10) rejects the moveFrom half (id 8) too.
    assert_eq!(s.resolve_revisions(Some(&[10]), false).unwrap(), 2);
    assert_eq!(texts(&s), ["X Y", "Z "]);
    assert_eq!(markers(s.doc()), 0);
    let mut s = Session::new(d);
    s.resolve_revisions(Some(&[8]), true).unwrap();
    assert_eq!(texts(&s), [" Y", "Z X"]);
}

#[test]
fn formatting_revisions_name_the_changed_properties() {
    let mut s = tracked(&["Mot hai"]);
    s.set_selection(Selection {
        anchor: Pos::new(0, 0),
        focus: Pos::new(0, 3),
    });
    s.format_text(&CharFormat::Bold).unwrap();
    s.format_text(&CharFormat::Italic).unwrap();
    let r = s.revisions();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].kind, RevKind::Format);
    // The second change keeps the first record (the properties before any change).
    assert_eq!(r[0].detail, ["bold", "italic"]);
    s.set_selection(Selection::caret(Pos::new(0, 0)));
    s.format_paragraphs(&ParaFormat::Align {
        align: Alignment::Center,
    })
    .unwrap();
    let p = s
        .revisions()
        .into_iter()
        .find(|r| r.kind == RevKind::ParagraphFormat)
        .unwrap();
    assert_eq!(p.detail, ["align"]);
    // Rejecting everything restores the plain paragraph.
    s.resolve_revisions(None, false).unwrap();
    assert_eq!(s.doc().sections, doc(&["Mot hai"]).sections);
}

#[test]
fn locked_tracking_cannot_be_turned_off_or_resolved() {
    let mut d = doc(&["A"]);
    d.protection = Some(DocProtection {
        edit: PROTECT_TRACKED.into(),
        enforced: true,
        ..DocProtection::default()
    });
    let mut s = Session::new(d);
    // Opening a document locked to tracked changes starts tracking.
    assert!(s.tracking_locked() && s.track_changes());
    s.set_track_changes(false);
    assert!(s.track_changes());
    s.set_selection(Selection::caret(Pos::new(0, 1)));
    s.insert_text("B").unwrap();
    assert_eq!(s.revisions().len(), 1);
    assert!(s.resolve_revisions(None, true).is_err());
    assert_eq!(s.revisions().len(), 1);
    // Not enforced: the lock is off.
    let mut p = s.protection().cloned().unwrap();
    p.enforced = false;
    s.set_protection(Some(p));
    assert!(!s.tracking_locked());
    assert_eq!(s.resolve_revisions(None, true).unwrap(), 1);
    // trackRevisions (§17.15.1.89) is saved from the session's mode.
    assert!(s.doc_with_track_setting().track_revisions);
    s.set_track_changes(false);
    assert!(!s.doc_with_track_setting().track_revisions);
}

/// Cut and paste the cut clipboard while tracking: one move (moveFrom at the old place,
/// moveTo at the new one, tied by named ranges) instead of a deletion and an insertion;
/// accepting one half resolves both, undo takes the paste back in one step.
#[test]
fn cut_and_paste_while_tracking_is_a_move() {
    let mut s = tracked(&["một hai ba", "bốn"]);
    // Cut "hai " (bytes 7..11), paste at the start of "bốn".
    s.set_selection(Selection {
        anchor: Pos::new(0, "một ".len()),
        focus: Pos::new(0, "một hai ".len()),
    });
    let clip = s.cut().unwrap();
    s.set_selection(Selection::caret(Pos::new(1, 0)));
    let mv = s.move_paste_start(Some(&clip.internal));
    assert!(mv.is_some());
    s.paste_internal(&clip.internal).unwrap();
    assert!(s.move_paste_finish(mv.unwrap()).unwrap());
    let kinds: Vec<RevKind> = s.revisions().iter().map(|r| r.kind).collect();
    assert_eq!(kinds, [RevKind::MoveFrom, RevKind::MoveTo]);
    assert_eq!(markers(s.doc()), 4);
    assert_eq!(texts(&s), ["một hai ba", "hai bốn"]);
    // One step undoes the paste (the cut stays a deletion).
    s.undo();
    let kinds: Vec<RevKind> = s.revisions().iter().map(|r| r.kind).collect();
    assert_eq!(kinds, [RevKind::Delete]);
    s.redo();
    // Accepting the moveTo half accepts the move.
    let to = s.revisions()[1].id;
    assert_eq!(s.resolve_revisions(Some(&[to]), true).unwrap(), 2);
    assert_eq!(texts(&s), ["một ba", "hai bốn"]);
    assert_eq!(markers(s.doc()), 0);
}

/// Another clipboard, no tracking, or a cut of one's own insertion: no move.
#[test]
fn paste_of_other_text_or_untracked_cut_is_not_a_move() {
    let mut s = tracked(&["một hai", "ba"]);
    s.set_selection(Selection {
        anchor: Pos::new(0, 0),
        focus: Pos::new(0, "một".len()),
    });
    let clip = s.cut().unwrap();
    assert!(s.move_paste_start(Some("[]")).is_none());
    assert!(s.move_paste_start(None).is_none());
    // Without tracking nothing is recorded.
    let mut u = Session::new(doc(&["một hai", "ba"]));
    u.set_selection(Selection {
        anchor: Pos::new(0, 0),
        focus: Pos::new(0, "một".len()),
    });
    let c2 = u.cut().unwrap();
    assert!(u.move_paste_start(Some(&c2.internal)).is_none());
    // Text typed while tracking and cut again is simply removed: no move either.
    s.set_selection(Selection::caret(Pos::new(1, 0)));
    s.insert_text("xyz").unwrap();
    s.set_selection(Selection {
        anchor: Pos::new(1, 0),
        focus: Pos::new(1, 3),
    });
    let c3 = s.cut().unwrap();
    assert!(s.move_paste_start(Some(&c3.internal)).is_none());
    // The first cut's clipboard was replaced by the last cut.
    assert!(s.move_paste_start(Some(&clip.internal)).is_none());
}

/// A cut over a paragraph mark: the moved paragraph mark is part of the moveFrom half.
#[test]
fn cut_and_paste_of_paragraphs_is_a_move() {
    let mut s = tracked(&["A", "B", "C"]);
    s.set_selection(Selection {
        anchor: Pos::new(0, 0),
        focus: Pos::new(1, 0),
    });
    let clip = s.cut().unwrap();
    s.set_selection(Selection::caret(Pos::new(2, 1)));
    let mv = s.move_paste_start(Some(&clip.internal)).unwrap();
    s.paste_internal(&clip.internal).unwrap();
    assert!(s.move_paste_finish(mv).unwrap());
    let kinds: Vec<RevKind> = s.revisions().iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&RevKind::MoveFrom) && kinds.contains(&RevKind::MoveTo));
    assert!(!kinds.contains(&RevKind::Delete) && !kinds.contains(&RevKind::Insert));
    // Rejecting the move restores the original text.
    let any = s.revisions()[0].id;
    s.resolve_revisions(Some(&[any]), false).unwrap();
    assert_eq!(texts(&s), ["A", "B", "C"]);
    assert_eq!(markers(s.doc()), 0);
}

/// Drag and drop of text: moved (one undo step, the dropped text selected), copied
/// with Ctrl; while tracking the move is one moveFrom / moveTo pair.
#[test]
fn drag_and_drop_of_text_moves_it() {
    let sel = |s: &mut Session, p: usize, a: usize, b: usize| {
        s.set_selection(Selection {
            anchor: Pos::new(p, a),
            focus: Pos::new(p, b),
        });
    };
    // Untracked: "một " dropped at the end of the paragraph, after the selection.
    let mut s = Session::new(doc(&["một hai", "ba"]));
    let before = s.doc().clone();
    sel(&mut s, 0, 0, "một ".len());
    let r = s
        .move_selection_to(Pos::new(0, "một hai".len()), false)
        .unwrap();
    assert_eq!(texts(&s), ["haimột ", "ba"]);
    assert_eq!(r.ordered(), (Pos::new(0, 3), Pos::new(0, "haimột ".len())));
    assert!(s.undo());
    assert_eq!(s.doc(), &before);
    // Before the selection, into another paragraph.
    sel(&mut s, 1, 0, 2);
    s.move_selection_to(Pos::new(0, 0), false).unwrap();
    assert_eq!(texts(&s), ["bamột hai", ""]);
    // Inside the selection: nothing happens.
    sel(&mut s, 0, 0, 4);
    let n = s.doc().clone();
    s.move_selection_to(Pos::new(0, 1), false).unwrap();
    assert_eq!(s.doc(), &n);
    // A copy keeps the original.
    sel(&mut s, 0, 0, 2);
    s.move_selection_to(Pos::new(1, 0), true).unwrap();
    assert_eq!(texts(&s), ["bamột hai", "ba"]);
    // Tracked: a move pair.
    let mut t = tracked(&["một hai", "ba"]);
    sel(&mut t, 0, 0, "một ".len());
    t.move_selection_to(Pos::new(1, 2), false).unwrap();
    let kinds: Vec<RevKind> = t.revisions().iter().map(|r| r.kind).collect();
    assert_eq!(kinds, [RevKind::MoveFrom, RevKind::MoveTo]);
    assert_eq!(texts(&t), ["một hai", "bamột "]);
    assert_eq!(markers(t.doc()), 4);
    assert!(t.undo());
    assert!(t.revisions().is_empty());
}
