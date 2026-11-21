//! Smart cut and paste against the reference's results (COM `Selection.Cut` / `Paste`, round 10).
use crate::edit::Command;
use crate::Engine;
use bo_edit::{Pos, Selection};

fn engine(text: &str) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").expect("new");
    e.command(Command::InsertText {
        text: text.to_owned(),
    })
    .expect("type");
    e
}

fn select(e: &mut Engine, a: usize, b: usize) {
    e.command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(0, a),
            focus: Pos::new(0, b),
        },
    })
    .expect("select");
}

fn text(e: &Engine) -> String {
    e.session
        .as_ref()
        .and_then(|s| s.para_text(0))
        .unwrap_or_default()
}

fn cut(src: &str, at: usize, len: usize) -> String {
    let mut e = engine(src);
    select(&mut e, at, at + len);
    e.command(Command::Cut).expect("cut");
    text(&e)
}

fn paste(src: &str, at: usize, clip: &str) -> String {
    let mut e = engine(&format!("{clip}|{src}"));
    select(&mut e, 0, clip.len());
    let c = e.command(Command::Copy).expect("copy").clip.expect("clip");
    select(&mut e, 0, clip.len() + 1);
    e.command(Command::DeleteForward).expect("delete");
    select(&mut e, at, at);
    e.command(Command::Paste {
        internal: Some(c.internal),
        html: None,
        text: None,
        mode: None,
    })
    .expect("paste");
    text(&e)
}

#[test]
fn cutting_words_leaves_single_spaces() {
    let s = "The quick brown fox jumps.";
    assert_eq!(cut(s, 10, 5), "The quick fox jumps.");
    assert_eq!(cut(s, 10, 6), "The quick fox jumps.");
    assert_eq!(cut(s, 9, 6), "The quick fox jumps.");
    assert_eq!(cut("The quick brown fox.", 16, 3), "The quick brown.");
    assert_eq!(cut("quick brown fox", 0, 5), "brown fox");
    assert_eq!(cut("brown fox", 5, 4), "brown");
    assert_eq!(cut("one two, three", 4, 3), "one, three");
    assert_eq!(cut("a (two) b", 3, 3), "a () b");
    assert_eq!(cut("one two three", 4, 4), "one three");
    assert_eq!(cut("one  two three", 5, 3), "one  three");
    // Not whole words: nothing extra.
    assert_eq!(cut("brown fox", 1, 3), "bn fox");
}

#[test]
fn pasting_words_adds_the_spaces_they_need() {
    assert_eq!(paste("The quick fox", 9, "brown"), "The quick brown fox");
    assert_eq!(paste("The quick fox", 10, "brown"), "The quick brown fox");
    assert_eq!(paste("The quick fox", 10, "brown "), "The quick brown fox");
    assert_eq!(paste("The quick fox", 9, "brown "), "The quick brown fox");
    assert_eq!(paste("The fox.", 7, "brown"), "The fox brown.");
    assert_eq!(paste("The fox.", 7, "brown "), "The fox brown.");
    assert_eq!(paste("quick", 2, "brown"), "qu brown ick");
    assert_eq!(paste("fox", 0, "brown"), "brown fox");
    assert_eq!(paste("fox", 3, "brown"), "fox brown");
    assert_eq!(paste("The quick fox", 10, " brown"), "The quick brown fox");
    assert_eq!(paste("a (b) c", 3, "x"), "a (x b) c");
    assert_eq!(paste("one, two", 3, "x"), "one x, two");
    assert_eq!(paste("one, two", 4, "x"), "one, x two");
}

#[test]
fn smart_spacing_is_one_undo_step_and_can_be_switched_off() {
    let mut e = engine("The quick brown fox.");
    select(&mut e, 16, 19);
    e.command(Command::Cut).expect("cut");
    assert_eq!(text(&e), "The quick brown.");
    e.command(Command::Undo).expect("undo");
    assert_eq!(text(&e), "The quick brown fox.");
    e.set_smart_cut_paste(false);
    assert!(!e.smart_cut_paste());
    select(&mut e, 16, 19);
    e.command(Command::Cut).expect("cut");
    assert_eq!(text(&e), "The quick brown .");
}

#[test]
fn dragging_a_word_between_two_others() {
    let mut e = engine("one two three four");
    // "two" (no space) dropped after "three" (before the space).
    select(&mut e, 4, 7);
    let r = e
        .command(Command::MoveText {
            to: Pos::new(0, 13),
            copy: false,
        })
        .expect("move");
    assert_eq!(text(&e), "one three two four");
    assert_eq!(r.selection.ordered(), (Pos::new(0, 10), Pos::new(0, 13)));
    e.command(Command::Undo).expect("undo");
    assert_eq!(text(&e), "one two three four");
}
