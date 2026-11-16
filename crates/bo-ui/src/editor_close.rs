//! Closing a changed document asks first, as the reference editor: "Do you want to save
//! changes to … before closing?" with Yes (save, then close), No (close without saving) and
//! Cancel. It covers the tab's close button, Ctrl+W, File › Close and the window's close
//! button (each changed document in turn).
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::editor_actions::Ask2;
use crate::editor_window::EditorWindow;
use crate::reader_window::ReaderRequest;
use crate::Request;

/// What closes once the question is answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Closing {
    /// Document tab `i`.
    Tab(usize),
    /// The window: tab `i` is the changed document asked about.
    Window(usize),
}

impl EditorWindow {
    fn tab_modified(&mut self, i: usize) -> bool {
        self.r.tab_view_mut(i).is_some_and(|v| v.doc().modified())
    }

    /// `ed.closeTab.N` from the reader window: close tab N, asking first when it changed.
    pub(crate) fn close_tab_command(&mut self, cmd: &str) -> bool {
        let Some(i) = cmd
            .strip_prefix("ed.closeTab.")
            .and_then(|v| v.parse::<usize>().ok())
        else {
            return false;
        };
        self.tw_commit();
        if self.tab_modified(i) {
            self.ask_close(Closing::Tab(i));
        } else {
            self.r.close_tab_now(i);
            self.after_close();
        }
        true
    }

    /// The window is asked to close: false while a changed document needs an answer.
    pub fn close_requested(&mut self) -> bool {
        self.tw_commit();
        if self.dialog_open() {
            return false;
        }
        match (0..self.r.tab_count()).find(|&i| self.tab_modified(i)) {
            Some(i) => {
                self.ask_close(Closing::Window(i));
                false
            }
            None => true,
        }
    }

    fn ask_close(&mut self, what: Closing) {
        let (Closing::Tab(i) | Closing::Window(i)) = what;
        self.r.select_tab(i);
        let name = self.r.doc_tab_name(i).unwrap_or_default().to_owned();
        let question = self.t("pdfEditor.saveChangesQ").replace("{name}", &name);
        let mut d = Dialog::new(
            DialogKind::Comment,
            "BlueEditor".to_owned(),
            Vec::new(),
            vec![
                (self.t("pdfEditor.yes"), DialogButton::Ok),
                (self.t("pdfEditor.no"), DialogButton::Extra(3)),
                (self.t("pdfEditor.cancel"), DialogButton::Cancel),
            ],
            vec![question],
        );
        d.layout(
            &crate::editor_window::Estimate,
            self.r.window_size(),
            self.r.ui_scale(),
        );
        self.dialog2 = Some((d, Ask2::Close(what)));
    }

    /// The answer to the question.
    pub(crate) fn close_answer(&mut self, what: Closing, b: DialogButton) {
        let (Closing::Tab(i) | Closing::Window(i)) = what;
        match b {
            DialogButton::Ok => {
                // Save (the host writes it now), then close: the tab once it is saved
                // (`close_pending`), the window by asking again (the next changed one).
                self.r.select_tab(i);
                self.requests.push(ReaderRequest::Save);
                match what {
                    Closing::Tab(_) => self.close_after_save = Some(i),
                    Closing::Window(_) => self.requests.push(ReaderRequest::Window(Request::Close)),
                }
            }
            DialogButton::Extra(3) => {
                // Close without saving.
                self.r.close_tab_now(i);
                self.after_close();
                if let Closing::Window(_) = what {
                    self.requests.push(ReaderRequest::Window(Request::Close));
                }
            }
            _ => {}
        }
    }

    /// After a Yes: close the tab once the host saved it (a failed or cancelled save keeps it).
    pub(crate) fn close_pending(&mut self) -> bool {
        let Some(i) = self.close_after_save.take() else {
            return false;
        };
        if self.tab_modified(i) {
            return false;
        }
        self.r.close_tab_now(i);
        self.after_close();
        true
    }

    fn after_close(&mut self) {
        self.edit.selected.clear();
        self.edit.typing = None;
        self.edit.page = None;
    }
}
