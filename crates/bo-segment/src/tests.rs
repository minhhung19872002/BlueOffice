use super::*;

const RED: [u8; 3] = [210, 45, 40];

/// Background: horizontal gradient blue → green.
fn bg_at(x: u32, w: u32) -> [u8; 3] {
    let t = f64::from(x) / f64::from(w.max(2) - 1);
    let mix = |a: f64, b: f64| (a + (b - a) * t).round() as u8;
    [mix(25.0, 40.0), mix(70.0, 190.0), mix(200.0, 70.0)]
}

/// A picture and its ground-truth foreground, drawn by `fg(x, y)`.
fn scene(w: u32, h: u32, fg: impl Fn(u32, u32) -> Option<[u8; 3]>) -> (OwnedImage, Vec<bool>) {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    let mut truth = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            // A little deterministic texture so the colours are not perfectly flat.
            let n = ((x * 7 + y * 13) % 5) as u8;
            let (c, t) = match fg(x, y) {
                Some(c) => (c, true),
                None => (bg_at(x, w), false),
            };
            rgba.extend_from_slice(&[c[0].saturating_add(n), c[1].saturating_add(n), c[2], 255]);
            truth.push(t);
        }
    }
    (
        OwnedImage {
            width: w,
            height: h,
            rgba,
        },
        truth,
    )
}

fn in_disk(x: u32, y: u32, cx: f64, cy: f64, r: f64) -> bool {
    (f64::from(x) + 0.5 - cx).powi(2) + (f64::from(y) + 0.5 - cy).powi(2) <= r * r
}

fn iou(mask: &Mask, truth: &[bool]) -> f64 {
    let (mut inter, mut union) = (0usize, 0usize);
    for (m, t) in mask.alpha.iter().zip(truth) {
        let k = *m >= 128;
        inter += usize::from(k && *t);
        union += usize::from(k || *t);
    }
    inter as f64 / union.max(1) as f64
}

#[test]
fn disk_on_a_gradient() {
    let (w, h) = (240u32, 180u32);
    let (img, truth) = scene(w, h, |x, y| in_disk(x, y, 120.0, 90.0, 60.0).then_some(RED));
    let (mask, seg) = remove_background(&img.view(), &Request::default(), &GraphCut);
    assert_eq!((mask.width, mask.height), (w, h));
    assert!(seg.iterations >= 1 && seg.iterations <= MAX_ITERATIONS + FINE_ITERATIONS);
    let score = iou(&mask, &truth);
    assert!(score >= 0.95, "IoU {score}");
    // Corners removed, centre kept.
    assert_eq!(mask.alpha[0], 0);
    assert_eq!(mask.alpha[(w * h - 1) as usize], 0);
    assert_eq!(mask.alpha[(90 * w + 120) as usize], 255);
}

#[test]
fn large_picture_is_segmented_on_a_reduced_copy() {
    let (w, h) = (1200u32, 800u32);
    let (img, truth) = scene(w, h, |x, y| {
        in_disk(x, y, 600.0, 400.0, 300.0).then_some(RED)
    });
    let (mask, seg) = remove_background(&img.view(), &Request::default(), &GraphCut);
    assert_eq!((seg.mask.width, seg.mask.height), (512, 341));
    assert_eq!((mask.width, mask.height), (w, h));
    let score = iou(&mask, &truth);
    assert!(score >= 0.95, "IoU {score}");
    // The edge is a short ramp: along the middle row, alpha goes 0 → 255 within a few pixels.
    let row = &mask.alpha[(400 * w) as usize..(401 * w) as usize];
    let first_partial = row.iter().position(|a| *a > 0).unwrap();
    let first_full = row.iter().position(|a| *a == 255).unwrap();
    assert!(
        first_full - first_partial <= 3,
        "{first_partial}..{first_full}"
    );
    // The edge is where the disk starts (x = 300), within two working pixels.
    assert!((first_full as i64 - 300).abs() <= 5, "{first_full}");
}

#[test]
fn remove_stroke_fixes_an_ambiguous_blob() {
    let (w, h) = (240u32, 180u32);
    // A red blob in the background, inside the default rectangle: same colour as the disk.
    let blob = |x: u32, y: u32| (24..54).contains(&x) && (20..50).contains(&y);
    let (img, _) = scene(w, h, |x, y| {
        (in_disk(x, y, 140.0, 95.0, 55.0) || blob(x, y)).then_some(RED)
    });
    let truth: Vec<bool> = (0..h)
        .flat_map(|y| (0..w).map(move |x| in_disk(x, y, 140.0, 95.0, 55.0)))
        .collect();
    let (plain, _) = remove_background(&img.view(), &Request::default(), &GraphCut);
    // Without help the blob looks like foreground.
    assert_eq!(plain.alpha[(35 * w + 39) as usize], 255);
    let before = iou(&plain, &truth);
    let req = Request {
        rect: None,
        marks: vec![Mark {
            kind: MarkKind::Remove,
            points: vec![[30.0 / 240.0, 35.0 / 180.0], [48.0 / 240.0, 35.0 / 180.0]],
        }],
    };
    let (fixed, _) = remove_background(&img.view(), &req, &GraphCut);
    let after = iou(&fixed, &truth);
    assert!(after >= 0.95 && after > before, "{before} → {after}");
    assert_eq!(fixed.alpha[(35 * w + 39) as usize], 0);
    assert_eq!(fixed.alpha[(22 * w + 26) as usize], 0);
}

#[test]
fn keep_stroke_restores_a_background_coloured_patch() {
    let (w, h) = (240u32, 180u32);
    // A patch inside the disk with the colours of the background around it.
    let patch = |x: u32, y: u32| (110..150).contains(&x) && (80..100).contains(&y);
    let (img, truth) = scene(w, h, |x, y| {
        if patch(x, y) {
            Some(bg_at(x, w))
        } else {
            in_disk(x, y, 130.0, 90.0, 62.0).then_some(RED)
        }
    });
    // Ground truth: the patch belongs to the subject.
    let truth: Vec<bool> = truth
        .iter()
        .enumerate()
        .map(|(i, t)| *t || in_disk(i as u32 % w, i as u32 / w, 130.0, 90.0, 62.0))
        .collect();
    let (plain, _) = remove_background(&img.view(), &Request::default(), &GraphCut);
    assert_eq!(
        plain.alpha[(90 * w + 130) as usize],
        0,
        "patch taken for background"
    );
    let req = Request {
        rect: None,
        marks: vec![Mark {
            kind: MarkKind::Keep,
            points: vec![[114.0 / 240.0, 90.0 / 180.0], [146.0 / 240.0, 90.0 / 180.0]],
        }],
    };
    let (fixed, _) = remove_background(&img.view(), &req, &GraphCut);
    let (before, after) = (iou(&plain, &truth), iou(&fixed, &truth));
    assert!(after >= 0.95 && after > before, "{before} → {after}");
    assert_eq!(fixed.alpha[(84 * w + 112) as usize], 255);
}

#[test]
fn user_rectangle_limits_the_foreground() {
    let (w, h) = (240u32, 180u32);
    // Two disks; the rectangle holds only the left one.
    let (img, _) = scene(w, h, |x, y| {
        (in_disk(x, y, 60.0, 90.0, 40.0) || in_disk(x, y, 180.0, 90.0, 40.0)).then_some(RED)
    });
    let req = Request {
        rect: Some([0.05, 0.1, 0.48, 0.9]),
        marks: vec![],
    };
    let (mask, _) = remove_background(&img.view(), &req, &GraphCut);
    assert_eq!(mask.alpha[(90 * w + 60) as usize], 255);
    assert_eq!(mask.alpha[(90 * w + 180) as usize], 0);
}

#[test]
fn same_input_same_mask() {
    let (img, _) = scene(300, 200, |x, y| {
        in_disk(x, y, 150.0, 100.0, 70.0).then_some(RED)
    });
    let req = Request {
        rect: Some([0.1, 0.1, 0.9, 0.9]),
        marks: vec![Mark {
            kind: MarkKind::Keep,
            points: vec![[0.5, 0.5]],
        }],
    };
    let a = remove_background(&img.view(), &req, &GraphCut);
    let b = remove_background(&img.view(), &req, &GraphCut);
    assert_eq!(a, b);
}

#[test]
fn prior_mask_replaces_the_rectangle() {
    let (w, h) = (120u32, 90u32);
    let (img, truth) = scene(w, h, |x, y| in_disk(x, y, 60.0, 45.0, 30.0).then_some(RED));
    // A coarse prior (a box around the disk) at another size, over the whole picture.
    let prior = Mask {
        width: 12,
        height: 9,
        alpha: (0..9)
            .flat_map(|y| {
                (0..12).map(move |x| {
                    if (2..10).contains(&x) && (1..8).contains(&y) {
                        255
                    } else {
                        0
                    }
                })
            })
            .collect(),
    };
    let req = Request {
        rect: Some([0.0, 0.0, 1.0, 1.0]),
        marks: vec![],
    };
    let p = PriorMask(prior);
    assert_eq!(p.id(), "prior-mask");
    let (mask, _) = remove_background(&img.view(), &req, &p);
    assert!(iou(&mask, &truth) >= 0.95);
    // The rectangle alone (the whole picture) gives no background samples: all kept.
    let (all, seg) = remove_background(&img.view(), &req, &GraphCut);
    assert_eq!(seg.iterations, 0);
    assert!(all.alpha.iter().all(|a| *a == 255));
}

#[test]
fn apply_and_tint() {
    let (img, _) = scene(64, 48, |x, y| {
        in_disk(x, y, 32.0, 24.0, 16.0).then_some(RED)
    });
    let (mask, seg) = remove_background(&img.view(), &Request::default(), &GraphCut);
    let mut rgba = img.rgba.clone();
    apply_mask(&mut rgba, &mask);
    assert_eq!(rgba[3], 0);
    assert_eq!(rgba[((24 * 64 + 32) * 4 + 3) as usize], 255);
    let t = tinted(&seg);
    // Corner tinted towards TINT, centre unchanged.
    let c = &t.rgba[0..4];
    let want: Vec<u8> = (0..3)
        .map(|i| (f64::from(img.rgba[i]) * 0.35 + f64::from(TINT[i]) * 0.65).round() as u8)
        .collect();
    assert_eq!(&c[..3], &want[..]);
    let o = ((24 * 64 + 32) * 4) as usize;
    assert_eq!(&t.rgba[o..o + 3], &img.rgba[o..o + 3]);
}

#[test]
fn work_size_and_downscale() {
    assert_eq!(work_size(100, 50), (100, 50));
    assert_eq!(work_size(2048, 1024), (512, 256));
    assert_eq!(work_size(300, 900), (171, 512));
    // 4×2 → 2×1: each output pixel averages a 2×2 block.
    let rgba: Vec<u8> = [0u8, 10, 20, 30, 40, 50, 60, 70]
        .iter()
        .flat_map(|v| [*v, *v, *v, 255])
        .collect();
    let d = downscale(
        &Image {
            width: 4,
            height: 2,
            rgba: &rgba,
        },
        2,
        1,
    );
    // (0+10+40+50)/4 = 25, (20+30+60+70)/4 = 45.
    assert_eq!(d.rgba, vec![25, 25, 25, 255, 45, 45, 45, 255]);
}

/// Time of a 512 × 512 segmentation (reported; the budget is checked in release
/// builds only, where the WASM build's speed is comparable).
#[test]
fn timing_512() {
    let (img, truth) = scene(512, 512, |x, y| {
        in_disk(x, y, 256.0, 256.0, 170.0).then_some(RED)
    });
    let t = std::time::Instant::now();
    let (mask, seg) = remove_background(&img.view(), &Request::default(), &GraphCut);
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!("segment 512×512: {ms:.0} ms, {} iterations", seg.iterations);
    assert!(iou(&mask, &truth) >= 0.95);
    if !cfg!(debug_assertions) {
        assert!(ms < 1500.0, "{ms} ms");
    }
}
