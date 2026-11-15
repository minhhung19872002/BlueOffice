//! Edit Text / Edit Object round trips.
use super::*;
use crate::tests::build_pdf;

/// One 300 × 200 page: two text runs, a filled rectangle and an image (`rotate` degrees).
fn objects_pdf(rotate: i32) -> Vec<u8> {
    let content = "BT /F1 12 Tf 20 150 Td (Hello) Tj ( World) Tj ET \
                   0 0 1 rg 20 20 60 40 re f \
                   q 50 0 0 30 150 30 cm /Im1 Do Q";
    let img = "<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceGray \
               /BitsPerComponent 8 /Filter /ASCIIHexDecode /Length 9 >>\nstream\n00FFFF00>\nendstream";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Rotate {rotate} /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> /XObject << /Im1 6 0 R >> >> >>"
        ),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        img.to_owned(),
    ];
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    build_pdf(&refs)
}

fn open(bytes: Vec<u8>) -> PdfDocument {
    PdfDocument::open(bytes, standard_fonts()).expect("opens")
}

fn text(doc: &mut PdfDocument) -> String {
    let p = doc.render(0, RenderOptions::default()).expect("renders");
    TextLayer::new(p.chars).text()
}

fn chars(doc: &mut PdfDocument) -> Vec<TextChar> {
    doc.render(0, RenderOptions::default())
        .expect("renders")
        .chars
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

#[test]
fn page_objects_have_kinds_and_boxes() {
    let mut doc = open(objects_pdf(0));
    let objs = doc.page_objects(0).expect("objects");
    let kinds: Vec<ObjectKind> = objs.iter().map(|o| o.kind).collect();
    assert_eq!(
        kinds,
        [
            ObjectKind::Text,
            ObjectKind::Text,
            ObjectKind::Path,
            ObjectKind::Image
        ]
    );
    assert_eq!(objs[0].text, "Hello");
    assert_eq!(objs[1].text, " World");
    assert!(near(objs[0].font_size, 12.0));
    // Rectangle 20,20 60×40 in user space → page points y down: top = 200 − 60.
    let r = objs[2].bbox;
    assert!(
        near(r[0], 20.0) && near(r[1], 140.0) && near(r[2], 80.0) && near(r[3], 180.0),
        "{r:?}"
    );
    let i = objs[3].bbox;
    assert!(
        near(i[0], 150.0) && near(i[1], 140.0) && near(i[2], 200.0) && near(i[3], 170.0),
        "{i:?}"
    );
}

#[test]
fn delete_and_move_keep_other_text_in_place() {
    let mut doc = open(objects_pdf(0));
    let before = chars(&mut doc);
    let world_x = before[5].rect[0];
    doc.delete_objects(0, &[0]).expect("deletes");
    let after = chars(&mut doc);
    assert_eq!(TextLayer::new(after.clone()).text().trim(), "World");
    // " World" did not move although "Hello" before it is gone.
    assert!(
        near(after[0].rect[0], world_x),
        "{} {world_x}",
        after[0].rect[0]
    );
    // Move " World" by (10, 20) page points.
    let objs = doc.page_objects(0).expect("objects");
    let w = objs
        .iter()
        .position(|o| o.kind == ObjectKind::Text)
        .expect("text");
    doc.transform_objects(0, &[w], [1.0, 0.0, 0.0, 1.0, 10.0, 20.0])
        .expect("moves");
    let moved = chars(&mut doc);
    assert!(near(moved[0].rect[0], world_x + 10.0));
    assert!(near(moved[0].rect[1], before[5].rect[1] + 20.0));
    // Path and image move by wrapping them in a transform.
    let objs = doc.page_objects(0).expect("objects");
    let (p, i) = (
        objs.iter()
            .position(|o| o.kind == ObjectKind::Path)
            .expect("path"),
        objs.iter()
            .position(|o| o.kind == ObjectKind::Image)
            .expect("image"),
    );
    let (pb, ib) = (objs[p].bbox, objs[i].bbox);
    doc.transform_objects(0, &[p, i], [1.0, 0.0, 0.0, 1.0, -5.0, 7.0])
        .expect("moves");
    let objs = doc.page_objects(0).expect("objects");
    let path = objs
        .iter()
        .find(|o| o.kind == ObjectKind::Path)
        .expect("path");
    let img = objs
        .iter()
        .find(|o| o.kind == ObjectKind::Image)
        .expect("image");
    assert!(near(path.bbox[0], pb[0] - 5.0) && near(path.bbox[1], pb[1] + 7.0));
    assert!(near(img.bbox[2], ib[2] - 5.0) && near(img.bbox[3], ib[3] + 7.0));
    // Resize the image to twice its width about its left edge.
    let k = objs
        .iter()
        .position(|o| o.kind == ObjectKind::Image)
        .expect("image");
    let b = objs[k].bbox;
    doc.transform_objects(0, &[k], [2.0, 0.0, 0.0, 1.0, -b[0], 0.0])
        .expect("resizes");
    let objs = doc.page_objects(0).expect("objects");
    let img = objs
        .iter()
        .find(|o| o.kind == ObjectKind::Image)
        .expect("image");
    assert!(near(img.bbox[2] - img.bbox[0], 2.0 * (b[2] - b[0])));
    // Saved, reopened: the same.
    let mut again = open(doc.save().expect("saves"));
    assert_eq!(text(&mut again).trim(), "World");
    // Undo everything.
    for _ in 0..4 {
        assert!(doc.undo());
    }
    assert_eq!(text(&mut doc), "Hello World");
}

#[test]
fn edit_text_in_own_font_or_embedded_font() {
    let mut doc = open(objects_pdf(0));
    let x0 = chars(&mut doc)[0].rect[0];
    // Every character exists in Helvetica (WinAnsi): the run's font is kept.
    let id = doc.replace_text(0, 0, "Hi there").expect("edits");
    let objs = doc.page_objects(0).expect("objects");
    assert_eq!(objs[id].text, "Hi there");
    assert!(near(objs[id].char_boxes[0][0], x0));
    assert!(!doc.bytes().windows(3).any(|w| w == b"BoF"));
    // Vietnamese needs the embedded Unicode font.
    let id = doc.replace_text(0, id, "Tiếng Việt").expect("edits");
    let objs = doc.page_objects(0).expect("objects");
    assert_eq!(objs[id].text, "Tiếng Việt");
    assert!(near(objs[id].font_size, 12.0));
    let saved = doc.save().expect("saves");
    let mut again = open(saved);
    let t = text(&mut again);
    assert!(t.contains("Tiếng Việt") && t.contains("World"), "{t}");
    // Every glyph drew (no missing-glyph boxes): glyph runs exist for the new text.
    let page = again.render(0, RenderOptions::default()).expect("renders");
    assert!(page
        .list
        .commands
        .iter()
        .any(|c| matches!(c, bo_render::Command::Glyphs { text, .. } if text.contains('ế'))));
    // An empty text deletes the run.
    let mut d = open(objects_pdf(0));
    d.replace_text(0, 0, "").expect("edits");
    assert_eq!(text(&mut d).trim(), "World");
}

fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().expect("header");
        let px: Vec<u8> = (0..w * h)
            .flat_map(|i| [200, (i % 255) as u8, 30, if i % 2 == 0 { 255 } else { 128 }])
            .collect();
        wr.write_image_data(&px).expect("data");
    }
    out
}

#[test]
fn add_text_add_and_replace_images() {
    let mut doc = open(objects_pdf(90));
    // Rotated page: shown 200 × 300.
    assert_eq!(
        doc.page_info(0).map(|p| (p.width, p.height)),
        Some((200.0, 300.0))
    );
    let id = doc
        .add_text(0, [30.0, 50.0], "Xin chào", 14.0, [200, 0, 0])
        .expect("adds");
    let objs = doc.page_objects(0).expect("objects");
    let o = &objs[id];
    assert_eq!(o.text, "Xin chào");
    assert!(near(o.font_size, 14.0));
    // Upright on screen: the run goes right from x 30 with its baseline at y 50.
    assert!(
        o.bbox[0] >= 29.0 && o.bbox[0] < 31.5 && o.bbox[3] > 50.0 && o.bbox[1] < 50.0,
        "{:?}",
        o.bbox
    );
    assert!(o.bbox[2] - o.bbox[0] > o.bbox[3] - o.bbox[1]);
    assert_eq!(o.color, [200, 0, 0]);
    doc.add_image(0, [10.0, 200.0, 70.0, 260.0], NewImage(&png_bytes(4, 3)))
        .expect("adds image");
    let objs = doc.page_objects(0).expect("objects");
    let img = objs
        .iter()
        .rposition(|o| o.kind == ObjectKind::Image)
        .expect("image");
    let b = objs[img].bbox;
    assert!(
        near(b[0], 10.0) && near(b[1], 200.0) && near(b[2], 70.0) && near(b[3], 260.0),
        "{b:?}"
    );
    doc.replace_image(0, img, NewImage(&png_bytes(2, 2)))
        .expect("replaces");
    let objs = doc.page_objects(0).expect("objects");
    let b2 = objs
        .iter()
        .rfind(|o| o.kind == ObjectKind::Image)
        .expect("image")
        .bbox;
    assert_eq!(b, b2);
    let mut again = open(doc.save().expect("saves"));
    let page = again.render(0, RenderOptions::default()).expect("renders");
    assert!(page.images.len() >= 2);
    assert!(TextLayer::new(page.chars).text().contains("Xin chào"));
    assert!(doc
        .add_image(0, [0.0; 4], NewImage(b"not an image"))
        .is_err());
}

#[test]
fn typing_group_undoes_as_one() {
    let mut doc = open(objects_pdf(0));
    doc.begin_group("typing");
    let mut id = 0;
    for t in ["H", "He", "Hel", "Hell!"] {
        id = doc.replace_text(0, id, t).expect("edits");
    }
    doc.end_group();
    assert_eq!(
        text(&mut doc).split_whitespace().collect::<String>(),
        "WorldHell!"
    );
    assert!(doc.undo());
    assert_eq!(text(&mut doc), "Hello World");
    assert!(!doc.can_undo());
}

#[test]
fn edit_text_in_embedded_subset_font() {
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
    let objs = doc.page_objects(0).expect("objects");
    let lines = text_lines(&objs);
    let first = lines.first().expect("a line");
    assert_eq!(first.text, "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM");
    assert_eq!(first.char_boxes.len(), first.text.chars().count());
    let run = first.text.clone();
    let ids = first.ids.clone();
    // The same characters reversed: every glyph is in the subset, the font is kept.
    let reversed: String = run.chars().rev().collect();
    let before = doc.bytes().len();
    let id = doc.replace_text_runs(0, &ids, &reversed).expect("edits");
    let objs = doc.page_objects(0).expect("objects");
    assert_eq!(objs[id].text, reversed);
    let lines = text_lines(&objs);
    assert!(lines.iter().any(|l| l.text == reversed));
    assert!(!lines.iter().any(|l| l.text == run));
    let saved = doc.save().expect("saves");
    assert!(saved.len() < before + 20_000, "no font embedded");
    assert!(!saved.windows(15).any(|w| w == b"BlueOffice-Sans"));
    // Characters outside the subset fall back to the embedded font.
    let mut doc = open(saved);
    let id = doc.replace_text(0, id, "∑ Ω ≈").expect("edits");
    let objs = doc.page_objects(0).expect("objects");
    assert_eq!(objs[id].text, "∑ Ω ≈");
}

#[test]
fn fallback_face_follows_the_font_name() {
    use crate::fallback::face_for;
    assert_eq!(face_for("ABCDEF+Tinos-Bold"), 1);
    assert_eq!(face_for("TimesNewRomanPS-BoldItalicMT"), 3);
    assert_eq!(face_for("Helvetica-Oblique"), 6);
    assert_eq!(face_for("Arial"), 4);
    assert_eq!(face_for("LiberationSans-Bold"), 5);
}
