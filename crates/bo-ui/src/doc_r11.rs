//! BlueDoc parity round 11 (docs/ui/REPORT.md "Vòng 101"): Find and Replace › Format ▾ opens
//! the Find Font / Paragraph / Tabs / Language / Frame / Style dialogs over the Find and
//! Replace dialog, and what they set becomes the search's (or the replacement's) format.
//! The navigation pane's Headings tab gets the reference's expand / collapse triangles, the
//! heading right-click menu (Promote, Demote, Move Up / Down, New Heading Before / After, New
//! Subheading, Delete, Select Heading and Content, Expand / Collapse All, Show Heading
//! Levels), dragging a heading (with its content) to another place, and the search box
//! searches as the user types.
//! A child module of the shell.
use super::DocWindow;
use crate::doc_forms::{self, findfmt11, FormContext};
use crate::nav_pane::{NavAction, NavTab};
use crate::popup::{Choice, Menu, MenuItem, MenuLook};
use crate::Input;
use bo_edit::{Pos, Selection};
use bo_engine::edit::Command;
use serde_json::json;

/// Round-11 state of the window.
#[derive(Default)]
pub(super) struct R11 {
    /// The heading (body paragraph id) the navigation pane's menu acts on.
    heading_target: Option<usize>,
    /// A heading drag in the pane moved far enough to be a drag.
    heading_drag: bool,
    press_y: f32,
}

/// The heading menu: label key, en-US label, command, icon, separator above.
const HEADING_MENU: [(&str, &str, &str, Option<&str>, bool); 13] = [
    (
        "nav11.promote",
        "Promote",
        "nav11.promote",
        Some("outline.promote"),
        false,
    ),
    (
        "nav11.demote",
        "Demote",
        "nav11.demote",
        Some("outline.demote"),
        false,
    ),
    (
        "nav11.moveUp",
        "Move Up",
        "nav11.moveUp",
        Some("outline.moveUp"),
        false,
    ),
    (
        "nav11.moveDown",
        "Move Down",
        "nav11.moveDown",
        Some("outline.moveDown"),
        false,
    ),
    (
        "nav11.newBefore",
        "New Heading Before",
        "nav11.newBefore",
        None,
        true,
    ),
    (
        "nav11.newAfter",
        "New Heading After",
        "nav11.newAfter",
        None,
        false,
    ),
    (
        "nav11.newSub",
        "New Subheading",
        "nav11.newSub",
        None,
        false,
    ),
    (
        "nav11.delete",
        "Delete",
        "nav11.delete",
        Some("edit.delete"),
        false,
    ),
    (
        "nav11.select",
        "Select Heading and Content",
        "nav11.select",
        Some("edit.select"),
        true,
    ),
    (
        "nav11.print",
        "Print Heading and Content",
        "nav11.print",
        Some("file.print"),
        false,
    ),
    (
        "nav11.expandAll",
        "Expand All",
        "nav11.expandAll",
        Some("outline.expand"),
        true,
    ),
    (
        "nav11.collapseAll",
        "Collapse All",
        "nav11.collapseAll",
        Some("outline.collapse"),
        false,
    ),
    (
        "nav11.levels",
        "Show Heading Levels",
        "nav11.levels",
        None,
        false,
    ),
];

impl DocWindow {
    fn r11_t(&self, key: &str, fallback: &str) -> String {
        let k = format!("docR11.{key}");
        let v = self.strings.get(&k);
        if v == k {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    /// Pointer and text input for the round-11 parts of the navigation pane: right-click
    /// menu, heading drag, search as you type. `None` lets it through.
    pub(super) fn r11_nav_input(&mut self, input: &Input) -> Option<bool> {
        let nav = self.nav.as_mut()?;
        match *input {
            Input::Context { x, y } if nav.contains(x, y) => {
                let Some(i) = nav.heading_at(x, y) else {
                    return Some(false);
                };
                self.r11.heading_drag = false;
                nav.drag = None;
                Some(self.r11_heading_menu(i))
            }
            Input::Press { y, .. } => {
                self.r11.press_y = y;
                self.r11.heading_drag = false;
                None
            }
            Input::Drag { y, .. } if nav.drag.is_some() => {
                if (y - self.r11.press_y).abs() > 4.0 * self.scale {
                    self.r11.heading_drag = true;
                }
                if self.r11.heading_drag {
                    nav.drag_over(y);
                }
                Some(true)
            }
            Input::Release { .. } if nav.drag.is_some() => {
                let drag = nav.drag.take();
                let moved = std::mem::take(&mut self.r11.heading_drag);
                if let (true, Some((i, Some(g)))) = (moved, drag) {
                    let para = nav.headings.get(i).map(|h| h.para);
                    let before = nav.drop_before(g);
                    if let Some(para) = para.filter(|p| Some(*p) != before) {
                        self.r11_run(vec![json!({
                            "cmd": "moveOutlineTo", "para": para, "before": before,
                        })]);
                    }
                }
                Some(true)
            }
            // The reference searches as the user types (and as letters are deleted).
            Input::Text(ref s) if nav.focus => {
                nav.text(s);
                Some(self.nav_action(NavAction::Search))
            }
            Input::Key(crate::Key::Backspace, m) if nav.focus && !m.ctrl => {
                nav.key(crate::Key::Backspace);
                Some(self.nav_action(NavAction::Search))
            }
            _ => None,
        }
    }

    /// Run engine commands (JSON) and read the headings again.
    fn r11_run(&mut self, cmds: Vec<serde_json::Value>) -> bool {
        for c in cmds {
            if let Some(cmd) = crate::catalog::from_json(c) {
                self.view.run_command(cmd);
            }
        }
        self.refresh_nav();
        true
    }

    /// The right-click menu of heading `i` of the navigation pane, as the reference's.
    pub(super) fn r11_heading_menu(&mut self, i: usize) -> bool {
        let Some(nav) = self.nav.as_ref() else {
            return false;
        };
        let Some(h) = nav.headings.get(i) else {
            return false;
        };
        self.r11.heading_target = Some(h.para);
        let level = h.level;
        let first = i == 0;
        let last = nav.headings.iter().skip(i + 1).all(|x| x.level > level);
        let (x, y) = self.pointer;
        let items: Vec<MenuItem> = HEADING_MENU
            .iter()
            .map(|(k, f, id, _, _)| MenuItem {
                label: self.r11_t(&k["nav11.".len()..], f),
                choice: Choice::Command((*id).to_owned()),
                checked: false,
                dim: match *id {
                    "nav11.promote" => level == 0,
                    "nav11.demote" => level >= 8,
                    "nav11.moveUp" => first,
                    "nav11.moveDown" => last,
                    "nav11.print" => true,
                    _ => false,
                },
            })
            .collect();
        let icons = HEADING_MENU
            .iter()
            .map(|r| r.3.filter(|n| self.icons.has(n)))
            .collect();
        let seps = HEADING_MENU
            .iter()
            .enumerate()
            .filter(|(_, r)| r.4)
            .map(|(i, _)| i)
            .collect();
        let mut arrows = vec![false; HEADING_MENU.len()];
        arrows[HEADING_MENU.len() - 1] = true;
        // 36 device px rows, 304 px wide at 125 % (UI Automation on the reference's menu).
        let look = MenuLook {
            item_h: 36.0 / 1.25,
            min_w: 300.0 / 1.25,
            ..MenuLook::default()
        };
        let menu = Menu::open_with(items, [x, y, 1.0, 0.0], self.size, self.scale, look)
            .with_decor(icons, seps)
            .with_arrows(arrows, self.theme.hover)
            .with_soft_shadow();
        self.menu = Some(menu);
        true
    }

    /// Body paragraph id range of heading `para` and its content: the heading through the
    /// paragraph before the next heading at its level or above.
    fn r11_section(&self, para: usize) -> Option<(usize, usize, u8)> {
        let nav = self.nav.as_ref()?;
        let i = nav.headings.iter().position(|h| h.para == para)?;
        let level = nav.headings[i].level;
        let count = self.view.engine().session()?.paragraph_count();
        let end = nav
            .headings
            .iter()
            .skip(i + 1)
            .find(|h| h.level <= level)
            .map_or(count, |h| h.para);
        Some((para, end.saturating_sub(1).max(para), level))
    }

    /// Select body paragraphs `a` through `b` with the last one's paragraph mark.
    fn r11_select(&mut self, a: usize, b: usize) -> bool {
        let Some(s) = self.view.engine().session() else {
            return false;
        };
        let count = s.paragraph_count();
        let focus = if b + 1 < count {
            Pos::new(b + 1, 0)
        } else {
            Pos::new(b, s.para_text(b).map_or(0, |t| t.len()))
        };
        self.view.run_command(Command::SetSelection {
            selection: Selection {
                anchor: Pos::new(a, 0),
                focus,
            },
        })
    }

    /// A new empty heading paragraph at level `level` after body paragraph `after` (or before
    /// `para` when `after` is `None`), with the caret in it.
    fn r11_new_heading(&mut self, para: usize, after: Option<usize>, level: u8) -> bool {
        let (at, new) = match after {
            Some(a) => {
                let len = self
                    .view
                    .engine()
                    .session()
                    .and_then(|s| s.para_text(a))
                    .map_or(0, |t| t.len());
                (Pos::new(a, len), a + 1)
            }
            None => (Pos::new(para, 0), para),
        };
        self.view.run_command(Command::SetSelection {
            selection: Selection::caret(at),
        });
        self.view.run_command(Command::SplitParagraph);
        self.r11_run(vec![
            json!({ "cmd": "setOutlineLevel", "para": new, "level": level }),
        ]);
        self.view.run_command(Command::SetSelection {
            selection: Selection::caret(Pos::new(new, 0)),
        })
    }

    /// Round-11 commands (the heading menu); `None` for other ids.
    pub(super) fn r11_action(&mut self, id: &str) -> Option<bool> {
        if !id.starts_with("nav11.") && id != crate::doc_table::DIALOGS[0] {
            return None;
        }
        if let Some(n) = id.strip_prefix("nav11.level.") {
            let l = n.parse::<u8>().unwrap_or(9).clamp(1, 9);
            if let Some(nav) = self.nav.as_mut() {
                nav.max_level = l;
                nav.collapsed.clear();
            }
            return Some(true);
        }
        if id == crate::doc_table::DIALOGS[0] {
            return Some(self.r11_table_properties());
        }
        if let Some(n) = id.strip_prefix("nav11.go.") {
            let what = n.parse::<usize>().unwrap_or(9);
            return Some(self.go_to_target(what, "+1"));
        }
        match id {
            "nav11.advFind" => return Some(self.open_find_dialog(0)),
            "nav11.options" | "nav11.none" => return Some(false),
            "nav11.expandAll" => {
                if let Some(nav) = self.nav.as_mut() {
                    nav.collapsed.clear();
                    nav.max_level = 9;
                }
                return Some(true);
            }
            "nav11.collapseAll" => {
                if let Some(nav) = self.nav.as_mut() {
                    nav.collapsed = (0..nav.headings.len())
                        .filter(|&i| nav.has_children(i))
                        .map(|i| nav.headings[i].para)
                        .collect();
                }
                return Some(true);
            }
            "nav11.levels" => return Some(self.r11_levels_menu()),
            _ => {}
        }
        let para = self.r11.heading_target?;
        let (a, b, level) = self.r11_section(para)?;
        let r = match id {
            "nav11.promote" if level > 0 => self.r11_run(vec![
                json!({ "cmd": "setOutlineLevel", "para": para, "level": level - 1 }),
            ]),
            "nav11.demote" if level < 8 => self.r11_run(vec![
                json!({ "cmd": "setOutlineLevel", "para": para, "level": level + 1 }),
            ]),
            "nav11.moveUp" => self.r11_run(vec![
                json!({ "cmd": "moveOutline", "para": para, "up": true }),
            ]),
            "nav11.moveDown" => self.r11_run(vec![
                json!({ "cmd": "moveOutline", "para": para, "up": false }),
            ]),
            "nav11.newBefore" => self.r11_new_heading(para, None, level),
            "nav11.newAfter" => self.r11_new_heading(para, Some(b), level),
            "nav11.newSub" => self.r11_new_heading(para, Some(b), (level + 1).min(8)),
            "nav11.select" => self.r11_select(a, b),
            "nav11.delete" => {
                self.r11_select(a, b);
                self.view.run_command(Command::DeleteBackward);
                self.refresh_nav();
                true
            }
            _ => false,
        };
        if let Some(nav) = self.nav.as_mut() {
            nav.show(NavTab::Headings);
        }
        Some(r)
    }

    /// The navigation pane's search ▾ menu: Options…, Advanced Find…, Replace…, Go To…, then
    /// find the next graphic, table, equation, footnote or comment (the reference's list).
    pub(super) fn r11_search_menu(&mut self) -> bool {
        let rows: [(&str, &str, &str, bool); 10] = [
            ("options", "Options...", "nav11.options", true),
            ("advFind", "Advanced Find...", "nav11.advFind", false),
            ("replace", "Replace...", "edit.replace", false),
            ("goTo", "Go To...", "edit.goTo", false),
            ("findHeader", "Find:", "nav11.none", true),
            ("graphics", "Graphics", "nav11.go.9", false),
            ("tables", "Tables", "nav11.go.8", false),
            ("equations", "Equations", "nav11.go.10", false),
            ("footnotes", "Footnotes/Endnotes", "nav11.go.5", false),
            ("comments", "Comments", "nav11.go.4", false),
        ];
        let items: Vec<MenuItem> = rows
            .iter()
            .map(|(k, f, id, dim)| MenuItem {
                label: self.r11_t(k, f),
                choice: Choice::Command((*id).to_owned()),
                checked: false,
                dim: *dim,
            })
            .collect();
        let (x, y) = self.pointer;
        let look = MenuLook {
            item_h: 36.0 / 1.25,
            min_w: 240.0 / 1.25,
            ..MenuLook::default()
        };
        let menu = Menu::open_with(
            items,
            [x - 1.0, y + 8.0, 1.0, 0.0],
            self.size,
            self.scale,
            look,
        )
        .with_decor(vec![None; rows.len()], vec![4])
        .with_soft_shadow();
        self.menu = Some(menu);
        true
    }

    /// Table Properties (Table / Row / Column / Cell / Alt Text) for the caret's table.
    fn r11_table_properties(&mut self) -> bool {
        let Some(table) = self.view.format().table.clone() else {
            return false;
        };
        let c = FormContext {
            table: Some(table),
            inches: self.units.inches,
            ..FormContext::default()
        };
        self.menu = None;
        self.r3.form = Some(doc_forms::open(
            doc_forms::DocFormKind::TableProps,
            &self.strings,
            &c,
        ));
        self.relayout_pending = true;
        true
    }

    /// Table Properties › Previous / Next Row / Column.
    pub(super) fn r11_table_move(&mut self, cmds: Vec<serde_json::Value>, part: &str) -> bool {
        let mut all = cmds;
        all.push(json!({ "cmd": "tableEdit", "edit": { "kind": "select", "part": part } }));
        for c in all {
            if let Some(cmd) = crate::catalog::from_json(c) {
                self.view.run_command(cmd);
            }
        }
        if let (Some(tbl), Some(f)) = (self.view.format().table.clone(), self.r3.form.as_mut()) {
            f.tp_refresh(&self.strings, &tbl);
        }
        true
    }

    /// Show Heading Levels ▸: All, Level 1 … Level 9 (checked: the levels shown).
    fn r11_levels_menu(&mut self) -> bool {
        let max = self.nav.as_ref().map_or(9, |n| n.max_level);
        let word = self.r11_t("showLevel", "Show Heading {{n}}");
        let mut items: Vec<MenuItem> = (1..=9u8)
            .map(|n| MenuItem {
                label: word.replace("{{n}}", &n.to_string()),
                choice: Choice::Command(format!("nav11.level.{n}")),
                checked: max == n,
                dim: false,
            })
            .collect();
        items.insert(
            0,
            MenuItem {
                label: self.r11_t("allLevels", "All"),
                choice: Choice::Command("nav11.level.9".into()),
                checked: max == 9,
                dim: false,
            },
        );
        let (x, y) = self.pointer;
        let look = MenuLook {
            item_h: 36.0 / 1.25,
            min_w: 200.0 / 1.25,
            ..MenuLook::default()
        };
        self.menu = Some(
            Menu::open_with(items, [x, y, 1.0, 0.0], self.size, self.scale, look)
                .with_soft_shadow(),
        );
        true
    }
    /// Format ▾ entry `i` (0 Font … 5 Style) of the Find and Replace dialog: its Find dialog.
    pub(super) fn r11_open_find_format(&mut self, i: usize) -> bool {
        let kind = findfmt11::kind_of(i);
        let mut c = FormContext::default();
        if i == 5 {
            c.styles = self
                .view
                .all_styles()
                .into_iter()
                .map(|s| (s.id, s.name, s.kind == "paragraph"))
                .collect();
        }
        if i == 0 {
            c.fonts = self.view.font_list().into_iter().map(|f| f.name).collect();
        }
        self.r3.form = Some(doc_forms::open(kind, &self.strings, &c));
        self.relayout_pending = true;
        true
    }

    /// A Find dialog's OK: its part of the focused box's format.
    pub(super) fn r11_find_format(
        &mut self,
        group: &str,
        v: serde_json::Value,
        words: String,
    ) -> bool {
        self.r3.form = None;
        if let Some(fd) = self.find_dialog.as_mut() {
            fd.set_format(group, v, words);
        }
        true
    }
}

#[cfg(test)]
#[path = "doc_r11_tests.rs"]
mod tests;
