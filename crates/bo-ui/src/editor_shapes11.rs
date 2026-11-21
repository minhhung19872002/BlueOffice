//! BlueEditor round 11: Edit › Add Shapes as the reference editor's drop-down grid (black box
//! at 125 %: 326 × 129 px, a bold "Add Path" heading over nine 30 px shapes on a 36 px pitch,
//! then "Add Shading" over the eight closed ones filled with a shading; the shapes are our
//! own drawings), and the shape's options in the Format pane: line and fill colours, line
//! width, opacity and dash (`bo_pdfread::ShapeStyle`). The reference shows its pane only for a
//! drawn shape, which posted input cannot draw there; ours follows the tool.
use crate::editor_tools::BoxTool;
use crate::editor_window::{inside, EditorWindow};
use crate::{Input, Painter, TextMeasure, TextStyle};
use bo_pdfread::{ShapeKind, ShapeStyle};
use bo_render::{PathSeg, Rgba};

/// Add Shapes state: the open grid (window rectangle), the cell under the pointer, and the
/// style new shapes get.
#[derive(Debug, Clone)]
pub(crate) struct Shapes11 {
    pub grid: Option<[f32; 4]>,
    pub hover: Option<(usize, usize)>,
    pub style: ShapeStyle,
}

impl Default for Shapes11 {
    fn default() -> Self {
        Self {
            grid: None,
            hover: None,
            style: ShapeStyle {
                stroke: Some([0, 0, 0]),
                fill: None,
                width: 1.0,
                opacity: 1.0,
                dash: Vec::new(),
                shading: false,
            },
        }
    }
}

/// The grid's rows: Add Path, then Add Shading (no line).
pub(crate) const PATH_ROW: [ShapeKind; 9] = [
    ShapeKind::Line,
    ShapeKind::Rectangle,
    ShapeKind::RoundedRectangle,
    ShapeKind::Ellipse,
    ShapeKind::Connector,
    ShapeKind::Polygon,
    ShapeKind::Curve,
    ShapeKind::Arrow,
    ShapeKind::Corner,
];

fn row_kinds(row: usize) -> &'static [ShapeKind] {
    if row == 0 {
        &PATH_ROW
    } else {
        &PATH_ROW[1..]
    }
}

/// Grid size and cells at 125 % (measured).
const GRID: (f32, f32) = (326.0, 129.0);
const CELL_X0: f32 = 4.0;
const CELL_PITCH: f32 = 36.0;
const CELL: f32 = 30.0;
const ROW_Y: [f32; 2] = [31.0, 93.0];
const HEAD_Y: [f32; 2] = [6.0, 68.0];

/// Colours offered in the pane (black, red, blue, green, orange, grey, white).
pub(crate) const COLORS: [[u8; 3]; 7] = [
    [0, 0, 0],
    [0xE0, 0x20, 0x20],
    [0x1F, 0x5F, 0xC8],
    [0x2E, 0x9E, 0x4F],
    [0xF2, 0x8C, 0x1E],
    [0x80, 0x80, 0x80],
    [0xFF, 0xFF, 0xFF],
];
const STROKE_CMDS: [&str; 8] = [
    "sh11.stroke.0",
    "sh11.stroke.1",
    "sh11.stroke.2",
    "sh11.stroke.3",
    "sh11.stroke.4",
    "sh11.stroke.5",
    "sh11.stroke.6",
    "sh11.stroke.none",
];
const FILL_CMDS: [&str; 8] = [
    "sh11.fill.0",
    "sh11.fill.1",
    "sh11.fill.2",
    "sh11.fill.3",
    "sh11.fill.4",
    "sh11.fill.5",
    "sh11.fill.6",
    "sh11.fill.none",
];
/// Line widths (points).
pub(crate) const WIDTHS: [f32; 6] = [0.5, 1.0, 2.0, 3.0, 4.0, 6.0];
const WIDTH_CMDS: [&str; 6] = [
    "sh11.width.0",
    "sh11.width.1",
    "sh11.width.2",
    "sh11.width.3",
    "sh11.width.4",
    "sh11.width.5",
];
/// Opacities (percent).
pub(crate) const OPACITIES: [u8; 4] = [25, 50, 75, 100];
const OPACITY_CMDS: [&str; 4] = [
    "sh11.opacity.0",
    "sh11.opacity.1",
    "sh11.opacity.2",
    "sh11.opacity.3",
];
/// Dash patterns: solid, dashed, dotted.
const DASH_CMDS: [&str; 3] = ["sh11.dash.0", "sh11.dash.1", "sh11.dash.2"];

fn dash_of(k: usize, width: f32) -> Vec<f32> {
    match k {
        1 => vec![4.0 * width.max(1.0), 2.0 * width.max(1.0)],
        2 => vec![width.max(1.0), 1.5 * width.max(1.0)],
        _ => Vec::new(),
    }
}

/// Our drawing of a shape in cell `r` (window pixels); `shaded`: a two-tone fill.
fn shape_icon(p: &mut dyn Painter, kind: ShapeKind, r: [f32; 4], shaded: bool, k: f32) {
    let ink = Rgba(0x40, 0x40, 0x40, 255);
    let light = Rgba(0xE6, 0xE6, 0xE6, 255);
    let (x0, y0, w, h) = (r[0], r[1], r[2], r[3]);
    let at = |u: f32, v: f32| (x0 + w * u, y0 + h * v);
    let poly = |pts: &[(f32, f32)], close: bool| {
        let mut v: Vec<PathSeg> = pts
            .iter()
            .enumerate()
            .map(|(i, (u, t))| {
                let (x, y) = at(*u, *t);
                if i == 0 {
                    PathSeg::M(x, y)
                } else {
                    PathSeg::L(x, y)
                }
            })
            .collect();
        if close {
            v.push(PathSeg::Z);
        }
        v
    };
    let path: Vec<PathSeg> = match kind {
        ShapeKind::Line => poly(&[(0.05, 0.5), (0.95, 0.5)], false),
        ShapeKind::Rectangle => poly(&[(0.1, 0.18), (0.9, 0.18), (0.9, 0.82), (0.1, 0.82)], true),
        ShapeKind::RoundedRectangle => {
            crate::reader_window::rounded(x0 + w * 0.06, y0 + h * 0.2, w * 0.88, h * 0.6, h * 0.18)
        }
        ShapeKind::Ellipse => {
            crate::editor_form8::circle(x0 + w / 2.0, y0 + h / 2.0, w.min(h) * 0.42)
        }
        ShapeKind::Connector => poly(
            &[(0.1, 0.15), (0.55, 0.15), (0.55, 0.85), (0.95, 0.85)],
            false,
        ),
        ShapeKind::Polygon => {
            let pts: Vec<(f32, f32)> = (0..6)
                .map(|i| {
                    let a = std::f32::consts::PI / 3.0 * i as f32;
                    (0.5 + 0.42 * a.cos(), 0.5 + 0.38 * a.sin())
                })
                .collect();
            poly(&pts, true)
        }
        ShapeKind::Curve => {
            let (a, b) = (at(0.08, 0.7), at(0.92, 0.35));
            let (c1, c2) = (at(0.35, 0.0), at(0.6, 1.1));
            vec![
                PathSeg::M(a.0, a.1),
                PathSeg::C(c1.0, c1.1, c2.0, c2.1, b.0, b.1),
            ]
        }
        ShapeKind::Arrow => poly(
            &[
                (0.08, 0.38),
                (0.55, 0.38),
                (0.55, 0.15),
                (0.94, 0.5),
                (0.55, 0.85),
                (0.55, 0.62),
                (0.08, 0.62),
            ],
            true,
        ),
        ShapeKind::Corner => poly(
            &[
                (0.12, 0.12),
                (0.45, 0.12),
                (0.45, 0.55),
                (0.9, 0.55),
                (0.9, 0.88),
                (0.12, 0.88),
            ],
            true,
        ),
    };
    let closed = !matches!(
        kind,
        ShapeKind::Line | ShapeKind::Connector | ShapeKind::Curve
    );
    let lw = (1.3 * k).max(1.0);
    if shaded && closed {
        // Our shading mark: dark on the left fading to light in three bands.
        p.path(&path, Some(light), None);
        for (i, g) in [0x78u8, 0x9C, 0xC0].iter().enumerate() {
            let band = w / 4.0;
            p.push_clip(x0 + band * i as f32, y0, band, h);
            p.path(&path, Some(Rgba(*g, *g, *g, 255)), None);
            p.pop_clip();
        }
        p.path(&path, None, Some((ink, lw)));
    } else if closed {
        p.path(&path, Some(light), Some((ink, lw)));
    } else if shaded {
        p.path(&path, None, Some((Rgba(0x90, 0x90, 0x90, 255), lw)));
    } else {
        p.path(&path, None, Some((ink, lw)));
    }
}

impl EditorWindow {
    /// Open (or close) the Add Shapes grid under its ribbon button.
    pub(crate) fn toggle_shapes11(&mut self) {
        if self.s11.grid.take().is_some() {
            return;
        }
        let Some(b) = self.r.control_rect("ed.addShapes") else {
            return;
        };
        let s = self.ui_scale() / 1.25;
        let (w, h) = ((GRID.0 * s).round(), (GRID.1 * s).round());
        let win = self.window_size();
        let x = b[0].min(win.0 - w - 2.0).max(0.0);
        self.s11.grid = Some([x, b[1] + b[3], w, h]);
        self.s11.hover = None;
    }

    fn cell_at11(&self, x: f32, y: f32) -> Option<(usize, usize)> {
        let g = self.s11.grid?;
        let s = self.ui_scale() / 1.25;
        for (row, ry) in ROW_Y.iter().enumerate() {
            for col in 0..row_kinds(row).len() {
                let c = [
                    g[0] + (CELL_X0 + CELL_PITCH * col as f32) * s,
                    g[1] + ry * s,
                    CELL * s,
                    CELL * s,
                ];
                if inside([c[0] - 2.0, c[1] - 2.0, c[2] + 4.0, c[3] + 4.0], x, y) {
                    return Some((row, col));
                }
            }
        }
        None
    }

    /// Input while the grid is open; true when it took the input.
    pub(crate) fn shapes11_input(&mut self, input: &Input) -> bool {
        let Some(g) = self.s11.grid else {
            return false;
        };
        match *input {
            Input::Move { x, y } => {
                let h = self.cell_at11(x, y);
                let changed = h != self.s11.hover;
                self.s11.hover = h;
                changed || inside(g, x, y)
            }
            Input::Press { x, y, .. } => {
                if let Some((row, col)) = self.cell_at11(x, y) {
                    self.s11.grid = None;
                    if let Some(kind) = row_kinds(row).get(col).copied() {
                        self.s11.style.shading = row == 1;
                        self.choose_shape11(kind);
                    }
                    return true;
                }
                self.s11.grid = None;
                // A press on the button itself only closes the grid.
                self.r
                    .control_rect("ed.addShapes")
                    .is_some_and(|b| inside(b, x, y))
                    || inside(g, x, y)
            }
            Input::Key(crate::Key::Escape, _) => {
                self.s11.grid = None;
                true
            }
            Input::Release { x, y } | Input::Drag { x, y } => inside(g, x, y),
            _ => false,
        }
    }

    /// Choose a shape tool (the Format pane shows its options).
    pub(crate) fn choose_shape11(&mut self, kind: ShapeKind) {
        self.tools.tool = None;
        self.tools_command(match kind {
            ShapeKind::Rectangle => "ed.shape.rect",
            ShapeKind::RoundedRectangle => "ed.shape.rounded",
            ShapeKind::Ellipse => "ed.shape.ellipse",
            ShapeKind::Line => "ed.shape.line",
            _ => "",
        });
        if !matches!(
            kind,
            ShapeKind::Rectangle
                | ShapeKind::RoundedRectangle
                | ShapeKind::Ellipse
                | ShapeKind::Line
        ) {
            self.set_box_tool11(BoxTool::Shape(kind));
        }
        self.sync_format();
    }

    /// The style a new shape of `kind` is drawn with (the comment colour stands for the line
    /// colour until the pane changes it, as before round 11).
    pub(crate) fn shape_style11(&self) -> ShapeStyle {
        self.s11.style.clone()
    }

    /// Pane commands (`sh11.*`).
    pub(crate) fn shapes11_command(&mut self, cmd: &str) -> bool {
        let Some(rest) = cmd.strip_prefix("sh11.") else {
            return false;
        };
        let (what, arg) = rest.split_once('.').unwrap_or((rest, ""));
        let idx = arg.parse::<usize>().ok();
        let st = &mut self.s11.style;
        match what {
            "stroke" => st.stroke = idx.and_then(|i| COLORS.get(i).copied()),
            "fill" => st.fill = idx.and_then(|i| COLORS.get(i).copied()),
            "width" => {
                if let Some(w) = idx.and_then(|i| WIDTHS.get(i)) {
                    let dashed = !st.dash.is_empty();
                    let dotted = st.dash.first() == st.dash.get(1).map(|v| v / 1.5).as_ref();
                    st.width = *w;
                    if dashed {
                        st.dash = dash_of(if dotted { 2 } else { 1 }, *w);
                    }
                }
            }
            "opacity" => {
                if let Some(o) = idx.and_then(|i| OPACITIES.get(i)) {
                    st.opacity = f32::from(*o) / 100.0;
                }
            }
            "dash" => st.dash = dash_of(idx.unwrap_or(0), st.width),
            _ => return false,
        }
        true
    }

    /// The Shape section of the Format pane while a shape tool is on.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint_shape_pane11(&mut self, p: &mut dyn Painter, x0: f32, y0: f32) {
        let k = self.ui_scale();
        let px = |v: f32| (v * k).round();
        let style = TextStyle {
            size: 16.0 / 1.375 * k,
            bold: false,
        };
        let sub = Rgba(118, 118, 118, 255);
        let sel = Rgba(0x2E, 0x6D, 0xAD, 255);
        let ty = y0 + px(46.0);
        self.section_heading(p, x0, ty, "pdfEditor11.shapeStyle");
        let st = self.s11.style.clone();
        let mut y = ty + px(30.0);
        let swatch_row = |p: &mut dyn Painter,
                          hits: &mut Vec<([f32; 4], &'static str)>,
                          y: f32,
                          cur: Option<[u8; 3]>,
                          cmds: &[&'static str; 8]| {
            for (i, cmd) in cmds.iter().enumerate() {
                let r = [x0 + px(27.0 + 30.0 * i as f32), y, px(22.0), px(22.0)];
                let color = COLORS.get(i);
                let chosen = match (cur, color) {
                    (Some(c), Some(o)) => c == *o,
                    (None, None) => true,
                    _ => false,
                };
                if chosen {
                    p.fill_rect(r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0, sel);
                }
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xC8, 0xC8, 0xC8, 255));
                if let Some(c) = color {
                    p.fill_rect(
                        r[0] + 1.0,
                        r[1] + 1.0,
                        r[2] - 2.0,
                        r[3] - 2.0,
                        Rgba(c[0], c[1], c[2], 255),
                    );
                } else {
                    p.fill_rect(
                        r[0] + 1.0,
                        r[1] + 1.0,
                        r[2] - 2.0,
                        r[3] - 2.0,
                        Rgba(255, 255, 255, 255),
                    );
                    p.path(
                        &[
                            PathSeg::M(r[0] + 3.0, r[1] + r[3] - 3.0),
                            PathSeg::L(r[0] + r[2] - 3.0, r[1] + 3.0),
                        ],
                        None,
                        Some((Rgba(0xE0, 0x20, 0x20, 255), 1.5)),
                    );
                }
                hits.push((r, *cmd));
            }
        };
        let mut hits = std::mem::take(&mut self.pane.hits);
        for (key, cur, cmds) in [
            ("pdfEditor11.lineColor", st.stroke, &STROKE_CMDS),
            ("pdfEditor11.fillColorPane", st.fill, &FILL_CMDS),
        ] {
            p.text(&self.t(key), x0 + px(27.0), y, style, sub);
            y += px(24.0);
            swatch_row(p, &mut hits, y, cur, cmds);
            y += px(40.0);
        }
        // Width, opacity and dash as rows of small text buttons.
        let mut row =
            |p: &mut dyn Painter, y: f32, labels: Vec<String>, cmds: &[&'static str], on: usize| {
                let mut x = x0 + px(27.0);
                for (i, l) in labels.iter().enumerate() {
                    let w = crate::editor_window::Estimate
                        .measure(l, style)
                        .0
                        .max(px(18.0))
                        + px(14.0);
                    let r = [x, y, w, px(26.0)];
                    if i == on {
                        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xE2, 0xE2, 0xE2, 255));
                    }
                    p.fill_rect(r[0], r[1], r[2], 1.0, Rgba(0xC8, 0xC8, 0xC8, 255));
                    p.fill_rect(
                        r[0],
                        r[1] + r[3] - 1.0,
                        r[2],
                        1.0,
                        Rgba(0xC8, 0xC8, 0xC8, 255),
                    );
                    p.fill_rect(r[0], r[1], 1.0, r[3], Rgba(0xC8, 0xC8, 0xC8, 255));
                    p.fill_rect(
                        r[0] + r[2] - 1.0,
                        r[1],
                        1.0,
                        r[3],
                        Rgba(0xC8, 0xC8, 0xC8, 255),
                    );
                    p.text(
                        l,
                        r[0] + px(7.0),
                        r[1] + px(4.0),
                        style,
                        Rgba(51, 51, 51, 255),
                    );
                    hits.push((r, cmds[i]));
                    x += w + px(6.0);
                }
            };
        p.text(
            &self.t("pdfEditor11.lineWidth"),
            x0 + px(27.0),
            y,
            style,
            sub,
        );
        y += px(24.0);
        let wi = WIDTHS
            .iter()
            .position(|w| (w - st.width).abs() < 0.01)
            .unwrap_or(1);
        row(
            p,
            y,
            WIDTHS.iter().map(|w| format!("{w}")).collect(),
            &WIDTH_CMDS,
            wi,
        );
        y += px(42.0);
        p.text(&self.t("pdfEditor11.opacity"), x0 + px(27.0), y, style, sub);
        y += px(24.0);
        let oi = OPACITIES
            .iter()
            .position(|o| (f32::from(*o) / 100.0 - st.opacity).abs() < 0.01)
            .unwrap_or(3);
        row(
            p,
            y,
            OPACITIES.iter().map(|o| format!("{o}%")).collect(),
            &OPACITY_CMDS,
            oi,
        );
        y += px(42.0);
        p.text(
            &self.t("pdfEditor11.lineStyle"),
            x0 + px(27.0),
            y,
            style,
            sub,
        );
        y += px(24.0);
        let di = if st.dash.is_empty() {
            0
        } else if (st.dash[0] - st.width.max(1.0)).abs() < 0.01 {
            2
        } else {
            1
        };
        row(
            p,
            y,
            vec![
                self.t("pdfEditor11.solid"),
                self.t("pdfEditor11.dashed"),
                self.t("pdfEditor11.dotted"),
            ],
            &DASH_CMDS,
            di,
        );
        self.pane.hits = hits;
    }

    /// The open grid (over the page and the ribbon).
    pub(crate) fn paint_shapes11(&mut self, p: &mut dyn Painter) {
        let Some(g) = self.s11.grid else { return };
        let k = self.ui_scale();
        let s = k / 1.25;
        let q = |v: f32| (v * s).round();
        // Frame and soft shadow as the round-9 menus.
        for (d, a) in [(3.0, 14u8), (2.0, 22), (1.0, 32)] {
            p.fill_rect(
                g[0] - d,
                g[1] - d + 1.0,
                g[2] + 2.0 * d,
                g[3] + 2.0 * d,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(g[0], g[1], g[2], g[3], Rgba(0xE0, 0xE0, 0xE0, 255));
        p.fill_rect(
            g[0] + 1.0,
            g[1] + 1.0,
            g[2] - 2.0,
            g[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let head = TextStyle {
            size: (15.0 * s).round(),
            bold: true,
        };
        for (row, key) in ["pdfEditor11.addPath", "pdfEditor11.addShading"]
            .iter()
            .enumerate()
        {
            p.text(
                &self.t(key),
                g[0] + q(12.0),
                g[1] + q(HEAD_Y[row]),
                head,
                Rgba(0x3B, 0x3B, 0x3B, 255),
            );
            for (col, kind) in row_kinds(row).iter().enumerate() {
                let c = [
                    g[0] + q(CELL_X0 + CELL_PITCH * col as f32),
                    g[1] + q(ROW_Y[row]),
                    q(CELL),
                    q(CELL),
                ];
                if self.s11.hover == Some((row, col)) {
                    p.fill_rect(
                        c[0] - 2.0,
                        c[1] - 2.0,
                        c[2] + 4.0,
                        c[3] + 4.0,
                        Rgba(0xE2, 0xE2, 0xE2, 255),
                    );
                }
                shape_icon(p, *kind, c, row == 1, s);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::editor_tests::{Fixed, Rec};
    use crate::editor_window::EditorWindow;
    use crate::Input;

    fn window() -> EditorWindow {
        let mut w = EditorWindow::new("en-US");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1100.0,
            },
            &Fixed,
        );
        let pdf = crate::editor_tests::pdf(1);
        let doc = bo_pdfread::PdfDocument::open(pdf, bo_pdfread::standard_fonts()).expect("pdf");
        w.open(doc, "s.pdf", None);
        w
    }

    #[test]
    fn grid_opens_and_a_cell_chooses_a_shading_tool() {
        let mut w = window();
        w.r.set_ribbon_tab(2, &Fixed);
        let mut rec = Rec::default();
        w.paint(&mut rec);
        w.toggle_shapes11();
        let g = w.s11.grid.expect("grid");
        // 326 × 129 at 125 % (scale 1.25 × 1.1 text factor is not applied here).
        let s = w.ui_scale() / 1.25;
        assert!((g[2] - (326.0 * s).round()).abs() < 1.0);
        let mut rec = Rec::default();
        w.paint(&mut rec);
        // Second row, sixth cell: a shaded arrow? cells: rect, rounded, ellipse, connector,
        // polygon, curve, arrow, corner → index 5 is the curve.
        let cx = g[0] + (4.0 + 36.0 * 6.0 + 15.0) * s;
        let cy = g[1] + (93.0 + 15.0) * s;
        w.handle(
            Input::Press {
                x: cx,
                y: cy,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        assert!(w.s11.grid.is_none());
        assert!(w.s11.style.shading);
        assert_eq!(
            w.tools.tool,
            Some(crate::editor_tools::BoxTool::Shape(
                bo_pdfread::ShapeKind::Arrow
            ))
        );
        // Pane commands change the style.
        assert!(w.shapes11_command("sh11.fill.2"));
        assert!(w.shapes11_command("sh11.width.3"));
        assert!(w.shapes11_command("sh11.opacity.1"));
        assert!(w.shapes11_command("sh11.dash.1"));
        let st = w.shape_style11();
        assert_eq!(st.fill, Some(super::COLORS[2]));
        assert!((st.width - 3.0).abs() < 1e-3 && (st.opacity - 0.5).abs() < 1e-3);
        assert_eq!(st.dash, vec![12.0, 6.0]);
        assert!(w.shapes11_command("sh11.stroke.none"));
        assert_eq!(w.shape_style11().stroke, None);
    }
}
