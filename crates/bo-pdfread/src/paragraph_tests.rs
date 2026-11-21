//! Paragraph blocks and block editing sessions.
use super::*;
use crate::tests::build_pdf;

/// One 300 × 220 page: a 16 pt title, a three-line 10 pt paragraph and a justified one
/// (lines of the same text: equal widths).
fn para_pdf() -> Vec<u8> {
    let content = "BT /F2 16 Tf 20 200 Td (Title) Tj ET \
                   BT /F1 10 Tf 12 TL 20 170 Td (The quick brown fox jumps over) Tj \
                   T* (the lazy dog and keeps on running) Tj T* (to the end.) Tj ET \
                   BT /F3 10 Tf 12 TL 20 100 Td (abcd abcd abcd) Tj T* \
                   (abcd abcd abcd) Tj T* (abcd abcd abcd) Tj T* (end) Tj ET";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 220] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R /F2 6 0 R /F3 7 0 R >> >> >>"
            .to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>".to_owned(),
    ];
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    build_pdf(&refs)
}

fn open(bytes: Vec<u8>) -> PdfDocument {
    PdfDocument::open(bytes, standard_fonts()).expect("opens")
}

fn page_text(doc: &mut PdfDocument) -> String {
    let p = doc.render(0, RenderOptions::default()).expect("renders");
    TextLayer::new(p.chars).text()
}

fn blocks(doc: &mut PdfDocument) -> Vec<TextBlock> {
    let objs = doc.page_objects(0).expect("objects");
    text_blocks(&objs)
}

#[test]
fn lines_group_into_paragraphs() {
    let mut doc = open(para_pdf());
    let b = blocks(&mut doc);
    assert_eq!(
        b.len(),
        3,
        "{:?}",
        b.iter().map(|b| &b.text).collect::<Vec<_>>()
    );
    assert_eq!(b[0].text, "Title");
    assert_eq!(b[0].lines, 1);
    assert_eq!(
        b[1].text,
        "The quick brown fox jumps over the lazy dog and keeps on running to the end."
    );
    assert_eq!(b[1].lines, 3);
    assert_eq!(b[1].align, Align::Left);
    assert_eq!(b[1].char_boxes.len(), b[1].text.chars().count());
    assert_eq!(b[1].runs.len(), b[1].text.chars().count());
    assert!((b[1].font_size - 10.0).abs() < 0.01);
}

#[test]
fn a_block_edit_reflows_and_undoes_as_one_step() {
    let mut doc = open(para_pdf());
    let b = blocks(&mut doc).remove(1);
    let mut e = doc.begin_block_edit(0, &b).expect("starts");
    let at = "The quick".chars().count();
    for (k, c) in " and very clever, very very clever".chars().enumerate() {
        e.text.insert(at + k, c);
        e.style_of.insert(at + k, e.style_of[at]);
        doc.block_edit_update(&mut e).expect("updates");
    }
    let want = "The quick and very clever, very very clever brown fox jumps over the lazy dog and keeps on running to the end.";
    assert_eq!(e.text.iter().collect::<String>(), want);
    assert!(e.lines >= 4, "wrapped into {} lines", e.lines);
    // Every line starts at the paragraph's left edge and stays within its width.
    for (i, r) in e
        .char_boxes
        .iter()
        .enumerate()
        .filter(|(i, _)| e.text[*i] != ' ')
    {
        assert!(
            r[2] <= b.bbox[2] + 1.0,
            "char {i} at {r:?} past {:?}",
            b.bbox
        );
        assert!(r[0] >= b.bbox[0] - 0.5);
    }
    let firsts: Vec<f32> = (0..e.text.len())
        .filter(|&i| i == 0 || e.line_of[i] != e.line_of[i - 1])
        .map(|i| e.char_boxes[i][0])
        .collect();
    assert!(
        firsts.iter().all(|x| (x - firsts[0]).abs() < 0.5),
        "{firsts:?}"
    );
    // Lines keep the pitch.
    let tops: Vec<f32> = (1..e.text.len())
        .filter(|&i| e.line_of[i] != e.line_of[i - 1])
        .map(|i| e.char_boxes[i][3])
        .collect();
    assert!((tops[1] - tops[0] - 12.0).abs() < 0.1, "{tops:?}");
    doc.end_block_edit(&e);
    let text = page_text(&mut doc);
    assert!(text.contains("The quick and very clever"), "{text}");
    assert!(!text.contains("The quick brown"));
    // The page stream is compressed again when editing ends.
    let saved = doc.save().expect("saves");
    let mut again = open(saved);
    let b2 = blocks(&mut again);
    assert!(b2.iter().any(|b| b.text == want), "{b2:?}");
    // One undo step for the whole session.
    assert!(doc.undo());
    let text = page_text(&mut doc);
    assert!(text.contains("The quick brown fox"), "{text}");
    assert!(!doc.can_undo());
}

#[test]
fn justified_paragraph_keeps_both_edges() {
    let mut doc = open(para_pdf());
    let b = blocks(&mut doc).remove(2);
    assert_eq!(b.align, Align::Justify, "{}", b.text);
    let mut e = doc.begin_block_edit(0, &b).expect("starts");
    e.text.insert(0, 'x');
    e.style_of.insert(0, 0);
    doc.block_edit_update(&mut e).expect("updates");
    // Lines other than the last of the paragraph end at the right edge.
    let n = e.text.len();
    let last_line = e.line_of[n - 1];
    for line in 0..last_line {
        let right = (0..n)
            .filter(|&i| e.line_of[i] == line && e.text[i] != ' ')
            .map(|i| e.char_boxes[i][2])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (right - b.bbox[2]).abs() < 1.0,
            "line {line} ends at {right}, box {:?}",
            b.bbox
        );
    }
    doc.end_block_edit(&e);
}

#[test]
fn style_changes_embed_a_bundled_face() {
    let mut doc = open(para_pdf());
    let b = blocks(&mut doc).remove(1);
    let mut e = doc.begin_block_edit(0, &b).expect("starts");
    assert_eq!(e.styles[0].family, "Helvetica");
    assert!(!e.styles[0].bold);
    doc.block_edit_style(&mut e, 4..9, &StyleChange::Bold(true))
        .expect("bold");
    assert!(e.styles[e.style_of[5]].bold);
    assert!(!e.styles[e.style_of[0]].bold);
    doc.block_edit_style(&mut e, 0..3, &StyleChange::Size(14.0))
        .expect("size");
    assert!((e.styles[e.style_of[0]].size - 14.0).abs() < 0.01);
    doc.block_edit_style(&mut e, 0..0, &StyleChange::Color([200, 0, 0]))
        .expect("colour");
    assert!(e.style_of.iter().all(|&k| e.styles[k].color == [200, 0, 0]));
    // Vietnamese is not in the standard font: the style moves to the embedded face.
    e.text.extend("ệ".chars());
    let last = *e.style_of.last().expect("a style");
    e.style_of.push(last);
    doc.block_edit_update(&mut e).expect("updates");
    doc.end_block_edit(&e);
    let text = page_text(&mut doc);
    assert!(text.contains("The quick brown"), "{text}");
    assert!(text.contains("end.ệ"), "{text}");
    let saved = doc.save().expect("saves");
    assert!(saved.windows(20).any(|w| w == b"BlueOffice-Sans-Bold"));
}

#[test]
fn typing_a_line_break_starts_a_new_line() {
    let mut doc = open(para_pdf());
    let b = blocks(&mut doc).remove(0);
    let mut e = doc.begin_block_edit(0, &b).expect("starts");
    e.text.extend("\nSub".chars());
    e.style_of.resize(e.text.len(), 0);
    doc.block_edit_update(&mut e).expect("updates");
    assert_eq!(e.lines, 2);
    let n = e.text.len();
    assert!(e.char_boxes[n - 1][1] > e.char_boxes[0][3] - 2.0);
    doc.end_block_edit(&e);
    let text = page_text(&mut doc);
    assert!(text.contains("Title\nSub"), "{text}");
}

#[test]
fn sample_paragraphs_are_found() {
    let mut engine = bo_engine::Engine::with_bundled_fonts();
    let docx = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/doc/phase1-sample.docx"
    ))
    .expect("corpus");
    engine
        .open(&docx, "phase1-sample.docx")
        .expect("opens docx");
    let mut doc = open(engine.export_pdf().expect("exports"));
    let b = blocks(&mut doc);
    for x in &b {
        eprintln!("{:?} {} {:?} {}", x.align, x.lines, x.bbox, x.text);
    }
    let art = b
        .iter()
        .find(|x| x.text.starts_with("Điều 1."))
        .expect("article 1");
    assert_eq!(art.lines, 4);
    assert_eq!(art.align, Align::Justify);
    assert!(art.text.ends_with("mục đích kiểm thử."), "{}", art.text);
    // Editing it keeps the first-line indent and both edges.
    let mut e = doc.begin_block_edit(0, art).expect("starts");
    e.text.insert(8, 'X');
    e.style_of.insert(8, e.style_of[8]);
    doc.block_edit_update(&mut e).expect("updates");
    assert_eq!(e.lines, 4);
    let first_x = e.char_boxes[0][0];
    assert!(
        (first_x - art.char_boxes[0][0]).abs() < 0.5,
        "{first_x} vs {:?}",
        art.char_boxes[0]
    );
    doc.end_block_edit(&e);
}

/// Round 12: the paragraph's lines are kept chunk by chunk; a key interprets only the lines
/// whose bytes changed and the drawing equals a full interpretation.
#[test]
fn typing_keeps_unchanged_lines() {
    let mut doc = open(para_pdf());
    let b = blocks(&mut doc);
    let para = b.iter().find(|x| x.lines == 3).expect("paragraph");
    let mut e = doc.begin_block_edit(0, para).expect("starts");
    for (k, c) in "xyz".chars().enumerate() {
        e.text.insert(3 + k, c);
        e.style_of.insert(3 + k, e.style_of[3]);
        doc.block_edit_update(&mut e).expect("updates");
        let fast = doc.render(0, RenderOptions::default()).expect("renders");
        assert_eq!(
            doc.typing.chunks.len(),
            e.lines,
            "one kept drawing per line"
        );
        let kept = std::mem::take(&mut doc.typing);
        let full = doc.render(0, RenderOptions::default()).expect("renders");
        doc.typing = kept;
        assert_eq!(fast.list.commands, full.list.commands);
        assert_eq!(fast.chars, full.chars);
    }
    doc.end_block_edit(&e);
}

/// While a paragraph is typed, the page drawing reuses the kept drawing of the rest of the
/// page: the result is the same display list and text layer as a full interpretation.
#[test]
fn typing_reuses_the_page_drawing() {
    let mut doc = open(para_pdf());
    let b = blocks(&mut doc);
    let para = b.iter().find(|x| x.lines == 3).expect("paragraph");
    let mut e = doc.begin_block_edit(0, para).expect("starts");
    for (k, c) in "xyz".chars().enumerate() {
        e.text.insert(3 + k, c);
        e.style_of.insert(3 + k, e.style_of[3]);
        doc.block_edit_update(&mut e).expect("updates");
        let fast = doc.render(0, RenderOptions::default()).expect("renders");
        assert!(doc.typing.drawn.is_some(), "the rest of the page is kept");
        doc.typing.clear();
        let full = doc.render(0, RenderOptions::default()).expect("renders");
        assert_eq!(fast.list.commands, full.list.commands);
        assert_eq!(fast.chars, full.chars);
    }
    doc.end_block_edit(&e);
    assert!(doc.typing.drawn.is_none());
    assert!(
        page_text(&mut doc).contains("Thexyz"),
        "{}",
        page_text(&mut doc)
    );
}

/// Typing cost per keystroke in a sample paragraph: reflow + write, then the page render
/// the view does next (`cargo test --release -p bo-pdfread typing_cost -- --ignored --nocapture`).
#[test]
#[ignore = "timing"]
fn typing_cost() {
    let mut engine = bo_engine::Engine::with_bundled_fonts();
    let docx = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/doc/phase1-sample.docx"
    ))
    .expect("corpus");
    engine
        .open(&docx, "phase1-sample.docx")
        .expect("opens docx");
    let mut doc = open(engine.export_pdf().expect("exports"));
    let _ = doc.render(0, RenderOptions::default());
    let b = blocks(&mut doc);
    let art = b
        .iter()
        .find(|x| x.text.starts_with("Điều 1."))
        .expect("article 1");
    let mut e = doc.begin_block_edit(0, art).expect("starts");
    let (mut up, mut re) = (Vec::new(), Vec::new());
    for k in 0..40 {
        e.text.insert(8 + k, 'x');
        e.style_of.insert(8 + k, e.style_of[8]);
        let t = std::time::Instant::now();
        doc.block_edit_update(&mut e).expect("updates");
        up.push(t.elapsed().as_secs_f64() * 1e3);
        let t = std::time::Instant::now();
        let _ = doc.render(0, RenderOptions::default()).expect("renders");
        re.push(t.elapsed().as_secs_f64() * 1e3);
    }
    doc.end_block_edit(&e);
    for v in [&mut up, &mut re] {
        v.sort_by(f64::total_cmp);
    }
    eprintln!(
        "update p50 {:.2} p95 {:.2} · render p50 {:.2} p95 {:.2} ms",
        up[20], up[38], re[20], re[38]
    );
}

/// Cost per key of a block edit and of the page drawing on a real PDF (`BO_TYPING_PDF`, the
/// largest paragraph of page 1): `cargo test --release -p bo-pdfread typing_cost_pdf -- --ignored
/// --nocapture`.
#[test]
#[ignore = "timing"]
fn typing_cost_pdf() {
    let Ok(path) = std::env::var("BO_TYPING_PDF") else {
        return;
    };
    let mut doc = open(std::fs::read(path).expect("pdf"));
    let b = blocks(&mut doc);
    let para = b.iter().max_by_key(|x| x.lines).expect("paragraph");
    let mut e = doc.begin_block_edit(0, para).expect("starts");
    let opts = RenderOptions::default();
    doc.render(0, opts).expect("renders");
    let (mut up, mut re) = (Vec::new(), Vec::new());
    for (k, c) in "nhanh hon nua di, go thu them mot doan".chars().enumerate() {
        e.text.insert(40 + k, c);
        e.style_of.insert(40 + k, e.style_of[40]);
        let t = std::time::Instant::now();
        doc.block_edit_update(&mut e).expect("updates");
        up.push(t.elapsed().as_secs_f64() * 1e3);
        let t = std::time::Instant::now();
        doc.render(0, opts).expect("renders");
        re.push(t.elapsed().as_secs_f64() * 1e3);
    }
    let q = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        (v[v.len() / 2], v[v.len() * 95 / 100])
    };
    eprintln!(
        "lines {} update {:?} render {:?} (p50, p95 ms)",
        e.lines,
        q(&mut up),
        q(&mut re)
    );
    doc.end_block_edit(&e);
}
