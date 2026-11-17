//! BlueEditor round 10: PDF Optimizer panels, Audit Space Usage, the File page's PDF Optimizer
//! and Reduce File Size.
use crate::editor_form8::Val;
use crate::editor_tests::{Fixed, Rec};
use crate::editor_window::EditorWindow;
use crate::{Input, Key, Mods, ReaderRequest};
use bo_pdfread::PdfDocument;

const SANS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/LiberationSans-Regular.ttf"
));

/// One page showing "Hi" in an embedded TrueType font, with a half-transparent square.
fn pdf() -> Vec<u8> {
    let content = "BT /F1 24 Tf 72 700 Td (Hi) Tj ET /G1 gs 0 0 1 rg 100 100 200 200 re f";
    let widths = vec!["600"; 95].join(" ");
    let objs: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
          /Resources << /Font << /F1 5 0 R >> /ExtGState << /G1 << /ca 0.5 >> >> >> >>"
            .to_vec(),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        )
        .into_bytes(),
        format!(
            "<< /Type /Font /Subtype /TrueType /BaseFont /LiberationSans /FirstChar 32 \
             /LastChar 126 /Encoding /WinAnsiEncoding /Widths [{widths}] /FontDescriptor 6 0 R >>"
        )
        .into_bytes(),
        b"<< /Type /FontDescriptor /FontName /LiberationSans /Flags 32 \
          /FontBBox [0 -200 1000 900] /ItalicAngle 0 /Ascent 900 /Descent -200 \
          /CapHeight 700 /StemV 80 /FontFile2 7 0 R >>"
            .to_vec(),
        {
            let mut v = format!(
                "<< /Length {} /Length1 {} >>\nstream\n",
                SANS.len(),
                SANS.len()
            )
            .into_bytes();
            v.extend_from_slice(SANS);
            v.extend_from_slice(b"\nendstream");
            v
        },
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
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
    out
}

fn window() -> EditorWindow {
    let doc = PdfDocument::open(pdf(), bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = EditorWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1100.0,
        },
        &Fixed,
    );
    w.open(doc, "fonts.pdf", None);
    w
}

fn paint(w: &mut EditorWindow) -> Rec {
    let mut p = Rec::default();
    w.paint(&mut p);
    p
}

fn show_panel(w: &mut EditorWindow, k: usize) {
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("panel", Val::Pick(k));
    }
    w.opt10_after_input(0);
}

fn enter(w: &mut EditorWindow) {
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
}

fn saved(w: &mut EditorWindow) -> Vec<u8> {
    w.take_requests()
        .into_iter()
        .find_map(|r| match r {
            ReaderRequest::SaveBytes { bytes, .. } => Some(bytes),
            _ => None,
        })
        .expect("saved copy")
}

#[test]
fn optimizer_shows_each_panel_and_unembeds_the_chosen_font() {
    let mut w = window();
    w.run("file.optimize", &Fixed);
    let p = paint(&mut w);
    for t in [
        "Compress Images",
        "Unembed Fonts",
        "Transparency",
        "Clean Up",
        "JPEG",
    ] {
        assert!(p.texts.iter().any(|x| x == t), "{t}: {:?}", p.texts);
    }
    show_panel(&mut w, 1);
    let p = paint(&mut w);
    for t in [
        "Embedded fonts:",
        "Fonts to unembed:",
        "LiberationSans",
        "Subset all embedded fonts",
    ] {
        assert!(p.texts.iter().any(|x| x == t), "{t}: {:?}", p.texts);
    }
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("fEmb", Val::Pick(1));
    }
    assert!(w.opt10_button("unembedBtn"));
    assert_eq!(w.opt10.unembed, vec!["LiberationSans".to_owned()]);
    assert_eq!(w.opt10.embedded, Vec::<String>::new());
    for (k, t) in [
        (2, "Medium Resolution"),
        (3, "Discard document tags"),
        (4, "Discard file attachments"),
        (5, "Compress entire file"),
    ] {
        show_panel(&mut w, k);
        let p = paint(&mut w);
        assert!(p.texts.iter().any(|x| x == t), "{t}: {:?}", p.texts);
    }
    enter(&mut w);
    let bytes = saved(&mut w);
    assert!(!String::from_utf8_lossy(&bytes).contains("/FontFile2"));
    assert!(bytes.len() * 4 < pdf().len());
}

#[test]
fn transparency_waits_for_the_rendered_pages() {
    let mut w = window();
    w.run("file.optimize", &Fixed);
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("transp", Val::Pick(1));
    }
    show_panel(&mut w, 2);
    let p = paint(&mut w);
    assert!(
        p.texts.iter().any(|x| x == "Line Art and Text: 300"),
        "{:?}",
        p.texts
    );
    enter(&mut w);
    let reqs = w.take_requests();
    assert!(reqs.iter().any(
        |r| matches!(r, ReaderRequest::RenderPages { purpose, .. } if purpose == "opt10flatten")
    ));
    // The host renders the page (a 4 × 4 picture here) and the optimizer goes on.
    let img = bo_raster::codecs::Rgba {
        width: 4,
        height: 4,
        pixels: vec![255; 64],
    };
    let png = bo_raster::rgba_to_png(img).expect("png");
    w.pages_rendered("opt10flatten", vec![png]);
    let bytes = saved(&mut w);
    let mut back = PdfDocument::open(bytes, bo_pdfread::standard_fonts()).expect("opens");
    let page = back
        .render(0, bo_pdfread::RenderOptions::default())
        .expect("renders");
    assert_eq!(page.images.len(), 1, "the page is a picture now");
}

#[test]
fn audit_lists_bytes_by_kind_with_a_total() {
    let mut w = window();
    w.run("file.optimize", &Fixed);
    assert!(w.opt10_button("audit"));
    let p = paint(&mut w);
    for t in [
        "Audit Space Usage",
        "Fonts",
        "Content Streams",
        "Total",
        "100 %",
    ] {
        assert!(p.texts.iter().any(|x| x == t), "{t}: {:?}", p.texts);
    }
    // OK goes back to the optimizer.
    enter(&mut w);
    assert_eq!(w.form8.len(), 1);
}

#[test]
fn file_page_has_the_optimizer_rows_and_reduce_file_size() {
    let mut w = window();
    w.open_backstage();
    w.backstage_command("bs.optimizer");
    let p = paint(&mut w);
    for t in [
        "Reduce File Size",
        "Advanced Optimization",
        "Optimize Scanned PDF",
        "Apply to Current File",
        "Current File",
    ] {
        assert!(p.texts.iter().any(|x| x == t), "{t}: {:?}", p.texts);
    }
    w.backstage_command("bs.opt.current");
    assert_eq!(
        w.form8.last().map(|f| f.0.title.clone()),
        Some("Reduce File Size".to_owned())
    );
    let p = paint(&mut w);
    assert!(p.texts.iter().any(|x| x == "PDF Version Compatibility"));
    enter(&mut w);
    let bytes = saved(&mut w);
    // Standard settings: the font is subset.
    assert!(bytes.len() * 4 < pdf().len(), "{}", bytes.len());
    // The Advanced row opens the full dialog.
    w.open_backstage();
    w.backstage_command("bs.optimizer");
    w.backstage_command("bs.opt.row1");
    w.backstage_command("bs.opt.current");
    assert_eq!(w.form8.last().map(|f| f.0.size), Some((1158.0, 795.0)));
}

#[test]
fn optimizer_settings_can_be_saved_and_deleted() {
    let mut w = window();
    w.run("file.optimize", &Fixed);
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("ppi", Val::Text("72".into()));
    }
    assert!(w.opt10_button("optSave"));
    w.handle(Input::Text("Tiny".into()), &Fixed);
    enter(&mut w);
    let f = &w.form8.last().expect("optimizer").0;
    assert_eq!(f.pick("saved"), 2);
    // Mobile, then the saved one again.
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("saved", Val::Pick(1));
    }
    w.opt10_after_input(2);
    assert_eq!(w.form8.last().expect("o").0.text("ppi"), "96");
    if let Some((f, _)) = w.form8.last_mut() {
        f.set("saved", Val::Pick(2));
    }
    w.opt10_after_input(1);
    assert_eq!(w.form8.last().expect("o").0.text("ppi"), "72");
    assert!(w.opt10_button("optDelete"));
    assert_eq!(w.form8.last().expect("o").0.pick("saved"), 0);
    assert_eq!(w.opt10_preset_names().len(), 2);
}

#[test]
fn rendered_pages_become_jpeg_tiff_and_bmp_files() {
    // The host hands rendered pages over as PNG (round 10 fix: they were read as GIF / BMP /
    // TIFF only, so these conversions saved nothing).
    let img = bo_raster::codecs::Rgba {
        width: 3,
        height: 2,
        pixels: vec![200; 24],
    };
    let png = bo_raster::rgba_to_png(img).expect("png");
    for purpose in ["jpeg", "tiff", "bmp"] {
        let mut w = window();
        w.pages_rendered(purpose, vec![png.clone()]);
        let files = w
            .take_requests()
            .into_iter()
            .find_map(|r| match r {
                ReaderRequest::SaveMany(f) => Some(f),
                _ => None,
            })
            .expect("files");
        assert_eq!(files.len(), 1, "{purpose}");
        assert!(files[0].1.len() > 20, "{purpose}");
    }
}

#[test]
fn edit_object_menu_picks_one_kind_and_web_links_are_listed() {
    let mut w = window();
    w.run("ed.obj.image", &Fixed);
    assert_eq!(w.edit_tool(), Some(crate::EditTool::Object));
    assert_eq!(w.edit.filter10, Some(bo_pdfread::ObjectKind::Image));
    w.run("ed.obj.all", &Fixed);
    assert_eq!(w.edit.filter10, None);
    w.run("ed.webLinks.view", &Fixed);
    let p = paint(&mut w);
    assert!(
        p.texts.iter().any(|x| x == "View Web Links"),
        "{:?}",
        p.texts
    );
}

#[test]
fn protect_tab_marks_by_search_and_pages_and_shows_security() {
    let mut w = window();
    // Search & Redact marks "Hi", Apply removes the text.
    w.run("prot.searchRedact", &Fixed);
    w.handle(Input::Text("hi".into()), &Fixed);
    enter(&mut w);
    assert_eq!(w.x.marks.len(), 1, "{:?}", w.x.marks);
    w.run("prot.applyRedact", &Fixed);
    let objs = w
        .view_mut()
        .expect("v")
        .doc_mut()
        .page_objects(0)
        .expect("objects");
    assert!(!bo_pdfread::text_lines(&objs)
        .iter()
        .any(|l| l.text.contains("Hi")));
    // Pages: the whole current page.
    w.run("prot.redactPages", &Fixed);
    enter(&mut w);
    assert_eq!(w.x.marks.len(), 1);
    assert!(w.x.marks[0].1[2] > 600.0, "{:?}", w.x.marks);
    // Security Properties of an unencrypted file.
    w.run("prot.securityProps", &Fixed);
    let p = paint(&mut w);
    assert!(p.texts.iter().any(|x| x == "No Security"), "{:?}", p.texts);
    enter(&mut w);
    // Sanitize asks first, then runs.
    w.run("prot.sanitize", &Fixed);
    assert_eq!(
        w.form8.last().map(|f| f.0.title.clone()),
        Some("Sanitize Document".to_owned())
    );
    enter(&mut w);
    assert!(w.form8.is_empty());
}
