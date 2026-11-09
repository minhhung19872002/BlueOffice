//! Display-list drawing of conditional-format data bars (ECMA-376 Part 1, §18.3.1.28
//! `dataBar`; [MS-XLSX] x14 `CT_DataBar` border, gradient and axis). Shared by the
//! grid canvas and the PNG / PDF export, which all render the sheet display list.

use crate::cf::BarDraw;
use crate::geometry::Rect;
use bo_render::{Command, GradientStop, Paint, PathSeg, Rgba, Stroke};

/// `c` mixed with white by `t` (0 = `c`, 1 = white).
fn tint(c: Rgba, t: f64) -> Rgba {
    let ch = |v: u8| (f64::from(v) + (255.0 - f64::from(v)) * t).round() as u8;
    Rgba(ch(c.0), ch(c.1), ch(c.2), c.3)
}

fn rect_path(x: f64, y: f64, w: f64, h: f64) -> Vec<PathSeg> {
    let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
    vec![
        PathSeg::M(x, y),
        PathSeg::L(x + w, y),
        PathSeg::L(x + w, y + h),
        PathSeg::L(x, y + h),
        PathSeg::Z,
    ]
}

/// Commands drawing `bar` inside the cell's inner rectangle `inner` (points).
///
/// The gradient runs from the bar colour at its origin (the axis, or the left edge)
/// to a light tint of it at the far end; the axis is a thin dashed vertical line.
#[must_use]
pub fn data_bar(bar: &BarDraw, inner: Rect) -> Vec<Command> {
    let mut out = Vec::new();
    if inner.w <= 0.0 || inner.h <= 0.0 {
        return out;
    }
    let x0 = inner.x + inner.w * bar.start.clamp(0.0, 1.0);
    let x1 = inner.x + inner.w * bar.end.clamp(0.0, 1.0);
    let w = x1 - x0;
    if w > 0.0 {
        let fill = if bar.gradient {
            let (from, to) = if bar.leftward { (x1, x0) } else { (x0, x1) };
            Paint::Linear {
                x1: from as f32,
                y1: inner.y as f32,
                x2: to as f32,
                y2: inner.y as f32,
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        color: bar.color,
                    },
                    GradientStop {
                        offset: 1.0,
                        color: tint(bar.color, 0.85),
                    },
                ],
            }
        } else {
            Paint::Solid { color: bar.color }
        };
        out.push(Command::Path {
            segs: rect_path(x0, inner.y, w, inner.h),
            fill: Some(fill),
            stroke: bar.border.map(|color| Stroke {
                width: 0.75,
                color,
                dash: vec![],
                cap: "butt".into(),
                join: "miter".into(),
            }),
        });
    }
    if let Some(a) = bar.axis {
        let ax = inner.x + inner.w * a;
        out.push(Command::Line {
            x1: ax as f32,
            y1: inner.y as f32,
            x2: ax as f32,
            y2: (inner.y + inner.h) as f32,
            width: 0.75,
            color: bar.axis_color,
            dash: Some([1.5, 1.5]),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(start: f64, end: f64, axis: Option<f64>, leftward: bool) -> BarDraw {
        BarDraw {
            start,
            end,
            color: Rgba(0, 0, 255, 255),
            border: Some(Rgba(0, 0, 0, 255)),
            gradient: true,
            leftward,
            axis,
            axis_color: Rgba(0, 0, 0, 255),
        }
    }

    #[test]
    fn bar_and_axis_positions() {
        let inner = Rect {
            x: 10.0,
            y: 0.0,
            w: 100.0,
            h: 10.0,
        };
        // Negative bar from 25 % to the axis at 50 %: x 35..60, gradient right → left.
        let cmds = data_bar(&bar(0.25, 0.5, Some(0.5), true), inner);
        let Command::Path {
            segs,
            fill: Some(Paint::Linear { x1, x2, .. }),
            stroke: Some(_),
        } = &cmds[0]
        else {
            panic!("bar path");
        };
        assert_eq!(segs[0], PathSeg::M(35.0, 0.0));
        assert_eq!(segs[1], PathSeg::L(60.0, 0.0));
        assert_eq!((*x1, *x2), (60.0, 35.0));
        assert!(matches!(cmds[1], Command::Line { x1, .. } if (x1 - 60.0).abs() < 1e-6));
        // A zero-length bar still draws its axis.
        assert_eq!(data_bar(&bar(0.5, 0.5, Some(0.5), false), inner).len(), 1);
    }
}
