//! BlueEditor round 5: commands that were grey — Form › Page Templates (name the current page as
//! a template, spawn templates after the current page, delete them), Convert › PDF Portfolio
//! (the embedded files of the document, read only: open PDFs in a tab, save the others) and
//! Protect › Secure › Batch Encrypt (password-protect several open documents and picked files,
//! saved as copies next to each other).
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::editor_actions::Ask2;
use crate::editor_window::EditorWindow;
use crate::reader_window::ReaderRequest;
use bo_pdfread::{PdfDocument, Permissions, SaveOptions};

/// A dialog of this module.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FiveAsk {
    /// Page Templates: template names in order.
    Templates(Vec<String>),
    /// Portfolio: embedded file names in order.
    Portfolio(Vec<String>),
    /// Batch Encrypt: open documents (tab indices) then picked files.
    Batch(Vec<usize>),
    /// Batch Print (`editor_file7`): number of open documents listed.
    BatchPrint(usize),
}

/// Files picked for Batch Encrypt (name, bytes) and the passwords typed so far.
#[derive(Debug, Default, Clone)]
pub(crate) struct FiveState {
    pub batch_files: Vec<(String, Vec<u8>)>,
    pub batch_typed: (String, String),
    /// The details view of an open portfolio.
    pub portfolio: Option<crate::editor_portfolio::PortfolioView>,
}

/// "1.2 KB" style size.
fn size_text(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f32 / 1024.0)
    } else {
        format!("{:.1} MB", n as f32 / 1024.0 / 1024.0)
    }
}

/// Stem of a tab or file name ("report.pdf" → "report").
fn stem(name: &str) -> &str {
    name.strip_suffix(".pdf")
        .or_else(|| name.strip_suffix(".PDF"))
        .unwrap_or(name)
}

impl EditorWindow {
    /// A list dialog of another module with this module's answers.
    pub(crate) fn ask_five(
        &mut self,
        what: FiveAsk,
        title: &str,
        checks: Vec<(String, bool)>,
        buttons: Vec<(String, DialogButton)>,
    ) {
        self.five_dialog(what, title, Vec::new(), checks, buttons);
    }

    fn five_dialog(
        &mut self,
        what: FiveAsk,
        title: &str,
        fields: Vec<(String, String)>,
        checks: Vec<(String, bool)>,
        buttons: Vec<(String, DialogButton)>,
    ) {
        let mut d = Dialog::new(
            DialogKind::Comment,
            self.t(title),
            fields,
            buttons,
            Vec::new(),
        );
        if !checks.is_empty() {
            d = d.with_checks(checks);
        }
        d.layout(
            &crate::editor_window::Estimate,
            self.window_size(),
            self.ui_scale(),
        );
        self.dialog2 = Some((d, Ask2::Five(what)));
    }

    /// Commands of this module; false when `cmd` is not one.
    pub(crate) fn five_command(&mut self, cmd: &str) -> bool {
        match cmd {
            "form.pageTemplates" => self.open_templates(),
            "view.transitions" => self.open_transitions(),
            "form.xfa" => self.static_xfa(),
            "conv.portfolio" => {
                if !self.portfolio_toggle() {
                    self.open_portfolio();
                }
            }
            "prot.batchEncrypt" => self.open_batch(),
            _ => return false,
        }
        true
    }

    fn open_templates(&mut self) {
        let list = self
            .with_doc(|d| Ok(d.page_templates()))
            .unwrap_or_default();
        let page = self.r.view().map_or(0, crate::ReaderView::current_page);
        let checks = list
            .iter()
            .map(|(n, p)| {
                (
                    format!("{n} — {} {}", self.t("pdfEditor.tplPage"), p + 1),
                    false,
                )
            })
            .collect();
        let names = list.into_iter().map(|(n, _)| n).collect();
        let buttons = vec![
            (self.t("pdfEditor.templateAdd"), DialogButton::Ok),
            (self.t("pdfEditor.templateReplace"), DialogButton::Extra(1)),
            (self.t("pdfEditor.templateRename"), DialogButton::Extra(2)),
            (self.t("pdfEditor.templateSpawn"), DialogButton::FindNext),
            (self.t("pdfEditor.templateDelete"), DialogButton::Replace),
            (self.t("pdfEditor.templateGoto"), DialogButton::ReplaceAll),
            (self.t("pdfEditor.close"), DialogButton::Cancel),
        ];
        self.five_dialog(
            FiveAsk::Templates(names),
            "pdfEditor.pageTemplates",
            vec![(
                format!(
                    "{} ({} {})",
                    self.t("pdfEditor.templateName"),
                    self.t("pdfEditor.tplPage"),
                    page + 1
                ),
                String::new(),
            )],
            checks,
            buttons,
        );
    }

    fn open_portfolio(&mut self) {
        let files = self
            .with_doc(|d| Ok(d.embedded_files()))
            .unwrap_or_default();
        if files.is_empty() {
            let msg = self.t("pdfEditor.noEmbeddedFiles");
            self.r.set_status(&msg);
            return;
        }
        let checks = files
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let desc = if f.description.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", f.description)
                };
                (format!("{} ({}){desc}", f.name, size_text(f.size)), i == 0)
            })
            .collect();
        let names = files.into_iter().map(|f| f.name).collect();
        let buttons = vec![
            (self.t("pdfEditor.portfolioOpen"), DialogButton::Ok),
            (self.t("pdfEditor.portfolioExtract"), DialogButton::FindNext),
            (self.t("pdfReader.cancel"), DialogButton::Cancel),
        ];
        self.five_dialog(
            FiveAsk::Portfolio(names),
            "pdfEditor.portfolio",
            Vec::new(),
            checks,
            buttons,
        );
    }

    fn open_batch(&mut self) {
        let mut tabs = Vec::new();
        let mut checks = Vec::new();
        let mut i = 0;
        while let Some(name) = self.r.tab_name(i) {
            checks.push((name.to_owned(), true));
            tabs.push(i);
            i += 1;
        }
        for (name, _) in &self.five.batch_files {
            checks.push((name.clone(), true));
        }
        checks.push((self.t("pdfEditor.aes256"), true));
        let (open, perm) = self.five.batch_typed.clone();
        let buttons = vec![
            (self.t("pdfEditor.batchRun"), DialogButton::Ok),
            (self.t("pdfEditor.batchAdd"), DialogButton::FindNext),
            (self.t("pdfReader.cancel"), DialogButton::Cancel),
        ];
        self.five_dialog(
            FiveAsk::Batch(tabs),
            "pdfEditor.batchEncrypt",
            vec![
                (self.t("pdfEditor.openPassword"), open),
                (self.t("pdfEditor.permissionsPassword"), perm),
            ],
            checks,
            buttons,
        );
    }

    /// A button of one of this module's dialogs.
    pub(crate) fn five_answer(&mut self, what: FiveAsk, b: DialogButton, v: &[String]) {
        let ticked = |from: usize, n: usize| -> Vec<usize> {
            (0..n)
                .filter(|&i| v.get(from + i).is_some_and(|s| s == "1"))
                .collect()
        };
        match what {
            FiveAsk::BatchPrint(n) => self.batch_print_answer(n, b, v),
            FiveAsk::Templates(names) => {
                let name = v.first().cloned().unwrap_or_default();
                let picked: Vec<String> = ticked(1, names.len())
                    .into_iter()
                    .filter_map(|i| names.get(i).cloned())
                    .collect();
                let page = self.r.view().map_or(0, crate::ReaderView::current_page);
                if b == DialogButton::ReplaceAll {
                    // Go to the first ticked template's page, as the reference's Goto.
                    let list = self
                        .with_doc(|d| Ok(d.page_templates()))
                        .unwrap_or_default();
                    let to = picked
                        .first()
                        .and_then(|n| list.iter().find(|(m, _)| m == n))
                        .map(|(_, p)| *p);
                    if let (Some(to), Some(v)) = (to, self.r.view_mut()) {
                        v.go_to(to, None);
                    }
                    return;
                }
                let r = match b {
                    DialogButton::Ok => self.doc_edit(|d| d.add_page_template(&name, page)),
                    DialogButton::FindNext => self.doc_edit(|d| {
                        // Spawned pages go after the current page, in the listed order.
                        let mut at = page + 1;
                        for n in &picked {
                            at = d.spawn_page_template(n, at)? + 1;
                        }
                        if picked.len() > 1 {
                            d.merge_last_steps(picked.len());
                        }
                        Ok(())
                    }),
                    DialogButton::Replace => self.doc_edit(|d| {
                        for n in &picked {
                            d.remove_page_template(n)?;
                        }
                        Ok(())
                    }),
                    // Replace: the ticked template names the current page instead.
                    DialogButton::Extra(1) => self.doc_edit(|d| {
                        let n = picked.first().ok_or("tick a template")?;
                        d.replace_page_template(n, page)
                    }),
                    // Rename: the ticked template takes the typed name.
                    DialogButton::Extra(2) => self.doc_edit(|d| {
                        let n = picked.first().ok_or("tick a template")?;
                        d.rename_page_template(n, &name)
                    }),
                    _ => return,
                };
                match r {
                    Ok(()) => self.after_doc_edit(),
                    Err(e) => self.r.set_status(&e),
                }
            }
            FiveAsk::Portfolio(names) => {
                let picked = ticked(0, names.len());
                let mut saves = Vec::new();
                for i in picked {
                    let Ok(bytes) = self.with_doc(|d| d.embedded_file_bytes(i)) else {
                        continue;
                    };
                    let name = names.get(i).cloned().unwrap_or_default();
                    if b == DialogButton::Ok && bytes.starts_with(b"%PDF") {
                        self.push(ReaderRequest::OpenDocument { name, bytes });
                    } else if matches!(b, DialogButton::Ok | DialogButton::FindNext) {
                        saves.push((name, bytes));
                    }
                }
                if !saves.is_empty() {
                    self.push(ReaderRequest::SaveMany(saves));
                }
            }
            FiveAsk::Batch(tabs) => {
                let open = v.first().cloned().unwrap_or_default();
                let perm = v.get(1).cloned().unwrap_or_default();
                self.five.batch_typed = (open.clone(), perm.clone());
                if b == DialogButton::FindNext {
                    self.push(ReaderRequest::PickFile {
                        purpose: "batchAdd".into(),
                        filters: vec![("PDF (*.pdf)".into(), "*.pdf".into())],
                    });
                    return;
                }
                if b != DialogButton::Ok {
                    self.five = crate::editor_five::FiveState::default();
                    return;
                }
                let nf = self.five.batch_files.len();
                let aes = v.get(2 + tabs.len() + nf).is_none_or(|s| s == "1");
                let owner = if perm.is_empty() { open.clone() } else { perm };
                if owner.is_empty() {
                    let msg = self.t("pdfEditor.batchNeedsPassword");
                    self.r.set_status(&msg);
                    return;
                }
                let mut out = Vec::new();
                let mut failed = Vec::new();
                for k in ticked(2, tabs.len()) {
                    let Some(&t) = tabs.get(k) else { continue };
                    let name = self.r.tab_name(t).unwrap_or_default().to_owned();
                    let bytes = self
                        .r
                        .tab_view_mut(t)
                        .and_then(|view| view.doc_mut().save_full(SaveOptions::default()).ok());
                    match bytes.and_then(|b| encrypt(b, &open, &owner, aes)) {
                        Some(b) => out.push((format!("{}-protected.pdf", stem(&name)), b)),
                        None => failed.push(name),
                    }
                }
                for k in ticked(2 + tabs.len(), nf) {
                    let Some((name, bytes)) = self.five.batch_files.get(k).cloned() else {
                        continue;
                    };
                    match encrypt(bytes, &open, &owner, aes) {
                        Some(b) => out.push((format!("{}-protected.pdf", stem(&name)), b)),
                        None => failed.push(name),
                    }
                }
                self.five = crate::editor_five::FiveState::default();
                if !failed.is_empty() {
                    let msg = format!("{}: {}", self.t("pdfEditor.batchFailed"), failed.join(", "));
                    self.r.set_status(&msg);
                }
                if !out.is_empty() {
                    let msg = format!("{} {}", out.len(), self.t("pdfEditor.batchDone"));
                    self.r.set_status(&msg);
                    self.push(ReaderRequest::SaveMany(out));
                }
            }
        }
    }

    /// A file picked for this module; false when `purpose` is not one of its.
    pub(crate) fn five_file_picked(&mut self, purpose: &str, name: &str, bytes: &[u8]) -> bool {
        if purpose != "batchAdd" {
            return false;
        }
        if !self.five.batch_files.iter().any(|(n, _)| n == name) {
            self.five
                .batch_files
                .push((name.to_owned(), bytes.to_vec()));
        }
        self.open_batch();
        true
    }
}

/// Password-protect PDF `bytes` (a plain file; an encrypted one is skipped).
fn encrypt(bytes: Vec<u8>, open: &str, owner: &str, aes256: bool) -> Option<Vec<u8>> {
    if PdfDocument::needs_password(&bytes) {
        return None;
    }
    let mut d = PdfDocument::open(bytes, bo_pdfread::standard_fonts()).ok()?;
    let allow = Permissions {
        print: true,
        copy: true,
        modify: false,
        annotate: true,
        assemble: false,
    };
    d.set_protection(open, owner, allow, aes256).ok()?;
    d.save_full(SaveOptions::default()).ok()
}
