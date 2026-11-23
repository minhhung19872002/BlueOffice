//! Cell diagonals (ECMA-376 Part 1, §17.4.80 tl2br, §17.4.79 tr2bl), paragraph border
//! shadows (§17.18.2 CT_Border `w:shadow`) and shading patterns (§17.18.78) on
//! paragraphs and cells, as the reference word processor draws them in its PDF export
//! (docs/ui/REPORT.md "Dàn trang vòng 6").
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, LayoutCache, LayoutDocument, PageItem};
use bo_model::run_fx::ShadingPattern;
use bo_model::{
    Block, Border, Borders, Color, Document, HalfPoints, ParaProps, Paragraph, Run, RunProps,
    Section, Table, Twips,
};
use bo_text::FontManager;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

fn p(text: &str, props: ParaProps) -> Block {
    Block::Paragraph(Paragraph::new(
        props,
        vec![Run::text(text, RunProps::default())],
    ))
}

fn run(blocks: Vec<Block>) -> LayoutDocument {
    let d = Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(24)),
            ..Default::default()
        },
        sections: vec![Section::new(blocks)],
        ..Default::default()
    };
    layout_document(&d, &mut fonts(), &mut LayoutCache::new()).unwrap()
}

fn rules(l: &LayoutDocument) -> Vec<(f32, f32, f32, f32, f32, String)> {
    l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            PageItem::Rule {
                x1,
                y1,
                x2,
                y2,
                width,
                style,
                ..
            } => Some((*x1, *y1, *x2, *y2, *width, style.clone())),
            _ => None,
        })
        .collect()
}

fn rects(l: &LayoutDocument) -> Vec<(f32, f32, f32, f32, Color)> {
    l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            PageItem::Rect {
                x, y, w, h, color, ..
            } => Some((*x, *y, *w, *h, *color)),
            _ => None,
        })
        .collect()
}

#[test]
fn cell_diagonals_join_the_inner_corners_of_the_cell_borders() {
    let mut t = Table::simple(1, 2, Twips(4000));
    let mut red = Border::single(12);
    red.color = Some(Color { r: 255, g: 0, b: 0 });
    for c in &mut t.rows[0].cells {
        c.props.borders = Some(Borders {
            top: Some(Border::single(4)),
            left: Some(Border::single(4)),
            bottom: Some(Border::single(4)),
            right: Some(Border::single(4)),
            ..Borders::default()
        });
    }
    t.rows[0].cells[0].props.diagonals[0] = Some(red.clone());
    t.rows[0].cells[1].props.diagonals = [Some(Border::single(4)), Some(red)];
    let l = run(vec![Block::Table(t), p("x", ParaProps::default())]);
    let all = rules(&l);
    let diag: Vec<_> = all
        .iter()
        .filter(|r| (r.0 - r.2).abs() > 1.0 && (r.1 - r.3).abs() > 1.0)
        .collect();
    assert_eq!(diag.len(), 3, "{all:?}");
    // Grid: the vertical lines at the cells' edges.
    let mut xs: Vec<f32> = all
        .iter()
        .filter(|r| (r.0 - r.2).abs() < 1e-3)
        .map(|r| r.0)
        .collect();
    xs.sort_by(f32::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    let ys: Vec<f32> = all
        .iter()
        .filter(|r| (r.1 - r.3).abs() < 1e-3)
        .map(|r| r.1)
        .collect();
    let (top, bottom) = (
        ys.iter().copied().fold(f32::MAX, f32::min),
        ys.iter().copied().fold(f32::MIN, f32::max),
    );
    // First cell: top-left to bottom-right, 0.25 pt (half a 0.5 pt line) inside.
    let d0 = diag[0];
    assert!((d0.0 - (xs[0] + 0.25)).abs() < 0.01 && (d0.1 - (top + 0.25)).abs() < 0.01);
    assert!((d0.2 - (xs[1] - 0.25)).abs() < 0.01 && (d0.3 - (bottom - 0.25)).abs() < 0.01);
    assert!((d0.4 - 1.5).abs() < 1e-3);
    // Second cell: both, the tr2bl one from the top-right corner.
    let d2 = diag[2];
    assert!((d2.0 - (xs[2] - 0.25)).abs() < 0.01 && (d2.2 - (xs[1] + 0.25)).abs() < 0.01);
}

#[test]
fn shadowed_box_gets_a_black_band_right_and_below() {
    let mut b = Border::single(12);
    b.space = 4;
    let plain = Borders {
        top: Some(b.clone()),
        left: Some(b.clone()),
        bottom: Some(b.clone()),
        right: Some(b.clone()),
        ..Borders::default()
    };
    let mut s = b.clone();
    s.shadow = true;
    let shadowed = Borders {
        bottom: Some(s.clone()),
        right: Some(s),
        ..plain.clone()
    };
    let none = run(vec![p(
        "Box",
        ParaProps {
            borders: Some(plain),
            ..ParaProps::default()
        },
    )]);
    let l = run(vec![p(
        "Box",
        ParaProps {
            borders: Some(shadowed),
            ..ParaProps::default()
        },
    )]);
    assert_eq!(rects(&none).len(), 0);
    let r = rects(&l);
    assert_eq!(r.len(), 2, "{r:?}");
    let all = rules(&l);
    let right_x = all.iter().map(|r| r.0).fold(f32::MIN, f32::max);
    let bottom_y = all.iter().map(|r| r.1).fold(f32::MIN, f32::max);
    // Right band: 1.5 pt wide, right outside the right line (0.75 pt from its centre).
    let (x, _, w, _, c) = r[0];
    assert!((x - (right_x + 0.75)).abs() < 0.01 && (w - 1.5).abs() < 0.01);
    assert_eq!(c, Color::BLACK);
    // Bottom band below the bottom line.
    let (_, y, _, h, _) = r[1];
    assert!((y - (bottom_y + 0.75)).abs() < 0.01 && (h - 1.5).abs() < 0.01);
}

#[test]
fn shading_patterns_mix_the_pattern_colour_into_paragraphs_and_cells() {
    let pat = |val: &str, color: Option<Color>| {
        Some(ShadingPattern {
            val: val.into(),
            color,
        })
    };
    let mut t = Table::simple(1, 1, Twips(3000));
    t.rows[0].cells[0].props.shading = Some(Color::WHITE);
    t.rows[0].cells[0].props.shading_pattern = pat(
        "pct12",
        Some(Color {
            r: 0,
            g: 0x70,
            b: 0xC0,
        }),
    );
    let l = run(vec![
        p(
            "Grey",
            ParaProps {
                shading_pattern: pat("pct25", None),
                ..ParaProps::default()
            },
        ),
        Block::Table(t),
    ]);
    let r = rects(&l);
    // pct25 of auto (black) on auto (white): 75 % white.
    assert!(r.iter().any(|x| x.4
        == Color {
            r: 191,
            g: 191,
            b: 191
        }));
    // pct12 is 12.5 % (0.875 0.929 0.969 in the reference's PDF).
    assert!(
        r.iter().any(|x| x.4
            == Color {
                r: 223,
                g: 237,
                b: 247
            }),
        "{r:?}"
    );
}

/// Screen patterns (round 7): the rectangles keep the mixed colour (print / PDF) and carry
/// the reference's 8 × 8 device-pixel tile, except a percentage with auto colour on an
/// auto fill, which the reference shows as flat grey.
#[test]
fn shading_patterns_carry_the_screen_tile() {
    let pat = |val: &str, color: Option<Color>| {
        Some(ShadingPattern {
            val: val.into(),
            color,
        })
    };
    let blue = Color {
        r: 0,
        g: 0x70,
        b: 0xC0,
    };
    let mut t = Table::simple(1, 1, Twips(3000));
    t.rows[0].cells[0].props.shading = Some(Color::WHITE);
    t.rows[0].cells[0].props.shading_pattern = pat("pct50", Some(blue));
    let run_props = RunProps {
        shading_pattern: pat("diagCross", None),
        ..RunProps::default()
    };
    let l = run(vec![
        p(
            "Grey",
            ParaProps {
                shading_pattern: pat("pct25", None),
                ..ParaProps::default()
            },
        ),
        p(
            "Stripes",
            ParaProps {
                shading_pattern: pat("horzStripe", None),
                ..ParaProps::default()
            },
        ),
        Block::Table(t),
        Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run::text("run", run_props)],
        )),
    ]);
    let pats: Vec<_> = l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            PageItem::Rect { color, pattern, .. } => Some((*color, *pattern)),
            _ => None,
        })
        .collect();
    // pct25 auto on auto: flat grey, no tile.
    assert!(
        pats.iter().any(|(c, p)| c.r == 191 && p.is_none()),
        "{pats:?}"
    );
    // horzStripe auto on auto: black / white rows of two.
    assert!(pats.iter().any(|(_, p)| p.is_some_and(|p| p.tile
        == [0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFF]
        && p.fg == Color::BLACK
        && p.bg == Color::WHITE)));
    // pct50 blue on white: a checkerboard.
    assert!(pats
        .iter()
        .any(|(_, p)| p.is_some_and(|p| p.fg == blue && p.tile[0] == 0x55 && p.tile[1] == 0xAA)));
    // The run's character shading.
    assert!(pats
        .iter()
        .any(|(_, p)| p.is_some_and(|p| p.tile[1] == 0xCC && p.tile[3] == 0x33)));
}

fn baselines(l: &LayoutDocument) -> Vec<f32> {
    l.pages[0].lines.iter().map(|l| l.baseline).collect()
}

/// Round 7 (docs/ui/REPORT.md "Dàn trang vòng 7", calibration gb-a … gb-j): consecutive
/// paragraphs with identical borders and indents share one box — one top and one bottom
/// line, sides (and shading) through the spacing between them; `w:between` draws a line
/// after the spacing with `space` on each side of it.
#[test]
fn consecutive_paragraphs_with_the_same_box_form_one_box() {
    let mut b = Border::single(4);
    b.space = 1;
    let boxed = |between: Option<Border>| ParaProps {
        borders: Some(Borders {
            top: Some(b.clone()),
            left: Some(b.clone()),
            bottom: Some(b.clone()),
            right: Some(b.clone()),
            inside_h: between,
            ..Borders::default()
        }),
        space_after: Some(Twips(240)),
        shading: Some(Color {
            r: 0xDD,
            g: 0xEB,
            b: 0xF7,
        }),
        ..ParaProps::default()
    };
    let three = |props: ParaProps| {
        run(vec![
            p("One", props.clone()),
            p("Two", props.clone()),
            p("Three", props),
        ])
    };
    let horizontal = |l: &LayoutDocument| {
        let mut ys: Vec<f32> = rules(l)
            .iter()
            .filter(|r| (r.1 - r.3).abs() < 0.01)
            .map(|r| r.1)
            .collect();
        ys.sort_by(f32::total_cmp);
        ys
    };
    let separate = run(vec![p("One", boxed(None))]);
    let one = three(boxed(None));
    // One top and one bottom line for the three paragraphs.
    assert_eq!(horizontal(&one).len(), 2, "{:?}", rules(&one));
    // Baselines 12 pt + one line apart (no box room between the paragraphs).
    let bl = baselines(&one);
    let single = baselines(&separate);
    let pitch = bl[1] - bl[0];
    assert!((bl[0] - single[0]).abs() < 0.01);
    assert!((pitch - (bl[2] - bl[1])).abs() < 0.01);
    // The sides run on through the spacing: the left side lines cover top to bottom.
    let ys = horizontal(&one);
    let vertical: Vec<_> = rules(&one)
        .into_iter()
        .filter(|r| (r.0 - r.2).abs() < 0.01)
        .collect();
    let left_x = vertical.iter().map(|r| r.0).fold(f32::MAX, f32::min);
    let mut left: Vec<(f32, f32)> = vertical
        .iter()
        .filter(|r| (r.0 - left_x).abs() < 0.01)
        .map(|r| (r.1, r.3))
        .collect();
    left.sort_by(|a, b| a.0.total_cmp(&b.0));
    // From the top line's outer edge (its centre is half its width lower).
    assert!((left[0].0 + 0.25 - ys[0]).abs() < 0.01, "{left:?} {ys:?}");
    for w in left.windows(2) {
        assert!((w[0].1 - w[1].0).abs() < 0.01, "gap in the side {left:?}");
    }
    // Shading too, reaching the box lines' inner edges (space + 1.4 pt left of the text).
    let r = rects(&one);
    let mut sh: Vec<(f32, f32)> = r.iter().map(|r| (r.1, r.1 + r.3)).collect();
    sh.sort_by(|a, b| a.0.total_cmp(&b.0));
    for w in sh.windows(2) {
        assert!((w[0].1 - w[1].0).abs() < 0.01, "gap in the shading {sh:?}");
    }
    let text_x = one.pages[0].lines[0].fragments[0].x;
    assert!((r[0].0 - (text_x - 2.4)).abs() < 0.01, "{r:?}");

    // w:between: a line between each pair, after the spacing, `space` each side of it.
    let mut bt = Border::single(4);
    bt.space = 1;
    let with = three(boxed(Some(bt)));
    assert_eq!(horizontal(&with).len(), 4, "{:?}", rules(&with));
    let bl2 = baselines(&with);
    assert!(
        ((bl2[1] - bl2[0]) - (pitch + 2.0 + 0.5)).abs() < 0.02,
        "{bl2:?} {pitch}"
    );

    // Another indent starts another box.
    let mut moved = boxed(None);
    moved.indent_left = Some(Twips(720));
    let two = run(vec![p("One", boxed(None)), p("Two", moved)]);
    assert_eq!(horizontal(&two).len(), 4);
}
