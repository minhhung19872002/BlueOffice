//! Recogniser tests on synthetic strokes: every shape clean, noisy, rotated,
//! drawn with a gap or an overshoot, and strokes that must stay ink.
#![allow(clippy::unwrap_used, clippy::float_cmp)]

use super::*;

/// Deterministic noise (64-bit LCG, Knuth's constants) in −1..1.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
}

/// Points along a closed polygon (from its first corner, `per_edge` points per edge).
fn polyline(v: &[Pt], per_edge: usize, closed: bool) -> Vec<Pt> {
    let mut out = Vec::new();
    let k = v.len();
    let edges = if closed { k } else { k - 1 };
    for i in 0..edges {
        let (a, b) = (v[i], v[(i + 1) % k]);
        for s in 0..per_edge {
            let t = s as f64 / per_edge as f64;
            out.push((a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)));
        }
    }
    out.push(if closed { v[0] } else { v[k - 1] });
    out
}

/// Jitter each point by up to `amp` points.
fn jitter(p: &[Pt], amp: f64, seed: u64) -> Vec<Pt> {
    let mut n = Noise(seed);
    p.iter()
        .map(|&(x, y)| (x + amp * n.next(), y + amp * n.next()))
        .collect()
}

fn rot_about(p: &[Pt], c: Pt, deg: f64) -> Vec<Pt> {
    p.iter()
        .map(|&q| {
            let r = rotate((q.0 - c.0, q.1 - c.1), deg);
            (c.0 + r.0, c.1 + r.1)
        })
        .collect()
}

fn regular_poly(c: Pt, r: f64, k: usize, start_deg: f64) -> Vec<Pt> {
    (0..k)
        .map(|i| {
            let a = (start_deg + 360.0 * i as f64 / k as f64).to_radians();
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect()
}

fn ellipse_pts(c: Pt, a: f64, b: f64, rot: f64, n: usize, sweep: f64) -> Vec<Pt> {
    let p: Vec<Pt> = (0..=n)
        .map(|i| {
            let t = (sweep * i as f64 / n as f64).to_radians();
            (c.0 + a * t.cos(), c.1 + b * t.sin())
        })
        .collect();
    rot_about(&p, c, rot)
}

fn rect_corners(c: Pt, w: f64, h: f64) -> Vec<Pt> {
    vec![
        (c.0 - w / 2.0, c.1 - h / 2.0),
        (c.0 + w / 2.0, c.1 - h / 2.0),
        (c.0 + w / 2.0, c.1 + h / 2.0),
        (c.0 - w / 2.0, c.1 + h / 2.0),
    ]
}

fn kind(p: &[Pt]) -> Option<ShapeKind> {
    recognize_stroke(p).map(|r| r.kind)
}

fn frame_of_rec(r: &Recognized) -> (Pt, f64, f64, f64, Option<f64>) {
    match r.fit {
        Fit::Frame {
            center,
            w,
            h,
            rot,
            adj,
        } => (center, w, h, rot, adj),
        Fit::Line { .. } => panic!("not a frame: {r:?}"),
    }
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn resampling_is_equidistant() {
    let r = resample(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], 5);
    assert_eq!(r.len(), 5);
    assert_eq!(r[0], (0.0, 0.0));
    assert_eq!(r[2], (10.0, 0.0));
    assert_eq!(r[4], (10.0, 10.0));
    assert!(near(r[1].0, 5.0, 1e-9) && near(r[3].1, 5.0, 1e-9));
}

#[test]
fn lines_straight_noisy_and_snapped() {
    let p = polyline(&[(100.0, 100.0), (300.0, 104.0)], 40, false);
    let r = recognize_stroke(&jitter(&p, 1.5, 1)).unwrap();
    assert_eq!(r.kind, ShapeKind::Line);
    let Fit::Line { from, to, .. } = r.fit else {
        panic!()
    };
    // 1.1° off horizontal: snapped flat, ends kept within the jitter.
    assert!(near(from.1, to.1, 1e-6), "{from:?} {to:?}");
    assert!(near(from.0, 100.0, 3.0) && near(to.0, 300.0, 3.0));
    // A 30° line stays at 30°.
    let a = 30f64.to_radians();
    let p = polyline(
        &[
            (50.0, 50.0),
            (50.0 + 200.0 * a.cos(), 50.0 + 200.0 * a.sin()),
        ],
        40,
        false,
    );
    let Fit::Line { from, to, .. } = recognize_stroke(&jitter(&p, 1.0, 2)).unwrap().fit else {
        panic!()
    };
    let got = (to.1 - from.1).atan2(to.0 - from.0).to_degrees();
    assert!(near(got, 30.0, 2.0), "{got}");
}

#[test]
fn rectangles_and_squares() {
    let r = recognize_stroke(&jitter(
        &polyline(&rect_corners((200.0, 150.0), 240.0, 120.0), 25, true),
        2.0,
        3,
    ))
    .unwrap();
    assert_eq!(r.kind, ShapeKind::Rectangle);
    let (c, w, h, rot, _) = frame_of_rec(&r);
    assert!(near(c.0, 200.0, 4.0) && near(c.1, 150.0, 4.0));
    assert!(near(w, 240.0, 8.0) && near(h, 120.0, 8.0), "{w} {h}");
    assert_eq!(rot, 0.0);

    let sq = polyline(&rect_corners((100.0, 100.0), 100.0, 104.0), 25, true);
    let r = recognize_stroke(&jitter(&sq, 1.5, 4)).unwrap();
    assert_eq!(r.kind, ShapeKind::Square);
    let (_, w, h, _, _) = frame_of_rec(&r);
    assert_eq!(w, h);

    // Rotated 30°: found with its rotation.
    let rr = rot_about(
        &polyline(&rect_corners((300.0, 200.0), 200.0, 100.0), 25, true),
        (300.0, 200.0),
        30.0,
    );
    let r = recognize_stroke(&jitter(&rr, 1.5, 5)).unwrap();
    assert_eq!(r.kind, ShapeKind::Rectangle);
    let (_, w, h, rot, _) = frame_of_rec(&r);
    let (w, h, rot) = if w < h {
        (h, w, rot + 90.0)
    } else {
        (w, h, rot)
    };
    assert!(near(wrap(rot, 180.0), 30.0, 3.0), "{rot}");
    assert!(near(w, 200.0, 8.0) && near(h, 100.0, 8.0));
}

#[test]
fn rectangle_with_a_gap_or_an_overshoot() {
    let mut p = polyline(&rect_corners((150.0, 150.0), 200.0, 120.0), 25, true);
    // Stops 15 pt short of the start.
    p.truncate(p.len() - 4);
    assert_eq!(kind(&jitter(&p, 1.0, 6)), Some(ShapeKind::Rectangle));
    // Runs 40 pt past the start along the first edge.
    let mut p = polyline(&rect_corners((150.0, 150.0), 200.0, 120.0), 25, true);
    for i in 1..=5 {
        p.push((50.0 + 8.0 * f64::from(i), 90.0));
    }
    assert_eq!(kind(&jitter(&p, 1.0, 7)), Some(ShapeKind::Rectangle));
}

#[test]
fn circles_and_ellipses() {
    let r = recognize_stroke(&jitter(
        &ellipse_pts((200.0, 200.0), 80.0, 80.0, 0.0, 90, 360.0),
        2.0,
        8,
    ))
    .unwrap();
    assert_eq!(r.kind, ShapeKind::Circle);
    let (c, w, h, _, _) = frame_of_rec(&r);
    assert!(near(c.0, 200.0, 3.0) && near(c.1, 200.0, 3.0));
    assert!(near(w, 160.0, 6.0) && w == h);

    let r = recognize_stroke(&jitter(
        &ellipse_pts((200.0, 200.0), 120.0, 60.0, 0.0, 90, 360.0),
        2.0,
        9,
    ))
    .unwrap();
    assert_eq!(r.kind, ShapeKind::Ellipse);
    let (_, w, h, rot, _) = frame_of_rec(&r);
    assert!(
        near(w, 240.0, 8.0) && near(h, 120.0, 8.0) && rot == 0.0,
        "{w} {h} {rot}"
    );

    // Tall: snapped upright with width < height.
    let r = recognize_stroke(&ellipse_pts((200.0, 200.0), 50.0, 110.0, 3.0, 90, 360.0)).unwrap();
    let (_, w, h, rot, _) = frame_of_rec(&r);
    assert!(
        near(w, 100.0, 4.0) && near(h, 220.0, 4.0) && rot == 0.0,
        "{w} {h} {rot}"
    );

    // Rotated 35°.
    let r = recognize_stroke(&jitter(
        &ellipse_pts((300.0, 300.0), 130.0, 55.0, 35.0, 90, 360.0),
        1.5,
        10,
    ))
    .unwrap();
    assert_eq!(r.kind, ShapeKind::Ellipse);
    let (_, w, _, rot, _) = frame_of_rec(&r);
    assert!(near(rot, 35.0, 3.0) && near(w, 260.0, 10.0), "{rot} {w}");

    // Drawn round one and a bit (overshoot).
    assert_eq!(
        kind(&ellipse_pts((100.0, 100.0), 60.0, 60.0, 0.0, 100, 400.0)),
        Some(ShapeKind::Circle)
    );
}

#[test]
fn triangles() {
    let v = [(200.0, 100.0), (300.0, 260.0), (100.0, 260.0)];
    let r = recognize_stroke(&jitter(&polyline(&v, 30, true), 1.5, 11)).unwrap();
    assert_eq!(r.kind, ShapeKind::Triangle);
    let (c, w, h, rot, adj) = frame_of_rec(&r);
    assert_eq!(rot, 0.0);
    assert_eq!(adj, Some(0.5));
    assert!(
        near(w, 200.0, 6.0) && near(h, 160.0, 6.0) && near(c.1, 180.0, 4.0),
        "{c:?} {w} {h}"
    );

    // Right triangle: apex above the left end of the base.
    let v = [(100.0, 100.0), (300.0, 250.0), (100.0, 250.0)];
    let (_, _, _, rot, adj) = frame_of_rec(&recognize_stroke(&polyline(&v, 30, true)).unwrap());
    assert_eq!((rot, adj), (0.0, Some(0.0)));

    // Upside down: rotated 180°.
    let v = [(100.0, 100.0), (300.0, 100.0), (200.0, 260.0)];
    let (_, _, _, rot, _) = frame_of_rec(&recognize_stroke(&polyline(&v, 30, true)).unwrap());
    assert!(near(wrap(rot, 360.0).abs(), 180.0, 1e-9), "{rot}");

    // Rotated 40°.
    let v = rot_about(
        &[(200.0, 100.0), (300.0, 260.0), (100.0, 260.0)],
        (200.0, 200.0),
        40.0,
    );
    let r = recognize_stroke(&jitter(&polyline(&v, 30, true), 1.0, 12)).unwrap();
    let (_, _, _, rot, _) = frame_of_rec(&r);
    assert!(near(rot, 40.0, 3.0), "{rot}");
}

#[test]
fn diamonds() {
    let v = [
        (200.0, 100.0),
        (300.0, 200.0),
        (200.0, 300.0),
        (100.0, 200.0),
    ];
    let r = recognize_stroke(&jitter(&polyline(&v, 25, true), 1.5, 13)).unwrap();
    assert_eq!(r.kind, ShapeKind::Diamond);
    let (c, w, h, rot, _) = frame_of_rec(&r);
    assert!(
        near(c.0, 200.0, 4.0) && near(w, 200.0, 8.0) && near(h, 200.0, 8.0) && rot == 0.0,
        "{c:?} {w} {h} {rot}"
    );
    // A wide diamond (not a square on its corner).
    let v = [
        (200.0, 150.0),
        (350.0, 200.0),
        (200.0, 250.0),
        (50.0, 200.0),
    ];
    let r = recognize_stroke(&jitter(&polyline(&v, 25, true), 1.0, 14)).unwrap();
    assert_eq!(r.kind, ShapeKind::Diamond);
    let (_, w, h, rot, _) = frame_of_rec(&r);
    assert!(
        near(w, 300.0, 8.0) && near(h, 100.0, 6.0) && rot == 0.0,
        "{w} {h} {rot}"
    );
}

#[test]
fn pentagons_hexagons_and_stars() {
    let pent = regular_poly((200.0, 200.0), 100.0, 5, -90.0);
    let r = recognize_stroke(&jitter(&polyline(&pent, 20, true), 1.5, 15)).unwrap();
    assert_eq!(r.kind, ShapeKind::Pentagon);
    assert_eq!(frame_of_rec(&r).3, 0.0);

    let hex = regular_poly((200.0, 200.0), 100.0, 6, 0.0);
    let r = recognize_stroke(&jitter(&polyline(&hex, 20, true), 1.5, 16)).unwrap();
    assert_eq!(r.kind, ShapeKind::Hexagon);
    let (_, w, h, rot, _) = frame_of_rec(&r);
    // Flat top and bottom: 2r wide, √3·r tall.
    assert!(
        near(w, 200.0, 8.0) && near(h, 173.2, 8.0) && rot == 0.0,
        "{w} {h} {rot}"
    );
    // Rotated 20°.
    let hex = regular_poly((200.0, 200.0), 100.0, 6, 20.0);
    let r = recognize_stroke(&polyline(&hex, 20, true)).unwrap();
    assert_eq!(r.kind, ShapeKind::Hexagon);
    assert!(near(frame_of_rec(&r).3, 20.0, 2.0));

    // Outline star: tips and notches alternate.
    let mut star = Vec::new();
    for i in 0..10 {
        let r = if i % 2 == 0 { 100.0 } else { 40.0 };
        let a = (-90.0 + 36.0 * f64::from(i)).to_radians();
        star.push((200.0 + r * a.cos(), 200.0 + r * a.sin()));
    }
    let r = recognize_stroke(&jitter(&polyline(&star, 12, true), 1.0, 17)).unwrap();
    assert_eq!(r.kind, ShapeKind::Star);
    assert_eq!(frame_of_rec(&r).3, 0.0);

    // Pentagram in one stroke: every second tip.
    let tips = regular_poly((200.0, 200.0), 100.0, 5, -90.0);
    let order: Vec<Pt> = [0, 2, 4, 1, 3].iter().map(|&i| tips[i]).collect();
    let r = recognize_stroke(&jitter(&polyline(&order, 25, true), 1.5, 18)).unwrap();
    assert_eq!(r.kind, ShapeKind::Star);
}

#[test]
fn arrows_from_a_line_and_head_strokes() {
    let shaft = polyline(&[(100.0, 200.0), (300.0, 200.0)], 40, false);
    let h1 = polyline(&[(300.0, 200.0), (275.0, 185.0)], 8, false);
    let h2 = polyline(&[(300.0, 200.0), (275.0, 215.0)], 8, false);
    let units = recognize_strokes(&[jitter(&shaft, 1.0, 19), h1.clone(), h2.clone()]);
    assert_eq!(units.len(), 1, "{units:?}");
    assert_eq!(units[0].strokes, vec![0, 1, 2]);
    assert_eq!(units[0].rec.kind, ShapeKind::Arrow);
    let Fit::Line {
        head_from, head_to, ..
    } = units[0].rec.fit
    else {
        panic!()
    };
    assert!(head_to && !head_from);

    // A "V" head in one stroke at the start of the line.
    let v = polyline(&[(125.0, 185.0), (100.0, 200.0), (125.0, 215.0)], 8, false);
    let units = recognize_strokes(&[v, shaft.clone()]);
    assert_eq!(units.len(), 1);
    let Fit::Line {
        head_from, head_to, ..
    } = units[0].rec.fit
    else {
        panic!()
    };
    assert!(head_from && !head_to);

    // A perpendicular line at the end is a corner, not a head: two lines.
    let corner = polyline(&[(300.0, 200.0), (300.0, 260.0)], 10, false);
    let units = recognize_strokes(&[shaft.clone(), corner]);
    assert_eq!(units.len(), 2);
    assert!(units.iter().all(|u| u.rec.kind == ShapeKind::Line));

    // Several shapes at once; the scribble stays ink.
    let circle = ellipse_pts((500.0, 300.0), 50.0, 50.0, 0.0, 80, 360.0);
    let mut n = Noise(20);
    let scribble: Vec<Pt> = (0..80)
        .map(|i| (600.0 + f64::from(i) * 2.0, 100.0 + 30.0 * n.next()))
        .collect();
    let units = recognize_strokes(&[shaft, h1, h2, circle, scribble]);
    let kinds: Vec<ShapeKind> = units.iter().map(|u| u.rec.kind).collect();
    assert_eq!(kinds, vec![ShapeKind::Arrow, ShapeKind::Circle]);
    assert!(units.iter().all(|u| !u.strokes.contains(&4)));
}

#[test]
fn strokes_that_stay_ink() {
    let mut n = Noise(21);
    // A random scribble.
    let mut p = vec![(200.0, 200.0)];
    for _ in 0..120 {
        let &(x, y) = p.last().unwrap();
        p.push((x + 12.0 * n.next(), y + 12.0 * n.next()));
    }
    assert_eq!(kind(&p), None);
    // An open arc (a "C").
    assert_eq!(
        kind(&ellipse_pts((200.0, 200.0), 80.0, 80.0, 0.0, 60, 240.0)),
        None
    );
    // A zigzag.
    let z: Vec<Pt> = (0..9)
        .map(|i| {
            (
                100.0 + 30.0 * f64::from(i),
                if i % 2 == 0 { 100.0 } else { 160.0 },
            )
        })
        .collect();
    assert_eq!(kind(&polyline(&z, 10, false)), None);
    // A spiral.
    let s: Vec<Pt> = (0..200)
        .map(|i| {
            let t = f64::from(i) * 0.08;
            (300.0 + 5.0 * t * t.cos(), 300.0 + 5.0 * t * t.sin())
        })
        .collect();
    assert_eq!(kind(&s), None);
    // An "L".
    assert_eq!(
        kind(&polyline(
            &[(100.0, 100.0), (100.0, 250.0), (200.0, 250.0)],
            20,
            false
        )),
        None
    );
    // An irregular quadrilateral (a kite with no right angles, unequal sides).
    let q = [
        (100.0, 100.0),
        (260.0, 130.0),
        (230.0, 220.0),
        (80.0, 300.0),
    ];
    assert_eq!(kind(&polyline(&q, 20, true)), None);
    // Too small, a dot, nothing, non-finite input.
    assert_eq!(kind(&[(10.0, 10.0), (13.0, 12.0)]), None);
    assert_eq!(kind(&[(10.0, 10.0)]), None);
    assert_eq!(kind(&[]), None);
    assert_eq!(kind(&[(f64::NAN, 1.0), (f64::INFINITY, 2.0)]), None);
}

#[test]
fn recognition_is_deterministic() {
    let p = jitter(
        &ellipse_pts((200.0, 200.0), 90.0, 60.0, 10.0, 90, 360.0),
        2.0,
        22,
    );
    assert_eq!(recognize_stroke(&p), recognize_stroke(&p));
}

#[test]
fn noisy_variants_across_seeds() {
    // Hand jitter of ±2.5 pt on 150–250 pt shapes, many seeds, a few rotations.
    for seed in 100..130 {
        let rot = f64::from((seed % 5) as u8) * 9.0;
        let c = (300.0, 250.0);
        let rect = rot_about(&polyline(&rect_corners(c, 240.0, 140.0), 25, true), c, rot);
        assert_eq!(
            kind(&jitter(&rect, 2.5, seed)),
            Some(ShapeKind::Rectangle),
            "rect {seed}"
        );
        let circle = ellipse_pts(c, 90.0, 90.0, 0.0, 90, 360.0);
        assert_eq!(
            kind(&jitter(&circle, 2.5, seed)),
            Some(ShapeKind::Circle),
            "circle {seed}"
        );
        let ell = ellipse_pts(c, 130.0, 70.0, rot, 90, 360.0);
        assert_eq!(
            kind(&jitter(&ell, 2.5, seed)),
            Some(ShapeKind::Ellipse),
            "ellipse {seed}"
        );
        let tri = rot_about(&[(300.0, 150.0), (400.0, 330.0), (200.0, 330.0)], c, rot);
        assert_eq!(
            kind(&jitter(&polyline(&tri, 30, true), 2.5, seed)),
            Some(ShapeKind::Triangle),
            "triangle {seed}"
        );
        let hex = regular_poly(c, 110.0, 6, rot);
        assert_eq!(
            kind(&jitter(&polyline(&hex, 20, true), 2.0, seed)),
            Some(ShapeKind::Hexagon),
            "hexagon {seed}"
        );
        let pent = regular_poly(c, 110.0, 5, -90.0 + rot);
        assert_eq!(
            kind(&jitter(&polyline(&pent, 20, true), 2.0, seed)),
            Some(ShapeKind::Pentagon),
            "pentagon {seed}"
        );
        let line = polyline(&[(100.0, 100.0), (400.0, 180.0)], 50, false);
        assert_eq!(
            kind(&jitter(&line, 2.5, seed)),
            Some(ShapeKind::Line),
            "line {seed}"
        );
    }
}
