//! Round 13 tests: the PivotTable Analyze / Design tabs and the File screen's Info dates.
use super::*;

/// A workbook with a PivotTable (Region in Rows, Sum of Sales) made as Insert › PivotTable
/// does, the active cell in its "Row Labels" header.
fn pivot_book() -> SheetWindow {
    let mut w = window();
    let cells: Vec<(u32, u32, String)> = [
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
    ]
    .iter()
    .map(|(r, c, s)| (*r, *c, (*s).to_owned()))
    .collect();
    w.view_mut()
        .engine_mut()
        .set_inputs(0, &cells)
        .expect("cells");
    w.view_mut().changed();
    w.view_mut().select((1, 1), (1, 1), (1, 1));
    assert!(w.action("sheet.pivotTable", &Fixed));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.pivot_toggle(0);
    w.pivot_toggle(2);
    w
}

fn info(w: &mut SheetWindow) -> serde_json::Value {
    w.view_mut()
        .query(json!({ "cmd": "pivots", "sheet": 0 }))
        .and_then(|v| v.get(0).cloned())
        .expect("pivot")
}

fn texts(w: &SheetWindow) -> Vec<String> {
    (0..8)
        .map(|r| {
            (0..3)
                .map(|c| {
                    w.view()
                        .engine()
                        .cell(0, r, c)
                        .map(|d| d.display)
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect()
}

#[test]
fn pivot_tabs_show_in_a_pivot_table_and_hide_outside() {
    let mut w = pivot_book();
    let loc = info(&mut w)["location"].clone();
    let n = |k: &str| loc[k].as_u64().unwrap_or(0) as u32;
    w.view_mut()
        .select((n("r0"), n("c0")), (n("r0"), n("c0")), (n("r0"), n("c0")));
    w.handle(Input::Key(Key::Right, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Left, Mods::default()), &Fixed);
    assert!(w.pivot_tab, "the PivotTable tabs show in the report");
    assert_eq!(
        w.ribbon.contextual_labels(),
        vec!["sheetPvt.tabAnalyze", "sheetPvt.tabDesign"]
    );
    // The Active Field box names the field of the active cell.
    w.view_mut().select(
        (n("r0") + 1, n("c0")),
        (n("r0") + 1, n("c0")),
        (n("r0") + 1, n("c0")),
    );
    w.sync_table_tab();
    assert_eq!(w.pivot_field_caption, "Region");
    // Outside the report the tabs go.
    w.view_mut().select((30, 8), (30, 8), (30, 8));
    w.sync_table_tab();
    assert!(!w.pivot_tab);
    assert_eq!(w.ribbon.contextual_labels(), Vec::<&str>::new());
}

#[test]
fn design_tab_sets_layout_totals_and_style_options_in_one_undo_step_each() {
    let mut w = pivot_book();
    let loc = info(&mut w)["location"].clone();
    let n = |k: &str| loc[k].as_u64().unwrap_or(0) as u32;
    let at = (n("r0") + 1, n("c0"));
    w.view_mut().select(at, at, at);
    w.sync_table_tab();
    assert!(w.pivot_tab);
    // Report Layout menu: Compact is ticked; Tabular changes the header to the field name.
    assert_eq!(
        w.pivot_menu_current("sheet.pvtReportLayout"),
        vec!["pvt.layoutCompact".to_owned()]
    );
    assert_eq!(pivottabs_impl::pivot_menu("sheet.pvtGrandTotals").len(), 4);
    let before = texts(&w);
    assert!(w.action("pvt.layoutTabular", &Fixed));
    assert_eq!(info(&mut w)["layout"], "tabular");
    assert!(
        texts(&w).iter().any(|t| t.contains("Region|")),
        "{:?}",
        texts(&w)
    );
    assert!(w.view_mut().undo());
    assert_eq!(texts(&w), before, "one undo step");
    // Grand Totals off removes the Grand Total line.
    w.view_mut().select(at, at, at);
    assert!(w.action("pvt.grandOff", &Fixed));
    let i = info(&mut w);
    assert_eq!(
        (i["rowGrandTotals"].as_bool(), i["colGrandTotals"].as_bool()),
        (Some(false), Some(false))
    );
    assert!(!texts(&w).iter().any(|t| t.contains("Grand Total")));
    // The value field keeps its default caption (not turned into a custom name).
    assert_eq!(i["values"][0]["name"], "Sum of Sales");
    // Banded Rows check box.
    assert!(w.pivot_checks().contains(&("sheet.pvtBandedRows", false)));
    assert!(w.action("sheet.pvtBandedRows", &Fixed));
    assert!(w.pivot_checks().contains(&("sheet.pvtBandedRows", true)));
    // A style tile sets the report's style.
    assert!(w.action(pivottabs_impl::STYLE_TILES[3], &Fixed));
    assert_eq!(info(&mut w)["style"]["name"], "PivotStyleLight17");
    // Subtotals: Do Not Show is ticked after choosing it.
    assert!(w.action("pvt.subNone", &Fixed));
    assert_eq!(
        w.pivot_menu_current("sheet.pvtSubtotals"),
        vec!["pvt.subNone".to_owned()]
    );
}

#[test]
fn analyze_tab_moves_and_selects_the_report() {
    let mut w = pivot_book();
    let loc = info(&mut w)["location"].clone();
    let n = |k: &str| loc[k].as_u64().unwrap_or(0) as u32;
    let at = (n("r0") + 1, n("c0"));
    w.view_mut().select(at, at, at);
    w.sync_table_tab();
    assert!(w.action("pvt.selectAll", &Fixed));
    let (r0, c0, _, _) = w.view().range_bounds();
    assert_eq!((r0, c0), (n("r0"), n("c0")));
    w.view_mut().select(at, at, at);
    assert!(w.action("pvt.move", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::PivotMove);
    if let Some(f) = w.form.as_mut() {
        f.set_text("dest", "=$F$10");
    }
    w.pivot_tab_form_event(form_impl::FormKind::PivotMove, &form_impl::Event::Ok);
    assert!(w.form.is_none(), "{}", w.status);
    let moved = info(&mut w)["location"].clone();
    assert_eq!(
        (moved["r0"].as_u64(), moved["c0"].as_u64()),
        (Some(9), Some(5))
    );
}

#[test]
fn info_dates_show_in_local_time() {
    assert_eq!(
        bshost_impl::local_stamp_at("2026-11-16T05:54:29Z", 420),
        "11/16/2026 12:54 PM"
    );
    assert_eq!(
        bshost_impl::local_stamp_at("2026-12-31T20:05:00Z", 420),
        "1/1/2027 3:05 AM"
    );
    assert_eq!(bshost_impl::local_stamp_at("soon", 420), "soon");
}
