//! Review display modes (BlueDoc): which version of the text is laid out, what the
//! markup marks and how the markup area (change bars, balloons) is drawn. These are
//! view settings only; the document model is never changed.
//!
//! Tracked changes: ECMA-376 Part 1, §17.13.5; comments: §17.13.4.

use bo_edit::{CommentInfo, RevKind, RevisionItem};
use bo_layout::{Mark, MarkFilter, MarkKind, MarkupStyle, RevisionView};
use bo_model::Color;
use serde::{Deserialize, Serialize};

/// How changes and comments are displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkupMode {
    /// Accepted text with a red bar next to changed lines; comments as icons.
    Simple,
    /// Every change marked inline, bars, comments in balloons in the markup area.
    #[default]
    All,
    /// Accepted text, nothing marked.
    None,
    /// Original text (every change rejected), nothing marked.
    Original,
}

/// Review display settings (kept across documents of an engine).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MarkupOptions {
    /// Display mode.
    pub mode: MarkupMode,
    /// Mark insertions and deletions.
    pub insertions_deletions: bool,
    /// Mark formatting changes.
    pub formatting: bool,
    /// Show comments.
    pub comments: bool,
    /// Reviewers whose changes and comments are not marked.
    pub hidden_reviewers: Vec<String>,
    /// All markup: deletions and formatting changes in balloons instead of inline.
    pub deletion_balloons: bool,
    /// Show All Revisions Inline: no balloons at all (no markup area), comments marked in
    /// the text only (BlueDoc round 16, the reference's Balloons ▸ submenu).
    pub inline: bool,
    /// Advanced Track Changes Options (BlueDoc round 16): insertion marks (`underline`,
    /// `none`, `colorOnly`, `doubleUnderline`, `strikethrough`).
    pub ins_mark: String,
    /// Deletion marks (`strikethrough`, `colorOnly`, `doubleStrikethrough`, `hidden`,
    /// `underline`, `doubleUnderline`).
    pub del_mark: String,
    /// Insertion colour (`RRGGBB`); empty = by author.
    pub ins_color: String,
    /// Deletion colour (`RRGGBB`); empty = by author.
    pub del_color: String,
    /// Changed lines: `outside` (left of the text, the default), `left`, `right`, `none`.
    pub changed_lines: String,
    /// Preferred balloon width (inches; the reference's default 3.7").
    pub balloon_width: f32,
    /// Show lines connecting balloons to the text.
    pub connectors: bool,
    /// Pictures by comments (the author's initials in a circle).
    pub pictures: bool,
    /// Comment colour (`RRGGBB`); empty = by author.
    pub comment_color: String,
}

impl Default for MarkupOptions {
    fn default() -> Self {
        Self {
            mode: MarkupMode::All,
            insertions_deletions: true,
            formatting: true,
            comments: true,
            hidden_reviewers: Vec::new(),
            deletion_balloons: false,
            inline: false,
            ins_mark: "underline".to_owned(),
            del_mark: "strikethrough".to_owned(),
            ins_color: String::new(),
            del_color: String::new(),
            changed_lines: "outside".to_owned(),
            balloon_width: DEFAULT_BALLOON_WIDTH,
            connectors: true,
            pictures: true,
            comment_color: String::new(),
        }
    }
}

/// Width of the markup area holding balloons (points): 394 px at 125 % on the reference
/// word processor (BlueDoc round 6).
pub const AREA_WIDTH: f32 = 236.0;
/// Preferred balloon width the markup area is measured at (inches).
pub const DEFAULT_BALLOON_WIDTH: f32 = 3.7;
/// Width of the simple-markup strip holding comment icons (points).
pub const ICON_AREA_WIDTH: f32 = 24.0;
/// Change bar colours: neutral in all markup, red in simple markup.
const BAR_ALL: Color = Color {
    r: 0x59,
    g: 0x59,
    b: 0x59,
};
const BAR_SIMPLE: Color = Color {
    r: 0xD0,
    g: 0x21,
    b: 0x21,
};

/// Balloon text of a formatting change, worded as the reference words it
/// ("Formatted: Font: Bold, Italic").
fn format_text(keys: &[&str]) -> String {
    const FONT: [(&str, &str); 8] = [
        ("bold", "Bold"),
        ("italic", "Italic"),
        ("underline", "Underline"),
        ("strike", "Strikethrough"),
        ("font", "Font"),
        ("size", "Size"),
        ("color", "Font color"),
        ("highlight", "Highlight"),
    ];
    let font: Vec<&str> = keys
        .iter()
        .filter_map(|k| FONT.iter().find(|(f, _)| f == k).map(|(_, v)| *v))
        .collect();
    let mut parts = Vec::new();
    if !font.is_empty() {
        parts.push(format!("Font: {}", font.join(", ")));
    }
    for k in keys.iter().filter(|k| !FONT.iter().any(|(f, _)| f == *k)) {
        let mut c = k.chars();
        if let Some(f) = c.next() {
            parts.push(f.to_uppercase().chain(c).collect());
        }
    }
    format!("Formatted: {}", parts.join(", "))
}

fn kind_of(k: RevKind) -> MarkKind {
    match k {
        RevKind::Insert
        | RevKind::MoveTo
        | RevKind::ParagraphInsert
        | RevKind::RowInsert
        | RevKind::CellInsert
        // An edited equation (ADR-0015 addendum): the deleted original and the inserted edit.
        | RevKind::Equation => MarkKind::Insert,
        RevKind::Delete
        | RevKind::MoveFrom
        | RevKind::ParagraphDelete
        | RevKind::RowDelete
        | RevKind::CellDelete => MarkKind::Delete,
        _ => MarkKind::Format,
    }
}

fn pos(p: bo_edit::Pos) -> (u32, u32) {
    (
        u32::try_from(p.para).unwrap_or(u32::MAX),
        u32::try_from(p.offset).unwrap_or(u32::MAX),
    )
}

impl MarkupOptions {
    /// Version of the text laid out for this mode.
    pub fn layout_view(&self) -> RevisionView {
        match self.mode {
            MarkupMode::All => RevisionView::Markup,
            MarkupMode::Simple | MarkupMode::None => RevisionView::Final,
            MarkupMode::Original => RevisionView::Original,
        }
    }

    /// Mode matching a legacy revision view (`setRevisionView`).
    pub fn mode_of(view: RevisionView) -> MarkupMode {
        match view {
            RevisionView::Markup => MarkupMode::All,
            RevisionView::Final => MarkupMode::None,
            RevisionView::Original => MarkupMode::Original,
        }
    }

    /// What the all-markup layout marks inline.
    pub fn filter(&self) -> MarkFilter {
        let hex = |h: &str| Color::from_hex(h);
        let mut f = MarkFilter {
            hide_ins_del: !self.insertions_deletions,
            deletions_in_balloons: self.deletion_balloons,
            ins_mark: match self.ins_mark.as_str() {
                "none" => 1,
                "colorOnly" => 2,
                "doubleUnderline" => 3,
                "strikethrough" => 4,
                _ => 0,
            },
            del_mark: match self.del_mark.as_str() {
                "colorOnly" => 1,
                "doubleStrikethrough" => 2,
                "hidden" => 3,
                "underline" => 4,
                "doubleUnderline" => 5,
                _ => 0,
            },
            ins_color: hex(&self.ins_color),
            del_color: hex(&self.del_color),
            ..MarkFilter::default()
        };
        f.hide_reviewers(self.hidden_reviewers.iter().map(String::as_str));
        f
    }

    fn shown(&self, author: &str) -> bool {
        !self.hidden_reviewers.iter().any(|a| a == author)
    }

    /// Changes and comments of the main text drawn in the markup (empty when the mode
    /// draws none).
    pub fn marks(&self, revisions: &[RevisionItem], comments: &[CommentInfo]) -> Vec<Mark> {
        if matches!(self.mode, MarkupMode::None | MarkupMode::Original) {
            return Vec::new();
        }
        let all = self.mode == MarkupMode::All;
        let mut out = Vec::new();
        for r in revisions.iter().filter(|r| r.story.is_none()) {
            let kind = kind_of(r.kind);
            let on = match kind {
                MarkKind::Format => self.formatting,
                _ => self.insertions_deletions,
            };
            if !on || !self.shown(&r.author) {
                continue;
            }
            out.push(Mark {
                id: r.id,
                kind,
                start: pos(r.start),
                end: pos(r.end),
                author: r.author.clone(),
                date: r.date.clone(),
                text: if kind == MarkKind::Format {
                    format_text(&r.detail)
                } else {
                    r.text.clone()
                },
                done: false,
                // All markup: formatting changes always in balloons ("Formatted: Font: Bold"),
                // deletions too when asked, as the reference shows revisions.
                balloon: all
                    && !self.inline
                    && (kind == MarkKind::Format
                        || (self.deletion_balloons && kind != MarkKind::Insert)),
            });
        }
        if self.comments {
            for c in comments.iter().filter(|c| c.parent.is_none()) {
                let (Some(start), Some(end)) = (c.start, c.end) else {
                    continue;
                };
                if !self.shown(&c.author) {
                    continue;
                }
                // Replies are listed in their thread's balloon.
                let mut text = c.text.clone();
                for r in comments.iter().filter(|r| r.parent == Some(c.id)) {
                    text.push_str(&format!("\n↳ {}: {}", r.author, r.text));
                }
                out.push(Mark {
                    id: c.id,
                    kind: MarkKind::Comment,
                    start: pos(start),
                    end: pos(end),
                    author: c.author.clone(),
                    date: c.date.clone(),
                    text,
                    done: c.done,
                    balloon: !self.inline,
                });
            }
        }
        out
    }

    /// How the markup area is drawn for `marks`.
    pub fn style(&self, marks: &[Mark]) -> MarkupStyle {
        let balloons = marks.iter().any(|m| m.balloon);
        // Preferred width: the area scales with it (the default gives the measured area).
        let k = (self.balloon_width / DEFAULT_BALLOON_WIDTH).clamp(0.3, 3.0);
        let area = if balloons { AREA_WIDTH * k } else { 0.0 };
        let bars = |c: Color| (self.changed_lines != "none").then_some(c);
        match self.mode {
            MarkupMode::All => MarkupStyle {
                area_width: area,
                bar_color: bars(BAR_ALL),
                icons: false,
            },
            // Simple markup keeps the comments' balloons (only comments have one here), as
            // the reference shows them beside the clean text.
            MarkupMode::Simple => MarkupStyle {
                area_width: area,
                bar_color: bars(BAR_SIMPLE),
                icons: false,
            },
            MarkupMode::None | MarkupMode::Original => MarkupStyle {
                area_width: 0.0,
                bar_color: None,
                icons: false,
            },
        }
    }
}

/// Partial update of the markup options (`setMarkup`): absent fields are kept.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkupUpdate {
    /// Display mode.
    #[serde(default)]
    pub mode: Option<MarkupMode>,
    /// Mark insertions and deletions.
    #[serde(default)]
    pub insertions_deletions: Option<bool>,
    /// Mark formatting changes.
    #[serde(default)]
    pub formatting: Option<bool>,
    /// Show comments.
    #[serde(default)]
    pub comments: Option<bool>,
    /// Reviewers not marked.
    #[serde(default)]
    pub hidden_reviewers: Option<Vec<String>>,
    /// Deletions and formatting changes in balloons.
    #[serde(default)]
    pub deletion_balloons: Option<bool>,
    /// No balloons (Show All Revisions Inline).
    #[serde(default)]
    pub inline: Option<bool>,
    /// Insertion / deletion marks and colours, changed lines, balloon width, connecting
    /// lines, pictures (Advanced Track Changes Options).
    #[serde(default)]
    pub ins_mark: Option<String>,
    /// Deletion marks.
    #[serde(default)]
    pub del_mark: Option<String>,
    /// Insertion colour (`""` = by author).
    #[serde(default)]
    pub ins_color: Option<String>,
    /// Deletion colour (`""` = by author).
    #[serde(default)]
    pub del_color: Option<String>,
    /// Changed lines position.
    #[serde(default)]
    pub changed_lines: Option<String>,
    /// Preferred balloon width (inches).
    #[serde(default)]
    pub balloon_width: Option<f32>,
    /// Lines connecting balloons to the text.
    #[serde(default)]
    pub connectors: Option<bool>,
    /// Pictures by comments.
    #[serde(default)]
    pub pictures: Option<bool>,
    /// Comment colour (`""` = by author).
    #[serde(default)]
    pub comment_color: Option<String>,
}

impl MarkupOptions {
    /// Apply `u`; returns whether anything changed.
    pub fn apply(&mut self, u: MarkupUpdate) -> bool {
        let before = self.clone();
        if let Some(v) = u.mode {
            self.mode = v;
        }
        if let Some(v) = u.insertions_deletions {
            self.insertions_deletions = v;
        }
        if let Some(v) = u.formatting {
            self.formatting = v;
        }
        if let Some(v) = u.comments {
            self.comments = v;
        }
        if let Some(mut v) = u.hidden_reviewers {
            v.sort();
            v.dedup();
            self.hidden_reviewers = v;
        }
        if let Some(v) = u.deletion_balloons {
            self.deletion_balloons = v;
        }
        if let Some(v) = u.inline {
            self.inline = v;
        }
        if let Some(v) = u.ins_mark {
            self.ins_mark = v;
        }
        if let Some(v) = u.del_mark {
            self.del_mark = v;
        }
        if let Some(v) = u.ins_color {
            self.ins_color = v;
        }
        if let Some(v) = u.del_color {
            self.del_color = v;
        }
        if let Some(v) = u.changed_lines {
            self.changed_lines = v;
        }
        if let Some(v) = u.balloon_width.filter(|w| w.is_finite() && *w > 0.0) {
            self.balloon_width = v;
        }
        if let Some(v) = u.connectors {
            self.connectors = v;
        }
        if let Some(v) = u.pictures {
            self.pictures = v;
        }
        if let Some(v) = u.comment_color {
            self.comment_color = v;
        }
        *self != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_map_to_layout_views() {
        let mut o = MarkupOptions::default();
        assert_eq!(o.layout_view(), RevisionView::Markup);
        for (m, v) in [
            (MarkupMode::Simple, RevisionView::Final),
            (MarkupMode::None, RevisionView::Final),
            (MarkupMode::Original, RevisionView::Original),
        ] {
            o.mode = m;
            assert_eq!(o.layout_view(), v);
            assert_eq!(o.marks(&[], &[]).len(), 0);
        }
        assert!(o.apply(MarkupUpdate {
            hidden_reviewers: Some(vec!["B".into(), "A".into(), "B".into()]),
            ..MarkupUpdate::default()
        }));
        assert_eq!(o.hidden_reviewers, ["A", "B"]);
        assert!(!o.apply(MarkupUpdate::default()));
    }
}
