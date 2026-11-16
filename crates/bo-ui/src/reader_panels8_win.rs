//! BlueReader round 8 (a child of `reader_window`): the Layers and Attachments panels and the
//! comment list like the reference reader's left panels, measured at 125 %:
//! - Attachments: a tool row (open, save, add), a rule, a column header band (Name /
//!   Description / Size, 125 px columns, 24 px high on 245, 246, 250) and 21 px rows.
//! - Comments: a tool row (expand / collapse all, previous / next, sort), a search box with a
//!   filter button, "Total Amount", groups (34 px bands on 230, 233, 240) of cards (frame 199,
//!   204, 217) with the kind icon, author, text, date and replies; review status and checkmark
//!   from state replies (ISO 32000-1 §12.5.6.3), a filter dialog by status / author / type /
//!   checkmark.
//! - Layers: optional content groups (§8.11.4) with a visibility box per row.
use super::{inside, kind_label, rounded, Ink, Panel, ReaderRequest, ReaderWindow, TEXT, WHITE};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, MenuItem};
use crate::reader_icons;
use crate::{Input, Key, Painter, TextMeasure, TextStyle};
use bo_pdfread::{Annot, AnnotKind, CommentThread};
use bo_render::{PathSeg, Rgba};
use std::collections::HashSet;
use std::sync::atomic::{AtomicI32, Ordering};

/// Group bands of the comment list and the separator rule.
const BAND: Rgba = Rgba(230, 233, 240, 255);
/// Card and column separator frames.
const FRAME: Rgba = Rgba(199, 204, 217, 255);
/// Column header band of the Attachments panel.
const HEADER: Rgba = Rgba(245, 246, 250, 255);
/// Search box frame.
const BOX: Rgba = Rgba(188, 193, 209, 255);
/// Muted text (placeholder, counts).
const SOFT: Rgba = Rgba(110, 110, 112, 255);
/// Disabled tool ink.
const DIM: Rgba = Rgba(175, 175, 176, 255);
/// Reply count mark.
const RED: Rgba = Rgba(222, 41, 22, 255);
/// Text of the comment cards (the reference draws them black).
const INK: Rgba = Rgba(17, 17, 17, 255);
/// Tool icons of the panels (measured stroke core 53, 53, 53).
const TOOL: Rgba = Rgba(53, 53, 53, 255);
/// Selected row of the Attachments / Layers panels.
const SELECTED: Rgba = Rgba(222, 226, 236, 255);

/// Review states a comment can be set to (§12.5.6.3 Table 172, `/StateModel /Review`).
pub(crate) const REVIEW_STATES: [&str; 5] =
    ["None", "Accepted", "Rejected", "Cancelled", "Completed"];

/// Local time offset from UTC in minutes (set by the host), for "today" in the comment list.
static UTC_OFFSET: AtomicI32 = AtomicI32::new(0);

/// File types an attachment may be opened as without saving it first (documents and pictures;
/// programs and scripts are only saved).
const SAFE_TYPES: [&str; 22] = [
    "txt", "csv", "tsv", "log", "md", "json", "xml", "png", "jpg", "jpeg", "gif", "bmp", "tif",
    "tiff", "docx", "xlsx", "pptx", "odt", "ods", "odp", "rtf", "pdf",
];

/// Sort orders of the comment list (the reference's Sort menu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SortBy {
    #[default]
    Page,
    Type,
    Author,
    Date,
    Status,
    Checkmark,
    Color,
}

const SORTS: [(SortBy, &str); 7] = [
    (SortBy::Page, "pdfPanels.sortPage"),
    (SortBy::Type, "pdfPanels.sortType"),
    (SortBy::Author, "pdfPanels.sortAuthor"),
    (SortBy::Date, "pdfPanels.sortDate"),
    (SortBy::Status, "pdfPanels.sortStatus"),
    (SortBy::Checkmark, "pdfPanels.sortCheckmark"),
    (SortBy::Color, "pdfPanels.sortColor"),
];

/// What the comment list shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CommentFilter {
    /// Review states ("None" for no status); empty: any.
    pub status: Vec<String>,
    /// Authors; empty: any.
    pub authors: Vec<String>,
    /// Kinds; empty: any.
    pub kinds: Vec<AnnotKind>,
    /// Checkmark; `None`: either.
    pub checked: Option<bool>,
}

impl CommentFilter {
    fn is_empty(&self) -> bool {
        self.status.is_empty()
            && self.authors.is_empty()
            && self.kinds.is_empty()
            && self.checked.is_none()
    }
}

/// The filter dialog: toggle buttons by section, Clear All, OK, Cancel.
#[derive(Debug, Clone, Default)]
struct FilterDialog {
    filter: CommentFilter,
    authors: Vec<String>,
    kinds: Vec<AnnotKind>,
    /// Button boxes from the last paint: (action, box).
    hits: Vec<(String, [f32; 4])>,
    frame: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
enum Hit {
    /// The menu button of the selected card.
    More,
    /// The Add Reply box of the selected card.
    ReplyBox,
    Layer(usize),
    Attachment(usize),
    Group(String),
    Card(usize, [f32; 4]),
}

/// Comments with their review / marked states.
type Shown = Vec<(CommentThread, bo_pdfread::CommentState)>;
/// A filter dialog section: title key, buttons (action, label, pressed).
type Section = (&'static str, Vec<(String, String, bool)>);

/// State of the round 8 panels.
#[derive(Default)]
pub(crate) struct Panels8 {
    tools: Vec<(&'static str, [f32; 4], bool)>,
    /// Reply being typed in the selected card's Add Reply box (focused when `Some`).
    reply: Option<String>,
    /// Search box options (the reference's Whole words only / Case-Sensitive).
    find_case: bool,
    find_word: bool,
    hover: Option<&'static str>,
    rows: Vec<([f32; 4], Hit)>,
    attach_selected: Option<usize>,
    layer_selected: Option<usize>,
    search: String,
    search_focus: bool,
    search_box: [f32; 4],
    funnel: [f32; 4],
    pub(crate) sort: SortBy,
    pub(crate) filter: CommentFilter,
    /// Groups whose open state differs from the default (the first group open).
    toggled: HashSet<String>,
    /// Expand All / Collapse All: every group open or closed.
    all_open: Option<bool>,
    dialog: Option<FilterDialog>,
    /// An attachment waiting for the "open it?" answer.
    confirm: Option<(Dialog, usize)>,
}

/// A comment's group key and label under a sort order.
fn group_of(
    sort: SortBy,
    a: &Annot,
    state: &bo_pdfread::CommentState,
    w: &ReaderWindow,
) -> (String, String) {
    match sort {
        SortBy::Page => (
            format!("{:08}", a.page),
            format!("{} {}", w.t("pdfReader.page"), a.page + 1),
        ),
        SortBy::Type => {
            let k = kind_label(a.kind);
            (k.to_owned(), w.t(k))
        }
        SortBy::Author => (a.author.to_lowercase(), a.author.clone()),
        SortBy::Date => {
            let (d, _) = split_date(&a.modified);
            (d.clone(), d)
        }
        SortBy::Status => {
            let s = state.review.clone().unwrap_or_else(|| "None".into());
            (s.clone(), w.t(&format!("pdfPanels.state{s}")))
        }
        SortBy::Checkmark => {
            if state.marked {
                ("0".into(), w.t("pdfPanels.checked"))
            } else {
                ("1".into(), w.t("pdfPanels.unchecked"))
            }
        }
        SortBy::Color => {
            let c = a.color;
            let hex = format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
            (hex.clone(), hex)
        }
    }
}

/// `D:YYYYMMDDHHmmSS…` as ("YYYY-MM-DD", "h:mm:ss AM"); empty strings when unreadable.
pub(crate) fn split_date(raw: &str) -> (String, String) {
    let s = raw.strip_prefix("D:").unwrap_or(raw);
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() < 8 {
        return (String::new(), String::new());
    }
    let n = |a: usize, b: usize| digits.get(a..b).and_then(|v| v.parse::<u32>().ok());
    let date = format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8]);
    let (h, m, sec) = (
        n(8, 10).unwrap_or(0),
        n(10, 12).unwrap_or(0),
        n(12, 14).unwrap_or(0),
    );
    let (h12, ampm) = match h {
        0 => (12, "AM"),
        1..=11 => (h, "AM"),
        12 => (12, "PM"),
        _ => (h - 12, "PM"),
    };
    (date, format!("{h12}:{m:02}:{sec:02} {ampm}"))
}

/// Today's local date as "YYYY-MM-DD".
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
        + i64::from(UTC_OFFSET.load(Ordering::Relaxed)) * 60;
    let days = secs.div_euclid(86_400);
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The date shown on a card: the time for today's comments, else the date (as the reference).
fn shown_date(raw: &str) -> String {
    let (d, t) = split_date(raw);
    if !d.is_empty() && d == today() {
        t
    } else {
        d
    }
}

/// Attachment size as the reference lists it (whole KB, rounded down).
pub(crate) fn size_text(bytes: usize) -> String {
    format!("{} KB", bytes / 1024)
}

fn kind_icon(k: AnnotKind) -> &'static str {
    match k {
        AnnotKind::Highlight => "highlight",
        AnnotKind::Underline => "underline",
        AnnotKind::StrikeOut => "strikeout",
        AnnotKind::Squiggly => "squiggly",
        AnnotKind::TextBox => "textbox",
        AnnotKind::Square => "rectangle",
        AnnotKind::Circle => "ellipse",
        AnnotKind::Ink => "pencil",
        AnnotKind::Line => "line",
        AnnotKind::Stamp => "stamp",
        AnnotKind::Note | AnnotKind::Other => "commentList",
    }
}

/// Lines of `s` that fit `max` pixels at about `per` pixels a character (at most `limit`).
fn wrap(s: &str, per: f32, max: f32, limit: usize) -> Vec<String> {
    let cols = ((max / per.max(1.0)) as usize).max(4);
    let mut out: Vec<String> = Vec::new();
    for para in s.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let n = line.chars().count();
            if n > 0 && n + 1 + word.chars().count() > cols {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
            while line.chars().count() > cols {
                let head: String = line.chars().take(cols).collect();
                line = line.chars().skip(cols).collect();
                out.push(head);
            }
        }
        out.push(line);
    }
    if out.len() > limit {
        out.truncate(limit);
        if let Some(l) = out.last_mut() {
            l.push_str("...");
        }
    }
    out
}

impl ReaderWindow {
    /// Set the local time offset from UTC in minutes (dates of the comment list).
    pub fn set_local_offset(minutes: i32) {
        UTC_OFFSET.store(minutes, Ordering::Relaxed);
    }

    /// Logical pixels of a length measured at 125 %.
    fn q(&self, v125: f32) -> f32 {
        self.px(v125 / 1.25)
    }

    /// Whether the round 8 panels draw `panel` (BlueReader only; BlueEditor keeps its list).
    pub(crate) fn panel8(&self, panel: Panel) -> bool {
        self.profile.is_none()
            && matches!(panel, Panel::Layers | Panel::Attachments | Panel::Comments)
    }

    /// The panel a new document opens with: Attachments when it has embedded files or asks
    /// for them (`/PageMode /UseAttachments`), as the reference reader does (BlueReader only).
    pub(crate) fn first_panel8(&self, doc: &bo_pdfread::PdfDocument) -> Option<Panel> {
        (self.profile.is_none()
            && (doc.page_mode().as_deref() == Some("UseAttachments")
                || !doc.embedded_files().is_empty()))
        .then_some(Panel::Attachments)
    }

    /// Navigation column icons the round 8 panels add after Comments.
    pub(crate) fn nav8(&self) -> Vec<(&'static str, Panel)> {
        if self.profile.is_some() {
            return Vec::new();
        }
        let mut v = Vec::new();
        if self
            .tabs
            .get(self.active)
            .is_some_and(|t| t.view.doc().layer_count() > 0)
        {
            v.push(("nav.layers", Panel::Layers));
        }
        v.push(("nav.attachments", Panel::Attachments));
        v
    }

    /// Comment threads with state replies taken out (they show as the comment's status).
    fn threads8(&self) -> Shown {
        let Some(t) = self.tabs.get(self.active) else {
            return Vec::new();
        };
        let doc = t.view.doc();
        doc.comment_threads()
            .into_iter()
            .filter(|th| !doc.is_state_reply(&th.comment))
            .map(|mut th| {
                th.replies.retain(|r| !doc.is_state_reply(r));
                let s = doc.comment_state(&th.comment);
                (th, s)
            })
            .collect()
    }

    /// The comments the list shows (search and filter), in sort order.
    pub(crate) fn comments8(&self) -> Shown {
        let q = self.panels8.search.to_lowercase();
        let f = &self.panels8.filter;
        let mut list: Vec<_> = self
            .threads8()
            .into_iter()
            .filter(|(th, s)| {
                let a = &th.comment;
                let text_hit = q.is_empty()
                    || a.contents.to_lowercase().contains(&q)
                    || a.author.to_lowercase().contains(&q)
                    || th
                        .replies
                        .iter()
                        .any(|r| r.contents.to_lowercase().contains(&q));
                let review = s.review.clone().unwrap_or_else(|| "None".into());
                text_hit
                    && (f.status.is_empty() || f.status.contains(&review))
                    && (f.authors.is_empty() || f.authors.contains(&a.author))
                    && (f.kinds.is_empty() || f.kinds.contains(&a.kind))
                    && f.checked.is_none_or(|c| c == s.marked)
            })
            .collect();
        let sort = self.panels8.sort;
        list.sort_by(|(a, sa), (b, sb)| {
            let ka = group_of(sort, &a.comment, sa, self).0;
            let kb = group_of(sort, &b.comment, sb, self).0;
            ka.cmp(&kb)
                .then(a.comment.page.cmp(&b.comment.page))
                .then(a.comment.rect[1].total_cmp(&b.comment.rect[1]))
        });
        list
    }

    fn group_open(&self, key: &str, first: bool) -> bool {
        match self.panels8.all_open {
            Some(all) => all != self.panels8.toggled.contains(key),
            None => first != self.panels8.toggled.contains(key),
        }
    }

    fn tool8(&mut self, p: &mut dyn Painter, id: &'static str, icon: &str, b: [f32; 4], on: bool) {
        if on && self.panels8.hover == Some(id) {
            p.path(&rounded(b[0], b[1], b[2], b[3], 2.0), Some(BAND), None);
        }
        // Measured on the reference: 19–20 px icons at 125 %, stroke core 53, 53, 53.
        let is = b[2] - 2.0;
        let ink = if on { TOOL } else { DIM };
        reader_icons::draw_weight(
            p,
            icon,
            b[0] + 1.0,
            b[1] + 1.0,
            is,
            Ink { ink, accent: ink },
            2.0,
        );
        self.panels8.tools.push((id, b, on));
    }

    /// Author names: the interface font at weight 700 like the reference's (the shared bold
    /// is 600), centred on `cy`.
    #[allow(clippy::unused_self)]
    fn bold8(&self, p: &mut dyn Painter, s: &str, x: f32, cy: f32, st: TextStyle) {
        let f = crate::Font {
            family: "",
            size: st.size,
            weight: 700,
            italic: false,
            underline: false,
        };
        p.text_font(s, x, (cy - st.size * 1.33 / 2.0).round(), &f, INK);
    }

    /// Text centred on `cy`.
    #[allow(clippy::unused_self)]
    fn line8(&self, p: &mut dyn Painter, s: &str, x: f32, cy: f32, st: TextStyle, c: Rgba) {
        p.text(s, x, (cy - st.size * 1.33 / 2.0).round(), st, c);
    }

    /// The selected navigation icon as the reference draws it (125 %): a tab 56 px high from
    /// x 15 in the panel's colour, framed left / top / bottom in 199, 204, 217 and open
    /// towards the panel.
    pub(crate) fn paint_nav_tab8(&self, p: &mut dyn Painter, r: [f32; 4]) {
        let t = self.nav_tab8(r);
        p.path(
            &rounded(
                t[0] + 0.5,
                t[1] + 0.5,
                t[2] + self.q(8.0),
                t[3] - 1.0,
                self.q(3.0),
            ),
            Some(self.look.chrome),
            Some((FRAME, 1.0)),
        );
    }

    fn nav_tab8(&self, r: [f32; 4]) -> [f32; 4] {
        let cy = r[1] + r[3] / 2.0;
        let x = self.q(15.0);
        let top = (cy - self.q(25.0)).round();
        let right = self.nav_rect[0] + self.nav_rect[2];
        [x, top, right - x, self.q(56.0)]
    }

    /// Red mark on the Attachments icon when the document has attachments (the reference's
    /// badge: 7 px, 226, 63, 54, up and right of the icon centre).
    pub(crate) fn paint_nav_badge8(&self, p: &mut dyn Painter, r: [f32; 4]) {
        let has = self.tabs.get(self.active).is_some_and(|t| {
            let d = t.view.doc();
            !d.embedded_files().is_empty() || (d.page_count() <= 300 && !d.attachments().is_empty())
        });
        if self.profile.is_none() && has {
            let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
            p.path(
                &circle(cx + self.q(11.5), cy - self.q(9.5), self.q(3.4)),
                Some(Rgba(226, 63, 54, 255)),
                None,
            );
        }
    }

    /// The panel's left edge (1 px, 199, 204, 217) except where the selected tab joins it.
    pub(crate) fn paint_panel_edge8(&self, p: &mut dyn Painter) {
        if self.profile.is_some() {
            return;
        }
        let r = self.panel_rect;
        let open = self.panel_open();
        let t = self
            .controls
            .iter()
            .find(|c| matches!(c.kind, super::Kind::Nav(n) if Some(n) == open))
            .map_or([0.0; 4], |c| self.nav_tab8(c.r));
        let x = r[0] - 1.0;
        if t[3] > 0.0 && t[1] > r[1] {
            p.fill_rect(x, r[1], 1.0, t[1] - r[1] + 1.0, FRAME);
            let below = t[1] + t[3] - 1.0;
            p.fill_rect(x, below, 1.0, r[1] + r[3] - below, FRAME);
            // The tab's inside runs into the panel.
            p.fill_rect(x, t[1] + 1.0, 1.0, t[3] - 2.0, self.look.chrome);
        } else {
            p.fill_rect(x, r[1], 1.0, r[3], FRAME);
        }
    }

    /// Full Screen's exit button (measured on the reference at 125 %: 31 x 29 px, 94 px from
    /// the right edge, 10 px from the top; our own corner marks, grey on the black).
    fn full_exit8(&self) -> [f32; 4] {
        let (w, _) = self.size;
        [w - self.q(125.0), self.q(10.0), self.q(31.0), self.q(29.0)]
    }

    pub(crate) fn paint_full_exit8(&self, p: &mut dyn Painter) {
        if self.profile.is_some() {
            return;
        }
        let b = self.full_exit8();
        p.fill_rect(b[0], b[1], b[2], b[3], Rgba(20, 20, 20, 255));
        let s = self.q(18.0);
        let grey = Rgba(150, 150, 152, 255);
        reader_icons::draw_weight(
            p,
            "exitFull",
            b[0] + ((b[2] - s) / 2.0).round(),
            b[1] + ((b[3] - s) / 2.0).round(),
            s,
            Ink {
                ink: grey,
                accent: grey,
            },
            2.2,
        );
    }

    /// Search options: (match case, whole words only).
    pub(crate) fn find_options8(&self) -> (bool, bool) {
        (self.panels8.find_case, self.panels8.find_word)
    }

    /// The options button at the right end of the title bar's search box.
    fn find_button8(&self) -> [f32; 4] {
        let s = self.search;
        let w = self.q(26.0);
        [s[0] + s[2] - w - 1.0, s[1] + 1.0, w, s[3] - 2.0]
    }

    fn paint_find_button8(&self, p: &mut dyn Painter) {
        if self.profile.is_some() || self.search[2] <= 0.0 {
            return;
        }
        let b = self.find_button8();
        let on = self.panels8.find_case || self.panels8.find_word;
        if on {
            p.path(
                &rounded(b[0], b[1] + 2.0, b[2] - 2.0, b[3] - 4.0, 2.0),
                Some(BAND),
                None,
            );
        }
        let s = self.q(12.0);
        reader_icons::draw(
            p,
            "chevronDown",
            b[0] + ((b[2] - s) / 2.0).round(),
            b[1] + ((b[3] - s) / 2.0).round(),
            s,
            Ink {
                ink: SOFT,
                accent: SOFT,
            },
        );
    }

    /// Draw a round 8 panel below its title (panel rectangle `r`).
    pub(crate) fn paint_panel8(&mut self, p: &mut dyn Painter, r: [f32; 4], panel: Panel) {
        self.panels8.tools.clear();
        self.panels8.rows.clear();
        self.panels8.search_box = [0.0; 4];
        self.panels8.funnel = [0.0; 4];
        match panel {
            Panel::Layers => self.paint_layers8(p, r),
            Panel::Attachments => self.paint_attachments8(p, r),
            _ => self.paint_comments8(p, r),
        }
    }

    fn paint_layers8(&mut self, p: &mut dyn Painter, r: [f32; 4]) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let rows = t.view.doc().layers();
        let scroll = t.panel_scroll;
        let top = r[1] + self.q(57.0);
        p.fill_rect(r[0] + self.q(10.0), top, r[2] - self.q(19.0), 1.0, BAND);
        let list_top = top + 1.0;
        p.push_clip(r[0], list_top, r[2], r[3] - (list_top - r[1]));
        let body = self.style(13.0, false);
        let pitch = self.q(24.0);
        let mut y = r[1] + self.q(66.0) - scroll;
        for row in rows {
            let x = r[0] + self.q(8.0) + self.q(22.0) * row.depth as f32;
            let cy = y + pitch / 2.0;
            let b = [r[0], y, r[2], pitch];
            if let Some(g) = row.group {
                if self.panels8.layer_selected == Some(g) {
                    p.fill_rect(b[0], b[1], b[2], b[3], SELECTED);
                }
                // Visibility box: our own eye mark inside a square.
                let s = self.q(16.0);
                let bx = [x, (cy - s / 2.0).round(), s, s];
                let edge = if row.locked { DIM } else { TEXT };
                p.path(
                    &rounded(bx[0], bx[1], bx[2], bx[3], 2.0),
                    Some(WHITE),
                    Some((edge, 1.0)),
                );
                if row.on {
                    let (ex, ey, ew) = (bx[0] + s / 2.0, bx[1] + s / 2.0, s * 0.36);
                    p.path(
                        &[
                            PathSeg::M(ex - ew, ey),
                            PathSeg::Q(ex, ey - ew * 1.1, ex + ew, ey),
                            PathSeg::Q(ex, ey + ew * 1.1, ex - ew, ey),
                            PathSeg::Z,
                        ],
                        None,
                        Some((edge, 1.0)),
                    );
                    let d = s * 0.11;
                    p.path(&circle(ex, ey, d), Some(self.theme.accent), None);
                }
                let label = row.label.clone();
                let c = if row.locked { DIM } else { TEXT };
                self.line8(p, &label, x + s + self.q(8.0), cy, body, c);
                self.panels8.rows.push((b, Hit::Layer(g)));
            } else {
                let label = row.label.clone();
                self.line8(p, &label, x, cy, self.style(13.0, true), TEXT);
            }
            y += pitch;
        }
        p.pop_clip();
    }

    fn paint_attachments8(&mut self, p: &mut dyn Painter, r: [f32; 4]) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let list = t.view.doc().attachments();
        let scroll = t.panel_scroll;
        let sel = self.panels8.attach_selected.filter(|i| *i < list.len());
        // Tools: open, save (enabled with a selection), add.
        let is = self.q(22.0);
        let cy = r[1] + self.q(52.5);
        for (k, (id, icon, on)) in [
            ("att.open", "open", sel.is_some()),
            ("att.save", "save", sel.is_some()),
            ("att.add", "attachAdd", true),
        ]
        .into_iter()
        .enumerate()
        {
            let cx = r[0] + self.q(20.5 + 32.0 * k as f32);
            let b = [(cx - is / 2.0).round(), (cy - is / 2.0).round(), is, is];
            self.tool8(p, id, icon, b, on);
        }
        p.fill_rect(
            r[0] + self.q(10.0),
            r[1] + self.q(72.0),
            r[2] - self.q(19.0),
            1.0,
            BAND,
        );
        // Column header band.
        let hy = r[1] + self.q(90.0);
        let hh = self.q(24.0);
        p.fill_rect(r[0], hy, r[2], hh, HEADER);
        let col = self.q(125.0);
        for k in 1..=2 {
            p.fill_rect(r[0] + col * k as f32, hy, 1.0, hh, FRAME);
        }
        let hs = self.style(14.0, false);
        let hc = hy + hh / 2.0;
        for (k, key) in ["pdfPanels.name", "pdfPanels.description", "pdfPanels.size"]
            .into_iter()
            .enumerate()
        {
            let label = self.t(key);
            let x = r[0] + col * k as f32 + self.q(if k == 0 { 8.0 } else { 7.0 });
            self.line8(p, &label, x, hc, hs, TEXT);
        }
        let list_top = hy + hh;
        p.push_clip(r[0], list_top, r[2], r[3] - (list_top - r[1]));
        let rs = self.style(12.5, false);
        let pitch = self.q(21.0);
        let mut y = list_top + self.q(1.0) - scroll;
        for (i, a) in list.iter().enumerate() {
            let b = [r[0], y, r[2], pitch];
            if sel == Some(i) {
                p.fill_rect(b[0], b[1], b[2], b[3], SELECTED);
            }
            let cy = y + pitch / 2.0;
            let cells = [a.name.clone(), a.description.clone(), size_text(a.size)];
            for (k, s) in cells.iter().enumerate() {
                let x = r[0] + col * k as f32 + self.q(7.0);
                let w = if k == 2 {
                    r[2] - col * 2.0 - self.q(8.0)
                } else {
                    col - self.q(10.0)
                };
                let shown = super::elide(s, self.width_of(s, rs), w, rs.size);
                self.line8(p, &shown, x, cy, rs, TEXT);
            }
            self.panels8.rows.push((b, Hit::Attachment(i)));
            y += pitch;
        }
        p.pop_clip();
    }

    fn paint_comments8(&mut self, p: &mut dyn Painter, r: [f32; 4]) {
        let list = self.comments8();
        let total = list.len();
        let scroll = self.tabs.get(self.active).map_or(0.0, |t| t.panel_scroll);
        // Tools.
        let is = self.q(22.0);
        let cy = r[1] + self.q(61.0);
        let selected = self.extra.selected.is_some();
        for (k, (id, icon, on)) in [
            ("cm.expandAll", "expandAll", true),
            ("cm.collapseAll", "collapseAll", true),
            ("cm.prev", "prevItem", selected),
            ("cm.next", "nextItem", selected),
            ("cm.sort", "sortAZ", true),
            ("cm.notes", "commentList", selected),
        ]
        .into_iter()
        .enumerate()
        {
            let cx = r[0] + self.q(20.5 + 31.0 * k as f32);
            let b = [(cx - is / 2.0).round(), (cy - is / 2.0).round(), is, is];
            self.tool8(p, id, icon, b, on);
        }
        // Search box and filter button.
        let sb = [
            r[0] + self.q(11.0),
            r[1] + self.q(86.0),
            r[2] - self.q(79.0),
            self.q(25.0),
        ];
        let edge = if self.panels8.search_focus {
            self.theme.accent
        } else {
            BOX
        };
        p.fill_rect(sb[0], sb[1], sb[2], sb[3], edge);
        p.fill_rect(sb[0] + 1.0, sb[1] + 1.0, sb[2] - 2.0, sb[3] - 2.0, WHITE);
        let ms = self.q(17.0);
        reader_icons::draw(
            p,
            "search",
            sb[0] + self.q(4.0),
            sb[1] + ((sb[3] - ms) / 2.0).round(),
            ms,
            Ink {
                ink: TEXT,
                accent: TEXT,
            },
        );
        let st = self.style(13.0, false);
        let tx = sb[0] + self.q(25.0);
        p.push_clip(sb[0], sb[1], sb[2], sb[3]);
        if self.panels8.search.is_empty() {
            let hint = self.t("pdfPanels.search");
            self.line8(p, &hint, tx, sb[1] + sb[3] / 2.0, st, DIM);
        } else {
            let q = self.panels8.search.clone();
            self.line8(p, &q, tx, sb[1] + sb[3] / 2.0, st, TEXT);
        }
        if self.panels8.search_focus {
            let q = self.panels8.search.clone();
            let w = self.width_of(&q, st);
            p.fill_rect((tx + w).round(), sb[1] + 4.0, 1.0, sb[3] - 8.0, TEXT);
        }
        p.pop_clip();
        self.panels8.search_box = sb;
        let fs = self.q(22.0);
        let fb = [
            (r[0] + r[2] - self.q(45.0) - fs / 2.0).round(),
            (sb[1] + (sb[3] - fs) / 2.0).round(),
            fs,
            fs,
        ];
        let filtering = !self.panels8.filter.is_empty();
        if filtering || self.panels8.hover == Some("cm.filter") {
            p.path(&rounded(fb[0], fb[1], fb[2], fb[3], 2.0), Some(BAND), None);
        }
        reader_icons::draw_weight(
            p,
            "funnel",
            fb[0] + 1.0,
            fb[1] + 1.0,
            fs - 2.0,
            Ink {
                ink: TOOL,
                accent: self.theme.accent,
            },
            1.8,
        );
        self.panels8.funnel = fb;
        let small = self.style(13.0, false);
        let label = format!("{} {total}", self.t("pdfPanels.total"));
        self.line8(
            p,
            &label,
            r[0] + self.q(11.0),
            r[1] + self.q(130.0),
            small,
            TEXT,
        );
        // Groups and cards.
        let list_top = r[1] + self.q(142.0);
        p.push_clip(r[0], list_top, r[2], r[3] - (list_top - r[1]));
        let mut y = r[1] + self.q(150.0) - scroll;
        let cx0 = r[0] + self.q(11.0);
        let cw = r[2] - self.q(23.0);
        let sort = self.panels8.sort;
        let mut groups: Vec<(String, String, Shown)> = Vec::new();
        for (th, s) in list {
            let (key, label) = group_of(sort, &th.comment, &s, self);
            match groups.last_mut() {
                Some(g) if g.0 == key => g.2.push((th, s)),
                _ => groups.push((key, label, vec![(th, s)])),
            }
        }
        let gs = self.style(12.5, false);
        let author = self.style(13.5, true);
        let body = self.style(13.5, false);
        let date = self.style(12.5, false);
        let bottom = r[1] + r[3];
        for (gi, (key, label, items)) in groups.into_iter().enumerate() {
            let open = self.group_open(&key, gi == 0);
            let gh = self.q(34.0);
            if y < bottom && y + gh > list_top {
                p.path(&rounded(cx0, y, cw, gh, 2.0), Some(BAND), None);
                let gc = y + gh / 2.0;
                if sort == SortBy::Color {
                    let c = items[0].0.comment.color;
                    let sw = self.q(14.0);
                    p.fill_rect(
                        cx0 + self.q(7.0),
                        (gc - sw / 2.0).round(),
                        sw,
                        sw,
                        Rgba(c[0], c[1], c[2], 255),
                    );
                } else {
                    self.line8(p, &label, cx0 + self.q(7.0), gc, gs, TEXT);
                }
                let n = items.len().to_string();
                let nw = self.width_of(&n, gs);
                self.line8(p, &n, cx0 + cw - self.q(36.0) - nw, gc, gs, TEXT);
                let ch = self.q(12.0);
                reader_icons::draw(
                    p,
                    if open { "chevronDown" } else { "chevronRight" },
                    cx0 + cw - self.q(27.0),
                    (gc - ch / 2.0).round(),
                    ch,
                    Ink {
                        ink: SOFT,
                        accent: SOFT,
                    },
                );
            }
            self.panels8
                .rows
                .push(([cx0, y, cw, gh], Hit::Group(key.clone())));
            y += gh;
            if !open {
                y += self.q(13.0);
                continue;
            }
            y += self.q(8.0);
            for (th, state) in items {
                let a = &th.comment;
                let text_w = cw - self.q(24.0);
                let per = body.size * 0.52;
                // Lines below the header row, laid out by content like the reference's cards
                // (centres 125 %: text 27 under the author, 20 a line, date 22.5 under the text,
                // reply author 37.5 under the date, its text 26.5, its date 23).
                let mut items8: Vec<(f32, f32, String, u8)> = Vec::new();
                let mut cur = self.q(23.5);
                let text_x = cx0 + self.q(12.0);
                let date_x = cx0 + cw - self.q(15.0);
                let mut first = true;
                for l in wrap(&a.contents, per, text_w, 4)
                    .into_iter()
                    .filter(|l| !l.is_empty())
                {
                    cur += if first { self.q(27.0) } else { self.q(20.0) };
                    first = false;
                    items8.push((cur, text_x, l, 1));
                }
                let d = shown_date(&a.modified);
                if !d.is_empty() {
                    cur += if first { self.q(27.0) } else { self.q(22.5) };
                    items8.push((cur, date_x, d, 2));
                }
                // The selected card's header area (measured: to 18 px under the date) is grey.
                let head_end = cur + self.q(18.5);
                let is_sel = self
                    .extra
                    .selected
                    .as_ref()
                    .is_some_and(|s| s.page == a.page && s.rect == a.rect);
                let replies: Vec<Annot> = th.replies.clone();
                for rp in &replies {
                    let rx = cx0 + self.q(37.0);
                    cur += self.q(37.5);
                    items8.push((cur, rx, rp.author.clone(), 0));
                    let mut rfirst = true;
                    for l in wrap(&rp.contents, per, text_w - self.q(25.0), 4)
                        .into_iter()
                        .filter(|l| !l.is_empty())
                    {
                        cur += if rfirst { self.q(26.5) } else { self.q(20.0) };
                        rfirst = false;
                        items8.push((cur, rx, l, 1));
                    }
                    let d = shown_date(&rp.modified);
                    if !d.is_empty() {
                        cur += self.q(23.0);
                        items8.push((cur, date_x, d, 2));
                    }
                }
                // Selected: an Add Reply box (30 px, 28 px under the last line, 6 px in).
                let reply_box = is_sel.then(|| {
                    let top = cur + self.q(25.0);
                    [cx0 + self.q(6.0), y + top, cw - self.q(12.0), self.q(30.0)]
                });
                let h = match reply_box {
                    Some(b) => b[1] - y + b[3] + self.q(13.0),
                    None => cur + self.q(17.5),
                };
                let card = [cx0, y, cw, h];
                if y < bottom && y + h > list_top {
                    p.path(
                        &rounded(
                            card[0] + 0.5,
                            card[1] + 0.5,
                            card[2] - 1.0,
                            card[3] - 1.0,
                            3.0,
                        ),
                        Some(super::CHROME),
                        Some((FRAME, 1.0)),
                    );
                    if is_sel {
                        p.path(
                            &rounded(
                                card[0] + 1.0,
                                card[1] + 1.0,
                                card[2] - 2.0,
                                head_end - 1.0,
                                2.5,
                            ),
                            Some(BAND),
                            None,
                        );
                        // Menu button (three dots) at the right of the header.
                        let mx = cx0 + cw - self.q(20.0);
                        let my = y + self.q(23.5);
                        for k in -1..=1 {
                            p.path(
                                &circle(mx, my + self.q(5.5) * k as f32, self.q(1.6)),
                                Some(Rgba(51, 51, 51, 255)),
                                None,
                            );
                        }
                        self.panels8.rows.push((
                            [
                                mx - self.q(9.0),
                                my - self.q(12.0),
                                self.q(18.0),
                                self.q(24.0),
                            ],
                            Hit::More,
                        ));
                    }
                    if let Some(b) = reply_box {
                        let focus = self.panels8.reply.is_some();
                        let edge = if focus { self.theme.accent } else { FRAME };
                        p.fill_rect(b[0], b[1], b[2], b[3], edge);
                        p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, WHITE);
                        let st = self.style(13.5, false);
                        let typed = self.panels8.reply.clone().unwrap_or_default();
                        let tx = b[0] + self.q(7.0);
                        if typed.is_empty() {
                            let hint = self.t("pdfPanels.addReply");
                            self.line8(p, &hint, tx, b[1] + b[3] / 2.0, st, DIM);
                        } else {
                            self.line8(p, &typed, tx, b[1] + b[3] / 2.0, st, INK);
                        }
                        if focus {
                            let w = self.width_of(&typed, st);
                            p.fill_rect((tx + w).round(), b[1] + 6.0, 1.0, b[3] - 12.0, INK);
                        }
                        self.panels8.rows.push((b, Hit::ReplyBox));
                    }
                    let ic = self.q(20.0);
                    let row1 = y + self.q(23.5);
                    let c = Rgba(a.color[0], a.color[1], a.color[2], 255);
                    reader_icons::draw_weight(
                        p,
                        kind_icon(a.kind),
                        cx0 + self.q(11.0),
                        (row1 - ic / 2.0).round(),
                        ic,
                        Ink {
                            ink: INK,
                            accent: c,
                        },
                        1.8,
                    );
                    let name = if a.author.is_empty() {
                        self.t(kind_label(a.kind))
                    } else {
                        a.author.clone()
                    };
                    let ax = cx0 + self.q(36.0);
                    self.bold8(p, &name, ax, row1, author);
                    // Status and checkmark after the author.
                    let mut sx = ax + self.width_of(&name, author) + self.q(8.0);
                    if let Some(rv) = &state.review {
                        let s = self.t(&format!("pdfPanels.state{rv}"));
                        let col = state_color(rv);
                        let sw = self.width_of(&s, date) + self.q(10.0);
                        let sh = self.q(18.0);
                        p.path(
                            &rounded(sx, (row1 - sh / 2.0).round(), sw, sh, sh / 2.0),
                            None,
                            Some((col, 1.0)),
                        );
                        self.line8(p, &s, sx + self.q(5.0), row1, date, col);
                        sx += sw + self.q(6.0);
                    }
                    if state.marked {
                        let ms = self.q(16.0);
                        reader_icons::draw(
                            p,
                            "check",
                            sx,
                            (row1 - ms / 2.0).round(),
                            ms,
                            Ink {
                                ink: self.theme.accent,
                                accent: self.theme.accent,
                            },
                        );
                    }
                    if !replies.is_empty() {
                        // Reply count in a bubble at the right (our own bubble shape).
                        let bs = self.q(20.0);
                        let shift = if is_sel { self.q(24.0) } else { 0.0 };
                        let bx = cx0 + cw - self.q(14.0) - bs - shift;
                        let by = (row1 - bs / 2.0).round();
                        // Our own bubble: a rounded box with a short tail, neutral grey.
                        let (r0, r1) = (bs / 2.0 - 1.0, bs * 0.42);
                        let (ccx, ccy) = (bx + bs / 2.0, by + bs / 2.0 - 1.0);
                        let grey = Rgba(110, 110, 112, 255);
                        p.path(
                            &rounded(ccx - r0, ccy - r1, 2.0 * r0, 2.0 * r1, r1 * 0.9),
                            Some(WHITE),
                            Some((grey, 1.0)),
                        );
                        p.path(
                            &[
                                PathSeg::M(ccx - r0 * 0.45, ccy + r1 - 0.5),
                                PathSeg::L(ccx - r0 * 0.6, ccy + r1 + bs * 0.18),
                                PathSeg::L(ccx - r0 * 0.05, ccy + r1 - 0.5),
                            ],
                            Some(WHITE),
                            Some((grey, 1.0)),
                        );
                        let n = replies.len().to_string();
                        let nw = self.width_of(&n, date);
                        self.line8(p, &n, bx + (bs - nw) / 2.0, row1 - 1.0, date, RED);
                    }
                    for (cy, x, text, kind) in &items8 {
                        let (st, x) = match kind {
                            0 => {
                                self.bold8(p, text, *x, y + cy, author);
                                continue;
                            }
                            1 => (body, *x),
                            _ => (date, *x - self.width_of(text, date)),
                        };
                        self.line8(p, text, x, y + cy, st, INK);
                    }
                }
                self.panels8.rows.push((card, Hit::Card(a.page, a.rect)));
                y += h + self.q(8.0);
            }
            y += self.q(5.0);
        }
        p.pop_clip();
    }

    /// Strings the panels draw, measured for alignment (the painter cannot measure).
    fn measure8(&mut self, m: &dyn TextMeasure) {
        let Some(panel) = self.panel_open() else {
            return;
        };
        if !self.panel8(panel) {
            return;
        }
        let mut words: Vec<(String, TextStyle)> = Vec::new();
        match panel {
            Panel::Attachments => {
                let rs = self.style(12.5, false);
                if let Some(t) = self.tabs.get(self.active) {
                    for a in t.view.doc().attachments() {
                        words.push((a.name, rs));
                        words.push((a.description, rs));
                    }
                }
            }
            Panel::Comments => {
                let date = self.style(12.5, false);
                let author = self.style(13.5, true);
                let gs = self.style(12.5, false);
                words.push((self.panels8.search.clone(), self.style(13.0, false)));
                for (th, s) in self.comments8() {
                    words.push((shown_date(&th.comment.modified), date));
                    words.push((th.comment.author.clone(), author));
                    words.push((th.replies.len().to_string(), date));
                    if let Some(rv) = &s.review {
                        words.push((self.t(&format!("pdfPanels.state{rv}")), date));
                    }
                    for r in &th.replies {
                        words.push((shown_date(&r.modified), date));
                    }
                }
                for n in 1..100 {
                    words.push((n.to_string(), gs));
                }
            }
            _ => {}
        }
        for (s, st) in words {
            self.measure(m, &s, st);
        }
    }

    /// Input of the round 8 panels (tools, rows, search box, filter dialog).
    pub(crate) fn route_panels8(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if let Input::Press { x, y, .. } = input {
            if self.profile.is_none() && self.full_screen8() && inside(self.full_exit8(), *x, *y) {
                self.toggle_full_screen(m);
                return Some(true);
            }
            if self.profile.is_none()
                && self.panels8.dialog.is_none()
                && self.panels8.confirm.is_none()
                && self.menu.is_none()
                && self.search[2] > 0.0
                && inside(self.find_button8(), *x, *y)
            {
                let item = |cmd: &str, key: &str, on: bool| MenuItem {
                    label: self.t(key),
                    choice: Choice::Command(cmd.to_owned()),
                    checked: on,
                    dim: false,
                };
                let items = vec![
                    item("find.word", "pdfPanels.wholeWords", self.panels8.find_word),
                    item("find.case", "pdfPanels.matchCase", self.panels8.find_case),
                ];
                let b = self.find_button8();
                self.menu_at(items, b, "find");
                return Some(true);
            }
        }
        if self.panels8.dialog.is_some() {
            return Some(self.route_filter_dialog(input, m));
        }
        if let Some((d, i)) = self.panels8.confirm.as_mut() {
            let i = *i;
            let done = match input {
                Input::Key(k, mods) => d.key(*k, *mods),
                Input::Press { x, y, .. } => d.press(*x, *y),
                Input::Move { x, y } => {
                    d.hover(*x, *y);
                    None
                }
                Input::Resize { .. } | Input::Scale(_) => return None,
                _ => None,
            };
            if let Some(b) = done {
                self.panels8.confirm = None;
                if b == DialogButton::Ok {
                    self.open_attachment(i, true);
                }
            }
            return Some(true);
        }
        let panel = self.panel_open().filter(|p| self.panel8(*p));
        let Some(panel) = panel else {
            self.panels8.search_focus = false;
            return None;
        };
        self.measure8(m);
        match input {
            Input::Move { x, y } => {
                let mut h = self
                    .panels8
                    .tools
                    .iter()
                    .find(|t| t.2 && inside(t.1, *x, *y))
                    .map(|t| t.0);
                if h.is_none() && inside(self.panels8.funnel, *x, *y) {
                    h = Some("cm.filter");
                }
                if h != self.panels8.hover {
                    self.panels8.hover = h;
                }
                None
            }
            Input::Press { x, y, count, .. } => {
                let (x, y) = (*x, *y);
                if !inside(self.panel_rect, x, y) {
                    self.panels8.search_focus = false;
                    return None;
                }
                if let Some(t) = self.panels8.tools.iter().find(|t| inside(t.1, x, y)) {
                    let (id, on) = (t.0, t.2);
                    if on {
                        self.panel_command8(id, m);
                    }
                    return Some(true);
                }
                if panel == Panel::Comments {
                    if inside(self.panels8.search_box, x, y) {
                        self.panels8.search_focus = true;
                        self.search_focus = false;
                        return Some(true);
                    }
                    if inside(self.panels8.funnel, x, y) {
                        self.open_filter_dialog(m);
                        return Some(true);
                    }
                }
                self.panels8.search_focus = false;
                let hit = self
                    .panels8
                    .rows
                    .iter()
                    .find(|(b, _)| inside(*b, x, y))
                    .map(|r| r.1.clone());
                match hit {
                    Some(Hit::Layer(g)) => {
                        self.panels8.layer_selected = Some(g);
                        self.toggle_layer(g);
                    }
                    Some(Hit::Attachment(i)) => {
                        self.panels8.attach_selected = Some(i);
                        if *count >= 2 {
                            self.ask_open_attachment(i, m);
                        } else {
                            self.go_to_attachment(i);
                        }
                    }
                    Some(Hit::Group(k)) => {
                        if !self.panels8.toggled.remove(&k) {
                            self.panels8.toggled.insert(k);
                        }
                    }
                    Some(Hit::Card(page, rect)) => {
                        self.panels8.reply = None;
                        self.select_comment8(page, rect);
                    }
                    Some(Hit::More) => {
                        let items = self.comment_menu8();
                        self.menu_at(items, [x, y, 0.0, 0.0], "comment");
                    }
                    Some(Hit::ReplyBox) => {
                        if self.panels8.reply.is_none() {
                            self.panels8.reply = Some(String::new());
                        }
                        self.search_focus = false;
                    }
                    None => {}
                }
                Some(true)
            }
            Input::Context { x, y } if inside(self.panel_rect, *x, *y) => {
                let hit = self
                    .panels8
                    .rows
                    .iter()
                    .find(|(b, _)| inside(*b, *x, *y))
                    .map(|r| r.1.clone());
                if let Some(Hit::Card(page, rect)) = hit {
                    self.select_comment8(page, rect);
                    let items = self.comment_menu8();
                    self.menu_at(items, [*x, *y, 0.0, 0.0], "comment");
                    return Some(true);
                }
                None
            }
            Input::Text(s) if self.panels8.reply.is_some() => {
                if let Some(r) = self.panels8.reply.as_mut() {
                    r.push_str(s);
                }
                let (q, st) = (
                    self.panels8.reply.clone().unwrap_or_default(),
                    self.style(13.5, false),
                );
                self.measure(m, &q, st);
                Some(true)
            }
            Input::Key(Key::Backspace, _) if self.panels8.reply.is_some() => {
                if let Some(r) = self.panels8.reply.as_mut() {
                    r.pop();
                }
                Some(true)
            }
            Input::Key(Key::Escape, _) if self.panels8.reply.is_some() => {
                self.panels8.reply = None;
                Some(true)
            }
            Input::Key(Key::Enter, _) if self.panels8.reply.is_some() => {
                self.add_reply8();
                Some(true)
            }
            Input::Text(s) if self.panels8.search_focus => {
                self.panels8.search.push_str(s);
                let (q, st) = (self.panels8.search.clone(), self.style(13.0, false));
                self.measure(m, &q, st);
                if let Some(t) = self.tabs.get_mut(self.active) {
                    t.panel_scroll = 0.0;
                }
                Some(true)
            }
            Input::Key(Key::Backspace, _) if self.panels8.search_focus => {
                self.panels8.search.pop();
                Some(true)
            }
            Input::Key(Key::Escape, _) if self.panels8.search_focus => {
                self.panels8.search_focus = false;
                self.panels8.search.clear();
                Some(true)
            }
            _ => None,
        }
    }

    /// Add the reply typed in the selected card's box (one undo step, saved like any edit).
    fn add_reply8(&mut self) {
        let text = self.panels8.reply.take().unwrap_or_default();
        let Some(a) = self.extra.selected.clone() else {
            return;
        };
        if text.trim().is_empty() {
            return;
        }
        let author = self.author.clone();
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        if let Err(e) = t
            .view
            .doc_mut()
            .reply_to_annotation(&a, &author, text.trim())
        {
            self.status = e;
        }
        self.after_comment_change(Some(a.rect), (0.0, 0.0));
    }

    fn select_comment8(&mut self, page: usize, rect: [f32; 4]) {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        t.view.go_to(page, Some((rect[1] - 40.0).max(0.0)));
        self.extra.selected = t
            .view
            .doc()
            .annotations()
            .into_iter()
            .find(|a| a.page == page && a.rect == rect);
    }

    /// The comment list's context menu (and the page's, [`Self::comment_menu8`] items).
    pub(crate) fn comment_menu8(&self) -> Vec<MenuItem> {
        let marked = self.extra.selected.as_ref().is_some_and(|a| {
            self.tabs
                .get(self.active)
                .is_some_and(|t| t.view.doc().comment_state(a).marked)
        });
        let item = |cmd: &str, label: String, checked: bool| MenuItem {
            label,
            choice: Choice::Command(cmd.to_owned()),
            checked,
            dim: false,
        };
        vec![
            item("comment.edit", self.t("pdfExtra.editText"), false),
            item("comment.reply", self.t("pdfExtra.reply"), false),
            item("comment.delete", self.t("pdfExtra.delete"), false),
            item("comment.setStatus", self.t("pdfPanels.setStatus"), false),
            item("comment.mark", self.t("pdfPanels.mark"), marked),
        ]
    }

    /// Commands of the round 8 panels and the comment status menu.
    pub(crate) fn command_panels8(&mut self, cmd: &str, m: &dyn TextMeasure) -> bool {
        if let Some(s) = cmd.strip_prefix("comment.state.") {
            self.set_state8("Review", s);
            return true;
        }
        if let Some(k) = cmd
            .strip_prefix("cm.sortBy.")
            .and_then(|v| v.parse::<usize>().ok())
        {
            if let Some((s, _)) = SORTS.get(k) {
                self.panels8.sort = *s;
                self.panels8.toggled.clear();
                self.panels8.all_open = None;
            }
            return true;
        }
        match cmd {
            "find.word" | "find.case" => {
                if cmd == "find.word" {
                    self.panels8.find_word = !self.panels8.find_word;
                } else {
                    self.panels8.find_case = !self.panels8.find_case;
                }
                // The next Enter searches again with the new options.
                self.status = String::new();
            }
            "nav.layers" | "panel.layers" => self.toggle_panel(Panel::Layers),
            "nav.attachments" | "panel.attachments" => self.toggle_panel(Panel::Attachments),
            "comment.setStatus" => {
                let current = self
                    .extra
                    .selected
                    .as_ref()
                    .and_then(|a| {
                        self.tabs
                            .get(self.active)
                            .map(|t| t.view.doc().comment_state(a).review)
                    })
                    .flatten()
                    .unwrap_or_else(|| "None".into());
                let items = REVIEW_STATES
                    .iter()
                    .map(|s| MenuItem {
                        label: self.t(&format!("pdfPanels.state{s}")),
                        choice: Choice::Command(format!("comment.state.{s}")),
                        checked: *s == current,
                        dim: false,
                    })
                    .collect();
                let at = [self.pointer.0, self.pointer.1, 0.0, 0.0];
                self.menu_at(items, at, "status");
            }
            "comment.mark" => {
                let marked = self.extra.selected.as_ref().is_some_and(|a| {
                    self.tabs
                        .get(self.active)
                        .is_some_and(|t| t.view.doc().comment_state(a).marked)
                });
                self.set_state8("Marked", if marked { "Unmarked" } else { "Marked" });
            }
            "cm.filter" => self.open_filter_dialog(m),
            _ => return false,
        }
        true
    }

    fn panel_command8(&mut self, id: &str, m: &dyn TextMeasure) {
        match id {
            "cm.expandAll" => {
                self.panels8.all_open = Some(true);
                self.panels8.toggled.clear();
            }
            "cm.collapseAll" => {
                self.panels8.all_open = Some(false);
                self.panels8.toggled.clear();
            }
            "cm.prev" | "cm.next" => self.step_comment8(id == "cm.next"),
            "cm.sort" => {
                let items = SORTS
                    .iter()
                    .enumerate()
                    .map(|(k, (s, key))| MenuItem {
                        label: self.t(key),
                        choice: Choice::Command(format!("cm.sortBy.{k}")),
                        checked: *s == self.panels8.sort,
                        dim: false,
                    })
                    .collect();
                let b = self
                    .panels8
                    .tools
                    .iter()
                    .find(|t| t.0 == "cm.sort")
                    .map_or([0.0; 4], |t| t.1);
                self.menu_at(items, b, "sort");
            }
            "att.open" => {
                if let Some(i) = self.panels8.attach_selected {
                    self.ask_open_attachment(i, m);
                }
            }
            "att.save" => {
                if let Some(i) = self.panels8.attach_selected {
                    self.save_attachment(i);
                }
            }
            "att.add" => self.command("comment.file", m),
            // Open the selected comment's text (the sixth tool of the reference's row).
            "cm.notes" => self.command("comment.edit", m),
            _ => {}
        }
    }

    /// Select the previous / next comment of the list (in its order).
    fn step_comment8(&mut self, next: bool) {
        let list: Vec<(usize, [f32; 4])> = self
            .comments8()
            .into_iter()
            .map(|(t, _)| (t.comment.page, t.comment.rect))
            .collect();
        if list.is_empty() {
            return;
        }
        let cur = self
            .extra
            .selected
            .as_ref()
            .and_then(|a| list.iter().position(|(p, r)| *p == a.page && *r == a.rect));
        let k = match cur {
            Some(c) if next => (c + 1).min(list.len() - 1),
            Some(c) => c.saturating_sub(1),
            None => 0,
        };
        let (page, rect) = list[k];
        // The comment's group opens.
        self.panels8.all_open = Some(true);
        self.panels8.toggled.clear();
        self.select_comment8(page, rect);
    }

    fn set_state8(&mut self, model: &str, state: &str) {
        let Some(a) = self.extra.selected.clone() else {
            return;
        };
        let author = self.author.clone();
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        if let Err(e) = t
            .view
            .doc_mut()
            .set_comment_state(&a, model, state, &author)
        {
            self.status = e;
        }
        // State replies are hidden: the page itself does not change.
        self.status = String::new();
    }

    fn toggle_layer(&mut self, g: usize) {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let Some(row) = t
            .view
            .doc()
            .layers()
            .into_iter()
            .find(|r| r.group == Some(g))
        else {
            return;
        };
        if row.locked {
            return;
        }
        t.view.doc_mut().set_layer_on(g, !row.on);
        t.view.document_changed();
    }

    fn go_to_attachment(&mut self, i: usize) {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let list = t.view.doc().attachments();
        if let Some(a) = list.get(i) {
            if let (Some(page), Some(r)) = (a.page, a.rect) {
                t.view.go_to(page, Some((r[1] - 40.0).max(0.0)));
            }
        }
    }

    fn attachment(&self, i: usize) -> Option<(String, Vec<u8>)> {
        let t = self.tabs.get(self.active)?;
        let doc = t.view.doc();
        let a = doc.attachments().into_iter().nth(i)?;
        let bytes = doc.attachment_bytes(&a).ok()?;
        Some((a.name, bytes))
    }

    fn save_attachment(&mut self, i: usize) {
        if let Some((name, bytes)) = self.attachment(i) {
            self.requests.push(ReaderRequest::SaveBytes { name, bytes });
        }
    }

    /// Ask before opening an attachment (the reference warns too); programs are only saved.
    fn ask_open_attachment(&mut self, i: usize, m: &dyn TextMeasure) {
        let Some((name, _)) = self.attachment(i) else {
            return;
        };
        let ext = name
            .rsplit_once('.')
            .map(|(_, e)| e.to_lowercase())
            .unwrap_or_default();
        if ext == "pdf" {
            self.open_attachment(i, true);
            return;
        }
        if !SAFE_TYPES.contains(&ext.as_str()) {
            self.status = self.t("pdfPanels.saveOnly");
            self.save_attachment(i);
            return;
        }
        let mut d = Dialog::new(
            DialogKind::WordCount,
            self.t("pdfPanels.openTitle"),
            Vec::new(),
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfPrint.cancel"), DialogButton::Cancel),
            ],
            vec![self.t("pdfPanels.openText").replace("{name}", &name)],
        );
        d.layout(m, self.size, self.scale);
        self.panels8.confirm = Some((d, i));
    }

    fn open_attachment(&mut self, i: usize, confirmed: bool) {
        let Some((name, bytes)) = self.attachment(i) else {
            return;
        };
        if name.to_lowercase().ends_with(".pdf") {
            self.requests
                .push(ReaderRequest::OpenDocument { name, bytes });
        } else if confirmed {
            self.requests
                .push(ReaderRequest::OpenAttachment { name, bytes });
        }
    }

    fn open_filter_dialog(&mut self, _m: &dyn TextMeasure) {
        let threads = self.threads8();
        let mut authors: Vec<String> = Vec::new();
        let mut kinds: Vec<AnnotKind> = Vec::new();
        for (th, _) in &threads {
            if !authors.contains(&th.comment.author) {
                authors.push(th.comment.author.clone());
            }
            if !kinds.contains(&th.comment.kind) {
                kinds.push(th.comment.kind);
            }
        }
        self.panels8.dialog = Some(FilterDialog {
            filter: self.panels8.filter.clone(),
            authors,
            kinds,
            hits: Vec::new(),
            frame: [0.0; 4],
        });
    }

    fn route_filter_dialog(&mut self, input: &Input, m: &dyn TextMeasure) -> bool {
        let Some(d) = self.panels8.dialog.as_mut() else {
            return false;
        };
        match input {
            Input::Key(Key::Escape, _) => self.panels8.dialog = None,
            Input::Key(Key::Enter, _) => {
                self.panels8.filter = d.filter.clone();
                self.panels8.dialog = None;
            }
            Input::Press { x, y, .. } => {
                let hit = d
                    .hits
                    .iter()
                    .find(|(_, b)| inside(*b, *x, *y))
                    .map(|h| h.0.clone());
                let Some(h) = hit else {
                    return true;
                };
                let f = &mut d.filter;
                if let Some(s) = h.strip_prefix("status:") {
                    toggle(&mut f.status, s.to_owned());
                } else if let Some(a) = h.strip_prefix("author:") {
                    toggle(&mut f.authors, a.to_owned());
                } else if let Some(k) = h
                    .strip_prefix("kind:")
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    if let Some(k) = d.kinds.get(k).copied() {
                        toggle(&mut f.kinds, k);
                    }
                } else if h == "checked" || h == "unchecked" {
                    let v = h == "checked";
                    f.checked = if f.checked == Some(v) { None } else { Some(v) };
                } else if h == "clear" {
                    *f = CommentFilter::default();
                } else if h == "ok" {
                    self.panels8.filter = f.clone();
                    self.panels8.dialog = None;
                    if let Some(t) = self.tabs.get_mut(self.active) {
                        t.panel_scroll = 0.0;
                    }
                } else if h == "cancel" {
                    self.panels8.dialog = None;
                }
            }
            _ => {}
        }
        // Button labels are measured for their widths.
        if self.panels8.dialog.is_some() {
            let st = self.style(14.0, false);
            let mut labels: Vec<String> = REVIEW_STATES
                .iter()
                .map(|s| self.t(&format!("pdfPanels.state{s}")))
                .collect();
            labels.extend(
                self.panels8
                    .dialog
                    .as_ref()
                    .map(|d| d.authors.clone())
                    .unwrap_or_default(),
            );
            for key in [
                "pdfPanels.checked",
                "pdfPanels.unchecked",
                "pdfPanels.clearAll",
                "pdfReader.ok",
                "pdfPrint.cancel",
            ] {
                labels.push(self.t(key));
            }
            for l in labels {
                self.measure(m, &l, st);
            }
        }
        true
    }

    /// The filter dialog over the window (BlueReader only).
    pub(crate) fn paint_panels8_overlay(&mut self, p: &mut dyn Painter) {
        if self.menu.is_none() {
            self.paint_find_button8(p);
        }
        if let Some((d, _)) = self.panels8.confirm.as_ref() {
            d.paint(p, &self.theme, self.size, false);
        }
        let Some(mut d) = self.panels8.dialog.take() else {
            return;
        };
        let (w, h) = self.size;
        p.fill_rect(0.0, 0.0, w, h, Rgba(0, 0, 0, 40));
        let dw = self.q(572.0);
        let dh = self.q(508.0);
        let x0 = ((w - dw) / 2.0).round();
        let y0 = ((h - dh) / 2.0).round();
        d.frame = [x0, y0, dw, dh];
        d.hits.clear();
        p.fill_rect(
            x0 - 1.0,
            y0 - 1.0,
            dw + 2.0,
            dh + 2.0,
            Rgba(120, 120, 122, 255),
        );
        p.fill_rect(x0, y0, dw, dh, super::CHROME);
        p.fill_rect(x0, y0, dw, self.q(32.0), Rgba(240, 241, 245, 255));
        let ts = self.style(14.0, false);
        let title = self.t("pdfPanels.filterTitle");
        self.line8(p, &title, x0 + self.q(7.0), y0 + self.q(16.0), ts, SOFT);
        let bs = self.style(14.0, false);
        let head = self.style(13.0, false);
        let mut y = y0 + self.q(66.0);
        let left = x0 + self.q(25.0);
        let right = x0 + dw - self.q(25.0);
        let bh = self.q(42.0);
        let button = |p: &mut dyn Painter,
                      w: &ReaderWindow,
                      hits: &mut Vec<(String, [f32; 4])>,
                      id: String,
                      label: &str,
                      x: f32,
                      y: f32,
                      on: bool|
         -> f32 {
            let lw = w.width_of(label, bs);
            let bw = (lw + w.q(34.0)).max(w.q(52.0));
            let fill = if on { BAND } else { WHITE };
            let edge = if on { w.theme.accent } else { FRAME };
            p.path(&rounded(x, y, bw, bh, 3.0), Some(fill), Some((edge, 1.0)));
            w.line8(p, label, x + (bw - lw) / 2.0, y + bh / 2.0, bs, TEXT);
            hits.push((id, [x, y, bw, bh]));
            bw
        };
        let sections: [Section; 4] = [
            (
                "pdfPanels.status",
                REVIEW_STATES
                    .iter()
                    .map(|s| {
                        (
                            format!("status:{s}"),
                            self.t(&format!("pdfPanels.state{s}")),
                            d.filter.status.iter().any(|v| v == s),
                        )
                    })
                    .collect(),
            ),
            (
                "pdfPanels.author",
                d.authors
                    .iter()
                    .map(|a| {
                        (
                            format!("author:{a}"),
                            a.clone(),
                            d.filter.authors.contains(a),
                        )
                    })
                    .collect(),
            ),
            (
                "pdfPanels.type",
                d.kinds
                    .iter()
                    .enumerate()
                    .map(|(k, kind)| {
                        (
                            format!("kind:{k}"),
                            self.t(kind_label(*kind)),
                            d.filter.kinds.contains(kind),
                        )
                    })
                    .collect(),
            ),
            (
                "pdfPanels.checkmark",
                vec![
                    (
                        "checked".to_owned(),
                        self.t("pdfPanels.checked"),
                        d.filter.checked == Some(true),
                    ),
                    (
                        "unchecked".to_owned(),
                        self.t("pdfPanels.unchecked"),
                        d.filter.checked == Some(false),
                    ),
                ],
            ),
        ];
        for (key, buttons) in sections {
            let label = self.t(key).to_uppercase();
            self.line8(p, &label, left, y, head, TEXT);
            y += self.q(22.0);
            let mut x = left;
            for (id, label, on) in buttons {
                let lw = self.width_of(&label, bs) + self.q(34.0);
                if x + lw > right && x > left {
                    x = left;
                    y += bh + self.q(10.0);
                }
                x += button(p, self, &mut d.hits, id, &label, x, y, on) + self.q(15.0);
            }
            y += bh + self.q(30.0);
        }
        // Footer: Clear All at the left, OK / Cancel at the right.
        let fy = y0 + dh - self.q(56.0);
        p.fill_rect(x0, fy - self.q(30.0), dw, 1.0, FRAME);
        let clear = self.t("pdfPanels.clearAll");
        let cw = self.width_of(&clear, bs);
        self.line8(
            p,
            &clear,
            x0 + self.q(57.0),
            fy + self.q(19.0),
            bs,
            self.theme.accent,
        );
        d.hits.push((
            "clear".into(),
            [x0 + self.q(50.0), fy, cw + self.q(14.0), self.q(38.0)],
        ));
        let bw = self.q(120.0);
        for (k, (id, key)) in [("ok", "pdfReader.ok"), ("cancel", "pdfPrint.cancel")]
            .into_iter()
            .enumerate()
        {
            let bx = x0 + dw - self.q(17.0) - bw * (2 - k) as f32 - self.q(8.0) * (1 - k) as f32;
            let b = [bx, fy, bw, self.q(38.0)];
            p.path(
                &rounded(b[0], b[1], b[2], b[3], 3.0),
                Some(WHITE),
                Some((FRAME, 1.0)),
            );
            let label = self.t(key);
            let lw = self.width_of(&label, bs);
            self.line8(p, &label, bx + (bw - lw) / 2.0, fy + b[3] / 2.0, bs, TEXT);
            d.hits.push((id.into(), b));
        }
        self.panels8.dialog = Some(d);
    }
}

fn toggle<T: PartialEq>(v: &mut Vec<T>, x: T) {
    if let Some(i) = v.iter().position(|e| *e == x) {
        v.remove(i);
    } else {
        v.push(x);
    }
}

fn state_color(s: &str) -> Rgba {
    match s {
        "Accepted" => Rgba(16, 124, 16, 255),
        "Rejected" => Rgba(196, 43, 28, 255),
        "Completed" => Rgba(0, 95, 184, 255),
        _ => Rgba(110, 110, 112, 255),
    }
}

fn circle(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let k = 0.5523 * r;
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathSeg::Z,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader_tests::Fixed;
    use bo_pdfread::{NewAnnot, PdfDocument};

    #[derive(Default)]
    struct Texts(Vec<String>);

    impl Painter for Texts {
        fn clear(&mut self, _: Rgba) {}
        fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba) {}
        fn page(&mut self, _: usize, _: i32, _: i32) {}
        fn text(&mut self, text: &str, _: f32, _: f32, _: TextStyle, _: Rgba) {
            self.0.push(text.to_owned());
        }
        fn path(&mut self, _: &[PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {}
        fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn pop_clip(&mut self) {}
    }

    /// A one-page PDF with two layers (one off), a document attachment and a page attachment.
    fn pdf() -> Vec<u8> {
        let content =
            "/OC /a BDC 0 0 1 rg 10 10 20 20 re f EMC /OC /b BDC 1 0 0 rg 40 10 20 20 re f EMC";
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [5 0 R 6 0 R] /D << /OFF [6 0 R] \
             >> >> /Names << /EmbeddedFiles << /Names [(data.csv) 7 0 R] >> >> >>"
                .to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Annots [9 0 R] \
             /Resources << /Properties << /a 5 0 R /b 6 0 R >> >> >>"
                .to_owned(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
            "<< /Type /OCG /Name (Text layer) >>".to_owned(),
            "<< /Type /OCG /Name (Hidden layer) >>".to_owned(),
            "<< /Type /Filespec /F (data.csv) /Desc (Data table) /EF << /F 8 0 R >> >>".to_owned(),
            "<< /Type /EmbeddedFile /Length 3 >>\nstream\n1,2\nendstream".to_owned(),
            "<< /Type /Annot /Subtype /FileAttachment /Rect [10 700 26 716] /FS << /Type \
             /Filespec /F (notes.txt) /EF << /F 10 0 R >> >> >>"
                .to_owned(),
            "<< /Length 5 >>\nstream\nhello\nendstream".to_owned(),
        ];
        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offsets = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes(),
        );
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objs.len() + 1
            )
            .as_bytes(),
        );
        out
    }

    fn window() -> ReaderWindow {
        let doc = PdfDocument::open(pdf(), bo_pdfread::standard_fonts()).expect("pdf");
        let mut w = ReaderWindow::new("en-US");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1100.0,
            },
            &Fixed,
        );
        w.open(doc, "nav.pdf", None);
        w
    }

    fn paint(w: &mut ReaderWindow) -> Vec<String> {
        // One input lets the panels measure their strings, as the host does.
        w.handle(Input::Move { x: 1.0, y: 1.0 }, &Fixed);
        let mut t = Texts::default();
        w.paint(&mut t);
        t.0
    }

    fn press(w: &mut ReaderWindow, b: [f32; 4]) {
        let (x, y) = (b[0] + 4.0, b[1] + b[3] / 2.0);
        w.handle(
            Input::Press {
                x,
                y,
                shift: false,
                count: 1,
            },
            &Fixed,
        );
        w.handle(Input::Release { x, y }, &Fixed);
    }

    #[test]
    fn layers_and_attachments_panels() {
        let mut w = window();
        let nav: Vec<&str> = w.nav8().into_iter().map(|n| n.0).collect();
        assert_eq!(nav, vec!["nav.layers", "nav.attachments"]);
        // A document with embedded files opens with the Attachments panel.
        assert_eq!(w.panel_open(), Some(Panel::Attachments));
        let texts = paint(&mut w);
        for s in [
            "Attachments",
            "Name",
            "Description",
            "Size",
            "data.csv",
            "Data table",
            "notes.txt",
            "0 KB",
        ] {
            assert!(texts.iter().any(|t| t == s), "{s} in {texts:?}");
        }
        // Open / Save are enabled once a row is selected; Save asks the host to write it.
        let row = w.panels8.rows[0].0;
        press(&mut w, row);
        paint(&mut w);
        let save = *w
            .panels8
            .tools
            .iter()
            .find(|t| t.0 == "att.save")
            .expect("save");
        assert!(save.2);
        press(&mut w, save.1);
        let saved = w.take_requests().into_iter().any(|r| {
            matches!(r, ReaderRequest::SaveBytes { ref name, ref bytes }
                if name == "data.csv" && bytes == b"1,2")
        });
        assert!(saved);
        // Layers: a press on the first row hides the text layer.
        w.run_command("nav.layers", &Fixed);
        let texts = paint(&mut w);
        assert!(texts.iter().any(|t| t == "Hidden layer"));
        let on = |w: &ReaderWindow| -> Vec<bool> {
            w.view()
                .expect("view")
                .doc()
                .layers()
                .iter()
                .map(|r| r.on)
                .collect()
        };
        assert_eq!(on(&w), vec![true, false]);
        let row = w.panels8.rows[0].0;
        press(&mut w, row);
        assert_eq!(on(&w), vec![false, false]);
    }

    #[test]
    fn comment_list_states_filter_and_sort() {
        let mut w = window();
        for (author, text) in [("Alice", "First note"), ("Bob", "Second note")] {
            let n = NewAnnot {
                kind: AnnotKind::Note,
                page: 0,
                rects: vec![[100.0, 100.0, 120.0, 120.0]],
                color: [255, 214, 0],
                author: author.into(),
                contents: text.into(),
            };
            w.tabs[0].view.doc_mut().add_annotation(&n).expect("note");
        }
        w.run_command("nav.comments", &Fixed);
        let texts = paint(&mut w);
        assert!(texts.iter().any(|t| t == "Total Amount: 3"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Page 1"));
        assert!(texts.iter().any(|t| t == "First note"));
        // Accept Alice's note: a hidden state reply, not a reply in the list.
        let alice = w
            .comments8()
            .into_iter()
            .find(|(t, _)| t.comment.author == "Alice")
            .expect("alice");
        let a = alice.0.comment;
        w.select_comment8(a.page, a.rect);
        w.run_command("comment.state.Accepted", &Fixed);
        w.run_command("comment.mark", &Fixed);
        let doc = w.view().expect("view").doc();
        let s = doc.comment_state(&a);
        assert_eq!((s.review.as_deref(), s.marked), (Some("Accepted"), true));
        let texts = paint(&mut w);
        assert!(texts.iter().any(|t| t == "Total Amount: 3"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Accepted"));
        // Filter by status and by author, then sort by author.
        w.panels8.filter.status = vec!["Accepted".into()];
        assert_eq!(w.comments8().len(), 1);
        w.panels8.filter = CommentFilter {
            authors: vec!["Bob".into()],
            ..CommentFilter::default()
        };
        assert_eq!(w.comments8()[0].0.comment.author, "Bob");
        w.panels8.filter = CommentFilter::default();
        w.run_command("cm.sortBy.2", &Fixed);
        let authors: Vec<String> = w
            .comments8()
            .into_iter()
            .map(|(t, _)| t.comment.author)
            .collect();
        assert_eq!(authors[1..], ["Alice".to_owned(), "Bob".to_owned()]);
        // Search narrows the list.
        w.panels8.search = "second".into();
        assert_eq!(w.comments8().len(), 1);
    }

    #[test]
    fn selected_card_takes_a_reply() {
        let mut w = window();
        let n = NewAnnot {
            kind: AnnotKind::Note,
            page: 0,
            rects: vec![[100.0, 100.0, 120.0, 120.0]],
            color: [255, 214, 0],
            author: "Alice".into(),
            contents: "First note".into(),
        };
        w.tabs[0].view.doc_mut().add_annotation(&n).expect("note");
        w.run_command("nav.comments", &Fixed);
        paint(&mut w);
        let a = w
            .comments8()
            .into_iter()
            .find(|(t, _)| t.comment.author == "Alice")
            .expect("alice")
            .0
            .comment;
        w.select_comment8(a.page, a.rect);
        let texts = paint(&mut w);
        assert!(texts.iter().any(|t| t == "Add Reply"), "{texts:?}");
        let b = w
            .panels8
            .rows
            .iter()
            .find(|r| r.1 == Hit::ReplyBox)
            .expect("reply box")
            .0;
        press(&mut w, b);
        w.handle(Input::Text("Thanks".into()), &Fixed);
        w.handle(Input::Key(Key::Enter, crate::Mods::default()), &Fixed);
        let replies: Vec<String> = w
            .comments8()
            .into_iter()
            .find(|(t, _)| t.comment.author == "Alice")
            .expect("alice")
            .0
            .replies
            .into_iter()
            .map(|r| r.contents)
            .collect();
        assert_eq!(replies, vec!["Thanks".to_owned()]);
    }

    #[test]
    fn dates_sizes_and_wrapping() {
        assert_eq!(
            split_date("D:20261116173005+07'00'"),
            ("2026-11-16".to_owned(), "5:30:05 PM".to_owned())
        );
        assert_eq!(split_date("D:2026"), (String::new(), String::new()));
        assert_eq!(split_date("D:20260102000000").1, "12:00:00 AM");
        assert_eq!(size_text(14), "0 KB");
        assert_eq!(size_text(5000), "4 KB");
        let w = wrap("one two three four five six", 1.0, 9.0, 2);
        assert_eq!(w, vec!["one two".to_owned(), "three...".to_owned()]);
        assert_eq!(today().len(), 10);
    }
}
