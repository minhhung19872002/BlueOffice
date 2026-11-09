//! Find / replace with special characters and wildcards (CORE-FIND-001).
//! Expected offsets are byte offsets worked out by hand (Vietnamese letters with
//! diacritics such as "ộ", "ệ", "ố" are 3 bytes in UTF-8, "à" is 2).

use super::*;
use crate::{Pos, Selection, Session};
use bo_model::{Block, ParaProps, Run, RunContent, Section};

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

fn ranges(ms: &[Match]) -> Vec<(usize, usize, usize, Option<usize>)> {
    ms.iter()
        .map(|m| (m.para, m.start, m.end, m.end_para))
        .collect()
}

const PLAIN: FindOptions = FindOptions {
    match_case: false,
    whole_word: false,
    regex: false,
    wildcards: false,
};

const WILD: FindOptions = FindOptions {
    wildcards: true,
    ..PLAIN
};

#[test]
fn special_characters_in_plain_search() {
    let s = Session::new(doc(&["Một\thai", "ba", "Năm 2026 ^ x"]));
    // "Một" = M(1) ộ(3) t(1) = 5 bytes; the tab is byte 5.
    assert_eq!(
        ranges(&s.find("^t", &PLAIN).unwrap()),
        vec![(0, 5, 6, None)]
    );
    // "hai" starts after "Một\t" (6 bytes); the match ends after "ba" in paragraph 1.
    assert_eq!(
        ranges(&s.find("hai^pba", &PLAIN).unwrap()),
        vec![(0, 6, 2, Some(1))]
    );
    // Two paragraph marks in the document.
    assert_eq!(s.find("^p", &PLAIN).unwrap().len(), 2);
    assert_eq!(s.find("^13", &PLAIN).unwrap().len(), 2);
    // Any digit: 2, 0, 2, 6.
    assert_eq!(s.find("^#", &PLAIN).unwrap().len(), 4);
    assert_eq!(s.find("^#^#^#^#", &PLAIN).unwrap().len(), 1);
    // A caret: "Năm 2026 " = N(1) ă(2) m(1) space(1) 2026(4) space(1) = 10 bytes.
    assert_eq!(
        ranges(&s.find("^^", &PLAIN).unwrap()),
        vec![(2, 10, 11, None)]
    );
    // Invalid codes are errors.
    assert!(s.find("^", &PLAIN).is_err());
    assert!(s.find("^x", &PLAIN).is_err());
}

#[test]
fn any_letter_and_any_character_use_char_boundaries() {
    let s = Session::new(doc(&["Việt 1"]));
    // V(0..1) i(1..2) ệ(2..5) t(5..6)
    assert_eq!(
        ranges(&s.find("^$", &PLAIN).unwrap()),
        vec![
            (0, 0, 1, None),
            (0, 1, 2, None),
            (0, 2, 5, None),
            (0, 5, 6, None)
        ]
    );
    // Any character: 4 letters, the space and the digit.
    assert_eq!(s.find("^?", &PLAIN).unwrap().len(), 6);
    assert_eq!(
        ranges(&s.find("i^?t", &PLAIN).unwrap()),
        vec![(0, 1, 6, None)]
    );
}

#[test]
fn line_and_page_breaks() {
    let mut d = doc(&[]);
    d.sections[0].blocks.push(Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![
                RunContent::Text("a".into()),
                RunContent::LineBreak,
                RunContent::Text("b".into()),
                RunContent::PageBreak,
                RunContent::Text("c".into()),
            ],
        }],
    )));
    let mut s = Session::new(d);
    // "a" (1) + U+2028 (3 bytes) + "b" (1) + U+000C (1) + "c".
    assert_eq!(
        ranges(&s.find("^l", &PLAIN).unwrap()),
        vec![(0, 1, 4, None)]
    );
    assert_eq!(
        ranges(&s.find("^m", &PLAIN).unwrap()),
        vec![(0, 5, 6, None)]
    );
    // Replacing the line break by a paragraph mark splits the paragraph.
    assert_eq!(s.replace_all("^l", &PLAIN, "^p").unwrap(), 1);
    assert_eq!(texts(&s), vec!["a", "b\u{000C}c"]);
    // A tab typed as ^t in the replacement becomes a tab.
    assert_eq!(s.replace_all("^m", &PLAIN, "^t").unwrap(), 1);
    assert_eq!(texts(&s), vec!["a", "b\tc"]);
}

#[test]
fn replacing_paragraph_marks_merges_and_undo_restores() {
    let mut s = Session::new(doc(&["Xin chào", "Việt Nam", "!"]));
    let before = s.doc().clone();
    assert_eq!(s.replace_all("^p", &PLAIN, " ").unwrap(), 2);
    assert_eq!(texts(&s), vec!["Xin chào Việt Nam !"]);
    // One undo step restores both paragraph marks.
    assert!(s.undo());
    assert_eq!(texts(&s), vec!["Xin chào", "Việt Nam", "!"]);
    assert_eq!(*s.doc(), before);
    assert!(s.redo());
    assert_eq!(texts(&s), vec!["Xin chào Việt Nam !"]);
}

#[test]
fn replacing_with_paragraph_marks_splits() {
    let mut s = Session::new(doc(&["một, hai, ba"]));
    assert_eq!(s.replace_all(", ", &PLAIN, "^p").unwrap(), 2);
    assert_eq!(texts(&s), vec!["một", "hai", "ba"]);
    assert!(s.undo());
    assert_eq!(texts(&s), vec!["một, hai, ba"]);
    // A match over a paragraph mark replaced by text with paragraph marks.
    let mut s = Session::new(doc(&["a", "b", "c"]));
    assert_eq!(s.replace_all("a^pb", &PLAIN, "x^py^pz").unwrap(), 1);
    assert_eq!(texts(&s), vec!["x", "y", "z", "c"]);
    assert!(s.undo());
    assert_eq!(texts(&s), vec!["a", "b", "c"]);
}

#[test]
fn double_paragraph_marks_collapse() {
    // Story text "a\n\n\nb": one non-overlapping "\n\n" at bytes 1..3.
    let mut s = Session::new(doc(&["a", "", "", "b"]));
    assert_eq!(
        ranges(&s.find("^p^p", &PLAIN).unwrap()),
        vec![(0, 1, 0, Some(2))]
    );
    assert_eq!(s.replace_all("^p^p", &PLAIN, "^p").unwrap(), 1);
    assert_eq!(texts(&s), vec!["a", "", "b"]);
}

#[test]
fn replace_one_match_with_matched_text() {
    let mut s = Session::new(doc(&["Hà Nội", "đẹp"]));
    let m = s.find("nội^p", &PLAIN).unwrap()[0];
    // "Hà " = 4 bytes; the match ends at the start of paragraph 1.
    assert_eq!((m.para, m.start, m.end, m.end_para), (0, 4, 0, Some(1)));
    let sel = s.replace(m, "[^&]").unwrap();
    // "^&" keeps the matched text, paragraph mark included.
    assert_eq!(texts(&s), vec!["Hà [Nội", "]đẹp"]);
    assert_eq!(sel.focus, Pos::new(1, 1));
    // A stale match is refused.
    let stale = Match::new(0, 4, 400);
    assert!(s.replace(stale, "x").is_err());
}

#[test]
fn matches_do_not_cross_table_cells() {
    let mut s = Session::new(doc(&["trước sau"]));
    s.set_selection(Selection::caret(Pos::new(0, "trước".len())));
    s.insert_table(1, 2).unwrap();
    s.set_selection(Selection::caret(Pos::new(1, 0)));
    s.insert_text("x").unwrap();
    s.set_selection(Selection::caret(Pos::new(2, 0)));
    s.insert_text("y").unwrap();
    assert_eq!(texts(&s), vec!["trước", "x", "y", " sau"]);
    assert_eq!(s.find("x^py", &PLAIN).unwrap().len(), 0);
    assert_eq!(s.find("trước^p", &PLAIN).unwrap().len(), 0);
    assert_eq!(s.find("^p", &PLAIN).unwrap().len(), 0);
}

#[test]
fn regex_anchors_stay_per_paragraph() {
    let s = Session::new(doc(&["ab", "bc"]));
    let re = FindOptions {
        regex: true,
        ..PLAIN
    };
    assert_eq!(ranges(&s.find("^b", &re).unwrap()), vec![(1, 0, 1, None)]);
    assert_eq!(ranges(&s.find("b$", &re).unwrap()), vec![(0, 1, 2, None)]);
}

#[test]
fn wildcard_basics() {
    let s = Session::new(doc(&["bat bet bit but"]));
    assert_eq!(s.find("b?t", &WILD).unwrap().len(), 4);
    // `*` is the shortest run: "bat", "bet", "bit", "but".
    assert_eq!(
        ranges(&s.find("b*t", &WILD).unwrap()),
        vec![
            (0, 0, 3, None),
            (0, 4, 7, None),
            (0, 8, 11, None),
            (0, 12, 15, None)
        ]
    );
    assert_eq!(s.find("b[ae]t", &WILD).unwrap().len(), 2);
    assert_eq!(s.find("b[!ae]t", &WILD).unwrap().len(), 2);
    assert_eq!(s.find("b[a-f]t", &WILD).unwrap().len(), 2);
    // Ranges must ascend.
    assert!(s.find("b[f-a]t", &WILD).is_err());

    let s = Session::new(doc(&["abcxcab aab"]));
    // One or more of [a-c]: "abc", "cab", "aab".
    assert_eq!(
        ranges(&s.find("[a-c]@", &WILD).unwrap()),
        vec![(0, 0, 3, None), (0, 4, 7, None), (0, 8, 11, None)]
    );
    // Not "a": x (3), space (7), b (10), plus "b" (1), "c" (2), "c" (4), "b" (6).
    assert_eq!(s.find("[!a]", &WILD).unwrap().len(), 7);
}

#[test]
fn wildcard_repetition_and_words() {
    let s = Session::new(doc(&["1111 foo fo"]));
    // Greedy count: "111" then a single "1" is left.
    assert_eq!(
        ranges(&s.find("1{2,3}", &WILD).unwrap()),
        vec![(0, 0, 3, None)]
    );
    assert_eq!(
        ranges(&s.find("1{2;3}", &WILD).unwrap()),
        vec![(0, 0, 3, None)]
    );
    assert_eq!(s.find("1{2}", &WILD).unwrap().len(), 2);
    assert_eq!(s.find("o{1,}", &WILD).unwrap().len(), 2);
    // Whole words starting with "f": "foo" (5..8) and "fo" (9..11).
    assert_eq!(
        ranges(&s.find("<f*>", &WILD).unwrap()),
        vec![(0, 5, 8, None), (0, 9, 11, None)]
    );
    // "<o" never matches inside "foo".
    assert_eq!(s.find("<o", &WILD).unwrap().len(), 0);
    assert!(s.find("{2}", &WILD).is_err());
    assert!(s.find("@", &WILD).is_err());
}

#[test]
fn wildcards_are_case_sensitive_and_unicode_aware() {
    let s = Session::new(doc(&["Hà hà", "Việt"]));
    // "Hà " = H(1) à(2) space(1) = 4 bytes.
    assert_eq!(ranges(&s.find("hà", &WILD).unwrap()), vec![(0, 4, 7, None)]);
    // `?` is one character: ệ is 3 bytes.
    assert_eq!(
        ranges(&s.find("Vi?t", &WILD).unwrap()),
        vec![(1, 0, 6, None)]
    );
    // `?` and `*` never take a paragraph mark …
    assert_eq!(s.find("hà?Việt", &WILD).unwrap().len(), 0);
    // … but ^p does: "hà" (4..7) to the end of "Vi" in paragraph 1.
    assert_eq!(
        ranges(&s.find("hà^pVi", &WILD).unwrap()),
        vec![(0, 4, 2, Some(1))]
    );
    // Escaped special characters are literal.
    let s = Session::new(doc(&["(x) x?"]));
    assert_eq!(
        ranges(&s.find(r"\(x\)", &WILD).unwrap()),
        vec![(0, 0, 3, None)]
    );
    assert_eq!(
        ranges(&s.find(r"x\?", &WILD).unwrap()),
        vec![(0, 4, 6, None)]
    );
    assert!(s.find("(x", &WILD).is_err());
    assert!(s.find("x)", &WILD).is_err());
}

#[test]
fn wildcard_groups_in_replacement() {
    let mut s = Session::new(doc(&["Hà Nội đẹp", "Bùi Hùng"]));
    // Shortest pairs of words: ("Hà", "Nội") and ("Bùi", "Hùng"); "đẹp" has no
    // following word in its paragraph (`*` never takes a paragraph mark).
    assert_eq!(s.replace_all("(<*>) (<*>)", &WILD, r"\2 \1").unwrap(), 2);
    assert_eq!(texts(&s), vec!["Nội Hà đẹp", "Hùng Bùi"]);
}

#[test]
fn wildcard_groups_with_paragraph_marks() {
    // "1. Một 2. Hai": numbers followed by ". " start new paragraphs.
    let mut s = Session::new(doc(&["1. Một 2. Hai"]));
    assert_eq!(s.replace_all(" ([0-9]@). ", &WILD, r"^p\1) ").unwrap(), 1);
    assert_eq!(texts(&s), vec!["1. Một", "2) Hai"]);
    assert!(s.undo());
    assert_eq!(texts(&s), vec!["1. Một 2. Hai"]);
    // Replace one match: groups are expanded against the match found again in context.
    let mut s = Session::new(doc(&["ab ab"]));
    let ms = s.find("(a)(b)", &WILD).unwrap();
    assert_eq!(ranges(&ms), vec![(0, 0, 2, None), (0, 3, 5, None)]);
    s.replace_match(ms[1], Some(("(a)(b)", &WILD)), r"\2\1")
        .unwrap();
    assert_eq!(texts(&s), vec!["ab ba"]);
}

#[test]
fn tracked_paragraph_mark_replacement_undoes() {
    let mut s = Session::new(doc(&["một", "hai"]));
    let before = s.doc().clone();
    s.set_track_changes(true);
    assert_eq!(s.replace_all("^p", &PLAIN, "-").unwrap(), 1);
    assert_ne!(*s.doc(), before);
    assert!(s.undo());
    assert_eq!(texts(&s), vec!["một", "hai"]);
}

#[test]
fn replacement_expansion() {
    assert_eq!(expand_replacement("a^pb^tc^^d^&", None, false), "a\nb\tc^d");
    assert_eq!(expand_replacement("^13^9^q", None, false), "\n\t^q");
    // Backslashes are literal outside wildcard mode.
    assert_eq!(expand_replacement(r"\1", None, false), r"\1");
    assert_eq!(expand_replacement(r"\1\\", None, true), r"\");
}

#[test]
fn match_json_shape() {
    let one = serde_json::to_string(&Match::new(2, 1, 4)).unwrap();
    assert_eq!(one, r#"{"p":2,"start":1,"end":4}"#);
    let multi: Match = serde_json::from_str(r#"{"p":0,"start":3,"end":1,"ep":1}"#).unwrap();
    assert_eq!(multi.last_para(), 1);
    assert!(multi.is_multi_para());
}
