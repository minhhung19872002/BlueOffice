//! CORE-PRINT-001: print jobs of real BlueDoc, BlueSlide and BlueSheet
//! documents, checked on the PDF (page count, MediaBox, placement matrices)
//! and on the preview. Expected values are derived by hand.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

#[path = "support/pdf_pages.rs"]
mod pdf;

use bo_engine::print::nup::Orientation;
use bo_engine::print::{PageChoice, PaperSize, PrintOptions, SheetScope, SlideLayout, Subset};
use bo_engine::{Engine, SheetEngine};
use bo_pptx::build::{build_pptx, DeckSpec, ParaSpec, ShapeSpec, SlideSpec};
use bo_render::{Command, PageList};
use serde_json::Value;

/// A4 paper in points (210 × 297 mm).
const A4: (f32, f32) = (595.2756, 841.8898);
/// A4 page of a new document: 11 906 × 16 838 twips (§17.6.13 pgSz) / 20.
const DOC_A4: (f32, f32) = (595.3, 841.9);

fn close(a: f32, b: f32) {
    assert!((a - b).abs() < 0.02, "{a} != {b}");
}

fn cmd(e: &mut Engine, json: &str) -> Value {
    serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
}

/// A new A4 document with `n` pages ("Trang 1" … separated by page breaks).
fn doc(n: usize) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    for i in 1..=n {
        cmd(
            &mut e,
            &format!(r#"{{"cmd":"insertText","text":"Trang {i}"}}"#),
        );
        if i < n {
            cmd(&mut e, r#"{"cmd":"insertPageBreak"}"#);
        }
    }
    e
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

#[test]
fn doc_range_two_up_pdf() {
    let mut e = doc(5);
    assert_eq!(e.info().unwrap().page_count, 5);
    let page = e.info().unwrap().pages[0];
    let o = PrintOptions {
        pages: PageChoice::Range,
        range: "2-4".into(),
        per_sheet: 2,
        ..PrintOptions::default()
    };
    let pdf = pdf::pages(&e.print_pdf(&o).unwrap());
    // Pages 2, 3, 4 two per sheet → 2 sheets; no paper: the page's size turned
    // to landscape (A4 pages side by side scale better, see print::nup).
    assert_eq!(pdf.len(), 2);
    close(pdf[0].media_box[2], page.height);
    close(pdf[0].media_box[3], page.width);
    // Cells: (H − 36 − 9) / 2 wide, W − 36 high (H, W = A4 height, width).
    let cw = (page.height - 45.0) / 2.0;
    let s = (cw / page.width).min((page.width - 36.0) / page.height);
    let cms = pdf[0].all("cm");
    assert!(cms.len() >= 2);
    close(cms[0][0], s);
    // Second page starts one cell and one gap to the right, centred in its cell.
    let left = (cw - page.width * s) / 2.0;
    close(cms[0][4], 18.0 + left);
    close(cms[1][4], 18.0 + cw + 9.0 + left);
    // The second sheet holds one page.
    let sheets = e.print_sheets(&o).unwrap();
    assert!(texts(&sheets[0]).iter().any(|t| t == "Trang 2"));
    assert!(texts(&sheets[0]).iter().any(|t| t == "Trang 3"));
    assert!(texts(&sheets[1]).iter().any(|t| t == "Trang 4"));
    assert!(!texts(&sheets[1]).iter().any(|t| t == "Trang 5"));
}

#[test]
fn doc_even_pages_reverse_copies() {
    let mut e = doc(5);
    let o = PrintOptions {
        subset: Subset::Even,
        reverse: true,
        copies: 2,
        collate: false,
        ..PrintOptions::default()
    };
    let pdf = pdf::pages(&e.print_pdf(&o).unwrap());
    // Pages 2, 4 → reversed 4, 2 → each twice: 4 PDF pages at A4.
    assert_eq!(pdf.len(), 4);
    close(pdf[0].media_box[2], DOC_A4.0);
    close(pdf[0].media_box[3], DOC_A4.1);
    let sheets = e.print_sheets(&o).unwrap();
    assert_eq!(sheets.len(), 2);
    assert!(texts(&sheets[0]).iter().any(|t| t == "Trang 4"));
    assert!(texts(&sheets[1]).iter().any(|t| t == "Trang 2"));
    // A range that selects nothing is an error for the PDF.
    let none = PrintOptions {
        pages: PageChoice::Range,
        range: "9".into(),
        ..PrintOptions::default()
    };
    assert!(e.print_pdf(&none).is_err());
}

#[test]
fn doc_section_ranges() {
    let mut e = doc(2);
    // A third page in a second section (numbering continues: page 3).
    cmd(&mut e, r#"{"cmd":"insertSectionBreak","kind":"nextPage"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"Phan 2"}"#);
    let n = e.info().unwrap().page_count;
    assert_eq!(n, 3);
    let o = PrintOptions {
        pages: PageChoice::Range,
        range: "p2s1".into(),
        ..PrintOptions::default()
    };
    let s = e.print_sheets(&o).unwrap();
    assert_eq!(s.len(), 1);
    assert!(texts(&s[0]).iter().any(|t| t == "Trang 2"));
}

/// "Hợp đồng" with a comment on "Hợp".
fn commented() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("c.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"setAuthor","name":"An"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"Hợp đồng"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":0},"focus":{"p":0,"o":4}}}"#,
    );
    cmd(&mut e, r#"{"cmd":"addComment","text":"Kiểm tra"}"#);
    e
}

#[test]
fn doc_markup_widens_the_page_and_draws_balloons() {
    let mut e = commented();
    let on = PrintOptions::default();
    let s = e.print_sheets(&on).unwrap();
    // A4 width + the 180 pt markup area (bo_engine::markup::AREA_WIDTH).
    close(s[0].width, DOC_A4.0 + 180.0);
    assert!(texts(&s[0]).iter().any(|t| t == "Kiểm tra"));
    assert!(texts(&s[0]).iter().any(|t| t == "An"));
    let off = PrintOptions {
        doc: bo_engine::print::DocPrint {
            markup: false,
            ..Default::default()
        },
        ..PrintOptions::default()
    };
    let s = e.print_sheets(&off).unwrap();
    close(s[0].width, DOC_A4.0);
    assert!(!texts(&s[0]).iter().any(|t| t == "Kiểm tra"));
    // The view keeps its markup: the page markup is still sent with the page.
    let page: Value = serde_json::from_str(&e.page_json(0).unwrap()).unwrap();
    assert_eq!(page["markup"]["width"], 180.0);
    // Markup on, scaled to A4: the widened page fits the paper's width.
    let fit = PrintOptions {
        paper: Some(PaperSize::A4),
        ..PrintOptions::default()
    };
    let p = pdf::pages(&e.print_pdf(&fit).unwrap());
    close(p[0].media_box[2], A4.0);
    let cm = &p[0].all("cm")[0];
    close(cm[0], A4.0 / (DOC_A4.0 + 180.0));
}

#[test]
fn doc_properties_page() {
    let mut e = doc(1);
    let mut o = PrintOptions::default();
    o.doc.properties = true;
    o.doc.labels.insert("filename".into(), "Tên tệp".into());
    let s = e.print_sheets(&o).unwrap();
    assert_eq!(s.len(), 2);
    let t = texts(&s[1]);
    assert!(t.iter().any(|l| l == "Tên tệp: a.docx"), "{t:?}");
    assert!(t.iter().any(|l| l == "pages: 1"), "{t:?}");
}

/// DOC-PAGE-001: the page colour (§17.2.1 background) is left out of a print job unless
/// "print background colours" is on, as in word processors; the view keeps it.
#[test]
fn doc_page_colour_printed_only_when_asked() {
    let mut e = doc(1);
    cmd(&mut e, r#"{"cmd":"setPageColor","color":"FFF2CC"}"#);
    let fill = |p: &PageList| {
        p.commands.iter().any(|c| {
            matches!(c, Command::FillRect { x, y, w, h, color }
                if *x == 0.0 && *y == 0.0 && *w == p.width && *h == p.height
                    && *color == bo_render::Rgba(0xFF, 0xF2, 0xCC, 255))
        })
    };
    assert!(fill(e.page(0).unwrap()), "the view shows the page colour");
    let s = e.print_sheets(&PrintOptions::default()).unwrap();
    assert!(!fill(&s[0]));
    assert!(texts(&s[0]).iter().any(|t| t == "Trang 1"));
    let mut o = PrintOptions::default();
    o.doc.background = true;
    let s = e.print_sheets(&o).unwrap();
    assert!(fill(&s[0]));
    // The JSON of the print panel.
    let o = PrintOptions::from_json(r#"{"doc":{"background":true}}"#).unwrap();
    assert!(o.doc.background);
}

#[test]
fn doc_preview_matches_the_pdf_sheets() {
    let mut e = doc(3);
    let json = r#"{"job":1,"perSheet":2,"paper":"a4","copies":2}"#;
    let v: Value = serde_json::from_str(&e.print_preview_json(json, 1).unwrap()).unwrap();
    assert_eq!(v["sheets"], 2);
    assert_eq!(v["copies"], 2);
    assert_eq!(v["index"], 1);
    let sheets = e
        .print_sheets(&PrintOptions::from_json(json).unwrap())
        .unwrap();
    let page: PageList = serde_json::from_value(v["page"].clone()).unwrap();
    assert_eq!(page, sheets[1]);
    // The PDF holds both copies of both sheets.
    assert_eq!(pdf::pages(&e.print_pdf_json(json).unwrap()).len(), 4);
}

/// 16:9 deck (960 × 540 pt) of `n` slides, slide k titled "Slide k".
fn deck(n: usize) -> Engine {
    let slides = (1..=n)
        .map(|i| SlideSpec {
            shapes: vec![ShapeSpec {
                name: "Title".into(),
                x: 914_400,
                y: 914_400,
                cx: 6_096_000,
                cy: 914_400,
                text: vec![ParaSpec::plain(&format!("Slide {i}"))],
                ..ShapeSpec::default()
            }],
            notes: vec![format!("Note {i}")],
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

#[test]
fn slides_hidden_frames_and_paper() {
    let mut e = deck(3);
    e.presentation_mut().unwrap().slides[1].show = Some(false);
    let pdf = pdf::pages(&e.print_pdf(&PrintOptions::default()).unwrap());
    // The hidden slide 2 is left out; slides keep their 960 × 540 size.
    assert_eq!(pdf.len(), 2);
    assert_eq!(pdf[0].media_box, [0.0, 0.0, 960.0, 540.0]);
    let mut o = PrintOptions {
        frame: true,
        paper: Some(PaperSize::A4),
        ..PrintOptions::default()
    };
    o.slide.hidden = true;
    let pdf = pdf::pages(&e.print_pdf(&o).unwrap());
    assert_eq!(pdf.len(), 3);
    // On A4 the slide turns the paper: s = min(841.89/960, 595.28/540) = 0.87697.
    close(pdf[0].media_box[2], A4.1);
    close(pdf[0].media_box[3], A4.0);
    close(pdf[0].all("cm")[0][0], A4.1 / 960.0);
}

#[test]
fn slides_handouts_notes_outline() {
    let mut e = deck(7);
    let mut o = PrintOptions::default();
    o.slide.layout = SlideLayout::Handouts;
    o.slide.handout_per_page = 6;
    // ⌈7 / 6⌉ = 2 A4 handout pages.
    let pdf = pdf::pages(&e.print_pdf(&o).unwrap());
    assert_eq!(pdf.len(), 2);
    close(pdf[0].media_box[2], A4.0);
    // Slides 2–4 only: one page.
    o.pages = PageChoice::Range;
    o.range = "2-4".into();
    assert_eq!(e.print_sheets(&o).unwrap().len(), 1);
    // Notes pages of slides 2–4: one page each, the notes of slide 2 first.
    o.slide.layout = SlideLayout::Notes;
    let s = e.print_sheets(&o).unwrap();
    assert_eq!(s.len(), 3);
    // The notes are laid out like slide text: runs may break at spaces.
    assert!(
        texts(&s[0]).concat().contains("Note 2"),
        "{:?}",
        texts(&s[0])
    );
    // Outline of every slide on A4: "N  title" lines.
    o.slide.layout = SlideLayout::Outline;
    o.pages = PageChoice::All;
    let s = e.print_sheets(&o).unwrap();
    assert_eq!(s.len(), 1);
    close(s[0].width, A4.0);
    let t = texts(&s[0]);
    assert!(t.iter().any(|l| l.starts_with("1  ")), "{t:?}");
    assert!(t.iter().any(|l| l.starts_with("7  ")), "{t:?}");
}

#[test]
fn slides_selection_and_greyscale() {
    let mut e = deck(4);
    let o = PrintOptions {
        pages: PageChoice::Selection,
        selected: vec![3, 1],
        color: bo_engine::print::color::ColorMode::Grayscale,
        ..PrintOptions::default()
    };
    let s = e.print_sheets(&o).unwrap();
    assert_eq!(s.len(), 2);
    // Every glyph and fill is grey (r = g = b).
    for p in &s {
        for c in &p.commands {
            match c {
                Command::Glyphs { color, .. } | Command::FillRect { color, .. } => {
                    assert!(color.0 == color.1 && color.1 == color.2, "{color:?}");
                }
                _ => {}
            }
        }
    }
}

fn book(rows: u32, cols: u32) -> SheetEngine {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").unwrap();
    for r in 0..rows {
        for c in 0..cols {
            e.set_input(0, r, c, &format!("r{}c{}", r + 1, c + 1))
                .unwrap();
        }
    }
    e
}

#[test]
fn sheet_selection_titles_gridlines() {
    let mut e = book(120, 2);
    let all = e.print_sheets(&PrintOptions::default()).unwrap();
    assert!(all.len() >= 2, "{} pages", all.len());
    // Selection A1:A2: one page with those two cells only.
    let mut o = PrintOptions::default();
    o.sheet.scope = SheetScope::Selection;
    o.sheet.selection = Some("A1:A2".into());
    let s = e.print_sheets(&o).unwrap();
    assert_eq!(s.len(), 1);
    assert!(texts(&s[0]).iter().any(|t| t == "r2c1"));
    // (Cells next to the area may be drawn clipped by the band; far ones are not.)
    assert!(!texts(&s[0]).iter().any(|t| t == "r60c1" || t == "r120c2"));
    // Title row 1 on every page, for this job only.
    let mut o = PrintOptions::default();
    o.sheet.title_rows = Some("1:1".into());
    o.sheet.gridlines = Some(true);
    let s = e.print_sheets(&o).unwrap();
    for p in &s {
        assert!(texts(p).iter().any(|t| t == "r1c1"), "title row");
        assert!(
            p.commands.iter().any(|c| matches!(c, Command::Line { .. })),
            "gridlines"
        );
    }
    let again = e.print_sheets(&PrintOptions::default()).unwrap();
    assert!(
        !texts(&again[1]).iter().any(|t| t == "r1c1"),
        "setup restored"
    );
    // Pages 1 and 3 of the plain job, 2 per sheet on Letter.
    let o = PrintOptions {
        pages: PageChoice::Range,
        range: "1".into(),
        per_sheet: 2,
        paper: Some(PaperSize::Letter),
        ..PrintOptions::default()
    };
    let pdf = pdf::pages(&e.print_pdf(&o).unwrap());
    assert_eq!(pdf.len(), 1);
    // Two A4 portrait pages on Letter: landscape 792 × 612 (print::nup rule).
    assert_eq!(pdf[0].media_box, [0.0, 0.0, 792.0, 612.0]);
}

#[test]
fn sheet_workbook_and_ignore_print_area() {
    let mut e = book(3, 1);
    e.add_sheet(Some("Two")).unwrap();
    e.set_input(1, 0, 0, "second").unwrap();
    let mut o = PrintOptions::default();
    o.sheet.scope = SheetScope::Workbook;
    let s = e.print_sheets(&o).unwrap();
    assert_eq!(s.len(), 2);
    assert!(texts(&s[1]).iter().any(|t| t == "second"));
    // Active sheet 2 only.
    o.sheet.scope = SheetScope::Active;
    o.sheet.sheets = vec![1];
    assert_eq!(e.print_sheets(&o).unwrap().len(), 1);
    // A print area of A1 on sheet 1, ignored by the job.
    e.exec_json(r#"{"cmd":"setPageSetup","sheet":0,"printAreas":[{"r0":0,"c0":0,"r1":0,"c1":0}]}"#)
        .unwrap();
    let mut o = PrintOptions::default();
    assert!(!texts(&e.print_sheets(&o).unwrap()[0])
        .iter()
        .any(|t| t == "r3c1"));
    o.sheet.ignore_print_area = true;
    assert!(texts(&e.print_sheets(&o).unwrap()[0])
        .iter()
        .any(|t| t == "r3c1"));
    let v: Value = serde_json::from_str(
        &e.print_preview_json(r#"{"job":3,"sheet":{"ignorePrintArea":true}}"#, 0)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(v["sheets"], 1);
}

#[test]
fn doc_selection_prints_only_the_selected_text() {
    let mut e = doc(3);
    // Select "Trang 2" (the second page's paragraph).
    let v = cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":1,"o":0},"focus":{"p":1,"o":7}}}"#,
    );
    assert!(v["error"].is_null(), "{v}");
    let o = PrintOptions {
        pages: PageChoice::Selection,
        ..PrintOptions::default()
    };
    let s = e.print_sheets(&o).unwrap();
    // One A4 page with the selected text only, from the top of the page.
    assert_eq!(s.len(), 1);
    close(s[0].width, DOC_A4.0);
    let t = texts(&s[0]);
    assert!(t.iter().any(|x| x == "Trang 2"), "{t:?}");
    assert!(!t.iter().any(|x| x == "Trang 1" || x == "Trang 3"), "{t:?}");
    let pdf = pdf::pages(&e.print_pdf(&o).unwrap());
    assert_eq!(pdf.len(), 1);
    // The document is unchanged; a caret selects nothing to print.
    assert_eq!(e.info().unwrap().page_count, 3);
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    assert_eq!(e.print_sheets(&o).unwrap().len(), 0);
    assert!(e.print_pdf(&o).is_err());
}

#[test]
fn paper_orientation_is_a_choice() {
    let mut e = doc(2);
    // Automatic: two portrait pages per sheet go side by side on a landscape sheet.
    let mut o = PrintOptions {
        per_sheet: 2,
        paper: Some(PaperSize::A4),
        ..PrintOptions::default()
    };
    let s = e.print_sheets(&o).unwrap();
    assert!(s[0].width > s[0].height);
    // Portrait: the same pages stacked on a portrait sheet.
    o.orientation = Orientation::Portrait;
    let s = e.print_sheets(&o).unwrap();
    close(s[0].width, A4.0);
    close(s[0].height, A4.1);
    // One portrait page on landscape paper (no paper size: its own size turned).
    let o = PrintOptions {
        orientation: Orientation::Landscape,
        ..PrintOptions::default()
    };
    let pdf = pdf::pages(&e.print_pdf(&o).unwrap());
    close(pdf[0].media_box[2], DOC_A4.1);
    close(pdf[0].media_box[3], DOC_A4.0);
    // From the panel JSON.
    let o = PrintOptions::from_json(r#"{"orientation":"landscape","paper":"a4"}"#).unwrap();
    let s = e.print_sheets(&o).unwrap();
    close(s[0].width, A4.1);
}

#[test]
fn sheet_several_selected_sheets() {
    let mut e = book(2, 1);
    e.add_sheet(Some("Two")).unwrap();
    e.set_input(1, 0, 0, "second").unwrap();
    e.add_sheet(Some("Three")).unwrap();
    e.set_input(2, 0, 0, "third").unwrap();
    // Sheets 1 and 3 selected (Ctrl + click on their tabs): one page each, in order.
    let mut o = PrintOptions::default();
    o.sheet.scope = SheetScope::Active;
    o.sheet.sheets = vec![0, 2];
    let s = e.print_sheets(&o).unwrap();
    assert_eq!(s.len(), 2);
    assert!(texts(&s[0]).iter().any(|t| t == "r1c1"));
    assert!(texts(&s[1]).iter().any(|t| t == "third"));
    assert!(!s.iter().any(|p| texts(p).iter().any(|t| t == "second")));
}

#[test]
fn preview_follows_document_changes() {
    let mut e = doc(1);
    let n = |e: &mut Engine, rev: u32| -> u64 {
        let v: Value = serde_json::from_str(
            &e.print_preview_json(&format!(r#"{{"job":7,"revision":{rev}}}"#), 0)
                .unwrap(),
        )
        .unwrap();
        v["sheets"].as_u64().unwrap()
    };
    assert_eq!(n(&mut e, 1), 1);
    cmd(&mut e, r#"{"cmd":"move","motion":"docEnd"}"#);
    cmd(&mut e, r#"{"cmd":"insertPageBreak"}"#);
    // The panel sends the new revision: the sheets are made again.
    assert_eq!(n(&mut e, 2), 2);
}
