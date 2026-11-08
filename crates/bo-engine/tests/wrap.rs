//! Text flowing around square/tight-wrapped pictures (ECMA-376 Part 1, §20.4.2.17
//! wrapSquare, §20.4.2.20 wrapTight): layout snapshots of the corpus files
//! `tests/corpus/doc/images-0{4,5,6}-*.docx` (PNG: `bo-cli render <file>`).

use bo_engine::Engine;
use bo_layout::PageItem;
use std::fmt::Write as _;

const FILES: &[&str] = &[
    "images-04-wrap-square-sides",
    "images-05-wrap-tight",
    "images-06-wrap-flow",
];

/// Pictures and line segments (x range and text of each fragment run) per page.
fn summary(e: &Engine) -> String {
    let mut s = String::new();
    for (pi, page) in e.layout().expect("layout").pages.iter().enumerate() {
        let _ = writeln!(s, "page {}", pi + 1);
        for it in &page.items {
            if let PageItem::Image { x, y, w, h, .. } = it {
                let _ = writeln!(s, "  image {x:.1},{y:.1} {w:.1}x{h:.1}");
            }
        }
        for l in &page.lines {
            let segs: Vec<String> = l
                .fragments
                .iter()
                .map(|f| format!("[{:.1}–{:.1}] {}", f.x, f.x + f.width, f.text.trim()))
                .collect();
            let _ = writeln!(s, "  line {:.1}: {}", l.top, segs.join(" | "));
        }
    }
    s
}

#[test]
fn wrapped_pictures_layout_snapshots() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/doc");
    let mut e = Engine::with_bundled_fonts();
    for name in FILES {
        let bytes = std::fs::read(dir.join(format!("{name}.docx"))).expect("corpus file");
        e.open(&bytes, &format!("{name}.docx")).expect("open");
        insta::assert_snapshot!(*name, summary(&e));
    }
}

#[test]
fn wrap_settings_survive_save() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/doc");
    for name in FILES {
        let bytes = std::fs::read(dir.join(format!("{name}.docx"))).expect("corpus file");
        let doc = bo_docx::read_docx(&bytes).expect("read");
        // Writing from the model (as after an edit) keeps wrapText, distances and the polygon.
        let mut edited = doc.clone();
        for s in &mut edited.sections {
            for b in &mut s.blocks {
                if let bo_model::Block::Paragraph(p) = b {
                    for r in &mut p.runs {
                        for c in &mut r.content {
                            if let bo_model::RunContent::Image(img) = c {
                                img.src = None;
                            }
                        }
                    }
                }
            }
        }
        let again =
            bo_docx::read_docx(&bo_docx::write_docx(&edited).expect("write")).expect("read");
        let anchors = |d: &bo_model::Document| -> Vec<bo_model::Anchor> {
            d.sections
                .iter()
                .flat_map(|s| s.blocks.iter())
                .filter_map(|b| match b {
                    bo_model::Block::Paragraph(p) => Some(p),
                    _ => None,
                })
                .flat_map(|p| p.runs.iter().flat_map(|r| r.content.iter()))
                .filter_map(|c| match c {
                    bo_model::RunContent::Image(img) => img.anchor.clone(),
                    _ => None,
                })
                .collect()
        };
        let (a, b) = (anchors(&doc), anchors(&again));
        assert!(!a.is_empty(), "{name}");
        if *name == "images-05-wrap-tight" {
            assert!(
                a.iter().any(|x| x.polygon_points.len() >= 4),
                "{name}: polygon points read"
            );
        }
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(
                (x.wrap, x.wrap_text, x.dist, x.polygon, &x.polygon_points),
                (y.wrap, y.wrap_text, y.dist, y.polygon, &y.polygon_points),
                "{name}"
            );
        }
    }
}
