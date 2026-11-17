//! Round 12 tests of the BlueSheet window: speed probes on large books, PivotTable finish.
use super::*;

fn ms(t: std::time::Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// A window on the `.xlsx` named by `BO_PERF_XLSX`, or on 100 000 `=An*2+$D$1` formulas.
fn big_window() -> SheetWindow {
    let mut e = SheetEngine::with_bundled_fonts();
    if let Ok(path) = std::env::var("BO_PERF_XLSX") {
        let bytes = std::fs::read(&path).expect("book");
        let t = std::time::Instant::now();
        e.open(&bytes, "big.xlsx").expect("opens");
        eprintln!("open: {:.1} ms", ms(t));
    } else {
        {
            e.new_workbook("Book1.xlsx").expect("workbook");
            let cells: Vec<(u32, u32, String)> = (0..100_000u32)
                .flat_map(|r| {
                    [
                        (r, 0, format!("{}", r + 1)),
                        (r, 1, format!("=A{}*2+$D$1", r + 1)),
                    ]
                })
                .chain(std::iter::once((0, 3, "1".to_owned())))
                .collect();
            e.set_inputs(0, &cells).expect("cells");
        }
    }
    let mut w = window_with(e);
    w.view_mut().changed();
    w.handle(Input::Blink, &Fixed);
    let mut store = super::Raster::default();
    w.prepare(&mut store).expect("grid");
    w
}

/// Where Enter spends its time on a large book (`BO_PERF_XLSX=… cargo test --release -p bo-ui
/// --features bo-engine/bundled-fonts -- --ignored enter_phases --nocapture`).
#[test]
#[ignore = "timing probe"]
fn enter_phases_on_a_big_book() {
    let mut w = big_window();
    let mut store = super::Raster::default();
    let col = std::env::var("BO_PERF_COL")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(3u32);
    for i in 0..5 {
        w.view_mut().select((0, col), (0, col), (0, col));
        w.handle(Input::Text(format!("{}", i + 2)), &Fixed);
        let t = std::time::Instant::now();
        w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
        let enter = ms(t);
        let t = std::time::Instant::now();
        w.prepare(&mut store).expect("grid");
        eprintln!("Enter #{i}: handle {enter:.1} ms, grid {:.1} ms", ms(t));
    }
    let one = |name: &str, f: &mut dyn FnMut()| {
        let t = std::time::Instant::now();
        f();
        eprintln!("  {name}: {:.2} ms", ms(t));
    };
    one("engine set_input", &mut || {
        w.view
            .engine_mut()
            .set_input(0, 0, col, "9")
            .expect("input");
    });
    one("view.changed", &mut || w.view.changed());
    one("reload_breaks", &mut || w.view.reload_breaks());
    one("reload_layout", &mut || w.view.reload_layout());
    one("content_size", &mut || {
        let _ = w.view.engine_mut().content_size(0);
    });
    one("update_bars", &mut || w.update_bars());
    one("acc_issue_count", &mut || {
        let _ = w.acc_issue_count();
    });
    one("refresh_calc_state", &mut || w.refresh_calc_state());
    one("status_stat_parts", &mut || {
        let _ = w.status_stat_parts();
    });
    one("sync_pivot_pane", &mut || w.sync_pivot_pane());
    one("format_state", &mut || {
        let _ = w.format_state();
    });
    one("prepare (grid)", &mut || {
        w.prepare(&mut store).expect("grid");
    });
    w.view_mut().select((5, 0), (5, 0), (5, 0));
    one("arrow down (handle)", &mut || {
        w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    });
    one("prepare after arrow", &mut || {
        w.prepare(&mut store).expect("grid");
    });
}

/// Where a wheel notch spends its time on a large book (`BO_PERF_XLSX=…`, as above, filter
/// `wheel_phases`).
#[test]
#[ignore = "timing probe"]
fn wheel_phases_on_a_big_book() {
    let mut w = big_window();
    let mut store = super::Raster::default();
    let vp = w.view.viewport();
    let t = |name: &str, f: &mut dyn FnMut()| {
        let s = std::time::Instant::now();
        for _ in 0..10 {
            f();
        }
        eprintln!("{name}: {:.2} ms", s.elapsed().as_secs_f64() * 1e3 / 10.0);
    };
    t("render whole viewport", &mut || {
        let _ = w.view.engine_mut().render_viewport_with(0, &vp, |_| {});
    });
    let band = bo_sheet::viewport::Viewport {
        scroll_y: vp.scroll_y + 300.0,
        height: 45.0,
        ..vp
    };
    t("render a 3-row band", &mut || {
        let _ = w.view.engine_mut().render_viewport_with(0, &band, |_| {});
    });
    t("view_layout", &mut || {
        let _ = w.view.engine_mut().view_layout(0, &vp);
    });
    t("wheel notch (handle + prepare)", &mut || {
        w.handle(Input::Wheel(-120.0), &Fixed);
        w.prepare(&mut store).expect("grid");
    });
}

fn pivot_window() -> SheetWindow {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Region"),
            (0, 1, "Product"),
            (0, 2, "Sales"),
            (1, 0, "North"),
            (1, 1, "Rice"),
            (1, 2, "10"),
            (2, 0, "South"),
            (2, 1, "Rice"),
            (2, 2, "5"),
            (3, 0, "North"),
            (3, 1, "Tea"),
            (3, 2, "7"),
        ],
    );
    w.view_mut().select((1, 1), (1, 1), (1, 1));
    assert!(w.action("sheet.pivotTable", &Fixed));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w
}

fn shown(w: &SheetWindow, r: u32, c: u32) -> String {
    w.view()
        .engine()
        .cell(0, r, c)
        .map(|d| d.display)
        .unwrap_or_default()
}

fn names(w: &SheetWindow) -> Vec<String> {
    w.view().sheet_names().into_iter().flatten().collect()
}

#[test]
fn new_pivot_sheet_goes_left_of_its_source_and_fits_its_columns() {
    let mut w = pivot_window();
    assert_eq!(names(&w), vec!["Sheet2".to_owned(), "Sheet1".to_owned()]);
    assert_eq!(w.view().sheet(), 0);
    let width = |w: &SheetWindow| {
        w.view()
            .engine()
            .workbook()
            .and_then(|wb| wb.sheets[0].cols.iter().find(|c| c.min == 0))
            .and_then(|c| c.width)
            .unwrap_or(0.0)
    };
    w.pivot_toggle(0);
    w.pivot_toggle(2);
    // "Grand Total" fits: the first column has a fitted width.
    assert!(width(&w) > 0.0, "fitted");
    // One undo step takes back the field and the fitting.
    assert!(w.view_mut().undo());
    w.sync_pivot_pane();
    assert!(w.pivot.as_ref().is_some_and(|p| p.values.is_empty()));
    assert!(w.view_mut().undo());
    // Undoing the creation removes the sheet in front again.
    assert!(w.view_mut().undo());
    assert_eq!(names(&w), vec!["Sheet1".to_owned()]);
}

#[test]
fn area_item_menus_drags_and_value_field_settings() {
    let mut w = pivot_window();
    w.pivot_toggle(0);
    w.pivot_toggle(1);
    w.pivot_toggle(2);
    let p = w.pivot.clone().expect("pane");
    assert_eq!((p.rows.clone(), p.values.len()), (vec![0, 1], 1));
    // The item menu: Move Down, Move to Column Labels.
    assert!(w.run_command("pivot.item.2.0", &Fixed));
    assert!(w.menu.is_some());
    w.menu = None;
    assert!(w.run_command("pivotitem.down.2.0", &Fixed));
    assert_eq!(w.pivot.as_ref().map(|p| p.rows.clone()), Some(vec![1, 0]));
    assert!(w.run_command("pivotitem.to1.2.0", &Fixed));
    let p = w.pivot.clone().expect("pane");
    assert_eq!((p.rows.clone(), p.cols.clone()), (vec![1], vec![0]));
    // Dragging Product from Rows to Filters (pane reference pixels).
    w.pivot_drag = Some((2, 1, 40.0, 630.0));
    assert!(w.pivot_release(40.0, 500.0));
    let p = w.pivot.clone().expect("pane");
    assert_eq!((p.rows.clone(), p.filters.clone()), (vec![], vec![1]));
    // A click (no move) opens the menu.
    w.pivot_drag = Some((0, 1, 40.0, 500.0));
    assert!(w.pivot_release(41.0, 501.0));
    assert!(w.menu.is_some());
    w.menu = None;
    // Value Field Settings: Average, shown as % of Grand Total, custom name.
    assert!(w.run_command("pivotitem.settings.3.2", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::PivotValue);
    let f = w.form.as_mut().expect("dialog");
    assert_eq!(f.text("name"), "Sum of Sales");
    f.set_sel("fn", Some(2));
    w.value_settings_event(&form_impl::Event::Changed("fn"));
    assert_eq!(
        w.form.as_ref().map(|f| f.text("name")),
        Some("Average of Sales".into())
    );
    let f = w.form.as_mut().expect("dialog");
    f.set_sel("show", Some(1));
    w.value_settings_event(&form_impl::Event::Ok);
    assert!(w.form.is_none());
    let p = w.pivot.clone().expect("pane");
    assert_eq!(p.values, vec![(2, "average".to_owned())]);
    assert_eq!(
        p.opts.get(&2).map(|o| o.show_as.as_str()),
        Some("percentOfTotal")
    );
    let cells: Vec<String> = (0..8)
        .flat_map(|r| (0..4).map(move |c| (r, c)))
        .map(|(r, c)| shown(&w, r, c))
        .collect();
    assert!(cells.iter().any(|s| s.contains('%')), "{cells:?}");
    // A custom name sticks.
    assert!(w.run_command("pivotitem.settings.3.2", &Fixed));
    w.form
        .as_mut()
        .expect("dialog")
        .set_text("name", "Mean sales");
    w.value_settings_event(&form_impl::Event::Ok);
    assert_eq!(
        w.pivot
            .as_ref()
            .and_then(|p| p.opts.get(&2))
            .map(|o| o.name.clone()),
        Some("Mean sales".into())
    );
    // Remove Field.
    assert!(w.run_command("pivotitem.remove.3.2", &Fixed));
    assert!(w.pivot.as_ref().is_some_and(|p| p.values.is_empty()));
}
