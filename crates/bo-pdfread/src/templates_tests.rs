//! Page templates and embedded files.
use super::*;
use crate::tests::build_pdf;

/// `n` pages "Page 1" …; with `files`, an embedded file "notes.txt" and a portfolio.
fn pdf(n: usize, files: bool) -> Vec<u8> {
    let mut objs: Vec<String> = vec![
        if files {
            "<< /Type /Catalog /Pages 2 0 R /Collection << /Type /Collection >> \
             /Names << /EmbeddedFiles << /Names [(notes.txt) 4 0 R] >> >> >>"
                .into()
        } else {
            "<< /Type /Catalog /Pages 2 0 R >>".into()
        },
        String::new(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        "<< /Type /Filespec /F (notes.txt) /UF (notes.txt) /Desc (Meeting notes) \
         /EF << /F 5 0 R >> >>"
            .into(),
        "<< /Type /EmbeddedFile /Params << /Size 11 >> /Length 11 >>\nstream\nhello notes\nendstream"
            .into(),
    ];
    let mut kids = Vec::new();
    for i in 0..n {
        let content = format!("BT /F1 24 Tf 72 700 Td (Page {}) Tj ET", i + 1);
        let id = objs.len() + 1;
        objs.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {} 0 R \
             /Resources << /Font << /F1 3 0 R >> >> >>",
            id + 1
        ));
        objs.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ));
        kids.push(format!("{id} 0 R"));
    }
    objs[1] = format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" "));
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    build_pdf(&refs)
}

fn texts(doc: &mut PdfDocument) -> Vec<String> {
    (0..doc.page_count())
        .map(|i| {
            let p = doc.render(i, RenderOptions::default()).expect("renders");
            TextLayer::new(p.chars).text().trim().to_owned()
        })
        .collect()
}

#[test]
fn templates_are_named_spawned_saved_and_removed() {
    let mut doc = PdfDocument::open(pdf(3, false), standard_fonts()).expect("opens");
    assert_eq!(doc.page_templates(), []);
    doc.add_page_template("Invoice", 1).expect("adds");
    doc.add_page_template("Cover", 0).expect("adds");
    assert_eq!(
        doc.page_templates(),
        [("Cover".to_owned(), 0), ("Invoice".to_owned(), 1)]
    );
    // Spawn "Invoice" after the last page: one undo step.
    let at = doc.spawn_page_template("Invoice", 3).expect("spawns");
    assert_eq!(at, 3);
    assert_eq!(texts(&mut doc), ["Page 1", "Page 2", "Page 3", "Page 2"]);
    // Before the first page too.
    let at = doc.spawn_page_template("Cover", 0).expect("spawns");
    assert_eq!(at, 0);
    assert_eq!(texts(&mut doc)[..2], ["Page 1", "Page 1"]);
    assert!(doc.undo());
    assert_eq!(texts(&mut doc).len(), 4);
    assert!(doc.undo());
    assert_eq!(texts(&mut doc), ["Page 1", "Page 2", "Page 3"]);
    // The templates survive a save.
    let bytes = doc.save_full(SaveOptions::default()).expect("saves");
    let mut again = PdfDocument::open(bytes, standard_fonts()).expect("opens");
    assert_eq!(again.page_templates().len(), 2);
    again.remove_page_template("Cover").expect("removes");
    assert_eq!(again.page_templates(), [("Invoice".to_owned(), 1)]);
    assert!(again.remove_page_template("Cover").is_err());
}

#[test]
fn portfolio_files_are_listed_and_read() {
    let doc = PdfDocument::open(pdf(1, true), standard_fonts()).expect("opens");
    assert!(doc.is_portfolio());
    let files = doc.embedded_files();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "notes.txt");
    assert_eq!(files[0].size, 11);
    assert_eq!(files[0].description, "Meeting notes");
    assert_eq!(doc.embedded_file_bytes(0).expect("bytes"), b"hello notes");
    assert!(doc.embedded_file_bytes(1).is_err());
    let plain = PdfDocument::open(pdf(1, false), standard_fonts()).expect("opens");
    assert!(!plain.is_portfolio() && plain.embedded_files().is_empty());
}
