//! Pipeline tests on synthetic pages: every source page is a Letter page
//! filled with its own colour `(k, 0, 0)` (k = page number), so the PDF's
//! fill operators tell which page landed where. Expected values by hand.
#![allow(clippy::unwrap_used)]

use super::*;
use bo_render::{Command, Rgba};

#[path = "../../tests/support/pdf_pages.rs"]
mod pdf;

const W: f32 = 612.0;
const H: f32 = 792.0;

fn page(k: u8) -> PageList {
    PageList {
        width: W,
        height: H,
        commands: vec![Command::FillRect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            color: Rgba(k, 0, 0, 255),
        }],
    }
}

fn logical(n: u8) -> Vec<Logical> {
    (1..=n).map(|k| (usize::from(k), page(k))).collect()
}

/// PDF of the whole job (every copy).
fn job_pdf(pages: Vec<Logical>, o: &PrintOptions) -> Vec<pdf::PdfPage> {
    let sheets = with_copies(finish(pages, o, o.frame), o);
    let bytes = bo_pdf::write_pdf(
        &DisplayList { pages: sheets },
        &bo_text::FontManager::new(),
        &bo_pdf::PdfMeta::default(),
    )
    .unwrap();
    pdf::pages(&bytes)
}

/// Red channel of every fill on a page: the source pages drawn there, in order.
fn sources(p: &pdf::PdfPage) -> Vec<u8> {
    p.fills().iter().map(|c| c[0]).collect()
}

fn close(a: f32, b: f32) {
    assert!((a - b).abs() < 0.01, "{a} != {b}");
}

#[test]
fn four_up_letter_pdf_structure() {
    let o = PrintOptions {
        per_sheet: 4,
        paper: Some(PaperSize::Letter),
        ..PrintOptions::default()
    };
    let pages = job_pdf(logical(5), &o);
    // ⌈5 / 4⌉ = 2 sheets of Letter.
    assert_eq!(pages.len(), 2);
    for p in &pages {
        assert_eq!(p.media_box, [0.0, 0.0, 612.0, 792.0]);
    }
    assert_eq!(sources(&pages[0]), vec![1, 2, 3, 4]);
    assert_eq!(sources(&pages[1]), vec![5]);
    // Placement (see nup tests): s = 283.5 / 612, page height 366.882, cells at
    // x = 18 / 310.5 and top y = 21.309 / 403.809 (y-down). The PDF matrix of a
    // y-down translation (e, t) on a sheet of height H is [s 0 0 s e  H − s·H − t]
    // (here the sheet and the pages are both Letter, so s·H is the page height).
    let s = 283.5 / 612.0;
    let ph = H * s;
    let dy = (373.5 - ph) / 2.0;
    let expect = [
        (18.0, 18.0 + dy),
        (310.5, 18.0 + dy),
        (18.0, 400.5 + dy),
        (310.5, 400.5 + dy),
    ];
    let cms = pages[0].all("cm");
    assert_eq!(cms.len(), 4);
    for (m, (e, t)) in cms.iter().zip(expect) {
        close(m[0], s);
        close(m[1], 0.0);
        close(m[2], 0.0);
        close(m[3], s);
        close(m[4], e);
        close(m[5], H - ph - t);
    }
    // First cell: H − s·H − t = 792 − 366.882 − 21.309 = 403.809.
    close(cms[0][5], 403.809);
}

#[test]
fn down_order_fills_columns_first() {
    let o = PrintOptions {
        per_sheet: 4,
        order: NupOrder::Down,
        paper: Some(PaperSize::Letter),
        ..PrintOptions::default()
    };
    let p = &job_pdf(logical(4), &o)[0];
    let cms = p.all("cm");
    // Page 2 goes under page 1 (same x = 18), page 3 to the right (x = 310.5).
    close(cms[1][4], 18.0);
    close(cms[2][4], 310.5);
    assert!(cms[1][5] < cms[0][5], "page 2 is lower on the sheet");
}

#[test]
fn two_up_turns_the_sheet() {
    let o = PrintOptions {
        per_sheet: 2,
        paper: Some(PaperSize::Letter),
        ..PrintOptions::default()
    };
    let pages = job_pdf(logical(3), &o);
    assert_eq!(pages.len(), 2);
    // Landscape Letter (nup::tests::two_up_turns_the_paper).
    assert_eq!(pages[0].media_box, [0.0, 0.0, 792.0, 612.0]);
    assert_eq!(sources(&pages[0]), vec![1, 2]);
    assert_eq!(sources(&pages[1]), vec![3]);
}

#[test]
fn range_reverse_and_copies() {
    let base = PrintOptions {
        pages: PageChoice::Range,
        range: "2-3, 5".into(),
        ..PrintOptions::default()
    };
    let chosen = choose(&base, 5, None).unwrap();
    assert_eq!(chosen, vec![1, 2, 4]);
    let pick = |o: &PrintOptions| -> Vec<u8> {
        let pages: Vec<Logical> = choose(o, 5, None)
            .unwrap()
            .into_iter()
            .map(|i| (i + 1, page(u8::try_from(i + 1).unwrap())))
            .collect();
        job_pdf(pages, o).iter().flat_map(sources).collect()
    };
    assert_eq!(pick(&base), vec![2, 3, 5]);
    // Reverse: last sheet first; two collated copies repeat the set.
    let o = PrintOptions {
        reverse: true,
        copies: 2,
        ..base.clone()
    };
    assert_eq!(pick(&o), vec![5, 3, 2, 5, 3, 2]);
    // Not collated: each sheet twice in a row.
    let o = PrintOptions {
        collate: false,
        ..o
    };
    assert_eq!(pick(&o), vec![5, 5, 3, 3, 2, 2]);
    // One page per sheet without a paper: pages keep their size, no transform.
    let pages = job_pdf(logical(1), &PrintOptions::default());
    assert_eq!(pages[0].media_box, [0.0, 0.0, 612.0, 792.0]);
    assert_eq!(pages[0].all("cm"), Vec::<Vec<f32>>::new());
}

#[test]
fn odd_even_current_selection() {
    let odd = PrintOptions {
        subset: Subset::Odd,
        ..PrintOptions::default()
    };
    let p = job_pdf(logical(5), &odd);
    assert_eq!(
        p.iter().flat_map(sources).collect::<Vec<_>>(),
        vec![1, 3, 5]
    );
    let even = PrintOptions {
        subset: Subset::Even,
        per_sheet: 2,
        ..PrintOptions::default()
    };
    let p = job_pdf(logical(5), &even);
    // Pages 2 and 4 on one sheet (no paper: the page's own size, turned).
    assert_eq!(p.len(), 1);
    assert_eq!(sources(&p[0]), vec![2, 4]);
    assert_eq!(p[0].media_box, [0.0, 0.0, 792.0, 612.0]);
    let cur = PrintOptions {
        pages: PageChoice::Current,
        current: 3,
        ..PrintOptions::default()
    };
    assert_eq!(choose(&cur, 5, None).unwrap(), vec![3]);
    assert_eq!(choose(&cur, 3, None).unwrap(), Vec::<usize>::new());
    let sel = PrintOptions {
        pages: PageChoice::Selection,
        selected: vec![4, 0, 9],
        ..PrintOptions::default()
    };
    assert_eq!(choose(&sel, 5, None).unwrap(), vec![4, 0]);
    let bad = PrintOptions {
        pages: PageChoice::Range,
        range: "x".into(),
        ..PrintOptions::default()
    };
    assert!(choose(&bad, 5, None).is_err());
}

#[test]
fn greyscale_in_the_pdf() {
    let o = PrintOptions {
        color: ColorMode::Grayscale,
        ..PrintOptions::default()
    };
    let pages = vec![(
        1,
        PageList {
            width: W,
            height: H,
            commands: vec![Command::FillRect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                color: Rgba(255, 0, 0, 255),
            }],
        },
    )];
    // Y(255, 0, 0) = 76.
    assert_eq!(job_pdf(pages, &o)[0].fills(), vec![[76, 76, 76]]);
}

#[test]
fn frames_and_scale_to_a4() {
    let o = PrintOptions {
        paper: Some(PaperSize::A4),
        frame: true,
        ..PrintOptions::default()
    };
    let p = &job_pdf(logical(1), &o)[0];
    // A4 = 595.276 × 841.89 pt; Letter on it: s = min(595.276/612, 841.89/792) = 0.97267.
    close(p.media_box[2], 595.276);
    close(p.media_box[3], 841.89);
    let cms = p.all("cm");
    assert_eq!(cms.len(), 1);
    let s = 595.276 / 612.0;
    close(cms[0][0], s);
    // Centred vertically: t = (841.89 − 792·s) / 2 = 35.766. The PDF matrix flips
    // about the sheet height H = 841.89: f = H − s·H − t = 841.89 − 818.879 − 35.766 = −12.755.
    let t = (841.89 - 792.0 * s) / 2.0;
    close(cms[0][5], 841.89 - s * 841.89 - t);
    close(cms[0][5], -12.755);
    // The frame is one stroked path (`S`).
    assert_eq!(p.all("S").len(), 1);
}

#[test]
fn preview_cache_follows_the_options() {
    let mut cache = PreviewCache::default();
    let mut builds = 0;
    let mut build = |_: &PrintOptions| {
        builds += 1;
        Ok(vec![page(1), page(2)])
    };
    let a = cache
        .preview(r#"{"job":1,"copies":3}"#, 7, &mut build)
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&a).unwrap();
    assert_eq!(v["sheets"], 2);
    assert_eq!(v["copies"], 3);
    // Index clamped to the last sheet.
    assert_eq!(v["index"], 1);
    assert_eq!(v["page"]["width"], 612.0);
    cache
        .preview(r#"{"job":1,"copies":3}"#, 0, &mut build)
        .unwrap();
    cache
        .preview(r#"{"job":2,"copies":3}"#, 0, &mut build)
        .unwrap();
    assert_eq!(builds, 2);
    assert!(PrintOptions::from_json(r#"{"perSheet":"x"}"#).is_err());
}
