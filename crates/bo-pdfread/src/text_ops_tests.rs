//! Split, kerning and convert-to-shapes commands of Edit Object on text.
use super::*;
use crate::tests::build_pdf;

/// One 300 × 200 page: a three-line paragraph and a kerned heading.
fn pdf() -> Vec<u8> {
    let content = "BT /F1 12 Tf 14 TL 20 150 Td (First line of text here) Tj T* \
                   (second line of the same) Tj T* (third line ends) Tj ET \
                   BT /F1 16 Tf 20 180 Td [(W) 80 (A) -500 (VE)] TJ ET";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R \
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

fn blocks(doc: &mut PdfDocument) -> Vec<TextBlock> {
    text_blocks(&doc.page_objects(0).expect("objects"))
}

#[test]
fn split_makes_one_object_per_line_and_undoes() {
    let mut doc = PdfDocument::open(pdf(), standard_fonts()).expect("opens");
    let before = blocks(&mut doc);
    let para = before
        .iter()
        .find(|b| b.lines == 3)
        .cloned()
        .expect("three-line paragraph");
    let n = doc.split_text_lines(0, &para).expect("splits");
    assert_eq!(n, 3);
    let after = blocks(&mut doc);
    assert_eq!(after.len(), before.len() + 2, "{after:#?}");
    assert!(after.iter().any(|b| b.text == "second line of the same"));
    // The drawing is unchanged: the same text in the same places.
    let p = doc.render(0, RenderOptions::default()).expect("renders");
    assert!(TextLayer::new(p.chars).text().contains("third line ends"));
    // Kept through a save.
    let bytes = doc.save_full(SaveOptions::default()).expect("saves");
    let mut again = PdfDocument::open(bytes, standard_fonts()).expect("reopens");
    assert_eq!(blocks(&mut again).len(), after.len());
    assert!(doc.undo());
    assert_eq!(blocks(&mut doc).len(), before.len());
}

#[test]
fn kerning_is_removed_but_word_gaps_stay() {
    let mut doc = PdfDocument::open(pdf(), standard_fonts()).expect("opens");
    let objs = doc.page_objects(0).expect("objects");
    let tj = objs
        .iter()
        .position(|o| o.text.contains('W'))
        .expect("heading run");
    let w0 = objs[tj].bbox[2] - objs[tj].bbox[0];
    assert!(doc.has_kerning(0, &[tj]));
    assert_eq!(doc.remove_kerning(0, &[tj]).expect("removes"), 1);
    let objs = doc.page_objects(0).expect("objects");
    let i = objs
        .iter()
        .position(|o| o.text.contains('W'))
        .expect("heading");
    let w1 = objs[i].bbox[2] - objs[i].bbox[0];
    // The 80 kern (tightening) is gone: wider by 80/1000 em of 16 pt; the -500 gap stays.
    assert!((w1 - w0 - 1.28).abs() < 0.2, "{w0} {w1}");
    assert!(!doc.has_kerning(0, &[i]));
}

#[test]
fn text_becomes_filled_paths() {
    let mut doc = PdfDocument::open(pdf(), standard_fonts()).expect("opens");
    let objs = doc.page_objects(0).expect("objects");
    let ids: Vec<usize> = objs
        .iter()
        .enumerate()
        .filter(|(_, o)| o.text.contains('W'))
        .map(|(i, _)| i)
        .collect();
    let bbox = objs[ids[0]].bbox;
    doc.text_to_shapes(0, &ids).expect("converts");
    let objs = doc.page_objects(0).expect("objects");
    assert!(!objs.iter().any(|o| o.text.contains('W')));
    let paths: Vec<_> = objs.iter().filter(|o| o.kind == ObjectKind::Path).collect();
    assert_ne!(paths.len(), 0);
    // The outlines sit where the text was.
    let p = paths[0].bbox;
    assert!(
        p[0] >= bbox[0] - 1.0 && p[2] <= bbox[2] + 1.0,
        "{p:?} {bbox:?}"
    );
    assert!(doc.undo());
    assert!(doc
        .page_objects(0)
        .expect("objects")
        .iter()
        .any(|o| o.text.contains('W')));
}

/// A page with one image (2 × 2 grey, drawn 60 × 40).
fn image_pdf() -> Vec<u8> {
    let content = "q 60 0 0 40 50 50 cm /Im1 Do Q";
    let img = "<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceGray \
               /BitsPerComponent 8 /Filter /ASCIIHexDecode /Length 9 >>\nstream\n00FFFF00>\nendstream";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R \
         /Resources << /XObject << /Im1 5 0 R >> >> >>"
            .to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
        img.to_owned(),
    ];
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    build_pdf(&refs)
}

#[test]
fn image_opacity_and_shear() {
    let mut doc = PdfDocument::open(image_pdf(), standard_fonts()).expect("opens");
    assert!((doc.object_opacity(0, 0) - 1.0).abs() < 1e-4);
    doc.set_object_opacity(0, &[0], 0.5).expect("opacity");
    let objs = doc.page_objects(0).expect("objects");
    let i = objs
        .iter()
        .position(|o| o.kind == ObjectKind::Image)
        .expect("image");
    assert!((doc.object_opacity(0, i) - 0.5).abs() < 1e-4);
    let b = objs[i].bbox;
    doc.shear_objects(0, &[i], b, 30.0).expect("shears");
    let objs = doc.page_objects(0).expect("objects");
    let o = objs
        .iter()
        .find(|o| o.kind == ObjectKind::Image)
        .expect("image");
    // Wider by the height × tan 30°, the bottom edge kept.
    let grow = (o.bbox[2] - o.bbox[0]) - (b[2] - b[0]);
    assert!(
        (grow - 40.0 * 30f32.to_radians().tan()).abs() < 0.5,
        "{b:?} {:?}",
        o.bbox
    );
    assert!(doc.undo() && doc.undo());
    assert!((doc.object_opacity(0, 0) - 1.0).abs() < 1e-4);
}
