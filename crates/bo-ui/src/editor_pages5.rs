//! Page grid clipboard and pages dragged between open documents (Organize, round 5): Ctrl+C /
//! Ctrl+X copy or cut the selected pages as a PDF kept by the window, Ctrl+V inserts them after
//! the selection of whichever document is active, and pages dropped on another document's tab
//! are copied to its end. Each change is one undo step of the document it changes.
use crate::editor_window::EditorWindow;
use bo_pdfread::SaveOptions;

impl EditorWindow {
    /// Copy (or cut) the selected pages of the grid.
    pub(crate) fn copy_pages(&mut self, cut: bool) {
        let pages = self.selected_pages();
        if pages.is_empty() {
            return;
        }
        let Some(v) = self.r.view_mut() else { return };
        let n = v.page_count();
        match v.doc().extract_pages(&pages, SaveOptions::default()) {
            Ok(bytes) => {
                self.page_clip = Some(bytes);
                // A document keeps at least one page.
                if cut && pages.len() < n {
                    let r = v.doc_mut().delete_pages(&pages);
                    if let Err(e) = r {
                        self.r.set_status(&e);
                        return;
                    }
                    self.after_doc_edit();
                    let first = pages[0].min(n - pages.len() - 1);
                    self.org.select_range(first, first);
                }
                let msg = format!("{} {}", pages.len(), self.t("pdfEditor.pagesCopied"));
                self.r.set_status(&msg);
            }
            Err(e) => self.r.set_status(&e),
        }
    }

    /// Insert the copied pages after the selection (or the current page).
    pub(crate) fn paste_pages(&mut self) {
        let Some(bytes) = self.page_clip.clone() else {
            return;
        };
        let at = self.target_pages().last().map_or(0, |p| p + 1);
        let Some(v) = self.r.view_mut() else { return };
        match v.doc_mut().insert_pdf(at, &bytes, None) {
            Ok(n) => {
                self.after_doc_edit();
                self.org.select_range(at, at + n.saturating_sub(1));
            }
            Err(e) => self.r.set_status(&e),
        }
    }

    /// Copy the selected pages to the end of document tab `tab`.
    pub(crate) fn copy_pages_to_tab(&mut self, tab: usize) {
        let pages = self.selected_pages();
        let Some(v) = self.r.view() else { return };
        let bytes = match v.doc().extract_pages(&pages, SaveOptions::default()) {
            Ok(b) => b,
            Err(e) => {
                self.r.set_status(&e);
                return;
            }
        };
        let name = self.r.tab_name(tab).unwrap_or_default().to_owned();
        let Some(other) = self.r.tab_view_mut(tab) else {
            return;
        };
        match other.doc_mut().append_pdf(&bytes) {
            Ok(n) => {
                other.document_changed();
                let msg = format!("{name}: +{n}");
                self.r.set_status(&msg);
            }
            Err(e) => self.r.set_status(&e),
        }
    }
}
