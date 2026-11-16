//! BlueReader round 9 (a child of `reader_window`): the Share tab and the Help tab laid out like
//! the reference reader's (black-box, 125 %). Share: e-mail the document, copy its path, show it
//! in its folder (the reference's cloud and review services are not offered). Help: command
//! line help, set as the default PDF reader (Windows' Default apps page), About.
use super::{Kind, ReaderRequest, ReaderWindow};
use crate::dialog::{Dialog, DialogButton, DialogKind};

/// Ribbon tab indexes of BlueReader (File not counted).
pub(crate) const SHARE_TAB: usize = 5;

impl ReaderWindow {
    /// Items of the Share tab after the pointer tools.
    pub(crate) fn share_items9() -> Vec<(&'static str, Kind)> {
        use Kind::{Big, Sep};
        vec![
            ("sep1", Sep),
            ("share.mail", Big("mail", "pdfTabs9.email", false)),
            ("sep2", Sep),
            ("share.copy", Big("copy", "pdfTabs9.copyPath", false)),
            ("share.folder", Big("open", "pdfTabs9.showFolder", false)),
        ]
    }

    /// Items of the Help tab (no pointer tools, like the reference's).
    pub(crate) fn help_items9() -> Vec<(&'static str, Kind)> {
        use Kind::{Big, Sep};
        vec![
            ("help.cli", Big("cmdLine", "pdfTabs9.commandLine", false)),
            ("sep1", Sep),
            (
                "help.default",
                Big("defaultApp", "pdfTabs9.setDefault", false),
            ),
            ("sep2", Sep),
            ("help.about", Big("info", "pdfTabs9.about", false)),
        ]
    }

    /// Commands of the Share and Help tabs; true when handled.
    pub(crate) fn command_tabs9(&mut self, cmd: &str) -> bool {
        match cmd {
            "share.mail" => {
                self.backstage_command8("bs.shareMail");
            }
            "share.copy" => {
                self.backstage_command8("bs.shareCopy");
            }
            "share.folder" => {
                self.backstage_command8("bs.shareFolder");
            }
            "search.advanced" => self.open_search9(),
            "help.default" => self
                .requests
                .push(ReaderRequest::OpenUrl("ms-settings:defaultapps".into())),
            "help.cli" => {
                let lines = [
                    "bluereader-native.exe [file.pdf ...]",
                    "--render in.pdf <page> out.png [zoom%]",
                    "--text in.pdf",
                    "--search in.pdf <words>",
                    "--print in.pdf [--pages 1-3] [--copies n] [--gray] [--nup 2x2]",
                    "--bench in.pdf",
                ];
                let mut text = vec![self.t("pdfTabs9.commandLineIntro")];
                text.extend(lines.iter().map(|s| (*s).to_owned()));
                let d = Dialog::new(
                    DialogKind::WordCount,
                    self.t("pdfTabs9.commandLine"),
                    Vec::new(),
                    vec![(self.t("pdfReader.ok"), DialogButton::Ok)],
                    text,
                );
                self.dialog = Some((d, None));
            }
            _ => return false,
        }
        true
    }

    /// Ribbon labels longer than this many bytes take two lines; BlueReader wraps "Read Mode"
    /// like the reference (round 9), windows built on it keep 9.
    pub(crate) fn wrap_after9(&self) -> usize {
        if self.profile.is_none() {
            8
        } else {
            9
        }
    }

    /// The two lines of a ribbon label: BlueReader splits at the space that balances the lines
    /// (the longer line first on a tie), as the reference does ("Command / Line Help", "Time
    /// Stamp / Document"); windows built on it split at the last space.
    pub(crate) fn split_label9<'a>(&self, text: &'a str) -> (&'a str, &'a str) {
        if self.profile.is_some() {
            return text.rsplit_once(' ').unwrap_or((text, ""));
        }
        balanced_split(text)
    }

    /// Share commands need a saved document.
    pub(crate) fn tabs9_enabled(&self, id: &str) -> Option<bool> {
        let has_path = self.tabs.get(self.active).is_some_and(|t| t.path.is_some());
        match id {
            "share.mail" => Some(self.view().is_some()),
            "share.copy" | "share.folder" => Some(has_path),
            // The reference greys Tool Settings when the document has no form fields.
            "menu.formSettings" => Some(self.has_form9()),
            _ => None,
        }
    }
}

/// Split `text` at the space that makes the longer of the two lines shortest (characters),
/// preferring the longer first line on a tie.
fn balanced_split(text: &str) -> (&str, &str) {
    let mut best: Option<(usize, usize, usize)> = None;
    for (i, _) in text.match_indices(' ') {
        let a = text[..i].chars().count();
        let b = text[i + 1..].chars().count();
        let worst = a.max(b);
        if best.is_none_or(|(w, fa, _)| worst < w || (worst == w && a > fa)) {
            best = Some((worst, a, i));
        }
    }
    best.map_or((text, ""), |(_, _, i)| (&text[..i], &text[i + 1..]))
}

#[cfg(test)]
mod tests {
    #[test]
    fn labels_split_like_the_reference() {
        for (t, a, b) in [
            ("Command Line Help", "Command", "Line Help"),
            ("Set to Default PDF Reader", "Set to Default", "PDF Reader"),
            ("Time Stamp Document", "Time Stamp", "Document"),
            ("Page Fit Option", "Page Fit", "Option"),
            ("Fill & Sign", "Fill &", "Sign"),
            ("Copy File Path", "Copy File", "Path"),
            ("Read Mode", "Read", "Mode"),
        ] {
            assert_eq!(super::balanced_split(t), (a, b), "{t}");
        }
    }

    use crate::reader_tests::{pdf, Fixed};
    use crate::reader_window::{ReaderRequest, ReaderWindow};
    use crate::Input;
    use bo_pdfread::PdfDocument;

    #[test]
    fn share_and_help_tabs() {
        let doc = PdfDocument::open(pdf(1), bo_pdfread::standard_fonts()).expect("pdf");
        let mut w = ReaderWindow::new("en-US");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1100.0,
            },
            &Fixed,
        );
        w.open(doc, "a.pdf", Some("C:/x/a.pdf".into()));
        let tabs: Vec<String> = w
            .tab_label_keys()
            .iter()
            .map(|k| w.strings.get(k).to_owned())
            .collect();
        assert_eq!(
            tabs,
            ["File", "Home", "Comment", "View", "Form", "Protect", "Share", "Help"]
        );
        w.set_ribbon_tab(super::SHARE_TAB, &Fixed);
        assert!(w.control_rect("share.mail").is_some());
        w.run_command("share.copy", &Fixed);
        assert!(w
            .take_requests()
            .iter()
            .any(|r| matches!(r, ReaderRequest::Copy(p) if p.contains("a.pdf"))));
        w.set_ribbon_tab(super::SHARE_TAB + 1, &Fixed);
        assert!(w.control_rect("help.cli").is_some());
        assert!(w.control_rect("tool.hand").is_none());
        w.run_command("help.default", &Fixed);
        assert!(w
            .take_requests()
            .contains(&ReaderRequest::OpenUrl("ms-settings:defaultapps".into())));
    }
}
