//! Paginated document view: page layout, scrolling, caret and selection, keyboard and mouse
//! editing, and incremental page repaints (only the band that changed is redrawn).
use crate::{Input, Key, Mods, PageStore, Painter};
use bo_edit::FormatState;
use bo_engine::edit::Command;
use bo_engine::nav::{CaretRect, Motion, SelRect};
use bo_engine::{EditResult, Engine, PageSize};
use bo_render::{changed_region, Change, PageList, Rgba};
use std::collections::{HashMap, HashSet};

/// Background behind the pages: the neutral grey measured on a desktop word processor
/// (docs/ui/REPORT.md round 30).
pub const CANVAS_BG: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
const PAGE_SHADOW: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
/// Text selection: neutral grey, 198 over white like a desktop word processor (text
/// stays dark under it).
const SELECTION: Rgba = Rgba(0, 0, 0, 57);
const CARET: Rgba = Rgba(0, 0, 0, 255);
/// Gap between pages and around them, in logical pixels.
const GAP: f32 = 16.0;
/// Duration of the caret glide to its new position after typing or arrow keys (ms).
const GLIDE_MS: f64 = 70.0;
/// Farther caret moves (device pixels) jump instead of gliding.
const GLIDE_MAX: f32 = 400.0;

/// Caret glide from a window position towards the caret's laid-out position.
#[derive(Debug, Clone, Copy)]
struct Glide {
    from: [f32; 4],
    /// Start time; set by the first [`DocView::advance`] after the move.
    start: Option<f64>,
}

/// Pages kept drawn beyond the visible ones (each way).
const OVERSCAN: usize = 1;

/// Where a page sits in the view (device pixels, top relative to the view after scrolling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSlot {
    /// Page index.
    pub index: usize,
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: i32,
    /// Height.
    pub height: i32,
}

/// Wavy line colours of spelling (red) and grammar (blue) issues.
const SPELL_RED: Rgba = Rgba(0xe8, 0x11, 0x23, 255);
const SPELL_GRAMMAR: Rgba = Rgba(0x2b, 0x57, 0xd8, 255);

/// A wavy line `w` long starting at (`x`, `y`): 2 px high, 4 px period at 100 % display scale.
fn wavy(p: &mut dyn Painter, x: f32, y: f32, w: f32, scale: f32, color: Rgba) {
    if w <= 0.0 {
        return;
    }
    let step = (2.0 * scale).max(1.0);
    let amp = (1.0 * scale).max(1.0);
    let mut segs = vec![bo_render::PathSeg::M(x, y)];
    let mut dx = 0.0;
    let mut up = true;
    while dx < w {
        dx = (dx + step).min(w);
        segs.push(bo_render::PathSeg::L(x + dx, if up { y - amp } else { y }));
        up = !up;
    }
    p.path(&segs, None, Some((color, scale.max(1.0))));
}

/// The editing view of one document.
pub struct DocView {
    engine: Engine,
    pages: Vec<PageSize>,
    /// Top-left of the view in the window (device pixels).
    origin: (f32, f32),
    viewport: (f32, f32),
    display_scale: f32,
    zoom: f32,
    scroll: f32,
    caret: Option<CaretRect>,
    rects: Vec<SelRect>,
    caret_on: bool,
    glide: Option<Glide>,
    /// Host clock of the frame being drawn (ms).
    now: f64,
    /// Display list each stored page image shows, with its scale (base of the next diff).
    shown: HashMap<usize, (f32, PageList)>,
    /// Rendering parameters changed; repaint retained images without losing eviction bookkeeping.
    repaint_all: bool,
    /// Pages the engine reported changed since their image was drawn.
    stale: HashSet<usize>,
    modified: bool,
    track: bool,
    /// Input-method composition shown at the caret.
    preedit: String,
    comments: Vec<bo_edit::comments::CommentInfo>,
    selection: Option<bo_edit::Selection>,
    format: FormatState,
    words: usize,
    /// Header, footer or note being edited (contextual Header & Footer tab).
    story: Option<bo_engine::story::StoryState>,
    /// Search results highlighted on the pages (navigation pane).
    highlights: Vec<SelRect>,
    /// Misspelt words and grammar issues under a wavy line (check as you type).
    spell: Vec<bo_engine::spell::SpellMark>,
    /// How tracked changes and comments are shown.
    markup: bo_engine::markup::MarkupOptions,
}

impl DocView {
    /// A view of the document already open in `engine`.
    pub fn new(mut engine: Engine) -> Result<Self, String> {
        let state = engine.command(Command::State).map_err(|e| e.to_string())?;
        let mut view = Self {
            engine,
            pages: Vec::new(),
            origin: (0.0, 0.0),
            viewport: (800.0, 600.0),
            display_scale: 1.0,
            zoom: 1.0,
            scroll: 0.0,
            caret: None,
            rects: Vec::new(),
            caret_on: true,
            glide: None,
            now: 0.0,
            shown: HashMap::new(),
            repaint_all: false,
            stale: HashSet::new(),
            modified: false,
            track: false,
            preedit: String::new(),
            comments: Vec::new(),
            selection: None,
            format: FormatState::default(),
            words: 0,
            story: None,
            highlights: Vec::new(),
            spell: Vec::new(),
            markup: bo_engine::markup::MarkupOptions::default(),
        };
        view.apply(&state);
        Ok(view)
    }

    /// The document engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// The document changed since it was opened or saved.
    pub fn modified(&self) -> bool {
        self.modified
    }

    /// Formatting at the selection (ribbon toggle state).
    pub fn format(&self) -> &FormatState {
        &self.format
    }

    /// Number of pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Words in the main story.
    pub fn word_count(&self) -> usize {
        self.words
    }

    /// Index of the first page visible in the view.
    pub fn current_page(&self) -> usize {
        self.visible().first().map_or(0, |s| s.index)
    }

    /// Zoom factor (1 = 100 %).
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Place the view at `(x, y)` with size `(width, height)` in window device pixels.
    pub fn set_frame(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.glide = None;
        self.origin = (x, y);
        self.viewport = (width.max(1.0), height.max(1.0));
        self.clamp_scroll();
    }

    /// Set the zoom factor (0.1 to 5), keeping the top of the view on the same content.
    pub fn set_zoom(&mut self, zoom: f32) -> bool {
        let zoom = zoom.clamp(0.1, 5.0);
        if (zoom - self.zoom).abs() < 1e-4 {
            return false;
        }
        let ratio = zoom / self.zoom;
        self.glide = None;
        self.zoom = zoom;
        self.scroll *= ratio;
        self.clamp_scroll();
        true
    }

    /// Copy (or cut) the selection; the clipboard forms for the host.
    pub fn copy(&mut self, cut: bool) -> Option<bo_engine::edit::ClipOut> {
        let result = self
            .engine
            .command(if cut { Command::Cut } else { Command::Copy })
            .ok()?;
        if cut {
            self.apply(&result);
            self.caret_on = true;
        }
        result.clip
    }

    /// Paste clipboard content (internal form, else HTML, else text).
    pub fn paste(
        &mut self,
        internal: Option<String>,
        html: Option<String>,
        text: Option<String>,
    ) -> bool {
        self.edit(Command::Paste {
            internal,
            html,
            text,
            mode: None,
        })
    }

    /// Quick styles for the gallery, in gallery order.
    pub fn styles(&mut self) -> Vec<bo_engine::gallery::StyleEntry> {
        let list = self
            .engine
            .command(Command::StyleList)
            .ok()
            .and_then(|r| r.styles)
            .unwrap_or_default();
        list.into_iter()
            .filter(|s| s.kind == "paragraph" && s.quick && s.recommended)
            .collect()
    }

    /// The document as PDF bytes.
    pub fn export_pdf(&self) -> Result<Vec<u8>, String> {
        self.engine.export_pdf().map_err(|e| e.to_string())
    }

    /// The document as `.docx` bytes.
    pub fn save_docx(&self) -> Result<Vec<u8>, String> {
        self.engine.save_docx().map_err(|e| e.to_string())
    }

    /// The document was written: it is no longer modified.
    pub fn mark_saved(&mut self) {
        self.engine.mark_saved();
        self.modified = false;
    }

    /// Id of the comment whose anchor holds the caret.
    pub fn comment_at_caret(&self) -> Option<i32> {
        let caret = self.selection?.focus;
        self.comments
            .iter()
            .find(|c| matches!((c.start, c.end), (Some(s), Some(e)) if s <= caret && caret <= e))
            .map(|c| c.id)
    }

    /// Track changes is on.
    pub fn track_changes(&self) -> bool {
        self.track
    }

    /// Run a command and return the engine's result (applied to the view).
    pub fn exec(&mut self, cmd: Command) -> Option<EditResult> {
        let r = self.engine.command(cmd).ok()?;
        self.apply(&r);
        self.caret_on = true;
        self.reveal_caret();
        Some(r)
    }

    /// Left edge and width (window pixels) of the page holding the caret, else the first
    /// visible page; for the ruler.
    pub fn ruler_page(&self) -> Option<(f32, f32)> {
        let page = self.caret.map(|c| c.page);
        let slots = self.visible();
        let slot = slots
            .iter()
            .find(|s| Some(s.index) == page)
            .or_else(|| slots.first())?;
        Some((slot.x as f32, slot.width as f32))
    }

    /// Top edge and height in window pixels of the page the caret is on (or the first
    /// visible page), for the vertical ruler.
    pub fn ruler_page_v(&self) -> Option<(f32, f32)> {
        let page = self.caret.map(|c| c.page);
        let slots = self.visible();
        let slot = slots
            .iter()
            .find(|s| Some(s.index) == page)
            .or_else(|| slots.first())?;
        Some((slot.y as f32, slot.height as f32))
    }

    /// How tracked changes and comments are shown (Review › markup).
    pub fn markup(&self) -> &bo_engine::markup::MarkupOptions {
        &self.markup
    }

    /// Zoom that fits the caret's page across the view (Page Width), or the whole page in
    /// it (One Page), with the page gap around it.
    pub fn fit_zoom(&self, whole_page: bool) -> f32 {
        let page = self
            .caret
            .and_then(|c| self.pages.get(c.page))
            .or_else(|| self.pages.first());
        let Some(page) = page else {
            return self.zoom;
        };
        let per_zoom = self.display_scale * 96.0 / 72.0;
        let gap = 2.0 * self.gap();
        let across = (self.viewport.0 - gap).max(1.0) / (page.width * per_zoom);
        let z = if whole_page {
            across.min((self.viewport.1 - gap).max(1.0) / (page.height * per_zoom))
        } else {
            across
        };
        (z * 100.0).floor() / 100.0
    }

    /// The header, footer or note being edited, if any.
    pub fn story(&self) -> Option<&bo_engine::story::StoryState> {
        self.story.as_ref()
    }

    /// Run a picture tools command (`{"cmd": "picture…"}`) on the selected picture.
    pub fn picture_command(&mut self, v: &serde_json::Value) -> bool {
        let Some(cmd) = bo_engine::picture_tools::PictureCommand::parse(v) else {
            return false;
        };
        match self.engine.doc_picture_command(cmd) {
            Ok(r) => {
                self.apply(&r);
                self.caret_on = true;
                true
            }
            Err(_) => false,
        }
    }

    /// Insert a picture file (`name` with its extension) at the selection.
    pub fn insert_picture(&mut self, bytes: Vec<u8>, name: &str) -> bool {
        match self.engine.insert_image(bytes, name) {
            Ok(r) => {
                let ok = r.error.is_none();
                self.apply(&r);
                self.caret_on = true;
                self.reveal_caret();
                ok
            }
            Err(_) => false,
        }
    }

    /// Page and whether a window position is in its header (`true`) or footer (`false`)
    /// area: above the top margin or below the bottom margin of the caret's section.
    pub fn hf_zone(&self, x: f32, y: f32) -> Option<(usize, bool)> {
        let slots = self.slots();
        slots
            .iter()
            .find(|p| y >= p.y as f32 && y < (p.y + p.height) as f32)?;
        let (page, _, py) = self.hit(x, y)?;
        let height = self.pages.get(page)?.height;
        let sec = &self.format.section;
        if py < sec.margin_top as f32 / 20.0 {
            Some((page, true))
        } else if py > height - sec.margin_bottom as f32 / 20.0 {
            Some((page, false))
        } else {
            None
        }
    }

    /// Headings of the document (outline levels set), with their pages.
    pub fn outline(&mut self) -> Vec<bo_engine::views::OutlineItem> {
        self.engine
            .command(Command::Outline)
            .ok()
            .and_then(|r| r.outline)
            .unwrap_or_default()
    }

    /// Search the document for `query`: results with their context; the matches are
    /// highlighted on the pages until [`DocView::clear_highlights`].
    pub fn find_results(&mut self, query: &str) -> Vec<bo_engine::views::FindResult> {
        let cmd = Command::FindResults {
            query: query.to_owned(),
            options: bo_edit::find::FindOptions::default(),
        };
        match self.engine.command(cmd) {
            Ok(r) => {
                self.highlights = r.highlights.clone().unwrap_or_default();
                r.results.unwrap_or_default()
            }
            Err(_) => Vec::new(),
        }
    }

    /// Check spelling and grammar of the whole document again (the engine caches checked
    /// paragraphs); `true` when the wavy lines changed.
    pub fn refresh_spelling(&mut self) -> bool {
        let marks = self
            .engine
            .command(Command::Spelling { pages: None })
            .ok()
            .and_then(|r| r.spelling)
            .unwrap_or_default();
        let changed = marks != self.spell;
        self.spell = marks;
        changed
    }

    /// Wavy-line marks of the last spelling check.
    pub fn spell_marks(&self) -> &[bo_engine::spell::SpellMark] {
        &self.spell
    }

    /// Spelling and grammar issues of the main text, in document order (proofing pane).
    pub fn proof_issues(&mut self) -> Vec<bo_engine::proof::ProofIssue> {
        self.engine
            .command(Command::ProofCheck)
            .ok()
            .and_then(|r| r.proof)
            .map(|p| {
                p.issues
                    .into_iter()
                    .filter(|i| i.story == bo_edit::proofing::Story::Body)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Select bytes `start..end` of body paragraph `para` and show them.
    pub fn select_range(&mut self, para: usize, start: usize, end: usize) -> bool {
        let sel = bo_edit::Selection {
            anchor: bo_edit::Pos {
                para,
                offset: start,
            },
            focus: bo_edit::Pos { para, offset: end },
        };
        self.edit(Command::SetSelection { selection: sel })
    }

    /// Number of spelling and grammar issues found by the last check.
    pub fn proofing_issues(&self) -> usize {
        self.spell.len()
    }

    /// The misspelt word or grammar issue at window point (`x`, `y`), with suggestions.
    pub fn spell_at(&mut self, x: f32, y: f32) -> Option<bo_engine::spell::SpellInfo> {
        let (page, px, py) = self.hit(x, y)?;
        self.engine
            .command(Command::SpellAt { page, x: px, y: py })
            .ok()
            .and_then(|r| r.spell)
    }

    /// Replace bytes `start..end` of body paragraph `para` by `text` (one undo step).
    pub fn replace_range(&mut self, para: usize, start: usize, end: usize, text: &str) -> bool {
        let sel = bo_edit::Selection {
            anchor: bo_edit::Pos {
                para,
                offset: start,
            },
            focus: bo_edit::Pos { para, offset: end },
        };
        self.edit(Command::SetSelection { selection: sel })
            && self.edit(Command::InsertText {
                text: text.to_owned(),
            })
    }

    /// Run a spelling command that ignores or adds a word, then refresh the wavy lines.
    pub fn spell_word(&mut self, add: bool, word: &str) -> bool {
        let word = word.to_owned();
        let cmd = if add {
            Command::SpellAddWord { word }
        } else {
            Command::SpellIgnore { word }
        };
        match self.engine.command(cmd) {
            Ok(r) => {
                self.spell = r.spelling.unwrap_or_default();
                true
            }
            Err(_) => false,
        }
    }

    /// Remove the search highlights.
    pub fn clear_highlights(&mut self) -> bool {
        let had = !self.highlights.is_empty();
        self.highlights.clear();
        had
    }

    /// Put the caret at the start of body paragraph `para` and show it near the top.
    pub fn goto_para(&mut self, para: usize) -> bool {
        let at = bo_edit::Pos { para, offset: 0 };
        let ok = self.edit(Command::SetSelection {
            selection: bo_edit::Selection {
                anchor: at,
                focus: at,
            },
        });
        if let Some(c) = self.caret {
            if let Some(top) = self.page_tops().get(c.page).copied() {
                self.scroll = top + c.y * self.scale() - self.gap();
                self.clamp_scroll();
            }
        }
        ok
    }

    /// Scroll so page `index` starts at the top of the view.
    pub fn show_page(&mut self, index: usize) -> bool {
        match self.page_tops().get(index).copied() {
            Some(top) => {
                self.scroll = top - self.gap();
                self.clamp_scroll();
                true
            }
            None => false,
        }
    }

    /// Body paragraph of the caret (the selection's focus).
    pub fn caret_para(&self) -> Option<usize> {
        self.selection.map(|s| s.focus.para)
    }

    /// Whether some text is selected (not just a caret).
    pub fn has_selection(&self) -> bool {
        !self.rects.is_empty()
    }

    /// Run an engine command that changes the document or selection.
    pub fn run_command(&mut self, cmd: Command) -> bool {
        self.edit(cmd)
    }

    /// The document's properties (title, author, dates) for the File screen.
    pub fn doc_properties(&mut self) -> Option<bo_engine::props::DocProperties> {
        self.engine
            .command(Command::DocProperties)
            .ok()
            .and_then(|r| r.doc_properties)
            .map(|p| *p)
    }

    /// Fonts offered by the font box (document fonts, bundled and installed families).
    pub fn font_list(&mut self) -> Vec<bo_engine::fonts::FontChoice> {
        self.engine
            .command(Command::FontList)
            .ok()
            .and_then(|r| r.fonts)
            .unwrap_or_default()
    }

    /// Run the engine command of interface command `id` (`format.bold`, …).
    pub fn run_action(&mut self, id: &str) -> bool {
        match crate::commands::command(id) {
            Some(cmd) => self.edit(cmd),
            None => false,
        }
    }

    /// Device pixels per point.
    pub fn scale(&self) -> f32 {
        self.zoom * self.display_scale * 96.0 / 72.0
    }

    /// Advance animations to the host clock `now_ms`; `true` while one is still running
    /// (the host keeps drawing frames until it returns `false`).
    pub fn advance(&mut self, now_ms: f64) -> bool {
        self.now = now_ms;
        let Some(g) = self.glide.as_mut() else {
            return false;
        };
        let start = *g.start.get_or_insert(now_ms);
        if now_ms - start >= GLIDE_MS {
            self.glide = None;
            return false;
        }
        true
    }

    /// Whether an animation is running.
    pub fn animating(&self) -> bool {
        self.glide.is_some()
    }

    /// Caret rectangle as drawn now (part way along a glide).
    fn caret_shown(&self) -> Option<[f32; 4]> {
        let to = self.caret_rect()?;
        let Some(g) = self.glide else {
            return Some(to);
        };
        let t = g
            .start
            .map_or(0.0, |s| ((self.now - s) / GLIDE_MS).clamp(0.0, 1.0)) as f32;
        // Ease-out cubic: fast start, gentle arrival.
        let e = 1.0 - (1.0 - t).powi(3);
        let lerp = |a: f32, b: f32| a + (b - a) * e;
        Some([
            lerp(g.from[0], to[0]),
            lerp(g.from[1], to[1]),
            to[2],
            lerp(g.from[3], to[3]),
        ])
    }

    /// Caret rectangle in view device pixels, when it is laid out and visible.
    pub fn caret_rect(&self) -> Option<[f32; 4]> {
        let c = self.caret?;
        let slot = self.slots().into_iter().find(|s| s.index == c.page)?;
        let s = self.scale();
        Some([
            slot.x as f32 + c.x * s,
            slot.y as f32 + c.y * s,
            self.display_scale.round().max(1.0),
            (c.h * s).max(1.0),
        ])
    }

    fn gap(&self) -> f32 {
        (GAP * self.display_scale).round()
    }

    fn page_px(&self, p: PageSize) -> (i32, i32) {
        let s = self.scale();
        ((p.width * s).ceil() as i32, (p.height * s).ceil() as i32)
    }

    /// Document y (device pixels from the top of the first gap) of each page's top.
    fn page_tops(&self) -> Vec<f32> {
        let gap = self.gap();
        let mut y = gap;
        self.pages
            .iter()
            .map(|p| {
                let top = y;
                y += self.page_px(*p).1 as f32 + gap;
                top
            })
            .collect()
    }

    fn content_height(&self) -> f32 {
        let gap = self.gap();
        gap + self
            .pages
            .iter()
            .map(|p| self.page_px(*p).1 as f32 + gap)
            .sum::<f32>()
    }

    /// Scroll offset, content height and viewport height in device pixels (scroll bar).
    pub fn scroll_info(&self) -> (f32, f32, f32) {
        (self.scroll, self.content_height(), self.viewport.1)
    }

    /// Scroll to `y` device pixels from the top (clamped); `true` when it moved.
    pub fn scroll_to(&mut self, y: f32) -> bool {
        let before = self.scroll;
        self.glide = None;
        self.scroll = y;
        self.clamp_scroll();
        self.scroll != before
    }

    fn clamp_scroll(&mut self) {
        let max = (self.content_height() - self.viewport.1).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max).round();
    }

    /// Every page with its position in the view (whole pixels, so images map 1:1).
    pub fn slots(&self) -> Vec<PageSlot> {
        let tops = self.page_tops();
        self.pages
            .iter()
            .zip(tops)
            .enumerate()
            .map(|(index, (p, top))| {
                let (width, height) = self.page_px(*p);
                let x = ((self.viewport.0 - width as f32) / 2.0)
                    .max(self.gap())
                    .floor();
                PageSlot {
                    index,
                    x: (x + self.origin.0) as i32,
                    y: (top - self.scroll + self.origin.1).round() as i32,
                    width,
                    height,
                }
            })
            .collect()
    }

    fn visible(&self) -> Vec<PageSlot> {
        let (top, bottom) = (
            self.origin.1 as i32,
            (self.origin.1 + self.viewport.1) as i32,
        );
        self.slots()
            .into_iter()
            .filter(|s| s.y + s.height > top && s.y < bottom)
            .collect()
    }

    /// Handle an input event; `true` when the view needs a new frame.
    pub fn handle(&mut self, input: Input) -> bool {
        match input {
            Input::Text(text) => {
                let text: String = text
                    .chars()
                    .filter(|c| !c.is_control() || *c == '\t')
                    .collect();
                !text.is_empty() && self.edit(Command::InsertText { text })
            }
            Input::Key(key, mods) => self.key(key, mods),
            Input::Wheel(dy) => {
                self.glide = None;
                let before = self.scroll;
                self.scroll += dy;
                self.clamp_scroll();
                self.scroll != before
            }
            Input::Press { x, y, count, shift } => match self.hit(x, y) {
                Some((page, px, py)) => self.edit(Command::Click {
                    page,
                    x: px,
                    y: py,
                    extend: shift,
                    count: count.max(1),
                }),
                None => false,
            },
            Input::Drag { x, y } => {
                // Past the top or bottom of the view, the drag scrolls the document.
                let (top, bottom) = (self.origin.1, self.origin.1 + self.viewport.1);
                let over = if y < top {
                    y - top
                } else if y > bottom {
                    y - bottom
                } else {
                    0.0
                };
                if over != 0.0 {
                    self.scroll += over * 0.5;
                    self.clamp_scroll();
                }
                match self.hit(x, y.clamp(top, bottom - 1.0)) {
                    Some((page, px, py)) => self.run(Command::DragTo { page, x: px, y: py }, false),
                    None => false,
                }
            }
            Input::Resize { width, height } => {
                self.viewport = (width.max(1.0), height.max(1.0));
                self.clamp_scroll();
                true
            }
            Input::Move { .. }
            | Input::Release { .. }
            | Input::Tick
            | Input::Maximized(_)
            | Input::Leave
            | Input::Zoom(_)
            | Input::Context { .. } => false,
            Input::Preedit(text) => {
                let changed = text != self.preedit;
                self.preedit = text;
                changed
            }
            Input::Scale(scale) => {
                if scale.is_finite() && scale > 0.0 && scale != self.display_scale {
                    self.display_scale = scale;
                    self.clamp_scroll();
                }
                true
            }
            Input::Blink => {
                self.caret_on = !self.caret_on;
                self.caret.is_some() && self.rects.is_empty()
            }
        }
    }

    fn key(&mut self, key: Key, m: Mods) -> bool {
        let motion = |plain, ctrl| if m.ctrl { ctrl } else { plain };
        let cmd = match key {
            Key::Left => Command::Move {
                motion: motion(Motion::Left, Motion::WordLeft),
                extend: m.shift,
            },
            Key::Right => Command::Move {
                motion: motion(Motion::Right, Motion::WordRight),
                extend: m.shift,
            },
            Key::Up => Command::Move {
                motion: motion(Motion::Up, Motion::ParaUp),
                extend: m.shift,
            },
            Key::Down => Command::Move {
                motion: motion(Motion::Down, Motion::ParaDown),
                extend: m.shift,
            },
            Key::Home => Command::Move {
                motion: motion(Motion::LineStart, Motion::DocStart),
                extend: m.shift,
            },
            Key::End => Command::Move {
                motion: motion(Motion::LineEnd, Motion::DocEnd),
                extend: m.shift,
            },
            Key::PageUp => Command::Move {
                motion: Motion::PageUp,
                extend: m.shift,
            },
            Key::PageDown => Command::Move {
                motion: Motion::PageDown,
                extend: m.shift,
            },
            Key::Backspace if m.ctrl => Command::DeleteWordBackward,
            Key::Backspace => Command::DeleteBackward,
            Key::Delete if m.ctrl => Command::DeleteWordForward,
            Key::Delete => Command::DeleteForward,
            Key::Enter => Command::SplitParagraph,
            Key::Tab => Command::InsertText { text: "\t".into() },
            Key::Letter('a') if m.ctrl => Command::SelectAll,
            Key::Letter('z') if m.ctrl => Command::Undo,
            Key::Letter('y') if m.ctrl => Command::Redo,
            Key::Letter('b') if m.ctrl => return self.run_action("format.bold"),
            Key::Letter('i') if m.ctrl => return self.run_action("format.italic"),
            Key::Letter('u') if m.ctrl => return self.run_action("format.underline"),
            Key::Letter('e') if m.ctrl => return self.run_action("format.alignCenter"),
            Key::Letter('l') if m.ctrl => return self.run_action("format.alignLeft"),
            Key::Letter('r') if m.ctrl => return self.run_action("format.alignRight"),
            Key::Letter('j') if m.ctrl => return self.run_action("format.justify"),
            Key::Escape | Key::Letter(_) | Key::F(_) => return false,
        };
        self.edit(cmd)
    }

    /// Run a command that moves the caret, then keep the caret in view.
    fn edit(&mut self, cmd: Command) -> bool {
        self.run(cmd, true)
    }

    fn run(&mut self, cmd: Command, reveal: bool) -> bool {
        match self.engine.command(cmd) {
            Ok(result) => {
                self.apply(&result);
                self.caret_on = true;
                if reveal {
                    self.reveal_caret();
                }
                true
            }
            Err(_) => false,
        }
    }

    fn apply(&mut self, r: &EditResult) {
        if r.info.pages.len() != self.pages.len()
            || r.info.pages.iter().zip(&self.pages).any(|(a, b)| a != b)
        {
            self.pages.clone_from(&r.info.pages);
            self.clamp_scroll();
        }
        self.stale.extend(r.dirty.iter().copied());
        // Glide the caret from where it is shown now, like desktop word processors do while
        // typing; long moves (another line far away, other page) jump.
        let shown = self.caret_shown();
        self.caret = r.caret;
        self.glide = match (shown, self.caret_rect()) {
            (Some(from), Some(to)) if from != to => {
                let d = (from[0] - to[0]).hypot(from[1] - to[1]);
                (d <= GLIDE_MAX * self.display_scale).then_some(Glide { from, start: None })
            }
            _ => None,
        };
        self.rects.clone_from(&r.rects);
        self.modified = r.modified;
        self.format.clone_from(&r.format);
        if let Some(review) = &r.review {
            self.track = review.track_changes;
            self.comments.clone_from(&review.comments);
            self.markup.clone_from(&review.markup);
        }
        self.selection = Some(r.selection);
        self.words = r.info.word_count;
        self.story.clone_from(&r.story);
    }

    fn reveal_caret(&mut self) {
        let Some(c) = self.caret else { return };
        let Some(top) = self.page_tops().get(c.page).copied() else {
            return;
        };
        let s = self.scale();
        let (y0, y1) = (top + c.y * s, top + (c.y + c.h) * s);
        let margin = self.gap();
        if y0 - margin < self.scroll {
            self.scroll = y0 - margin;
        } else if y1 + margin > self.scroll + self.viewport.1 {
            self.scroll = y1 + margin - self.viewport.1;
        }
        self.clamp_scroll();
    }

    /// Page and point (points from the page's top-left) under a view position.
    fn hit(&self, x: f32, y: f32) -> Option<(usize, f32, f32)> {
        let s = self.scale();
        let slots = self.slots();
        // The page under the pointer, else the nearest one vertically (clicks in the gaps).
        let slot = slots
            .iter()
            .find(|p| y >= p.y as f32 && y < (p.y + p.height) as f32)
            .or_else(|| {
                slots.iter().min_by_key(|p| {
                    let (t, b) = (p.y as f32, (p.y + p.height) as f32);
                    ((t - y).abs().min((b - y).abs())) as i64
                })
            })?;
        let px = ((x - slot.x as f32) / s).clamp(0.0, slot.width as f32 / s);
        let py = ((y - slot.y as f32) / s).clamp(0.0, slot.height as f32 / s);
        Some((slot.index, px, py))
    }

    /// Forget cached page images after the host's text rendering parameters change.
    pub fn invalidate_page_images(&mut self) {
        self.repaint_all = true;
    }

    /// Bring the images of the visible pages up to date, drawing only what changed.
    pub fn prepare(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        let scale = self.scale();
        let visible = self.visible();
        let (Some(first), Some(last)) = (visible.first(), visible.last()) else {
            return Ok(());
        };
        let keep = first.index.saturating_sub(OVERSCAN)..=last.index + OVERSCAN;
        let far: Vec<usize> = self
            .shown
            .keys()
            .copied()
            .filter(|i| !keep.contains(i))
            .collect();
        for index in far {
            store.remove(index);
            self.shown.remove(&index);
        }
        for index in keep {
            if index >= self.pages.len() {
                break;
            }
            let list = self.engine.page(index).map_err(|e| e.to_string())?;
            let band = if self.repaint_all {
                None
            } else {
                match self.shown.get(&index) {
                    Some((s, _)) if *s != scale => None,
                    Some((_, old)) if self.stale.contains(&index) => {
                        match changed_region(old, list) {
                            Change::None => {
                                self.stale.remove(&index);
                                continue;
                            }
                            change => change.device_band(scale),
                        }
                    }
                    Some(_) => continue,
                    None => None,
                }
            };
            store.update(index, list, &self.engine, scale, band)?;
            self.shown.insert(index, (scale, list.clone()));
            self.stale.remove(&index);
        }
        self.repaint_all = false;
        Ok(())
    }

    /// Paint a frame: background, pages, selection and caret.
    pub fn paint(&self, p: &mut dyn Painter) {
        let (ox, oy) = self.origin;
        let (vw, vh) = self.viewport;
        p.push_clip(ox, oy, vw, vh);
        p.fill_rect(ox, oy, vw, vh, CANVAS_BG);
        let s = self.scale();
        let visible = self.visible();
        for slot in &visible {
            let edge = self.display_scale.round().max(1.0);
            p.fill_rect(
                slot.x as f32 - edge,
                slot.y as f32 - edge,
                slot.width as f32 + 2.0 * edge,
                slot.height as f32 + 2.0 * edge,
                PAGE_SHADOW,
            );
            if self.shown.contains_key(&slot.index) {
                p.page(slot.index, slot.x, slot.y);
            }
            // Wavy lines under misspelt words (red) and grammar issues (blue), as a desktop
            // word processor checks as you type.
            for m in self.spell.iter().filter(|m| m.page == slot.index) {
                let color = if m.grammar { SPELL_GRAMMAR } else { SPELL_RED };
                wavy(
                    p,
                    slot.x as f32 + m.x * s,
                    (slot.y as f32 + m.y * s + 1.5 * self.display_scale).round(),
                    m.w * s,
                    self.display_scale,
                    color,
                );
            }
            // Search results (navigation pane) in a translucent highlighter yellow.
            for r in self.highlights.iter().filter(|r| r.page == slot.index) {
                p.fill_rect(
                    slot.x as f32 + r.x * s,
                    slot.y as f32 + r.y * s,
                    r.w * s,
                    r.h * s,
                    Rgba(255, 238, 0, 110),
                );
            }
            for r in self.rects.iter().filter(|r| r.page == slot.index) {
                p.fill_rect(
                    slot.x as f32 + r.x * s,
                    slot.y as f32 + r.y * s,
                    r.w * s,
                    r.h * s,
                    SELECTION,
                );
            }
        }
        if !self.preedit.is_empty() {
            if let Some([x, y, _, h]) = self.caret_rect() {
                // Composition in the paragraph's font, underlined, over a paper patch.
                let size = self
                    .format
                    .size_half_points
                    .map_or(h * 0.75, |hp| hp as f32 / 2.0 * self.scale());
                let font = crate::Font {
                    family: self.format.font.as_deref().unwrap_or("Times New Roman"),
                    size,
                    weight: if self.format.bold { 700 } else { 400 },
                    italic: self.format.italic,
                    underline: true,
                };
                let w = self.preedit.chars().count() as f32 * size * 0.6;
                p.fill_rect(x, y, w, h, Rgba(255, 255, 255, 255));
                p.text_font(
                    &self.preedit,
                    x,
                    y + (h - size * 1.2) / 2.0,
                    &font,
                    Rgba(0, 0, 0, 255),
                );
            }
            p.pop_clip();
            return;
        }
        if (self.caret_on || self.glide.is_some()) && self.rects.is_empty() {
            if let Some([x, y, w, h]) = self.caret_shown() {
                p.fill_rect(x.round(), y.round(), w, h.round(), CARET);
            }
        }
        p.pop_clip();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Store {
        full: Vec<usize>,
        bands: Vec<(usize, [u32; 4])>,
        removed: Vec<usize>,
    }

    impl PageStore for Store {
        fn update(
            &mut self,
            index: usize,
            _: &PageList,
            _: &Engine,
            _: f32,
            band: Option<[u32; 4]>,
        ) -> Result<(), String> {
            match band {
                Some(b) => self.bands.push((index, b)),
                None => self.full.push(index),
            }
            Ok(())
        }
        fn remove(&mut self, index: usize) {
            self.removed.push(index);
        }
    }

    #[derive(Default)]
    struct Recorder {
        pages: Vec<(usize, i32, i32)>,
        rects: usize,
    }

    impl Painter for Recorder {
        fn clear(&mut self, _: Rgba) {}
        fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba) {
            self.rects += 1;
        }
        fn page(&mut self, index: usize, x: i32, y: i32) {
            self.pages.push((index, x, y));
        }
        fn text(&mut self, _: &str, _: f32, _: f32, _: crate::TextStyle, _: Rgba) {}
        fn path(&mut self, _: &[bo_render::PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {}
        fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn pop_clip(&mut self) {}
    }

    fn view() -> DocView {
        let mut engine = Engine::with_bundled_fonts();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/corpus/doc/phase1-sample.docx"
        );
        engine
            .open(&std::fs::read(path).unwrap(), "phase1-sample.docx")
            .unwrap();
        let mut v = DocView::new(engine).unwrap();
        v.handle(Input::Scale(1.25));
        v.handle(Input::Resize {
            width: 1600.0,
            height: 900.0,
        });
        v
    }

    #[test]
    fn typing_redraws_a_band_and_keeps_pages_on_whole_pixels() {
        let mut v = view();
        let mut store = Store::default();
        v.prepare(&mut store).unwrap();
        assert_eq!(store.full, vec![0, 1], "visible page and the next");
        assert!(v.handle(Input::Text("x".into())));
        v.prepare(&mut store).unwrap();
        assert_eq!(store.full, vec![0, 1], "no full redraw after typing");
        assert_eq!(store.bands.len(), 1);
        let (page, [_, _, w, h]) = store.bands[0];
        assert_eq!(page, 0);
        let slot = v.slots()[0];
        assert!(u64::from(w) * u64::from(h) * 10 < (slot.width * slot.height) as u64);
        let mut rec = Recorder::default();
        v.paint(&mut rec);
        assert_eq!(rec.pages[0], (0, slot.x, slot.y));
        assert!(v.caret_rect().is_some());
    }

    #[test]
    fn changing_text_parameters_repaints_cached_pages_completely() {
        let mut v = view();
        let mut store = Store::default();
        v.prepare(&mut store).unwrap();
        assert_eq!(store.full, vec![0, 1]);
        store.full.clear();
        v.prepare(&mut store).unwrap();
        assert_eq!(store.full, [] as [usize; 0]);
        v.invalidate_page_images();
        v.prepare(&mut store).unwrap();
        assert_eq!(store.full, vec![0, 1]);
        assert_eq!(store.bands, [] as [(usize, [u32; 4]); 0]);
    }

    #[test]
    fn changing_text_parameters_preserves_eviction_of_old_pages() {
        let mut v = view();
        let mut store = Store::default();
        v.prepare(&mut store).unwrap();
        assert_eq!(store.full, vec![0, 1]);
        v.invalidate_page_images();
        v.handle(Input::Wheel(10_000.0));
        v.prepare(&mut store).unwrap();
        assert!(store.removed.contains(&0));
        assert!(store.removed.contains(&1));
        assert!(store.full.iter().any(|&page| page >= 3));
    }

    #[test]
    fn keys_wheel_and_clicks() {
        let mut v = view();
        let mut store = Store::default();
        v.prepare(&mut store).unwrap();
        assert!(v.handle(Input::Wheel(400.0)));
        assert_eq!(v.slots()[0].y, (16.0f32 * 1.25).round() as i32 - 400);
        // A click on the first visible page places the caret there.
        let slot = v.visible()[0];
        assert!(v.handle(Input::Press {
            x: (slot.x + 300) as f32,
            y: (slot.y.max(0) + 200) as f32,
            count: 1,
            shift: false,
        }));
        assert!(v.caret.is_some());
        assert!(v.handle(Input::Key(
            Key::Letter('a'),
            Mods {
                ctrl: true,
                ..Mods::default()
            }
        )));
        assert!(!v.rects.is_empty(), "select all");
        assert!(v.handle(Input::Key(Key::Right, Mods::default())));
        assert_eq!(v.rects.len(), 0);
        let before = v.engine.page(0).unwrap().clone();
        v.handle(Input::Key(
            Key::Letter('a'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ));
        v.handle(Input::Text("z".into()));
        v.handle(Input::Key(
            Key::Letter('z'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ));
        assert_eq!(v.engine.page(0).unwrap(), &before, "undo restores the page");
    }

    #[test]
    fn caret_glides_to_its_new_place_then_stops() {
        let mut v = view();
        v.advance(0.0);
        let before = v.caret_shown().unwrap();
        v.handle(Input::Text("x".into()));
        assert!(v.animating());
        assert!(v.advance(1000.0), "the glide starts at the next frame");
        assert_eq!(
            v.caret_shown().unwrap(),
            before,
            "first frame: where it was"
        );
        assert!(v.advance(1035.0));
        let mid = v.caret_shown().unwrap();
        let end = v.caret_rect().unwrap();
        assert!(
            mid[0] > before[0] && mid[0] < end[0],
            "{before:?} {mid:?} {end:?}"
        );
        assert!(!v.advance(1100.0), "done after the glide");
        assert_eq!(v.caret_shown().unwrap(), end);
        // Scrolling cancels a glide.
        v.handle(Input::Text("y".into()));
        v.handle(Input::Wheel(50.0));
        assert!(!v.animating());
    }
}
