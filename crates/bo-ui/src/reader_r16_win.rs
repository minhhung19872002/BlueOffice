//! BlueReader round 16 (a child of `reader_window`), measured on the reference reader with
//! posted messages on a scratch copy (`docs/ui/REPORT-reader.md`, "Vòng 16"):
//!
//! - Shift+Space pages up. Space arrives as text (`WM_CHAR`), which does not say whether Shift
//!   was held; the Windows host also reports Shift+Space on key down as `Key::Letter(' ')` with
//!   Shift, and the text that follows pages up instead of down.
//! - Ctrl+Shift+N picks the Note tool; the Note tool shows a Format pane at the right of the
//!   document (Preview, Note Style icons, colours, Opacity, Arrange — disabled until a comment
//!   is selected), as the reference does. New notes take the pane's icon, colour and opacity
//!   (`/Name`, `/C`, `/CA`, ISO 32000-1 §12.5.6.4).
//! - Mouse, measured with posted clicks (the button held in the thread's key state): a triple
//!   click selects the line; the middle button starts auto-scroll (an anchor mark, speed by the
//!   pointer's distance, any click or Esc ends it), driven by the Windows host.
//! - Assistive technology: the focused element and the page text with its boxes
//!   (`access_focus16`, `access_text16`) for the host's UI Automation provider.
use super::{circle, inside, rounded, ReaderWindow, Tool, WHITE};
use crate::reader_view::Selection;
use crate::{Input, Key, Mods, Painter, TextMeasure};
use bo_pdfread::{AnnotKind, NOTE_ICONS};
use bo_render::{PathSeg, Rgba};

/// Width of the Format pane (logical pixels: 476 px at 125 %, strip included).
const PANE_W: f32 = 380.8;
/// The pane's background, its tab row and edges (measured).
const PANE_BG: Rgba = Rgba(251, 251, 253, 255);
const TAB_ROW: Rgba = Rgba(240, 241, 245, 255);
const STRIP: Rgba = Rgba(230, 233, 240, 255);
const EDGE: Rgba = Rgba(199, 204, 217, 255);
const RULE: Rgba = Rgba(232, 232, 232, 255);
/// Preview box: edge, inner ring, fill.
const PREVIEW_RING: Rgba = Rgba(219, 221, 228, 255);
const PREVIEW_FILL: Rgba = Rgba(238, 238, 238, 255);
/// Section labels and arrows.
const LABEL: Rgba = Rgba(0, 0, 0, 255);
const ARROW: Rgba = Rgba(153, 153, 153, 255);
/// Icons and their disabled grey.
const INK: Rgba = Rgba(51, 51, 51, 255);
const DIM: Rgba = Rgba(175, 175, 176, 255);
const SWATCH_EDGE: Rgba = Rgba(196, 196, 197, 255);
const HOVER: Rgba = Rgba(229, 231, 238, 255);
/// The slider: knob ring.
const KNOB_RING: Rgba = Rgba(217, 220, 229, 255);

/// Note colours of the pane, two rows of five (measured on the reference's swatches).
pub(crate) const NOTE_COLOURS: [[u8; 3]; 10] = [
    [255, 237, 0],
    [141, 240, 253],
    [165, 251, 112],
    [236, 94, 193],
    [255, 0, 0],
    [51, 158, 0],
    [45, 99, 245],
    [0, 0, 0],
    [136, 136, 136],
    [255, 255, 254],
];

/// What a part of the pane does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Close,
    Icon(usize),
    Colour(usize),
    Slider,
}

/// The note style new notes get (kept when the pane closes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NoteStyle {
    /// Index in [`NOTE_ICONS`].
    pub(crate) icon: usize,
    pub(crate) colour: [u8; 3],
    /// Opacity in per cent.
    pub(crate) opacity: u8,
}

impl Default for NoteStyle {
    fn default() -> Self {
        // The reference starts with the speech-bubble icon in yellow at 100 %.
        Self {
            icon: 0,
            colour: NOTE_COLOURS[0],
            opacity: 100,
        }
    }
}

/// Round 16 state of the window.
#[derive(Debug, Default)]
pub(crate) struct R16 {
    /// Shift+Space went down: the space text that follows pages up.
    shift_space: bool,
    pub(crate) style: NoteStyle,
    /// The Format pane shows (Note tool) and was not closed with ×.
    pub(crate) pane: bool,
    closed: bool,
    hits: Vec<([f32; 4], Part)>,
    hover: Option<Part>,
    slider: [f32; 4],
    dragging: bool,
    /// Middle-button auto-scroll anchor (window pixels).
    autoscroll: Option<(f32, f32)>,
}

impl ReaderWindow {
    fn px16(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    // ---- Input -------------------------------------------------------------------------------

    /// Round 16 input before the other handlers; `Some(redraw)` when taken.
    pub(crate) fn route16(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        match input {
            Input::Key(Key::Letter(' '), mods) if mods.shift && !mods.ctrl && !mods.alt => {
                self.r16.shift_space = true;
                return Some(false);
            }
            Input::Key(..) => self.r16.shift_space = false,
            Input::Text(s)
                if s == " "
                    && std::mem::take(&mut self.r16.shift_space)
                    && self.typing_target16().is_none() =>
            {
                return Some(self.shift_space16());
            }
            _ => {}
        }
        if self.menu.is_some() || self.dialog.is_some() || self.extra_modal() {
            return None;
        }
        if let Input::Key(
            Key::Letter('n'),
            Mods {
                ctrl: true,
                shift: true,
                alt: false,
            },
        ) = input
        {
            if self.tabs.is_empty() || self.show_start {
                return Some(false);
            }
            self.run_command("tool.note", m);
            self.r16.closed = false;
            return Some(true);
        }
        if self.r16.pane {
            if let Some(r) = self.route_pane16(input) {
                return Some(r);
            }
        }
        if let Input::Press { x, y, count, .. } = *input {
            if let Some(r) = self.press16(x, y, count) {
                return Some(r);
            }
        }
        None
    }

    /// A text field that takes typed text now (the Shift+Space page-up does not apply).
    fn typing_target16(&self) -> Option<&'static str> {
        if self.search_focus {
            Some("search")
        } else if self.r15.find.as_ref().is_some_and(|f| f.focus) {
            Some("find")
        } else if self.r15.page_edit.is_some() {
            Some("page")
        } else {
            None
        }
    }

    /// Shift+Space: page up by the Page Up step.
    pub(crate) fn shift_space16(&mut self) -> bool {
        if self.extra.mode == super::extra::Mode::Full {
            return false;
        }
        let d = self.page_step15();
        match self.tabs.get_mut(self.active) {
            Some(t) => {
                t.view.scroll_by(0.0, -d);
                true
            }
            None => false,
        }
    }

    /// With the pane shown the document's scroll bar meets the pane (the reference keeps no
    /// margin there); called after the document area's layout.
    pub(crate) fn layout16(&mut self) {
        if !self.r16.pane || self.right_pane_rect()[2] <= 0.0 {
            return;
        }
        let margin = self.px16(6.4);
        self.vbar[0] += margin;
        self.doc_rect[2] += margin;
    }

    /// Round 16 enable rules: the Home tab's Clipboard menu dims while a commenting tool is
    /// picked (measured on the reference: on with Hand and nothing selected, dim with Note).
    pub(crate) fn sixteen_enabled(&self, id: &str) -> Option<bool> {
        match id {
            "menu.clipboard" => Some(matches!(self.tool, Tool::Hand | Tool::Select)),
            _ => None,
        }
    }

    /// After every input: the pane follows the Note tool.
    pub(crate) fn after16(&mut self) -> bool {
        let want = self.tool == Tool::Note && !self.r16.closed && !self.tabs.is_empty();
        if self.tool != Tool::Note {
            self.r16.closed = false;
        }
        if want && self.search9.open {
            // One pane at a time: the Format pane replaces Advanced Search.
            self.search9.open = false;
        }
        let changed = want != self.r16.pane;
        self.r16.pane = want;
        if changed {
            self.set_right_pane(if want { PANE_W } else { 0.0 });
        }
        if want {
            self.layout_pane16();
        }
        changed
    }

    fn route_pane16(&mut self, input: &Input) -> Option<bool> {
        let pane = self.right_pane_rect();
        match *input {
            Input::Move { x, y } if inside(pane, x, y) || self.r16.hover.is_some() => {
                let h = self.pane_hit16(x, y);
                let changed = h != self.r16.hover;
                self.r16.hover = h;
                inside(pane, x, y).then_some(changed)
            }
            Input::Press { x, y, .. } if inside(pane, x, y) => {
                match self.pane_hit16(x, y) {
                    Some(Part::Close) => {
                        self.r16.closed = true;
                        self.r16.hover = None;
                    }
                    Some(Part::Icon(i)) => self.r16.style.icon = i,
                    Some(Part::Colour(i)) => self.r16.style.colour = NOTE_COLOURS[i],
                    Some(Part::Slider) => {
                        self.r16.dragging = true;
                        self.slide16(x);
                    }
                    None => {}
                }
                Some(true)
            }
            Input::Drag { x, .. } if self.r16.dragging => {
                self.slide16(x);
                Some(true)
            }
            Input::Release { .. } if self.r16.dragging => {
                self.r16.dragging = false;
                Some(true)
            }
            Input::Wheel(_) if inside(pane, self.pointer.0, self.pointer.1) => Some(false),
            _ => None,
        }
    }

    fn pane_hit16(&self, x: f32, y: f32) -> Option<Part> {
        self.r16
            .hits
            .iter()
            .find(|(r, _)| inside(*r, x, y))
            .map(|(_, p)| *p)
    }

    fn slide16(&mut self, x: f32) {
        let s = self.r16.slider;
        if s[2] > 0.0 {
            let f = ((x - s[0]) / s[2]).clamp(0.0, 1.0);
            self.r16.style.opacity = (f * 100.0).round() as u8;
        }
    }

    /// Hit boxes of the pane (window pixels).
    fn layout_pane16(&mut self) {
        let r = self.right_pane_rect();
        let px = |v: f32| (v * self.scale).round();
        let (x0, y0) = (r[0], r[1]);
        let mut hits = Vec::new();
        // × of the Format tab (measured at 103 px from the pane, 25 px down).
        hits.push((
            [x0 + px(76.8), y0 + px(9.6), px(16.0), px(16.0)],
            Part::Close,
        ));
        for i in 0..NOTE_ICONS.len() {
            let (cx, cy) = icon_centre(i);
            let s = px(30.4);
            hits.push((
                [x0 + px(cx) - s / 2.0, y0 + px(cy) - s / 2.0, s, s],
                Part::Icon(i),
            ));
        }
        for i in 0..NOTE_COLOURS.len() {
            let (cx, cy) = colour_centre(i);
            let s = px(24.0);
            hits.push((
                [x0 + px(cx) - s / 2.0, y0 + px(cy) - s / 2.0, s, s],
                Part::Colour(i),
            ));
        }
        let slider = [
            x0 + px(130.4),
            y0 + px(395.2) - px(8.0),
            px(124.8),
            px(16.0),
        ];
        hits.push((slider, Part::Slider));
        self.r16.slider = slider;
        self.r16.hits = hits;
    }

    // ---- Notes -------------------------------------------------------------------------------

    /// Colour of a new note.
    pub(crate) fn note_colour16(&self) -> [u8; 3] {
        self.r16.style.colour
    }

    /// A note was just added on `page`: give it the pane's icon and opacity.
    pub(crate) fn style_new_note16(&mut self, page: usize) {
        let style = self.r16.style;
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let doc = t.view.doc_mut();
        let Some(a) = doc
            .annotations()
            .into_iter()
            .rfind(|a| a.page == page && a.kind == AnnotKind::Note)
        else {
            return;
        };
        let icon = NOTE_ICONS.get(style.icon).copied().unwrap_or("Comment");
        match doc.set_note_style(&a, icon, f32::from(style.opacity) / 100.0) {
            Ok(()) => t.view.invalidate_page(page),
            Err(e) => self.status = e,
        }
    }

    // ---- Mouse ---------------------------------------------------------------------------------

    /// A triple click over page text with the Hand or Select tool selects the line (measured
    /// on the reference with posted clicks; double click = word and Shift+click are in
    /// `press`). `Some(redraw)` when taken.
    pub(crate) fn press16(&mut self, x: f32, y: f32, count: u8) -> Option<bool> {
        if count < 3 || !matches!(self.tool, Tool::Hand | Tool::Select) {
            return None;
        }
        if !inside(self.doc_rect, x, y) {
            return None;
        }
        let t = self.tabs.get_mut(self.active)?;
        let (page, px, py) = t.view.page_at(x, y)?;
        let layer = t.view.text(page)?;
        if !layer.over_text(px, py) {
            return None;
        }
        let c = layer.hit(px, py, 2.0)?;
        let (a, b) = line_span(layer, c);
        t.view.set_selection(Some(Selection { page, a, b }));
        Some(true)
    }

    /// Middle-button auto-scroll (the host drives it): the anchor mark at (`x`, `y`), or none.
    pub fn set_autoscroll16(&mut self, at: Option<(f32, f32)>) -> bool {
        let changed = self.r16.autoscroll != at;
        self.r16.autoscroll = at;
        changed
    }

    /// Auto-scroll step: the pointer is `dy` device pixels below the anchor (negative above)
    /// and `ms` milliseconds passed. Measured on the reference at 125 %: nothing within 28 px,
    /// then about 16.5 px per second for every pixel further.
    pub fn autoscroll_step16(&mut self, dy: f32, ms: f32) -> bool {
        let dead = 22.4 * self.scale;
        let d = dy.abs() - dead;
        if d <= 0.0 || self.r16.autoscroll.is_none() {
            return false;
        }
        let speed = 16.5 * d * 1.25 / self.scale.max(0.5);
        let step = speed * ms / 1000.0 * dy.signum();
        match self.tabs.get_mut(self.active) {
            Some(t) => {
                t.view.scroll_by(0.0, step);
                true
            }
            None => false,
        }
    }

    // ---- Assistive technology ----------------------------------------------------------------

    /// Key of the element with keyboard focus (`doc`, `edit:search`, `edit:find`,
    /// `edit:page`, `tab:<n>` while a menu or backstage shows); `None` without a document.
    pub fn access_focus16(&self) -> Option<String> {
        if let Some(t) = self.typing_target16() {
            return Some(format!("edit:{t}"));
        }
        if self.show_start || self.tabs.is_empty() {
            return None;
        }
        Some("doc".into())
    }

    /// [`ReaderWindow::accessibility12`] with the text fields that can take the keyboard focus
    /// (title bar search, the Search box's find field, the page box while edited).
    pub fn accessibility16(&self) -> Vec<crate::AccessItem> {
        let mut out = self.accessibility12();
        let mut field = |key: &str, name: String, rect: [f32; 4]| {
            if rect[2] > 0.0 {
                out.push(crate::AccessItem {
                    key: key.into(),
                    name,
                    role: crate::AccessRole::Edit,
                    rect,
                    enabled: true,
                    selected: false,
                });
            }
        };
        field("edit:search", self.t("pdfReader15.findHint"), self.search);
        if let Some(f) = &self.r15.find {
            field("edit:find", self.t("pdfReader15.find"), f.field_rect());
        }
        if self.r15.page_edit.is_some() {
            field("edit:page", self.t("pdfReader.page"), self.page_box);
        }
        out
    }

    /// The text of the pages in view for UI Automation's Text pattern: the text, for each
    /// character its box in window pixels (`None` for an added space or line break), and the
    /// selection as character offsets of the text.
    pub fn access_text16(&mut self) -> Option<AccessText> {
        if self.show_start {
            return None;
        }
        let t = self.tabs.get_mut(self.active)?;
        let pages = t.view.visible_pages();
        let sel = t.view.selection();
        let mut out = AccessText::default();
        for page in pages {
            let Some(layer) = t.view.text(page) else {
                continue;
            };
            let text: Vec<char> = layer.text().chars().collect();
            let map = layer.reading_chars();
            let rects: Vec<[f32; 4]> = layer.chars().iter().map(|c| c.rect).collect();
            if !out.text.is_empty() && !out.text.ends_with('\n') {
                out.text.push('\n');
                out.boxes.push(None);
                out.chars.push(None);
            }
            let start = out.boxes.len();
            for (k, ch) in text.iter().enumerate() {
                out.text.push(*ch);
                let i = map.get(k).copied().unwrap_or(usize::MAX);
                let b = rects
                    .get(i)
                    .and_then(|r| t.view.doc_rect_to_window(page, *r));
                out.boxes.push(b);
                out.chars.push((i != usize::MAX).then_some((page, i)));
            }
            if let Some(s) = sel.filter(|s| s.page == page) {
                let (lo, hi) = (s.a.min(s.b), s.a.max(s.b));
                let first = map.iter().position(|&i| i != usize::MAX && i >= lo);
                let last = map.iter().rposition(|&i| i != usize::MAX && i <= hi);
                if let (Some(f), Some(l)) = (first, last) {
                    out.selection = Some((start + f, start + l + 1));
                }
            }
            out.pages.push((page, start));
        }
        Some(out)
    }

    /// Select characters `a..=b` of `page` (a UI Automation client selected a text range).
    pub fn select_text16(&mut self, page: usize, a: usize, b: usize) -> bool {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return false;
        };
        if page >= t.view.page_count() {
            return false;
        }
        t.view.set_selection(Some(Selection { page, a, b }));
        true
    }

    // ---- Paint -------------------------------------------------------------------------------

    pub(crate) fn paint16(&self, p: &mut dyn Painter) {
        if let Some((ax, ay)) = self.r16.autoscroll {
            self.paint_anchor16(p, ax, ay);
        }
        if !self.r16.pane {
            return;
        }
        let r = self.right_pane_rect();
        if r[2] <= 0.0 {
            return;
        }
        let px = |v: f32| (v * self.scale).round();
        let (x0, y0) = (r[0], r[1]);
        let one = self.scale.max(1.0).round();
        p.push_clip(r[0], r[1], r[2], r[3]);
        p.fill_rect(r[0], r[1], r[2], r[3], PANE_BG);
        // Tab row (45 px at 125 %) with the Format tab, and the collapse strip.
        let row_h = px(35.2);
        p.fill_rect(x0, y0, r[2], row_h, TAB_ROW);
        p.fill_rect(x0, y0 + row_h - one, r[2], one, EDGE);
        p.fill_rect(x0, y0 + row_h, px(13.6), r[3] - row_h, STRIP);
        p.fill_rect(x0 + px(13.6), y0 + row_h, one, r[3] - row_h, EDGE);
        let tab = [x0 + px(17.6), y0, px(76.8), row_h];
        p.fill_rect(tab[0], tab[1], tab[2], tab[3], WHITE);
        p.fill_rect(tab[0], tab[1], one, tab[3], EDGE);
        p.fill_rect(tab[0] + tab[2] - one, tab[1], one, tab[3], EDGE);
        // Measured: "Format" 44 px wide, the pane's labels ("Note Style" 60 px) a size smaller.
        let tab_st = self.style(14.0, false);
        let st = self.style(13.0, false);
        let line = st.size * 1.33;
        // Labels inside the sections ("Page Center" 80 px, "Opacity:" 55 px) are larger.
        let lst = self.style(15.0, false);
        let lline = lst.size * 1.33;
        p.text(
            &self.t("pdfReader16.format"),
            tab[0] + px(12.0),
            y0 + ((row_h - tab_st.size * 1.33) / 2.0).round(),
            tab_st,
            self.theme().accent_text,
        );
        let close = self.r16.hits.first().map_or([0.0; 4], |h| h.0);
        if self.r16.hover == Some(Part::Close) {
            p.fill_rect(close[0], close[1], close[2], close[3], HOVER);
        }
        let k = px(4.4);
        let (cx, cy) = (close[0] + close[2] / 2.0, close[1] + close[3] / 2.0);
        p.path(
            &[
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((INK, one)),
        );
        // Collapse arrow on the strip (▸), level with the document's middle.
        let ay = (y0 + row_h + (r[3] - row_h) / 2.0).round();
        let ax = x0 + px(5.6);
        p.path(
            &[
                PathSeg::M(ax, ay - px(4.0)),
                PathSeg::L(ax + px(4.0), ay),
                PathSeg::L(ax, ay + px(4.0)),
                PathSeg::Z,
            ],
            Some(INK),
            None,
        );
        // Sections.
        let left = x0 + px(33.6);
        let right = x0 + r[2] - px(32.0);
        let section = |p: &mut dyn Painter, key: &str, cy: f32| {
            let y = y0 + px(cy);
            // ◢ open-section mark, just below the rule.
            let (tx, s) = (x0 + px(20.8), px(4.8));
            p.path(
                &[
                    PathSeg::M(tx + s, y + one),
                    PathSeg::L(tx + s, y + one + s),
                    PathSeg::L(tx, y + one + s),
                    PathSeg::Z,
                ],
                Some(ARROW),
                None,
            );
            let label = self.t(key);
            p.text(&label, left, (y - line / 2.0).round(), st, LABEL);
            let lw = self.width_of(&label, st);
            if left + lw + px(6.0) < right {
                p.fill_rect(
                    left + lw + px(6.0),
                    y,
                    right - left - lw - px(6.0),
                    one,
                    RULE,
                );
            }
        };
        let style = self.r16.style;
        let icon_name = NOTE_ICONS.get(style.icon).copied().unwrap_or("Comment");
        let colour = Rgba(style.colour[0], style.colour[1], style.colour[2], 255);
        let alpha = (f32::from(style.opacity) / 100.0 * 255.0).round() as u8;
        // Preview.
        section(p, "pdfReader16.preview", 52.0);
        let b = [x0 + px(35.2), y0 + px(68.0), px(245.6), px(73.6)];
        p.fill_rect(b[0], b[1], b[2], b[3], EDGE);
        p.fill_rect(
            b[0] + one,
            b[1] + one,
            b[2] - 2.0 * one,
            b[3] - 2.0 * one,
            PREVIEW_RING,
        );
        p.fill_rect(
            b[0] + 2.0 * one,
            b[1] + 2.0 * one,
            b[2] - 4.0 * one,
            b[3] - 4.0 * one,
            PREVIEW_FILL,
        );
        let s = px(19.2);
        note_glyph(
            p,
            icon_name,
            (b[0] + (b[2] - s) / 2.0).round(),
            (b[1] + (b[3] - s) / 2.0).round(),
            s,
            Some(Rgba(colour.0, colour.1, colour.2, alpha)),
            Rgba(64, 64, 64, alpha),
        );
        // Note Style.
        section(p, "pdfReader16.noteStyle", 163.2);
        for (i, name) in NOTE_ICONS.iter().enumerate() {
            let (cx, cy) = icon_centre(i);
            let (cx, cy) = (x0 + px(cx), y0 + px(cy));
            let s = px(17.6);
            // The reference marks no icon as chosen (the Preview shows it); hover only.
            if self.r16.hover == Some(Part::Icon(i)) {
                let bs = px(26.4);
                p.fill_rect(
                    (cx - bs / 2.0).round(),
                    (cy - bs / 2.0).round(),
                    bs,
                    bs,
                    HOVER,
                );
            }
            note_glyph(
                p,
                name,
                (cx - s / 2.0).round(),
                (cy - s / 2.0).round(),
                s,
                None,
                INK,
            );
        }
        // Current colour (a note filled with it, 23 px), the more-colours wheel, the swatches.
        let (sx, sy) = (x0 + px(49.6), y0 + px(312.8));
        let s = px(18.4);
        note_glyph(
            p,
            "Comment",
            (sx - s / 2.0).round(),
            (sy - s / 2.0).round(),
            s,
            Some(colour),
            INK,
        );
        let (wx, wy, wr) = (x0 + px(49.6), y0 + px(353.6), px(8.8));
        let wheel = [
            Rgba(255, 0, 0, 255),
            Rgba(255, 196, 0, 255),
            Rgba(90, 200, 60, 255),
            Rgba(0, 170, 230, 255),
            Rgba(60, 80, 230, 255),
            Rgba(200, 60, 200, 255),
        ];
        for (k, c) in wheel.iter().enumerate() {
            let a0 = k as f32 / 6.0 * std::f32::consts::TAU;
            let a1 = (k + 1) as f32 / 6.0 * std::f32::consts::TAU;
            p.path(
                &[
                    PathSeg::M(wx, wy),
                    PathSeg::L(wx + wr * a0.cos(), wy + wr * a0.sin()),
                    PathSeg::L(
                        wx + wr * f32::midpoint(a0, a1).cos(),
                        wy + wr * f32::midpoint(a0, a1).sin(),
                    ),
                    PathSeg::L(wx + wr * a1.cos(), wy + wr * a1.sin()),
                    PathSeg::Z,
                ],
                Some(*c),
                None,
            );
        }
        p.path(&circle(wx, wy, wr * 0.45), Some(WHITE), None);
        p.fill_rect(x0 + px(78.4), y0 + px(300.8), one, px(65.6), SWATCH_EDGE);
        for (i, c) in NOTE_COLOURS.iter().enumerate() {
            let (cx, cy) = colour_centre(i);
            let (cx, cy) = (x0 + px(cx), y0 + px(cy));
            let rr = px(9.6);
            if self.r16.hover == Some(Part::Colour(i)) {
                p.path(&circle(cx, cy, rr + px(2.4)), Some(PREVIEW_RING), None);
            }
            p.path(
                &circle(cx, cy, rr),
                Some(Rgba(c[0], c[1], c[2], 255)),
                Some((SWATCH_EDGE, one)),
            );
        }
        // Opacity.
        let oy = y0 + px(395.2);
        let ty = (oy - lline / 2.0).round() - px(1.6);
        p.text(
            &self.t("pdfReader16.opacity"),
            x0 + px(37.6),
            ty,
            lst,
            LABEL,
        );
        p.text("0", x0 + px(102.4), ty, lst, LABEL);
        p.text("100", x0 + px(291.2), ty, lst, LABEL);
        let sl = self.r16.slider;
        let accent = self.theme().accent_text;
        let th = px(3.2);
        p.path(
            &rounded(sl[0], oy - th / 2.0, sl[2], th, th / 2.0),
            Some(Rgba(204, 204, 204, 255)),
            None,
        );
        let kx = sl[0] + sl[2] * f32::from(style.opacity) / 100.0;
        p.path(
            &rounded(sl[0], oy - th / 2.0, kx - sl[0], th, th / 2.0),
            Some(accent),
            None,
        );
        p.path(
            &circle(kx, oy, px(7.2)),
            Some(WHITE),
            Some((KNOB_RING, one)),
        );
        p.path(&circle(kx, oy, px(4.0)), Some(accent), None);
        // Arrange: nothing selected, so its tools are dim.
        section(p, "pdfReader16.arrange", 436.8);
        let rows: [(&str, f32, &[f32]); 3] = [
            ("pdfReader16.pageCenter", 459.2, &[48.0, 118.4, 190.4]),
            (
                "pdfReader16.alignment",
                533.6,
                &[48.0, 94.4, 144.0, 193.6, 244.0, 294.4],
            ),
            ("pdfReader16.distribute", 599.2, &[48.0, 116.8]),
        ];
        for (n, (key, ty, xs)) in rows.iter().enumerate() {
            p.text(
                &self.t(key),
                left,
                (y0 + px(*ty) - lline / 2.0).round(),
                lst,
                LABEL,
            );
            let iy = y0 + px(*ty + [36.8, 27.2, 28.8][n.min(2)]);
            for (j, ix) in xs.iter().enumerate() {
                arrange_glyph(p, n, j, x0 + px(*ix), iy, px(16.0), one);
            }
        }
        p.pop_clip();
    }

    /// The auto-scroll anchor: a ring with four arrow heads and a centre dot (ours).
    fn paint_anchor16(&self, p: &mut dyn Painter, x: f32, y: f32) {
        let ink = self.theme().accent_text;
        let r = self.px16(11.2);
        let one = self.scale.max(1.0).round();
        p.path(&circle(x, y, r), Some(WHITE), Some((ink, one * 1.5)));
        let a = self.px16(3.2);
        let o = r - self.px16(2.0);
        for (dx, dy) in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
            let (tx, ty) = (x + dx * o, y + dy * o);
            // Arrow head pointing outwards.
            let (bx, by) = (x + dx * (o - a), y + dy * (o - a));
            let (px_, py_) = (-dy * a, dx * a);
            p.path(
                &[
                    PathSeg::M(tx, ty),
                    PathSeg::L(bx + px_, by + py_),
                    PathSeg::L(bx - px_, by - py_),
                    PathSeg::Z,
                ],
                Some(ink),
                None,
            );
        }
        p.path(&circle(x, y, self.px16(2.0)), Some(ink), None);
    }

    /// Measure the pane's labels (widths for the section rules).
    pub(crate) fn measure16(&mut self, m: &dyn TextMeasure) {
        if !self.r16.pane {
            return;
        }
        let st = self.style(13.0, false);
        for key in [
            "pdfReader16.preview",
            "pdfReader16.noteStyle",
            "pdfReader16.arrange",
        ] {
            let s = self.t(key);
            self.measure(m, &s, st);
        }
    }
}

/// The page text of the view for UI Automation (see [`ReaderWindow::access_text16`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AccessText {
    /// The text, pages joined by line breaks.
    pub text: String,
    /// For each character of `text`, its box in window pixels (x, y, w, h).
    pub boxes: Vec<Option<[f32; 4]>>,
    /// Selected characters (start, end) of `text`.
    pub selection: Option<(usize, usize)>,
    /// Pages shown: (page index, offset of its first character).
    pub pages: Vec<(usize, usize)>,
    /// For each character of `text`, its page and character index there (`None` for an added
    /// space or line break): what [`ReaderWindow::select_text16`] takes.
    pub chars: Vec<Option<(usize, usize)>>,
}

/// Note Style grid: six columns 49.6 apart, rows 36.8 apart (logical pixels in the pane).
fn icon_centre(i: usize) -> (f32, f32) {
    let (col, row) = (i % 6, i / 6);
    (49.6 + 49.6 * col as f32, 196.0 + 36.8 * row as f32)
}

/// Colour swatches: two rows of five, 48 apart, right of the current-colour box.
fn colour_centre(i: usize) -> (f32, f32) {
    let (col, row) = (i % 5, i / 5);
    (108.0 + 48.0 * col as f32, 312.8 + 40.8 * row as f32)
}

/// The characters of the line of character `c`.
fn line_span(layer: &bo_pdfread::TextLayer, c: usize) -> (usize, usize) {
    let chars = layer.chars();
    let same_line = |a: usize, b: usize| {
        let (p, q) = (&chars[a], &chars[b]);
        (p.origin[1] - q.origin[1]).abs() < p.size.max(q.size).max(1.0) * 0.5
            && q.rect[0] >= p.rect[0] - p.size.max(1.0) * 1.5
    };
    let mut a = c;
    while a > 0 && same_line(a - 1, a) {
        a -= 1;
    }
    let mut b = c;
    while b + 1 < chars.len() && same_line(b, b + 1) {
        b += 1;
    }
    (a, b)
}

/// Draw note icon `name` in a `size` square: the outline of the appearance drawn for the PDF
/// ([`bo_pdfread::note_icon_content`]), filled with `fill` when given.
pub(crate) fn note_glyph(
    p: &mut dyn Painter,
    name: &str,
    x: f32,
    y: f32,
    size: f32,
    fill: Option<Rgba>,
    ink: Rgba,
) {
    let content = bo_pdfread::note_icon_content(name, [0.0, 0.0, 20.0, 20.0], [1.0; 3], 1.0);
    let k = size / 20.0;
    let width = (size / 14.0).max(1.0);
    let at = |u: f32, v: f32| (x + u * k, y + (20.0 - v) * k);
    let mut nums: Vec<f32> = Vec::new();
    let mut segs: Vec<PathSeg> = Vec::new();
    let mut dark_fill = false;
    let mut line_w = width;
    for tok in content.split_whitespace() {
        if let Ok(v) = tok.parse::<f32>() {
            nums.push(v);
            continue;
        }
        match tok {
            "m" if nums.len() >= 2 => {
                let (u, v) = at(nums[nums.len() - 2], nums[nums.len() - 1]);
                segs.push(PathSeg::M(u, v));
            }
            "l" if nums.len() >= 2 => {
                let (u, v) = at(nums[nums.len() - 2], nums[nums.len() - 1]);
                segs.push(PathSeg::L(u, v));
            }
            "c" if nums.len() >= 6 => {
                let n = &nums[nums.len() - 6..];
                let (a, b) = at(n[0], n[1]);
                let (c, d) = at(n[2], n[3]);
                let (e, f) = at(n[4], n[5]);
                segs.push(PathSeg::C(a, b, c, d, e, f));
            }
            "h" => segs.push(PathSeg::Z),
            "w" if !nums.is_empty() => {
                line_w = (nums[nums.len() - 1] / 0.8 * width).max(1.0);
            }
            "rg" => dark_fill = nums.first().is_some_and(|v| *v < 0.5) && nums.len() >= 3,
            "B" | "S" | "f" => {
                let f = if tok == "S" {
                    None
                } else if dark_fill {
                    Some(ink)
                } else {
                    fill
                };
                let s = if tok == "f" {
                    None
                } else {
                    Some((ink, line_w))
                };
                if f.is_some() || s.is_some() {
                    p.path(&segs, f, s);
                }
                segs.clear();
            }
            _ => {}
        }
        nums.clear();
    }
}

/// The Arrange section's tools (our own drawings: bars against a guide line), dim.
fn arrange_glyph(p: &mut dyn Painter, row: usize, j: usize, cx: f32, cy: f32, s: f32, one: f32) {
    let h = s / 2.0;
    let (x, y) = ((cx - h).round(), (cy - h).round());
    let stroke = Some((DIM, one));
    let bar = |p: &mut dyn Painter, bx: f32, by: f32, bw: f32, bh: f32| {
        p.path(
            &rounded(
                bx.round() + 0.5,
                by.round() + 0.5,
                bw.round(),
                bh.round(),
                0.0,
            ),
            None,
            stroke,
        );
    };
    let guide_v = |p: &mut dyn Painter, gx: f32| p.fill_rect(gx.round(), y, one, s, DIM);
    let guide_h = |p: &mut dyn Painter, gy: f32| p.fill_rect(x, gy.round(), s, one, DIM);
    match (row, j) {
        // Page centre: horizontally, vertically, both.
        (0, 0) => {
            guide_h(p, cy);
            bar(p, x + s * 0.2, y + s * 0.15, s * 0.6, s * 0.7);
        }
        (0, 1) => {
            guide_v(p, cx);
            bar(p, x + s * 0.15, y + s * 0.2, s * 0.7, s * 0.6);
        }
        (0, _) => {
            p.path(&rounded(x + 0.5, y + 0.5, s, s, 0.0), None, stroke);
            bar(p, x + s * 0.3, y + s * 0.3, s * 0.4, s * 0.4);
        }
        // Alignment: centre (two), left, right, top, bottom.
        (1, 0) => {
            guide_h(p, cy);
            bar(p, x + s * 0.1, y + s * 0.1, s * 0.8, s * 0.3);
            bar(p, x + s * 0.25, y + s * 0.6, s * 0.5, s * 0.3);
        }
        (1, 1) => {
            guide_v(p, cx);
            bar(p, x + s * 0.1, y + s * 0.1, s * 0.3, s * 0.8);
            bar(p, x + s * 0.6, y + s * 0.25, s * 0.3, s * 0.5);
        }
        (1, 2) => {
            guide_v(p, x);
            bar(p, x + s * 0.15, y + s * 0.1, s * 0.75, s * 0.3);
            bar(p, x + s * 0.15, y + s * 0.6, s * 0.5, s * 0.3);
        }
        (1, 3) => {
            guide_v(p, x + s - one);
            bar(p, x + s * 0.1, y + s * 0.1, s * 0.75, s * 0.3);
            bar(p, x + s * 0.35, y + s * 0.6, s * 0.5, s * 0.3);
        }
        (1, 4) => {
            guide_h(p, y);
            bar(p, x + s * 0.1, y + s * 0.15, s * 0.3, s * 0.75);
            bar(p, x + s * 0.6, y + s * 0.15, s * 0.3, s * 0.5);
        }
        (1, _) => {
            guide_h(p, y + s - one);
            bar(p, x + s * 0.1, y + s * 0.1, s * 0.3, s * 0.75);
            bar(p, x + s * 0.6, y + s * 0.35, s * 0.3, s * 0.5);
        }
        // Distribute: vertically, horizontally.
        (_, 0) => {
            guide_h(p, y);
            guide_h(p, y + s - one);
            bar(p, x + s * 0.2, y + s * 0.3, s * 0.6, s * 0.4);
        }
        _ => {
            guide_v(p, x);
            guide_v(p, x + s - one);
            bar(p, x + s * 0.3, y + s * 0.2, s * 0.4, s * 0.6);
        }
    }
}

#[cfg(test)]
#[path = "reader_r16_tests.rs"]
mod tests;
