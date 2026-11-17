//! Document-level tests: hand-built PDFs and PDFs exported by BlueDoc.
use super::*;
use bo_render::Command;

/// Build a PDF from object bodies (object `i + 1` is `objs[i]`); object 1 is the catalog.
pub(crate) fn build_pdf(objs: &[&str]) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
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
    out
}

fn stream(body: &str) -> String {
    xstream("", body)
}

fn xstream(dict: &str, body: &str) -> String {
    format!(
        "<< {dict} /Length {} >>\nstream\n{body}\nendstream",
        body.len()
    )
}

fn simple_pdf(content: &str, extra: &[&str]) -> Vec<u8> {
    let page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
                /Resources << /Font << /F1 5 0 R >> >> >>";
    let mut objs = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        page.to_owned(),
        stream(content),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    objs.extend(extra.iter().map(|s| (*s).to_owned()));
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    build_pdf(&refs)
}

fn open(bytes: Vec<u8>) -> PdfDocument {
    PdfDocument::open(bytes, standard_fonts()).expect("opens")
}

#[test]
fn page_geometry_paths_and_text() {
    let mut doc = open(simple_pdf(
        "1 0 0 RG 2 w 10 10 m 190 10 l S 0 0 1 rg 20 20 50 30 re f \
         BT /F1 12 Tf 1 0 0 1 20 70 Tm (Hi there) Tj ET",
        &[],
    ));
    assert_eq!(doc.page_count(), 1);
    let info = doc.page_info(0).expect("page");
    assert_eq!((info.width, info.height), (200.0, 100.0));
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    // The stroked line is at y = 100 - 10 in page points (y down).
    let line = page.list.commands.iter().find_map(|c| match c {
        Command::Path {
            segs,
            stroke: Some(s),
            ..
        } => Some((segs.clone(), s.clone())),
        _ => None,
    });
    let (segs, s) = line.expect("stroked path");
    assert_eq!(segs[0], bo_render::PathSeg::M(10.0, 90.0));
    assert_eq!(s.color, bo_render::Rgba(255, 0, 0, 255));
    assert!((s.width - 2.0).abs() < 1e-4);
    let run = page.list.commands.iter().find_map(|c| match c {
        Command::Glyphs {
            x,
            y,
            size,
            text,
            glyphs,
            ..
        } => Some((*x, *y, *size, text.clone(), glyphs.len())),
        _ => None,
    });
    let (x, y, size, text, n) = run.expect("glyph run");
    assert_eq!((x, y, size), (20.0, 30.0, 12.0));
    assert_eq!(text, "Hi there");
    assert_eq!(n, 8);
    let layer = TextLayer::new(page.chars);
    assert_eq!(layer.text(), "Hi there");
    assert_eq!(layer.find("there", false, false), vec![(3, 7)]);
    // Helvetica's "H" is 722/1000 em wide: the next glyph starts 8.664 pt later.
    let c = &layer.chars()[1];
    assert!(
        (c.origin[0] - (20.0 + 8.664)).abs() < 0.05,
        "{:?}",
        c.origin
    );
}

#[test]
fn segments_after_closepath_start_a_new_subpath() {
    // `h` then `l` (and a doubled `h`): the line continues from the closed subpath's start
    // (ISO 32000-1 §8.5.2.1), so every subpath of the display list begins with a move.
    let mut doc = open(simple_pdf(
        "10 10 m 50 10 l 50 50 l h 90 90 l 90 10 l h h 20 20 m 30 30 40 40 50 20 c h \
         60 60 70 70 80 60 c f",
        &[],
    ));
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    let segs = page
        .list
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Path { segs, .. } => Some(segs.clone()),
            _ => None,
        })
        .expect("path");
    let mut open_path = false;
    for s in &segs {
        match s {
            bo_render::PathSeg::M(..) => open_path = true,
            bo_render::PathSeg::Z => {
                assert!(open_path, "{segs:?}");
                open_path = false;
            }
            _ => assert!(open_path, "{segs:?}"),
        }
    }
    // The line after the first `h` starts at (10, 10) → (10, 90) in page points.
    assert_eq!(segs[3], bo_render::PathSeg::Z);
    assert_eq!(segs[4], bo_render::PathSeg::M(10.0, 90.0));
}

#[test]
fn rotation_crop_labels_and_outline() {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /Outlines 6 0 R /PageLabels << /Nums [0 << /S /r >> 1 << /S /D /P (A-) >>] >> >>",
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 300 200] /Rotate 90 >>",
        "<< /Type /Page /Parent 2 0 R /Contents 5 0 R >>",
        "<< /Type /Page /Parent 2 0 R /CropBox [10 10 110 60] /Rotate 0 /Contents 5 0 R >>",
        &stream("0 g 0 0 10 10 re f"),
        "<< /Type /Outlines /First 7 0 R /Last 7 0 R /Count 1 >>",
        "<< /Title <FEFF0054006900EA0075> /Parent 6 0 R /Dest [4 0 R /XYZ 0 50 0] >>",
    ];
    let doc = open(build_pdf(&objs));
    let p0 = doc.page_info(0).expect("p0");
    assert_eq!((p0.width, p0.height, p0.rotate), (200.0, 300.0, 90));
    assert_eq!(p0.label, "i");
    let p1 = doc.page_info(1).expect("p1");
    assert_eq!((p1.width, p1.height), (100.0, 50.0));
    assert_eq!(p1.label, "A-1");
    let o = doc.outline();
    assert_eq!(o.len(), 1);
    assert_eq!(o[0].title, "Tiêu");
    assert_eq!(
        o[0].target,
        Some(LinkTarget::Page {
            index: 1,
            top: Some(10.0)
        })
    );
}

#[test]
fn rotated_page_maps_user_space() {
    // Rotate 90: the user-space origin (bottom left) shows at the top left.
    let m = page_matrix([0.0, 0.0, 300.0, 200.0], 90);
    let (x, y) = interp::apply(m, 0.0, 0.0);
    assert!((x - 0.0).abs() < 1e-4 && (y - 0.0).abs() < 1e-4);
    let (x, y) = interp::apply(m, 300.0, 0.0);
    assert!((x - 0.0).abs() < 1e-4 && (y - 300.0).abs() < 1e-4);
}

#[test]
fn reading_loads_objects_on_demand() {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R /Outlines 6 0 R /PageMode /UseOutlines >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R          /Resources << /Font << /F1 5 0 R >> >> /Annots [8 0 R 9 0 R] >>",
        &stream("BT /F1 12 Tf 10 50 Td (Hello) Tj ET"),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        "<< /Type /Outlines /First 7 0 R /Last 7 0 R /Count 1 >>",
        "<< /Title (One) /Parent 6 0 R /Dest [3 0 R /Fit] >>",
        "<< /Type /Annot /Subtype /Text /Rect [150 50 170 70] /Contents (Note) /T (Me) >>",
        "<< /Type /Annot /Subtype /Link /Rect [10 40 60 60] /A << /S /URI /URI (https://example.com) >> >>",
        "<< /Title (Doc) /Author (A) >>",
    ];
    let mut bytes = build_pdf(&objs);
    let at = bytes
        .windows(11)
        .rposition(|w| w == b"/Root 1 0 R")
        .expect("trailer");
    bytes.splice(at..at, b"/Info 10 0 R ".iter().copied());
    // The startxref offset is unchanged: the trailer comes after the table.
    let mut doc = open(bytes);
    let p = doc.render(0, RenderOptions::default()).expect("renders");
    assert!(TextLayer::new(p.chars.clone()).text().contains("Hello"));
    assert_eq!(p.links.len(), 1);
    assert_eq!(doc.title().as_deref(), Some("Doc"));
    assert_eq!(doc.info("Author").as_deref(), Some("A"));
    assert_eq!(doc.outline().len(), 1);
    assert_eq!(doc.annotations().len(), 1);
    assert_eq!(doc.page_mode().as_deref(), Some("UseOutlines"));
    let _ = doc.doc_info();
    let _ = doc.font_list();
    assert!(!doc.fully_loaded());
    // An edit loads the whole document and saves as before.
    let a = doc.annotations().remove(0);
    doc.set_annotation_contents(&a, "Changed").expect("edits");
    assert!(doc.fully_loaded());
    let saved = doc.save().expect("saves");
    let again = open(saved);
    assert_eq!(again.annotations()[0].contents, "Changed");
}

#[test]
fn images_clips_forms_and_annotation_appearance() {
    let content = "q 0 0 100 50 re W n q 40 0 0 20 10 10 cm /Im1 Do Q Q /Fm1 Do";
    let page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
                /Resources << /XObject << /Im1 5 0 R /Fm1 6 0 R >> >> /Annots [7 0 R] >>";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        page,
        &stream(content),
        &xstream(
            "/Type /XObject /Subtype /Image /Width 2 /Height 1 /BitsPerComponent 8 \
             /ColorSpace /DeviceRGB /Filter /ASCIIHexDecode",
            "ff000000ff00>",
        ),
        &xstream(
            "/Type /XObject /Subtype /Form /BBox [0 0 50 50] /Matrix [1 0 0 1 100 0]",
            "0 1 0 rg 0 0 5 5 re f",
        ),
        "<< /Type /Annot /Subtype /Square /Rect [150 50 190 90] /AP << /N 8 0 R >> >>",
        &xstream(
            "/Type /XObject /Subtype /Form /BBox [0 0 40 40]",
            "1 0 0 rg 0 0 40 40 re f",
        ),
    ];
    let mut doc = open(build_pdf(&objs));
    let p = doc.render(0, RenderOptions::default()).expect("renders");
    let cmds = &p.list.commands;
    assert!(
        matches!(cmds[0], Command::PushClip { x, y, w, h } if (x, y, w, h) == (0.0, 50.0, 100.0, 50.0))
    );
    let img = cmds.iter().find_map(|c| match c {
        Command::Image {
            x, y, w, h, image, ..
        } => Some((*x, *y, *w, *h, image.clone())),
        _ => None,
    });
    let (x, y, w, h, id) = img.expect("image");
    assert_eq!((x, y, w, h), (10.0, 70.0, 40.0, 20.0));
    // No /Interpolate: enlarged without smoothing.
    assert!(id.ends_with(crate::interp::PIXELATED));
    let pic = &p.images[&id];
    assert_eq!((pic.width, pic.height), (2, 1));
    assert_eq!(&pic.pixels[..4], &[255, 0, 0, 255]);
    // Form at x 100 and the annotation appearance mapped onto its rectangle.
    let fills: Vec<(Vec<bo_render::PathSeg>, bo_render::Rgba)> = cmds
        .iter()
        .filter_map(|c| match c {
            Command::Path {
                segs,
                fill: Some(bo_render::Paint::Solid { color }),
                ..
            } => Some((segs.clone(), *color)),
            _ => None,
        })
        .collect();
    assert!(fills
        .iter()
        .any(|(s, c)| *c == bo_render::Rgba(0, 255, 0, 255)
            && s[0] == bo_render::PathSeg::M(100.0, 100.0)));
    assert!(fills
        .iter()
        .any(|(s, c)| *c == bo_render::Rgba(255, 0, 0, 255)
            && s[0] == bo_render::PathSeg::M(150.0, 50.0)));
    let balance = cmds.iter().fold(0_i32, |n, c| match c {
        Command::PushClip { .. } | Command::PushClipPath { .. } => n + 1,
        Command::PopClip => n - 1,
        _ => n,
    });
    assert_eq!(balance, 0);
}

#[test]
fn add_highlight_and_save_incrementally() {
    let mut doc = open(simple_pdf("BT /F1 12 Tf 20 70 Td (Hello) Tj ET", &[]));
    let before = doc.bytes().len();
    doc.add_annotation(&annot::NewAnnot {
        kind: AnnotKind::Highlight,
        page: 0,
        rects: vec![[20.0, 20.0, 50.0, 32.0]],
        color: [255, 230, 0],
        author: "Tester".into(),
        contents: "Ghi chú".into(),
    })
    .expect("adds");
    assert!(doc.modified());
    let saved = doc.save().expect("saves");
    assert!(saved.len() > before);
    assert!(saved.ends_with(b"%%EOF\n"));
    let mut again = open(saved);
    let list = again.annotations();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, AnnotKind::Highlight);
    assert_eq!(list[0].contents, "Ghi chú");
    assert_eq!(list[0].author, "Tester");
    let r = list[0].rect;
    assert!((r[0] - 20.0).abs() < 0.01 && (r[1] - 20.0).abs() < 0.01 && (r[3] - 32.0).abs() < 0.01);
    // The appearance stream draws the highlight.
    let p = again.render(0, RenderOptions::default()).expect("renders");
    assert!(p.list.commands.iter().any(|c| matches!(c,
        Command::Path { fill: Some(bo_render::Paint::Solid { color }), .. } if color.0 == 255 && color.1 == 230)));
}

#[test]
fn bluedoc_export_round_trip() {
    let mut engine = bo_engine::Engine::with_bundled_fonts();
    let docx = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/doc/phase1-sample.docx"
    ))
    .expect("corpus");
    engine
        .open(&docx, "phase1-sample.docx")
        .expect("opens docx");
    let pdf = engine.export_pdf().expect("exports");
    let mut doc = open(pdf);
    assert!(doc.page_count() >= 1);
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    let runs = page
        .list
        .commands
        .iter()
        .filter(|c| matches!(c, Command::Glyphs { .. }))
        .count();
    assert!(runs > 10, "embedded fonts draw as glyph runs ({runs})");
    let text = TextLayer::new(page.chars).text();
    assert!(text.contains("THÔNG BÁO"), "{text}");
    assert!(text.contains("Điều 1."), "{text}");
}

#[test]
fn garbage_and_hostile_input_never_panics() {
    assert!(PdfDocument::open(b"not a pdf".to_vec(), FontManager::new()).is_err());
    assert!(PdfDocument::open(
        b"%PDF-1.4\n1 0 obj << >> endobj".to_vec(),
        FontManager::new()
    )
    .is_err());
    // Deep form recursion and a self-referencing form.
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 4 0 R /Resources << /XObject << /X 5 0 R >> >> >>",
        &stream("/X Do q q q Q Q Q Q Q ( unterminated"),
        &xstream(
            "/Type /XObject /Subtype /Form /BBox [0 0 1 1] /Resources << /XObject << /X 5 0 R >> >>",
            "/X Do",
        ),
    ];
    let mut doc = open(build_pdf(&objs));
    let p = doc.render(0, RenderOptions::default()).expect("renders");
    let balance = p.list.commands.iter().fold(0_i32, |n, c| match c {
        Command::PushClip { .. } | Command::PushClipPath { .. } => n + 1,
        Command::PopClip => n - 1,
        _ => n,
    });
    assert_eq!(balance, 0);
    // Random byte flips of a real file.
    let base = simple_pdf(
        "BT /F1 12 Tf 20 70 Td (Hello) Tj ET 0 0 1 rg 0 0 5 5 re f",
        &[],
    );
    let mut seed = 12345_u32;
    for _ in 0..200 {
        let mut b = base.clone();
        for _ in 0..8 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let i = (seed as usize >> 4) % b.len();
            b[i] = (seed >> 16) as u8;
        }
        if let Ok(mut d) = PdfDocument::open(b, FontManager::new()) {
            for i in 0..d.page_count().min(3) {
                let _ = d.render(i, RenderOptions::default());
            }
            let _ = d.outline();
            let _ = d.annotations();
        }
    }
}

#[test]
fn blend_modes_and_soft_masks_become_groups() {
    // ISO 32000-1 §11.3.5 /BM and §11.6.5 /SMask in an ExtGState: each painted object under
    // them is one group; the clip of `W n` stays outside the group; a form is one group.
    let page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
                /Resources << /ExtGState << /M 5 0 R /S 6 0 R >> /XObject << /X 8 0 R >> >> >>";
    let mask = xstream(
        "/Type /XObject /Subtype /Form /BBox [0 0 200 100] \
         /Group << /S /Transparency /CS /DeviceGray >>",
        "0.5 g 0 0 100 100 re f",
    );
    let form = xstream(
        "/Type /XObject /Subtype /Form /BBox [0 0 200 100]",
        "0 0 1 rg 0 0 5 5 re f 1 0 0 rg 5 5 5 5 re f",
    );
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        page.to_owned(),
        stream(
            "q /M gs 1 1 0 rg 10 10 50 20 re f 0 0 80 80 re W n /X Do Q \
             q /S gs 0 0 1 rg 0 0 40 40 re f Q 0 g 0 0 1 1 re f",
        ),
        "<< /Type /ExtGState /BM /Multiply >>".to_owned(),
        "<< /Type /ExtGState /SMask << /Type /Mask /S /Luminosity /G 7 0 R /BC [1] >> >>"
            .to_owned(),
        mask,
        form,
    ];
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    let mut doc = open(build_pdf(&refs));
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    let kinds: Vec<&str> = page
        .list
        .commands
        .iter()
        .map(|c| match c {
            Command::PushGroup {
                blend: Some(_),
                mask: None,
                ..
            } => "blend",
            Command::PushGroup { mask: Some(_), .. } => "mask",
            Command::PopGroup => "end",
            Command::Path { .. } => "path",
            Command::PushClip { .. } | Command::PushClipPath { .. } => "clip",
            Command::PopClip => "unclip",
            _ => "other",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "blend", "path", "end", "clip", "blend", "clip", "path", "path", "unclip", "end",
            "unclip", "mask", "path", "end", "path"
        ]
    );
    let mask = page.list.commands.iter().find_map(|c| match c {
        Command::PushGroup { mask: Some(m), .. } => Some(m.clone()),
        _ => None,
    });
    let mask = mask.expect("soft mask");
    assert!(mask.luminosity);
    assert_eq!(mask.backdrop, bo_render::Rgba(255, 255, 255, 255));
    assert!(mask
        .commands
        .iter()
        .any(|c| matches!(c, Command::Path { .. })));
    assert!(matches!(
        page.list.commands.first(),
        Some(Command::PushGroup {
            blend: Some(bo_render::BlendMode::Multiply),
            ..
        })
    ));
}

#[test]
fn text_render_modes_stroke_and_clip() {
    // Mode 1: outlines stroked, not filled; mode 7: invisible, clips what follows to the text.
    let mut doc = open(simple_pdf(
        "BT /F1 40 Tf 1 Tr 1 0 0 RG 10 50 Td (A) Tj ET \
         q BT /F1 40 Tf 7 Tr 100 50 Td (B) Tj ET 0 0 1 rg 0 0 200 100 re f Q",
        &[],
    ));
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    let c = &page.list.commands;
    assert!(c.iter().any(|c| matches!(
        c,
        Command::Path {
            fill: None,
            stroke: Some(_),
            ..
        }
    )));
    let clip = c
        .iter()
        .position(|c| matches!(c, Command::PushClipPath { .. }))
        .expect("text clip");
    let fill = c
        .iter()
        .position(|c| {
            matches!(
                c,
                Command::FillRect { .. } | Command::Path { fill: Some(_), .. }
            )
        })
        .expect("fill");
    assert!(clip < fill);
    assert!(!c.iter().any(|c| matches!(c, Command::Glyphs { .. })));
}

#[test]
fn embedded_truetype_without_widths_advances_by_its_own_metrics() {
    // A symbolic simple TrueType font with no /Widths: the glyphs must not pile up.
    let ttf = crate::bundled::FONTS[0];
    let content = b"BT /F 20 Tf 72 700 Td (abc) Tj ET";
    let objs: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
          /Resources << /Font << /F 5 0 R >> >> >>"
            .to_vec(),
        [
            format!("<< /Length {} >>\nstream\n", content.len()).as_bytes(),
            content,
            b"\nendstream",
        ]
        .concat(),
        b"<< /Type /Font /Subtype /TrueType /BaseFont /Test /FontDescriptor 6 0 R >>".to_vec(),
        b"<< /Type /FontDescriptor /FontName /Test /Flags 4 /FontFile2 7 0 R >>".to_vec(),
        [
            format!("<< /Length {} >>\nstream\n", ttf.len()).as_bytes(),
            ttf,
            b"\nendstream",
        ]
        .concat(),
    ];
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offs = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offs.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        pdf.extend_from_slice(o);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let x = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offs {
        pdf.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    let mut doc = open(pdf);
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    let xs: Vec<f32> = page.chars.iter().map(|c| c.origin[0]).collect();
    assert_eq!(xs.len(), 3, "{xs:?}");
    assert!(xs[1] - xs[0] > 5.0 && xs[2] - xs[1] > 5.0, "{xs:?}");
}

#[test]
fn standard_symbol_fonts_use_their_built_in_encodings() {
    // Symbol and ZapfDingbats not embedded (Annex D.5 / D.6): 'a' is alpha, 'H' a black star
    // whatever face substitutes them.
    let pdf = build_pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << /Font << /S 5 0 R /Z 6 0 R >> >> >>",
        "<< /Length 58 >>\nstream\nBT /S 12 Tf 72 700 Td (ab) Tj /Z 12 Tf 0 -20 Td (H4) Tj ET\nendstream",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Symbol >>",
        "<< /Type /Font /Subtype /Type1 /BaseFont /ZapfDingbats >>",
    ]);
    let mut doc = open(pdf);
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    let text: String = page.chars.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(text, "\u{3b1}\u{3b2}\u{2605}\u{2714}");
}

#[test]
fn zapf_dingbats_ignores_a_named_base_encoding() {
    // PyMuPDF's check boxes: `/BaseFont /ZapfDingbats /Encoding /WinAnsiEncoding`, code '3'.
    let pdf = build_pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << /Font << /Z 5 0 R >> >> >>",
        "<< /Length 33 >>\nstream\nBT /Z 12 Tf 72 700 Td (3) Tj ET\nendstream",
        "<< /Type /Font /Subtype /Type1 /BaseFont /ZapfDingbats /Encoding /WinAnsiEncoding >>",
    ]);
    let mut doc = open(pdf);
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    let text: String = page.chars.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(text, "\u{2713}");
}

#[test]
fn aes_256_with_an_empty_user_password_opens() {
    // Revision 6 (ISO 32000-2 §7.6.4.4) with only an owner password (made with MuPDF).
    let bytes = include_bytes!("../testdata/aes256-empty-user.pdf").to_vec();
    assert!(!PdfDocument::needs_password(&bytes));
    let mut doc = open(bytes);
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    let text: String = page.chars.iter().map(|c| c.text.as_str()).collect();
    assert!(text.contains("AES-256"), "{text}");
}

#[test]
fn no_zoom_notes_keep_their_screen_size() {
    // A 16 pt sticky note with Print | NoZoom | NoRotate (28), drawn at 2× zoom: half its
    // rectangle about the upper left corner (§12.5.3).
    let mut d = Dictionary::new();
    d.set("F", 28);
    let r = [330.0, 656.0, 346.0, 672.0];
    assert_eq!(no_zoom_rect(&d, r, 0.5), [330.0, 664.0, 338.0, 672.0]);
    assert_eq!(no_zoom_rect(&d, r, 0.0), r);
    d.set("F", 4);
    assert_eq!(no_zoom_rect(&d, r, 0.5), r);
    // Rendered: the note's appearance fills only the scaled rectangle.
    let pdf = build_pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R] >>",
        "<< /Type /Annot /Subtype /Text /F 28 /Rect [330 656 346 672] /AP << /N 5 0 R >> >>",
        "<< /Type /XObject /Subtype /Form /BBox [0 0 16 16] /Length 22 >>\nstream\n1 1 0 rg 0 0 16 16 re f\nendstream",
    ]);
    let mut doc = PdfDocument::open(pdf, standard_fonts()).expect("opens");
    let extra = RenderExtra {
        no_zoom: 0.5,
        ..RenderExtra::default()
    };
    let page = doc
        .render_extra(0, RenderOptions::default(), extra)
        .expect("renders");
    let xs: Vec<f32> = page
        .list
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Path { segs, .. } => Some(segs.iter().filter_map(|s| match s {
                bo_render::PathSeg::M(x, _) | bo_render::PathSeg::L(x, _) => Some(*x),
                _ => None,
            })),
            _ => None,
        })
        .flatten()
        .collect();
    let right = xs.iter().copied().fold(f32::MIN, f32::max);
    assert!((right - 338.0).abs() < 0.01, "{xs:?}");
}
