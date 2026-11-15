//! BlueEditor's comment tools beyond the reader's (as in the reference editor's Comment tab):
//! typewriter, callout, drawing (rectangle, ellipse, line, arrow, polygon, cloud), pencil,
//! stamps (our own designs and words) and file attachments. Colour, opacity and line width are
//! set in the Format pane while a tool is on. Shapes are dragged; polygons and clouds take a
//! click per corner and a double click to finish; typewriter, callout and stamp ask for or
//! use their text.
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::editor_actions::Ask2;
use crate::editor_window::{inside, EditorWindow};
use crate::reader_window::{ReaderRequest, Tool};
use crate::{Input, Painter};
use bo_pdfread::{Markup, MarkupKind};
use bo_render::{PathSeg, Rgba};

/// A comment tool of the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommentTool {
    Typewriter,
    Callout,
    Rectangle,
    Ellipse,
    Line,
    Arrow,
    Polygon,
    Cloud,
    Pencil,
    Stamp,
    File,
}

/// Stamps: label key.
pub(crate) const STAMPS: [&str; 6] = [
    "pdfEditor.stamps.approved",
    "pdfEditor.stamps.draft",
    "pdfEditor.stamps.confidential",
    "pdfEditor.stamps.final",
    "pdfEditor.stamps.notApproved",
    "pdfEditor.stamps.forComment",
];

/// Comment tool state and the properties of new comments.
#[derive(Debug, Clone)]
pub(crate) struct CommentState {
    pub tool: Option<CommentTool>,
    pub color: [u8; 3],
    pub opacity: f32,
    pub width: f32,
    /// Stamp placed by the stamp tool (index into [`STAMPS`]).
    pub stamp: usize,
    /// Page and points of the shape being drawn (page points).
    pub points: Option<(usize, Vec<[f32; 2]>)>,
    /// Where a picked file goes: page and point.
    pub file_at: Option<(usize, [f32; 2])>,
}

impl Default for CommentState {
    fn default() -> Self {
        Self {
            tool: None,
            color: [0xE5, 0x39, 0x35],
            opacity: 1.0,
            width: 2.0,
            stamp: 0,
            points: None,
            file_at: None,
        }
    }
}

fn tool_of(cmd: &str) -> Option<CommentTool> {
    Some(match cmd {
        "cm.typewriter" => CommentTool::Typewriter,
        "cm.callout" => CommentTool::Callout,
        "cm.rect" => CommentTool::Rectangle,
        "cm.ellipse" => CommentTool::Ellipse,
        "cm.line" => CommentTool::Line,
        "cm.arrow" => CommentTool::Arrow,
        "cm.polygon" => CommentTool::Polygon,
        "cm.cloud" => CommentTool::Cloud,
        "cm.pencil" => CommentTool::Pencil,
        "cm.file" => CommentTool::File,
        _ => return None,
    })
}

/// Command of a comment tool (pressed state of its button).
pub(crate) fn tool_command(t: CommentTool) -> &'static str {
    match t {
        CommentTool::Typewriter => "cm.typewriter",
        CommentTool::Callout => "cm.callout",
        CommentTool::Rectangle
        | CommentTool::Ellipse
        | CommentTool::Line
        | CommentTool::Arrow
        | CommentTool::Polygon
        | CommentTool::Cloud => "cm.drawing",
        CommentTool::Pencil => "cm.pencil",
        CommentTool::Stamp => "cm.stamp",
        CommentTool::File => "cm.file",
    }
}

fn bounds(pts: &[[f32; 2]]) -> [f32; 4] {
    pts.iter().fold(
        [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ],
        |b, p| {
            [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ]
        },
    )
}

impl EditorWindow {
    /// Comment tool commands; false when `cmd` is not one.
    pub(crate) fn comment_command(&mut self, cmd: &str) -> bool {
        if let Some(t) = tool_of(cmd) {
            self.choose_comment_tool(t);
            return true;
        }
        match cmd {
            "cm.drawing" => {
                let items = [
                    ("cm.rect", "pdfReader.rectangle"),
                    ("cm.ellipse", "pdfEditor.ellipse"),
                    ("cm.line", "pdfEditor.line"),
                    ("cm.arrow", "pdfEditor.arrow"),
                    ("cm.polygon", "pdfEditor.polygon"),
                    ("cm.cloud", "pdfEditor.cloud"),
                ];
                self.menu(&items, "cm.drawing");
            }
            "cm.stamp" => {
                let items: Vec<(String, String)> = STAMPS
                    .iter()
                    .enumerate()
                    .map(|(i, k)| (format!("cm.stamp.{i}"), (*k).to_owned()))
                    .collect();
                let refs: Vec<(&str, &str)> = items
                    .iter()
                    .map(|(a, b)| (a.as_str(), b.as_str()))
                    .collect();
                self.menu(&refs, "cm.stamp");
            }
            _ => {
                if let Some(i) = cmd
                    .strip_prefix("cm.stamp.")
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    self.comment.stamp = i.min(STAMPS.len() - 1);
                    self.choose_comment_tool(CommentTool::Stamp);
                    return true;
                }
                return false;
            }
        }
        true
    }

    fn choose_comment_tool(&mut self, t: CommentTool) {
        self.set_tool(None);
        self.r.set_tool(Tool::Hand);
        self.comment.tool = if self.comment.tool == Some(t) {
            None
        } else {
            Some(t)
        };
        self.comment.points = None;
    }

    /// Pointer input of a comment tool over the document.
    pub(crate) fn comment_input(&mut self, input: &Input) -> bool {
        let Some(tool) = self.comment.tool else {
            return false;
        };
        let area = self.r.doc_area();
        let to_doc = |w: &Self, x: f32, y: f32, page: usize| -> Option<[f32; 2]> {
            let v = w.view()?;
            let pr = v.page_rect(page)?;
            let s = v.scale();
            let (qx, qy) = ((x - pr[0]) / s, (y - pr[1]) / s);
            let r = v.unrotate_rect(page, [qx, qy, qx, qy]);
            Some([r[0], r[1]])
        };
        match *input {
            Input::Press { x, y, count, .. } if inside(area, x, y) => {
                let Some((page, px, py)) = self.view().and_then(|v| v.window_to_doc(x, y)) else {
                    return true;
                };
                let p = [px, py];
                match tool {
                    CommentTool::Polygon | CommentTool::Cloud => {
                        let done = count >= 2;
                        match self.comment.points.as_mut() {
                            Some((pg, pts)) if *pg == page => {
                                if !done {
                                    pts.push(p);
                                }
                            }
                            _ => self.comment.points = Some((page, vec![p])),
                        }
                        if done {
                            if let Some((pg, pts)) = self.comment.points.take() {
                                let kind = if tool == CommentTool::Cloud {
                                    MarkupKind::Cloud
                                } else {
                                    MarkupKind::Polygon
                                };
                                self.add_comment(kind, pg, bounds(&pts), pts, String::new());
                            }
                        }
                    }
                    CommentTool::Stamp => {
                        let label = self.t(STAMPS[self.comment.stamp]);
                        self.add_comment(
                            MarkupKind::Stamp,
                            page,
                            [px, py, px + 120.0, py + 36.0],
                            Vec::new(),
                            label,
                        );
                    }
                    CommentTool::Typewriter => self.ask_text(Ask2::Typewriter(page, p)),
                    CommentTool::File => {
                        self.comment.file_at = Some((page, p));
                        self.push(ReaderRequest::PickFile {
                            purpose: "attach".into(),
                            filters: vec![(self.t("pdfReader.allFiles"), "*.*".into())],
                        });
                    }
                    _ => self.comment.points = Some((page, vec![p, p])),
                }
                true
            }
            Input::Drag { x, y } => {
                let Some(page) = self.comment.points.as_ref().map(|p| p.0) else {
                    return false;
                };
                if matches!(tool, CommentTool::Polygon | CommentTool::Cloud) {
                    return false;
                }
                let Some(p) = to_doc(self, x, y, page) else {
                    return true;
                };
                if let Some((_, pts)) = self.comment.points.as_mut() {
                    if tool == CommentTool::Pencil {
                        if pts
                            .last()
                            .is_none_or(|l| (l[0] - p[0]).abs() + (l[1] - p[1]).abs() > 0.8)
                        {
                            pts.push(p);
                        }
                    } else if pts.len() >= 2 {
                        pts[1] = p;
                    }
                }
                true
            }
            Input::Release { .. } => {
                if matches!(tool, CommentTool::Polygon | CommentTool::Cloud) {
                    return true;
                }
                let Some((page, pts)) = self.comment.points.take() else {
                    return false;
                };
                let b = bounds(&pts);
                let big = b[2] - b[0] > 3.0 || b[3] - b[1] > 3.0;
                match tool {
                    CommentTool::Rectangle | CommentTool::Ellipse if big => {
                        let kind = if tool == CommentTool::Rectangle {
                            MarkupKind::Rectangle
                        } else {
                            MarkupKind::Ellipse
                        };
                        self.add_comment(kind, page, b, Vec::new(), String::new());
                    }
                    CommentTool::Line | CommentTool::Arrow if big => {
                        let kind = if tool == CommentTool::Line {
                            MarkupKind::Line
                        } else {
                            MarkupKind::Arrow
                        };
                        self.add_comment(kind, page, b, pts, String::new());
                    }
                    CommentTool::Pencil if pts.len() > 2 => {
                        self.add_comment(MarkupKind::Pencil, page, b, pts, String::new());
                    }
                    CommentTool::Callout => {
                        // Dragged from the point the callout shows to where its box goes.
                        let target = pts[0];
                        let end = pts.get(1).copied().unwrap_or(target);
                        let end = if big {
                            end
                        } else {
                            [target[0] + 40.0, target[1] - 40.0]
                        };
                        let rect = [end[0], end[1] - 20.0, end[0] + 150.0, end[1] + 20.0];
                        self.ask_text(Ask2::Callout(page, rect, target));
                    }
                    _ => {}
                }
                true
            }
            Input::Key(crate::Key::Escape, _) => {
                self.comment.points = None;
                self.comment.tool = None;
                true
            }
            _ => false,
        }
    }

    fn ask_text(&mut self, what: Ask2) {
        let mut d = Dialog::new(
            DialogKind::Comment,
            self.t("pdfReader.commentText"),
            vec![(self.t("pdfReader.comment"), String::new())],
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfReader.cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        );
        d.layout(
            &crate::editor_window::Estimate,
            self.r.window_size(),
            self.r.ui_scale(),
        );
        self.dialog2 = Some((d, what));
    }

    /// Text typed for a typewriter or callout comment.
    pub(crate) fn comment_text(&mut self, what: &Ask2, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        match *what {
            Ask2::Typewriter(page, p) => self.add_comment(
                MarkupKind::Typewriter,
                page,
                [p[0], p[1], p[0] + 10.0, p[1] + 10.0],
                Vec::new(),
                text.to_owned(),
            ),
            Ask2::Callout(page, rect, target) => {
                self.add_comment(
                    MarkupKind::Callout,
                    page,
                    rect,
                    vec![target],
                    text.to_owned(),
                );
            }
            _ => {}
        }
    }

    /// A file was picked to attach.
    pub(crate) fn attach_picked(&mut self, name: &str, bytes: &[u8]) {
        let Some((page, p)) = self.comment.file_at.take() else {
            return;
        };
        self.add_comment(
            MarkupKind::File(name.to_owned(), bytes.to_vec()),
            page,
            [p[0], p[1], p[0] + 14.0, p[1] + 20.0],
            Vec::new(),
            name.to_owned(),
        );
    }

    fn add_comment(
        &mut self,
        kind: MarkupKind,
        page: usize,
        rect: [f32; 4],
        points: Vec<[f32; 2]>,
        contents: String,
    ) {
        let c = &self.comment;
        let m = Markup {
            kind,
            page,
            rect,
            points,
            color: c.color,
            opacity: c.opacity,
            width: c.width,
            author: std::env::var("USERNAME").unwrap_or_else(|_| "BlueEditor".into()),
            contents,
            font_size: 12.0,
        };
        match self.doc_edit(|d| d.add_markup(&m)) {
            Ok(()) => {
                if let Some(v) = self.r.view_mut() {
                    v.invalidate_page(page);
                }
                self.after_doc_edit();
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// The shape being drawn.
    pub(crate) fn paint_comment(&mut self, p: &mut dyn Painter) {
        let (Some(tool), Some((page, pts))) = (self.comment.tool, self.comment.points.as_ref())
        else {
            return;
        };
        let Some(v) = self.view() else { return };
        let c = self.comment.color;
        let col = Rgba(c[0], c[1], c[2], 255);
        let win: Vec<(f32, f32)> = pts
            .iter()
            .filter_map(|q| v.doc_rect_to_window(*page, [q[0], q[1], q[0], q[1]]))
            .map(|r| (r[0], r[1]))
            .collect();
        if win.is_empty() {
            return;
        }
        let mut segs = Vec::new();
        match tool {
            CommentTool::Rectangle | CommentTool::Ellipse | CommentTool::Callout => {
                let (a, b) = (win[0], *win.last().unwrap_or(&win[0]));
                segs.extend([
                    PathSeg::M(a.0, a.1),
                    PathSeg::L(b.0, a.1),
                    PathSeg::L(b.0, b.1),
                    PathSeg::L(a.0, b.1),
                    PathSeg::Z,
                ]);
            }
            _ => {
                for (i, q) in win.iter().enumerate() {
                    segs.push(if i == 0 {
                        PathSeg::M(q.0, q.1)
                    } else {
                        PathSeg::L(q.0, q.1)
                    });
                }
            }
        }
        let w = (self.comment.width * v.scale()).max(1.0);
        p.path(&segs, None, Some((col, w)));
    }
}
