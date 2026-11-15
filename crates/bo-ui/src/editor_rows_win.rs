//! Small `ReaderWindow` helpers BlueEditor uses (a child of `reader_window`). Row buttons
//! themselves are laid out and painted by the reader window's shared three-row columns.
use super::ReaderWindow;

impl ReaderWindow {
    /// A reader pointer tool beyond Hand / Select is on (drawing, measure, eraser, area
    /// highlight, stamps, SnapShot): BlueEditor turns its own tools off when one is chosen.
    pub(crate) fn extra_tool_on(&self) -> bool {
        self.tabs4.tool.is_some() || self.draw.tool.is_some()
    }

    /// The status bar message (tests).
    #[cfg(test)]
    pub(crate) fn status_text(&self) -> &str {
        &self.status
    }
}
