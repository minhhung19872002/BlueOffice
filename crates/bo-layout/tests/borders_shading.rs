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
