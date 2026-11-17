//! BlueEditor round 11: the Share tab, laid out as the reference editor's (black box at 125 %):
//! Email ▾ | Send for Shared Review ▾, Manage Shared Review ▾, Send for Email Review, Manage
//! Email Review ▾, Tracker. The reference's buttons for third-party note-taking and
//! document-management services are not offered (their names are other companies' brands);
//! its review services need an online account, so they stay grey here as in BlueReader.
use crate::editor_window::EditorWindow;
use crate::reader_window::Kind;

/// Ribbon items of the Share tab.
pub(crate) fn share_tab_items() -> Vec<(&'static str, Kind)> {
    use Kind::{Big, Sep};
    vec![
        ("sep1", Sep),
        ("share.mailMenu", Big("mail", "pdfEditor11.email", true)),
        ("sep2", Sep),
        (
            "share.sharedReview",
            Big("sharedReview", "pdfShare12.sharedReview", true),
        ),
        (
            "share.manageShared",
            Big("sharedReview", "pdfEditor11.manageShared", true),
        ),
        (
            "share.emailReview",
            Big("emailReview", "pdfEditor11.sendEmailReview", false),
        ),
        (
            "share.manageEmail",
            Big("emailReview", "pdfEditor11.manageEmail", true),
        ),
        ("share.tracker", Big("tracker", "pdfShare12.tracker", false)),
        ("sep3", Sep),
    ]
}

/// Share tab commands shown grey (online review services).
pub(crate) const SHARE_LATER: [&str; 5] = [
    "share.sharedReview",
    "share.manageShared",
    "share.emailReview",
    "share.manageEmail",
    "share.tracker",
];

impl EditorWindow {
    /// Commands of the Share tab; false when `cmd` is not one.
    pub(crate) fn share11_command(&mut self, cmd: &str) -> bool {
        match cmd {
            "share.mailMenu" => self.menu11(
                &[
                    (
                        "share.mail",
                        "pdfEditor11.emailCurrent",
                        Some("mail"),
                        false,
                    ),
                    ("share.mailAll", "pdfEditor11.emailAll", None, true),
                ],
                &[],
                "share.mailMenu",
            ),
            _ => return false,
        }
        true
    }
}
