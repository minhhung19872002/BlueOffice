//! Round 13 tests of the BlueSheet window: the File screen and its print preview.
use super::*;
use backstage_impl::BsPage;

fn filled() -> SheetWindow {
    let mut w = window();
    let cells: Vec<(u32, u32, String)> = (0..120u32)
        .flat_map(|r| (0..4u32).map(move |c| (r, c, format!("{}", r * 10 + c))))
        .collect();
    w.view_mut()
        .engine_mut()
        .set_inputs(0, &cells)
        .expect("cells");
    w.view_mut().changed();
    w
}

#[test]
fn file_tab_opens_the_file_screen_and_escape_closes_it() {
    let mut w = filled();
    let mut store = super::Raster::default();
    w.prepare(&mut store).expect("grid");
    let r = w.ribbon.rect_of(RibbonHit::File).expect("file tab");
    let (x, y) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    w.handle(
        Input::Press {
            x,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x, y }, &Fixed);
    assert!(w.backstage_open());
    assert_eq!(
        w.bs.screen.as_ref().map(|b| b.page),
        Some(BsPage::Home),
        "the File screen opens on Home"
    );
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(!w.backstage_open());
}

#[test]
fn print_page_previews_the_sheet_and_follows_the_page_setup() {
    let mut w = filled();
    let mut store = super::Raster::default();
    w.prepare(&mut store).expect("grid");
    w.set_printers(vec!["Microsoft Print to PDF".into()], 0, Vec::new());
    assert!(w.open_backstage(BsPage::Print, &Fixed));
    let pages = w.bs.screen.as_ref().map_or(0, |b| b.facts.pages);
    assert!(
        pages >= 2,
        "120 rows print on more than one A4 page: {pages}"
    );
    let fulls = store.fulls;
    w.prepare(&mut store).expect("preview");
    assert_eq!(
        store.fulls,
        fulls + 1,
        "the preview page is drawn into the grid image"
    );
    // Its size is the preview rectangle's.
    let (_, r) =
        w.bs.screen
            .as_ref()
            .and_then(backstage_impl::SheetBackstage::preview_rect)
            .expect("preview rect");
    assert!((store.size.0 as f32 - r[2]).abs() <= 2.0);
    // Drawn once: a second prepare keeps it.
    w.prepare(&mut store).expect("preview");
    assert_eq!(store.fulls, fulls + 1);
    // Landscape from the orientation drop-down: the page turns, fewer pages.
    assert!(w.choose(Choice::Command("orient.landscape".into()), &Fixed));
    let b = w.bs.screen.as_ref().expect("screen");
    assert!(b.facts.landscape);
    let (pw, ph) = b.facts.page_sizes[0];
    assert!(pw > ph, "landscape preview page: {pw} x {ph}");
    // Fit Sheet on One Page: one page.
    assert!(w.choose(Choice::Command("bs13.scaling.1".into()), &Fixed));
    let b = w.bs.screen.as_ref().expect("screen");
    assert_eq!(b.facts.pages, 1);
    assert_eq!(b.print.scaling, 1);
    // Entire workbook / selection change what prints.
    w.view_mut().select((0, 0), (1, 1), (0, 0));
    assert!(w.choose(Choice::Command("bs13.scaling.0".into()), &Fixed));
    assert!(w.choose(Choice::Command("bs13.range.2".into()), &Fixed));
    assert_eq!(w.bs.screen.as_ref().map(|b| b.facts.pages), Some(1));
    // Print hands the job to the host.
    let pr =
        w.bs.screen
            .as_ref()
            .and_then(backstage_impl::SheetBackstage::rect_of_action)
            .expect("print button");
    w.handle(
        Input::Press {
            x: pr[0] + 5.0,
            y: pr[1] + 5.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    let jobs = w.take_jobs();
    assert!(
        matches!(&jobs[..], [SheetJob::Print { printer, pages, .. }]
            if printer == "Microsoft Print to PDF" && pages == &vec![0]),
        "{jobs:?}"
    );
    assert_eq!(w.job_pages(2).len(), 1);
    // Closing draws the sheet again.
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    let fulls = store.fulls;
    w.prepare(&mut store).expect("grid");
    assert_eq!(store.fulls, fulls + 1);
}

#[test]
fn templates_fill_a_new_workbook() {
    for (id, _, _) in backstage_impl::TEMPLATES {
        let mut w = window();
        assert!(w.load_template(id), "{id}");
        let title = w
            .view()
            .engine()
            .cell(0, 0, 0)
            .map(|d| d.display)
            .unwrap_or_default();
        assert!(!title.is_empty(), "{id} has a title");
    }
    let mut w = window();
    assert!(!w.load_template("nope"));
}

/// Wheel cost on a dense book (`BO_PERF_XLSX=… cargo test --release -p bo-ui --features
/// bo-engine/bundled-fonts --lib -- --ignored wheel_bands --nocapture`): laying out newly
/// exposed 3-row bands (new text each time, as when scrolling) and the same band again.
#[test]
#[ignore = "timing probe"]
fn wheel_bands_on_a_dense_book() {
    let mut e = SheetEngine::with_bundled_fonts();
    if let Ok(path) = std::env::var("BO_PERF_XLSX") {
        let bytes = std::fs::read(&path).expect("book");
        e.open(&bytes, "big.xlsx").expect("opens");
    } else {
        e.new_workbook("Book1.xlsx").expect("workbook");
        let cells: Vec<(u32, u32, String)> = (0..400u32)
            .flat_map(|r| {
                (0..100u32).map(move |c| (r, c, format!("{}", (r * 7919 + c * 104_729) % 99_991)))
            })
            .collect();
        e.set_inputs(0, &cells).expect("cells");
    }
    let mut w = window_with(e);
    w.view_mut().changed();
    let mut store = super::Raster::default();
    w.prepare(&mut store).expect("grid");
    let vp = w.view.viewport();
    let n = 40;
    let ms = |t: std::time::Instant| t.elapsed().as_secs_f64() * 1e3 / f64::from(n);
    let t = std::time::Instant::now();
    for i in 0..n {
        let band = bo_sheet::viewport::Viewport {
            scroll_y: vp.scroll_y + 45.0 * f64::from(i),
            height: 45.0,
            ..vp
        };
        let _ = w.view.engine_mut().render_viewport_with(0, &band, |_| {});
    }
    eprintln!("new 3-row band: {:.3} ms", ms(t));
    let band = bo_sheet::viewport::Viewport { height: 45.0, ..vp };
    let t = std::time::Instant::now();
    for _ in 0..n {
        let _ = w.view.engine_mut().render_viewport_with(0, &band, |_| {});
    }
    eprintln!("same 3-row band: {:.3} ms", ms(t));
    let t = std::time::Instant::now();
    let _ = w.view.engine_mut().render_viewport_with(0, &vp, |_| {});
    eprintln!("whole viewport: {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
}
