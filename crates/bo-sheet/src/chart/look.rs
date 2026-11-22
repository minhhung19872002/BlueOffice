//! Chart formatting resolved for drawing (CORE-CHART-007): the colours, widths and switches of
//! a [`ChartFormat`] (series treatment, chart and plot area, gridlines, text, data labels) as
//! the renderer uses them, and the painting of filled series shapes.

use super::{fmt_num, Align, ColorFn, LabelFn, AXIS, GRID};
use crate::numfmt::Locale;
use bo_model::chart_style::{ChartFormat, DataLabels, SeriesFill};
use bo_render::{Command, GradientStop, Paint, PathSeg, Rgba, Stroke};

/// EMU per point.
const EMU_PT: f64 = 12_700.0;

/// The look of one chart.
pub(crate) struct Look {
    pub fill: SeriesFill,
    pub outline: Option<Rgba>,
    pub outline_w: f64,
    pub line_w: f64,
    pub markers: bool,
    pub shadow: bool,
    /// Gap between bar groups as a share of a bar (§21.2.2.75 gapWidth / 100).
    pub gap: f64,
    pub chart_fill: Option<Rgba>,
    pub chart_line: Option<Rgba>,
    pub plot_fill: Option<Rgba>,
    pub grid: Option<Rgba>,
    pub grid_w: Option<f64>,
    pub axis: Option<Rgba>,
    pub text: Option<Rgba>,
    /// Base font size in points.
    pub font: Option<f64>,
    /// Title size in points (the title's own runs).
    pub title_font: Option<f64>,
    pub val_grid: bool,
    pub cat_grid: bool,
    pub labels: Option<DataLabels>,
    /// §21.2.2.156 roundedCorners of the chart area.
    pub rounded: bool,
    /// Axis line width in points (`None` = the renderer's).
    pub axis_w: Option<f64>,
    /// The reference's automatic formatting and margins ([`Look::use_classic`]).
    pub classic: bool,
}

impl Look {
    pub(crate) fn new(f: &ChartFormat, color: &ColorFn<'_>) -> Look {
        let c = |x: &Option<bo_model::sheet::ChartColor>| x.as_ref().map(|x| color(Some(x), 0));
        Look {
            fill: f.series_fill,
            outline: c(&f.outline),
            outline_w: if f.outline_w == 0 {
                0.75
            } else {
                f64::from(f.outline_w) / EMU_PT
            },
            line_w: f
                .line_w
                .map_or(2.0, |w| (f64::from(w) / EMU_PT).clamp(0.25, 12.0)),
            markers: f.markers,
            shadow: f.shadow,
            gap: f64::from(f.gap_width.unwrap_or(150).min(500)) / 100.0,
            chart_fill: c(&f.chart_fill),
            chart_line: c(&f.chart_line),
            plot_fill: c(&f.plot_fill),
            grid: c(&f.grid_color),
            grid_w: f.grid_w.map(|w| f64::from(w) / EMU_PT),
            axis: c(&f.axis_line),
            text: c(&f.text_color),
            font: f.font_size.map(|s| (f64::from(s) / 100.0).clamp(5.0, 28.0)),
            title_font: f
                .title_size
                .map(|s| (f64::from(s) / 100.0).clamp(5.0, 48.0)),
            val_grid: !f.hide_val_grid,
            cat_grid: f.cat_grid,
            labels: f.data_labels.clone().filter(DataLabels::any),
            rounded: f.rounded,
            axis_w: None,
            classic: false,
        }
    }

    /// A chart part without formatting of its own (`ChartFormat::classic`) as the reference word
    /// processor draws it (black-box, PDF export of office-22: text and lines `0 G`, gridlines and
    /// axis lines `0.5 w`, chart border `0.537 G` 0.5 pt), with its margins
    /// ([`super::classic_legend`], `draw_xy_plain`).
    pub(crate) fn use_classic(&mut self, f: &ChartFormat) {
        const BLACK: Rgba = Rgba(0, 0, 0, 255);
        self.classic = true;
        // Lines 2.25 pt (28 575 EMU, `2.25 w` in the reference's export).
        if f.line_w.is_none() {
            self.line_w = 2.25;
        }
        self.text.get_or_insert(BLACK);
        self.grid.get_or_insert(BLACK);
        self.grid_w.get_or_insert(0.5);
        self.axis.get_or_insert(BLACK);
        self.axis_w = Some(0.5);
        self.chart_line.get_or_insert(Rgba(0x89, 0x89, 0x89, 255));
    }

    /// The chart area outline (`w × h` at the origin, inset by half the line): square corners, or
    /// with §21.2.2.156 roundedCorners quarter circles of a radius of 6 % of the shorter side (at
    /// most 10 pt) — BlueOffice's rendering of the flag, which the schema leaves to the application.
    pub(crate) fn area_path(&self, w: f64, h: f64) -> Vec<PathSeg> {
        let (x0, y0, x1, y1) = (0.25, 0.25, w - 0.25, h - 0.25);
        let f = |v: f64| v as f32;
        if !self.rounded {
            return vec![
                PathSeg::M(f(x0), f(y0)),
                PathSeg::L(f(x1), f(y0)),
                PathSeg::L(f(x1), f(y1)),
                PathSeg::L(f(x0), f(y1)),
                PathSeg::Z,
            ];
        }
        let r = (w.min(h) * 0.06).clamp(0.0, 10.0);
        // Cubic Bézier quarter circle: control points at 0.5523 r from the corner's ends.
        let k = r * (1.0 - 0.552_284_75);
        vec![
            PathSeg::M(f(x0 + r), f(y0)),
            PathSeg::L(f(x1 - r), f(y0)),
            PathSeg::C(f(x1 - k), f(y0), f(x1), f(y0 + k), f(x1), f(y0 + r)),
            PathSeg::L(f(x1), f(y1 - r)),
            PathSeg::C(f(x1), f(y1 - k), f(x1 - k), f(y1), f(x1 - r), f(y1)),
            PathSeg::L(f(x0 + r), f(y1)),
            PathSeg::C(f(x0 + k), f(y1), f(x0), f(y1 - k), f(x0), f(y1 - r)),
            PathSeg::L(f(x0), f(y0 + r)),
            PathSeg::C(f(x0), f(y0 + k), f(x0 + k), f(y0), f(x0 + r), f(y0)),
            PathSeg::Z,
        ]
    }

    /// Gridlines and axis lines drawn by the plot renderers in their default colours take the
    /// format's colours.
    pub(crate) fn recolor(&self, cmds: &mut [Command]) {
        let map = |c: &mut Rgba, w: &mut f32| {
            if *c == GRID {
                if let Some(g) = self.grid {
                    *c = g;
                }
                if let Some(gw) = self.grid_w {
                    *w = gw as f32;
                }
            } else if *c == super::elements::GRID_MINOR {
                // Minor gridlines: a lighter, thinner version of the major ones.
                if let Some(g) = self.grid {
                    *c = tint(g, 0.5);
                }
                if let Some(gw) = self.grid_w {
                    *w = (gw * 0.6) as f32;
                }
            } else if *c == AXIS {
                if let Some(a) = self.axis {
                    *c = a;
                }
                if let Some(aw) = self.axis_w {
                    *w = aw as f32;
                }
            }
        };
        for cmd in cmds {
            match cmd {
                Command::Line { color, width, .. } => map(color, width),
                Command::Path {
                    stroke: Some(s), ..
                } => map(&mut s.color, &mut s.width),
                _ => {}
            }
        }
    }

    /// Paints a filled series shape (`segs`, bounding box `bbox` = l, t, r, b) in colour `c`:
    /// shadow, fill treatment and outline.
    pub(crate) fn shape(
        &self,
        segs: Vec<PathSeg>,
        bbox: [f64; 4],
        c: Rgba,
        out: &mut Vec<Command>,
    ) {
        if self.shadow {
            let off = |s: &PathSeg| match *s {
                PathSeg::M(x, y) => PathSeg::M(x + 1.0, y + 1.5),
                PathSeg::L(x, y) => PathSeg::L(x + 1.0, y + 1.5),
                PathSeg::Q(a, b, x, y) => PathSeg::Q(a + 1.0, b + 1.5, x + 1.0, y + 1.5),
                PathSeg::C(a, b, c2, d, x, y) => {
                    PathSeg::C(a + 1.0, b + 1.5, c2 + 1.0, d + 1.5, x + 1.0, y + 1.5)
                }
                PathSeg::Z => PathSeg::Z,
            };
            out.push(Command::Path {
                segs: segs.iter().map(off).collect(),
                fill: Some(Paint::Solid {
                    color: Rgba(0, 0, 0, 70),
                }),
                stroke: None,
            });
        }
        let Rgba(r, g, b, _) = c;
        let fill = match self.fill {
            SeriesFill::Solid => Some(Paint::Solid { color: c }),
            SeriesFill::Soft => Some(Paint::Solid {
                color: Rgba(r, g, b, 166),
            }),
            // A lighter tint at the top (lumMod 60 % + lumOff 40 %) down to the colour.
            SeriesFill::Gradient => Some(Paint::Linear {
                x1: bbox[0] as f32,
                y1: bbox[1] as f32,
                x2: bbox[0] as f32,
                y2: bbox[3] as f32,
                stops: vec![
                    GradientStop {
                        offset: 0.0,
                        color: tint(c, 0.4),
                    },
                    GradientStop {
                        offset: 1.0,
                        color: c,
                    },
                ],
            }),
            SeriesFill::Outline => None,
        };
        let stroke = match (self.fill, self.outline) {
            (SeriesFill::Outline, _) => Some(stroke(c, self.outline_w.max(1.5))),
            (_, Some(o)) => Some(stroke(o, self.outline_w)),
            _ => None,
        };
        out.push(Command::Path { segs, fill, stroke });
    }

    /// A filled rectangle series shape (bars, columns).
    pub(crate) fn bar(&self, [x, y, w, h]: [f64; 4], c: Rgba, out: &mut Vec<Command>) {
        if self.fill == SeriesFill::Solid && self.outline.is_none() && !self.shadow {
            out.push(super::rect(x, y, w, h, c));
            return;
        }
        let (w, h) = (w.max(0.0), h.max(0.0));
        let segs = vec![
            PathSeg::M(x as f32, y as f32),
            PathSeg::L((x + w) as f32, y as f32),
            PathSeg::L((x + w) as f32, (y + h) as f32),
            PathSeg::L(x as f32, (y + h) as f32),
            PathSeg::Z,
        ];
        self.shape(segs, [x, y, x + w, y + h], c, out);
    }

    /// Line markers (§21.2.2.106): small circles at the points.
    pub(crate) fn markers(&self, pts: &[(f64, f64)], c: Rgba, out: &mut Vec<Command>) {
        if !self.markers {
            return;
        }
        let r = (self.line_w * 1.6).clamp(2.0, 5.0);
        for (x, y) in pts {
            out.push(Command::Path {
                segs: super::circle(*x, *y, r),
                fill: Some(Paint::Solid { color: c }),
                stroke: Some(stroke(Rgba(255, 255, 255, 255), 0.75)),
            });
        }
    }

    /// Text of a data label (§21.2.2.49): series name, category, value and percentage, joined by
    /// `"; "`.
    pub(crate) fn label_text(
        &self,
        series: &str,
        cat: &str,
        v: f64,
        percent: Option<f64>,
        locale: &Locale,
    ) -> Option<String> {
        let d = self.labels.as_ref()?;
        let mut parts = Vec::new();
        if d.series {
            parts.push(series.to_owned());
        }
        if d.category {
            parts.push(cat.to_owned());
        }
        if d.value {
            parts.push(fmt_num(v, locale));
        }
        if let Some(p) = percent.filter(|_| d.percent) {
            parts.push(format!("{}%", (p * 100.0).round()));
        }
        (!parts.is_empty()).then(|| parts.join("; "))
    }

    /// Draws a data label centred on `(x, y)`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn put_label(
        &self,
        text: &str,
        x: f64,
        y: f64,
        font: f64,
        ink: Rgba,
        label: &mut LabelFn<'_>,
        out: &mut Vec<Command>,
    ) {
        let w = font * (text.chars().count() as f64 * 0.6 + 1.0);
        // The automatic formatting's data labels: the chart's text size and colour, also inside
        // bars and slices (chart-m variant of office-22: 10 pt black "55%" in a blue slice).
        let (size, ink) = if self.classic {
            (font, super::TEXT)
        } else {
            (font * 0.9, ink)
        };
        label(
            text,
            [x - w / 2.0, y - font * 0.7, x + w / 2.0, y + font * 0.7],
            Align::Center,
            size,
            ink,
            out,
        );
    }
}

fn stroke(c: Rgba, w: f64) -> Stroke {
    Stroke {
        width: w as f32,
        color: c,
        dash: vec![],
        cap: "butt".into(),
        join: "miter".into(),
    }
}

/// `c` lightened towards white by `k` (0..=1).
pub(crate) fn tint(Rgba(r, g, b, a): Rgba, k: f64) -> Rgba {
    let t = |v: u8| {
        (f64::from(v) + (255.0 - f64::from(v)) * k)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Rgba(t(r), t(g), t(b), a)
}
