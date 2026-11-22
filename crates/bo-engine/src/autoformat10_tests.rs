//! AutoFormat As You Type of round 10 against the reference's results (posted keys, COM).
use crate::edit::Command;
use crate::Engine;
use bo_edit::{AutoCorrectOptions, CharFormat};

fn engine(opts: AutoCorrectOptions) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").expect("new");
    e.command(Command::AutoCorrect {
        options: Some(Box::new(opts)),
        reset: false,
    })
    .expect("autocorrect");
    e
}

fn typed(e: &mut Engine, text: &str) {
    for c in text.chars() {
        e.command(Command::InsertText { text: c.into() })
            .expect("type");
    }
}

fn bold(e: &mut Engine) {
    e.command(Command::FormatText {
        format: CharFormat::Bold,
    })
    .expect("bold");
}

/// Text of body paragraph `i` and a `1` / `0` per character for bold.
fn para(e: &Engine, i: usize) -> (String, String) {
    let s = e.session.as_ref().expect("session");
    let p = bo_model::index::para_by_id(s.doc(), i).expect("para");
    let mut text = String::new();
    let mut b = String::new();
    for r in &p.runs {
        for c in &r.content {
            if let bo_model::RunContent::Text(t) = c {
                text.push_str(t);
                for _ in t.chars() {
                    b.push(if r.props.bold == Some(true) { '1' } else { '0' });
                }
            }
        }
    }
    (text, b)
}

fn style(e: &Engine, i: usize) -> Option<String> {
    let s = e.session.as_ref().expect("session");
    bo_model::index::para_by_id(s.doc(), i).and_then(|p| p.props.style.clone())
}

#[test]
fn a_list_item_starts_like_the_one_before_until_its_punctuation() {
    let mut e = engine(AutoCorrectOptions::default());
    typed(&mut e, "* ");
    bold(&mut e);
    typed(&mut e, "Term");
    bold(&mut e);
    typed(&mut e, ": rest of item");
    e.command(Command::SplitParagraph).expect("enter");
    typed(&mut e, "Next: more");
    e.command(Command::SplitParagraph).expect("enter");
    typed(&mut e, "Plain");
    assert_eq!(para(&e, 0).1, format!("1111{}", "0".repeat(14)));
    assert_eq!(para(&e, 1), ("Next: more".into(), "1111000000".into()));
    // "Next" had the lead's punctuation: its item starts bold again, all of "Plain".
    assert_eq!(para(&e, 2), ("Plain".into(), "11111".into()));
    // Switched off: the new item starts plain.
    let mut e = engine(AutoCorrectOptions {
        format_list_beginning: false,
        ..AutoCorrectOptions::default()
    });
    typed(&mut e, "* ");
    bold(&mut e);
    typed(&mut e, "Term");
    bold(&mut e);
    typed(&mut e, ": x");
    e.command(Command::SplitParagraph).expect("enter");
    typed(&mut e, "Next");
    assert_eq!(para(&e, 1).1, "0000");
}

#[test]
fn two_enters_after_a_short_line_make_heading_1_when_switched_on() {
    let on = AutoCorrectOptions {
        builtin_headings: true,
        ..AutoCorrectOptions::default()
    };
    let mut e = engine(on.clone());
    typed(&mut e, "Introduction");
    e.command(Command::SplitParagraph).expect("enter");
    e.command(Command::SplitParagraph).expect("enter");
    typed(&mut e, "Body text.");
    e.command(Command::SplitParagraph).expect("enter");
    typed(&mut e, "Ends with a period.");
    e.command(Command::SplitParagraph).expect("enter");
    e.command(Command::SplitParagraph).expect("enter");
    assert_eq!(para(&e, 0).0, "Introduction");
    assert_eq!(style(&e, 0).as_deref(), Some("Heading1"));
    // The second Enter adds no paragraph.
    assert_eq!(para(&e, 1).0, "Body text.");
    assert_ne!(style(&e, 1).as_deref(), Some("Heading1"));
    assert_ne!(style(&e, 2).as_deref(), Some("Heading1"));
    // Off (the reference's default): nothing.
    let mut e = engine(AutoCorrectOptions::default());
    typed(&mut e, "Introduction");
    e.command(Command::SplitParagraph).expect("enter");
    e.command(Command::SplitParagraph).expect("enter");
    assert_ne!(style(&e, 0).as_deref(), Some("Heading1"));
    assert_eq!(para(&e, 2).0, "");
}

#[test]
fn heading_lines_follow_the_measured_rules() {
    use super::heading_like;
    assert!(heading_like("Introduction"));
    assert!(heading_like("Short line"));
    assert!(heading_like("One two three four five"));
    assert!(!heading_like("One two three four five six"));
    assert!(!heading_like("Ends with a period."));
    assert!(!heading_like("x"));
    assert!(!heading_like("\tSecond level"));
}

#[test]
fn a_small_first_letter_in_a_table_cell_is_not_a_grammar_issue() {
    // The reference leaves "cell one" in a table cell alone; in the body it is still flagged.
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").expect("new");
    e.command(Command::SetGrammar { on: true })
        .expect("grammar");
    e.command(Command::InsertText {
        text: "first line. lower start here".into(),
    })
    .expect("type");
    let body = e
        .spelling_marks(None)
        .into_iter()
        .filter(|m| m.grammar)
        .count();
    assert!(body > 0);
    e.command(Command::SelectAll).expect("all");
    e.command(Command::DeleteForward).expect("clear");
    e.command(Command::InsertTable { rows: 1, cols: 2 })
        .expect("table");
    e.command(Command::InsertText {
        text: "cell one".into(),
    })
    .expect("type");
    let cell = e
        .spelling_marks(None)
        .into_iter()
        .filter(|m| m.grammar)
        .count();
    assert_eq!(cell, 0);
}
