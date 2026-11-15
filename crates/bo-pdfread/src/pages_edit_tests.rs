//! Duplicate, swap, replace, interleave, resize, flatten and page labels: edit, save, reopen.
use crate::tests::build_pdf;
use crate::{standard_fonts, LabelStyle, PdfDocument, RenderOptions, TextLayer};

/// `n` pages of 200 × 100 pt, page `i` showing "<tag> i".
fn pages_pdf(n: usize, tag: &str) -> Vec<u8> {
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
    let mut objs = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {n} /MediaBox [0 0 200 100] \
             /Resources << /Font << /F1 3 0 R >> >> >>",
            kids.join(" ")
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    for i in 0..n {
        objs.push(format!(
            "<< /Type /Page /Parent 2 0 R /Contents {} 0 R >>",
            5 + 2 * i
        ));
        let body = format!("BT /F1 12 Tf 20 50 Td ({tag} {}) Tj ET", i + 1);
        objs.push(format!(
            "<< /Length {} >>\nstream\n{body}\nendstream",
            body.len()
        ));
    }
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    build_pdf(&refs)
}

fn open(bytes: Vec<u8>) -> PdfDocument {
    PdfDocument::open(bytes, standard_fonts()).expect("opens")
}

fn texts(doc: &mut PdfDocument) -> Vec<String> {
    (0..doc.page_count())
        .map(|i| {
            let p = doc.render(i, RenderOptions::default()).expect("renders");
            TextLayer::new(p.chars).text().trim().to_owned()
        })
        .collect()
}

fn reopen(doc: &mut PdfDocument) -> PdfDocument {
    open(doc.save().expect("saves"))
}

#[test]
fn duplicate_swap_and_undo() {
    let mut doc = open(pages_pdf(3, "P"));
    let copies = doc.duplicate_pages(&[0, 2]).expect("duplicates");
    assert_eq!(copies, [1, 4]);
    assert_eq!(texts(&mut doc), ["P 1", "P 1", "P 2", "P 3", "P 3"]);
    doc.swap_pages(0, 2).expect("swaps");
    assert_eq!(texts(&mut doc), ["P 2", "P 1", "P 1", "P 3", "P 3"]);
    let mut again = reopen(&mut doc);
    assert_eq!(texts(&mut again), ["P 2", "P 1", "P 1", "P 3", "P 3"]);
    assert!(doc.undo() && doc.undo());
    assert_eq!(texts(&mut doc), ["P 1", "P 2", "P 3"]);
}

#[test]
fn replace_and_interleave_are_one_step() {
    let mut doc = open(pages_pdf(3, "P"));
    let n = doc
        .replace_pages(&[1, 2], &pages_pdf(2, "Q"), Some(&[1]))
        .expect("replaces");
    assert_eq!(n, 1);
    assert_eq!(texts(&mut doc), ["P 1", "Q 2"]);
    assert!(doc.undo());
    assert_eq!(texts(&mut doc), ["P 1", "P 2", "P 3"]);
    doc.interleave_pdf(&pages_pdf(2, "Q"), false)
        .expect("interleaves");
    assert_eq!(texts(&mut doc), ["P 1", "Q 1", "P 2", "Q 2", "P 3"]);
    assert!(doc.undo());
    doc.interleave_pdf(&pages_pdf(2, "Q"), true)
        .expect("interleaves");
    assert_eq!(texts(&mut doc), ["P 1", "Q 2", "P 2", "Q 1", "P 3"]);
    let mut again = reopen(&mut doc);
    assert_eq!(texts(&mut again).len(), 5);
}

#[test]
fn resize_scales_content_and_size() {
    let mut doc = open(pages_pdf(2, "P"));
    doc.resize_pages(&[0], 400.0, 400.0, true).expect("resizes");
    let p = doc.page_info(0).expect("page");
    assert_eq!((p.width, p.height), (400.0, 400.0));
    assert_eq!(doc.page_info(1).map(|p| p.width), Some(200.0));
    // Content scaled ×2 and centred vertically: the text moved from x 20 to x 40.
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    let x = page.chars.first().map(|c| c.rect[0]).expect("text");
    assert!((x - 40.0).abs() < 1.0, "text at {x}");
    let mut again = reopen(&mut doc);
    assert_eq!(again.page_info(0).map(|p| p.width), Some(400.0));
    assert_eq!(texts(&mut again)[0], "P 1");
}

#[test]
fn page_labels_round_trip() {
    let mut doc = open(pages_pdf(5, "P"));
    assert_eq!(doc.page_label_ranges(), []);
    doc.set_page_labels(0, 1, LabelStyle::RomanLower, "", 1)
        .expect("labels");
    let labels: Vec<String> = (0..5)
        .map(|i| doc.page_info(i).expect("page").label.clone())
        .collect();
    assert_eq!(labels, ["i", "ii", "3", "4", "5"]);
    doc.set_page_labels(2, 4, LabelStyle::Decimal, "A-", 1)
        .expect("labels");
    let again = reopen(&mut doc);
    let labels: Vec<String> = (0..5)
        .map(|i| again.page_info(i).expect("page").label.clone())
        .collect();
    assert_eq!(labels, ["i", "ii", "A-1", "A-2", "A-3"]);
    assert_eq!(again.page_label_ranges().len(), 2);
}

#[test]
fn flatten_draws_comments_into_the_page() {
    let mut doc = open(pages_pdf(2, "P"));
    let a = crate::NewAnnot {
        kind: crate::AnnotKind::Square,
        page: 1,
        rects: vec![[10.0, 10.0, 60.0, 40.0]],
        color: [255, 0, 0],
        author: "t".into(),
        contents: String::new(),
    };
    doc.add_annotation(&a).expect("adds");
    assert_eq!(doc.annotations().len(), 1);
    let before = doc
        .render(1, RenderOptions::default())
        .expect("renders")
        .list
        .commands
        .len();
    let n = doc.flatten_pages(&[0, 1]).expect("flattens");
    assert_eq!(n, 1);
    assert_eq!(doc.annotations(), []);
    let mut again = reopen(&mut doc);
    assert_eq!(again.annotations(), []);
    let after = again
        .render(1, RenderOptions::default())
        .expect("renders")
        .list
        .commands
        .len();
    assert!(after >= before, "{after} < {before}");
}
