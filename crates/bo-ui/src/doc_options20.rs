//! File › Options pages of round 20 (docs/ui/REPORT.md "Vòng 110"), on the frame of round 6
//! (`doc_options6`): **Customize Ribbon** (the main tabs shown, BlueOffice's own ribbon
//! customisation kept with the application preferences in `%LOCALAPPDATA%\BlueOffice`),
//! **Add-ins** (what BlueDoc loads: its own built-in parts; it loads no COM add-ins) and
//! **Trust Center** (BlueOffice's own settings: macros are never run, external content is
//! never loaded, the message about a document's macros, Protected View for files from the
//! Internet and e-mail attachments). A child module of `doc_forms`.
use super::form::{Form, Kind};
use crate::strings::Strings;

/// Pages of the Options dialog.
const RIBBON: usize = 7;
const ADDINS: usize = 9;
const TRUST: usize = 10;

/// BlueDoc's main tabs: (label key, check box id).
pub(crate) const TABS: [(&str, &str); 8] = [
    ("ribbon.tabs.home", "rTab0"),
    ("ribbon.tabs.insert", "rTab1"),
    ("ribbon.tabs.design", "rTab2"),
    ("ribbon.tabs.layout", "rTab3"),
    ("ribbon.tabs.references", "rTab4"),
    ("ribbon.tabs.mailings", "rTab5"),
    ("ribbon.tabs.review", "rTab6"),
    ("ribbon.tabs.view", "rTab7"),
];

/// The built-in parts BlueDoc loads, shown on the Add-ins page: (name key, English, type
/// key, English).
const ADDIN_ROWS: [(&str, &str, &str, &str); 5] = [
    (
        "doc20.aiScript",
        "BlueScript (automation, sandboxed)",
        "doc20.aiBuiltIn",
        "Built-in",
    ),
    (
        "doc20.aiProofing",
        "Proofing tools (Vietnamese, English)",
        "doc20.aiBuiltIn",
        "Built-in",
    ),
    (
        "doc20.aiPdf",
        "PDF and PDF/A export",
        "doc20.aiBuiltIn",
        "Built-in",
    ),
    (
        "doc20.aiSign",
        "Digital signatures (PAdES / OOXML)",
        "doc20.aiBuiltIn",
        "Built-in",
    ),
    (
        "doc20.aiWeb",
        "Organisation web add-ins",
        "doc20.aiWebType",
        "Web add-in",
    ),
];

/// Round-20 preferences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Extra {
    /// Main tabs hidden (label keys).
    pub(crate) hidden_tabs: Vec<String>,
    /// Show a message when a document holds macros (they are never run).
    pub(crate) macro_message: bool,
    /// Open files from the Internet read-only (Protected View).
    pub(crate) protect_internet: bool,
    /// Open e-mail attachments read-only (Protected View).
    pub(crate) protect_attachments: bool,
    /// Round 21: the ribbon customisation (empty: the default ribbon).
    pub(crate) ribbon: crate::doc_ribbon21::Layout,
    /// Round 21: the Quick Access Toolbar below the ribbon.
    pub(crate) qat_below: bool,
}

impl Default for Extra {
    fn default() -> Self {
        Self {
            hidden_tabs: Vec::new(),
            macro_message: true,
            protect_internet: true,
            protect_attachments: true,
            ribbon: crate::doc_ribbon21::Layout::default(),
            qat_below: false,
        }
    }
}

impl Extra {
    /// Read one `key=value` line of the preferences (unknown keys are ignored).
    pub(crate) fn read(&mut self, key: &str, v: &str) {
        match key {
            "hiddenTabs" => {
                self.hidden_tabs = v
                    .split(',')
                    .map(str::trim)
                    .filter(|t| TABS.iter().any(|(k, _)| k == t))
                    .map(str::to_owned)
                    .collect();
            }
            "macroMessage" => self.macro_message = v != "0",
            "protectInternet" => self.protect_internet = v != "0",
            "protectAttachments" => self.protect_attachments = v != "0",
            "ribbon21" => {
                self.ribbon = crate::doc_ribbon21::Layout::from_json(v).unwrap_or_default();
            }
            "qatBelow" => self.qat_below = v == "1",
            _ => {}
        }
    }

    /// The preferences as `key=value` lines.
    pub(crate) fn to_text(&self) -> String {
        let b = |v: bool| if v { "1" } else { "0" };
        format!(
            "hiddenTabs={}\nmacroMessage={}\nprotectInternet={}\nprotectAttachments={}\n",
            self.hidden_tabs.join(","),
            b(self.macro_message),
            b(self.protect_internet),
            b(self.protect_attachments),
        ) + &format!(
            "ribbon21={}
qatBelow={}
",
            if self.ribbon.tabs.is_empty() {
                String::new()
            } else {
                serde_json::to_string(&self.ribbon).unwrap_or_default()
            },
            b(self.qat_below),
        )
    }

    /// Whether a file opened from `zone` (the Windows security zone of its download mark:
    /// 3 Internet, 4 Restricted) or as an e-mail attachment opens in Protected View (the host
    /// does not ask yet: see docs/ui/REPORT.md "Vòng 110").
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn protected(&self, zone: u32, attachment: bool) -> bool {
        (zone >= 3 && self.protect_internet) || (attachment && self.protect_attachments)
    }
}

fn tr(s: &Strings, key: &str, fallback: &str) -> String {
    let v = s.get(key);
    if v == key {
        fallback.to_owned()
    } else {
        v.to_owned()
    }
}

/// Add the Customize Ribbon, Add-ins and Trust Center pages.
#[allow(clippy::too_many_lines)]
pub(super) fn add_pages(f: &mut Form, s: &Strings, p: &super::options6::DocPrefs) {
    let t = |k: &str, fb: &str| tr(s, k, fb);
    let intro = |f: &mut Form, id: &'static str, icon: &'static str, page: usize, text: String| {
        f.add(
            id,
            Some(page),
            [305.0, 70.0, 660.0, 24.0],
            Kind::Label(text),
        );
        f.add(icon, Some(page), [258.0, 56.0, 40.0, 46.0], Kind::Canvas);
    };
    let heading = |f: &mut Form, id: &'static str, page: usize, y: f32, text: String| {
        f.add(id, Some(page), [241.0, y, 755.0, 0.0], Kind::Group(text));
    };
    // ------------------------------------------------------------ Customize Ribbon
    intro(
        f,
        "rIntro",
        "icon7",
        RIBBON,
        t("doc20.ribbonIntro", "Customize the Ribbon."),
    );
    heading(f, "rMain", RIBBON, 130.0, t("doc20.mainTabs", "Main Tabs"));
    for (i, (key, id)) in TABS.iter().enumerate() {
        let label = t(key, key.rsplit('.').next().unwrap_or(key));
        f.add(
            id,
            Some(RIBBON),
            [257.0, 170.0 + 36.0 * i as f32, 400.0, 24.0],
            Kind::Check {
                label,
                on: !p.extra.hidden_tabs.iter().any(|h| h == key),
            },
        );
    }
    f.add(
        "rNote",
        Some(RIBBON),
        [257.0, 170.0 + 36.0 * 8.0 + 8.0, 720.0, 24.0],
        Kind::Note(t(
            "doc20.ribbonNote",
            "Clear a tab to hide it. Contextual tabs (Table, Picture, Shape…) come with what you select.",
        )),
    );
    f.add(
        "rReset",
        Some(RIBBON),
        [257.0, 716.0, 110.0, 32.0],
        Kind::Button {
            label: t("docR6.opt.reset", "Reset"),
            default: false,
        },
    );
    // ------------------------------------------------------------ Add-ins
    intro(
        f,
        "aIntro",
        "icon9",
        ADDINS,
        t(
            "doc20.addinsIntro",
            "View and manage the add-ins of BlueDoc.",
        ),
    );
    // The reference's table: Name / Location / Type, active and inactive groups.
    f.add(
        "aTable",
        Some(ADDINS),
        [257.0, 120.0, 740.0, 380.0],
        Kind::Canvas,
    );
    f.add(
        "aManageL",
        Some(ADDINS),
        [257.0, 726.0, 90.0, 24.0],
        Kind::Label(t("doc20.manage", "Manage:")),
    );
    f.add(
        "aManage",
        Some(ADDINS),
        [350.0, 720.0, 260.0, 30.0],
        Kind::Combo {
            // Round 22: every choice opens its list with Go….
            items: super::r22::MANAGE.iter().map(|(k, en)| t(k, en)).collect(),
            sel: 0,
        },
    );
    f.add(
        "aGo",
        Some(ADDINS),
        [624.0, 719.0, 90.0, 32.0],
        Kind::Button {
            label: t("doc20.go", "Go..."),
            default: false,
        },
    );
    f.add(
        "aNote",
        Some(ADDINS),
        [257.0, 520.0, 740.0, 24.0],
        Kind::Note(t(
            "doc20.addinsNote",
            "BlueDoc loads no COM add-ins and runs no macros; web add-ins run in a sandbox with the permissions they ask for.",
        )),
    );
    // ------------------------------------------------------------ Trust Center
    intro(
        f,
        "tIntro",
        "icon10",
        TRUST,
        t(
            "doc20.trustIntro",
            "Help keep your documents safe and your computer secure and healthy.",
        ),
    );
    heading(
        f,
        "tMacros",
        TRUST,
        130.0,
        t("doc20.macros", "Macro Settings"),
    );
    f.add(
        "tMacroNote",
        Some(TRUST),
        [257.0, 166.0, 740.0, 24.0],
        Kind::Note(t(
            "doc20.macroNote",
            "BlueOffice never runs macros (VBA): documents keep them, View › Macros shows their code.",
        )),
    );
    f.add(
        "tMacroMsg",
        Some(TRUST),
        [257.0, 202.0, 700.0, 24.0],
        Kind::Check {
            label: t(
                "doc20.macroMessage",
                "Tell me when a document contains macros",
            ),
            on: p.extra.macro_message,
        },
    );
    heading(
        f,
        "tExternal",
        TRUST,
        252.0,
        t("doc20.external", "External Content"),
    );
    f.add(
        "tExtNote",
        Some(TRUST),
        [257.0, 288.0, 740.0, 24.0],
        Kind::Note(t(
            "doc20.externalNote",
            "Linked pictures, linked files and data connections are never fetched when a document opens.",
        )),
    );
    heading(
        f,
        "tProtected",
        TRUST,
        338.0,
        t("doc20.protected", "Protected View"),
    );
    for (i, (id, key, en, on)) in [
        (
            "tInternet",
            "doc20.protectInternet",
            "Open files that come from the Internet as read-only (Protected View)",
            p.extra.protect_internet,
        ),
        (
            "tAttach",
            "doc20.protectAttachments",
            "Open e-mail attachments as read-only (Protected View)",
            p.extra.protect_attachments,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        f.add(
            id,
            Some(TRUST),
            [257.0, 374.0 + 36.0 * i as f32, 740.0, 24.0],
            Kind::Check {
                label: t(key, en),
                on,
            },
        );
    }
}

/// Buttons of the round-20 pages.
pub(crate) fn handles(id: &str) -> bool {
    id == "rReset" || id == "aGo"
}

/// A button of the round-20 pages was pressed.
pub(super) fn clicked(f: &mut Form, id: &str) {
    if id == "rReset" {
        for (_, cid) in TABS {
            f.set_on(cid, true);
        }
    }
}

/// The round-20 preferences shown.
pub(super) fn collect(f: &Form, p: &mut super::options6::DocPrefs) {
    p.extra.hidden_tabs = TABS
        .iter()
        .filter(|(_, id)| !f.on(id))
        .map(|(k, _)| (*k).to_owned())
        .collect();
    // At least one tab stays.
    if p.extra.hidden_tabs.len() == TABS.len() {
        p.extra.hidden_tabs.remove(0);
    }
    p.extra.macro_message = f.on("tMacroMsg");
    p.extra.protect_internet = f.on("tInternet");
    p.extra.protect_attachments = f.on("tAttach");
}

/// Text of the Add-ins table.
pub(super) struct AddinTable {
    heads: [String; 3],
    groups: [String; 2],
    active: Vec<[String; 3]>,
    inactive: Vec<[String; 3]>,
}

impl AddinTable {
    /// The table in the interface language.
    pub(super) fn new(s: &Strings) -> Self {
        let (active, inactive) = addin_rows(s);
        AddinTable {
            heads: [
                tr(s, "doc20.colName", "Name"),
                tr(s, "doc20.colLocation", "Location"),
                tr(s, "doc20.colType", "Type"),
            ],
            groups: [
                tr(s, "doc20.active", "Active Application Add-ins"),
                tr(s, "doc20.inactive", "Inactive Application Add-ins"),
            ],
            active,
            inactive,
        }
    }
}

/// Rows of the Add-ins table: (name, location, type), active ones first.
pub(super) fn addin_rows(s: &Strings) -> (Vec<[String; 3]>, Vec<[String; 3]>) {
    let built_in = tr(s, "doc20.aiLocation", "Built into BlueDoc");
    let mut active = Vec::new();
    let mut inactive = Vec::new();
    for (k, en, tk, ten) in ADDIN_ROWS {
        let row = [tr(s, k, en), built_in.clone(), tr(s, tk, ten)];
        if k == "doc20.aiWeb" {
            inactive.push([
                row[0].clone(),
                tr(s, "doc20.aiServer", "Organisation catalog"),
                row[2].clone(),
            ]);
        } else {
            active.push(row);
        }
    }
    (active, inactive)
}

/// Paint the Add-ins table into `r` (device pixels; `k` device px per reference px, `base`
/// the form's text size).
pub(super) fn paint_addins(
    p: &mut dyn crate::Painter,
    r: [f32; 4],
    k: f32,
    base: f32,
    t: &AddinTable,
) {
    use bo_render::Rgba;
    let size = base.round();
    let st = crate::TextStyle { size, bold: false };
    let bold = crate::TextStyle { size, bold: true };
    let row_h = (size * 1.6).round();
    let cols = [r[0] + 6.0 * k, r[0] + 380.0 * k, r[0] + 620.0 * k];
    let ink = Rgba(0x26, 0x26, 0x26, 255);
    let grey = Rgba(0x60, 0x60, 0x60, 255);
    let mut y = r[1];
    for (i, h) in t.heads.iter().enumerate() {
        p.text(h, cols[i], y, bold, ink);
    }
    y += row_h;
    p.fill_rect(
        r[0],
        y - 4.0 * k,
        r[2],
        k.max(1.0),
        Rgba(0xC8, 0xC8, 0xC8, 255),
    );
    for (title, rows) in [(&t.groups[0], &t.active), (&t.groups[1], &t.inactive)] {
        y += 4.0 * k;
        p.text(title, cols[0], y, bold, ink);
        y += row_h;
        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                p.text(
                    cell,
                    cols[i] + if i == 0 { 14.0 * k } else { 0.0 },
                    y,
                    st,
                    grey,
                );
            }
            y += row_h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::options6::{DocPrefs, OptionsDialog};
    use super::*;

    #[test]
    fn extra_preferences_round_trip() {
        let mut p = DocPrefs::default();
        p.extra.hidden_tabs = vec!["ribbon.tabs.mailings".into()];
        p.extra.protect_internet = false;
        let back = DocPrefs::from_text(&p.to_text());
        assert_eq!(back, p);
        assert!(back.extra.protected(0, true));
        assert!(!back.extra.protected(3, false));
        assert!(DocPrefs::default().extra.protected(3, false));
    }

    #[test]
    fn pages_collect_and_reset() {
        let s = Strings::load("en-US");
        let mut p = DocPrefs::default();
        p.extra.hidden_tabs = vec!["ribbon.tabs.review".into()];
        let d = OptionsDialog::open(&s, &p);
        assert_eq!(d.collect().extra.hidden_tabs, vec!["ribbon.tabs.review"]);
        let mut f = Form::new("x", (100.0, 100.0), vec![]);
        add_pages(&mut f, &s, &p);
        assert!(!f.on("rTab6"));
        clicked(&mut f, "rReset");
        assert!(f.on("rTab6"));
        for (_, id) in TABS {
            f.set_on(id, false);
        }
        let mut q = DocPrefs::default();
        collect(&f, &mut q);
        assert_eq!(q.extra.hidden_tabs.len(), TABS.len() - 1);
    }
}
