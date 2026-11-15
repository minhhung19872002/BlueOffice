//! BlueReader Comment tab drawing tools (round 3, a child of `reader_window`): pencil, line,
//! arrow, oval, polygon, polyline, callout, typewriter, stamps, measurements (distance,
//! perimeter, area), resizing the selected comment by its corner handles, and a properties
//! panel (colour, opacity, thickness, font size).
use super::{inside, ReaderWindow, TEXT};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, Menu, MenuItem};
use crate::{Input, Key, Painter, TextMeasure, TextStyle};
use bo_pdfread::{Annot, DrawStyle, Shape, StampKind};
use bo_render::{PathSeg, Rgba};

/// A drawing tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrawTool {
    Pencil,
    Line,
    Arrow,
    Oval,
    Polygon,
    Polyline,
    Callout,
    Typewriter,
    Stamp,
    Distance,
    Perimeter,
    Area,
}

impl DrawTool {
    fn from_command(cmd: &str) -> Option<Self> {
        Some(match cmd {
            "draw.pencil" => Self::Pencil,
            "draw.line" => Self::Line,
            "draw.arrow" => Self::Arrow,
            "draw.oval" => Self::Oval,
            "draw.polygon" => Self::Polygon,
            "draw.polyline" => Self::Polyline,
            "draw.callout" => Self::Callout,
            "draw.typewriter" => Self::Typewriter,
            "draw.stamp" => Self::Stamp,
            "draw.distance" => Self::Distance,
            "draw.perimeter" => Self::Perimeter,
            "draw.area" => Self::Area,
            _ => return None,
        })
    }

    fn command(self) -> &'static str {
        match self {
            Self::Pencil => "draw.pencil",
            Self::Line | Self::Arrow | Self::Oval | Self::Polygon | Self::Polyline => {
                "menu.drawing"
            }
            Self::Callout => "draw.callout",
            Self::Typewriter => "draw.typewriter",
            Self::Stamp => "menu.stamp",
            Self::Distance | Self::Perimeter | Self::Area => "menu.measure",
        }
    }

    /// Clicks add vertices; a double click or Enter finishes.
    fn by_clicks(self) -> bool {
        matches!(
            self,
            Self::Polygon | Self::Polyline | Self::Perimeter | Self::Area
        )
    }
}

/// Swatches, opacities, thicknesses and text sizes of the properties panel.
const COLORS: [[u8; 3]; 8] = [
    [229, 57, 53],
    [255, 152, 0],
    [255, 230, 0],
    [67, 160, 71],
    [30, 136, 229],
    [142, 36, 170],
    [33, 33, 33],
    [255, 255, 255],
];
const OPACITIES: [f32; 4] = [0.25, 0.5, 0.75, 1.0];
const WIDTHS: [f32; 5] = [0.5, 1.0, 2.0, 3.0, 5.0];
const SIZES: [f32; 6] = [8.0, 10.0, 12.0, 14.0, 18.0, 24.0];
const PANEL_BG: Rgba = Rgba(251, 251, 253, 255);
const PANEL_EDGE: Rgba = Rgba(199, 204, 217, 255);
const ACCENT: Rgba = Rgba(0x1f, 0x5f, 0xbf, 255);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Prop {
    Color([u8; 3]),
    Opacity(f32),
    Width(f32),
    Size(f32),
}

/// State of the drawing tools.
#[derive(Default)]
pub(crate) struct Draw {
    pub(crate) tool: Option<DrawTool>,
    style: Option<DrawStyle>,
    stamp: Option<StampKind>,
    /// Points of the drawing in progress (document page points) and its page.
    points: Vec<[f32; 2]>,
    page: usize,
    /// Pointer position for rubber bands (page points).
    cursor: Option<[f32; 2]>,
    dragging: bool,
    /// Resizing the selected comment: the comment, the fixed corner (page points) and the
    /// moving one.
    resize: Option<(Annot, [f32; 2], [f32; 2])>,
    /// Text asked for a callout / typewriter: dialog, page, shape.
    text: Option<(Dialog, usize, Shape)>,
    /// Properties panel shown and its hit areas.
    pub(crate) props: bool,
    prop_rects: Vec<(Prop, [f32; 4])>,
    panel: [f32; 4],
}

impl Draw {
    fn style(&self) -> DrawStyle {
        self.style.unwrap_or_default()
    }
}

fn rect_of(a: [f32; 2], b: [f32; 2]) -> [f32; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ]
}

impl ReaderWindow {
    /// Comment tab drawing commands; `true` when `cmd` was one.
    pub(crate) fn command_draw(&mut self, cmd: &str, _m: &dyn TextMeasure) -> bool {
        if cmd.starts_with("tool.") {
            // Another pointer tool replaces the drawing tool.
            self.set_draw_tool(None);
            return false;
        }
        if let Some(name) = cmd.strip_prefix("draw.stamp.") {
            self.draw.stamp = StampKind::ALL.into_iter().find(|s| s.name() == name);
            self.set_draw_tool(Some(DrawTool::Stamp));
            return true;
        }
        if let Some(tool) = DrawTool::from_command(cmd) {
            let again = self.draw.tool == Some(tool) && !matches!(tool, DrawTool::Stamp);
            self.set_draw_tool(if again { None } else { Some(tool) });
            return true;
        }
        match cmd {
            "menu.stamp" => {
                let items = StampKind::ALL
                    .iter()
                    .map(|s| MenuItem {
                        label: self.t(&format!("pdfDraw.stamp{}", s.name())),
                        choice: Choice::Command(format!("draw.stamp.{}", s.name())),
                        checked: self.draw.stamp == Some(*s),
                        dim: false,
                    })
                    .collect();
                self.open_draw_menu(items, "menu.stamp");
            }
            "menu.drawing" => {
                let items = [
                    ("tool.rectangle", "pdfReader.rectangle", None),
                    ("draw.oval", "pdfDraw.oval", Some(DrawTool::Oval)),
                    ("draw.line", "pdfDraw.line", Some(DrawTool::Line)),
                    ("draw.arrow", "pdfDraw.arrow", Some(DrawTool::Arrow)),
                    ("draw.polygon", "pdfDraw.polygon", Some(DrawTool::Polygon)),
                    (
                        "draw.polyline",
                        "pdfDraw.polyline",
                        Some(DrawTool::Polyline),
                    ),
                ]
                .iter()
                .map(|(c, k, t)| MenuItem {
                    label: self.t(k),
                    choice: Choice::Command((*c).to_owned()),
                    checked: t.is_some() && self.draw.tool == *t,
                    dim: false,
                })
                .collect();
                self.open_draw_menu(items, "menu.drawing");
            }
            "menu.measure" => {
                let items = [
                    ("draw.distance", "pdfDraw.distance", DrawTool::Distance),
                    ("draw.perimeter", "pdfDraw.perimeter", DrawTool::Perimeter),
                    ("draw.area", "pdfDraw.area", DrawTool::Area),
                ]
                .iter()
                .map(|(c, k, t)| MenuItem {
                    label: self.t(k),
                    choice: Choice::Command((*c).to_owned()),
                    checked: self.draw.tool == Some(*t),
                    dim: false,
                })
                .collect();
                self.open_draw_menu(items, "menu.measure");
            }
            "draw.props" => {
                self.draw.props = !self.draw.props;
                if self.draw.props {
                    self.on.insert("draw.props");
                } else {
                    self.on.remove("draw.props");
                }
            }
            _ => return false,
        }
        true
    }

    fn open_draw_menu(&mut self, items: Vec<MenuItem>, id: &'static str) {
        let r = self.control_rect(id).unwrap_or([0.0; 4]);
        self.menu = Some((Menu::open(items, r, self.size, self.scale), id));
    }

    fn set_draw_tool(&mut self, tool: Option<DrawTool>) {
        if let Some(old) = self.draw.tool {
            self.on.remove(old.command());
        }
        self.draw.tool = tool;
        self.draw.points.clear();
        self.draw.cursor = None;
        self.draw.dragging = false;
        self.tool_hidden = tool.is_some();
        if let Some(t) = tool {
            self.on.insert(t.command());
            self.extra.selected = None;
        }
    }

    /// The drawing tool in use (tests).
    #[cfg(test)]
    pub(crate) fn draw_tool(&self) -> Option<DrawTool> {
        self.draw.tool
    }

    fn doc_point(&self, x: f32, y: f32) -> Option<(usize, [f32; 2])> {
        let t = self.tabs.get(self.active)?;
        let (i, px, py) = t.view.window_to_doc(x, y)?;
        Some((i, [px, py]))
    }

    fn win_point(&self, page: usize, p: [f32; 2]) -> Option<(f32, f32)> {
        let t = self.tabs.get(self.active)?;
        let r = t.view.doc_rect_to_window(page, [p[0], p[1], p[0], p[1]])?;
        Some((r[0], r[1]))
    }

    /// Corner handle of the selected comment under a window point: the opposite (fixed)
    /// corner and the grabbed one, in page points.
    fn handle_at(&self, x: f32, y: f32) -> Option<([f32; 2], [f32; 2])> {
        let a = self.extra.selected.as_ref()?;
        let t = self.tabs.get(self.active)?;
        let w = t.view.doc_rect_to_window(a.page, a.rect)?;
        let reach = self.px(6.0);
        let r = a.rect;
        let corners = [
            ((w[0], w[1]), [r[0], r[1]], [r[2], r[3]]),
            ((w[0] + w[2], w[1]), [r[2], r[1]], [r[0], r[3]]),
            ((w[0], w[1] + w[3]), [r[0], r[3]], [r[2], r[1]]),
            ((w[0] + w[2], w[1] + w[3]), [r[2], r[3]], [r[0], r[1]]),
        ];
        corners
            .into_iter()
            .find(|((cx, cy), _, _)| (x - cx).abs() <= reach && (y - cy).abs() <= reach)
            .map(|(_, grab, fixed)| (fixed, grab))
    }

    /// Input of the drawing tools first: `Some(redraw)` when handled.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn route_draw(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        // Text of a callout / typewriter.
        if let Some((d, _, _)) = self.draw.text.as_mut() {
            let done = match input {
                Input::Text(s) => {
                    d.text(s);
                    None
                }
                Input::Key(k, mods) => d.key(*k, *mods),
                Input::Press { x, y, .. } if d.contains(*x, *y) => d.press(*x, *y),
                Input::Move { x, y } => {
                    d.hover(*x, *y);
                    None
                }
                Input::Resize { .. } | Input::Scale(_) | Input::Tick | Input::Blink => return None,
                _ => None,
            };
            if let Some(b) = done {
                if let Some((d, page, shape)) = self.draw.text.take() {
                    if b == DialogButton::Ok {
                        let text = d.values().into_iter().next().unwrap_or_default();
                        if !text.trim().is_empty() {
                            let shape = self.fit_text(shape, &text);
                            self.add_shape(page, &shape, &text);
                        }
                    }
                }
            } else if let Some((d, _, _)) = self.draw.text.as_mut() {
                d.layout(m, self.size, self.scale);
            }
            return Some(true);
        }
        if self.menu.is_some() || self.dialog.is_some() {
            return None;
        }
        // Properties panel.
        if self.draw.props {
            if let Input::Press { x, y, .. } = input {
                if inside(self.draw.panel, *x, *y) {
                    let hit = self
                        .draw
                        .prop_rects
                        .iter()
                        .find(|(_, r)| inside(*r, *x, *y))
                        .map(|(p, _)| *p);
                    if let Some(p) = hit {
                        self.apply_prop(p);
                    }
                    return Some(true);
                }
            }
        }
        // Resizing the selected comment by a corner handle.
        match input {
            Input::Press { x, y, .. } if self.draw.tool.is_none() => {
                if let Some((fixed, grab)) = self.handle_at(*x, *y) {
                    if let Some(a) = self.extra.selected.clone() {
                        self.draw.resize = Some((a, fixed, grab));
                        return Some(true);
                    }
                }
            }
            Input::Drag { x, y } | Input::Move { x, y } if self.draw.resize.is_some() => {
                if let Some((_, p)) = self.doc_point(*x, *y) {
                    if let Some(r) = self.draw.resize.as_mut() {
                        r.2 = p;
                    }
                }
                return Some(true);
            }
            Input::Release { .. } if self.draw.resize.is_some() => {
                if let Some((a, fixed, grab)) = self.draw.resize.take() {
                    let rect = rect_of(fixed, grab);
                    let page = a.page;
                    let ok = self
                        .tabs
                        .get_mut(self.active)
                        .map(|t| t.view.doc_mut().resize_annotation(&a, rect));
                    match ok {
                        Some(Ok(())) => {
                            self.document_changed();
                            self.reselect(page, rect);
                        }
                        Some(Err(e)) => self.status = e,
                        None => {}
                    }
                }
                return Some(true);
            }
            _ => {}
        }
        let tool = self.draw.tool?;
        match input {
            Input::Key(Key::Escape, _) => {
                if self.draw.points.is_empty() {
                    self.set_draw_tool(None);
                } else {
                    self.draw.points.clear();
                    self.draw.cursor = None;
                }
                Some(true)
            }
            Input::Key(Key::Enter, _) if tool.by_clicks() => {
                self.finish_draw();
                Some(true)
            }
            Input::Press { x, y, count, .. } if inside(self.doc_rect, *x, *y) => {
                let Some((page, p)) = self.doc_point(*x, *y) else {
                    return Some(false);
                };
                if tool.by_clicks() {
                    if *count >= 2 && !self.draw.points.is_empty() {
                        self.finish_draw();
                        return Some(true);
                    }
                    if self.draw.points.is_empty() {
                        self.draw.page = page;
                    }
                    if page == self.draw.page {
                        self.draw.points.push(p);
                    }
                } else {
                    self.draw.page = page;
                    self.draw.points = vec![p];
                    self.draw.cursor = Some(p);
                    self.draw.dragging = true;
                }
                Some(true)
            }
            Input::Drag { x, y } | Input::Move { x, y }
                if self.draw.dragging || tool.by_clicks() && !self.draw.points.is_empty() =>
            {
                let Some((page, p)) = self.doc_point(*x, *y) else {
                    return Some(false);
                };
                if page != self.draw.page {
                    return Some(false);
                }
                if tool == DrawTool::Pencil && self.draw.dragging {
                    let far = self
                        .draw
                        .points
                        .last()
                        .is_none_or(|q| (q[0] - p[0]).abs() + (q[1] - p[1]).abs() > 0.75);
                    if far {
                        self.draw.points.push(p);
                    }
                }
                self.draw.cursor = Some(p);
                Some(true)
            }
            Input::Release { .. } if self.draw.dragging => {
                self.draw.dragging = false;
                self.finish_draw();
                Some(true)
            }
            _ => None,
        }
    }

    /// The comment drawn so far as a shape (with the pointer as its last point).
    fn current_shape(&self) -> Option<Shape> {
        let tool = self.draw.tool?;
        let pts = &self.draw.points;
        let a = *pts.first()?;
        let b = self.draw.cursor.unwrap_or(a);
        let far = (a[0] - b[0]).abs().max((a[1] - b[1]).abs()) > 3.0;
        let boxed = |w: f32, h: f32| {
            if far {
                rect_of(a, b)
            } else {
                [a[0], a[1], a[0] + w, a[1] + h]
            }
        };
        let mut poly = pts.clone();
        if let Some(c) = self.draw.cursor.filter(|_| tool.by_clicks()) {
            poly.push(c);
        }
        Some(match tool {
            DrawTool::Pencil => Shape::Ink(vec![pts.clone()]),
            DrawTool::Line if far => Shape::Line { a, b, arrow: false },
            DrawTool::Arrow if far => Shape::Line { a, b, arrow: true },
            DrawTool::Distance if far => Shape::Distance(a, b),
            DrawTool::Line | DrawTool::Arrow | DrawTool::Distance => return None,
            DrawTool::Oval => Shape::Oval(boxed(72.0, 48.0)),
            DrawTool::Polygon => Shape::Polygon(poly),
            DrawTool::Polyline => Shape::Polyline(poly),
            DrawTool::Perimeter => Shape::Perimeter(poly),
            DrawTool::Area => Shape::Area(poly),
            DrawTool::Callout => {
                let r = boxed(150.0, 40.0);
                Shape::Callout {
                    rect: r,
                    tip: [r[0] - 40.0, r[3] + 40.0],
                }
            }
            DrawTool::Typewriter => Shape::Typewriter(boxed(200.0, 16.0)),
            DrawTool::Stamp => {
                let r = if far {
                    rect_of(a, b)
                } else {
                    [a[0] - 75.0, a[1] - 22.0, a[0] + 75.0, a[1] + 22.0]
                };
                Shape::Stamp(r, self.draw.stamp.unwrap_or(StampKind::Approved))
            }
        })
    }

    fn finish_draw(&mut self) {
        let Some(tool) = self.draw.tool else { return };
        if tool.by_clicks() {
            self.draw.cursor = None;
        }
        let shape = self.current_shape();
        let page = self.draw.page;
        self.draw.points.clear();
        self.draw.cursor = None;
        let Some(shape) = shape else { return };
        let enough = match &shape {
            Shape::Polygon(p) | Shape::Area(p) => p.len() >= 3,
            Shape::Polyline(p) | Shape::Perimeter(p) => p.len() >= 2,
            Shape::Ink(s) => s.first().is_some_and(|v| !v.is_empty()),
            _ => true,
        };
        if !enough {
            return;
        }
        if matches!(tool, DrawTool::Callout | DrawTool::Typewriter) {
            let title = self.t(if tool == DrawTool::Callout {
                "pdfDraw.callout"
            } else {
                "pdfDraw.typewriter"
            });
            let mut d = Dialog::new(
                DialogKind::Comment,
                title,
                vec![(self.t("pdfExtra.text"), String::new())],
                vec![
                    (self.t("pdfReader.ok"), DialogButton::Ok),
                    (self.t("pdfPrint.cancel"), DialogButton::Cancel),
                ],
                Vec::new(),
            );
            d.layout(&EstimateMeasure, self.size, self.scale);
            self.draw.text = Some((d, page, shape));
            return;
        }
        self.add_shape(page, &shape, "");
    }

    /// A text comment's box grown to its text.
    fn fit_text(&self, shape: Shape, text: &str) -> Shape {
        let size = self.draw.style().font_size;
        let lines = text.lines().count().max(1) as f32;
        let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
        match shape {
            Shape::Typewriter(r) => Shape::Typewriter([
                r[0],
                r[1],
                r[0] + longest * size * 0.55 + 4.0,
                r[1] + lines * size * 1.2 + 4.0,
            ]),
            Shape::Callout { rect, tip } => Shape::Callout {
                rect: [
                    rect[0],
                    rect[1],
                    rect[2],
                    rect[3].max(rect[1] + lines * size * 1.2 + 8.0),
                ],
                tip,
            },
            s => s,
        }
    }

    fn add_shape(&mut self, page: usize, shape: &Shape, text: &str) {
        let mut style = self.draw.style();
        if let Shape::Stamp(_, k) = shape {
            style.color = k.color();
            style.width = 2.0;
        }
        let author = self.author.clone();
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        match t
            .view
            .doc_mut()
            .add_drawing(page, shape, &style, &author, text)
        {
            Ok(()) => self.document_changed(),
            Err(e) => self.status = e,
        }
    }

    /// Select the comment of `page` whose box is closest to `rect`.
    fn reselect(&mut self, page: usize, rect: [f32; 4]) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let d = |a: &Annot| {
            a.rect
                .iter()
                .zip(rect)
                .map(|(x, y)| (x - y).abs())
                .sum::<f32>()
        };
        self.extra.selected = t
            .view
            .doc()
            .annotations()
            .into_iter()
            .filter(|a| a.page == page)
            .min_by(|a, b| d(a).total_cmp(&d(b)));
    }

    fn apply_prop(&mut self, p: Prop) {
        let mut style = self.draw.style();
        match p {
            Prop::Color(c) => style.color = c,
            Prop::Opacity(o) => style.opacity = o,
            Prop::Width(w) => style.width = w,
            Prop::Size(s) => style.font_size = s,
        }
        self.draw.style = Some(style);
        // The selected comment takes the change too.
        let Some(a) = self.extra.selected.clone() else {
            return;
        };
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let doc = t.view.doc_mut();
        let r = match doc.drawing(&a) {
            Some((shape, mut s, text)) => {
                match p {
                    Prop::Color(c) => s.color = c,
                    Prop::Opacity(o) => s.opacity = o,
                    Prop::Width(w) => s.width = w,
                    Prop::Size(z) => s.font_size = z,
                }
                doc.update_drawing(&a, &shape, &s, &text)
            }
            None => match p {
                Prop::Color(c) => doc.set_annotation_color(&a, c),
                _ => Ok(()),
            },
        };
        match r {
            Ok(()) => {
                self.document_changed();
                self.reselect(a.page, a.rect);
            }
            Err(e) => self.status = e,
        }
    }

    /// Drawing in progress, the comment being resized and the properties panel.
    pub(crate) fn paint_draw(&mut self, p: &mut dyn Painter) {
        let d = self.doc_rect;
        p.push_clip(d[0], d[1], d[2], d[3]);
        if let Some(shape) = self.current_shape() {
            let style = self.draw.style();
            let [r, g, b] = style.color;
            let c = Rgba(r, g, b, (style.opacity * 255.0) as u8);
            let s = self
                .tabs
                .get(self.active)
                .map_or(1.0, |t| t.view.px_per_pt());
            let w = (style.width * s).max(1.0);
            let page = self.draw.page;
            let pts = |v: &[[f32; 2]]| -> Vec<PathSeg> {
                v.iter()
                    .enumerate()
                    .filter_map(|(k, q)| {
                        let (x, y) = self.win_point(page, *q)?;
                        Some(if k == 0 {
                            PathSeg::M(x, y)
                        } else {
                            PathSeg::L(x, y)
                        })
                    })
                    .collect()
            };
            let boxed = |q: [f32; 4]| {
                pts(&[
                    [q[0], q[1]],
                    [q[2], q[1]],
                    [q[2], q[3]],
                    [q[0], q[3]],
                    [q[0], q[1]],
                ])
            };
            let segs = match &shape {
                Shape::Ink(s) => pts(s.first().map_or(&[][..], Vec::as_slice)),
                Shape::Line { a, b, .. } | Shape::Distance(a, b) => pts(&[*a, *b]),
                Shape::Polyline(v) | Shape::Perimeter(v) => pts(v),
                Shape::Polygon(v) | Shape::Area(v) => {
                    let mut v = v.clone();
                    if let Some(f) = v.first().copied() {
                        v.push(f);
                    }
                    pts(&v)
                }
                Shape::Oval(q) | Shape::Typewriter(q) | Shape::Stamp(q, _) | Shape::Rect(q) => {
                    boxed(*q)
                }
                Shape::Callout { rect, tip } => {
                    let mut s = boxed(*rect);
                    s.extend(pts(&[[rect[0], f32::midpoint(rect[1], rect[3])], *tip]));
                    s
                }
            };
            p.path(&segs, None, Some((c, w)));
            if let Some(label) = bo_pdfread::measure_label(&shape) {
                if let Some(q) = self.draw.cursor.and_then(|q| self.win_point(page, q)) {
                    let st = TextStyle {
                        size: self.px(9.0),
                        bold: false,
                    };
                    p.fill_rect(
                        q.0 + 8.0,
                        q.1 + 8.0,
                        label.len() as f32 * st.size * 0.6 + 8.0,
                        st.size * 1.6,
                        Rgba(255, 255, 225, 255),
                    );
                    p.text(&label, q.0 + 12.0, q.1 + 9.0, st, TEXT);
                }
            }
        }
        if let Some((a, fixed, grab)) = &self.draw.resize {
            let r = rect_of(*fixed, *grab);
            if let Some(w) = self
                .tabs
                .get(self.active)
                .and_then(|t| t.view.doc_rect_to_window(a.page, r))
            {
                let k = self.scale.max(1.0).round();
                for (x, y, ww, hh) in [
                    (w[0], w[1], w[2], k),
                    (w[0], w[1] + w[3] - k, w[2], k),
                    (w[0], w[1], k, w[3]),
                    (w[0] + w[2] - k, w[1], k, w[3]),
                ] {
                    p.fill_rect(x, y, ww, hh, ACCENT);
                }
            }
        }
        p.pop_clip();
        self.paint_props(p);
    }

    /// The properties panel at the right of the document area.
    fn paint_props(&mut self, p: &mut dyn Painter) {
        self.draw.prop_rects.clear();
        if !self.draw.props || self.show_start || self.tabs.is_empty() {
            self.draw.panel = [0.0; 4];
            return;
        }
        let d = self.doc_rect;
        let k = self.scale;
        let s = move |v: f32| (v * k).round();
        let w = s(196.0);
        let x0 = d[0] + d[2] - w - s(8.0);
        let y0 = d[1] + s(8.0);
        let st = TextStyle {
            size: s(9.5),
            bold: false,
        };
        let row_h = s(22.0);
        let rows = 8.0;
        let h = s(10.0) + rows * row_h;
        self.draw.panel = [x0, y0, w, h];
        p.fill_rect(x0 - 1.0, y0 - 1.0, w + 2.0, h + 2.0, PANEL_EDGE);
        p.fill_rect(x0, y0, w, h, PANEL_BG);
        let style = self.draw.style();
        let mut y = y0 + s(5.0);
        let cell = s(20.0);
        let gap = s(3.0);
        let label = |p: &mut dyn Painter, key: &str, y: f32| {
            p.text(&self.t(key), x0 + s(8.0), y + s(3.0), st, TEXT);
        };
        let mut rects = Vec::new();
        // Colour: two rows of swatches.
        label(p, "pdfDraw.color", y);
        y += row_h;
        for (k, c) in COLORS.iter().enumerate() {
            let r = [
                x0 + s(8.0) + k as f32 * (cell + gap),
                y,
                cell,
                cell - s(2.0),
            ];
            let on = style.color == *c;
            p.fill_rect(
                r[0] - 1.0,
                r[1] - 1.0,
                r[2] + 2.0,
                r[3] + 2.0,
                if on { ACCENT } else { PANEL_EDGE },
            );
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(c[0], c[1], c[2], 255));
            rects.push((Prop::Color(*c), r));
        }
        y += row_h;
        let chips = |p: &mut dyn Painter,
                     y: f32,
                     items: &[(Prop, String, bool)],
                     rects: &mut Vec<(Prop, [f32; 4])>| {
            let cw = s(42.0);
            for (k, (prop, text, on)) in items.iter().enumerate() {
                let r = [x0 + s(8.0) + k as f32 * (cw + gap), y, cw, cell - s(2.0)];
                p.fill_rect(
                    r[0] - 1.0,
                    r[1] - 1.0,
                    r[2] + 2.0,
                    r[3] + 2.0,
                    if *on { ACCENT } else { PANEL_EDGE },
                );
                p.fill_rect(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    if *on {
                        Rgba(204, 228, 247, 255)
                    } else {
                        Rgba(255, 255, 255, 255)
                    },
                );
                p.text(text, r[0] + s(5.0), r[1] + s(2.0), st, TEXT);
                rects.push((*prop, r));
            }
        };
        label(p, "pdfDraw.opacity", y);
        y += row_h;
        let items: Vec<(Prop, String, bool)> = OPACITIES
            .iter()
            .map(|o| {
                (
                    Prop::Opacity(*o),
                    format!("{}%", (o * 100.0) as u32),
                    (style.opacity - o).abs() < 0.01,
                )
            })
            .collect();
        chips(p, y, &items, &mut rects);
        y += row_h;
        label(p, "pdfDraw.thickness", y);
        y += row_h;
        let items: Vec<(Prop, String, bool)> = WIDTHS
            .iter()
            .take(4)
            .map(|w| {
                (
                    Prop::Width(*w),
                    format!("{w} pt"),
                    (style.width - w).abs() < 0.01,
                )
            })
            .collect();
        chips(p, y, &items, &mut rects);
        y += row_h;
        label(p, "pdfDraw.fontSize", y);
        y += row_h;
        let items: Vec<(Prop, String, bool)> = SIZES
            .iter()
            .skip(1)
            .take(4)
            .map(|z| {
                (
                    Prop::Size(*z),
                    format!("{z}"),
                    (style.font_size - z).abs() < 0.01,
                )
            })
            .collect();
        chips(p, y, &items, &mut rects);
        self.draw.prop_rects = rects;
    }

    /// The text dialog of a callout / typewriter.
    pub(crate) fn paint_draw_dialog(&self, p: &mut dyn Painter) {
        if let Some((d, _, _)) = &self.draw.text {
            d.paint(p, &self.theme, self.size, true);
        }
    }
}

/// Text widths estimated from the size (dialog layout outside an input).
struct EstimateMeasure;

impl TextMeasure for EstimateMeasure {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.5,
            style.size * 1.3,
        )
    }
}
