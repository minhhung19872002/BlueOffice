//! Drawing of the chart element options (CORE-CHART-005): each option group changes what the
//! shared chart renderer draws.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use super::*;
use bo_model::chart_style::DataLabels;
use bo_model::sheet::{AxisOptions, Cell, CellRange, ErrorBars, Marker, Trendline};

/// Provinces × two years: HN 10 / 12, HUE 5 / 4, DN 7 / 9.
fn wb() -> Workbook {
    let mut wb = Workbook::new();
    let rows: [(&str, f64, f64); 3] = [("HN", 10.0, 12.0), ("HUE", 5.0, 4.0), ("DN", 7.0, 9.0)];
    let ws = &mut wb.sheets[0];
    for (c, h) in ["Tỉnh", "2024", "2025"].iter().enumerate() {
        ws.set_cell(0, c as u32, Cell::new(CellValue::Text((*h).into())));
    }
    for (r, (name, a, b)) in rows.iter().enumerate() {
        let r = r as u32 + 1;
        ws.set_cell(r, 0, Cell::new(CellValue::Text((*name).into())));
        ws.set_cell(r, 1, Cell::new(CellValue::Number(*a)));
        ws.set_cell(r, 2, Cell::new(CellValue::Number(*b)));
    }
    wb
}

/// X 1..4 with y = 3x + 1 and y = 2x².
fn xy_wb() -> Workbook {
    let mut wb = Workbook::new();
    let ws = &mut wb.sheets[0];
    for (c, h) in ["x", "a", "b"].iter().enumerate() {
        ws.set_cell(0, c as u32, Cell::new(CellValue::Text((*h).into())));
    }
    for r in 1..=4u32 {
        let x = f64::from(r);
        ws.set_cell(r, 0, Cell::new(CellValue::Number(x)));
        ws.set_cell(r, 1, Cell::new(CellValue::Number(3.0 * x + 1.0)));
        ws.set_cell(r, 2, Cell::new(CellValue::Number(2.0 * x * x)));
    }
    wb
}

fn chart(wb: &Workbook, kind: &str) -> Chart {
    new_chart(wb, 0, CellRange::parse("A1:C4").unwrap(), kind).unwrap()
}

/// The scatter chart of [`xy_wb`] (four points per series).
fn xy_chart(wb: &Workbook) -> Chart {
    let mut c = new_chart(wb, 0, CellRange::parse("A1:C5").unwrap(), "scatter").unwrap();
    c.legend = Some("b".into());
    c
}

type Boxes = Vec<(String, [f64; 4])>;

fn draw(wb: &Workbook, c: &Chart) -> (Vec<Command>, Boxes) {
    let mut labels = Vec::new();
    let mut out = Vec::new();
    draw_chart(
        wb,
        0,
        c,
        400.0,
        250.0,
        &Locale::VI_VN,
        &mut |t, r, _, _, _, _| labels.push((t.to_owned(), r)),
        &mut out,
    );
    (out, labels)
}

fn texts(b: &Boxes) -> Vec<&str> {
    b.iter().map(|(t, _)| t.as_str()).collect()
}

fn box_of(b: &Boxes, text: &str) -> [f64; 4] {
    b.iter()
        .find(|(t, _)| t == text)
        .unwrap_or_else(|| panic!("no label {text} in {:?}", texts(b)))
        .1
}

/// The bars of series `i` (the legend key is smaller than a bar).
fn bars(out: &[Command], i: usize) -> Vec<(f32, f32, f32, f32)> {
    let c = PALETTE[i];
    out.iter()
        .filter_map(|cmd| match cmd {
            Command::FillRect { x, y, w, h, color } if *color == c && *w > 9.0 => {
                Some((*x, *y, *w, *h))
            }
            _ => None,
        })
        .collect()
}

// source: §21.2.2 scaling (min, max, logBase), majorUnit, dispUnits and numFmt of c:valAx — the
// labels are arithmetic on the options: 0..20 by 5; powers of ten up to the fixed maximum;
// values divided by 100 in the format 0.00 (vi-VN decimal comma).
#[test]
fn value_axis_scale_units_and_formats() {
    let wb = wb();
    let mut c = chart(&wb, "column");
    c.axes.min = Some(0.0);
    c.axes.max = Some(20.0);
    c.axes.val.major_unit = Some(5.0);
    let (_, l) = draw(&wb, &c);
    let t = texts(&l);
    for want in ["0", "5", "10", "15", "20"] {
        assert!(t.contains(&want), "{want} in {t:?}");
    }
    assert!(!t.contains(&"25"));

    let mut c = chart(&wb, "column");
    c.axes.log_base = Some(10.0);
    c.axes.max = Some(500.0);
    let (_, l) = draw(&wb, &c);
    let t = texts(&l);
    assert!(
        t.contains(&"1") && t.contains(&"10") && t.contains(&"100"),
        "{t:?}"
    );
    assert!(!t.contains(&"1000"), "{t:?}");

    let mut c = chart(&wb, "column");
    c.axes.val = AxisOptions {
        disp_unit: Some("hundreds".into()),
        disp_unit_label: true,
        num_fmt: Some("0.00".into()),
        ..AxisOptions::default()
    };
    let (_, l) = draw(&wb, &c);
    let t = texts(&l);
    assert!(
        t.contains(&"0,05") && t.contains(&"0,10") && t.contains(&"×10^2"),
        "{t:?}"
    );

    // An axis that is not shown, or whose labels are off, has no tick labels.
    for o in [
        AxisOptions {
            deleted: true,
            ..AxisOptions::default()
        },
        AxisOptions {
            label_pos: Some("none".into()),
            ..AxisOptions::default()
        },
    ] {
        let mut c = chart(&wb, "column");
        c.axes.val = o;
        let (_, l) = draw(&wb, &c);
        assert!(!texts(&l).contains(&"15"), "{:?}", texts(&l));
    }
}

// source: c:scaling/c:orientation maxMin — the axis runs the other way and the axis crossing it
// moves to the opposite side; c:minorGridlines / c:minorTickMark add lines between the major ones.
#[test]
fn reversed_axes_minor_gridlines_and_ticks() {
    let wb = wb();
    let c = chart(&wb, "column");
    let (out, l) = draw(&wb, &c);
    assert!(box_of(&l, "HN")[0] < box_of(&l, "DN")[0]);
    assert!(box_of(&l, "0")[1] > box_of(&l, "15")[1]);
    // Columns stand on a common base line.
    let base: Vec<f32> = bars(&out, 0).iter().map(|(_, y, _, h)| y + h).collect();
    assert!(base.iter().all(|b| (b - base[0]).abs() < 0.01), "{base:?}");
    let lines = |out: &[Command]| {
        out.iter()
            .filter(|c| matches!(c, Command::Line { .. }))
            .count()
    };
    let plain = lines(&out);

    let mut r = c.clone();
    r.axes.cat.reverse = true;
    let (_, l) = draw(&wb, &r);
    assert!(box_of(&l, "HN")[0] > box_of(&l, "DN")[0]);
    // The value axis follows the first category to the right side.
    assert!(box_of(&l, "0")[0] > box_of(&l, "HUE")[0]);

    let mut r = c.clone();
    r.axes.val.reverse = true;
    let (out, l) = draw(&wb, &r);
    assert!(box_of(&l, "0")[1] < box_of(&l, "15")[1]);
    // Columns hang from the top; the category labels are above them.
    let top: Vec<f32> = bars(&out, 0).iter().map(|(_, y, _, _)| *y).collect();
    assert!(top.iter().all(|t| (t - top[0]).abs() < 0.01), "{top:?}");
    assert!(box_of(&l, "HN")[3] <= f64::from(top[0]) + 0.01);

    // Scale 0..15 by 5: four minor lines per step (12), a minor tick on each, and four major ticks.
    let mut g = c.clone();
    g.axes.val.minor_grid = true;
    let (out, _) = draw(&wb, &g);
    assert_eq!(lines(&out), plain + 12);
    g.axes.val.minor_tick = Some("out".into());
    g.axes.val.major_tick = Some("cross".into());
    let (out, _) = draw(&wb, &g);
    assert_eq!(lines(&out), plain + 12 + 12 + 4);
    // Category axis: a minor gridline through each of the 3 categories, a tick at each of 4 edges.
    let mut g = c.clone();
    g.axes.cat.minor_grid = true;
    g.axes.cat.major_tick = Some("in".into());
    let (out, _) = draw(&wb, &g);
    assert_eq!(lines(&out), plain + 3 + 4);
}

// source: c:crossesAt of the category axis — columns start at that value: above it they stand
// on the axis, below it they hang from it.
#[test]
fn category_axis_crossing_moves_the_base_line() {
    let wb = wb();
    let mut c = chart(&wb, "column");
    c.axes.cat.crosses_at = Some(6.0);
    let (out, _) = draw(&wb, &c);
    let axis_y = out
        .iter()
        .find_map(|cmd| match cmd {
            Command::Line { y1, y2, color, .. } if *color == AXIS && y1 == y2 => Some(*y1),
            _ => None,
        })
        .unwrap();
    // 2024: HN 10 and DN 7 above 6, HUE 5 below.
    let b = bars(&out, 0);
    assert_eq!(b.len(), 3);
    assert!((b[0].1 + b[0].3 - axis_y).abs() < 0.01 && (b[2].1 + b[2].3 - axis_y).abs() < 0.01);
    assert!((b[1].1 - axis_y).abs() < 0.01, "{b:?} {axis_y}");
}

// source: c:dLbls of the chart, of a series and c:dLbl of a point (the nearest wins); c:separator
// and c:numFmt; a pie's percentages are value / total: 10, 5, 7 of 22 = 45.5 %, 22.7 %, 31.8 %.
#[test]
fn data_labels_of_series_and_points() {
    let wb = wb();
    let mut c = chart(&wb, "column");
    c.plots[0].series[1].labels = Some(DataLabels {
        value: true,
        ..DataLabels::default()
    });
    c.plots[0].series[0].point_labels = vec![(
        1,
        DataLabels {
            value: true,
            category: true,
            separator: Some(" | ".into()),
            num_fmt: Some("0.0".into()),
            ..DataLabels::default()
        },
    )];
    let (_, l) = draw(&wb, &c);
    let t = texts(&l);
    assert!(
        t.contains(&"12") && t.contains(&"4") && t.contains(&"9"),
        "{t:?}"
    );
    assert!(t.contains(&"HUE | 5,0"), "{t:?}");
    // The other points of the first series (10, 7) have no label; 7 is not a tick label either.
    assert!(!t.contains(&"7"), "{t:?}");

    // Labels of the whole chart, turned off for one series.
    let mut c = chart(&wb, "column");
    c.format.data_labels = Some(DataLabels {
        value: true,
        pos: Some("inEnd".into()),
        ..DataLabels::default()
    });
    c.plots[0].series[0].labels = Some(DataLabels::default());
    let (out, l) = draw(&wb, &c);
    let t = texts(&l);
    assert!(t.contains(&"9") && !t.contains(&"7"), "{t:?}");
    // Inside the end: the label box lies within its column (HN 2025 = 12).
    let b = bars(&out, 1)[0];
    let lb = box_of(&l, "12");
    assert!(
        lb[1] >= f64::from(b.1) && lb[3] <= f64::from(b.1 + b.3),
        "{lb:?} {b:?}"
    );

    let mut c = new_chart(&wb, 0, CellRange::parse("A1:B4").unwrap(), "pie").unwrap();
    c.plots[0].series[0].labels = Some(DataLabels {
        percent: true,
        pos: Some("outEnd".into()),
        num_fmt: Some("0.0%".into()),
        ..DataLabels::default()
    });
    let leaders = |out: &[Command]| {
        out.iter()
            .filter(|c| matches!(c, Command::Line { color, .. } if *color == AXIS))
            .count()
    };
    let (out, l) = draw(&wb, &c);
    let t = texts(&l);
    assert!(
        t.contains(&"45,5%") && t.contains(&"22,7%") && t.contains(&"31,8%"),
        "{t:?}"
    );
    assert_eq!(leaders(&out), 3);
    c.plots[0].series[0].labels.as_mut().unwrap().leader_lines = Some(false);
    assert_eq!(leaders(&draw(&wb, &c).0), 0);
}

// source: c:trendline with c:intercept (the fit goes through it at x = 0), c:forward (the line
// runs past the last point) and c:name (its legend entry). y = 3x + 1 through 1 has slope 3;
// y = 2·e^(0.5x) through 2 has the exponent 0.5.
#[test]
fn trend_lines_with_intercept_forecast_and_name() {
    let t = |kind: &str, intercept: f64| Trendline {
        kind: kind.into(),
        intercept: Some(intercept),
        ..Trendline::default()
    };
    let line: Vec<(f64, f64)> = (1..=4)
        .map(|x| (f64::from(x), 3.0 * f64::from(x) + 1.0))
        .collect();
    let fit = Fit::new(&t("linear", 1.0), &line).unwrap();
    assert!((fit.c[0] - 1.0).abs() < 1e-12 && (fit.c[1] - 3.0).abs() < 1e-9);
    let exp: Vec<(f64, f64)> = (1..=4)
        .map(|x| (f64::from(x), 2.0 * (0.5 * f64::from(x)).exp()))
        .collect();
    let fit = Fit::new(&t("exp", 2.0), &exp).unwrap();
    assert!(
        (fit.c[0] - 2.0).abs() < 1e-12 && (fit.c[1] - 0.5).abs() < 1e-9,
        "{:?}",
        fit.c
    );
    // Forced through another value the line is no longer the data's own.
    let off = Fit::new(&t("linear", 0.0), &line).unwrap();
    assert!((off.c[1] - 3.0).abs() > 0.1 && off.c[0] == 0.0);

    let wb = xy_wb();
    let mut c = xy_chart(&wb);
    c.plots[0].series[0].trendlines = vec![Trendline {
        kind: "linear".into(),
        name: Some("Xu hướng".into()),
        ..Trendline::default()
    }];
    let end = |out: &[Command]| {
        out.iter()
            .find_map(|cmd| match cmd {
                Command::Path {
                    segs,
                    stroke: Some(s),
                    ..
                } if !s.dash.is_empty() => match (segs.first(), segs.last()) {
                    (Some(PathSeg::M(a, _)), Some(PathSeg::L(z, _))) => Some((*a, *z)),
                    _ => None,
                },
                _ => None,
            })
            .unwrap()
    };
    // The marker of the last point (x = 4) of the first series.
    let last = |out: &[Command]| {
        out.iter()
            .filter_map(|cmd| match cmd {
                Command::FillRect { x, w, color, .. } if *color == PALETTE[0] && *w == 5.0 => {
                    Some(x + 2.5)
                }
                _ => None,
            })
            .fold(0.0_f32, f32::max)
    };
    let (out, l) = draw(&wb, &c);
    assert!(texts(&l).contains(&"Xu hướng"));
    assert!(!texts(&l).contains(&"6"));
    let (_, z0) = end(&out);
    assert!((z0 - last(&out)).abs() < 0.5);
    // X runs 1..4; forward 2 extends the axis to 6 and the line to x = 6: two of the five units
    // of the axis past the last point.
    c.plots[0].series[0].trendlines[0].forward = Some(2.0);
    let (out, l) = draw(&wb, &c);
    assert!(texts(&l).contains(&"6"), "{:?}", texts(&l));
    let (a1, z1) = end(&out);
    let past = f64::from(z1 - last(&out)) / f64::from(z1 - a1);
    assert!((past - 2.0 / 5.0).abs() < 0.01, "{past}");
}

// source: c:errBars — custom amounts (c:plus / c:minus, one for all points or one each),
// c:noEndCap, c:errDir x on an XY series.
#[test]
fn error_bars_custom_amounts_caps_and_direction() {
    let e = ErrorBars {
        kind: "both".into(),
        value_type: "cust".into(),
        plus: vec![Some(1.0), Some(2.0), Some(3.0)],
        minus: vec![Some(0.5)],
        ..ErrorBars::default()
    };
    assert_eq!(err_amounts(&e, 1, 10.0, &[]), (2.0, 0.5));
    assert_eq!(err_amounts(&e, 5, 10.0, &[]), (0.0, 0.5));
    let fixed = ErrorBars {
        kind: "both".into(),
        value_type: "fixedVal".into(),
        value: 1.0,
        ..ErrorBars::default()
    };
    assert_eq!(err_amounts(&fixed, 0, 10.0, &[10.0]), (1.0, 1.0));

    let wb = wb();
    let ink = |out: &[Command]| -> (usize, usize) {
        let mut n = (0, 0);
        for c in out {
            if let Command::Line {
                x1,
                x2,
                y1,
                y2,
                color,
                ..
            } = c
            {
                if *color == TEXT && x1 == x2 {
                    n.0 += 1;
                } else if *color == TEXT && y1 == y2 {
                    n.1 += 1;
                }
            }
        }
        n
    };
    let mut c = chart(&wb, "line");
    c.plots[0].series[0].err_bars = Some(fixed.clone());
    // Three bars, each with two caps.
    assert_eq!(ink(&draw(&wb, &c).0), (3, 6));
    c.plots[0].series[0].err_bars.as_mut().unwrap().no_end_cap = true;
    assert_eq!(ink(&draw(&wb, &c).0), (3, 0));
    // Along X: four horizontal bars with vertical caps.
    let wb = xy_wb();
    let mut c = xy_chart(&wb);
    c.plots[0].series[0].err_bars = Some(ErrorBars {
        dir: Some("x".into()),
        value: 0.25,
        ..fixed
    });
    assert_eq!(ink(&draw(&wb, &c).0), (8, 4));
}

// source: c:smooth (a curve), c:marker symbol / size, a:ln@w of one series; c:overlap of a bar
// group: 100 puts the columns of a category on one another, −100 a column's width apart.
#[test]
fn series_lines_markers_and_bar_overlap() {
    let wb = wb();
    let mut c = chart(&wb, "line");
    c.plots[0].series[0].smooth = true;
    c.plots[0].series[0].line_w = Some(38_100);
    c.plots[0].series[1].marker = Some(Marker {
        symbol: "square".into(),
        size: Some(8),
    });
    let (out, _) = draw(&wb, &c);
    let curve = out.iter().any(|cmd| {
        matches!(cmd, Command::Path { segs, stroke: Some(s), fill: None }
            if s.color == PALETTE[0] && (s.width - 3.0).abs() < 1e-6
                && segs.iter().any(|g| matches!(g, PathSeg::C(..))))
    });
    assert!(curve);
    let squares = out
        .iter()
        .filter(|cmd| {
            matches!(cmd, Command::Path { segs, fill: Some(Paint::Solid { color }), .. }
                if *color == PALETTE[1] && segs.len() == 5)
        })
        .count();
    assert_eq!(squares, 3);

    let mut c = chart(&wb, "column");
    let (out, _) = draw(&wb, &c);
    let (a, b) = (bars(&out, 0), bars(&out, 1));
    assert!((b[0].0 - (a[0].0 + a[0].2)).abs() < 0.01);
    c.plots[0].overlap = Some(100);
    let (out, _) = draw(&wb, &c);
    let (a, b) = (bars(&out, 0), bars(&out, 1));
    assert!(
        (b[0].0 - a[0].0).abs() < 0.01
            && b[0].2 > 1.9 * (bars(&draw(&wb, &chart(&wb, "column")).0, 0)[0].2)
    );
    c.plots[0].overlap = Some(-100);
    let (out, _) = draw(&wb, &c);
    let (a, b) = (bars(&out, 0), bars(&out, 1));
    assert!((b[0].0 - (a[0].0 + 2.0 * a[0].2)).abs() < 0.01);
}

// source: c:overlay of the legend (it takes no room from the plot), c:legendPos l, c:overlay of
// the title, c:dTable/c:showKeys.
#[test]
fn overlays_legend_side_and_table_keys() {
    let wb = wb();
    let mut c = chart(&wb, "column");
    c.legend = Some("r".into());
    c.title = Some("Tiêu đề".into());
    let right = |out: &[Command]| {
        bars(out, 1)
            .iter()
            .map(|b| b.0 + b.2)
            .fold(0.0_f32, f32::max)
    };
    let top = |out: &[Command]| bars(out, 1).iter().map(|b| b.1).fold(f32::MAX, f32::min);
    let (plain, l) = draw(&wb, &c);
    assert!(box_of(&l, "2024")[0] > 250.0);
    c.legend_overlay = true;
    let (over, _) = draw(&wb, &c);
    assert!(right(&over) > right(&plain) + 40.0);
    // The overlaid legend is painted last (over the columns).
    assert!(matches!(over.last(), Some(Command::FillRect { color, .. }) if *color == PALETTE[1]));
    c.legend_overlay = false;
    c.title_overlay = true;
    assert!(top(&draw(&wb, &c).0) < top(&plain) - 10.0);
    c.legend = Some("l".into());
    let (_, l) = draw(&wb, &c);
    assert!(box_of(&l, "2024")[0] < 60.0);

    let mut c = chart(&wb, "column");
    c.legend = None;
    c.axes.data_table = true;
    let keys = |out: &[Command]| {
        out.iter()
            .filter(|c| matches!(c, Command::FillRect { w, color, .. } if *w <= 12.0 && (*color == PALETTE[0] || *color == PALETTE[1])))
            .count()
    };
    assert_eq!(keys(&draw(&wb, &c).0), 2);
    c.axes.table_no_keys = true;
    assert_eq!(keys(&draw(&wb, &c).0), 0);
}
