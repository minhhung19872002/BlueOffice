//! Layout tab commands for BlueDoc: text columns (ECMA-376 Part 1, §17.6.4 cols),
//! line numbering (§17.6.8 lnNumType), automatic hyphenation (§17.15.1.10), the
//! text direction of sections (§17.6.20 textDirection) and drop caps (§17.3.1.11
//! framePr), see `bo_edit::page_layout`.

use crate::edit::{Command, EditResult};
use crate::{Engine, EngineError, Result};
use bo_model::TextDirection;

impl Engine {
    /// Run a Layout tab command (`None`: not one).
    pub(crate) fn page_layout_command(&mut self, cmd: &Command) -> Result<Option<EditResult>> {
        let edit = matches!(
            cmd,
            Command::SetColumns { .. }
                | Command::SetLineNumbering { .. }
                | Command::SetSectionTextDirection { .. }
                | Command::SetHyphenation { .. }
                | Command::SetDropCap { .. }
        );
        if !edit && !matches!(cmd, Command::PageLayout) {
            return Ok(None);
        }
        let mut error = None;
        let mut dirty = Vec::new();
        if edit {
            // Header/footer, note and text box editors mirror into the document:
            // close them before changing it.
            self.close_header_footer()?;
            self.close_note()?;
            self.close_text_box()?;
            let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
            let res = match cmd {
                Command::SetColumns { columns, scope } => s.set_columns(columns, *scope),
                Command::SetLineNumbering { numbering, scope } => {
                    s.set_line_numbering(*numbering, *scope)
                }
                Command::SetSectionTextDirection { dir, scope } => s.set_section_text_direction(
                    dir.as_deref().and_then(TextDirection::parse),
                    *scope,
                ),
                Command::SetHyphenation { hyphenation } => s.set_hyphenation(*hyphenation),
                Command::SetDropCap { drop_cap } => s.set_drop_cap(drop_cap.as_ref()),
                _ => Ok(()),
            };
            if let Err(e) = res {
                error = Some(e.to_string());
            }
            dirty = self.relayout()?;
        }
        let mut r = self.edit_result(dirty, (None, None, None), error)?;
        r.page_layout = self
            .session
            .as_ref()
            .map(|s| Box::new(s.page_layout_state()));
        if edit {
            r.collab = self.collab_after_edit();
        }
        Ok(Some(r))
    }
}
