//! BlueReader round 17 (a child of `reader_window`), measured on the reference reader with
//! posted messages on a scratch copy (`docs/ui/REPORT-reader.md`, "Vòng 17"):
//!
//! - Dragging over text selects to the nearest character boundary at both ends (the reference:
//!   a press in the left half of a letter keeps it, a release in the left half leaves it out);
//!   word snapping only after a double click. Dragging past the top or bottom of the view keeps
//!   scrolling while the button is held (the host's edge timer calls [`ReaderWindow::edge_step17`]).
//! - Cascading context menus: the comment menu's Align ▸ / Center ▸ / Distribute ▸ and
//!   Set Status ▸ open beside their row on hover, → or Enter, as the reference's; ← or Esc
//!   closes only the cascade. Center moves the comment to the page's middle.
//! - Review states None, Accepted, Rejected, Cancelled, Completed, Deferred, Future (the
//!   reference's list; ISO 32000-1 §12.5.6.3 Table 172 names the first five, `/State` is open).
//! - Selecting a note comment shows the Format pane with Arrange › Page Center on.
use super::{ReaderWindow, Tool};
use crate::popup::{Choice, Menu, MenuItem};
use crate::reader_view::Selection;
use crate::{Input, Key, Painter, TextMeasure};
use bo_pdfread::{AnnotKind, TextLayer};

/// Rows of a context menu that open a cascade beside them.
const CASCADES: [&str; 4] = [
    "comment.setStatus",
    "arrange17.align",
    "arrange17.center",
    "arrange17.distribute",
];

/// The reference's review states, in its order.
pub(crate) const STATES17: [&str; 7] = [
    "None",
    "Accepted",
    "Rejected",
    "Cancelled",
    "Completed",
    "Deferred",
    "Future",
];

/// Round 17 state of the window.
#[derive(Default)]
pub(crate) struct R17 {
    /// The open cascade and the parent row it belongs to.
    sub: Option<(Menu, usize)>,
    /// A character drag: its page and the caret boundary where it started.
    pub(crate) anchor: Option<(usize, usize)>,
    /// The pointer while a selection drag is past the top or bottom of the view.
    edge: Option<(f32, f32)>,
    /// Pages the next Print dialog starts with (a bookmark's Print Page(s) / Section(s)).
    pub(crate) print_pages: Option<String>,
}

/// The caret boundary nearest to (`x`, `y`) (page points): before the character under the
/// point when in its left half, after it when in its right half.
pub(crate) fn boundary17(layer: &TextLayer, x: f32, y: f32) -> Option<usize> {
    let c = layer.hit(x, y, f32::INFINITY)?;
    let r = layer.chars().get(c)?.rect;
    Some(if x > r[0].midpoint(r[2]) { c + 1 } else { c })
}

/// Characters between two boundaries as an inclusive selection, `None` when empty.
pub(crate) fn span17(page: usize, from: usize, to: usize) -> Option<Selection> {
    match from.cmp(&to) {
        std::cmp::Ordering::Less => Some(Selection {
            page,
            a: from,
            b: to - 1,
        }),
        std::cmp::Ordering::Greater => Some(Selection {
            page,
            a: to,
            b: from - 1,
        }),
        std::cmp::Ordering::Equal => None,
    }
}

impl ReaderWindow {
    // ---- Selection drags ---------------------------------------------------------------------

    /// A plain press over text at `c` started a drag: remember its boundary.
    pub(crate) fn start_drag17(&mut self, page: usize, x: f32, y: f32) {
        self.r17.anchor = self.tabs.get_mut(self.active).and_then(|t| {
            let (_, px, py) = t.view.page_at(x, y)?;
            let b = boundary17(t.view.text(page)?, px, py)?;
            Some((page, b))
        });
    }

    /// A character drag moved to (`x`, `y`): select between the boundaries. `None` when the
    /// drag is not a character drag (word drags keep the old way).
    pub(crate) fn drag17(&mut self, page: usize, x: f32, y: f32) -> Option<bool> {
        let (p, from) = self.r17.anchor.filter(|a| a.0 == page)?;
        let t = self.tabs.get_mut(self.active)?;
        let s = t.view.px_per_pt();
        let r = t.view.page_rect(p)?;
        let (px, py) = ((x - r[0]) / s, (y - r[1]) / s);
        if let Some(to) = t.view.text(p).and_then(|l| boundary17(l, px, py)) {
            t.view.set_selection(span17(p, from, to));
        }
        let f = t.view.frame();
        self.r17.edge = (y > f[1] + f[3] || y < f[1]).then_some((x, y));
        Some(true)
    }

    /// The drag ended.
    pub(crate) fn release17(&mut self) {
        self.r17.anchor = None;
        self.r17.edge = None;
    }

    /// A selection drag is past the view's top or bottom (the host runs its edge timer).
    pub fn edge_scroll17(&self) -> bool {
        self.r17.edge.is_some() && self.r17.anchor.is_some()
    }

    /// Edge timer: `ms` milliseconds passed with the pointer past the view; scroll by the
    /// distance (100 px a second at the edge, 16.5 px a second more for every further pixel
    /// at 125 %, the slope of the middle-button scroll) and extend the selection.
    pub fn edge_step17(&mut self, ms: f32) -> bool {
        let Some((x, y)) = self.r17.edge else {
            return false;
        };
        let Some((page, _)) = self.r17.anchor else {
            return false;
        };
        let Some(t) = self.tabs.get_mut(self.active) else {
            return false;
        };
        let f = t.view.frame();
        let d = if y > f[1] + f[3] {
            y - f[1] - f[3]
        } else if y < f[1] {
            y - f[1]
        } else {
            return false;
        };
        let speed = 100.0 * self.scale / 1.25 + 16.5 * d.abs();
        t.view.scroll_by(0.0, speed * ms / 1000.0 * d.signum());
        self.drag17(page, x, y);
        true
    }

    // ---- Cascading menus ---------------------------------------------------------------------

    /// Input for an open context menu's cascades; `Some(redraw)` when taken.
    pub(crate) fn route17(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if self.menu.is_none() {
            self.r17.sub = None;
            return None;
        }
        if self.r17.sub.is_some() {
            return self.route_sub17(input, m);
        }
        let cascade = self.hovered_cascade17();
        match *input {
            Input::Move { x, y } | Input::Drag { x, y } => {
                let (menu, _) = self.menu.as_mut()?;
                if !menu.contains(x, y) {
                    return None;
                }
                let changed = menu.hover(x, y);
                if let Some(i) = self.hovered_cascade17() {
                    self.open_cascade17(i, false, m);
                    return Some(true);
                }
                Some(changed)
            }
            Input::Release { x, y } => {
                let (menu, _) = self.menu.as_mut()?;
                if !menu.contains(x, y) {
                    return None;
                }
                menu.hover(x, y);
                let i = self.hovered_cascade17()?;
                if let Some((menu, _)) = self.menu.as_mut() {
                    // Forget the press: the row opens its cascade instead of running.
                    menu.release(-1.0, -1.0);
                }
                self.open_cascade17(i, false, m);
                Some(true)
            }
            Input::Key(Key::Right | Key::Enter, _) => {
                let i = cascade?;
                self.open_cascade17(i, true, m);
                Some(true)
            }
            _ => None,
        }
    }

    fn route_sub17(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        let (sub, parent) = self.r17.sub.as_mut()?;
        let parent = *parent;
        match *input {
            Input::Move { x, y } | Input::Drag { x, y } => {
                if sub.contains(x, y) {
                    return Some(sub.hover(x, y));
                }
                let (menu, _) = self.menu.as_mut()?;
                if !menu.contains(x, y) {
                    return Some(false);
                }
                menu.hover(x, y);
                if menu.hovered() == Some(parent) {
                    return Some(false);
                }
                self.r17.sub = None;
                if let Some(i) = self.hovered_cascade17() {
                    self.open_cascade17(i, false, m);
                }
                Some(true)
            }
            Input::Press { x, y, .. } => {
                if sub.contains(x, y) {
                    sub.press(x, y);
                    return Some(true);
                }
                if self.menu.as_ref().is_some_and(|(p, _)| p.contains(x, y)) {
                    self.r17.sub = None;
                    return None;
                }
                self.r17.sub = None;
                self.menu = None;
                Some(true)
            }
            Input::Release { x, y } => {
                if !sub.contains(x, y) {
                    return Some(false);
                }
                let i = sub.hovered();
                let chosen = sub.release(x, y);
                self.choose_sub17(i, chosen, m);
                Some(true)
            }
            Input::Key(Key::Escape | Key::Left, _) => {
                self.r17.sub = None;
                Some(true)
            }
            Input::Key(Key::Down, _) => {
                sub.step(1);
                Some(true)
            }
            Input::Key(Key::Up, _) => {
                sub.step(-1);
                Some(true)
            }
            Input::Key(Key::Enter | Key::Right, _) => {
                let i = sub.hovered();
                let chosen = sub.current();
                self.choose_sub17(i, chosen, m);
                Some(true)
            }
            Input::Wheel(_) | Input::Text(_) => Some(false),
            _ => None,
        }
    }

    /// A cascade row was chosen: run it unless dim, and close the menus.
    fn choose_sub17(&mut self, i: Option<usize>, chosen: Option<Choice>, m: &dyn TextMeasure) {
        let dim = match (&self.r17.sub, i) {
            (Some((sub, _)), Some(i)) => sub.items().get(i).is_none_or(|it| it.dim),
            _ => true,
        };
        if dim {
            return;
        }
        self.r17.sub = None;
        self.menu = None;
        if let Some(Choice::Command(cmd)) = chosen {
            self.command(&cmd, m);
        }
    }

    /// The parent menu's highlighted row when it opens a cascade and is not dim.
    fn hovered_cascade17(&self) -> Option<usize> {
        let (menu, _) = self.menu.as_ref()?;
        let i = menu.hovered()?;
        let it = menu.items().get(i)?;
        match &it.choice {
            Choice::Command(c) if CASCADES.contains(&c.as_str()) && !it.dim => Some(i),
            _ => None,
        }
    }

    /// Open the cascade of parent row `i` beside it (left of the menu when there is no room
    /// on the right); `keyboard` highlights its first row.
    fn open_cascade17(&mut self, i: usize, keyboard: bool, m: &dyn TextMeasure) {
        let Some((menu, _)) = self.menu.as_ref() else {
            return;
        };
        let Some(Choice::Command(cmd)) = menu.items().get(i).map(|it| it.choice.clone()) else {
            return;
        };
        let (pr, rr) = (menu.rect(), menu.row_rect(i));
        let (items, seps) = self.cascade_items17(&cmd);
        // Measured: the cascade overlaps the menu by 1 px and its top is 2 px above the row
        // (device pixels at 125 %).
        let one = self.scale.max(1.0).round();
        let gap = (2.0 * self.scale).round();
        let top = rr[1] - (1.6 * self.scale).round();
        let build = |w: &Self, x: f32| {
            let sub = w
                .context_menu9(items.clone(), [x, top - gap, 0.0, 0.0])
                .with_decor(Vec::new(), seps.clone());
            w.keyed_menu9(sub, &[], m)
        };
        let probe = build(self, pr[0] + pr[2] - one);
        let sw = probe.rect()[2];
        let x = if pr[0] + pr[2] - one + sw <= self.size.0 {
            pr[0] + pr[2] - one
        } else {
            (pr[0] - sw + one).max(0.0)
        };
        let mut sub = build(self, x);
        if keyboard {
            sub.set_hovered(sub.items().iter().position(|it| !it.dim).or(Some(0)));
        } else if sub.items().iter().any(|it| it.checked) {
            // The current state is not pre-highlighted on hover (the reference).
            sub.set_hovered(None);
        }
        self.r17.sub = Some((sub, i));
    }

    /// Rows and separators of a cascade.
    fn cascade_items17(&self, cmd: &str) -> (Vec<MenuItem>, Vec<usize>) {
        let item = |label: String, cmd: String, checked: bool, dim: bool| MenuItem {
            label,
            choice: Choice::Command(cmd),
            checked,
            dim,
        };
        match cmd {
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
                let items = STATES17
                    .iter()
                    .map(|s| {
                        let key = if matches!(*s, "Deferred" | "Future") {
                            format!("pdfReader17.state{s}")
                        } else {
                            format!("pdfPanels.state{s}")
                        };
                        item(
                            self.t(&key),
                            format!("comment.state.{s}"),
                            *s == current,
                            false,
                        )
                    })
                    .collect();
                (items, Vec::new())
            }
            // One comment selected: aligning and distributing need two or more (dim, as the
            // reference shows them).
            "arrange17.align" => (
                [
                    "left",
                    "right",
                    "top",
                    "bottom",
                    "vertically",
                    "horizontally",
                ]
                .iter()
                .map(|k| {
                    item(
                        self.t(&format!("pdfReader17.{k}")),
                        format!("arrange17.align.{k}"),
                        false,
                        true,
                    )
                })
                .collect(),
                vec![4],
            ),
            "arrange17.center" => (
                ["vertically", "horizontally", "both"]
                    .iter()
                    .map(|k| {
                        item(
                            self.t(&format!("pdfReader17.{k}")),
                            format!("arrange17.center.{k}"),
                            false,
                            self.extra.selected.is_none(),
                        )
                    })
                    .collect(),
                Vec::new(),
            ),
            _ => (
                ["vertically", "horizontally"]
                    .iter()
                    .map(|k| {
                        item(
                            self.t(&format!("pdfReader17.{k}")),
                            format!("arrange17.distribute.{k}"),
                            false,
                            true,
                        )
                    })
                    .collect(),
                Vec::new(),
            ),
        }
    }

    // ---- Bookmarks ---------------------------------------------------------------------------

    /// The context menu on a bookmark, as the reference's (measured round 17): Go to Bookmark,
    /// Print Page(s), Print Section(s) | Expand/Collapse All Bookmarks.
    pub(crate) fn bookmark_menu17(&mut self, x: f32, y: f32, m: &dyn TextMeasure) {
        let has_page = self.bookmark_pages17(false).is_some();
        let item = |label: String, cmd: &str, dim: bool| MenuItem {
            label,
            choice: Choice::Command(cmd.to_owned()),
            checked: false,
            dim,
        };
        let items = vec![
            item(self.t("pdfReader17.goBookmark"), "bm17.go", !has_page),
            item(
                self.t("pdfReader17.printPages"),
                "bm17.printPages",
                !has_page,
            ),
            item(
                self.t("pdfReader17.printSection"),
                "bm17.printSection",
                !has_page,
            ),
            item(self.t("pdfReader17.toggleAll"), "bm17.toggleAll", false),
        ];
        let menu = self
            .context_menu9(items, [x, y, 0.0, 0.0])
            .with_decor(Vec::new(), vec![3]);
        let menu = self.keyed_menu9(menu, &[], m);
        self.menu = Some((menu, "bookmarks17"));
    }

    /// The selected bookmark's page (`section`: to the page before the next bookmark that is
    /// not inside it, or the last page), 0-based.
    fn bookmark_pages17(&self, section: bool) -> Option<(usize, usize)> {
        let t = self.tabs.get(self.active)?;
        let path = self.selected_outline.as_ref()?;
        let mut flat = Vec::new();
        flatten17(&t.outline, &mut Vec::new(), &mut flat);
        let k = flat.iter().position(|(p, _)| p == path)?;
        let first = flat[k].1?;
        if !section {
            return Some((first, first));
        }
        let last_page = t.view.page_count().saturating_sub(1);
        let next = flat[k + 1..]
            .iter()
            .find(|(p, _)| !p.starts_with(path))
            .and_then(|(_, pg)| *pg);
        let last = next.map_or(last_page, |n| n.saturating_sub(1).max(first));
        Some((first, last))
    }

    /// Commands of round 17; true when handled.
    pub(crate) fn command17(&mut self, cmd: &str) -> bool {
        match cmd {
            "bm17.go" => {
                if let (Some((p, _)), Some(t)) = (
                    self.bookmark_pages17(false),
                    self.tabs.get(self.active).map(|t| t.outline.clone()),
                ) {
                    let top = self.selected_outline.as_ref().and_then(|path| {
                        let mut flat = Vec::new();
                        flatten_targets17(&t, &mut Vec::new(), &mut flat);
                        flat.into_iter().find(|(q, _)| q == path).and_then(|f| f.1)
                    });
                    if let Some(t) = self.tabs.get_mut(self.active) {
                        t.view.go_to(p, top);
                    }
                }
                return true;
            }
            "bm17.printPages" | "bm17.printSection" => {
                if let Some((a, b)) = self.bookmark_pages17(cmd == "bm17.printSection") {
                    self.r17.print_pages = Some(if a == b {
                        (a + 1).to_string()
                    } else {
                        format!("{} - {}", a + 1, b + 1)
                    });
                    self.requests.push(super::ReaderRequest::Print);
                }
                return true;
            }
            "bm17.toggleAll" => {
                let all_open = self.tabs.get(self.active).is_some_and(|t| {
                    let mut parents = std::collections::HashSet::new();
                    super::bm5::all_parents(&t.outline, &mut Vec::new(), &mut parents);
                    !parents.is_empty() && parents.iter().all(|p| t.expanded.contains(p))
                });
                return self.command_bookmarks17(if all_open {
                    "bm.collapseAll"
                } else {
                    "bm.expandAll"
                });
            }
            _ => {}
        }
        let Some(how) = cmd.strip_prefix("arrange17.center.") else {
            return cmd.starts_with("arrange17.");
        };
        let (v, h) = match how {
            "vertically" => (true, false),
            "horizontally" => (false, true),
            _ => (true, true),
        };
        self.center17(v, h);
        true
    }

    /// Move the selected comment to the middle of its page: up/down (`v`), sideways (`h`).
    pub(crate) fn center17(&mut self, v: bool, h: bool) -> bool {
        let Some(a) = self.extra.selected.clone() else {
            return false;
        };
        let Some(t) = self.tabs.get_mut(self.active) else {
            return false;
        };
        let (pw, ph) = t.view.page_size(a.page);
        let r = a.rect;
        let dx = if h {
            (pw - (r[2] - r[0])) / 2.0 - r[0]
        } else {
            0.0
        };
        let dy = if v {
            (ph - (r[3] - r[1])) / 2.0 - r[1]
        } else {
            0.0
        };
        if dx.abs() < 0.01 && dy.abs() < 0.01 {
            return false;
        }
        if let Err(e) = t.view.doc_mut().move_annotation(&a, dx, dy) {
            self.status = e;
            return false;
        }
        self.after_comment_change(Some(r), (dx, dy));
        true
    }

    /// Run a bookmark panel command of round 5.
    fn command_bookmarks17(&mut self, cmd: &str) -> bool {
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.expanded.clear();
            if cmd == "bm.expandAll" {
                let outline = t.outline.clone();
                super::bm5::all_parents(&outline, &mut Vec::new(), &mut t.expanded);
            }
            t.panel_scroll = 0.0;
        }
        true
    }

    /// The Format pane also shows while a note comment is selected with the Hand or Select
    /// tool (the reference opens it when a note is selected).
    pub(crate) fn note_selected17(&self) -> bool {
        matches!(self.tool, Tool::Hand | Tool::Select)
            && self
                .extra
                .selected
                .as_ref()
                .is_some_and(|a| a.kind == AnnotKind::Note)
    }

    // ---- Assistive technology ----------------------------------------------------------------

    /// [`ReaderWindow::accessibility16`] with the open menu, its cascade and an open dialog:
    /// a Menu element and a MenuItem per row (`menuitem:<n>`, `subitem:<n>`), a Window for the
    /// dialog and a Button per dialog button (`dialogbtn:<n>`).
    pub fn accessibility17(&self) -> Vec<crate::AccessItem> {
        use crate::AccessRole;
        let mut out = self.accessibility16();
        let item = |key: String, name: String, role, rect, enabled, selected| crate::AccessItem {
            key,
            name,
            role,
            rect,
            enabled,
            selected,
        };
        let menus = self
            .menu
            .as_ref()
            .map(|(m, _)| (m, "menuitem"))
            .into_iter()
            .chain(self.r17.sub.as_ref().map(|(m, _)| (m, "subitem")));
        for (menu, kind) in menus {
            out.push(item(
                kind.replace("item", ""),
                self.t("pdfReader17.menu"),
                AccessRole::Menu,
                menu.rect(),
                true,
                false,
            ));
            for (i, it) in menu.items().iter().enumerate() {
                out.push(item(
                    format!("{kind}:{i}"),
                    it.label.clone(),
                    AccessRole::MenuItem,
                    menu.row_rect(i),
                    !it.dim,
                    menu.hovered() == Some(i),
                ));
            }
        }
        if let Some((d, _)) = &self.dialog {
            let (title, rect, buttons) = d.access_parts();
            let name = if title.is_empty() {
                self.t("pdfReader17.dialog")
            } else {
                title.to_owned()
            };
            out.push(item(
                "dialog".into(),
                name,
                AccessRole::Window,
                rect,
                true,
                false,
            ));
            for (i, (label, r)) in buttons.into_iter().enumerate() {
                out.push(item(
                    format!("dialogbtn:{i}"),
                    label,
                    AccessRole::Button,
                    r,
                    true,
                    false,
                ));
            }
        }
        out
    }

    /// Key of the focused element: the highlighted menu row (cascade first), the dialog, or
    /// [`ReaderWindow::access_focus16`].
    pub fn access_focus17(&self) -> Option<String> {
        if let Some((sub, _)) = &self.r17.sub {
            return Some(
                sub.hovered()
                    .map_or("sub".into(), |i| format!("subitem:{i}")),
            );
        }
        if let Some((menu, _)) = &self.menu {
            return Some(
                menu.hovered()
                    .map_or("menu".into(), |i| format!("menuitem:{i}")),
            );
        }
        if self.dialog.is_some() {
            return Some("dialog".into());
        }
        self.access_focus16()
    }

    /// UI Automation Invoke: a menu row or a dialog button is clicked as the pointer would
    /// (the same routing: cascades open, dim rows do nothing); others go to round 12.
    pub fn invoke17(&mut self, key: &str, m: &dyn TextMeasure) -> bool {
        let at = |r: [f32; 4]| (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        let point = if let Some(i) = key.strip_prefix("menuitem:").and_then(|v| v.parse().ok()) {
            self.menu
                .as_ref()
                .filter(|(menu, _)| i < menu.items().len())
                .map(|(menu, _)| at(menu.row_rect(i)))
        } else if let Some(i) = key.strip_prefix("subitem:").and_then(|v| v.parse().ok()) {
            self.r17
                .sub
                .as_ref()
                .filter(|(menu, _)| i < menu.items().len())
                .map(|(menu, _)| at(menu.row_rect(i)))
        } else if let Some(i) = key
            .strip_prefix("dialogbtn:")
            .and_then(|v| v.parse::<usize>().ok())
        {
            self.dialog
                .as_ref()
                .and_then(|(d, _)| d.access_parts().2.get(i).map(|b| at(b.1)))
        } else {
            return self.invoke12(key, m);
        };
        let Some((x, y)) = point else {
            return false;
        };
        self.handle(Input::Move { x, y }, m);
        self.handle(
            Input::Press {
                x,
                y,
                count: 1,
                shift: false,
            },
            m,
        );
        self.handle(Input::Release { x, y }, m);
        true
    }

    // ---- Paint ---------------------------------------------------------------------------------

    pub(crate) fn paint17(&self, p: &mut dyn Painter) {
        if let Some((sub, _)) = &self.r17.sub {
            sub.paint(p, &self.theme, &self.icons);
        }
    }

    /// The open cascade (tests).
    #[cfg(test)]
    pub(crate) fn sub_labels17(&self) -> Vec<(String, bool, bool)> {
        self.r17
            .sub
            .as_ref()
            .map(|(s, _)| {
                s.items()
                    .iter()
                    .map(|i| (i.label.clone(), i.dim, i.checked))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Row `i` of the open cascade (tests).
    #[cfg(test)]
    pub(crate) fn sub_row17(&self, i: usize) -> Option<[f32; 4]> {
        self.r17.sub.as_ref().map(|(s, _)| s.row_rect(i))
    }

    /// The open cascade's box (tests).
    #[cfg(test)]
    pub(crate) fn sub_rect17(&self) -> Option<[f32; 4]> {
        self.r17.sub.as_ref().map(|(s, _)| s.rect())
    }
}

/// Outline entries in reading order: path and target page.
fn flatten17(
    items: &[bo_pdfread::OutlineItem],
    path: &mut Vec<usize>,
    out: &mut Vec<(Vec<usize>, Option<usize>)>,
) {
    for (i, it) in items.iter().enumerate() {
        path.push(i);
        let page = match &it.target {
            Some(bo_pdfread::LinkTarget::Page { index, .. }) => Some(*index),
            _ => None,
        };
        out.push((path.clone(), page));
        flatten17(&it.children, path, out);
        path.pop();
    }
}

/// Outline entries in reading order: path and target top (page points).
fn flatten_targets17(
    items: &[bo_pdfread::OutlineItem],
    path: &mut Vec<usize>,
    out: &mut Vec<(Vec<usize>, Option<f32>)>,
) {
    for (i, it) in items.iter().enumerate() {
        path.push(i);
        let top = match &it.target {
            Some(bo_pdfread::LinkTarget::Page { top, .. }) => *top,
            _ => None,
        };
        out.push((path.clone(), top));
        flatten_targets17(&it.children, path, out);
        path.pop();
    }
}

#[cfg(test)]
#[path = "reader_r17_tests.rs"]
mod tests;
