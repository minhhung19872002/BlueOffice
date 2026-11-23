//! BlueDoc ribbon and Quick Access Toolbar customisation of round 21 (docs/ui/REPORT.md
//! "Vòng 111"): the model behind File › Options › Customize Ribbon and Quick Access Toolbar,
//! as on the reference — main and tool tabs in any order, shown or hidden, renamed; built-in
//! groups removed, moved or added to other tabs; new tabs and groups holding any command;
//! the result laid into BlueDoc's ribbon with the reference's scaling (custom groups lose
//! their labels, then become one drop-down button), stored with the application preferences
//! and written to / read from BlueOffice's own customisation file (JSON, `.bocustom`).
//!
//! Custom tabs, groups and their collapsed buttons get interned ids (`c21t.N`, `c21g.N`,
//! `c21b.N`) whose display names live in the window's strings; a small process-wide
//! registry gives the ribbon's static scaling / holder rules and the collapsed-group
//! drop-down (`doc_scale18`, `ribbon::doc_group`) what the current customisation adds.
use crate::ribbon::{Group, Item, Tab};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[cfg(not(test))]
use std::sync::RwLock;
use std::sync::{Arc, Mutex, OnceLock};

/// Prefix of custom tab ids.
pub(crate) const TAB_PREFIX: &str = "c21t.";
/// Prefix of custom group ids.
pub(crate) const GROUP_PREFIX: &str = "c21g.";
/// Prefix of the collapsed buttons of custom groups.
const BUTTON_PREFIX: &str = "c21b.";
/// Format name written into customisation files.
pub(crate) const FILE_FORMAT: &str = "BlueOffice.UICustomization";

/// One tab of the customised ribbon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CTab {
    /// Built-in tab label key, or a custom id (`c21t.N`).
    pub(crate) id: String,
    /// Display name given by the user (empty: the built-in name).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) name: String,
    /// Shown in the ribbon (the tree's check box).
    pub(crate) shown: bool,
    /// A tool (contextual) tab.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) tool: bool,
    pub(crate) groups: Vec<CGroup>,
}

/// One group of a customised tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CGroup {
    /// Built-in group as `<tab label>|<group label>`, or a custom id (`c21g.N`).
    pub(crate) id: String,
    /// Display name given by the user (empty: the built-in name).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) name: String,
    /// Commands of a custom group.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) commands: Vec<String>,
}

impl CTab {
    pub(crate) fn is_custom(&self) -> bool {
        self.id.starts_with(TAB_PREFIX)
    }
}

impl CGroup {
    pub(crate) fn is_custom(&self) -> bool {
        self.id.starts_with(GROUP_PREFIX)
    }

    /// Source tab and label of a built-in group.
    pub(crate) fn builtin(&self) -> Option<(&str, &str)> {
        self.id.split_once('|')
    }
}

/// The whole ribbon customisation (empty: the default ribbon).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct Layout {
    pub(crate) tabs: Vec<CTab>,
    /// Next number for custom ids.
    #[serde(default)]
    pub(crate) next: u32,
}

/// The built-in main tabs.
pub(crate) fn main_tabs() -> Vec<Tab> {
    crate::ribbon::doc_tabs()
}

/// The built-in tool (contextual) tabs.
pub(crate) fn tool_tabs() -> Vec<Tab> {
    crate::doc_scale19::tabs()
}

fn default_tab(t: &Tab, tool: bool) -> CTab {
    CTab {
        id: t.label.to_owned(),
        name: String::new(),
        shown: true,
        tool,
        groups: t
            .groups
            .iter()
            .map(|g| CGroup {
                id: format!("{}|{}", t.label, g.label),
                name: String::new(),
                commands: Vec::new(),
            })
            .collect(),
    }
}

impl Layout {
    /// The default ribbon spelled out (main tabs, then tool tabs).
    pub(crate) fn default_full() -> Self {
        let mut tabs: Vec<CTab> = main_tabs().iter().map(|t| default_tab(t, false)).collect();
        tabs.extend(tool_tabs().iter().map(|t| default_tab(t, true)));
        Layout { tabs, next: 1 }
    }

    /// This customisation spelled out: an empty one is the default; built-in tabs missing
    /// from it (a newer BlueDoc) are added where the default has them.
    pub(crate) fn full(&self) -> Self {
        if self.tabs.is_empty() {
            return Self::default_full();
        }
        let mut out = self.clone();
        out.next = out.next.max(1);
        for t in Self::default_full().tabs {
            if !out.tabs.iter().any(|x| x.id == t.id) {
                out.tabs.push(t);
            }
        }
        out
    }

    /// Whether this is the default ribbon.
    pub(crate) fn is_default(&self) -> bool {
        self.tabs.is_empty() || self.full() == Self::default_full().with_next(self.full().next)
    }

    fn with_next(mut self, n: u32) -> Self {
        self.next = n;
        self
    }

    /// A fresh custom id with `prefix`.
    pub(crate) fn new_id(&mut self, prefix: &str) -> String {
        let n = self.next.max(1);
        self.next = n + 1;
        format!("{prefix}{n}")
    }

    /// Reset one tab to the built-in one (a custom tab is left as is).
    pub(crate) fn reset_tab(&mut self, i: usize) {
        let Some(t) = self.tabs.get(i) else {
            return;
        };
        let id = t.id.clone();
        if let Some(d) = Self::default_full().tabs.into_iter().find(|d| d.id == id) {
            self.tabs[i] = d;
        }
    }

    /// Main tab labels hidden (round 20's preference, kept in step).
    pub(crate) fn hidden_main(&self) -> Vec<String> {
        self.full()
            .tabs
            .iter()
            .filter(|t| !t.tool && !t.is_custom() && !t.shown)
            .map(|t| t.id.clone())
            .collect()
    }

    /// Read from text (JSON); `None` when it is not a customisation.
    pub(crate) fn from_json(text: &str) -> Option<Self> {
        let mut l: Layout = serde_json::from_str(text).ok()?;
        l.sanitize();
        Some(l)
    }

    /// Keep only what BlueDoc knows: built-in tabs and groups that exist, commands of the
    /// command catalog.
    pub(crate) fn sanitize(&mut self) {
        let cmds = command_set();
        let known_tabs: HashSet<&str> = main_tabs()
            .iter()
            .chain(tool_tabs().iter())
            .map(|t| t.label)
            .collect();
        let mut max = 0u32;
        let num = |id: &str, p: &str| id.strip_prefix(p).and_then(|n| n.parse::<u32>().ok());
        self.tabs
            .retain(|t| t.is_custom() || known_tabs.contains(t.id.as_str()));
        for t in &mut self.tabs {
            max = max.max(num(&t.id, TAB_PREFIX).unwrap_or(0));
            t.groups.retain(|g| {
                g.is_custom()
                    || g.builtin()
                        .is_some_and(|(tab, label)| builtin_group(tab, label).is_some())
            });
            for g in &mut t.groups {
                max = max.max(num(&g.id, GROUP_PREFIX).unwrap_or(0));
                g.commands.retain(|c| cmds.contains(c.as_str()));
            }
        }
        self.next = self.next.max(max + 1);
    }
}

/// A built-in group from its source tab (not the customised one).
pub(crate) fn builtin_group(tab: &str, label: &str) -> Option<Group> {
    main_tabs()
        .into_iter()
        .chain(tool_tabs())
        .filter(|t| t.label == tab)
        .flat_map(|t| t.groups)
        .find(|g| g.label == label)
}

// ------------------------------------------------------------------ command catalog

/// File tab commands offered (the File screen's pages that run as commands).
pub(crate) const FILE_COMMANDS: [&str; 8] = [
    "file.new",
    "file.open",
    "file.save",
    "file.saveAs",
    "file.print",
    "file.exportPdf",
    "file.close",
    "file.options",
];

/// Commands BlueDoc runs that no tab shows (Commands Not in the Ribbon).
pub(crate) const NOT_IN_RIBBON: [&str; 6] = [
    "file.new",
    "file.open",
    "file.save",
    "file.saveAs",
    "file.print",
    "file.exportPdf",
];

/// The reference's popular commands, by meaning, among BlueDoc's (missing ones are skipped).
const POPULAR: [&str; 44] = [
    "review.accept",
    "insert.table",
    "format.alignLeft",
    "format.bullets",
    "format.alignCenter",
    "edit.copy",
    "edit.cut",
    "review.deleteComment",
    "edit.find",
    "view.pageWidth",
    "format.fontSettings",
    "format.color",
    "format.growStep",
    "insert.comment",
    "layout.breaks",
    "insert.picture",
    "insert.textBox",
    "format.lineSpacing",
    "insert.link",
    "view.macros",
    "file.new",
    "review.next",
    "format.numbering",
    "file.open",
    "layout.pageSetup",
    "format.paragraphSettings",
    "edit.paste",
    "review.previous",
    "file.print",
    "edit.redo",
    "review.reject",
    "file.save",
    "file.saveAs",
    "view.multiplePages",
    "insert.shapes",
    "format.shrinkStep",
    "review.spellingDialog",
    "format.highlight",
    "review.trackChanges",
    "edit.undo",
    "view.onePage",
    "insert.footnote",
    "format.painter",
    "insert.symbol",
];

/// Whether `id` is a collapsed-group drop-down rather than a command.
fn is_group_button(id: &str) -> bool {
    crate::doc_r4_fit::GROUP_MENUS.contains(&id)
        || crate::doc_review16::GROUP_IDS.contains(&id)
        || crate::doc_scale18::is_group(id)
}

/// Commands of a group, in order.
pub(crate) fn group_commands(g: &Group) -> Vec<&'static str> {
    let mut out = Vec::new();
    for i in g.large.iter().chain(g.rows.iter().flatten()) {
        if let Item::Large(x) | Item::Labeled(x) | Item::Button(x) = i {
            if !is_group_button(x) && !out.contains(x) {
                out.push(*x);
            }
        }
    }
    out
}

/// Every command a customisation may use: those of the main and tool tabs and the File
/// tab's, each once.
pub(crate) fn all_commands() -> &'static [&'static str] {
    static ALL: OnceLock<Vec<&'static str>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut out: Vec<&'static str> = Vec::new();
        let mut seen = HashSet::new();
        for t in main_tabs().iter().chain(tool_tabs().iter()) {
            for g in &t.groups {
                for c in group_commands(g) {
                    if seen.insert(c) {
                        out.push(c);
                    }
                }
            }
        }
        for c in FILE_COMMANDS.iter().chain(
            crate::doc_forms::options6::QUICK_CHOICES
                .iter()
                .map(|q| &q.0),
        ) {
            if seen.insert(*c) {
                out.push(*c);
            }
        }
        out
    })
}

fn command_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| all_commands().iter().copied().collect())
}

/// The static id of a known command.
pub(crate) fn static_command(id: &str) -> Option<&'static str> {
    command_set().get(id).copied()
}

/// "Choose commands from" categories (label key, English).
pub(crate) const SOURCES: [(&str, &str); 9] = [
    ("doc21.srcPopular", "Popular Commands"),
    ("doc21.srcNotInRibbon", "Commands Not in the Ribbon"),
    ("doc21.srcAll", "All Commands"),
    ("doc21.srcMacros", "Macros"),
    ("doc21.srcFile", "File Tab"),
    ("doc21.srcAllTabs", "All Tabs"),
    ("doc21.srcMainTabs", "Main Tabs"),
    ("doc21.srcToolTabs", "Tool Tabs"),
    ("doc21.srcCustom", "Custom Tabs and Groups"),
];

/// Commands of a flat category (`None` for the tab categories, which are trees).
pub(crate) fn source_commands(i: usize) -> Option<Vec<&'static str>> {
    let have = command_set();
    match i {
        0 => Some(
            POPULAR
                .iter()
                .filter_map(|c| have.get(c).copied())
                .collect(),
        ),
        1 => Some(
            NOT_IN_RIBBON
                .iter()
                .filter_map(|c| have.get(c).copied())
                .filter(|c| {
                    !main_tabs()
                        .iter()
                        .any(|t| t.groups.iter().any(|g| group_commands(g).contains(c)))
                })
                .collect(),
        ),
        2 => Some(all_commands().to_vec()),
        // BlueOffice never runs macros: the list stays empty, as the reference's without any.
        3 => Some(Vec::new()),
        4 => Some(FILE_COMMANDS.to_vec()),
        _ => None,
    }
}

// ------------------------------------------------------------------ building the ribbon

/// Intern a string for the ribbon's `&'static str` ids (custom ids repeat across rebuilds,
/// so each is leaked once).
pub(crate) fn intern(s: &str) -> &'static str {
    static POOL: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let pool = POOL.get_or_init(|| Mutex::new(HashSet::new()));
    let mut g = match pool.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    if let Some(x) = g.get(s) {
        return x;
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    g.insert(leaked);
    leaked
}

/// The collapsed button of a custom group.
pub(crate) fn button_of(group_id: &str) -> &'static str {
    intern(&format!(
        "{BUTTON_PREFIX}{}",
        group_id.trim_start_matches(GROUP_PREFIX)
    ))
}

/// A custom group's commands as small labeled buttons, three to a column.
pub(crate) fn custom_group(g: &CGroup) -> Group {
    let items: Vec<Item> = g
        .commands
        .iter()
        .filter_map(|c| static_command(c))
        .map(Item::Labeled)
        .collect();
    let per = items.len().clamp(1, 3);
    Group {
        label: intern(&g.id),
        large: vec![],
        rows: (0..per)
            .map(|r| items.iter().skip(r).step_by(per).copied().collect())
            .filter(|row: &Vec<Item>| !row.is_empty())
            .collect(),
    }
}

/// The ribbon tab of a customised tab (`None` when hidden).
pub(crate) fn build_tab(t: &CTab) -> Option<Tab> {
    if !t.shown {
        return None;
    }
    let groups = t
        .groups
        .iter()
        .filter_map(|g| {
            if g.is_custom() {
                Some(custom_group(g))
            } else {
                g.builtin()
                    .and_then(|(tab, label)| builtin_group(tab, label))
            }
        })
        .collect();
    Some(Tab {
        label: intern(&t.id),
        groups,
    })
}

/// The permanent tabs of a customisation.
pub(crate) fn build_main(l: &Layout) -> Vec<Tab> {
    l.full()
        .tabs
        .iter()
        .filter(|t| !t.tool)
        .filter_map(build_tab)
        .collect()
}

/// The contextual tabs `tabs` (built by `doc_tools`) as customised: hidden ones dropped,
/// groups as the customisation orders them.
pub(crate) fn build_tools(tabs: Vec<Tab>) -> Vec<Tab> {
    let reg = registry();
    let Some(r) = reg.as_ref() else {
        return tabs;
    };
    tabs.into_iter()
        .filter_map(|t| match r.tools.iter().find(|c| c.id == t.label) {
            Some(c) => build_tab(c),
            None => Some(t),
        })
        .collect()
}

/// Names to set in the window's strings: (key, display name).
pub(crate) fn names(l: &Layout) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for t in &l.full().tabs {
        if !t.name.is_empty() {
            out.push((t.id.clone(), t.name.clone()));
        }
        for g in &t.groups {
            if g.name.is_empty() {
                continue;
            }
            let key = if g.is_custom() {
                g.id.clone()
            } else {
                g.builtin().map_or_else(String::new, |(_, l)| l.to_owned())
            };
            out.push((key, g.name.clone()));
            if g.is_custom() {
                out.push((format!("commands.{}", button_of(&g.id)), g.name.clone()));
            }
        }
    }
    out
}

// ------------------------------------------------------------------ registry

/// What the current customisation adds to the ribbon's static rules.
struct Registry {
    /// Collapsed buttons of custom groups: (button id, tab, group label, icon of).
    buttons: &'static [(&'static str, &'static str, &'static str, &'static str)],
    /// Scaling order of customised tabs.
    scaling: HashMap<&'static str, &'static [&'static str]>,
    /// Custom groups by tab (for the collapsed group's drop-down).
    groups: Vec<(&'static str, Group)>,
    /// Command → collapsed button of the custom group holding it.
    holders: HashMap<&'static str, &'static str>,
    /// Tool tabs customised (the others are shown as `doc_tools` builds them).
    tools: Vec<CTab>,
}

#[cfg(not(test))]
fn store() -> &'static RwLock<Option<Arc<Registry>>> {
    static REG: OnceLock<RwLock<Option<Arc<Registry>>>> = OnceLock::new();
    REG.get_or_init(|| RwLock::new(None))
}

#[cfg(test)]
thread_local! {
    /// Tests run in parallel threads: each keeps its own customisation.
    static LOCAL: std::cell::RefCell<Option<Arc<Registry>>> = const { std::cell::RefCell::new(None) };
}

/// The installed registry.
fn registry() -> Option<Arc<Registry>> {
    #[cfg(test)]
    {
        LOCAL.with(|l| l.borrow().clone())
    }
    #[cfg(not(test))]
    {
        match store().read() {
            Ok(g) => g.clone(),
            Err(e) => e.into_inner().clone(),
        }
    }
}

fn set_registry(r: Option<Registry>) {
    let r = r.map(Arc::new);
    #[cfg(test)]
    {
        LOCAL.with(|l| *l.borrow_mut() = r);
    }
    #[cfg(not(test))]
    {
        let mut w = match store().write() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        *w = r;
    }
}

/// Scaling order of a customised tab: custom groups lose their labels (right to left),
/// then the built-in groups collapse in their own tabs' order, then custom groups become
/// buttons.
fn tab_scaling(t: &CTab) -> Vec<&'static str> {
    let customs: Vec<&'static str> = t
        .groups
        .iter()
        .rev()
        .filter(|g| g.is_custom())
        .map(|g| intern(&g.id))
        .collect();
    let mut out = customs.clone();
    let mut sources: Vec<&str> = Vec::new();
    for g in &t.groups {
        if let Some((tab, _)) = g.builtin() {
            if !sources.contains(&tab) {
                sources.push(tab);
            }
        }
    }
    let present: HashSet<&str> = t
        .groups
        .iter()
        .filter_map(|g| g.builtin().map(|(_, l)| l))
        .collect();
    for s in sources {
        out.extend(
            crate::doc_scale18::scaling(s)
                .iter()
                .filter(|l| present.contains(**l)),
        );
    }
    out.extend(customs);
    out
}

/// Install the customisation `l` for the ribbon's rules (process-wide: every BlueDoc window
/// shows the same ribbon, as the reference).
pub(crate) fn install(l: &Layout) {
    let full = l.full();
    let default = Layout::default_full();
    let mut buttons = Vec::new();
    let mut scaling = HashMap::new();
    let mut groups = Vec::new();
    let mut holders = HashMap::new();
    for t in &full.tabs {
        let tab = intern(&t.id);
        let same = default
            .tabs
            .iter()
            .any(|d| d.id == t.id && d.groups == t.groups);
        if !same {
            let order = tab_scaling(t);
            scaling.insert(tab, &*Box::leak(order.into_boxed_slice()));
        }
        for g in t.groups.iter().filter(|g| g.is_custom()) {
            let label = intern(&g.id);
            let button = button_of(&g.id);
            let first = g
                .commands
                .iter()
                .find_map(|c| static_command(c))
                .unwrap_or("view.properties");
            buttons.push((button, tab, label, first));
            for c in g.commands.iter().filter_map(|c| static_command(c)) {
                holders.entry(c).or_insert(button);
            }
            groups.push((tab, custom_group(g)));
        }
    }
    let reg = Registry {
        buttons: Box::leak(buttons.into_boxed_slice()),
        scaling,
        groups,
        holders,
        tools: full
            .tabs
            .iter()
            .filter(|t| t.tool && !default.tabs.contains(t))
            .cloned()
            .collect(),
    };
    set_registry(if l.is_default() { None } else { Some(reg) });
}

/// Collapsed buttons of custom groups (chained into `doc_scale18`'s table).
pub(crate) fn group_buttons() -> &'static [(&'static str, &'static str, &'static str, &'static str)]
{
    registry().as_ref().map_or(&[], |r| r.buttons)
}

/// Scaling order of tab `tab` (customised, or BlueDoc's own).
pub(crate) fn scaling(tab: &str) -> &'static [&'static str] {
    if let Some(s) = registry()
        .as_ref()
        .and_then(|r| r.scaling.get(tab).copied())
    {
        return s;
    }
    crate::doc_scale18::scaling(tab)
}

/// Group `label` at collapse stage `stage`: a custom group drops its labels, then becomes
/// its drop-down button.
pub(crate) fn shrink(label: &str, stage: u8) -> Option<Group> {
    if !label.starts_with(GROUP_PREFIX) {
        return crate::doc_scale18::shrink(label, stage);
    }
    let reg = registry();
    let g = reg
        .as_ref()?
        .groups
        .iter()
        .find(|(_, g)| g.label == label)?
        .1
        .clone();
    match stage {
        0 => Some(g),
        1 => {
            let mut g = g;
            for row in &mut g.rows {
                for item in row.iter_mut() {
                    if let Item::Labeled(id) = *item {
                        *item = Item::Button(id);
                    }
                }
            }
            Some(g)
        }
        _ => Some(Group {
            label: g.label,
            large: vec![Item::Large(button_of(label))],
            rows: vec![],
        }),
    }
}

/// The collapsed group button holding command `id`.
pub(crate) fn holder(id: &str) -> Option<&'static str> {
    crate::doc_scale18::group_of(id)
        .or_else(|| registry().as_ref().and_then(|r| r.holders.get(id).copied()))
}

/// Custom groups as tabs (for `ribbon::doc_group`).
pub(crate) fn lookup_tabs() -> Vec<Tab> {
    let reg = registry();
    let Some(r) = reg.as_ref() else {
        return Vec::new();
    };
    r.groups
        .iter()
        .map(|(tab, g)| Tab {
            label: tab,
            groups: vec![g.clone()],
        })
        .collect()
}

// ------------------------------------------------------------------ customisation file

/// The customisation file: the ribbon, the Quick Access Toolbar and its position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct File {
    pub(crate) format: String,
    pub(crate) version: u32,
    pub(crate) app: String,
    pub(crate) ribbon: Layout,
    pub(crate) qat: Vec<String>,
    #[serde(default)]
    pub(crate) qat_below: bool,
}

/// Text of a customisation file.
pub(crate) fn export(l: &Layout, qat: &[String], below: bool) -> String {
    let f = File {
        format: FILE_FORMAT.into(),
        version: 1,
        app: "doc".into(),
        ribbon: l.clone(),
        qat: qat.to_vec(),
        qat_below: below,
    };
    serde_json::to_string_pretty(&f).unwrap_or_default()
}

/// Read a customisation file (`None`: not one of BlueDoc's).
pub(crate) fn import(text: &str) -> Option<File> {
    let mut f: File = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    if f.format != FILE_FORMAT || f.app != "doc" {
        return None;
    }
    f.ribbon.sanitize();
    f.qat
        .retain(|c| c == SEPARATOR || static_command(c).is_some());
    Some(f)
}

/// The Quick Access Toolbar's separator entry.
pub(crate) const SEPARATOR: &str = "|";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_builds_the_default_ribbon() {
        let l = Layout::default();
        let tabs = build_main(&l);
        let want = main_tabs();
        assert_eq!(tabs.len(), want.len());
        for (a, b) in tabs.iter().zip(want.iter()) {
            assert_eq!(a.label, b.label);
            assert_eq!(a.groups.len(), b.groups.len());
        }
        assert!(l.is_default());
        assert!(Layout::default_full().is_default());
        assert!(all_commands().len() > 150, "{}", all_commands().len());
        assert!(source_commands(0).is_some_and(|v| v.len() >= 25));
        assert!(source_commands(1).is_some_and(|v| !v.is_empty()));
    }

    #[test]
    fn custom_tab_and_group_with_commands() {
        let mut l = Layout::default().full();
        let tid = l.new_id(TAB_PREFIX);
        let gid = l.new_id(GROUP_PREFIX);
        l.tabs.insert(
            1,
            CTab {
                id: tid.clone(),
                name: "Mine".into(),
                shown: true,
                tool: false,
                groups: vec![CGroup {
                    id: gid.clone(),
                    name: "Fast".into(),
                    commands: vec!["format.bold".into(), "edit.find".into(), "nope".into()],
                }],
            },
        );
        // Remove Home's Clipboard group, hide Mailings.
        l.tabs[0]
            .groups
            .retain(|g| g.id != "ribbon.tabs.home|ribbon.groups.clipboard");
        if let Some(m) = l.tabs.iter_mut().find(|t| t.id == "ribbon.tabs.mailings") {
            m.shown = false;
        }
        l.sanitize();
        assert_eq!(
            l.tabs[1].groups[0].commands,
            vec!["format.bold", "edit.find"]
        );
        let tabs = build_main(&l);
        assert_eq!(tabs[1].label, tid);
        assert_eq!(tabs[0].groups.len(), main_tabs()[0].groups.len() - 1);
        assert!(!tabs.iter().any(|t| t.label == "ribbon.tabs.mailings"));
        assert_eq!(l.hidden_main(), vec!["ribbon.tabs.mailings".to_owned()]);
        let n = names(&l);
        assert!(n.contains(&(tid.clone(), "Mine".into())));
        assert!(n.contains(&(gid.clone(), "Fast".into())));
        // The file round-trips.
        let text = export(&l, &["file.save".into(), SEPARATOR.into()], true);
        let f = import(&text).expect("file");
        assert_eq!(f.ribbon, l);
        assert!(f.qat_below);
        assert_eq!(f.qat.len(), 2);
        assert!(import("{\"format\":\"other\"}").is_none());
        // Scaling: custom group first (labels), built-in Home order, then the button.
        let order = tab_scaling(&l.tabs[1]);
        assert_eq!(order, vec![intern(&gid), intern(&gid)]);
        let home = tab_scaling(&l.tabs[0]);
        assert!(!home.contains(&"ribbon.groups.clipboard"));
        assert!(home.contains(&"ribbon.groups.font"));
    }
}
