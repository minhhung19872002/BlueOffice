//! The rest of BlueEditor's Edit tab (as in the reference editor): Link & Join Text, Check
//! Spelling, Add Shapes, Reflow Editing, Article Box, Web Links, Link, Bookmark, Cross
//! Reference, Image Annotation, Audio & Video and 3D; the Form tab's field recognition; the
//! Optimize dialog; paragraph spacing; and the command search box in the tab row.
//!
//! Box tools (link, cross reference, article, image annotation, media, 3D, shapes) are
//! dragged on a page; the ones that need more ask with a dialog or a file picker.
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::editor_actions::Ask2;
use crate::editor_window::{inside, EditorWindow};
use crate::popup::{Choice, MenuItem};
use crate::reader_window::ReaderRequest;
use crate::{Input, Key, Painter};
use bo_pdfread::{LinkTarget, OutlineItem, ShapeKind};
use bo_render::{PathSeg, Rgba};

/// A box tool of the Edit tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoxTool {
    Link,
    CrossRef,
    Article,
    ImageAnnot,
    Media,
    Model3d,
    Shape(ShapeKind),
    /// Link & Join Text: click the text blocks to join, in order.
    Join,
    /// Whiteout: a white box drawn into the page content.
    Whiteout,
    /// Crop Pages (round 9): the drawn box fills in Set Page Dimensions.
    Crop,
}

/// What a dialog of this module asks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ToolAsk {
    /// Link over (page, box): web address or page number.
    Link(usize, [f32; 4]),
    /// Cross reference over (page, box): page number.
    CrossRef(usize, [f32; 4]),
    /// Title of the article whose boxes were drawn.
    Article,
    /// Bookmark title: add at the top level, add under the selected one, rename it.
    BookmarkAdd,
    BookmarkChild,
    BookmarkRename,
    /// A misspelt word: change to.
    Spell,
    /// Optimize: resolution, quality.
    Optimize,
    /// Crop pages: margins in millimetres and which pages.
    Crop,
    /// The Preferences dialog.
    Preferences,
}

/// A misspelt word found by Check Spelling.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Misspelt {
    pub page: usize,
    /// Box of the text block it is in (finds the block again).
    pub block: [f32; 4],
    /// Character range in the block text.
    pub range: (usize, usize),
    pub word: String,
    /// Box of the word (page points).
    pub rect: [f32; 4],
    pub suggestions: Vec<String>,
}

/// State of the tools of this module.
#[derive(Default)]
pub(crate) struct ToolsState {
    pub tool: Option<BoxTool>,
    /// Box being dragged: page, start, end (page points).
    pub drag: Option<(usize, [f32; 2], [f32; 2])>,
    /// Box waiting for a picked file.
    pub pending: Option<(usize, [f32; 4])>,
    /// Article boxes drawn so far.
    pub beads: Vec<(usize, [f32; 4])>,
    /// Text blocks picked to join: page, box.
    pub join: Vec<(usize, [f32; 4])>,
    /// Reflow Editing is on.
    pub reflow: bool,
    /// Words Check Spelling still has to show, the one shown first.
    pub spell: Vec<Misspelt>,
    /// Words to skip for the rest of the check.
    pub ignored: Vec<String>,
    speller: Option<bo_spell::Speller>,
    /// Command search box: focused, typed text.
    pub search_on: bool,
    pub search: String,
}

impl std::fmt::Debug for ToolsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolsState")
            .field("tool", &self.tool)
            .field("drag", &self.drag)
            .finish_non_exhaustive()
    }
}

/// Ribbon commands the command search finds: command and label key.
pub(crate) fn searchable() -> Vec<(&'static str, &'static str)> {
    let mut v = Vec::new();
    for tab in 0..9 {
        for (id, kind) in crate::editor_window::ribbon_items(tab) {
            if let crate::reader_window::Kind::Big(_, label, _) = kind {
                if !v.iter().any(|(c, _)| *c == id) {
                    v.push((id, label));
                }
            }
        }
    }
    v
}

/// The MIME type of an audio or video file from its extension.
fn media_type(name: &str) -> Option<&'static str> {
    let ext = name.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "wmv" => "video/x-ms-wmv",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        _ => return None,
    })
}

fn norm(a: [f32; 2], b: [f32; 2]) -> [f32; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ]
}

/// The bundled checkers: Vietnamese rules and the English dictionary.
fn new_speller() -> bo_spell::Speller {
    let mut s = bo_spell::Speller::new();
    s.add_language("vi", Box::new(bo_spell::vi::Vietnamese));
    if let Some((aff, dic)) = bo_engine::bundled_dictionary() {
        if let Ok(d) = bo_spell::Dictionary::from_aff_dic(aff, dic) {
            s.add_language("en", Box::new(d));
        }
    }
    s
}

/// The entry at `path` of `items`.
fn outline_at<'a>(items: &'a mut [OutlineItem], path: &[usize]) -> Option<&'a mut OutlineItem> {
    let (first, rest) = path.split_first()?;
    let it = items.get_mut(*first)?;
    if rest.is_empty() {
        Some(it)
    } else {
        outline_at(&mut it.children, rest)
    }
}

/// The list holding the entry at `path` and its index there.
fn outline_parent<'a>(
    items: &'a mut Vec<OutlineItem>,
    path: &[usize],
) -> Option<(&'a mut Vec<OutlineItem>, usize)> {
    let (last, head) = path.split_last()?;
    if head.is_empty() {
        return Some((items, *last));
    }
    let p = outline_at(items, head)?;
    Some((&mut p.children, *last))
}

impl EditorWindow {
    fn tools_dialog(&mut self, what: ToolAsk, title: &str, fields: Vec<(String, String)>) {
        let buttons = if what == ToolAsk::Spell {
            vec![
                (self.t("pdfEditor.spellChange"), DialogButton::Ok),
                (self.t("pdfEditor.spellChangeAll"), DialogButton::ReplaceAll),
                (self.t("pdfEditor.spellIgnore"), DialogButton::FindNext),
                (self.t("pdfReader.cancel"), DialogButton::Cancel),
            ]
        } else {
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfReader.cancel"), DialogButton::Cancel),
            ]
        };
        let mut d = Dialog::new(
            DialogKind::Comment,
            self.t(title),
            fields,
            buttons,
            Vec::new(),
        );
        d.layout(
            &crate::editor_window::Estimate,
            self.window_size(),
            self.ui_scale(),
        );
        self.dialog2 = Some((d, Ask2::Tools(what)));
    }

    /// Round 11: choose a box tool from another module (the Add Shapes grid).
    pub(crate) fn set_box_tool11(&mut self, t: BoxTool) {
        self.choose_box_tool(t);
    }

    fn choose_box_tool(&mut self, t: BoxTool) {
        let was = self.tools.tool;
        self.set_tool(None);
        self.comment.tool = None;
        self.design.tool = None;
        self.r.set_tool(crate::reader_window::Tool::Hand);
        self.tools.drag = None;
        if was == Some(BoxTool::Article) {
            self.finish_article();
        }
        if was == Some(BoxTool::Join) {
            self.finish_join();
        }
        self.tools.tool = if was == Some(t) { None } else { Some(t) };
        let hint = match t {
            BoxTool::Join => Some("pdfEditor.joinHint"),
            BoxTool::Article => Some("pdfEditor.articleHint"),
            _ => None,
        };
        if let (Some(h), Some(_)) = (hint, self.tools.tool) {
            let s = self.t(h);
            self.set_status(&s);
        }
    }

    /// Commands of this module; false when `cmd` is not one.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn tools_command(&mut self, cmd: &str) -> bool {
        let shape = |k| Some(BoxTool::Shape(k));
        let tool = match cmd {
            "ed.link" => Some(BoxTool::Link),
            "ed.crossRef" => Some(BoxTool::CrossRef),
            "ed.articleBox" => Some(BoxTool::Article),
            "ed.imageAnnot" => Some(BoxTool::ImageAnnot),
            "ed.media" => Some(BoxTool::Media),
            "ed.add3d" => Some(BoxTool::Model3d),
            "ed.linkJoin" => Some(BoxTool::Join),
            "prot.whiteout" => Some(BoxTool::Whiteout),
            "org.crop" => Some(BoxTool::Crop),
            "ed.shape.rect" => shape(ShapeKind::Rectangle),
            "ed.shape.rounded" => shape(ShapeKind::RoundedRectangle),
            "ed.shape.ellipse" => shape(ShapeKind::Ellipse),
            "ed.shape.line" => shape(ShapeKind::Line),
            _ => None,
        };
        if let Some(t) = tool {
            self.choose_box_tool(t);
            return true;
        }
        match cmd {
            "ed.addShapes" => {
                let items = [
                    ("ed.shape.rect", "pdfReader.rectangle"),
                    ("ed.shape.rounded", "pdfEditor.roundedRect"),
                    ("ed.shape.ellipse", "pdfEditor.ellipse"),
                    ("ed.shape.line", "pdfEditor.line"),
                ];
                self.menu(&items, "ed.addShapes");
            }
            "ed.webLinks" => {
                let items = [
                    ("ed.webLinks.create", "pdfEditor.webLinksCreate"),
                    ("ed.webLinks.remove", "pdfEditor.webLinksRemove"),
                ];
                self.menu(&items, "ed.webLinks");
            }
            "ed.webLinks.create" => {
                let r = self.doc_edit(|d| d.create_web_links(&[]));
                self.counted(r, "pdfEditor.webLinksCreated");
            }
            "ed.webLinks.remove" => {
                let r = self.doc_edit(|d| d.remove_links(&[], true));
                self.counted(r, "pdfEditor.webLinksRemoved");
            }
            "ed.reflow" => {
                self.tools.reflow = !self.tools.reflow;
                self.edit.reflow = self.tools.reflow;
                if self.tools.reflow && self.edit_tool() != Some(crate::EditTool::Text) {
                    self.run_edit_text();
                }
                let s = self.t(if self.tools.reflow {
                    "pdfEditor.reflowOn"
                } else {
                    "pdfEditor.reflowOff"
                });
                self.set_status(&s);
            }
            "ed.spell" => self.start_spelling(),
            "ed.clipboard" => {
                let items = [
                    ("edit.copy", "pdfReader.copy"),
                    ("ed.selectAll", "pdfEditor.selectAll"),
                ];
                self.menu(&items, "ed.clipboard");
            }
            "ed.selectAll" => {
                let ctrl = crate::Mods {
                    ctrl: true,
                    ..crate::Mods::default()
                };
                self.r.handle(
                    Input::Key(Key::Letter('a'), ctrl),
                    &crate::editor_window::Estimate,
                );
            }
            "org.crop" => {
                let u = self.prefs.units.clone();
                let lab = |w: &Self, k: &str| w.t(k).replace("mm", &u);
                let fields = vec![
                    (lab(self, "pdfEditor.cropLeft"), "0".into()),
                    (lab(self, "pdfEditor.cropTop"), "0".into()),
                    (lab(self, "pdfEditor.cropRight"), "0".into()),
                    (lab(self, "pdfEditor.cropBottom"), "0".into()),
                    (
                        self.t("pdfEditor.cropPagesField"),
                        self.t("pdfEditor.cropAll"),
                    ),
                ];
                self.tools_dialog(ToolAsk::Crop, "pdfEditor.cropPages", fields);
            }
            "cm.summary" => {
                let text = self.comment_summary();
                let name = format!("{}-comments.txt", self.doc_stem());
                self.push(ReaderRequest::SaveBytes {
                    name,
                    bytes: text.into_bytes(),
                });
            }
            "ed.bookmark" => {
                if self.r.selected_outline.is_some() {
                    let items = [
                        ("ed.bm.add", "pdfEditor.bmAdd"),
                        ("ed.bm.child", "pdfEditor.bmChild"),
                        ("ed.bm.rename", "pdfEditor.bmRename"),
                        ("ed.bm.delete", "pdfEditor.bmDelete"),
                        ("ed.bm.up", "pdfEditor.bmUp"),
                        ("ed.bm.down", "pdfEditor.bmDown"),
                    ];
                    self.menu(&items, "ed.bookmark");
                } else {
                    self.ask_bookmark(ToolAsk::BookmarkAdd);
                }
            }
            "ed.bm.add" => self.ask_bookmark(ToolAsk::BookmarkAdd),
            "ed.bm.child" => self.ask_bookmark(ToolAsk::BookmarkChild),
            "ed.bm.rename" => self.ask_bookmark(ToolAsk::BookmarkRename),
            "ed.bm.delete" | "ed.bm.up" | "ed.bm.down" => self.bookmark_edit(cmd),
            "form.recognize" => {
                let r = self.doc_edit(|d| d.add_recognized_fields(&[]));
                self.counted(r, "pdfEditor.fieldsRecognized");
            }
            "file.optimize" => {
                let before = self.with_doc(|d| Ok(d.bytes().len())).unwrap_or(0);
                let f = |w: &Self, k: &str, v: &str| (w.t(k), v.to_owned());
                let mut fields = vec![
                    f(self, "pdfEditor.optimizePpi", "150"),
                    f(self, "pdfEditor.optimizeQuality", "75"),
                ];
                fields.push((
                    self.t("pdfEditor.optimizeBefore"),
                    format!("{:.1} KB", before as f32 / 1024.0),
                ));
                self.tools_dialog(ToolAsk::Optimize, "pdfEditor.optimize", fields);
            }
            "fmt.paraSp" => {
                let items: Vec<MenuItem> = [0.0_f32, 2.0, 4.0, 6.0, 8.0, 12.0, 18.0, 24.0]
                    .iter()
                    .map(|v| MenuItem {
                        label: format!("{v:.2}"),
                        choice: Choice::Command(format!("fmt.paraSp.{v}")),
                        checked: false,
                        dim: false,
                    })
                    .collect();
                if let Some(r) = self
                    .pane
                    .hits
                    .iter()
                    .find(|h| h.1 == "fmt.paraSp")
                    .map(|h| h.0)
                {
                    self.menu_items(items, r);
                }
            }
            _ => {
                if let Some(v) = cmd
                    .strip_prefix("fmt.paraSp.")
                    .and_then(|v| v.parse::<f32>().ok())
                {
                    self.set_para_spacing(v);
                    return true;
                }
                return false;
            }
        }
        true
    }

    fn run_edit_text(&mut self) {
        self.push_command("ed.editText");
    }

    /// Run `cmd` as a ribbon click would (the reader window's own commands too).
    pub(crate) fn push_command(&mut self, cmd: &str) {
        self.run(cmd, &crate::editor_window::Estimate);
    }

    fn counted(&mut self, r: Result<usize, String>, key: &str) {
        match r {
            Ok(n) => {
                if n > 0 {
                    if let Some(v) = self.r.view_mut() {
                        for i in 0..v.page_count() {
                            v.invalidate_page(i);
                        }
                    }
                    self.after_doc_edit();
                }
                let s = self.t(key).replace("{count}", &n.to_string());
                self.set_status(&s);
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// Pointer input of a box tool over the document.
    pub(crate) fn tools_input(&mut self, input: &Input) -> bool {
        let Some(tool) = self.tools.tool else {
            return false;
        };
        let area = self.doc_area();
        match *input {
            Input::Press { x, y, .. } if inside(area, x, y) => {
                let Some((page, px, py)) = self.view().and_then(|v| v.window_to_doc(x, y)) else {
                    return true;
                };
                if tool == BoxTool::Join {
                    self.pick_join(page, px, py);
                    return true;
                }
                self.tools.drag = Some((page, [px, py], [px, py]));
                true
            }
            Input::Drag { x, y } => {
                let Some((page, a, _)) = self.tools.drag else {
                    return false;
                };
                let p = self.view().and_then(|v| {
                    let pr = v.page_rect(page)?;
                    let s = v.scale();
                    let (qx, qy) = ((x - pr[0]) / s, (y - pr[1]) / s);
                    let r = v.unrotate_rect(page, [qx, qy, qx, qy]);
                    Some([r[0], r[1]])
                });
                if let Some(p) = p {
                    self.tools.drag = Some((page, a, p));
                }
                true
            }
            Input::Release { .. } => {
                let Some((page, a, b)) = self.tools.drag.take() else {
                    return false;
                };
                let mut r = norm(a, b);
                if tool == BoxTool::Crop {
                    if r[2] - r[0] < 4.0 && r[3] - r[1] < 4.0 {
                        self.open_crop9();
                    } else {
                        self.open_crop9_at(page, r);
                    }
                    return true;
                }
                if r[2] - r[0] < 4.0 && r[3] - r[1] < 4.0 {
                    // A click: a box of the usual size.
                    let (w, h) = match tool {
                        BoxTool::Link | BoxTool::CrossRef => (120.0, 16.0),
                        BoxTool::Shape(ShapeKind::Line) => (100.0, 0.0),
                        _ => (160.0, 120.0),
                    };
                    r = [a[0], a[1], a[0] + w, a[1] + h];
                }
                self.box_drawn(tool, page, r);
                true
            }
            Input::Key(Key::Enter, _) if matches!(tool, BoxTool::Article | BoxTool::Join) => {
                self.tools.tool = None;
                if tool == BoxTool::Article {
                    self.finish_article();
                } else {
                    self.finish_join();
                }
                true
            }
            Input::Key(Key::Escape, _) => {
                self.tools = ToolsState {
                    speller: self.tools.speller.take(),
                    ..ToolsState::default()
                };
                true
            }
            _ => false,
        }
    }

    fn box_drawn(&mut self, tool: BoxTool, page: usize, r: [f32; 4]) {
        let current = self.view().map_or(1, |v| v.current_page() + 1);
        match tool {
            BoxTool::Link => {
                let fields = vec![
                    (self.t("pdfEditor.linkUrl"), String::new()),
                    (self.t("pdfEditor.linkPage"), current.to_string()),
                ];
                self.tools_dialog(ToolAsk::Link(page, r), "pdfEditor.link", fields);
            }
            BoxTool::CrossRef => {
                let fields = vec![(self.t("pdfEditor.linkPage"), current.to_string())];
                self.tools_dialog(ToolAsk::CrossRef(page, r), "pdfEditor.crossRef", fields);
            }
            BoxTool::Article => {
                self.tools.beads.push((page, r));
                let s = self
                    .t("pdfEditor.articleBoxes")
                    .replace("{count}", &self.tools.beads.len().to_string());
                self.set_status(&s);
            }
            BoxTool::ImageAnnot | BoxTool::Media | BoxTool::Model3d => {
                self.tools.pending = Some((page, r));
                let (purpose, filters) = match tool {
                    BoxTool::ImageAnnot => (
                        "imageAnnot",
                        vec![("Images (*.png; *.jpg)".into(), "*.png;*.jpg;*.jpeg".into())],
                    ),
                    BoxTool::Media => (
                        "media",
                        vec![(
                            self.t("pdfEditor.mediaFiles"),
                            "*.mp4;*.m4v;*.mov;*.avi;*.wmv;*.webm;*.mp3;*.m4a;*.wav;*.ogg;*.flac"
                                .into(),
                        )],
                    ),
                    _ => (
                        "model3d",
                        vec![("3D (*.u3d; *.prc)".into(), "*.u3d;*.prc".into())],
                    ),
                };
                self.push(ReaderRequest::PickFile {
                    purpose: purpose.into(),
                    filters,
                });
            }
            BoxTool::Shape(kind) => {
                // Round 11: the style of the Format pane's Shape section.
                let style = self.shape_style11();
                let r = if kind == ShapeKind::Line {
                    r
                } else {
                    norm([r[0], r[1]], [r[2], r[3]])
                };
                let res = self.doc_edit(|d| d.add_shape11(page, kind, r, &style));
                self.page_done(page, res);
            }
            BoxTool::Whiteout => {
                let res = self.doc_edit(|d| {
                    d.add_shape(
                        page,
                        ShapeKind::Rectangle,
                        r,
                        None,
                        Some([255, 255, 255]),
                        0.0,
                    )
                });
                self.page_done(page, res);
            }
            BoxTool::Join | BoxTool::Crop => {}
        }
    }

    fn page_done(&mut self, page: usize, r: Result<(), String>) {
        match r {
            Ok(()) => {
                if let Some(v) = self.r.view_mut() {
                    v.invalidate_page(page);
                }
                self.after_doc_edit();
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// A file for a box tool; false when `purpose` is not one of this module.
    pub(crate) fn tools_file_picked(&mut self, purpose: &str, name: &str, bytes: &[u8]) -> bool {
        if !matches!(purpose, "imageAnnot" | "media" | "model3d") {
            return false;
        }
        let Some((page, r)) = self.tools.pending.take() else {
            return true;
        };
        let res = match purpose {
            "imageAnnot" => self
                .doc_edit(|d| d.add_image_annotation(page, r, bo_pdfread::NewImage(bytes), name)),
            "media" => match media_type(name) {
                Some(mime) => self.doc_edit(|d| d.add_media(page, r, name, mime, bytes)),
                None => Err(self.t("pdfEditor.mediaUnknown")),
            },
            _ => self
                .doc_edit(|d| d.add_3d(page, r, name, bytes))
                .map_err(|_| self.t("pdfEditor.model3dUnknown")),
        };
        self.page_done(page, res);
        true
    }

    fn finish_article(&mut self) {
        if self.tools.beads.is_empty() {
            return;
        }
        let n = self.with_doc(|d| Ok(d.articles().len())).unwrap_or(0) + 1;
        let fields = vec![(self.t("pdfEditor.articleTitle"), format!("Article {n}"))];
        self.tools_dialog(ToolAsk::Article, "pdfEditor.articleBox", fields);
    }

    fn pick_join(&mut self, page: usize, x: f32, y: f32) {
        let Ok(objs) = self.with_doc(|d| d.page_objects(page)) else {
            return;
        };
        let Some(b) = bo_pdfread::text_blocks(&objs)
            .into_iter()
            .find(|b| x >= b.bbox[0] && x <= b.bbox[2] && y >= b.bbox[1] && y <= b.bbox[3])
        else {
            return;
        };
        if self.tools.join.first().is_some_and(|(p, _)| *p != page) {
            let s = self.t("pdfEditor.joinSamePage");
            self.set_status(&s);
            return;
        }
        if let Some(i) = self.tools.join.iter().position(|j| j.1 == b.bbox) {
            self.tools.join.remove(i);
        } else {
            self.tools.join.push((page, b.bbox));
        }
    }

    /// Join the picked blocks: the others' text goes at the end of the first, in order.
    fn finish_join(&mut self) {
        let picked = std::mem::take(&mut self.tools.join);
        if picked.len() < 2 {
            return;
        }
        let page = picked[0].0;
        let r = self.doc_edit(|d| {
            let objs = d.page_objects(page)?;
            let blocks = bo_pdfread::text_blocks(&objs);
            let find = |r: [f32; 4]| blocks.iter().find(|b| b.bbox == r).cloned();
            let first = find(picked[0].1).ok_or("the text changed")?;
            let rest: Vec<bo_pdfread::TextBlock> =
                picked[1..].iter().filter_map(|p| find(p.1)).collect();
            let text: Vec<String> = rest.iter().map(|b| b.text.clone()).collect();
            let mut ids: Vec<usize> = rest.iter().flat_map(|b| b.ids.clone()).collect();
            ids.sort_unstable();
            ids.dedup();
            d.begin_group("link & join text");
            let r = (|| {
                d.delete_objects(page, &ids)?;
                let objs = d.page_objects(page)?;
                let b = bo_pdfread::text_blocks(&objs)
                    .into_iter()
                    .find(|b| b.bbox == first.bbox)
                    .ok_or("the text changed")?;
                let mut e = d.begin_block_edit(page, &b)?;
                let st = e.style_of.last().copied().unwrap_or(0);
                for t in text {
                    for c in std::iter::once(' ').chain(t.chars()) {
                        e.text.push(c);
                        e.style_of.push(st);
                    }
                }
                let r = d.block_edit_update(&mut e);
                d.end_block_edit(&e);
                r
            })();
            d.end_group();
            r
        });
        self.page_done(page, r);
    }

    fn ask_bookmark(&mut self, what: ToolAsk) {
        let current = if what == ToolAsk::BookmarkRename {
            let path = self.r.selected_outline.clone().unwrap_or_default();
            let mut items = self.with_doc(|d| Ok(d.outline())).unwrap_or_default();
            outline_at(&mut items, &path)
                .map(|i| i.title.clone())
                .unwrap_or_default()
        } else {
            // The selected text, as the reference names a new bookmark.
            self.r
                .selected_text()
                .filter(|s| !s.trim().is_empty())
                .map_or_else(
                    || self.t("pdfEditor.bmUntitled"),
                    |s| s.trim().chars().take(80).collect(),
                )
        };
        let fields = vec![(self.t("pdfEditor.bmTitle"), current)];
        self.tools_dialog(what, "pdfEditor.bookmark", fields);
    }

    fn write_outline(&mut self, items: &[OutlineItem]) {
        let r = self.doc_edit(|d| d.set_outline(items));
        match r {
            Ok(()) => {
                self.after_doc_edit();
                self.r.show_panel(crate::reader_window::Panel::Bookmarks);
            }
            Err(e) => self.set_status(&e),
        }
    }

    fn bookmark_edit(&mut self, cmd: &str) {
        let Some(path) = self.r.selected_outline.clone() else {
            return;
        };
        let mut items = self.with_doc(|d| Ok(d.outline())).unwrap_or_default();
        let Some((list, i)) = outline_parent(&mut items, &path) else {
            return;
        };
        if i >= list.len() {
            return;
        }
        let mut new_path = path.clone();
        match cmd {
            "ed.bm.delete" => {
                list.remove(i);
                self.r.selected_outline = None;
            }
            "ed.bm.up" if i > 0 => {
                list.swap(i, i - 1);
                if let Some(l) = new_path.last_mut() {
                    *l = i - 1;
                }
            }
            "ed.bm.down" if i + 1 < list.len() => {
                list.swap(i, i + 1);
                if let Some(l) = new_path.last_mut() {
                    *l = i + 1;
                }
            }
            _ => return,
        }
        self.write_outline(&items);
        if cmd != "ed.bm.delete" {
            self.r.selected_outline = Some(new_path);
        }
    }

    fn bookmark_answer(&mut self, what: &ToolAsk, title: &str) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        let target = self.view().map(|v| LinkTarget::Page {
            index: v.current_page(),
            top: None,
        });
        let mut items = self.with_doc(|d| Ok(d.outline())).unwrap_or_default();
        let sel = self.r.selected_outline.clone();
        let item = OutlineItem {
            title: title.to_owned(),
            target,
            open: false,
            children: Vec::new(),
        };
        match (what, sel) {
            (ToolAsk::BookmarkRename, Some(p)) => {
                if let Some(it) = outline_at(&mut items, &p) {
                    title.clone_into(&mut it.title);
                }
            }
            (ToolAsk::BookmarkChild, Some(p)) => {
                if let Some(it) = outline_at(&mut items, &p) {
                    it.open = true;
                    it.children.push(item);
                }
            }
            (_, Some(p)) => {
                // After the selected entry, at its level.
                if let Some((list, i)) = outline_parent(&mut items, &p) {
                    let at = (i + 1).min(list.len());
                    list.insert(at, item);
                    let mut np = p.clone();
                    if let Some(l) = np.last_mut() {
                        *l = at;
                    }
                    self.r.selected_outline = Some(np);
                }
            }
            (_, None) => {
                items.push(item);
                self.r.selected_outline = Some(vec![items.len() - 1]);
            }
        }
        self.write_outline(&items);
    }

    /// Answer of a dialog of this module (`button`: which one closed it).
    pub(crate) fn tools_answer(&mut self, what: ToolAsk, button: DialogButton, v: &[String]) {
        let get = |i: usize| v.get(i).cloned().unwrap_or_default();
        if what == ToolAsk::Spell {
            self.spell_answer(button, &get(1));
            return;
        }
        if button != DialogButton::Ok {
            if what == ToolAsk::Article {
                self.tools.beads.clear();
            }
            return;
        }
        let page_of = |s: &str, n: usize| {
            s.trim()
                .parse::<usize>()
                .ok()
                .filter(|p| (1..=n).contains(p))
                .map(|p| p - 1)
        };
        let n = self.view().map_or(0, crate::ReaderView::page_count);
        match what {
            ToolAsk::Link(page, r) => {
                let url = get(0).trim().to_owned();
                let target = if url.is_empty() {
                    page_of(&get(1), n).map(|index| LinkTarget::Page { index, top: None })
                } else if url.contains(':') {
                    Some(LinkTarget::Uri(url))
                } else if url.contains('@') {
                    Some(LinkTarget::Uri(format!("mailto:{url}")))
                } else {
                    Some(LinkTarget::Uri(format!("http://{url}")))
                };
                self.link_to(page, r, target);
            }
            ToolAsk::CrossRef(page, r) => {
                let target = page_of(&get(0), n).map(|index| LinkTarget::Page { index, top: None });
                self.link_to(page, r, target);
            }
            ToolAsk::Article => {
                let beads = std::mem::take(&mut self.tools.beads);
                let title = get(0);
                let r = self.doc_edit(|d| d.add_article(&title, &beads));
                match r {
                    Ok(()) => self.after_doc_edit(),
                    Err(e) => self.set_status(&e),
                }
            }
            ToolAsk::BookmarkAdd | ToolAsk::BookmarkChild | ToolAsk::BookmarkRename => {
                self.bookmark_answer(&what, &get(0));
            }
            ToolAsk::Optimize => {
                let ppi = get(0)
                    .trim()
                    .parse::<f32>()
                    .unwrap_or(150.0)
                    .clamp(0.0, 2400.0);
                let q = get(1).trim().parse::<u8>().unwrap_or(75).clamp(10, 100);
                self.optimize(ppi, q);
            }
            ToolAsk::Crop => {
                // Points per unit of the preferences.
                let per = match self.prefs.units.as_str() {
                    "cm" => 72.0 / 2.54,
                    "in" => 72.0,
                    "pt" => 1.0,
                    _ => 72.0 / 25.4,
                };
                let mm = |i: usize| {
                    get(i)
                        .trim()
                        .replace(',', ".")
                        .parse::<f32>()
                        .unwrap_or(0.0)
                        .max(0.0)
                        * per
                };
                let all = self.t("pdfEditor.cropAll");
                let which = get(4);
                let pages: Vec<usize> = if which.trim().is_empty() || which.trim() == all {
                    Vec::new()
                } else {
                    which
                        .split(',')
                        .filter_map(|s| s.trim().parse::<usize>().ok())
                        .filter(|p| (1..=n).contains(p))
                        .map(|p| p - 1)
                        .collect()
                };
                let m = [mm(0), mm(1), mm(2), mm(3)];
                let r = self.doc_edit(|d| d.crop_pages(&pages, m));
                match r {
                    Ok(()) => self.after_doc_edit(),
                    Err(e) => self.set_status(&e),
                }
            }
            ToolAsk::Preferences => self.set_preferences(v),
            ToolAsk::Spell => {}
        }
    }

    fn link_to(&mut self, page: usize, r: [f32; 4], target: Option<LinkTarget>) {
        if let Some(t) = target {
            let res = self.doc_edit(|d| d.add_link(page, r, &t));
            self.page_done(page, res);
        } else {
            let s = self.t("pdfEditor.badPage");
            self.set_status(&s);
        }
    }

    fn optimize(&mut self, ppi: f32, quality: u8) {
        self.optimize9(ppi, 1.5, quality, true);
    }

    /// Optimize with the PDF Optimizer's values (round 9) and save the copy.
    pub(crate) fn optimize9(&mut self, ppi: f32, threshold: f32, quality: u8, streams: bool) {
        let o = bo_pdfread::OptimizeOptions {
            max_ppi: ppi,
            threshold,
            jpeg_quality: quality,
            object_streams: streams,
        };
        let enc = |rgba: &[u8], w: u32, h: u32, q: u8| bo_raster::jpeg::encode_jpeg(rgba, w, h, q);
        let name = format!("{}-optimized.pdf", self.doc_stem());
        match self.doc_edit(|d| d.optimize(&o, &enc)) {
            Ok((bytes, rep)) => {
                if let Some(v) = self.r.view_mut() {
                    for i in 0..v.page_count() {
                        v.invalidate_page(i);
                    }
                }
                self.after_doc_edit();
                let s = self
                    .t("pdfEditor.optimized")
                    .replace("{before}", &format!("{:.1}", rep.before as f32 / 1024.0))
                    .replace("{after}", &format!("{:.1}", rep.after as f32 / 1024.0))
                    .replace("{images}", &rep.images.to_string());
                self.set_status(&s);
                self.push(ReaderRequest::SaveBytes { name, bytes });
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// The comments as text: page, author, kind and contents of each, in page order.
    pub(crate) fn comment_summary(&mut self) -> String {
        let name = self.doc_stem();
        let list = self.with_doc(|d| Ok(d.annotations())).unwrap_or_default();
        let mut s = format!(
            "{name}.pdf
{}: {}

",
            self.t("pdfEditor.summarize"),
            list.len()
        );
        for a in list {
            if a.kind == bo_pdfread::AnnotKind::Other && a.contents.is_empty() {
                continue;
            }
            s.push_str(&format!(
                "{} {} — {} ({:?}) {}
{}

",
                self.t("pdfEditor.summaryPage"),
                a.page + 1,
                a.author,
                a.kind,
                a.modified,
                a.contents
            ));
        }
        s
    }

    // ---- Check Spelling ------------------------------------------------------------------

    fn start_spelling(&mut self) {
        if self.tools.speller.is_none() {
            self.tools.speller = Some(new_speller());
        }
        self.tools.ignored.clear();
        let Some(speller) = self.tools.speller.take() else {
            return;
        };
        let found = self
            .with_doc(|d| {
                let mut out = Vec::new();
                for page in 0..d.page_count() {
                    let Ok(objs) = d.page_objects(page) else {
                        continue;
                    };
                    for b in bo_pdfread::text_blocks(&objs) {
                        let starts: Vec<usize> = b.text.char_indices().map(|(i, _)| i).collect();
                        for r in speller.misspelt(&b.text) {
                            let c0 = starts.iter().position(|&i| i == r.start).unwrap_or(0);
                            let c1 = starts
                                .iter()
                                .position(|&i| i >= r.end)
                                .unwrap_or(starts.len());
                            let mut rect = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                            for c in b.char_boxes.get(c0..c1).unwrap_or(&[]) {
                                rect = [
                                    rect[0].min(c[0]),
                                    rect[1].min(c[1]),
                                    rect[2].max(c[2]),
                                    rect[3].max(c[3]),
                                ];
                            }
                            let word = b.text[r.clone()].to_owned();
                            out.push(Misspelt {
                                page,
                                block: b.bbox,
                                range: (c0, c1),
                                suggestions: speller.suggest(&word).into_iter().take(6).collect(),
                                word,
                                rect,
                            });
                        }
                    }
                }
                Ok(out)
            })
            .unwrap_or_default();
        self.tools.speller = Some(speller);
        self.tools.spell = found;
        self.next_misspelt();
    }

    /// Show the next misspelt word (or say the check is done).
    fn next_misspelt(&mut self) {
        while let Some(m) = self.tools.spell.first() {
            if self.tools.ignored.contains(&m.word) {
                self.tools.spell.remove(0);
            } else {
                break;
            }
        }
        let Some(m) = self.tools.spell.first().cloned() else {
            let s = self.t("pdfEditor.spellDone");
            self.set_status(&s);
            return;
        };
        if let Some(v) = self.r.view_mut() {
            v.go_to(m.page, Some((m.rect[1] - 80.0).max(0.0)));
        }
        let fields = vec![
            (self.t("pdfEditor.spellNotFound"), m.word.clone()),
            (
                self.t("pdfEditor.spellChangeTo"),
                m.suggestions
                    .first()
                    .cloned()
                    .unwrap_or_else(|| m.word.clone()),
            ),
        ];
        self.tools_dialog(ToolAsk::Spell, "pdfEditor.spell", fields);
    }

    fn spell_answer(&mut self, button: DialogButton, with: &str) {
        if self.tools.spell.is_empty() {
            return;
        }
        let m = self.tools.spell.remove(0);
        match button {
            DialogButton::Ok if with != m.word && !with.is_empty() => {
                let r = self.doc_edit(|d| replace_chars(d, &m, with));
                // Later words of the same block moved: find them again by position.
                let shift = with.chars().count() as isize - (m.range.1 - m.range.0) as isize;
                for o in &mut self.tools.spell {
                    if o.page == m.page && o.block == m.block && o.range.0 >= m.range.1 {
                        o.range.0 = (o.range.0 as isize + shift) as usize;
                        o.range.1 = (o.range.1 as isize + shift) as usize;
                    }
                }
                if let Err(e) = r {
                    self.set_status(&e);
                }
                if let Some(v) = self.r.view_mut() {
                    v.invalidate_page(m.page);
                }
                self.after_doc_edit();
            }
            DialogButton::ReplaceAll if !with.is_empty() => {
                if with != m.word {
                    self.replace_all(&m.word, with);
                }
                self.tools.ignored.push(m.word);
            }
            DialogButton::Cancel => {
                self.tools.spell.clear();
                return;
            }
            _ => {}
        }
        self.next_misspelt();
    }

    /// Reflow Editing: everything below a paragraph moves with it while typing (the block
    /// edit's followers, `bo_pdfread::PushMode`); off, only the paragraphs after it in its box
    /// do. Keeps the editing state in step with the toggle. Also hands the reader's Text Box
    /// tool to the editor's own (typed in place, `editor_textbox`). True when that changed.
    pub(crate) fn reflow_check(&mut self) -> bool {
        self.edit.reflow = self.tools.reflow;
        if self.r.current_tool() == crate::reader_window::Tool::TextBox {
            self.r.set_tool(crate::reader_window::Tool::Hand);
            self.comment_command("tool.textbox");
            return true;
        }
        false
    }

    // ---- Paragraph spacing ---------------------------------------------------------------

    /// Set the space below the paragraph being edited: everything below it on the page moves.
    fn set_para_spacing(&mut self, value: f32) {
        let Some((page, bottom, gap)) = self.edit.typing.as_ref().and_then(|t| {
            let b = t.block.as_ref()?;
            Some((t.page, b.bbox[3], b.line_gap()))
        }) else {
            return;
        };
        if let Some(v) = self.r.view_mut() {
            self.edit.finish_typing(Some(v));
        }
        let r = self.doc_edit(|d| {
            let objs = d.page_objects(page)?;
            let below: Vec<&bo_pdfread::PageObject> =
                objs.iter().filter(|o| o.bbox[1] >= bottom - 2.0).collect();
            let next = below
                .iter()
                .map(|o| o.bbox[1])
                .fold(f32::INFINITY, f32::min);
            if !next.is_finite() {
                return Err("no paragraph below".into());
            }
            let current = (next - bottom - gap).max(0.0);
            let dy = value - current;
            let ids: Vec<usize> = objs
                .iter()
                .enumerate()
                .filter(|(_, o)| o.bbox[1] >= bottom - 2.0)
                .map(|(i, _)| i)
                .collect();
            d.transform_objects(page, &ids, [1.0, 0.0, 0.0, 1.0, 0.0, dy])
        });
        self.edit.reset(self.r.view_mut());
        self.page_done(page, r);
    }

    // ---- Command search ------------------------------------------------------------------

    /// The command search box (window pixels): left of the Find box, as the reference's.
    pub(crate) fn search_box(&self) -> [f32; 4] {
        let s = self.ui_scale();
        let px = |v: f32| (v * s).round();
        let (w, _) = self.window_size();
        let find_x = w - px(38.4) - px(90.4);
        [find_x - px(72.8) - px(88.0), px(40.0), px(88.0), px(22.4)]
    }

    /// Input of the command search box; true when used.
    pub(crate) fn search_input(&mut self, input: &Input) -> bool {
        if !self.showing_doc() {
            return false;
        }
        let b = self.search_box();
        match input {
            Input::Press { x, y, .. } => {
                let on = inside(b, *x, *y);
                if on != self.tools.search_on {
                    self.tools.search_on = on;
                    if on {
                        self.show_matches();
                    }
                }
                on
            }
            Input::Text(t) if self.tools.search_on => {
                self.tools.search.push_str(t);
                self.show_matches();
                true
            }
            Input::Key(k, _) if self.tools.search_on => {
                match k {
                    Key::Backspace => {
                        self.tools.search.pop();
                        self.show_matches();
                    }
                    Key::Escape => {
                        self.tools.search_on = false;
                        self.r.close_menu();
                        self.tools.search.clear();
                    }
                    Key::Enter => {
                        if let Some(cmd) = self.matches().first().map(|m| m.0) {
                            self.tools.search_on = false;
                            self.r.close_menu();
                            self.tools.search.clear();
                            self.push_command(cmd);
                        }
                    }
                    _ => {}
                }
                true
            }
            _ => false,
        }
    }

    /// Commands whose label contains the typed words.
    pub(crate) fn matches(&self) -> Vec<(&'static str, String)> {
        let q = self.tools.search.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        searchable()
            .into_iter()
            .map(|(cmd, key)| (cmd, self.t(key).replace('\u{a0}', " ")))
            .filter(|(_, label)| {
                let l = label.to_lowercase();
                q.split_whitespace().all(|w| l.contains(w))
            })
            .take(12)
            .collect()
    }

    fn show_matches(&mut self) {
        let items: Vec<MenuItem> = self
            .matches()
            .into_iter()
            .map(|(cmd, label)| MenuItem {
                label,
                choice: Choice::Command(format!("search.run.{cmd}")),
                checked: false,
                dim: false,
            })
            .collect();
        if items.is_empty() {
            self.r.close_menu();
        } else {
            let b = self.search_box();
            self.r.menu_at(items, b, "search");
        }
    }

    /// A command picked from the search results.
    pub(crate) fn search_command(&mut self, cmd: &str) -> bool {
        let Some(run) = cmd.strip_prefix("search.run.") else {
            return false;
        };
        self.tools.search_on = false;
        self.tools.search.clear();
        self.push_command(run);
        true
    }

    // ---- Painting ------------------------------------------------------------------------

    /// The box being dragged, article boxes, picked blocks, the misspelt word and the
    /// command search box.
    pub(crate) fn paint_tools(&mut self, p: &mut dyn Painter, m: &dyn crate::TextMeasure) {
        let accent = self.accent();
        let mut boxes: Vec<(usize, [f32; 4], bool)> = Vec::new();
        if let Some((page, a, b)) = self.tools.drag {
            boxes.push((page, norm(a, b), true));
        }
        for (page, r) in self.tools.beads.iter().chain(&self.tools.join) {
            boxes.push((*page, *r, false));
        }
        if self
            .dialog2
            .as_ref()
            .is_some_and(|d| d.1 == Ask2::Tools(ToolAsk::Spell))
        {
            if let Some(w) = self.tools.spell.first() {
                boxes.push((w.page, w.rect, false));
            }
        }
        if self.showing_doc() {
            if let Some(v) = self.view() {
                for (page, r, dashed) in boxes {
                    let Some(w) = v.doc_rect_to_window(page, r) else {
                        continue;
                    };
                    p.fill_rect(
                        w[0],
                        w[1],
                        w[2],
                        w[3],
                        Rgba(accent.0, accent.1, accent.2, 36),
                    );
                    let (x0, y0, x1, y1) = (w[0], w[1], w[0] + w[2], w[1] + w[3]);
                    let _ = dashed;
                    p.path(
                        &[
                            PathSeg::M(x0, y0),
                            PathSeg::L(x1, y0),
                            PathSeg::L(x1, y1),
                            PathSeg::L(x0, y1),
                            PathSeg::Z,
                        ],
                        None,
                        Some((accent, 1.0)),
                    );
                }
            }
            self.paint_search(p, m);
        }
    }

    fn paint_search(&self, p: &mut dyn Painter, m: &dyn crate::TextMeasure) {
        let b = self.search_box();
        let s = self.ui_scale();
        let edge = if self.tools.search_on {
            self.accent()
        } else {
            Rgba(0xD0, 0xD0, 0xD0, 255)
        };
        let r = b[3] / 2.0;
        p.path(
            &crate::reader_window::rounded(b[0], b[1], b[2], b[3], r),
            Some(Rgba(255, 255, 255, 255)),
            Some((edge, 1.0)),
        );
        // A light bulb of our own drawing.
        let (cx, cy) = (b[0] + 12.0 * s, b[1] + b[3] / 2.0 - 1.0 * s);
        let ink = Rgba(0x55, 0x55, 0x55, 255);
        let rr = 4.4 * s;
        let k = 0.552 * rr;
        p.path(
            &[
                PathSeg::M(cx - rr * 0.6, cy + rr * 0.8),
                PathSeg::C(
                    cx - rr * 1.1,
                    cy + rr * 0.3,
                    cx - rr,
                    cy - k,
                    cx - rr * 0.1,
                    cy - rr,
                ),
                PathSeg::C(
                    cx + k,
                    cy - rr * 1.05,
                    cx + rr * 1.2,
                    cy - k * 0.5,
                    cx + rr * 0.6,
                    cy + rr * 0.8,
                ),
                PathSeg::L(cx + rr * 0.6, cy + rr * 1.4),
                PathSeg::L(cx - rr * 0.6, cy + rr * 1.4),
                PathSeg::Z,
            ],
            None,
            Some((ink, 1.0)),
        );
        let style = crate::TextStyle {
            size: 15.0 / 1.375 * s,
            bold: false,
        };
        let (text, col) = if self.tools.search.is_empty() {
            (
                self.t("pdfEditor.searchCommands"),
                Rgba(0xA0, 0xA0, 0xA0, 255),
            )
        } else {
            (self.tools.search.clone(), Rgba(0x26, 0x26, 0x26, 255))
        };
        let (_, th) = m.measure(&text, style);
        p.push_clip(b[0], b[1], b[2] - 4.0 * s, b[3]);
        p.text(
            &text,
            b[0] + 24.0 * s,
            b[1] + ((b[3] - th) / 2.0).round(),
            style,
            col,
        );
        if self.tools.search_on {
            let (tw, _) = m.measure(&self.tools.search, style);
            p.fill_rect(
                b[0] + 24.0 * s + tw + 1.0,
                b[1] + 4.0 * s,
                1.0,
                b[3] - 8.0 * s,
                Rgba(0x26, 0x26, 0x26, 255),
            );
        }
        p.pop_clip();
    }
}

/// Replace the characters of `m` in its text block with `with`; returns the block's new box.
fn replace_chars(
    d: &mut bo_pdfread::PdfDocument,
    m: &Misspelt,
    with: &str,
) -> Result<[f32; 4], String> {
    let objs = d.page_objects(m.page)?;
    let (cx, cy) = (
        f32::midpoint(m.rect[0], m.rect[2]),
        f32::midpoint(m.rect[1], m.rect[3]),
    );
    let b = bo_pdfread::text_blocks(&objs)
        .into_iter()
        .find(|b| cx >= b.bbox[0] && cx <= b.bbox[2] && cy >= b.bbox[1] && cy <= b.bbox[3])
        .ok_or("the text changed")?;
    let mut e = d.begin_block_edit(m.page, &b)?;
    let (a, z) = (m.range.0.min(e.text.len()), m.range.1.min(e.text.len()));
    let st = e.style_of.get(a).copied().unwrap_or(0);
    let new: Vec<char> = with.chars().collect();
    e.text.splice(a..z, new.iter().copied());
    e.style_of.splice(a..z, std::iter::repeat_n(st, new.len()));
    let r = d.block_edit_update(&mut e);
    let bbox = e.bbox;
    d.end_block_edit(&e);
    r.map(|()| bbox)
}
