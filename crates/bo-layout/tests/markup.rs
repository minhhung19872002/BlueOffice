//! Review display modes: inline marks filtered per reviewer, change bars and
//! balloons in the markup area (ECMA-376 Part 1, §17.13.4 comments, §17.13.5
//! tracked changes).
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{
    layout_document_with, page_markup, LayoutCache, LayoutDocument, LayoutOptions, Mark,
    MarkFilter, MarkKind, MarkupStyle, RevisionView, BALLOON_GAP,
};
use bo_model::{
    Block, Color, Document, HalfPoints, ParaProps, Paragraph, RevMark, RevisionInfo, Run, RunProps,
    Section, Underline,
};
use bo_text::FontManager;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

fn rev(id: i32, author: &str) -> RevMark {
    RevMark {
        info: RevisionInfo {
            id,
            author: author.into(),
            date: None,
        },
        moved: false,
    }
}

/// "Giữ " + "chèn" (inserted by An) + "xoá" (deleted by Bình), then `extra` plain
/// paragraphs.
fn tracked(extra: usize) -> Document {
    let mut blocks = vec![Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![
            Run::text("Giữ ", RunProps::default()),
            Run::text(
                "chèn",
                RunProps {
                    ins: Some(rev(1, "An")),
                    ..Default::default()
                },
            ),
            Run::text(
                "xoá",
                RunProps {
                    del: Some(rev(2, "Bình")),
                    ..Default::default()
                },
            ),
        ],
    ))];
    for i in 0..extra {
        blocks.push(Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run::text(format!("Đoạn {i}"), RunProps::default())],
        )));
    }
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(26)),
            ..Default::default()
        },
        sections: vec![Section::new(blocks)],
        ..Default::default()
    }
}

fn lay(d: &Document, view: RevisionView, marks: MarkFilter) -> LayoutDocument {
    layout_document_with(
        d,
        &mut fonts(),
        &mut LayoutCache::new(),
        LayoutOptions {
            view,
            marks,
            ..LayoutOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn display_modes_show_the_original_or_the_accepted_text() {
    let d = tracked(0);
    // All markup: both sides of the changes.
    assert_eq!(
        lay(&d, RevisionView::Markup, MarkFilter::default()).page_text(0),
        "Giữ chènxoá"
    );
    // Simple markup and no markup lay out the accepted text.
    assert_eq!(
        lay(&d, RevisionView::Final, MarkFilter::default()).page_text(0),
        "Giữ chèn"
    );
    // Original: every change rejected.
    assert_eq!(
        lay(&d, RevisionView::Original, MarkFilter::default()).page_text(0),
        "Giữ xoá"
    );
}

#[test]
fn markup_filter_hides_reviewers_and_moves_deletions_to_balloons() {
    let d = tracked(0);
    // Bình hidden: the deletion is shown as accepted, An's insertion still marked.
    let mut f = MarkFilter::default();
    f.hide_reviewers(["Bình"]);
    let l = lay(&d, RevisionView::Markup, f);
    assert_eq!(l.page_text(0), "Giữ chèn");
    let ins = l.pages[0].lines[0]
        .fragments
        .iter()
        .find(|f| f.text.contains("chèn"))
        .unwrap();
    assert_eq!(ins.underline, Underline::Single);
    // Insertions and deletions off: the accepted text without any mark.
    let l = lay(
        &d,
        RevisionView::Markup,
        MarkFilter {
            hide_ins_del: true,
            ..MarkFilter::default()
        },
    );
    assert_eq!(l.page_text(0), "Giữ chèn");
    assert!(l.pages[0].lines[0]
        .fragments
        .iter()
        .all(|f| f.underline == Underline::None && !f.strike));
    // Deletions in balloons: removed from the line, insertion still underlined.
    let l = lay(
        &d,
        RevisionView::Markup,
        MarkFilter {
            deletions_in_balloons: true,
            ..MarkFilter::default()
        },
    );
    assert_eq!(l.page_text(0), "Giữ chèn");
}

fn mark(id: i32, kind: MarkKind, start: (u32, u32), end: (u32, u32), text: &str) -> Mark {
    Mark {
        id,
        kind,
        start,
        end,
        author: "An".into(),
        date: None,
        text: text.into(),
        done: false,
        balloon: kind == MarkKind::Comment,
        story: None,
    }
}

#[test]
fn markup_area_bars_and_balloons() {
    let d = tracked(3);
    let l = lay(&d, RevisionView::Markup, MarkFilter::default());
    let page_w = l.pages[0].width;
    let marks = vec![
        mark(1, MarkKind::Insert, (0, 7), (0, 13), "chèn"),
        mark(7, MarkKind::Comment, (2, 0), (2, 6), "Xem lại đoạn này"),
    ];
    let style = MarkupStyle {
        area_width: 180.0,
        bar_color: Some(Color { r: 0, g: 0, b: 0 }),
        icons: false,
    };
    let pm = page_markup(&l, &marks, &style);
    assert_eq!(pm.len(), l.pages.len());
    let p0 = &pm[0];
    assert!((p0.width - 180.0).abs() < 0.01);
    // One change bar, beside the first paragraph's line, left of the text.
    assert_eq!(p0.bars.len(), 1);
    let first = &l.pages[0].lines[0];
    assert!((p0.bars[0].top - first.top).abs() < 0.01);
    assert!(p0.bars[0].x < first.carets.first().unwrap().1);
    // One balloon (the comment) in the markup area, anchored on the third line.
    assert_eq!(p0.balloons.len(), 1);
    let b = &p0.balloons[0];
    assert_eq!((b.id, b.kind), (7, MarkKind::Comment));
    assert!(b.x >= page_w && b.x + b.w <= page_w + 180.0);
    let third = l.lines_of(2).next().unwrap().1;
    assert!((b.anchor_y - (third.baseline + 2.0)).abs() < 0.01);
    // No area, no balloons; no bar colour, no bars.
    let none = page_markup(
        &l,
        &marks,
        &MarkupStyle {
            area_width: 0.0,
            bar_color: None,
            icons: false,
        },
    );
    assert!(none[0].is_empty());
}

#[test]
fn balloons_on_one_line_are_stacked_without_overlap() {
    let d = tracked(0);
    let l = lay(&d, RevisionView::Markup, MarkFilter::default());
    let long = "Một nhận xét khá dài cần nhiều dòng trong bóng chú thích ở lề phải";
    let marks: Vec<Mark> = (0..6)
        .map(|i| mark(i, MarkKind::Comment, (0, 0), (0, 4), long))
        .collect();
    let pm = page_markup(
        &l,
        &marks,
        &MarkupStyle {
            area_width: 180.0,
            bar_color: None,
            icons: false,
        },
    );
    let b = &pm[0].balloons;
    assert_eq!(b.len(), 6);
    for w in b.windows(2) {
        assert!(w[0].y + w[0].h + BALLOON_GAP <= w[1].y + 0.01, "{w:?}");
    }
    // Collapsed into icons (simple markup): small squares beside the page, stacked too.
    let icons = page_markup(
        &l,
        &marks,
        &MarkupStyle {
            area_width: 24.0,
            bar_color: Some(Color { r: 200, g: 0, b: 0 }),
            icons: true,
        },
    );
    let i = &icons[0].balloons;
    assert!(i.iter().all(|b| b.icon && b.w <= 24.0));
    for w in i.windows(2) {
        assert!(w[0].y + w[0].h <= w[1].y);
    }
    // Comments never draw change bars.
    assert_eq!(icons[0].bars.len(), 0);
}

/// The markup filters apply to tracked table rows too: a row deleted by a hidden
/// reviewer (or with insertions/deletions off) shows as accepted — gone — while the
/// other reviewers' rows stay marked (§17.13.5.12 del in trPr).
#[test]
fn markup_filter_applies_to_tracked_rows() {
    let mut t = bo_model::Table::simple(2, 1, bo_model::Twips(4000));
    for (i, row) in t.rows.iter_mut().enumerate() {
        row.cells[0].blocks = vec![Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run::text(format!("Hàng{i}"), RunProps::default())],
        ))];
    }
    t.rows[1].props.del = Some(rev(7, "Bình"));
    let mut d = tracked(0);
    d.sections[0].blocks.push(Block::Table(t));
    let text = |f: MarkFilter| lay(&d, RevisionView::Markup, f).page_text(0);
    assert!(text(MarkFilter::default()).contains("Hàng1"));
    let mut f = MarkFilter::default();
    f.hide_reviewers(["Bình"]);
    assert!(!text(f).contains("Hàng1"));
    assert!(text(f).contains("Hàng0"));
    let mut f = MarkFilter::default();
    f.hide_reviewers(["An"]);
    assert!(text(f).contains("Hàng1"));
    let off = MarkFilter {
        hide_ins_del: true,
        ..MarkFilter::default()
    };
    assert!(!text(off).contains("Hàng1"));
}
