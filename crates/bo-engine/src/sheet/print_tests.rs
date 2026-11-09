//! SHEET-PRINT-001: pagination, titles, print areas, headers / footers,
//! headings, gridlines, errors, comments at end, PDF and round-trip.
//! Expected values are derived by hand from the inputs.

use super::*;
use bo_model::sheet::{Comment, TextRun};
use serde_json::{json, Value};

fn engine() -> SheetEngine {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").unwrap();
    e
}

#[allow(clippy::needless_pass_by_value)]
fn run(e: &mut SheetEngine, v: Value) -> Value {
    let s = e.exec_json(&v.to_string()).unwrap();
    serde_json::from_str(&s).unwrap()
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

fn has_text(p: &PageList, s: &str) -> bool {
    texts(p).iter().any(|t| t == s)
}

fn fixed(w: f64) -> impl Fn(u32) -> f64 {
    move |_| w
}

#[test]
fn page_order_both_ways() {
    // 5 columns × 100 pt, 10 rows × 20 pt on a 250 × 100 pt printable area:
    // 2 columns per page (200 ≤ 250 < 300) → A:B, C:D, E; 5 rows per page.
    let cw = fixed(100.0);
    let rh = fixed(20.0);
    let mut input = PageInput {
        col_w: &cw,
        row_h: &rh,
        area: CellRange::new(CellRef::new(0, 0), CellRef::new(9, 4)),
        avail: (250.0, 100.0),
        fixed: (0.0, 0.0),
        fit: None,
        scale_pct: 100,
        col_breaks: &[],
        row_breaks: &[],
        over_then_down: false,
    };
    let p = paginate(&input);
    assert!((p.scale - 1.0).abs() < 1e-12);
    assert_eq!(p.cols, vec![(0, 1), (2, 3), (4, 4)]);
    assert_eq!(p.rows, vec![(0, 4), (5, 9)]);
    // Down, then over: every row band of the first column band first.
    assert_eq!(
        p.order,
        vec![(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (2, 1)]
    );
    input.over_then_down = true;
    let p = paginate(&input);
    assert_eq!(
        p.order,
        vec![(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)]
    );
    // 50 %: 500 × 200 pt of sheet fit on one page.
    input.scale_pct = 50;
    let p = paginate(&input);
    assert_eq!((p.cols.len(), p.rows.len()), (1, 1));
    // Manual row break after row 3 (index 2): 0–2, then 5 rows, then the rest.
    input.scale_pct = 100;
    input.row_breaks = &[2];
    let p = paginate(&input);
    assert_eq!(p.rows, vec![(0, 2), (3, 7), (8, 9)]);
}

#[test]
fn fit_to_pages_scale() {
    let cw = fixed(100.0);
    let rh = fixed(20.0);
    // 6 columns (600 pt) × 10 rows (200 pt) on 250 × 100 pt, 2 pages wide, any height:
    // scale = 2·250 / 600 = 5/6 → 300 pt of sheet per page = 3 columns; rows: 100/(5/6) = 120 pt = 6 rows.
    let mut input = PageInput {
        col_w: &cw,
        row_h: &rh,
        area: CellRange::new(CellRef::new(0, 0), CellRef::new(9, 5)),
        avail: (250.0, 100.0),
        fixed: (0.0, 0.0),
        fit: Some((2, 0)),
        scale_pct: 100,
        col_breaks: &[],
        row_breaks: &[],
        over_then_down: false,
    };
    let p = paginate(&input);
    assert!((p.scale - 5.0 / 6.0).abs() < 1e-9, "{}", p.scale);
    assert_eq!(p.cols, vec![(0, 2), (3, 5)]);
    assert_eq!(p.rows, vec![(0, 5), (6, 9)]);
    // 1 × 1: min(250/600, 100/200) = 0.41666…
    input.fit = Some((1, 1));
    let p = paginate(&input);
    assert!((p.scale - 250.0 / 600.0).abs() < 1e-9);
    assert_eq!((p.cols.len(), p.rows.len()), (1, 1));
    // A 50 pt title column repeated on the single page: 250 / (600 + 50).
    input.fixed = (50.0, 0.0);
    let p = paginate(&input);
    assert!((p.scale - 250.0 / 650.0).abs() < 1e-9);
    // Fitting never enlarges: a small area stays at 100 %.
    input.area = CellRange::new(CellRef::new(0, 0), CellRef::new(1, 0));
    assert!((paginate(&input).scale - 1.0).abs() < 1e-12);
}

#[test]
fn titles_text_round_trip() {
    let t = titles_text("Sheet1", Some((0, 1)), Some((0, 0))).unwrap();
    assert_eq!(t, "Sheet1!$1:$2,Sheet1!$A:$A");
    assert_eq!(parse_titles(&t), (Some((0, 1)), Some((0, 0))));
    assert_eq!(titles_text("S", None, None), None);
    // Row numbers up to 9: one digit → 7 + 8; up to 10: two digits → 14 + 8.
    assert!((heading_width(8) - 15.0).abs() < 1e-12);
    assert!((heading_width(9) - 22.0).abs() < 1e-12);
}

fn filled(rows: u32, cols: u32) -> SheetEngine {
    let mut e = engine();
    for r in 0..rows {
        for c in 0..cols {
            e.set_input(0, r, c, &format!("r{}c{}", r + 1, c + 1))
                .unwrap();
        }
    }
    e
}

#[test]
fn titles_repeat_and_print_areas_start_new_pages() {
    let mut e = filled(120, 2);
    run(
        &mut e,
        json!({ "cmd": "setPageSetup", "sheet": 0, "titleRows": [0, 0], "titleCols": [0, 0] }),
    );
    let pages = e.print_pages(0).unwrap();
    assert!(pages.len() >= 2);
    // Row 1 and column A are on every page.
    for p in &pages {
        assert!(has_text(p, "r1c1") && has_text(p, "r1c2"), "title row");
    }
    assert!(has_text(&pages[1], "r120c1") || pages.len() > 2);
    // Two print areas: each small area gets its own page.
    run(
        &mut e,
        json!({ "cmd": "setPageSetup", "sheet": 0, "titleRows": null, "titleCols": null,
                "printAreas": [{ "r0": 0, "c0": 0, "r1": 1, "c1": 0 }, { "r0": 10, "c0": 1, "r1": 11, "c1": 1 }] }),
    );
    let ps = run(&mut e, json!({ "cmd": "pageSetup", "sheet": 0 }));
    assert_eq!(ps["printArea"], "Sheet1!$A$1:$A$2,Sheet1!$B$11:$B$12");
    let pages = e.print_pages(0).unwrap();
    assert_eq!(pages.len(), 2);
    assert!(has_text(&pages[0], "r2c1") && !has_text(&pages[0], "r11c2"));
    assert!(has_text(&pages[1], "r12c2") && !has_text(&pages[1], "r1c1"));
}

#[test]
fn header_footer_codes_with_fixed_clock() {
    let mut e = filled(120, 1);
    // 2026-11-10 08:30 UTC: 20 767 days after 1970-01-01 (56·365 + 14 leap days
    // + 313 days of 2026 before 10 November), plus 8.5 h.
    e.set_now_ms(20_767 * 86_400_000 + 30_600_000);
    run(
        &mut e,
        json!({ "cmd": "setPageSetup", "sheet": 0, "firstPageNumber": 5,
                "headerFooter": {
                    "oddHeader": "&L&A&C&F&RTrang &P / &N",
                    "oddFooter": "&C&D &T",
                    "evenHeader": "&CEVEN &P",
                    "firstHeader": "&CFIRST",
                    "differentOddEven": true,
                    "differentFirst": true } }),
    );
    let pages = e.print_pages(0).unwrap();
    let n = pages.len();
    assert!(n >= 3, "{n} pages");
    // Page 1 (number 5): first-page header; no first footer → none.
    assert!(has_text(&pages[0], "FIRST"));
    assert!(!has_text(&pages[0], "Sheet1"));
    // Page 2 (number 6): even header.
    assert!(has_text(&pages[1], "EVEN 6"));
    // Page 3 (number 7): odd header and footer.
    assert!(has_text(&pages[2], "Sheet1"));
    assert!(has_text(&pages[2], "Book1.xlsx"));
    assert!(has_text(&pages[2], &format!("Trang 7 / {n}")));
    assert!(
        has_text(&pages[2], "10/11/2026 08:30"),
        "{:?}",
        texts(&pages[2])
    );
    // Header text sits above the cells (top margin 0.75 in, header at 0.3 in).
    let y = pages[2]
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Glyphs { text, y, .. } if text == "Sheet1" => Some(*y),
            _ => None,
        })
        .unwrap();
    assert!(y > 21.6 && y < 54.0, "{y}");
}

#[test]
fn gridlines_and_headings_in_display_list() {
    let mut e = filled(3, 3);
    let grid = Rgba(0xDA, 0xDC, 0xE0, 255);
    let has_grid = |p: &PageList| {
        p.commands
            .iter()
            .any(|c| matches!(c, Command::Line { color, .. } if *color == grid))
    };
    let p = &e.print_pages(0).unwrap()[0];
    assert!(!has_grid(p));
    assert!(!has_text(p, "B"));
    run(
        &mut e,
        json!({ "cmd": "setPageSetup", "sheet": 0, "gridLines": true, "headings": true }),
    );
    let p = &e.print_pages(0).unwrap()[0];
    assert!(has_grid(p));
    for label in ["A", "B", "C", "1", "2", "3"] {
        assert!(has_text(p, label), "heading {label}");
    }
    // The cells start right of the 15 pt row-heading column (1 digit: 7 + 8)
    // and below the 14 pt column headings, at the 0.7 in / 0.75 in margins.
    let first_band = p
        .commands
        .iter()
        .find_map(|c| match c {
            Command::PushTransform { m } => Some(*m),
            _ => None,
        })
        .unwrap();
    assert!((f64::from(first_band[4]) - (50.4 + 15.0)).abs() < 1e-3);
    assert!((f64::from(first_band[5]) - (54.0 + 14.0)).abs() < 1e-3);
}

#[test]
fn centring_on_page() {
    let mut e = filled(1, 1);
    let w = e.geometry(0).unwrap().col_width(0);
    let h = e.geometry(0).unwrap().row_height(0);
    run(
        &mut e,
        json!({ "cmd": "setPageSetup", "sheet": 0, "centerH": true, "centerV": true }),
    );
    let p = &e.print_pages(0).unwrap()[0];
    let m = p
        .commands
        .iter()
        .find_map(|c| match c {
            Command::PushTransform { m } => Some(*m),
            _ => None,
        })
        .unwrap();
    // A4 595 × 842 pt; margins 50.4 / 50.4 / 54 / 54 pt.
    let x = 50.4 + (595.0 - 100.8 - w) / 2.0;
    let y = 54.0 + (842.0 - 108.0 - h) / 2.0;
    assert!((f64::from(m[4]) - x).abs() < 1e-2, "{} vs {x}", m[4]);
    assert!((f64::from(m[5]) - y).abs() < 1e-2, "{} vs {y}", m[5]);
}

#[test]
fn print_errors_modes() {
    let mut e = engine();
    e.set_input(0, 0, 0, "=1/0").unwrap();
    let first = |e: &mut SheetEngine| e.print_pages(0).unwrap().remove(0);
    assert!(has_text(&first(&mut e), "#DIV/0!"));
    for (mode, shown) in [("dash", Some("--")), ("NA", Some("#N/A")), ("blank", None)] {
        run(
            &mut e,
            json!({ "cmd": "setPageSetup", "sheet": 0, "errors": mode }),
        );
        let p = first(&mut e);
        assert!(!has_text(&p, "#DIV/0!"), "{mode}");
        if let Some(s) = shown {
            assert!(has_text(&p, s), "{mode}");
        } else {
            assert!(texts(&p).is_empty(), "{mode}");
        }
    }
}

#[test]
fn comments_at_end_page_and_pdf_page_count() {
    let mut e = filled(2, 1);
    e.wb.as_mut().unwrap().sheets[0].comments.push(Comment {
        cell: CellRef::new(1, 0),
        author: "An".into(),
        text: vec![TextRun {
            text: "Kiểm tra lại".into(),
            font: None,
        }],
    });
    assert_eq!(e.print_pages(0).unwrap().len(), 1);
    run(
        &mut e,
        json!({ "cmd": "setPageSetup", "sheet": 0, "comments": "atEnd" }),
    );
    let pages = e.print_pages(0).unwrap();
    assert_eq!(pages.len(), 2);
    assert!(has_text(&pages[1], "A2: An: Kiểm tra lại"));
    let pdf = e.export_pdf().unwrap();
    let s = String::from_utf8_lossy(&pdf);
    let count = s.matches("/Type /Page").count() - s.matches("/Type /Pages").count();
    assert_eq!(count, 2);
    // Print preview uses the same pages.
    let pv = run(
        &mut e,
        json!({ "cmd": "printPreview", "sheet": 0, "page": 1 }),
    );
    assert_eq!(pv["pages"], 2);
    assert!(pv["page"]["commands"]
        .as_array()
        .is_some_and(|c| !c.is_empty()));
}

#[test]
fn black_and_white_and_breaks() {
    let mut cmds = vec![
        Command::FillRect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            color: Rgba(255, 0, 0, 255),
        },
        Command::Line {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 0.0,
            width: 1.0,
            color: Rgba(0, 0, 255, 255),
            dash: None,
        },
    ];
    black_and_white(&mut cmds);
    assert!(
        matches!(cmds[0], Command::FillRect { color, .. } if color == Rgba(255, 255, 255, 255))
    );
    assert!(matches!(cmds[1], Command::Line { color, .. } if color == Rgba(0, 0, 0, 255)));
    // Manual breaks: before row 6 (index 5) and column C (index 2).
    let mut e = filled(10, 4);
    run(
        &mut e,
        json!({ "cmd": "pageBreak", "sheet": 0, "row": 5, "col": 2 }),
    );
    let ws = &e.wb.as_ref().unwrap().sheets[0];
    assert_eq!(
        (
            ws.page_setup.row_breaks.clone(),
            ws.page_setup.col_breaks.clone()
        ),
        (vec![4], vec![1])
    );
    // 2 row bands × 2 column bands.
    assert_eq!(e.print_pages(0).unwrap().len(), 4);
    run(
        &mut e,
        json!({ "cmd": "pageBreak", "sheet": 0, "row": 5, "remove": true }),
    );
    assert_eq!(e.print_pages(0).unwrap().len(), 2);
    run(&mut e, json!({ "cmd": "resetPageBreaks", "sheet": 0 }));
    assert_eq!(e.print_pages(0).unwrap().len(), 1);
}

#[test]
fn page_setup_round_trips_through_xlsx() {
    let mut e = filled(3, 3);
    run(
        &mut e,
        json!({ "cmd": "setPageSetup", "sheet": 0, "fit": [2, 3], "titleRows": [0, 0], "titleCols": [0, 1],
                "headings": true, "centerH": true, "centerV": true, "pageOrder": "overThenDown",
                "firstPageNumber": 3, "blackAndWhite": true, "draft": true, "errors": "dash",
                "comments": "atEnd", "hfMargins": [0.4, 0.5],
                "headerFooter": { "oddHeader": "&L&\"Arial,Bold\"&14A & B&&C", "oddFooter": "&CTrang &P",
                    "evenHeader": "&RE", "evenFooter": "&LEF", "firstHeader": "&CF", "firstFooter": "&CFF",
                    "differentOddEven": true, "differentFirst": true, "scaleWithDoc": false } }),
    );
    let before = run(&mut e, json!({ "cmd": "pageSetup", "sheet": 0 }));
    let bytes = e.save("xlsx", 0).unwrap();
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&bytes, "x.xlsx").unwrap();
    let after = run(&mut back, json!({ "cmd": "pageSetup", "sheet": 0 }));
    assert_eq!(before, after);
    assert_eq!(after["fit"], json!([2, 3]));
    assert_eq!(after["extra"]["titleCols"], json!([0, 1]));
    assert_eq!(after["extra"]["pageOrder"], "overThenDown");
    assert_eq!(after["extra"]["firstPageNumber"], 3);
    assert_eq!(after["extra"]["errors"], "dash");
    assert_eq!(after["extra"]["comments"], "atEnd");
    assert_eq!(after["extra"]["headerFooter"]["evenFooter"], "&LEF");
    assert_eq!(after["extra"]["headerFooter"]["scaleWithDoc"], false);
    assert_eq!(after["extra"]["hfMargins"], json!([0.4, 0.5]));
    // Clearing back to defaults removes the attributes.
    run(
        &mut back,
        json!({ "cmd": "setPageSetup", "sheet": 0, "pageOrder": "downThenOver", "firstPageNumber": null,
                "errors": "displayed", "comments": "none", "blackAndWhite": false, "draft": false }),
    );
    assert_eq!(
        back.wb.as_ref().unwrap().sheets[0].page_setup.attrs,
        Vec::<RawAttr>::new()
    );
}
