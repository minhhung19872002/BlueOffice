//! Chart element options (CORE-CHART-005): every option group is written as the `c:` elements
//! of ECMA-376 Part 1, §21.2.2 and read back unchanged — the part written from what was read is
//! byte-identical to the first one.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use crate::chart::parse_chart_space;
use crate::chart_write::chart_space_xml;
use bo_model::chart_style::DataLabels;
use bo_model::sheet::{
    AxisOptions, Chart, ChartKind, ChartPlot, ChartSeries, ErrorBars, Marker, Trendline,
};

fn series(name: &str, values: [f64; 4]) -> ChartSeries {
    ChartSeries {
        name: Some(name.into()),
        categories: ["Q1", "Q2", "Q3", "Q4"].map(String::from).to_vec(),
        values: values.iter().map(|v| Some(*v)).collect(),
        ..ChartSeries::default()
    }
}

fn chart(kind: ChartKind) -> Chart {
    let mut a = series("A", [12.0, 150.0, 40.0, 900.0]);
    let mut b = series("B", [30.0, 20.0, 600.0, 75.0]);
    if matches!(kind, ChartKind::Scatter | ChartKind::Bubble) {
        for s in [&mut a, &mut b] {
            s.categories = ["1", "2", "3", "4"].map(String::from).to_vec();
        }
    }
    Chart {
        title: Some("Thành phần".into()),
        legend: Some("r".into()),
        plots: vec![ChartPlot {
            bar_dir: (kind == ChartKind::Bar).then(|| "col".into()),
            kind,
            series: vec![a, b],
            ..ChartPlot::default()
        }],
        ..Chart::default()
    }
}

/// Writes `c`, reads the part and writes it again: the two parts must be identical.
fn cycle(c: &Chart) -> (String, Chart) {
    let xml = chart_space_xml(c);
    let back = parse_chart_space(xml.as_bytes(), "/xl/charts/chart1.xml").unwrap();
    let again = chart_space_xml(&back);
    assert_eq!(again, xml, "the part is not stable");
    (xml, back)
}

fn has(xml: &str, parts: &[&str]) {
    for p in parts {
        assert!(xml.contains(p), "missing {p} in\n{xml}");
    }
}

// source: §21.2.2.160 scaling (logBase, orientation, max, min), majorUnit,
// minorUnit, dispUnits / builtInUnit / dispUnitsLbl,
// numFmt, majorTickMark, minorTickMark, §21.2.2.207
// tickLblPos, §21.2.2.33 crosses, crossesAt, minorGridlines.
#[test]
fn axis_options_round_trip() {
    let mut c = chart(ChartKind::Bar);
    c.axes.log_base = Some(10.0);
    c.axes.max = Some(1000.0);
    c.axes.min = Some(1.0);
    c.axes.val = AxisOptions {
        reverse: true,
        crosses: Some("max".into()),
        num_fmt: Some("#,##0".into()),
        major_tick: Some("cross".into()),
        minor_tick: Some("in".into()),
        label_pos: Some("high".into()),
        minor_grid: true,
        ..AxisOptions::default()
    };
    c.axes.cat = AxisOptions {
        reverse: true,
        crosses_at: Some(10.0),
        label_pos: Some("low".into()),
        major_tick: Some("none".into()),
        ..AxisOptions::default()
    };
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &[
            "<c:scaling><c:logBase val=\"10\"/><c:orientation val=\"maxMin\"/><c:max val=\"1000\"/><c:min val=\"1\"/></c:scaling>",
            "<c:minorGridlines/>",
            "<c:numFmt formatCode=\"#,##0\" sourceLinked=\"0\"/>",
            "<c:majorTickMark val=\"cross\"/><c:minorTickMark val=\"in\"/><c:tickLblPos val=\"high\"/>",
            "<c:crosses val=\"max\"/><c:crossBetween val=\"between\"/>",
            "<c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"low\"/>",
            "<c:crossesAt val=\"10\"/>",
        ],
    );
    assert_eq!(back.axes, c.axes);

    // Units and display units on a linear axis; an axis that is not shown.
    let mut c = chart(ChartKind::Line);
    c.axes.val = AxisOptions {
        major_unit: Some(250.0),
        minor_unit: Some(50.0),
        disp_unit: Some("thousands".into()),
        disp_unit_label: true,
        ..AxisOptions::default()
    };
    c.axes.cat.deleted = true;
    c.format.cat_grid = true;
    c.axes.cat.minor_grid = true;
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &[
            "<c:crossBetween val=\"between\"/><c:majorUnit val=\"250\"/><c:minorUnit val=\"50\"/><c:dispUnits><c:builtInUnit val=\"thousands\"/><c:dispUnitsLbl/></c:dispUnits></c:valAx>",
            "<c:delete val=\"1\"/><c:axPos val=\"b\"/><c:majorGridlines/><c:minorGridlines/>",
        ],
    );
    assert_eq!(back.axes, c.axes);
    assert!(back.format.cat_grid);
}

// source: §21.2.2.39 dateAx with baseTimeUnit; a scatter chart's X axis is a valAx
// with its own §21.2.2.160 scaling.
#[test]
fn date_and_scatter_axes_round_trip() {
    let mut c = chart(ChartKind::Line);
    c.axes.date = true;
    c.axes.cat.base_unit = Some("months".into());
    c.axes.cat.num_fmt = Some("mm/yyyy".into());
    c.x_title = Some("Tháng".into());
    c.y_title = Some("Tấn".into());
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &[
            "<c:dateAx>",
            "<c:numFmt formatCode=\"mm/yyyy\" sourceLinked=\"0\"/>",
            "<c:baseTimeUnit val=\"months\"/></c:dateAx>",
        ],
    );
    assert_eq!(back.axes, c.axes);
    assert_eq!(
        (back.x_title, back.y_title),
        (c.x_title.clone(), c.y_title.clone())
    );

    let mut c = chart(ChartKind::Scatter);
    c.axes.cat = AxisOptions {
        min: Some(0.0),
        max: Some(5.0),
        major_unit: Some(0.5),
        ..AxisOptions::default()
    };
    c.axes.min = Some(-100.0);
    c.axes.max = Some(1000.0);
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &[
            "<c:max val=\"5\"/><c:min val=\"0\"/>",
            "<c:majorUnit val=\"0.5\"/>",
            "<c:max val=\"1000\"/><c:min val=\"-100\"/>",
        ],
    );
    // The Y axis keeps its bounds apart from the X axis.
    assert_eq!(back.axes, c.axes);
}

// source: §21.2.2.226 valAx — a group referring to a second pair of axes is on the secondary
// value axis (crossing at the maximum, §21.2.2.33); the axis has a scale and a title of its own.
#[test]
fn secondary_axis_options_round_trip() {
    let mut c = chart(ChartKind::Bar);
    let b = c.plots[0].series.pop().unwrap();
    c.plots.push(ChartPlot {
        kind: ChartKind::Line,
        series: vec![b],
        secondary: true,
        ..ChartPlot::default()
    });
    c.y_title = Some("Chính".into());
    c.axes.val2 = AxisOptions {
        min: Some(0.0),
        max: Some(800.0),
        major_unit: Some(200.0),
        num_fmt: Some("0.0".into()),
        title: Some("Phụ".into()),
        ..AxisOptions::default()
    };
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &[
            "<c:axId val=\"50040\"/><c:axId val=\"50050\"/></c:lineChart>",
            "<c:max val=\"800\"/><c:min val=\"0\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"r\"/>",
            "<c:crosses val=\"max\"/><c:crossBetween val=\"between\"/><c:majorUnit val=\"200\"/></c:valAx>",
        ],
    );
    assert!(!back.plots[0].secondary && back.plots[1].secondary);
    assert_eq!(back.axes, c.axes);
    assert_eq!(back.y_title.as_deref(), Some("Chính"));
}

// source: overlay of §21.2.2.210 title and §21.2.2.93 legend; §21.2.2.54 dTable
// with showKeys; §21.2.2.53 dropLines, §21.2.2.80 hiLowLines, §21.2.2.218 upDownBars.
#[test]
fn overlays_table_and_lines_round_trip() {
    let mut c = chart(ChartKind::Line);
    c.title_overlay = true;
    c.legend = Some("l".into());
    c.legend_overlay = true;
    c.axes.data_table = true;
    c.axes.table_no_keys = true;
    c.plots[0].drop_lines = true;
    c.plots[0].hi_low_lines = true;
    c.plots[0].flag = true;
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &[
            "<c:overlay val=\"1\"/></c:title>",
            "<c:legend><c:legendPos val=\"l\"/><c:overlay val=\"1\"/></c:legend>",
            "<c:showKeys val=\"0\"/></c:dTable>",
            "<c:dropLines/><c:hiLowLines/><c:upDownBars>",
        ],
    );
    assert!(back.title_overlay && back.legend_overlay);
    assert!(back.axes.data_table && back.axes.table_no_keys);
    assert!(back.plots[0].drop_lines && back.plots[0].hi_low_lines && back.plots[0].flag);
}

// source: §21.2.2.106 marker (§21.2.2.174 symbol, size), smooth,
// §20.1.2.2.24 ln@w (EMU), overlap, §21.2.2.65 explosion.
#[test]
fn series_options_round_trip() {
    let mut c = chart(ChartKind::Line);
    c.plots[0].series[0].line_w = Some(38_100);
    c.plots[0].series[0].marker = Some(Marker {
        symbol: "diamond".into(),
        size: Some(9),
    });
    c.plots[0].series[1].smooth = true;
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &[
            "<a:ln w=\"38100\" cap=\"rnd\">",
            "<c:marker><c:symbol val=\"diamond\"/><c:size val=\"9\"/>",
            "<c:smooth val=\"1\"/>",
        ],
    );
    let (s0, s1) = (&back.plots[0].series[0], &back.plots[0].series[1]);
    assert_eq!(
        s0.marker.as_ref().map(|m| (m.symbol.as_str(), m.size)),
        Some(("diamond", Some(9)))
    );
    assert!(!s0.smooth && s1.smooth);
    // The first series' width is the chart format's; the second keeps the default of its own.
    assert_eq!(s0.line_w.or(back.format.line_w), Some(38_100));
    assert_eq!(s1.line_w.or(back.format.line_w), Some(28_575));

    let mut c = chart(ChartKind::Bar);
    c.plots[0].overlap = Some(-25);
    c.format.gap_width = Some(60);
    let (xml, back) = cycle(&c);
    has(&xml, &["<c:gapWidth val=\"60\"/><c:overlap val=\"-25\"/>"]);
    assert_eq!(back.plots[0].overlap, Some(-25));
    assert_eq!(back.format.gap_width, Some(60));
    // The default overlap of stacked bars is not an option of its own.
    c.plots[0].grouping = Some("stacked".into());
    c.plots[0].overlap = None;
    let (xml, back) = cycle(&c);
    has(&xml, &["<c:overlap val=\"100\"/>"]);
    assert_eq!(back.plots[0].overlap, None);

    let mut c = chart(ChartKind::Pie);
    c.plots[0].series.truncate(1);
    c.plots[0].series[0].explosion = 15;
    let (xml, back) = cycle(&c);
    has(&xml, &["<c:explosion val=\"15\"/>"]);
    assert_eq!(back.plots[0].series[0].explosion, 15);
}

// source: §21.2.2.49 dLbls (group and series), dLbl (idx), delete,
// §21.2.2.48 dLblPos, §21.2.2.189 showVal / §21.2.2.184 showCatName / §21.2.2.188 showSerName /
// §21.2.2.187 showPercent, separator, showLeaderLines, numFmt.
#[test]
fn data_labels_round_trip() {
    let mut c = chart(ChartKind::Bar);
    c.format.data_labels = Some(DataLabels {
        value: true,
        pos: Some("outEnd".into()),
        num_fmt: Some("#,##0.0".into()),
        ..DataLabels::default()
    });
    // The first series shows names too, joined by a dash; one point is off, one shows its category.
    c.plots[0].series[0].labels = Some(DataLabels {
        value: true,
        series: true,
        pos: Some("inEnd".into()),
        separator: Some(" – ".into()),
        ..DataLabels::default()
    });
    c.plots[0].series[0].point_labels = vec![
        (1, DataLabels::default()),
        (
            3,
            DataLabels {
                category: true,
                pos: Some("ctr".into()),
                ..DataLabels::default()
            },
        ),
    ];
    // The second series has no labels.
    c.plots[0].series[1].labels = Some(DataLabels::default());
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &[
            "<c:dLbls><c:dLbl><c:idx val=\"1\"/><c:delete val=\"1\"/></c:dLbl><c:dLbl><c:idx val=\"3\"/><c:dLblPos val=\"ctr\"/><c:showLegendKey val=\"0\"/><c:showVal val=\"0\"/><c:showCatName val=\"1\"/>",
            "<c:dLblPos val=\"inEnd\"/><c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/><c:showCatName val=\"0\"/><c:showSerName val=\"1\"/><c:showPercent val=\"0\"/><c:showBubbleSize val=\"0\"/><c:separator> – </c:separator></c:dLbls>",
            "<c:dLbls><c:delete val=\"1\"/></c:dLbls>",
            "<c:dLbls><c:numFmt formatCode=\"#,##0.0\" sourceLinked=\"0\"/><c:dLblPos val=\"outEnd\"/>",
        ],
    );
    assert_eq!(back.format.data_labels, c.format.data_labels);
    assert_eq!(back.plots[0].series[0].labels, c.plots[0].series[0].labels);
    assert_eq!(
        back.plots[0].series[0].point_labels,
        c.plots[0].series[0].point_labels
    );
    assert_eq!(back.plots[0].series[1].labels, Some(DataLabels::default()));

    // A pie: percentages outside without leader lines; only the labels of a single point.
    let mut c = chart(ChartKind::Pie);
    c.plots[0].series.truncate(1);
    c.plots[0].series[0].labels = Some(DataLabels {
        percent: true,
        category: true,
        pos: Some("outEnd".into()),
        leader_lines: Some(false),
        num_fmt: Some("0.0%".into()),
        ..DataLabels::default()
    });
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &[
            "<c:numFmt formatCode=\"0.0%\" sourceLinked=\"0\"/><c:dLblPos val=\"outEnd\"/>",
            "<c:showPercent val=\"1\"/><c:showBubbleSize val=\"0\"/><c:showLeaderLines val=\"0\"/></c:dLbls>",
        ],
    );
    assert_eq!(back.plots[0].series[0].labels, c.plots[0].series[0].labels);
    let mut c = chart(ChartKind::Line);
    c.plots[0].series[1].point_labels = vec![(
        2,
        DataLabels {
            value: true,
            pos: Some("t".into()),
            ..DataLabels::default()
        },
    )];
    let (xml, back) = cycle(&c);
    has(&xml, &["<c:dLbl><c:idx val=\"2\"/><c:dLblPos val=\"t\"/>"]);
    assert_eq!(
        back.plots[0].series[1].point_labels,
        c.plots[0].series[1].point_labels
    );
    // The series itself shows nothing.
    assert!(!back.plots[0].series[1]
        .labels
        .as_ref()
        .is_some_and(DataLabels::any));
}

// source: §21.2.2.211 trendline — name, trendlineType, §21.2.2.129
// order, §21.2.2.136 period, forward, backward, intercept,
// §21.2.2.50 dispRSqr, §21.2.2.51 dispEq (in this order, CT_Trendline).
#[test]
fn trendlines_round_trip() {
    let mut c = chart(ChartKind::Scatter);
    let t = |kind: &str| Trendline {
        kind: kind.into(),
        ..Trendline::default()
    };
    c.plots[0].series[0].trendlines = vec![
        Trendline {
            name: Some("Xu hướng A".into()),
            forward: Some(2.0),
            backward: Some(0.5),
            intercept: Some(0.0),
            disp_eq: true,
            disp_r2: true,
            ..t("linear")
        },
        Trendline {
            order: 3,
            intercept: Some(4.5),
            disp_eq: true,
            ..t("poly")
        },
        Trendline {
            order: 3,
            ..t("movingAvg")
        },
    ];
    c.plots[0].series[1].trendlines = vec![
        Trendline {
            forward: Some(1.0),
            ..t("exp")
        },
        t("log"),
        Trendline {
            disp_r2: true,
            ..t("power")
        },
    ];
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &[
            "<c:trendline><c:name>Xu hướng A</c:name><c:trendlineType val=\"linear\"/><c:forward val=\"2\"/><c:backward val=\"0.5\"/><c:intercept val=\"0\"/><c:dispRSqr val=\"1\"/><c:dispEq val=\"1\"/></c:trendline>",
            "<c:trendlineType val=\"poly\"/><c:order val=\"3\"/><c:intercept val=\"4.5\"/><c:dispRSqr val=\"0\"/><c:dispEq val=\"1\"/>",
            "<c:trendlineType val=\"movingAvg\"/><c:period val=\"3\"/><c:dispRSqr val=\"0\"/>",
            "<c:trendlineType val=\"exp\"/><c:forward val=\"1\"/>",
            "<c:trendlineType val=\"log\"/>",
            "<c:trendlineType val=\"power\"/><c:dispRSqr val=\"1\"/>",
        ],
    );
    assert_eq!(
        back.plots[0].series[0].trendlines,
        c.plots[0].series[0].trendlines
    );
    assert_eq!(
        back.plots[0].series[1].trendlines,
        c.plots[0].series[1].trendlines
    );
}

// source: §21.2.2.58 errBars — errDir, errBarType, §21.2.2.59 errValType,
// noEndCap, plus / minus (numRef or numLit), §21.2.2.229 val.
#[test]
fn error_bars_round_trip() {
    let bars = |kind: &str, value_type: &str, value: f64| ErrorBars {
        kind: kind.into(),
        value_type: value_type.into(),
        value,
        ..ErrorBars::default()
    };
    for (kind, vt, v) in [
        ("both", "fixedVal", 2.5),
        ("plus", "percentage", 10.0),
        ("minus", "stdDev", 1.5),
        ("both", "stdErr", 1.0),
    ] {
        let mut c = chart(ChartKind::Bar);
        c.plots[0].series[0].err_bars = Some(ErrorBars {
            no_end_cap: vt == "percentage",
            ..bars(kind, vt, v)
        });
        let (xml, back) = cycle(&c);
        has(
            &xml,
            &[&format!(
                "<c:errBars><c:errBarType val=\"{kind}\"/><c:errValType val=\"{vt}\"/><c:noEndCap val=\"{}\"/><c:val val=\"{v}\"/></c:errBars>",
                u8::from(vt == "percentage")
            )],
        );
        assert_eq!(
            back.plots[0].series[0].err_bars,
            c.plots[0].series[0].err_bars
        );
    }
    // Custom amounts: a range above (with its cache), literal numbers below.
    let mut c = chart(ChartKind::Line);
    c.plots[0].series[1].err_bars = Some(ErrorBars {
        plus_ref: Some("Sheet1!$D$2:$D$5".into()),
        plus: vec![Some(1.0), Some(2.0), None, Some(4.0)],
        minus: vec![Some(0.5)],
        value: 1.0,
        ..bars("both", "cust", 1.0)
    });
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &[
            "<c:errValType val=\"cust\"/><c:noEndCap val=\"0\"/><c:plus><c:numRef><c:f>Sheet1!$D$2:$D$5</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val=\"4\"/><c:pt idx=\"0\"><c:v>1</c:v></c:pt>",
            "</c:plus><c:minus><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>0.5</c:v></c:pt></c:numLit></c:minus></c:errBars>",
        ],
    );
    assert_eq!(
        back.plots[0].series[1].err_bars,
        c.plots[0].series[1].err_bars
    );
    // Along the X axis of a scatter series.
    let mut c = chart(ChartKind::Scatter);
    c.plots[0].series[0].err_bars = Some(ErrorBars {
        dir: Some("x".into()),
        ..bars("both", "fixedVal", 0.25)
    });
    let (xml, back) = cycle(&c);
    has(
        &xml,
        &["<c:errBars><c:errDir val=\"x\"/><c:errBarType val=\"both\"/>"],
    );
    assert_eq!(
        back.plots[0].series[0].err_bars,
        c.plots[0].series[0].err_bars
    );
}

// source: §21.2.2.50 doughnutChart holeSize (ST_HoleSize, 1..=90 percent).
#[test]
fn doughnut_hole_size_round_trips() {
    let mut c = chart(ChartKind::Doughnut);
    c.plots[0].hole_size = Some(75);
    let (xml, back) = cycle(&c);
    has(&xml, &["<c:holeSize val=\"75\"/>"]);
    assert_eq!(back.plots[0].hole_size, Some(75));
    // Out-of-range values are brought into the type's range.
    let odd = xml.replace("<c:holeSize val=\"75\"/>", "<c:holeSize val=\"95\"/>");
    let back = parse_chart_space(odd.as_bytes(), "/xl/charts/chart1.xml").unwrap();
    assert_eq!(back.plots[0].hole_size, Some(90));
}
