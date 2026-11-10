//! Region maps (CORE-CHART-006, chartex `regionMap`): each location of the series is matched to a
//! region of a boundary set ([`crate::geo`]) and filled from the colour scale (numbers) or with a
//! colour per category (texts); the view fits the data's extent in the chosen projection.

use super::{fmt_num, rect, Align, ColorFn, LabelFn, Ser, TEXT};
use crate::cells::resolve_ref;
use crate::geo::{self, projection::Projection, LonLat, MapLevel};
use crate::numfmt::Locale;
use crate::render::format_cell;
use bo_model::sheet::{Chart, ChartSeries, MapColorScale, MapOptions, Workbook};
use bo_render::{Command, Paint, PathSeg, Rgba, Stroke};

/// Fill of regions without data.
const NO_DATA: Rgba = Rgba(0xE4, 0xE7, 0xEB, 255);
/// Region borders.
const BORDER: Rgba = Rgba(255, 255, 255, 255);
/// Colour of the sea / plot background.
const SEA: Rgba = Rgba(0xF4, 0xF8, 0xFB, 255);

/// The texts of a categorical map's values (live from the range, else the cache).
pub(super) fn value_texts(
    wb: &Workbook,
    sheet: usize,
    s: &ChartSeries,
    m: &MapOptions,
    locale: &Locale,
) -> Vec<String> {
    s.val_ref
        .as_deref()
        .and_then(|f| resolve_ref(wb, sheet, f))
        .map_or_else(
            || m.text_values.clone(),
            |(si, r)| {
                let ws = &wb.sheets[si];
                let mut out = Vec::new();
                for row in r.first.row..=r.last.row {
                    for col in r.first.col..=r.last.col {
                        out.push(
                            ws.cell(row, col)
                                .map(|c| format_cell(wb, c, locale).text)
                                .unwrap_or_default(),
                        );
                        if out.len() >= super::MAX_POINTS {
                            return out;
                        }
                    }
                }
                out
            },
        )
}

/// Sutherland–Hodgman clipping of a closed ring to `[x0, y0, x1, y1]` (degrees).
fn clip_ring(ring: &[LonLat], [x0, y0, x1, y1]: [f64; 4]) -> Vec<LonLat> {
    let mut pts: Vec<LonLat> = ring.to_vec();
    let edges: [(u8, f64); 4] = [(0, x0), (1, x1), (2, y0), (3, y1)];
    for (side, v) in edges {
        if pts.is_empty() {
            break;
        }
        let inside = |p: LonLat| match side {
            0 => p.0 >= v,
            1 => p.0 <= v,
            2 => p.1 >= v,
            _ => p.1 <= v,
        };
        let cut = |a: LonLat, b: LonLat| -> LonLat {
            if side < 2 {
                let t = (v - a.0) / (b.0 - a.0);
                (v, a.1 + (b.1 - a.1) * t)
            } else {
                let t = (v - a.1) / (b.1 - a.1);
                (a.0 + (b.0 - a.0) * t, v)
            }
        };
        let input = std::mem::take(&mut pts);
        let n = input.len();
        for i in 0..n {
            let cur = input[i];
            let prev = input[(i + n - 1) % n];
            match (inside(prev), inside(cur)) {
                (true, true) => pts.push(cur),
                (true, false) => pts.push(cut(prev, cur)),
                (false, true) => {
                    pts.push(cut(prev, cur));
                    pts.push(cur);
                }
                (false, false) => {}
            }
        }
    }
    pts
}

fn overlaps(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

fn lerp(a: Rgba, b: Rgba, t: f64) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
    Rgba(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2), 255)
}

/// A colour scale resolved for the series' values.
struct Scale {
    lo: f64,
    mid: Option<f64>,
    hi: f64,
    c_lo: Rgba,
    c_mid: Option<Rgba>,
    c_hi: Rgba,
}

impl Scale {
    fn new(sc: &MapColorScale, vals: &[f64], color: &ColorFn<'_>) -> Scale {
        let (dmin, dmid, dmax) = MapColorScale::default_colors(sc.diverging);
        let lo = sc
            .min_number
            .unwrap_or_else(|| vals.iter().copied().fold(f64::INFINITY, f64::min));
        let hi = sc
            .max_number
            .unwrap_or_else(|| vals.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        let (lo, hi) = if lo.is_finite() && hi.is_finite() {
            (lo.min(hi), hi.max(lo))
        } else {
            (0.0, 1.0)
        };
        let mid = sc.diverging.then(|| {
            sc.mid_number.filter(|v| v.is_finite()).unwrap_or_else(|| {
                lo + (hi - lo) * sc.mid_percent.unwrap_or(50.0).clamp(0.0, 100.0) / 100.0
            })
        });
        Scale {
            lo,
            mid,
            hi,
            c_lo: color(Some(sc.min.as_ref().unwrap_or(&dmin)), 0),
            c_mid: sc
                .diverging
                .then(|| sc.mid.as_ref().or(dmid.as_ref()).map(|c| color(Some(c), 0)))
                .flatten(),
            c_hi: color(Some(sc.max.as_ref().unwrap_or(&dmax)), 0),
        }
    }

    fn at(&self, v: f64) -> Rgba {
        let frac = |a: f64, z: f64| if z - a > 0.0 { (v - a) / (z - a) } else { 1.0 };
        if let (Some(m), Some(cm)) = (self.mid, self.c_mid) {
            if v <= m {
                lerp(self.c_lo, cm, frac(self.lo, m))
            } else {
                lerp(cm, self.c_hi, frac(m, self.hi))
            }
        } else {
            lerp(self.c_lo, self.c_hi, frac(self.lo, self.hi))
        }
    }
}

/// What a map shows for each data point.
enum Fill {
    Value(f64),
    Category(usize),
}

fn round2(v: f64) -> f32 {
    ((v * 100.0).round() / 100.0) as f32
}

/// Draws a region map into `[l, t, r, b]`: the regions, the legend (`legend` position) and labels.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn draw(
    wb: &Workbook,
    sheet: usize,
    chart: &Chart,
    series: Option<(&ChartSeries, &Ser)>,
    [l, t, r, b]: [f64; 4],
    font: f64,
    locale: &Locale,
    color: &ColorFn<'_>,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    let opts = chart.map.clone().unwrap_or_default();
    let Some((model, ser)) = series else { return };
    let names = &ser.cats;
    let level = MapLevel::of_chartex(opts.level.as_deref());
    let message = |text: &str, label: &mut LabelFn<'_>, out: &mut Vec<Command>| {
        let mid = f64::midpoint(t, b);
        // Two lines: the sentence, then the folder in brackets.
        let (a, z) = text.split_once(" (").map_or((text, ""), |(a, z)| (a, z));
        label(
            a,
            [l, mid - font * 1.8, r, mid],
            Align::Center,
            font,
            TEXT,
            out,
        );
        if !z.is_empty() {
            label(
                &format!("({z}"),
                [l, mid, r, mid + font * 1.8],
                Align::Center,
                font * 0.9,
                TEXT,
                out,
            );
        }
    };
    let Some(provider) = geo::provider_for(level, names) else {
        message(geo::needs_dataset_message(locale.lcid == 0x42A), label, out);
        return;
    };
    let regions = provider.regions();
    // Data: region index → what to fill it with (the first point naming a region wins).
    let texts = if opts.categorical {
        value_texts(wb, sheet, model, &opts, locale)
    } else {
        Vec::new()
    };
    let mut cats: Vec<String> = Vec::new();
    let mut data: Vec<(usize, usize, Fill)> = Vec::new();
    for (i, n) in names.iter().enumerate() {
        let Some(k) = provider.find(n) else { continue };
        if data.iter().any(|(r, _, _)| *r == k) {
            continue;
        }
        let fill = if opts.categorical {
            let Some(txt) = texts.get(i).filter(|t| !t.trim().is_empty()) else {
                continue;
            };
            let c = cats.iter().position(|c| c == txt).unwrap_or_else(|| {
                cats.push(txt.clone());
                cats.len() - 1
            });
            Fill::Category(c)
        } else {
            match ser.vals.get(i).copied().flatten() {
                Some(v) => Fill::Value(v),
                None => continue,
            }
        };
        data.push((k, i, fill));
    }
    let values: Vec<f64> = data
        .iter()
        .filter_map(|(_, _, f)| match f {
            Fill::Value(v) => Some(*v),
            Fill::Category(_) => None,
        })
        .collect();
    let scale = Scale::new(&opts.scale, &values, color);
    // Extent: the whole world, else the regions with data (else every region of the set).
    let world_view = opts.level.as_deref() == Some("world");
    let data_bbox = geo::bbox_of(
        data.iter()
            .flat_map(|(k, _, _)| regions[*k].rings.iter().flatten()),
    );
    let mut ext = if world_view || (data_bbox.is_none() && level == MapLevel::Country) {
        [
            -180.0,
            geo::projection::WORLD_LAT.0,
            180.0,
            geo::projection::WORLD_LAT.1,
        ]
    } else {
        data_bbox
            .or_else(|| geo::bbox_of(regions.iter().flat_map(|r| r.rings.iter().flatten())))
            .unwrap_or([-180.0, -58.0, 180.0, 84.0])
    };
    // A margin, and at least 4° across.
    let (cx, cy) = (f64::midpoint(ext[0], ext[2]), f64::midpoint(ext[1], ext[3]));
    let half_w = ((ext[2] - ext[0]) * 0.55).max(2.0);
    let half_h = ((ext[3] - ext[1]) * 0.55).max(2.0);
    if !world_view {
        ext = [
            (cx - half_w).max(-180.0),
            (cy - half_h).max(-85.0),
            (cx + half_w).min(180.0),
            (cy + half_h).min(85.0),
        ];
    }
    let default = if ext[2] - ext[0] > 150.0 {
        Projection::Miller
    } else {
        Projection::Mercator
    };
    let proj = Projection::of_chartex(opts.projection.as_deref(), default, ext);
    // Legend.
    let legend = chart.legend.as_deref();
    let (mut pl, mut pt, mut pr, mut pb) = (l, t, r, b);
    let has_scale = !opts.categorical && !values.is_empty();
    match legend {
        Some("b" | "t") if has_scale || !cats.is_empty() => {
            let h = font * 2.6;
            let y = if legend == Some("b") { pb - h } else { pt };
            draw_legend_h(
                &scale,
                &cats,
                has_scale,
                [l, y, r, y + h],
                font,
                locale,
                color,
                label,
                out,
            );
            if legend == Some("b") {
                pb -= h + 2.0;
            } else {
                pt += h + 2.0;
            }
        }
        Some(_) if has_scale || !cats.is_empty() => {
            let w = if has_scale {
                font * 6.0
            } else {
                (r - l) * 0.24
            }
            .clamp(36.0, 140.0);
            let x = if legend == Some("l") { l } else { r - w };
            draw_legend_v(
                &scale,
                &cats,
                has_scale,
                [x, t, x + w, b],
                font,
                locale,
                color,
                label,
                out,
            );
            if legend == Some("l") {
                pl += w + 4.0;
            } else {
                pr -= w + 4.0;
            }
        }
        _ => {}
    }
    if pr - pl < 8.0 || pb - pt < 8.0 {
        return;
    }
    // Fit the projected extent (its outline sampled: conic and pseudo-cylindrical edges curve).
    let mut bx = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for k in 0..=24 {
        let f = f64::from(k) / 24.0;
        for (lon, lat) in [
            (ext[0] + (ext[2] - ext[0]) * f, ext[1]),
            (ext[0] + (ext[2] - ext[0]) * f, ext[3]),
            (ext[0], ext[1] + (ext[3] - ext[1]) * f),
            (ext[2], ext[1] + (ext[3] - ext[1]) * f),
        ] {
            let (x, y) = proj.project(lon, lat);
            bx = [bx[0].min(x), bx[1].min(y), bx[2].max(x), bx[3].max(y)];
        }
    }
    let (w, h) = ((bx[2] - bx[0]).max(1e-9), (bx[3] - bx[1]).max(1e-9));
    let k = ((pr - pl) / w).min((pb - pt) / h);
    let ox = pl + ((pr - pl) - w * k) / 2.0;
    let oy = pt + ((pb - pt) - h * k) / 2.0;
    let screen = |lon: f64, lat: f64| -> (f64, f64) {
        let (x, y) = proj.project(lon, lat);
        (ox + (x - bx[0]) * k, oy + (bx[3] - y) * k)
    };
    out.push(rect(ox, oy, w * k, h * k, SEA));
    out.push(Command::PushClip {
        x: ox as f32,
        y: oy as f32,
        w: (w * k) as f32,
        h: (h * k) as f32,
    });
    // Rings are clipped a little outside the view (projections bend the edges).
    let clip = if world_view {
        [-180.0, -90.0, 180.0, 90.0]
    } else {
        let mx = (ext[2] - ext[0]) * 0.15;
        let my = (ext[3] - ext[1]) * 0.15;
        [
            (ext[0] - mx).max(-180.0),
            (ext[1] - my).max(-89.0),
            (ext[2] + mx).min(180.0),
            (ext[3] + my).min(89.0),
        ]
    };
    let fill_of = |k: usize| -> Option<Rgba> {
        data.iter()
            .find(|(r, _, _)| *r == k)
            .map(|(_, _, f)| match f {
                Fill::Value(v) => scale.at(*v),
                Fill::Category(c) => color(None, *c),
            })
    };
    let stroke = Stroke {
        width: 0.5,
        color: BORDER,
        dash: vec![],
        cap: "round".into(),
        join: "round".into(),
    };
    // Regions without data first, then those with data (their borders on top).
    for pass in [false, true] {
        for (ri, region) in regions.iter().enumerate() {
            let fill = fill_of(ri);
            if fill.is_some() != pass {
                continue;
            }
            if !region.bbox().is_some_and(|bb| overlaps(bb, clip)) {
                continue;
            }
            let mut segs = Vec::new();
            for ring in &region.rings {
                let pts = clip_ring(ring, clip);
                if pts.len() < 3 {
                    continue;
                }
                let mut last: Option<(f64, f64)> = None;
                let start = segs.len();
                for (lon, lat) in pts {
                    let (x, y) = screen(lon, lat);
                    // Points closer than half a point to the previous one add nothing.
                    if let Some((px, py)) = last {
                        if (x - px).abs() < 0.5 && (y - py).abs() < 0.5 {
                            continue;
                        }
                    }
                    segs.push(if last.is_none() {
                        PathSeg::M(round2(x), round2(y))
                    } else {
                        PathSeg::L(round2(x), round2(y))
                    });
                    last = Some((x, y));
                }
                if segs.len() - start < 3 {
                    segs.truncate(start);
                } else {
                    segs.push(PathSeg::Z);
                }
            }
            if segs.is_empty() {
                continue;
            }
            out.push(Command::Path {
                segs,
                fill: Some(Paint::Solid {
                    color: fill.unwrap_or(NO_DATA),
                }),
                stroke: Some(stroke.clone()),
            });
        }
    }
    out.push(Command::PopClip);
    // Region labels (`cx:regionLabelLayout`): every one, or those that fit their region.
    let show_all = opts.labels.as_deref() == Some("showAll");
    if show_all || opts.labels.as_deref() == Some("bestFitOnly") {
        for (k, i, _) in &data {
            let Some(ring) = regions[*k].main_ring() else {
                continue;
            };
            let pts: Vec<(f64, f64)> = clip_ring(ring, clip)
                .into_iter()
                .map(|(lon, lat)| screen(lon, lat))
                .collect();
            let Some(sb) = geo::bbox_of(pts.iter()) else {
                continue;
            };
            let text = names.get(*i).cloned().unwrap_or_default();
            let tw = text.chars().count() as f64 * font * 0.5;
            let Some((x, y, room)) = label_spot(&pts, sb) else {
                continue;
            };
            if !show_all && (room < tw || sb[3] - sb[1] < font) {
                continue;
            }
            label(
                &text,
                [x - tw, y - font, x + tw, y + font],
                Align::Center,
                font * 0.9,
                TEXT,
                out,
            );
        }
    }
}

/// Where a region's label goes: the middle of the widest stretch inside the ring on one of a few
/// horizontal lines (near the vertical middle first), with that stretch's width. A concave region
/// (a long coast) keeps its label inside it, unlike the bounding box centre.
fn label_spot(pts: &[(f64, f64)], [_, y0, _, y1]: [f64; 4]) -> Option<(f64, f64, f64)> {
    let n = pts.len();
    if n < 3 {
        return None;
    }
    // (x, y, stretch width, score)
    let mut best: Option<(f64, f64, f64, f64)> = None;
    for f in [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8] {
        let y = y0 + (y1 - y0) * f;
        // Crossings of the scan line with the ring's edges (even–odd stretches).
        let mut xs: Vec<f64> = Vec::new();
        for k in 0..n {
            let (ax, ay) = pts[k];
            let (bx, by) = pts[(k + 1) % n];
            if (ay <= y) != (by <= y) {
                xs.push(ax + (y - ay) / (by - ay) * (bx - ax));
            }
        }
        xs.sort_by(f64::total_cmp);
        for w in xs.chunks_exact(2) {
            let len = w[1] - w[0];
            // Lines near the middle win near ties.
            let score = len * (1.0 - (f - 0.5).abs());
            if best.is_none_or(|b| score > b.3) {
                best = Some((f64::midpoint(w[0], w[1]), y, len, score));
            }
        }
    }
    best.map(|(x, y, len, _)| (x, y, len))
}
/// Steps of a drawn colour bar.
const BAR_STEPS: usize = 24;

#[allow(clippy::too_many_arguments)]
fn draw_legend_v(
    scale: &Scale,
    cats: &[String],
    has_scale: bool,
    [l, t, r, b]: [f64; 4],
    font: f64,
    locale: &Locale,
    color: &ColorFn<'_>,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    if has_scale {
        // A vertical bar, the highest value at the top.
        let h = ((b - t) * 0.6).max(font * 4.0).min(b - t);
        let y0 = t + ((b - t) - h) / 2.0;
        let bw = font * 1.1;
        let step = h / BAR_STEPS as f64;
        for i in 0..BAR_STEPS {
            let f = 1.0 - (i as f64 + 0.5) / BAR_STEPS as f64;
            let v = scale.lo + (scale.hi - scale.lo) * f;
            out.push(rect(l, y0 + step * i as f64, bw, step + 0.2, scale.at(v)));
        }
        let lx = l + bw + 3.0;
        let mut put = |v: f64, y: f64| {
            label(
                &fmt_num(v, locale),
                [lx, y - font, r, y + font],
                Align::Left,
                font,
                TEXT,
                out,
            );
        };
        put(scale.hi, y0);
        if let Some(m) = scale.mid {
            let span = (scale.hi - scale.lo).max(1e-12);
            put(m, y0 + h * (1.0 - (m - scale.lo) / span));
        }
        put(scale.lo, y0 + h);
    } else {
        let row = font * 1.6;
        let total = row * cats.len() as f64;
        let mut y = t + ((b - t) - total).max(0.0) / 2.0;
        for (i, c) in cats.iter().enumerate() {
            out.push(rect(
                l,
                y + row * 0.3,
                font * 0.8,
                font * 0.8,
                color(None, i),
            ));
            label(
                c,
                [l + font * 1.1, y, r, y + row],
                Align::Left,
                font,
                TEXT,
                out,
            );
            y += row;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_legend_h(
    scale: &Scale,
    cats: &[String],
    has_scale: bool,
    [l, t, r, b]: [f64; 4],
    font: f64,
    locale: &Locale,
    color: &ColorFn<'_>,
    label: &mut LabelFn<'_>,
    out: &mut Vec<Command>,
) {
    if has_scale {
        let w = ((r - l) * 0.5).max(font * 8.0).min(r - l);
        let x0 = l + ((r - l) - w) / 2.0;
        let step = w / BAR_STEPS as f64;
        for i in 0..BAR_STEPS {
            let f = (i as f64 + 0.5) / BAR_STEPS as f64;
            let v = scale.lo + (scale.hi - scale.lo) * f;
            out.push(rect(x0 + step * i as f64, t, step + 0.2, font, scale.at(v)));
        }
        let y = t + font;
        let mut put = |v: f64, x: f64| {
            label(
                &fmt_num(v, locale),
                [x - font * 4.0, y, x + font * 4.0, b],
                Align::Center,
                font,
                TEXT,
                out,
            );
        };
        put(scale.lo, x0);
        if let Some(m) = scale.mid {
            let span = (scale.hi - scale.lo).max(1e-12);
            put(m, x0 + w * (m - scale.lo) / span);
        }
        put(scale.hi, x0 + w);
    } else {
        let n = cats.len().max(1) as f64;
        let cell = ((r - l) / n).min(110.0);
        let mut x = l + ((r - l) - cell * n) / 2.0;
        for (i, c) in cats.iter().enumerate() {
            out.push(rect(
                x,
                t + font * 0.5,
                font * 0.8,
                font * 0.8,
                color(None, i),
            ));
            label(
                c,
                [x + font * 1.1, t, x + cell, b],
                Align::Left,
                font,
                TEXT,
                out,
            );
            x += cell;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_keeps_inside_and_cuts_edges() {
        let sq = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let c = clip_ring(&sq, [5.0, -5.0, 20.0, 20.0]);
        let b = geo::bbox_of(c.iter()).unwrap();
        assert_eq!(b, [5.0, 0.0, 10.0, 10.0]);
        assert_eq!(
            clip_ring(&sq, [20.0, 20.0, 30.0, 30.0]),
            Vec::<LonLat>::new()
        );
    }

    #[test]
    fn scale_interpolates_two_and_three_colours() {
        let pal =
            |c: Option<&bo_model::sheet::ChartColor>, i: usize| super::super::default_color(c, i);
        let two = Scale::new(&MapColorScale::default(), &[0.0, 10.0], &pal);
        assert_eq!(two.at(0.0), Rgba(0xDC, 0xE9, 0xF5, 255));
        assert_eq!(two.at(10.0), Rgba(0x1F, 0x4E, 0x89, 255));
        let three = Scale::new(
            &MapColorScale {
                diverging: true,
                ..MapColorScale::default()
            },
            &[-4.0, 0.0, 4.0],
            &pal,
        );
        assert_eq!(three.mid, Some(0.0));
        assert_eq!(three.at(0.0), Rgba(0xF5, 0xF1, 0xE6, 255));
        assert_eq!(three.at(-4.0), Rgba(0xC0, 0x50, 0x4D, 255));
    }
}
