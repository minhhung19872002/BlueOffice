//! BlueEditor round 7: File › Batch Print as the reference editor offers it — tick the open
//! documents to print, add more files (opened in tabs so they can be printed), then the Print
//! dialog chooses the printer and its settings once for all of them (the host prints every
//! ticked document whole: `PdfWindow::take_batch`).
use crate::dialog::DialogButton;
use crate::editor_five::FiveAsk;
use crate::editor_window::EditorWindow;
use crate::reader_window::ReaderRequest;
use bo_pdfread::PdfDocument;

impl EditorWindow {
    /// The Batch Print list: every open document ticked.
    pub(crate) fn open_batch_print(&mut self) {
        let mut checks = Vec::new();
        let mut i = 0;
        while let Some(name) = self.r.tab_name(i) {
            checks.push((name.to_owned(), true));
            i += 1;
        }
        let buttons = vec![
            (self.t("pdfBatch.print"), DialogButton::Ok),
            (self.t("pdfEditor7.addFiles"), DialogButton::FindNext),
            (self.t("pdfReader.cancel"), DialogButton::Cancel),
        ];
        self.ask_five(FiveAsk::BatchPrint(i), "pdfBatch.title", checks, buttons);
    }

    /// A button of the Batch Print list (`values`: one tick per open document).
    pub(crate) fn batch_print_answer(&mut self, tabs: usize, b: DialogButton, values: &[String]) {
        match b {
            DialogButton::FindNext => self.push(ReaderRequest::PickFile {
                purpose: "batchPrintAdd".into(),
                filters: vec![("PDF (*.pdf)".into(), "*.pdf".into())],
            }),
            DialogButton::Ok => {
                let picked: Vec<usize> = (0..tabs)
                    .filter(|&i| values.get(i).is_some_and(|s| s == "1"))
                    .collect();
                if picked.is_empty() {
                    return;
                }
                self.r.set_batch(picked);
                self.push(ReaderRequest::Print);
            }
            _ => {}
        }
    }

    /// A PDF added to Batch Print: opened in a tab, then the list again.
    pub(crate) fn batch_print_picked(&mut self, path: &std::path::Path, bytes: &[u8]) {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        match PdfDocument::open(bytes.to_vec(), bo_pdfread::standard_fonts()) {
            Ok(doc) => self.open(doc, &name, Some(path.to_path_buf())),
            Err(e) => self.set_status(&e),
        }
        self.open_batch_print();
    }
}
