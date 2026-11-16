//! BlueEditor round 9: Combine Files. The reference editor (black-box, 125 %) opens a page with
//! the commands Add Files / Add Open Files / Remove / Move Up / Move Down and Combine /
//! Cancel over a list of the files; BlueEditor shows them in a classic dialog of the same
//! commands over a table of the files with each one's page range (all pages, or "1-3, 5"),
//! its number of pages and size. Combine makes a new document of the ranges in list order
//! and opens it in a tab (the files themselves are not changed).
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{button, label, Ctl, Form, On, Val};
use crate::editor_window::EditorWindow;
use crate::reader_window::ReaderRequest;

/// One file of the list.
#[derive(Debug, Clone)]
pub(crate) struct CombineItem {
    pub name: String,
    pub bytes: Vec<u8>,
    pub pages: usize,
    /// Page range text ("" = all pages).
    pub range: String,
}

fn item(name: &str, bytes: Vec<u8>) -> Option<CombineItem> {
    let doc = bo_pdfread::PdfDocument::open(bytes.clone(), bo_pdfread::standard_fonts()).ok()?;
    Some(CombineItem {
        name: name.to_owned(),
        pages: doc.page_count(),
        bytes,
        range: String::new(),
    })
}

impl EditorWindow {
    /// Open Combine Files with the active document first.
    pub(crate) fn open_combine9(&mut self) {
        if self.combine9.is_empty() {
            let name = self.doc_name();
            let name = if name.to_ascii_lowercase().ends_with(".pdf") {
                name
            } else {
                format!("{name}.pdf")
            };
            if let Some(bytes) = self.r.view_mut().and_then(|v| v.doc_mut().save().ok()) {
                self.combine9.extend(item(&name, bytes));
            }
        }
        self.show_combine9(0);
    }

    /// (Re)build the dialog from the list, row `row` selected.
    fn show_combine9(&mut self, row: usize) {
        if matches!(self.form8.last(), Some((_, FormWhat::Combine9))) {
            self.form8.pop();
        }
        let t = |k: &str| self.t(k);
        let row = row.min(self.combine9.len().saturating_sub(1));
        let n = self.combine9.len();
        let rows: Vec<Vec<String>> = self
            .combine9
            .iter()
            .map(|c| {
                vec![
                    c.name.clone(),
                    if c.range.trim().is_empty() {
                        t("pdfEditor9.allPages")
                    } else {
                        c.range.clone()
                    },
                    c.pages.to_string(),
                    format!("{:.1} KB", c.bytes.len() as f32 / 1024.0),
                ]
            })
            .collect();
        let some = if n > 0 { On::Always } else { On::Never };
        let c = vec![
            button(
                "c9add",
                [17.0, 46.0, 120.0, 28.0],
                t("pdfEditor9.addFiles"),
                On::Always,
            ),
            button(
                "c9open",
                [145.0, 46.0, 140.0, 28.0],
                t("pdfEditor9.addOpenFiles"),
                On::Always,
            ),
            button(
                "c9remove",
                [293.0, 46.0, 91.0, 28.0],
                t("pdfEditor9.remove"),
                some,
            ),
            button(
                "c9up",
                [392.0, 46.0, 100.0, 28.0],
                t("pdfEditor9.moveUp"),
                if row > 0 { On::Always } else { On::Never },
            ),
            button(
                "c9down",
                [500.0, 46.0, 110.0, 28.0],
                t("pdfEditor9.moveDown"),
                if row + 1 < n { On::Always } else { On::Never },
            ),
            Ctl::Table {
                r: [17.0, 88.0, 899.0, 330.0],
                cols: vec![
                    (t("pdfEditor9.colName"), 380.0),
                    (t("pdfEditor9.colRange"), 220.0),
                    (t("pdfEditor9.colPages"), 120.0),
                    (t("pdfEditor9.colSize"), 177.0),
                ],
                rows,
            },
            label(17.0, 446.0, t("pdfEditor9.pageRange")),
            crate::editor_form8::edit("c9range", [123.0, 432.0, 260.0, 28.0], some),
            button(
                "c9set",
                [391.0, 432.0, 91.0, 28.0],
                t("pdfEditor9.apply"),
                some,
            ),
            button(
                "ok",
                [725.0, 487.0, 91.0, 28.0],
                t("pdfEditor9.combine"),
                if n > 0 { On::Always } else { On::Never },
            ),
            button(
                "cancel",
                [825.0, 487.0, 91.0, 28.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let range = self
            .combine9
            .get(row)
            .map(|c| c.range.clone())
            .unwrap_or_default();
        let f = Form::new(t("pdfEditor9.combineTitle"), (933.0, 540.0), c)
            .with("row", Val::Pick(row))
            .with("c9range", Val::Text(range));
        self.form8.push((f, FormWhat::Combine9));
    }

    fn combine9_row(&self) -> usize {
        self.form8.last().map_or(0, |f| f.0.pick("row"))
    }

    /// A button of Combine Files; true when the top dialog is Combine Files.
    pub(crate) fn combine9_button(&mut self, id: &str) -> bool {
        if !matches!(self.form8.last(), Some((_, FormWhat::Combine9)))
            || id == "ok"
            || id == "cancel"
        {
            return false;
        }
        let row = self.combine9_row();
        match id {
            "row" => {
                let range = self
                    .combine9
                    .get(row)
                    .map(|c| c.range.clone())
                    .unwrap_or_default();
                self.show_combine9(row);
                if let Some((f, _)) = self.form8.last_mut() {
                    f.set("c9range", Val::Text(range));
                }
            }
            "c9add" => self.push(ReaderRequest::PickFile {
                purpose: "combine9".into(),
                filters: vec![("PDF".into(), "*.pdf".into())],
            }),
            "c9open" => {
                for i in 0..self.r.tab_count() {
                    let name = self.r.doc_tab_name(i).unwrap_or_default().to_owned();
                    if self.combine9.iter().any(|c| c.name == name) {
                        continue;
                    }
                    if let Some(bytes) =
                        self.r.tab_view_mut(i).and_then(|v| v.doc_mut().save().ok())
                    {
                        self.combine9.extend(item(&name, bytes));
                    }
                }
                self.show_combine9(row);
            }
            "c9remove" => {
                if row < self.combine9.len() {
                    self.combine9.remove(row);
                }
                self.show_combine9(row);
            }
            "c9up" if row > 0 => {
                self.combine9.swap(row, row - 1);
                self.show_combine9(row - 1);
            }
            "c9down" if row + 1 < self.combine9.len() => {
                self.combine9.swap(row, row + 1);
                self.show_combine9(row + 1);
            }
            "c9set" => {
                let text = self
                    .form8
                    .last()
                    .map(|f| f.0.text("c9range"))
                    .unwrap_or_default();
                if let Some(c) = self.combine9.get_mut(row) {
                    c.range = text.trim().to_owned();
                }
                self.show_combine9(row);
            }
            _ => {}
        }
        true
    }

    /// A file picked for Combine Files.
    pub(crate) fn combine9_file_picked(&mut self, purpose: &str, name: &str, bytes: &[u8]) -> bool {
        if purpose != "combine9" {
            return false;
        }
        if let Some(c) = item(name, bytes.to_vec()) {
            self.combine9.push(c);
        } else {
            let s = self.t("pdfEditor9.notPdf");
            self.set_status(&s);
        }
        let row = self.combine9.len().saturating_sub(1);
        self.show_combine9(row);
        true
    }

    /// Combine: the ranges in list order as a new document in a tab.
    pub(crate) fn combine9_ok(&mut self, f: &Form) {
        let row = f.pick("row");
        let text = f.text("c9range");
        if let Some(c) = self.combine9.get_mut(row) {
            c.range = text.trim().to_owned();
        }
        let items = std::mem::take(&mut self.combine9);
        match combine(&items) {
            Ok(bytes) => {
                let name = format!("{}.pdf", self.t("pdfEditor9.combinedName"));
                self.push(ReaderRequest::OpenDocument { name, bytes });
            }
            Err(e) => {
                self.combine9 = items;
                self.set_status(&e);
            }
        }
    }

    /// Cancel of Combine Files forgets the list.
    pub(crate) fn combine9_cancel(&mut self) {
        self.combine9.clear();
    }
}

/// 0-based pages of `range` ("" or "all": every page) of a file of `n` pages.
pub(crate) fn range_pages(range: &str, n: usize) -> Vec<usize> {
    crate::editor_more::parse_pages(range, "all", n)
        .into_iter()
        .filter(|&p| p < n)
        .collect()
}

/// A new PDF of the items' page ranges in order.
pub(crate) fn combine(items: &[CombineItem]) -> Result<Vec<u8>, String> {
    let first = items.first().ok_or("no files")?;
    let mut doc = bo_pdfread::PdfDocument::open(first.bytes.clone(), bo_pdfread::standard_fonts())?;
    let keep = range_pages(&first.range, first.pages);
    if keep.is_empty() {
        return Err(format!("{}: no pages in the range", first.name));
    }
    let drop: Vec<usize> = (0..first.pages).filter(|p| !keep.contains(p)).collect();
    if !drop.is_empty() {
        doc.delete_pages(&drop)?;
    }
    for c in &items[1..] {
        let pick = range_pages(&c.range, c.pages);
        if pick.is_empty() {
            continue;
        }
        let at = doc.page_count();
        doc.insert_pdf(at, &c.bytes, Some(&pick))?;
    }
    doc.save()
}
