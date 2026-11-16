//! Page Templates, PDF Portfolio and Batch Encrypt.
use crate::dialog::DialogButton;
use crate::editor_actions::Ask2;
use crate::editor_five::FiveAsk;
use crate::editor_tests::{texts, window, Fixed};
use crate::reader_window::ReaderRequest;

#[test]
fn page_templates_add_spawn_and_delete() {
    let mut w = window(2);
    w.run("form.pageTemplates", &Fixed);
    assert!(matches!(
        w.dialog2,
        Some((_, Ask2::Five(FiveAsk::Templates(_))))
    ));
    w.dialog2 = None;
    // Name page 1 "Cover", then spawn it after the current page (page 1).
    w.five_answer(
        FiveAsk::Templates(Vec::new()),
        DialogButton::Ok,
        &["Cover".into()],
    );
    w.run("form.pageTemplates", &Fixed);
    let Some((_, Ask2::Five(FiveAsk::Templates(names)))) = w.dialog2.take() else {
        panic!("templates dialog");
    };
    assert_eq!(names, ["Cover"]);
    w.five_answer(
        FiveAsk::Templates(names.clone()),
        DialogButton::FindNext,
        &[String::new(), "1".into()],
    );
    assert_eq!(texts(&mut w), ["Page 1", "Page 1", "Page 2"]);
    w.undo(true);
    assert_eq!(texts(&mut w), ["Page 1", "Page 2"]);
    w.five_answer(
        FiveAsk::Templates(names),
        DialogButton::Replace,
        &[String::new(), "1".into()],
    );
    assert!(w
        .view_mut()
        .is_some_and(|v| v.doc_mut().page_templates().is_empty()));
}

/// A one-page portfolio embedding a text file.
fn portfolio_pdf() -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /Collection << /Type /Collection >> \
         /Names << /EmbeddedFiles << /Names [(notes.txt) 4 0 R] >> >> >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>",
        "<< /Type /Filespec /F (notes.txt) /Desc (Notes) /EF << /F 5 0 R >> >>",
        "<< /Type /EmbeddedFile /Length 5 >>\nstream\nhello\nendstream",
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let x = out.len();
    out.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for o in offs {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n").as_bytes(),
    );
    out
}

#[test]
fn a_portfolio_opens_in_its_details_view() {
    let mut w = window(1);
    let doc =
        bo_pdfread::PdfDocument::open(portfolio_pdf(), bo_pdfread::standard_fonts()).expect("pdf");
    w.open(doc, "folio.pdf", None);
    let mut rec = crate::editor_tests::Rec::default();
    w.paint(&mut rec);
    assert!(
        rec.texts.iter().any(|t| t == "notes.txt"),
        "{:?}",
        rec.texts
    );
    assert!(rec.texts.iter().any(|t| t == "5 Bytes"));
    // Double click on the row saves the text file.
    let r = w.five.portfolio.as_ref().expect("view").rows[0];
    w.handle(
        crate::Input::Press {
            x: r[0] + 50.0,
            y: r[1] + 5.0,
            count: 2,
            shift: false,
        },
        &Fixed,
    );
    assert!(w.take_requests().iter().any(
        |q| matches!(q, ReaderRequest::SaveBytes { name, bytes } if name == "notes.txt" && bytes == b"hello")
    ));
    // Convert › PDF Portfolio shows the cover page.
    w.run("conv.portfolio", &Fixed);
    let mut rec = crate::editor_tests::Rec::default();
    w.paint(&mut rec);
    assert!(!rec.texts.iter().any(|t| t == "notes.txt"));
}

#[test]
fn portfolio_without_files_says_so_and_batch_encrypts_copies() {
    let mut w = window(1);
    w.run("conv.portfolio", &Fixed);
    assert!(w.dialog2.is_none());
    w.run("prot.batchEncrypt", &Fixed);
    let Some((_, Ask2::Five(FiveAsk::Batch(tabs)))) = w.dialog2.take() else {
        panic!("batch dialog");
    };
    assert_eq!(tabs, [0]);
    // Add a file, then encrypt the open document and the file.
    w.five_answer(
        FiveAsk::Batch(tabs.clone()),
        DialogButton::FindNext,
        &["pw".into(), String::new(), "1".into(), "1".into()],
    );
    assert!(w
        .take_requests()
        .iter()
        .any(|r| matches!(r, ReaderRequest::PickFile { purpose, .. } if purpose == "batchAdd")));
    let extra = crate::editor_tests::pdf(2);
    w.file_picked("batchAdd", std::path::Path::new("other.pdf"), &extra);
    let Some((d, Ask2::Five(FiveAsk::Batch(tabs)))) = w.dialog2.take() else {
        panic!("batch dialog again");
    };
    // The typed password stays.
    assert_eq!(d.values()[0], "pw");
    w.five_answer(
        FiveAsk::Batch(tabs),
        DialogButton::Ok,
        &[
            "pw".into(),
            String::new(),
            "1".into(),
            "1".into(),
            "1".into(),
        ],
    );
    let saves = w
        .take_requests()
        .into_iter()
        .find_map(|r| match r {
            ReaderRequest::SaveMany(f) => Some(f),
            _ => None,
        })
        .expect("files");
    assert_eq!(saves.len(), 2);
    assert_eq!(saves[0].0, "test-protected.pdf");
    assert_eq!(saves[1].0, "other-protected.pdf");
    for (_, b) in saves {
        assert!(bo_pdfread::PdfDocument::needs_password(&b));
    }
}
