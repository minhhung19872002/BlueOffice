//! Per-preset geometry checks. Every expected value is derived by hand from the
//! guide formulas of the preset definition for a 2000 × 1000 shape (ss = 1000)
//! with default adjust values unless noted.

use bo_drawingml::geom::{eval_preset, Evaluated, FillMode, Seg};
use bo_model::slide::Guide;

const W: f64 = 2000.0;
const H: f64 = 1000.0;
const TOL: f64 = 0.05;

fn ev(name: &str) -> Evaluated {
    ev_av(name, &[])
}

fn ev_av(name: &str, av: &[(&str, &str)]) -> Evaluated {
    let av: Vec<Guide> = av
        .iter()
        .map(|(n, f)| Guide {
            name: (*n).into(),
            fmla: (*f).into(),
        })
        .collect();
    eval_preset(name, &av, W, H)
        .unwrap_or_else(|| panic!("preset {name} missing"))
        .unwrap_or_else(|e| panic!("preset {name}: {e}"))
}

/// Path end points (Move/Line/curve end), all paths.
fn points(e: &Evaluated) -> Vec<(f64, f64)> {
    e.paths
        .iter()
        .flat_map(|p| p.segs.iter())
        .filter_map(|s| match *s {
            Seg::Move(x, y)
            | Seg::Line(x, y)
            | Seg::Quad(_, _, x, y)
            | Seg::Cubic(_, _, _, _, x, y) => Some((x, y)),
            Seg::Close => None,
        })
        .collect()
}

#[track_caller]
fn has(e: &Evaluated, x: f64, y: f64) {
    let pts = points(e);
    assert!(
        pts.iter()
            .any(|(px, py)| (px - x).abs() < TOL && (py - y).abs() < TOL),
        "point ({x}, {y}) not found in {pts:?}"
    );
}

#[track_caller]
fn bbox(e: &Evaluated, b: [f64; 4]) {
    let got = e.bounds().expect("empty geometry");
    for i in 0..4 {
        assert!((got[i] - b[i]).abs() < 0.5, "bounds {got:?} != {b:?}");
    }
}

#[track_caller]
fn full(e: &Evaluated) {
    bbox(e, [0.0, 0.0, W, H]);
}

#[track_caller]
fn text_rect(e: &Evaluated, r: [f64; 4]) {
    for i in 0..4 {
        assert!(
            (e.text_rect[i] - r[i]).abs() < TOL,
            "text rect {:?} != {r:?}",
            e.text_rect
        );
    }
}

#[test]
fn preset_count() {
    assert!(bo_drawingml::preset_names().len() >= 70);
}

#[test]
fn rect() {
    // source: ECMA-376 Part 1 presetShapeDefinitions rect
    let e = ev("rect");
    full(&e);
    has(&e, 0.0, 0.0);
    has(&e, W, H);
}

#[test]
fn round_rect() {
    // source: ECMA-376 Part 1 presetShapeDefinitions roundRect
    // x1 = ss·16667/100000 = 166.67; il = x1·29289/100000 = 48.82
    let e = ev("roundRect");
    full(&e);
    has(&e, 0.0, 166.67);
    has(&e, 166.67, 0.0);
    has(&e, 1833.33, 0.0);
    has(&e, 2000.0, 833.33);
    text_rect(&e, [48.815, 48.815, 1951.185, 951.185]);
    // adj pinned to 50000 → half circle ends.
    let e = ev_av("roundRect", &[("adj", "val 90000")]);
    has(&e, 500.0, 0.0);
}

#[test]
fn ellipse() {
    // source: ECMA-376 Part 1 presetShapeDefinitions ellipse
    let e = ev("ellipse");
    full(&e);
    for (x, y) in [
        (0.0, 500.0),
        (1000.0, 0.0),
        (2000.0, 500.0),
        (1000.0, 1000.0),
    ] {
        has(&e, x, y);
    }
    // idx = wd2·cos 45° = 707.11 → il = 292.89; idy = 353.55 → it = 146.45
    text_rect(&e, [292.893, 146.447, 1707.107, 853.553]);
}

#[test]
fn flow_chart_connector() {
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartConnector
    let e = ev("flowChartConnector");
    full(&e);
    has(&e, 1000.0, 0.0);
}

#[test]
fn triangle() {
    // source: ECMA-376 Part 1 presetShapeDefinitions triangle
    let e = ev("triangle");
    full(&e);
    has(&e, 0.0, 1000.0);
    has(&e, 1000.0, 0.0);
    has(&e, 2000.0, 1000.0);
    // x1 = w·a/200000 = 500, x3 = x1 + wd2 = 1500
    text_rect(&e, [500.0, 500.0, 1500.0, 1000.0]);
    let e = ev_av("triangle", &[("adj", "val 0")]);
    has(&e, 0.0, 0.0);
}

#[test]
fn rt_triangle() {
    // source: ECMA-376 Part 1 presetShapeDefinitions rtTriangle
    let e = ev("rtTriangle");
    full(&e);
    has(&e, 0.0, 0.0);
    text_rect(&e, [166.667, 583.333, 1166.667, 916.667]);
}

#[test]
fn diamond() {
    // source: ECMA-376 Part 1 presetShapeDefinitions diamond
    let e = ev("diamond");
    full(&e);
    has(&e, 0.0, 500.0);
    has(&e, 1000.0, 0.0);
    text_rect(&e, [500.0, 250.0, 1500.0, 750.0]);
}

#[test]
fn parallelogram() {
    // source: ECMA-376 Part 1 presetShapeDefinitions parallelogram
    // x2 = ss·25000/100000 = 250; x6 = r − x2 = 1750
    let e = ev("parallelogram");
    full(&e);
    has(&e, 250.0, 0.0);
    has(&e, 1750.0, 1000.0);
}

#[test]
fn trapezoid() {
    // source: ECMA-376 Part 1 presetShapeDefinitions trapezoid
    let e = ev("trapezoid");
    full(&e);
    has(&e, 250.0, 0.0);
    has(&e, 1750.0, 0.0);
    has(&e, 0.0, 1000.0);
}

#[test]
fn pentagon() {
    // source: ECMA-376 Part 1 presetShapeDefinitions pentagon
    // hf/vf scale the regular pentagon so it fills the box.
    let e = ev("pentagon");
    full(&e);
    has(&e, 1000.0, 0.0);
    // y1 = svc − shd2·sin 18° = 552.785 − 170.82 = 381.97
    has(&e, 0.0, 381.966);
}

#[test]
fn hexagon() {
    // source: ECMA-376 Part 1 presetShapeDefinitions hexagon
    // x1 = 250; dy1 = shd2·sin 60° = hd2 → y1 = t
    let e = ev("hexagon");
    full(&e);
    has(&e, 0.0, 500.0);
    has(&e, 250.0, 0.0);
    has(&e, 1750.0, 1000.0);
}

#[test]
fn octagon() {
    // source: ECMA-376 Part 1 presetShapeDefinitions octagon
    let e = ev("octagon");
    full(&e);
    has(&e, 0.0, 292.89);
    has(&e, 292.89, 0.0);
    has(&e, 1707.11, 1000.0);
}

#[test]
fn star4() {
    // source: ECMA-376 Part 1 presetShapeDefinitions star4
    // iwd2 = 250, ihd2 = 125; sdx = 176.78, sdy = 88.39
    let e = ev("star4");
    full(&e);
    has(&e, 1000.0, 0.0);
    has(&e, 823.223, 411.612);
}

#[test]
fn star5() {
    // source: ECMA-376 Part 1 presetShapeDefinitions star5
    let e = ev("star5");
    full(&e);
    has(&e, 1000.0, 0.0);
    has(&e, 0.0, 381.966);
    assert_eq!(points(&e).len(), 10);
}

#[test]
fn star6() {
    // source: ECMA-376 Part 1 presetShapeDefinitions star6
    // dx1 = swd2·cos 30° = 1154.7·0.866 = 1000 → x1 = l, x2 = r
    let e = ev("star6");
    full(&e);
    has(&e, 0.0, 250.0);
    has(&e, 2000.0, 750.0);
    has(&e, 1000.0, 1000.0);
}

#[test]
fn plus() {
    // source: ECMA-376 Part 1 presetShapeDefinitions plus
    let e = ev("plus");
    full(&e);
    has(&e, 250.0, 250.0);
    has(&e, 1750.0, 0.0);
    has(&e, 2000.0, 750.0);
    // d = w − h > 0 → il = l, it = x1
    text_rect(&e, [0.0, 250.0, 2000.0, 750.0]);
}

#[test]
fn right_arrow() {
    // source: ECMA-376 Part 1 presetShapeDefinitions rightArrow
    // dx1 = 500 → x1 = 1500; dy1 = 250
    let e = ev("rightArrow");
    full(&e);
    has(&e, 0.0, 250.0);
    has(&e, 1500.0, 0.0);
    has(&e, 2000.0, 500.0);
}

#[test]
fn left_arrow() {
    // source: ECMA-376 Part 1 presetShapeDefinitions leftArrow
    let e = ev("leftArrow");
    full(&e);
    has(&e, 0.0, 500.0);
    has(&e, 500.0, 0.0);
    has(&e, 2000.0, 250.0);
}

#[test]
fn up_arrow() {
    // source: ECMA-376 Part 1 presetShapeDefinitions upArrow
    // maxAdj2 = 100000·h/ss = 100000; dy2 = 500; dx1 = w·a1/200000 = 500
    let e = ev("upArrow");
    full(&e);
    has(&e, 1000.0, 0.0);
    has(&e, 1500.0, 500.0);
    has(&e, 500.0, 1000.0);
}

#[test]
fn down_arrow() {
    // source: ECMA-376 Part 1 presetShapeDefinitions downArrow
    let e = ev("downArrow");
    full(&e);
    has(&e, 1000.0, 1000.0);
    has(&e, 500.0, 0.0);
    has(&e, 0.0, 500.0);
}

#[test]
fn left_right_arrow() {
    // source: ECMA-376 Part 1 presetShapeDefinitions leftRightArrow
    let e = ev("leftRightArrow");
    full(&e);
    has(&e, 500.0, 250.0);
    has(&e, 1500.0, 750.0);
}

#[test]
fn up_down_arrow() {
    // source: ECMA-376 Part 1 presetShapeDefinitions upDownArrow
    // maxAdj2 = 50000·h/ss = 50000 → y2 = y3 = 500
    let e = ev("upDownArrow");
    full(&e);
    has(&e, 1000.0, 0.0);
    has(&e, 1000.0, 1000.0);
    has(&e, 1500.0, 500.0);
}

#[test]
fn notched_right_arrow() {
    // source: ECMA-376 Part 1 presetShapeDefinitions notchedRightArrow
    // x1 = dy1·dx2/hd2 = 250·500/500 = 250
    let e = ev("notchedRightArrow");
    full(&e);
    has(&e, 250.0, 500.0);
    has(&e, 2000.0, 500.0);
}

#[test]
fn chevron() {
    // source: ECMA-376 Part 1 presetShapeDefinitions chevron
    let e = ev("chevron");
    full(&e);
    has(&e, 1500.0, 0.0);
    has(&e, 500.0, 500.0);
}

#[test]
fn home_plate() {
    // source: ECMA-376 Part 1 presetShapeDefinitions homePlate
    let e = ev("homePlate");
    full(&e);
    has(&e, 1500.0, 0.0);
    has(&e, 2000.0, 500.0);
}

#[test]
fn can() {
    // source: ECMA-376 Part 1 presetShapeDefinitions can
    // y1 = ss·25000/200000 = 125, y3 = 875
    let e = ev("can");
    full(&e);
    assert_eq!(e.paths.len(), 3);
    assert_eq!(e.paths[1].fill, FillMode::Lighten);
    assert_eq!(e.paths[2].fill, FillMode::None);
    assert!(!e.paths[0].stroke);
    has(&e, 2000.0, 875.0);
    text_rect(&e, [0.0, 250.0, 2000.0, 875.0]);
}

#[test]
fn cube() {
    // source: ECMA-376 Part 1 presetShapeDefinitions cube
    let e = ev("cube");
    full(&e);
    assert_eq!(e.paths.len(), 4);
    assert_eq!(e.paths[1].fill, FillMode::DarkenLess);
    assert_eq!(e.paths[2].fill, FillMode::LightenLess);
    has(&e, 250.0, 0.0);
    has(&e, 1750.0, 250.0);
    has(&e, 2000.0, 750.0);
}

#[test]
fn donut() {
    // source: ECMA-376 Part 1 presetShapeDefinitions donut
    // dr = 250: inner ellipse radii 750 × 250
    let e = ev("donut");
    full(&e);
    has(&e, 250.0, 500.0);
    has(&e, 1000.0, 250.0);
}

#[test]
fn no_smoking() {
    // source: ECMA-376 Part 1 presetShapeDefinitions noSmoking
    let e = ev("noSmoking");
    full(&e);
    let moves = e.paths[0]
        .segs
        .iter()
        .filter(|s| matches!(s, Seg::Move(..)))
        .count();
    assert_eq!(moves, 3);
}

#[test]
fn block_arc() {
    // source: ECMA-376 Part 1 presetShapeDefinitions blockArc
    // stAng = 180°, istAng = 0 → swAng = +180° over the top; dr = 250
    let e = ev("blockArc");
    bbox(&e, [0.0, 0.0, 2000.0, 500.0]);
    has(&e, 2000.0, 500.0);
    has(&e, 1750.0, 500.0);
    has(&e, 250.0, 500.0);
}

#[test]
fn pie() {
    // source: ECMA-376 Part 1 presetShapeDefinitions pie
    // 0° → 270° clockwise, then to the centre.
    let e = ev("pie");
    full(&e);
    has(&e, 2000.0, 500.0);
    has(&e, 1000.0, 0.0);
    has(&e, 1000.0, 500.0);
}

#[test]
fn arc() {
    // source: ECMA-376 Part 1 presetShapeDefinitions arc
    // 270° → 0°: quarter from the top to the right.
    let e = ev("arc");
    assert_eq!(e.paths.len(), 2);
    assert_eq!(e.paths[1].fill, FillMode::None);
    has(&e, 1000.0, 0.0);
    has(&e, 2000.0, 500.0);
    let stroke_only = Evaluated {
        paths: vec![e.paths[1].clone()],
        text_rect: e.text_rect,
    };
    bbox(&stroke_only, [1000.0, 0.0, 2000.0, 500.0]);
}

#[test]
fn chord() {
    // source: ECMA-376 Part 1 presetShapeDefinitions chord
    // stAng 45°: atan2(wd2·sin45, hd2·cos45) = 63.435° → (1447.21, 947.21)
    let e = ev("chord");
    has(&e, 1447.214, 947.214);
    has(&e, 1000.0, 0.0);
}

#[test]
fn heart() {
    // source: ECMA-376 Part 1 presetShapeDefinitions heart
    // dx1 = w·49/48 = 2041.67; y1 = −hd3 = −333.33 (control points)
    let e = ev("heart");
    has(&e, 1000.0, 250.0);
    has(&e, 1000.0, 1000.0);
    bbox(&e, [-1041.667, -333.333, 3041.667, 1000.0]);
}

#[test]
fn line_and_connectors() {
    // source: ECMA-376 Part 1 presetShapeDefinitions line
    let e = ev("line");
    has(&e, 0.0, 0.0);
    has(&e, 2000.0, 1000.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions straightConnector1
    let e = ev("straightConnector1");
    assert_eq!(e.paths[0].fill, FillMode::None);
    full(&e);
    // source: ECMA-376 Part 1 presetShapeDefinitions bentConnector2
    let e = ev("bentConnector2");
    has(&e, 2000.0, 0.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions bentConnector3
    let e = ev("bentConnector3");
    has(&e, 1000.0, 0.0);
    has(&e, 1000.0, 1000.0);
    let e = ev_av("bentConnector3", &[("adj1", "val 25000")]);
    has(&e, 500.0, 0.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions curvedConnector2
    let e = ev("curvedConnector2");
    has(&e, 2000.0, 1000.0);
    full(&e);
    // source: ECMA-376 Part 1 presetShapeDefinitions curvedConnector3
    let e = ev("curvedConnector3");
    has(&e, 1000.0, 500.0);
    has(&e, 2000.0, 1000.0);
}

#[test]
fn flow_chart_basic() {
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartProcess
    let e = ev("flowChartProcess");
    full(&e);
    has(&e, 2000.0, 1000.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartDecision
    let e = ev("flowChartDecision");
    full(&e);
    has(&e, 1000.0, 0.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartTerminator
    // 3475/21600·w = 321.76
    let e = ev("flowChartTerminator");
    full(&e);
    has(&e, 321.759, 0.0);
    has(&e, 1678.241, 0.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartAlternateProcess
    let e = ev("flowChartAlternateProcess");
    full(&e);
    has(&e, 0.0, 166.667);
    has(&e, 1833.333, 0.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartInputOutput
    let e = ev("flowChartInputOutput");
    full(&e);
    has(&e, 400.0, 0.0);
    has(&e, 1600.0, 1000.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartDocument
    // 17322/21600·h = 801.94; control y 23922/21600·h = 1107.5
    let e = ev("flowChartDocument");
    has(&e, 2000.0, 801.944);
    bbox(&e, [0.0, 0.0, 2000.0, 1107.5]);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartManualInput
    let e = ev("flowChartManualInput");
    has(&e, 0.0, 200.0);
    has(&e, 2000.0, 0.0);
}

#[test]
fn flow_chart_more() {
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartPredefinedProcess
    let e = ev("flowChartPredefinedProcess");
    assert_eq!(e.paths.len(), 3);
    has(&e, 250.0, 1000.0);
    has(&e, 1750.0, 0.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartPreparation
    let e = ev("flowChartPreparation");
    full(&e);
    has(&e, 400.0, 0.0);
    has(&e, 1600.0, 1000.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartMerge
    let e = ev("flowChartMerge");
    has(&e, 1000.0, 1000.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartExtract
    let e = ev("flowChartExtract");
    has(&e, 1000.0, 0.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartManualOperation
    let e = ev("flowChartManualOperation");
    has(&e, 1600.0, 1000.0);
    has(&e, 400.0, 1000.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartOffpageConnector
    let e = ev("flowChartOffpageConnector");
    has(&e, 2000.0, 800.0);
    has(&e, 1000.0, 1000.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartPunchedCard
    let e = ev("flowChartPunchedCard");
    has(&e, 0.0, 200.0);
    has(&e, 400.0, 0.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartCollate
    let e = ev("flowChartCollate");
    has(&e, 1000.0, 500.0);
    full(&e);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartSort
    let e = ev("flowChartSort");
    assert_eq!(e.paths.len(), 3);
    has(&e, 2000.0, 500.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions flowChartDelay
    let e = ev("flowChartDelay");
    full(&e);
    has(&e, 1000.0, 1000.0);
    has(&e, 2000.0, 500.0);
}

#[test]
fn wedge_rect_callout() {
    // source: ECMA-376 Part 1 presetShapeDefinitions wedgeRectCallout
    // xPos = hc + w·(−20833)/100000 = 583.34; yPos = vc + h·62500/100000 = 1125
    let e = ev("wedgeRectCallout");
    has(&e, 583.34, 1125.0);
    bbox(&e, [0.0, 0.0, 2000.0, 1125.0]);
    // Tail to the right: adj1 = 70000, adj2 = 0 → (2400, 500).
    let e = ev_av(
        "wedgeRectCallout",
        &[("adj1", "val 70000"), ("adj2", "val 0")],
    );
    has(&e, 2400.0, 500.0);
}

#[test]
fn wedge_round_rect_callout() {
    // source: ECMA-376 Part 1 presetShapeDefinitions wedgeRoundRectCallout
    let e = ev("wedgeRoundRectCallout");
    has(&e, 583.34, 1125.0);
    has(&e, 166.67, 0.0);
    bbox(&e, [0.0, 0.0, 2000.0, 1125.0]);
}

#[test]
fn snip_and_round_rects() {
    // source: ECMA-376 Part 1 presetShapeDefinitions snip1Rect
    let e = ev("snip1Rect");
    full(&e);
    has(&e, 1833.33, 0.0);
    has(&e, 2000.0, 166.67);
    // source: ECMA-376 Part 1 presetShapeDefinitions snip2SameRect
    let e = ev("snip2SameRect");
    has(&e, 166.67, 0.0);
    has(&e, 2000.0, 1000.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions snip2DiagRect
    let e = ev("snip2DiagRect");
    has(&e, 0.0, 0.0);
    has(&e, 1833.33, 0.0);
    has(&e, 2000.0, 166.67);
    // source: ECMA-376 Part 1 presetShapeDefinitions snipRoundRect
    let e = ev("snipRoundRect");
    has(&e, 1833.33, 0.0);
    has(&e, 0.0, 1000.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions round1Rect
    let e = ev("round1Rect");
    full(&e);
    has(&e, 1833.33, 0.0);
    has(&e, 2000.0, 166.67);
    // source: ECMA-376 Part 1 presetShapeDefinitions round2SameRect
    let e = ev("round2SameRect");
    full(&e);
    has(&e, 166.67, 0.0);
    has(&e, 2000.0, 1000.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions round2DiagRect
    let e = ev("round2DiagRect");
    full(&e);
    has(&e, 2000.0, 0.0);
    has(&e, 2000.0, 833.33);
}

#[test]
fn frames_and_corners() {
    // source: ECMA-376 Part 1 presetShapeDefinitions frame
    let e = ev("frame");
    full(&e);
    has(&e, 125.0, 125.0);
    has(&e, 1875.0, 875.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions halfFrame
    // x1 = 333.33; y1 = 333.33; x2 = r − y1·w/h = 1333.33; y2 = b − x1·h/w = 833.33
    let e = ev("halfFrame");
    has(&e, 1333.333, 333.333);
    has(&e, 333.333, 833.333);
    // source: ECMA-376 Part 1 presetShapeDefinitions corner
    let e = ev("corner");
    full(&e);
    has(&e, 500.0, 500.0);
    has(&e, 2000.0, 500.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions plaque
    let e = ev("plaque");
    full(&e);
    has(&e, 166.67, 0.0);
    has(&e, 0.0, 166.67);
    // source: ECMA-376 Part 1 presetShapeDefinitions foldedCorner
    let e = ev("foldedCorner");
    full(&e);
    assert_eq!(e.paths[1].fill, FillMode::DarkenLess);
    has(&e, 1833.333, 1000.0);
    has(&e, 1866.667, 866.667);
}

#[test]
fn brackets() {
    // source: ECMA-376 Part 1 presetShapeDefinitions bracketPair
    let e = ev("bracketPair");
    full(&e);
    assert_eq!(e.paths.len(), 2);
    has(&e, 0.0, 166.67);
    // source: ECMA-376 Part 1 presetShapeDefinitions bracePair
    // x1 = ss·8333/100000 = 83.33, x2 = 166.66
    let e = ev("bracePair");
    full(&e);
    has(&e, 0.0, 500.0);
    has(&e, 2000.0, 500.0);
    has(&e, 166.66, 1000.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions leftBracket
    // y1 = 83.33; arcs of radius w × y1
    let e = ev("leftBracket");
    full(&e);
    has(&e, 0.0, 916.67);
    has(&e, 2000.0, 0.0);
    // source: ECMA-376 Part 1 presetShapeDefinitions rightBracket
    let e = ev("rightBracket");
    full(&e);
    has(&e, 2000.0, 83.33);
    has(&e, 0.0, 1000.0);
}

#[test]
fn unknown_preset_falls_back_to_rect() {
    let mut warnings = Vec::new();
    let g = bo_model::slide::Geometry::Preset {
        name: "doesNotExist".into(),
        av: vec![],
    };
    let e = bo_drawingml::evaluate_geometry(Some(&g), W, H, &mut warnings);
    full(&e);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn every_preset_evaluates_at_odd_sizes() {
    for name in bo_drawingml::preset_names() {
        for (w, h) in [(1000.0, 3000.0), (500.0, 500.0), (0.0, 100.0)] {
            let e = eval_preset(name, &[], w, h).unwrap();
            let e = e.unwrap_or_else(|err| panic!("{name}: {err}"));
            assert!(!e.paths.is_empty(), "{name}");
            for p in points(&e) {
                assert!(p.0.is_finite() && p.1.is_finite(), "{name} {p:?}");
            }
        }
    }
}

#[test]
fn full_annex_preset_set() {
    // ECMA-376 Part 1, §20.1.10.56 ST_ShapeType lists 187 preset shapes.
    let names = bo_drawingml::preset_names();
    assert_eq!(names.len(), 187);
    for n in [
        "star32",
        "gear9",
        "cloud",
        "lightningBolt",
        "mathMultiply",
        "actionButtonHome",
        "borderCallout3",
        "ellipseRibbon2",
        "circularArrow",
        "chartPlus",
        "funnel",
        "swooshArrow",
    ] {
        assert!(names.contains(&n), "{n}");
    }
}
