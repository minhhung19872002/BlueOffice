//! Chart element options (CORE-CHART-005, the "Format chart" pane) through the chart design
//! commands of the three apps: BlueSheet `chartDesign`, BlueSlide `setChart`, BlueDoc `setChart`
//! — apply, read back (`elements`), undo as one step, save, reopen.
//!
//! With `BO_CHART_OUT=<dir>` the written files are kept there (for OpenXmlValidator:
//! `bo-cli validate <dir> --openxml require`).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::sheet::SheetEngine;
use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::sheet::{Chart, ChartColor};
use serde_json::{json, Value};
use std::path::PathBuf;

fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("BO_CHART_OUT") {
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(PathBuf::from(dir).join(name), bytes).unwrap();
    }
}

fn exec(e: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap()
}

fn sheet_with_chart(kind: &str) -> SheetEngine {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").unwrap();
    let data = [
        ["Tỉnh", "2024", "2025"],
        ["HN", "10", "120"],
        ["HUE", "5", "40"],
        ["DN", "7", "900"],
        ["CT", "3", "65"],
    ];
    for (r, row) in data.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            e.set_input(0, r as u32, c as u32, v).unwrap();
        }
    }
    exec(
        &mut e,
        &json!({ "cmd": "insertChart", "sheet": 0,
            "range": { "r0": 0, "c0": 0, "r1": 4, "c1": 2 }, "kind": kind }),
    );
    e
}

fn elements(e: &mut SheetEngine, patch: &Value) -> Value {
    exec(
        e,
        &json!({ "cmd": "chartDesign", "sheet": 0, "index": 0, "elements": patch }),
    )
}

fn sheet_chart(e: &SheetEngine) -> Chart {
    e.workbook().unwrap().sheets[0].charts[0].clone()
}

fn info(e: &mut SheetEngine) -> Value {
    exec(e, &json!({ "cmd": "charts", "sheet": 0 }))[0]["elements"].clone()
}

fn chart_xml(bytes: &[u8], prefix: &str) -> String {
    let p = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
    let part = p
        .parts()
        .iter()
        .find(|x| x.name.starts_with(prefix))
        .unwrap();
    String::from_utf8(part.data.clone()).unwrap()
}

// source: ECMA-376 Part 1, §21.2.2 — c:scaling (logBase, max), c:minorGridlines, c:numFmt,
// c:dLbls, c:trendline (poly, order, dispEq, dispRSqr), c:errBars (percentage).
#[test]
fn sheet_axis_labels_trendline_and_error_bars() {
    let mut e = sheet_with_chart("column");
    let before = sheet_chart(&e);
    elements(
        &mut e,
        &json!({ "axes": { "val": { "logBase": 10, "max": 1000, "minorGrid": true,
            "numFmt": "#,##0", "majorTick": "cross" } } }),
    );
    let c = sheet_chart(&e);
    assert_eq!((c.axes.log_base, c.axes.max), (Some(10.0), Some(1000.0)));
    assert!(c.axes.val.minor_grid);
    let i = info(&mut e);
    assert_eq!(i["axes"]["val"]["logBase"], 10.0);
    assert_eq!(i["axes"]["val"]["max"], 1000.0);
    assert_eq!(i["axes"]["val"]["numFmt"], "#,##0");
    assert_eq!(i["axes"]["val"]["majorTick"], "cross");
    assert_eq!(i["axes"]["val"]["kind"], "value");
    assert_eq!(i["axes"]["cat"]["kind"], "category");
    assert_eq!(i["series"][1]["name"], "2025");
    // One undo step per change.
    assert!(e.undo().unwrap());
    assert_eq!(sheet_chart(&e).axes, before.axes);
    assert!(e.redo().unwrap());
    elements(
        &mut e,
        &json!({ "series": [{ "index": 1,
            "labels": { "value": true, "pos": "outEnd", "numFmt": "0.0" },
            "pointLabels": [{ "point": 2, "labels": { "value": true, "category": true, "separator": ": " } }],
            "trendlines": [{ "kind": "poly", "order": 2, "dispEq": true, "dispR2": true, "name": "Bậc 2" }],
            "errorBars": { "kind": "both", "valueType": "percentage", "value": 5, "noEndCap": true } }] }),
    );
    let c = sheet_chart(&e);
    let s = &c.plots[0].series[1];
    assert!(s
        .labels
        .as_ref()
        .is_some_and(|d| d.value && d.num_fmt.as_deref() == Some("0.0")));
    assert_eq!(s.point_labels.len(), 1);
    assert_eq!(
        (s.trendlines[0].kind.as_str(), s.trendlines[0].order),
        ("poly", 2)
    );
    assert!(s.trendlines[0].disp_eq && s.trendlines[0].disp_r2);
    assert_eq!(
        s.err_bars
            .as_ref()
            .map(|b| (b.value_type.as_str(), b.value, b.no_end_cap)),
        Some(("percentage", 5.0, true))
    );
    let i = info(&mut e);
    assert_eq!(i["series"][1]["trendlines"][0]["dispEq"], true);
    assert_eq!(i["series"][1]["errorBars"]["valueType"], "percentage");
    assert_eq!(i["series"][1]["pointLabels"][0]["point"], 2);
    assert!(e.undo().unwrap());
    let u = sheet_chart(&e);
    assert_eq!(u.plots[0].series[1].trendlines.len(), 0);
    // The axis change before it stays.
    assert_eq!(u.axes.log_base, Some(10.0));
    assert!(e.redo().unwrap());
    // Saved: the part has the elements; reopened: the options are read back.
    let bytes = e.save("xlsx", 0).unwrap();
    keep("chart-elements-sheet-1.xlsx", &bytes);
    let xml = chart_xml(&bytes, "/xl/charts/chart");
    for want in [
        "<c:logBase val=\"10\"/>",
        "<c:max val=\"1000\"/>",
        "<c:minorGridlines/>",
        "<c:numFmt formatCode=\"#,##0\" sourceLinked=\"0\"/>",
        "<c:trendlineType val=\"poly\"/><c:order val=\"2\"/>",
        "<c:dispRSqr val=\"1\"/><c:dispEq val=\"1\"/>",
        "<c:errValType val=\"percentage\"/><c:noEndCap val=\"1\"/><c:val val=\"5\"/>",
        "<c:separator>: </c:separator>",
    ] {
        assert!(xml.contains(want), "{want}\n{xml}");
    }
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&bytes, "back.xlsx").unwrap();
    let r = sheet_chart(&back);
    assert_eq!(r.axes, c.axes);
    assert_eq!(r.plots[0].series[1].trendlines, s.trendlines);
    assert_eq!(r.plots[0].series[1].err_bars, s.err_bars);
    assert_eq!(r.plots[0].series[1].labels, s.labels);
    assert_eq!(r.plots[0].series[1].point_labels, s.point_labels);
    // `null` returns an option to automatic; an empty list removes the trend lines.
    elements(
        &mut back,
        &json!({ "axes": { "val": { "logBase": null, "max": null } },
            "series": [{ "index": 1, "trendlines": [], "errorBars": null, "labels": null }] }),
    );
    let r = sheet_chart(&back);
    assert_eq!((r.axes.log_base, r.axes.max), (None, None));
    assert!(r.plots[0].series[1].trendlines.is_empty() && r.plots[0].series[1].err_bars.is_none());
    assert!(r.plots[0].series[1].labels.is_none());
}

// source: the options the schema bounds (ST_LogBase 2..1000, min < max, ST_Overlap −100..100) and
// the elements a chart type does not have are refused; the chart stays as it was.
#[test]
fn sheet_bad_options_are_refused() {
    let mut e = sheet_with_chart("column");
    let before = sheet_chart(&e);
    for bad in [
        json!({ "axes": { "val": { "logBase": 1 } } }),
        json!({ "axes": { "val": { "min": 5, "max": 5 } } }),
        json!({ "axes": { "val": { "logBase": 10, "min": 0 } } }),
        json!({ "axes": { "val2": { "max": 5 } } }),
        json!({ "axes": { "cat": { "max": 5 } } }),
        json!({ "axes": { "val": { "majorTick": "sideways" } } }),
        json!({ "groups": [{ "overlap": 150 }] }),
        json!({ "groups": [{ "hiLowLines": true }] }),
        json!({ "series": [{ "index": 9, "smooth": false }] }),
        json!({ "series": [{ "index": 0, "smooth": true }] }),
        json!({ "series": [{ "index": 0, "trendlines": [{ "kind": "cubic" }] }] }),
        json!({ "series": [{ "index": 0, "errorBars": { "kind": "both", "valueType": "cust" } }] }),
        json!({ "legend": { "pos": "middle" } }),
        json!({ "chartArea": { "fill": "bluish" } }),
    ] {
        let cmd = json!({ "cmd": "chartDesign", "sheet": 0, "index": 0, "elements": bad });
        assert!(e.exec_json(&cmd.to_string()).is_err(), "{bad}");
        assert_eq!(sheet_chart(&e), before, "{bad}");
    }
    // A pie has neither axes nor trend lines.
    let mut pie = sheet_with_chart("pie");
    for bad in [
        json!({ "axes": { "val": { "max": 5 } } }),
        json!({ "series": [{ "index": 0, "trendlines": [{ "kind": "linear" }] }] }),
        json!({ "dataTable": { "show": true } }),
    ] {
        let cmd = json!({ "cmd": "chartDesign", "sheet": 0, "index": 0, "elements": bad });
        assert!(pie.exec_json(&cmd.to_string()).is_err(), "{bad}");
    }
    // …but slices can be pulled out and labelled with percentages.
    elements(
        &mut pie,
        &json!({ "series": [{ "index": 0, "explosion": 20,
            "labels": { "percent": true, "category": true, "pos": "outEnd", "leaderLines": false } }] }),
    );
    let c = sheet_chart(&pie);
    assert_eq!(c.plots[0].series[0].explosion, 20);
    assert_eq!(
        c.plots[0].series[0].labels.as_ref().unwrap().leader_lines,
        Some(false)
    );
}

// source: a series on the secondary axis is a chart group of its own referring to a second pair
// of axes (c:valAx crossing at the maximum); its colour does not change with the move.
#[test]
fn sheet_series_on_the_secondary_axis() {
    let mut e = sheet_with_chart("column");
    elements(
        &mut e,
        &json!({ "series": [{ "index": 1, "secondary": true }] }),
    );
    let c = sheet_chart(&e);
    assert_eq!(c.plots.len(), 2);
    assert!(!c.plots[0].secondary && c.plots[1].secondary);
    // The colours the series had as the first and second of the chart.
    let rgb = |i: usize| {
        let bo_render::Rgba(r, g, b, _) = bo_sheet::chart::PALETTE[i];
        ChartColor::Rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b))
    };
    assert_eq!(c.plots[0].series[0].fill, Some(rgb(0)));
    assert_eq!(c.plots[1].series[0].fill, Some(rgb(1)));
    elements(
        &mut e,
        &json!({ "axes": { "val2": { "min": 0, "max": 1000, "majorUnit": 250, "title": "Năm 2025" } } }),
    );
    let i = info(&mut e);
    assert_eq!(i["has"]["secondary"], true);
    assert_eq!(i["axes"]["val2"]["majorUnit"], 250.0);
    assert_eq!(i["axes"]["val2"]["title"], "Năm 2025");
    assert_eq!(i["series"][1]["secondary"], true);
    let bytes = e.save("xlsx", 0).unwrap();
    keep("chart-elements-sheet-2.xlsx", &bytes);
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&bytes, "back.xlsx").unwrap();
    let r = sheet_chart(&back);
    assert!(r.plots[1].secondary);
    assert_eq!(r.axes.val2, sheet_chart(&e).axes.val2);
    // The last series on the primary axis cannot leave it.
    let cmd = json!({ "cmd": "chartDesign", "sheet": 0, "index": 0,
        "elements": { "series": [{ "index": 0, "secondary": true }] } });
    assert!(back.exec_json(&cmd.to_string()).is_err());
    // Back on the primary axis the groups merge and the secondary options go.
    elements(
        &mut back,
        &json!({ "series": [{ "index": 1, "secondary": false }] }),
    );
    let r = sheet_chart(&back);
    assert_eq!(r.plots.len(), 1);
    assert_eq!(r.plots[0].series.len(), 2);
    assert_eq!(r.axes.val2, bo_model::sheet::AxisOptions::default());
}

// source: every remaining group through one command each: titles and legend (c:overlay), chart /
// plot area and gridline format, axis options, c:gapWidth / c:overlap, c:dTable, the chart's
// c:dLbls, c:dropLines / c:hiLowLines / c:upDownBars of a line chart, c:marker, c:smooth.
#[test]
fn sheet_every_other_group_round_trips() {
    let mut e = sheet_with_chart("column");
    elements(
        &mut e,
        &json!({
            "title": { "text": "Doanh thu", "overlay": true },
            "legend": { "pos": "t", "overlay": true },
            "chartArea": { "fill": "#F4F6F8", "line": "#99A1AB", "rounded": true },
            "plotArea": { "fill": "#FFFFFF" },
            "gridlines": { "color": "#C0C6CE", "width": 0.5 },
            "axes": {
                "cat": { "title": "Tỉnh", "reverse": true, "majorGrid": true, "labelPos": "low" },
                "val": { "title": "Tỷ đồng", "min": 0, "max": 1000, "majorUnit": 250, "minorUnit": 50,
                    "dispUnit": "hundreds", "dispUnitLabel": true, "minorTick": "in", "crosses": "max" }
            },
            "groups": [{ "gapWidth": 60, "overlap": -20 }],
            "dataTable": { "show": true, "keys": false },
            "labels": { "value": true, "pos": "inEnd", "separator": ", " }
        }),
    );
    let c = sheet_chart(&e);
    assert_eq!(c.title.as_deref(), Some("Doanh thu"));
    assert!(c.title_overlay && c.legend_overlay && c.format.rounded);
    assert_eq!(
        (c.x_title.as_deref(), c.y_title.as_deref()),
        (Some("Tỉnh"), Some("Tỷ đồng"))
    );
    assert_eq!(c.format.grid_w, Some(6_350));
    assert_eq!(c.plots[0].overlap, Some(-20));
    assert!(c.axes.data_table && c.axes.table_no_keys);
    let bytes = e.save("xlsx", 0).unwrap();
    keep("chart-elements-sheet-3.xlsx", &bytes);
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&bytes, "back.xlsx").unwrap();
    let r = sheet_chart(&back);
    assert_eq!(r.axes, c.axes);
    assert_eq!((r.title_overlay, r.legend_overlay), (true, true));
    assert_eq!(
        (r.legend.as_deref(), r.format.gap_width),
        (Some("t"), Some(60))
    );
    assert_eq!(r.format.data_labels, c.format.data_labels);
    assert_eq!(r.format.chart_fill, Some(ChartColor::Rgb(0xF4_F6F8)));
    assert_eq!(r.format.grid_color, Some(ChartColor::Rgb(0xC0_C6CE)));
    let i = info(&mut back);
    assert_eq!(i["title"], json!({ "text": "Doanh thu", "overlay": true }));
    assert_eq!(i["legend"], json!({ "pos": "t", "overlay": true }));
    assert_eq!(i["dataTable"], json!({ "show": true, "keys": false }));
    assert_eq!(i["groups"][0]["overlap"], -20);
    assert_eq!(i["groups"][0]["gapWidth"], 60);
    assert_eq!(i["gridlines"]["width"], 0.5);
    assert_eq!(i["axes"]["val"]["dispUnit"], "hundreds");
    assert_eq!(i["axes"]["val"]["crosses"], "max");
    assert_eq!(i["axes"]["cat"]["reverse"], true);

    // A line chart on dates: lines, bars, markers, smoothing and the date axis.
    let mut e = sheet_with_chart("line");
    let plus_ref = format!("'{}'!$B$2:$B$5", e.workbook().unwrap().sheets[0].name);
    elements(
        &mut e,
        &json!({
            "groups": [{ "dropLines": true, "hiLowLines": true, "upDownBars": true }],
            "axes": { "cat": { "kind": "date", "baseUnit": "months" } },
            "series": [
                { "index": 0, "line": "#C0504D", "lineWidth": 3, "smooth": true,
                  "marker": { "symbol": "diamond", "size": 9 },
                  "trendlines": [{ "kind": "movingAvg", "order": 2 },
                      { "kind": "exp", "forward": 1.5, "intercept": 4 },
                      { "kind": "linear", "backward": 0.5, "intercept": 0 },
                      { "kind": "log" }, { "kind": "power", "dispR2": true }] },
                { "index": 1, "errorBars": { "kind": "minus", "valueType": "cust",
                    "plusRef": format!("={plus_ref}"), "minus": [1, 2, 3, 4] } }
            ]
        }),
    );
    let c = sheet_chart(&e);
    assert!(c.plots[0].drop_lines && c.plots[0].hi_low_lines && c.plots[0].flag);
    assert_eq!(c.plots[0].series[0].line_w, Some(38_100));
    assert_eq!(c.plots[0].series[0].trendlines.len(), 5);
    assert_eq!(
        c.plots[0].series[1]
            .err_bars
            .as_ref()
            .unwrap()
            .plus_ref
            .as_deref(),
        Some(plus_ref.as_str())
    );
    let bytes = e.save("xlsx", 0).unwrap();
    keep("chart-elements-sheet-4.xlsx", &bytes);
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&bytes, "back.xlsx").unwrap();
    let r = sheet_chart(&back);
    assert!(r.axes.date && r.axes.cat.base_unit.as_deref() == Some("months"));
    assert!(r.plots[0].drop_lines && r.plots[0].hi_low_lines && r.plots[0].flag);
    let (s, rs) = (&c.plots[0].series[0], &r.plots[0].series[0]);
    assert_eq!(rs.trendlines, s.trendlines);
    assert_eq!(rs.marker, s.marker);
    assert!(rs.smooth);
    assert_eq!(rs.line, Some(ChartColor::Rgb(0xC0_504D)));
    // The custom amounts above are cached from their range (10, 5, 7, 3) when saving.
    let eb = r.plots[0].series[1].err_bars.clone().unwrap();
    assert_eq!(eb.plus, vec![Some(10.0), Some(5.0), Some(7.0), Some(3.0)]);
    assert_eq!(eb.minus, vec![Some(1.0), Some(2.0), Some(3.0), Some(4.0)]);
    let i = info(&mut back);
    assert_eq!(i["series"][0]["lineWidth"], 3.0);
    assert_eq!(
        i["series"][0]["marker"],
        json!({ "symbol": "diamond", "size": 9 })
    );
    assert_eq!(i["series"][0]["trendlines"][1]["forward"], 1.5);
    assert_eq!(i["axes"]["cat"]["kind"], "date");
    assert_eq!(i["groups"][0]["upDownBars"], true);
}

// source: a quick layout sets the data labels of the whole chart (CORE-CHART-007): the labels a
// series or a point had of its own give way, so a layout without labels shows none.
#[test]
fn sheet_layout_replaces_series_labels() {
    let mut e = sheet_with_chart("column");
    elements(
        &mut e,
        &json!({ "series": [{ "index": 0, "labels": { "value": true },
            "pointLabels": [{ "point": 1, "labels": { "category": true } }] }] }),
    );
    assert!(sheet_chart(&e).plots[0].series[0].labels.is_some());
    exec(
        &mut e,
        &json!({ "cmd": "chartDesign", "sheet": 0, "index": 0, "layout": "layout1" }),
    );
    let c = sheet_chart(&e);
    assert_eq!(c.plots[0].series[0].labels, None);
    assert_eq!(c.plots[0].series[0].point_labels.len(), 0);
    assert_eq!(c.format.data_labels, None);
}

fn slide_run(e: &mut Engine, v: &Value) -> SlideEditResult {
    let cmd: SlideCommand = serde_json::from_value(v.clone()).unwrap();
    let r = e.slide_command(cmd).unwrap();
    assert!(r.error.is_none(), "{v}: {:?}", r.error);
    r
}

fn slide_chart(e: &Engine) -> Chart {
    let p = e.presentation().unwrap();
    let part = p
        .parts
        .iter()
        .find(|x| x.name.starts_with("/ppt/charts/chart"))
        .unwrap();
    bo_xlsx::chart_write::parse_chart_part(&part.data, &part.name).unwrap()
}

// source: the same `elements` patch through BlueSlide's `setChart` (one undo step), kept by a
// later change of the data grid, saved in the chart part of the deck.
#[test]
fn slide_elements_through_set_chart() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide/slide-text-bullets.pptx");
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(p).unwrap(), "deck.pptx").unwrap();
    let data = json!({ "categories": ["Q1", "Q2", "Q3", "Q4"],
        "series": [{ "name": "A", "values": [10, 20, 40, 80] }, { "name": "B", "values": [3, 2, 5, 4] }] });
    let r = slide_run(
        &mut e,
        &json!({ "cmd": "insertChart", "kind": "line", "title": "Doanh thu", "data": data }),
    );
    let info = r.chart.unwrap().elements;
    assert_eq!(info["series"].as_array().unwrap().len(), 2);
    assert_eq!(info["has"]["axes"], true);
    let r = slide_run(
        &mut e,
        &json!({ "cmd": "setChart", "elements": {
            "axes": { "val": { "logBase": 2, "max": 128 } },
            "groups": [{ "dropLines": true }],
            "dataTable": { "show": true },
            "series": [
                { "index": 0, "smooth": true, "marker": { "symbol": "square", "size": 7 },
                  "trendlines": [{ "kind": "exp", "dispEq": true }],
                  "labels": { "value": true, "pos": "t" } },
                { "index": 1, "errorBars": { "kind": "both", "valueType": "cust", "plus": [1, 1, 2, 2], "minus": [0.5] } }
            ] } }),
    );
    let info = r.chart.unwrap().elements;
    assert_eq!(info["axes"]["val"]["logBase"], 2.0);
    assert_eq!(info["series"][0]["trendlines"][0]["kind"], "exp");
    assert_eq!(info["series"][0]["marker"]["symbol"], "square");
    assert_eq!(
        info["series"][1]["errorBars"]["plus"],
        json!([1.0, 1.0, 2.0, 2.0])
    );
    assert_eq!(info["dataTable"]["show"], true);
    let c = slide_chart(&e);
    assert!(c.plots[0].drop_lines && c.plots[0].series[0].smooth);
    // One undo step for the whole change.
    slide_run(&mut e, &json!({ "cmd": "undo" }));
    let u = slide_chart(&e);
    assert!(u.plots[0].series[0].trendlines.is_empty() && u.axes.log_base.is_none());
    slide_run(&mut e, &json!({ "cmd": "redo" }));
    // A new data grid keeps the options of the series.
    let data = json!({ "categories": ["Q1", "Q2", "Q3", "Q4"],
        "series": [{ "name": "A", "values": [11, 22, 44, 88] }, { "name": "B", "values": [3, 2, 5, 4] }] });
    slide_run(&mut e, &json!({ "cmd": "setChart", "data": data }));
    let c = slide_chart(&e);
    assert_eq!(c.plots[0].series[0].values[3], Some(88.0));
    assert_eq!(c.plots[0].series[0].trendlines.len(), 1);
    assert!(c.plots[0].series[1].err_bars.is_some());
    assert_eq!(c.axes.log_base, Some(2.0));
    let bytes = e.save_pptx().unwrap();
    keep("chart-elements-slide.pptx", &bytes);
    let xml = chart_xml(&bytes, "/ppt/charts/chart");
    for want in [
        "<c:logBase val=\"2\"/>",
        "<c:trendlineType val=\"exp\"/>",
        "<c:errValType val=\"cust\"/>",
        "<c:dropLines/>",
        "<c:dTable>",
        "<c:symbol val=\"square\"/><c:size val=\"7\"/>",
    ] {
        assert!(xml.contains(want), "{want}\n{xml}");
    }
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "back.pptx").unwrap();
    assert_eq!(
        slide_chart(&back).plots[0].series[0].trendlines,
        c.plots[0].series[0].trendlines
    );
    // A refused option reports an error and changes nothing.
    let cmd: SlideCommand = serde_json::from_value(
        json!({ "cmd": "setChart", "elements": { "axes": { "val": { "logBase": 1 } } } }),
    )
    .unwrap();
    assert!(back.slide_command(cmd).unwrap().error.is_some());
}

#[allow(clippy::needless_pass_by_value)]
fn doc_run(e: &mut Engine, v: Value) -> Value {
    serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap()
}

// source: the same `elements` patch through BlueDoc's `setChart`: the chart at the selection,
// one undo step, written to the chart part of the document.
#[test]
fn doc_elements_through_set_chart() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Báo cáo doanh thu".as_bytes(), "new.txt").unwrap();
    let data = json!({ "categories": ["Q1", "Q2", "Q3"],
        "series": [{ "name": "Bắc", "values": [10, 30, 50] }, { "name": "Nam", "values": [20, 40, 60] }] });
    let r = doc_run(
        &mut e,
        json!({ "cmd": "insertChart", "kind": "column", "title": "Doanh thu", "data": data }),
    );
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(r["chart"]["elements"]["series"][1]["name"], "Nam");
    let r = doc_run(
        &mut e,
        json!({ "cmd": "setChart", "elements": {
            "axes": { "val": { "min": 0, "max": 100, "majorUnit": 25 } },
            "legend": { "pos": "t", "overlay": true },
            "labels": { "value": true, "pos": "outEnd" },
            "series": [{ "index": 0, "trendlines": [{ "kind": "linear", "forward": 1, "dispR2": true }],
                "errorBars": { "kind": "both", "valueType": "stdErr", "value": 1 } }] } }),
    );
    assert!(r["error"].is_null(), "{r}");
    let el = &r["chart"]["elements"];
    assert_eq!(el["axes"]["val"]["majorUnit"], 25.0);
    assert_eq!(el["legend"], json!({ "pos": "t", "overlay": true }));
    assert_eq!(el["labels"]["value"], true);
    assert_eq!(el["series"][0]["trendlines"][0]["forward"], 1.0);
    assert_eq!(el["series"][0]["errorBars"]["valueType"], "stdErr");
    // One undo step.
    doc_run(&mut e, json!({ "cmd": "undo" }));
    let el = serde_json::to_value(e.doc_chart().unwrap()).unwrap()["elements"].clone();
    assert!(el["axes"]["val"]["majorUnit"].is_null());
    assert_eq!(el["series"][0]["trendlines"], json!([]));
    doc_run(&mut e, json!({ "cmd": "redo" }));
    let bytes = e.save_docx().unwrap();
    keep("chart-elements-doc.docx", &bytes);
    let p = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
    let xml: String = p
        .parts()
        .iter()
        .filter(|x| x.name.starts_with("/word/charts/chart"))
        .map(|x| String::from_utf8(x.data.clone()).unwrap())
        .collect();
    for want in [
        "<c:max val=\"100\"/><c:min val=\"0\"/>",
        "<c:majorUnit val=\"25\"/>",
        "<c:legendPos val=\"t\"/><c:overlay val=\"1\"/>",
        "<c:trendlineType val=\"linear\"/><c:forward val=\"1\"/><c:dispRSqr val=\"1\"/>",
        "<c:errValType val=\"stdErr\"/>",
        "<c:dLblPos val=\"outEnd\"/>",
    ] {
        assert!(xml.contains(want), "{want}\n{xml}");
    }
    // A refused option reports an error.
    let r = doc_run(
        &mut e,
        json!({ "cmd": "setChart", "elements": { "axes": { "val": { "min": 9, "max": 1 } } } }),
    );
    assert!(!r["error"].is_null(), "{r}");
}

/// One line per drawing command of a chart (coordinates rounded to 0.1 pt), then its labels.
fn chart_summary(wb: &bo_model::sheet::Workbook, chart: &Chart) -> String {
    use bo_render::{Command, Paint, PathSeg};
    use std::fmt::Write as _;
    let rgba = |c: bo_render::Rgba| format!("#{:02x}{:02x}{:02x}{:02x}", c.0, c.1, c.2, c.3);
    let mut labels = Vec::new();
    let mut out = Vec::new();
    bo_sheet::chart::draw_chart(
        wb,
        0,
        chart,
        480.0,
        288.0,
        &bo_sheet::numfmt::Locale::VI_VN,
        &mut |t, r, _, size, _, _| {
            labels.push(format!(
                "label {size:.1}pt {:.1},{:.1} {:.1}x{:.1} {t:?}",
                r[0],
                r[1],
                r[2] - r[0],
                r[3] - r[1]
            ));
        },
        &mut out,
    );
    let mut s = String::new();
    for c in &out {
        match c {
            Command::FillRect { x, y, w, h, color } => {
                let _ = writeln!(s, "rect {x:.1},{y:.1} {w:.1}x{h:.1} {}", rgba(*color));
            }
            Command::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                color,
                ..
            } => {
                let _ = writeln!(
                    s,
                    "line {x1:.1},{y1:.1} → {x2:.1},{y2:.1} {width:.2} {}",
                    rgba(*color)
                );
            }
            Command::Path { segs, fill, stroke } => {
                let mut d = String::new();
                for g in segs.iter().take(6) {
                    match g {
                        PathSeg::M(x, y) => {
                            let _ = write!(d, "M{x:.1},{y:.1} ");
                        }
                        PathSeg::L(x, y) => {
                            let _ = write!(d, "L{x:.1},{y:.1} ");
                        }
                        PathSeg::Q(_, _, x, y) => {
                            let _ = write!(d, "Q{x:.1},{y:.1} ");
                        }
                        PathSeg::C(_, _, _, _, x, y) => {
                            let _ = write!(d, "C{x:.1},{y:.1} ");
                        }
                        PathSeg::Z => d.push_str("Z "),
                    }
                }
                let f = match fill {
                    Some(Paint::Solid { color }) => rgba(*color),
                    Some(_) => "gradient".into(),
                    None => "none".into(),
                };
                let st = stroke.as_ref().map_or_else(
                    || "none".into(),
                    |k| {
                        format!(
                            "{} {:.2}{}",
                            rgba(k.color),
                            k.width,
                            if k.dash.is_empty() { "" } else { " dashed" }
                        )
                    },
                );
                let _ = writeln!(s, "path {}segs {d}fill {f} stroke {st}", segs.len());
            }
            Command::PushTransform { .. } => s.push_str("transform\n"),
            Command::PopTransform => s.push_str("end transform\n"),
            _ => s.push_str("other\n"),
        }
    }
    for l in labels {
        s.push_str(&l);
        s.push('\n');
    }
    s
}

// source: corpus writer-charts-format-elements-{1,2,3}.xlsx (written by `write_xlsx`,
// crates/bo-xlsx/tests/corpus.rs): what the shared chart renderer draws for each chart as read
// from the file.
#[test]
fn format_elements_corpus_snapshots() {
    for i in 1..=3 {
        let name = format!("writer-charts-format-elements-{i}");
        let path = format!(
            "{}/../../tests/corpus/sheet/{name}.xlsx",
            env!("CARGO_MANIFEST_DIR")
        );
        let wb = bo_xlsx::read_xlsx(&std::fs::read(path).unwrap()).unwrap();
        for (k, chart) in wb.sheets[0].charts.iter().enumerate() {
            insta::assert_snapshot!(format!("{name}-chart{}", k + 1), chart_summary(&wb, chart));
        }
    }
}
