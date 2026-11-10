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

const NS_M: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";
const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// Start of the first equation of paragraph 0.
fn eq_at(e: &Engine) -> bo_edit::Pos {
    let text = e.session.as_ref().unwrap().para_text(0).unwrap();
    bo_edit::Pos::new(0, text.find('\u{FFFC}').unwrap())
}

fn eq_xml(e: &Engine, at: bo_edit::Pos) -> String {
    e.session.as_ref().unwrap().equation_at(at).unwrap()
}

/// "Ta có " and the equation `xml` (an `m:oMath` / `m:oMathPara`), as a file holds it: saved and
/// opened again. The caret is at the end.
fn open_with(xml: &str) -> (Engine, bo_edit::Pos) {
    let mut e = engine("a");
    let at = eq_at(&e);
    e.session
        .as_mut()
        .unwrap()
        .replace_equation(at, xml, false)
        .unwrap();
    let bytes = e.save_docx().unwrap();
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "t.docx").unwrap();
    back.command(Command::Move {
        motion: Motion::DocEnd,
        extend: false,
    })
    .unwrap();
    let at = eq_at(&back);
    (back, at)
}

fn refused(r: &EditResult) -> bool {
    r.error.as_deref() == Some(super::MATH_UNSUPPORTED) && r.math.is_none()
}

// source: ECMA-376 Part 1, §22.1.2 (OMML the in-place model still does not keep: several oMath in
// one oMathPara, a tracked change inside the zone, argument properties argPr, a run with other
// content than m:t, a hidden radical degree with content).
#[test]
fn unsupported_markup_is_not_edited_in_place() {
    let m =
        |inner: &str| format!(r#"<m:oMath xmlns:m="{NS_M}" xmlns:w="{NS_W}">{inner}</m:oMath>"#);
    let r = |t: &str| format!("<m:r><m:t>{t}</m:t></m:r>");
    let cases = [
        format!(
            r#"<m:oMathPara xmlns:m="{NS_M}"><m:oMath>{}</m:oMath><m:oMath>{}</m:oMath></m:oMathPara>"#,
            r("a"),
            r("b")
        ),
        m(&format!(
            r#"{}<w:ins w:id="7" w:author="An" w:date="2026-11-11T00:00:00Z">{}</w:ins>"#,
            r("a"),
            r("b")
        )),
        m(&format!(
            r#"<m:f><m:num><m:argPr><m:argSz m:val="-1"/></m:argPr>{}</m:num><m:den>{}</m:den></m:f>"#,
            r("1"),
            r("2")
        )),
        m("<m:r><m:t>x</m:t><w:br/><m:t>y</m:t></m:r>"),
        m(&format!(
            r#"<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg>{}</m:deg><m:e>{}</m:e></m:rad>"#,
            r("3"),
            r("x")
        )),
    ];
    for xml in &cases {
        let (mut e, at) = open_with(xml);
        let before = eq_xml(&e, at);
        assert!(!super::lossless(&before), "{before}");
        let range = bo_edit::Selection {
            anchor: at,
            focus: bo_edit::Pos::new(at.para, at.offset + 3),
        };
        // mathEnter: refused, the equation selected (the dialog edits it).
        let r = e.command(Command::MathEnter { end: true }).unwrap();
        assert!(refused(&r), "{xml}: {:?}", r.error);
        assert_eq!(r.selection, range);
        assert!(r.equation.is_some(), "{xml}");
        // Typing right after does not replace or rewrite it.
        let r = e.command(Command::InsertText { text: "q".into() }).unwrap();
        assert!(refused(&r), "{xml}");
        assert_eq!(eq_xml(&e, at), before, "{xml}");
        // A click on it: refused the same way, typing changes nothing.
        let (page, ox, base) = crate::nav::object_origin(e.layout.document(), at).unwrap();
        let r = e
            .command(Command::Click {
                page,
                x: ox + 1.0,
                y: base - 1.0,
                extend: false,
                count: 1,
            })
            .unwrap();
        assert!(refused(&r), "{xml}: {:?} {:?}", r.error, r.selection);
        assert_eq!(r.selection, range);
        for c in ["q", " ", "^"] {
            e.command(Command::InsertText { text: c.into() }).unwrap();
            e.command(Command::MathInsert { text: c.into() }).unwrap();
        }
        assert_eq!(eq_xml(&e, at), before, "{xml}");
        // Arrows pass over it as before in-place editing.
        e.command(Command::Move {
            motion: Motion::DocEnd,
            extend: false,
        })
        .unwrap();
        let r = run(
            &mut e,
            Command::Move {
                motion: Motion::Left,
                extend: false,
            },
        );
        assert!(r.math.is_none(), "{xml}");
        assert_eq!(r.selection.focus, at);
        let r = run(
            &mut e,
            Command::Move {
                motion: Motion::Right,
                extend: false,
            },
        );
        assert!(r.math.is_none(), "{xml}");
        assert_eq!(eq_xml(&e, at), before, "{xml}");
        // Saved unchanged.
        assert_eq!(saved_omml(&e), before, "{xml}");
    }
}

/// The math font on a run as other applications write it (ECMA-376 Part 1, §17.3.2.26 rFonts).
const FONT: &str =
    r#"<w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/><w:i/></w:rPr>"#;

// source: ECMA-376 Part 1, §22.1.2 (box, groupChr, sPre, borderBox, phant; m:rPr scr, w:rPr in m:r,
// ctrlPr; oMathParaPr) — kept by the tree since ADR-0015 addendum C.
#[test]
fn foreign_style_equations_are_edited_in_place_and_keep_their_properties() {
    let m =
        |inner: &str| format!(r#"<m:oMath xmlns:m="{NS_M}" xmlns:w="{NS_W}">{inner}</m:oMath>"#);
    let r = |t: &str| format!("<m:r>{FONT}<m:t>{t}</m:t></m:r>");
    let ctrl = format!("<m:ctrlPr>{FONT}</m:ctrlPr>");
    let cases = [
        m(&format!(
            "<m:box><m:boxPr>{ctrl}</m:boxPr><m:e>{}</m:e></m:box>",
            r("x")
        )),
        m(&format!(
            r#"<m:groupChr><m:groupChrPr><m:chr m:val="⏟"/>{ctrl}</m:groupChrPr><m:e>{}</m:e></m:groupChr>"#,
            r("ab")
        )),
        m(&format!(
            "<m:sPre><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:sPre>",
            r("1"),
            r("2"),
            r("X")
        )),
        m(&format!("<m:borderBox><m:e>{}</m:e></m:borderBox>", r("y"))),
        m(&format!("<m:phant><m:e>{}</m:e></m:phant>", r("z"))),
        m(&r("x")),
        m(r#"<m:r><m:rPr><m:scr m:val="script"/></m:rPr><m:t>L</m:t></m:r>"#),
        m(&format!(
            r#"<m:f><m:fPr><m:type m:val="skw"/>{ctrl}</m:fPr><m:num>{}</m:num><m:den>{}</m:den></m:f>"#,
            r("1"),
            r("2")
        )),
        m(&format!(
            r#"<m:nary><m:naryPr><m:chr m:val="∑"/><m:grow m:val="1"/>{ctrl}</m:naryPr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:nary>"#,
            r("k"),
            r("n"),
            r("k")
        )),
        format!(
            r#"<m:oMathPara xmlns:m="{NS_M}" xmlns:w="{NS_W}"><m:oMathParaPr><m:jc m:val="left"/></m:oMathParaPr><m:oMath>{}</m:oMath></m:oMathPara>"#,
            r("q")
        ),
    ];
    for xml in &cases {
        let (mut e, at) = open_with(xml);
        let before = eq_xml(&e, at);
        assert!(super::lossless(&before), "{before}");
        let r = run(&mut e, Command::MathEnter { end: true });
        assert!(r.math.is_some(), "{xml}");
        typed(&mut e, "c");
        // What the tree model gives for the same edit (independently: read, insert, write).
        let mut nodes = bo_math::from_omml(&before).unwrap();
        let end = bo_math::edit::zone_end(&nodes, true);
        bo_math::edit::insert_text(&mut nodes, &end, "c").unwrap();
        let want = super::Zone::of(&before).xml(&nodes);
        let saved = saved_omml(&e);
        assert!(bo_math::read::same_omml(&saved, &want), "{saved}\n{want}");
        // Every property element of the original is still there.
        for keep in [
            "<m:ctrlPr>",
            r#"<m:scr m:val="script"/>"#,
            r#"<m:jc m:val="left"/>"#,
            r#"<m:type m:val="skw"/>"#,
            r#"<m:grow m:val="1"/>"#,
            r#"<m:chr m:val="⏟"/>"#,
        ] {
            assert_eq!(
                saved.matches(keep).count(),
                before.matches(keep).count(),
                "{keep}: {saved}"
            );
        }
        // The typed character is in a run with the math font when the zone has one.
        if before.contains("w:rFonts") {
            assert!(saved.contains("c</m:t>"), "{saved}");
            let last_run = &saved[saved.rfind("<m:r>").unwrap()..];
            assert!(last_run.contains("w:rFonts"), "{saved}");
        }
    }
}

// source: UnicodeMath build-up (UTN #28) in an equation written with the math font on every run:
// the built structure takes control properties with that font (ECMA-376 Part 1, §22.1.2 ctrlPr),
// its runs the font; the untouched fraction is written back byte for byte.
#[test]
fn autobuild_in_a_foreign_equation_keeps_the_math_font() {
    let r = |t: &str| format!("<m:r>{FONT}<m:t>{t}</m:t></m:r>");
    let frac = format!(
        "<m:f><m:fPr><m:ctrlPr>{FONT}</m:ctrlPr></m:fPr><m:num>{}</m:num><m:den>{}</m:den></m:f>",
        r("a"),
        r("b")
    );
    let xml = format!(
        r#"<m:oMath xmlns:m="{NS_M}" xmlns:w="{NS_W}">{frac}{}</m:oMath>"#,
        r("=")
    );
    let (mut e, _) = open_with(&xml);
    run(&mut e, Command::MathEnter { end: true });
    typed(&mut e, "x^2 ");
    let saved = saved_omml(&e);
    assert!(saved.contains(&frac), "{saved}");
    assert!(
        saved.contains(&format!(
            "<m:sSup><m:sSupPr><m:ctrlPr>{FONT}</m:ctrlPr></m:sSupPr><m:e>{}</m:e><m:sup>{}</m:sup></m:sSup>",
            r("x"),
            r("2")
        )),
        "{saved}"
    );
    assert_eq!(linear(&mut e), "a/b=x^2");
    // Backspace twice removes the built structure; the rest is as it was.
    run(&mut e, Command::DeleteBackward);
    run(&mut e, Command::DeleteBackward);
    let saved = saved_omml(&e);
    assert!(saved.contains(&format!("{frac}{}", r("="))), "{saved}");
    assert!(!saved.contains("sSup"), "{saved}");
}

/// Equations starting a paragraph of a corpus file: (paragraph, OMML).
fn corpus_equations(name: &str) -> (Engine, Vec<(usize, String)>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/doc")
        .join(name);
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(path).unwrap(), name).unwrap();
    let s = e.session.as_ref().unwrap();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(text) = s.para_text(i) {
        if text.starts_with('\u{FFFC}') {
            if let Some(xml) = s.equation_at(bo_edit::Pos::new(i, 0)) {
                out.push((i, xml));
            }
        }
        i += 1;
    }
    (e, out)
}

#[test]
fn corpus_foreign_style_is_edited_in_place_and_unsupported_is_not() {
    let (_, eqs) = corpus_equations("math-06-unsupported.docx");
    assert_eq!(eqs.len(), 4);
    for (_, xml) in &eqs {
        assert!(!super::lossless(xml), "{xml}");
    }
    let (mut e, eqs) = corpus_equations("math-07-foreign-style.docx");
    assert_eq!(eqs.len(), 7);
    for (_, xml) in &eqs {
        assert!(super::lossless(xml), "{xml}");
    }
    // Every one enters and takes typing; the saved file keeps the math font on every run.
    for (para, _) in eqs.iter().rev() {
        let s = e.session.as_mut().unwrap();
        s.set_selection(bo_edit::Selection::caret(bo_edit::Pos::new(*para, 0)));
        let r = run(&mut e, Command::MathEnter { end: true });
        assert!(r.math.is_some(), "paragraph {para}");
        typed(&mut e, "q");
        run(
            &mut e,
            Command::MathMove {
                motion: MathMotion::Exit,
            },
        );
    }
    let document = |bytes: &[u8]| {
        let pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
        String::from_utf8(pkg.part("/word/document.xml").unwrap().data.clone()).unwrap()
    };
    let original = document(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/corpus/doc/math-07-foreign-style.docx"),
        )
        .unwrap(),
    );
    let doc = document(&e.save_docx().unwrap());
    // Every math run (each split at its end) has the math font.
    let runs: Vec<&str> = doc
        .split("<m:r>")
        .skip(1)
        .map(|r| r.split("</m:r>").next().unwrap())
        .collect();
    assert!(runs.len() > 30, "{}", runs.len());
    for r in &runs {
        assert!(r.contains("<w:rFonts "), "{r}");
    }
    assert_eq!(doc.matches("q</m:t>").count(), 7, "{doc}");
    for keep in [
        "<m:ctrlPr>",
        "<m:grow ",
        "<m:mcs>",
        "<m:scr ",
        "<m:jc ",
        "<m:opEmu ",
        "<m:show ",
    ] {
        assert_eq!(
            doc.matches(keep).count(),
            original.matches(keep).count(),
            "{keep}"
        );
    }
}

#[test]
fn supported_equations_still_enter() {
    // Markup that differs only in what carries no information: whitespace, split runs of one
    // style, an empty property element.
    let xml = format!(
        "<m:oMath xmlns:m=\"{NS_M}\">\n  <m:r><m:t>a</m:t></m:r><m:r><m:t>b</m:t></m:r>\n  <m:f><m:fPr></m:fPr><m:num><m:r><m:t>1</m:t></m:r></m:num><m:den><m:r><m:t>2</m:t></m:r></m:den></m:f>\n</m:oMath>"
    );
    let (mut e, _) = open_with(&xml);
    let r = run(&mut e, Command::MathEnter { end: true });
    assert!(r.math.is_some());
    typed(&mut e, "c");
    assert_eq!(linear(&mut e), "ab1/2c");
    // Every template, inline and display, and an edited equation after save + reopen.
    for (_, src) in crate::equation::TEMPLATES {
        for display in [false, true] {
            let mut e = Engine::with_bundled_fonts();
            e.load(crate::plain_text_document("x "), "t.docx").unwrap();
            run(
                &mut e,
                Command::InsertEquation {
                    text: (*src).into(),
                    latex: false,
                    display,
                    replace: false,
                    numbered: None,
                },
            );
            let r = run(&mut e, Command::MathEnter { end: true });
            assert!(r.math.is_some(), "{src} display={display}");
        }
    }
    let mut e = engine("a/b");
    run(&mut e, Command::MathEnter { end: true });
    typed(&mut e, "+x^2 ");
    let bytes = e.save_docx().unwrap();
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "t.docx").unwrap();
    back.command(Command::Move {
        motion: Motion::DocEnd,
        extend: false,
    })
    .unwrap();
    assert!(run(&mut back, Command::MathEnter { end: true })
        .math
        .is_some());
}

/// Revisions of the main text.
fn revisions(e: &Engine) -> Vec<bo_edit::RevisionItem> {
    e.session.as_ref().unwrap().revisions()
}

fn reopen(e: &Engine) -> Engine {
    let bytes = e.save_docx().unwrap();
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "t.docx").unwrap();
    back
}

/// The equations of paragraph 0: (start, OMML, tracked as deleted).
fn equations(e: &Engine) -> Vec<(usize, String, bool)> {
    let s = e.session.as_ref().unwrap();
    let text = s.para_text(0).unwrap();
    text.match_indices('\u{FFFC}')
        .filter_map(|(i, _)| {
            let at = bo_edit::Pos::new(0, i);
            let xml = s.equation_at(at)?;
            Some((i, xml, s.equation_deleted(at)))
        })
        .collect()
}

// source: ECMA-376 Part 1, §17.13.5.14 del / §17.13.5.18 ins around the math zone (CT_RunTrackChange
// holds m:oMath); ADR-0015 addendum: one revision per edit session of an equation.
#[test]
fn tracked_edit_is_one_revision_of_the_whole_equation() {
    let mut e = engine("a/b");
    e.set_clock_ms(1_700_000_000_000);
    run(&mut e, Command::SetAuthor { name: "An".into() });
    run(&mut e, Command::SetTrackChanges { on: true });
    let at = eq_at(&e);
    let original = eq_xml(&e, at);
    run(&mut e, Command::MathEnter { end: true });
    let r = typed(&mut e, "+c");
    // The review state comes with the edit: one change.
    let review = r.review.as_ref().unwrap();
    assert_eq!(review.revisions.len(), 1);
    assert_eq!(review.revisions[0].kind, bo_edit::RevKind::Equation);
    assert_eq!(review.revisions[0].author, "An");
    assert!(review.revisions[0].date.is_some());
    // The original is kept, byte for byte, as a deletion; the edit follows it as an insertion.
    let eqs = equations(&e);
    assert_eq!(eqs.len(), 2, "{eqs:?}");
    assert_eq!(eqs[0].1, original);
    assert!(eqs[0].2 && !eqs[1].2);
    assert_eq!(crate::equation::info(&eqs[1].1).unwrap().linear, "a/b+c");
    // More typing, an autobuild, a structure deleted: the same pending revision.
    typed(&mut e, "+x^2 ");
    run(&mut e, Command::DeleteBackward);
    run(&mut e, Command::DeleteBackward);
    assert_eq!(revisions(&e).len(), 1);
    // A later session on the edited equation still updates that revision.
    mv(&mut e, MathMotion::Exit);
    run(&mut e, Command::MathEnter { end: true });
    typed(&mut e, "d");
    mv(&mut e, MathMotion::Exit);
    let revs = revisions(&e);
    assert_eq!(revs.len(), 1, "{revs:?}");
    let id = revs[0].id;
    let edited = equations(&e)[1].1.clone();
    // "+x^2 " built a superscript the two Backspaces removed (select, delete).
    assert_eq!(crate::equation::info(&edited).unwrap().linear, "a/b+c+d");
    // The deleted original is not edited in place.
    run(
        &mut e,
        Command::SetSelection {
            selection: bo_edit::Selection::caret(at),
        },
    );
    let r = e.command(Command::MathEnter { end: false }).unwrap();
    assert!(refused(&r));
    assert_eq!(equations(&e)[0].1, original);

    // Saved and reopened: the same single revision, both equations unchanged.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fidelity/output/math-tracked-edit.docx"
    );
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
    std::fs::write(path, e.save_docx().unwrap()).unwrap();
    let back = reopen(&e);
    let revs = revisions(&back);
    assert_eq!(revs.len(), 1, "{revs:?}");
    assert_eq!(revs[0].kind, bo_edit::RevKind::Equation);
    let eqs = equations(&back);
    assert_eq!(
        (eqs[0].1.as_str(), eqs[1].1.as_str()),
        (original.as_str(), edited.as_str())
    );

    // Reject: exactly the old equation; accept: the new one.
    let cases = [
        (Box::new(reopen(&e)), false),
        (Box::new(reopen(&e)), true),
        (Box::new(e), false),
    ];
    for (mut x, accept) in cases {
        let id = revisions(&x).first().map_or(id, |r| r.id);
        let cmd = if accept {
            Command::AcceptRevision { id: Some(id) }
        } else {
            Command::RejectRevision { id: Some(id) }
        };
        run(&mut x, cmd);
        assert_eq!(revisions(&x).len(), 0);
        let eqs = equations(&x);
        assert_eq!(eqs.len(), 1);
        let want = if accept { &edited } else { &original };
        assert_eq!(&eqs[0].1, want);
        // After rejecting, the original edits in place again.
        let at = bo_edit::Pos::new(0, eqs[0].0);
        assert!(super::lossless(&eqs[0].1));
        assert!(!x.session.as_ref().unwrap().equation_deleted(at));
    }
}

#[test]
fn untracked_edits_make_no_revision_and_undo_of_a_tracked_edit() {
    let mut e = engine("a");
    run(&mut e, Command::MathEnter { end: true });
    typed(&mut e, "b");
    assert_eq!(revisions(&e).len(), 0);
    assert_eq!(equations(&e).len(), 1);
    mv(&mut e, MathMotion::Exit);
    run(&mut e, Command::SetTrackChanges { on: true });
    let original = equations(&e)[0].1.clone();
    run(&mut e, Command::MathEnter { end: true });
    typed(&mut e, "cd");
    assert_eq!(revisions(&e).len(), 1);
    // The tracked session is one undo step: undo restores the equation without a revision.
    run(&mut e, Command::Undo);
    assert_eq!(revisions(&e).len(), 0);
    let eqs = equations(&e);
    assert_eq!(eqs.len(), 1);
    assert_eq!(eqs[0].1, original);
}

fn linear_of_para0(e: &mut Engine) -> String {
    let s = e.session.as_ref().unwrap();
    let text = s.para_text(0).unwrap();
    let at = bo_edit::Pos::new(0, text.find('\u{FFFC}').unwrap());
    crate::equation::info(&s.equation_at(at).unwrap())
        .unwrap()
        .linear
}
