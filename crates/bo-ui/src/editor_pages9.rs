//! BlueEditor round 9: Organize › Delete and Organize › Swap open the reference editor's
//! classic dialogs (black-box at 125 %): "Delete Pages" 453 × 364 px (Page Range group: Page
//! field pre-filled with the chosen pages and a trailing comma, "of N", the sample line with an
//! information mark, Delete: All pages in range / Even pages only / Odd pages only; Show
//! Preview, OK / Cancel) and "Swap Two Pages" 419 × 178 px (Swap / For number boxes with
//! arrows, "of N", Show Preview, OK / Cancel) and, after the file is picked, "Replace Pages"
//! 763 × 311 px (Select pages to replace: From / To of N in this file; Select replacement
//! pages: the file, Browse…, Page field of M with the sample line). The Delete key in the page
//! grid still deletes the chosen pages at once.
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{
    button, check, combo, edit, group, label, rlabel, spin, Ctl, Form, On, Val,
};
use crate::editor_window::EditorWindow;

impl EditorWindow {
    /// Open Delete Pages.
    pub(crate) fn open_delete9(&mut self) {
        let t = |k: &str| self.t(k);
        let n = self
            .r
            .view()
            .map_or(0, crate::ReaderView::page_count)
            .max(1);
        let pages = self.target_pages();
        let c = vec![
            group(t("pdfEditor9.pageRange2"), [21.0, 53.0, 406.0, 224.0]),
            label(38.0, 110.0, t("pdfEditor9.pageR")),
            edit("pages", [94.0, 95.0, 187.0, 29.0], On::Always),
            label(303.0, 110.0, t("pdfEditor9.ofShort")),
            label(369.0, 110.0, n.to_string()),
            label(119.0, 150.0, t("pdfEditor9.sample")),
            Ctl::Info { x: 270.0, y: 149.0 },
            label(39.0, 219.0, t("pdfEditor9.deleteWhich")),
            combo(
                "subset",
                [94.0, 205.0, 187.0, 27.0],
                vec![
                    t("pdfEditor9.allInRange"),
                    t("pdfEditor9.evenOnly"),
                    t("pdfEditor9.oddOnly"),
                ],
                On::Always,
            ),
            check("preview", 21.0, 331.0, t("pdfEditor9.showPreview")),
            button(
                "ok",
                [243.0, 318.0, 91.0, 26.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [343.0, 318.0, 91.0, 26.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let text = format!(
            "{},",
            crate::editor_more::range_text(&pages).replace(' ', "")
        );
        let f = Form::new(t("pdfEditor9.deletePagesTitle"), (453.0, 364.0), c)
            .with("pages", Val::Text(text))
            .with("subset", Val::Pick(0))
            .focused("pages");
        self.form8.push((f, FormWhat::Delete9));
    }

    /// OK of Delete Pages.
    pub(crate) fn delete9_ok(&mut self, f: &Form) {
        let n = self.r.view().map_or(0, crate::ReaderView::page_count);
        let text = f.text("pages");
        let text = text.trim().trim_end_matches(',');
        if text.is_empty() {
            return;
        }
        let mut pages = crate::editor_more::parse_pages(text, "all", n);
        match f.pick("subset") {
            1 => pages.retain(|p| (p + 1) % 2 == 0),
            2 => pages.retain(|p| (p + 1) % 2 == 1),
            _ => {}
        }
        if pages.is_empty() {
            return;
        }
        if pages.len() >= n {
            self.set_status(&self.t("pdfEditor9.cannotDeleteAll"));
            return;
        }
        match self.doc_edit(|d| d.delete_pages(&pages)) {
            Ok(()) => {
                self.after_doc_edit();
                let first = pages.first().copied().unwrap_or(0).min(n - pages.len() - 1);
                self.org.select_range(first, first);
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// Open Swap Two Pages.
    pub(crate) fn open_swap9(&mut self) {
        let t = |k: &str| self.t(k);
        let n = self
            .r
            .view()
            .map_or(0, crate::ReaderView::page_count)
            .max(1);
        let pages = self.target_pages();
        let a = pages.first().copied().unwrap_or(0);
        let b = if pages.len() > 1 {
            pages.last().copied().unwrap_or(a)
        } else {
            a
        };
        #[allow(clippy::cast_precision_loss)]
        let max = n as f32;
        let c = vec![
            rlabel(78.0, 71.0, t("pdfEditor9.swapLabel"), On::Always),
            spin("a", [82.0, 57.0, 75.0, 28.0], 1.0, (1.0, max), On::Always),
            rlabel(219.0, 71.0, t("pdfEditor9.forLabel"), On::Always),
            spin("b", [223.0, 57.0, 75.0, 28.0], 1.0, (1.0, max), On::Always),
            label(312.0, 71.0, t("pdfEditor9.ofShort")),
            label(367.0, 71.0, n.to_string()),
            check("preview", 18.0, 145.0, t("pdfEditor9.showPreview")),
            button(
                "ok",
                [212.0, 132.0, 85.0, 26.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [309.0, 132.0, 91.0, 26.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor9.swapTitle"), (419.0, 178.0), c)
            .with("a", Val::Text((a + 1).to_string()))
            .with("b", Val::Text((b + 1).to_string()))
            .focused("a");
        self.form8.push((f, FormWhat::Swap9));
    }

    /// OK of Swap Two Pages.
    pub(crate) fn swap9_ok(&mut self, f: &Form) {
        let n = self.r.view().map_or(0, crate::ReaderView::page_count);
        let num = |id: &str| f.text(id).trim().parse::<usize>().unwrap_or(0);
        let (a, b) = (num("a"), num("b"));
        if a == 0 || b == 0 || a > n || b > n {
            self.set_status(&self.t("pdfEditor.badPage"));
            return;
        }
        if a == b {
            return;
        }
        match self.doc_edit(|d| d.swap_pages(a - 1, b - 1)) {
            Ok(()) => self.after_doc_edit(),
            Err(e) => self.set_status(&e),
        }
    }

    /// A PDF picked for Replace (or Browse in Replace Pages); false for other purposes.
    pub(crate) fn pages9_file_picked(&mut self, purpose: &str, name: &str, bytes: &[u8]) -> bool {
        match purpose {
            "replacePages" => {
                let pages = self.target_pages();
                let a = pages.first().copied().unwrap_or(0);
                let b = pages.last().copied().unwrap_or(a);
                self.open_replace9(name, bytes, (a + 1).to_string(), (b + 1).to_string());
            }
            "replacePages9" => {
                let (from, to) = match self.form8.last() {
                    Some((f, FormWhat::Replace9(..))) => (f.text("from"), f.text("to")),
                    _ => return true,
                };
                self.form8.pop();
                self.open_replace9(name, bytes, from, to);
            }
            _ => return false,
        }
        true
    }

    /// Open Replace Pages for the picked file.
    fn open_replace9(&mut self, name: &str, bytes: &[u8], from: String, to: String) {
        let Ok(src) = bo_pdfread::PdfDocument::open(bytes.to_vec(), bo_pdfread::standard_fonts())
        else {
            self.set_status(&self.t("pdfEditor9.notPdf"));
            return;
        };
        let m = src.page_count();
        let t = |k: &str| self.t(k);
        let n = self
            .r
            .view()
            .map_or(0, crate::ReaderView::page_count)
            .max(1);
        #[allow(clippy::cast_precision_loss)]
        let max = n as f32;
        let c = vec![
            group(t("pdfEditor9.replaceWhich"), [16.0, 55.0, 362.0, 202.0]),
            label(29.0, 93.0, t("pdfEditor9.replacePagesLabel")),
            rlabel(76.0, 125.0, t("pdfEditor9.fromLabel"), On::Always),
            spin(
                "from",
                [86.0, 110.0, 86.0, 28.0],
                1.0,
                (1.0, max),
                On::Always,
            ),
            rlabel(76.0, 159.0, t("pdfEditor9.toLabel"), On::Always),
            spin("to", [86.0, 145.0, 86.0, 28.0], 1.0, (1.0, max), On::Always),
            label(191.0, 159.0, t("pdfEditor9.ofShort")),
            label(224.0, 159.0, n.to_string()),
            label(48.0, 193.0, t("pdfEditor9.inLabel")),
            label(86.0, 193.0, format!("{}.pdf", self.doc_name())),
            group(t("pdfEditor9.replaceWith"), [385.0, 55.0, 362.0, 202.0]),
            label(398.0, 93.0, t("pdfEditor9.selectFile")),
            label(533.0, 93.0, name.to_owned()),
            button(
                "replace9browse",
                [535.0, 110.0, 120.0, 28.0],
                t("pdfEditor9.browse"),
                On::Always,
            ),
            label(398.0, 159.0, t("pdfEditor9.replaceWithPages")),
            rlabel(482.0, 193.0, t("pdfEditor9.pageR"), On::Always),
            edit("pick", [491.0, 178.0, 166.0, 29.0], On::Always),
            label(676.0, 193.0, t("pdfEditor9.ofShort")),
            label(710.0, 193.0, m.to_string()),
            label(506.0, 222.0, t("pdfEditor9.sample")),
            Ctl::Info { x: 646.0, y: 227.0 },
            check("preview", 16.0, 280.0, t("pdfEditor9.showPreview")),
            button(
                "ok",
                [556.0, 267.0, 90.0, 26.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [657.0, 267.0, 88.0, 26.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor9.replaceTitle"), (763.0, 311.0), c)
            .with("from", Val::Text(from))
            .with("to", Val::Text(to))
            .focused("from");
        self.form8.push((f, FormWhat::Replace9(bytes.to_vec())));
    }

    /// Browse… of Replace Pages: pick another file; true when it was that button.
    pub(crate) fn pages9_button(&mut self, id: &str) -> bool {
        if id != "replace9browse" {
            return false;
        }
        self.push(crate::reader_window::ReaderRequest::PickFile {
            purpose: "replacePages9".into(),
            filters: vec![("PDF (*.pdf)".into(), "*.pdf".into())],
        });
        true
    }

    /// OK of Replace Pages: pages From–To give way to the picked pages (all of the file when
    /// the Page field is empty).
    pub(crate) fn replace9_ok(&mut self, f: &Form, bytes: &[u8]) {
        let n = self.r.view().map_or(0, crate::ReaderView::page_count);
        if n == 0 {
            return;
        }
        let num = |id: &str| f.text(id).trim().parse::<usize>().unwrap_or(1).clamp(1, n) - 1;
        let (a, b) = (num("from"), num("to"));
        let pages: Vec<usize> = (a.min(b)..=a.max(b)).collect();
        let text = f.text("pick");
        let text = text.trim().trim_end_matches(',');
        let pick: Option<Vec<usize>> = if text.is_empty() {
            None
        } else {
            let m = bo_pdfread::PdfDocument::open(bytes.to_vec(), bo_pdfread::standard_fonts())
                .map_or(0, |d| d.page_count());
            Some(crate::editor_more::parse_pages(text, "all", m))
        };
        match self.doc_edit(|d| d.replace_pages(&pages, bytes, pick.as_deref())) {
            Ok(k) => {
                self.after_doc_edit();
                self.org
                    .select_range(pages[0], pages[0] + k.saturating_sub(1));
            }
            Err(e) => self.set_status(&e),
        }
    }
}
