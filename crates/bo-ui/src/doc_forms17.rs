//! BlueDoc dialogs of parity round 17 (docs/ui/REPORT.md "Vòng 107"), laid out from the
//! reference word processor's dialogs captured at 125 %: **Bookmark** (Insert › Links:
//! name box and list, Sort by Name / Location, Hidden bookmarks, Add / Delete / Go To),
//! **Object** (Insert › Text › Object: Create New / Create from File, an embedded or linked
//! workbook), **Macros** (View › Macros: the document's VBA macros, read only — BlueOffice
//! never runs VBA) and **Insert a video** (Insert › Media › Online Videos). A child module
//! of `doc_forms`.
use super::{t, DocForm, DocFormKind, FormContext, FormOut};
use crate::strings::Strings;
use serde_json::{json, Value};

/// Whether `kind` is one of this module's dialogs.
pub(super) fn handles(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::Bookmark
            | DocFormKind::InsertObject
            | DocFormKind::Macros
            | DocFormKind::OnlineVideo
            | DocFormKind::MacroSource
            | DocFormKind::Icons
            | DocFormKind::StyleNumbering
            | DocFormKind::CustomizeKeyboard
            | DocFormKind::StyleEffects
            | DocFormKind::StyleFrame
    )
}

/// The name of a shortcut (`Ctrl+Shift+L`, `Alt+F5`); `None` without Ctrl or Alt, or for a
/// key a shortcut cannot use.
pub(crate) fn key_name(k: crate::Key, m: crate::Mods) -> Option<String> {
    if !m.ctrl && !m.alt {
        return None;
    }
    let key = match k {
        crate::Key::Letter(c) if c.is_ascii_graphic() => c.to_ascii_uppercase().to_string(),
        crate::Key::F(n) if (1..=12).contains(&n) => format!("F{n}"),
        _ => return None,
    };
    let mut s = String::new();
    for (on, name) in [(m.ctrl, "Ctrl+"), (m.alt, "Alt+"), (m.shift, "Shift+")] {
        if on {
            s.push_str(name);
        }
    }
    s.push_str(&key);
    Some(s)
}

/// Number formats Format ▾ › Numbering… offers a style: (label, §17.18.59 numFmt, lvlText).
pub(crate) const STYLE_NUMBERS: [(&str, &str, &str); 12] = [
    ("None", "", ""),
    ("1. 2. 3.", "decimal", "%1."),
    ("1) 2) 3)", "decimal", "%1)"),
    ("I. II. III.", "upperRoman", "%1."),
    ("A. B. C.", "upperLetter", "%1."),
    ("a) b) c)", "lowerLetter", "%1)"),
    ("a. b. c.", "lowerLetter", "%1."),
    ("i. ii. iii.", "lowerRoman", "%1."),
    ("\u{2022}  Bullet", "bullet", "\u{2022}"),
    ("\u{25CB}  Hollow bullet", "bullet", "\u{25CB}"),
    ("\u{25A0}  Square bullet", "bullet", "\u{25A0}"),
    ("\u{2713}  Check mark", "bullet", "\u{2713}"),
];

/// Open dialog `kind` with the state in `c.extra["r17"]`.
pub(super) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    match kind {
        DocFormKind::InsertObject => open_object(s, c),
        DocFormKind::Macros => open_macros(s, c),
        DocFormKind::OnlineVideo => open_video(s),
        DocFormKind::MacroSource => open_source(s, c),
        DocFormKind::Icons => open_icons(s),
        DocFormKind::StyleNumbering => open_style_numbering(s),
        DocFormKind::CustomizeKeyboard => open_keyboard(s, c),
        DocFormKind::StyleEffects => open_style_effects(s),
        DocFormKind::StyleFrame => open_style_frame(s),
        _ => open_bookmark(s, c),
    }
}

/// A bookmark of the document: name, (paragraph, offset) of its start, hidden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mark {
    name: String,
    at: (u64, u64),
    hidden: bool,
}

fn marks_of(v: &Value) -> Vec<Mark> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|b| {
                    Some(Mark {
                        name: b["name"].as_str()?.to_owned(),
                        at: (
                            b["start"]["p"].as_u64().unwrap_or(0),
                            b["start"]["o"].as_u64().unwrap_or(0),
                        ),
                        hidden: b["hidden"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Bookmark (567 × 451 measured): name box over the list, Add / Delete / Go To at the
/// right, Sort by Name / Location, Hidden bookmarks, Cancel (Close after a Go To).
fn open_bookmark(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::Bookmark,
        &tr("doc17.bmTitle", "Bookmark"),
        (566.0, 450.0),
        vec![],
    );
    f.label(
        "nameL",
        [18.0, 47.0, 300.0, 18.0],
        &tr("doc17.bmName", "Bookmark name:"),
    );
    f.edit("name", [18.0, 68.0, 404.0, 26.0], "");
    f.list("marks", [18.0, 94.0, 404.0, 206.0], Vec::new(), None);
    f.button(
        "add",
        [436.0, 68.0, 111.0, 28.0],
        &tr("doc17.bmAdd", "Add"),
        true,
    );
    f.button(
        "delete",
        [436.0, 107.0, 111.0, 28.0],
        &tr("doc17.bmDelete", "Delete"),
        false,
    );
    f.button(
        "goTo",
        [436.0, 146.0, 111.0, 28.0],
        &tr("doc17.bmGoTo", "Go To"),
        false,
    );
    f.label(
        "sortL",
        [18.0, 309.0, 60.0, 18.0],
        &tr("doc17.bmSortBy", "Sort by:"),
    );
    f.radio(
        "byName",
        "sort",
        [78.0, 306.0, 160.0, 22.0],
        &tr("doc17.bmByName", "Name"),
        true,
    );
    f.radio(
        "byLocation",
        "sort",
        [78.0, 332.0, 160.0, 22.0],
        &tr("doc17.bmByLocation", "Location"),
        false,
    );
    f.check(
        "hidden",
        [18.0, 368.0, 250.0, 22.0],
        &tr("doc17.bmHidden", "Hidden bookmarks"),
        false,
    );
    f.button(
        "cancel",
        [436.0, 403.0, 111.0, 28.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.r17 = Some(Box::new(R17 {
        marks: marks_of(&c.extra["r17"]["bookmarks"]),
        close: tr("docR3.close", "Close"),
        ..R17::default()
    }));
    f.bm_refresh(None);
    // The selection's bookmark (or the first) is chosen, its name in the box, as the
    // reference opens.
    let first = f.ids.first().cloned();
    if let Some(n) = c.extra["r17"]["current"]
        .as_str()
        .map(str::to_owned)
        .or(first)
    {
        f.bm_choose(&n);
    }
    f.form.focus("name");
    // The name is selected in its box, as the reference opens.
    f.form.select_all("name");
    f
}

/// Round-17 state of a dialog.
#[derive(Debug, Default)]
pub(crate) struct R17 {
    marks: Vec<Mark>,
    /// "Close" (Cancel's label after a Go To).
    close: String,
    /// Macros: (macro, module) per row, and the module sources.
    macros: Vec<(String, String)>,
    /// Insert Icons: the library, the icons shown (filtered), the chosen ones, the scroll
    /// (reference pixels) and the category ids of the drop-down.
    icons: Vec<IconEntry>,
    shown: Vec<usize>,
    chosen: Vec<usize>,
    scroll: f32,
    categories: Vec<String>,
    /// "{n} selected" with its placeholder, and Insert's label.
    count_label: String,
}

/// An icon of the library with its category and search words (both languages).
#[derive(Debug, Clone)]
pub(crate) struct IconEntry {
    icon: crate::slide_pickers::LibIcon,
    category: String,
    words: String,
}

impl DocForm {
    /// Fill the bookmark list (sorted, hidden ones when asked) and choose `keep` again.
    fn bm_refresh(&mut self, keep: Option<&str>) {
        let Some(st) = self.r17.as_ref() else {
            return;
        };
        let hidden = self.on("hidden");
        let mut v: Vec<&Mark> = st.marks.iter().filter(|m| hidden || !m.hidden).collect();
        if self.chosen("sort") == Some("byLocation") {
            v.sort_by_key(|m| m.at);
        } else {
            v.sort_by_key(|m| m.name.to_lowercase());
        }
        let names: Vec<String> = v.iter().map(|m| m.name.clone()).collect();
        let sel = keep.and_then(|k| names.iter().position(|n| n == k));
        self.form.set_items("marks", names.clone(), sel);
        self.ids = names;
        self.bm_enable();
    }

    fn bm_choose(&mut self, name: &str) {
        let i = self.ids.iter().position(|n| n == name);
        self.form.set_sel("marks", i);
        self.set_text("name", name);
        self.bm_enable();
    }

    /// Add needs a valid name; Delete and Go To an existing bookmark.
    fn bm_enable(&mut self) {
        let name = self.text("name");
        let exists = self.ids.contains(&name);
        self.form.enable(
            "add",
            bo_edit::bookmarks17::valid_bookmark_name(name.trim()),
        );
        self.form.enable("delete", exists);
        self.form.enable("goTo", exists);
        // The reference shows Go To as the default-looking button once a bookmark of the
        // list is named, Add otherwise.
        for (id, on) in [("add", !exists), ("goTo", exists)] {
            if let Some(super::form::Kind::Button { default, .. }) =
                self.form.get_mut(id).map(|c| &mut c.kind)
            {
                *default = on;
            }
        }
    }

    pub(super) fn r17_changed(&mut self, id: &str) {
        match (self.kind, id) {
            (DocFormKind::Bookmark, "marks") => {
                if let Some(n) = self.sel("marks").and_then(|i| self.ids.get(i).cloned()) {
                    self.set_text("name", &n);
                }
                self.bm_enable();
            }
            (DocFormKind::Bookmark, "name") => {
                // The list follows the name typed (its row when there is one).
                let name = self.text("name");
                let i = self.ids.iter().position(|n| *n == name);
                self.form.set_sel("marks", i);
                self.bm_enable();
            }
            (DocFormKind::Bookmark, "byName" | "byLocation" | "hidden") => {
                let keep = self.text("name");
                self.bm_refresh(Some(&keep));
            }
            (DocFormKind::InsertObject, "fileName" | "link") => {}
            (DocFormKind::Macros, "macros") => {
                if let Some(i) = self.sel("macros") {
                    let (m, module) = self
                        .r17
                        .as_ref()
                        .and_then(|r| r.macros.get(i).cloned())
                        .unwrap_or_default();
                    self.set_text("name", &m);
                    self.set_label("desc", &format!("Module: {module}"));
                }
                self.mac_enable();
            }
            (DocFormKind::Macros, "name") => {
                let name = self.text("name");
                let i = self
                    .r17
                    .as_ref()
                    .and_then(|r| r.macros.iter().position(|m| m.0 == name));
                self.form.set_sel("macros", i);
                self.mac_enable();
            }
            (DocFormKind::Icons, "search" | "category") => self.icons_filter(),
            (DocFormKind::OnlineVideo, "url") => {
                let ok = video_url(&self.text("url")).is_some();
                self.form.enable("ok", ok);
            }
            _ => {}
        }
    }

    pub(super) fn r17_clicked(&mut self, id: &str) -> Option<FormOut> {
        if !handles(self.kind) {
            return None;
        }
        Some(match (self.kind, id) {
            (DocFormKind::Bookmark, "add") => return Some(self.r17_ok()),
            (DocFormKind::Bookmark, "delete") => {
                let name = self.text("name");
                if let Some(st) = self.r17.as_mut() {
                    st.marks.retain(|m| m.name != name);
                }
                self.bm_refresh(None);
                self.set_text("name", "");
                self.bm_enable();
                FormOut::Run(
                    vec![json!({ "cmd": "bookmarkDelete", "name": name })],
                    false,
                )
            }
            (DocFormKind::Bookmark, "goTo") => {
                let name = self.text("name");
                let close = self
                    .r17
                    .as_ref()
                    .map(|r| r.close.clone())
                    .unwrap_or_default();
                if let Some(super::form::Kind::Button { label, .. }) =
                    self.form.get_mut("cancel").map(|c| &mut c.kind)
                {
                    *label = close;
                }
                FormOut::Run(vec![json!({ "cmd": "bookmarkGoTo", "name": name })], false)
            }
            (DocFormKind::InsertObject, "browse") => FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "r17.pick:object" })],
                false,
            ),
            (DocFormKind::Macros, "edit") => FormOut::Stash(DocFormKind::MacroSource),
            (DocFormKind::StyleFrame, "removeFrame") => FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "r17.styleFrame:" })],
                true,
            ),
            (DocFormKind::CustomizeKeyboard, "assign") => return Some(self.kb_assign()),
            (DocFormKind::CustomizeKeyboard, "remove") => {
                let Some(key) = self.sel("current").and_then(|i| self.ids.get(i).cloned()) else {
                    return Some(FormOut::Redraw);
                };
                self.ids.retain(|k| *k != key);
                let keys = self.ids.clone();
                self.form.set_items("current", keys, None);
                FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("r17.key.remove:{key}") })],
                    false,
                )
            }
            _ => FormOut::Redraw,
        })
    }

    /// OK (Enter) of a round-17 dialog.
    pub(crate) fn r17_ok(&mut self) -> FormOut {
        match self.kind {
            DocFormKind::Bookmark => {
                let name = self.text("name").trim().to_owned();
                if !bo_edit::bookmarks17::valid_bookmark_name(&name) {
                    return FormOut::Redraw;
                }
                FormOut::Run(vec![json!({ "cmd": "bookmarkAdd", "name": name })], true)
            }
            DocFormKind::InsertObject => {
                if self.form.tab == 0 {
                    return FormOut::Run(vec![json!({ "cmd": "oleInsert" })], true);
                }
                let path = self.text("fileName");
                if self.on("link") && !path.trim().is_empty() {
                    let data = self.files[0].as_ref().map(|f| f.1.clone());
                    return FormOut::Run(
                        vec![json!({ "cmd": "oleInsert", "link": path.trim(), "data": data })],
                        true,
                    );
                }
                match &self.files[0] {
                    Some((_, b64)) => {
                        FormOut::Run(vec![json!({ "cmd": "oleInsert", "data": b64 })], true)
                    }
                    None => FormOut::Redraw,
                }
            }
            DocFormKind::Icons => {
                let ids: Vec<String> = self
                    .r17
                    .as_ref()
                    .map(|st| {
                        st.chosen
                            .iter()
                            .filter_map(|i| st.icons.get(*i))
                            .map(|e| e.icon.id.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                if ids.is_empty() {
                    return FormOut::Redraw;
                }
                FormOut::Run(
                    vec![
                        json!({ "cmd": "ui.action", "id": format!("r17.icons:{}", ids.join(",")) }),
                    ],
                    true,
                )
            }
            DocFormKind::CustomizeKeyboard => self.kb_assign(),
            DocFormKind::StyleFrame => FormOut::Run(
                vec![
                    json!({ "cmd": "ui.action", "id": format!("r17.styleFrame:{}", self.frame_xml()) }),
                ],
                true,
            ),
            DocFormKind::StyleEffects => {
                // The same effects as Home › Text Effects (w14; [MS-DOCX] 2.6); unticked
                // ones are removed from the style.
                let pick = |on: bool, v: Value| if on { v } else { Value::Null };
                let patch = json!({
                    "textOutline": pick(self.on("fxOutline"), json!({ "color": "4472C4", "width": 0.75 })),
                    "softShadow": pick(self.on("fxShadow"), json!({
                        "color": "000000", "blur": 4.0, "distance": 3.0, "angle": 45.0, "transparency": 60,
                    })),
                    "glow": pick(self.on("fxGlow"), json!({ "color": "4472C4", "radius": 5.0, "transparency": 60 })),
                    "reflection": pick(self.on("fxReflection"), json!({
                        "size": 50, "transparency": 50, "distance": 0.0,
                    })),
                });
                FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("r17.styleFx:{patch}") })],
                    true,
                )
            }
            DocFormKind::StyleNumbering => FormOut::Run(
                vec![json!({ "cmd": "ui.action",
                    "id": format!("r17.styleNum:{}", self.sel("formats").unwrap_or(0)) })],
                true,
            ),
            DocFormKind::OnlineVideo => match video_url(&self.text("url")) {
                Some(u) => FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("r17.video:{u}") })],
                    true,
                ),
                None => FormOut::Redraw,
            },
            _ => FormOut::Close,
        }
    }

    /// Macros: Edit (view the source) needs a macro of the list.
    fn mac_enable(&mut self) {
        let has = self.sel("macros").is_some();
        self.form.enable("edit", has);
    }

    /// The module source shown by Macros › Edit (`extra` of the source window).
    pub(crate) fn r17_macro_module(&self) -> Option<String> {
        let st = self.r17.as_ref()?;
        let i = self.sel("macros")?;
        st.macros.get(i).map(|m| m.1.clone())
    }

    /// A file picked for Create from File (name, base64).
    pub(crate) fn r17_set_file(&mut self, name: &str, path: &str, b64: &str) {
        self.files[0] = Some((name.to_owned(), b64.to_owned()));
        self.set_text("fileName", if path.is_empty() { name } else { path });
    }
}

/// Object (768 × 464 measured): tabs Create New (object types BlueOffice embeds) and
/// Create from File (file name, Browse…, Link to file).
fn open_object(s: &Strings, _c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::InsertObject,
        &tr("doc17.objTitle", "Object"),
        (766.0, 462.0),
        vec![
            tr("doc17.objCreateNew", "Create New"),
            tr("doc17.objFromFile", "Create from File"),
        ],
    );
    f.label_on(
        0,
        "typeL",
        [32.0, 89.0, 300.0, 18.0],
        &tr("doc17.objType", "Object type:"),
    );
    f.form.add(
        "types",
        Some(0),
        [32.0, 109.0, 464.0, 138.0],
        super::form::Kind::List {
            items: vec![tr("doc17.objWorkbook", "BlueSheet Worksheet")],
            sel: Some(0),
            top: 0,
        },
    );
    f.form.add(
        "resultG",
        Some(0),
        [32.0, 260.0, 464.0, 128.0],
        super::form::Kind::Group(tr("doc17.objResult", "Result")),
    );
    let new_text = tr(
        "doc17.objResultNew",
        "Inserts a new BlueSheet Worksheet object into your document.",
    );
    for (i, line) in wrap(&new_text, 44).iter().take(3).enumerate() {
        let id = ["result", "resultB", "resultC"][i];
        f.label_on(0, id, [134.0, 280.0 + 22.0 * i as f32, 350.0, 20.0], line);
    }
    f.label_on(
        1,
        "fileL",
        [32.0, 89.0, 300.0, 18.0],
        &tr("doc17.objFileName", "File name:"),
    );
    f.edit_on(1, "fileName", [32.0, 109.0, 464.0, 26.0], "*.*");
    f.button_on(
        1,
        "browse",
        [32.0, 145.0, 111.0, 28.0],
        &tr("doc17.objBrowse", "Browse..."),
    );
    f.check_on(
        1,
        "link",
        [510.0, 109.0, 200.0, 22.0],
        &tr("doc17.objLink", "Link to file"),
        false,
    );
    f.form.add(
        "resultG2",
        Some(1),
        [32.0, 260.0, 464.0, 128.0],
        super::form::Kind::Group(tr("doc17.objResult", "Result")),
    );
    let file_text = tr(
        "doc17.objResultFile",
        "Inserts the contents of the workbook into your document so that you can edit it later with BlueSheet.",
    );
    for (i, line) in wrap(&file_text, 44).iter().take(4).enumerate() {
        let id = ["result2", "result2B", "result2C", "result2D"][i];
        f.label_on(1, id, [134.0, 280.0 + 22.0 * i as f32, 350.0, 20.0], line);
    }
    f.button(
        "ok",
        [510.0, 416.0, 111.0, 30.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [637.0, 416.0, 111.0, 30.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

/// `text` in lines of at most `n` characters, broken at spaces.
fn wrap(text: &str, n: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in text.split_whitespace() {
        match out.last_mut() {
            Some(l) if l.chars().count() + 1 + w.chars().count() <= n => {
                l.push(' ');
                l.push_str(w);
            }
            _ => out.push(w.to_owned()),
        }
    }
    out
}

/// Public `Sub` procedures without parameters of a module's source: the macros a user can
/// pick ([MS-VBAL] §5.3.1.1 subroutine declarations; `Private` ones are not listed).
pub(crate) fn macro_names(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let lower = l.to_ascii_lowercase();
            let rest = if let Some(r) = lower.strip_prefix("public sub ") {
                &l[l.len() - r.len()..]
            } else {
                let r = lower.strip_prefix("sub ")?;
                &l[l.len() - r.len()..]
            };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let after = rest[name.len()..].trim_start();
            (!name.is_empty() && after.starts_with("()")).then_some(name)
        })
        .collect()
}

/// Macros (684 × 479 measured): Macro name, the list, Macros in, Description. BlueOffice
/// never runs VBA: Run / Step Into stay dimmed; Edit shows the module's source read only.
fn open_macros(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::Macros,
        &tr("doc17.macTitle", "Macros"),
        (682.0, 477.0),
        vec![],
    );
    let mut macros = Vec::new();
    if let Some(mods) = c.extra["r17"]["vba"]["modules"].as_array() {
        for m in mods {
            let name = m["name"].as_str().unwrap_or_default().to_owned();
            let src = m["source"].as_str().unwrap_or_default().to_owned();
            if m["kind"].as_str() == Some("standard") {
                for n in macro_names(&src) {
                    macros.push((n, name.clone()));
                }
            }
        }
    }
    f.label(
        "nameL",
        [18.0, 47.0, 300.0, 18.0],
        &tr("doc17.macName", "Macro name:"),
    );
    f.edit("name", [18.0, 68.0, 520.0, 26.0], "");
    f.list(
        "macros",
        [18.0, 94.0, 520.0, 206.0],
        macros.iter().map(|m| m.0.clone()).collect(),
        None,
    );
    let buttons: [(&'static str, &str, &str, f32); 6] = [
        ("run", "doc17.macRun", "Run", 68.0),
        ("stepInto", "doc17.macStepInto", "Step Into", 107.0),
        ("edit", "doc17.macEdit", "Edit", 146.0),
        ("create", "doc17.macCreate", "Create", 185.0),
        ("delete", "doc17.macDelete", "Delete", 224.0),
        ("organizer", "doc17.macOrganizer", "Organizer...", 263.0),
    ];
    for (id, k, en, y) in buttons {
        f.button(id, [553.0, y, 111.0, 28.0], &tr(k, en), false);
        // BlueOffice never runs, writes or deletes VBA (CORE-VBA-002): view only.
        if id != "edit" {
            f.disable(id);
        }
    }
    f.label(
        "inL",
        [18.0, 313.0, 80.0, 18.0],
        &tr("doc17.macIn", "Macros in:"),
    );
    let doc = c.extra["r17"]["docName"].as_str().unwrap_or("Document");
    f.combo(
        "in",
        [93.0, 307.0, 445.0, 28.0],
        vec![
            tr("doc17.macAll", "All active templates and documents"),
            format!("{doc} ({})", tr("doc17.macDocument", "document")),
        ],
        0,
    );
    f.label(
        "descL",
        [18.0, 340.0, 200.0, 18.0],
        &tr("doc17.macDescription", "Description:"),
    );
    // A white framed box, as the reference's description field.
    f.canvas("descBox", [18.0, 360.0, 646.0, 55.0]);
    f.label("desc", [26.0, 368.0, 630.0, 40.0], "");
    f.button(
        "cancel",
        [553.0, 432.0, 111.0, 28.0],
        &tr("common.cancel", "Cancel"),
        true,
    );
    f.r17 = Some(Box::new(R17 {
        macros,
        ..R17::default()
    }));
    f.mac_enable();
    f.form.focus("name");
    f
}

/// Macros › Edit: the module's source, read only (a list of its lines).
fn open_source(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let module = c.extra["r17"]["module"].as_str().unwrap_or_default();
    let src = c.extra["r17"]["vba"]["modules"]
        .as_array()
        .and_then(|a| a.iter().find(|m| m["name"].as_str() == Some(module)))
        .and_then(|m| m["source"].as_str())
        .unwrap_or_default();
    let mut f = DocForm::new(
        DocFormKind::MacroSource,
        &format!("{module} ({})", tr("doc17.macViewOnly", "view only")),
        (760.0, 560.0),
        vec![],
    );
    f.list(
        "lines",
        [12.0, 44.0, 736.0, 460.0],
        src.lines().map(|l| l.replace('\t', "    ")).collect(),
        None,
    );
    f.button(
        "cancel",
        [637.0, 516.0, 111.0, 30.0],
        &tr("docR3.close", "Close"),
        true,
    );
    f
}

/// The video address typed: an http(s) URL (embed code `<iframe src="…">` gives its src).
pub(crate) fn video_url(s: &str) -> Option<String> {
    let s = s.trim();
    let s = match s.find("src=\"") {
        Some(i) => s[i + 5..].split('"').next().unwrap_or_default(),
        None => s,
    };
    let lower = s.to_ascii_lowercase();
    ((lower.starts_with("https://") || lower.starts_with("http://"))
        && s.len() > 10
        && !s.contains(char::is_whitespace))
    .then(|| s.to_owned())
}

/// Insert a video: the address (or embed code) of an online video; Insert puts a video
/// frame linked to it at the selection.
fn open_video(s: &Strings) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::OnlineVideo,
        &tr("doc17.videoTitle", "Insert a video"),
        (560.0, 230.0),
        vec![],
    );
    f.label(
        "urlL",
        [20.0, 52.0, 520.0, 20.0],
        &tr(
            "doc17.videoPrompt",
            "Enter the address of the online video or its embed code:",
        ),
    );
    f.edit("url", [20.0, 78.0, 520.0, 28.0], "");
    f.label(
        "note",
        [20.0, 116.0, 520.0, 40.0],
        &tr(
            "doc17.videoNote",
            "The video plays from its website: BlueDoc keeps a link to it in the document.",
        ),
    );
    f.button(
        "ok",
        [302.0, 180.0, 111.0, 30.0],
        &tr("doc17.videoInsert", "Insert"),
        true,
    );
    f.button(
        "cancel",
        [429.0, 180.0, 111.0, 30.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.disable("ok");
    f.form.focus("url");
    f
}

/// Customize Keyboard (Format ▾ › Shortcut key… of New / Modify Style): the style's
/// current keys, a box that takes the next Ctrl / Alt combination pressed, Assign / Remove;
/// the keys are BlueDoc's own (kept per user by the host, like the reference's template).
fn open_keyboard(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let style = c.extra["r17"]["styleName"]
        .as_str()
        .unwrap_or("Normal")
        .to_owned();
    let mut f = DocForm::new(
        DocFormKind::CustomizeKeyboard,
        &tr("doc17.kbTitle", "Customize Keyboard"),
        (560.0, 400.0),
        vec![],
    );
    f.label(
        "cmdL",
        [18.0, 47.0, 250.0, 18.0],
        &tr("doc17.kbCommands", "Commands:"),
    );
    f.list(
        "commands",
        [18.0, 68.0, 250.0, 120.0],
        vec![style.clone()],
        Some(0),
    );
    f.label(
        "curL",
        [18.0, 200.0, 250.0, 18.0],
        &tr("doc17.kbCurrent", "Current keys:"),
    );
    let current: Vec<String> = c.extra["r17"]["keys"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|k| k[1].as_str() == Some(style.as_str()))
                .filter_map(|k| k[0].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    f.list(
        "current",
        [18.0, 221.0, 250.0, 100.0],
        current.clone(),
        None,
    );
    f.label(
        "newL",
        [290.0, 200.0, 250.0, 18.0],
        &tr("doc17.kbPress", "Press new shortcut key:"),
    );
    f.edit("newKey", [290.0, 221.0, 252.0, 26.0], "");
    f.label(
        "inL",
        [290.0, 260.0, 250.0, 18.0],
        &tr("doc17.kbSaveIn", "Save changes in:"),
    );
    f.label(
        "in",
        [290.0, 280.0, 250.0, 18.0],
        &tr("doc17.kbBlueDoc", "BlueDoc (this computer)"),
    );
    f.label("assignedL", [290.0, 304.0, 252.0, 18.0], "");
    f.button(
        "assign",
        [176.0, 350.0, 111.0, 30.0],
        &tr("doc17.kbAssign", "Assign"),
        true,
    );
    f.button(
        "remove",
        [303.0, 350.0, 111.0, 30.0],
        &tr("doc17.kbRemove", "Remove"),
        false,
    );
    f.button(
        "cancel",
        [431.0, 350.0, 111.0, 30.0],
        &tr("docR3.close", "Close"),
        false,
    );
    f.ids = current;
    f.r17 = Some(Box::new(R17 {
        count_label: style,
        ..R17::default()
    }));
    f.form.focus("newKey");
    f
}

impl DocForm {
    /// Customize Keyboard takes the next Ctrl / Alt combination into its box.
    pub(super) fn r17_key(&mut self, k: crate::Key, m: crate::Mods) -> bool {
        if self.kind != DocFormKind::CustomizeKeyboard {
            return false;
        }
        let Some(name) = key_name(k, m) else {
            return false;
        };
        self.set_text("newKey", &name);
        true
    }

    fn kb_assign(&mut self) -> FormOut {
        let key = self.text("newKey");
        if key.trim().is_empty() {
            return FormOut::Redraw;
        }
        let style = self
            .r17
            .as_ref()
            .map(|r| r.count_label.clone())
            .unwrap_or_default();
        if !self.ids.contains(&key) {
            self.ids.push(key.clone());
        }
        let keys = self.ids.clone();
        self.form.set_items("current", keys, None);
        self.set_text("newKey", "");
        FormOut::Run(
            vec![json!({ "cmd": "ui.action", "id": format!("r17.key.assign:{key}\u{1f}{style}") })],
            false,
        )
    }
}

/// Frame (Format ▾ › Frame… of New / Modify Style; §17.3.1.11 framePr): text wrapping,
/// size, horizontal and vertical position with their anchors and distances, Move with
/// text, Lock anchor, Remove Frame.
fn open_style_frame(s: &Strings) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let names = |rows: &[(&str, &str)]| -> Vec<String> {
        rows.iter()
            .map(|(_, en)| tr(&format!("doc17.fr_{en}"), en))
            .collect()
    };
    let mut f = DocForm::new(
        DocFormKind::StyleFrame,
        &tr("doc17.frTitle", "Frame"),
        (420.0, 470.0),
        vec![],
    );
    f.label(
        "wrapL",
        [18.0, 46.0, 200.0, 18.0],
        &tr("doc17.frWrap", "Text wrapping"),
    );
    f.radio(
        "wrapNone",
        "wrap",
        [30.0, 70.0, 120.0, 22.0],
        &tr("doc17.frNone", "None"),
        false,
    );
    f.radio(
        "wrapAround",
        "wrap",
        [170.0, 70.0, 120.0, 22.0],
        &tr("doc17.frAround", "Around"),
        true,
    );
    f.label(
        "sizeL",
        [18.0, 104.0, 200.0, 18.0],
        &tr("doc17.frSize", "Size"),
    );
    f.label(
        "wL",
        [30.0, 128.0, 60.0, 18.0],
        &tr("doc17.frWidth", "Width:"),
    );
    f.combo(
        "wRule",
        [90.0, 124.0, 130.0, 26.0],
        vec![tr("doc17.frAuto", "Auto"), tr("doc17.frExactly", "Exactly")],
        0,
    );
    f.label("wAtL", [232.0, 128.0, 30.0, 18.0], &tr("doc17.frAt", "At:"));
    f.edit("wAt", [266.0, 124.0, 130.0, 26.0], "");
    f.label(
        "hL",
        [30.0, 160.0, 60.0, 18.0],
        &tr("doc17.frHeight", "Height:"),
    );
    f.combo(
        "hRule",
        [90.0, 156.0, 130.0, 26.0],
        vec![
            tr("doc17.frAuto", "Auto"),
            tr("doc17.frAtLeast", "At least"),
            tr("doc17.frExactly", "Exactly"),
        ],
        0,
    );
    f.label("hAtL", [232.0, 160.0, 30.0, 18.0], &tr("doc17.frAt", "At:"));
    f.edit("hAt", [266.0, 156.0, 130.0, 26.0], "");
    f.label(
        "horL",
        [18.0, 194.0, 200.0, 18.0],
        &tr("doc17.frHorizontal", "Horizontal"),
    );
    f.label(
        "hPosL",
        [30.0, 218.0, 100.0, 18.0],
        &tr("doc17.frPosition", "Position:"),
    );
    f.combo("hPos", [30.0, 238.0, 170.0, 26.0], names(&H_ALIGN), 0);
    f.label(
        "hRelL",
        [214.0, 218.0, 120.0, 18.0],
        &tr("doc17.frRelative", "Relative to:"),
    );
    f.combo("hRel", [214.0, 238.0, 182.0, 26.0], names(&H_ANCHOR), 0);
    f.label(
        "hDistL",
        [214.0, 270.0, 160.0, 18.0],
        &tr("doc17.frDistance", "Distance from text:"),
    );
    f.edit("hDist", [214.0, 290.0, 182.0, 26.0], "0.13\"");
    f.label(
        "verL",
        [18.0, 326.0, 200.0, 18.0],
        &tr("doc17.frVertical", "Vertical"),
    );
    f.label(
        "vPosL",
        [30.0, 350.0, 100.0, 18.0],
        &tr("doc17.frPosition", "Position:"),
    );
    f.edit("vPos", [30.0, 370.0, 170.0, 26.0], "0\"");
    f.label(
        "vRelL",
        [214.0, 350.0, 120.0, 18.0],
        &tr("doc17.frRelative", "Relative to:"),
    );
    f.combo("vRel", [214.0, 370.0, 182.0, 26.0], names(&V_ANCHOR), 0);
    f.check(
        "move",
        [30.0, 404.0, 160.0, 22.0],
        &tr("doc17.frMove", "Move with text"),
        true,
    );
    f.check(
        "lock",
        [214.0, 404.0, 160.0, 22.0],
        &tr("doc17.frLock", "Lock anchor"),
        false,
    );
    f.button(
        "removeFrame",
        [18.0, 432.0, 120.0, 28.0],
        &tr("doc17.frRemove", "Remove Frame"),
        false,
    );
    f.button(
        "ok",
        [168.0, 432.0, 111.0, 28.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [291.0, 432.0, 111.0, 28.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

/// Horizontal positions (`w:xAlign`, ST_XAlign §17.18.108) and anchors (`w:hAnchor`).
const H_ALIGN: [(&str, &str); 5] = [
    ("left", "Left"),
    ("right", "Right"),
    ("center", "Center"),
    ("inside", "Inside"),
    ("outside", "Outside"),
];
const H_ANCHOR: [(&str, &str); 3] = [("text", "Column"), ("margin", "Margin"), ("page", "Page")];
/// Vertical anchors (`w:vAnchor`, ST_VAnchor §17.18.100).
const V_ANCHOR: [(&str, &str); 3] = [
    ("text", "Paragraph"),
    ("margin", "Margin"),
    ("page", "Page"),
];

/// A length typed as inches (`0.5"`, `0.5in` or a plain number), centimetres (`cm`) or
/// points (`pt`), in twips.
fn twips_of(s: &str) -> Option<i32> {
    let t = s.trim().replace(',', ".");
    let num: String = t
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    let v: f32 = num.parse().ok()?;
    let k = match t[num.len()..].trim() {
        "cm" => 1440.0 / 2.54,
        "pt" => 20.0,
        _ => 1440.0,
    };
    Some((v * k).round() as i32)
}

impl DocForm {
    /// The `w:framePr` element of the Frame dialog.
    fn frame_xml(&self) -> String {
        let mut a = vec![];
        if self.sel("wRule") == Some(1) {
            if let Some(w) = twips_of(&self.text("wAt")) {
                a.push(format!("w:w=\"{}\"", w.max(0)));
            }
        }
        let h_rule = self.sel("hRule").unwrap_or(0);
        if h_rule > 0 {
            if let Some(h) = twips_of(&self.text("hAt")) {
                a.push(format!("w:h=\"{}\"", h.max(0)));
                let rule = if h_rule == 1 { "atLeast" } else { "exact" };
                a.push(format!("w:hRule=\"{rule}\""));
            }
        }
        if let Some(d) = twips_of(&self.text("hDist")) {
            a.push(format!("w:hSpace=\"{}\"", d.max(0)));
        }
        let wrap = if self.chosen("wrap") == Some("wrapNone") {
            "none"
        } else {
            "around"
        };
        a.push(format!("w:wrap=\"{wrap}\""));
        a.push(format!(
            "w:vAnchor=\"{}\"",
            V_ANCHOR[self.sel("vRel").unwrap_or(0).min(2)].0
        ));
        a.push(format!(
            "w:hAnchor=\"{}\"",
            H_ANCHOR[self.sel("hRel").unwrap_or(0).min(2)].0
        ));
        a.push(format!(
            "w:xAlign=\"{}\"",
            H_ALIGN[self.sel("hPos").unwrap_or(0).min(4)].0
        ));
        if let Some(y) = twips_of(&self.text("vPos")).filter(|y| *y != 0) {
            a.push(format!("w:y=\"{y}\""));
        }
        if self.on("lock") {
            a.push("w:anchorLock=\"1\"".to_owned());
        }
        format!("<w:framePr {}/>", a.join(" "))
    }
}

/// Format ▾ › Text Effects… of New / Modify Style: outline, shadow, glow and reflection of
/// the style's text (the effects of Home › Text Effects).
fn open_style_effects(s: &Strings) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::StyleEffects,
        &tr("doc17.fxTitle", "Format Text Effects"),
        (360.0, 260.0),
        vec![],
    );
    for (i, (id, k, en)) in [
        ("fxOutline", "doc17.fxOutline", "Text Outline"),
        ("fxShadow", "doc17.fxShadow", "Shadow"),
        ("fxGlow", "doc17.fxGlow", "Glow"),
        ("fxReflection", "doc17.fxReflection", "Reflection"),
    ]
    .into_iter()
    .enumerate()
    {
        f.check(
            id,
            [20.0, 52.0 + 34.0 * i as f32, 300.0, 24.0],
            &tr(k, en),
            false,
        );
    }
    f.button(
        "ok",
        [104.0, 214.0, 111.0, 30.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [231.0, 214.0, 111.0, 30.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

/// Format ▾ › Numbering… of New / Modify Style: the numbering the style's paragraphs get.
fn open_style_numbering(s: &Strings) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::StyleNumbering,
        &tr("doc17.snTitle", "Numbering and Bullets"),
        (380.0, 420.0),
        vec![],
    );
    f.label(
        "fmtL",
        [18.0, 47.0, 300.0, 18.0],
        &tr("doc17.snFormat", "Number or bullet format:"),
    );
    f.list(
        "formats",
        [18.0, 68.0, 344.0, 290.0],
        STYLE_NUMBERS
            .iter()
            .map(|(l, ..)| {
                if l.starts_with('N') {
                    tr("doc17.snNone", "None")
                } else {
                    (*l).to_owned()
                }
            })
            .collect(),
        Some(1),
    );
    f.button(
        "ok",
        [124.0, 374.0, 111.0, 30.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [251.0, 374.0, 111.0, 30.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

// ------------------------------------------------------------------ Insert Icons

/// Grid of the icon dialog (reference pixels): canvas, cell size, icon size.
const GRID: [f32; 4] = [20.0, 132.0, 840.0, 420.0];
const CELL: f32 = 105.0;
const CELL_H: f32 = 96.0;
const ICON: f32 = 52.0;

/// The icon library with the categories and keywords of its manifest.
fn icon_entries() -> (Vec<IconEntry>, Vec<(String, String, String)>) {
    const DATA: &str = include_str!("../../../packages/ui/src/iconLibrary/data.json");
    let v: Value = serde_json::from_str(DATA).unwrap_or(Value::Null);
    let meta: Vec<&Value> = v["manifest"]["icons"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let words_of = |m: &Value| -> String {
        let mut w = vec![
            m["id"].as_str().unwrap_or_default().to_owned(),
            m["name"]["en"].as_str().unwrap_or_default().to_owned(),
            m["name"]["vi"].as_str().unwrap_or_default().to_owned(),
        ];
        for lang in ["en", "vi"] {
            if let Some(a) = m["keywords"][lang].as_array() {
                w.extend(a.iter().filter_map(|k| k.as_str().map(str::to_owned)));
            }
        }
        w.join(" ").to_lowercase()
    };
    let icons = crate::slide_pickers::load_library()
        .into_iter()
        .map(|icon| {
            let m = meta.iter().find(|m| m["id"].as_str() == Some(&icon.id));
            IconEntry {
                category: m
                    .and_then(|m| m["category"].as_str())
                    .unwrap_or_default()
                    .to_owned(),
                words: m.map(|m| words_of(m)).unwrap_or_default(),
                icon,
            }
        })
        .collect();
    let cats = v["manifest"]["categories"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|c| {
                    (
                        c["id"].as_str().unwrap_or_default().to_owned(),
                        c["name"]["en"].as_str().unwrap_or_default().to_owned(),
                        c["name"]["vi"].as_str().unwrap_or_default().to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    (icons, cats)
}

/// Insert Icons (880 × 626): search box, category, a grid of BlueOffice's own icons (a
/// press chooses or un-chooses one), Insert (n) / Cancel.
fn open_icons(s: &Strings) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let vi = s.get("doc14.lang") == "vi";
    let mut f = DocForm::new(
        DocFormKind::Icons,
        &tr("doc17.icTitle", "Insert Icons"),
        (880.0, 626.0),
        vec![],
    );
    let (icons, cats) = icon_entries();
    f.edit("search", [20.0, 52.0, 520.0, 30.0], "");
    f.form
        .set_placeholder("search", &tr("doc17.icSearch", "Search icons"));
    let mut names = vec![tr("doc17.icAll", "All")];
    names.extend(cats.iter().map(|c| {
        if vi && !c.2.is_empty() {
            c.2.clone()
        } else {
            c.1.clone()
        }
    }));
    f.label(
        "catL",
        [560.0, 58.0, 90.0, 20.0],
        &tr("doc17.icCategory", "Category:"),
    );
    f.combo("category", [650.0, 52.0, 210.0, 30.0], names, 0);
    f.label(
        "hint",
        [20.0, 100.0, 840.0, 20.0],
        &tr(
            "doc17.icHint",
            "BlueOffice icons: click one or more, then Insert. They are pictures you can recolour.",
        ),
    );
    f.canvas("iconGrid", GRID);
    f.label("count", [20.0, 582.0, 300.0, 20.0], "");
    f.button(
        "ok",
        [622.0, 576.0, 111.0, 30.0],
        &tr("doc17.icInsert", "Insert"),
        true,
    );
    f.button(
        "cancel",
        [749.0, 576.0, 111.0, 30.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.r17 = Some(Box::new(R17 {
        shown: (0..icons.len()).collect(),
        icons,
        categories: cats.into_iter().map(|c| c.0).collect(),
        count_label: tr("doc17.icSelected", "{n} selected"),
        ..R17::default()
    }));
    f.icons_count();
    f.form.focus("search");
    f
}

impl DocForm {
    /// The icons that match the search words and the category.
    fn icons_filter(&mut self) {
        let q = self.text("search").to_lowercase();
        let cat = self.sel("category").unwrap_or(0);
        let Some(st) = self.r17.as_mut() else {
            return;
        };
        let want = cat
            .checked_sub(1)
            .and_then(|i| st.categories.get(i))
            .cloned();
        st.shown = st
            .icons
            .iter()
            .enumerate()
            .filter(|(_, e)| want.as_ref().is_none_or(|c| *c == e.category))
            .filter(|(_, e)| q.split_whitespace().all(|w| e.words.contains(w)))
            .map(|(i, _)| i)
            .collect();
        st.scroll = 0.0;
    }

    fn icons_count(&mut self) {
        let Some(st) = self.r17.as_ref() else {
            return;
        };
        let n = st.chosen.len();
        let text = if n == 0 {
            String::new()
        } else {
            st.count_label.replace("{n}", &n.to_string())
        };
        self.set_label("count", &text);
        self.form.enable("ok", n > 0);
    }

    /// A press on the icon grid at (x, y) from its top-left (reference pixels).
    pub(super) fn r17_canvas(&mut self, id: &str, x: f32, y: f32) -> bool {
        if id != "iconGrid" || self.kind != DocFormKind::Icons {
            return false;
        }
        let Some(st) = self.r17.as_mut() else {
            return false;
        };
        let cols = (GRID[2] / CELL).floor() as usize;
        let (col, row) = ((x / CELL) as usize, ((y + st.scroll) / CELL_H) as usize);
        if col < cols {
            if let Some(&i) = st.shown.get(row * cols + col) {
                if let Some(at) = st.chosen.iter().position(|c| *c == i) {
                    st.chosen.remove(at);
                } else {
                    st.chosen.push(i);
                }
            }
        }
        self.icons_count();
        true
    }

    /// The wheel over the icon grid scrolls it.
    pub(super) fn r17_wheel(&mut self, d: f32) -> bool {
        if self.kind != DocFormKind::Icons {
            return false;
        }
        let Some(st) = self.r17.as_mut() else {
            return false;
        };
        let cols = (GRID[2] / CELL).floor() as usize;
        let rows = st.shown.len().div_ceil(cols.max(1));
        let max = (rows as f32 * CELL_H - GRID[3]).max(0.0);
        st.scroll = (st.scroll - d * CELL_H).clamp(0.0, max);
        true
    }
}

/// Paint the icon grid: white panel, each icon centred in its cell with its name below; a
/// chosen icon has a blue frame and a check mark.
pub(super) fn paint_canvas(
    f: &DocForm,
    id: &str,
    r: [f32; 4],
    p: &mut dyn crate::Painter,
    font: &crate::Font<'_>,
) -> bool {
    use bo_render::Rgba;
    if id == "descBox" && f.kind == DocFormKind::Macros {
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xab, 0xad, 0xb3, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        return true;
    }
    if id != "iconGrid" || f.kind != DocFormKind::Icons {
        return false;
    }
    let Some(st) = f.r17.as_ref() else {
        return true;
    };
    let k = r[2] / GRID[2];
    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xab, 0xad, 0xb3, 255));
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(255, 255, 255, 255),
    );
    p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
    let cols = (GRID[2] / CELL).floor() as usize;
    let blue = Rgba(0x00, 0x5f, 0xb8, 255);
    let small = crate::Font {
        size: font.size * 0.85,
        ..*font
    };
    for (n, &i) in st.shown.iter().enumerate() {
        let (col, row) = (n % cols, n / cols);
        let y = row as f32 * CELL_H - st.scroll;
        if y + CELL_H < 0.0 || y > GRID[3] {
            continue;
        }
        let Some(e) = st.icons.get(i) else { continue };
        let (cx, cy) = (r[0] + col as f32 * CELL * k, r[1] + y * k);
        let on = st.chosen.contains(&i);
        if on {
            p.fill_rect(
                cx + 4.0 * k,
                cy + 4.0 * k,
                (CELL - 8.0) * k,
                (CELL_H - 8.0) * k,
                blue,
            );
            p.fill_rect(
                cx + 6.0 * k,
                cy + 6.0 * k,
                (CELL - 12.0) * k,
                (CELL_H - 12.0) * k,
                Rgba(0xe5, 0xf1, 0xfb, 255),
            );
        }
        crate::slide_pickers::paint_icon(
            p,
            &e.icon,
            (cx + (CELL - ICON) / 2.0 * k).round(),
            (cy + 12.0 * k).round(),
            ICON * k,
        );
        let label: String = e.icon.id.replace('-', " ").chars().take(16).collect();
        let tw = label.chars().count() as f32 * small.size * 0.5;
        p.text_font(
            &label,
            cx + ((CELL * k) - tw) / 2.0,
            cy + 70.0 * k,
            &small,
            Rgba(0x40, 0x40, 0x40, 255),
        );
        if on {
            // A check mark in the top-right corner.
            p.fill_rect(
                cx + (CELL - 26.0) * k,
                cy + 8.0 * k,
                16.0 * k,
                16.0 * k,
                blue,
            );
            p.path(
                &[
                    bo_render::PathSeg::M(cx + (CELL - 23.0) * k, cy + 16.0 * k),
                    bo_render::PathSeg::L(cx + (CELL - 19.5) * k, cy + 19.5 * k),
                    bo_render::PathSeg::L(cx + (CELL - 13.0) * k, cy + 12.0 * k),
                ],
                None,
                Some((Rgba(255, 255, 255, 255), 1.6 * k)),
            );
        }
    }
    p.pop_clip();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_names_lists_public_subs_without_parameters() {
        let src = "Option Explicit\nSub Hello()\n  MsgBox 1\nEnd Sub\nPrivate Sub Hidden()\nEnd Sub\nPublic Sub Two ()\nEnd Sub\nSub WithArg(x)\nEnd Sub\nFunction F()\nEnd Function\n";
        assert_eq!(macro_names(src), vec!["Hello", "Two"]);
    }

    #[test]
    fn video_addresses_and_embed_codes() {
        assert_eq!(
            video_url(" https://example.org/v/1 ").as_deref(),
            Some("https://example.org/v/1")
        );
        assert_eq!(
            video_url(r#"<iframe width="560" src="https://example.org/embed/2" ></iframe>"#)
                .as_deref(),
            Some("https://example.org/embed/2")
        );
        assert!(video_url("not a link").is_none());
        assert!(video_url("ftp://x.org/a").is_none());
    }
}
