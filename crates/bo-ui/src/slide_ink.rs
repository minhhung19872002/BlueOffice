//! Drawing on the slide with BlueSlide's Draw tab (N6, round 3): pens and a highlighter
//! leave ink strokes, the eraser removes the strokes it touches. Strokes are collected in
//! slide points while the pointer is down, drawn live over the slide image, and handed to
//! the engine's ink commands on release (`slide_edit::ink`: one InkML content part per
//! stroke, [MS-ODRAWXML] §1.3.2; ECMA-376 Part 1, §19.3.1.14 contentPart), one undo step
//! each.
use super::SlideView;
use crate::{Input, Painter};
use bo_engine::slide_edit::ink::{EraseMode, InkCommand, InkStrokeIn};
use bo_render::{PathSeg, Rgba};

/// A Draw tab tool.
#[derive(Debug, Clone, PartialEq)]
pub enum InkTool {
    /// A pen or the highlighter: colour `RRGGBB`, width in points.
    Pen {
        /// `RRGGBB`.
        color: String,
        /// Width in points.
        width: f64,
        /// Highlighter (wide, translucent, drawn under text).
        highlighter: bool,
    },
    /// Removes the strokes it touches.
    Eraser,
}

impl InkTool {
    /// The tools of the Draw tab by command id (own choice of colours: black and red pens,
    /// a yellow highlighter), `None` for Select.
    #[must_use]
    pub fn of(id: &str) -> Option<InkTool> {
        let pen = |c: &str, w: f64, h: bool| InkTool::Pen {
            color: c.to_owned(),
            width: w,
            highlighter: h,
        };
        Some(match id {
            "draw.pen" => pen("000000", 1.0, false),
            "draw.red" => pen("E71224", 1.0, false),
            "draw.highlight" => pen("FFFF00", 6.0, true),
            "draw.eraser" => InkTool::Eraser,
            _ => return None,
        })
    }
}

/// Eraser radius in points.
const ERASER: f64 = 6.0;

fn rgb(hex: &str) -> Rgba {
    let v = u32::from_str_radix(hex, 16).unwrap_or(0);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

impl SlideView {
    /// Choose a Draw tab tool (`None`: back to selecting shapes).
    pub fn set_ink_tool(&mut self, tool: Option<InkTool>) {
        self.ink_tool = tool;
        self.ink_stroke.clear();
    }

    /// The Draw tab tool in use.
    pub fn ink_tool(&self) -> Option<&InkTool> {
        self.ink_tool.as_ref()
    }

    /// Pointer input while a Draw tool is active on the slide; `None` when the input is not
    /// for the ink (other panes, keys).
    pub(super) fn ink_input(&mut self, input: &Input) -> Option<bool> {
        let tool = self.ink_tool.clone()?;
        if self.sorter {
            return None;
        }
        match *input {
            Input::Press { x, y, .. } => {
                let c = self.canvas_rect();
                if !(x >= c[0] && x < c[0] + c[2] && y >= c[1] && y < c[1] + c[3]) {
                    return None;
                }
                let (sx, sy) = self.to_slide(x, y);
                self.ink_stroke = vec![[sx, sy]];
                Some(true)
            }
            Input::Drag { x, y } if !self.ink_stroke.is_empty() => {
                let (sx, sy) = self.to_slide(x, y);
                // Skip points closer than a device pixel to the last one.
                let last = self.ink_stroke[self.ink_stroke.len() - 1];
                let k = f64::from(self.scale());
                if ((sx - last[0]) * k).hypot((sy - last[1]) * k) < 1.0 {
                    return Some(false);
                }
                self.ink_stroke.push([sx, sy]);
                Some(true)
            }
            Input::Release { .. } if !self.ink_stroke.is_empty() => {
                let points = std::mem::take(&mut self.ink_stroke);
                let cmd = match tool {
                    InkTool::Pen {
                        color,
                        width,
                        highlighter,
                    } => {
                        // A click leaves a dot.
                        let mut points = points;
                        if points.len() == 1 {
                            let p = points[0];
                            points.push([p[0] + 0.01, p[1]]);
                        }
                        InkCommand::AddInk {
                            stroke: InkStrokeIn {
                                points,
                                pressure: None,
                                times: None,
                                color,
                                width,
                                highlighter,
                            },
                            name: String::new(),
                            auto_shape: false,
                            attach_to: None,
                        }
                    }
                    InkTool::Eraser => InkCommand::EraseInk {
                        points,
                        radius: ERASER,
                        mode: EraseMode::Stroke,
                    },
                };
                match self.engine.slide_ink_command(cmd) {
                    Ok(r) => self.apply(r),
                    Err(_) => return Some(true),
                }
                Some(true)
            }
            _ => None,
        }
    }

    /// The stroke being drawn, over the slide.
    pub(super) fn paint_ink(&self, p: &mut dyn Painter) {
        let Some(tool) = self.ink_tool.as_ref() else {
            return;
        };
        if self.ink_stroke.is_empty() {
            return;
        }
        let k = self.scale();
        let (color, width) = match tool {
            InkTool::Pen {
                color,
                width,
                highlighter,
            } => {
                let mut c = rgb(color);
                if *highlighter {
                    c.3 = 128;
                }
                (c, (*width as f32 * k).max(1.0))
            }
            InkTool::Eraser => (Rgba(0x80, 0x80, 0x80, 160), (ERASER as f32 * k).max(1.0)),
        };
        let segs: Vec<PathSeg> = self
            .ink_stroke
            .iter()
            .enumerate()
            .map(|(i, pt)| {
                let (x, y) = self.to_px(pt[0], pt[1]);
                if i == 0 {
                    PathSeg::M(x, y)
                } else {
                    PathSeg::L(x, y)
                }
            })
            .collect();
        p.path(&segs, None, Some((color, width)));
    }
}
