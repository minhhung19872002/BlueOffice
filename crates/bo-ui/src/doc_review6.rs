//! Review › Tracking of parity round 6: the Track Changes split button (Track Changes /
//! Lock Tracking), the Reviewing Pane (vertical, docked left of the document as the
//! reference docks it; the horizontal form below the document), and the Accept ▾ / Reject ▾
//! menus with the reference's five entries each (… and Move to Next, This Change, All
//! Changes Shown, All Changes, All Changes and Stop Tracking). Revisions are ECMA-376
//! Part 1, §17.13.5; locked tracking is §17.15.1.29 documentProtection
//! `w:edit="trackedChanges"`.
use super::super::DocWindow;
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use crate::{Input, Painter, TextStyle};
use bo_edit::revisions::RevKind;
use bo_engine::edit::Command;
use bo_render::Rgba;
use serde_json::json;

/// One row of the Reviewing Pane.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PaneRow {
    pub(crate) author: String,
    /// "Inserted", "Deleted", "Formatted", "Comment" …
    pub(crate) what: String,
    pub(crate) text: String,
    /// Revision id, or the comment id when `comment`.
    pub(crate) id: i32,
    pub(crate) comment: bool,
}

/// The Reviewing Pane.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReviewPane {
    /// Below the document (horizontal) instead of left of it.
    pub(crate) horizontal: bool,
    pub(crate) rect: [f32; 4],
    pub(crate) rows: Vec<PaneRow>,
    /// Revisions by kind for the summary line: insertions, deletions, moves, formatting,
    /// comments.
    pub(crate) counts: [usize; 5],
    pub(crate) scroll: f32,
    pub(crate) hover: Option<usize>,
    pub(crate) scale: f32,
}

/// Pane width (vertical) / height (horizontal), logical pixels (measured on the reference at
/// 125 %: 345 px wide, the list box 16–266, rows 60 px).
const PANE_W: f32 = 276.0;
const PANE_H: f32 = 180.0;
const HEAD_H: f32 = 92.0;
const ROW_H: f32 = 48.0;

impl ReviewPane {
    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Width the pane takes left of the document (0 when horizontal).
    pub(crate) fn width(&self) -> f32 {
        if self.horizontal {
            0.0
        } else {
            self.px(PANE_W)
        }
    }

    /// Height the pane takes below the document (0 when vertical).
    pub(crate) fn height(&self) -> f32 {
        if self.horizontal {
            self.px(PANE_H)
        } else {
            0.0
        }
    }

    fn close_rect(&self) -> [f32; 4] {
        let r = self.rect;
        [
            r[0] + r[2] - self.px(32.0),
            r[1] + self.px(10.0),
            self.px(24.0),
            self.px(24.0),
        ]
    }

    /// The white list box under the summary.
    fn list_box(&self) -> [f32; 4] {
        let r = self.rect;
        [
            r[0] + self.px(12.0),
            r[1] + self.px(HEAD_H) - self.px(8.0),
            r[2] - self.px(24.0),
            (r[3] - self.px(HEAD_H) - self.px(4.0)).max(1.0),
        ]
    }

    fn row_rect(&self, i: usize) -> [f32; 4] {
        let lb = self.list_box();
        [
            lb[0] + 1.0,
            lb[1] + self.px(14.0) + self.px(ROW_H) * i as f32 - self.scroll,
            lb[2] - 2.0,
            self.px(ROW_H),
        ]
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let r = self.rect;
        x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
    }

    fn row_at(&self, x: f32, y: f32) -> Option<usize> {
        if !self.contains(x, y) || y < self.list_box()[1] {
            return None;
        }
        (0..self.rows.len()).find(|&i| {
            let r = self.row_rect(i);
            y >= r[1] && y < r[1] + r[3]
        })
    }

    /// Summary under the title, as the reference words it ("3 revisions").
    fn summary(&self, s: &crate::strings::Strings) -> String {
        // The reference counts comments among the revisions.
        let n: usize = self.counts.iter().sum();
        let one = tr(s, "docR6.rv.revision", "revision");
        let many = tr(s, "docR6.rv.revisions", "revisions");
        format!("{n} {}", if n == 1 { one } else { many })
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter, s: &crate::strings::Strings, accent: Rgba) {
        let r = self.rect;
        // Pane on the window's light grey (#F3F3F3), a white list box framed in grey, as the
        // reference's Revisions pane.
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(243, 243, 243, 255));
        if self.horizontal {
            p.fill_rect(r[0], r[1], r[2], 1.0, Rgba(212, 212, 212, 255));
        } else {
            p.fill_rect(r[0] + r[2] - 1.0, r[1], 1.0, r[3], Rgba(212, 212, 212, 255));
        }
        let ink = Rgba(38, 38, 38, 255);
        let muted = Rgba(89, 89, 89, 255);
        let light = crate::Font {
            family: "Segoe UI Light",
            size: self.px(20.0),
            weight: 300,
            italic: false,
            underline: false,
        };
        p.text_font(
            &tr(s, "docR6.rv.title", "Revisions"),
            r[0] + self.px(12.0),
            r[1] + self.px(6.0),
            &light,
            ink,
        );
        let small = TextStyle {
            size: self.px(11.0),
            bold: false,
        };
        let bold = TextStyle {
            size: self.px(12.0),
            bold: true,
        };
        // "∧ 5 revisions".
        let (ax, ay) = (r[0] + self.px(18.0), r[1] + self.px(48.0));
        let k = self.px(5.0);
        p.path(
            &[
                bo_render::PathSeg::M(ax - k, ay + k / 2.0),
                bo_render::PathSeg::L(ax, ay - k / 2.0),
                bo_render::PathSeg::L(ax + k, ay + k / 2.0),
            ],
            None,
            Some((ink, self.scale.max(1.0))),
        );
        p.text(
            &self.summary(s),
            r[0] + self.px(38.0),
            ay - self.px(9.0),
            bold,
            ink,
        );
        // Close mark.
        let c = self.close_rect();
        let k = self.px(4.5);
        let (cx, cy) = (c[0] + c[2] / 2.0, c[1] + c[3] / 2.0);
        p.path(
            &[
                bo_render::PathSeg::M(cx - k, cy - k),
                bo_render::PathSeg::L(cx + k, cy + k),
                bo_render::PathSeg::M(cx + k, cy - k),
                bo_render::PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((ink, self.scale.max(1.0))),
        );
        let lb = self.list_box();
        p.fill_rect(lb[0], lb[1], lb[2], lb[3], Rgba(171, 171, 171, 255));
        p.fill_rect(
            lb[0] + 1.0,
            lb[1] + 1.0,
            lb[2] - 2.0,
            lb[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        p.push_clip(lb[0] + 1.0, lb[1] + 1.0, lb[2] - 2.0, lb[3] - 2.0);
        if self.rows.is_empty() {
            p.text(
                &tr(s, "docR6.rv.none", "No revisions in this document."),
                lb[0] + self.px(16.0),
                lb[1] + self.px(14.0),
                small,
                muted,
            );
        }
        let doc_font = crate::Font {
            family: "Times New Roman",
            size: self.px(15.0),
            weight: 400,
            italic: false,
            underline: false,
        };
        for (i, row) in self.rows.iter().enumerate() {
            let rr = self.row_rect(i);
            if rr[1] + rr[3] < lb[1] || rr[1] > lb[1] + lb[3] {
                continue;
            }
            if self.hover == Some(i) {
                p.fill_rect(rr[0], rr[1], rr[2], rr[3], Rgba(229, 241, 251, 255));
            }
            if row.comment {
                p.fill_rect(
                    rr[0] + self.px(4.0),
                    rr[1] + self.px(6.0),
                    self.px(2.0),
                    self.px(36.0),
                    accent,
                );
            }
            p.text(
                &format!("{} {}", row.author, row.what),
                rr[0] + self.px(30.0),
                rr[1] + self.px(6.0),
                small,
                muted,
            );
            p.push_clip(rr[0], rr[1], rr[2] - self.px(6.0), rr[3]);
            p.text_font(
                &row.text,
                rr[0] + self.px(42.0),
                rr[1] + self.px(21.0),
                &doc_font,
                ink,
            );
            p.pop_clip();
        }
        p.pop_clip();
    }
}

/// Formatting changes as the reference words them ("Font: Bold, Italic", "Centered").
fn format_detail(keys: &[&str]) -> String {
    let font: Vec<&str> = keys
        .iter()
        .filter_map(|k| {
            Some(match *k {
                "bold" => "Bold",
                "italic" => "Italic",
                "underline" => "Underline",
                "strike" => "Strikethrough",
                "font" => "Font",
                "size" => "Size",
                "color" => "Font color",
                "highlight" => "Highlight",
                _ => return None,
            })
        })
        .collect();
    let mut parts = Vec::new();
    if !font.is_empty() {
        parts.push(format!("Font: {}", font.join(", ")));
    }
    for k in keys {
        if !matches!(
            *k,
            "bold" | "italic" | "underline" | "strike" | "font" | "size" | "color" | "highlight"
        ) {
            let mut c = k.chars();
            if let Some(f) = c.next() {
                parts.push(f.to_uppercase().chain(c).collect());
            }
        }
    }
    parts.join(", ")
}

fn tr(s: &crate::strings::Strings, key: &str, fallback: &str) -> String {
    let v = s.get(key);
    if v == key {
        fallback.to_owned()
    } else {
        v.to_owned()
    }
}

impl DocWindow {
    /// Width taken left of the document by the vertical Reviewing Pane.
    pub(in crate::shell) fn r6_review_w(&self) -> f32 {
        self.r6.review.as_ref().map_or(0.0, ReviewPane::width)
    }

    /// Height taken below the document by the horizontal Reviewing Pane.
    pub(in crate::shell) fn r6_review_h(&self) -> f32 {
        self.r6.review.as_ref().map_or(0.0, ReviewPane::height)
    }

    /// Rows of the Reviewing Pane from the review state.
    fn review_rows(&self) -> (Vec<PaneRow>, [usize; 5]) {
        let s = &self.strings;
        let mut rows = Vec::new();
        let mut counts = [0; 5];
        for r in self.view.revisions() {
            let (key, fb, slot) = match r.kind {
                RevKind::Insert | RevKind::ParagraphInsert => ("docR6.rv.inserted", "Added", 0),
                RevKind::Delete | RevKind::ParagraphDelete => ("docR6.rv.deleted", "Deleted", 1),
                RevKind::MoveTo | RevKind::MoveFrom => ("docR6.rv.moved", "Moved", 2),
                _ => ("docR6.rv.formatted", "Formatted", 3),
            };
            counts[slot] += 1;
            let text = if slot == 3 && !r.detail.is_empty() {
                format_detail(&r.detail)
            } else {
                r.text.replace(['\n', '\r'], " ")
            };
            rows.push(PaneRow {
                author: r.author.clone(),
                what: tr(s, key, fb),
                text,
                id: r.id,
                comment: false,
            });
        }
        // At the same place the reference lists the added text before the deleted text.
        for i in 1..rows.len() {
            let (a, b) = (&rows[i - 1], &rows[i]);
            if a.what == tr(s, "docR6.rv.deleted", "Deleted")
                && b.what == tr(s, "docR6.rv.inserted", "Added")
            {
                rows.swap(i - 1, i);
            }
        }
        for c in self.view.comments() {
            counts[4] += 1;
            rows.push(PaneRow {
                author: c.author.clone(),
                what: tr(s, "docR6.rv.commented", "Commented"),
                text: c.text.replace(['\n', '\r'], " "),
                id: c.id,
                comment: true,
            });
        }
        (rows, counts)
    }

    /// Show or hide the Reviewing Pane (`horizontal`: below the document).
    pub(crate) fn r6_toggle_review(&mut self, horizontal: Option<bool>) -> bool {
        match (self.r6.review.as_ref(), horizontal) {
            (Some(p), Some(h)) if p.horizontal != h => {}
            (Some(_), _) => {
                self.r6.review = None;
                self.relayout_pending = true;
                return true;
            }
            _ => {}
        }
        let (rows, counts) = self.review_rows();
        self.r6.review = Some(ReviewPane {
            horizontal: horizontal.unwrap_or(false),
            rows,
            counts,
            scale: self.scale,
            ..ReviewPane::default()
        });
        self.relayout_pending = true;
        true
    }

    /// Lay the pane out (after the shell's layout) and refresh its rows.
    pub(super) fn r6_layout_review(&mut self) {
        if self.r6.review.is_none() {
            return;
        }
        let (rows, counts) = self.review_rows();
        let top = self.chrome_bottom();
        let bottom = self.size.1 - self.status_h();
        let left = if self.nav.is_some() {
            crate::nav_pane::NavPane::width(self.scale)
        } else {
            0.0
        };
        let (w, scale) = (self.size.0, self.scale);
        if let Some(p) = self.r6.review.as_mut() {
            p.scale = scale;
            p.rows = rows;
            p.counts = counts;
            p.rect = if p.horizontal {
                let h = p.px(PANE_H);
                [left, bottom - h, w - left, h]
            } else {
                [left, top, p.px(PANE_W), (bottom - top).max(1.0)]
            };
        }
    }

    /// Pointer input over the pane; `None` lets it through.
    pub(super) fn r6_review_input(&mut self, input: &Input) -> Option<bool> {
        let p = self.r6.review.as_mut()?;
        match *input {
            Input::Move { x, y } => {
                if !p.contains(x, y) {
                    return p.hover.take().map(|_| true);
                }
                let h = p.row_at(x, y);
                Some(std::mem::replace(&mut p.hover, h) != h)
            }
            Input::Press { x, y, .. } if p.contains(x, y) => {
                let c = p.close_rect();
                if x >= c[0] && x < c[0] + c[2] && y >= c[1] && y < c[1] + c[3] {
                    return Some(self.r6_toggle_review(None));
                }
                let row = p.row_at(x, y).and_then(|i| p.rows.get(i).cloned());
                Some(row.is_some_and(|r| self.r6_go_to_row(&r)))
            }
            Input::Wheel(dy) => {
                let (x, y) = self.pointer;
                if !p.contains(x, y) {
                    return None;
                }
                let room = (p.rows.len() as f32 * p.px(ROW_H) + p.px(HEAD_H) - p.rect[3]).max(0.0);
                p.scroll = (p.scroll + dy).clamp(0.0, room);
                Some(true)
            }
            _ => None,
        }
    }

    /// Select the change (or the comment's anchor) of a pane row.
    fn r6_go_to_row(&mut self, row: &PaneRow) -> bool {
        let sel = if row.comment {
            self.view
                .comments()
                .iter()
                .find(|c| c.id == row.id)
                .and_then(|c| Some((c.start?, c.end?)))
        } else {
            self.view
                .revisions()
                .iter()
                .find(|r| r.id == row.id && r.story.is_none())
                .map(|r| (r.start, r.end))
        };
        let Some((anchor, focus)) = sel else {
            return false;
        };
        self.view.run_command(Command::SetSelection {
            selection: bo_edit::Selection { anchor, focus },
        })
    }

    /// Review commands of round 6; `None` passes the id on.
    pub(super) fn r6_review_action(&mut self, id: &str) -> Option<bool> {
        let run = |w: &mut Self, v: serde_json::Value| {
            crate::catalog::from_json(v).is_some_and(|c| w.view.run_command(c))
        };
        Some(match id {
            "review.trackChangesMenu" => self.r6_track_menu(),
            "review.lockTracking" => {
                if self.view.tracking_locked() {
                    run(self, json!({ "cmd": "unprotectDocument" }))
                } else {
                    // Locking turns tracking on and keeps it on (§17.15.1.29).
                    run(self, json!({ "cmd": "setTrackChanges", "on": true }));
                    run(
                        self,
                        json!({ "cmd": "protectDocument", "edit": "trackedChanges" }),
                    )
                }
            }
            "review.reviewingPane.vertical" => self.r6_toggle_review(Some(false)),
            "review.reviewingPane.horizontal" => self.r6_toggle_review(Some(true)),
            "review.acceptNext" => {
                run(self, json!({ "cmd": "acceptRevision" }));
                run(self, json!({ "cmd": "nextRevision", "forward": true }))
            }
            "review.rejectNext" => {
                run(self, json!({ "cmd": "rejectRevision" }));
                run(self, json!({ "cmd": "nextRevision", "forward": true }))
            }
            "review.acceptAllShown" => run(self, json!({ "cmd": "acceptAllRevisions" })),
            "review.rejectAllShown" => run(self, json!({ "cmd": "rejectAllRevisions" })),
            "review.acceptAllStop" => {
                run(self, json!({ "cmd": "acceptAllRevisions" }));
                run(self, json!({ "cmd": "setTrackChanges", "on": false }))
            }
            "review.rejectAllStop" => {
                run(self, json!({ "cmd": "rejectAllRevisions" }));
                run(self, json!({ "cmd": "setTrackChanges", "on": false }))
            }
            _ => return None,
        })
    }

    /// The Track Changes split button's menu.
    fn r6_track_menu(&mut self) -> bool {
        let Some(anchor) = self
            .ribbon
            .rect_of(RibbonHit::Command("review.trackChanges"))
        else {
            return false;
        };
        let locked = self.view.tracking_locked();
        let items = vec![
            MenuItem {
                label: tr(&self.strings, "docR6.rv.forEveryone", "For Everyone"),
                choice: Choice::Command("review.trackChanges".into()),
                checked: self.view.track_changes(),
                dim: locked,
            },
            MenuItem {
                label: tr(&self.strings, "docR6.rv.lock", "Lock Tracking"),
                choice: Choice::Command("review.lockTracking".into()),
                checked: locked,
                dim: false,
            },
        ];
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        true
    }

    /// The Track Changes split button: its lower part (label and ▾) opens the menu.
    pub(super) fn r6_split_release(&mut self, input: &Input) -> Option<bool> {
        let Input::Release { x, y } = *input else {
            return None;
        };
        let hit = RibbonHit::Command("review.trackChanges");
        if self.menu.is_some()
            || self.ribbon.hit(x, y) != Some(hit)
            || self.ribbon.hit(self.pointer.0, self.pointer.1) != Some(hit)
        {
            return None;
        }
        let r = self.ribbon.rect_of(hit)?;
        if self.pointer.1 < r[1] + r[3] * 0.52 {
            return None;
        }
        self.ribbon.release(x, y);
        Some(self.r6_track_menu())
    }
}
