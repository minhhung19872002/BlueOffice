//! Markup area of the review display modes: change bars in the margin next to
//! lines holding tracked changes (ECMA-376 Part 1, §17.13.5) and balloons for
//! comments (§17.13.4) and, optionally, deletions and formatting changes, stacked
//! in an area to the right of each page.
//!
//! The markup is a view of the laid-out pages: it never changes the document, the
//! page geometry or the display lists used for printing / PDF export. Hosts draw it
//! next to the page (the view's page grows by [`PageMarkup::width`]).

use crate::{LayoutDocument, Line, LineSource, Page};
use bo_model::Color;
use serde::{Deserialize, Serialize};

/// Reviewers that can be hidden at once (more are shown).
pub const MAX_HIDDEN_REVIEWERS: usize = 16;

/// What the all-markup view marks in the text. The default marks everything inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MarkFilter {
    /// Show insertions and deletions unmarked (as if accepted).
    pub hide_ins_del: bool,
    /// Deleted text leaves the line (it is shown in a balloon instead).
    pub deletions_in_balloons: bool,
    /// Keys of reviewers whose changes are shown unmarked (`0` = free slot; set with
    /// [`MarkFilter::hide_reviewers`]).
    pub hidden: [u64; MAX_HIDDEN_REVIEWERS],
    /// Formatting marks are shown: hidden text (§17.3.2.41 vanish) is laid out with a
    /// dotted underline, as word processors show it (never set for printing).
    pub show_hidden: bool,
}

/// Stable non-zero key of an author name.
fn author_key(author: &str) -> u64 {
    bo_model::fingerprint(author).max(1)
}

impl MarkFilter {
    /// Hide the changes of `authors` (at most [`MAX_HIDDEN_REVIEWERS`]).
    pub fn hide_reviewers<'a>(&mut self, authors: impl IntoIterator<Item = &'a str>) {
        self.hidden = [0; MAX_HIDDEN_REVIEWERS];
        for (slot, a) in self.hidden.iter_mut().zip(authors) {
            *slot = author_key(a);
        }
    }

    /// The changes of `author` are shown unmarked.
    pub fn hides(&self, author: &str) -> bool {
        self.hidden[0] != 0 && self.hidden.contains(&author_key(author))
    }
}

/// Kind of a marked change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkKind {
    /// Inserted (or moved-to) content.
    Insert,
    /// Deleted (or moved-from) content.
    Delete,
    /// Formatting change (run, paragraph, table or section properties).
    Format,
    /// Comment.
    Comment,
}

/// Where a mark outside the main text is (§17.10 headers/footers, §17.11 notes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkStory {
    /// A footnote or endnote: `start` / `end` are (paragraph index inside the note,
    /// byte offset).
    Note {
        /// Endnote (else footnote).
        endnote: bool,
        /// Note id.
        id: i32,
    },
    /// A header (`header`) or footer shown on `pages` (0-based): the balloon points at
    /// the header's first (the footer's last) line on each of them.
    Region {
        /// Header, else footer.
        header: bool,
        /// Pages showing it.
        pages: Vec<u32>,
    },
}

/// A change or comment to show in the markup (main text, or a story elsewhere).
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    /// Revision or comment id.
    pub id: i32,
    /// Kind of mark.
    pub kind: MarkKind,
    /// Start: (body paragraph id, byte offset in its index text).
    pub start: (u32, u32),
    /// End of the range (same coordinates).
    pub end: (u32, u32),
    /// Author name.
    pub author: String,
    /// Date (`ST_DateTime`).
    pub date: Option<String>,
    /// Text shown in the balloon.
    pub text: String,
    /// Resolved comment.
    pub done: bool,
    /// The mark gets a balloon (else only a change bar).
    pub balloon: bool,
    /// Outside the main text: where (`None`: the body, positions are body paragraphs).
    pub story: Option<MarkStory>,
}

/// How the markup of a page is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkupStyle {
    /// Width of the markup area right of the page (points); `0` = no balloons.
    pub area_width: f32,
    /// Change bar colour; `None` = no change bars.
    pub bar_color: Option<Color>,
    /// Balloons are collapsed into small icons (opened by the host on demand).
    pub icons: bool,
}

/// A vertical change bar in the left margin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeBar {
    /// X of the bar (points from the page's left edge).
    pub x: f32,
    /// Top (points).
    pub top: f32,
    /// Bottom (points).
    pub bottom: f32,
    /// `#rrggbb`.
    pub color: String,
}

/// A balloon (or, collapsed, an icon) in the markup area.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balloon {
    /// Revision or comment id.
    pub id: i32,
    /// Kind of mark.
    pub kind: MarkKind,
    /// Author name.
    pub author: String,
    /// Date (`ST_DateTime`).
    pub date: Option<String>,
    /// Text of the balloon.
    pub text: String,
    /// Resolved comment.
    pub done: bool,
    /// Author colour, `#rrggbb`.
    pub color: String,
    /// Left edge (points from the page's left edge; ≥ the page width).
    pub x: f32,
    /// Top edge (points).
    pub y: f32,
    /// Width (points).
    pub w: f32,
    /// Height (points).
    pub h: f32,
    /// Anchor in the text the balloon's connector points to.
    pub anchor_x: f32,
    /// Anchor y (just under the anchor's baseline).
    pub anchor_y: f32,
    /// Collapsed into an icon (simple markup).
    pub icon: bool,
}

/// Markup of one page.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageMarkup {
    /// Width of the markup area right of the page (points).
    pub width: f32,
    /// Change bars.
    pub bars: Vec<ChangeBar>,
    /// Balloons, top to bottom (never overlapping).
    pub balloons: Vec<Balloon>,
}

impl PageMarkup {
    /// Nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.width == 0.0 && self.bars.is_empty() && self.balloons.is_empty()
    }
}

/// Space between stacked balloons (points).
pub const BALLOON_GAP: f32 = 4.0;
/// Balloon padding, author line and text line heights (points).
const PAD: f32 = 4.0;
const LINE_H: f32 = 10.0;
/// Average advance of the balloon font (≈7.5 pt sans-serif), points per character.
const CHAR_W: f32 = 4.0;
/// Longest balloon text shown (lines); hosts clip the rest.
const MAX_LINES: usize = 6;
/// Side of a collapsed balloon (icon).
const ICON: f32 = 12.0;

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Height of a balloon of width `w` holding `text` under the author line.
pub fn balloon_height(text: &str, w: f32) -> f32 {
    let per_line = ((w - 2.0 * PAD) / CHAR_W).floor().max(1.0) as usize;
    let lines: usize = text
        .split('\n')
        .map(|p| p.chars().count().div_ceil(per_line).max(1))
        .sum();
    2.0 * PAD + LINE_H * (1 + lines.min(MAX_LINES)) as f32
}

/// Ranges of the marks drawing change bars, sorted by start, with the running
/// maximum of their ends: a line overlaps some range in O(log n).
struct Ranges {
    starts: Vec<(u32, u32)>,
    max_end: Vec<(u32, u32)>,
}

impl Ranges {
    fn new(marks: &[Mark]) -> Self {
        let mut v: Vec<((u32, u32), (u32, u32))> = marks
            .iter()
            .filter(|m| m.kind != MarkKind::Comment)
            .map(|m| (m.start, m.end.max(m.start)))
            .collect();
        v.sort_unstable();
        let mut max_end = Vec::with_capacity(v.len());
        let mut cur = (0, 0);
        for &(_, e) in &v {
            cur = cur.max(e);
            max_end.push(cur);
        }
        Self {
            starts: v.into_iter().map(|r| r.0).collect(),
            max_end,
        }
    }

    /// Some range meets `[from, to]`.
    fn meets(&self, from: (u32, u32), to: (u32, u32)) -> bool {
        let n = self.starts.partition_point(|&s| s <= to);
        n > 0 && self.max_end[n - 1] >= from
    }
}

/// Body paragraph of a line.
fn body(l: &Line) -> Option<u32> {
    match l.source {
        LineSource::Body(p) => Some(p),
        _ => None,
    }
}

/// X of offset `o` in line `l` (the last caret stop at or before it).
fn caret_x(l: &Line, o: u32) -> f32 {
    l.carets
        .iter()
        .take_while(|c| c.0 <= o)
        .last()
        .or(l.carets.first())
        .map_or_else(|| l.fragments.first().map_or(0.0, |f| f.x), |c| c.1)
}

/// Body lines of the paragraphs in `paras`, in document order: paragraph → (page, line).
fn line_index<'a>(
    layout: &'a LayoutDocument,
    paras: &std::collections::HashSet<u32>,
) -> std::collections::HashMap<u32, Vec<(usize, &'a Line)>> {
    let mut m: std::collections::HashMap<u32, Vec<(usize, &'a Line)>> =
        std::collections::HashMap::new();
    for (pi, page) in layout.pages.iter().enumerate() {
        for l in &page.lines {
            if let Some(p) = body(l).filter(|p| paras.contains(p)) {
                m.entry(p).or_default().push((pi, l));
            }
        }
    }
    m
}

/// The (page, line) pairs a mark's balloons point at: one in the body or a note, one
/// per page for a header or footer.
fn anchors<'a>(
    layout: &'a LayoutDocument,
    index: &std::collections::HashMap<u32, Vec<(usize, &'a Line)>>,
    m: &Mark,
) -> Vec<(usize, &'a Line)> {
    match &m.story {
        None => index
            .get(&m.start.0)
            .and_then(|v| locate(v, m.start.1))
            .into_iter()
            .collect(),
        Some(MarkStory::Note { endnote, id }) => {
            let lines: Vec<(usize, &Line)> = layout
                .pages
                .iter()
                .enumerate()
                .flat_map(|(pi, p)| p.lines.iter().map(move |l| (pi, l)))
                .filter(|(_, l)| {
                    matches!(l.source, LineSource::Note { endnote: e, id: i, para }
                        if e == *endnote && i == *id && para == m.start.0)
                })
                .collect();
            locate(&lines, m.start.1).into_iter().collect()
        }
        Some(MarkStory::Region { header, pages }) => pages
            .iter()
            .filter_map(|&pi| {
                let pi = pi as usize;
                let page = layout.pages.get(pi)?;
                let half = page.height / 2.0;
                let mut it = page
                    .lines
                    .iter()
                    .filter(|l| l.source == LineSource::Other)
                    .filter(|l| (l.top < half) == *header);
                let line = if *header { it.next() } else { it.next_back() }?;
                Some((pi, line))
            })
            .collect(),
    }
}

/// Page and line holding offset `o` among a paragraph's lines (its last line when
/// the offset lies past them, e.g. hidden deleted text at its end).
fn locate<'a>(lines: &[(usize, &'a Line)], o: u32) -> Option<(usize, &'a Line)> {
    lines
        .iter()
        .find(|(_, l)| l.start <= o && o <= l.end)
        .or(lines.last())
        .copied()
}

/// Left edge of the text of a page (for change bars).
fn text_left(page: &Page) -> f32 {
    page.lines
        .iter()
        .filter(|l| body(l).is_some())
        .filter_map(|l| l.carets.first().map(|c| c.1))
        .reduce(f32::min)
        .unwrap_or(72.0)
}

/// Change bars of a page: one per run of consecutive marked lines.
fn bars(page: &Page, ranges: &Ranges, color: Color) -> Vec<ChangeBar> {
    let x = (text_left(page) - 12.0).max(4.0);
    let mut spans: Vec<(f32, f32)> = page
        .lines
        .iter()
        .filter(|l| body(l).is_some_and(|p| ranges.meets((p, l.start), (p, l.end))))
        .map(|l| (l.top, l.top + l.height))
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for (t, b) in spans {
        match merged.last_mut() {
            Some(m) if t <= m.1 + 2.0 => m.1 = m.1.max(b),
            _ => merged.push((t, b)),
        }
    }
    merged
        .into_iter()
        .map(|(top, bottom)| ChangeBar {
            x,
            top,
            bottom,
            color: hex(color),
        })
        .collect()
}

/// Stack balloons (sorted by anchor) so that none overlaps the next: each starts at
/// its anchor or under the previous one, then the stack is pushed up from the page
/// bottom when it overflows, and finally pushed down again where the top was reached.
pub fn stack_balloons(balloons: &mut [Balloon], page_height: f32) {
    let mut bottom = f32::NEG_INFINITY;
    for b in balloons.iter_mut() {
        b.y = (b.anchor_y - 8.0).max(bottom + BALLOON_GAP).max(0.0);
        bottom = b.y + b.h;
    }
    let mut limit = page_height - BALLOON_GAP;
    for b in balloons.iter_mut().rev() {
        if b.y + b.h > limit {
            b.y = (limit - b.h).max(0.0);
        }
        limit = b.y - BALLOON_GAP;
    }
    let mut bottom = f32::NEG_INFINITY;
    for b in balloons.iter_mut() {
        b.y = b.y.max(bottom + BALLOON_GAP);
        bottom = b.y + b.h;
    }
}

/// Markup of every page of `layout` for `marks` (main-text positions).
pub fn page_markup(
    layout: &LayoutDocument,
    marks: &[Mark],
    style: &MarkupStyle,
) -> Vec<PageMarkup> {
    let body_marks: Vec<Mark> = marks
        .iter()
        .filter(|m| m.story.is_none())
        .cloned()
        .collect();
    let ranges = Ranges::new(&body_marks);
    let mut out: Vec<PageMarkup> = layout
        .pages
        .iter()
        .map(|p| PageMarkup {
            width: style.area_width,
            bars: style
                .bar_color
                .map(|c| bars(p, &ranges, c))
                .unwrap_or_default(),
            balloons: Vec::new(),
        })
        .collect();
    if style.area_width > 0.0 {
        let paras = marks
            .iter()
            .filter(|m| m.balloon)
            .map(|m| m.start.0)
            .collect();
        let index = line_index(layout, &paras);
        for m in marks.iter().filter(|m| m.balloon) {
            for (pi, line) in anchors(layout, &index, m) {
                let page_w = layout.pages[pi].width;
                let (x, w, h) = if style.icons {
                    (page_w + 4.0, ICON, ICON)
                } else {
                    let w = (style.area_width - 16.0).max(24.0);
                    (page_w + 10.0, w, balloon_height(&m.text, w))
                };
                out[pi].balloons.push(Balloon {
                    id: m.id,
                    kind: m.kind,
                    author: m.author.clone(),
                    date: m.date.clone(),
                    text: m.text.clone(),
                    done: m.done,
                    color: hex(crate::style::author_color(&m.author)),
                    x,
                    y: 0.0,
                    w,
                    h,
                    anchor_x: caret_x(line, m.start.1),
                    anchor_y: line.baseline + 2.0,
                    icon: style.icons,
                });
            }
        }
    }
    for (pm, page) in out.iter_mut().zip(&layout.pages) {
        pm.balloons.sort_by(|a, b| {
            a.anchor_y
                .total_cmp(&b.anchor_y)
                .then(a.anchor_x.total_cmp(&b.anchor_x))
        });
        stack_balloons(&mut pm.balloons, page.height);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn balloon(anchor_y: f32, h: f32) -> Balloon {
        Balloon {
            id: 0,
            kind: MarkKind::Comment,
            author: String::new(),
            date: None,
            text: String::new(),
            done: false,
            color: String::new(),
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h,
            anchor_x: 0.0,
            anchor_y,
            icon: false,
        }
    }

    #[test]
    fn stacked_balloons_never_overlap() {
        // Five balloons anchored on the same line near the page bottom.
        let mut v: Vec<Balloon> = (0..5).map(|_| balloon(780.0, 40.0)).collect();
        stack_balloons(&mut v, 842.0);
        for w in v.windows(2) {
            assert!(w[0].y + w[0].h + BALLOON_GAP <= w[1].y + 0.01, "{w:?}");
        }
        // Pushed up so the last one ends inside the page.
        let last = v.last().map_or(0.0, |b| b.y + b.h);
        assert!(last <= 842.0 - BALLOON_GAP + 0.01);
        // A single balloon sits at its anchor.
        let mut one = vec![balloon(200.0, 30.0)];
        stack_balloons(&mut one, 842.0);
        assert!((one[0].y - 192.0).abs() < 0.01);
    }

    #[test]
    fn balloon_height_grows_with_text() {
        let short = balloon_height("ok", 160.0);
        let long = balloon_height(&"x".repeat(200), 160.0);
        assert!((short - 28.0).abs() < 0.01);
        assert!(long > short);
        // Capped: very long texts are clipped by the host.
        assert!(balloon_height(&"x".repeat(5000), 160.0) <= 2.0 * PAD + LINE_H * 7.0 + 0.01);
    }

    #[test]
    fn reviewer_filter() {
        let mut f = MarkFilter::default();
        assert!(!f.hides("An"));
        f.hide_reviewers(["An", "Bình"]);
        assert!(f.hides("An") && f.hides("Bình") && !f.hides("Chi"));
    }
}
