//! Charts drawn point by point (a slide show building a chart by series, category or element):
//! `draw_chart_masked` leaves out the points the mask hides and draws everything else exactly
//! as the full chart does.
#![allow(clippy::unwrap_used)]

use bo_model::sheet::{Chart, ChartSeries, Workbook};
use bo_render::Command;
use bo_sheet::chart::{default_color, draw_chart_masked, maskable, plot_for, PointMask};
use bo_sheet::numfmt::Locale;

fn chart(kind: &str) -> Chart {
    let mut plot = plot_for(kind);
    plot.series = ["A", "B"]
        .iter()
        .enumerate()
        .map(|(k, n)| ChartSeries {
            name: Some((*n).to_owned()),
            categories: vec!["Q1".into(), "Q2".into(), "Q3".into()],
            values: vec![
                Some(3.0 + k as f64),
                Some(5.0 + k as f64),
                Some(4.0 + k as f64),
            ],
            ..ChartSeries::default()
        })
        .collect();
    Chart {
        plots: vec![plot],
        legend: Some("b".into()),
        title: Some("T".into()),
        ..Chart::default()
    }
}

fn draw(c: &Chart, mask: Option<&PointMask<'_>>) -> Vec<Command> {
    let mut out = Vec::new();
    draw_chart_masked(
        &Workbook::default(),
        0,
        c,
        360.0,
        240.0,
        &Locale::default(),
        &default_color,
        false,
        mask,
        &mut |_, _, _, _, _, _| {},
        &mut out,
    );
    out
}

/// `small` is `big` with some commands left out (in order).
fn subsequence(small: &[Command], big: &[Command]) -> bool {
    let mut j = 0;
    for c in big {
        if j < small.len() && small[j] == *c {
            j += 1;
        }
    }
    j == small.len()
}

/// Commands of `big` that `small` does not have.
fn extra(small: &[Command], big: &[Command]) -> Vec<Command> {
    let mut j = 0;
    let mut out = Vec::new();
    for c in big {
        if j < small.len() && small[j] == *c {
            j += 1;
        } else {
            out.push(c.clone());
        }
    }
    out
}

// source: a column chart of 2 series × 3 categories draws one bar per point; the background
// (no point shown) is part of every masked drawing, and each point adds its own bar.
#[test]
fn columns_build_point_by_point() {
    let c = chart("column");
    assert!(maskable(&c));
    let full = draw(&c, None);
    let none = draw(&c, Some(&|_, _| false));
    assert!(subsequence(&none, &full));
    // Every mask shows the background plus its bars.
    let series0 = draw(&c, Some(&|s, _| s == 0));
    assert!(subsequence(&none, &series0));
    assert!(subsequence(&series0, &full));
    let bars = |cmds: &[Command]| {
        cmds.iter()
            .filter(|c| matches!(c, Command::FillRect { .. } | Command::Path { .. }))
            .count()
    };
    let one_series = extra(&none, &series0);
    assert_eq!(bars(&one_series), 3, "{one_series:?}");
    let one_point = extra(&none, &draw(&c, Some(&|s, i| s == 1 && i == 2)));
    assert_eq!(bars(&one_point), 1);
    // All points: the full drawing.
    assert_eq!(draw(&c, Some(&|_, _| true)), full);
}

// source: a pie's slices keep their angles when others are left out; a line builds segment by
// segment (each shown point brings the piece from the point before it, and its marker).
#[test]
fn pies_and_lines_build_by_element() {
    let p = chart("pie");
    let full = draw(&p, None);
    let none = draw(&p, Some(&|_, _| false));
    assert!(subsequence(&none, &full));
    let second = extra(&none, &draw(&p, Some(&|_, i| i == 1)));
    assert_eq!(second.len(), 1);
    assert!(
        full.contains(&second[0]),
        "the slice is drawn as in the full pie"
    );

    let l = chart("line");
    let none = draw(&l, Some(&|_, _| false));
    let first_point = extra(&none, &draw(&l, Some(&|s, i| s == 0 && i == 0)));
    // The first point has no segment before it: its marker only (when markers are drawn).
    assert!(first_point.iter().all(|c| !matches!(
        c,
        Command::Path {
            stroke: Some(_),
            fill: None,
            ..
        }
    )));
    let second_point = extra(&none, &draw(&l, Some(&|s, i| s == 0 && i == 1)));
    assert_ne!(second_point.len(), 0);
    assert!(!maskable(&chart("radar")));
}
