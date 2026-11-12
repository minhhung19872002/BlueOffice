//! BlueSlide exports (SLIDE-EXPORT-002): handouts, notes pages, slide images
//! (PNG / JPEG / SVG) and the presentation package. Expected geometry is
//! derived by hand from the layout rules in `slide_export::handout`.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_export::{
    handout::Order, HandoutOptions, HeaderFooter, ImageFormat, ImageOptions, NotesOptions, Paper,
    SlideExport,
};
use bo_engine::Engine;
use bo_model::slide::RawPart;
use bo_pptx::build::{build_pptx, DeckSpec, ParaSpec, ShapeSpec, SlideSpec};
use bo_render::{Command, PageList, Rgba};
use std::io::Read as _;

/// 16:9 deck (960 × 540 pt) with `n` slides; slide 1 shows "Hello World".
fn deck(n: usize) -> Engine {
    let slides = (0..n)
        .map(|i| SlideSpec {
            background: Some(if i == 0 {
                "FF0000".into()
            } else {
                "FFFFFF".into()
            }),
            shapes: if i == 0 {
                vec![ShapeSpec {
                    name: "Text".into(),
                    x: 914_400,
                    y: 914_400,
                    cx: 6_096_000,
                    cy: 914_400,
                    text: vec![ParaSpec::plain("Hello World")],
                    ..ShapeSpec::default()
                }]
            } else {
                Vec::new()
            },
            notes: if i == 0 {
                vec!["First speaker note".into(), "Second paragraph".into()]
            } else {
                Vec::new()
            },
            ..SlideSpec::default()
        })
        .collect();
    let bytes = build_pptx(&DeckSpec {
        width: 12_192_000,
        height: 6_858_000,
        slides,
        title: None,
    })
    .unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open_pptx(&bytes, "deck.pptx").unwrap();
    e
}

fn pdf_pages(pdf: &[u8]) -> usize {
    let s = String::from_utf8_lossy(pdf);
    s.matches("/Type /Page").count() - s.matches("/Type /Pages").count()
}

/// Transforms that place slides on a page: `[scale, x, y]`.
fn placements(p: &PageList) -> Vec<[f32; 3]> {
    p.commands
        .iter()
        .filter_map(|c| match c {
            Command::PushTransform { m } if m[1] == 0.0 && m[2] == 0.0 && m[0] < 1.0 => {
                Some([m[0], m[4], m[5]])
            }
            _ => None,
        })
        .collect()
}

fn texts(p: &PageList) -> Vec<String> {
    p.commands
        .iter()
        .filter_map(|c| match c {
            Command::Glyphs { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn close(a: f32, b: f32) {
    assert!((a - b).abs() < 0.01, "{a} != {b}");
}

fn handout(per_page: u8) -> HandoutOptions {
    HandoutOptions {
        per_page,
        ..HandoutOptions::default()
    }
}

#[test]
fn handout_page_count_is_ceil_n_over_k() {
    let mut e = deck(7);
    for (k, want) in [(1u8, 7usize), (2, 4), (3, 3), (4, 2), (6, 2), (9, 1)] {
        let pdf = e.export_slides(&SlideExport::Handout(handout(k))).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert_eq!(pdf_pages(&pdf), want, "{k} per page");
        let list = e.handout_pages(&handout(k)).unwrap();
        assert_eq!(list.pages.len(), want);
        // The last page holds the remaining slides.
        let last = 7 - (want - 1) * usize::from(k);
        assert_eq!(placements(list.pages.last().unwrap()).len(), last, "{k}");
    }
    assert!(e.export_slides(&SlideExport::Handout(handout(5))).is_err());
}

#[test]
fn six_per_page_a4_frames_and_vertical_order() {
    let mut e = deck(6);
    let list = e.handout_pages(&handout(6)).unwrap();
    let page = &list.pages[0];
    close(page.width, 595.2756);
    close(page.height, 841.8898);
    // Frame 252.6378 wide → scale 252.6378 / 960; slot 0 at (36, 99.2606), slot 1 at x = 306.6378.
    let p = placements(page);
    close(p[0][0], 252.6378 / 960.0);
    close(p[0][1], 36.0);
    close(p[0][2], 99.2606);
    close(p[1][1], 306.6378);
    close(p[1][2], 99.2606);
    // Frames: one stroked outline per slide.
    let outlines = page
        .commands
        .iter()
        .filter(|c| {
            matches!(
                c,
                Command::Path {
                    fill: None,
                    stroke: Some(_),
                    ..
                }
            )
        })
        .count();
    assert!(outlines >= 6);
    // Vertical order: the second slide goes below the first (row 1: y = 54 + 250.6299 + 45.2606).
    let v = e
        .handout_pages(&HandoutOptions {
            order: Order::Vertical,
            frame: false,
            ..handout(6)
        })
        .unwrap();
    let p = placements(&v.pages[0]);
    close(p[1][1], 36.0);
    close(p[1][2], 349.8905);
    // Without frames no outline is added around slides that have no shapes.
    let none = v.pages[0]
        .commands
        .iter()
        .filter(
            |c| matches!(c, Command::Path { fill: None, stroke: Some(s), .. } if s.width == 0.75),
        )
        .count();
    assert_eq!(none, 0);
}

#[test]
fn three_per_page_letter_has_note_lines() {
    let mut e = deck(3);
    let list = e
        .handout_pages(&HandoutOptions {
            paper: Paper::Letter,
            ..handout(3)
        })
        .unwrap();
    let page = &list.pages[0];
    // 16:9 in a 261 pt column: 146.8125 high → floor(146.8125 / 24) = 6 lines per slide.
    let lines: Vec<_> = page
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Line { x1, x2, y1, .. } => Some((*x1, *x2, *y1)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 18);
    close(lines[0].0, 315.0);
    close(lines[0].1, 576.0);
    // Cell 216 high, frame top 54 + (216 − 146.8125) / 2 = 88.59375; first line 24 below.
    close(lines[0].2, 112.59375);
    let p = placements(page);
    close(p[0][0], 261.0 / 960.0);
    close(p[0][2], 88.59375);
}

const HANDOUT_MASTER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:handoutMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Header"/><p:cNvSpPr/><p:nvPr><p:ph type="hdr" sz="quarter"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2971800" cy="457200"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="vi-VN" sz="1100"/><a:t>Lớp học</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Date"/><p:cNvSpPr/><p:nvPr><p:ph type="dt" sz="quarter" idx="1"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="3884613" y="0"/><a:ext cx="2971800" cy="457200"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn="r"/><a:r><a:rPr lang="vi-VN"/><a:t>01/01/2026</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="4" name="Number"/><p:cNvSpPr/><p:nvPr><p:ph type="sldNum" sz="quarter" idx="3"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="3884613" y="8685213"/><a:ext cx="2971800" cy="458787"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn="r"/><a:r><a:rPr lang="vi-VN"/><a:t>Trang </a:t></a:r><a:fld id="{B6F15528-21DE-4FAA-801E-634DDDAF4B2B}" type="slidenum"><a:rPr lang="vi-VN"/><a:t>‹#›</a:t></a:fld></a:p></p:txBody></p:sp></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:hf dt="0" ftr="0"/></p:handoutMaster>"#;

#[test]
fn handout_master_header_footer() {
    let mut e = deck(7);
    e.presentation_mut().unwrap().parts.push(RawPart {
        name: "/ppt/handoutMasters/handoutMaster1.xml".into(),
        content_type: Some(
            "application/vnd.openxmlformats-officedocument.presentationml.handoutMaster+xml".into(),
        ),
        data: HANDOUT_MASTER.as_bytes().to_vec(),
    });
    let list = e.handout_pages(&handout(6)).unwrap();
    let t = texts(&list.pages[1]);
    assert!(t.iter().any(|s| s == "Lớp học"), "{t:?}");
    // Slide number field → page number; the date is switched off by p:hf dt="0".
    assert!(t.iter().any(|s| s == "Trang 2"), "{t:?}");
    assert!(!t.iter().any(|s| s == "01/01/2026"), "{t:?}");
    // The header sits in the master's box scaled from notesSz (540 × 720 pt) to A4:
    // baseline 3.6 + 11 pt below the top × 841.8898 / 720.
    let y = list.pages[1]
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Glyphs { text, y, size, .. } if text == "Lớp học" => Some((*y, *size)),
            _ => None,
        })
        .unwrap();
    close(y.1, 11.0);
    close(y.0, 14.6);
    // Options override the master.
    let list = e
        .handout_pages(&HandoutOptions {
            hf: HeaderFooter {
                header: Some(String::new()),
                date: Some("09/11/2026".into()),
                page_numbers: Some(false),
                footer: Some("Bluestar".into()),
            },
            ..handout(6)
        })
        .unwrap();
    let t = texts(&list.pages[0]);
    assert!(
        !t.iter().any(|s| s == "Lớp học" || s.starts_with("Trang")),
        "{t:?}"
    );
    assert!(t.iter().any(|s| s == "09/11/2026"));
    assert!(t.iter().any(|s| s == "Bluestar"));
}

#[test]
fn notes_pages_place_slide_and_notes() {
    let mut e = deck(3);
    let o = NotesOptions {
        paper: Paper::Notes,
        ..NotesOptions::default()
    };
    let list = e.notes_pages(&o).unwrap();
    assert_eq!(list.pages.len(), 3);
    let page = &list.pages[0];
    close(page.width, 540.0);
    close(page.height, 720.0);
    // Notes master sldImg: off (685 800, 1 143 000) ext 5 486 400 × 3 086 100 EMU
    // = (54, 90) 432 × 243 pt; a 960-pt slide scales by 432 / 960 = 0.45.
    let p = placements(page);
    assert_eq!(p.len(), 1);
    close(p[0][0], 0.45);
    close(p[0][1], 54.0);
    close(p[0][2], 90.0);
    // Notes text, laid out like slide text (formatting kept), in the body
    // placeholder's box: the transform puts its origin at (54, 346.5)
    // (y = 4 400 550 EMU); 12 pt from the notes master's notesStyle.
    let at = page
        .commands
        .iter()
        .rposition(|c| matches!(c, Command::PushTransform { m } if m[0] == 1.0))
        .unwrap();
    let Command::PushTransform { m } = &page.commands[at] else {
        unreachable!()
    };
    close(m[4], 54.0);
    close(m[5], 346.5);
    let mut lines: Vec<(f32, String)> = Vec::new();
    for c in &page.commands[at..] {
        if let Command::Glyphs { text, y, size, .. } = c {
            close(*size, 12.0);
            match lines.iter_mut().find(|l| (l.0 - *y).abs() < 0.01) {
                Some(l) => l.1.push_str(text),
                None => lines.push((*y, text.clone())),
            }
        }
    }
    let texts_of: Vec<&str> = lines.iter().map(|l| l.1.as_str()).collect();
    assert_eq!(texts_of, ["First speaker note", "Second paragraph"]);
    // First baseline: one 12 pt line under the 3.6 pt top inset; the second
    // paragraph a single-spaced 12 pt line lower.
    assert!(lines[0].0 > 3.6 + 12.0 * 0.7 && lines[0].0 < 3.6 + 12.0 * 1.3, "{lines:?}");
    let pitch = lines[1].0 - lines[0].0;
    assert!(pitch > 12.0 && pitch < 16.0, "{lines:?}");
    // Slides without notes have no notes text; the PDF has one page per slide.
    assert!(texts(&list.pages[1]).iter().all(|t| !t.contains("speaker")));
    let pdf = e.export_slides(&SlideExport::Notes(o)).unwrap();
    assert_eq!(pdf_pages(&pdf), 3);
    // A4 scales the master boxes: x × 595.2756 / 540, y × 841.8898 / 720.
    let a4 = e.notes_pages(&NotesOptions::default()).unwrap();
    let p = placements(&a4.pages[0]);
    close(p[0][1], 54.0 * 595.2756 / 540.0);
    // sldImg box 476.22 × 284.14 → width-limited 16:9 frame 476.22 × 267.87, centred.
    let w = 432.0 * 595.2756 / 540.0;
    let h = 243.0 * 841.8898 / 720.0;
    close(
        p[0][2],
        90.0 * 841.8898 / 720.0 + (h - w * 9.0 / 16.0) / 2.0,
    );
}

/// Text runs of a page with their size, colour and baseline (local to any transform).
fn runs(page: &PageList) -> Vec<(String, f32, Rgba, f32)> {
    page.commands
        .iter()
        .filter_map(|c| match c {
            Command::Glyphs {
                text,
                size,
                color,
                y,
                ..
            } => Some((text.clone(), *size, *color, *y)),
            _ => None,
        })
        .collect()
}

/// Notes of slide 1 replaced by a bold red 20 pt run and `n` plain lines.
fn rich_notes(e: &mut Engine, n: usize) {
    use bo_model::slide::{Color, Fill, Paragraph, RunProps, ShapeNode, TextRun};
    let pres = e.presentation_mut().unwrap();
    let notes = pres.slides[0].notes.as_mut().unwrap();
    let body = notes
        .data
        .shapes
        .iter_mut()
        .find_map(|n| match n {
            ShapeNode::Shape(s)
                if s.nv.placeholder.as_ref().and_then(|p| p.kind.as_deref()) == Some("body") =>
            {
                s.text.as_mut()
            }
            _ => None,
        })
        .unwrap();
    let mut paras = vec![Paragraph {
        runs: vec![
            TextRun::Run {
                props: RunProps {
                    b: Some(true),
                    sz: Some(2000),
                    fill: Some(Fill::Solid(Color::srgb(0xFF_00_00))),
                    ..RunProps::default()
                },
                text: "Bold red".into(),
            },
            TextRun::Run {
                props: RunProps {
                    i: Some(true),
                    ..RunProps::default()
                },
                text: " then italic".into(),
            },
        ],
        ..Paragraph::default()
    }];
    for k in 1..=n {
        paras.push(Paragraph {
            runs: vec![TextRun::Run {
                props: RunProps::default(),
                text: format!("Line {k}"),
            }],
            ..Paragraph::default()
        });
    }
    body.paragraphs = paras;
}

#[test]
fn notes_pages_keep_run_formatting() {
    let mut e = deck(1);
    rich_notes(&mut e, 0);
    let o = NotesOptions {
        paper: Paper::Notes,
        ..NotesOptions::default()
    };
    let list = e.notes_pages(&o).unwrap();
    assert_eq!(list.pages.len(), 1);
    let r = runs(&list.pages[0]);
    let bold = r.iter().find(|x| x.0.contains("Bold")).unwrap();
    // The run's own 20 pt and red; the italic run takes the notes style's 12 pt, black (tx1).
    close(bold.1, 20.0);
    assert_eq!(bold.2, Rgba(255, 0, 0, 255));
    let it = r.iter().find(|x| x.0.contains("italic")).unwrap();
    close(it.1, 12.0);
    assert_eq!(it.2, Rgba(0, 0, 0, 255));
    // The notes are drawn into the body box (54, 346.5) of the notes master.
    let m = list.pages[0]
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::PushTransform { m } => Some(*m),
            _ => None,
        })
        .last()
        .unwrap();
    close(m[4], 54.0);
    close(m[5], 346.5);
}

#[test]
fn long_notes_flow_onto_further_pages() {
    let mut e = deck(2);
    rich_notes(&mut e, 60);
    let o = NotesOptions {
        paper: Paper::Notes,
        ..NotesOptions::default()
    };
    let list = e.notes_pages(&o).unwrap();
    // 60 lines at 12 pt do not fit the 283.5 pt notes box: slide 1 takes more
    // than one page, slide 2 one.
    assert!(list.pages.len() >= 3, "{}", list.pages.len());
    let slide1 = list.pages.len() - 1;
    // Every line is drawn exactly once, in order, across slide 1's pages.
    let mut seen = Vec::new();
    for p in &list.pages[..slide1] {
        // Runs may split at spaces: join them, then read "Line N" back.
        let all: String = runs(p).into_iter().map(|r| r.0).collect();
        for part in all.split("Line").skip(1) {
            let n: String = part
                .trim_start()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            seen.push(n.parse::<usize>().unwrap_or_else(|_| panic!("{all}")));
        }
    }
    assert_eq!(seen, (1..=60).collect::<Vec<_>>());
    // Continuation pages show no slide picture, only notes: their text box
    // starts at the slide image's top (90 pt) and every line stays above the
    // body box's bottom (346.5 + 283.5 = 630 pt).
    for p in &list.pages[1..slide1] {
        assert!(placements(p).is_empty());
        let clip = p
            .commands
            .iter()
            .find_map(|c| match c {
                Command::PushClip { y, h, .. } => Some((*y, *h)),
                _ => None,
            })
            .unwrap();
        close(clip.0, 90.0);
        assert!(clip.0 + clip.1 <= 630.5 + 0.5, "{clip:?}");
    }
    // The first page: the slide, then lines clipped to the body box.
    assert_eq!(placements(&list.pages[0]).len(), 1);
    let pdf = e.export_slides(&SlideExport::Notes(o)).unwrap();
    assert_eq!(pdf_pages(&pdf), list.pages.len());
}

#[test]
fn landscape_handouts() {
    let mut e = deck(6);
    let o = HandoutOptions {
        per_page: 6,
        landscape: Some(true),
        ..HandoutOptions::default()
    };
    let list = e.handout_pages(&o).unwrap();
    assert_eq!(list.pages.len(), 1);
    let page = &list.pages[0];
    // A4 turned: 841.8898 × 595.2756 pt.
    close(page.width, 841.8898);
    close(page.height, 595.2756);
    // Three columns of two rows (handout::tests::landscape_pages): slide 3 at the
    // top right, x = 36 + 2 × (244.6299 + 18) = 561.2598; scale 244.6299 / 960.
    let p = placements(page);
    assert_eq!(p.len(), 6);
    close(p[0][0], 244.6299 / 960.0);
    close(p[2][1], 561.2598);
    close(p[2][2], 102.5168);
    close(p[3][1], 36.0);
    let pdf = e
        .export_slides_json(r#"{"kind":"handout","perPage":2,"landscape":true}"#)
        .unwrap();
    assert_eq!(pdf_pages(&pdf), 3);
}

#[test]
fn package_copies_local_linked_clips_only() {
    use bo_engine::slide_export::PackageOptions;
    use bo_model::slide::Rel;
    let dir = std::env::temp_dir().join(format!("bo-pkg-links-{}", std::process::id()));
    let clips = dir.join("clips");
    std::fs::create_dir_all(&clips).unwrap();
    std::fs::write(clips.join("intro.mp4"), b"linked video").unwrap();
    let abs = dir.join("narration.wav");
    std::fs::write(&abs, b"linked audio").unwrap();
    let mut e = deck(1);
    let video = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video";
    let audio = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio";
    let rels = &mut e.presentation_mut().unwrap().slides[0].info.rels;
    for (id, ty, target) in [
        ("rIdL1", video, "clips/intro.mp4".to_owned()),
        (
            "rIdL2",
            audio,
            format!("file:///{}", abs.to_string_lossy().replace('\\', "/")),
        ),
        ("rIdL3", video, "https://example.com/remote.mp4".to_owned()),
        ("rIdL4", video, "missing.mp4".to_owned()),
    ] {
        rels.push(Rel {
            id: id.into(),
            rel_type: ty.into(),
            target,
            external: true,
        });
    }
    let zip = e
        .export_slides(&SlideExport::Package(PackageOptions {
            link_base: Some(dir.to_string_lossy().into_owned()),
        }))
        .unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let read = |z: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>, n: &str| {
        let mut v = Vec::new();
        z.by_name(n).unwrap().read_to_end(&mut v).unwrap();
        v
    };
    // The relative link keeps its place beside the deck; the absolute one goes to media/.
    assert_eq!(read(&mut z, "clips/intro.mp4"), b"linked video");
    assert_eq!(read(&mut z, "media/narration.wav"), b"linked audio");
    let links = String::from_utf8(read(&mut z, "links.txt")).unwrap();
    assert!(links.contains("https://example.com/remote.mp4"), "{links}");
    assert!(links.contains("missing.mp4"), "{links}");
    assert!(links.contains("-> media/narration.wav"), "{links}");
    assert!(!links.contains("clips/intro.mp4"), "{links}");
    // Without the deck's folder nothing is read.
    let zip = e
        .export_slides_json(r#"{"kind":"package"}"#)
        .unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    assert!(z.by_name("clips/intro.mp4").is_err());
    let links = String::from_utf8(read(&mut z, "links.txt")).unwrap();
    assert!(links.contains("clips/intro.mp4"), "{links}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn png_size(png: &[u8]) -> (u32, u32) {
    // PNG §11.2.2 IHDR: width and height follow the 8-byte signature and the chunk header.
    let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
    (w, h)
}

#[test]
fn slide_images_png_jpeg() {
    let mut e = deck(2);
    let png = e
        .export_slides(&SlideExport::Image(ImageOptions {
            slides: Some(vec![0]),
            width: 640,
            ..ImageOptions::default()
        }))
        .unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    assert_eq!(png_size(&png), (640, 360));
    let jpg = e.slide_image(0, ImageFormat::Jpg, 640, 90).unwrap();
    assert_eq!(bo_render::image::jpeg_info(&jpg), Some((640, 360, 3)));
    // Decoded: the red background survives JPEG coding within a small error.
    let opts = zune_jpeg::zune_core::options::DecoderOptions::default()
        .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGB);
    let mut dec = zune_jpeg::JpegDecoder::new_with_options(jpg.as_slice(), opts);
    let px = dec.decode().unwrap();
    let at = |x: usize, y: usize| &px[(y * 640 + x) * 3..(y * 640 + x) * 3 + 3];
    for (x, y) in [(10, 10), (600, 340), (320, 300)] {
        let p = at(x, y);
        assert!(p[0] > 240 && p[1] < 16 && p[2] < 16, "({x},{y}) {p:?}");
    }
    // All slides → a zip of slide1.jpg, slide2.jpg.
    let zip = e
        .export_slides(&SlideExport::Image(ImageOptions {
            format: ImageFormat::Jpg,
            width: 320,
            ..ImageOptions::default()
        }))
        .unwrap();
    let z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let mut names: Vec<String> = z.file_names().map(|n| n.unwrap().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["slide1.jpg", "slide2.jpg"]);
}

#[test]
fn slide_svg_is_well_formed() {
    let mut e = deck(1);
    let svg = e
        .export_slides_json(r#"{"kind":"image","format":"svg","slides":[0],"width":960}"#)
        .unwrap();
    let s = String::from_utf8(svg).unwrap();
    let root = bo_drawingml::dom::parse(s.as_bytes(), "slide1.svg").unwrap();
    assert_eq!(root.local(), "svg");
    assert_eq!(root.attr("viewBox"), Some("0 0 960 540"));
    assert_eq!(root.attr("height"), Some("540"));
    // "Hello World": 10 visible glyphs placed with <use>, 7 distinct outlines
    // (H e l o W r d) defined once, one invisible <text> per glyph run.
    assert_eq!(s.matches("<use ").count(), 10, "{s}");
    assert_eq!(s.matches("<path id=\"g").count(), 7);
    let runs = texts(e.slide(0).unwrap());
    assert_eq!(s.matches("<text ").count(), runs.len());
    assert_eq!(runs.concat().trim(), "Hello World");
    for r in runs {
        assert!(s.contains(&format!(">{r}</text>")), "{r}");
    }
    // The red background.
    assert!(s.contains("#ff0000"));
}

#[test]
fn presentation_package_contents() {
    let mut e = deck(2);
    e.slide_insert_picture(b"not really a video".to_vec(), "clip.mp4")
        .unwrap();
    let zip = e.export_slides(&SlideExport::Package(Default::default())).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let mut names: Vec<String> = z.file_names().map(|n| n.unwrap().into_owned()).collect();
    names.sort();
    assert!(names.contains(&"deck.pptx".to_owned()), "{names:?}");
    // The built deck's theme font is Arial, drawn with Liberation Sans (OFL) → font + licence.
    assert!(
        names.contains(&"fonts/LiberationSans-Regular.ttf".to_owned()),
        "{names:?}"
    );
    assert!(
        names.contains(&"fonts/OFL-Liberation.txt".to_owned()),
        "{names:?}"
    );
    assert_eq!(names.iter().filter(|n| n.starts_with("fonts/")).count(), 2);
    // Apache-licensed and unused faces are not packaged.
    assert!(!names.iter().any(|n| n.starts_with("fonts/Tinos")));
    let media: Vec<_> = names.iter().filter(|n| n.starts_with("media/")).collect();
    assert_eq!(media.len(), 1, "{names:?}");
    let mut clip = Vec::new();
    z.by_name(media[0]).unwrap().read_to_end(&mut clip).unwrap();
    assert_eq!(clip, b"not really a video");
    // The packaged deck opens again.
    let mut pptx = Vec::new();
    z.by_name("deck.pptx")
        .unwrap()
        .read_to_end(&mut pptx)
        .unwrap();
    let mut again = Engine::with_bundled_fonts();
    assert_eq!(again.open_pptx(&pptx, "deck.pptx").unwrap().page_count, 2);
}

#[test]
fn bad_json_is_rejected() {
    let mut e = deck(1);
    assert!(e.export_slides_json("{\"kind\":\"poster\"}").is_err());
    let pdf = e
        .export_slides_json(r#"{"kind":"handout","perPage":4,"order":"vertical","paper":"letter"}"#)
        .unwrap();
    assert_eq!(pdf_pages(&pdf), 1);
}
