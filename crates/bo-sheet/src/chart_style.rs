//! Chart styles, colour variations, quick layouts and chart templates (CORE-CHART-007), shared by
//! BlueSheet, BlueSlide and BlueDoc charts.
//!
//! Every preset here is BlueOffice's own design: the styles set the shape and text properties of
//! a chart ([`ChartFormat`]: series fill / outline / effects, chart and plot area, gridlines,
//! text), the colour variations its series palette (from the document theme's accents —
//! §20.1.6.2 clrScheme `accent1`…`accent6` — or BlueOffice palettes), the quick layouts its
//! title / legend / axis titles / data labels / gridlines. A chart template keeps a chart's type
//! and formatting without its data, to apply to another chart or a new one.
//!
//! The formatting is written into the chart part as explicit `c:spPr` / `c:txPr` elements by the
//! chart writer (`bo_xlsx::chart_write`), so it round-trips in `.xlsx`, `.pptx` and `.docx`.

use crate::chart::{self, ColorFn};
use bo_model::chart_style::{ChartFormat, DataLabels, SeriesFill};
use bo_model::sheet::{Chart, ChartColor, ChartKind, View3D, Workbook};
use bo_render::{Command, Paint, PathSeg, Rgba};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// BlueOffice's default accents (used when a document has no theme).
pub const DEFAULT_ACCENTS: [u32; 6] = [
    0x2E_6DB4, 0xF2_8C28, 0x2F_A36B, 0xD6_4545, 0x7E_57C2, 0xC9_A227,
];

/// Chart style ids, in gallery order.
pub const STYLES: &[&str] = &[
    "clean",
    "crisp",
    "soft",
    "gradient",
    "outline",
    "shadow",
    "bold",
    "paper",
    "slate",
    "night",
    "blueprint",
    "minimal",
];

/// Colour variation ids, in menu order.
pub const PALETTES: &[&str] = &[
    "colorful",
    "colorfulAlt",
    "monoAccent1",
    "monoAccent2",
    "grey",
    "bluestar",
    "lotus",
    "terrace",
];

/// Quick layout ids, in menu order.
pub const LAYOUTS: &[&str] = &[
    "layout1", "layout2", "layout3", "layout4", "layout5", "layout6",
];

/// Version of the template format.
pub const TEMPLATE_VERSION: u32 = 1;

#[allow(clippy::unnecessary_wraps)]
fn rgb(v: u32) -> Option<ChartColor> {
    Some(ChartColor::Rgb(v))
}

/// The style part of `id` on top of `base` (palette, gridline switches and data labels — the
/// colour variation and quick layout — are kept); `None` for an unknown style.
#[must_use]
pub fn style_format(id: &str, base: &ChartFormat) -> Option<ChartFormat> {
    let mut f = ChartFormat {
        palette: base.palette.clone(),
        hide_val_grid: base.hide_val_grid,
        cat_grid: base.cat_grid,
        data_labels: base.data_labels.clone(),
        ..ChartFormat::default()
    };
    match id {
        "clean" => {
            f.grid_color = rgb(0xD9_DDE3);
            f.text_color = rgb(0x40_4040);
        }
        "crisp" => {
            f.outline = rgb(0xFF_FFFF);
            f.outline_w = 12_700;
            f.gap_width = Some(80);
            f.grid_color = rgb(0xE3_E6EA);
            f.title_bold = true;
        }
        "soft" => {
            f.series_fill = SeriesFill::Soft;
            f.gap_width = Some(60);
            f.rounded = true;
            f.plot_fill = rgb(0xF7_F9FC);
            f.grid_color = rgb(0xEE_F1F5);
            f.line_w = Some(25_400);
        }
        "gradient" => {
            f.series_fill = SeriesFill::Gradient;
            f.gap_width = Some(100);
            f.grid_color = rgb(0xDD_E1E6);
            f.title_bold = true;
        }
        "outline" => {
            f.series_fill = SeriesFill::Outline;
            f.outline_w = 19_050;
            f.line_w = Some(19_050);
            f.markers = true;
            f.grid_color = rgb(0xE6_E9ED);
        }
        "shadow" => {
            f.shadow = true;
            f.gap_width = Some(120);
            f.line_w = Some(31_750);
            f.markers = true;
            f.grid_color = rgb(0xE1_E4E8);
        }
        "bold" => {
            f.gap_width = Some(50);
            f.line_w = Some(44_450);
            f.title_bold = true;
            f.text_color = rgb(0x1F_2933);
            f.axis_line = rgb(0x1F_2933);
            f.grid_color = rgb(0xCB_D2D9);
        }
        "paper" => {
            f.chart_fill = rgb(0xFB_F8F1);
            f.plot_fill = rgb(0xFB_F8F1);
            f.grid_color = rgb(0xE8_DFCC);
            f.text_color = rgb(0x5C_4B37);
            f.axis_line = rgb(0x8C_7A5B);
            f.outline = rgb(0x5C_4B37);
            f.outline_w = 6_350;
        }
        "slate" => {
            f.chart_fill = rgb(0xEE_F1F5);
            f.plot_fill = rgb(0xFF_FFFF);
            f.grid_color = rgb(0xDC_E1E7);
            f.text_color = rgb(0x33_4155);
            f.rounded = true;
            f.markers = true;
        }
        "night" => {
            f.chart_fill = rgb(0x1E_2530);
            f.chart_line = rgb(0x1E_2530);
            f.plot_fill = rgb(0x1E_2530);
            f.grid_color = rgb(0x3A_4553);
            f.text_color = rgb(0xE6_EBF2);
            f.axis_line = rgb(0x6B_7787);
            f.outline = rgb(0x1E_2530);
            f.title_bold = true;
        }
        "blueprint" => {
            f.chart_fill = rgb(0x11_3A6B);
            f.chart_line = rgb(0x11_3A6B);
            f.plot_fill = rgb(0x11_3A6B);
            f.grid_color = rgb(0x2F_5C8F);
            f.text_color = rgb(0xDC_E8F7);
            f.axis_line = rgb(0x9D_B8D9);
            f.series_fill = SeriesFill::Outline;
            f.outline_w = 19_050;
            f.line_w = Some(22_225);
        }
        "minimal" => {
            f.grid_color = rgb(0xF0_F0F0);
            f.grid_w = Some(6_350);
            f.text_color = rgb(0x6B_7280);
            f.line_w = Some(15_875);
            f.gap_width = Some(200);
            f.font_size = Some(800);
        }
        _ => return None,
    }
    Some(f)
}

/// The theme accents of a workbook (§20.1.6.2 clrScheme; [`DEFAULT_ACCENTS`] when absent).
#[must_use]
pub fn theme_accents(wb: &Workbook) -> [u32; 6] {
    let mut out = DEFAULT_ACCENTS;
    if let Some(t) = &wb.theme {
        // dk1, lt1, dk2, lt2, accent1–6, …
        for (i, slot) in out.iter_mut().enumerate() {
            if let Some(v) = t
                .colors
                .get(4 + i)
                .and_then(|c| u32::from_str_radix(c.trim(), 16).ok())
            {
                *slot = v;
            }
        }
    }
    out
}

fn split(v: u32) -> (f64, f64, f64) {
    (
        f64::from((v >> 16) & 0xFF),
        f64::from((v >> 8) & 0xFF),
        f64::from(v & 0xFF),
    )
}

fn join(r: f64, g: f64, b: f64) -> u32 {
    let c = |x: f64| x.round().clamp(0.0, 255.0) as u32;
    (c(r) << 16) | (c(g) << 8) | c(b)
}

/// `v` mixed with white (`k > 0`) or black (`k < 0`) by `|k|`.
#[must_use]
pub fn shade(v: u32, k: f64) -> u32 {
    let (r, g, b) = split(v);
    if k >= 0.0 {
        join(
            r + (255.0 - r) * k,
            g + (255.0 - g) * k,
            b + (255.0 - b) * k,
        )
    } else {
        join(r * (1.0 + k), g * (1.0 + k), b * (1.0 + k))
    }
}

/// Six shades of one colour, neighbours contrasting (monochromatic variation).
fn mono(v: u32) -> Vec<u32> {
    [0.0, -0.3, 0.35, -0.5, 0.6, 0.8]
        .iter()
        .map(|k| shade(v, *k))
        .collect()
}

/// The colours (`RRGGBB`) of colour variation `id` with the theme accents `accents`; `None` for
/// an unknown id.
#[must_use]
pub fn palette_colors(id: &str, accents: &[u32; 6]) -> Option<Vec<u32>> {
    Some(match id {
        "colorful" => accents.to_vec(),
        "colorfulAlt" => [0, 2, 4, 1, 3, 5].iter().map(|i| accents[*i]).collect(),
        "monoAccent1" => mono(accents[0]),
        "monoAccent2" => mono(accents[1]),
        "grey" => vec![
            0x40_4040, 0x7F_8A99, 0x2B_2F36, 0xA0_A8B4, 0x5F_6B7A, 0xC2_C8D0,
        ],
        "bluestar" => vec![
            0x2E_6DB4, 0xF2_8C28, 0x2F_A36B, 0xD6_4545, 0x7E_57C2, 0xC9_A227, 0x1F_A7A8, 0x6B_7280,
        ],
        "lotus" => vec![
            0xB2_3A6F, 0xE9_849B, 0x7A_3E9D, 0xF2_A65A, 0x4E_8D7C, 0xD9_B44A,
        ],
        "terrace" => vec![
            0x3D_7A3A, 0x8D_B255, 0xC9_B458, 0x5B_8C85, 0xA0_522D, 0x2F_4B26,
        ],
        _ => return None,
    })
}

fn is_pie(c: &Chart) -> bool {
    c.plots.first().is_some_and(|p| {
        matches!(
            p.kind,
            ChartKind::Pie | ChartKind::Doughnut | ChartKind::OfPie
        )
    })
}

/// Applies chart style `id`; `false` for an unknown style.
pub fn apply_style(c: &mut Chart, id: &str) -> bool {
    let Some(f) = style_format(id, &c.format) else {
        return false;
    };
    c.format = f;
    c.dirty = true;
    true
}

/// Applies colour variation `id`: the palette replaces every series' and point's own colour.
pub fn apply_colors(c: &mut Chart, id: &str, accents: &[u32; 6]) -> bool {
    let Some(colors) = palette_colors(id, accents) else {
        return false;
    };
    set_palette(c, colors.into_iter().map(ChartColor::Rgb).collect());
    true
}

fn set_palette(c: &mut Chart, palette: Vec<ChartColor>) {
    c.format.palette = palette;
    for p in &mut c.plots {
        for s in &mut p.series {
            s.fill = None;
            s.line = None;
            s.point_fills.clear();
        }
    }
    c.dirty = true;
}

/// Texts a quick layout gives to titles the chart does not have yet (localised by the UI).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LayoutTexts {
    /// Chart title.
    pub title: Option<String>,
    /// Horizontal (category) axis title.
    pub x_title: Option<String>,
    /// Vertical (value) axis title.
    pub y_title: Option<String>,
}

/// What a quick layout shows.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Layout {
    title: bool,
    legend: Option<&'static str>,
    axis_titles: (bool, bool),
    labels: Option<DataLabels>,
    val_grid: bool,
    cat_grid: bool,
    table: bool,
}

fn layout_of(id: &str, pie: bool) -> Option<Layout> {
    let values = |pos: &str| DataLabels {
        value: !pie,
        percent: pie,
        pos: Some(pos.to_owned()),
        ..DataLabels::default()
    };
    Some(match id {
        // Title, legend on the right, value gridlines.
        "layout1" => Layout {
            title: true,
            legend: Some("r"),
            axis_titles: (false, false),
            labels: None,
            val_grid: true,
            cat_grid: false,
            table: false,
        },
        // Title, legend below, values on the points, no gridlines.
        "layout2" => Layout {
            title: true,
            legend: Some("b"),
            axis_titles: (false, false),
            labels: Some(values(if pie { "bestFit" } else { "outEnd" })),
            val_grid: false,
            cat_grid: false,
            table: false,
        },
        // Title, legend below, both axis titles, value gridlines.
        "layout3" => Layout {
            title: true,
            legend: Some("b"),
            axis_titles: (true, true),
            labels: None,
            val_grid: true,
            cat_grid: false,
            table: false,
        },
        // No title, legend on top, a full grid.
        "layout4" => Layout {
            title: false,
            legend: Some("t"),
            axis_titles: (false, false),
            labels: None,
            val_grid: true,
            cat_grid: true,
            table: false,
        },
        // Title, a data table under the plot, value axis title, no legend.
        "layout5" => Layout {
            title: true,
            legend: None,
            axis_titles: (false, true),
            labels: None,
            val_grid: true,
            cat_grid: false,
            table: true,
        },
        // Title, labels with category and value (or percentage) instead of a legend.
        "layout6" => Layout {
            title: true,
            legend: None,
            axis_titles: (false, false),
            labels: Some(DataLabels {
                category: true,
                ..values(if pie { "bestFit" } else { "outEnd" })
            }),
            val_grid: false,
            cat_grid: false,
            table: false,
        },
        _ => return None,
    })
}

fn want_text(
    have: Option<&String>,
    on: bool,
    text: Option<&String>,
    fallback: &str,
) -> Option<String> {
    if !on {
        return None;
    }
    have.cloned()
        .or_else(|| text.cloned())
        .filter(|t| !t.trim().is_empty())
        .or_else(|| Some(fallback.to_owned()))
}

/// A layout or template sets the data labels of the whole chart: labels the series or single
/// points had of their own (`c:ser/c:dLbls`) give way to them.
fn clear_series_labels(c: &mut Chart) {
    for s in c.plots.iter_mut().flat_map(|p| &mut p.series) {
        s.labels = None;
        s.point_labels.clear();
    }
}

/// Applies quick layout `id`; titles the layout adds take `texts`; `false` for an unknown id.
pub fn apply_layout(c: &mut Chart, id: &str, texts: &LayoutTexts) -> bool {
    let Some(l) = layout_of(id, is_pie(c)) else {
        return false;
    };
    c.title = want_text(c.title.as_ref(), l.title, texts.title.as_ref(), "Chart");
    c.legend = l.legend.map(str::to_owned);
    if !is_pie(c) {
        c.x_title = want_text(
            c.x_title.as_ref(),
            l.axis_titles.0,
            texts.x_title.as_ref(),
            "X",
        );
        c.y_title = want_text(
            c.y_title.as_ref(),
            l.axis_titles.1,
            texts.y_title.as_ref(),
            "Y",
        );
        c.axes.data_table = l.table;
    }
    c.format.data_labels = l.labels;
    clear_series_labels(c);
    c.format.hide_val_grid = !l.val_grid;
    c.format.cat_grid = l.cat_grid;
    c.dirty = true;
    true
}

/// A chart template: the type and formatting of a chart without its data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartTemplate {
    /// [`TEMPLATE_VERSION`].
    #[serde(default)]
    pub version: u32,
    /// Name shown in the template picker.
    #[serde(default)]
    pub name: String,
    /// Chart type ([`chart::CHART_TYPES`] name).
    pub kind: String,
    /// Formatting (the palette holds the series colours).
    #[serde(default)]
    pub format: ChartFormat,
    /// Legend position; `None` = no legend.
    #[serde(default)]
    pub legend: Option<String>,
    /// Shows a chart title.
    #[serde(default)]
    pub title: bool,
    /// Shows the horizontal axis title.
    #[serde(default)]
    pub x_title: bool,
    /// Shows the vertical axis title.
    #[serde(default)]
    pub y_title: bool,
    /// Shows a data table (§21.2.2.54).
    #[serde(default)]
    pub data_table: bool,
    /// 3-D view of a 3-D type (§21.2.2.228).
    #[serde(default)]
    pub view3d: Option<View3D>,
}

/// The template of chart `c` named `name`. Series colours of the chart become the template's
/// palette.
#[must_use]
pub fn save_template(c: &Chart, name: &str) -> ChartTemplate {
    let mut format = c.format.clone();
    let own: Vec<ChartColor> = if is_pie(c) {
        c.plots
            .first()
            .and_then(|p| p.series.first())
            .map(|s| {
                let mut v = s.point_fills.clone();
                v.sort_by_key(|(i, _)| *i);
                v.into_iter().map(|(_, c)| c).collect()
            })
            .unwrap_or_default()
    } else {
        c.plots
            .iter()
            .flat_map(|p| &p.series)
            .map_while(|s| s.fill.clone().or_else(|| s.line.clone()))
            .collect()
    };
    if format.palette.is_empty() && !own.is_empty() {
        format.palette = own;
    }
    ChartTemplate {
        version: TEMPLATE_VERSION,
        name: name.trim().to_owned(),
        kind: chart::type_name(c).to_owned(),
        format,
        legend: c.legend.clone(),
        title: c.title.is_some(),
        x_title: c.x_title.is_some(),
        y_title: c.y_title.is_some(),
        data_table: c.axes.data_table,
        view3d: c.axes.view3d.clone(),
    }
}

/// Applies template `t` to chart `c` (type, formatting, titles shown, legend); titles the
/// template shows and the chart lacks take `texts`.
pub fn apply_template(c: &mut Chart, t: &ChartTemplate, texts: &LayoutTexts) {
    if chart::CHART_TYPES.contains(&t.kind.as_str()) && chart::type_name(c) != t.kind {
        chart::set_chart_type(c, &t.kind);
    }
    let palette = t.format.palette.clone();
    c.format = t.format.clone();
    clear_series_labels(c);
    if !palette.is_empty() {
        set_palette(c, palette);
    }
    c.legend = t.legend.clone();
    c.title = want_text(c.title.as_ref(), t.title, texts.title.as_ref(), "Chart");
    c.x_title = want_text(c.x_title.as_ref(), t.x_title, texts.x_title.as_ref(), "X");
    c.y_title = want_text(c.y_title.as_ref(), t.y_title, texts.y_title.as_ref(), "Y");
    c.axes.data_table = t.data_table;
    if c.plots.iter().any(|p| p.three_d) {
        if let Some(v) = &t.view3d {
            c.axes.view3d = Some(v.clone());
        }
    }
    c.dirty = true;
}

// ---------------------------------------------------------------------------
// Previews
// ---------------------------------------------------------------------------

fn hex(Rgba(r, g, b, _): Rgba) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn alpha(Rgba(_, _, _, a): Rgba) -> String {
    if a == 255 {
        String::new()
    } else {
        format!(" fill-opacity=\"{:.2}\"", f64::from(a) / 255.0)
    }
}

fn path_d(segs: &[PathSeg]) -> String {
    let mut d = String::new();
    for s in segs {
        let _ = match *s {
            PathSeg::M(x, y) => write!(d, "M{x:.1} {y:.1}"),
            PathSeg::L(x, y) => write!(d, "L{x:.1} {y:.1}"),
            PathSeg::Q(a, b, x, y) => write!(d, "Q{a:.1} {b:.1} {x:.1} {y:.1}"),
            PathSeg::C(a, b, c, e, x, y) => {
                write!(d, "C{a:.1} {b:.1} {c:.1} {e:.1} {x:.1} {y:.1}")
            }
            PathSeg::Z => write!(d, "Z"),
        };
    }
    d
}

/// The display list of a chart preview as an SVG document (rectangles, lines, paths and
/// gradients; text is drawn as bars).
#[must_use]
pub fn commands_svg(cmds: &[Command], w: f64, h: f64) -> String {
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.0} {h:.0}\" width=\"{w:.0}\" height=\"{h:.0}\">"
    );
    let mut grads = 0usize;
    for c in cmds {
        match c {
            Command::FillRect { x, y, w, h, color } => {
                let _ = write!(
                    out,
                    "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" fill=\"{}\"{}/>",
                    hex(*color),
                    alpha(*color)
                );
            }
            Command::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                color,
                ..
            } => {
                let _ = write!(
                    out,
                    "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"{}\" stroke-width=\"{width:.2}\"/>",
                    hex(*color)
                );
            }
            Command::Path { segs, fill, stroke } => {
                let fill_attr = match fill {
                    None => "fill=\"none\"".to_owned(),
                    Some(Paint::Solid { color }) => {
                        format!("fill=\"{}\"{}", hex(*color), alpha(*color))
                    }
                    Some(Paint::Linear {
                        x1,
                        y1,
                        x2,
                        y2,
                        stops,
                    }) => {
                        grads += 1;
                        let _ = write!(
                            out,
                            "<defs><linearGradient id=\"g{grads}\" gradientUnits=\"userSpaceOnUse\" x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\">"
                        );
                        for s in stops {
                            let _ = write!(
                                out,
                                "<stop offset=\"{:.2}\" stop-color=\"{}\"/>",
                                s.offset,
                                hex(s.color)
                            );
                        }
                        out.push_str("</linearGradient></defs>");
                        format!("fill=\"url(#g{grads})\"")
                    }
                    Some(Paint::Radial { stops, .. }) => stops.last().map_or_else(
                        || "fill=\"none\"".to_owned(),
                        |s| format!("fill=\"{}\"", hex(s.color)),
                    ),
                };
                let stroke_attr = stroke.as_ref().map_or_else(String::new, |s| {
                    format!(
                        " stroke=\"{}\" stroke-width=\"{:.2}\"",
                        hex(s.color),
                        s.width
                    )
                });
                let _ = write!(
                    out,
                    "<path d=\"{}\" {fill_attr}{stroke_attr}/>",
                    path_d(segs)
                );
            }
            Command::PushTransform { m } => {
                let _ = write!(
                    out,
                    "<g transform=\"matrix({} {} {} {} {} {})\">",
                    m[0], m[1], m[2], m[3], m[4], m[5]
                );
            }
            Command::PopTransform => out.push_str("</g>"),
            _ => {}
        }
    }
    out.push_str("</svg>");
    out
}

/// A thumbnail of `c` drawn by the shared chart renderer at `w × h` points, as SVG. Labels are
/// drawn as bars of their ink (the thumbnail is too small to read them).
#[must_use]
pub fn preview_svg(c: &Chart, w: f64, h: f64, color: &ColorFn<'_>) -> String {
    let wb = Workbook::default();
    let mut cmds = Vec::new();
    let mut label = |text: &str,
                     [l, t, r, b]: [f64; 4],
                     align: chart::Align,
                     size: f64,
                     ink: Rgba,
                     out: &mut Vec<Command>| {
        let bw = (text.chars().count() as f64 * size * 0.5)
            .min(r - l)
            .max(1.0);
        let bh = (size * 0.55).max(1.0);
        let x = match align {
            chart::Align::Left => l,
            chart::Align::Center => f64::midpoint(l, r) - bw / 2.0,
            chart::Align::Right => r - bw,
        };
        let Rgba(cr, cg, cb, _) = ink;
        out.push(Command::FillRect {
            x: x as f32,
            y: (f64::midpoint(t, b) - bh / 2.0) as f32,
            w: bw as f32,
            h: bh as f32,
            color: Rgba(cr, cg, cb, 150),
        });
    };
    chart::draw_chart_with(
        &wb,
        0,
        c,
        w,
        h,
        &crate::numfmt::Locale::default(),
        color,
        true,
        &mut label,
        &mut cmds,
    );
    commands_svg(&cmds, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{ChartPlot, ChartSeries};

    fn sample() -> Chart {
        let s = |n: &str, v: [f64; 3]| ChartSeries {
            name: Some(n.into()),
            categories: vec!["A".into(), "B".into(), "C".into()],
            values: v.iter().map(|x| Some(*x)).collect(),
            fill: Some(ChartColor::Rgb(0x12_3456)),
            ..ChartSeries::default()
        };
        Chart {
            title: Some("Doanh thu".into()),
            legend: Some("r".into()),
            plots: vec![ChartPlot {
                kind: ChartKind::Bar,
                bar_dir: Some("col".into()),
                grouping: Some("clustered".into()),
                series: vec![s("HN", [3.0, 5.0, 2.0]), s("HCM", [4.0, 1.0, 6.0])],
                ..ChartPlot::default()
            }],
            ..Chart::default()
        }
    }

    #[test]
    fn every_preset_is_defined() {
        for id in STYLES {
            assert!(style_format(id, &ChartFormat::default()).is_some(), "{id}");
        }
        for id in PALETTES {
            let p = palette_colors(id, &DEFAULT_ACCENTS).unwrap();
            assert!(p.len() >= 6, "{id}");
        }
        for id in LAYOUTS {
            assert!(layout_of(id, false).is_some() && layout_of(id, true).is_some());
        }
        assert!(style_format("nope", &ChartFormat::default()).is_none());
        // The styles look different from one another.
        let all: Vec<ChartFormat> = STYLES
            .iter()
            .filter_map(|id| style_format(id, &ChartFormat::default()))
            .collect();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn style_keeps_colours_and_layout() {
        let mut c = sample();
        c.format.hide_val_grid = true;
        assert!(apply_style(&mut c, "night"));
        assert!(c.dirty);
        assert_eq!(c.format.chart_fill, Some(ChartColor::Rgb(0x1E_2530)));
        assert_eq!(c.format.text_color, Some(ChartColor::Rgb(0xE6_EBF2)));
        assert!(c.format.hide_val_grid);
        // Series colours stay.
        assert_eq!(c.plots[0].series[0].fill, Some(ChartColor::Rgb(0x12_3456)));
        assert!(apply_style(&mut c, "crisp"));
        assert_eq!(c.format.chart_fill, None);
        assert_eq!(c.format.outline_w, 12_700);
    }

    #[test]
    fn palettes_come_from_the_theme() {
        let accents = [
            0x11_1111, 0x22_2222, 0x33_3333, 0x44_4444, 0x55_5555, 0x66_6666,
        ];
        assert_eq!(
            palette_colors("colorful", &accents).unwrap(),
            accents.to_vec()
        );
        assert_eq!(
            palette_colors("colorfulAlt", &accents).unwrap()[1],
            0x33_3333
        );
        let mono = palette_colors("monoAccent2", &[0x80_4020, 0x20_4080, 0, 0, 0, 0]).unwrap();
        assert_eq!(mono[0], 0x20_4080);
        // Darker, then lighter shades of the accent.
        assert_eq!(mono[1], shade(0x20_4080, -0.3));
        assert!(mono[2] > 0x20_4080);
        let wb = Workbook {
            theme: Some(bo_model::sheet::ThemeColors {
                colors: [
                    "000000", "FFFFFF", "1F2933", "F5F7FA", "AA0000", "00AA00", "0000AA", "AAAA00",
                    "00AAAA", "AA00AA",
                ]
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            }),
            ..Workbook::default()
        };
        assert_eq!(theme_accents(&wb)[0], 0xAA_0000);
        assert_eq!(theme_accents(&Workbook::default()), DEFAULT_ACCENTS);
        let mut c = sample();
        assert!(apply_colors(&mut c, "colorful", &theme_accents(&wb)));
        assert_eq!(c.plots[0].series[0].fill, None);
        assert_eq!(c.format.palette[1], ChartColor::Rgb(0x00_AA00));
    }

    #[test]
    fn quick_layouts() {
        let mut c = sample();
        let texts = LayoutTexts {
            x_title: Some("Tháng".into()),
            ..LayoutTexts::default()
        };
        assert!(apply_layout(&mut c, "layout3", &texts));
        assert_eq!(c.x_title.as_deref(), Some("Tháng"));
        assert_eq!(c.y_title.as_deref(), Some("Y"));
        assert_eq!(c.legend.as_deref(), Some("b"));
        assert!(apply_layout(&mut c, "layout2", &texts));
        assert_eq!(c.x_title, None);
        assert!(c.format.hide_val_grid);
        assert_eq!(c.format.data_labels.as_ref().map(|d| d.value), Some(true));
        assert!(apply_layout(&mut c, "layout4", &texts));
        assert_eq!(c.title, None);
        assert!(c.format.cat_grid && !c.format.hide_val_grid);
        assert!(!apply_layout(&mut c, "layout9", &texts));
    }

    #[test]
    fn template_round_trip_on_another_chart() {
        let mut a = sample();
        apply_style(&mut a, "gradient");
        apply_colors(&mut a, "lotus", &DEFAULT_ACCENTS);
        apply_layout(&mut a, "layout3", &LayoutTexts::default());
        chart::set_chart_type(&mut a, "bar");
        let t = save_template(&a, " Báo cáo ");
        assert_eq!(t.name, "Báo cáo");
        assert_eq!(t.kind, "bar");
        let json = serde_json::to_string(&t).unwrap();
        let back: ChartTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
        // Another chart (a line chart with other data) takes the same formatting.
        let mut b = sample();
        chart::set_chart_type(&mut b, "line");
        b.plots[0].series[0].values = vec![Some(9.0)];
        b.title = None;
        apply_template(&mut b, &back, &LayoutTexts::default());
        assert_eq!(chart::type_name(&b), "bar");
        assert_eq!(b.format, a.format);
        assert_eq!(b.legend, a.legend);
        assert!(b.title.is_some() && b.x_title.is_some());
        assert_eq!(b.plots[0].series[0].values, vec![Some(9.0)]);
        // A chart whose colours are its own: the template keeps them as its palette.
        let own = save_template(&sample(), "own");
        assert_eq!(own.format.palette.len(), 2);
    }

    #[test]
    fn style_reaches_the_rendered_chart() {
        let mut c = sample();
        apply_colors(&mut c, "terrace", &DEFAULT_ACCENTS);
        apply_style(&mut c, "night");
        let wb = Workbook::default();
        let mut cmds = Vec::new();
        chart::draw_chart(
            &wb,
            0,
            &c,
            300.0,
            200.0,
            &crate::numfmt::Locale::default(),
            &mut |_, _, _, _, _, _| {},
            &mut cmds,
        );
        let night = Rgba(0x1E, 0x25, 0x30, 255);
        assert!(cmds
            .iter()
            .any(|c| matches!(c, Command::FillRect { color, .. } if *color == night)));
        // Bars in the terrace palette, outlined in the chart's colour.
        let first = Rgba(0x3D, 0x7A, 0x3A, 255);
        assert!(cmds.iter().any(|c| matches!(c,
            Command::Path { fill: Some(Paint::Solid { color }), stroke: Some(s), .. }
                if *color == first && s.color == night)));
        // Gridlines in the style's colour.
        let grid = Rgba(0x3A, 0x45, 0x53, 255);
        assert!(cmds
            .iter()
            .any(|c| matches!(c, Command::Line { color, .. } if *color == grid)));
        let svg = preview_svg(&c, 120.0, 80.0, &chart::default_color);
        assert!(svg.starts_with("<svg") && svg.contains("#1E2530"));
    }

    #[test]
    fn data_labels_are_drawn() {
        let mut c = sample();
        apply_layout(&mut c, "layout2", &LayoutTexts::default());
        let mut labels = Vec::new();
        chart::draw_chart(
            &Workbook::default(),
            0,
            &c,
            300.0,
            200.0,
            &crate::numfmt::Locale::default(),
            &mut |t, _, _, _, _, _| labels.push(t.to_owned()),
            &mut Vec::new(),
        );
        assert!(labels.contains(&"6".to_owned()), "{labels:?}");
        let mut pie = sample();
        chart::set_chart_type(&mut pie, "pie");
        apply_layout(&mut pie, "layout6", &LayoutTexts::default());
        let mut labels = Vec::new();
        chart::draw_chart(
            &Workbook::default(),
            0,
            &pie,
            300.0,
            200.0,
            &crate::numfmt::Locale::default(),
            &mut |t, _, _, _, _, _| labels.push(t.to_owned()),
            &mut Vec::new(),
        );
        // Category and percentage: 3 / 10 = 30 %.
        assert!(labels.contains(&"A; 30%".to_owned()), "{labels:?}");
    }
}
