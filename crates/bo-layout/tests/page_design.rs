//! Page colour (ECMA-376 Part 1, §17.2.1), page borders (§17.6.10 pgBorders: line
//! and art styles, offsetFrom, display) and watermarks (VML text path / picture in
//! the header, Part 4 §19.1.2.23 / §19.1.2.11) on every page.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, Layer, LayoutCache, LayoutDocument, PageItem, PathCmd};
use bo_model::{
    Anchor, Block, Border, Color, Document, Emu, HalfPoints, Image, PageBorder, PageBorderDisplay,
    PageBorders, ParaProps, Paragraph, Run, RunContent, RunProps, Section, Shape, ShapeArt,
    TextPath, Wrap,
};
use bo_text::FontManager;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

fn para(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run::text(text, RunProps::default())],
    ))
}

/// A document of `paras` paragraphs (60 make about three A4 pages at 12 pt).
fn doc(paras: usize) -> Document {
    let blocks = (0..paras)
        .map(|i| {
            para(&format!(
                "Đoạn văn số {i} của tài liệu thử nghiệm viền trang và nền."
            ))
        })
        .collect();
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(24)),
            ..Default::default()
        },
        sections: vec![Section::new(blocks)],
        ..Default::default()
    }
}

fn layout(d: &Document) -> LayoutDocument {
    layout_document(d, &mut fonts(), &mut LayoutCache::new()).unwrap()
}

fn edge(style: &str, size: u32, space: u32) -> PageBorder {
    PageBorder {
        line: Border {
            style: style.into(),
            size,
            color: None,
            space,
        },
        shadow: false,
        frame: false,
    }
}

fn boxed(style: &str, size: u32, space: u32, from_page: bool) -> PageBorders {
    let e = Some(edge(style, size, space));
    PageBorders {
        top: e.clone(),
        left: e.clone(),
        bottom: e.clone(),
        right: e,
        offset_from_page: from_page,
        ..PageBorders::default()
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// Rules of the page borders: (x1, y1, x2, y2, width).
fn rules(page: &bo_layout::Page) -> Vec<(f32, f32, f32, f32, f32)> {
    page.decor
        .borders
        .iter()
        .filter_map(|i| match i {
            PageItem::Rule {
                x1,
                y1,
                x2,
                y2,
                width,
                ..
            } => Some((*x1, *y1, *x2, *y2, *width)),
            _ => None,
        })
        .collect()
}

#[test]
fn line_borders_offset_from_text() {
    // Default A4 page: 595.3 × 841.9 pt, margins top/bottom 56.7, left 85.05,
    // right 42.5 pt. A 1 pt line (sz 8) 24 pt from the text: its centre line is
    // 24 + 0.5 pt outside the text area.
    let mut d = doc(3);
    d.sections[0].page_borders = Some(boxed("single", 8, 24, false));
    let l = layout(&d);
    let r = rules(&l.pages[0]);
    assert_eq!(r.len(), 4);
    let (top, left, bottom, right) = (32.2, 60.55, 809.7, 577.3);
    // Top: from the outer edge of the left line to the outer edge of the right one.
    let t = r[0];
    assert!(close(t.1, top) && close(t.3, top), "{t:?}");
    assert!(close(t.0, left - 0.5) && close(t.2, right + 0.5), "{t:?}");
    assert!(close(t.4, 1.0));
    let b = r[1];
    assert!(close(b.1, bottom) && close(b.3, bottom), "{b:?}");
    let lf = r[2];
    assert!(close(lf.0, left) && close(lf.2, left), "{lf:?}");
    assert!(
        close(lf.1, top - 0.5) && close(lf.3, bottom + 0.5),
        "{lf:?}"
    );
    let rt = r[3];
    assert!(close(rt.0, right) && close(rt.2, right), "{rt:?}");
    // In front of the text by default (§17.18.65 front).
    assert!(!l.pages[0].decor.borders_behind);
}

#[test]
fn line_borders_offset_from_page_edge() {
    // 3 pt double line (sz 24) 20 pt from the page edge: centre 21.5 pt in.
    let mut d = doc(3);
    let mut pb = boxed("double", 24, 20, true);
    pb.behind = true;
    d.sections[0].page_borders = Some(pb);
    let l = layout(&d);
    let r = rules(&l.pages[0]);
    assert!(close(r[0].1, 21.5));
    assert!(close(r[1].1, 841.9 - 21.5));
    assert!(close(r[2].0, 21.5));
    assert!(close(r[3].0, 595.3 - 21.5));
    assert!(l.pages[0].decor.borders_behind);
    // The style reaches the renderer (two strokes for double).
    assert!(l.pages[0]
        .decor
        .borders
        .iter()
        .all(|i| matches!(i, PageItem::Rule { style, .. } if style == "double")));
}

#[test]
fn art_border_repeats_its_motif() {
    // basicBlackDots, 12 pt (art sz in points), 24 pt from the page edge: centre
    // lines at 30 pt in. The top edge runs from 24 to 571.3 (547.3 pt): 46 tiles of
    // 11.898 pt; the left edge between the corner tiles from 36 to 805.9 (769.9 pt):
    // 64 tiles.
    let mut d = doc(3);
    d.sections[0].page_borders = Some(boxed("basicBlackDots", 12, 24, true));
    let l = layout(&d);
    let shapes: Vec<&Vec<bo_layout::ShapePath>> = l.pages[0]
        .decor
        .borders
        .iter()
        .filter_map(|i| match i {
            PageItem::Shape { paths, .. } => Some(paths),
            _ => None,
        })
        .collect();
    let counts: Vec<usize> = shapes.iter().map(|p| p.len()).collect();
    assert_eq!(counts, vec![46, 46, 64, 64]);
    // First dot of the top edge: centre (24 + 11.898 / 2, 30), radius 0.6 × 5.949;
    // the circle starts at its rightmost point.
    let step = 547.3 / 46.0;
    let r = step / 2.0 * 0.6;
    match shapes[0][0].segs[0] {
        PathCmd::M(x, y) => {
            assert!(close(x, 24.0 + step / 2.0 + r), "{x}");
            assert!(close(y, 30.0), "{y}");
        }
        ref s => panic!("{s:?}"),
    }
    assert!(shapes[0][0].fill.is_some());
}

#[test]
fn unknown_art_border_falls_back_to_a_line() {
    // `apples` has no BlueOffice motif: a plain line of a sixth of the art width.
    let mut d = doc(3);
    d.sections[0].page_borders = Some(boxed("apples", 18, 10, true));
    let l = layout(&d);
    let r = rules(&l.pages[0]);
    assert_eq!(r.len(), 4);
    assert!(r.iter().all(|r| close(r.4, 3.0)));
    // Centre line 10 + 9 pt from the page edge (the art width sets the position).
    assert!(close(r[0].1, 19.0));
}

#[test]
fn display_first_and_not_first_page() {
    let mut d = doc(80);
    let mut pb = boxed("single", 4, 24, false);
    pb.display = PageBorderDisplay::FirstPage;
    d.sections[0].page_borders = Some(pb.clone());
    let l = layout(&d);
    assert!(l.pages.len() >= 2);
    assert_ne!(l.pages[0].decor.borders.len(), 0);
    assert!(l.pages[1..].iter().all(|p| p.decor.borders.is_empty()));
    pb.display = PageBorderDisplay::NotFirstPage;
    d.sections[0].page_borders = Some(pb);
    let l = layout(&d);
    assert_eq!(l.pages[0].decor.borders.len(), 0);
    assert!(l.pages[1..].iter().all(|p| !p.decor.borders.is_empty()));
}

#[test]
fn page_colour_on_every_page() {
    let mut d = doc(80);
    let c = Color {
        r: 0xFD,
        g: 0xF6,
        b: 0xE3,
    };
    d.design.background = Some(c);
    let l = layout(&d);
    assert!(l.pages.len() >= 2);
    assert!(l.pages.iter().all(|p| p.decor.background == Some(c)));
}

/// A watermark shape anchored in the header, centred on the margins.
fn watermark(art: ShapeArt, w: f32, h: f32, rotation: i32) -> Run {
    Run {
        props: RunProps::default(),
        content: vec![RunContent::Image(Box::new(Image {
            media: String::new(),
            width: Emu::from_pt(w),
            height: Emu::from_pt(h),
            anchor: Some(Anchor {
                h_relative: "margin".into(),
                v_relative: "margin".into(),
                h_align: Some("center".into()),
                v_align: Some("center".into()),
                wrap: Wrap::None,
                behind: true,
                ..Anchor::default()
            }),
            crop: None,
            rotation,
            description: String::new(),
            shape: Some(Box::new(Shape {
                preset: "rect".into(),
                fill: Some(Color {
                    r: 192,
                    g: 192,
                    b: 192,
                }),
                art: Some(Box::new(art)),
                ..Shape::default()
            })),
            format: None,
            src: None,
            ole: None,
        }))],
    }
}

#[test]
fn text_watermark_on_every_page() {
    let mut d = doc(80);
    let art = ShapeArt {
        text: Some(TextPath {
            text: "BẢN NHÁP".into(),
            font: "Calibri".into(),
            ..TextPath::default()
        }),
        opacity: 500,
        watermark: true,
        ..ShapeArt::default()
    };
    let header = Paragraph::new(
        ParaProps::default(),
        vec![watermark(art, 400.0, 100.0, 315 * 60_000)],
    );
    d.sections[0].headers.default = Some(vec![Block::Paragraph(header)]);
    let l = layout(&d);
    assert!(l.pages.len() >= 2);
    // Margins box: x 85.05 .. 552.8 (467.75 wide), y 56.7 .. 785.2 (728.5 high).
    let (bx, by) = (85.05 + (467.75 - 400.0) / 2.0, 56.7 + (728.5 - 100.0) / 2.0);
    for p in &l.pages {
        let wm: Vec<_> = p
            .items
            .iter()
            .filter_map(|i| match i {
                PageItem::Shape {
                    x,
                    y,
                    paths,
                    layer,
                    alpha,
                    ..
                } => Some((*x, *y, paths, *layer, *alpha)),
                _ => None,
            })
            .collect();
        assert_eq!(wm.len(), 1, "one watermark per page");
        let (x, y, paths, layer, alpha) = wm[0];
        assert!(close(x, bx) && close(y, by), "({x}, {y})");
        assert_eq!(layer, Layer::Back);
        assert!(close(alpha, 0.5));
        assert!(paths[0].segs.len() > 20, "glyph outlines");
        // Rotated 315° about the box centre: the outline leaves the box upwards and
        // downwards (a horizontal 100 pt band would not).
        let ys: Vec<f32> = paths[0]
            .segs
            .iter()
            .filter_map(|s| match s {
                PathCmd::M(_, y) | PathCmd::L(_, y) => Some(*y),
                _ => None,
            })
            .collect();
        let (lo, hi) = ys
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &y| (a.min(y), b.max(y)));
        assert!(lo < -50.0 && hi > 150.0, "{lo} {hi}");
    }
}

#[test]
fn picture_watermark_is_washed_out() {
    let mut d = doc(3);
    let art = ShapeArt {
        picture: Some("/word/media/logo.png".into()),
        washout: true,
        watermark: true,
        ..ShapeArt::default()
    };
    let header = Paragraph::new(ParaProps::default(), vec![watermark(art, 200.0, 150.0, 0)]);
    d.sections[0].headers.default = Some(vec![Block::Paragraph(header)]);
    let l = layout(&d);
    let img = l.pages[0]
        .items
        .iter()
        .find_map(|i| match i {
            PageItem::Image {
                media, alpha, w, h, ..
            } => Some((media.clone(), *alpha, *w, *h)),
            _ => None,
        })
        .unwrap();
    assert_eq!(img.0, "/word/media/logo.png");
    assert!(close(img.1, 0.3));
    assert!(close(img.2, 200.0) && close(img.3, 150.0));
}
