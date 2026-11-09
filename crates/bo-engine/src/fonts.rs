//! Font picker entries: the loaded (bundled) families, the installed OS fonts
//! (desktop), plus common document fonts
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
        // Installed OS fonts (desktop): loaded when first used.
        for fam in self.fonts.system_families() {
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
            // Named without loading installed fonts (they load when a document uses them).
            let sub = self.fonts.display_family(name);
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
        let mut r = self.special_command_inner(cmd)?;
        // Edits of headers, footers and notes may record tracked changes (§17.13.5):
        // the review pane follows them.
        if let (Some(r), Some(s)) = (r.as_mut(), self.session.as_ref()) {
            if r.review.is_none() {
                r.review = Some(crate::review::review_state(s, self.markup_options()));
            }
        }
        Ok(r)
    }

    fn special_command_inner(&mut self, cmd: &Command) -> crate::Result<Option<EditResult>> {
        if let Some(r) = self.spell_command(cmd)? {
            return Ok(Some(r));
        }
        if let Some(r) = self.inspect_command(cmd)? {
            return Ok(Some(r));
        }
        if matches!(cmd, Command::Equation) && self.session.is_some() {
            return Ok(Some(self.query_result()?));
        }
        if self.session.is_none() {
            return Ok(None);
        }
        if let Some(r) = self.view_command(cmd)? {
            return Ok(Some(r));
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
        if let Some(r) = self.design_command(cmd)? {
            return Ok(Some(r));
        }
        if let Some(r) = self.page_layout_command(cmd)? {
            return Ok(Some(r));
        }
        if let Some(r) = self.template_command(cmd)? {
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
            Command::Click { page, y, count, .. }
                if *count >= 2 && self.view_layout().mode == crate::views::ViewMode::Print =>
            {
                match self.hf_zone(*page, *y) {
                    Some(header) => {
                        self.open_header_footer(*page, header)?;
                        Ok(Some(self.story_result(Vec::new(), None)?))
                    }
                    None => Ok(None),
                }
            }
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
            Command::EquationTemplates => {
                let mut r = self.edit_result(Vec::new(), (None, None, None), None)?;
                r.templates = Some(
                    crate::equation::TEMPLATES
                        .iter()
                        .map(|(id, linear)| crate::edit::EquationTemplate {
                            id: (*id).to_owned(),
                            linear: (*linear).to_owned(),
                        })
                        .collect(),
                );
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
