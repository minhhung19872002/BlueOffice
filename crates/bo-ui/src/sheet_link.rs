//! Insert › Link (Ctrl+K) as the reference's Insert Hyperlink dialog (round 49, measured at
//! 125 %: 964 × 432 px, `show/sheet-48`): the "Link to" bar (Existing File or Web Page,
//! Place in This Document, Create New Document greyed, E-mail Address; own icons), Text to
//! display, ScreenTip…, and each kind's page. The link is a real cell hyperlink (engine
//! `setHyperlink`, §18.3.1.47), Edit Hyperlink starts from the cell's link.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::Painter;
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

/// The "Link to" bar's entries (label key, fallback) and their pages.
const KINDS: [(&str, &str); 4] = [
    ("sheetLink.existing", "Existing File or Web Page"),
    ("sheetLink.place", "Place in This Document"),
    ("sheetLink.newDoc", "Create New Document"),
    ("sheetLink.email", "E-mail Address"),
];
/// The bar: left, top, width, height; each entry's height.
const BAR: [f32; 4] = [18.0, 74.0, 118.0, 338.0];
const ENTRY_H: f32 = 78.0;
/// The browser's tabs left of the list (label key, fallback) and their height.
const LTABS: [(&str, &str); 3] = [
    ("sheetLink.currentFolder", "Current Folder"),
    ("sheetLink.browsedPages", "Browsed Pages"),
    ("sheetLink.recentFiles", "Recent Files"),
];
const LTAB_H: f32 = 69.0;

impl SheetWindow {
    /// Insert › Link: the hyperlink of the active cell.
    pub(super) fn open_hyperlink(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let (r, c) = self.view.active();
        let sheet = self.view.sheet();
        let now = self
            .view
            .query(json!({ "cmd": "hyperlink", "sheet": sheet, "row": r, "col": c }))
            .unwrap_or(Value::Null);
        let s = |k: &str| now.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let shown = if now.is_null() {
            self.view
                .engine()
                .cell(sheet, r, c)
                .map(|d| d.display)
                .unwrap_or_default()
        } else {
            s("display")
        };
        let target = s("target");
        let (page, addr, mail, subject) = match target.strip_prefix("mailto:") {
            Some(m) => {
                let (to, q) = m.split_once('?').unwrap_or((m, ""));
                let subj = q.strip_prefix("subject=").unwrap_or("").replace("%20", " ");
                (3, String::new(), to.to_owned(), subj)
            }
            None if !s("location").is_empty() => (1, String::new(), String::new(), String::new()),
            None => (0, target, String::new(), String::new()),
        };
        let (loc_sheet, loc_cell) = match s("location").split_once('!') {
            Some((a, b)) => (a.trim_matches('\'').to_owned(), b.to_owned()),
            None => (
                String::new(),
                if s("location").is_empty() {
                    "A1".into()
                } else {
                    s("location")
                },
            ),
        };
        let names: Vec<String> = self
            .view
            .query(json!({ "cmd": "nameList" }))
            .and_then(|v| {
                v.get("names")
                    .or(Some(&v))
                    .and_then(Value::as_array)
                    .cloned()
            })
            .unwrap_or_default()
            .iter()
            .filter_map(|n| n.get("name").and_then(Value::as_str).map(str::to_owned))
            .collect();
        let t = |k: &str, f: &str| self.t(k, f);
        let title = if now.is_null() {
            t("sheetInsert.linkTitle", "Insert Hyperlink")
        } else {
            t("sheetLink.editTitle", "Edit Hyperlink")
        };
        let mut f = Form::new(&title, (964.0, 432.0), vec![]);
        let edit = |v: &str| Kind::Edit {
            value: v.chars().collect(),
            caret: v.chars().count(),
        };
        f.add(
            "linkL",
            None,
            [18.0, 50.0, 118.0, 22.0],
            Kind::Label(t("sheetLink.linkTo", "Link to:")),
        );
        f.add("bar", None, BAR, Kind::Canvas);
        f.add(
            "textL",
            None,
            [151.0, 50.0, 95.0, 22.0],
            Kind::Label(t("sheetInsert.linkText", "Text to display:")),
        );
        f.set_access("textL", 0);
        f.add("text", None, [244.0, 47.0, 573.0, 26.0], edit(&shown));
        f.add(
            "tip",
            None,
            [832.0, 46.0, 113.0, 28.0],
            Kind::Button {
                label: t("sheetLink.screenTip", "ScreenTip..."),
                default: false,
            },
        );
        f.set_access("tip", 6);
        f.add("rule", None, [151.0, 86.0, 666.0, 1.0], Kind::Canvas);
        // Page 0: web page or file. Look in (the workbook's folder), three small buttons,
        // the Current Folder / Browsed Pages / Recent Files tabs left of the list, Bookmark….
        let p = Some(0);
        f.add(
            "lookL",
            p,
            [151.0, 98.0, 90.0, 22.0],
            Kind::Label(t("sheetLink.lookIn", "Look in:")),
        );
        f.set_access("lookL", 1);
        let folder = self.link_dir_name();
        f.add(
            "look",
            p,
            [248.0, 93.0, 394.0, 27.0],
            Kind::Combo {
                items: vec![folder],
                sel: 0,
            },
        );
        f.add("upDir", p, [662.0, 93.0, 28.0, 28.0], Kind::Canvas);
        f.add("web", p, [717.0, 93.0, 28.0, 28.0], Kind::Canvas);
        f.add("file", p, [752.0, 93.0, 28.0, 28.0], Kind::Canvas);
        f.enable("web", false);
        f.enable("file", false);
        f.add("ltabs", p, [151.0, 124.0, 96.0, 207.0], Kind::Canvas);
        f.add(
            "used",
            p,
            [247.0, 124.0, 570.0, 207.0],
            Kind::List {
                items: Vec::new(),
                sel: None,
                top: 0,
            },
        );
        f.add(
            "bookmark",
            p,
            [832.0, 126.0, 113.0, 28.0],
            Kind::Button {
                label: t("sheetLink.bookmark", "Bookmark..."),
                default: false,
            },
        );
        f.set_access("bookmark", 1);
        // Page 2: a new workbook or document beside this one.
        let p = Some(2);
        f.add(
            "newL",
            p,
            [151.0, 104.0, 400.0, 22.0],
            Kind::Label(t("sheetLink.newName", "Name of new document:")),
        );
        f.set_access("newL", 0);
        f.add("newName", p, [151.0, 126.0, 666.0, 26.0], edit(""));
        f.add(
            "pathL",
            p,
            [151.0, 164.0, 400.0, 22.0],
            Kind::Label(t("sheetLink.fullPath", "Full path:")),
        );
        let full = self
            .link_dir
            .as_ref()
            .map_or_else(String::new, |d| format!("{}\\", d.display()));
        f.add("path", p, [151.0, 186.0, 666.0, 22.0], Kind::Label(full));
        f.add(
            "change",
            p,
            [832.0, 180.0, 113.0, 28.0],
            Kind::Button {
                label: t("sheetLink.change", "Change..."),
                default: false,
            },
        );
        f.enable("change", false);
        f.add(
            "whenG",
            p,
            [151.0, 226.0, 666.0, 22.0],
            Kind::Group(t("sheetLink.when", "When to edit:")),
        );
        f.add(
            "later",
            p,
            [165.0, 252.0, 400.0, 24.0],
            Kind::Check {
                label: t("sheetLink.editLater", "Edit the new document later"),
                on: true,
            },
        );
        f.add(
            "now",
            p,
            [165.0, 278.0, 400.0, 24.0],
            Kind::Check {
                label: t("sheetLink.editNow", "Edit the new document now"),
                on: false,
            },
        );
        f.enable("now", false);
        f.set_radios(&["later", "now"]);
        f.add(
            "addrL",
            p,
            [151.0, 341.0, 95.0, 22.0],
            Kind::Label(t("sheetInsert.linkAddress", "Address:")),
        );
        f.set_access("addrL", 2);
        f.add("addr", p, [248.0, 338.0, 569.0, 26.0], edit(&addr));
        // Page 1: a place in this workbook.
        let p = Some(1);
        f.add(
            "cellL",
            p,
            [151.0, 104.0, 400.0, 22.0],
            Kind::Label(t("sheetLink.cellRef", "Type the cell reference:")),
        );
        f.set_access("cellL", 0);
        f.add("cell", p, [151.0, 126.0, 400.0, 26.0], edit(&loc_cell));
        f.add(
            "placeL",
            p,
            [151.0, 164.0, 400.0, 22.0],
            Kind::Label(t(
                "sheetLink.selectPlace",
                "Or select a place in this document:",
            )),
        );
        f.set_access("placeL", 1);
        let mut places = self
            .view
            .sheet_names()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let pick = places.iter().position(|n| *n == loc_sheet).unwrap_or(sheet);
        let n_sheets = places.len();
        places.extend(names);
        f.add(
            "places",
            p,
            [151.0, 186.0, 666.0, 178.0],
            Kind::List {
                items: places,
                sel: Some(pick.min(n_sheets.saturating_sub(1))),
                top: 0,
            },
        );
        // Page 3: e-mail.
        let p = Some(3);
        f.add(
            "mailL",
            p,
            [151.0, 104.0, 400.0, 22.0],
            Kind::Label(t("sheetLink.mail", "E-mail address:")),
        );
        f.set_access("mailL", 0);
        f.add("mail", p, [151.0, 126.0, 666.0, 26.0], edit(&mail));
        f.add(
            "subjL",
            p,
            [151.0, 164.0, 400.0, 22.0],
            Kind::Label(t("sheetLink.subject", "Subject:")),
        );
        f.set_access("subjL", 1);
        f.add("subject", p, [151.0, 186.0, 666.0, 26.0], edit(&subject));
        f.add("rule2", None, [151.0, 374.0, 794.0, 1.0], Kind::Canvas);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [706.0, 384.0, 111.0, 30.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [832.0, 384.0, 113.0, 30.0];
        }
        f.set_page(page);
        self.form = Some(f);
        self.link_dir = self.folder.clone();
        self.link_list(0);
        let Some(mut f) = self.form.take() else {
            return false;
        };
        f.focus(match page {
            1 => "cell",
            3 => "mail",
            _ => "addr",
        });
        self.link_tip = s("tooltip");
        self.form = Some(f);
        self.form_kind = FormKind::Link;
        true
    }

    /// External addresses of the workbook's links (newest first, once each).
    fn used_link_targets(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if let Some(wb) = self.view.engine().workbook() {
            for ws in &wb.sheets {
                for h in ws.hyperlinks.iter().rev() {
                    if let Some(t) = h.target.as_ref().filter(|t| !t.starts_with("mailto:")) {
                        if !out.contains(t) {
                            out.push(t.clone());
                        }
                    }
                }
            }
        }
        out
    }

    /// Events of Insert Hyperlink and its ScreenTip box.
    pub(super) fn link_event(&mut self, ev: &Event) {
        if self.form_kind == FormKind::LinkTip {
            match ev {
                Event::Ok | Event::Clicked("ok")
                    if self
                        .form
                        .as_ref()
                        .is_some_and(|f| f.get("bmCell").is_some()) =>
                {
                    let place = self.link_bookmark_text();
                    self.link_back();
                    if let Some(f) = self.form.as_mut() {
                        let a = f.text("addr");
                        let base = a.split('#').next().unwrap_or("").to_owned();
                        f.set_text("addr", &format!("{base}#{place}"));
                    }
                }
                Event::Ok | Event::Clicked("ok") => {
                    self.link_tip = self
                        .form
                        .as_ref()
                        .map(|f| f.text("tipText"))
                        .unwrap_or_default();
                    self.link_back();
                }
                Event::Cancel | Event::Clicked("cancel") => self.link_back(),
                _ => {}
            }
            return;
        }
        let Some(f) = self.form.as_mut() else {
            return;
        };
        match ev {
            Event::Cancel | Event::Clicked("cancel") => self.form = None,
            Event::Canvas("bar", _, y) => {
                let i = ((y / ENTRY_H).floor().max(0.0) as usize).min(3);
                f.set_page(i);
                f.focus(match i {
                    1 => "cell",
                    2 => "newName",
                    3 => "mail",
                    _ => "addr",
                });
            }
            Event::Canvas("ltabs", _, y) => {
                let i = ((y / LTAB_H).floor().max(0.0) as usize).min(2);
                self.link_list(i);
            }
            Event::Canvas("upDir", ..) => {
                if let Some(up) = self
                    .link_dir
                    .as_ref()
                    .and_then(|d| d.parent())
                    .map(std::path::Path::to_path_buf)
                {
                    self.link_dir = Some(up);
                    self.link_refresh_folder();
                }
            }
            Event::Clicked("bookmark") => self.open_link_bookmark(),
            Event::Changed("later" | "now") => {
                f.set_on("later", true);
                f.set_on("now", false);
            }
            Event::Changed("used") | Event::Clicked("used") => {
                let double = matches!(ev, Event::Clicked(_));
                let at = f.sel("used").unwrap_or(0);
                let Some((name, dir)) = self.link_entries.get(at).cloned() else {
                    return;
                };
                if self.link_tab == 0 {
                    let path = self.link_dir.as_ref().map(|d| d.join(&name));
                    if dir && double {
                        self.link_dir = path;
                        self.link_refresh_folder();
                    } else if !dir {
                        if let Some(f) = self.form.as_mut() {
                            f.set_text("addr", &name);
                        }
                    }
                } else if let Some(f) = self.form.as_mut() {
                    f.set_text("addr", &name);
                }
            }
            Event::Clicked("tip") => self.open_link_tip(),
            Event::Ok | Event::Clicked("ok") => {
                let shown = f.text("text");
                let (target, location) = match f.tab {
                    1 => {
                        let place = match f.get("places").map(|c| &c.kind) {
                            Some(Kind::List { items, sel, .. }) => {
                                sel.and_then(|i| items.get(i)).cloned().unwrap_or_default()
                            }
                            _ => String::new(),
                        };
                        let sheets = self.view.sheet_names().len();
                        let at = f.sel("places").unwrap_or(0);
                        let loc = if at < sheets {
                            let q = if place.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                place
                            } else {
                                format!("'{}'", place.replace('\'', "''"))
                            };
                            format!("{q}!{}", f.text("cell").trim())
                        } else {
                            place
                        };
                        (None, Some(loc))
                    }
                    3 => {
                        let mail = f.text("mail");
                        let mail = mail.trim().trim_start_matches("mailto:");
                        if mail.is_empty() {
                            (None, None)
                        } else {
                            let subj = f.text("subject");
                            let mut t = format!("mailto:{mail}");
                            if !subj.trim().is_empty() {
                                t.push_str(&format!(
                                    "?subject={}",
                                    subj.trim().replace(' ', "%20")
                                ));
                            }
                            (Some(t), None)
                        }
                    }
                    2 => {
                        let name = f.text("newName").trim().to_owned();
                        match self.link_create_document(&name) {
                            Some(path) => (Some(path), None),
                            None => return,
                        }
                    }
                    _ => {
                        let a = f.text("addr").trim().to_owned();
                        // A bare host gets the web scheme, as the reference adds it.
                        let a = if a.starts_with("www.") {
                            format!("http://{a}")
                        } else {
                            a
                        };
                        // "file.xlsx#Sheet1!A1": a place in another workbook (Bookmark…).
                        match a.split_once('#') {
                            Some((t, l)) if !t.is_empty() => {
                                (Some(t.to_owned()), Some(l.to_owned()))
                            }
                            Some((_, l)) => (None, Some(l.to_owned())),
                            None => ((!a.is_empty()).then_some(a), None),
                        }
                    }
                };
                self.form = None;
                if target.is_none() && location.is_none() {
                    return;
                }
                let (r, c) = self.view.active();
                let sheet = self.view.sheet();
                let display = (!shown.trim().is_empty()).then_some(shown);
                self.view.run(json!({
                    "cmd": "setHyperlink", "sheet": sheet, "row": r, "col": c,
                    "target": target, "location": location, "display": display,
                    "tooltip": self.link_tip,
                }));
            }
            _ => {}
        }
    }

    fn open_link_tip(&mut self) {
        let t = |k: &str, f: &str| self.t(k, f);
        let mut f = Form::new(
            &t("sheetLink.tipTitle", "Set Hyperlink ScreenTip"),
            (420.0, 190.0),
            vec![],
        );
        f.add(
            "tipL",
            None,
            [18.0, 48.0, 380.0, 22.0],
            Kind::Label(t("sheetLink.tipText", "ScreenTip text:")),
        );
        f.set_access("tipL", 0);
        let tip = self.link_tip.clone();
        f.add(
            "tipText",
            None,
            [18.0, 72.0, 384.0, 26.0],
            Kind::Edit {
                value: tip.chars().collect(),
                caret: tip.chars().count(),
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [176.0, 140.0, 107.0, 30.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [295.0, 140.0, 107.0, 30.0];
        }
        f.focus("tipText");
        if let Some(old) = self.form.take() {
            self.form_stash = Some((old, self.form_kind));
        }
        self.form = Some(f);
        self.form_kind = FormKind::LinkTip;
    }

    /// The workbook's folder name for Look in.
    fn link_dir_name(&self) -> String {
        self.link_dir
            .as_ref()
            .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default()
    }

    /// Show browser tab `tab`: 0 Current Folder (the folder's entries, folders first),
    /// 1 Browsed Pages (web addresses of this workbook's links), 2 Recent Files (files
    /// this workbook links to).
    fn link_list(&mut self, tab: usize) {
        self.link_tab = tab;
        if tab == 0 {
            if self.link_dir.is_none() {
                self.link_dir = self.folder.clone();
            }
            self.link_refresh_folder();
            return;
        }
        let used = self.used_link_targets();
        let web = |t: &String| t.contains("://");
        self.link_entries = used
            .into_iter()
            .filter(|t| if tab == 1 { web(t) } else { !web(t) })
            .map(|t| (t, false))
            .collect();
        self.link_show_entries();
    }

    fn link_refresh_folder(&mut self) {
        let mut v: Vec<(String, bool)> = self
            .link_dir
            .as_ref()
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|rd| {
                rd.filter_map(Result::ok)
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().into_owned();
                        (!name.starts_with('.') && !name.starts_with("~$"))
                            .then(|| (name, e.file_type().is_ok_and(|t| t.is_dir())))
                    })
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
        });
        self.link_entries = v;
        let name = self.link_dir_name();
        if let Some(f) = self.form.as_mut() {
            f.set_items("look", vec![name], Some(0));
        }
        self.link_show_entries();
    }

    fn link_show_entries(&mut self) {
        let items: Vec<String> = self
            .link_entries
            .iter()
            .map(|(n, d)| {
                if *d {
                    format!("\u{25A0} {n}")
                } else {
                    format!("    {n}")
                }
            })
            .collect();
        if let Some(f) = self.form.as_mut() {
            f.set_items("used", items, None);
        }
    }

    /// Create New Document: an empty workbook (.xlsx, the default) or document (.docx) in
    /// the workbook's folder; its path, or `None` (status says why).
    fn link_create_document(&mut self, name: &str) -> Option<String> {
        if name.is_empty() {
            return None;
        }
        let dir = self.link_dir.clone().or_else(|| self.folder.clone())?;
        let mut file = name.to_owned();
        let lower = file.to_lowercase();
        if !lower.ends_with(".xlsx") && !lower.ends_with(".docx") {
            file.push_str(".xlsx");
        }
        let path = dir.join(&file);
        if path.exists() {
            self.status = self.t("sheetLink.exists", "A file with this name already exists.");
            return None;
        }
        let bytes = if file.to_lowercase().ends_with(".docx") {
            let mut e = bo_engine::Engine::new();
            e.new_document(&file).ok()?;
            e.save_docx().ok()?
        } else {
            let mut e = bo_engine::SheetEngine::new();
            e.new_workbook(&file).ok()?;
            e.save("xlsx", 0).ok()?
        };
        if std::fs::write(&path, bytes).is_err() {
            self.status = self.t("sheetLink.cannotCreate", "The document cannot be created.");
            return None;
        }
        Some(path.display().to_string())
    }

    /// Bookmark…: Select Place in Document (cell reference, sheets and names).
    fn open_link_bookmark(&mut self) {
        let t = |k: &str, f: &str| self.t(k, f);
        let mut f = Form::new(
            &t("sheetLink.bmTitle", "Select Place in Document"),
            (420.0, 360.0),
            vec![],
        );
        f.add(
            "bmCellL",
            None,
            [18.0, 48.0, 380.0, 22.0],
            Kind::Label(t("sheetLink.cellRef", "Type the cell reference:")),
        );
        f.add(
            "bmCell",
            None,
            [18.0, 72.0, 384.0, 26.0],
            Kind::Edit {
                value: "A1".chars().collect(),
                caret: 2,
            },
        );
        f.add(
            "bmL",
            None,
            [18.0, 108.0, 380.0, 22.0],
            Kind::Label(t(
                "sheetLink.selectPlace",
                "Or select a place in this document:",
            )),
        );
        let sheets: Vec<String> = self.view.sheet_names().into_iter().flatten().collect();
        f.add(
            "bmList",
            None,
            [18.0, 132.0, 384.0, 170.0],
            Kind::List {
                items: sheets,
                sel: Some(self.view.sheet()),
                top: 0,
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [176.0, 314.0, 107.0, 30.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [295.0, 314.0, 107.0, 30.0];
        }
        f.focus("bmCell");
        if let Some(old) = self.form.take() {
            self.form_stash = Some((old, self.form_kind));
        }
        self.form = Some(f);
        self.form_kind = FormKind::LinkTip;
    }

    fn link_bookmark_text(&self) -> String {
        let Some(f) = self.form.as_ref() else {
            return String::new();
        };
        let sheet = match f.get("bmList").map(|c| &c.kind) {
            Some(Kind::List { items, sel, .. }) => {
                sel.and_then(|i| items.get(i)).cloned().unwrap_or_default()
            }
            _ => String::new(),
        };
        let q = if sheet.chars().all(|c| c.is_alphanumeric() || c == '_') {
            sheet
        } else {
            format!("'{}'", sheet.replace('\'', "''"))
        };
        format!("{q}!{}", f.text("bmCell").trim())
    }

    fn link_back(&mut self) {
        if let Some((f, k)) = self.form_stash.take() {
            self.form = Some(f);
            self.form_kind = k;
        }
    }

    /// The "Link to" bar: white, framed, the chosen entry shaded, own icons over two-line
    /// captions.
    pub(super) fn paint_link(&self, f: &Form, id: &str, r: [f32; 4], p: &mut dyn Painter) {
        let k = f.scale_k();
        if id.starts_with("rule") {
            p.fill_rect(r[0], r[1], r[2], 1.0, Rgba(0xDC, 0xDC, 0xDC, 255));
            return;
        }
        if id == "ltabs" {
            let font = f.font();
            let line = font.size * 1.33;
            for (i, (key, fb)) in LTABS.iter().enumerate() {
                let y = r[1] + (i as f32 * LTAB_H * k).round();
                let h = (LTAB_H * k).round();
                if i == self.link_tab {
                    p.fill_rect(r[0], y, r[2], h, Rgba(0x99, 0xC9, 0xEF, 255));
                    p.fill_rect(
                        r[0] + 1.0,
                        y + 1.0,
                        r[2] - 2.0,
                        h - 2.0,
                        Rgba(0xCC, 0xE4, 0xF7, 255),
                    );
                }
                let label = self.t(key, fb);
                let w = f.width(&label);
                p.text_font(
                    &label,
                    (r[0] + (r[2] - w) / 2.0).round(),
                    (y + (h - line) / 2.0).round(),
                    &font,
                    Rgba(0, 0, 0, 255),
                );
            }
            return;
        }
        if matches!(id, "upDir" | "web" | "file") {
            let on = f.get(id).is_some_and(|c| c.enabled);
            let ink = if on {
                Rgba(0x40, 0x40, 0x40, 255)
            } else {
                Rgba(0xA0, 0xA0, 0xA0, 255)
            };
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xD0, 0xD0, 0xD0, 255));
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                Rgba(0xFD, 0xFD, 0xFD, 255),
            );
            let (cx, cy) = ((r[0] + r[2] / 2.0).round(), (r[1] + r[3] / 2.0).round());
            let a = (6.0 * k).round();
            match id {
                // A folder with an arrow up.
                "upDir" => {
                    p.fill_rect(
                        cx - a,
                        cy - a * 0.4,
                        2.0 * a,
                        a * 1.3,
                        Rgba(0xE8, 0xB8, 0x4A, 255),
                    );
                    p.path(
                        &[
                            PathSeg::M(cx, cy + a * 0.6),
                            PathSeg::L(cx, cy - a),
                            PathSeg::M(cx - a * 0.5, cy - a * 0.5),
                            PathSeg::L(cx, cy - a),
                            PathSeg::L(cx + a * 0.5, cy - a * 0.5),
                        ],
                        None,
                        Some((ink, (1.4 * k).max(1.0))),
                    );
                }
                // A globe.
                "web" => {
                    let d = a;
                    let kk = d * 0.552_284_8;
                    p.path(
                        &[
                            PathSeg::M(cx + d, cy),
                            PathSeg::C(cx + d, cy + kk, cx + kk, cy + d, cx, cy + d),
                            PathSeg::C(cx - kk, cy + d, cx - d, cy + kk, cx - d, cy),
                            PathSeg::C(cx - d, cy - kk, cx - kk, cy - d, cx, cy - d),
                            PathSeg::C(cx + kk, cy - d, cx + d, cy - kk, cx + d, cy),
                            PathSeg::Z,
                            PathSeg::M(cx - d, cy),
                            PathSeg::L(cx + d, cy),
                            PathSeg::M(cx, cy - d),
                            PathSeg::L(cx, cy + d),
                        ],
                        None,
                        Some((ink, k.max(1.0))),
                    );
                }
                // A page.
                _ => {
                    p.fill_rect(cx - a * 0.7, cy - a, a * 1.4, 2.0 * a, ink);
                    p.fill_rect(
                        cx - a * 0.7 + 1.0,
                        cy - a + 1.0,
                        a * 1.4 - 2.0,
                        2.0 * a - 2.0,
                        Rgba(0xFD, 0xFD, 0xFD, 255),
                    );
                }
            }
            return;
        }
        if id != "bar" {
            return;
        }
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x87, 0x90, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let font = f.font();
        let line = font.size * 1.33;
        for (i, (key, fb)) in KINDS.iter().enumerate() {
            let y = r[1] + (i as f32 * ENTRY_H * k).round();
            let h = (ENTRY_H * k).round();
            if i == f.tab {
                p.fill_rect(
                    r[0] + 2.0,
                    y + 2.0,
                    r[2] - 4.0,
                    h - 4.0,
                    Rgba(0xCC, 0xE4, 0xF7, 255),
                );
            }
            let ink = if i == 2 {
                Rgba(0x83, 0x83, 0x83, 255)
            } else {
                Rgba(0, 0, 0, 255)
            };
            // Icon: a page with a globe / a sheet with an arrow / a new page / an envelope.
            let (cx, cy) = ((r[0] + r[2] / 2.0).round(), y + (20.0 * k).round());
            let a = (9.0 * k).round();
            let accent = Rgba(0x2E, 0x75, 0xB6, 255);
            if i == 3 {
                {
                    p.fill_rect(cx - a, cy - a * 0.7, 2.0 * a, 1.4 * a, ink);
                    p.fill_rect(
                        cx - a + 1.0,
                        cy - a * 0.7 + 1.0,
                        2.0 * a - 2.0,
                        1.4 * a - 2.0,
                        Rgba(255, 255, 255, 255),
                    );
                    p.path(
                        &[
                            PathSeg::M(cx - a, cy - a * 0.7),
                            PathSeg::L(cx, cy + 1.0),
                            PathSeg::L(cx + a, cy - a * 0.7),
                        ],
                        None,
                        Some((accent, k.max(1.0))),
                    );
                }
            } else {
                {
                    p.fill_rect(cx - a * 0.8, cy - a, 1.6 * a, 2.0 * a, ink);
                    p.fill_rect(
                        cx - a * 0.8 + 1.0,
                        cy - a + 1.0,
                        1.6 * a - 2.0,
                        2.0 * a - 2.0,
                        Rgba(255, 255, 255, 255),
                    );
                    let mark = if i == 2 {
                        Rgba(0xF2, 0xB7, 0x00, 255)
                    } else {
                        accent
                    };
                    p.fill_rect(cx - a * 0.3, cy - a * 0.2, a * 0.9, a * 0.9, mark);
                }
            }
            // Caption wrapped on two lines at the space nearest the middle.
            let label = self.t(key, fb);
            let words: Vec<&str> = label.split(' ').collect();
            let lines: Vec<String> = if words.len() > 2 {
                let mid = words.len().div_ceil(2);
                vec![words[..mid].join(" "), words[mid..].join(" ")]
            } else {
                vec![label.clone()]
            };
            for (n, l) in lines.iter().enumerate() {
                let w = f.width(l);
                p.text_font(
                    l,
                    (r[0] + (r[2] - w) / 2.0).round(),
                    (cy + a + 6.0 * k + n as f32 * line).round(),
                    &font,
                    ink,
                );
            }
        }
    }

    /// Texts the bar draws (measured at layout time).
    pub(super) fn link_strings(&self) -> Vec<String> {
        KINDS
            .iter()
            .chain(LTABS.iter())
            .flat_map(|(key, fb)| {
                let label = self.t(key, fb);
                let words: Vec<String> = label.split(' ').map(str::to_owned).collect();
                let mid = words.len().div_ceil(2);
                vec![words[..mid].join(" "), words[mid..].join(" "), label]
            })
            .collect()
    }
}
