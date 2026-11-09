//! Shape geometry tools (CORE-SHAPE-001): edit points, merge shapes and
//! connectors on connection sites. Expected values are derived by hand from
//! the shapes' boxes (points) and ECMA-376 Part 1 presetShapeDefinitions.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::{freeform, SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::{Geometry, PathCommand, ShapeNode};
use std::f64::consts::PI;
use std::path::PathBuf;

const EMU_PT: f64 = 12_700.0;

fn open() -> Engine {
    let name = "slide-shapes-presets.pptx";
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    let bytes = std::fs::read(&p).unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, name).unwrap();
    e
}

fn try_run(e: &mut Engine, json: &str) -> SlideEditResult {
    let cmd: SlideCommand =
        serde_json::from_str(json).unwrap_or_else(|err| panic!("{json}: {err}"));
    e.slide_command(cmd).unwrap()
}

fn run(e: &mut Engine, json: &str) -> SlideEditResult {
    let r = try_run(e, json);
    assert!(r.error.is_none(), "{json}: {:?}", r.error);
    r
}

fn insert(e: &mut Engine, preset: &str, x: f64, y: f64, w: f64, h: f64) -> u32 {
    run(
        e,
        &format!(
            r#"{{"cmd":"insertShape","preset":"{preset}","rect":{{"x":{x},"y":{y},"w":{w},"h":{h}}}}}"#
        ),
    )
    .selection[0]
        .id
}

fn select(e: &mut Engine, ids: &[u32]) -> SlideEditResult {
    run(e, &format!(r#"{{"cmd":"select","ids":{ids:?}}}"#))
}

fn top_shapes(e: &Engine) -> Vec<bo_model::slide::Shape> {
    e.presentation().unwrap().slides[0]
        .data
        .shapes
        .iter()
        .filter_map(|n| match n {
            ShapeNode::Shape(s) => Some((**s).clone()),
            _ => None,
        })
        .collect()
}

fn shape(e: &Engine, id: u32) -> bo_model::slide::Shape {
    top_shapes(e)
        .into_iter()
        .find(|s| s.nv.id == id)
        .expect("shape")
}

/// Net area (points²) of a merged freeform: its line contours in EMU.
fn area_pt2(s: &bo_model::slide::Shape) -> f64 {
    let Some(Geometry::Custom(c)) = &s.props.geometry else {
        panic!("custom geometry expected");
    };
    let mut total = 0.0;
    for p in &c.paths {
        let mut ring: Vec<(f64, f64)> = Vec::new();
        let flush = |ring: &mut Vec<(f64, f64)>| {
            let n = ring.len();
            let mut a = 0.0;
            for i in 0..n {
                let (x0, y0) = ring[i];
                let (x1, y1) = ring[(i + 1) % n];
                a += x0 * y1 - x1 * y0;
            }
            ring.clear();
            a / 2.0
        };
        for cmd in &p.commands {
            match cmd {
                PathCommand::MoveTo(x, y) | PathCommand::LineTo(x, y) => {
                    ring.push((x.parse().unwrap(), y.parse().unwrap()));
                }
                PathCommand::Close => total += flush(&mut ring),
                other => panic!("merged outlines are polygons: {other:?}"),
            }
        }
    }
    total / (EMU_PT * EMU_PT)
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn edit_points_of_a_rectangle() {
    let mut e = open();
    let id = insert(&mut e, "rect", 100.0, 100.0, 100.0, 60.0);
    let r = run(&mut e, r#"{"cmd":"convertToFreeform"}"#);
    let ff = r.freeform.expect("freeform state");
    assert_eq!(ff.id, id);
    let pts: Vec<[f64; 2]> = ff.paths[0].points.iter().map(|p| [p.x, p.y]).collect();
    // rect path (annex): l,t → r,t → r,b → l,b.
    let want = [
        [100.0, 100.0],
        [200.0, 100.0],
        [200.0, 160.0],
        [100.0, 160.0],
    ];
    for (g, w) in pts.iter().zip(want) {
        assert!(near(g[0], w[0], 0.01) && near(g[1], w[1], 0.01), "{pts:?}");
    }
    assert!(matches!(
        shape(&e, id).props.geometry,
        Some(Geometry::Custom(_))
    ));
    // Drag the bottom-right corner to (250, 200): the box grows to
    // x 100…250, y 100…200 (centre 175, 150; 150 × 100).
    let r = run(
        &mut e,
        r#"{"cmd":"editPoint","edit":{"kind":"move","path":0,"point":2,"x":240,"y":190},"merge":true}"#,
    );
    assert!(r.freeform.is_some());
    let r = run(
        &mut e,
        r#"{"cmd":"editPoint","edit":{"kind":"move","path":0,"point":2,"x":250,"y":200},"merge":true}"#,
    );
    let b = r.selection[0].rect;
    assert!(near(b.cx, 175.0, 0.01) && near(b.cy, 150.0, 0.01), "{b:?}");
    assert!(near(b.w, 150.0, 0.01) && near(b.h, 100.0, 0.01), "{b:?}");
    let p2 = &r.freeform.as_ref().unwrap().paths[0].points[2];
    assert!(near(p2.x, 250.0, 0.01) && near(p2.y, 200.0, 0.01));
    let p0 = &r.freeform.as_ref().unwrap().paths[0].points[0];
    assert!(near(p0.x, 100.0, 0.01) && near(p0.y, 100.0, 0.01));
    // The two drag steps are one undo step.
    let r = run(&mut e, r#"{"cmd":"undo"}"#);
    let b = r.selection[0].rect;
    assert!(near(b.w, 100.0, 0.01) && near(b.h, 60.0, 0.01), "{b:?}");
    run(&mut e, r#"{"cmd":"redo"}"#);
    // Add a point on the top edge near (150, 101): it lands on the edge.
    let r = run(
        &mut e,
        r#"{"cmd":"editPoint","edit":{"kind":"add","x":150,"y":101,"tolerance":5}}"#,
    );
    let pts = &r.freeform.as_ref().unwrap().paths[0].points;
    assert_eq!(pts.len(), 5);
    assert!(near(pts[1].x, 150.0, 0.01) && near(pts[1].y, 100.0, 0.01));
    // Too far from the outline: refused, nothing changes.
    let r = try_run(
        &mut e,
        r#"{"cmd":"editPoint","edit":{"kind":"add","x":170,"y":150,"tolerance":5}}"#,
    );
    assert_eq!(r.error.as_deref(), Some("noSegment"));
    // Curve the first segment, make the new point smooth, then delete it.
    let r = run(
        &mut e,
        r#"{"cmd":"editPoint","edit":{"kind":"segment","path":0,"segment":0,"curved":true}}"#,
    );
    assert!(r.freeform.as_ref().unwrap().paths[0].segments[0].curved);
    let r = run(
        &mut e,
        r#"{"cmd":"editPoint","edit":{"kind":"pointType","path":0,"point":1,"pointType":"smooth"}}"#,
    );
    let p1 = &r.freeform.as_ref().unwrap().paths[0].points[1];
    assert_ne!(p1.kind, freeform::PointType::Corner);
    // Its handles lie on the horizontal through (150, 100) (prev and next
    // anchors are both at y = 100).
    for h in p1.handles.iter().flatten() {
        assert!(near(h[1], 100.0, 0.01), "{p1:?}");
    }
    let r = run(
        &mut e,
        r#"{"cmd":"editPoint","edit":{"kind":"delete","path":0,"point":1}}"#,
    );
    assert_eq!(r.freeform.unwrap().paths[0].points.len(), 4);
    // Saved and reopened: the custom geometry survives.
    let bytes = e.save_pptx().unwrap();
    let p = bo_pptx::read_pptx(&bytes).unwrap();
    let s = p.slides[0]
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            ShapeNode::Shape(s) if s.nv.id == id => Some((**s).clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(s.props.geometry, shape(&e, id).props.geometry);
    save_for_validation(&bytes, "edit-points.pptx");
}

/// Keep a written deck for `bo-cli validate --openxml require`.
fn save_for_validation(bytes: &[u8], name: &str) {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::write(dir.join(name), bytes).unwrap();
}

#[test]
fn edit_points_of_a_rotated_shape() {
    let mut e = open();
    let id = insert(&mut e, "triangle", 100.0, 100.0, 100.0, 60.0);
    // Rotate by 90° (pointer to the right of the centre with steps).
    let r = run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"rotate","x":1000,"y":130,"step":true}}"#,
    );
    assert!(near(r.selection[0].rect.rot, 90.0, 1e-6));
    let r = run(&mut e, r#"{"cmd":"convertToFreeform"}"#);
    // triangle apex (hc, t) = local (50, 0); rotated 90° about (150, 130):
    // local offset (0, −30) → slide offset (30, 0) → (180, 130).
    let pts = &r.freeform.as_ref().unwrap().paths[0].points;
    assert!(
        pts.iter()
            .any(|p| near(p.x, 180.0, 0.01) && near(p.y, 130.0, 0.01)),
        "{pts:?}"
    );
    let k = pts.iter().position(|p| near(p.x, 180.0, 0.01)).unwrap();
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"editPoint","edit":{{"kind":"move","path":0,"point":{k},"x":200,"y":140}}}}"#
        ),
    );
    let p = &r.freeform.as_ref().unwrap().paths[0].points[k];
    assert!(near(p.x, 200.0, 0.01) && near(p.y, 140.0, 0.01), "{p:?}");
    assert!(near(r.selection[0].rect.rot, 90.0, 1e-6));
    // The other two corners stay put: base l,b and r,b → local (0, 60), (100, 60)
    // → offsets (−50, 30), (50, 30) → slide (120, 80), (120, 180).
    for want in [[120.0, 80.0], [120.0, 180.0]] {
        let pts = &r.freeform.as_ref().unwrap().paths[0].points;
        assert!(
            pts.iter()
                .any(|p| near(p.x, want[0], 0.01) && near(p.y, want[1], 0.01)),
            "{want:?} {pts:?}"
        );
    }
    assert_eq!(shape(&e, id).nv.id, id);
}

fn two_squares(e: &mut Engine) -> (u32, u32) {
    // A = [100,200]², B = [150,250]²: overlap 50 × 50.
    let a = insert(e, "rect", 100.0, 100.0, 100.0, 100.0);
    let b = insert(e, "rect", 150.0, 150.0, 100.0, 100.0);
    select(e, &[a, b]);
    (a, b)
}

#[test]
fn merge_two_squares() {
    // By hand: |A| = |B| = 10 000, |A∩B| = 2 500, |A∪B| = 17 500,
    // |A−B| = 7 500, |A xor B| = 15 000.
    for (op, area, count) in [
        ("union", 17_500.0, 1),
        ("intersect", 2_500.0, 1),
        ("subtract", 7_500.0, 1),
        ("combine", 15_000.0, 1),
    ] {
        let mut e = open();
        let before = top_shapes(&e).len();
        let (a, b) = two_squares(&mut e);
        let r = run(&mut e, &format!(r#"{{"cmd":"mergeShapes","op":"{op}"}}"#));
        assert_eq!(r.selection.len(), count, "{op}");
        let id = r.selection[0].id;
        assert_ne!(id, a);
        assert_ne!(id, b);
        let s = shape(&e, id);
        assert!(near(area_pt2(&s), area, 0.01), "{op}: {}", area_pt2(&s));
        assert_eq!(top_shapes(&e).len(), before + 1, "{op}");
        // The merged shape takes the first shape's formatting.
        let first_style = {
            let mut f = open();
            let (fa, _) = two_squares(&mut f);
            shape(&f, fa).style
        };
        assert_eq!(s.style, first_style);
        // One undo step brings both squares back.
        run(&mut e, r#"{"cmd":"undo"}"#);
        let ids: Vec<u32> = top_shapes(&e).iter().map(|s| s.nv.id).collect();
        assert!(ids.contains(&a) && ids.contains(&b) && !ids.contains(&id));
        if op == "intersect" {
            // Box of A∩B: [150,200]².
            run(&mut e, r#"{"cmd":"redo"}"#);
            let x = shape(&e, id).props.xfrm.unwrap();
            assert_eq!(x.off, Some((150 * 12_700, 150 * 12_700)));
            assert_eq!(x.ext, Some((50 * 12_700, 50 * 12_700)));
        }
    }
}

#[test]
fn subtract_follows_the_selection_order() {
    let mut e = open();
    let (a, b) = two_squares(&mut e);
    // B − A: B is selected first.
    select(&mut e, &[b, a]);
    let r = run(&mut e, r#"{"cmd":"mergeShapes","op":"subtract"}"#);
    let s = shape(&e, r.selection[0].id);
    assert!(near(area_pt2(&s), 7_500.0, 0.01));
    // B − A spans x 150…250.
    let x = s.props.xfrm.unwrap();
    assert_eq!(x.off, Some((150 * 12_700, 150 * 12_700)));
}

#[test]
fn fragment_two_squares() {
    let mut e = open();
    let before = top_shapes(&e).len();
    two_squares(&mut e);
    let r = run(&mut e, r#"{"cmd":"mergeShapes","op":"fragment"}"#);
    assert_eq!(r.selection.len(), 3);
    let mut areas: Vec<f64> = r
        .selection
        .iter()
        .map(|s| area_pt2(&shape(&e, s.id)))
        .collect();
    areas.sort_by(f64::total_cmp);
    assert!(near(areas[0], 2_500.0, 0.01));
    assert!(near(areas[1], 7_500.0, 0.01) && near(areas[2], 7_500.0, 0.01));
    assert_eq!(top_shapes(&e).len(), before + 3);
    let bytes = e.save_pptx().unwrap();
    save_for_validation(&bytes, "merge-fragment.pptx");
}

#[test]
fn merge_curves_and_holes() {
    let mut e = open();
    // Circle r = 50 at (150, 150) intersected with its left half [100,150]×[100,200]:
    // a half disc, π·50²/2 ≈ 3 926.99.
    let c = insert(&mut e, "ellipse", 100.0, 100.0, 100.0, 100.0);
    let h = insert(&mut e, "rect", 100.0, 100.0, 50.0, 100.0);
    select(&mut e, &[c, h]);
    let r = run(&mut e, r#"{"cmd":"mergeShapes","op":"intersect"}"#);
    let half = PI * 2_500.0 / 2.0;
    let got = area_pt2(&shape(&e, r.selection[0].id));
    assert!((got - half).abs() / half < 0.005, "{got} vs {half}");
    // A square with a smaller one cut out: 100² − 40² = 8 400, two contours.
    let a = insert(&mut e, "rect", 300.0, 100.0, 100.0, 100.0);
    let b = insert(&mut e, "rect", 330.0, 130.0, 40.0, 40.0);
    select(&mut e, &[a, b]);
    let r = run(&mut e, r#"{"cmd":"mergeShapes","op":"subtract"}"#);
    let s = shape(&e, r.selection[0].id);
    assert!(near(area_pt2(&s), 8_400.0, 0.01));
    let Some(Geometry::Custom(g)) = &s.props.geometry else {
        panic!()
    };
    let closes = g.paths[0]
        .commands
        .iter()
        .filter(|c| matches!(c, PathCommand::Close))
        .count();
    assert_eq!(closes, 2, "outer boundary and hole");
    // Merging needs two shapes.
    select(&mut e, &[a]);
    let r = try_run(&mut e, r#"{"cmd":"mergeShapes","op":"union"}"#);
    assert!(r.error.is_some());
}

#[test]
fn elbow_connector_routes() {
    let mut e = open();
    // A = [100,200]×[100,160], B = [300,400]×[300,360]. Nearest sites:
    // A right (200,130) – B left (300,330), distance √(100² + 200²) ≈ 223.6.
    let a = insert(&mut e, "rect", 100.0, 100.0, 100.0, 60.0);
    let b = insert(&mut e, "rect", 300.0, 300.0, 100.0, 60.0);
    select(&mut e, &[a, b]);
    let r = run(
        &mut e,
        r#"{"cmd":"insertConnector","preset":"bentConnector3"}"#,
    );
    let cid = r.selection[0].id;
    let pts = freeform::connector_points(e.presentation().unwrap(), 0, cid).unwrap();
    // bentConnector3: (l,t) → (x1,t) → (x1,b) → (r,b), x1 = w·adj1 = 50%.
    let want = [
        [200.0, 130.0],
        [250.0, 130.0],
        [250.0, 330.0],
        [300.0, 330.0],
    ];
    assert_eq!(pts.len(), 4, "{pts:?}");
    for (g, w) in pts.iter().zip(want) {
        assert!(near(g.0, w[0], 0.01) && near(g.1, w[1], 0.01), "{pts:?}");
    }
    // Move B down by 100: the bend stays at x = 250, the end follows the site.
    select(&mut e, &[b]);
    run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"move","dx":0,"dy":100},"snap":false}"#,
    );
    let pts = freeform::connector_points(e.presentation().unwrap(), 0, cid).unwrap();
    let want = [
        [200.0, 130.0],
        [250.0, 130.0],
        [250.0, 430.0],
        [300.0, 430.0],
    ];
    for (g, w) in pts.iter().zip(want) {
        assert!(near(g.0, w[0], 0.01) && near(g.1, w[1], 0.01), "{pts:?}");
    }
}

#[test]
fn elbow_connector_leaves_a_bottom_site_vertically() {
    let mut e = open();
    // A = [100,200]×[100,160], B = [130,230]×[300,360]: A bottom (150,160) –
    // B top (180,300) are the nearest sites (√(30² + 140²) ≈ 143.2).
    let a = insert(&mut e, "rect", 100.0, 100.0, 100.0, 60.0);
    let b = insert(&mut e, "rect", 130.0, 300.0, 100.0, 60.0);
    select(&mut e, &[a, b]);
    let r = run(
        &mut e,
        r#"{"cmd":"insertConnector","preset":"bentConnector3"}"#,
    );
    let cid = r.selection[0].id;
    let conn = r.connector.unwrap();
    assert!(near(conn.ends[0][0], 150.0, 0.01) && near(conn.ends[0][1], 160.0, 0.01));
    assert!(near(conn.ends[1][0], 180.0, 0.01) && near(conn.ends[1][1], 300.0, 0.01));
    // Down, across at half the height (y = 230), down.
    let pts = freeform::connector_points(e.presentation().unwrap(), 0, cid).unwrap();
    let want = [
        [150.0, 160.0],
        [150.0, 230.0],
        [180.0, 230.0],
        [180.0, 300.0],
    ];
    for (g, w) in pts.iter().zip(want) {
        assert!(near(g.0, w[0], 0.01) && near(g.1, w[1], 0.01), "{pts:?}");
    }
    let bytes = e.save_pptx().unwrap();
    save_for_validation(&bytes, "elbow.pptx");
}

#[test]
fn connector_snaps_to_a_freeform_site() {
    let mut e = open();
    let a = insert(&mut e, "rect", 100.0, 100.0, 100.0, 60.0);
    run(&mut e, r#"{"cmd":"convertToFreeform"}"#);
    // The baked custom geometry keeps the rectangle's sites (cxnLst guides):
    // its right site is (200, 130).
    let b = insert(&mut e, "ellipse", 400.0, 100.0, 100.0, 60.0);
    select(&mut e, &[a, b]);
    let r = run(&mut e, r#"{"cmd":"insertConnector"}"#);
    let conn = r.connector.unwrap();
    assert_eq!(conn.attached, [true, true]);
    assert!(near(conn.ends[0][0], 200.0, 0.01) && near(conn.ends[0][1], 130.0, 0.01));
    // Resizing the freeform moves its sites with it: double the width from the
    // right edge → right site at (300, 130).
    select(&mut e, &[a]);
    run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"resize","handle":"e","dx":100,"dy":0},"snap":false}"#,
    );
    let cid = conn.id;
    let r = select(&mut e, &[cid]);
    let conn = r.connector.unwrap();
    assert!(
        near(conn.ends[0][0], 300.0, 0.01) && near(conn.ends[0][1], 130.0, 0.01),
        "{conn:?}"
    );
}
