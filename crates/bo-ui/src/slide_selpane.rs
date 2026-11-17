//! BlueSlide's Selection Pane and Alt Text pane (N6, round 5), docked right of the editing
//! area like the reference presentation app's task panes (measured at 125 %: light grey 230
//! under a 191 divider, title 16 logical px, framed white buttons 35 device px tall).
//!
//! - Selection: Show All / Hide All, bring forward / send backward chevrons, one row per shape
//!   of the slide from the front to the back (group members indented), an eye at the right
//!   of each row to hide or show it, the selected shapes highlighted (grey 198 framed 128);
//!   a double click renames the shape in place.
//! - Alt Text: what to describe, a multi-line text box for the alternative text and "Mark as
//!   decorative".
use crate::{Key, Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};

const PANE_BG: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
const SPLIT: Rgba = Rgba(0xbf, 0xbf, 0xbf, 255);
const BUTTON_EDGE: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const WHITE: Rgba = Rgba(0xff, 0xff, 0xff, 255);
const ROW_SEL: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const ROW_EDGE: Rgba = Rgba(0x80, 0x80, 0x80, 255);
const DIM: Rgba = Rgba(0x9a, 0x9a, 0x9a, 255);
const BOX_EDGE: Rgba = Rgba(0xab, 0xab, 0xab, 255);
const BOX_FOCUS: Rgba = Rgba(0xb8, 0x45, 0x2b, 255);

/// Which task pane is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SideKind {
    /// None.
    #[default]
    None,
    /// The Selection Pane.
    Selection,
    /// The Alt Text pane.
    AltText,
    /// The Spelling pane (Review > Spelling, F7).
    Spelling,
    /// The Accessibility pane (Review > Check Accessibility).
    Accessibility,
    /// The Thesaurus pane (Review > Thesaurus, Shift+F7).
    Thesaurus,
}

/// A shape listed by the Selection Pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelRow {
    /// Shape id.
    pub id: u32,
    /// Shape name.
    pub name: String,
    /// Hidden (`cNvPr@hidden`).
    pub hidden: bool,
    /// Nesting depth (group members 1, …).
    pub depth: usize,
    /// Selected on the slide.
    pub selected: bool,
}

/// What a press in a pane does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SideHit {
    /// Close the pane.
    Close,
    /// Show every shape of the slide.
    ShowAll,
    /// Hide every shape of the slide.
    HideAll,
    /// Bring the selected shape forward.
    Forward,
    /// Send it backward.
    Backward,
    /// Select shape `id` (a row).
    Select(u32),
    /// Hide or show shape `id` (its eye).
    Toggle(u32),
    /// The Alt Text box (focus it).
    AltBox,
    /// "Mark as decorative".
    Decorative,
    /// Spelling: skip this occurrence.
    IgnoreOnce,
    /// Spelling: skip the word everywhere.
    IgnoreAll,
    /// Spelling: add the word to the dictionary.
    AddWord,
    /// Spelling: replace by the chosen correction.
    Change,
    /// Spelling: replace every occurrence.
    ChangeAll,
    /// Spelling: choose correction `n`.
    Suggestion(usize),
    /// The pane's options button (the chevron left of the close button).
    PaneMenu,
    /// Spelling: read the chosen correction aloud.
    Speak,
    /// Accessibility: go to shape `id` (0: the slide itself) of slide `n`.
    Goto(usize, u32),
    /// Thesaurus: back to the word looked up before.
    ThesBack,
    /// Thesaurus: look a synonym up (a double press puts it in the text).
    ThesLookup(String),
    /// Thesaurus: fold / unfold meaning `n`.
    ThesToggle(usize),
}

/// An issue listed by the Accessibility pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A11yRow {
    /// Slide index.
    pub slide: usize,
    /// Shape id (0: the slide).
    pub id: u32,
    /// What is wrong, e.g. "Missing alternative text".
    pub kind: String,
    /// Where: "Picture 3 (Slide 2)".
    pub what: String,
}

/// What the Spelling pane shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpellShown {
    /// The flagged text (`None`: the check is complete).
    pub word: Option<String>,
    /// A grammar or style issue rather than a misspelt word.
    pub grammar: bool,
    /// Corrections, best first.
    pub suggestions: Vec<String>,
    /// The chosen correction.
    pub chosen: usize,
    /// Meanings of the chosen correction: definition, synonyms (round 17, English thesaurus).
    pub meanings: Vec<(String, String)>,
    /// Name of the text's proofing language, shown at the pane's foot.
    pub lang: String,
}

/// What typing in a pane asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SideEdit {
    /// The text changed (redraw).
    Changed,
    /// Rename shape `id` to the text.
    Rename(u32, String),
    /// Set the alternative text.
    AltText(String),
    /// Editing given up.
    Cancel,
}

#[derive(Debug, Clone)]
enum Op {
    Rect([f32; 4], Rgba),
    Text(String, f32, f32, TextStyle, Rgba),
    Path(Vec<PathSeg>, Option<Rgba>, Option<(Rgba, f32)>),
    Check([f32; 4], bool, bool, Rgba),
}

/// The pane's state and last layout.
#[derive(Debug, Default)]
pub struct SidePane {
    /// The pane shown.
    pub kind: SideKind,
    /// Renaming a row: shape id and the new name typed so far.
    pub rename: Option<(u32, String)>,
    /// The Alt Text box: the text typed (`None` when the box has no focus).
    pub alt_edit: Option<String>,
    rect: [f32; 4],
    scale: f32,
    pub(crate) hits: Vec<([f32; 4], SideHit)>,
    ops: Vec<Op>,
    /// The Thesaurus pane's word and history.
    pub thes: thespane::ThesState,
}

#[path = "slide_thespane.rs"]
pub(super) mod thespane;

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

/// An eye (shown) or a closed eye (hidden) centred on (`cx`, `cy`), own drawing.
fn eye(cx: f32, cy: f32, r: f32, open: bool) -> Vec<PathSeg> {
    let h = r * 0.55;
    let mut v = vec![
        PathSeg::M(cx - r, cy),
        PathSeg::Q(cx, cy - 2.0 * h, cx + r, cy),
    ];
    if open {
        v.extend([
            PathSeg::Q(cx, cy + 2.0 * h, cx - r, cy),
            PathSeg::M(cx + h * 0.6, cy),
            PathSeg::C(
                cx + h * 0.6,
                cy + h * 0.8,
                cx - h * 0.6,
                cy + h * 0.8,
                cx - h * 0.6,
                cy,
            ),
            PathSeg::C(
                cx - h * 0.6,
                cy - h * 0.8,
                cx + h * 0.6,
                cy - h * 0.8,
                cx + h * 0.6,
                cy,
            ),
        ]);
    } else {
        // Closed: the lower lid only, with lashes.
        v = vec![
            PathSeg::M(cx - r, cy),
            PathSeg::Q(cx, cy + 2.0 * h, cx + r, cy),
        ];
        for k in [-0.5f32, 0.0, 0.5] {
            let x = cx + k * r;
            v.push(PathSeg::M(x, cy + h * 0.95));
            v.push(PathSeg::L(x + k * h * 0.6, cy + h * 1.6));
        }
    }
    v
}

/// `text` broken into lines no wider than `width`.
fn wrap(text: &str, width: f32, m: &dyn TextMeasure, style: TextStyle) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let trial = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && m.measure(&trial, style).0 > width {
                lines.push(std::mem::take(&mut line));
                word.clone_into(&mut line);
            } else {
                line = trial;
            }
        }
        lines.push(line);
    }
    lines
}

impl SidePane {
    /// Whether a pane is shown.
    pub fn shown(&self) -> bool {
        self.kind != SideKind::None
    }

    /// Show `kind` (or close it when it is the one shown).
    pub fn toggle(&mut self, kind: SideKind) {
        self.kind = if self.kind == kind {
            SideKind::None
        } else {
            kind
        };
        self.rename = None;
        self.alt_edit = None;
    }

    /// Close the pane.
    pub fn close(&mut self) {
        self.kind = SideKind::None;
        self.rename = None;
        self.alt_edit = None;
    }

    /// Place the pane.
    pub fn layout(&mut self, rect: [f32; 4], scale: f32) {
        self.rect = rect;
        self.scale = scale;
    }

    /// Whether a window position is inside the pane.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.shown() && inside(self.rect, x, y)
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// What a press at a window position hits.
    pub fn press(&self, x: f32, y: f32) -> Option<SideHit> {
        self.hits
            .iter()
            .rev()
            .find(|(r, _)| inside(*r, x, y))
            .map(|(_, h)| h.clone())
    }

    /// Start renaming row `id` (a double click).
    pub fn start_rename(&mut self, id: u32, name: &str) {
        self.rename = Some((id, name.to_owned()));
    }

    /// Typed text goes to the name being edited or the Alt Text box.
    pub fn text(&mut self, t: &str) -> Option<SideEdit> {
        let ((Some((_, s)), _) | (None, Some(s))) = (&mut self.rename, &mut self.alt_edit) else {
            return None;
        };
        s.extend(t.chars().filter(|c| !c.is_control() || *c == '\n'));
        Some(SideEdit::Changed)
    }

    /// Keys while a name or the Alt Text box is edited.
    pub fn key(&mut self, k: Key) -> Option<SideEdit> {
        if let Some((id, s)) = &mut self.rename {
            return Some(match k {
                Key::Enter => {
                    let (id, s) = (*id, s.clone());
                    self.rename = None;
                    SideEdit::Rename(id, s)
                }
                Key::Escape => {
                    self.rename = None;
                    SideEdit::Cancel
                }
                Key::Backspace => {
                    s.pop();
                    SideEdit::Changed
                }
                _ => return None,
            });
        }
        let s = self.alt_edit.as_mut()?;
        Some(match k {
            Key::Enter => {
                s.push('\n');
                SideEdit::Changed
            }
            Key::Backspace => {
                s.pop();
                SideEdit::Changed
            }
            Key::Escape | Key::Tab => {
                let s = self.alt_edit.take().unwrap_or_default();
                SideEdit::AltText(s)
            }
            _ => return None,
        })
    }

    /// Draw the pane as last recorded.
    pub fn paint(&self, p: &mut dyn Painter) {
        if !self.shown() {
            return;
        }
        let [x, y, w, h] = self.rect;
        p.push_clip(x, y, w, h);
        for op in &self.ops {
            match op {
                Op::Rect([x, y, w, h], c) => p.fill_rect(*x, *y, *w, *h, *c),
                Op::Text(t, x, y, s, c) => p.text(t, *x, *y, *s, *c),
                Op::Path(segs, f, st) => p.path(segs, *f, *st),
                Op::Check(r, on, enabled, ink) => {
                    crate::spin_box::paint_check(p, *r, *on, *enabled, *ink);
                }
            }
        }
        p.pop_clip();
    }

    fn header(
        &self,
        ops: &mut Vec<Op>,
        hits: &mut Vec<([f32; 4], SideHit)>,
        title: &str,
        ink: Rgba,
    ) {
        let [x, y, w, h] = self.rect;
        ops.push(Op::Rect([x, y, w, h], PANE_BG));
        ops.push(Op::Rect([x + 1.0, y, 1.0, h], SPLIT));
        let style = TextStyle {
            size: self.px(20.0),
            bold: false,
        };
        ops.push(Op::Text(
            title.to_owned(),
            x + self.px(12.8),
            y + self.px(9.6),
            style,
            ink,
        ));
        let cs = self.px(22.0);
        let close = [x + w - self.px(12.0) - cs, y + self.px(12.0), cs, cs];
        let k = self.px(5.0);
        let (cx, cy) = (close[0] + cs / 2.0, close[1] + cs / 2.0);
        ops.push(Op::Path(
            vec![
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((ink, self.scale.max(1.0).round())),
        ));
        hits.push((close, SideHit::Close));
    }

    fn button(
        ops: &mut Vec<Op>,
        r: [f32; 4],
        label: &str,
        text: &dyn TextMeasure,
        style: TextStyle,
        ink: Rgba,
    ) {
        ops.push(Op::Rect(r, BUTTON_EDGE));
        ops.push(Op::Rect(
            [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
            WHITE,
        ));
        // "&" marks the access key, drawn underlined.
        let key = label.find('&');
        let shown = label.replacen('&', "", 1);
        let (tw, th) = text.measure(&shown, style);
        let (tx, ty) = (
            (r[0] + (r[2] - tw) / 2.0).round(),
            (r[1] + (r[3] - th) / 2.0).round(),
        );
        if let Some(k) = key {
            let ch = shown[k..].chars().next().map_or(0, char::len_utf8);
            let x0 = tx + text.measure(&shown[..k], style).0;
            let cw = text.measure(&shown[k..k + ch], style).0;
            let base = (ty + th * 0.86).round();
            ops.push(Op::Rect([x0, base, cw.max(1.0), 1.0], ink));
        }
        ops.push(Op::Text(shown, tx, ty, style, ink));
    }

    /// Lay out the Selection Pane for `rows` (front to back); `labels`: title, Show All,
    /// Hide All.
    pub fn record_selection(
        &mut self,
        text: &dyn TextMeasure,
        rows: &[SelRow],
        labels: &[String; 3],
        ink: Rgba,
    ) {
        let mut ops = Vec::new();
        let mut hits = Vec::new();
        self.header(&mut ops, &mut hits, &labels[0], ink);
        let [x, y, w, h] = self.rect;
        let body = TextStyle {
            size: self.px(12.8),
            bold: false,
        };
        // Reference at 125 %: buttons 15 px from the divider, 60 px below the pane's top,
        // 35 px tall; Show All 91 px wide, Hide All 84, 5 px apart; chevrons 36 px wide.
        let by = y + self.px(48.0);
        let bh = self.px(28.0);
        let mut bx = x + self.px(12.8);
        for (label, hit) in [
            (&labels[1], SideHit::ShowAll),
            (&labels[2], SideHit::HideAll),
        ] {
            let bw = text.measure(label, body).0 + self.px(16.0);
            let r = [bx, by, bw.round(), bh];
            Self::button(&mut ops, r, label, text, body, ink);
            hits.push((r, hit));
            bx += r[2] + self.px(4.0);
        }
        let aw = self.px(28.8);
        let any = rows.iter().any(|r| r.selected);
        for (k, (hit, up)) in [(SideHit::Forward, true), (SideHit::Backward, false)]
            .into_iter()
            .enumerate()
        {
            let r = [
                x + w - self.px(11.2) - (2 - k) as f32 * aw - (1 - k) as f32 * self.px(3.2),
                by,
                aw,
                bh,
            ];
            ops.push(Op::Rect(r, WHITE));
            let (cx, cy) = (r[0] + aw / 2.0, r[1] + bh / 2.0);
            let s = self.px(4.8);
            let d = if up { -1.0 } else { 1.0 };
            ops.push(Op::Path(
                vec![
                    PathSeg::M(cx - s, cy - d * s / 2.0),
                    PathSeg::L(cx, cy + d * s / 2.0),
                    PathSeg::L(cx + s, cy - d * s / 2.0),
                ],
                None,
                Some((if any { ink } else { DIM }, self.scale.max(1.0).round())),
            ));
            hits.push((r, hit));
        }
        // Rows: 28 logical px tall from 89 px below the pane's top, 27 px from its left edge.
        let row_h = self.px(28.0);
        let mut ry = y + self.px(89.6);
        let left = x + self.px(27.2);
        let right = x + w - self.px(12.8);
        let line = text.measure("Ag", body).1;
        for row in rows {
            if ry > y + h {
                break;
            }
            let r = [left, ry, right - left, row_h];
            if row.selected {
                ops.push(Op::Rect(r, ROW_EDGE));
                ops.push(Op::Rect(
                    [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
                    ROW_SEL,
                ));
            }
            let indent = self.px(16.0) * row.depth as f32;
            let tx = left + self.px(4.8) + indent;
            let color = if row.hidden { DIM } else { ink };
            match &self.rename {
                Some((id, s)) if *id == row.id => {
                    let br = [
                        tx - self.px(2.0),
                        ry + self.px(2.0),
                        right - tx - self.px(40.0),
                        row_h - self.px(4.0),
                    ];
                    ops.push(Op::Rect(br, BOX_FOCUS));
                    ops.push(Op::Rect(
                        [br[0] + 1.0, br[1] + 1.0, br[2] - 2.0, br[3] - 2.0],
                        WHITE,
                    ));
                    ops.push(Op::Text(
                        s.clone(),
                        tx,
                        ry + (row_h - line) / 2.0,
                        body,
                        ink,
                    ));
                    let cw = text.measure(s, body).0;
                    ops.push(Op::Rect(
                        [
                            tx + cw + 1.0,
                            br[1] + self.px(3.0),
                            1.0,
                            br[3] - self.px(6.0),
                        ],
                        ink,
                    ));
                }
                _ => ops.push(Op::Text(
                    row.name.clone(),
                    tx,
                    ry + (row_h - line) / 2.0,
                    body,
                    color,
                )),
            }
            let eye_r = [right - self.px(36.0), ry, self.px(32.0), row_h];
            ops.push(Op::Path(
                eye(
                    eye_r[0] + eye_r[2] / 2.0,
                    ry + row_h / 2.0,
                    self.px(7.2),
                    !row.hidden,
                ),
                None,
                Some((ink, self.scale.max(1.0).round())),
            ));
            hits.push((r, SideHit::Select(row.id)));
            hits.push((eye_r, SideHit::Toggle(row.id)));
            ry += row_h;
        }
        self.ops = ops;
        self.hits = hits;
    }

    /// Lay out the Alt Text pane for the selected shape's text (`None`: nothing selected);
    /// `labels`: title, what to describe, hint, Mark as decorative.
    pub fn record_alt(
        &mut self,
        text: &dyn TextMeasure,
        alt: Option<(&str, bool)>,
        labels: &[String; 4],
        ink: Rgba,
    ) {
        let mut ops = Vec::new();
        let mut hits = Vec::new();
        self.header(&mut ops, &mut hits, &labels[0], ink);
        let [x, y, w, _] = self.rect;
        let body = TextStyle {
            size: self.px(12.8),
            bold: false,
        };
        let left = x + self.px(12.0);
        let width = w - self.px(24.0);
        let line = text.measure("Ag", body).1.max(self.px(16.0));
        let grey = Rgba(0x44, 0x44, 0x44, 255);
        let mut ty = y + self.px(48.0);
        for l in wrap(&labels[1], width, text, body) {
            ops.push(Op::Text(l, left, ty, body, grey));
            ty += line;
        }
        ty += self.px(6.0);
        for l in wrap(&labels[2], width, text, body) {
            ops.push(Op::Text(l, left, ty, body, grey));
            ty += line;
        }
        ty += self.px(16.0);
        let (value, decorative) = alt.unwrap_or(("", false));
        let enabled = alt.is_some() && !decorative;
        let shown = self.alt_edit.as_deref().unwrap_or(value);
        let bh = self.px(188.0);
        let r = [left, ty, width, bh];
        let edge = if self.alt_edit.is_some() {
            BOX_FOCUS
        } else {
            BOX_EDGE
        };
        ops.push(Op::Rect(r, edge));
        ops.push(Op::Rect(
            [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
            if enabled { WHITE } else { PANE_BG },
        ));
        let mut ly = ty + self.px(6.0);
        let lines = wrap(shown, width - self.px(12.0), text, body);
        let last = lines.last().cloned().unwrap_or_default();
        for l in &lines {
            if ly + line > ty + bh {
                break;
            }
            ops.push(Op::Text(l.clone(), left + self.px(6.0), ly, body, ink));
            ly += line;
        }
        if self.alt_edit.is_some() {
            let cw = text.measure(&last, body).0;
            ops.push(Op::Rect(
                [
                    left + self.px(6.0) + cw + 1.0,
                    ly - line + self.px(1.0),
                    1.0,
                    line - self.px(2.0),
                ],
                ink,
            ));
        }
        if enabled {
            hits.push((r, SideHit::AltBox));
        }
        // "Mark as decorative": a check box under the text box.
        let cy = ty + bh + self.px(28.0);
        let b = self.px(14.4);
        let cr = [left, cy, b, b];
        ops.push(Op::Check(cr, decorative, alt.is_some(), ink));

        let (_, th) = text.measure(&labels[3], body);
        ops.push(Op::Text(
            labels[3].clone(),
            left + b + self.px(8.0),
            (cy + (b - th) / 2.0).round(),
            body,
            if alt.is_some() { ink } else { DIM },
        ));
        if alt.is_some() {
            let tw = text.measure(&labels[3], body).0;
            hits.push((
                [
                    left,
                    cy - self.px(4.0),
                    b + self.px(8.0) + tw,
                    b + self.px(8.0),
                ],
                SideHit::Decorative,
            ));
        }
        self.ops = ops;
        self.hits = hits;
    }

    /// Lay out the Spelling pane; `labels`: title, Ignore Once, Ignore All, Add, Change,
    /// Change All, "complete" message, no suggestions.
    pub fn record_spelling(
        &mut self,
        text: &dyn TextMeasure,
        shown: &SpellShown,
        labels: &[String; 8],
        ink: Rgba,
    ) {
        let mut ops = Vec::new();
        let mut hits = Vec::new();
        self.header(&mut ops, &mut hits, &labels[0], ink);
        let [x, y, w, _] = self.rect;
        // Pane options (a chevron left of the close button, as on the reference).
        let cs = self.px(22.0);
        let opt = [
            x + w - self.px(12.0) - 2.0 * cs - self.px(8.0),
            y + self.px(12.0),
            cs,
            cs,
        ];
        let (cx, cy, k) = (opt[0] + cs / 2.0, opt[1] + cs / 2.0, self.px(5.0));
        ops.push(Op::Path(
            vec![
                PathSeg::M(cx - k, cy - k / 2.0),
                PathSeg::L(cx, cy + k / 2.0),
                PathSeg::L(cx + k, cy - k / 2.0),
            ],
            None,
            Some((ink, self.scale.max(1.0).round())),
        ));
        hits.push((opt, SideHit::PaneMenu));
        let body = TextStyle {
            size: self.px(12.8),
            bold: false,
        };
        let left = x + self.px(12.0);
        let width = w - self.px(24.0);
        let line = text.measure("Ag", body).1.max(self.px(16.0));
        let Some(word) = &shown.word else {
            let mut ty = y + self.px(60.0);
            for l in wrap(&labels[6], width, text, body) {
                ops.push(Op::Text(l, left, ty, body, ink));
                ty += line;
            }
            self.ops = ops;
            self.hits = hits;
            return;
        };
        // Measured on the reference at 125 % (device px from the pane's top): the flagged
        // word bold 20 px at 61, buttons 35 px tall at 92 (15 px in, 14 apart, 10 px of
        // padding), the corrections list at 144 (202 px tall, 148 frame, the chosen row 198
        // framed 148, 40 px rows), Change / Change All 10 px under it.
        let big = TextStyle {
            size: self.px(16.0),
            bold: true,
        };
        ops.push(Op::Text(word.clone(), left, y + self.px(48.8), big, ink));
        let bh = self.px(28.0);
        let gap = self.px(11.2);
        let pad = self.px(8.0);
        let mut bx = left;
        let ty = y + self.px(73.6);
        let buttons: &[(usize, SideHit)] = if shown.grammar {
            &[(1, SideHit::IgnoreOnce)]
        } else {
            &[
                (1, SideHit::IgnoreOnce),
                (2, SideHit::IgnoreAll),
                (3, SideHit::AddWord),
            ]
        };
        for (i, hit) in buttons {
            let bw = (text.measure(&labels[*i].replacen('&', "", 1), body).0 + 2.0 * pad).round();
            let r = [bx, ty, bw, bh];
            Self::button(&mut ops, r, &labels[*i], text, body, ink);
            hits.push((r, hit.clone()));
            bx += bw + gap;
        }
        // Corrections list.
        let ty = y + self.px(115.2);
        let row = self.px(32.0);
        let lh = self.px(161.6);
        let lr = [left, ty, width, lh];
        ops.push(Op::Rect(lr, BUTTON_EDGE));
        ops.push(Op::Rect(
            [lr[0] + 1.0, lr[1] + 1.0, lr[2] - 2.0, lr[3] - 2.0],
            WHITE,
        ));
        if shown.suggestions.is_empty() {
            ops.push(Op::Text(
                labels[7].clone(),
                left + self.px(9.6),
                ty + (row - line) / 2.0,
                body,
                DIM,
            ));
        }
        let fits = ((lh - 2.0) / row).floor().max(1.0) as usize;
        for (i, sug) in shown.suggestions.iter().take(fits).enumerate() {
            let ry = ty + 1.0 + row * i as f32;
            let rr = [left + 1.0, ry, width - 2.0, row];
            if i == shown.chosen {
                ops.push(Op::Rect(rr, BUTTON_EDGE));
                ops.push(Op::Rect(
                    [rr[0] + 1.0, rr[1] + 1.0, rr[2] - 2.0, rr[3] - 2.0],
                    ROW_SEL,
                ));
            }
            ops.push(Op::Text(
                sug.clone(),
                left + self.px(9.6),
                ry + ((row - line) / 2.0).round(),
                body,
                ink,
            ));
            hits.push((rr, SideHit::Suggestion(i)));
        }
        // Change / Change All.
        let enabled = !shown.suggestions.is_empty();
        let ty = ty + lh + self.px(8.0);
        let mut bx = left;
        let changes: &[(usize, SideHit)] = if shown.grammar {
            &[(4, SideHit::Change)]
        } else {
            &[(4, SideHit::Change), (5, SideHit::ChangeAll)]
        };
        for (i, hit) in changes {
            let bw = (text.measure(&labels[*i].replacen('&', "", 1), body).0 + 2.0 * pad).round();
            let r = [bx, ty, bw, bh];
            Self::button(
                &mut ops,
                r,
                &labels[*i],
                text,
                body,
                if enabled { ink } else { DIM },
            );
            if enabled {
                hits.push((r, hit.clone()));
            }
            bx += bw + gap;
        }
        // A rule under the buttons, then the chosen correction in bold with a speaker
        // button that reads it aloud (the reference adds its meaning; not shown here).
        let rule_y = ty + bh + self.px(16.0);
        ops.push(Op::Rect([left, rule_y, width, 1.0], ROW_SEL));
        if let Some(sug) = shown.suggestions.get(shown.chosen) {
            let big = TextStyle {
                size: self.px(16.0),
                bold: true,
            };
            let wy = rule_y + self.px(18.4);
            ops.push(Op::Text(sug.clone(), left, wy, big, ink));
            let (sw, sh) = text.measure(sug, big);
            let s = self.px(18.0);
            let sx = left + sw + self.px(10.0);
            let sy = wy + ((sh - s) / 2.0).round();
            let u = s / 24.0;
            let seg = |pts: &[(f32, f32)]| -> Vec<PathSeg> {
                pts.iter()
                    .enumerate()
                    .map(|(i, (a, b))| {
                        let (px, py) = (sx + a * u, sy + b * u);
                        if i == 0 {
                            PathSeg::M(px, py)
                        } else {
                            PathSeg::L(px, py)
                        }
                    })
                    .collect()
            };
            let mut speaker = seg(&[
                (3.0, 9.0),
                (7.0, 9.0),
                (12.0, 5.0),
                (12.0, 19.0),
                (7.0, 15.0),
                (3.0, 15.0),
                (3.0, 9.0),
            ]);
            speaker.extend(seg(&[(15.0, 9.0), (16.5, 12.0), (15.0, 15.0)]));
            speaker.extend(seg(&[(18.0, 6.5), (20.5, 12.0), (18.0, 17.5)]));
            ops.push(Op::Path(
                speaker,
                None,
                Some((ink, self.scale.max(1.0).round())),
            ));
            hits.push(([sx - 2.0, sy - 2.0, s + 4.0, s + 4.0], SideHit::Speak));
            // Its meanings (numbered definitions, synonyms under each), as the reference
            // lists them under the word.
            let mut my = wy + sh + self.px(10.0);
            let [_, py, _, ph] = self.rect;
            let foot = py + ph - self.px(40.0);
            for (n, (gloss, syn)) in shown.meanings.iter().enumerate() {
                for (k, l) in wrap(&format!("{}. {gloss}", n + 1), width, text, body)
                    .into_iter()
                    .enumerate()
                {
                    if my + line > foot {
                        break;
                    }
                    let x = if k == 0 { left } else { left + self.px(14.0) };
                    ops.push(Op::Text(l, x, my, body, ink));
                    my += line;
                }
                if !syn.is_empty() {
                    for l in wrap(syn, width - self.px(14.0), text, body) {
                        if my + line > foot {
                            break;
                        }
                        ops.push(Op::Text(l, left + self.px(14.0), my, body, DIM));
                        my += line;
                    }
                }
                my += self.px(6.0);
            }
        }
        // The proofing language at the foot of the pane.
        if !shown.lang.is_empty() {
            let [_, py, _, ph] = self.rect;
            let fy = py + ph - self.px(32.0);
            ops.push(Op::Rect([left, fy - self.px(8.0), width, 1.0], ROW_SEL));
            ops.push(Op::Text(shown.lang.clone(), left, fy, body, ink));
        }
        self.ops = ops;
        self.hits = hits;
    }
}

impl SidePane {
    /// Lay out the Accessibility pane: `labels` title, "no issues" text, Errors heading.
    pub fn record_a11y(
        &mut self,
        text: &dyn TextMeasure,
        rows: &[A11yRow],
        labels: &[String; 3],
        ink: Rgba,
    ) {
        let mut ops = Vec::new();
        let mut hits = Vec::new();
        self.header(&mut ops, &mut hits, &labels[0], ink);
        let [x, y, w, h] = self.rect;
        let body = TextStyle {
            size: self.px(12.8),
            bold: false,
        };
        let bold = TextStyle {
            size: self.px(12.8),
            bold: true,
        };
        let left = x + self.px(12.0);
        let width = w - self.px(24.0);
        let line = text.measure("Ag", body).1.max(self.px(16.0));
        let mut ty = y + self.px(56.0);
        if rows.is_empty() {
            for l in wrap(&labels[1], width, text, body) {
                ops.push(Op::Text(l, left, ty, body, ink));
                ty += line;
            }
        } else {
            ops.push(Op::Text(
                format!("{} ({})", labels[2], rows.len()),
                left,
                ty,
                bold,
                ink,
            ));
            ty += line + self.px(8.0);
            let mut kind = "";
            for r in rows {
                if ty + line > y + h {
                    break;
                }
                if r.kind != kind {
                    kind = &r.kind;
                    ops.push(Op::Text(r.kind.clone(), left + self.px(8.0), ty, bold, ink));
                    ty += line + self.px(4.0);
                }
                let rr = [left, ty - self.px(2.0), width, line + self.px(4.0)];
                ops.push(Op::Text(
                    r.what.clone(),
                    left + self.px(24.0),
                    ty,
                    body,
                    ink,
                ));
                hits.push((rr, SideHit::Goto(r.slide, r.id)));
                ty += line + self.px(6.0);
            }
        }
        self.ops = ops;
        self.hits = hits;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;
    impl TextMeasure for Fixed {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            (
                text.chars().count() as f32 * style.size * 0.55,
                style.size * 1.33,
            )
        }
    }

    fn rows() -> Vec<SelRow> {
        vec![
            SelRow {
                id: 7,
                name: "Picture 6".into(),
                hidden: false,
                depth: 0,
                selected: true,
            },
            SelRow {
                id: 3,
                name: "Rectangle 2".into(),
                hidden: true,
                depth: 0,
                selected: false,
            },
        ]
    }

    #[test]
    fn rows_eyes_and_buttons_hit() {
        let mut pane = SidePane::default();
        pane.toggle(SideKind::Selection);
        pane.layout([1000.0, 200.0, 312.0, 700.0], 1.25);
        let labels = [
            "Selection".to_owned(),
            "Show All".to_owned(),
            "Hide All".to_owned(),
        ];
        pane.record_selection(&Fixed, &rows(), &labels, Rgba(0, 0, 0, 255));
        let find = |h: &SideHit| pane.hits.iter().find(|(_, x)| x == h).map(|(r, _)| *r);
        let r = find(&SideHit::Select(3)).expect("row");
        assert_eq!(pane.press(r[0] + 5.0, r[1] + 5.0), Some(SideHit::Select(3)));
        let e = find(&SideHit::Toggle(3)).expect("eye");
        assert_eq!(pane.press(e[0] + 5.0, e[1] + 5.0), Some(SideHit::Toggle(3)));
        let s = find(&SideHit::ShowAll).expect("show all");
        let hd = find(&SideHit::HideAll).expect("hide all");
        assert!(hd[0] > s[0] + s[2]);
        assert!(find(&SideHit::Forward).is_some() && find(&SideHit::Backward).is_some());
        // Rename in place.
        pane.start_rename(3, "Rectangle 2");
        pane.key(Key::Backspace);
        pane.text("9");
        assert_eq!(
            pane.key(Key::Enter),
            Some(SideEdit::Rename(3, "Rectangle 9".into()))
        );
        assert!(pane.rename.is_none());
    }

    #[test]
    fn alt_text_box_types_and_commits() {
        let mut pane = SidePane::default();
        pane.toggle(SideKind::AltText);
        pane.layout([1000.0, 200.0, 312.0, 700.0], 1.0);
        let labels = [
            "Alt Text".to_owned(),
            "How would you describe this object?".to_owned(),
            "(1-2 sentences)".to_owned(),
            "Mark as decorative".to_owned(),
        ];
        pane.record_alt(&Fixed, Some(("A logo", false)), &labels, Rgba(0, 0, 0, 255));
        assert!(pane.hits.iter().any(|(_, h)| *h == SideHit::AltBox));
        assert!(pane.hits.iter().any(|(_, h)| *h == SideHit::Decorative));
        pane.alt_edit = Some("A logo".into());
        pane.text(" in blue");
        assert_eq!(
            pane.key(Key::Tab),
            Some(SideEdit::AltText("A logo in blue".into()))
        );
        // Decorative: the box is not editable.
        pane.record_alt(&Fixed, Some(("", true)), &labels, Rgba(0, 0, 0, 255));
        assert!(!pane.hits.iter().any(|(_, h)| *h == SideHit::AltBox));
        assert_eq!(pane.key(Key::Enter), None);
    }

    #[test]
    fn wrap_breaks_on_spaces() {
        let style = TextStyle {
            size: 10.0,
            bold: false,
        };
        let lines = wrap("one two three four", 40.0, &Fixed, style);
        assert!(lines.len() > 1);
        assert_eq!(lines.join(" "), "one two three four");
    }
}
