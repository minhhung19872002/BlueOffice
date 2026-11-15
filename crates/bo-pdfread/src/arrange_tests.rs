//! Copies, rotation and flips of page objects (Edit Object).
use super::*;
use crate::tests::build_pdf;

/// One 300 × 200 page: a text run, a filled rectangle and an image.
fn pdf() -> Vec<u8> {
    let content = "BT /F1 12 Tf 20 150 Td (Hello) Tj ET \
                   0 0 1 rg 20 20 60 40 re f \
                   q 50 0 0 30 150 30 cm /Im1 Do Q";
    let img = "<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceGray \
               /BitsPerComponent 8 /Filter /ASCIIHexDecode /Length 9 >>\nstream\n00FFFF00>\nendstream";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R >> /XObject << /Im1 6 0 R >> >> >>"
            .to_owned(),
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

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.1
}

fn kinds(doc: &mut PdfDocument, k: ObjectKind) -> Vec<[f32; 4]> {
    doc.page_objects(0)
        .expect("objects")
        .iter()
        .filter(|o| o.kind == k)
        .map(|o| o.bbox)
        .collect()
}

#[test]
fn copies_keep_the_original_and_undo_as_one_step() {
    let mut doc = PdfDocument::open(pdf(), standard_fonts()).expect("opens");
    let objs = doc.page_objects(0).expect("objects");
    let all: Vec<usize> = (0..objs.len()).collect();
    doc.copy_objects(0, &all, [1.0, 0.0, 0.0, 1.0, 30.0, -10.0])
        .expect("copies");
    for k in [ObjectKind::Text, ObjectKind::Path, ObjectKind::Image] {
        let b = kinds(&mut doc, k);
        assert_eq!(b.len(), 2, "{k:?}: {b:?}");
        let (a, c) = (b[0], b[1]);
        assert!(
            near(c[0], a[0] + 30.0) && near(c[1], a[1] - 10.0),
            "{k:?}: {b:?}"
        );
    }
    // The copied text is readable text.
    let p = doc.render(0, RenderOptions::default()).expect("renders");
    assert_eq!(TextLayer::new(p.chars).text().matches("Hello").count(), 2);
    assert!(doc.undo());
    assert_eq!(kinds(&mut doc, ObjectKind::Image).len(), 1);
    assert_eq!(kinds(&mut doc, ObjectKind::Text).len(), 1);
}

#[test]
fn rotation_and_flip_matrices_turn_about_the_centre() {
    let mut doc = PdfDocument::open(pdf(), standard_fonts()).expect("opens");
    let objs = doc.page_objects(0).expect("objects");
    let i = objs
        .iter()
        .position(|o| o.kind == ObjectKind::Image)
        .expect("image");
    let b = objs[i].bbox;
    let (cx, cy) = (f32::midpoint(b[0], b[2]), f32::midpoint(b[1], b[3]));
    doc.transform_objects(0, &[i], rotation_about(1, cx, cy))
        .expect("rotates");
    let r = kinds(&mut doc, ObjectKind::Image)[0];
    // 50 × 30 becomes 30 × 50 about the same centre.
    assert!(near(r[2] - r[0], 30.0) && near(r[3] - r[1], 50.0), "{r:?}");
    assert!(near(f32::midpoint(r[0], r[2]), cx) && near(f32::midpoint(r[1], r[3]), cy));
    // Four quarter turns are the identity; flips keep the box.
    let m = rotation_about(4, 10.0, 20.0);
    assert!(near(m[0], 1.0) && near(m[3], 1.0) && near(m[4], 0.0) && near(m[5], 0.0));
    let objs = doc.page_objects(0).expect("objects");
    let p = objs
        .iter()
        .position(|o| o.kind == ObjectKind::Path)
        .expect("path");
    let pb = objs[p].bbox;
    doc.transform_objects(0, &[p], flip_about(true, f32::midpoint(pb[0], pb[2]), 0.0))
        .expect("flips");
    let f = kinds(&mut doc, ObjectKind::Path)[0];
    assert!(
        near(f[0], pb[0]) && near(f[2], pb[2]) && near(f[1], pb[1]),
        "{f:?} {pb:?}"
    );
}

#[test]
fn each_object_moves_by_its_own_matrix_in_one_step() {
    let mut doc = PdfDocument::open(pdf(), standard_fonts()).expect("opens");
    let objs = doc.page_objects(0).expect("objects");
    let at = |k: ObjectKind| objs.iter().position(|o| o.kind == k).expect("object");
    let (t, p, i) = (
        at(ObjectKind::Text),
        at(ObjectKind::Path),
        at(ObjectKind::Image),
    );
    let (tb, pb, ib) = (objs[t].bbox, objs[p].bbox, objs[i].bbox);
    doc.transform_each(
        0,
        &[
            (t, [1.0, 0.0, 0.0, 1.0, 5.0, 0.0]),
            (p, [1.0, 0.0, 0.0, 1.0, 10.0, 0.0]),
            (i, [1.0, 0.0, 0.0, 1.0, 0.0, -10.0]),
        ],
    )
    .expect("arranges");
    assert!(near(kinds(&mut doc, ObjectKind::Text)[0][0], tb[0] + 5.0));
    assert!(near(kinds(&mut doc, ObjectKind::Path)[0][0], pb[0] + 10.0));
    assert!(near(kinds(&mut doc, ObjectKind::Image)[0][1], ib[1] - 10.0));
    assert!(doc.undo());
    assert!(near(kinds(&mut doc, ObjectKind::Path)[0][0], pb[0]));
    assert!(near(kinds(&mut doc, ObjectKind::Text)[0][0], tb[0]));
    assert!(!doc.can_undo());
}
