//! BlueEditor round 9: the Organize drop-down menus as the reference editor has them
//! (black-box, 125 %): Insert ▾ — From File, From Folder, From Open Files, From Web Page,
//! From Scanner, From Clipboard, Blank Page, a line, Into File; Watermark / Background /
//! Header & Footer ▾ — Add, Update, Remove All (Bates Numbering ▾: Add, Remove All). Rows have
//! icons (our own drawings). Update opens the Add dialog with the values last applied and
//! replaces the marks of that kind on OK.
use crate::editor_dlg8::FormWhat;
use crate::editor_dlg8b::Vals;
use crate::editor_form8::Form;
use crate::editor_window::EditorWindow;
use crate::popup::{Choice, MenuItem};
use crate::reader_window::ReaderRequest;

/// One menu row: command, label key, icon, dimmed.
type Row = (&'static str, &'static str, Option<&'static str>, bool);

/// Rows of the Insert menu and the rows a line comes before.
pub(crate) fn insert_rows() -> (Vec<Row>, Vec<usize>) {
    (
        vec![
            (
                "org.insertFile",
                "pdfEditor9.fromFile",
                Some("insertPage"),
                false,
            ),
            // Folders: the system file dialog of the host picks files only.
            (
                "org9.insertFolder",
                "pdfEditor9.fromFolder",
                Some("open"),
                true,
            ),
            (
                "org9.insertOpen",
                "pdfEditor9.fromOpenFiles",
                Some("pages"),
                false,
            ),
            (
                "org9.insertWeb",
                "pdfEditor9.fromWebPage",
                Some("fromWeb"),
                true,
            ),
            (
                "org9.insertScanner",
                "pdfEditor9.fromScanner",
                Some("scanner"),
                true,
            ),
            (
                "org9.insertClipboard",
                "pdfEditor9.fromClipboard",
                Some("fromClipboard"),
                false,
            ),
            (
                "org.insertBlank",
                "pdfEditor9.blankPage",
                Some("file"),
                false,
            ),
            ("org9.intoFile", "pdfEditor9.intoFile", Some("export"), true),
        ],
        vec![7],
    )
}

/// Rows of the Add / Update / Remove All menu of a page design kind (`headerFooter`,
/// `watermark`, `background`, `bates`).
pub(crate) fn design_rows(kind: &str) -> Vec<(String, &'static str, Option<&'static str>)> {
    let mut rows = vec![(format!("org8.add.{kind}"), "pdfEditor9.add", Some("plus"))];
    if kind != "bates" {
        rows.push((
            format!("org9.update.{kind}"),
            "pdfEditor9.update",
            Some("redo"),
        ));
    }
    rows.push((
        format!("org8.remove.{kind}"),
        "pdfEditor9.removeAll",
        Some("cross"),
    ));
    rows
}

fn kind_of(w: &FormWhat) -> Option<&'static str> {
    match w {
        FormWhat::HeaderFooter => Some("headerFooter"),
        FormWhat::Watermark => Some("watermark"),
        FormWhat::Background => Some("background"),
        _ => None,
    }
}

/// Mark subtypes written for a kind (`bo_pdfread` page design artifacts).
fn subtypes(kind: &str) -> &'static [&'static str] {
    match kind {
        "watermark" => &["Watermark"],
        "background" => &["Background"],
        _ => &["Header", "Footer"],
    }
}

impl EditorWindow {
    fn menu9(
        &mut self,
        rows: &[(String, &'static str, Option<&'static str>, bool)],
        seps: Vec<usize>,
        id: &'static str,
    ) {
        let Some(r) = self.r.control_rect(id) else {
            return;
        };
        let items: Vec<MenuItem> = rows
            .iter()
            .map(|(cmd, key, _, dim)| MenuItem {
                label: self.t(key),
                choice: Choice::Command(cmd.clone()),
                checked: false,
                dim: *dim,
            })
            .collect();
        let icons = rows.iter().map(|r| r.2).collect();
        let menu = self
            .r
            .reader_menu(items, r)
            .with_decor(icons, seps)
            .with_icon_fn(crate::reader_icons::menu_icon);
        self.r.set_menu9(menu, id);
    }

    /// Round 9 Organize commands; false when `cmd` is not one.
    pub(crate) fn org9_command(&mut self, cmd: &str) -> bool {
        match cmd {
            "org.insertMenu" => {
                let (rows, seps) = insert_rows();
                let rows: Vec<_> = rows
                    .into_iter()
                    .map(|(c, k, i, d)| (c.to_owned(), k, i, d))
                    .collect();
                self.menu9(&rows, seps, "org.insertMenu");
            }
            "org.headerFooter" | "org.watermark" | "org.background" | "org.bates" => {
                let kind = cmd.trim_start_matches("org.");
                let rows: Vec<_> = design_rows(kind)
                    .into_iter()
                    .map(|(c, k, i)| (c, k, i, false))
                    .collect();
                let id: &'static str = match kind {
                    "headerFooter" => "org.headerFooter",
                    "watermark" => "org.watermark",
                    "background" => "org.background",
                    _ => "org.bates",
                };
                self.menu9(&rows, Vec::new(), id);
            }
            "org9.update.headerFooter" | "org9.update.watermark" | "org9.update.background" => {
                let kind: &'static str = match cmd {
                    "org9.update.headerFooter" => "headerFooter",
                    "org9.update.watermark" => "watermark",
                    _ => "background",
                };
                self.run_dlg9(&format!("org8.add.{kind}"));
                if let (Some(vals), Some((f, _))) =
                    (self.org9_last.get(kind).cloned(), self.form8.last_mut())
                {
                    for (k, v) in vals {
                        if k != "saved" {
                            f.set(k, v);
                        }
                    }
                }
                self.org9_update = Some(kind);
            }
            "org9.insertOpen" => self.insert_open_menu(),
            c if c.starts_with("org9.insertTab.") => {
                if let Ok(i) = c["org9.insertTab.".len()..].parse::<usize>() {
                    self.insert_tab9(i);
                }
            }
            "org9.insertClipboard" => {
                self.clip_insert9 = true;
                self.push(ReaderRequest::Command("host.clipboardImage".into()));
            }
            "org9.insertFolder" | "org9.insertWeb" | "org9.insertScanner" | "org9.intoFile" => {}
            _ => return false,
        }
        true
    }

    fn run_dlg9(&mut self, cmd: &str) {
        let _ = self.dlg8_command(cmd);
    }

    /// From Open Files: a menu of the other open documents.
    fn insert_open_menu(&mut self) {
        let active = self.r.active_tab();
        let rows: Vec<(String, &'static str, Option<&'static str>, bool)> = (0..self.r.tab_count())
            .filter(|&i| i != active)
            .map(|i| (format!("org9.insertTab.{i}"), "", Some("file"), false))
            .collect();
        if rows.is_empty() {
            let s = self.t("pdfEditor9.noOtherFiles");
            self.set_status(&s);
            return;
        }
        let Some(r) = self.r.control_rect("org.insertMenu") else {
            return;
        };
        let items: Vec<MenuItem> = rows
            .iter()
            .map(|(cmd, _, _, _)| {
                let i: usize = cmd["org9.insertTab.".len()..].parse().unwrap_or(0);
                MenuItem {
                    label: self.r.doc_tab_name(i).unwrap_or_default().to_owned(),
                    choice: Choice::Command(cmd.clone()),
                    checked: false,
                    dim: false,
                }
            })
            .collect();
        let icons = rows.iter().map(|r| r.2).collect();
        let menu = self
            .r
            .reader_menu(items, r)
            .with_decor(icons, Vec::new())
            .with_icon_fn(crate::reader_icons::menu_icon);
        self.r.set_menu9(menu, "org.insertMenu");
    }

    /// Insert all pages of open document tab `i` after the current page.
    pub(crate) fn insert_tab9(&mut self, i: usize) {
        let Some(bytes) = self.r.tab_view_mut(i).and_then(|v| v.doc_mut().save().ok()) else {
            return;
        };
        self.insert_bytes9(&bytes);
    }

    fn insert_bytes9(&mut self, bytes: &[u8]) {
        let at = self.insert_at();
        match self.doc_edit(|d| d.insert_pdf(at, bytes, None)) {
            Ok(n) => {
                self.after_doc_edit();
                self.org.select_range(at, at + n.saturating_sub(1));
                let s = self
                    .t("pdfEditor9.pagesInserted")
                    .replace("{{n}}", &n.to_string());
                self.set_status(&s);
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// Insert › From Clipboard: the picture on the clipboard as a new page after the current
    /// one. True when the file was for it.
    pub(crate) fn org9_file_picked(&mut self, purpose: &str, bytes: &[u8]) -> bool {
        if purpose != "clipboardImage" || !self.clip_insert9 {
            return false;
        }
        self.clip_insert9 = false;
        let png = bo_raster::codecs::decode_dib(bytes).and_then(bo_raster::rgba_to_png);
        match png.map(|p| crate::editor_more::picture_pdf(&p)) {
            Some(Ok(pdf)) => self.insert_bytes9(&pdf),
            Some(Err(e)) => self.set_status(&e),
            None => {
                let s = self.t("pdfEditor.noClipboardImage");
                self.set_status(&s);
            }
        }
        true
    }

    /// Before the OK of a page design dialog: keep its values for Update, and when it was
    /// opened by Update take the marks of that kind away first.
    pub(crate) fn org9_before_ok(&mut self, what: &FormWhat, f: &Form) {
        let Some(kind) = kind_of(what) else { return };
        let vals: Vals = f.vals.clone();
        self.org9_last.insert(kind, vals);
        if self.org9_update.take() == Some(kind) {
            if let Err(e) = self.doc_edit(|d| d.remove_page_marks(subtypes(kind))) {
                self.set_status(&e);
            }
        }
    }
}
