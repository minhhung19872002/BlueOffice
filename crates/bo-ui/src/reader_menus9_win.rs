//! BlueReader round 9 (a child of `reader_window`): the document's context menus, laid out like
//! the reference reader's (measured at 125 %): on an empty part of the page or the canvas (view
//! commands), on selected text (copy, markup) and on a comment (reply, status, properties).
//! Rows carry a shortcut column; separators add space above their row.
use super::{AnnotKind, Fit, Panel, ReaderView, ReaderWindow};
use crate::popup::{Choice, MenuItem};
use crate::{Input, TextMeasure};

/// Shortcuts end this far from the menu's right edge, and the space a separator adds (logical
/// px; the reference: 21 / 8 device px at 125 %).
pub(crate) const KEY_RIGHT: f32 = 16.8;
pub(crate) const SEP_GAP: f32 = 6.4;
/// Least space between a label and its shortcut (logical px; 40 device px at 125 %).
const KEY_GAP: f32 = 32.0;

/// One row: command, label key, shortcut, icon, dimmed.
type Row = (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
    bool,
);

impl ReaderWindow {
    /// Right click in the document area (BlueReader); `Some(redraw)` when it opened a menu.
    pub(crate) fn route_menus9(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        let Input::Context { x, y } = *input else {
            return None;
        };
        if self.profile.is_some()
            || self.show_start
            || !super::inside(self.doc_rect, x, y)
            || self.tabs.is_empty()
        {
            return None;
        }
        let (rows, seps) = if let Some(a) = self.comment_at(x, y) {
            self.extra.selected = Some(a);
            comment_rows()
        } else if self.over_selection9(x, y) {
            selection_rows()
        } else {
            self.view_rows9()
        };
        let items: Vec<MenuItem> = rows
            .iter()
            .map(|&(cmd, key, _, _, dim)| MenuItem {
                label: self.t(key),
                choice: Choice::Command(cmd.to_owned()),
                checked: false,
                dim,
            })
            .collect();
        let icons: Vec<Option<&'static str>> = rows.iter().map(|r| r.3).collect();
        let keys: Vec<&str> = rows.iter().map(|r| r.2).collect();
        let menu = self
            .reader_menu(items, [x, y, 0.0, 0.0])
            .with_decor(icons, seps);
        let menu = self
            .keyed_menu9(menu, &keys, m)
            .with_icon_fn(crate::reader_icons::menu_icon);
        self.menu = Some((menu, "context9"));
        Some(true)
    }

    /// Add measured shortcuts (`keys` per item, "" for none) to a context menu.
    pub(crate) fn keyed_menu9(
        &self,
        menu: crate::popup::Menu,
        keys: &[&str],
        m: &dyn TextMeasure,
    ) -> crate::popup::Menu {
        let st = crate::TextStyle {
            size: (super::READER_MENU.text * self.scale).round(),
            bold: false,
        };
        let keys: Vec<(String, f32)> = keys
            .iter()
            .map(|k| {
                (
                    (*k).to_owned(),
                    if k.is_empty() {
                        0.0
                    } else {
                        m.measure(k, st).0
                    },
                )
            })
            .collect();
        // Widest row: its label, then (with a shortcut) the gap and the shortcut.
        let gap = (KEY_GAP * self.scale).round();
        let content = menu
            .items()
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let kw = keys.get(i).map_or(0.0, |k| k.1);
                m.measure(&it.label, st).0 + if kw > 0.0 { gap + kw } else { 0.0 }
            })
            .fold(0.0_f32, f32::max);
        menu.with_keys(keys, content, KEY_RIGHT, SEP_GAP, self.size)
    }

    /// The point is over the current text selection.
    fn over_selection9(&self, x: f32, y: f32) -> bool {
        let Some(t) = self.tabs.get(self.active) else {
            return false;
        };
        let v = &t.view;
        let Some(sel) = v.selection() else {
            return false;
        };
        let Some(r) = v.page_rect(sel.page) else {
            return false;
        };
        let s = v.px_per_pt();
        v.selection_rects().iter().any(|q| {
            x >= r[0] + q[0] * s - 2.0
                && x <= r[0] + q[2] * s + 2.0
                && y >= r[1] + q[1] * s - 2.0
                && y <= r[1] + q[3] * s + 2.0
        })
    }

    /// The view menu of an empty part of the page: the current fit is ticked.
    fn view_rows9(&self) -> (Vec<Row>, Vec<usize>) {
        let (fit, zoom) = self
            .view()
            .map_or((None, 0.0), |v| (Some(v.fit()), v.zoom()));
        let actual = (fit == Some(Fit::None) && (zoom - 1.0).abs() < 1e-3).then_some("check");
        let tick = |f: Fit| (fit == Some(f)).then_some("check");
        (
            vec![
                ("ctx.paste", "pdfCtx.paste", "Ctrl+V", None, true),
                (
                    "ctx.pasteInPlace",
                    "pdfCtx.pasteInPlace",
                    "Ctrl+Alt+V",
                    None,
                    true,
                ),
                (
                    "view.readMode",
                    "pdfCtx.readMode",
                    "Ctrl+H",
                    Some("readMode"),
                    false,
                ),
                (
                    "view.fullscreen",
                    "pdfCtx.fullScreen",
                    "F11",
                    Some("fullscreen"),
                    false,
                ),
                ("zoom.in", "pdfCtx.zoomIn", "Ctrl++", None, false),
                ("zoom.out", "pdfCtx.zoomOut", "Ctrl+-", None, false),
                ("zoom.actual", "pdfCtx.actualSize", "Ctrl+1", actual, false),
                (
                    "zoom.page",
                    "pdfCtx.fitPage",
                    "Ctrl+0",
                    tick(Fit::Page),
                    false,
                ),
                (
                    "zoom.width",
                    "pdfCtx.fitWidth",
                    "Ctrl+2",
                    tick(Fit::Width),
                    false,
                ),
                (
                    "rotate.right",
                    "pdfCtx.rotateRight",
                    "Ctrl+Shift++",
                    Some("rotateRight"),
                    false,
                ),
                (
                    "view.reverse",
                    "pdfCtx.reverseView",
                    "",
                    Some("reverseView"),
                    false,
                ),
                ("file.print", "pdfCtx.print", "Ctrl+P", Some("print"), false),
                ("view.rulers", "pdfCtx.rulers", "Ctrl+R", None, false),
                ("view.grid", "pdfCtx.guides", "", None, false),
                ("ctx.hidePanels", "pdfCtx.hidePanels", "F4", None, false),
            ],
            vec![2, 4, 6, 9, 10, 11, 12, 14],
        )
    }

    /// Commands of the context menus; true when handled.
    pub(crate) fn command_menus9(&mut self, cmd: &str) -> bool {
        let kind = match cmd {
            "ctx.paste" | "ctx.pasteInPlace" => return true,
            "ctx.hidePanels" => {
                let open = self.panel_open().is_some();
                self.set_panel(if open { None } else { Some(Panel::Bookmarks) });
                return true;
            }
            "sel.highlight" => AnnotKind::Highlight,
            "sel.strikeout" => AnnotKind::StrikeOut,
            "sel.underline" => AnnotKind::Underline,
            "sel.squiggly" => AnnotKind::Squiggly,
            _ => return false,
        };
        if let Some(page) = self.view().and_then(ReaderView::selection).map(|s| s.page) {
            self.markup_selection(kind, page);
        }
        true
    }

    /// Context menu open (tests): its labels.
    #[cfg(test)]
    pub(crate) fn menu_labels9(&self) -> Vec<String> {
        self.menu
            .as_ref()
            .map(|(m, _)| m.items().iter().map(|i| i.label.clone()).collect())
            .unwrap_or_default()
    }
}

/// The menu of selected text.
fn selection_rows() -> (Vec<Row>, Vec<usize>) {
    (
        vec![
            ("edit.copy", "pdfCtx.copy", "Ctrl+C", Some("copy"), false),
            ("edit.selectAll", "pdfCtx.selectAll", "Ctrl+A", None, false),
            ("sel.highlight", "pdfCtx.highlight", "Shift+H", None, false),
            ("sel.strikeout", "pdfCtx.strikeout", "Shift+S", None, false),
            ("sel.underline", "pdfCtx.underline", "Ctrl+U", None, false),
            ("sel.squiggly", "pdfCtx.squiggly", "Shift+U", None, false),
            (
                "comment.replace",
                "pdfCtx.replaceText",
                "Ctrl+Shift+R",
                None,
                false,
            ),
            ("view.wordCount", "pdfCtx.wordCount", "", None, false),
        ],
        vec![1, 2, 7],
    )
}

/// The menu of a comment.
fn comment_rows() -> (Vec<Row>, Vec<usize>) {
    (
        vec![
            ("comment.reply", "pdfCtx.reply", "", None, false),
            ("comment.delete", "pdfCtx.delete", "", None, false),
            ("comment.setStatus", "pdfCtx.setStatus", "", None, false),
            ("comment.mark", "pdfCtx.mark", "", None, false),
            ("comment.edit", "pdfCtx.openPopup", "", None, false),
            ("draw.props", "pdfCtx.properties", "", None, false),
        ],
        vec![1, 2, 4, 5],
    )
}

#[cfg(test)]
mod tests {
    use crate::reader_tests::{pdf, Fixed};
    use crate::reader_window::ReaderWindow;
    use crate::{Input, Key, Mods};
    use bo_pdfread::PdfDocument;

    fn window() -> ReaderWindow {
        let doc = PdfDocument::open(pdf(2), bo_pdfread::standard_fonts()).expect("pdf");
        let mut w = ReaderWindow::new("en-US");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1100.0,
            },
            &Fixed,
        );
        w.open(doc, "test.pdf", None);
        w
    }

    #[test]
    fn page_selection_and_comment_menus() {
        let mut w = window();
        let d = w.doc_area();
        // Empty canvas: the view menu, Fit Width ticked.
        w.handle(
            Input::Context {
                x: d[0] + 6.0,
                y: d[1] + d[3] - 6.0,
            },
            &Fixed,
        );
        let labels = w.menu_labels9();
        assert_eq!(labels.first().map(String::as_str), Some("Paste"));
        assert!(labels.iter().any(|l| l == "Hide Navigation Panels"));
        assert_eq!(labels.len(), 15);
        w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
        // Selected text: copy and markup.
        w.run_command("edit.selectAll", &Fixed);
        let v = w.view().expect("view");
        let sel = v.selection().expect("selection");
        let r = v.page_rect(sel.page).expect("page");
        let q = v.selection_rects()[0];
        let s = v.px_per_pt();
        let (x, y) = (r[0] + (q[0] + 2.0) * s, r[1] + q[1].midpoint(q[3]) * s);
        w.handle(Input::Context { x, y }, &Fixed);
        let labels = w.menu_labels9();
        assert_eq!(labels.first().map(String::as_str), Some("Copy"));
        assert!(labels.iter().any(|l| l == "Squiggly Underline"));
        // Highlight from the menu adds a comment on the selection.
        w.run_command("sel.highlight", &Fixed);
        let n = w.view().expect("view").doc().annotations().len();
        assert_eq!(n, 1);
        // On that comment: reply, status, properties.
        w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
        w.handle(Input::Context { x, y }, &Fixed);
        let labels = w.menu_labels9();
        assert_eq!(labels.first().map(String::as_str), Some("Reply"));
        assert!(labels.iter().any(|l| l == "Properties..."));
    }
}
