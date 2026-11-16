//! What moves when a paragraph being edited gains lines (`paragraph_push`).
use super::*;
use crate::tests::build_pdf;

/// One 300 × 300 page: a three-line 10 pt paragraph (12 pt leading), a second one 6 pt below
/// it, a rule further down and a last paragraph after a large gap.
fn box_pdf() -> Vec<u8> {
    let content = "BT /F1 10 Tf 12 TL 20 270 Td (The quick brown fox jumps over) Tj \
                   T* (the lazy dog and keeps on running) Tj T* (to the end.) Tj ET \
                   BT /F1 10 Tf 12 TL 20 228 Td (A second paragraph close below) Tj \
                   T* (in the same column.) Tj ET \
                   0 0 0 RG 1 w 20 190 m 200 190 l S \
                   BT /F1 10 Tf 20 120 Td (Far below the others.) Tj ET";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R >> >> >>"
            .to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    build_pdf(&refs)
}

fn open(bytes: Vec<u8>) -> PdfDocument {
    PdfDocument::open(bytes, standard_fonts()).expect("opens")
}

/// Top of the first block whose text starts with `start`, and the rule's top.
fn tops(doc: &mut PdfDocument, start: &str) -> (f32, f32) {
    let objs = doc.page_objects(0).expect("objects");
    let b = text_blocks(&objs)
        .into_iter()
        .find(|b| b.text.starts_with(start))
        .unwrap_or_else(|| panic!("no block {start}"));
    let rule = objs
        .iter()
        .find(|o| o.kind == ObjectKind::Path)
        .expect("rule")
        .bbox[1];
    (b.bbox[1], rule)
}

/// Type enough at the end of the first paragraph for two more lines.
fn grow(doc: &mut PdfDocument, mode: Option<PushMode>) -> BlockEdit {
    let objs = doc.page_objects(0).expect("objects");
    let b = text_blocks(&objs).remove(0);
    assert_eq!(b.lines, 3);
    let mut e = doc.begin_block_edit(0, &b).expect("starts");
    if let Some(m) = mode {
        doc.block_edit_push(&mut e, m).expect("followers");
    }
    let at = e.text.len();
    for c in " and then a good many more words to make it wrap twice over".chars() {
        e.text.push(c);
        e.style_of.push(e.style_of[at - 1]);
        doc.block_edit_update(&mut e).expect("updates");
    }
    assert_eq!(e.lines, 5, "two more lines");
    e
}

#[test]
fn paragraphs_in_the_same_box_move_down_while_typing_others_stay() {
    let mut doc = open(box_pdf());
    let (second0, rule0) = tops(&mut doc, "A second");
    let (far0, _) = tops(&mut doc, "Far below");
    let e = grow(&mut doc, Some(PushMode::Box));
    // While typing: the next paragraph moved two lines down, the rule and the paragraph
    // after the gap did not (the reference editor overlaps them).
    let (second, rule) = tops(&mut doc, "A second");
    let (far, _) = tops(&mut doc, "Far below");
    assert!(
        (second - second0 - 24.0).abs() < 0.5,
        "{second0} -> {second}"
    );
    assert!((rule - rule0).abs() < 0.01);
    assert!((far - far0).abs() < 0.01);
    doc.end_block_edit(&e);
    let (second, _) = tops(&mut doc, "A second");
    assert!((second - second0 - 24.0).abs() < 0.5);
    // One undo step puts everything back.
    assert!(doc.undo());
    let (second, _) = tops(&mut doc, "A second");
    assert!((second - second0).abs() < 0.01);
}

#[test]
fn reflow_editing_moves_everything_below() {
    let mut doc = open(box_pdf());
    let (second0, rule0) = tops(&mut doc, "A second");
    let (far0, _) = tops(&mut doc, "Far below");
    let e = grow(&mut doc, Some(PushMode::Below));
    let (second, rule) = tops(&mut doc, "A second");
    let (far, _) = tops(&mut doc, "Far below");
    assert!((second - second0 - 24.0).abs() < 0.5);
    assert!((rule - rule0 - 24.0).abs() < 0.5, "{rule0} -> {rule}");
    assert!((far - far0 - 24.0).abs() < 0.5);
    doc.end_block_edit(&e);
}

#[test]
fn without_followers_the_paragraph_overlaps_what_is_below() {
    let mut doc = open(box_pdf());
    let (second0, _) = tops(&mut doc, "A second");
    let e = grow(&mut doc, None);
    let objs = doc.page_objects(0).expect("objects");
    let second = objs
        .iter()
        .filter(|o| o.text.starts_with("A second"))
        .map(|o| o.bbox[1])
        .next()
        .expect("second");
    assert!((second - second0).abs() < 0.01);
    doc.end_block_edit(&e);
}

#[test]
fn deleting_lines_pulls_the_box_back_up() {
    let mut doc = open(box_pdf());
    let (second0, _) = tops(&mut doc, "A second");
    let mut e = grow(&mut doc, Some(PushMode::Box));
    // Back to two lines: the follower ends one line above where it started.
    let keep = "The quick brown fox jumps over the lazy".chars().count();
    e.text.truncate(keep);
    e.style_of.truncate(keep);
    doc.block_edit_update(&mut e).expect("updates");
    assert_eq!(e.lines, 2);
    let (second, _) = tops(&mut doc, "A second");
    assert!(
        (second - second0 + 12.0).abs() < 0.5,
        "{second0} -> {second}"
    );
    doc.end_block_edit(&e);
}

/// `BO_PARAS=<file.pdf>`: blocks of page 1 and what follows the first one (diagnostics).
#[test]
fn debug_file() {
    let Ok(p) = std::env::var("BO_PARAS") else {
        return;
    };
    let mut doc = open(std::fs::read(p).expect("file"));
    let objs = doc.page_objects(0).expect("objects");
    for b in text_blocks(&objs) {
        eprintln!("block {:?} lines {} {:?}", b.bbox, b.lines, b.text);
    }
    let b = text_blocks(&objs).remove(0);
    let mut e = doc.begin_block_edit(0, &b).expect("starts");
    doc.block_edit_push(&mut e, PushMode::Box).expect("push");
    eprintln!(
        "followers {}",
        e.push.as_ref().map_or(0, |p| p.followers.len())
    );
    for c in " and then a good many more words to make it wrap twice over and over".chars() {
        e.text.push(c);
        e.style_of.push(0);
        doc.block_edit_update(&mut e).expect("updates");
    }
    eprintln!("lines {}", e.lines);
    let objs = doc.page_objects(0).expect("objects");
    for b in text_blocks(&objs) {
        eprintln!("after {:?} {:?}", b.bbox, b.text);
    }
}
