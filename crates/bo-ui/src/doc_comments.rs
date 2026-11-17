//! Comments pane of BlueDoc (Review › Show Comments): the threads of the document on the
//! right of the window, laid out after a desktop word processor's markup area — a round
//! initials badge, the author in bold, the comment text, replies indented behind a grey bar.
//! The selected thread offers Reply, Resolve / Reopen and Delete (engine comments,
//! ECMA-376 Part 1 §17.13.4 with [MS-DOCX] threading).
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Font, Painter, TextMeasure, TextStyle};
use bo_edit::comments::CommentInfo;
use bo_render::{PathSeg, Rgba};

/// Measured on the reference at 125 %: white markup area behind a 198 grey edge, badges
/// 30 px across in 0,120,212 with white initials, author 44 px right of the badge, replies
/// behind a 1 px 198 bar 72 px right of the badge and their author 14 px after it.
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const EDGE: Rgba = Rgba(198, 198, 198, 255);
const MUTED: Rgba = Rgba(118, 118, 118, 255);
const LINK: Rgba = Rgba(0x1f, 0x5f, 0xb4, 255);
const SELECTED_BG: Rgba = Rgba(0xf3, 0xf3, 0xf3, 255);
/// Badge colours by author (the first is the reference's measured blue; the others are
/// Bluestar's own choice).
const BADGES: [Rgba; 5] = [
    Rgba(0, 120, 212, 255),
    Rgba(0x8a, 0x3f, 0xb8, 255),
    Rgba(0x0b, 0x7a, 0x5a, 255),
    Rgba(0xc2, 0x4f, 0x1c, 255),
    Rgba(0x9c, 0x2a, 0x54, 255),
];
/// Font of the comment text (the document's default face, as the reference shows it).
const BODY_FACE: &str = "Times New Roman";

/// What a press in the pane asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommentAction {
    Close,
    /// Select the text thread `id` is anchored to.
    Select(i32),
    /// Reply to thread `id`.
    Reply(i32),
    /// Mark thread `id` resolved (`true`) or open again.
    Resolve(i32, bool),
    /// Delete thread `id` with its replies.
    Delete(i32),
    /// Post the text of the comment being written: a new comment on the selection
    /// (`None`) or a reply to thread `id` (round 7, the reference's in-card editor).
    Post(Option<i32>, String),
    Redraw,
}

/// The comment being written in a card (round 7): a new comment or a reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Draft {
    /// Thread replied to (`None`: a new comment on the selection).
    pub(crate) parent: Option<i32>,
    /// Text typed so far.
    pub(crate) text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Reply,
    Resolve,
    Delete,
    /// The text box of the card (a new comment or the reply box).
    Field,
    Post,
    Cancel,
}

/// Card id of the new comment's card (threads have ids from 0).
const NEW_CARD: i32 = -1;

const PARTS: [(Part, &str, &str); 3] = [
    (Part::Reply, "comments.reply", "Reply"),
    (Part::Resolve, "comments.resolve", "Resolve"),
    (Part::Delete, "comments.delete", "Delete"),
];

/// One laid-out thread, in pane coordinates relative to the list's top (before scrolling).
#[derive(Debug, Clone, Default)]
struct Card {
    id: i32,
    done: bool,
    /// x, y, w, h of the whole card.
    rect: [f32; 4],
    /// Badges: centre x, centre y, initials, colour, initials width.
    badges: Vec<(f32, f32, String, Rgba, f32)>,
    /// Text lines: text, x, y, bold (author, UI font) or body (document face), colour.
    lines: Vec<(String, f32, f32, bool, Rgba)>,
    /// Reply bars: x, y, h.
    bars: Vec<[f32; 3]>,
    /// Buttons of the selected thread: part, rectangle, label.
    buttons: Vec<(Part, [f32; 4], String)>,
    /// The card's text box: rectangle, lines shown (the placeholder when empty), empty,
    /// focused.
    field: Option<([f32; 4], Vec<String>, bool, bool)>,
}

/// The comments pane.
pub(crate) struct CommentsPane {
    scale: f32,
    rect: [f32; 4],
    scroll: f32,
    /// Thread chosen in the pane (else the one holding the caret).
    pub(crate) selected: Option<i32>,
    hover: Option<(i32, Option<Part>)>,
    cards: Vec<Card>,
    content_h: f32,
    /// The comment being written, when the editor is open.
    pub(crate) draft: Option<Draft>,
    /// Author of new comments and replies (File › Options › User name).
    pub(crate) author: String,
    /// Width of the draft text's last line (caret position).
    caret_x: f32,
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

/// Initials of an author: the given ones, else the first letters of the first and last word.
fn initials(c: &CommentInfo) -> String {
    if let Some(i) = c.initials.as_deref().filter(|i| !i.trim().is_empty()) {
        return i.chars().take(2).collect::<String>().to_uppercase();
    }
    name_initials(&c.author)
}

/// Initials of a name: the first letters of its first and last word.
fn name_initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let first = words.first().and_then(|w| w.chars().next());
    let last = words
        .get(1..)
        .and_then(|r| r.last())
        .and_then(|w| w.chars().next());
    first
        .into_iter()
        .chain(last)
        .collect::<String>()
        .to_uppercase()
}

/// Break `text` into lines no wider than `width` (paragraphs kept, words not split unless
/// a single word is wider than the line).
fn wrap(text: &str, width: f32, measure: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure(&candidate) > width {
                out.push(std::mem::take(&mut line));
                line = word.to_owned();
            } else {
                line = candidate;
            }
        }
        out.push(line);
    }
    out
}

impl CommentsPane {
    pub(crate) fn new() -> Self {
        Self {
            scale: 1.0,
            rect: [0.0; 4],
            scroll: 0.0,
            selected: None,
            hover: None,
            cards: Vec::new(),
            content_h: 0.0,
            draft: None,
            author: String::new(),
            caret_x: 0.0,
        }
    }

    /// Open the editor for a new comment on the selection.
    pub(crate) fn start_new(&mut self) {
        self.draft = Some(Draft {
            parent: None,
            text: String::new(),
        });
        self.selected = None;
        self.scroll = 0.0;
    }

    /// Type into the comment being written; `false` when no editor is open.
    pub(crate) fn type_text(&mut self, s: &str) -> bool {
        let Some(d) = self.draft.as_mut() else {
            return false;
        };
        d.text.push_str(&s.replace('\r', ""));
        true
    }

    /// A key while the editor is open: Backspace, Enter (new line), Ctrl+Enter (post),
    /// Escape (cancel); `None` when no editor is open or the key is not for it.
    pub(crate) fn key(&mut self, k: crate::Key, ctrl: bool) -> Option<CommentAction> {
        let d = self.draft.as_mut()?;
        Some(match k {
            crate::Key::Backspace => {
                d.text.pop();
                CommentAction::Redraw
            }
            crate::Key::Enter if ctrl => {
                let d = self.draft.take()?;
                CommentAction::Post(d.parent, d.text)
            }
            crate::Key::Enter => {
                d.text.push('\n');
                CommentAction::Redraw
            }
            crate::Key::Escape => {
                self.draft = None;
                CommentAction::Redraw
            }
            _ => return None,
        })
    }

    /// Width of the pane at display `scale` (the reference's markup area: 394 px at 125 %).
    pub(crate) fn width(scale: f32) -> f32 {
        (315.0 * scale).round()
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Place the pane at the right edge `right` of the window, from `top` to `bottom`.
    pub(crate) fn layout(&mut self, right: f32, top: f32, bottom: f32, scale: f32) {
        self.scale = scale;
        let w = Self::width(scale);
        self.rect = [right - w, top, w, (bottom - top).max(0.0)];
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        inside(self.rect, x, y)
    }

    fn header_h(&self) -> f32 {
        self.px(48.0)
    }

    fn close_rect(&self) -> [f32; 4] {
        let s = self.px(32.0);
        [
            self.rect[0] + self.rect[2] - self.px(10.0) - s,
            self.rect[1] + self.px(8.0),
            s,
            s,
        ]
    }

    /// Author names: bold UI face, as the reference.
    fn author_font(&self) -> Font<'static> {
        Font {
            family: "Segoe UI",
            size: self.px(11.5),
            weight: 700,
            italic: false,
            underline: false,
        }
    }

    fn body_font(&self) -> Font<'static> {
        Font {
            family: BODY_FACE,
            size: self.px(12.0),
            weight: 400,
            italic: false,
            underline: false,
        }
    }

    fn badge_style(&self) -> TextStyle {
        TextStyle {
            size: self.px(10.0),
            bold: false,
        }
    }

    fn button_style(&self) -> TextStyle {
        TextStyle {
            size: self.px(11.0),
            bold: false,
        }
    }

    /// The thread shown as selected: the chosen one, else `caret` (thread at the caret).
    fn active(&self, caret: Option<i32>) -> Option<i32> {
        self.selected.or(caret)
    }

    /// Lay the threads of `comments` out as cards (measured text); `caret` is the thread
    /// whose anchor holds the caret.
    pub(crate) fn measure(
        &mut self,
        comments: &[CommentInfo],
        caret: Option<i32>,
        strings: &Strings,
        text: &dyn TextMeasure,
    ) {
        // The chosen thread may have been deleted.
        if self
            .selected
            .is_some_and(|id| !comments.iter().any(|c| c.id == id))
        {
            self.selected = None;
        }
        let active = self.active(caret);
        let mut authors: Vec<String> = Vec::new();
        let mut badge = |name: &str| -> Rgba {
            let i = authors.iter().position(|a| a == name).unwrap_or_else(|| {
                authors.push(name.to_owned());
                authors.len() - 1
            });
            BADGES[i % BADGES.len()]
        };
        let pane_w = self.rect[2];
        let d = self.px(24.0);
        let pad = self.px(14.0);
        let text_x = pad + self.px(35.0);
        let bar_x = pad + self.px(58.0);
        let reply_x = bar_x + self.px(11.0);
        let right = pane_w - pad;
        let author = self.author_font();
        let body = self.body_font();
        // Measured on the reference at 125 %: author ink 5 px under the badge top, text
        // 23 px under it, a reply's author 19 px above its text.
        let author_h = (author.size * 1.5).round();
        let body_h = (body.size * 1.3).round();
        let gap = self.px(22.0);
        let mut y = self.px(10.0);
        let mut cards = Vec::new();
        let field_lines = |t: &str, width: f32, placeholder: &str| -> (Vec<String>, bool) {
            if t.is_empty() {
                return (vec![placeholder.to_owned()], true);
            }
            let measure = |s: &str| text.measure_font(s, &body).0;
            (wrap(t, width, &measure), false)
        };
        let label = |key: &str, fallback: &str| {
            let v = strings.get(key);
            if v == key { fallback } else { v }.to_owned()
        };
        // The new comment's card first, as the reference opens it beside the selection.
        if let Some(dr) = self.draft.as_ref().filter(|dr| dr.parent.is_none()) {
            let start = y;
            let mut card = Card {
                id: NEW_CARD,
                ..Card::default()
            };
            let me = if self.author.is_empty() {
                label("docR7.c.author", "Author")
            } else {
                self.author.clone()
            };
            let ini = name_initials(&me);
            let ini_w = text.measure(&ini, self.badge_style()).0;
            card.badges
                .push((pad + d / 2.0, y + d / 2.0, ini, BADGES[0], ini_w));
            card.lines.push((me, text_x, y, true, Rgba(0, 0, 0, 255)));
            y += author_h + self.px(4.0);
            let fw = right - text_x;
            let (lines, empty) = field_lines(
                &dr.text,
                fw - self.px(12.0),
                &label("docR7.c.start", "Start a conversation"),
            );
            let fh = (lines.len().max(2) as f32 * body_h + self.px(12.0)).round();
            card.field = Some(([text_x, y, fw, fh], lines, empty, true));
            y += fh + self.px(8.0);
            y = self.post_cancel(&mut card, strings, text, right, y);
            card.rect = [
                self.px(4.0),
                start - self.px(6.0),
                pane_w - self.px(8.0),
                y - start + self.px(12.0),
            ];
            cards.push(card);
            y += self.px(6.0) + gap;
        }
        for top in comments.iter().filter(|c| c.parent.is_none()) {
            let mut card = Card {
                id: top.id,
                done: top.done,
                ..Card::default()
            };
            let start = y;
            let selected = active == Some(top.id);
            let ink = if top.done { MUTED } else { Rgba(0, 0, 0, 255) };
            let entry = |card: &mut Card, c: &CommentInfo, x: f32, y: &mut f32| {
                card.lines.push((c.author.clone(), x, *y, true, ink));
                *y += author_h;
                let width = (right - x).max(self.px(40.0));
                let measure = |s: &str| text.measure_font(s, &body).0;
                for l in wrap(&c.text, width, &measure) {
                    card.lines.push((l, x, *y, false, ink));
                    *y += body_h;
                }
            };
            let ini = initials(top);
            let ini_w = text.measure(&ini, self.badge_style()).0;
            card.badges
                .push((pad + d / 2.0, y + d / 2.0, ini, badge(&top.author), ini_w));
            entry(&mut card, top, text_x, &mut y);
            for reply in comments.iter().filter(|c| c.parent == Some(top.id)) {
                y += self.px(5.0);
                let from = y;
                entry(&mut card, reply, reply_x, &mut y);
                card.bars
                    .push([bar_x, from - self.px(4.0), y - from + self.px(4.0)]);
            }
            if top.done {
                card.lines.push((
                    strings.get("comments.resolved").to_owned(),
                    text_x,
                    y + self.px(4.0),
                    false,
                    MUTED,
                ));
                y += body_h + self.px(4.0);
            }
            // The reply box of the selected open thread (typing there writes a reply).
            if selected && !top.done {
                y += self.px(8.0);
                let writing = self
                    .draft
                    .as_ref()
                    .filter(|dr| dr.parent == Some(top.id))
                    .map(|dr| dr.text.clone());
                let fw = right - text_x;
                let (lines, empty) = field_lines(
                    writing.as_deref().unwrap_or(""),
                    fw - self.px(12.0),
                    &label("comments.reply", "Reply"),
                );
                let fh = (lines.len() as f32 * body_h + self.px(10.0)).round();
                card.field = Some(([text_x, y, fw, fh], lines, empty, writing.is_some()));
                y += fh + self.px(4.0);
                if writing.is_some() {
                    y += self.px(4.0);
                    y = self.post_cancel(&mut card, strings, text, right, y);
                    y += self.px(6.0);
                    card.rect = [
                        self.px(4.0),
                        start - self.px(6.0),
                        pane_w - self.px(8.0),
                        y - start + self.px(6.0),
                    ];
                    cards.push(card);
                    y += gap;
                    continue;
                }
            }
            if selected {
                y += self.px(8.0);
                let bs = self.button_style();
                let mut x = text_x;
                let bh = self.px(24.0);
                for (part, key, fallback) in PARTS {
                    let key = if part == Part::Resolve && top.done {
                        "comments.reopen"
                    } else {
                        key
                    };
                    let v = strings.get(key);
                    let label = if v == key { fallback } else { v }.to_owned();
                    let w = text.measure(&label, bs).0 + self.px(16.0);
                    card.buttons.push((part, [x, y, w, bh], label));
                    x += w + self.px(6.0);
                }
                y += bh;
            }
            y += self.px(6.0);
            card.rect = [
                self.px(4.0),
                start - self.px(6.0),
                pane_w - self.px(8.0),
                y - start + self.px(6.0),
            ];
            cards.push(card);
            y += gap;
        }
        self.content_h = y;
        self.cards = cards;
        // Caret after the draft's last line.
        self.caret_x = self
            .draft
            .as_ref()
            .and_then(|dr| {
                let width = right - text_x - self.px(12.0);
                let measure = |s: &str| text.measure_font(s, &body).0;
                wrap(&dr.text, width, &measure)
                    .last()
                    .map(|l| text.measure_font(l, &body).0)
            })
            .unwrap_or(0.0);
        self.clamp_scroll();
    }

    /// Post and Cancel buttons right-aligned at `y` (Post filled with the accent, as the
    /// reference's send button); returns the y under them.
    fn post_cancel(
        &self,
        card: &mut Card,
        strings: &Strings,
        text: &dyn TextMeasure,
        right: f32,
        y: f32,
    ) -> f32 {
        let bs = self.button_style();
        let bh = self.px(26.0);
        let mut x = right;
        for (part, key, fallback) in [
            (Part::Cancel, "comments.cancel", "Cancel"),
            (Part::Post, "docR7.c.post", "Post"),
        ] {
            let v = strings.get(key);
            let label = if v == key { fallback } else { v }.to_owned();
            let w = (text.measure(&label, bs).0 + self.px(24.0)).max(self.px(64.0));
            x -= w;
            card.buttons.push((part, [x, y, w, bh], label));
            x -= self.px(8.0);
        }
        y + bh
    }

    fn list_rect(&self) -> [f32; 4] {
        let h = self.header_h();
        [
            self.rect[0],
            self.rect[1] + h,
            self.rect[2],
            (self.rect[3] - h).max(0.0),
        ]
    }

    fn clamp_scroll(&mut self) {
        let max = (self.content_h - self.list_rect()[3]).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max);
    }

    /// Scroll the list by `dy` device pixels; `true` when it moved.
    pub(crate) fn wheel(&mut self, dy: f32) -> bool {
        let before = self.scroll;
        self.scroll += dy;
        self.clamp_scroll();
        (self.scroll - before).abs() > 0.1
    }

    /// Card offset to window coordinates.
    fn origin(&self) -> (f32, f32) {
        let l = self.list_rect();
        (l[0], l[1] - self.scroll)
    }

    fn hit(&self, x: f32, y: f32) -> Option<(i32, Option<Part>)> {
        if !inside(self.list_rect(), x, y) {
            return None;
        }
        let (ox, oy) = self.origin();
        let (lx, ly) = (x - ox, y - oy);
        let card = self.cards.iter().find(|c| inside(c.rect, lx, ly))?;
        let part = card
            .buttons
            .iter()
            .find(|(_, r, _)| inside(*r, lx, ly))
            .map(|(p, _, _)| *p)
            .or_else(|| {
                card.field
                    .as_ref()
                    .filter(|f| inside(f.0, lx, ly))
                    .map(|_| Part::Field)
            });
        Some((card.id, part))
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.hit(x, y);
        let changed = h != self.hover;
        self.hover = h;
        changed
    }

    pub(crate) fn press(&mut self, x: f32, y: f32) -> CommentAction {
        if inside(self.close_rect(), x, y) {
            return CommentAction::Close;
        }
        let Some((id, part)) = self.hit(x, y) else {
            return CommentAction::Redraw;
        };
        let done = self
            .cards
            .iter()
            .find(|c| c.id == id)
            .is_some_and(|c| c.done);
        if id == NEW_CARD || part == Some(Part::Post) || part == Some(Part::Cancel) {
            return match part {
                Some(Part::Post) => match self.draft.take() {
                    Some(dr) => CommentAction::Post(dr.parent, dr.text),
                    None => CommentAction::Redraw,
                },
                Some(Part::Cancel) => {
                    self.draft = None;
                    CommentAction::Redraw
                }
                _ => CommentAction::Redraw,
            };
        }
        if part == Some(Part::Field) {
            if self.draft.as_ref().is_none_or(|dr| dr.parent != Some(id)) {
                self.draft = Some(Draft {
                    parent: Some(id),
                    text: String::new(),
                });
            }
            self.selected = Some(id);
            return CommentAction::Redraw;
        }
        match part {
            Some(Part::Reply) => CommentAction::Reply(id),
            Some(Part::Resolve) => CommentAction::Resolve(id, !done),
            Some(Part::Delete) => CommentAction::Delete(id),
            Some(Part::Field | Part::Post | Part::Cancel) | None => {
                self.selected = Some(id);
                CommentAction::Select(id)
            }
        }
    }

    /// Paint the pane; `caret` is the thread whose anchor holds the caret.
    pub(crate) fn paint(
        &self,
        p: &mut dyn Painter,
        t: &Theme,
        strings: &Strings,
        caret: Option<i32>,
    ) {
        let [x, y, w, h] = self.rect;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        p.push_clip(x, y, w, h);
        p.fill_rect(x, y, w, h, WHITE);
        p.fill_rect(x, y, 1.0, h, EDGE);
        let title = TextStyle {
            size: self.px(15.0),
            bold: false,
        };
        p.text(
            strings.get("comments.title"),
            x + self.px(14.0),
            y + self.px(12.0),
            title,
            t.text,
        );
        let c = self.close_rect();
        let (cx, cy, k) = (c[0] + c[2] / 2.0, c[1] + c[3] / 2.0, self.px(5.0));
        p.path(
            &[
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((t.text, self.scale.max(1.0))),
        );
        let l = self.list_rect();
        if self.cards.is_empty() && self.draft.is_none() {
            p.text(
                strings.get("comments.none"),
                x + self.px(14.0),
                l[1] + self.px(10.0),
                self.button_style(),
                MUTED,
            );
        }
        p.push_clip(l[0], l[1], l[2], l[3]);
        let (ox, oy) = self.origin();
        let active = self.active(caret);
        let d = self.px(24.0);
        for card in &self.cards {
            let r = card.rect;
            if r[1] + oy > l[1] + l[3] || r[1] + r[3] + oy < l[1] {
                continue;
            }
            if active == Some(card.id) {
                p.fill_rect(ox + r[0], oy + r[1], r[2], r[3], SELECTED_BG);
                p.fill_rect(ox + r[0], oy + r[1], self.px(3.0), r[3], t.accent);
            }
            for bar in &card.bars {
                p.fill_rect(ox + bar[0], oy + bar[1], 1.0, bar[2], EDGE);
            }
            for (bx, by, ini, color, tw) in &card.badges {
                let (cx, cy, rad) = (ox + bx, oy + by, d / 2.0);
                let mut segs = Vec::with_capacity(26);
                for k in 0..=24 {
                    let a = k as f32 / 24.0 * std::f32::consts::TAU;
                    let pt = (cx + rad * a.cos(), cy + rad * a.sin());
                    segs.push(if k == 0 {
                        PathSeg::M(pt.0, pt.1)
                    } else {
                        PathSeg::L(pt.0, pt.1)
                    });
                }
                segs.push(PathSeg::Z);
                let fill = if card.done { MUTED } else { *color };
                p.path(&segs, Some(fill), None);
                let st = self.badge_style();
                p.text(ini, cx - tw / 2.0, cy - st.size * 0.68, st, WHITE);
            }
            for (s, lx, ly, bold, color) in &card.lines {
                if *bold {
                    p.text_font(s, ox + lx, oy + ly, &self.author_font(), *color);
                } else {
                    p.text_font(s, ox + lx, oy + ly, &self.body_font(), *color);
                }
            }
            if let Some((fr, lines, empty, focused)) = &card.field {
                let b = [ox + fr[0], oy + fr[1], fr[2], fr[3]];
                let edge = if *focused { t.accent } else { EDGE };
                p.fill_rect(b[0], b[1], b[2], b[3], edge);
                p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, WHITE);
                let body = self.body_font();
                let lh = (body.size * 1.3).round();
                let mut ly = b[1] + self.px(5.0);
                for l in lines {
                    let ink = if *empty { MUTED } else { Rgba(0, 0, 0, 255) };
                    p.text_font(l, b[0] + self.px(6.0), ly, &body, ink);
                    ly += lh;
                }
                if *focused {
                    let cx = if *empty { 0.0 } else { self.caret_x };
                    p.fill_rect(
                        (b[0] + self.px(6.0) + cx).round(),
                        ly - lh + self.px(1.0),
                        1.0,
                        lh - self.px(2.0),
                        Rgba(0, 0, 0, 255),
                    );
                }
            }
            for (part, br, label) in &card.buttons {
                let hovered = self.hover == Some((card.id, Some(*part)));
                let b = [ox + br[0], oy + br[1], br[2], br[3]];
                let st = self.button_style();
                let ty = b[1] + (b[3] - st.size * 1.33) / 2.0;
                if *part == Part::Post {
                    // The send button: filled with the accent, white label.
                    p.fill_rect(b[0], b[1], b[2], b[3], t.accent_strong);
                    let tw = label.chars().count() as f32 * st.size * 0.5;
                    p.text(label, b[0] + (b[2] - tw) / 2.0, ty, st, WHITE);
                    continue;
                }
                p.fill_rect(b[0], b[1], b[2], b[3], EDGE);
                p.fill_rect(
                    b[0] + 1.0,
                    b[1] + 1.0,
                    b[2] - 2.0,
                    b[3] - 2.0,
                    if hovered { t.hover } else { WHITE },
                );
                p.text(label, b[0] + self.px(8.0), ty, st, LINK);
            }
        }
        p.pop_clip();
        p.pop_clip();
    }

    /// Ids of the threads laid out, in order (tests).
    #[cfg(test)]
    pub(crate) fn thread_ids(&self) -> Vec<i32> {
        self.cards.iter().map(|c| c.id).collect()
    }

    /// Centre of the button `label_key` of thread `id` in window coordinates (tests).
    #[cfg(test)]
    pub(crate) fn button_center(&self, id: i32, index: usize) -> Option<(f32, f32)> {
        let (ox, oy) = self.origin();
        let card = self.cards.iter().find(|c| c.id == id)?;
        let (_, r, _) = card.buttons.get(index)?;
        Some((ox + r[0] + r[2] / 2.0, oy + r[1] + r[3] / 2.0))
    }

    /// Centre of thread `id`'s first line in window coordinates (tests).
    #[cfg(test)]
    pub(crate) fn card_center(&self, id: i32) -> Option<(f32, f32)> {
        let (ox, oy) = self.origin();
        let card = self.cards.iter().find(|c| c.id == id)?;
        Some((
            ox + card.rect[0] + card.rect[2] / 2.0,
            oy + card.rect[1] + self.px(10.0),
        ))
    }
}
