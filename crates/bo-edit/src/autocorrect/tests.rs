// source: BlueDoc AutoCorrect rule table (CORE-AUTOCORR-001); expected results are
// the black-box behaviour common to word processors, written down by hand.
use super::*;
use crate::{Block, Document, ParaProps, Paragraph, Section};
use bo_model::{Run, RunContent};

/// Apply [`correct`] to `text` where `|` marks the caret right after the typed character
/// (`typed` = [`ENTER`]: Enter at `|`). Returns the corrected text with the caret marked.
fn run_ctx(input: &str, typed: char, ctx: Context, opts: &AutoCorrectOptions) -> String {
    let caret = input.find('|').expect("caret mark");
    let text = input.replace('|', "");
    let Some(c) = correct(&text, caret, typed, &ctx, opts) else {
        return input.to_owned();
    };
    let mut out = text.clone();
    let mut caret = caret as isize;
    let mut edits = c.edits.clone();
    if let Some((_, end)) = c.list {
        edits.push(TextEdit {
            start: 0,
            end,
            text: "[list]".into(),
            superscript: false,
        });
    }
    if let Some((s, e, t)) = &c.link {
        edits.push(TextEdit {
            start: *s,
            end: *e,
            text: format!("<{}>{}</>", t, &text[*s..*e]),
            superscript: false,
        });
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.start));
    for e in edits {
        let new = if e.superscript {
            format!("^{}", e.text)
        } else {
            e.text.clone()
        };
        if e.end as isize <= caret {
            caret += new.len() as isize - (e.end - e.start) as isize;
        }
        out.replace_range(e.start..e.end, &new);
    }
    out.insert(caret as usize, '|');
    out
}

fn run(input: &str, typed: char) -> String {
    run_ctx(
        input,
        typed,
        Context::default(),
        &AutoCorrectOptions::default(),
    )
}

#[test]
fn rule_table() {
    let cases: &[(&str, char, &str)] = &[
        // Replacement list: words, case kept, Vietnamese multi-word entries.
        ("Tôi thấy teh |", ' ', "Tôi thấy the |"),
        ("Teh |", ' ', "The |"),
        ("TEH |", ' ', "THE |"),
        ("x teh,|", ',', "x the,|"),
        ("x (teh)|", ')', "x (the)|"),
        ("Kết quả chuẩn đoán |", ' ', "Kết quả chẩn đoán |"),
        ("Cần bổ xung |", ' ', "Cần bổ sung |"),
        ("Sử lý |", ' ', "Xử lý |"),
        ("Có ateh |", ' ', "Có ateh |"),
        // Symbols apply when their last character is typed.
        ("Bản quyền (c)|", ')', "Bản quyền ©|"),
        ("BlueDoc(tm)|", ')', "BlueDoc™|"),
        ("A -->|", '>', "A →|"),
        ("A <--|", '-', "A ←|"),
        ("A <=>|", '>', "A ⇔|"),
        ("Chờ...|", '.', "Chờ…|"),
        ("vui :)|", ')', "vui ☺|"),
        // Sentence capitals, Vietnamese diacritics at the start of the word.
        ("đây là câu. ở |", ' ', "đây là câu. Ở |"),
        ("ấn |", ' ', "Ấn |"),
        ("Xong rồi! được |", ' ', "Xong rồi! Được |"),
        ("Hỏi? vâng,|", ',', "Hỏi? Vâng,|"),
        ("“xin |", ' ', "“Xin |"),
        ("Câu một. (ghi)|", ')', "Câu một. (Ghi)|"),
        ("xin\n|", ENTER, "Xin\n|"),
        // Exceptions and words that are not plain.
        ("Các loại v.v. và |", ' ', "Các loại v.v. và |"),
        ("Ở TP. hồ |", ' ', "Ở TP. hồ |"),
        ("See e.g. this |", ' ', "See e.g. this |"),
        ("See E.g. this |", ' ', "See E.g. this |"),
        ("Đợi... rồi |", ' ', "Đợi... rồi |"),
        ("Câu. iPhone |", ' ', "Câu. iPhone |"),
        ("www.|", '.', "www.|"),
        ("Có: một |", ' ', "Có: một |"),
        // Two initial capitals.
        ("THe |", ' ', "The |"),
        ("Xem ĐIện |", ' ', "Xem Điện |"),
        ("Các IDs |", ' ', "Các IDs |"),
        ("Hà NỘI |", ' ', "Hà NỘI |"),
        ("TP |", ' ', "TP |"),
        // Smart quotes.
        ("Nói \"|", '"', "Nói “|"),
        ("“Chào\"|", '"', "“Chào”|"),
        ("'|", '\'', "‘|"),
        ("don'|", '\'', "don’|"),
        ("(\"|", '"', "(“|"),
        // Dashes.
        ("Hà Nội - Huế |", ' ', "Hà Nội – Huế |"),
        ("Hà Nội -- Huế |", ' ', "Hà Nội – Huế |"),
        ("Một--hai |", ' ', "Một—hai |"),
        ("x - - y |", ' ', "x - - y |"),
        // Ordinals and fractions.
        ("the 1st |", ' ', "the 1^st |"),
        ("the 22nd,|", ',', "the 22^nd,|"),
        ("the 13th |", ' ', "the 13^th |"),
        ("the 11st |", ' ', "the 11st |"),
        ("Lấy 1/2 |", ' ', "Lấy ½ |"),
        ("Lấy 3/4.|", '.', "Lấy ¾.|"),
        ("Lấy 11/2 |", ' ', "Lấy 11/2 |"),
        // Lists at a paragraph start.
        ("- |", ' ', "[list]|"),
        ("* |", ' ', "[list]|"),
        ("1. |", ' ', "[list]|"),
        ("a) |", ' ', "[list]|"),
        ("- |nội dung", ' ', "[list]|nội dung"),
        ("x - |", ' ', "x - |"),
        ("2. |", ' ', "2. |"),
        // Links.
        (
            "Xem www.bluestar.vn |",
            ' ',
            "Xem <http://www.bluestar.vn>www.bluestar.vn</> |",
        ),
        (
            "(https://a.vn/x). |",
            ' ',
            "(<https://a.vn/x>https://a.vn/x</>). |",
        ),
        (
            "Gửi ai@bluestar.vn\n|",
            ENTER,
            "Gửi <mailto:ai@bluestar.vn>ai@bluestar.vn</>\n|",
        ),
        ("Gửi ai@local |", ' ', "Gửi ai@local |"),
        // Nothing to do.
        ("Xin chào |", ' ', "Xin chào |"),
        ("Xin chào|", 'o', "Xin chào|"),
        (" |", ' ', " |"),
    ];
    for (input, typed, want) in cases {
        // ENTER cases have no typed character in the text: drop the "\n" before "|".
        let (inp, exp) = if *typed == ENTER {
            (input.replace("\n|", "|"), want.replace("\n|", "|"))
        } else {
            ((*input).to_owned(), (*want).to_owned())
        };
        assert_eq!(run(&inp, *typed), exp, "input {input:?} typed {typed:?}");
    }
}

#[test]
fn options_switch_rules_off() {
    let off = AutoCorrectOptions {
        enabled: false,
        ..AutoCorrectOptions::default()
    };
    assert_eq!(run_ctx("teh |", ' ', Context::default(), &off), "teh |");
    let o = AutoCorrectOptions {
        capitalize_sentences: false,
        ..AutoCorrectOptions::default()
    };
    assert_eq!(
        run_ctx("câu. một |", ' ', Context::default(), &o),
        "câu. một |"
    );
    // Table cells have their own switch.
    let cell = Context {
        in_table: true,
        list: false,
    };
    assert_eq!(run_ctx("một |", ' ', cell, &o), "Một |");
    let o = AutoCorrectOptions {
        capitalize_cells: false,
        ..AutoCorrectOptions::default()
    };
    assert_eq!(run_ctx("một |", ' ', cell, &o), "một |");
    assert_eq!(run_ctx("một |", ' ', Context::default(), &o), "Một |");
    // A list item does not start another list.
    let item = Context {
        in_table: false,
        list: true,
    };
    assert_eq!(
        run_ctx("- |", ' ', item, &AutoCorrectOptions::default()),
        "- |"
    );
    // User entries and exceptions.
    let mut o = AutoCorrectOptions::default();
    o.entries.push(Replacement::new("bxd", "Bộ Xây dựng"));
    o.exceptions.push("Ô.".into());
    assert_eq!(
        run_ctx("Theo bxd |", ' ', Context::default(), &o),
        "Theo Bộ Xây dựng |"
    );
    assert_eq!(
        run_ctx("Gặp Ô. ba |", ' ', Context::default(), &o),
        "Gặp Ô. ba |"
    );
    let o = AutoCorrectOptions {
        smart_quotes: false,
        ..AutoCorrectOptions::default()
    };
    assert_eq!(run_ctx("\"|", '"', Context::default(), &o), "\"|");
}

#[test]
fn options_json_defaults() {
    let o: AutoCorrectOptions = serde_json::from_str(r#"{"smartQuotes":false}"#).unwrap();
    assert!(!o.smart_quotes && o.enabled && !o.entries.is_empty());
    let j = serde_json::to_string(&AutoCorrectOptions::default()).unwrap();
    assert!(j.contains("\"capsExceptions\"") && j.contains("v.v."));
}

// ------------------------------------------------------------------ session

fn doc(text: &str) -> Document {
    Document {
        sections: vec![Section::new(vec![Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run::text(text, RunProps::default())],
        ))])],
        ..Default::default()
    }
}

fn type_str(s: &mut Session, o: &AutoCorrectOptions, text: &str) {
    for c in text.chars() {
        s.insert_text(&c.to_string()).unwrap();
        s.autocorrect(o, c).unwrap();
    }
}

fn text0(s: &Session) -> String {
    s.para_text(0).unwrap_or_default()
}

#[test]
fn undo_reverts_only_the_correction() {
    let o = AutoCorrectOptions::default();
    let mut s = Session::new(doc(""));
    type_str(&mut s, &o, "Đây là teh ");
    assert_eq!(text0(&s), "Đây là the ");
    assert_eq!(s.selection().focus, Pos::new(0, "Đây là the ".len()));
    assert!(s.undo());
    assert_eq!(text0(&s), "Đây là teh ");
    assert_eq!(s.selection().focus, Pos::new(0, "Đây là teh ".len()));
    // Typing on does not correct the word again.
    type_str(&mut s, &o, "x");
    assert_eq!(text0(&s), "Đây là teh x");
    // Redo after undo.
    let mut s = Session::new(doc(""));
    type_str(&mut s, &o, "ấn ");
    assert_eq!(text0(&s), "Ấn ");
    assert!(s.undo());
    assert_eq!(text0(&s), "ấn ");
    assert!(s.redo());
    assert_eq!(text0(&s), "Ấn ");
}

#[test]
fn smart_quote_undo_gives_straight_quote() {
    let o = AutoCorrectOptions::default();
    let mut s = Session::new(doc(""));
    type_str(&mut s, &o, "Nói \"chào\" ");
    assert_eq!(text0(&s), "Nói “chào” ");
    let mut s = Session::new(doc("Nói "));
    s.set_selection(Selection::caret(Pos::new(0, 5)));
    type_str(&mut s, &o, "\"");
    assert_eq!(text0(&s), "Nói “");
    assert!(s.undo());
    assert_eq!(text0(&s), "Nói \"");
}

#[test]
fn text_after_the_caret_keeps_its_place() {
    let o = AutoCorrectOptions::default();
    let mut s = Session::new(doc("Một đoạn"));
    s.set_selection(Selection::caret(Pos::new(0, 0)));
    type_str(&mut s, &o, "teh ");
    assert_eq!(text0(&s), "The Một đoạn");
    assert_eq!(s.selection().focus, Pos::new(0, 4));
}

#[test]
fn ordinal_is_raised() {
    let o = AutoCorrectOptions::default();
    let mut s = Session::new(doc(""));
    type_str(&mut s, &o, "Ngày 1st ");
    assert_eq!(text0(&s), "Ngày 1st ");
    let p = &s.doc().sections[0].blocks[0];
    let Block::Paragraph(p) = p else { panic!() };
    let sup: Vec<&Run> = p
        .runs
        .iter()
        .filter(|r| r.props.vert_align == Some(VertAlign::Superscript))
        .collect();
    assert_eq!(sup.len(), 1);
    assert_eq!(sup[0].content, vec![RunContent::Text("st".into())]);
}

#[test]
fn list_marker_starts_a_list() {
    let o = AutoCorrectOptions::default();
    let mut s = Session::new(doc(""));
    type_str(&mut s, &o, "a) ");
    assert_eq!(text0(&s), "");
    let Block::Paragraph(p) = &s.doc().sections[0].blocks[0] else {
        panic!()
    };
    let num = p.props.num.expect("list");
    let level = s.doc().numbering.level(num).expect("level");
    assert_eq!(level.format, NumFormat::LowerLetter);
    assert_eq!(level.text, "%1)");
    assert_eq!(s.selection().focus, Pos::new(0, 0));
    type_str(&mut s, &o, "một ");
    assert_eq!(text0(&s), "Một ");
    // Undo twice: the capital, then the word; once more brings the marker back.
    assert!(s.undo());
    assert_eq!(text0(&s), "một ");
    assert!(s.undo());
    assert!(s.undo());
    assert_eq!(text0(&s), "a) ");
    let Block::Paragraph(p) = &s.doc().sections[0].blocks[0] else {
        panic!()
    };
    assert!(p.props.num.is_none());
}

#[test]
fn url_becomes_a_link() {
    let o = AutoCorrectOptions::default();
    let mut s = Session::new(doc(""));
    type_str(&mut s, &o, "Xem www.bluestar.vn ");
    let Block::Paragraph(p) = &s.doc().sections[0].blocks[0] else {
        panic!()
    };
    let linked: Vec<&Run> = p.runs.iter().filter(|r| r.props.link.is_some()).collect();
    assert_eq!(linked.len(), 1);
    assert_eq!(
        linked[0].props.link.as_deref(),
        Some("http://www.bluestar.vn")
    );
    assert_eq!(
        linked[0].content,
        vec![RunContent::Text("www.bluestar.vn".into())]
    );
    assert!(s.undo());
    let Block::Paragraph(p) = &s.doc().sections[0].blocks[0] else {
        panic!()
    };
    assert!(p.runs.iter().all(|r| r.props.link.is_none()));
}

#[test]
fn enter_corrects_the_previous_paragraph() {
    let o = AutoCorrectOptions::default();
    let mut s = Session::new(doc(""));
    type_str(&mut s, &o, "xin chào teh");
    s.split_paragraph().unwrap();
    s.autocorrect_break(&o).unwrap();
    assert_eq!(s.para_text(0).unwrap(), "Xin chào the");
    assert_eq!(s.selection().focus, Pos::new(1, 0));
    assert!(s.undo());
    assert_eq!(s.para_text(0).unwrap(), "Xin chào teh");
    assert_eq!(s.paragraph_count(), 2);
}

#[test]
fn tracked_correction() {
    let o = AutoCorrectOptions::default();
    let mut s = Session::new(doc("Có teh"));
    s.set_track_changes(true);
    s.set_selection(Selection::caret(Pos::new(0, "Có teh".len())));
    type_str(&mut s, &o, " ");
    // The old word stays as a deletion, the new one is an insertion; the caret
    // follows the typed blank.
    let t = text0(&s);
    assert!(t.contains("teh") && t.contains("the"), "{t}");
    assert_eq!(s.selection().focus.offset, t.len());
    assert!(s.undo());
    assert_eq!(text0(&s), "Có teh ");
}
