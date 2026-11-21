//! Round 14 tests: Grouping, Calculated Field / Item, Field Settings of row fields and
//! PivotTable Options of the PivotTable Analyze tab.
use super::*;
use form_impl::{Event, FormKind};

/// A PivotTable over Date / Region / Qty / Sales with `row` in Rows and Sum of Sales, the
/// active cell on the first item.
fn book(row: u32) -> SheetWindow {
    let mut w = window();
    let mut cells: Vec<(u32, u32, String)> = ["Date", "Region", "Qty", "Sales"]
        .iter()
        .enumerate()
        .map(|(c, s)| (0, c as u32, (*s).to_owned()))
        .collect();
    let rows = [
        ("=DATE(2025,1,5)", "North", "3", "10"),
        ("=DATE(2025,2,10)", "South", "7", "5"),
        ("=DATE(2025,2,20)", "North", "12", "7"),
        ("=DATE(2025,7,1)", "East", "1", "4"),
        ("=DATE(2026,3,3)", "South", "9", "6"),
    ];
    for (i, (d, r, q, s)) in rows.iter().enumerate() {
        let r0 = i as u32 + 1;
        for (c, v) in [d, r, q, s].iter().enumerate() {
            cells.push((r0, c as u32, (**v).to_owned()));
        }
    }
    w.view_mut()
        .engine_mut()
        .set_inputs(0, &cells)
        .expect("cells");
    w.view_mut().changed();
    w.view_mut().select((1, 1), (1, 1), (1, 1));
    assert!(w.action("sheet.pivotTable", &Fixed));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.pivot_toggle(3);
    let at = first_item(&mut w);
    w.view_mut().select(at, at, at);
    assert!(w.pivot_update_settings(&json!({ "rows": [row] })));
    let at = first_item(&mut w);
    w.view_mut().select(at, at, at);
    w.sync_table_tab();
    w
}

fn info(w: &mut SheetWindow) -> serde_json::Value {
    w.view_mut()
        .query(json!({ "cmd": "pivots", "sheet": 0 }))
        .and_then(|v| v.get(0).cloned())
        .expect("pivot")
}

fn first_item(w: &mut SheetWindow) -> (u32, u32) {
    let loc = info(w)["location"].clone();
    let n = |k: &str| loc[k].as_u64().unwrap_or(0) as u32;
    (n("r0") + 1, n("c0"))
}

fn column(w: &SheetWindow, c: u32) -> Vec<String> {
    (0..16)
        .map(|r| {
            w.view()
                .engine()
                .cell(0, r, c)
                .map(|d| d.display)
                .unwrap_or_default()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

#[test]
fn grouping_dates_by_months_and_years_then_ungroup() {
    let mut w = book(0);
    assert!(w.pivot_tab);
    // Group Field is offered on a date field; Ungroup is not (nothing grouped yet).
    assert!(!w.pivot_menu_dim("pvt.groupField"));
    assert!(w.pivot_menu_dim("pvt.ungroup"));
    assert!(
        !w.ribbon.extra_dimmed("sheet.pvtTimeline"),
        "Insert Timeline is enabled"
    );
    assert!(w.action("pvt.groupField", &Fixed));
    assert_eq!(w.form_kind, FormKind::PivotGrouping);
    let f = w.form.as_ref().expect("form");
    assert_eq!(f.text("start"), "1/5/2025");
    assert_eq!(f.text("end"), "3/3/2026");
    // Months is proposed; add Years with a click in the By list (row 6).
    assert_eq!(
        w.pvt_dlg.as_ref().map(|d| d.by),
        Some([false, false, false, false, true, false, false])
    );
    w.pivot_dlg_event(
        FormKind::PivotGrouping,
        &Event::Canvas("by", 20.0, 6.5 * 17.0),
    );
    w.pivot_dlg_event(FormKind::PivotGrouping, &Event::Ok);
    assert!(w.form.is_none(), "{}", w.status);
    let col = column(&w, 0);
    assert!(col.iter().any(|t| t == "2025"), "{col:?}");
    assert!(col.iter().any(|t| t == "Feb"), "{col:?}");
    // Ungroup on a month item brings the dates back.
    let at = first_item(&mut w);
    w.view_mut()
        .select((at.0 + 1, 0), (at.0 + 1, 0), (at.0 + 1, 0));
    w.sync_table_tab();
    assert!(!w.pivot_menu_dim("pvt.ungroup"));
    assert!(w.action("pvt.ungroup", &Fixed));
    let col = column(&w, 0);
    assert!(!col.iter().any(|t| t == "Feb"), "{col:?}");
}

#[test]
fn grouping_numbers_by_ranges() {
    let mut w = book(2);
    assert!(
        w.action("pvt.groupSelection", &Fixed),
        "numbers open Grouping"
    );
    assert_eq!(w.form_kind, FormKind::PivotGrouping);
    if let Some(f) = w.form.as_mut() {
        f.set_text("byN", "5");
        f.set_text("start", "1");
        f.set_on("autoS", false);
    }
    w.pivot_dlg_event(FormKind::PivotGrouping, &Event::Ok);
    assert!(w.form.is_none(), "{}", w.status);
    let col = column(&w, 0);
    assert!(col.iter().any(|t| t == "1-5"), "{col:?}");
    assert!(col.iter().any(|t| t == "6-10"), "{col:?}");
}

#[test]
fn calculated_field_add_modify_and_delete() {
    let mut w = book(1);
    assert!(w.action("pvt.calcField", &Fixed));
    assert_eq!(w.form_kind, FormKind::PivotCalc);
    assert_eq!(
        w.form.as_ref().map(|f| f.text("name")),
        Some("Field1".into())
    );
    if let Some(f) = w.form.as_mut() {
        f.set_text("name", "Bonus");
        f.set_text("formula", "= 0");
        f.set_sel("fields", Some(3));
    }
    w.pivot_dlg_event(FormKind::PivotCalc, &Event::Clicked("insField"));
    assert_eq!(
        w.form.as_ref().map(|f| f.text("formula")),
        Some("= Sales".into())
    );
    if let Some(f) = w.form.as_mut() {
        f.set_text("formula", "= Sales*2");
    }
    w.pivot_dlg_event(FormKind::PivotCalc, &Event::Ok);
    assert!(w.form.is_none(), "{}", w.status);
    let i = info(&mut w);
    let names: Vec<String> = i["values"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| v["name"].as_str().unwrap_or("").to_owned())
        .collect();
    assert!(names.contains(&"Sum of Bonus".to_owned()), "{names:?}");
    // Reopened: typing the name loads its formula and offers Modify / Delete.
    let at = first_item(&mut w);
    w.view_mut().select(at, at, at);
    assert!(w.action("pvt.calcField", &Fixed));
    if let Some(f) = w.form.as_mut() {
        f.set_text("name", "Bonus");
    }
    w.pivot_dlg_event(FormKind::PivotCalc, &Event::Changed("name"));
    let f = w.form.as_ref().expect("form");
    assert_eq!(f.text("formula"), "= Sales*2");
    assert!(f.get("delete").is_some_and(|c| c.enabled));
    w.pivot_dlg_event(FormKind::PivotCalc, &Event::Clicked("delete"));
    w.pivot_dlg_event(FormKind::PivotCalc, &Event::Cancel);
    let i = info(&mut w);
    assert!(
        !i["fields"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|f| f["name"] == "Bonus"),
        "deleted"
    );
}

#[test]
fn calculated_item_adds_a_row() {
    let mut w = book(1);
    assert!(!w.pivot_menu_dim("pvt.calcItem"));
    assert!(w.action("pvt.calcItem", &Fixed));
    assert_eq!(w.form_kind, FormKind::PivotCalc);
    if let Some(f) = w.form.as_mut() {
        f.set_text("name", "NorthSouth");
        f.set_text("formula", "=North+South");
    }
    w.pivot_dlg_event(FormKind::PivotCalc, &Event::Ok);
    assert!(w.form.is_none(), "{}", w.status);
    assert!(column(&w, 0).contains(&"NorthSouth".to_owned()));
}

#[test]
fn field_settings_rename_subtotals_and_layout() {
    let mut w = book(1);
    assert!(w.action("sheet.pvtFieldSettings", &Fixed));
    assert_eq!(w.form_kind, FormKind::PivotFieldSettings);
    if let Some(f) = w.form.as_mut() {
        f.set_text("name", "Area");
        f.set_on("tabular", true);
    }
    w.pivot_dlg_event(FormKind::PivotFieldSettings, &Event::Changed("tabular"));
    assert!(!w.form.as_ref().is_some_and(|f| f.on("outline")));
    w.pivot_dlg_event(FormKind::PivotFieldSettings, &Event::Ok);
    assert!(w.form.is_none(), "{}", w.status);
    let i = info(&mut w);
    assert_eq!(i["layout"], "tabular");
    assert_eq!(i["fieldOptions"][1]["name"], "Area");
    assert!(column(&w, 0).contains(&"Area".to_owned()), "header renamed");
}

#[test]
fn pivot_options_rename_empty_caption_and_headers() {
    let mut w = book(1);
    assert!(!w.pivot_menu_dim("pvt.options"));
    assert!(w.action("pvt.options", &Fixed));
    assert_eq!(w.form_kind, FormKind::PivotOptions);
    if let Some(f) = w.form.as_mut() {
        f.set_text("name", "Sales by region");
        f.set_on("headers", false);
        f.set_on("grandCols", false);
    }
    w.pivot_dlg_event(FormKind::PivotOptions, &Event::Ok);
    assert!(w.form.is_none(), "{}", w.status);
    let i = info(&mut w);
    assert_eq!(i["name"], "Sales by region");
    assert_eq!(i["options"]["showHeaders"], false);
    let col = column(&w, 0);
    assert!(!col.contains(&"Row Labels".to_owned()), "{col:?}");
    assert!(!col.contains(&"Grand Total".to_owned()), "{col:?}");
}

#[test]
fn field_button_sorts_hides_items_and_filters_labels() {
    let mut w = book(1);
    let at = first_item(&mut w);
    let header = (at.0 - 1, at.1);
    let b = w.view().cell_box(header.0, header.1).expect("cell");
    let (x, y) = (b[2] - 4.0, f32::midpoint(b[1], b[3]));
    assert!(w.pivot_button_at(x, y).is_some(), "the Row Labels button");
    assert!(
        w.pivot_button_at(b[0] + 2.0, y).is_none(),
        "the caption is not the button"
    );
    let (s, i, f) = w.pivot_button_at(x, y).unwrap_or_default();
    assert!(w.open_pivot_filter(s, i, f));
    assert!(w
        .filter
        .as_ref()
        .is_some_and(|d| d.pivot && d.lines.len() == 3));
    assert_eq!(w.filter_labels.items[4], "Label Filters");
    // Sort Z to A.
    assert!(w.filter_item(1));
    assert_eq!(column(&w, 0)[1], "South");
    // Uncheck "North" and OK: hidden.
    assert!(w.open_pivot_filter(s, i, f));
    if let Some(d) = w.filter.as_mut() {
        let n = d.lines.iter().position(|l| l.text == "North").unwrap_or(0);
        d.lines[n].checked = false;
    }
    assert!(w.pivot_filter_ok());
    assert!(!column(&w, 0).contains(&"North".to_owned()));
    // Label filter "begins with E" leaves East only; Clear Filter brings all back.
    assert!(w.open_pivot_filter(s, i, f));
    assert!(w.filter.as_ref().is_some_and(|d| d.active));
    assert!(w.pivot_filter_choice("captionBeginsWith"));
    assert_eq!(w.form_kind, FormKind::PivotFilter);
    if let Some(fm) = w.form.as_mut() {
        fm.set_text("v1", "E");
    }
    w.pivot_filter_form_event(&Event::Ok);
    assert!(w.form.is_none(), "{}", w.status);
    let col = column(&w, 0);
    assert!(
        col.contains(&"East".to_owned()) && !col.contains(&"South".to_owned()),
        "{col:?}"
    );
    assert!(w.open_pivot_filter(s, i, f));
    assert!(w.filter_item(3));
    let col = column(&w, 0);
    assert!(
        col.contains(&"North".to_owned()) && col.contains(&"South".to_owned()),
        "{col:?}"
    );
}

#[test]
fn pane_layouts_from_the_gear_menu() {
    let mut w = book(1);
    w.sync_pivot_pane();
    assert!(w.pivot.is_some());
    assert!(w.pivot_gear_command("areas22"));
    assert_eq!(w.pivot.as_ref().map(|p| p.layout), Some(3));
    // The layout stays when the pane is rebuilt.
    w.sync_pivot_pane();
    assert_eq!(w.pivot.as_ref().map(|p| p.layout), Some(3));
    assert!(w.pivot_gear_command("stacked"));
    assert_eq!(w.pivot.as_ref().map(|p| p.layout), Some(0));
    // Sort A to Z is kept in the report (fieldListSortAscending).
    assert!(w.pivot_gear_command("az"));
    assert_eq!(info(&mut w)["options"]["fieldListSortAscending"], true);
}
