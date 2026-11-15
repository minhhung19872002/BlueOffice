//! Table Design > Draw Borders, as the reference presentation app lays it out (N6, round 5):
//! the pen style and pen weight as boxes showing the line, Pen Color, and the Draw Table and
//! Eraser tools — Draw Table drags out a new table on the slide, Eraser removes the border
//! between the clicked cell and its right-hand neighbour (the two cells merge).
use super::SlideWindow;
use crate::ribbon::RibbonHit;
use crate::{Input, Key, Painter};
use bo_engine::slide_edit::geom::Rect;
use bo_engine::slide_edit::SlideCommand;
use bo_render::{PathSeg, Rgba};

/// The table tool in use.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TableTool {
    /// None.
    #[default]
    Off,
    /// Draw Table: a drag from this window point, when started.
    Draw(Option<(f32, f32, f32, f32)>),
    /// Eraser.
    Erase,
}

/// Widths (logical px) of the pen style / weight boxes (reference: 112 device px at 125 %).
pub fn row_width(id: &str) -> Option<f32> {
    matches!(id, "sl.penStyle" | "sl.penWeight").then_some(96.0)
}

impl SlideWindow {
    /// Run Draw Table / Eraser; `None` when `id` is not one of them.
    pub(super) fn table_tool_action(&mut self, id: &str) -> Option<bool> {
        let want = match id {
            "sl.drawTable" => TableTool::Draw(None),
            "sl.eraser" => TableTool::Erase,
            _ => return None,
        };
        let same = std::mem::discriminant(&self.table_tool) == std::mem::discriminant(&want);
        self.table_tool = if same { TableTool::Off } else { want };
        Some(true)
    }

    /// Commands shown as on.
    pub(super) fn table_tool_toggled(&self, on: &mut Vec<&'static str>) {
        match self.table_tool {
            TableTool::Draw(_) => on.push("sl.drawTable"),
            TableTool::Erase => on.push("sl.eraser"),
            TableTool::Off => {}
        }
    }

    fn slide_point(&self, x: f32, y: f32) -> (f64, f64) {
        let (ox, oy) = self.view.slide_origin();
        let k = self.view.scale();
        (f64::from((x - ox) / k), f64::from((y - oy) / k))
    }

    /// Pointer input on the slide while a table tool is on.
    pub(super) fn table_tool_input(&mut self, input: &Input) -> Option<bool> {
        if self.table_tool == TableTool::Off {
            return None;
        }
        let area = self.view.canvas_rect();
        match *input {
            Input::Key(Key::Escape, _) => {
                self.table_tool = TableTool::Off;
                Some(true)
            }
            Input::Press { x, y, .. } if crate::spin_box::inside(area, x, y) => {
                match self.table_tool {
                    TableTool::Draw(_) => {
                        self.table_tool = TableTool::Draw(Some((x, y, x, y)));
                        Some(true)
                    }
                    TableTool::Erase => Some(self.erase_at(x, y)),
                    TableTool::Off => None,
                }
            }
            Input::Drag { x, y } => match self.table_tool {
                TableTool::Draw(Some((x0, y0, _, _))) => {
                    self.table_tool = TableTool::Draw(Some((x0, y0, x, y)));
                    Some(true)
                }
                _ => None,
            },
            Input::Release { x, y } => match self.table_tool {
                TableTool::Draw(Some((x0, y0, _, _))) => {
                    self.table_tool = TableTool::Draw(None);
                    let (a, b) = (self.slide_point(x0, y0), self.slide_point(x, y));
                    let rect = Rect {
                        x: a.0.min(b.0),
                        y: a.1.min(b.1),
                        w: (a.0 - b.0).abs(),
                        h: (a.1 - b.1).abs(),
                    };
                    if rect.w < 4.0 || rect.h < 4.0 {
                        return Some(true);
                    }
                    self.activate_context = true;
                    Some(self.view.run(SlideCommand::InsertTable {
                        rows: 1,
                        cols: 1,
                        rect: Some(rect),
                    }))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Eraser: merge the clicked cell with the one on its right.
    fn erase_at(&mut self, x: f32, y: f32) -> bool {
        let (sx, sy) = self.slide_point(x, y);
        if !self.view.run(SlideCommand::TableHit {
            x: sx,
            y: sy,
            extend: false,
        }) {
            return false;
        }
        let Some(t) = self.view.state().and_then(|s| s.table.clone()) else {
            return false;
        };
        let [r0, c0, _, _] = t.sel;
        if c0 + 1 >= t.cols {
            return false;
        }
        self.view.run(SlideCommand::TableSelect {
            id: Some(t.id),
            r0,
            c0,
            r1: Some(r0),
            c1: Some(c0 + 1),
        }) && self.view.run(SlideCommand::TableMerge)
    }

    /// The dragged table outline, and the pen style / weight boxes with their line.
    pub(super) fn paint_table_tools(&self, p: &mut dyn Painter) {
        if let TableTool::Draw(Some((x0, y0, x1, y1))) = self.table_tool {
            let ink = Rgba(0x40, 0x40, 0x40, 255);
            let (l, t, r, b) = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
            p.path(
                &[
                    PathSeg::M(l, t),
                    PathSeg::L(r, t),
                    PathSeg::L(r, b),
                    PathSeg::L(l, b),
                    PathSeg::Z,
                ],
                None,
                Some((ink, 1.0)),
            );
        }
        for id in ["sl.penStyle", "sl.penWeight"] {
            let Some(r) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
                continue;
            };
            let h = self.px(22.4);
            let b = [
                r[0] + self.px(2.0),
                r[1] + (r[3] - h) / 2.0,
                r[2] - self.px(4.0),
                h,
            ];
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xf3, 0xf3, 0xf3, 255));
            p.fill_rect(b[0], b[1], b[2], b[3], Rgba(0xab, 0xab, 0xab, 255));
            p.fill_rect(
                b[0] + 1.0,
                b[1] + 1.0,
                b[2] - 2.0,
                b[3] - 2.0,
                Rgba(255, 255, 255, 255),
            );
            // Drop-down arrow at the right, the line in the rest.
            let aw = self.px(14.0);
            let (ax, ay) = (b[0] + b[2] - aw / 2.0 - 1.0, b[1] + b[3] / 2.0);
            let k = self.px(3.0);
            p.path(
                &[
                    PathSeg::M(ax - k, ay - k / 2.0),
                    PathSeg::L(ax + k, ay - k / 2.0),
                    PathSeg::L(ax, ay + k / 2.0),
                    PathSeg::Z,
                ],
                Some(self.theme.text),
                None,
            );
            p.fill_rect(
                b[0] + b[2] - aw - 1.0,
                b[1] + 2.0,
                1.0,
                b[3] - 4.0,
                Rgba(0xd2, 0xd2, 0xd2, 255),
            );
            let (lx, lw) = (b[0] + self.px(5.0), b[2] - aw - self.px(10.0));
            let ink = Rgba(0x26, 0x26, 0x26, 255);
            let width = (self.pen.width as f32 * self.scale * 4.0 / 3.0)
                .clamp(1.0, h / 2.0)
                .round();
            let ly = (b[1] + (b[3] - width) / 2.0).round();
            if id == "sl.penWeight" {
                let label = format!("{} pt", self.pen.width);
                let style = crate::TextStyle {
                    size: self.px(11.2),
                    bold: false,
                };
                let tw = crate::slide_pickers::estimate(&label, style) + self.px(4.0);
                p.text(
                    &label,
                    lx,
                    b[1] + (b[3] - style.size * 1.33) / 2.0,
                    style,
                    ink,
                );
                p.fill_rect(lx + tw, ly, (lw - tw).max(0.0), width, ink);
            } else {
                // Dashes of the pen's style.
                let (on, off) = match self.pen.dash {
                    "dash" => (self.px(6.0), self.px(3.0)),
                    "sysDot" => (self.px(1.5), self.px(1.5)),
                    "sysDash" => (self.px(3.0), self.px(1.5)),
                    _ => (lw, 0.0),
                };
                let mut t = 0.0;
                while t < lw {
                    p.fill_rect(lx + t, ly, on.min(lw - t), width, ink);
                    t += on + off.max(0.0);
                    if off <= 0.0 {
                        break;
                    }
                }
            }
        }
    }
}
