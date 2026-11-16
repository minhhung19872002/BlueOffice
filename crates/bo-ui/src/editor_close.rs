//! Closing a changed document asks first, as the reference editor: "Do you want to save
//! changes to … before closing?" with Yes (save, then close), No (close without saving) and
//! Cancel. It covers the tab's close button, Ctrl+W, File › Close and the window's close
//! button (each changed document in turn).
use crate::dialog::DialogButton;
use crate::editor_window::{inside, EditorWindow};
use crate::reader_window::rounded;
use crate::reader_window::ReaderRequest;
use crate::Request;
use crate::{Input, Key, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

/// The question shown (the reference's message box).
#[derive(Debug, Clone)]
pub(crate) struct CloseBox {
    pub what: Closing,
    text: String,
    labels: [String; 3],
    hover: Option<usize>,
    /// Button boxes of the last paint and their answers.
    hits: Vec<([f32; 4], DialogButton)>,
}

impl CloseBox {
    /// Box of button `j` (Yes, No, Cancel) from the last paint.
    #[cfg(test)]
    pub(crate) fn button(&self, j: usize) -> [f32; 4] {
        self.hits.get(j).map_or([0.0; 4], |h| h.0)
    }
}

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
        if self.dialog_open() || self.close_box.is_some() {
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
        let text = self.t("pdfEditor.saveChangesQ").replace("{name}", &name);
        let labels = [
            self.t("pdfEditor.yes"),
            self.t("pdfEditor.no"),
            self.t("pdfEditor.cancel"),
        ];
        self.close_box = Some(CloseBox {
            what,
            text,
            labels,
            hover: None,
            hits: Vec::new(),
        });
    }

    /// Input while the question is shown: it keeps everything but resizing.
    pub(crate) fn close_box_input(&mut self, input: &Input) -> bool {
        let Some(b) = self.close_box.as_mut() else {
            return false;
        };
        let what = b.what;
        let pick = match input {
            Input::Resize { .. } | Input::Scale(_) | Input::Maximized(_) | Input::Tick => {
                return false
            }
            Input::Move { x, y } => {
                b.hover = b.hits.iter().position(|(r, _)| inside(*r, *x, *y));
                return true;
            }
            Input::Press { x, y, .. } => b
                .hits
                .iter()
                .find(|(r, _)| inside(*r, *x, *y))
                .map(|(_, k)| *k),
            Input::Key(Key::Enter | Key::Letter('y'), _) => Some(DialogButton::Ok),
            Input::Key(Key::Letter('n'), _) => Some(DialogButton::Extra(3)),
            Input::Key(Key::Escape, _) => Some(DialogButton::Cancel),
            _ => None,
        };
        if let Some(k) = pick {
            self.close_box = None;
            self.close_answer(what, k);
        }
        true
    }

    /// The question as the reference's message box: caption with the app name and a close
    /// mark, white body, information mark, the text, and Yes / No / Cancel pill buttons at the
    /// bottom right. Measured at 125 % (pixels / 1.25 here): 634 x 154, caption 25.6, mark 25.6
    /// at (28, 50), text 13.4 from x 70.4, buttons 80 x 28 radius 14, 10.4 apart and from the edges.
    pub(crate) fn paint_close_box(&mut self, p: &mut dyn Painter) {
        // Display scale; the host scales UI text by the Windows text size on top.
        let k = self.r.ui_scale();
        let accent = self.r.theme().accent_strong;
        let (ww, wh) = self.r.window_size();
        let Some(b) = self.close_box.as_mut() else {
            return;
        };
        let px = |v: f32| (v * k).round();
        let (w, h) = (px(634.0), px(154.0));
        let x = ((ww - w) / 2.0).round();
        let y = ((wh - h) / 2.0).round();
        // Shadow, body, caption.
        for (d, a) in [(3.0, 18), (2.0, 28), (1.0, 40)] {
            p.fill_rect(
                x - d,
                y - d + 1.0,
                w + 2.0 * d,
                h + 2.0 * d,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x, y, w, h, Rgba(255, 255, 255, 255));
        let cap = px(25.6);
        p.fill_rect(x, y, w, cap, Rgba(243, 243, 243, 255));
        let grey = Rgba(145, 145, 145, 255);
        let small = TextStyle {
            size: px(11.0),
            bold: false,
        };
        p.text("BlueEditor", x + px(6.0), y + px(5.0), small, grey);
        let (cx, cy, r) = (x + w - px(16.0), y + cap / 2.0, px(4.4));
        p.path(
            &[
                PathSeg::M(cx - r, cy - r),
                PathSeg::L(cx + r, cy + r),
                PathSeg::M(cx + r, cy - r),
                PathSeg::L(cx - r, cy + r),
            ],
            None,
            Some((grey, 1.0_f32.max(k))),
        );
        // Information mark (our own drawing: a filled circle with an "i").
        let d = px(25.6);
        let (mx, my) = (x + px(28.0), y + px(50.0));
        p.path(&rounded(mx, my, d, d, d / 2.0), Some(accent), None);
        let white = Rgba(255, 255, 255, 255);
        let sw = px(2.6);
        p.fill_rect(mx + (d - sw) / 2.0, my + px(6.0), sw, sw, white);
        p.fill_rect(mx + (d - sw) / 2.0, my + px(10.5), sw, px(9.0), white);
        let body = TextStyle {
            size: px(13.4),
            bold: false,
        };
        p.text(
            &b.text,
            x + px(70.4),
            (my + d / 2.0 - body.size * 0.66).round(),
            body,
            Rgba(34, 34, 34, 255),
        );
        // Buttons, Yes first; Cancel ends at the right edge.
        b.hits.clear();
        let (bw, bh, gap) = (px(80.0), px(28.0), px(10.4));
        let by = y + h - gap - bh;
        let keys = [
            DialogButton::Ok,
            DialogButton::Extra(3),
            DialogButton::Cancel,
        ];
        for (j, (label, key)) in b.labels.iter().zip(keys).enumerate() {
            let left = 3.0 - j as f32;
            let bx = x + w - gap - left * bw - (left - 1.0) * gap;
            let shape = rounded(bx, by, bw, bh, bh / 2.0);
            let hot = b.hover == Some(j);
            let dark = Rgba(34, 34, 34, 255);
            let (fill, edge, ink) = match (j, hot) {
                (0, _) => (accent, accent, white),
                (_, true) => (Rgba(243, 243, 243, 255), Rgba(190, 190, 190, 255), dark),
                _ => (white, Rgba(213, 213, 213, 255), dark),
            };
            p.path(&shape, Some(fill), Some((edge, 1.0)));
            let tw = label.chars().count() as f32 * body.size * 0.5;
            p.text(
                label,
                (bx + (bw - tw) / 2.0).round(),
                by + (bh - body.size * 1.33) / 2.0,
                body,
                ink,
            );
            b.hits.push(([bx, by, bw, bh], key));
        }
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
