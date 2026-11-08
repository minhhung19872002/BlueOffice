//! Text boxes and shapes (§17.17.1, §20.1.9), run font slots and theme fonts
//! (§17.3.2.26), kerning (§17.3.2.19), bidirectional text (§17.3.1.6) and soft
//! hyphens (§17.3.3.29).

use bo_layout::{layout_document, LayoutCache, LayoutDocument, LineSource, PageItem};
use bo_model::{
    Alignment, Anchor, Block, Color, Document, Emu, HalfPoints, Image, ParaProps, Paragraph, Run,
    RunContent, RunProps, Section, Shape, ShapeLine, TextAnchor, ThemeFonts, Wrap,
};
use bo_text::FontManager;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

fn doc(blocks: Vec<Block>) -> Document {
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

fn run(d: &Document, fm: &mut FontManager) -> LayoutDocument {
    layout_document(d, fm, &mut LayoutCache::new()).unwrap()
}

fn para(runs: Vec<Run>) -> Block {
    Block::Paragraph(Paragraph::new(ParaProps::default(), runs))
}

fn text_box(anchor: Option<Anchor>, preset: &str, text: &str) -> RunContent {
    RunContent::Image(Box::new(Image {
        media: String::new(),
        width: Emu(2_540_000),  // 200 pt
        height: Emu(1_270_000), // 100 pt
        anchor,
        crop: None,
        rotation: 0,
        description: String::new(),
        shape: Some(Box::new(Shape {
            preset: preset.into(),
            fill: Some(Color {
                r: 255,
                g: 255,
                b: 0,
            }),
            line: Some(ShapeLine {
                width: Emu(12_700),
                color: Color::BLACK,
                dash: None,
            }),
            text: Some(vec![para(vec![Run::text(text, RunProps::default())])]),
            insets: Shape::INSETS,
            anchor: TextAnchor::Center,
            ..Shape::default()
        })),
        src: None,
    }))
}

#[test]
fn anchored_text_box_draws_its_shape_and_text() {
    let mut fm = fonts();
    let anchor = Anchor {
        h_relative: "page".into(),
        v_relative: "page".into(),
        x: Emu(1_270_000),
        y: Emu(2_540_000),
        wrap: Wrap::None,
        ..Default::default()
    };
    let d = doc(vec![
        para(vec![
            Run::text("Trước ", RunProps::default()),
            Run {
                props: RunProps::default(),
                content: vec![text_box(Some(anchor), "roundRect", "Hộp chữ tiếng Việt")],
            },
        ]),
        para(vec![Run::text("Đoạn sau", RunProps::default())]),
    ]);
    let l = run(&d, &mut fm);
    let page = &l.pages[0];
    let shape = page.items.iter().find_map(|i| match i {
        PageItem::Shape {
            x, y, w, h, paths, ..
        } => Some((*x, *y, *w, *h, paths.clone())),
        _ => None,
    });
    let (x, y, w, h, paths) = shape.expect("shape item");
    assert_eq!((x, y, w, h), (100.0, 200.0, 200.0, 100.0));
    assert!(paths[0].fill.is_some() && paths[0].stroke.is_some());
    // Rounded rectangle: curves in the outline.
    assert!(paths[0]
        .segs
        .iter()
        .any(|s| matches!(s, bo_layout::PathCmd::C(..) | bo_layout::PathCmd::Q(..))));
    let boxed: Vec<_> = page
        .lines
        .iter()
        .filter(|l| matches!(l.source, LineSource::TextBox { .. }))
        .collect();
    assert_eq!(boxed.len(), 1);
    let t: String = boxed[0].fragments.iter().map(|f| f.text.as_str()).collect();
    assert_eq!(t, "Hộp chữ tiếng Việt");
    // Inside the box, vertically centred.
    assert!(boxed[0].top > 220.0 && boxed[0].top + boxed[0].height < 280.0);
    let LineSource::TextBox { para, at, sub } = boxed[0].source else {
        unreachable!()
    };
    assert_eq!((para, at, sub), (0, 9, 0)); // "Trước " is 9 bytes
                                            // Body text keeps flowing (wrap none).
    assert!(l.page_text(0).contains("Đoạn sau"));
}

#[test]
fn inline_shape_sits_on_the_baseline() {
    let mut fm = fonts();
    let d = doc(vec![para(vec![Run {
        props: RunProps::default(),
        content: vec![
            RunContent::Text("A ".into()),
            text_box(None, "ellipse", "Elip"),
        ],
    }])]);
    let l = run(&d, &mut fm);
    let page = &l.pages[0];
    let line = page
        .lines
        .iter()
        .find(|l| l.source == LineSource::Body(0))
        .unwrap();
    assert!(line.height >= 100.0);
    let (y, h) = page
        .items
        .iter()
        .find_map(|i| match i {
            PageItem::Shape { y, h, .. } => Some((*y, *h)),
            _ => None,
        })
        .unwrap();
    assert!((y + h - line.baseline).abs() < 0.01);
    assert!(page
        .lines
        .iter()
        .any(|l| matches!(l.source, LineSource::TextBox { para: 0, .. })));
}

#[test]
fn font_slots_and_theme_fonts() {
    let mut fm = fonts();
    // ascii Arial, hAnsi Times New Roman: "a" in Liberation Sans, "ă" in Tinos.
    let rp = RunProps {
        font: Some("Arial".into()),
        font_h_ansi: Some("Times New Roman".into()),
        ..Default::default()
    };
    let d = doc(vec![para(vec![Run::text("aă", rp)])]);
    let l = run(&d, &mut fm);
    let fams: Vec<String> = l.pages[0].lines[0]
        .fragments
        .iter()
        .map(|f| fm.info(f.face).family.clone())
        .collect();
    assert_eq!(fams, vec!["Liberation Sans", "Tinos"]);

    // Theme fonts (§17.18.96): minorHAnsi → the theme's minor Latin font.
    let mut d = doc(vec![para(vec![Run::text(
        "Theme",
        RunProps {
            theme_ascii: Some("minorHAnsi".into()),
            font: Some("Times New Roman".into()),
            ..Default::default()
        },
    )])]);
    d.theme_fonts = ThemeFonts {
        minor_latin: "Calibri".into(),
        ..Default::default()
    };
    let l = run(&d, &mut fm);
    let f = &l.pages[0].lines[0].fragments[0];
    assert_eq!(fm.info(f.face).family, "Carlito");

    // eastAsia slot for CJK, cs slot for a complex script run.
    let rp = RunProps {
        font: Some("Arial".into()),
        font_east_asia: Some("Calibri".into()),
        font_cs: Some("Times New Roman".into()),
        ..Default::default()
    };
    let d = doc(vec![para(vec![
        Run::text("a\u{4E00}", rp.clone()),
        Run::text(
            "b",
            RunProps {
                complex: Some(true),
                ..rp
            },
        ),
    ])]);
    let l = run(&d, &mut fm);
    let frags = &l.pages[0].lines[0].fragments;
    assert_eq!(fm.info(frags[0].face).family, "Liberation Sans");
    assert_eq!(fm.info(frags.last().unwrap().face).family, "Tinos");
}

#[test]
fn kerning_only_from_the_threshold() {
    let mut fm = fonts();
    let width = |kern: Option<u32>, fm: &mut FontManager| {
        let d = doc(vec![para(vec![Run::text(
            "AVAVAVAV",
            RunProps {
                kern: kern.map(HalfPoints),
                ..Default::default()
            },
        )])]);
        let l = run(&d, fm);
        l.pages[0].lines[0].fragments[0].width
    };
    let none = width(None, &mut fm);
    let on = width(Some(2), &mut fm);
    let above = width(Some(48), &mut fm); // 24 pt threshold > 12 pt text: no kerning
    assert!(on < none - 1.0, "{on} vs {none}");
    assert!((above - none).abs() < 0.01);
}

#[test]
fn right_to_left_runs_are_reordered() {
    let mut fm = fonts();
    // Hebrew (strong R) between Latin words: visual order "abc", Hebrew reversed, "xyz".
    let d = doc(vec![para(vec![Run::text(
        "abc \u{05D0}\u{05D1}\u{05D2} \u{05D3}\u{05D4} xyz",
        RunProps::default(),
    )])]);
    let l = run(&d, &mut fm);
    let line = &l.pages[0].lines[0];
    let mut frags: Vec<_> = line.fragments.iter().collect();
    frags.sort_by(|a, b| a.x.total_cmp(&b.x));
    let order: String = frags.iter().map(|f| f.text.as_str()).collect();
    let second = order.find('\u{05D3}').unwrap();
    let first = order.find('\u{05D0}').unwrap();
    assert!(second < first, "{order}");
    // Carets inside the first Hebrew word are right of those of the second one and
    // move leftwards.
    let x_of = |off: u32| {
        line.carets
            .iter()
            .find(|c| c.0 == off)
            .map(|c| c.1)
            .unwrap()
    };
    assert!(x_of(6) > x_of(13));
    assert!(x_of(8) < x_of(6));

    // A bidi paragraph (§17.3.1.6) is right aligned by default.
    let d = doc(vec![Block::Paragraph(Paragraph::new(
        ParaProps {
            bidi: Some(true),
            ..Default::default()
        },
        vec![Run::text(
            "\u{05E9}\u{05DC}\u{05D5}\u{05DD}",
            RunProps::default(),
        )],
    ))]);
    let l = run(&d, &mut fm);
    let f = &l.pages[0].lines[0].fragments[0];
    let s = bo_model::PageSetup::default();
    let right = s.width.to_pt() - s.margin_right.to_pt();
    assert!(
        (f.x + f.width - right).abs() < 1.0,
        "{} {}",
        f.x + f.width,
        right
    );
    let _ = Alignment::Right;
}

#[test]
fn soft_hyphen_break_shows_a_hyphen() {
    let mut fm = fonts();
    let word = "Hyphen\u{00AD}ation";
    let text = std::iter::repeat_n(word, 30).collect::<Vec<_>>().join("");
    let d = doc(vec![para(vec![Run::text(text, RunProps::default())])]);
    let l = run(&d, &mut fm);
    let lines = &l.pages[0].lines;
    assert!(lines.len() > 1);
    let first: String = lines[0].fragments.iter().map(|f| f.text.as_str()).collect();
    assert!(first.ends_with('-'), "{first}");
    assert!(!first.contains('\u{00AD}'));
}
