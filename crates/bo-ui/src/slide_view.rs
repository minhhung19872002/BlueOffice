//! BlueSlide's editing area (N6): the slide thumbnails pane, the current slide on the grey
//! canvas and the notes pane, with shape selection, text editing in shapes and the slide show.
//!
//! Slides are drawn from the engine's display lists into page images through the same
//! [`PageStore`] path as BlueDoc's pages (crisp text, only the changed band redrawn after an
//! edit). Images are keyed by the slide's stable id (`p:sldId@id`), so inserting, deleting or
//! moving slides keeps every other image; switching slides shows a kept image at once.
//! Geometry measured on the reference presentation app at 125 % (docs/ui/REPORT-slide.md).
use crate::{Input, Key, Mods, PageStore, Painter, TextStyle};
use bo_engine::slide_edit::geom::{BoxR, Handle, HandleKind};
use bo_engine::slide_edit::rich::TextEditOut;
use bo_engine::slide_edit::show::ShowOut;
use bo_engine::slide_edit::text::{Align, FormatState, TextFormat};
use bo_engine::slide_edit::{Preview, ShapeInfo, SlideCommand, SlideEditResult, Transform};
use bo_engine::Engine;
use bo_render::{changed_region, Change, PageList, PathSeg, Rgba};
use std::collections::{HashMap, HashSet};

/// Grey behind the slide and the thumbnails (reference: 230).
pub const SLIDE_CANVAS: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
/// One-pixel frame around the slide on the canvas (reference: 198).
const SLIDE_EDGE: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
/// Frame of a thumbnail that is not selected (reference: 200, 198, 196).
const THUMB_EDGE: Rgba = Rgba(0xc8, 0xc6, 0xc4, 255);
/// Slide numbers beside the thumbnails (reference: 72).
const THUMB_NUMBER: Rgba = Rgba(0x48, 0x48, 0x48, 255);
/// Lines between the panes (reference: 191).
const SPLITTER: Rgba = Rgba(0xbf, 0xbf, 0xbf, 255);
/// Selection frame and handle outline of shapes (reference: 97).
const FRAME: Rgba = Rgba(0x61, 0x61, 0x61, 255);
const HANDLE_FILL: Rgba = Rgba(255, 255, 255, 255);
/// Text selection inside a shape: neutral grey over the text (reference: 185 over white).
const TEXT_SELECTION: Rgba = Rgba(0, 0, 0, 70);
/// Prompt text of empty placeholders.
const PROMPT_INK: Rgba = Rgba(0x00, 0x00, 0x00, 255);
/// Dashed outline of empty placeholders.
const PLACEHOLDER_EDGE: Rgba = Rgba(0x9a, 0x9a, 0x9a, 255);
/// Slide frames of the Handout Master view (reference: dots of 32–160 grey).
const HANDOUT_SLOT: Rgba = Rgba(0x50, 0x50, 0x50, 255);
/// Wavy lines of spelling (red) and grammar (blue) issues, as BlueDoc draws them.
const SPELL_RED: Rgba = Rgba(0xe8, 0x11, 0x23, 255);
const SPELL_GRAMMAR: Rgba = Rgba(0x2b, 0x57, 0xd8, 255);
/// Selection rectangle dragged on the empty slide.
/// Reference: 1 px edge of grey 115 and the same grey at half opacity inside (white shows
/// 185, a 46/117/182 fill 81/116/149).
const MARQUEE_EDGE: Rgba = Rgba(0x73, 0x73, 0x73, 255);
const MARQUEE_FILL: Rgba = Rgba(0x73, 0x73, 0x73, 0x80);
/// Smart guides while moving shapes: 1 px dashes of 209/139/121, 7 px period at 125 %
/// (reference).
const GUIDE: Rgba = Rgba(0xd1, 0x8b, 0x79, 255);
const CARET: Rgba = Rgba(0, 0, 0, 255);
const NOTES_HINT: Rgba = Rgba(0x52, 0x52, 0x52, 255);
const BLACK: Rgba = Rgba(0, 0, 0, 255);
/// BlueSlide accent (tokens.json `slide` accent): selected thumbnail frame and number.
pub const ACCENT: Rgba = Rgba(0xc2, 0x18, 0x5b, 255);
const ACCENT_TEXT: Rgba = Rgba(0xb5, 0x19, 0x5a, 255);

/// Image keys: main slide images use the slide id, thumbnails and show images an offset.
const THUMB_KEY: usize = 1 << 40;
const SHOW_KEY: usize = 2 << 40;
/// Main slide images kept besides the current one (neighbours, recently visited).
const MAIN_CACHE: usize = 12;
/// Slide-show images kept.
const SHOW_CACHE: usize = 4;
/// Keyboard nudge of a selected shape (points): one step of the default grid; Ctrl: fine.
const NUDGE: f64 = 6.0;
const NUDGE_FINE: f64 = 1.0;
/// Pointer travel (device pixels) before a press on a shape becomes a move.
const DRAG_SLOP: f32 = 3.0;

/// Which part of the editing area has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The slide thumbnails.
    Thumbs,
    /// The slide being edited.
    Canvas,
    /// The speaker notes.
    Notes,
}

/// A pointer gesture on the slide.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    /// Pressed on a selected shape: moves it once the pointer travels.
    Move { from: (f32, f32), moved: bool },
    /// Dragging a resize or the rotation handle.
    Handle { kind: HandleKind, from: (f32, f32) },
    /// Extending the text selection.
    Text,
    /// Pressed on the empty slide: a selection rectangle from `from` to `to`.
    Marquee { from: (f32, f32), to: (f32, f32) },
    /// Pressed on a slide thumbnail (pane or sorter): drags the chosen slides once the
    /// pointer travels; `to` is the insertion index.
    Slides {
        from: (f32, f32),
        moved: bool,
        to: Option<usize>,
    },
}

#[path = "slide_sorter.rs"]
mod sorter;
pub use sorter::ViewMode;

#[path = "slide_ink.rs"]
mod ink;
pub use ink::InkTool;

#[path = "slide_present.rs"]
mod present;
pub use present::ShowPlan;

/// The running slide show.
#[derive(Debug, Clone, PartialEq)]
struct Show {
    index: usize,
    /// Past the last slide: the black end screen.
    ended: bool,
    /// Clicks played on this slide.
    step: usize,
    /// What the engine says the slide plays (layers, effects, transition).
    info: Option<ShowOut>,
    /// Transition into this slide: the slide it comes from (`None`: black), when it started.
    trans: Option<(Option<usize>, crate::slide_show::Transition, f64)>,
    /// Start of the current step's effects while they run.
    fx_start: Option<f64>,
    /// Slide id and scale of the stored layer images.
    layers: Option<(usize, f32)>,
    /// Slide id and scale of the stored Morph images.
    morph: Option<(usize, f32)>,
    /// When the slide was fully shown (automatic advance counts from there).
    arrived: f64,
    /// A preview on the editing canvas (Transitions / Animations tab): `Some(true)` plays
    /// the effects one click after the other, `Some(false)` the transition only.
    preview: Option<bool>,
}

impl Show {
    fn at(index: usize) -> Self {
        Self {
            index,
            ended: false,
            step: 0,
            info: None,
            trans: None,
            fx_start: None,
            layers: None,
            morph: None,
            arrived: 0.0,
            preview: None,
        }
    }
}

/// Image keys of the show layers of the current slide (one slide's layers at a time).
const LAYER_KEY: usize = 3 << 40;
/// Image keys of the layers' pieces (paragraphs, letters), one image per layer.
const PIECES_KEY: usize = 4 << 40;
/// Image keys of a running Morph (`slide_show::morph_keys` offsets).
const MORPH_KEY: usize = 5 << 40;
/// Image key of the slide under the shapes being resized (see [`SlideView::ghost`]).
const GHOST_KEY: usize = 12 << 40;

/// The slide without the shapes being resized or turned.
struct Ghost {
    /// Slide index.
    slide: usize,
    /// Its display list, until stored.
    list: Option<bo_render::PageList>,
    /// Zoom it was stored at.
    scale: f32,
}

/// Device-pixel rectangle `[x, y, w, h]`.
type R = [f32; 4];

fn inside(r: R, x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

/// Byte index of UTF-16 offset `u` in `s` (clamped to the end).
fn byte_at(s: &str, u: usize) -> usize {
    let mut n = 0;
    for (i, c) in s.char_indices() {
        if n >= u {
            return i;
        }
        n += c.len_utf16();
    }
    s.len()
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// UTF-16 offset of the previous / next character boundary from `u`.
fn step_char(s: &str, u: usize, forward: bool) -> usize {
    let b = byte_at(s, u);
    let c = if forward {
        s[b..].chars().next()
    } else {
        s[..b].chars().next_back()
    };
    match c {
        Some(c) if forward => u + c.len_utf16(),
        Some(c) => u - c.len_utf16(),
        None => u,
    }
}

/// UTF-16 offset of the previous / next word start from `u` (Ctrl + arrows).
fn step_word(s: &str, u: usize, forward: bool) -> usize {
    let b = byte_at(s, u);
    let word = |c: char| c.is_alphanumeric() || c == '_';
    if forward {
        let rest = &s[b..];
        let mut it = rest.char_indices().peekable();
        // Skip the current word, then the spaces after it.
        while let Some((_, c)) = it.peek().copied() {
            if !word(c) {
                break;
            }
            it.next();
        }
        while let Some((_, c)) = it.peek().copied() {
            if word(c) || c == '\n' {
                break;
            }
            it.next();
        }
        let off = it.peek().map_or(rest.len(), |(i, _)| *i);
        u + utf16_len(&rest[..off])
    } else {
        let before = &s[..b];
        let chars: Vec<char> = before.chars().collect();
        let mut i = chars.len();
        while i > 0 && !word(chars[i - 1]) {
            i -= 1;
        }
        while i > 0 && word(chars[i - 1]) {
            i -= 1;
        }
        chars[..i].iter().map(|c| c.len_utf16()).sum()
    }
}

/// UTF-16 range of the word around offset `u` (double click).
fn word_at(s: &str, u: usize) -> (usize, usize) {
    let chars: Vec<char> = s.chars().collect();
    let mut idx = 0;
    let mut n = 0;
    while idx < chars.len() && n < u {
        n += chars[idx].len_utf16();
        idx += 1;
    }
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let (mut a, mut b) = (idx, idx);
    while a > 0 && word(chars[a - 1]) {
        a -= 1;
    }
    while b < chars.len() && word(chars[b]) {
        b += 1;
    }
    // A word takes its trailing spaces, as desktop presentation apps select it.
    while b < chars.len() && chars[b] == ' ' {
        b += 1;
    }
    let off = |k: usize| chars[..k].iter().map(|c| c.len_utf16()).sum();
    (off(a), off(b))
}

/// UTF-16 range of the paragraph around offset `u`, its paragraph mark included (triple
/// click, as the reference selects it).
fn para_at(s: &str, u: usize) -> (usize, usize) {
    let b = byte_at(s, u);
    let start = s[..b].rfind('\n').map_or(0, |i| i + 1);
    let end = s[b..].find('\n').map_or(s.len(), |i| b + i + 1);
    (utf16_len(&s[..start]), utf16_len(&s[..end]))
}

/// Offset `u` starts a paragraph.
fn para_start(s: &str, u: usize) -> bool {
    let b = byte_at(s, u);
    b == 0 || s[..b].ends_with('\n')
}

#[path = "slide_colormode.rs"]
pub mod colormode;

#[path = "slide_thumb_sections.rs"]
mod thumb_sections;

#[path = "slide_show_tools.rs"]
pub mod show_tools;

/// The editing area of one presentation.
pub struct SlideView {
    /// Ids of the sections collapsed in the thumbnails pane.
    collapsed: Vec<String>,
    /// Colour, Grayscale or Black and White (View tab).
    color_mode: colormode::ColorMode,
    /// The selection's frame and handles are not drawn (Remove Background).
    hide_handles: bool,
    /// While a handle is dragged: the slide without the selected shapes (to store as
    /// [`GHOST_KEY`]) — the shapes' new size is drawn half see-through over it, the
    /// original hidden, as the reference does.
    ghost: Option<Ghost>,
    /// [`GHOST_KEY`] holds an image that is no longer needed.
    ghost_stored: bool,
    /// Ink drawn in the show that just ended, until the user keeps or discards it.
    pending_ink: Option<Vec<bo_engine::slide_edit::present::InkSlide>>,
    /// Prompts of empty title, subtitle and body placeholders (interface language).
    /// Prompts: title, subtitle, body, then header and footer (page master views).
    prompts: [String; 5],
    engine: Engine,
    /// Stable id of each slide, in order.
    ids: Vec<usize>,
    /// Slide size in points.
    slide_size: (f32, f32),
    current: usize,
    origin: (f32, f32),
    size: (f32, f32),
    display_scale: f32,
    /// Zoom factor; `None` fits the slide in the canvas.
    zoom: Option<f32>,
    /// Scroll of the canvas when the zoomed slide is taller than it (device pixels).
    canvas_scroll: f32,
    thumb_scroll: f32,
    show_notes: bool,
    /// Slide show keys and pointer tools (blank screens, pen, laser).
    show_tools: show_tools::ShowTools,
    /// Shift / Ctrl held with the mouse button (host: the mouse message's key flags).
    pointer_mods: Mods,
    /// Shift + press on a selected shape: it leaves the selection on release unless the
    /// press became a (constrained) move (slide point of the press).
    shift_toggle: Option<(f64, f64)>,
    /// Second click on a selected shape: its text takes the caret on release (shape id,
    /// slide point) unless the press became a move.
    pending_text: Option<(u32, f64, f64)>,
    /// A move / resize / rotation applied live while the pointer drags (history length
    /// before it): the shapes themselves follow the pointer, as the reference draws them.
    live_since: Option<usize>,
    /// Wavy lines under spelling (red) and grammar (blue) issues of the current slide
    /// (slide points: from, to, grammar), checked again on the next tick after a change.
    spell: Vec<([f64; 2], [f64; 2], bool)>,
    spell_dirty: bool,
    /// Slide Sorter zoom (1 = 100 %).
    sort_zoom: f32,
    /// Draw tab tool in use, and the stroke being drawn (slide points).
    ink_tool: Option<InkTool>,
    ink_stroke: Vec<[f64; 2]>,
    focus: Focus,
    state: Option<SlideEditResult>,
    /// Display list each stored image shows, with its scale.
    shown: HashMap<usize, (f32, PageList)>,
    /// Images whose slide changed since they were drawn.
    stale: HashSet<usize>,
    /// Images to redraw whole (text rendering changed).
    full: HashSet<usize>,
    /// Main images in the order they were last shown (most recent last).
    recent: Vec<usize>,
    caret_on: bool,
    drag: Option<Drag>,
    preview: Option<Preview>,
    show: Option<Show>,
    /// Notes caret (UTF-16 offset into the notes text) while the notes pane has the keyboard.
    notes_caret: usize,
    /// The window's full size (device pixels): the slide show covers it.
    window: (f32, f32),
    /// Interface texts: empty notes hint, end of the slide show.
    labels: (String, String),
    /// Presenter View texts: "Next Slide", "Slide {{n}} of {{total}}".
    present_labels: (String, String),
    /// How the running show plays (order, loop, timings, presenter, rehearsal).
    presenting: Option<present::Presenting>,
    /// Timings of the last rehearsal, until the window takes them.
    rehearsed: Option<Vec<bo_engine::slide_edit::present::SlideTiming>>,
    /// Advance of the notes text before the caret on its line (device pixels).
    notes_caret_x: f32,
    /// Width of one digit of the thumbnail numbers (device pixels).
    digit_w: f32,
    /// Host clock (ms) for slide show animations.
    clock: f64,
    /// Slide Sorter view.
    sorter: bool,
    /// Reading view (the show inside the window's editing area).
    reading: bool,
    /// Scroll of the sorter (device pixels).
    sorter_scroll: f32,
    /// Slides chosen in the sorter / thumbnails pane (Shift extends).
    slide_sel: Vec<usize>,
}

impl SlideView {
    /// A view of the presentation already open in `engine`.
    pub fn new(engine: Engine) -> Result<Self, String> {
        let mut view = Self {
            collapsed: Vec::new(),
            color_mode: colormode::ColorMode::Color,
            hide_handles: false,
            ghost: None,
            ghost_stored: false,
            pending_ink: None,
            prompts: [
                "Click to add title".into(),
                "Click to add subtitle".into(),
                "Click to add text".into(),
                "Header".into(),
                "Footer".into(),
            ],
            engine,
            ids: Vec::new(),
            slide_size: (960.0, 540.0),
            current: 0,
            origin: (0.0, 0.0),
            size: (800.0, 600.0),
            display_scale: 1.0,
            zoom: None,
            canvas_scroll: 0.0,
            thumb_scroll: 0.0,
            show_notes: true,
            show_tools: show_tools::ShowTools::default(),
            pointer_mods: Mods::default(),
            shift_toggle: None,
            pending_text: None,
            live_since: None,
            spell: Vec::new(),
            spell_dirty: true,
            sort_zoom: 1.0,
            ink_tool: None,
            ink_stroke: Vec::new(),
            focus: Focus::Canvas,
            state: None,
            shown: HashMap::new(),
            stale: HashSet::new(),
            full: HashSet::new(),
            recent: Vec::new(),
            caret_on: true,
            drag: None,
            preview: None,
            show: None,
            notes_caret: 0,
            window: (800.0, 600.0),
            labels: ("Click to add notes".into(), "End of slide show".into()),
            present_labels: ("Next Slide".into(), "Slide {{n}} of {{total}}".into()),
            presenting: None,
            rehearsed: None,
            notes_caret_x: 0.0,
            digit_w: 10.0,
            clock: 0.0,
            sorter: false,
            reading: false,
            sorter_scroll: 0.0,
            slide_sel: Vec::new(),
        };
        if view.engine.slide_count() == 0 {
            return Err("no slides".into());
        }
        view.run(SlideCommand::State);
        view.refresh_ids();
        Ok(view)
    }

    /// The presentation engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Mutable engine access (host file operations).
    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// Number of slides.
    pub fn slide_count(&self) -> usize {
        self.ids.len()
    }

    /// Index of the current slide.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Which pane has the keyboard.
    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// The presentation changed since it was opened or saved.
    pub fn modified(&self) -> bool {
        self.state.as_ref().is_some_and(|s| s.modified)
    }

    /// Formatting of the selection (ribbon state).
    pub fn format(&self) -> FormatState {
        self.state
            .as_ref()
            .map(|s| s.format.clone())
            .unwrap_or_default()
    }

    /// Undo / redo are available.
    pub fn history(&self) -> (bool, bool) {
        self.state
            .as_ref()
            .map_or((false, false), |s| (s.can_undo, s.can_redo))
    }

    /// Selected shapes of the current slide.
    pub fn selection(&self) -> &[ShapeInfo] {
        self.state.as_ref().map_or(&[], |s| s.selection.as_slice())
    }

    /// Text being edited inside a shape.
    pub fn text_edit(&self) -> Option<&TextEditOut> {
        self.state.as_ref().and_then(|s| s.text_edit.as_ref())
    }

    /// Speaker notes of the current slide.
    pub fn notes(&self) -> &str {
        self.state.as_ref().map_or("", |s| s.notes.as_str())
    }

    /// Whether the notes pane is shown.
    pub fn notes_shown(&self) -> bool {
        self.show_notes
    }

    /// Show or hide the notes pane.
    pub fn set_notes_shown(&mut self, on: bool) {
        self.show_notes = on;
        if !on && self.focus == Focus::Notes {
            self.focus = Focus::Canvas;
        }
        self.clamp_canvas_scroll();
    }

    /// Slide layouts of the deck (index, name).
    pub fn layouts(&self) -> Vec<(usize, String)> {
        self.state.as_ref().map_or_else(Vec::new, |s| {
            s.layouts
                .iter()
                .map(|l| (l.index, l.name.clone()))
                .collect()
        })
    }

    /// The slide show is running full screen (Reading view plays it in the window).
    pub fn showing(&self) -> bool {
        self.show.is_some() && !self.reading && !self.previewing()
    }

    /// A transition or effects preview plays on the editing canvas.
    pub fn previewing(&self) -> bool {
        self.show.as_ref().is_some_and(|s| s.preview.is_some())
    }

    /// Preview the current slide on the canvas: its transition (`effects` false, from the
    /// previous slide) or its effects one after the other, as the reference does when a
    /// transition or an effect is chosen.
    pub fn start_preview(&mut self, effects: bool) -> bool {
        if self.text_edit().is_some() {
            self.run(SlideCommand::TextExit);
        }
        let cur = self.current;
        let mut s = Show::at(cur);
        s.preview = Some(effects);
        self.show = Some(s);
        self.enter_slide(cur.checked_sub(1));
        let Some(s) = self.show.as_mut() else {
            return false;
        };
        if effects {
            s.trans = None;
            let clicks = s.info.as_ref().map_or(0, |i| i.clicks);
            if clicks == 0 && s.fx_start.is_none() {
                self.show = None;
                return false;
            }
            if s.fx_start.is_none() {
                s.step = 1;
                s.fx_start = Some(self.clock);
            }
        } else if s.trans.is_none() {
            self.show = None;
            return false;
        }
        true
    }

    /// The presentation as `.pptx` bytes.
    pub fn save_pptx(&self) -> Result<Vec<u8>, String> {
        self.engine.save_pptx().map_err(|e| e.to_string())
    }

    /// The presentation was written: no longer modified.
    pub fn mark_saved(&mut self) {
        self.engine.mark_saved();
        self.run(SlideCommand::State);
    }

    /// Prompts of empty title, subtitle and body placeholders.
    pub fn set_prompts(&mut self, title: &str, subtitle: &str, body: &str) {
        self.prompts[..3].clone_from_slice(&[
            title.to_owned(),
            subtitle.to_owned(),
            body.to_owned(),
        ]);
    }

    /// Prompts of the header and footer placeholders of the handout / notes master views.
    pub fn set_hf_prompts(&mut self, header: &str, footer: &str) {
        self.prompts[3] = header.to_owned();
        self.prompts[4] = footer.to_owned();
    }

    /// A notes / handout master view is open: the reference shows the page alone, without
    /// the thumbnails and notes panes.
    pub fn in_page_master(&self) -> bool {
        self.state.as_ref().is_some_and(|s| s.page_master.is_some())
    }

    /// Interface texts in the window's language: the empty notes hint and the end screen of
    /// the slide show.
    pub fn set_labels(&mut self, notes_hint: &str, show_end: &str) {
        self.labels = (notes_hint.to_owned(), show_end.to_owned());
    }

    /// Size of the notes text (device pixels) for a UI text size.
    pub fn notes_size(text_size: f32) -> f32 {
        (text_size * 1.33).round()
    }

    /// Measure the notes caret position with the platform's text engine.
    pub fn measure_notes(&mut self, text: &dyn crate::TextMeasure, text_size: f32) {
        let notes = self.notes();
        let before = &notes[..byte_at(notes, self.notes_caret)];
        let col = before.rsplit('\n').next().unwrap_or("");
        let style = TextStyle {
            size: Self::notes_size(text_size),
            bold: false,
        };
        self.notes_caret_x = text.measure(col, style).0;
        self.digit_w = text.measure("0", self.number_style()).0;
    }

    /// Thumbnail numbers: digits 16 px tall at 125 % on the reference.
    fn number_style(&self) -> TextStyle {
        TextStyle {
            size: (16.8 * self.display_scale).round(),
            bold: false,
        }
    }

    /// Forget cached images after the host's text rendering changed.
    pub fn invalidate_page_images(&mut self) {
        self.full.extend(self.shown.keys().copied());
    }

    /// Place the area at `(x, y)` with `width` × `height` device pixels.
    pub fn set_frame(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.origin = (x, y);
        self.size = (width.max(1.0), height.max(1.0));
        self.clamp_canvas_scroll();
        self.clamp_thumb_scroll();
    }

    /// Full window size (the slide show covers the window).
    pub fn set_window(&mut self, width: f32, height: f32) {
        self.window = (width.max(1.0), height.max(1.0));
    }

    /// Display scale (device pixels per logical pixel).
    pub fn set_display_scale(&mut self, scale: f32) {
        if scale.is_finite() && scale > 0.0 {
            self.display_scale = scale;
            self.clamp_thumb_scroll();
            self.clamp_canvas_scroll();
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.display_scale).round()
    }

    // ------------------------------------------------------------------ geometry

    /// Width of the thumbnails pane (reference: 187 logical px), its splitter excluded.
    fn thumbs_w(&self) -> f32 {
        if self.in_page_master() {
            return 0.0;
        }
        self.px(187.0).min((self.size.0 * 0.4).max(0.0))
    }

    /// Height of the notes pane (reference: 36 logical px), its splitter excluded.
    fn notes_h(&self) -> f32 {
        if self.notes_on() {
            // Reference: splitter 47 px above the status bar line at 125 %.
            self.px(36.8)
        } else {
            0.0
        }
    }

    /// The notes pane is shown (never in the page master views).
    fn notes_on(&self) -> bool {
        self.show_notes && !self.in_page_master()
    }

    /// Width of the canvas scroll bar.
    fn sb_w(&self) -> f32 {
        self.px(15.0)
    }

    /// The canvas: right of the thumbnails, above the notes, left of the scroll bar.
    pub fn canvas_rect(&self) -> R {
        let x = self.origin.0 + self.thumbs_w() + 1.0;
        let y = self.origin.1;
        let w = (self.origin.0 + self.size.0 - self.sb_w() - x).max(1.0);
        let notes = if self.notes_on() {
            self.notes_h() + 1.0
        } else {
            0.0
        };
        let h = (self.size.1 - notes).max(1.0);
        [x, y, w, h]
    }

    /// The notes pane rectangle (below its splitter line).
    pub fn notes_rect(&self) -> Option<R> {
        if !self.notes_on() {
            return None;
        }
        let c = self.canvas_rect();
        let x = c[0];
        let y = c[1] + c[3] + 1.0;
        Some([x, y, self.origin.0 + self.size.0 - x, self.notes_h()])
    }

    /// The thumbnails pane (Outline view draws the outline there).
    pub fn thumbs_area(&self) -> R {
        self.thumbs_rect()
    }

    /// Draw the current slide's image scaled into `dest` (Notes Page).
    pub fn paint_slide_into(&self, p: &mut dyn Painter, dest: R) {
        let Some(&id) = self.ids.get(self.current) else {
            return;
        };
        if let Some((s, l)) = self.shown.get(&id) {
            let src = [0.0, 0.0, (l.width * s).ceil(), (l.height * s).ceil()];
            p.page_part(id, src, dest, 1.0);
        }
    }

    /// Slide size in points.
    pub fn slide_points(&self) -> (f32, f32) {
        self.slide_size
    }

    fn thumbs_rect(&self) -> R {
        [self.origin.0, self.origin.1, self.thumbs_w(), self.size.1]
    }

    /// Device pixels per point at 100 %.
    fn base_scale(&self) -> f32 {
        self.display_scale * 96.0 / 72.0
    }

    /// The zoom that fits the slide in the canvas, in whole percent (the reference floors it).
    pub fn fit_zoom(&self) -> f32 {
        let [_, _, w, h] = self.canvas_rect();
        let m = 2.0 * self.px(8.0);
        let base = self.base_scale();
        let z = ((w - m) / (self.slide_size.0 * base)).min((h - m) / (self.slide_size.1 * base));
        ((z * 100.0).floor() / 100.0).max(0.1)
    }

    /// Zoom factor in use (1 = 100 %): the Slide Sorter's own zoom in that view (its
    /// images are 320 logical pixels wide at 100 %, as the reference's), else the editing
    /// area's.
    pub fn zoom(&self) -> f32 {
        if self.sorter {
            return self.sort_zoom;
        }
        self.edit_zoom()
    }

    fn edit_zoom(&self) -> f32 {
        self.zoom.unwrap_or_else(|| self.fit_zoom())
    }

    /// The slide fits the canvas (no fixed zoom).
    pub fn fits(&self) -> bool {
        self.zoom.is_none()
    }

    /// Set a fixed zoom (`None`: fit to the window).
    pub fn set_zoom(&mut self, zoom: Option<f32>) -> bool {
        let zoom = zoom.map(|z| (z.clamp(0.1, 4.0) * 100.0).round() / 100.0);
        if self.sorter {
            let z = zoom.unwrap_or(1.0);
            let changed = (z - self.sort_zoom).abs() > f32::EPSILON;
            self.sort_zoom = z;
            self.reveal_sorted(self.current);
            return changed;
        }
        let changed = zoom != self.zoom;
        self.zoom = zoom;
        self.clamp_canvas_scroll();
        changed
    }

    /// Device pixels per point of the slide on the canvas.
    pub fn scale(&self) -> f32 {
        self.edit_zoom() * self.base_scale()
    }

    /// The slide's image size in device pixels.
    fn slide_px(&self, scale: f32) -> (f32, f32) {
        (
            (self.slide_size.0 * scale).ceil(),
            (self.slide_size.1 * scale).ceil(),
        )
    }

    /// Top-left of the slide on the canvas (whole pixels so images map 1:1).
    pub fn slide_origin(&self) -> (f32, f32) {
        let [cx, cy, cw, ch] = self.canvas_rect();
        let (w, h) = self.slide_px(self.scale());
        let x = if w + 2.0 * self.px(8.0) <= cw {
            cx + ((cw - w) / 2.0).round()
        } else {
            cx + self.px(8.0)
        };
        // Centred, one pixel higher like the reference; a taller slide scrolls.
        let y = if h + 2.0 * self.px(8.0) <= ch {
            cy + ((ch - h) / 2.0).floor() - 1.0
        } else {
            cy + self.px(8.0) - self.canvas_scroll
        };
        (x.round(), y.round())
    }

    fn clamp_canvas_scroll(&mut self) {
        let ch = self.canvas_rect()[3];
        let (_, h) = self.slide_px(self.scale());
        let max = (h + 2.0 * self.px(8.0) - ch).max(0.0);
        self.canvas_scroll = self.canvas_scroll.clamp(0.0, max).round();
    }

    /// Slide point of a window position.
    fn to_slide(&self, x: f32, y: f32) -> (f64, f64) {
        let (ox, oy) = self.slide_origin();
        let s = self.scale();
        (f64::from((x - ox) / s), f64::from((y - oy) / s))
    }

    /// Window position of a slide point.
    pub(crate) fn to_px(&self, x: f64, y: f64) -> (f32, f32) {
        let (ox, oy) = self.slide_origin();
        let s = self.scale();
        (ox + x as f32 * s, oy + y as f32 * s)
    }

    /// Width of a thumbnail image (reference: 125 logical px in a 187 px pane).
    fn thumb_w(&self) -> f32 {
        (self.thumbs_w() - self.px(39.0) - self.px(23.0)).max(self.px(24.0))
    }

    fn thumb_scale(&self) -> f32 {
        self.thumb_w() / self.slide_size.0
    }

    /// Vertical distance from one thumbnail's top to the next (reference: 88 + 27 px).
    fn thumb_pitch(&self) -> f32 {
        self.slide_px(self.thumb_scale()).1 + self.px(21.6)
    }

    /// Image rectangle of thumbnail `i` (window pixels).
    pub fn thumb_rect(&self, i: usize) -> R {
        let (w, h) = self.slide_px(self.thumb_scale());
        let x = self.origin.0 + self.px(39.0);
        let y = self.origin.1 + self.thumb_y(i).0 - self.thumb_scroll;
        [x, y.round(), w, h]
    }

    fn thumbs_content_h(&self) -> f32 {
        self.thumbs_height()
    }

    fn clamp_thumb_scroll(&mut self) {
        let max = (self.thumbs_content_h() - self.size.1).max(0.0);
        self.thumb_scroll = self.thumb_scroll.clamp(0.0, max).round();
    }

    fn visible_thumbs(&self) -> std::ops::Range<usize> {
        // No thumbnails pane in the page master views (zero-sized images would fail).
        if self.in_page_master() {
            return 0..0;
        }
        self.thumbs_in_view()
    }

    /// Keep thumbnail `i` inside the pane.
    fn reveal_thumb(&mut self, i: usize) {
        let [_, y, _, h] = self.thumb_rect(i);
        let (top, bottom) = (self.origin.1, self.origin.1 + self.size.1);
        let pad = self.px(12.0);
        if y - pad < top {
            self.thumb_scroll -= top - (y - pad);
        } else if y + h + pad > bottom {
            self.thumb_scroll += y + h + pad - bottom;
        }
        self.clamp_thumb_scroll();
    }

    // ------------------------------------------------------------------ engine

    fn refresh_ids(&mut self) {
        let pres = self.engine.presentation();
        let ids: Vec<usize> = pres
            .map(|p| p.slides.iter().map(|s| s.slide_id as usize).collect())
            .unwrap_or_default();
        let unique: HashSet<usize> = ids.iter().copied().collect();
        // Without unique ids (a damaged file), images fall back to slide positions.
        self.ids = if unique.len() == ids.len() {
            ids
        } else {
            (0..ids.len()).map(|i| (1 << 32) + i).collect()
        };
        if let Ok(list) = self.engine.slide(0) {
            self.slide_size = (list.width.max(1.0), list.height.max(1.0));
        }
    }

    /// Run a slide command and apply its result; `true` when it ran.
    pub fn run(&mut self, cmd: SlideCommand) -> bool {
        match self.engine.slide_command(cmd) {
            Ok(r) => {
                self.apply(r);
                true
            }
            Err(_) => false,
        }
    }

    /// Run a Design command (theme, variants, slide size, background).
    pub fn run_master(&mut self, cmd: bo_engine::slide_edit::master::MasterCommand) -> bool {
        match self.engine.slide_master_command(cmd) {
            Ok(r) if r.error.is_none() => {
                // A theme or size change can touch every slide.
                self.stale.extend(self.shown.keys().copied());
                if r.dirty.is_empty() {
                    self.full.extend(self.shown.keys().copied());
                }
                self.apply(r);
                true
            }
            _ => false,
        }
    }

    /// Run a picture command (crop, reset, insert SVG) on the selection.
    pub fn run_picture(&mut self, cmd: bo_engine::picture_tools::PictureCommand) -> bool {
        match self.engine.slide_picture_command(cmd) {
            Ok(r) if r.error.is_none() => {
                self.apply(r);
                true
            }
            _ => false,
        }
    }

    /// Run a hyperlink / action command on the selection.
    pub fn run_action_cmd(&mut self, cmd: bo_engine::slide_edit::action::ActionCommand) -> bool {
        match self.engine.slide_action_command(cmd) {
            Ok(r) if r.error.is_none() => {
                self.apply(r);
                true
            }
            _ => false,
        }
    }

    /// Draw slides in colour, grayscale or black and white (every image is drawn again).
    pub fn set_color_mode(&mut self, mode: colormode::ColorMode) -> bool {
        if mode == self.color_mode {
            return false;
        }
        self.color_mode = mode;
        self.invalidate_page_images();
        true
    }

    /// The colour mode of the editing area.
    pub fn color_mode(&self) -> colormode::ColorMode {
        self.color_mode
    }

    /// Hide the selection's frame and handles (Remove Background draws its own frame).
    pub fn set_hide_handles(&mut self, hide: bool) {
        self.hide_handles = hide;
    }

    /// Run a notes / handout master view command (the view replaces the slide list).
    pub fn run_page_master(
        &mut self,
        cmd: bo_engine::slide_edit::page_master::PageMasterCommand,
    ) -> bool {
        match self.engine.slide_page_master_command(cmd) {
            Ok(r) if r.error.is_none() => {
                self.stale.extend(self.shown.keys().copied());
                self.full.extend(self.shown.keys().copied());
                self.apply(r);
                self.refresh_ids();
                true
            }
            _ => false,
        }
    }

    /// Run an outline / section command.
    pub fn run_view(&mut self, cmd: bo_engine::slide_edit::views::ViewCommand) -> bool {
        match self.engine.slide_view_command(cmd) {
            Ok(r) if r.error.is_none() => {
                self.apply(r);
                true
            }
            _ => false,
        }
    }

    /// Run a shape properties / table style command (Selection Pane, Alt Text, Table Design).
    pub fn run_props(
        &mut self,
        cmd: bo_engine::slide_edit::shape_props::ShapePropsCommand,
    ) -> bool {
        match self.engine.slide_props_command(cmd) {
            Ok(r) if r.error.is_none() => {
                self.apply(r);
                true
            }
            _ => false,
        }
    }

    /// Insert a picture, video or audio file on the current slide.
    pub fn insert_media(&mut self, bytes: Vec<u8>, name: &str) -> bool {
        match self.engine.slide_insert_picture(bytes, name) {
            Ok(r) if r.error.is_none() => {
                self.apply(r);
                true
            }
            _ => false,
        }
    }

    /// The engine's last answer for the current slide (selection, transition, effects, design).
    pub fn state(&self) -> Option<&SlideEditResult> {
        self.state.as_ref()
    }

    fn apply(&mut self, r: SlideEditResult) {
        if r.slides_changed || r.info.page_count != self.ids.len() {
            self.refresh_ids();
            // Slide numbers and links may have changed anywhere: compare every kept image.
            self.stale.extend(self.shown.keys().copied());
            self.clamp_thumb_scroll();
        }
        for &i in &r.dirty {
            if let Some(&id) = self.ids.get(i) {
                self.stale.extend([id, THUMB_KEY + id, SHOW_KEY + id]);
            }
        }
        let moved = r.slide != self.current;
        self.current = r.slide.min(self.ids.len().saturating_sub(1));
        if moved {
            self.canvas_scroll = 0.0;
            self.notes_caret = utf16_len(&r.notes);
            let cur = self.current;
            self.reveal_thumb(cur);
        }
        self.notes_caret = self.notes_caret.min(utf16_len(&r.notes));
        self.caret_on = true;
        if moved || !r.dirty.is_empty() || r.slides_changed {
            self.spell_dirty = true;
        }
        self.state = Some(r);
    }

    /// Go to slide `i` (clamped).
    pub fn go_to(&mut self, i: usize) -> bool {
        let i = i.min(self.ids.len().saturating_sub(1));
        if i == self.current && self.text_edit().is_none() && self.selection().is_empty() {
            return false;
        }
        self.run(SlideCommand::GoTo { slide: i })
    }

    /// New slide after the current one (`layout`: the deck's layout index, else like the
    /// current slide).
    pub fn new_slide(&mut self, layout: Option<usize>) -> bool {
        let ok = self.run(SlideCommand::AddSlide {
            after: Some(self.current),
            layout,
        });
        self.focus = Focus::Thumbs;
        ok
    }

    /// Delete the current slide (never the last remaining one).
    pub fn delete_slide(&mut self) -> bool {
        if self.ids.len() <= 1 {
            return false;
        }
        self.run(SlideCommand::DeleteSlide { index: None })
    }

    /// Duplicate the current slide.
    pub fn duplicate_slide(&mut self) -> bool {
        self.run(SlideCommand::DuplicateSlide { index: None })
    }

    /// Apply a text format to the selection (text being edited, else the selected shapes).
    pub fn format_text(&mut self, format: TextFormat) -> bool {
        self.run(SlideCommand::FormatText {
            format,
            range: None,
        })
    }

    /// Run interface command `id` (`format.bold`, `slide.new`, …); `true` when handled.
    pub fn action(&mut self, id: &str) -> bool {
        let toggle = None;
        let align = |a| TextFormat::Align { align: a };
        let format = match id {
            "format.bold" => Some(TextFormat::Bold { value: toggle }),
            "format.italic" => Some(TextFormat::Italic { value: toggle }),
            "format.underline" => Some(TextFormat::Underline { value: toggle }),
            "format.strike" => Some(TextFormat::Strike { value: toggle }),
            "format.growStep" => Some(TextFormat::SizeStep { up: true }),
            "format.shrinkStep" => Some(TextFormat::SizeStep { up: false }),
            "format.alignLeft" => Some(align(Align::Left)),
            "format.alignCenter" => Some(align(Align::Center)),
            "format.alignRight" => Some(align(Align::Right)),
            "format.justify" => Some(align(Align::Justify)),
            "format.bullets" => Some(TextFormat::Bullets { value: toggle }),
            "format.numbering" => Some(TextFormat::Numbering { value: toggle }),
            "format.indent" => Some(TextFormat::Level { delta: 1 }),
            "format.outdent" => Some(TextFormat::Level { delta: -1 }),
            "format.color" => Some(TextFormat::Color {
                hex: Some("FF0000".into()),
            }),
            "format.highlight" => Some(TextFormat::Highlight {
                hex: Some("FFFF00".into()),
            }),
            _ => None,
        };
        if let Some(f) = format {
            return self.format_text(f);
        }
        match id {
            "edit.undo" => self.run(SlideCommand::Undo),
            "edit.redo" => self.run(SlideCommand::Redo),
            "slide.new" => self.new_slide(None),
            "slide.delete" => self.delete_slide(),
            "slide.duplicate" => self.duplicate_slide(),
            "edit.selectAll" => self.run(SlideCommand::SelectAll),
            "edit.delete" => self.run(SlideCommand::Delete),
            "arrange.front" | "arrange.back" | "arrange.forward" | "arrange.backward" => {
                let order = match id {
                    "arrange.front" => bo_engine::slide_edit::ZOrder::Front,
                    "arrange.back" => bo_engine::slide_edit::ZOrder::Back,
                    "arrange.forward" => bo_engine::slide_edit::ZOrder::Forward,
                    _ => bo_engine::slide_edit::ZOrder::Backward,
                };
                self.run(SlideCommand::ZOrder { to: order })
            }
            "arrange.group" => self.run(SlideCommand::Group),
            "arrange.ungroup" => self.run(SlideCommand::Ungroup),
            "insert.textBox" | "insert.rect" | "insert.ellipse" | "insert.triangle"
            | "insert.arrow" => self.insert_shape(id),
            "show.fromStart" => {
                let plan = self.show_plan(None);
                let first = plan.order.first().copied().unwrap_or(0);
                self.start_show_with(first, plan)
            }
            "show.fromCurrent" => {
                let plan = self.show_plan(None);
                self.start_show_with(self.current, plan)
            }
            _ => false,
        }
    }

    /// Insert a shape (or a text box) in the middle of the slide and select it.
    fn insert_shape(&mut self, id: &str) -> bool {
        if id == "insert.textBox" {
            return self.run(SlideCommand::InsertTextBox {
                text: String::new(),
                rect: None,
            }) && self.enter_text(false);
        }
        let preset = match id {
            "insert.ellipse" => "ellipse",
            "insert.triangle" => "triangle",
            "insert.arrow" => "rightArrow",
            _ => "rect",
        };
        self.run(SlideCommand::InsertShape {
            preset: preset.into(),
            rect: None,
        })
    }

    // ------------------------------------------------------------------ text editing

    /// Start editing the text of the single selected shape; `all` selects its text.
    fn enter_text(&mut self, all: bool) -> bool {
        let Some(s) = self.selection().first().filter(|s| s.editable_text) else {
            return false;
        };
        let id = s.id;
        let len = s.text.as_deref().map_or(0, utf16_len);
        self.run(SlideCommand::TextSelect {
            id: Some(id),
            start: if all { 0 } else { len },
            end: len,
            cell: None,
        })
    }

    /// Replace the text selection with `insert` (one merged undo step per typing run).
    fn type_text(&mut self, insert: &str) -> bool {
        let Some(te) = self.text_edit() else {
            return false;
        };
        let (a, b) = (te.start.min(te.end), te.start.max(te.end));
        let (ba, bb) = (byte_at(&te.text, a), byte_at(&te.text, b));
        let mut text = String::with_capacity(te.text.len() + insert.len());
        text.push_str(&te.text[..ba]);
        text.push_str(insert);
        text.push_str(&te.text[bb..]);
        let caret = a + utf16_len(insert);
        let id = te.id;
        self.set_text(id, text, caret, caret)
    }

    fn set_text(&mut self, id: u32, text: String, start: usize, end: usize) -> bool {
        // In a table, the text of the cell being edited (typing went to the frame before).
        let cell = self.text_edit().filter(|t| t.id == id).and_then(|t| t.cell);
        self.run(SlideCommand::SetText {
            id: Some(id),
            text,
            merge: true,
            sel: Some([start, end]),
            cell,
        })
    }

    /// Delete backwards (or forwards) from the caret, or the selected text.
    fn delete_text(&mut self, forward: bool, word: bool) -> bool {
        let Some(te) = self.text_edit() else {
            return false;
        };
        let (mut a, mut b) = (te.start.min(te.end), te.start.max(te.end));
        if a == b {
            let to = match (forward, word) {
                (f, true) => step_word(&te.text, a, f),
                (f, false) => step_char(&te.text, a, f),
            };
            if forward {
                b = to;
            } else {
                a = to;
            }
        }
        if a == b {
            return false;
        }
        let (ba, bb) = (byte_at(&te.text, a), byte_at(&te.text, b));
        let mut text = te.text.clone();
        text.replace_range(ba..bb, "");
        let id = te.id;
        self.set_text(id, text, a, a)
    }

    fn select_text(&mut self, start: usize, end: usize) -> bool {
        let Some(id) = self.text_edit().map(|t| t.id) else {
            return false;
        };
        self.run(SlideCommand::TextSelect {
            id: Some(id),
            start,
            end,
            cell: None,
        })
    }

    /// Caret line `[x1, y1, x2, y2]` (slide points) of the text being edited.
    fn caret_line(&self) -> Option<[f64; 4]> {
        self.text_edit().and_then(|t| t.caret)
    }

    fn text_key(&mut self, key: Key, m: Mods) -> bool {
        let Some(te) = self.text_edit().cloned() else {
            return false;
        };
        let len = utf16_len(&te.text);
        let collapse = |forward: bool| {
            if forward {
                te.start.max(te.end)
            } else {
                te.start.min(te.end)
            }
        };
        match key {
            Key::Left | Key::Right => {
                let forward = key == Key::Right;
                let to = if !m.shift && te.start != te.end && !m.ctrl {
                    collapse(forward)
                } else if m.ctrl {
                    step_word(&te.text, te.end, forward)
                } else {
                    step_char(&te.text, te.end, forward)
                };
                let start = if m.shift { te.start } else { to };
                self.select_text(start, to)
            }
            Key::Up | Key::Down => {
                let Some([x1, y1, _, y2]) = self.caret_line() else {
                    return false;
                };
                let h = (y2 - y1).abs().max(1.0);
                let y = if key == Key::Up {
                    y1.min(y2) - h * 0.5
                } else {
                    y1.max(y2) + h * 0.5
                };
                self.run(SlideCommand::TextHit {
                    x: x1,
                    y,
                    extend: m.shift,
                })
            }
            Key::Home | Key::End if m.ctrl => {
                let to = if key == Key::Home { 0 } else { len };
                let start = if m.shift { te.start } else { to };
                self.select_text(start, to)
            }
            Key::Home | Key::End => {
                let Some([_, y1, _, y2]) = self.caret_line() else {
                    return false;
                };
                let b = te.rect;
                let x = if key == Key::Home {
                    b.cx - b.w / 2.0 + 0.5
                } else {
                    b.cx + b.w / 2.0 - 0.5
                };
                self.run(SlideCommand::TextHit {
                    x,
                    y: f64::midpoint(y1, y2),
                    extend: m.shift,
                })
            }
            Key::Backspace => self.delete_text(false, m.ctrl),
            Key::Delete => self.delete_text(true, m.ctrl),
            // Shift+Enter: line break inside the paragraph (§21.1.2.2.1 br).
            Key::Enter if m.shift => self.type_text("\u{b}"),
            Key::Enter => self.type_text("\n"),
            // In a table, Tab goes to the next cell (its text selected), Shift+Tab to the
            // previous one; Tab in the last cell adds a row (reference).
            Key::Tab if te.cell.is_some() => self.table_tab(te.id, m.shift),
            // Tab inside a paragraph types a tab; at its start (or over a selection) it
            // demotes the paragraph, Shift+Tab promotes (reference).
            Key::Tab if !m.shift && te.start == te.end && !para_start(&te.text, te.end) => {
                self.type_text("\t")
            }
            Key::Tab => self.format_text(TextFormat::Level {
                delta: if m.shift { -1 } else { 1 },
            }),
            Key::Escape => self.run(SlideCommand::TextExit),
            // Page keys leave the text for the previous / next slide, as on the reference.
            Key::PageUp => self.go_to(self.current.saturating_sub(1)),
            Key::PageDown => self.go_to(self.current + 1),
            Key::Letter('a') if m.ctrl => self.select_text(0, len),
            _ => false,
        }
    }

    /// Tab / Shift+Tab in a table cell: the next / previous cell in reading order, its text
    /// selected; Tab in the last cell adds a row below and goes to its first cell.
    fn table_tab(&mut self, id: u32, back: bool) -> bool {
        let Some(cur) = self.text_edit().and_then(|t| t.cell) else {
            return false;
        };
        let Some(t) = self.state.as_ref().and_then(|s| s.table.clone()) else {
            return false;
        };
        let mut cells: Vec<(usize, usize, usize)> = t
            .cells
            .iter()
            .map(|c| (c.row, c.col, utf16_len(&c.text)))
            .collect();
        cells.sort_unstable();
        let Some(i) = cells
            .iter()
            .position(|(r, c, _)| *r == cur[0] && *c == cur[1])
        else {
            return false;
        };
        let target = if back {
            i.checked_sub(1).map(|j| cells[j])
        } else {
            cells.get(i + 1).copied()
        };
        let (r, c, _) = match target {
            Some(cell) => cell,
            None if back => return false,
            None => {
                // The last cell: a new row below, its first cell (empty).
                self.run(SlideCommand::TableSelect {
                    id: Some(id),
                    r0: cur[0],
                    c0: cur[1],
                    r1: None,
                    c1: None,
                });
                if !self.run(SlideCommand::TableInsertRow { below: true }) {
                    return false;
                }
                (t.rows, 0, 0)
            }
        };
        if !self.run(SlideCommand::TextSelect {
            id: Some(id),
            start: 0,
            end: 0,
            cell: Some([r, c]),
        }) {
            return false;
        }
        // The cell's whole text selected (as the edit reports it now).
        let len = self.text_edit().map_or(0, |t| utf16_len(&t.text));
        if len > 0 {
            self.run(SlideCommand::TextSelect {
                id: Some(id),
                start: 0,
                end: len,
                cell: Some([r, c]),
            });
        }
        true
    }

    // ------------------------------------------------------------------ keyboard

    fn key(&mut self, key: Key, m: Mods) -> bool {
        // Shortcuts that act the same everywhere.
        if m.ctrl {
            match key {
                Key::Letter('z') => return self.run(SlideCommand::Undo),
                Key::Letter('y') => return self.run(SlideCommand::Redo),
                Key::Letter('m') => return self.new_slide(None),
                Key::Letter(c @ ('b' | 'i' | 'u' | 'e' | 'l' | 'r' | 'j'))
                    if self.focus != Focus::Notes =>
                {
                    let id = match c {
                        'b' => "format.bold",
                        'i' => "format.italic",
                        'u' => "format.underline",
                        'e' => "format.alignCenter",
                        'l' => "format.alignLeft",
                        'r' => "format.alignRight",
                        _ => "format.justify",
                    };
                    return self.action(id);
                }
                // Increase / Decrease Font Size: Ctrl+] / Ctrl+[ and Ctrl+Shift+> / <.
                Key::Letter(']') => return self.action("format.growStep"),
                Key::Letter('[') => return self.action("format.shrinkStep"),
                Key::Letter('.') if m.shift => return self.action("format.growStep"),
                Key::Letter(',') if m.shift => return self.action("format.shrinkStep"),
                _ => {}
            }
        }
        if let Key::F(5) = key {
            // F5 from the first slide of the setup, Shift+F5 from the current one, Alt+F5
            // in Presenter View.
            let mut plan = self.show_plan(None);
            plan.presenter = m.alt;
            let from = if m.shift || m.alt {
                self.current
            } else {
                plan.order.first().copied().unwrap_or(0)
            };
            return self.start_show_with(from, plan);
        }
        match self.focus {
            Focus::Notes => self.notes_key(key, m),
            Focus::Thumbs => self.thumbs_key(key, m),
            Focus::Canvas if self.text_edit().is_some() => self.text_key(key, m),
            Focus::Canvas => self.canvas_key(key, m),
        }
    }

    fn thumbs_key(&mut self, key: Key, m: Mods) -> bool {
        let n = self.ids.len();
        match key {
            Key::Up | Key::Left | Key::PageUp => self.go_to(self.current.saturating_sub(1)),
            Key::Down | Key::Right | Key::PageDown => self.go_to(self.current + 1),
            Key::Home => self.go_to(0),
            Key::End => self.go_to(n.saturating_sub(1)),
            Key::Delete | Key::Backspace => self.delete_slides(),
            Key::Enter => self.new_slide(None),
            Key::Letter('d') if m.ctrl => self.duplicate_slide(),
            _ => false,
        }
    }

    fn canvas_key(&mut self, key: Key, m: Mods) -> bool {
        let selected = !self.selection().is_empty();
        let nudge = if m.ctrl { NUDGE_FINE } else { NUDGE };
        let mv = |dx: f64, dy: f64| SlideCommand::Transform {
            transform: Transform::Move { dx, dy },
            snap: false,
            tolerance: 4.0,
            preview: false,
        };
        match key {
            Key::Left if selected => self.run(mv(-nudge, 0.0)),
            Key::Right if selected => self.run(mv(nudge, 0.0)),
            Key::Up if selected => self.run(mv(0.0, -nudge)),
            Key::Down if selected => self.run(mv(0.0, nudge)),
            Key::Up | Key::Left | Key::PageUp => self.go_to(self.current.saturating_sub(1)),
            Key::Down | Key::Right | Key::PageDown => self.go_to(self.current + 1),
            Key::Home if !selected => self.go_to(0),
            Key::End if !selected => self.go_to(self.ids.len().saturating_sub(1)),
            Key::Delete | Key::Backspace if selected => self.run(SlideCommand::Delete),
            Key::Escape if selected => self.run(SlideCommand::Select { ids: vec![] }),
            Key::Tab => self.run(SlideCommand::SelectNext { back: m.shift }),
            Key::Enter if selected => self.enter_text(true),
            Key::F(2) if selected => self.enter_text(false),
            Key::Letter('a') if m.ctrl => self.run(SlideCommand::SelectAll),
            Key::Letter('d') if m.ctrl && selected => self.run(SlideCommand::Duplicate),
            _ => false,
        }
    }

    fn notes_key(&mut self, key: Key, m: Mods) -> bool {
        let text = self.notes().to_owned();
        let len = utf16_len(&text);
        let caret = self.notes_caret.min(len);
        match key {
            Key::Left => {
                self.notes_caret = step_char(&text, caret, false);
                true
            }
            Key::Right => {
                self.notes_caret = step_char(&text, caret, true);
                true
            }
            Key::Home => {
                self.notes_caret = 0;
                true
            }
            Key::End => {
                self.notes_caret = len;
                true
            }
            Key::Backspace | Key::Delete => {
                let to = if m.ctrl {
                    step_word(&text, caret, key == Key::Delete)
                } else {
                    step_char(&text, caret, key == Key::Delete)
                };
                let (a, b) = (caret.min(to), caret.max(to));
                if a == b {
                    return false;
                }
                let mut t = text;
                t.replace_range(byte_at(&t, a)..byte_at(&t, b), "");
                self.notes_caret = a;
                self.run(SlideCommand::SetNotes {
                    slide: None,
                    text: t,
                })
            }
            Key::Enter => self.notes_insert("\n"),
            Key::Escape => {
                self.focus = Focus::Canvas;
                true
            }
            _ => false,
        }
    }

    fn notes_insert(&mut self, s: &str) -> bool {
        let mut t = self.notes().to_owned();
        let caret = self.notes_caret.min(utf16_len(&t));
        t.insert_str(byte_at(&t, caret), s);
        self.notes_caret = caret + utf16_len(s);
        self.run(SlideCommand::SetNotes {
            slide: None,
            text: t,
        })
    }

    // ------------------------------------------------------------------ pointer

    /// The handle of the selection under a window position.
    fn handle_at(&self, x: f32, y: f32) -> Option<Handle> {
        let st = self.state.as_ref()?;
        let b = st.bounds?;
        let r = self.px(7.0);
        st.handles.iter().copied().find(|h| {
            let (hx, hy) = if h.kind == HandleKind::Rot {
                self.rot_handle_px(&b)
            } else {
                self.to_px(h.x, h.y)
            };
            (hx - x).abs() <= r && (hy - y).abs() <= r
        })
    }

    fn in_box(b: &BoxR, x: f64, y: f64, pad: f64) -> bool {
        let (lx, ly) = b.to_local(x, y);
        lx.abs() <= b.w / 2.0 + pad && ly.abs() <= b.h / 2.0 + pad
    }

    fn press_canvas(&mut self, x: f32, y: f32, count: u8, shift: bool) -> bool {
        self.focus = Focus::Canvas;
        let (sx, sy) = self.to_slide(x, y);
        if let Some(h) = self.handle_at(x, y) {
            self.drag = Some(Drag::Handle {
                kind: h.kind,
                from: (x, y),
            });
            self.start_ghost();
            return true;
        }
        // Inside the text being edited: place the caret (Shift extends; double click
        // selects the word).
        if let Some(te) = self.text_edit().cloned() {
            if Self::in_box(&te.rect, sx, sy, 2.0) {
                let ok = self.run(SlideCommand::TextHit {
                    x: sx,
                    y: sy,
                    extend: shift,
                });
                if count >= 3 {
                    if let Some(t) = self.text_edit().cloned() {
                        let (a, b) = para_at(&t.text, t.end);
                        return self.select_text(a, b);
                    }
                }
                if count == 2 {
                    if let Some(t) = self.text_edit().cloned() {
                        let (a, b) = word_at(&t.text, t.end);
                        return self.select_text(a, b);
                    }
                }
                self.drag = Some(Drag::Text);
                return ok;
            }
        }
        let before: Vec<u32> = self.selection().iter().map(|s| s.id).collect();
        // Shift on a selected shape: a drag moves the selection straight; a click without
        // moving takes the shape out of it (on release), as the reference does.
        self.shift_toggle = None;
        if shift
            && self
                .selection()
                .iter()
                .any(|s| Self::in_box(&s.rect, sx, sy, 3.0))
        {
            self.shift_toggle = Some((sx, sy));
            self.drag = Some(Drag::Move {
                from: (x, y),
                moved: false,
            });
            return true;
        }
        self.run(SlideCommand::Click {
            x: sx,
            y: sy,
            extend: shift,
            deep: false,
        });
        let hit = self
            .state
            .as_ref()
            .and_then(|s| {
                s.selection
                    .iter()
                    .find(|i| Self::in_box(&i.rect, sx, sy, 3.0))
            })
            .cloned();
        let Some(hit) = hit else {
            // Empty slide: drag a selection rectangle (reference).
            if inside(self.canvas_rect(), x, y) {
                self.drag = Some(Drag::Marquee {
                    from: (x, y),
                    to: (x, y),
                });
            }
            return true;
        };
        // Text placeholders and text boxes take the caret at once; other shapes on a second
        // click (or a double click), as the reference does.
        let text_box = hit.placeholder.is_some() || hit.name.starts_with("TextBox");
        self.pending_text = None;
        if !shift && hit.editable_text && Self::in_box(&hit.rect, sx, sy, -2.0) {
            if text_box || count >= 2 {
                return self.enter_text_at(hit.id, sx, sy, count);
            }
            // A second click on a selected shape: the caret goes in on release, unless the
            // press becomes a move (the reference moves a selected shape dragged anywhere).
            if before == [hit.id] {
                self.pending_text = Some((hit.id, sx, sy));
            }
        }
        self.drag = Some(Drag::Move {
            from: (x, y),
            moved: false,
        });
        true
    }

    /// Put the caret in shape `id`'s text at slide point (`sx`, `sy`); a double click
    /// selects the word there.
    fn enter_text_at(&mut self, id: u32, sx: f64, sy: f64, count: u8) -> bool {
        self.run(SlideCommand::TextSelect {
            id: Some(id),
            start: 0,
            end: 0,
            cell: None,
        });
        self.run(SlideCommand::TextHit {
            x: sx,
            y: sy,
            extend: false,
        });
        if count == 2 {
            if let Some(t) = self.text_edit().cloned() {
                let (a, b) = word_at(&t.text, t.end);
                self.select_text(a, b);
            }
        }
        self.drag = Some(Drag::Text);
        true
    }

    fn transform_of(&self, x: f32, y: f32, shift: bool) -> Option<Transform> {
        let s = f64::from(self.scale());
        match self.drag? {
            Drag::Move { from, .. } => {
                let (mut dx, mut dy) = (f64::from(x - from.0) / s, f64::from(y - from.1) / s);
                // Shift keeps the move horizontal or vertical (reference).
                if shift {
                    if dx.abs() >= dy.abs() {
                        dy = 0.0;
                    } else {
                        dx = 0.0;
                    }
                }
                Some(Transform::Move { dx, dy })
            }
            Drag::Handle {
                kind: HandleKind::Rot,
                ..
            } => {
                let (px, py) = self.to_slide(x, y);
                Some(Transform::Rotate {
                    x: px,
                    y: py,
                    step: shift,
                })
            }
            Drag::Handle { kind, from } => Some(Transform::Resize {
                handle: kind,
                dx: f64::from(x - from.0) / s,
                dy: f64::from(y - from.1) / s,
                keep_aspect: shift,
            }),
            Drag::Text | Drag::Slides { .. } | Drag::Marquee { .. } => None,
        }
    }

    fn drag_to(&mut self, x: f32, y: f32) -> bool {
        match self.drag {
            Some(Drag::Text) => {
                let (sx, sy) = self.to_slide(x, y);
                self.run(SlideCommand::TextHit {
                    x: sx,
                    y: sy,
                    extend: true,
                })
            }
            Some(Drag::Move { from, moved }) => {
                if !moved && (x - from.0).hypot(y - from.1) < DRAG_SLOP * self.display_scale {
                    return false;
                }
                self.drag = Some(Drag::Move { from, moved: true });
                self.preview_transform(x, y)
            }
            Some(Drag::Handle { .. }) => self.preview_transform(x, y),
            Some(Drag::Slides { .. }) => self.drag_slides(x, y),
            Some(Drag::Marquee { from, .. }) => {
                self.drag = Some(Drag::Marquee { from, to: (x, y) });
                true
            }
            None => false,
        }
    }

    fn preview_transform(&mut self, x: f32, y: f32) -> bool {
        let Some(t) = self.transform_of(x, y, self.pointer_mods.shift) else {
            return false;
        };
        self.undo_live();
        let r = self.engine.slide_command(SlideCommand::Transform {
            transform: t,
            snap: true,
            tolerance: self.snap_tol(),
            preview: true,
        });
        self.preview = r.ok().and_then(|r| r.preview);
        // Ctrl + drag copies: the shapes stay, their outlines follow the pointer.
        if !self.pointer_mods.ctrl {
            let since = self.history_len();
            let ok = self.engine.slide_command(SlideCommand::Transform {
                transform: t,
                snap: true,
                tolerance: self.snap_tol(),
                preview: false,
            });
            if let Ok(r) = ok {
                let applied = r.error.is_none();
                self.apply(r);
                if applied && self.history_len() > since {
                    self.live_since = Some(since);
                }
            }
        }
        true
    }

    /// Smart guide snapping distance: 6 logical pixels on screen in slide points (the
    /// reference snaps a 5.5 px gap at 75 % zoom and 125 % display scale).
    fn snap_tol(&self) -> f64 {
        f64::from(6.0 * self.display_scale / self.scale().max(0.01))
    }

    fn history_len(&mut self) -> usize {
        self.engine
            .slide_command(SlideCommand::HistoryLen)
            .ok()
            .and_then(|r| r.history)
            .unwrap_or(0)
    }

    /// Take back the transform applied live during a drag.
    fn undo_live(&mut self) {
        if let Some(since) = self.live_since.take() {
            if self.history_len() > since {
                self.run(SlideCommand::Undo);
            }
        }
    }

    /// Keep the slide without the selected shapes while a handle is dragged (the
    /// reference hides them and draws their new size half see-through).
    fn start_ghost(&mut self) {
        let ids: Vec<u32> = self
            .state
            .as_ref()
            .map(|s| s.selection.iter().map(|i| i.id).collect())
            .unwrap_or_default();
        let slide = self.current;
        self.ghost = (!ids.is_empty())
            .then(|| self.engine.slide_without_shapes(slide, &ids))
            .flatten()
            .map(|list| Ghost {
                slide,
                list: Some(list),
                scale: 0.0,
            });
    }

    /// The see-through outline is drawn (a handle drag changed the shapes).
    pub fn ghost_shown(&self) -> bool {
        self.live_since.is_some()
            && matches!(self.drag, Some(Drag::Handle { .. }))
            && self.ghost.as_ref().is_some_and(|g| {
                g.list.is_none()
                    && g.slide == self.current
                    && (g.scale - self.scale()).abs() <= f32::EPSILON
            })
    }

    /// Store the slide under a handle drag; let its image go after the drag.
    fn prepare_ghost(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        if !matches!(self.drag, Some(Drag::Handle { .. })) {
            self.ghost = None;
        }
        let scale = self.scale();
        match self.ghost.as_mut() {
            Some(g) => {
                if let Some(list) = g.list.take() {
                    if self.color_mode == colormode::ColorMode::Color {
                        store.update(GHOST_KEY, &list, &self.engine, scale, None)?;
                    } else {
                        let shown = colormode::recolor(&list, self.color_mode);
                        store.update(GHOST_KEY, &shown, &self.engine, scale, None)?;
                    }
                    g.scale = scale;
                    self.ghost_stored = true;
                }
            }
            None if self.ghost_stored => {
                store.remove(GHOST_KEY);
                self.ghost_stored = false;
            }
            None => {}
        }
        Ok(())
    }

    fn release(&mut self, x: f32, y: f32) -> bool {
        let drag = self.drag.take();
        self.ghost = None;
        self.preview = None;
        self.undo_live();
        if let Some(Drag::Slides { moved, to, .. }) = drag {
            return match to.filter(|_| moved) {
                Some(t) => {
                    self.drop_slides(t);
                    true
                }
                None => true,
            };
        }
        if let Some(Drag::Marquee { from, .. }) = drag {
            return self.select_marquee(from, (x, y));
        }
        if let Some((id, sx, sy)) = self.pending_text.take() {
            if matches!(drag, Some(Drag::Move { moved: false, .. })) {
                self.enter_text_at(id, sx, sy, 1);
                self.drag = None;
                return true;
            }
        }
        if let Some((sx, sy)) = self.shift_toggle.take() {
            if matches!(drag, Some(Drag::Move { moved: false, .. })) {
                return self.run(SlideCommand::Click {
                    x: sx,
                    y: sy,
                    extend: true,
                    deep: false,
                });
            }
        }
        let commit = matches!(
            drag,
            Some(Drag::Move { moved: true, .. } | Drag::Handle { .. })
        );
        if !commit {
            return drag.is_some();
        }
        // Ctrl + drag leaves the shapes and moves copies of them (one undo step).
        if self.pointer_mods.ctrl && matches!(drag, Some(Drag::Move { .. })) {
            return self.copy_by_drag(drag, x, y);
        }
        match self.transform_of_drag(drag, x, y) {
            Some(t) => self.run(SlideCommand::Transform {
                transform: t,
                snap: true,
                tolerance: self.snap_tol(),
                preview: false,
            }),
            None => true,
        }
    }

    /// Shift / Ctrl held with the mouse button; the host sets them before each mouse event.
    pub fn set_pointer_mods(&mut self, m: Mods) {
        self.pointer_mods = m;
    }

    /// The selection rectangle being dragged (window pixels), if any.
    pub fn marquee(&self) -> Option<R> {
        match self.drag {
            Some(Drag::Marquee { from, to }) => Some([
                from.0.min(to.0),
                from.1.min(to.1),
                (from.0 - to.0).abs(),
                (from.1 - to.1).abs(),
            ]),
            _ => None,
        }
    }

    /// Select the shapes inside the rectangle dragged from `a` to `b`.
    fn select_marquee(&mut self, a: (f32, f32), b: (f32, f32)) -> bool {
        if (a.0 - b.0).abs() < 2.0 && (a.1 - b.1).abs() < 2.0 {
            return true;
        }
        let (x0, y0) = self.to_slide(a.0.min(b.0), a.1.min(b.1));
        let (x1, y1) = self.to_slide(a.0.max(b.0), a.1.max(b.1));
        self.run(SlideCommand::SelectRect {
            rect: bo_engine::slide_edit::geom::Rect {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            },
            extend: self.pointer_mods.shift,
        })
    }

    /// Ctrl + drag: duplicate the selection and move the copies where the drag ended.
    fn copy_by_drag(&mut self, drag: Option<Drag>, x: f32, y: f32) -> bool {
        let Some(Transform::Move { dx, dy }) = self.transform_of_drag(drag, x, y) else {
            return true;
        };
        let since = self
            .engine
            .slide_command(SlideCommand::HistoryLen)
            .ok()
            .and_then(|r| r.history)
            .unwrap_or(0);
        if !self.run(SlideCommand::Duplicate) {
            return true;
        }
        // The copies sit 12 pt down and right of the shapes (engine `DUPLICATE_OFFSET`).
        self.run(SlideCommand::Transform {
            transform: Transform::Move {
                dx: dx - 12.0,
                dy: dy - 12.0,
            },
            snap: true,
            tolerance: self.snap_tol(),
            preview: false,
        });
        let copies: Vec<u32> = self.selection().iter().map(|s| s.id).collect();
        self.run(SlideCommand::MergeHistory { since });
        self.run(SlideCommand::Select { ids: copies })
    }

    fn transform_of_drag(&mut self, drag: Option<Drag>, x: f32, y: f32) -> Option<Transform> {
        let saved = std::mem::replace(&mut self.drag, drag);
        let t = self.transform_of(x, y, self.pointer_mods.shift);
        self.drag = saved;
        t
    }

    /// Which thumbnail is at a window position.
    fn thumb_at(&self, x: f32, y: f32) -> Option<usize> {
        if !inside(self.thumbs_rect(), x, y) {
            return None;
        }
        self.visible_thumbs().find(|&i| {
            if self.thumb_hidden(i) {
                return false;
            }
            let [tx, ty, tw, th] = self.thumb_rect(i);
            let pad = self.px(8.0);
            inside(
                [tx - 3.0 * pad, ty - pad, tw + 4.0 * pad, th + 2.0 * pad],
                x,
                y,
            )
        })
    }

    /// Handle an input event inside the area; `true` when a new frame is needed.
    pub fn handle(&mut self, input: Input) -> bool {
        if self.previewing() {
            // Anything but timers stops a preview and acts as usual.
            if matches!(input, Input::Blink | Input::Tick | Input::Move { .. }) {
                return false;
            }
            self.show = None;
        }
        if self.show.is_some() {
            return self.show_input(&input);
        }
        if let Some(r) = self.ink_input(&input) {
            return r;
        }
        match input {
            Input::Text(text) => {
                let text: String = text.chars().filter(|c| !c.is_control()).collect();
                if text.is_empty() {
                    return false;
                }
                match self.focus {
                    Focus::Notes => self.notes_insert(&text),
                    Focus::Thumbs => false,
                    Focus::Canvas => {
                        // Typing over a selected shape replaces its text.
                        if self.text_edit().is_none() && !self.enter_text(true) {
                            return false;
                        }
                        self.type_text(&text)
                    }
                }
            }
            Input::Key(k, m)
                if self.sorter && !(m.ctrl && matches!(k, Key::Letter('z' | 'y' | 'm'))) =>
            {
                self.sorter_key(k, m)
            }
            Input::Key(k, m) => self.key(k, m),
            Input::Wheel(dy) if self.sorter => self.sorter_wheel(dy),
            Input::Wheel(dy) => self.wheel(dy),
            Input::Press { x, y, count, shift } => {
                if self.sorter {
                    return match self.sort_at(x, y) {
                        Some(i) => self.press_slide(i, x, y, count, shift),
                        None => false,
                    };
                }
                if let Some(i) = self.thumb_at(x, y) {
                    return self.press_slide(i, x, y, count, shift);
                }
                if inside(self.thumbs_rect(), x, y) {
                    self.focus = Focus::Thumbs;
                    return true;
                }
                if self.notes_rect().is_some_and(|r| inside(r, x, y)) {
                    if self.text_edit().is_some() {
                        self.run(SlideCommand::TextExit);
                    }
                    self.focus = Focus::Notes;
                    self.notes_caret = utf16_len(self.notes());
                    return true;
                }
                if inside(self.canvas_rect(), x, y) {
                    return self.press_canvas(x, y, count, shift);
                }
                false
            }
            Input::Drag { x, y } => self.drag_to(x, y),
            Input::Release { x, y } => self.release(x, y),
            Input::Blink => {
                self.caret_on = !self.caret_on;
                let collapsed = self.text_edit().is_some_and(|t| t.start == t.end);
                collapsed || self.focus == Focus::Notes
            }
            Input::Scale(s) => {
                self.set_display_scale(s);
                true
            }
            _ => false,
        }
    }

    fn wheel(&mut self, dy: f32) -> bool {
        // Over the thumbnails the pane scrolls; on the canvas a fitted slide steps to the
        // previous / next slide like the reference, a zoomed one scrolls.
        if matches!(self.drag, Some(Drag::Slides { .. })) || self.focus == Focus::Thumbs {
            let before = self.thumb_scroll;
            self.thumb_scroll += dy;
            self.clamp_thumb_scroll();
            return before != self.thumb_scroll;
        }
        let before = self.canvas_scroll;
        self.canvas_scroll += dy;
        self.clamp_canvas_scroll();
        if before != self.canvas_scroll {
            return true;
        }
        if dy > 0.0 {
            self.go_to(self.current + 1)
        } else {
            self.go_to(self.current.saturating_sub(1))
        }
    }

    /// Scroll the thumbnails pane (wheel over it).
    pub fn scroll_thumbs(&mut self, dy: f32) -> bool {
        let before = self.thumb_scroll;
        self.thumb_scroll += dy;
        self.clamp_thumb_scroll();
        before != self.thumb_scroll
    }

    /// Whether a window position is over the thumbnails pane.
    pub fn over_thumbs(&self, x: f32, y: f32) -> bool {
        inside(self.thumbs_rect(), x, y)
    }

    // ------------------------------------------------------------------ slide show

    /// Start the slide show at slide `from` (its transition plays from black).
    pub fn start_show(&mut self, from: usize) -> bool {
        if self.text_edit().is_some() {
            self.run(SlideCommand::TextExit);
        }
        let index = from.min(self.ids.len().saturating_sub(1));
        self.reset_show_tools();
        self.show = Some(Show::at(index));
        self.enter_slide(None);
        self.drag = None;
        true
    }

    /// Presenter View texts: the next slide's heading and the slide counter pattern.
    pub fn set_present_labels(&mut self, next: &str, counter: &str) {
        self.present_labels = (next.to_owned(), counter.to_owned());
    }

    /// Leave the slide show (the slide shown last becomes current).
    pub fn end_show(&mut self) -> bool {
        if let Some(i) = self.show.as_ref().map(|s| s.index) {
            self.finish_presenting(i);
        }
        let Some(s) = self.show.take() else {
            return false;
        };
        let ink = self.show_ink();
        self.pending_ink = (!ink.is_empty()).then_some(ink);
        self.reset_show_tools();
        self.reading = false;
        if !s.ended {
            self.go_to(s.index);
        }
        true
    }

    /// The show arrived on its slide (after `from`): read what it plays, start its
    /// transition, or its automatic effects when it has none.
    fn enter_slide(&mut self, from: Option<usize>) {
        let now = self.clock;
        let Some(index) = self.show.as_ref().map(|s| s.index) else {
            return;
        };
        let info = self
            .engine
            .slide_command(SlideCommand::Show { slide: index })
            .ok()
            .and_then(|r| r.show);
        let Some(s) = self.show.as_mut() else {
            return;
        };
        s.step = 0;
        s.fx_start = None;
        s.trans =
            crate::slide_show::Transition::of(info.as_ref().and_then(|i| i.transition.as_ref()))
                .map(|t| (from, t, now));
        s.info = info;
        if s.trans.is_none() {
            let automatic = s
                .info
                .as_ref()
                .is_some_and(|i| crate::slide_show::step_end(i, 0) > 0.0);
            s.fx_start = automatic.then_some(now);
            s.arrived = now;
        }
    }

    /// Whether a transition or effects are playing (the host draws every frame).
    pub fn animating(&self) -> bool {
        self.show
            .as_ref()
            .is_some_and(|s| !s.ended && (s.trans.is_some() || s.fx_start.is_some()))
    }

    /// The host's clock (ms, any origin) for the next frame; `true` when the show changed.
    pub fn set_clock(&mut self, ms: f64) -> bool {
        self.clock = ms;
        let Some(s) = self.show.as_mut() else {
            return false;
        };
        let mut changed = false;
        if let Some((_, t, start)) = &s.trans {
            if ms - start >= t.dur_ms {
                // Transition done: the slide's automatic effects (click 0) start.
                s.fx_start = Some(start + t.dur_ms);
                s.arrived = start + t.dur_ms;
                s.trans = None;
                changed = true;
            }
        }
        if let (Some(start), Some(info)) = (s.fx_start, s.info.as_ref()) {
            if ms - start >= crate::slide_show::step_end(info, s.step) {
                s.fx_start = None;
                s.arrived = ms;
                changed = true;
                // An effects preview goes on with the next click's effects.
                if s.preview == Some(true) && s.step < info.clicks {
                    s.step += 1;
                    s.fx_start = Some(ms);
                }
            }
        } else if s.fx_start.is_some() && s.info.is_none() {
            s.fx_start = None;
        }
        // A finished preview gives the canvas back.
        let done = match s.preview {
            Some(false) => s.trans.is_none(),
            Some(true) => s.trans.is_none() && s.fx_start.is_none(),
            None => false,
        };
        if done {
            self.show = None;
            return true;
        }
        changed || self.animating()
    }

    /// Advance on its own when the slide's time is up (§19.3.1.50 transition@advTm), once
    /// every click has played; `true` when the show moved on.
    pub fn show_tick(&mut self) -> bool {
        let now = self.clock;
        let due = self.show.as_ref().is_some_and(|s| {
            let Some(info) = s.info.as_ref() else {
                return false;
            };
            let after = info.transition.as_ref().and_then(|t| t.advance_after_ms);
            !s.ended
                && s.trans.is_none()
                && s.fx_start.is_none()
                && s.step >= info.clicks
                && after.is_some_and(|a| now - s.arrived >= f64::from(a))
        });
        let tick = self.timer_tick();
        (due && self.timings_on() && self.show_step(true)) || tick
    }

    fn show_step(&mut self, forward: bool) -> bool {
        let now = self.clock;
        let (next, prev) = match self.show.as_ref() {
            Some(s) => (self.next_in_show(s.index), self.prev_in_show(s.index)),
            None => return false,
        };
        let leaving = self.show.as_ref().map(|s| s.index);
        let Some(s) = self.show.as_mut() else {
            return false;
        };
        if forward {
            if s.ended {
                return self.end_show();
            }
            // A click during a transition or effects finishes them first.
            if let Some((_, t, start)) = &s.trans {
                s.fx_start = Some(start + t.dur_ms);
                s.trans = None;
                s.arrived = now;
                return true;
            }
            if let Some(start) = s.fx_start.take() {
                let end = s
                    .info
                    .as_ref()
                    .map_or(0.0, |i| crate::slide_show::step_end(i, s.step));
                if now - start < end {
                    s.arrived = now;
                    return true;
                }
            }
            let clicks = s.info.as_ref().map_or(0, |i| i.clicks);
            if s.step < clicks {
                s.step += 1;
                s.fx_start = Some(now);
                return true;
            }
            if let Some(next) = next {
                let from = s.index;
                s.index = next;
                if let Some(i) = leaving {
                    self.left_slide(i);
                }
                self.enter_slide(Some(from));
            } else {
                s.ended = true;
            }
            return true;
        }
        if s.ended {
            s.ended = false;
            return true;
        }
        s.trans = None;
        s.fx_start = None;
        if s.step > 0 {
            s.step -= 1;
            return true;
        }
        let Some(prev) = prev else {
            return false;
        };
        // Back to the previous slide, fully built.
        s.index = prev;
        if let Some(i) = leaving {
            self.left_slide(i);
        }
        self.enter_slide(None);
        if let Some(s) = self.show.as_mut() {
            s.trans = None;
            s.fx_start = None;
            s.step = s.info.as_ref().map_or(0, |i| i.clicks);
        }
        true
    }

    /// Jump to slide `index` of the show (Home / End), without its transition.
    fn show_jump(&mut self, index: usize) -> bool {
        if let Some(s) = self.show.as_mut() {
            *s = Show::at(index);
        }
        self.enter_slide(None);
        if let Some(s) = self.show.as_mut() {
            s.trans = None;
        }
        true
    }

    fn show_input(&mut self, input: &Input) -> bool {
        if !self.previewing() {
            if let Some(r) = self.show_tools_input(input) {
                return r;
            }
        }
        match *input {
            Input::Press { .. } | Input::Text(_) if matches!(input, Input::Press { .. }) => {
                self.show_step(true)
            }
            Input::Text(ref t) if t == " " || t == "n" => self.show_step(true),
            Input::Text(ref t) if t == "p" => self.show_step(false),
            Input::Key(k, _) => match k {
                Key::Escape => self.end_show(),
                Key::Right | Key::Down | Key::PageDown | Key::Enter => self.show_step(true),
                Key::Left | Key::Up | Key::PageUp | Key::Backspace => self.show_step(false),
                Key::Home => {
                    let first = self
                        .presenting
                        .as_ref()
                        .and_then(|p| p.plan.order.first().copied());
                    self.show_jump(first.unwrap_or(0))
                }
                Key::End => {
                    let last = self
                        .presenting
                        .as_ref()
                        .and_then(|p| p.plan.order.last().copied());
                    self.show_jump(last.unwrap_or(self.ids.len().saturating_sub(1)))
                }
                _ => false,
            },
            Input::Wheel(dy) => self.show_step(dy > 0.0),
            Input::Context { .. } => self.show_step(false),
            _ => false,
        }
    }

    /// Slide index shown by the slide show (`None` on the end screen).
    pub fn show_slide(&self) -> Option<usize> {
        self.show.as_ref().filter(|s| !s.ended).map(|s| s.index)
    }

    /// Where the show draws: the whole window, or the editing area in Reading view.
    fn show_area(&self) -> R {
        if self.presenter() {
            return self.presenter_slide_area();
        }
        if self.previewing() {
            let (x, y) = self.slide_origin();
            let (w, h) = self.slide_px(self.scale());
            return [x, y, w, h];
        }
        if self.reading {
            [self.origin.0, self.origin.1, self.size.0, self.size.1]
        } else {
            [0.0, 0.0, self.window.0, self.window.1]
        }
    }

    fn show_scale(&self) -> f32 {
        let [_, _, w, h] = self.show_area();
        (w / self.slide_size.0).min(h / self.slide_size.1)
    }

    // ------------------------------------------------------------------ images

    /// Bring one image up to date; `true` when something was drawn.
    fn ensure(
        &mut self,
        store: &mut dyn PageStore,
        key: usize,
        slide: usize,
        scale: f32,
    ) -> Result<bool, String> {
        let list = self.engine.slide(slide).map_err(|e| e.to_string())?;
        let band = if self.full.contains(&key) {
            None
        } else {
            match self.shown.get(&key) {
                Some((s, _)) if (*s - scale).abs() > f32::EPSILON => None,
                Some((_, old)) if self.stale.contains(&key) => match changed_region(old, list) {
                    Change::None => {
                        self.stale.remove(&key);
                        return Ok(false);
                    }
                    change => change.device_band(scale),
                },
                Some(_) => return Ok(false),
                None => None,
            }
        };
        // The editing area and thumbnails show the View tab's colour mode (slide shows
        // stay in colour).
        if self.color_mode != colormode::ColorMode::Color && key < SHOW_KEY {
            let shown = colormode::recolor(list, self.color_mode);
            store.update(key, &shown, &self.engine, scale, band)?;
        } else {
            store.update(key, list, &self.engine, scale, band)?;
        }
        self.shown.insert(key, (scale, list.clone()));
        self.stale.remove(&key);
        self.full.remove(&key);
        Ok(true)
    }

    fn forget(&mut self, store: &mut dyn PageStore, key: usize) {
        store.remove(key);
        self.shown.remove(&key);
        self.stale.remove(&key);
        self.full.remove(&key);
        self.recent.retain(|k| *k != key);
    }

    /// Bring the images shown in the next frame up to date (current slide, visible
    /// thumbnails; the show slide while presenting).
    pub fn prepare(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        // Images of slides that no longer exist.
        let live: HashSet<usize> = self.ids.iter().copied().collect();
        let dead: Vec<usize> = self
            .shown
            .keys()
            .copied()
            .filter(|k| !live.contains(&(k & (THUMB_KEY - 1))))
            .collect();
        for k in dead {
            self.forget(store, k);
        }
        if let Some(i) = self.show_slide() {
            let scale = self.show_scale();
            let key = SHOW_KEY + self.ids[i];
            self.ensure(store, key, i, scale)?;
            self.touch(key);
            // The slide a transition comes from.
            let from = self
                .show
                .as_ref()
                .and_then(|s| s.trans.as_ref())
                .and_then(|(f, _, _)| *f);
            if let Some(j) = from.filter(|j| *j < self.ids.len()) {
                let fk = SHOW_KEY + self.ids[j];
                self.ensure(store, fk, j, scale)?;
                self.touch(fk);
            }
            // Presenter View shows the next slide beside the current one.
            if self.presenter() {
                if let Some(j) = self.next_in_show(i) {
                    let nk = SHOW_KEY + self.ids[j];
                    self.ensure(store, nk, j, scale)?;
                    self.touch(nk);
                }
            }
            self.prepare_layers(store, i, scale)?;
            self.evict(store, SHOW_KEY, SHOW_CACHE);
            return Ok(());
        }
        if self.show.is_some() {
            return Ok(());
        }
        if self.sorter {
            return self.prepare_sorter(store);
        }
        let cur = self.current;
        if let Some(&id) = self.ids.get(cur) {
            let scale = self.scale();
            self.ensure(store, id, cur, scale)?;
            self.touch(id);
        }
        self.prepare_ghost(store)?;
        let ts = self.thumb_scale();
        // While typing, the current slide's thumbnail is brought up to date when idle
        // (`prefetch`), off the keystroke's path: it costs as much as the slide itself.
        let typing = self.text_edit().is_some();
        for i in self.visible_thumbs() {
            let key = THUMB_KEY + self.ids[i];
            if typing && i == cur && self.shown.contains_key(&key) {
                continue;
            }
            self.ensure(store, key, i, ts)?;
        }
        self.evict(store, 0, MAIN_CACHE);
        Ok(())
    }

    /// Store the show layers of slide `i` when its effects need them.
    fn prepare_layers(
        &mut self,
        store: &mut dyn PageStore,
        i: usize,
        scale: f32,
    ) -> Result<(), String> {
        let id = self.ids[i];
        self.prepare_morph(store, id, scale)?;
        let Some(s) = self.show.as_ref() else {
            return Ok(());
        };
        let animated = s
            .info
            .as_ref()
            .is_some_and(|info| !info.animations.is_empty());
        if !animated || s.layers == Some((id, scale)) {
            return Ok(());
        }
        let old = s.layers.and_then(|(old, _)| (old != id).then_some(old));
        let count = s.info.as_ref().map_or(0, |info| info.layers.len());
        if old.is_some() {
            for k in 0..64 {
                store.remove(LAYER_KEY + k);
                store.remove(PIECES_KEY + k);
            }
        }
        for k in 0..count {
            let images = self
                .show
                .as_ref()
                .and_then(|s| s.info.as_ref())
                .map(|info| crate::slide_show::layer_images(info, k));
            if let Some((base, pieces)) = images {
                store.update_layer(LAYER_KEY + k, &base, &self.engine, scale)?;
                match pieces {
                    Some(list) => store.update_layer(PIECES_KEY + k, &list, &self.engine, scale)?,
                    None => store.remove(PIECES_KEY + k),
                }
            }
        }
        if let Some(s) = self.show.as_mut() {
            s.layers = Some((id, scale));
        }
        Ok(())
    }

    /// Store the drawings of a Morph into slide `id` while it plays.
    fn prepare_morph(
        &mut self,
        store: &mut dyn PageStore,
        id: usize,
        scale: f32,
    ) -> Result<(), String> {
        let Some(s) = self.show.as_ref() else {
            return Ok(());
        };
        let morphing = s.trans.as_ref().is_some_and(|(_, t, _)| t.kind == "morph");
        let Some(m) = s
            .info
            .as_ref()
            .and_then(|i| i.morph.as_ref())
            .filter(|_| morphing)
        else {
            return Ok(());
        };
        if s.morph == Some((id, scale)) {
            return Ok(());
        }
        for (k, list) in crate::slide_show::morph_images(m).into_iter().enumerate() {
            match list {
                Some(l) => store.update_layer(MORPH_KEY + k, &l, &self.engine, scale)?,
                None => store.remove(MORPH_KEY + k),
            }
        }
        if let Some(s) = self.show.as_mut() {
            s.morph = Some((id, scale));
        }
        Ok(())
    }

    fn touch(&mut self, key: usize) {
        self.recent.retain(|k| *k != key);
        self.recent.push(key);
    }

    /// Keep at most `cap` images of a kind (main slides or show slides), least recent out.
    fn evict(&mut self, store: &mut dyn PageStore, kind: usize, cap: usize) {
        let of_kind = |k: &usize| (k & !(THUMB_KEY - 1)) == kind;
        let mut kept: Vec<usize> = self.recent.iter().copied().filter(of_kind).collect();
        while kept.len() > cap {
            let k = kept.remove(0);
            self.forget(store, k);
        }
        // Main images at an older zoom are of no use any more.
        if kind == 0 {
            let scale = self.scale();
            let old: Vec<usize> = self
                .shown
                .iter()
                .filter(|(k, (s, _))| of_kind(k) && (*s - scale).abs() > f32::EPSILON)
                .map(|(k, _)| *k)
                .collect();
            for k in old {
                self.forget(store, k);
            }
        }
    }

    /// Draw one image ahead of need (the neighbouring slides, thumbnails just off screen);
    /// `true` while more such work remains. Hosts call it when idle.
    pub fn prefetch(&mut self, store: &mut dyn PageStore) -> Result<bool, String> {
        if let Some(i) = self.show_slide() {
            let scale = self.show_scale();
            let near = [self.next_in_show(i), self.prev_in_show(i)];
            for j in near.into_iter().flatten() {
                if let Some(&id) = self.ids.get(j) {
                    let key = SHOW_KEY + id;
                    if self.needs(key, scale) {
                        self.ensure(store, key, j, scale)?;
                        self.touch(key);
                        return Ok(true);
                    }
                }
            }
            return Ok(false);
        }
        if self.show.is_some() {
            return Ok(false);
        }
        let scale = self.scale();
        let cur = self.current;
        // The current slide's thumbnail left behind while typing.
        if let Some(&id) = self.ids.get(cur) {
            let key = THUMB_KEY + id;
            let ts = self.thumb_scale();
            if self.needs(key, ts) && !self.sorter {
                self.ensure(store, key, cur, ts)?;
                return Ok(true);
            }
        }
        for j in [cur + 1, cur.wrapping_sub(1), cur + 2] {
            if let Some(&id) = self.ids.get(j) {
                if self.needs(id, scale) {
                    self.ensure(store, id, j, scale)?;
                    // Behind the current slide in the recency order.
                    self.recent.retain(|k| *k != id);
                    self.recent.insert(self.recent.len().saturating_sub(1), id);
                    return Ok(true);
                }
            }
        }
        let ts = self.thumb_scale();
        if self.in_page_master() || ts <= 0.0 {
            return Ok(false);
        }
        let vis = self.visible_thumbs();
        let more = vis.end..(vis.end + 4).min(self.ids.len());
        let less = vis.start.saturating_sub(2)..vis.start;
        for j in more.chain(less) {
            let key = THUMB_KEY + self.ids[j];
            if self.needs(key, ts) {
                self.ensure(store, key, j, ts)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn needs(&self, key: usize, scale: f32) -> bool {
        match self.shown.get(&key) {
            Some((s, _)) => {
                (*s - scale).abs() > f32::EPSILON
                    || self.stale.contains(&key)
                    || self.full.contains(&key)
            }
            None => true,
        }
    }

    // ------------------------------------------------------------------ painting

    /// Caret rectangle in window pixels (input method placement).
    pub fn caret_rect(&self) -> Option<[f32; 4]> {
        let [x1, y1, x2, y2] = self.caret_line()?;
        let (ax, ay) = self.to_px(x1, y1);
        let (bx, by) = self.to_px(x2, y2);
        Some([ax.min(bx), ay.min(by), 1.0, (by - ay).abs().max(1.0)])
    }

    fn poly(&self, b: &BoxR) -> Vec<PathSeg> {
        let c = b.corners();
        let p: Vec<(f32, f32)> = c.iter().map(|[x, y]| self.to_px(*x, *y)).collect();
        vec![
            PathSeg::M(p[0].0, p[0].1),
            PathSeg::L(p[1].0, p[1].1),
            PathSeg::L(p[2].0, p[2].1),
            PathSeg::L(p[3].0, p[3].1),
            PathSeg::Z,
        ]
    }

    /// Dashed outline of an axis-aligned or rotated box.
    /// The selection rectangle while it is dragged.
    fn paint_marquee(&self, p: &mut dyn Painter) {
        let Some([x, y, w, h]) = self.marquee() else {
            return;
        };
        let one = self.display_scale.round().max(1.0);
        p.fill_rect(x, y, w, h, MARQUEE_FILL);
        p.fill_rect(x, y, w, one, MARQUEE_EDGE);
        p.fill_rect(x, y + h - one, w, one, MARQUEE_EDGE);
        p.fill_rect(x, y, one, h, MARQUEE_EDGE);
        p.fill_rect(x + w - one, y, one, h, MARQUEE_EDGE);
    }

    /// Check the current slide's spelling again after a change (timer tick, as the
    /// reference checks while typing); `true` when the wavy lines changed.
    pub fn spell_tick(&mut self) -> bool {
        if !self.spell_dirty || self.show.is_some() || self.drag.is_some() {
            return false;
        }
        self.spell_dirty = false;
        let marks: Vec<([f64; 2], [f64; 2], bool)> = self
            .engine
            .slide_command_json(r#"{"cmd":"slideSpelling"}"#)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| {
                v.pointer("/slideSpell/marks")
                    .and_then(|m| m.as_array())
                    .cloned()
            })
            .unwrap_or_default()
            .iter()
            .filter_map(|m| {
                let pt = |k: &str| -> Option<[f64; 2]> {
                    let a = m.get(k)?.as_array()?;
                    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
                };
                let grammar = m.get("grammar").and_then(serde_json::Value::as_bool);
                Some((pt("from")?, pt("to")?, grammar.unwrap_or(false)))
            })
            .collect();
        let changed = marks != self.spell;
        self.spell = marks;
        changed
    }

    /// Spelling / grammar wavy lines of the current slide.
    pub fn spell_marks(&self) -> &[([f64; 2], [f64; 2], bool)] {
        &self.spell
    }

    /// Answer of a spelling command (`slideSpell*`) of the engine, its `slideSpell` member.
    pub fn spell_command(&mut self, cmd: &serde_json::Value) -> Option<serde_json::Value> {
        let out = self.engine.slide_command_json(&cmd.to_string()).ok()?;
        let mut v: serde_json::Value = serde_json::from_str(&out).ok()?;
        Some(v.get_mut("slideSpell")?.take())
    }

    /// The spelling / grammar issue under a window position on the slide (with its
    /// corrections), as the engine's `slideSpellAt` answers it.
    pub fn spell_at(&mut self, x: f32, y: f32) -> Option<serde_json::Value> {
        if !inside(self.canvas_rect(), x, y) || self.spell.is_empty() {
            return None;
        }
        let (sx, sy) = self.to_slide(x, y);
        let hit = self.spell_command(&serde_json::json!({
            "cmd": "slideSpellAt", "x": sx, "y": sy
        }))?;
        hit.get("hit").filter(|h| !h.is_null()).cloned()
    }

    /// Replace a flagged word (an issue of [`Self::spell_at`] / `slideSpellCheck`) by
    /// `text`, one undo step; the wavy lines are checked again.
    pub fn spell_fix(&mut self, issue: &serde_json::Value, text: &str) -> bool {
        self.spell_fix_many(&[(issue, text)])
    }

    /// Several corrections (Change All) as one undo step.
    pub fn spell_fix_many(&mut self, fixes: &[(&serde_json::Value, &str)]) -> bool {
        let edits: Vec<bo_engine::slide_edit::spell::SlideReplace> = fixes
            .iter()
            .filter_map(|(issue, text)| {
                serde_json::from_value(serde_json::json!({
                    "target": issue.get("target"),
                    "para": issue.get("para"),
                    "start": issue.get("start"),
                    "end": issue.get("end"),
                    "text": text,
                    "word": issue.get("word"),
                }))
                .ok()
            })
            .collect();
        if edits.is_empty() {
            return false;
        }
        let ok = match self.engine.slide_spell_fix(&edits) {
            Ok(r) if r.error.is_none() => {
                self.apply(r);
                true
            }
            _ => false,
        };
        self.spell_dirty = true;
        ok
    }

    /// Ignore a word everywhere (this session) or add it to the dictionary; the wavy lines
    /// are checked again.
    pub fn spell_learn(&mut self, word: &str, add: bool) -> bool {
        let cmd = if add {
            "slideSpellAddWord"
        } else {
            "slideSpellIgnore"
        };
        let ok = self
            .spell_command(&serde_json::json!({ "cmd": cmd, "word": word }))
            .is_some();
        self.spell_dirty = true;
        ok
    }

    /// Whether a window position is over the thumbnails pane; the slide there.
    pub fn thumb_under(&self, x: f32, y: f32) -> Option<Option<usize>> {
        inside(self.thumbs_rect(), x, y).then(|| self.thumb_at(x, y))
    }

    fn paint_spelling(&self, p: &mut dyn Painter) {
        let k = self.display_scale;
        for (a, b, grammar) in &self.spell {
            let (x0, y0) = self.to_px(a[0], a[1]);
            let (x1, _) = self.to_px(b[0], b[1]);
            let color = if *grammar { SPELL_GRAMMAR } else { SPELL_RED };
            let (step, amp) = ((2.0 * k).max(1.0), (1.0 * k).max(1.0));
            let y = (y0 + 1.5 * k).round();
            let mut segs = vec![PathSeg::M(x0, y)];
            let (mut dx, mut up) = (0.0, true);
            while dx < x1 - x0 {
                dx = (dx + step).min(x1 - x0);
                segs.push(PathSeg::L(x0 + dx, if up { y - amp } else { y }));
                up = !up;
            }
            p.path(&segs, None, Some((color, k.max(1.0))));
        }
    }

    /// Handout Master: where the slides go on the page, as dotted frames (reference: dark
    /// grey dots about 2.6 px apart at 125 %, over the white page).
    fn paint_handout_slots(&self, p: &mut dyn Painter, st: &SlideEditResult) {
        let Some(pm) = st.page_master.as_ref() else {
            return;
        };
        for r in &pm.slots {
            self.dashed(p, &BoxR::from_rect(*r), HANDOUT_SLOT, (0.8, 1.3));
        }
    }

    fn dashed(&self, p: &mut dyn Painter, b: &BoxR, color: Rgba, pattern: (f32, f32)) {
        let c = b.corners();
        let pts: Vec<(f32, f32)> = c.iter().map(|[x, y]| self.to_px(*x, *y)).collect();
        let k = self.display_scale;
        let (dash, gap) = (
            (pattern.0 * k).round().max(1.0),
            (pattern.1 * k).round().max(1.0),
        );
        let w = self.display_scale.round().max(1.0);
        for k in 0..4 {
            let (a, z) = (pts[k], pts[(k + 1) % 4]);
            let len = (z.0 - a.0).hypot(z.1 - a.1);
            if len < 1.0 {
                continue;
            }
            let (ux, uy) = ((z.0 - a.0) / len, (z.1 - a.1) / len);
            let mut t = 0.0;
            while t < len {
                let e = (t + dash).min(len);
                if ux.abs() < 1e-3 || uy.abs() < 1e-3 {
                    let (x0, y0) = (a.0 + ux * t, a.1 + uy * t);
                    let (x1, y1) = (a.0 + ux * e, a.1 + uy * e);
                    p.fill_rect(
                        x0.min(x1).round(),
                        y0.min(y1).round(),
                        (x1 - x0).abs().max(w),
                        (y1 - y0).abs().max(w),
                        color,
                    );
                } else {
                    p.path(
                        &[
                            PathSeg::M(a.0 + ux * t, a.1 + uy * t),
                            PathSeg::L(a.0 + ux * e, a.1 + uy * e),
                        ],
                        None,
                        Some((color, w)),
                    );
                }
                t += dash + gap;
            }
        }
    }

    /// Window position of the rotation handle: above the top edge's middle at a fixed
    /// distance (reference: 43 px at 125 %), turned with the box.
    fn rot_handle_px(&self, b: &BoxR) -> (f32, f32) {
        let (tx, ty) = b.to_slide(0.0, -b.h / 2.0);
        let (x, y) = self.to_px(tx, ty);
        let a = b.rot.to_radians() as f32;
        let d = self.display_scale * 34.4;
        (x + d * a.sin(), y - d * a.cos())
    }

    /// Selection frame (solid, or dotted while its text is edited), the eight round
    /// handles and the rotation handle, measured on the reference: 1 px frame of grey 97,
    /// handles 14 px across with a 2 px ring at 125 %.
    fn paint_handles(&self, p: &mut dyn Painter, b: &BoxR, handles: &[Handle], dotted: bool) {
        let w = self.display_scale.round().max(1.0);
        if dotted {
            self.dashed(p, b, FRAME, (1.6, 0.8));
        } else {
            p.path(&self.poly(b), None, Some((FRAME, w)));
        }
        let ring = (1.6 * self.display_scale).max(1.0);
        let r = 5.6 * self.display_scale - ring / 2.0;
        for h in handles {
            if h.kind == HandleKind::Rot {
                // Line from the top edge, then an open circle with an arrow head (own drawing).
                let (tx, ty) = b.to_slide(0.0, -b.h / 2.0);
                let (ex, ey) = self.to_px(tx, ty);
                let (x, y) = self.rot_handle_px(b);
                let k = 8.8 * self.display_scale;
                let a = b.rot.to_radians() as f32;
                let (sx, sy) = (x - k * a.sin(), y + k * a.cos());
                p.path(
                    &[PathSeg::M(ex, ey), PathSeg::L(sx, sy)],
                    None,
                    Some((FRAME, w)),
                );
                let (a0, a1) = (-1.2 + a, 4.0 + a);
                p.path(&arc(x, y, k, a0, a1), None, Some((FRAME, ring)));
                let (hx, hy) = (x + k * a0.cos(), y + k * a0.sin());
                let s = self.display_scale * 3.2;
                p.path(
                    &[
                        PathSeg::M(hx - s, hy - s * 0.2),
                        PathSeg::L(hx, hy),
                        PathSeg::L(hx - s * 0.2, hy + s),
                    ],
                    None,
                    Some((FRAME, ring)),
                );
                continue;
            }
            let (x, y) = self.to_px(h.x, h.y);
            let segs = arc(x.round(), y.round(), r, 0.0, std::f32::consts::TAU);
            p.path(&segs, Some(HANDLE_FILL), Some((FRAME, ring)));
        }
    }

    /// Paint the editing area (or the slide show over the whole window).
    pub fn paint(&self, p: &mut dyn Painter, text_size: f32) {
        if self.show.is_some() && !self.previewing() {
            self.paint_show(p, text_size);
            self.paint_show_tools(p);
            self.paint_rehearsal_bar(p, text_size);
            return;
        }
        let (ox, oy) = self.origin;
        let (w, h) = self.size;
        p.push_clip(ox, oy, w, h);
        if self.sorter {
            self.paint_sorter(p);
            p.pop_clip();
            return;
        }
        p.fill_rect(ox, oy, w, h, SLIDE_CANVAS);
        self.paint_thumbs(p);
        self.paint_thumb_drop(p);
        // Pane splitters.
        let tw = self.thumbs_w();
        p.fill_rect(ox + tw, oy, 1.0, h, SPLITTER);
        let canvas = self.canvas_rect();
        p.push_clip(canvas[0], canvas[1], canvas[2], canvas[3]);
        self.paint_slide(p);
        self.paint_ink(p);
        self.paint_marquee(p);
        p.pop_clip();
        if let Some(n) = self.notes_rect() {
            p.fill_rect(n[0], n[1] - 1.0, w - (n[0] - ox), 1.0, SPLITTER);
            self.paint_notes(p, n, text_size);
        }
        // A handout / notes master is one page: no scroll bar (reference).
        if !self.in_page_master() {
            self.paint_scrollbar(p);
        }
        if self.previewing() {
            p.push_clip(canvas[0], canvas[1], canvas[2], canvas[3]);
            self.paint_show(p, text_size);
            p.pop_clip();
        }
        p.pop_clip();
    }

    fn paint_thumbs(&self, p: &mut dyn Painter) {
        let r = self.thumbs_rect();
        p.push_clip(r[0], r[1], r[2], r[3]);
        let one = self.display_scale.round().max(1.0);
        let frame = self.px(2.4).max(2.0);
        self.paint_section_rows(p);
        for i in self.visible_thumbs() {
            if self.thumb_hidden(i) {
                continue;
            }
            let [x, y, tw, th] = self.thumb_rect(i);
            let key = THUMB_KEY + self.ids[i];
            let selected = i == self.current;
            if selected {
                // Accent frame three pixels wide, one pixel off the image (reference).
                let o = one + frame;
                p.fill_rect(x - o, y - o, tw + 2.0 * o, th + 2.0 * o, ACCENT);
                p.fill_rect(
                    x - one,
                    y - one,
                    tw + 2.0 * one,
                    th + 2.0 * one,
                    SLIDE_CANVAS,
                );
            } else {
                p.fill_rect(x - one, y - one, tw + 2.0 * one, th + 2.0 * one, THUMB_EDGE);
            }
            if self.shown.contains_key(&key) {
                p.page(key, x as i32, y as i32);
            } else {
                p.fill_rect(x, y, tw, th, HANDLE_FILL);
            }
            let n = (i + 1).to_string();
            let color = if selected { ACCENT_TEXT } else { THUMB_NUMBER };
            // Digit ink ends 22 px left of the image (advance: 16.4 logical px), digit tops 2 px
            // above it (reference).
            let nx = x - 16.4 * self.display_scale - self.digit_w * n.len() as f32;
            let ny = y - 8.8 * self.display_scale;
            p.text(&n, nx.round(), ny.round(), self.number_style(), color);
        }
        p.pop_clip();
    }

    /// A handle drag: the slide without the shapes, and the shapes at their new size half
    /// see-through over it (reference: 60 % opaque — 46/117/182 over white gives 130/172/211 —, original hidden, no handles).
    fn paint_ghost(&self, p: &mut dyn Painter, id: usize, (x, y): (f32, f32)) {
        // Pages are drawn at whole pixels.
        let (x, y) = ((x as i32) as f32, (y as i32) as f32);
        p.page(GHOST_KEY, x as i32, y as i32);
        let Some(b) = self.state.as_ref().and_then(|s| s.bounds) else {
            return;
        };
        let r = b.bounds();
        let (l, t) = self.to_px(r.x, r.y);
        let (rr, bb) = self.to_px(r.x + r.w, r.y + r.h);
        // Room for outlines, shadows drawn just outside the box.
        let pad = 8.0 * self.display_scale;
        let (w, h) = self.slide_px(self.scale());
        let x0 = (l - pad).max(x).floor();
        let y0 = (t - pad).max(y).floor();
        let x1 = (rr + pad).min(x + w).ceil();
        let y1 = (bb + pad).min(y + h).ceil();
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let dest = [x0, y0, x1 - x0, y1 - y0];
        p.page_part(id, [x0 - x, y0 - y, dest[2], dest[3]], dest, 0.6);
    }

    fn paint_slide(&self, p: &mut dyn Painter) {
        let (x, y) = self.slide_origin();
        let (w, h) = self.slide_px(self.scale());
        let one = 1.0;
        p.fill_rect(x - one, y - one, w + 2.0 * one, h + 2.0 * one, SLIDE_EDGE);
        let ghost = self.ghost_shown();
        match self.ids.get(self.current) {
            Some(id) if ghost => self.paint_ghost(p, *id, (x, y)),
            Some(id) if self.shown.contains_key(id) => p.page(*id, x as i32, y as i32),
            _ => p.fill_rect(x, y, w, h, HANDLE_FILL),
        }
        let Some(st) = self.state.as_ref() else {
            return;
        };
        let editing = st.text_edit.as_ref();
        // Empty placeholders: dashed outline and a grey prompt ("Click to add title"), as a
        // desktop presentation app shows them while editing.
        let s = self.scale();
        for (i, b) in st.placeholders.iter().enumerate() {
            self.dashed(p, b, PLACEHOLDER_EDGE, (2.4, 1.6));
            let kind = st.placeholder_kinds.get(i).map_or("obj", String::as_str);
            // Size and vertical anchor of the default title, subtitle and body styles, as
            // measured on the reference: the title slide's title sits on the bottom of its
            // box, the subtitle on the top, other titles in the middle.
            let (text, pt, centred, anchor) = match kind {
                "ctrTitle" => (&self.prompts[0], 54.0, true, 'b'),
                "title" => (&self.prompts[0], 44.0, true, 'm'),
                "subTitle" => (&self.prompts[1], 24.0, true, 't'),
                "body" | "obj" => (&self.prompts[2], 28.0, false, 't'),
                // Handout / notes master: "Header" / "Footer" at 12 pt (reference).
                "hdr" if st.page_master.is_some() => (&self.prompts[3], 12.0, false, 't'),
                "ftr" if st.page_master.is_some() => (&self.prompts[4], 12.0, false, 'b'),
                _ => continue,
            };
            // The layout's own prompt text (`hasCustomPrompt`) replaces the default one.
            let text = match st.placeholder_prompts.get(i) {
                Some(c) if !c.trim().is_empty() => c,
                _ => text,
            };
            if text.is_empty() {
                continue;
            }
            let size = (pt * s).max(6.0);
            let font = crate::Font {
                family: if centred && pt > 30.0 {
                    "Calibri Light"
                } else {
                    "Calibri"
                },
                size,
                weight: 400,
                italic: false,
                underline: false,
            };
            let (left, top) = self.to_px(b.cx - b.w / 2.0, b.cy - b.h / 2.0);
            let (cx, cy) = self.to_px(b.cx, b.cy);
            let (_, bottom) = self.to_px(b.cx, b.cy + b.h / 2.0);
            let line = size * 1.22;
            let rows: Vec<&str> = text.lines().collect();
            let n = rows.len().max(1) as f32;
            let y0 = match anchor {
                'b' => bottom - 3.6 * s - n * line - 0.15 * size,
                'm' => cy - n * line / 2.0,
                _ => top + 0.4 * s,
            };
            for (k, row) in rows.iter().enumerate() {
                // Average advance of the prompt in Calibri, measured: 0.39 em.
                let est_w = row.chars().count() as f32 * size * 0.39;
                let x = if centred {
                    cx - est_w / 2.0
                } else {
                    left + 7.2 * s
                };
                let y = y0 + k as f32 * line;
                p.text_font(row, x.round(), y.round(), &font, PROMPT_INK);
            }
        }
        self.paint_handout_slots(p, st);
        self.paint_spelling(p);
        if let Some(pv) = &self.preview {
            if self.live_since.is_none() {
                for b in &pv.boxes {
                    self.dashed(p, b, FRAME, (3.2, 1.6));
                }
            }
            let wline = self.display_scale.round().max(1.0);
            let (dash, gap) = (
                (3.5 * self.display_scale).round(),
                (2.1 * self.display_scale).round(),
            );
            for g in &pv.guides {
                let vertical = g.axis == bo_engine::slide_edit::guides::GuideAxis::V;
                let (a, z) = if vertical {
                    (self.to_px(g.pos, g.from), self.to_px(g.pos, g.to))
                } else {
                    (self.to_px(g.from, g.pos), self.to_px(g.to, g.pos))
                };
                // The reference runs the line about 10 px past both shapes at 125 %.
                let ext = 8.0 * self.display_scale;
                let (from, to) = if vertical {
                    (a.1.min(z.1) - ext, a.1.max(z.1) + ext)
                } else {
                    (a.0.min(z.0) - ext, a.0.max(z.0) + ext)
                };
                let mut t = from;
                while t < to {
                    let l = dash.min(to - t);
                    if vertical {
                        p.fill_rect(a.0.round(), t, wline, l, GUIDE);
                    } else {
                        p.fill_rect(t, a.1.round(), l, wline, GUIDE);
                    }
                    t += dash + gap;
                }
            }
        }
        if let Some(te) = editing {
            for r in &te.rects {
                p.path(&self.poly(r), Some(TEXT_SELECTION), None);
            }
            // The edited shape keeps its handles with a dotted frame, like the reference's
            // text frame, and the caret blinks.
            match st.bounds {
                Some(b) if te.cell.is_none() => self.paint_handles(p, &b, &st.handles, true),
                _ => self.dashed(p, &te.rect, FRAME, (1.6, 0.8)),
            }
            if self.caret_on && te.start == te.end && self.focus == Focus::Canvas {
                if let Some([x1, y1, x2, y2]) = te.caret {
                    let (ax, ay) = self.to_px(x1, y1);
                    let (bx, by) = self.to_px(x2, y2);
                    let cw = self.display_scale.round().max(1.0);
                    if (ax - bx).abs() < 0.5 {
                        p.fill_rect(ax.round(), ay.min(by).round(), cw, (by - ay).abs(), CARET);
                    } else {
                        p.path(
                            &[PathSeg::M(ax, ay), PathSeg::L(bx, by)],
                            None,
                            Some((CARET, cw)),
                        );
                    }
                }
            }
        } else if self.preview.is_none() && !self.hide_handles {
            for s in &st.selection {
                if st.selection.len() > 1 {
                    let w = self.display_scale.round().max(1.0);
                    p.path(&self.poly(&s.rect), None, Some((FRAME, w)));
                }
            }
            if let Some(b) = st.bounds {
                self.paint_handles(p, &b, &st.handles, false);
            }
        }
    }

    fn paint_notes(&self, p: &mut dyn Painter, r: R, text_size: f32) {
        let notes = self.notes();
        let (x, _) = self.slide_origin();
        let tx = x.max(r[0] + self.px(8.0));
        let size = Self::notes_size(text_size);
        let style = TextStyle { size, bold: false };
        let line = size * 1.33;
        let ty = r[1] + (2.8 * self.display_scale).round();
        p.push_clip(r[0], r[1], r[2], r[3]);
        if notes.is_empty() && self.focus != Focus::Notes {
            p.text(&self.labels.0, tx, ty, style, NOTES_HINT);
        } else {
            for (i, l) in notes.split('\n').enumerate() {
                p.text(l, tx, ty + i as f32 * line, style, BLACK);
            }
            if self.focus == Focus::Notes && self.caret_on {
                // Caret after the text before it on its line (measured by the window).
                let before = &notes[..byte_at(notes, self.notes_caret)];
                let row = before.matches('\n').count();
                let cx = tx + self.notes_caret_x;
                p.fill_rect(
                    cx.round(),
                    (ty + row as f32 * line).round(),
                    self.display_scale.round().max(1.0),
                    line.round(),
                    CARET,
                );
            }
        }
        p.pop_clip();
    }

    /// Canvas scroll bar: slide position in the deck, and previous / next slide buttons.
    fn paint_scrollbar(&self, p: &mut dyn Painter) {
        let c = self.canvas_rect();
        let w = self.sb_w();
        let x = c[0] + c[2];
        let (top, h) = (c[1] + self.px(2.0), c[3] - self.px(2.0));
        p.fill_rect(x, c[1], w, c[3], Rgba(0xf3, 0xf3, 0xf3, 255));
        let edge = Rgba(0x94, 0x94, 0x94, 255);
        let arrow = Rgba(0x26, 0x26, 0x26, 255);
        let boxed = |p: &mut dyn Painter, r: R| {
            p.fill_rect(r[0], r[1], r[2], r[3], edge);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, HANDLE_FILL);
        };
        let up = [x, top, w, w];
        // Previous / next slide buttons at the bottom, under the down arrow.
        let pn = self.px(16.0);
        let down = [x, top + h - 2.0 * pn - self.px(4.0) - w, w, w];
        boxed(p, up);
        boxed(p, down);
        let n = self.ids.len().max(1) as f32;
        let track = (down[1] - (up[1] + w)).max(0.0);
        let th = (track / n).max(self.px(12.0)).min(track);
        let ty = up[1] + w + (track - th) * (self.current as f32 / (n - 1.0).max(1.0));
        if th > 2.0 {
            boxed(p, [x, ty.round(), w, th.round()]);
        }
        let k = self.px(3.0);
        let tri = |p: &mut dyn Painter, cx: f32, cy: f32, dir: f32| {
            p.path(
                &[
                    PathSeg::M(cx - k, cy + dir * k / 2.0),
                    PathSeg::L(cx + k, cy + dir * k / 2.0),
                    PathSeg::L(cx, cy - dir * (k / 2.0 + 1.0)),
                    PathSeg::Z,
                ],
                Some(arrow),
                None,
            );
        };
        tri(p, up[0] + w / 2.0, up[1] + w / 2.0, 1.0);
        tri(p, down[0] + w / 2.0, down[1] + w / 2.0, -1.0);
        let grey = Rgba(0x66, 0x66, 0x66, 255);
        for (i, dir) in [(0.0, 1.0), (1.0, -1.0)] {
            let cy = down[1] + w + self.px(4.0) + i * pn + pn / 2.0;
            let cx = x + w / 2.0;
            for off in [-k * 0.8, k * 0.8] {
                p.path(
                    &[
                        PathSeg::M(cx - k, cy + off + dir * k / 2.0),
                        PathSeg::L(cx, cy + off - dir * k / 2.0),
                        PathSeg::L(cx + k, cy + off + dir * k / 2.0),
                    ],
                    None,
                    Some((grey, self.display_scale.max(1.0))),
                );
            }
        }
    }

    /// Previous / next slide buttons of the canvas scroll bar: -1, +1 or 0 (none).
    pub fn scrollbar_hit(&self, x: f32, y: f32) -> i32 {
        let c = self.canvas_rect();
        let w = self.sb_w();
        let sx = c[0] + c[2];
        if x < sx || x >= sx + w || y < c[1] || y >= c[1] + c[3] {
            return 0;
        }
        let (top, h) = (c[1] + self.px(2.0), c[3] - self.px(2.0));
        let pn = self.px(16.0);
        let prev_y = top + h - 2.0 * pn;
        let down = y >= prev_y - self.px(4.0) - w && y < prev_y - self.px(4.0);
        if (y >= prev_y && y < prev_y + pn) || y < top + w {
            -1
        } else {
            i32::from(y >= prev_y + pn || down)
        }
    }

    fn paint_show(&self, p: &mut dyn Painter, text_size: f32) {
        use crate::slide_show::{paint_transition, SlideImage};
        if self.presenter() {
            self.paint_presenter(p, text_size);
        }
        let [ax, ay, ww, wh] = self.show_area();
        p.fill_rect(ax, ay, ww, wh, BLACK);
        let Some(i) = self.show_slide() else {
            let style = TextStyle {
                size: text_size,
                bold: false,
            };
            p.text(
                &self.labels.1,
                self.px(16.0),
                self.px(12.0),
                style,
                Rgba(0xff, 0xff, 0xff, 255),
            );
            return;
        };
        let Some(s) = self.show.as_ref() else {
            return;
        };
        let scale = self.show_scale();
        let (w, h) = self.slide_px(scale);
        let r = [
            (ax + (ww - w) / 2.0).floor(),
            (ay + (wh - h) / 2.0).floor(),
            w.round(),
            h.round(),
        ];
        let size = (
            (self.slide_size.0 * scale).ceil(),
            (self.slide_size.1 * scale).ceil(),
        );
        let now = self.clock;
        let key = SHOW_KEY + self.ids[i];
        let elapsed = match (s.trans.as_ref(), s.fx_start) {
            (Some(_), _) => 0.0,
            (None, Some(start)) => now - start,
            (None, None) => f64::INFINITY,
        };
        let layered = s
            .layers
            .is_some_and(|(id, sc)| id == self.ids[i] && sc == scale);
        let to = ShowImage {
            key,
            size,
            layers: if layered {
                s.info
                    .as_ref()
                    .map(|info| (info, crate::slide_show::pieces(info, s.step, elapsed)))
            } else {
                None
            },
        };
        if !self.shown.contains_key(&key) {
            return;
        }
        match s.trans.as_ref() {
            Some((_, t, start))
                if t.kind == "morph"
                    && s.morph
                        .is_some_and(|(id, sc)| id == self.ids[i] && sc == scale) =>
            {
                let k = ((now - start) / t.dur_ms).clamp(0.0, 1.0);
                if let Some(m) = s.info.as_ref().and_then(|i| i.morph.as_ref()) {
                    let slide = (f64::from(self.slide_size.0), f64::from(self.slide_size.1));
                    crate::slide_show::paint_morph(p, m, MORPH_KEY, size, r, slide, k, &|_| true);
                }
            }
            Some((from, t, start)) => {
                let k = ((now - start) / t.dur_ms).clamp(0.0, 1.0);
                let from_img = from
                    .filter(|j| *j < self.ids.len())
                    .map(|j| SHOW_KEY + self.ids[j])
                    .filter(|fk| self.shown.contains_key(fk))
                    .map(|fk| ShowImage {
                        key: fk,
                        size,
                        layers: None,
                    });
                paint_transition(
                    p,
                    t,
                    k,
                    r,
                    from_img.as_ref().map(|f| f as &dyn SlideImage),
                    &to,
                );
            }
            None => to.draw(p, r, [0.0, 0.0, 1.0, 1.0], 1.0),
        }
    }
}

/// A slide as the show draws it: its stored image, or its layers in their current looks.
struct ShowImage<'a> {
    key: usize,
    size: (f32, f32),
    layers: Option<(&'a ShowOut, Vec<crate::slide_show::Piece>)>,
}

impl crate::slide_show::SlideImage for ShowImage<'_> {
    fn draw(&self, p: &mut dyn Painter, dest: [f32; 4], part: [f32; 4], opacity: f32) {
        let Some((info, pieces)) = &self.layers else {
            crate::slide_show::draw_part(p, self.key, self.size, dest, part, opacity);
            return;
        };
        let clip = [
            dest[0] + part[0] * dest[2],
            dest[1] + part[1] * dest[3],
            part[2] * dest[2],
            part[3] * dest[3],
        ];
        p.push_clip(clip[0], clip[1], clip[2], clip[3]);
        let slide = (f64::from(info.width), f64::from(info.height));
        for pc in pieces {
            let key = match pc.image {
                crate::slide_show::PieceImage::Base(k) => LAYER_KEY + k,
                crate::slide_show::PieceImage::Pieces(k) => PIECES_KEY + k,
            };
            crate::slide_show::paint_piece(p, key, self.size, dest, pc, slide, opacity);
        }
        p.pop_clip();
    }

    fn draw_turned(&self, p: &mut dyn Painter, dest: [f32; 4], opacity: f32, degrees: f32) {
        if self.layers.is_some() {
            self.draw(p, dest, crate::slide_show::WHOLE, opacity);
            return;
        }
        let src = [0.0, 0.0, self.size.0, self.size.1];
        p.page_part_rotated(self.key, src, dest, opacity, degrees);
    }
}

/// An arc of radius `r` around (`cx`, `cy`) from angle `a0` to `a1` (radians) as line
/// segments (closed when it is a full turn).
fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32) -> Vec<PathSeg> {
    let n = 24;
    let mut segs = Vec::with_capacity(n + 2);
    for i in 0..=n {
        let a = a0 + (a1 - a0) * i as f32 / n as f32;
        let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
        segs.push(if i == 0 {
            PathSeg::M(x, y)
        } else {
            PathSeg::L(x, y)
        });
    }
    if (a1 - a0 - std::f32::consts::TAU).abs() < 1e-3 {
        segs.push(PathSeg::Z);
    }
    segs
}

#[cfg(test)]
#[path = "slide_view_tests.rs"]
mod tests;
