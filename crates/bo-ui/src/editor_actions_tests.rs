//! Headless tests of BlueEditor's forms, comments, redaction, signing and conversion.
use crate::editor_tests::{key, texts, window, Fixed};
use crate::reader_window::ReaderRequest;
use crate::{EditTool, EditorWindow, Input, Key, Mods};
use bo_pdfread::PdfDocument;

/// A page with a text field and a check box.
fn form_window() -> EditorWindow {
    let on = "<< /Type /XObject /Subtype /Form /BBox [0 0 12 12] /Length 16 >>\nstream\n0 g 2 2 8 8 re f\nendstream";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R 5 0 R] >> >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R 5 0 R] >>".to_owned(),
        "<< /FT /Tx /T (name) /Subtype /Widget /Rect [100 700 300 720] /DA (/Helv 12 Tf 0 g) /P 3 0 R >>".to_owned(),
        "<< /FT /Btn /T (ok) /Subtype /Widget /Rect [100 650 112 662] /AS /Off \
         /AP << /N << /Yes 6 0 R /Off 6 0 R >> >> /P 3 0 R >>"
            .to_owned(),
        on.to_owned(),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    let doc = PdfDocument::open(out, bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = EditorWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1100.0,
        },
        &Fixed,
    );
    w.open(doc, "form.pdf", None);
    w
}

fn click_doc(w: &mut EditorWindow, page: usize, x: f32, y: f32, count: u8) {
    let r = w
        .view()
        .and_then(|v| v.doc_rect_to_window(page, [x, y, x, y]))
        .expect("on screen");
    w.handle(
        Input::Press {
            x: r[0],
            y: r[1],
            count,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x: r[0], y: r[1] }, &Fixed);
}

#[test]
fn fill_fields_by_clicking() {
    let mut w = form_window();
    w.run("form.highlight", &Fixed);
    let mut rec = crate::editor_tests::Rec::default();
    w.paint(&mut rec);
    // The check box (page points: y down from the 792 pt top).
    click_doc(&mut w, 0, 106.0, 136.0, 1);
    let fields = w.view().expect("view").doc().form_fields();
    assert!(fields[1].checked);
    // The text field asks for a value.
    click_doc(&mut w, 0, 150.0, 82.0, 1);
    w.handle(Input::Text("Trần Thị B".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let fields = w.view().expect("view").doc().form_fields();
    assert_eq!(fields[0].value, "Trần Thị B");
    assert!(texts(&mut w)[0].contains("Trần Thị B"));
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(w.view().expect("view").doc().form_fields()[0].value, "");
    w.run("form.flatten", &Fixed);
    assert_eq!(w.view().expect("view").doc().form_fields().len(), 0);
}

#[test]
fn comments_edit_and_delete() {
    let mut w = window(1);
    w.view_mut()
        .expect("view")
        .doc_mut()
        .add_annotation(&bo_pdfread::NewAnnot {
            kind: bo_pdfread::AnnotKind::Note,
            page: 0,
            rects: vec![[300.0, 300.0, 320.0, 320.0]],
            color: [255, 214, 0],
            author: "A".into(),
            contents: "cũ".into(),
        })
        .expect("adds");
    // Double-click edits the text.
    click_doc(&mut w, 0, 310.0, 310.0, 2);
    w.handle(Input::Key(Key::End, Mods::default()), &Fixed);
    w.handle(Input::Text(" mới".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let list = w.view().expect("view").doc().annotations();
    assert_eq!(list[0].contents, "cũ mới");
    // Click selects, Delete deletes.
    click_doc(&mut w, 0, 310.0, 310.0, 1);
    key(&mut w, Key::Delete, false, false);
    assert_eq!(w.view().expect("view").doc().annotations().len(), 0);
}

#[test]
fn mark_and_apply_redactions() {
    let mut w = window(1);
    w.run("prot.redact", &Fixed);
    assert_eq!(w.edit_tool(), Some(EditTool::Redact));
    // "Page 1" at x 72, baseline 92 (page points from the top), 24 pt.
    let v = w.view().expect("view");
    let a = v
        .doc_rect_to_window(0, [60.0, 60.0, 60.0, 60.0])
        .expect("page");
    let b = v
        .doc_rect_to_window(0, [300.0, 100.0, 300.0, 100.0])
        .expect("page");
    w.handle(
        Input::Press {
            x: a[0],
            y: a[1],
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: b[0], y: b[1] }, &Fixed);
    w.handle(Input::Release { x: b[0], y: b[1] }, &Fixed);
    let mut rec = crate::editor_tests::Rec::default();
    w.paint(&mut rec);
    w.run("prot.applyRedact", &Fixed);
    assert_eq!(texts(&mut w), [""]);
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(texts(&mut w), ["Page 1"]);
}

#[test]
fn sign_with_a_certificate() {
    let mut w = window(2);
    let signer = bo_sign::testing::self_signed(&mut rand::thread_rng(), "BlueEditor Test");
    w.sign_with(&signer, "Approved", "Hà Nội").expect("signs");
    let req = w.take_requests();
    let Some(ReaderRequest::SaveBytes { name, bytes }) = req.first() else {
        panic!("{req:?}");
    };
    assert_eq!(name, "document-signed.pdf");
    let sigs = bo_sign::pdf::verify_pdf(bytes).expect("verifies");
    assert_eq!(sigs.len(), 1);
    assert!(sigs[0].covers_whole_file);
    // The signed file still opens with both pages.
    let doc = PdfDocument::open(bytes.clone(), bo_pdfread::standard_fonts()).expect("opens");
    assert_eq!(doc.page_count(), 2);
}

#[test]
fn convert_to_office_formats_text_and_back() {
    let mut w = window(2);
    for (cmd, ext) in [
        ("conv.word", "docx"),
        ("conv.excel", "xlsx"),
        ("conv.text", "txt"),
    ] {
        w.run(cmd, &Fixed);
        let req = w.take_requests();
        let Some(ReaderRequest::SaveBytes { name, bytes }) = req.first() else {
            panic!("{cmd}: {req:?}");
        };
        assert_eq!(name, &format!("document.{ext}"));
        match ext {
            "docx" => {
                let mut e = bo_engine::Engine::with_bundled_fonts();
                e.open(bytes, "x.docx").expect("opens");
                let text = String::from_utf8_lossy(&e.save_docx().expect("saves")).len();
                assert!(text > 0);
            }
            "xlsx" => {
                let mut s = bo_engine::SheetEngine::with_bundled_fonts();
                s.open(bytes, "x.xlsx").expect("opens");
                let c = s.cell(0, 0, 0).expect("cell");
                assert!(format!("{c:?}").contains("Page 1"), "{c:?}");
            }
            _ => {
                let t = String::from_utf8_lossy(bytes);
                assert!(t.contains("Page 1") && t.contains("Page 2"), "{t}");
            }
        }
    }
    // Images and slides are rendered by the host, then written.
    w.run("conv.slides", &Fixed);
    assert!(
        matches!(&w.take_requests()[..], [ReaderRequest::RenderPages { purpose, .. }] if purpose == "slides")
    );
    let png = crate::editor_tests::png2x2();
    w.pages_rendered("slides", vec![png.clone(), png.clone()]);
    let req = w.take_requests();
    let Some(ReaderRequest::SaveBytes { name, bytes }) = req.first() else {
        panic!("{req:?}");
    };
    assert_eq!(name, "document.pptx");
    assert!(bo_pptx::is_pptx(bytes));
    w.pages_rendered("images", vec![png.clone(), png]);
    assert!(matches!(&w.take_requests()[..], [ReaderRequest::SaveMany(f)] if f.len() == 2));
    // A document made into a PDF opens in a new tab.
    let mut e = bo_engine::Engine::with_bundled_fonts();
    e.new_document("a.docx").expect("new");
    let docx = e.save_docx().expect("docx");
    w.file_picked("create", std::path::Path::new("a.docx"), &docx);
    let req = w.take_requests();
    assert!(
        matches!(&req[..], [ReaderRequest::OpenDocument { name, bytes }] if name == "a.pdf" && bytes.starts_with(b"%PDF"))
    );
    w.run("conv.blank", &Fixed);
    let req = w.take_requests();
    let Some(ReaderRequest::OpenDocument { bytes, .. }) = req.first() else {
        panic!("{req:?}");
    };
    assert_eq!(
        PdfDocument::open(bytes.clone(), bo_pdfread::standard_fonts())
            .expect("opens")
            .page_count(),
        1
    );
}
