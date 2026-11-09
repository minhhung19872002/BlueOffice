//! Hand-computed layout cases. The fake font gives every character a width of
//! 6 pt except `)` (3 pt), `-` (4 pt) and space (2 pt).

use super::*;
use crate::numfmt::{format_number, FormatOptions, Locale};

fn measure(s: &str) -> f64 {
    s.chars()
        .map(|c| match c {
            ')' => 3.0,
            '-' => 4.0,
            ' ' => 2.0,
            _ => 6.0,
        })
        .sum()
}

fn en() -> FormatOptions {
    FormatOptions {
        locale: Locale::EN_US,
        date1904: false,
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn skip_reserves_the_width_of_the_next_character() {
    // `0_)`: "5" followed by a gap as wide as ")".
    let f = format_number(5.0, "0_)", &en());
    assert_eq!(f.text, "5 ");
    assert_eq!(f.skips, vec![(1, ')')]);
    let l = layout_line(&f, 100.0, &mut measure);
    assert_eq!(
        l.segments,
        vec![Segment {
            text: "5".into(),
            x: 0.0
        }]
    );
    // 6 (digit) + 3 (")" gap) — not the 2 pt of a space.
    assert!(close(l.width, 9.0), "{}", l.width);

    // A minus sign shifts the marker.
    let f = format_number(-5.0, "0_)", &en());
    assert_eq!(f.text, "-5 ");
    assert_eq!(f.skips, vec![(2, ')')]);
}

#[test]
fn fill_repeats_to_the_cell_width() {
    let f = Formatted {
        text: "$12".into(),
        fill: Some((1, '-')),
        ..Formatted::default()
    };
    // Base 6 + 12 = 18; 40 − 18 = 22 → ⌊22 / 4⌋ = 5 dashes (20 pt).
    let l = layout_line(&f, 40.0, &mut measure);
    let want = vec![
        Segment {
            text: "$".into(),
            x: 0.0,
        },
        Segment {
            text: "-----".into(),
            x: 6.0,
        },
        Segment {
            text: "12".into(),
            x: 26.0,
        },
    ];
    assert_eq!(l.segments, want);
    assert!(close(l.width, 38.0));
    assert_eq!(l.text, "$-----12");
    // Too narrow: no fill at all.
    let l = layout_line(&f, 10.0, &mut measure);
    assert_eq!(l.text, "$12");
    assert!(close(l.width, 18.0));
}

#[test]
fn accounting_style_gaps_and_fill_together() {
    // `_(* #,##0_)`: gap "(" · fill with spaces · number · gap ")".
    let f = format_number(1234.0, "_(* #,##0_)", &en());
    assert_eq!(f.text, " 1,234 ");
    assert_eq!(f.skips, vec![(0, '('), (6, ')')]);
    assert_eq!(f.fill, Some((1, ' ')));
    // Base: 6 ("(") + 30 ("1,234") + 3 (")") = 39; 60 − 39 = 21 → 10 spaces (20 pt).
    let l = layout_line(&f, 60.0, &mut measure);
    assert_eq!(l.segments.len(), 2);
    assert_eq!(l.segments[0].text, " ".repeat(10));
    assert!(close(l.segments[0].x, 6.0));
    assert_eq!(l.segments[1].text, "1,234");
    assert!(close(l.segments[1].x, 26.0));
    assert!(close(l.width, 59.0));
}

#[test]
fn shrink_to_fit_scale() {
    assert!(close(shrink_scale(120.0, 60.0), 0.5));
    assert!(close(shrink_scale(50.0, 60.0), 1.0));
    assert!(close(shrink_scale(60.0, 60.0), 1.0));
    // 50 / 1000 = 0.05 is clamped to the legibility minimum.
    assert!(close(shrink_scale(1000.0, 50.0), MIN_SHRINK));
}

#[test]
fn rotation_values() {
    assert_eq!(rotation_degrees(0), None);
    assert_eq!(rotation_degrees(45), Some(45.0));
    assert_eq!(rotation_degrees(90), Some(90.0));
    assert_eq!(rotation_degrees(91), Some(-1.0));
    assert_eq!(rotation_degrees(135), Some(-45.0));
    assert_eq!(rotation_degrees(180), Some(-90.0));
    assert_eq!(rotation_degrees(255), None);
    let (w, h) = rotated_extent(60.0, 10.0, 90.0);
    assert!(close(w, 10.0) && close(h, 60.0));
}

#[test]
fn rotated_45_degrees_in_a_cell() {
    // Line 60 pt wide, ascent 8, descent 2 (height 10), rotated 45° in a
    // 100 × 50 cell at the origin, left / bottom aligned, padding 1.5.
    let rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 50.0,
    };
    let g = RotatedLine::new(
        rect,
        60.0,
        8.0,
        2.0,
        45.0,
        Place::Start,
        Place::End,
        1.5,
        0.0,
    );
    let s = std::f64::consts::FRAC_1_SQRT_2;
    // Box: (60 + 10) · √½ = 49.4975 both ways.
    assert!(close(g.bw, 70.0 * s) && close(g.bh, 70.0 * s));
    // Centre: x = 1.5 + 24.7487, y = 50 − 0.75 − 24.7487.
    assert!(close(g.cx, 1.5 + 35.0 * s), "{}", g.cx);
    assert!(close(g.cy, 49.25 - 35.0 * s), "{}", g.cy);
    // Baseline start in the rotated frame: (−30, 8 − 5 = 3).
    assert!(close(g.local_x, -30.0) && close(g.local_y, 3.0));
    // On the page: x = cx + √½·(−30) + √½·3 = 7.1569, y = cy + √½·30 + √½·3 = 47.8358.
    let (x, y) = g.baseline_start();
    assert!(close(x, 1.5 + 35.0 * s - 27.0 * s), "{x}");
    assert!(close(y, 49.25 - 35.0 * s + 33.0 * s), "{y}");
    assert!((x - 7.1569).abs() < 1e-3 && (y - 47.8358).abs() < 1e-3);
    // The end of the baseline lies up and to the right (counter-clockwise).
    let (x2, y2) = g.to_page(30.0, 3.0);
    assert!(close(x2 - x, 60.0 * s) && close(y - y2, 60.0 * s));
    let m = g.matrix();
    assert!(close(m[0], s) && close(m[1], -s) && close(m[2], s) && close(m[3], s));
}
