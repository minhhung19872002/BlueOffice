//! BlueEditor round 4, Comment / View / Protect / Form tabs as in the reference editor:
//! comment Import / Export (XFDF, highlighted text), Compare documents (side-by-side text
//! differences as a new PDF), Hidden Data, Restrict Access, Time Stamp, Calculation Order,
//! Add Tooltip, Form to sheet (CSV / workbook), form Import / Export (XFDF, FDF, CSV) and
//! simple button actions (reset the form, show or hide fields). The reader window's own
//! tools (Eraser, Area / Search & Highlight, Measure, Text Viewer, rulers, Read aloud,
//! Validate) are used as they are.
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::editor_actions::Ask2;
use crate::editor_window::EditorWindow;
use crate::reader_window::ReaderRequest;
use bo_pdfread::{AnnotKind, ButtonAction, FieldKind, FormField, HiddenData, PdfDocument};

/// What a dialog of this module asks.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum More2Ask {
    HiddenData,
    Restrict,
    TimeStamp,
    CalcOrder,
    Tooltip,
    /// Button action: reset (`None`), or show / hide (`Some(hide)`).
    Action(Option<bool>),
}

/// Grey commands of these tabs with their tooltip.
pub(crate) const NOT_OFFERED2: [(&str, &str); 5] = [
    ("view.transitions", "pdfEditor.tipTransitions"),
    ("form.xfa", "pdfEditor.tipXfa"),
    ("conv.form", "pdfEditor.tipForm"),
    ("conv.preflight", "pdfEditor.tipPreflight"),
    ("cm.calculator", "pdfEditor.tipCalculator"),
];

const DATA_FILTERS: &str = "*.xfdf;*.fdf;*.csv";

/// Lines of `a` and `b` paired by a longest common subsequence: (left, right), `None`
/// where a side has no line.
pub(crate) fn diff_lines<'a>(
    a: &[&'a str],
    b: &[&'a str],
) -> Vec<(Option<&'a str>, Option<&'a str>)> {
    let (n, m) = (a.len(), b.len());
    // Very long pages: compare line by line without alignment.
    if n * m > 4_000_000 {
        return (0..n.max(m))
            .map(|i| (a.get(i).copied(), b.get(i).copied()))
            .collect();
    }
    let mut l = vec![vec![0_u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            l[i][j] = if a[i] == b[j] {
                l[i + 1][j + 1] + 1
            } else {
                l[i + 1][j].max(l[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            out.push((Some(a[i]), Some(b[j])));
            i += 1;
            j += 1;
        } else if j < m && (i == n || l[i][j + 1] >= l[i + 1][j]) {
            out.push((None, Some(b[j])));
            j += 1;
        } else {
            out.push((Some(a[i]), None));
            i += 1;
        }
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// An HTML report of the differences between the texts of two documents, page by page.
pub(crate) fn compare_report(
    a_name: &str,
    a: &[String],
    b_name: &str,
    b: &[String],
    labels: [&str; 4],
) -> (String, usize) {
    let mut changes = 0;
    let mut rows = String::new();
    for p in 0..a.len().max(b.len()) {
        let la: Vec<&str> = a
            .get(p)
            .map(|t| t.lines().filter(|l| !l.trim().is_empty()).collect())
            .unwrap_or_default();
        let lb: Vec<&str> = b
            .get(p)
            .map(|t| t.lines().filter(|l| !l.trim().is_empty()).collect())
            .unwrap_or_default();
        let d = diff_lines(&la, &lb);
        if d.iter().all(|(x, y)| x.is_some() && x == y) {
            continue;
        }
        rows.push_str(&format!(
            "<tr><td colspan=\"2\"><b>{} {}</b></td></tr>\n",
            labels[2],
            p + 1
        ));
        for (x, y) in d {
            if x.is_some() && x == y {
                continue;
            }
            changes += 1;
            let cell = |s: Option<&str>, color: &str| {
                s.map_or_else(String::new, |s| {
                    format!("<span style=\"color:{color}\">{}</span>", esc(s))
                })
            };
            rows.push_str(&format!(
                "<tr><td>{}</td><td>{}</td></tr>\n",
                cell(x, "#C00000"),
                cell(y, "#0050C0")
            ));
        }
    }
    let body = if changes == 0 {
        format!("<p>{}</p>", labels[3])
    } else {
        format!(
            "<table border=\"1\" cellpadding=\"4\" style=\"border-collapse:collapse;width:100%\">\n\
             <tr><th>{}</th><th>{}</th></tr>\n{rows}</table>",
            esc(a_name),
            esc(b_name)
        )
    };
    (
        format!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{}</title></head><body>\
             <h1>{}</h1><p>{}: {changes}</p>{body}</body></html>",
            labels[0], labels[0], labels[1]
        ),
        changes,
    )
}

/// Text of every page, one line per text line.
fn page_texts(doc: &mut PdfDocument) -> Vec<String> {
    (0..doc.page_count())
        .map(|i| {
            doc.render(i, bo_pdfread::RenderOptions::default())
                .map(|p| bo_pdfread::TextLayer::new(p.chars).text())
                .unwrap_or_default()
        })
        .collect()
}

fn names_of(s: &str) -> Vec<String> {
    s.split([',', ';', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

impl EditorWindow {
    fn more2_dialog(
        &mut self,
        what: More2Ask,
        title: &str,
        fields: Vec<(String, String)>,
        checks: Vec<(String, bool)>,
    ) {
        let mut d = Dialog::new(
            DialogKind::Comment,
            self.t(title),
            fields,
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfReader.cancel"), DialogButton::Cancel),
            ],
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
        self.dialog2 = Some((d, Ask2::More2(what)));
    }

    fn author(&self) -> String {
        self.prefs.author.clone()
    }

    fn pick(&mut self, purpose: &str, filter: &str) {
        self.push(ReaderRequest::PickFile {
            purpose: purpose.into(),
            filters: vec![(filter.into(), filter.into())],
        });
    }

    /// Commands of this module; false when `cmd` is not one.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn more2_command(&mut self, cmd: &str) -> bool {
        if NOT_OFFERED2.iter().any(|(c, _)| *c == cmd) {
            return true;
        }
        let stem = self.doc_stem();
        match cmd {
            "cm.import" => self.pick("importComments", "*.xfdf;*.fdf"),
            "cm.exportMenu" => {
                let items = [
                    ("cm.exportXfdf", "pdfEditor.exportCommentsXfdf"),
                    ("cm.exportHighlights", "pdfEditor.exportHighlights"),
                ];
                self.menu(&items, "cm.exportMenu");
            }
            "cm.exportXfdf" => {
                let file = format!("{stem}.pdf");
                if let Ok(x) = self.with_doc(|d| Ok(d.to_xfdf(true, false, &file))) {
                    self.push(ReaderRequest::SaveBytes {
                        name: format!("{stem}.xfdf"),
                        bytes: x.into_bytes(),
                    });
                }
            }
            "cm.exportHighlights" => {
                let text = self.highlighted_text();
                self.push(ReaderRequest::SaveBytes {
                    name: format!("{stem}-highlights.txt"),
                    bytes: text.into_bytes(),
                });
            }
            "view.compare" => self.pick("compare", "*.pdf"),
            "prot.secureMenu" => {
                let items = [
                    ("prot.password", "pdfEditor.passwordProtect"),
                    ("prot.restrict", "pdfEditor.restrictAccess"),
                    ("prot.removeSecurity", "pdfEditor.removeSecurity"),
                    ("prot.batchEncrypt", "pdfEditor.batchEncrypt"),
                ];
                self.menu(&items, "prot.secureMenu");
            }
            "prot.hiddenData" => {
                let c = self.with_doc(|d| Ok(d.hidden_counts())).unwrap_or_default();
                let lab = |w: &Self, k: &str, n: usize| (format!("{} ({n})", w.t(k)), n > 0);
                let checks = vec![
                    lab(self, "pdfEditor.hdMetadata", c.metadata),
                    lab(self, "pdfEditor.hdAttachments", c.attachments),
                    lab(self, "pdfEditor.hdComments", c.comments),
                    lab(self, "pdfEditor.hdForms", c.forms),
                    lab(self, "pdfEditor.hdLayers", c.hidden_layers),
                    lab(self, "pdfEditor.hdBookmarks", c.bookmarks),
                    lab(self, "pdfEditor.hdLinks", c.links),
                    lab(self, "pdfEditor.hdScripts", c.scripts),
                ];
                self.more2_dialog(
                    More2Ask::HiddenData,
                    "pdfEditor.hiddenData",
                    Vec::new(),
                    checks,
                );
            }
            "prot.restrict" => {
                let fields = vec![(self.t("pdfEditor.permissionsPassword"), String::new())];
                let checks = vec![
                    (self.t("pdfEditor.allowPrint"), true),
                    (self.t("pdfEditor.allowCopy"), true),
                    (self.t("pdfEditor.allowModify"), false),
                    (self.t("pdfEditor.allowComments"), true),
                    (self.t("pdfEditor.allowAssemble"), false),
                    (self.t("pdfEditor.aes256"), true),
                ];
                self.more2_dialog(
                    More2Ask::Restrict,
                    "pdfEditor.restrictAccess",
                    fields,
                    checks,
                );
            }
            "prot.timestamp" => {
                let fields = vec![(
                    self.t("pdfEditor.tsaUrl"),
                    "http://timestamp.digicert.com".to_owned(),
                )];
                self.more2_dialog(
                    More2Ask::TimeStamp,
                    "pdfEditor.timeStamp",
                    fields,
                    Vec::new(),
                );
            }
            "form.calcOrder" => {
                let cur = self
                    .with_doc(|d| Ok(d.calculation_order()))
                    .unwrap_or_default();
                let names = if cur.is_empty() {
                    self.form_names(true)
                } else {
                    cur
                };
                let fields = vec![(self.t("pdfEditor.calcOrderFields"), names.join(", "))];
                self.more2_dialog(
                    More2Ask::CalcOrder,
                    "pdfEditor.calcOrder",
                    fields,
                    Vec::new(),
                );
            }
            "form.tooltip" => {
                let first = self
                    .form_names(false)
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                let tip = self
                    .field_named(&first)
                    .and_then(|f| self.with_doc(|d| Ok(d.field_tooltip(&f))).ok())
                    .unwrap_or_default();
                let fields = vec![
                    (self.t("pdfEditor.fieldName"), first),
                    (self.t("pdfEditor.fieldTooltip"), tip),
                ];
                self.more2_dialog(
                    More2Ask::Tooltip,
                    "pdfEditor.addTooltip",
                    fields,
                    Vec::new(),
                );
            }
            "form.toSheetMenu" => {
                let items = [
                    ("form.sheetCsv", "pdfEditor.sheetCsv"),
                    ("form.sheetXlsx", "pdfEditor.sheetXlsx"),
                ];
                self.menu(&items, "form.toSheetMenu");
            }
            "form.sheetCsv" => {
                if let Ok(csv) = self.with_doc(|d| Ok(d.fields_csv())) {
                    let mut bytes = "\u{feff}".as_bytes().to_vec();
                    bytes.extend_from_slice(csv.as_bytes());
                    self.push(ReaderRequest::SaveBytes {
                        name: format!("{stem}.csv"),
                        bytes,
                    });
                }
            }
            "form.sheetXlsx" => {
                let r = self.with_doc(|d| {
                    let csv = d.fields_csv();
                    let mut s = bo_engine::SheetEngine::with_bundled_fonts();
                    s.open(csv.as_bytes(), "form.csv")
                        .map_err(|e| e.to_string())?;
                    s.save("xlsx", 0).map_err(|e| e.to_string())
                });
                match r {
                    Ok(bytes) => self.push(ReaderRequest::SaveBytes {
                        name: format!("{stem}.xlsx"),
                        bytes,
                    }),
                    Err(e) => self.set_status(&e),
                }
            }
            "form.importData" => self.pick("importForm", DATA_FILTERS),
            "form.exportMenu" => {
                let items = [
                    ("form.exportXfdf", "pdfEditor.formXfdf"),
                    ("form.exportFdf", "pdfEditor.formFdf"),
                    ("form.sheetCsv", "pdfEditor.formCsv"),
                ];
                self.menu(&items, "form.exportMenu");
            }
            "form.exportXfdf" => {
                let file = format!("{stem}.pdf");
                if let Ok(x) = self.with_doc(|d| Ok(d.to_xfdf(false, true, &file))) {
                    self.push(ReaderRequest::SaveBytes {
                        name: format!("{stem}.xfdf"),
                        bytes: x.into_bytes(),
                    });
                }
            }
            "form.exportFdf" => {
                if let Ok(bytes) = self.with_doc(|d| Ok(d.fields_fdf())) {
                    self.push(ReaderRequest::SaveBytes {
                        name: format!("{stem}.fdf"),
                        bytes,
                    });
                }
            }
            "form.actions" => {
                let items = [
                    ("form.actionReset", "pdfEditor.actionReset"),
                    ("form.actionShow", "pdfEditor.actionShow"),
                    ("form.actionHide", "pdfEditor.actionHide"),
                ];
                self.menu(&items, "form.actions");
            }
            "form.actionReset" | "form.actionShow" | "form.actionHide" => {
                let button = self
                    .fields()
                    .into_iter()
                    .find(|f| f.kind == FieldKind::Button)
                    .map(|f| f.name)
                    .unwrap_or_default();
                let mut fields = vec![(self.t("pdfEditor.buttonName"), button)];
                let what = match cmd {
                    "form.actionReset" => None,
                    c => {
                        fields.push((self.t("pdfEditor.targetFields"), String::new()));
                        Some(c == "form.actionHide")
                    }
                };
                self.more2_dialog(
                    More2Ask::Action(what),
                    "pdfEditor.buttonAction",
                    fields,
                    Vec::new(),
                );
            }
            "form.settings" => {
                let items = [
                    ("form.highlight", "pdfEditor.highlightFields"),
                    ("form.fill", "pdfEditor.fillSign"),
                    ("form.flatten", "pdfEditor.flatten"),
                ];
                self.menu(&items, "form.settings");
            }
            _ => return false,
        }
        true
    }

    /// Field names of the document (text fields only with `text_only`).
    fn form_names(&mut self, text_only: bool) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for f in self.fields() {
            let keep = !text_only || matches!(f.kind, FieldKind::Text | FieldKind::Combo);
            if keep && f.kind != FieldKind::Signature && !v.contains(&f.name) {
                v.push(f.name);
            }
        }
        v
    }

    fn field_named(&mut self, name: &str) -> Option<FormField> {
        self.fields().into_iter().find(|f| f.name == name)
    }

    /// The text under every highlight, one per line with its page.
    fn highlighted_text(&mut self) -> String {
        let label = self.t("pdfEditor.pageWord");
        let Some(v) = self.view_mut() else {
            return String::new();
        };
        let annots: Vec<_> = v
            .doc()
            .annotations()
            .into_iter()
            .filter(|a| a.kind == AnnotKind::Highlight)
            .collect();
        let mut out = String::new();
        for a in annots {
            let Some(layer) = v.text(a.page) else {
                continue;
            };
            let s: String = layer
                .chars()
                .iter()
                .filter(|c| {
                    let (cx, cy) = (
                        f32::midpoint(c.rect[0], c.rect[2]),
                        f32::midpoint(c.rect[1], c.rect[3]),
                    );
                    cx >= a.rect[0] && cx <= a.rect[2] && cy >= a.rect[1] && cy <= a.rect[3]
                })
                .map(|c| c.text.as_str())
                .collect();
            out.push_str(&format!("{label} {}: {}\r\n", a.page + 1, s.trim()));
        }
        out
    }

    /// A file picked for this module; false when `purpose` is not one of its own.
    pub(crate) fn more2_file_picked(&mut self, purpose: &str, name: &str, bytes: &[u8]) -> bool {
        match purpose {
            "importComments" | "importForm" => {
                let author = self.author();
                match self.doc_edit(|d| d.import_data(bytes, &author)) {
                    Ok(rep) => {
                        self.after_doc_edit();
                        let s = self
                            .t("pdfEditor.imported")
                            .replace("{comments}", &rep.comments.to_string())
                            .replace("{fields}", &rep.fields.to_string());
                        self.set_status(&s);
                    }
                    Err(e) => self.set_status(&e),
                }
            }
            "compare" => {
                let other = PdfDocument::open(bytes.to_vec(), bo_pdfread::standard_fonts())
                    .map(|mut d| page_texts(&mut d));
                let mine = self.with_doc(|d| Ok(page_texts(d)));
                match (mine, other) {
                    (Ok(a), Ok(b)) => {
                        let labels = [
                            self.t("pdfEditor.compareTitle"),
                            self.t("pdfEditor.compareChanges"),
                            self.t("pdfEditor.pageWord"),
                            self.t("pdfEditor.compareSame"),
                        ];
                        let l = [
                            labels[0].as_str(),
                            labels[1].as_str(),
                            labels[2].as_str(),
                            labels[3].as_str(),
                        ];
                        let me = format!("{}.pdf", self.doc_stem());
                        let (html, n) = compare_report(&me, &a, name, &b, l);
                        let pdf =
                            crate::editor_actions::office_to_pdf("compare.html", html.as_bytes());
                        match pdf {
                            Ok(pdf) => {
                                self.push(ReaderRequest::OpenDocument {
                                    name: format!("{}.pdf", labels[0]),
                                    bytes: pdf,
                                });
                                let s = format!("{}: {n}", labels[1]);
                                self.set_status(&s);
                            }
                            Err(e) => self.set_status(&e),
                        }
                    }
                    (Err(e), _) | (_, Err(e)) => self.set_status(&e),
                }
            }
            _ => return false,
        }
        true
    }

    /// The answer of a dialog of this module.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn more2_answer(&mut self, what: &More2Ask, v: &[String]) {
        let get = |i: usize| v.get(i).map_or("", String::as_str).trim().to_owned();
        let on = |i: usize| v.get(i).is_some_and(|s| s == "1");
        let r: Result<(), String> = match what {
            More2Ask::HiddenData => {
                let h = HiddenData {
                    metadata: on(0),
                    attachments: on(1),
                    comments: on(2),
                    forms: on(3),
                    hidden_layers: on(4),
                    bookmarks: on(5),
                    links: on(6),
                    scripts: on(7),
                };
                let r = self.doc_edit(|d| d.remove_hidden_data(h));
                if r.is_ok() {
                    self.x.selected_comment = None;
                    let s = self.t("pdfEditor.hiddenRemoved");
                    self.set_status(&s);
                }
                r
            }
            More2Ask::Restrict => {
                let owner = get(0);
                if owner.is_empty() {
                    Err(self.t("pdfEditor.needPassword"))
                } else {
                    let allow = bo_pdfread::Permissions {
                        print: on(1),
                        copy: on(2),
                        modify: on(3),
                        annotate: on(4),
                        assemble: on(5),
                    };
                    let aes = on(6);
                    let r = self.doc_edit(|d| d.set_protection("", &owner, allow, aes));
                    if r.is_ok() {
                        let s = self.t("pdfEditor.securityAdded");
                        self.set_status(&s);
                    }
                    r
                }
            }
            More2Ask::TimeStamp => self.time_stamp(&get(0)),
            More2Ask::CalcOrder => {
                let names = names_of(&get(0));
                self.doc_edit(|d| d.set_calculation_order(&names).map(|_| ()))
            }
            More2Ask::Tooltip => match self.field_named(&get(0)) {
                Some(f) => {
                    let tip = get(1);
                    self.doc_edit(|d| d.set_field_tooltip(&f, &tip))
                }
                None => Err(self.t("pdfEditor.noSuchField")),
            },
            More2Ask::Action(kind) => match self.field_named(&get(0)) {
                Some(f) if f.kind == FieldKind::Button => {
                    let a = match kind {
                        None => ButtonAction::ResetForm,
                        Some(hide) => ButtonAction::Hide(names_of(&get(1)), *hide),
                    };
                    self.doc_edit(|d| d.set_button_action(&f, &a))
                }
                _ => Err(self.t("pdfEditor.noSuchButton")),
            },
        };
        match r {
            Ok(()) => {
                if let Some(v) = self.view_mut() {
                    for i in 0..v.page_count() {
                        v.invalidate_page(i);
                    }
                }
                self.after_doc_edit();
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// Time Stamp Document: a document time-stamp (PAdES B-LTA `/DocTimeStamp`) from the
    /// time-stamp authority at `url`; the stamped copy opens in a new tab.
    fn time_stamp(&mut self, url: &str) -> Result<(), String> {
        if url.is_empty() {
            return Err(self.t("pdfEditor.needTsa"));
        }
        let bytes = self.with_doc(PdfDocument::save)?;
        let client = bo_sign::http::HttpTimestampClient::new(
            url.to_owned(),
            Box::new(bo_sign::http::UreqTransport::new(
                std::time::Duration::from_secs(20),
            )),
        );
        let stamped =
            bo_sign::pdf::add_document_timestamp(&bytes, &client).map_err(|e| e.to_string())?;
        let file = format!("{}-timestamped.pdf", self.doc_stem());
        self.push(ReaderRequest::SaveBytes {
            name: file.clone(),
            bytes: stamped.clone(),
        });
        self.push(ReaderRequest::OpenDocument {
            name: file,
            bytes: stamped,
        });
        let s = self.t("pdfEditor.timeStamped");
        self.set_status(&s);
        Ok(())
    }

    /// A push button with an action was clicked while filling: run it. True when it ran.
    pub(crate) fn run_button_action(&mut self, f: &FormField) -> bool {
        if f.kind != FieldKind::Button {
            return false;
        }
        let Some(a) = self.with_doc(|d| Ok(d.button_action(f))).ok().flatten() else {
            return false;
        };
        let r = match a {
            ButtonAction::ResetForm => self.doc_edit(PdfDocument::reset_form),
            ButtonAction::Hide(names, hide) => {
                self.doc_edit(|d| d.hide_fields(&names, hide).map(|_| ()))
            }
        };
        match r {
            Ok(()) => {
                if let Some(v) = self.view_mut() {
                    v.invalidate_page(f.page);
                }
                self.after_doc_edit();
            }
            Err(e) => self.set_status(&e),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_diff_pairs_common_lines() {
        let d = diff_lines(&["a", "b", "c"], &["a", "x", "c", "d"]);
        assert_eq!(
            d,
            vec![
                (Some("a"), Some("a")),
                (None, Some("x")),
                (Some("b"), None),
                (Some("c"), Some("c")),
                (None, Some("d")),
            ]
        );
        let (html, n) = compare_report(
            "a.pdf",
            &["one\ntwo".into()],
            "b.pdf",
            &["one\n2".into()],
            ["Compare", "Changes", "Page", "Same"],
        );
        assert_eq!(n, 2);
        assert!(html.contains("color:#C00000\">two") && html.contains("color:#0050C0\">2"));
        let (_, n) = compare_report("a", &["x".into()], "b", &["x".into()], ["", "", "", ""]);
        assert_eq!(n, 0);
    }
}
