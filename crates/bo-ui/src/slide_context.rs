//! BlueSlide's context menus (right button), as the reference presentation app shows them:
//!
//! - over a misspelt word: its corrections (up to five), Ignore All, Add to Dictionary, then
//!   the clipboard commands;
//! - over the selection on the slide: Cut, Copy, Paste, then Bring to Front / Send to Back,
//!   Group / Ungroup and New Comment;
//! - over the slide with nothing selected: Paste, then Layout, Reset Slide, Format
//!   Background and New Comment;
//! - over a thumbnail: Cut, Copy, Paste, then New Slide, Duplicate Slide, Delete Slide,
//!   Hide Slide and New Comment.
use super::{MenuKind, SlideWindow};
use crate::popup::{Choice, Menu, MenuItem};

/// What a context menu was opened over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Over {
    /// A thumbnail of the slides pane.
    Thumbnail,
    /// The selected shapes.
    Selection,
    /// The slide, nothing selected.
    Slide,
}

/// The commands of each menu; `None` draws a separator before the next one.
fn ids(over: Over) -> &'static [Option<&'static str>] {
    match over {
        Over::Thumbnail => &[
            Some("edit.cut"),
            Some("edit.copy"),
            Some("edit.paste"),
            None,
            Some("slide.new"),
            Some("slide.duplicate"),
            Some("slide.delete"),
            None,
            Some("show.hideSlide"),
            Some("review.newComment"),
        ],
        Over::Selection => &[
            Some("edit.cut"),
            Some("edit.copy"),
            Some("edit.paste"),
            None,
            Some("arrange.group"),
            Some("arrange.ungroup"),
            Some("arrange.front"),
            Some("arrange.back"),
            None,
            Some("review.newComment"),
        ],
        Over::Slide => &[
            Some("edit.cut"),
            Some("edit.copy"),
            Some("edit.paste"),
            None,
            Some(LAYOUT),
            Some("slide.reset"),
            Some(FORMAT_BG),
            None,
            Some("review.newComment"),
        ],
    }
}

/// Layout (the layout menu opens at the pointer).
const LAYOUT: &str = "ctxsp:layout";

/// Format Background… (opens the pane, as the Design tab's menu item does).
const FORMAT_BG: &str = "ins:formatBg";

/// Icons drawn beside the clipboard commands (as in BlueDoc's context menu).
fn icon(id: &'static str) -> Option<&'static str> {
    matches!(
        id,
        "edit.cut" | "edit.copy" | "edit.paste" | "review.newComment"
    )
    .then_some(id)
}

impl SlideWindow {
    /// Open the context menu for a right click at a window position; `false` when the
    /// position has none (ribbon, status bar, panes).
    pub(super) fn open_context(&mut self, x: f32, y: f32) -> bool {
        let over = match self.view.thumb_under(x, y) {
            Some(Some(i)) => {
                if !self.view.selected_slides().contains(&i) {
                    self.view.go_to(i);
                }
                Over::Thumbnail
            }
            Some(None) => return false,
            None => {
                let [cx, cy, cw, ch] = self.view.canvas_rect();
                if x < cx || y < cy || x >= cx + cw || y >= cy + ch {
                    return false;
                }
                if self.view.selection().is_empty() && self.view.text_edit().is_none() {
                    Over::Slide
                } else {
                    Over::Selection
                }
            }
        };
        let selected = over == Over::Thumbnail || !self.view.selection().is_empty();
        let mut items = Vec::new();
        let mut icons = Vec::new();
        let mut separators = Vec::new();
        // Over a misspelt word: its corrections first, then ignore / add.
        self.ctx_issue = if over == Over::Thumbnail {
            None
        } else {
            self.view.spell_at(x, y)
        };
        if let Some(hit) = &self.ctx_issue {
            let sugg: Vec<String> = hit
                .get("suggestions")
                .and_then(serde_json::Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(str::to_owned))
                        .take(5)
                        .collect()
                })
                .unwrap_or_default();
            if sugg.is_empty() {
                items.push(MenuItem {
                    label: self.strings.get("spell.noSuggestions").to_owned(),
                    choice: Choice::Command("ctxsp:none".into()),
                    checked: false,
                    dim: true,
                });
            }
            for (i, s) in sugg.into_iter().enumerate() {
                items.push(MenuItem {
                    label: s,
                    choice: Choice::Command(format!("ctxsp:{i}")),
                    checked: false,
                    dim: false,
                });
            }
            let grammar = hit
                .get("grammar")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if !grammar {
                for (key, c) in [
                    ("spell.ignoreAll", "ctxsp:ignore"),
                    ("spell.addToDictionary", "ctxsp:add"),
                ] {
                    items.push(MenuItem {
                        label: self.strings.get(key).to_owned(),
                        choice: Choice::Command(c.into()),
                        checked: false,
                        dim: false,
                    });
                }
            }
            icons.resize(items.len(), None);
            separators.push(items.len());
        }
        for id in ids(over) {
            let Some(id) = *id else {
                separators.push(items.len());
                continue;
            };
            let label = if id == FORMAT_BG {
                self.t("slideNative.formatBackgroundMenu", "Format Background...")
            } else if id == LAYOUT {
                self.strings.get("commands.slide.layout").to_owned()
            } else if id == "slide.reset" {
                self.t("slideNative.resetSlideMenu", "Reset Slide")
            } else {
                self.strings.get(&format!("commands.{id}")).to_owned()
            };
            items.push(MenuItem {
                label,
                choice: Choice::Command(id.to_owned()),
                checked: false,
                dim: !selected && matches!(id, "edit.cut" | "edit.copy"),
            });
            icons.push(icon(id));
        }
        self.menu_kind = MenuKind::Duration;
        self.ctx_at = [x, y - 2.0, 1.0, 0.0];
        self.menu = Some(
            Menu::open(items, self.ctx_at, self.size, self.scale).with_decor(icons, separators),
        );
        true
    }

    /// A spelling choice of the context menu (`ctxsp:<n>` / `ignore` / `add`).
    pub(super) fn choose_context(&mut self, v: &str) -> bool {
        if v == "layout" {
            self.menu_anchor = Some(self.ctx_at);
            self.open_menu(
                MenuKind::Layout,
                crate::ribbon::RibbonHit::Command("slide.layout"),
            );
            return true;
        }
        let Some(hit) = self.ctx_issue.take() else {
            return false;
        };
        let word = hit
            .get("word")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        match v {
            "ignore" => self.view.spell_learn(&word, false),
            "add" => self.view.spell_learn(&word, true),
            _ => {
                let Some(s) = v.parse::<usize>().ok().and_then(|i| {
                    hit.get("suggestions")
                        .and_then(|a| a.get(i))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                }) else {
                    return false;
                };
                self.view.spell_fix(&hit, &s)
            }
        }
    }
}
