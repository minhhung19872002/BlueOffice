//! Comment cards in the markup area (round 7, the reference's Contextual view): New Comment
//! opens a floating card beside the selection with the author's initials circle (in the
//! author's colour, as the markup balloons), the name in bold and "A few seconds ago", a text
//! box and Post / Cancel; the comment holding the caret shows as a card over its balloon with
//! Reply and Resolve. The Comments pane stays the List view (Review › Show Comments).
use super::super::DocWindow;
use crate::{Font, Input, Key, Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};
use serde_json::json;

const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(38, 38, 38, 255);
const MUTED: Rgba = Rgba(96, 96, 96, 255);
const EDGE: Rgba = Rgba(198, 198, 198, 255);
/// Link buttons (Reply / Resolve) measured on the reference: #2B579A.
const LINK: Rgba = Rgba(0x2b, 0x57, 0x9a, 255);

/// What the card is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CardFor {
    /// A new comment on the selection being written.
    New,
    /// The thread holding the caret (`reply`: a reply being written).
    Thread { id: i32, reply: bool },
}

/// The floating card.
#[derive(Debug, Clone)]
pub(crate) struct Card {
    /// What it shows (`None`: no card).
    pub(crate) what: Option<CardFor>,
    /// Text being written.
    pub(crate) text: String,
    /// Laid-out parts (window px): card, text box, buttons (id, rect, label).
    rect: [f32; 4],
    field: Option<[f32; 4]>,
    buttons: Vec<(&'static str, [f32; 4], String)>,
    /// Lines drawn: text, x, y, bold, colour.
    lines: Vec<(String, f32, f32, bool, Rgba)>,
    /// Field lines and the caret x.
    field_lines: Vec<String>,
    caret_x: f32,
    /// Initials circle: centre, radius, initials, colour.
    badge: Option<(f32, f32, f32, String, Rgba)>,
    border: Rgba,
    clip: [f32; 4],
}

impl Default for Card {
    fn default() -> Self {
        Card {
            what: None,
            text: String::new(),
            rect: [0.0; 4],
            field: None,
            buttons: Vec::new(),
            lines: Vec::new(),
            field_lines: Vec::new(),
            caret_x: 0.0,
            badge: None,
            border: EDGE,
            clip: [0.0; 4],
        }
    }
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

/// Initials of a name (first letters of the first and last word).
fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    words
        .first()
        .and_then(|w| w.chars().next())
        .into_iter()
        .chain(
            words
                .get(1..)
                .and_then(|r| r.last())
                .and_then(|w| w.chars().next()),
        )
        .flat_map(char::to_uppercase)
        .collect()
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "A few seconds ago", "5 minutes ago", "2 hours ago", else the date (M/D/YYYY), as the
/// reference labels comment times; `date` is ISO 8601 (`w:date`, §17.13.4.2), `now` Unix s.
pub(crate) fn relative_time(date: Option<&str>, now: i64) -> String {
    let Some(d) = date else {
        return "A few seconds ago".to_owned();
    };
    let num = |r: std::ops::Range<usize>| d.get(r).and_then(|s| s.parse::<i64>().ok());
    let (Some(y), Some(mo), Some(da)) = (num(0..4), num(5..7), num(8..10)) else {
        return String::new();
    };
    let (h, mi, s) = (
        num(11..13).unwrap_or(0),
        num(14..16).unwrap_or(0),
        num(17..19).unwrap_or(0),
    );
    let t = days(y, mo, da) * 86_400 + h * 3600 + mi * 60 + s;
    let ago = now - t;
    match ago {
        i64::MIN..60 => "A few seconds ago".to_owned(),
        60..120 => "1 minute ago".to_owned(),
        120..3600 => format!("{} minutes ago", ago / 60),
        3600..7200 => "1 hour ago".to_owned(),
        7200..86_400 => format!("{} hours ago", ago / 3600),
        _ => format!("{mo}/{da}/{y}"),
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn wrap(text: &str, width: f32, measure: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
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

impl DocWindow {
    /// Open the card for a new comment on the selection (markup area), when the view has
    /// pages to place it beside; `false` lets the Comments pane take it.
    pub(super) fn r7_card_new(&mut self) -> bool {
        if self.view.slots().is_empty() {
            return false;
        }
        self.r7.card = Card {
            what: Some(CardFor::New),
            ..Card::default()
        };
        true
    }

    /// The thread whose card shows: the one holding the caret (top-level id).
    fn r7_card_thread(&self) -> Option<i32> {
        let id = self.view.comment_at_caret()?;
        let c = self.view.comments().iter().find(|c| c.id == id)?;
        Some(c.parent.unwrap_or(c.id))
    }

    /// Lay the card out from the document's geometry (each frame).
    #[allow(clippy::too_many_lines)]
    pub(super) fn r7_layout_card(&mut self, text: &dyn TextMeasure) {
        // The thread at the caret shows its card when nothing is being written.
        let writing = !self.r7.card.text.is_empty()
            || matches!(
                self.r7.card.what,
                Some(CardFor::New | CardFor::Thread { reply: true, .. })
            );
        if !writing {
            self.r7.card.what = self
                .r7_card_thread()
                .filter(|_| self.comments_pane.is_none())
                .map(|id| CardFor::Thread { id, reply: false });
        }
        let Some(what) = self.r7.card.what.clone() else {
            return;
        };
        let s = self.scale;
        let px = |v: f32| (v * s).round();
        let vs = self.view.scale();
        let engine = self.view.engine();
        let comments = self.view.comments().to_vec();
        let thread = match &what {
            CardFor::Thread { id, .. } => comments.iter().find(|c| c.id == *id).cloned(),
            CardFor::New => None,
        };
        // Where: the balloon of the thread, else the anchor's line.
        let (a, b) = match (&thread, engine.session()) {
            (Some(t), _) => match (t.start, t.end) {
                (Some(a), Some(b)) => (a, b),
                _ => return,
            },
            (None, Some(sess)) => sess.selection().ordered(),
            _ => return,
        };
        let Some(r) = engine.range_rects(a, b).into_iter().next() else {
            return;
        };
        let slots = self.view.slots();
        let Some(slot) = slots.iter().find(|sl| sl.index == r.page) else {
            self.r7.card.rect = [0.0; 4];
            return;
        };
        let markup = self.view.markup_px();
        let x0 = slot.x as f32 + slot.width as f32;
        let (x, w) = if markup > px(120.0) {
            (x0 + px(1.0), markup - px(2.0))
        } else {
            (x0 + px(8.0), px(300.0))
        };
        let mut y = slot.y as f32 + r.y * vs - px(8.0);
        if let Some(t) = &thread {
            if let Some(bl) = engine
                .page_markup(r.page)
                .and_then(|m| m.balloons.iter().find(|bl| bl.id == t.id))
            {
                y = slot.y as f32 + bl.y * vs - px(6.0);
            }
        }
        let me = self.r6.prefs.user_name.trim();
        let author = if let Some(t) = &thread {
            t.author.clone()
        } else if me.is_empty() {
            self.t("docR7.c.author", "Author")
        } else {
            me.to_owned()
        };
        let k = bo_layout::author_color(&author);
        let colour = Rgba(k.r, k.g, k.b, 255);
        let name = Font {
            family: "Segoe UI",
            size: px(11.5),
            weight: 700,
            italic: false,
            underline: false,
        };
        let body = Font {
            family: "Segoe UI",
            size: px(11.0),
            weight: 400,
            italic: false,
            underline: false,
        };
        let lh = (body.size * 1.4).round();
        let clip = [
            0.0,
            self.chrome_bottom(),
            self.size.0 - self.r3_pane_w() - self.sb_w(),
            self.size.1 - self.status_h() - self.chrome_bottom(),
        ];
        let labels: Vec<(&'static str, String)> =
            if matches!(what, CardFor::New | CardFor::Thread { reply: true, .. }) {
                vec![
                    ("cancel", self_t(&self.strings, "comments.cancel", "Cancel")),
                    ("post", self_t(&self.strings, "docR7.c.post", "Post")),
                ]
            } else {
                vec![
                    (
                        "resolve",
                        self_t(&self.strings, "comments.resolve", "Resolve"),
                    ),
                    ("reply", self_t(&self.strings, "comments.reply", "Reply")),
                ]
            };
        let card = &mut self.r7.card;
        card.lines.clear();
        card.buttons.clear();
        card.border = colour;
        let pad = px(12.0);
        let rad = px(12.0);
        let cx = x + pad + rad;
        let top = y;
        let mut yy = y + pad;
        card.badge = Some((cx, yy + rad, rad, initials(&author), colour));
        let tx = cx + rad + px(10.0);
        let right = x + w - pad;
        card.lines.push((author.clone(), tx, yy, true, INK));
        let name_w = text.measure_font(&author, &name).0;
        let when = relative_time(thread.as_ref().and_then(|t| t.date.as_deref()), now_secs());
        card.lines
            .push((when, tx + name_w + px(6.0), yy + px(1.0), false, MUTED));
        yy += (name.size * 1.5).round().max(rad * 2.0) + px(2.0);
        let measure = |s: &str| text.measure_font(s, &body).0;
        if let Some(t) = &thread {
            for l in wrap(&t.text, right - tx, &measure) {
                card.lines.push((l, tx, yy, false, INK));
                yy += lh;
            }
            for r in comments.iter().filter(|c| c.parent == Some(t.id)) {
                yy += px(6.0);
                card.lines.push((r.author.clone(), tx, yy, true, INK));
                yy += (name.size * 1.5).round();
                for l in wrap(&r.text, right - tx, &measure) {
                    card.lines.push((l, tx, yy, false, INK));
                    yy += lh;
                }
            }
        }
        let editing = matches!(what, CardFor::New | CardFor::Thread { reply: true, .. });
        if editing {
            yy += px(4.0);
            card.field_lines = if card.text.is_empty() {
                Vec::new()
            } else {
                wrap(&card.text, right - tx - px(12.0), &measure)
            };
            let n = card.field_lines.len().max(1) as f32;
            let fh = (n * lh + px(12.0)).max(px(32.0));
            card.field = Some([tx, yy, right - tx, fh]);
            card.caret_x = card.field_lines.last().map_or(0.0, |l| measure(l));
            yy += fh + px(8.0);
        } else {
            card.field = None;
        }
        // Buttons right-aligned at the bottom: Post / Cancel while writing, else Reply /
        // Resolve as the reference's card.
        let st = TextStyle {
            size: px(11.0),
            bold: false,
        };
        let bh = px(26.0);
        let mut bx = right;
        for (id, label) in labels {
            let icon = if editing { 0.0 } else { px(18.0) };
            let bw = (text.measure(&label, st).0 + icon + px(16.0)).max(px(56.0));
            bx -= bw;
            card.buttons.push((id, [bx, yy, bw, bh], label));
            bx -= px(4.0);
        }
        yy += bh + px(8.0);
        card.rect = [x, top, w, yy - top];
        card.clip = clip;
    }

    /// Input on the card; `None` lets it through.
    pub(super) fn r7_card_input(&mut self, input: &Input) -> Option<bool> {
        let what = self.r7.card.what.clone()?;
        let editing = matches!(what, CardFor::New | CardFor::Thread { reply: true, .. });
        match input {
            Input::Text(t) if editing => {
                self.r7.card.text.push_str(&t.replace('\r', ""));
                Some(true)
            }
            Input::Key(k, m) if editing => match k {
                Key::Backspace => {
                    self.r7.card.text.pop();
                    Some(true)
                }
                Key::Enter if m.ctrl => Some(self.r7_card_post()),
                Key::Enter => {
                    self.r7.card.text.push('\n');
                    Some(true)
                }
                Key::Escape => {
                    self.r7.card = Card::default();
                    Some(true)
                }
                _ => Some(false),
            },
            Input::Press { x, y, .. } => {
                let (x, y) = (*x, *y);
                if !inside(self.r7.card.rect, x, y) {
                    // A click elsewhere posts what was written (an empty card goes away).
                    if editing {
                        self.r7_card_post();
                    }
                    return None;
                }
                let hit = self
                    .r7
                    .card
                    .buttons
                    .iter()
                    .find(|(_, r, _)| inside(*r, x, y))
                    .map(|(id, ..)| *id);
                Some(match (hit, &what) {
                    (Some("post"), _) => self.r7_card_post(),
                    (Some("cancel"), _) => {
                        self.r7.card = Card::default();
                        true
                    }
                    (Some("reply"), CardFor::Thread { id, .. }) => {
                        self.r7.card.what = Some(CardFor::Thread {
                            id: *id,
                            reply: true,
                        });
                        true
                    }
                    (Some("resolve"), CardFor::Thread { id, .. }) => {
                        let id = *id;
                        self.r7.card = Card::default();
                        self.r3_run(vec![
                            json!({ "cmd": "resolveComment", "id": id, "done": true }),
                        ])
                    }
                    _ => false,
                })
            }
            Input::Release { x, y } | Input::Drag { x, y } | Input::Move { x, y }
                if inside(self.r7.card.rect, *x, *y) =>
            {
                Some(false)
            }
            _ => None,
        }
    }

    /// Post the card's text: a new comment on the selection or a reply.
    fn r7_card_post(&mut self) -> bool {
        let card = std::mem::take(&mut self.r7.card);
        let text = card.text.trim_end().to_owned();
        if text.trim().is_empty() {
            return true;
        }
        let cmd = match card.what {
            Some(CardFor::New) => json!({ "cmd": "addComment", "text": text }),
            Some(CardFor::Thread { id, .. }) => {
                json!({ "cmd": "replyComment", "id": id, "text": text })
            }
            None => return false,
        };
        self.r3_run(vec![cmd])
    }

    /// Paint the card over the document.
    pub(super) fn r7_paint_card(&self, p: &mut dyn Painter) {
        let c = &self.r7.card;
        if c.what.is_none() || c.rect[2] <= 0.0 {
            return;
        }
        let s = self.scale;
        let px = |v: f32| (v * s).round();
        let [cx0, cy0, cw, ch] = c.clip;
        p.push_clip(cx0, cy0, cw, ch);
        let [x, y, w, h] = c.rect;
        // A soft shadow, then the card framed in the author's colour.
        p.fill_rect(x + px(2.0), y + px(2.0), w, h, Rgba(0, 0, 0, 28));
        p.fill_rect(x, y, w, h, c.border);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
        if let Some((bx, by, r, ini, col)) = &c.badge {
            let k = r * 0.552_284_8;
            p.path(
                &[
                    PathSeg::M(bx + r, *by),
                    PathSeg::C(bx + r, by + k, bx + k, by + r, *bx, by + r),
                    PathSeg::C(bx - k, by + r, bx - r, by + k, bx - r, *by),
                    PathSeg::C(bx - r, by - k, bx - k, by - r, *bx, by - r),
                    PathSeg::C(bx + k, by - r, bx + r, by - k, bx + r, *by),
                    PathSeg::Z,
                ],
                Some(*col),
                None,
            );
            let st = TextStyle {
                size: px(9.0),
                bold: false,
            };
            p.text(
                ini,
                bx - st.size * 0.33 * ini.chars().count() as f32,
                by - st.size * 0.66,
                st,
                WHITE,
            );
        }
        let name = Font {
            family: "Segoe UI",
            size: px(11.5),
            weight: 700,
            italic: false,
            underline: false,
        };
        let body = Font {
            size: px(11.0),
            weight: 400,
            ..name
        };
        for (t, lx, ly, bold, col) in &c.lines {
            p.text_font(t, *lx, *ly, if *bold { &name } else { &body }, *col);
        }
        if let Some(f) = c.field {
            p.fill_rect(f[0], f[1], f[2], f[3], self.theme.accent);
            p.fill_rect(f[0] + 1.0, f[1] + 1.0, f[2] - 2.0, f[3] - 2.0, WHITE);
            let lh = (body.size * 1.4).round();
            let mut ly = f[1] + px(6.0);
            if c.field_lines.is_empty() {
                let ph = match c.what {
                    Some(CardFor::New) => self.t("docR7.c.start", "Start a conversation"),
                    _ => self.t("comments.reply", "Reply"),
                };
                p.text_font(&ph, f[0] + px(6.0), ly, &body, MUTED);
                ly += lh;
            }
            for l in &c.field_lines {
                p.text_font(l, f[0] + px(6.0), ly, &body, INK);
                ly += lh;
            }
            p.fill_rect(
                (f[0] + px(6.0) + c.caret_x).round(),
                ly - lh + px(1.0),
                1.0,
                lh - px(2.0),
                INK,
            );
        }
        let st = TextStyle {
            size: px(11.0),
            bold: false,
        };
        for (id, r, label) in &c.buttons {
            let ty = (r[1] + (r[3] - st.size * 1.33) / 2.0).round();
            match *id {
                "post" => {
                    p.fill_rect(r[0], r[1], r[2], r[3], self.theme.accent_strong);
                    p.text(label, r[0] + px(8.0), ty, st, WHITE);
                }
                "cancel" => {
                    p.fill_rect(r[0], r[1], r[2], r[3], EDGE);
                    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
                    p.text(label, r[0] + px(8.0), ty, st, INK);
                }
                _ => {
                    // Reply: a hooked arrow; Resolve: a tick in a speech box (own icons).
                    let (ix, iy) = (r[0] + px(4.0), r[1] + r[3] / 2.0);
                    let segs = if *id == "reply" {
                        vec![
                            PathSeg::M(ix + px(12.0), iy + px(5.0)),
                            PathSeg::L(ix + px(12.0), iy),
                            PathSeg::L(ix + px(3.0), iy),
                            PathSeg::M(ix + px(6.0), iy - px(3.0)),
                            PathSeg::L(ix + px(3.0), iy),
                            PathSeg::L(ix + px(6.0), iy + px(3.0)),
                        ]
                    } else {
                        vec![
                            PathSeg::M(ix + px(1.0), iy - px(6.0)),
                            PathSeg::L(ix + px(14.0), iy - px(6.0)),
                            PathSeg::L(ix + px(14.0), iy + px(4.0)),
                            PathSeg::L(ix + px(5.0), iy + px(4.0)),
                            PathSeg::L(ix + px(2.0), iy + px(7.0)),
                            PathSeg::L(ix + px(1.0), iy - px(6.0)),
                            PathSeg::M(ix + px(4.0), iy - px(1.0)),
                            PathSeg::L(ix + px(7.0), iy + px(2.0)),
                            PathSeg::L(ix + px(12.0), iy - px(3.0)),
                        ]
                    };
                    p.path(&segs, None, Some((LINK, s.max(1.0))));
                    p.text(label, r[0] + px(22.0), ty, st, LINK);
                }
            }
        }
        p.pop_clip();
    }
}

fn self_t(s: &crate::strings::Strings, key: &str, fallback: &str) -> String {
    let v = s.get(key);
    if v == key { fallback } else { v }.to_owned()
}

#[cfg(test)]
mod tests {
    #[test]
    fn relative_times_read_like_the_reference() {
        let now = super::days(2026, 11, 17) * 86_400 + 12 * 3600;
        assert_eq!(
            super::relative_time(Some("2026-11-17T11:59:30Z"), now),
            "A few seconds ago"
        );
        assert_eq!(
            super::relative_time(Some("2026-11-17T11:55:00Z"), now),
            "5 minutes ago"
        );
        assert_eq!(
            super::relative_time(Some("2026-11-17T09:00:00Z"), now),
            "3 hours ago"
        );
        assert_eq!(
            super::relative_time(Some("2026-10-02T09:00:00Z"), now),
            "10/2/2026"
        );
        assert_eq!(super::initials("An Nguyen"), "AN");
    }
}
