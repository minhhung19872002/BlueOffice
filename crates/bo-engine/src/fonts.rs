//! Font picker entries: the loaded (bundled) families plus common document fonts
//! that are displayed with a metric-compatible substitute (product brief §2,
//! `bo_text` fallback table).

use crate::edit::{Command, EditResult};
use crate::Engine;
use bo_text::FaceId;
use serde::Serialize;

/// One font picker entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontChoice {
    /// Family name written to the document (§17.3.2.26 rFonts).
    pub name: String,
    /// Loaded family used to display it when the font itself is not available.
    pub substitute: Option<String>,
    /// The family itself is loaded.
    pub available: bool,
}

/// Document fonts commonly found in files from other applications.
const COMMON: &[&str] = &[
    "Times New Roman",
    "Arial",
    "Calibri",
    "Cambria",
    "Helvetica",
    "Georgia",
    "Verdana",
    "Tahoma",
    "Segoe UI",
    "Courier New",
];

impl Engine {
    /// Font picker entries: loaded families first, then common names with the family
    /// that displays them.
    pub fn font_choices(&mut self) -> Vec<FontChoice> {
        let mut out: Vec<FontChoice> = Vec::new();
        for i in 0..self.fonts.len() {
            let fam = self.fonts.info(FaceId(i as u32)).family.clone();
            if !out.iter().any(|c| c.name.eq_ignore_ascii_case(&fam)) {
                out.push(FontChoice {
                    name: fam,
                    substitute: None,
                    available: true,
                });
            }
        }
        out.sort_by_key(|c| c.name.to_lowercase());
        for name in COMMON {
            if out.iter().any(|c| c.name.eq_ignore_ascii_case(name)) {
                continue;
            }
            let sub = self
                .fonts
                .resolve(name, false, false)
                .map(|id| self.fonts.info(id).family.clone());
            out.push(FontChoice {
                name: (*name).to_owned(),
                substitute: sub,
                available: false,
            });
        }
        out
    }

    /// Commands answered without touching the document model (header/footer editor,
    /// notes and font lists). `None` = run the command normally.
    pub(crate) fn special_command(&mut self, cmd: &Command) -> crate::Result<Option<EditResult>> {
        if self.session.is_none() {
            return Ok(None);
        }
        if self.hf.is_some() {
            if let Some(r) = self.story_command(cmd)? {
                return Ok(Some(r));
            }
        }
        if self.note.is_some() {
            if let Some(r) = self.note_command(cmd)? {
                return Ok(Some(r));
            }
        }
        if let Some(r) = self.drawing_command(cmd)? {
            return Ok(Some(r));
        }
        match cmd {
            // A click on a footnote/endnote edits it in place (§17.11).
            Command::Click { page, y, .. } if self.note_at(*page, *y).is_some() => {
                if let Some((endnote, id)) = self.note_at(*page, *y) {
                    self.open_note(endnote, id, *page)?;
                }
                self.note_command(cmd)
            }
            Command::EditHeaderFooter { page, header } => {
                self.open_header_footer(*page, *header)?;
                Ok(Some(self.story_result(Vec::new(), None)?))
            }
            Command::Click { page, y, count, .. } if *count >= 2 => match self.hf_zone(*page, *y) {
                Some(header) => {
                    self.open_header_footer(*page, header)?;
                    Ok(Some(self.story_result(Vec::new(), None)?))
                }
                None => Ok(None),
            },
            Command::CloseHeaderFooter => {
                self.close_header_footer()?;
                self.close_note()?;
                Ok(Some(self.edit_result(
                    Vec::new(),
                    (None, None, None),
                    None,
                )?))
            }
            Command::EditNote { endnote, id } => {
                self.close_header_footer()?;
                self.close_note()?;
                match self.open_new_note(*endnote, *id, Vec::new())? {
                    Some(r) => Ok(Some(r)),
                    None => Ok(Some(self.edit_result(
                        Vec::new(),
                        (None, None, None),
                        Some(format!("no note {id}")),
                    )?)),
                }
            }
            Command::Notes => {
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
                r.notes = self.session.as_ref().map(bo_edit::Session::notes);
                Ok(Some(r))
            }
            Command::StyleList => {
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
                r.styles = self
                    .session
                    .as_ref()
                    .map(|s| crate::gallery::styles(s.doc()));
                Ok(Some(r))
            }
            Command::FontList => {
                let fonts = self.font_choices();
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
                r.fonts = Some(fonts);
                Ok(Some(r))
            }
            _ => Ok(None),
        }
    }
}
