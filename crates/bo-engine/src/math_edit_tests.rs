//! Equations edited in place (CORE-MATH-010): the editing keys inside a math zone, autobuild, undo /
//! redo and save + reopen (OMML, ECMA-376 Part 1, §22.1).
#![allow(clippy::unwrap_used)]

use super::MathMotion;
use crate::edit::{Command, EditResult};
use crate::nav::Motion;
use crate::Engine;

/// A document "Ta có " followed by the equation `linear` (UnicodeMath), the caret after it.
fn engine(linear: &str) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.load(crate::plain_text_document("Ta có "), "t.docx")
        .unwrap();
    e.command(Command::Move {
        motion: Motion::DocEnd,
        extend: false,
    })
    .unwrap();
    e.command(Command::InsertEquation {
        text: linear.into(),
        latex: false,
        display: false,
        replace: false,
        numbered: None,
    })
    .unwrap();
    e.command(Command::Move {
        motion: Motion::DocEnd,
        extend: false,
    })
    .unwrap();
    e
}

fn run(e: &mut Engine, c: Command) -> EditResult {
    let r = e.command(c).unwrap();
    assert!(r.error.is_none(), "{:?}", r.error);
    r
}

fn mv(e: &mut Engine, motion: MathMotion) -> EditResult {
    run(e, Command::MathMove { motion })
}

fn typed(e: &mut Engine, text: &str) -> EditResult {
    let mut r = None;
    for c in text.chars() {
        r = Some(run(
            e,
            Command::InsertText {
                text: c.to_string(),
            },
        ));
    }
    r.unwrap()
}

fn linear(e: &mut Engine) -> String {
    run(e, Command::Equation).equation.unwrap().linear
}

fn path(r: &EditResult) -> (Vec<[usize; 2]>, usize) {
    let m = r.math.as_ref().unwrap();
    (m.path.clone(), m.offset)
}

/// The equation's OMML as saved and read back.
fn saved_omml(e: &Engine) -> String {
    let bytes = e.save_docx().unwrap();
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "t.docx").unwrap();
    back.command(Command::Move {
        motion: Motion::DocEnd,
        extend: false,
    })
    .unwrap();
    let s = back.session.as_ref().unwrap();
    s.selected_equation().unwrap().1
}

// source: reading order of the slots of §22.1.2 structures; UnicodeMath (UTN #28) for the input.
#[test]
fn arrows_enter_move_through_slots_and_leave() {
    let mut e = engine("a/b");
    // Left from after the equation enters it at its end.
    let r = run(
        &mut e,
        Command::Move {
            motion: Motion::Left,
            extend: false,
        },
    );
    assert_eq!(path(&r), (vec![], 1));
    let r = mv(&mut e, MathMotion::Left);
    assert_eq!(path(&r), (vec![[0, 1]], 1)); // end of the denominator
    let r = run(
        &mut e,
        Command::Move {
            motion: Motion::Left,
            extend: false,
        },
    );
    assert_eq!(path(&r), (vec![[0, 1]], 0));
    let r = mv(&mut e, MathMotion::Left);
    assert_eq!(path(&r), (vec![[0, 0]], 1)); // end of the numerator
                                             // Home / End: the zone ends; Esc leaves the zone with the caret after it.
    let r = run(
        &mut e,
        Command::Move {
            motion: Motion::LineStart,
            extend: false,
        },
    );
    assert_eq!(path(&r), (vec![], 0));
    let caret_in = r.caret.unwrap();
    let r = mv(&mut e, MathMotion::Left);
    assert!(r.math.is_none());
    assert_eq!(r.selection.focus.offset, "Ta có ".len());
    // Right from before the equation enters it at its start.
    let r = run(
        &mut e,
        Command::Move {
            motion: Motion::Right,
            extend: false,
        },
    );
    assert_eq!(path(&r), (vec![], 0));
    assert_eq!(r.caret.unwrap(), caret_in);
    let r = mv(&mut e, MathMotion::Exit);
    assert!(r.math.is_none());
    assert_eq!(r.selection.focus.offset, "Ta có ".len() + 3);
}

#[test]
fn caret_geometry_follows_the_slots() {
    let mut e = engine("a/b");
    run(&mut e, Command::MathEnter { end: false });
    let zone = mv(&mut e, MathMotion::Home).caret.unwrap();
    let num = mv(&mut e, MathMotion::Next).caret.unwrap();
    let den = mv(&mut e, MathMotion::Next).caret.unwrap();
    // Numerator above the denominator, both shorter than the zone, inside it.
    assert!(num.y + num.h <= den.y + 0.01, "{num:?} {den:?}");
    assert!(num.h < zone.h && den.h < zone.h);
    assert!(num.x > zone.x && den.x > zone.x);
    let r = run(&mut e, Command::State);
    let m = r.math.unwrap();
    assert!(m.zone.w > 0.0 && m.zone.x <= zone.x + 0.01);
    assert_eq!(m.placeholders.len(), 0);
}

#[test]
fn clicking_into_the_denominator_and_typing() {
    let mut e = engine("a/b");
    // Find the denominator's caret by keys, then click there from outside the equation.
    run(&mut e, Command::MathEnter { end: false });
    mv(&mut e, MathMotion::Next);
    let den = mv(&mut e, MathMotion::Next).caret.unwrap();
    mv(&mut e, MathMotion::Exit);
    let r = run(
        &mut e,
        Command::Click {
            page: den.page,
            x: den.x + 0.1,
            y: den.y + den.h / 2.0,
            extend: false,
            count: 1,
        },
    );
    assert_eq!(path(&r), (vec![[0, 1]], 0));
    typed(&mut e, "c");
    let want = |src: &str| bo_math::to_linear(&bo_math::unicode_math::parse(src));
    assert_eq!(linear(&mut e), want("a/(cb)"));
    // Tab goes after the fraction; typing there builds x^2 on the space.
    let r = run(&mut e, Command::Tab { back: false });
    assert_eq!(path(&r), (vec![], 1));
    let r = typed(&mut e, "+x^2 ");
    assert_eq!(path(&r), (vec![], 3));
    assert_eq!(linear(&mut e), want("a/(cb)+x^2"));
    let omml = saved_omml(&e);
    assert!(omml.contains("<m:sSup><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sup><m:r><m:t>2</m:t></m:r></m:sup></m:sSup>"), "{omml}");
    // A click elsewhere leaves the equation.
    let r = run(
        &mut e,
        Command::Click {
            page: 0,
            x: 80.0,
            y: den.y,
            extend: false,
            count: 1,
        },
    );
    assert!(r.math.is_none());
}

#[test]
fn backspace_selects_then_deletes_a_structure_and_undo_restores_it() {
    let mut e = engine("y");
    run(&mut e, Command::MathEnter { end: true });
    typed(&mut e, "+x^2 ");
    assert_eq!(linear(&mut e), "y+x^2");
    // First Backspace selects the superscript structure (drawn as a selection), the second deletes it.
    let r = run(&mut e, Command::DeleteBackward);
    let m = r.math.as_ref().unwrap();
    assert!(m.selected);
    assert_eq!(r.rects.len(), 1);
    assert_eq!(linear(&mut e), "y+x^2");
    let r = run(&mut e, Command::DeleteBackward);
    assert!(!r.math.as_ref().unwrap().selected);
    assert_eq!(linear(&mut e), "y+");
    // Undo brings the structure back; the caret stays in the zone.
    let r = run(&mut e, Command::Undo);
    assert!(r.math.is_some());
    assert_eq!(linear(&mut e), "y+x^2");
    // Undo again: the build-up step (back to the typed text), then the typed group.
    run(&mut e, Command::Undo);
    assert_eq!(linear(&mut e), "y+x^2");
    run(&mut e, Command::Redo);
    run(&mut e, Command::Redo);
    assert_eq!(linear(&mut e), "y+");
    // The result is saved as valid OMML that reads back the same.
    assert!(saved_omml(&e).contains("<m:t>y+</m:t>"));
}

#[test]
fn typing_is_one_undo_step_per_group() {
    let mut e = engine("a");
    run(&mut e, Command::MathEnter { end: true });
    let before = run(&mut e, Command::HistoryLen).count.unwrap();
    typed(&mut e, "bcd");
    let after = run(&mut e, Command::HistoryLen).count.unwrap();
    assert_eq!(after, before + 1);
    assert_eq!(linear(&mut e), "abcd");
    // A move ends the group.
    mv(&mut e, MathMotion::Left);
    typed(&mut e, "e");
    assert_eq!(run(&mut e, Command::HistoryLen).count.unwrap(), before + 2);
    assert_eq!(linear(&mut e), "abced");
    run(&mut e, Command::Undo);
    assert_eq!(linear(&mut e), "abcd");
    run(&mut e, Command::Undo);
    assert_eq!(linear(&mut e), "a");
    run(&mut e, Command::Redo);
    assert_eq!(linear(&mut e), "abcd");
}

// source: UnicodeMath control words and build-up (UTN #28).
#[test]
fn autobuild_of_the_common_cases() {
    let cases: &[(&str, &str, &str)] = &[
        ("a/b ", "a/b", "<m:f>"),
        ("\\sqrt x", "√x", "<m:rad>"),
        ("\\alpha ", "α", "<m:t>α</m:t>"),
        ("\\sum k", "∑▒k", "<m:nary>"),
        ("\\int x", "∫▒x", "<m:nary>"),
        ("(a+b) ", "(a+b)", "<m:d>"),
    ];
    for &(input, want, xml) in cases {
        let mut e = engine("z");
        run(&mut e, Command::MathEnter { end: false });
        run(&mut e, Command::MathDelete { back: false });
        typed(&mut e, input);
        assert_eq!(linear(&mut e), want, "{input}");
        assert!(saved_omml(&e).contains(xml), "{input}");
    }
    // mathAutobuild builds without typing a space.
    let mut e = engine("z");
    run(&mut e, Command::MathEnter { end: true });
    typed(&mut e, "^2");
    run(&mut e, Command::MathAutobuild);
    assert_eq!(linear(&mut e), "z^2");
}

#[test]
fn every_structure_kind_takes_typing_in_its_slots() {
    // Each template: Tab into its first slot, type a letter, read back.
    let cases = [
        "x_i^2",
        "√(n&x)",
        "∑_k^n k",
        "(a)",
        "■(a&b@c&d)",
        "█(a@b)",
        "sin x",
        "x̂",
        "x̅",
        "lim_(n→∞) a",
    ];
    for src in cases {
        let mut e = engine(src);
        run(&mut e, Command::MathEnter { end: false });
        let r = run(&mut e, Command::Tab { back: false });
        assert!(!path(&r).0.is_empty(), "{src}: Tab enters a slot");
        let before = linear(&mut e);
        typed(&mut e, "q");
        let after = linear(&mut e);
        assert_ne!(before, after, "{src}");
        assert_eq!(
            after.chars().filter(|c| *c == 'q').count(),
            1,
            "{src}: {after}"
        );
        // Saved OMML is well formed and reads back the same.
        let omml = saved_omml(&e);
        let back = crate::equation::info(&omml).unwrap().linear;
        assert_eq!(back, after, "{src}");
    }
}

#[test]
fn other_commands_leave_the_equation() {
    let mut e = engine("a/b");
    run(&mut e, Command::MathEnter { end: false });
    assert!(run(&mut e, Command::State).math.is_some());
    // Enter leaves the zone and splits the paragraph after the equation.
    let r = run(&mut e, Command::SplitParagraph);
    assert!(r.math.is_none());
    assert_eq!(r.selection.focus.para, 1);
    assert_eq!(linear_of_para0(&mut e), "a/b");
}

fn linear_of_para0(e: &mut Engine) -> String {
    let s = e.session.as_ref().unwrap();
    let text = s.para_text(0).unwrap();
    let at = bo_edit::Pos::new(0, text.find('\u{FFFC}').unwrap());
    crate::equation::info(&s.equation_at(at).unwrap())
        .unwrap()
        .linear
}
