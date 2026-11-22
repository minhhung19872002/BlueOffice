//! Page Setup: gutter (ECMA-376 Part 1, §17.6.11 pgMar `w:gutter`), mirror margins
//! (§17.15.1.57), gutter at top (§17.15.1.49), book fold (§17.15.1.13) and vertical
//! alignment (§17.6.23 vAlign), placed as the reference word processor exports them
//! (docs/ui/REPORT.md "Dàn trang vòng 6").
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, LayoutCache, LayoutDocument, Line};
use bo_model::{
    Block, Document, HalfPoints, HeaderFooter, PageSetup, PageVAlign, PagesSettings, ParaProps,
    Paragraph, Run, RunContent, RunProps, Section, SectionLayout, Twips,
};
use bo_text::FontManager;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

fn p(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run::text(text, RunProps::default())],
    ))
}

fn page_break() -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![RunContent::PageBreak],
        }],
    ))
}

/// Three pages "P1" / "P2" / "P3", left margin 1 in, right margin 2 in, a header.
fn doc(layout: SectionLayout, pages: PagesSettings) -> Document {
    let mut sec = Section::new(vec![p("P1"), page_break(), p("P2"), page_break(), p("P3")]);
    sec.page = PageSetup {
        margin_left: Twips(1440),
        margin_right: Twips(2880),
        ..PageSetup::default()
    };
    sec.layout = layout;
    sec.headers = HeaderFooter {
        default: Some(vec![p("Head")]),
        ..HeaderFooter::default()
    };
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(24)),
            ..Default::default()
        },
        sections: vec![sec],
        pages,
        ..Default::default()
    }
}

fn run(d: &Document) -> LayoutDocument {
    layout_document(d, &mut fonts(), &mut LayoutCache::new()).unwrap()
}

fn text(l: &Line) -> String {
    l.fragments.iter().map(|f| f.text.as_str()).collect()
}

/// (x, top) of the line reading `t` on page `pi`.
fn at(l: &LayoutDocument, pi: usize, t: &str) -> (f32, f32) {
    let line = l.pages[pi].lines.iter().find(|l| text(l) == t).unwrap();
    (line.fragments[0].x, line.top)
}

fn gutter(g: i32) -> SectionLayout {
    SectionLayout {
        gutter: Twips(g),
        ..SectionLayout::default()
    }
}

#[test]
fn gutter_widens_the_left_margin_and_moves_the_header() {
    let l = run(&doc(gutter(720), PagesSettings::default()));
    for (pi, t) in [(0, "P1"), (1, "P2")] {
        assert!((at(&l, pi, t).0 - 108.0).abs() < 0.01);
        assert!((at(&l, pi, "Head").0 - 108.0).abs() < 0.01);
    }
}

#[test]
fn mirror_margins_put_even_pages_at_the_outside_margin() {
    let mirror = PagesSettings {
        mirror_margins: true,
        ..PagesSettings::default()
    };
    let l = run(&doc(gutter(720), mirror));
    // Odd: inside 72 + gutter 36; even: the outside (right) margin, 144.
    assert!((at(&l, 0, "P1").0 - 108.0).abs() < 0.01);
    assert!((at(&l, 1, "P2").0 - 144.0).abs() < 0.01);
    assert!((at(&l, 1, "Head").0 - 144.0).abs() < 0.01);
    assert!((at(&l, 2, "P3").0 - 108.0).abs() < 0.01);
    // Book fold: the other way round.
    let fold = PagesSettings {
        book_fold: true,
        ..PagesSettings::default()
    };
    let l = run(&doc(gutter(360), fold));
    assert!((at(&l, 0, "P1").0 - 162.0).abs() < 0.01);
    assert!((at(&l, 1, "P2").0 - 72.0).abs() < 0.01);
}

#[test]
fn gutter_at_top_lowers_the_body_not_the_header() {
    let top = PagesSettings {
        gutter_at_top: true,
        ..PagesSettings::default()
    };
    let plain = run(&doc(SectionLayout::default(), PagesSettings::default()));
    let l = run(&doc(gutter(720), top));
    assert!((at(&l, 0, "P1").0 - 72.0).abs() < 0.01);
    assert!((at(&l, 0, "P1").1 - at(&plain, 0, "P1").1 - 36.0).abs() < 0.01);
    assert!((at(&l, 0, "Head").1 - at(&plain, 0, "Head").1).abs() < 0.01);
}

#[test]
fn vertical_alignment_moves_the_body_down() {
    let va = |v| SectionLayout {
        v_align: v,
        ..SectionLayout::default()
    };
    let top = run(&doc(va(PageVAlign::Top), PagesSettings::default()));
    let center = run(&doc(va(PageVAlign::Center), PagesSettings::default()));
    let bottom = run(&doc(va(PageVAlign::Bottom), PagesSettings::default()));
    let both = run(&doc(va(PageVAlign::Both), PagesSettings::default()));
    let s = PageSetup::default();
    let body_bottom = s.height.to_pt() - s.margin_bottom.to_pt();
    // Page 3 holds one line: bottom puts it on the bottom margin, center halfway.
    let line = |l: &LayoutDocument| {
        let ln = l.pages[2].lines.iter().find(|l| text(l) == "P3").unwrap();
        (ln.top, ln.top + ln.height)
    };
    let (t0, _) = line(&top);
    let (_, b) = line(&bottom);
    assert!((b - body_bottom).abs() < 0.01, "{b} vs {body_bottom}");
    let (tc, _) = line(&center);
    let (tb, _) = line(&bottom);
    assert!((tc - t0.midpoint(tb)).abs() < 0.01);
    // The header stays.
    assert!((at(&bottom, 2, "Head").1 - at(&top, 2, "Head").1).abs() < 0.01);
    // Justified: page 1 (P1 and the page-break paragraph): the first paragraph stays at
    // the top, the last one moves down by all the room.
    assert!((at(&both, 0, "P1").1 - at(&top, 0, "P1").1).abs() < 0.01);
}
