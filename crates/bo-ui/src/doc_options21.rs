//! File › Options › Customize Ribbon and Quick Access Toolbar pages of round 21 (docs/ui/
//! REPORT.md "Vòng 111"), laid out as the reference's (UI Automation of its Options dialog
//! at 125 %, `ref-o-CustomizeRibbon` / `ref-o-QuickAccessToolbar`): "Choose commands from"
//! ▾ with its list (a tree for the tab categories), Add >> / << Remove, "Customize the
//! Ribbon" ▾ with the tab → group → command tree and its check boxes, New Tab / New Group /
//! Rename…, ↑ ↓, Reset ▾ and Import/Export ▾ (BlueOffice's own customisation file); the
//! Quick Access Toolbar's two lists, <Separator>, Show below the Ribbon. The model is
//! `doc_ribbon21`. A child module of `doc_options6` (it reads the dialog's fields).
use super::super::form::{Event, Kind};
use super::{OptOut, OptionsDialog};
use crate::doc_ribbon21::{self as rb, CGroup, CTab, Layout, SEPARATOR};
use crate::icons::{IconColors, Icons};
use crate::strings::Strings;
use crate::{Input, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};
use std::collections::{HashMap, HashSet};

/// Pages of the Options dialog.
const RIBBON: usize = 7;
const QAT: usize = 8;
/// Rows: tabs 33 px, the rest 28 px (reference).
const TAB_ROW: f32 = 33.0;
const ROW: f32 = 28.0;
/// Scroll bar width (reference).
const BAR: f32 = 21.0;

/// What the dialog asks of the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Out {
    /// Pick a customisation file to import (the host reads it, then
    /// [`OptionsDialog::r21_import`]).
    Import,
    /// Save this customisation file text.
    Export(String),
    /// Keyboard shortcuts: Customize… (the Customize Keyboard dialog).
    Keyboard,
}

/// A row of a tree (or flat list).
#[derive(Debug, Clone, PartialEq)]
struct Row {
    level: u8,
    node: Node,
    label: String,
    /// `Some(expanded)` when the row has children.
    open: Option<bool>,
    check: Option<bool>,
    icon: Option<&'static str>,
    bold: bool,
    height: f32,
}

/// What a row stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Header,
    /// Right tree: tab / group / command by index into the layout.
    Tab(usize),
    Group(usize, usize),
    Cmd(usize, usize, usize),
    /// Left list: a source tab, a source group (tab, group label), a command.
    STab(&'static str),
    SGroup(&'static str, &'static str),
    SCmd(&'static str),
    /// Quick Access Toolbar entry (index).
    Quick(usize),
    Separator,
}

/// A scrolling list drawn by this module.
#[derive(Debug, Clone, Default)]
struct View {
    sel: Option<Node>,
    /// Scroll offset (reference pixels).
    top: f32,
    open: HashSet<String>,
}

/// Popup menu of Reset ▾ / Import/Export ▾.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pop {
    Reset,
    Io,
}

/// Round-21 state of the Options dialog.
pub(crate) struct State {
    layout: Layout,
    /// "Customize the Ribbon": 0 Main Tabs, 1 Tool Tabs, 2 All Tabs.
    scope: usize,
    left: View,
    right: View,
    qleft: View,
    qright: View,
    qat: Vec<String>,
    labels: HashMap<&'static str, String>,
    /// Built-in tab and group names (label key → text).
    names: HashMap<String, String>,
    custom_suffix: String,
    separator: String,
    headers: [String; 3],
    /// Open popup menu and its entries.
    pop: Option<(Pop, usize)>,
    pop_items: [[String; 2]; 2],
    /// Rename… target (right tree node) while the rename panel is open.
    rename: Option<Node>,
    new_tab: String,
    new_group: String,
    /// Messages: commands must go into custom groups; built-in tabs cannot be removed.
    msgs: [String; 3],
    /// "Choose commands from" of each page.
    src_cache: usize,
    qsrc: usize,
}

fn tr(s: &Strings, key: &str, fallback: &str) -> String {
    let v = s.get(key);
    if v == key {
        fallback.to_owned()
    } else {
        v.to_owned()
    }
}

impl State {
    pub(crate) fn new(s: &Strings, p: &super::DocPrefs) -> Self {
        let mut layout = p.extra.ribbon.full();
        // Round 20's hidden tabs.
        for t in &mut layout.tabs {
            if p.extra.hidden_tabs.contains(&t.id) {
                t.shown = false;
            }
        }
        let mut labels = HashMap::new();
        for c in rb::all_commands() {
            labels.insert(*c, command_label(s, c));
        }
        let mut names = HashMap::new();
        for t in rb::main_tabs().into_iter().chain(rb::tool_tabs()) {
            names.insert(t.label.to_owned(), s.get(t.label).to_owned());
            for g in &t.groups {
                names.insert(g.label.to_owned(), s.get(g.label).to_owned());
            }
        }
        let t = |k: &str, f: &str| tr(s, k, f);
        let mut right = View::default();
        right.open.insert("ribbon.tabs.home".into());
        right.sel = Some(Node::Tab(0));
        State {
            layout,
            scope: 0,
            left: View {
                sel: None,
                ..View::default()
            },
            right,
            qleft: View {
                sel: Some(Node::Separator),
                ..View::default()
            },
            // Nothing chosen at first (as the reference): Add >> appends.
            qright: View::default(),
            qat: p.quick.clone(),
            labels,
            names,
            custom_suffix: t("doc21.customSuffix", " (Custom)"),
            separator: t("doc21.separator", "<Separator>"),
            headers: [
                t("doc21.srcMainTabs", "Main Tabs"),
                t("doc21.srcToolTabs", "Tool Tabs"),
                t("doc21.srcAllTabs", "All Tabs"),
            ],
            pop: None,
            pop_items: [
                [
                    t("doc21.resetTab", "Reset only selected Ribbon tab"),
                    t("doc21.resetAll", "Reset all customizations"),
                ],
                [
                    t("doc21.import", "Import customization file"),
                    t("doc21.export", "Export all customizations"),
                ],
            ],
            rename: None,
            new_tab: t("doc21.newTabName", "New Tab"),
            new_group: t("doc21.newGroupName", "New Group"),
            msgs: [
                t(
                    "doc21.msgCustomGroup",
                    "Commands must be added to custom groups. Select a custom group or click New Group.",
                ),
                t(
                    "doc21.msgBuiltinTab",
                    "Built-in tabs cannot be removed; clear their check box to hide them.",
                ),
                t(
                    "doc21.msgImportFailed",
                    "The file is not a BlueDoc customization file.",
                ),
            ],
            src_cache: 0,
            qsrc: 0,
        }
    }

    fn label(&self, c: &str) -> String {
        if c == SEPARATOR {
            return self.separator.clone();
        }
        self.labels.get(c).cloned().unwrap_or_else(|| c.to_owned())
    }

    fn tab_name(&self, t: &CTab) -> String {
        if !t.name.is_empty() {
            if t.is_custom() {
                return format!("{}{}", t.name, self.custom_suffix);
            }
            return t.name.clone();
        }
        self.names
            .get(&t.id)
            .cloned()
            .unwrap_or_else(|| t.id.clone())
    }

    fn group_name(&self, g: &CGroup) -> String {
        if g.is_custom() {
            return format!("{}{}", g.name, self.custom_suffix);
        }
        if !g.name.is_empty() {
            return g.name.clone();
        }
        g.builtin()
            .and_then(|(_, l)| self.names.get(l).cloned())
            .unwrap_or_else(|| g.id.clone())
    }

    /// Rows of the "Customize the Ribbon" tree.
    fn right_rows(&self) -> Vec<Row> {
        let mut rows = vec![Row {
            level: 0,
            node: Node::Header,
            label: self.headers[self.scope].clone(),
            open: None,
            check: None,
            icon: None,
            bold: true,
            height: ROW,
        }];
        for (i, t) in self.layout.tabs.iter().enumerate() {
            let want = match self.scope {
                0 => !t.tool,
                1 => t.tool,
                _ => true,
            };
            if !want {
                continue;
            }
            let open = self.right.open.contains(&t.id);
            rows.push(Row {
                level: 0,
                node: Node::Tab(i),
                label: self.tab_name(t),
                open: Some(open),
                check: Some(t.shown),
                icon: None,
                bold: false,
                height: TAB_ROW,
            });
            if !open {
                continue;
            }
            for (j, g) in t.groups.iter().enumerate() {
                let key = format!("{}/{}", t.id, g.id);
                let gopen = self.right.open.contains(&key);
                rows.push(Row {
                    level: 1,
                    node: Node::Group(i, j),
                    label: self.group_name(g),
                    open: Some(gopen),
                    check: None,
                    icon: None,
                    bold: false,
                    height: ROW,
                });
                if !gopen {
                    continue;
                }
                let cmds: Vec<String> = if g.is_custom() {
                    g.commands.clone()
                } else {
                    g.builtin()
                        .and_then(|(tab, l)| rb::builtin_group(tab, l))
                        .map(|bg| {
                            rb::group_commands(&bg)
                                .into_iter()
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default()
                };
                for (k, c) in cmds.iter().enumerate() {
                    rows.push(Row {
                        level: 2,
                        node: Node::Cmd(i, j, k),
                        label: self.label(c),
                        open: None,
                        check: None,
                        icon: rb::static_command(c),
                        bold: false,
                        height: ROW,
                    });
                }
            }
        }
        rows
    }

    /// Rows of a "Choose commands from" list (`qat`: with <Separator> first).
    fn left_rows(&self, src: usize, view: &View, qat: bool) -> Vec<Row> {
        let mut rows = Vec::new();
        if qat {
            rows.push(Row {
                level: 0,
                node: Node::Separator,
                label: self.separator.clone(),
                open: None,
                check: None,
                icon: None,
                bold: false,
                height: ROW,
            });
        }
        if let Some(mut cmds) = rb::source_commands(src) {
            cmds.sort_by_key(|c| self.label(c).to_lowercase());
            rows.extend(cmds.into_iter().map(|c| Row {
                level: 0,
                node: Node::SCmd(c),
                label: self.label(c),
                open: None,
                check: None,
                icon: Some(c),
                bold: false,
                height: ROW,
            }));
            return rows;
        }
        let tabs: Vec<crate::ribbon::Tab> = match src {
            5 => rb::main_tabs().into_iter().chain(rb::tool_tabs()).collect(),
            6 => rb::main_tabs(),
            7 => rb::tool_tabs(),
            _ => Vec::new(),
        };
        if src == 8 {
            // Custom Tabs and Groups.
            for (i, t) in self.layout.tabs.iter().enumerate() {
                let customs: Vec<(usize, &CGroup)> = t
                    .groups
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| g.is_custom())
                    .collect();
                if !t.is_custom() && customs.is_empty() {
                    continue;
                }
                rows.push(Row {
                    level: 0,
                    node: Node::Tab(i),
                    label: self.tab_name(t),
                    open: None,
                    check: None,
                    icon: None,
                    bold: false,
                    height: TAB_ROW,
                });
                for (j, g) in customs {
                    rows.push(Row {
                        level: 1,
                        node: Node::Group(i, j),
                        label: self.group_name(g),
                        open: None,
                        check: None,
                        icon: None,
                        bold: false,
                        height: ROW,
                    });
                    for (k, c) in g.commands.iter().enumerate() {
                        rows.push(Row {
                            level: 2,
                            node: Node::Cmd(i, j, k),
                            label: self.label(c),
                            open: None,
                            check: None,
                            icon: rb::static_command(c),
                            bold: false,
                            height: ROW,
                        });
                    }
                }
            }
            return rows;
        }
        for t in tabs {
            let open = view.open.contains(t.label);
            rows.push(Row {
                level: 0,
                node: Node::STab(t.label),
                label: self
                    .names
                    .get(t.label)
                    .cloned()
                    .unwrap_or_else(|| t.label.to_owned()),
                open: Some(open),
                check: None,
                icon: None,
                bold: false,
                height: TAB_ROW,
            });
            if !open {
                continue;
            }
            for g in &t.groups {
                let key = format!("{}/{}", t.label, g.label);
                let gopen = view.open.contains(&key);
                rows.push(Row {
                    level: 1,
                    node: Node::SGroup(t.label, g.label),
                    label: self
                        .names
                        .get(g.label)
                        .cloned()
                        .unwrap_or_else(|| g.label.to_owned()),
                    open: Some(gopen),
                    check: None,
                    icon: None,
                    bold: false,
                    height: ROW,
                });
                if gopen {
                    for c in rb::group_commands(g) {
                        rows.push(Row {
                            level: 2,
                            node: Node::SCmd(c),
                            label: self.label(c),
                            open: None,
                            check: None,
                            icon: Some(c),
                            bold: false,
                            height: ROW,
                        });
                    }
                }
            }
        }
        rows
    }

    /// Rows of the Quick Access Toolbar list.
    fn quick_rows(&self) -> Vec<Row> {
        self.qat
            .iter()
            .enumerate()
            .map(|(i, c)| Row {
                level: 0,
                node: Node::Quick(i),
                label: self.label(c),
                open: None,
                check: None,
                icon: (c != SEPARATOR).then(|| rb::static_command(c)).flatten(),
                bold: false,
                height: ROW,
            })
            .collect()
    }

    // -------------------------------------------------------------- editing

    /// The tab of the right tree's selection.
    fn sel_tab(&self) -> Option<usize> {
        match self.right.sel {
            Some(Node::Tab(i) | Node::Group(i, _) | Node::Cmd(i, _, _)) => Some(i),
            _ => None,
        }
    }

    /// Add >> on the Customize Ribbon page; `Err(message index)` when it cannot.
    fn add(&mut self, src: usize) -> Result<(), usize> {
        let left = self.left_rows(src, &self.left, false);
        let Some(sel) = self.left.sel.clone() else {
            return Ok(());
        };
        if !left.iter().any(|r| r.node == sel) {
            return Ok(());
        }
        match sel {
            Node::SCmd(c) => {
                let Some(Node::Group(i, j) | Node::Cmd(i, j, _)) = self.right.sel else {
                    return Err(0);
                };
                let Some(g) = self
                    .layout
                    .tabs
                    .get_mut(i)
                    .and_then(|t| t.groups.get_mut(j))
                else {
                    return Err(0);
                };
                if !g.is_custom() {
                    return Err(0);
                }
                let at = match self.right.sel {
                    Some(Node::Cmd(_, _, k)) => (k + 1).min(g.commands.len()),
                    _ => g.commands.len(),
                };
                g.commands.insert(at, c.to_owned());
                let key = format!(
                    "{}/{}",
                    self.layout.tabs[i].id, self.layout.tabs[i].groups[j].id
                );
                self.right.open.insert(self.layout.tabs[i].id.clone());
                self.right.open.insert(key);
                self.right.sel = Some(Node::Cmd(i, j, at));
            }
            Node::SGroup(tab, label) => {
                let Some(i) = self.sel_tab() else {
                    return Ok(());
                };
                let id = format!("{tab}|{label}");
                let t = &mut self.layout.tabs[i];
                let at = match self.right.sel {
                    Some(Node::Group(_, j) | Node::Cmd(_, j, _)) => (j + 1).min(t.groups.len()),
                    _ => t.groups.len(),
                };
                t.groups.insert(
                    at,
                    CGroup {
                        id,
                        name: String::new(),
                        commands: Vec::new(),
                    },
                );
                self.right.open.insert(t.id.clone());
                self.right.sel = Some(Node::Group(i, at));
            }
            Node::STab(tab) => {
                // A built-in tab already shown is just shown again (as the reference).
                if let Some(i) = self.layout.tabs.iter().position(|t| t.id == tab) {
                    self.layout.tabs[i].shown = true;
                    self.right.sel = Some(Node::Tab(i));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// << Remove on the Customize Ribbon page.
    fn remove(&mut self) -> Result<(), usize> {
        match self.right.sel.clone() {
            Some(Node::Cmd(i, j, k)) => {
                let Some(g) = self
                    .layout
                    .tabs
                    .get_mut(i)
                    .and_then(|t| t.groups.get_mut(j))
                else {
                    return Ok(());
                };
                if !g.is_custom() {
                    return Err(0);
                }
                if k < g.commands.len() {
                    g.commands.remove(k);
                }
                self.right.sel = if g.commands.is_empty() {
                    Some(Node::Group(i, j))
                } else {
                    Some(Node::Cmd(i, j, k.min(g.commands.len() - 1)))
                };
            }
            Some(Node::Group(i, j)) => {
                let Some(t) = self.layout.tabs.get_mut(i) else {
                    return Ok(());
                };
                if j < t.groups.len() {
                    t.groups.remove(j);
                }
                self.right.sel = if t.groups.is_empty() {
                    Some(Node::Tab(i))
                } else {
                    Some(Node::Group(i, j.min(t.groups.len() - 1)))
                };
            }
            Some(Node::Tab(i)) => {
                if !self.layout.tabs.get(i).is_some_and(CTab::is_custom) {
                    return Err(1);
                }
                self.layout.tabs.remove(i);
                self.right.sel = Some(Node::Tab(i.saturating_sub(1)));
            }
            _ => {}
        }
        Ok(())
    }

    /// New Tab: a custom tab with one custom group, after the selected tab.
    fn new_tab(&mut self) {
        let at = self.sel_tab().map_or(self.layout.tabs.len(), |i| i + 1);
        let tid = self.layout.new_id(rb::TAB_PREFIX);
        let gid = self.layout.new_id(rb::GROUP_PREFIX);
        let tool = self.scope == 1;
        self.layout.tabs.insert(
            at,
            CTab {
                id: tid.clone(),
                name: self.new_tab.clone(),
                shown: true,
                tool,
                groups: vec![CGroup {
                    id: gid.clone(),
                    name: self.new_group.clone(),
                    commands: Vec::new(),
                }],
            },
        );
        self.right.open.insert(tid.clone());
        self.right.sel = Some(Node::Group(at, 0));
    }

    /// New Group: a custom group after the selection, in the selected tab.
    fn new_group(&mut self) {
        let Some(i) = self.sel_tab() else {
            return;
        };
        let gid = self.layout.new_id(rb::GROUP_PREFIX);
        let t = &mut self.layout.tabs[i];
        let at = match self.right.sel {
            Some(Node::Group(_, j) | Node::Cmd(_, j, _)) => (j + 1).min(t.groups.len()),
            _ => t.groups.len(),
        };
        t.groups.insert(
            at,
            CGroup {
                id: gid,
                name: self.new_group.clone(),
                commands: Vec::new(),
            },
        );
        self.right.open.insert(t.id.clone());
        self.right.sel = Some(Node::Group(i, at));
    }

    /// The name shown in the rename panel.
    fn rename_text(&self, n: &Node) -> String {
        match *n {
            Node::Tab(i) => self.layout.tabs.get(i).map_or_else(String::new, |t| {
                if t.name.is_empty() {
                    self.names.get(&t.id).cloned().unwrap_or_default()
                } else {
                    t.name.clone()
                }
            }),
            Node::Group(i, j) => self
                .layout
                .tabs
                .get(i)
                .and_then(|t| t.groups.get(j))
                .map_or_else(String::new, |g| {
                    if g.name.is_empty() {
                        g.builtin()
                            .and_then(|(_, l)| self.names.get(l).cloned())
                            .unwrap_or_default()
                    } else {
                        g.name.clone()
                    }
                }),
            _ => String::new(),
        }
    }

    fn set_name(&mut self, n: &Node, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        match *n {
            Node::Tab(i) => {
                if let Some(t) = self.layout.tabs.get_mut(i) {
                    name.clone_into(&mut t.name);
                }
            }
            Node::Group(i, j) => {
                if let Some(g) = self
                    .layout
                    .tabs
                    .get_mut(i)
                    .and_then(|t| t.groups.get_mut(j))
                {
                    name.clone_into(&mut g.name);
                }
            }
            _ => {}
        }
    }

    /// ↑ / ↓ on the Customize Ribbon page.
    fn move_sel(&mut self, up: bool) {
        let step = |k: usize, n: usize| -> Option<usize> {
            if up {
                k.checked_sub(1)
            } else {
                (k + 1 < n).then_some(k + 1)
            }
        };
        match self.right.sel.clone() {
            Some(Node::Tab(i)) => {
                // Move past the next tab of the same kind shown in the tree.
                let tool = self.layout.tabs[i].tool;
                let mut k = i;
                loop {
                    let Some(nk) = step(k, self.layout.tabs.len()) else {
                        return;
                    };
                    k = nk;
                    if self.layout.tabs[k].tool == tool || self.scope == 2 {
                        break;
                    }
                }
                let t = self.layout.tabs.remove(i);
                self.layout.tabs.insert(k, t);
                self.right.sel = Some(Node::Tab(k));
            }
            Some(Node::Group(i, j)) => {
                let n = self.layout.tabs[i].groups.len();
                if let Some(k) = step(j, n) {
                    self.layout.tabs[i].groups.swap(j, k);
                    self.right.sel = Some(Node::Group(i, k));
                }
            }
            Some(Node::Cmd(i, j, c)) => {
                let g = &mut self.layout.tabs[i].groups[j];
                if !g.is_custom() {
                    return;
                }
                if let Some(k) = step(c, g.commands.len()) {
                    g.commands.swap(c, k);
                    self.right.sel = Some(Node::Cmd(i, j, k));
                }
            }
            _ => {}
        }
    }

    /// The layout to keep (an unchanged one is stored as the default).
    fn result(&self) -> Layout {
        if self.layout.is_default() {
            Layout::default()
        } else {
            self.layout.clone()
        }
    }
}

/// Label of a command: its ribbon label without the drop-down mark.
fn command_label(s: &Strings, id: &str) -> String {
    let key = format!("commands.{id}");
    let v = s.get(&key);
    if v == key {
        id.rsplit('.').next().unwrap_or(id).to_owned()
    } else {
        v.trim_end_matches(['\u{25BE}', ' ']).to_owned()
    }
}

// ------------------------------------------------------------------ dialog integration

/// Control ids of round 20's Customize Ribbon page, replaced by this module's.
const OLD: [&str; 11] = [
    "rMain", "rTab0", "rTab1", "rTab2", "rTab3", "rTab4", "rTab5", "rTab6", "rTab7", "rNote",
    "rReset",
];
/// Round 6's Quick Access Toolbar controls replaced by this module's.
const OLD_QAT: [&str; 3] = ["qFrom", "qTo", "qReset"];

/// Add the controls of both pages to the dialog `f`.
#[allow(clippy::too_many_lines)]
pub(super) fn add_pages(f: &mut super::Form, s: &Strings) {
    let t = |k: &str, fb: &str| tr(s, k, fb);
    for id in OLD.iter().chain(OLD_QAT.iter()) {
        if let Some(c) = f.get_mut(id) {
            c.page = Some(usize::MAX);
        }
    }
    if let Some(c) = f.get_mut("rIntro") {
        c.kind = Kind::Label(t(
            "doc21.ribbonIntro",
            "Customize the Ribbon and keyboard shortcuts.",
        ));
    }
    let sources: Vec<String> = rb::SOURCES.iter().map(|(k, fb)| t(k, fb)).collect();
    let button = |label: String| Kind::Button {
        label,
        default: false,
    };
    // Left and right columns (the reference's at a 1050 px dialog fit to BlueDoc's page:
    // 300 px lists, the buttons between them).
    let (lx, mx, rx, ax) = (257.0, 568.0, 688.0, 996.0);
    for (page, pre) in [(RIBBON, "r21"), (QAT, "r21Q")] {
        let id = |s: &str| -> &'static str { rb::intern(&format!("{pre}{s}")) };
        f.add(
            id("SrcL"),
            Some(page),
            [lx, 121.0, 300.0, 25.0],
            Kind::Label(t("docR6.opt.chooseFrom", "Choose commands from:")),
        );
        f.add(
            id("Src"),
            Some(page),
            [lx, 151.0, 300.0, 31.0],
            Kind::Combo {
                items: sources.clone(),
                sel: 0,
            },
        );
        f.add(
            id("Left"),
            Some(page),
            [lx, 217.0, 300.0, if page == QAT { 469.0 } else { 485.0 }],
            Kind::Canvas,
        );
        f.add(
            id("Right"),
            Some(page),
            [rx, 217.0, 300.0, 423.0],
            Kind::Canvas,
        );
        f.add(id("Up"), Some(page), [ax, 426.0, 32.0, 35.0], Kind::Canvas);
        f.add(
            id("Down"),
            Some(page),
            [ax, 466.0, 32.0, 35.0],
            Kind::Canvas,
        );
        f.add(
            id("CustL"),
            Some(page),
            [rx, 690.0, 126.0, 25.0],
            Kind::Label(t("doc21.customizations", "Customizations:")),
        );
        f.add(
            id("Reset"),
            Some(page),
            [rx + 130.0, 685.0, 90.0, 35.0],
            button(t("doc21.reset", "Reset \u{25BE}")),
        );
        f.add(
            id("Io"),
            Some(page),
            [rx + 130.0, 725.0, 160.0, 35.0],
            button(t("doc21.importExport", "Import/Export \u{25BE}")),
        );
        f.add(
            id("Msg"),
            Some(page),
            [lx, 760.0, 420.0, 24.0],
            Kind::Note(String::new()),
        );
    }
    // Customize Ribbon only.
    f.add(
        "r21Add",
        Some(RIBBON),
        [mx, 426.0, 108.0, 35.0],
        button(t("docR6.opt.add", "Add >>")),
    );
    f.add(
        "r21Remove",
        Some(RIBBON),
        [mx, 466.0, 108.0, 35.0],
        button(t("docR6.opt.remove", "<< Remove")),
    );
    f.add(
        "r21RibL",
        Some(RIBBON),
        [rx, 121.0, 300.0, 25.0],
        Kind::Label(t("doc21.customizeRibbon", "Customize the Ribbon:")),
    );
    f.add(
        "r21Rib",
        Some(RIBBON),
        [rx, 151.0, 300.0, 31.0],
        Kind::Combo {
            items: vec![
                t("doc21.srcMainTabs", "Main Tabs"),
                t("doc21.srcToolTabs", "Tool Tabs"),
                t("doc21.srcAllTabs", "All Tabs"),
            ],
            sel: 0,
        },
    );
    f.add(
        "r21NewTab",
        Some(RIBBON),
        [rx, 645.0, 92.0, 35.0],
        button(t("doc21.newTab", "New Tab")),
    );
    f.add(
        "r21NewGroup",
        Some(RIBBON),
        [rx + 102.0, 645.0, 104.0, 35.0],
        button(t("doc21.newGroup", "New Group")),
    );
    f.add(
        "r21Rename",
        Some(RIBBON),
        [rx + 216.0, 645.0, 92.0, 35.0],
        button(t("doc21.rename", "Rename...")),
    );
    f.add(
        "r21KeysL",
        Some(RIBBON),
        [lx, 721.0, 160.0, 25.0],
        Kind::Label(t("doc21.keyboard", "Keyboard shortcuts:")),
    );
    f.add(
        "r21Keys",
        Some(RIBBON),
        [lx + 172.0, 716.0, 115.0, 35.0],
        button(t("doc21.customize", "Customize...")),
    );
    // Quick Access Toolbar only (Add / Remove are round 6's buttons, moved).
    for (id, y) in [("qAdd", 426.0), ("qRemove", 466.0)] {
        if let Some(c) = f.get_mut(id) {
            c.rect = [mx, y, 108.0, 35.0];
        }
    }
    if let Some(c) = f.get_mut("qFromL") {
        c.page = Some(usize::MAX);
    }
    if let Some(c) = f.get_mut("qToL") {
        c.rect = [rx, 121.0, 300.0, 25.0];
    }
    f.add(
        "r21QFor",
        Some(QAT),
        [rx, 151.0, 300.0, 31.0],
        Kind::Combo {
            items: vec![t("doc21.forAll", "For all documents (default)")],
            sel: 0,
        },
    );
    f.add(
        "r21QBelow",
        Some(QAT),
        [lx, 700.0, 300.0, 51.0],
        Kind::Check {
            label: t(
                "doc21.qatBelow",
                "Show Quick Access Toolbar below the Ribbon",
            ),
            on: false,
        },
    );
    // Rename panel (over the page while open) and the popup menu, added last so they are on
    // top.
    f.add("r21RenBg", None, [380.0, 300.0, 440.0, 190.0], Kind::Canvas);
    f.add(
        "r21RenL",
        None,
        [400.0, 330.0, 380.0, 25.0],
        Kind::Label(t("doc21.displayName", "Display name:")),
    );
    f.add(
        "r21RenE",
        None,
        [400.0, 362.0, 400.0, 31.0],
        Kind::Edit {
            value: Vec::new(),
            caret: 0,
        },
    );
    f.add(
        "r21RenOk",
        None,
        [580.0, 430.0, 105.0, 35.0],
        button(t("common.ok", "OK")),
    );
    f.add(
        "r21RenCancel",
        None,
        [695.0, 430.0, 105.0, 35.0],
        button(t("common.cancel", "Cancel")),
    );
    f.add("r21Pop", None, [0.0, 0.0, 0.0, 0.0], Kind::Canvas);
    for id in [
        "r21RenBg",
        "r21RenL",
        "r21RenE",
        "r21RenOk",
        "r21RenCancel",
        "r21Pop",
    ] {
        f.show(id, false);
    }
}

/// Set the controls from the state after opening.
pub(super) fn init(d: &mut OptionsDialog) {
    let below = d.prefs.extra.qat_below;
    d.form.set_on("r21QBelow", below);
}

/// The canvases of the page (device-space hit tests use their rectangles).
fn canvas_at(d: &OptionsDialog, x: f32, y: f32) -> Option<&'static str> {
    let (lx, ly) = d.form.to_local(x, y);
    let page = d.page;
    ["r21Left", "r21Right", "r21QLeft", "r21QRight"]
        .into_iter()
        .find(|id| {
            d.form.get(id).is_some_and(|c| {
                c.page == Some(page)
                    && lx >= c.rect[0]
                    && lx < c.rect[0] + c.rect[2]
                    && ly >= c.rect[1]
                    && ly < c.rect[1] + c.rect[3]
            })
        })
}

/// Input handled before the form: the wheel over this module's lists.
pub(super) fn input(d: &mut OptionsDialog, input: &Input) -> Option<OptOut> {
    if let Input::Wheel(delta) = input {
        let (x, y) = d.pointer;
        let id = canvas_at(d, x, y)?;
        let h = d.form.get(id).map_or(0.0, |c| c.rect[3]);
        let total = rows_height(&d.r21, id);
        let v = view_mut(&mut d.r21, id);
        let step = if *delta > 0.0 { -3.0 * ROW } else { 3.0 * ROW };
        v.top = (v.top + step).clamp(0.0, (total - h + 4.0).max(0.0));
        return Some(OptOut::Redraw);
    }
    None
}

fn view_mut<'a>(s: &'a mut State, id: &str) -> &'a mut View {
    match id {
        "r21Left" => &mut s.left,
        "r21Right" => &mut s.right,
        "r21QLeft" => &mut s.qleft,
        _ => &mut s.qright,
    }
}

fn rows_of(s: &State, id: &str, src: usize) -> Vec<Row> {
    match id {
        "r21Left" => s.left_rows(src, &s.left, false),
        "r21Right" => s.right_rows(),
        "r21QLeft" => s.left_rows(src, &s.qleft, true),
        _ => s.quick_rows(),
    }
}

fn rows_height(s: &State, id: &str) -> f32 {
    let src = if id == "r21QLeft" {
        s.qsrc
    } else {
        s.src_cache
    };
    rows_of(s, id, src).iter().map(|r| r.height).sum()
}

/// Event handling before round 6's; `None` lets round 6 handle it.
#[allow(clippy::too_many_lines)]
pub(super) fn event(d: &mut OptionsDialog, ev: &Event) -> Option<OptOut> {
    // The popup menu takes the next press wherever it is.
    if let Some((which, _)) = d.r21.pop {
        if let Event::Canvas("r21Pop", _, y) = ev {
            let i = usize::from(*y >= 34.0);
            d.r21.pop = None;
            d.form.show("r21Pop", false);
            return Some(pop_chosen(d, which, i));
        }
        if !matches!(ev, Event::None) {
            d.r21.pop = None;
            d.form.show("r21Pop", false);
            return Some(OptOut::Redraw);
        }
    }
    if let Some(n) = d.r21.rename.clone() {
        return Some(match ev {
            Event::Clicked("r21RenOk") | Event::Ok => {
                let name = d.form.text("r21RenE");
                d.r21.set_name(&n, &name);
                close_rename(d);
                OptOut::Redraw
            }
            Event::Clicked("r21RenCancel") | Event::Cancel => {
                close_rename(d);
                OptOut::Redraw
            }
            _ => OptOut::Redraw,
        });
    }
    let msg = |d: &mut OptionsDialog, id: &str, m: Option<usize>| {
        let text = m.map_or_else(String::new, |i| d.r21.msgs[i].clone());
        if let Some(c) = d.form.get_mut(id) {
            c.kind = Kind::Note(text);
        }
    };
    let out = match ev {
        Event::Changed("r21Src") => {
            d.r21.left = View::default();
            d.r21.src_cache = d.form.sel("r21Src").unwrap_or(0);
            OptOut::Redraw
        }
        Event::Changed("r21QSrc") => {
            d.r21.qleft = View {
                sel: Some(Node::Separator),
                ..View::default()
            };
            d.r21.qsrc = d.form.sel("r21QSrc").unwrap_or(0);
            OptOut::Redraw
        }
        Event::Changed("r21Rib") => {
            d.r21.scope = d.form.sel("r21Rib").unwrap_or(0).min(2);
            d.r21.right.top = 0.0;
            d.r21.right.sel = None;
            OptOut::Redraw
        }
        Event::Canvas(id @ ("r21Left" | "r21Right" | "r21QLeft" | "r21QRight"), x, y) => {
            tree_press(d, id, *x, *y);
            msg(d, "r21Msg", None);
            OptOut::Redraw
        }
        Event::Clicked("r21Add") => {
            let r = d.r21.add(d.r21.src_cache);
            msg(d, "r21Msg", r.err());
            OptOut::Redraw
        }
        Event::Clicked("r21Remove") => {
            let r = d.r21.remove();
            msg(d, "r21Msg", r.err());
            OptOut::Redraw
        }
        Event::Clicked("r21NewTab") => {
            d.r21.new_tab();
            OptOut::Redraw
        }
        Event::Clicked("r21NewGroup") => {
            d.r21.new_group();
            OptOut::Redraw
        }
        Event::Clicked("r21Rename") => {
            if let Some(n @ (Node::Tab(_) | Node::Group(..))) = d.r21.right.sel.clone() {
                open_rename(d, n);
            }
            OptOut::Redraw
        }
        Event::Canvas("r21Up", ..) => {
            d.r21.move_sel(true);
            OptOut::Redraw
        }
        Event::Canvas("r21Down", ..) => {
            d.r21.move_sel(false);
            OptOut::Redraw
        }
        Event::Canvas("r21QUp", ..) => {
            quick_move(&mut d.r21, true);
            OptOut::Redraw
        }
        Event::Canvas("r21QDown", ..) => {
            quick_move(&mut d.r21, false);
            OptOut::Redraw
        }
        Event::Clicked(id @ ("r21Reset" | "r21Io" | "r21QReset" | "r21QIo")) => {
            let which = if id.ends_with("Reset") {
                Pop::Reset
            } else {
                Pop::Io
            };
            let page = d.page;
            d.r21.pop = Some((which, page));
            let r = d.form.get(id).map_or([0.0; 4], |c| c.rect);
            let w = 330.0;
            if let Some(c) = d.form.get_mut("r21Pop") {
                c.rect = [r[0], r[1] + r[3], w, 68.0];
                c.page = Some(page);
            }
            d.form.show("r21Pop", true);
            OptOut::Redraw
        }
        Event::Clicked("r21Keys") => OptOut::R21(Out::Keyboard),
        Event::Clicked("qAdd") => {
            quick_add(&mut d.r21);
            OptOut::Redraw
        }
        Event::Clicked("qRemove") => {
            if let Some(Node::Quick(i)) = d.r21.qright.sel {
                if i < d.r21.qat.len() {
                    d.r21.qat.remove(i);
                }
                d.r21.qright.sel =
                    (!d.r21.qat.is_empty()).then(|| Node::Quick(i.min(d.r21.qat.len() - 1)));
            }
            OptOut::Redraw
        }
        _ => return None,
    };
    Some(out)
}

fn open_rename(d: &mut OptionsDialog, n: Node) {
    let text = d.r21.rename_text(&n);
    d.r21.rename = Some(n);
    let page = d.page;
    for id in ["r21RenBg", "r21RenL", "r21RenE", "r21RenOk", "r21RenCancel"] {
        if let Some(c) = d.form.get_mut(id) {
            c.page = Some(page);
        }
        d.form.show(id, true);
    }
    d.form.set_text("r21RenE", &text);
    d.form.focus("r21RenE");
    d.form.select_all("r21RenE");
}

fn close_rename(d: &mut OptionsDialog) {
    d.r21.rename = None;
    for id in ["r21RenBg", "r21RenL", "r21RenE", "r21RenOk", "r21RenCancel"] {
        d.form.show(id, false);
    }
}

/// An entry of Reset ▾ / Import/Export ▾ was chosen.
fn pop_chosen(d: &mut OptionsDialog, which: Pop, i: usize) -> OptOut {
    let qat_page = d.page == QAT;
    match (which, i) {
        (Pop::Reset, 0) => {
            if qat_page {
                d.r21.qat = super::QUICK_DEFAULT
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect();
            } else if let Some(t) = d.r21.sel_tab() {
                d.r21.layout.reset_tab(t);
            }
            OptOut::Redraw
        }
        (Pop::Reset, _) => {
            d.r21.layout = Layout::default_full();
            d.r21.qat = super::QUICK_DEFAULT
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
            d.form.set_on("r21QBelow", false);
            d.r21.right.sel = Some(Node::Tab(0));
            OptOut::Redraw
        }
        (Pop::Io, 0) => OptOut::R21(Out::Import),
        (Pop::Io, _) => OptOut::R21(Out::Export(rb::export(
            &d.r21.result(),
            &d.r21.qat,
            d.form.on("r21QBelow"),
        ))),
    }
}

fn quick_add(s: &mut State) {
    let Some(sel) = s.qleft.sel.clone() else {
        return;
    };
    let id = match sel {
        Node::Separator => SEPARATOR.to_owned(),
        Node::SCmd(c) => c.to_owned(),
        _ => return,
    };
    if id != SEPARATOR && s.qat.contains(&id) {
        return;
    }
    let at = match s.qright.sel {
        Some(Node::Quick(i)) => (i + 1).min(s.qat.len()),
        _ => s.qat.len(),
    };
    s.qat.insert(at, id);
    s.qright.sel = Some(Node::Quick(at));
}

fn quick_move(s: &mut State, up: bool) {
    let Some(Node::Quick(i)) = s.qright.sel else {
        return;
    };
    let k = if up {
        i.checked_sub(1)
    } else {
        (i + 1 < s.qat.len()).then_some(i + 1)
    };
    if let Some(k) = k {
        s.qat.swap(i, k);
        s.qright.sel = Some(Node::Quick(k));
    }
}

/// A press in one of the lists at (`x`, `y`) reference pixels from its corner.
fn tree_press(d: &mut OptionsDialog, id: &'static str, x: f32, y: f32) {
    let src = if id == "r21QLeft" {
        d.r21.qsrc
    } else {
        d.r21.src_cache
    };
    let rows = rows_of(&d.r21, id, src);
    let width = d.form.get(id).map_or(0.0, |c| c.rect[2]);
    let v = view_mut(&mut d.r21, id);
    let mut at = y + v.top - 1.0;
    let mut hit = None;
    for r in &rows {
        if at < r.height {
            hit = Some(r.clone());
            break;
        }
        at -= r.height;
    }
    if x >= width - BAR {
        return;
    }
    let Some(r) = hit else {
        return;
    };
    let indent = 4.0 + f32::from(r.level) * 42.0;
    if r.open.is_some() && x < indent + 22.0 {
        let key = node_key(&d.r21, &r.node);
        let v = view_mut(&mut d.r21, id);
        if !v.open.remove(&key) {
            v.open.insert(key);
        }
        v.sel = Some(r.node);
        return;
    }
    if let (Some(on), Node::Tab(i)) = (r.check, &r.node) {
        if x < indent + 22.0 + 20.0 {
            if let Some(t) = d.r21.layout.tabs.get_mut(*i) {
                t.shown = !on;
            }
        }
    }
    view_mut(&mut d.r21, id).sel = Some(r.node);
}

/// Key of a row for its expanded state.
fn node_key(s: &State, n: &Node) -> String {
    match *n {
        Node::Tab(i) => s
            .layout
            .tabs
            .get(i)
            .map_or_else(String::new, |t| t.id.clone()),
        Node::Group(i, j) => s
            .layout
            .tabs
            .get(i)
            .and_then(|t| t.groups.get(j).map(|g| format!("{}/{}", t.id, g.id)))
            .unwrap_or_default(),
        Node::STab(t) => t.to_owned(),
        Node::SGroup(t, g) => format!("{t}/{g}"),
        _ => String::new(),
    }
}

/// Fill the preferences from the pages.
pub(super) fn collect(d: &OptionsDialog, p: &mut super::DocPrefs) {
    let l = d.r21.result();
    p.extra.hidden_tabs = l.hidden_main();
    p.extra.ribbon = l;
    p.quick.clone_from(&d.r21.qat);
    p.extra.qat_below = d.form.on("r21QBelow");
}

/// A customisation file the host read (Import/Export ▾ › Import).
pub(super) fn import(d: &mut OptionsDialog, text: &str) -> bool {
    let Some(f) = rb::import(text) else {
        let m = d.r21.msgs[2].clone();
        for id in ["r21Msg", "r21QMsg"] {
            if let Some(c) = d.form.get_mut(id) {
                c.kind = Kind::Note(m.clone());
            }
        }
        return true;
    };
    d.r21.layout = f.ribbon.full();
    d.r21.qat = f.qat;
    d.form.set_on("r21QBelow", f.qat_below);
    d.r21.right = View::default();
    d.r21.right.sel = Some(Node::Tab(0));
    d.r21.qright.sel = (!d.r21.qat.is_empty()).then_some(Node::Quick(0));
    true
}

// ------------------------------------------------------------------ painting

const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const EDGE: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const FACE: Rgba = Rgba(0xF3, 0xF3, 0xF3, 255);
const SEL: Rgba = Rgba(0xC6, 0xC6, 0xC6, 255);
const CHECK_FILL: Rgba = Rgba(0x2B, 0x57, 0x9A, 255);

/// Paint canvas `id` into `r` (device pixels; `k` device px per reference px, `base` the
/// form's text size). `false` when `id` is not one of this module's.
pub(super) fn paint(
    s: &State,
    p: &mut dyn Painter,
    id: &str,
    r: [f32; 4],
    k: f32,
    base: f32,
    icons: Option<&Icons>,
) -> bool {
    match id {
        "r21Left" | "r21Right" | "r21QLeft" | "r21QRight" => {
            let src = if id == "r21QLeft" {
                s.qsrc
            } else {
                s.src_cache
            };
            let rows = rows_of(s, id, src);
            let view = match id {
                "r21Left" => &s.left,
                "r21Right" => &s.right,
                "r21QLeft" => &s.qleft,
                _ => &s.qright,
            };
            paint_list(p, r, k, base, &rows, view, icons);
        }
        "r21Up" | "r21Down" | "r21QUp" | "r21QDown" => {
            paint_arrow(p, r, k, id.ends_with("Up"));
        }
        "r21RenBg" => {
            p.fill_rect(r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0, EDGE);
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xFF, 0xFF, 0xFF, 255));
        }
        "r21Pop" => {
            if let Some((which, _)) = s.pop {
                let items = &s.pop_items[usize::from(which == Pop::Io)];
                p.fill_rect(r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0, EDGE);
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xFF, 0xFF, 0xFF, 255));
                let st = TextStyle {
                    size: base.round(),
                    bold: false,
                };
                for (i, t) in items.iter().enumerate() {
                    p.text(
                        t,
                        r[0] + 14.0 * k,
                        r[1] + (6.0 + 34.0 * i as f32) * k,
                        st,
                        INK,
                    );
                }
            }
        }
        _ => return false,
    }
    true
}

fn paint_arrow(p: &mut dyn Painter, r: [f32; 4], k: f32, up: bool) {
    p.fill_rect(r[0], r[1], r[2], r[3], EDGE);
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(0xE1, 0xE1, 0xE1, 255),
    );
    let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    let (w, h) = (7.0 * k, 8.0 * k);
    let (tip, base) = if up {
        (cy - h / 2.0, cy + h / 2.0)
    } else {
        (cy + h / 2.0, cy - h / 2.0)
    };
    p.path(
        &[
            PathSeg::M(cx, tip),
            PathSeg::L(cx + w, base),
            PathSeg::L(cx - w, base),
            PathSeg::Z,
        ],
        Some(Rgba(0x40, 0x40, 0x40, 255)),
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_list(
    p: &mut dyn Painter,
    r: [f32; 4],
    k: f32,
    base: f32,
    rows: &[Row],
    view: &View,
    icons: Option<&Icons>,
) {
    p.fill_rect(r[0], r[1], r[2], r[3], EDGE);
    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, FACE);
    let total: f32 = rows.iter().map(|r| r.height).sum::<f32>() * k;
    let inner_h = r[3] - 2.0;
    let bar = total > inner_h;
    let list_w = r[2] - 2.0 - if bar { BAR * k } else { 0.0 };
    let top = (view.top * k).min((total - inner_h).max(0.0));
    p.push_clip(r[0] + 1.0, r[1] + 1.0, list_w, inner_h);
    let mut y = r[1] + 1.0 - top;
    let st = TextStyle {
        size: base.round(),
        bold: false,
    };
    let line = st.size * 1.33;
    for row in rows {
        let h = row.height * k;
        if y + h >= r[1] && y <= r[1] + r[3] {
            if view.sel.as_ref() == Some(&row.node) {
                p.fill_rect(r[0] + 1.0, y, list_w, h, SEL);
            }
            let mut x = r[0] + (4.0 + f32::from(row.level) * 42.0) * k;
            let cy = y + h / 2.0;
            if let Some(open) = row.open {
                chevron(p, x + 8.0 * k, cy, k, open);
            }
            if row.open.is_some() || row.level > 0 {
                x += 22.0 * k;
            }
            if let Some(on) = row.check {
                let b = (15.0 * k).round();
                let (bx, by) = (x.round(), (cy - b / 2.0).round());
                p.fill_rect(
                    bx,
                    by,
                    b,
                    b,
                    if on {
                        CHECK_FILL
                    } else {
                        Rgba(0x33, 0x33, 0x33, 255)
                    },
                );
                if on {
                    p.path(
                        &[
                            PathSeg::M(bx + 3.0 * k, by + 7.5 * k),
                            PathSeg::L(bx + 6.2 * k, by + 10.8 * k),
                            PathSeg::L(bx + 12.0 * k, by + 4.0 * k),
                        ],
                        None,
                        Some((Rgba(0xFF, 0xFF, 0xFF, 255), 1.6 * k)),
                    );
                } else {
                    p.fill_rect(
                        bx + 1.0,
                        by + 1.0,
                        b - 2.0,
                        b - 2.0,
                        Rgba(0xFF, 0xFF, 0xFF, 255),
                    );
                }
                x += 20.0 * k;
            }
            if let Some(icon) = row.icon {
                if let Some(ic) = icons {
                    let name = crate::ribbon::icon_of(icon);
                    let size = (20.0 * k).round();
                    ic.draw(
                        p,
                        name,
                        x,
                        (cy - size / 2.0).round(),
                        size,
                        IconColors {
                            ink: Rgba(0x44, 0x44, 0x44, 255),
                            accent: Rgba(0x2B, 0x57, 0x9A, 255),
                            on: Rgba(0xFF, 0xFF, 0xFF, 255),
                            tinted: true,
                        },
                    );
                }
                x += 26.0 * k;
            } else if matches!(row.node, Node::Separator | Node::Quick(_)) {
                x += 26.0 * k;
            }
            let style = TextStyle {
                size: st.size,
                bold: row.bold,
            };
            p.text(&row.label, x, (cy - line / 2.0).round(), style, INK);
        }
        y += h;
    }
    p.pop_clip();
    if bar {
        let bx = r[0] + r[2] - 1.0 - BAR * k;
        p.fill_rect(
            bx,
            r[1] + 1.0,
            BAR * k,
            inner_h,
            Rgba(0xF0, 0xF0, 0xF0, 255),
        );
        let arrow = BAR * k;
        let track = inner_h - 2.0 * arrow;
        let th = (track * inner_h / total).max(12.0 * k);
        let ty = r[1] + 1.0 + arrow + (track - th) * (top / (total - inner_h).max(1.0));
        p.fill_rect(bx + 2.0, ty, BAR * k - 4.0, th, Rgba(0xC1, 0xC1, 0xC1, 255));
    }
}

fn chevron(p: &mut dyn Painter, x: f32, cy: f32, k: f32, open: bool) {
    let s = 4.5 * k;
    let segs = if open {
        [
            PathSeg::M(x - s, cy - s / 2.0),
            PathSeg::L(x, cy + s / 2.0),
            PathSeg::L(x + s, cy - s / 2.0),
        ]
    } else {
        [
            PathSeg::M(x - s / 2.0, cy - s),
            PathSeg::L(x + s / 2.0, cy),
            PathSeg::L(x - s / 2.0, cy + s),
        ]
    };
    p.path(&segs, None, Some((Rgba(0x40, 0x40, 0x40, 255), 1.2 * k)));
}

/// Tests: choose command `id` in the Quick Access Toolbar page's command list.
#[cfg(test)]
pub(super) fn pick_quick(d: &mut OptionsDialog, id: &'static str) {
    d.r21.qleft.sel = Some(Node::SCmd(id));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog() -> OptionsDialog {
        let s = Strings::load("en-US");
        OptionsDialog::open(&s, &super::super::DocPrefs::default())
    }

    #[test]
    fn new_tab_group_add_command_rename_and_collect() {
        let mut d = dialog();
        d.set_page(RIBBON);
        let n0 = d.r21.layout.tabs.len();
        assert_eq!(d.r21.right.sel, Some(Node::Tab(0)));
        event(&mut d, &Event::Clicked("r21NewTab"));
        assert_eq!(d.r21.layout.tabs.len(), n0 + 1);
        assert_eq!(d.r21.right.sel, Some(Node::Group(1, 0)));
        // Pick Bold from All Commands and add it.
        d.form.set_sel("r21Src", Some(2));
        event(&mut d, &Event::Changed("r21Src"));
        d.r21.left.sel = Some(Node::SCmd("format.bold"));
        event(&mut d, &Event::Clicked("r21Add"));
        assert_eq!(d.r21.layout.tabs[1].groups[0].commands, vec!["format.bold"]);
        // Adding into a built-in group is refused with a message.
        d.r21.right.sel = Some(Node::Group(0, 0));
        event(&mut d, &Event::Clicked("r21Add"));
        assert!(
            matches!(d.form.get("r21Msg").map(|c| &c.kind), Some(Kind::Note(t)) if !t.is_empty())
        );
        // Rename the new tab.
        d.r21.right.sel = Some(Node::Tab(1));
        event(&mut d, &Event::Clicked("r21Rename"));
        assert!(d.r21.rename.is_some());
        d.form.set_text("r21RenE", "Mine");
        event(&mut d, &Event::Clicked("r21RenOk"));
        assert_eq!(d.r21.layout.tabs[1].name, "Mine");
        // Move it down, hide Mailings, collect.
        event(&mut d, &Event::Canvas("r21Down", 1.0, 1.0));
        assert_eq!(d.r21.right.sel, Some(Node::Tab(2)));
        if let Some(t) = d
            .r21
            .layout
            .tabs
            .iter_mut()
            .find(|t| t.id == "ribbon.tabs.mailings")
        {
            t.shown = false;
        }
        let p = d.collect();
        assert!(!p.extra.ribbon.is_default());
        assert_eq!(p.extra.hidden_tabs, vec!["ribbon.tabs.mailings".to_owned()]);
        let tabs = rb::build_main(&p.extra.ribbon);
        assert_eq!(tabs[2].label, p.extra.ribbon.tabs[2].id);
        // Remove the custom tab; a built-in one cannot be removed.
        d.r21.right.sel = Some(Node::Tab(2));
        event(&mut d, &Event::Clicked("r21Remove"));
        assert_eq!(d.r21.layout.tabs.len(), n0);
        d.r21.right.sel = Some(Node::Tab(0));
        assert_eq!(d.r21.remove(), Err(1));
    }

    #[test]
    fn quick_access_add_separator_move_reset_and_file() {
        let mut d = dialog();
        d.set_page(QAT);
        let n = d.r21.qat.len();
        d.r21.qright.sel = Some(Node::Quick(n - 1));
        event(&mut d, &Event::Clicked("qAdd"));
        assert_eq!(d.r21.qat.last().map(String::as_str), Some(SEPARATOR));
        d.r21.qleft.sel = Some(Node::SCmd("edit.find"));
        event(&mut d, &Event::Clicked("qAdd"));
        assert_eq!(d.r21.qat.len(), n + 2);
        event(&mut d, &Event::Canvas("r21QUp", 1.0, 1.0));
        assert_eq!(d.r21.qat[n], "edit.find");
        d.form.set_on("r21QBelow", true);
        let p = d.collect();
        assert!(p.extra.qat_below);
        assert_eq!(p.quick.len(), n + 2);
        // Export, reset all, import back.
        let OptOut::R21(Out::Export(text)) = pop_chosen(&mut d, Pop::Io, 1) else {
            panic!("export");
        };
        pop_chosen(&mut d, Pop::Reset, 1);
        assert_eq!(d.r21.qat.len(), super::super::QUICK_DEFAULT.len());
        assert!(!d.form.on("r21QBelow"));
        assert!(import(&mut d, &text));
        assert_eq!(d.r21.qat.len(), n + 2);
        assert!(d.form.on("r21QBelow"));
        // A foreign file leaves a message.
        assert!(import(&mut d, "{}"));
        assert_eq!(d.r21.qat.len(), n + 2);
    }

    #[test]
    fn trees_expand_and_check_by_pointer() {
        let mut d = dialog();
        d.set_page(RIBBON);
        let rows = d.r21.right_rows();
        // Header, Home (open) with its groups, then Insert…
        assert_eq!(rows[0].node, Node::Header);
        assert_eq!(rows[1].open, Some(true));
        // Clear Insert's check box: its row comes after Home's groups.
        let at: f32 = rows
            .iter()
            .take_while(|r| r.node != Node::Tab(1))
            .map(|r| r.height)
            .sum();
        tree_press(&mut d, "r21Right", 8.0 + 22.0 + 5.0, at + 10.0);
        assert!(!d.r21.layout.tabs[1].shown);
        // Collapse Home with its chevron.
        tree_press(&mut d, "r21Right", 10.0, ROW + 10.0);
        assert_eq!(d.r21.right_rows()[1].open, Some(false));
        // Tab categories are trees.
        assert!(d
            .r21
            .left_rows(6, &d.r21.left, false)
            .iter()
            .all(|r| matches!(r.node, Node::STab(_))));
    }
}
