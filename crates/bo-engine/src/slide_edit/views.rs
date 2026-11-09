//! BlueSlide views and sections (SLIDE-VIEW-001): the commands of the outline
//! view, the notes page view, the notes / handout master views and the
//! section list, routed from the JSON bridge (ADR-0008).

use super::outline::{self, OutlineEdit};
use super::page_master::{self, PageMasterCommand};
use super::{sections, Outcome, SlideEditResult};
use crate::{Engine, EngineError};
use serde::Deserialize;

/// Outline, notes page and section commands.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ViewCommand {
    /// The deck's outline → `outline`.
    Outline,
    /// Replace the outline (one undoable step).
    ApplyOutline {
        /// Every slide of the edited outline, in order.
        slides: Vec<OutlineEdit>,
        /// Entry to select afterwards.
        #[serde(default)]
        current: Option<usize>,
    },
    /// Notes page geometry and text of every slide → `notesPages`.
    NotesPages,
    /// New section starting at `slide` (default: the current slide).
    AddSection {
        /// Section name.
        name: String,
        /// First slide of the section.
        #[serde(default)]
        slide: Option<usize>,
        /// Name of the section created for the slides before it when the deck
        /// has no sections yet.
        #[serde(default)]
        first_name: Option<String>,
    },
    /// Rename a section.
    RenameSection {
        /// Section index.
        index: usize,
        /// New name.
        name: String,
    },
    /// Remove a section (its slides join the previous one, or are deleted).
    RemoveSection {
        /// Section index.
        index: usize,
        /// Delete the section's slides too.
        #[serde(default)]
        slides: bool,
    },
    /// Remove every section.
    RemoveAllSections,
    /// Move a section and its slides.
    MoveSection {
        /// Section index.
        index: usize,
        /// New position.
        to: usize,
    },
}

/// Command names of [`ViewCommand`] (routing in the JSON bridge).
pub const COMMANDS: &[&str] = &[
    "outline",
    "applyOutline",
    "notesPages",
    "addSection",
    "renameSection",
    "removeSection",
    "removeAllSections",
    "moveSection",
];

/// Is `name` a command of this module or of the notes / handout master views?
pub fn handles(name: &str) -> bool {
    COMMANDS.contains(&name) || page_master::COMMANDS.contains(&name)
}

impl Engine {
    /// Run a view / section / notes-handout-master command given as JSON.
    pub fn slide_view_command_json(&mut self, json: &str) -> crate::Result<SlideEditResult> {
        let name = serde_json::from_str::<serde_json::Value>(json)
            .ok()
            .and_then(|v| v.get("cmd").and_then(|c| c.as_str()).map(str::to_owned))
            .unwrap_or_default();
        if page_master::COMMANDS.contains(&name.as_str()) {
            let cmd: PageMasterCommand =
                serde_json::from_str(json).map_err(|e| EngineError::BadCommand(e.to_string()))?;
            return self.slide_page_master_command(cmd);
        }
        let cmd: ViewCommand =
            serde_json::from_str(json).map_err(|e| EngineError::BadCommand(e.to_string()))?;
        self.slide_view_command(cmd)
    }

    /// Run a notes / handout master view command.
    pub fn slide_page_master_command(
        &mut self,
        cmd: PageMasterCommand,
    ) -> crate::Result<SlideEditResult> {
        let collab = self.slide_collab.is_some();
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            ed.text = None;
            ed.table = None;
            if collab && matches!(cmd, PageMasterCommand::PageMasterView { on: true, .. }) {
                out.error = Some("masterViewCollab".into());
            } else {
                page_master::run(ed, pres, cmd, &mut out);
            }
            ed.clamp(pres);
        }
        let mut r = self.finish_slide_command(out)?;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }

    /// Run an outline / notes page / section command.
    pub fn slide_view_command(&mut self, cmd: ViewCommand) -> crate::Result<SlideEditResult> {
        let mut out = Outcome::default();
        let mut outline_out = false;
        let mut notes_out = false;
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            let mark = ed.history_mark();
            let edits = !matches!(cmd, ViewCommand::Outline | ViewCommand::NotesPages);
            if edits {
                ed.text = None;
                ed.table = None;
            }
            if edits && ed.in_any_master_view() {
                out.error = Some("masterView".into());
            } else {
                match cmd {
                    ViewCommand::Outline => outline_out = true,
                    ViewCommand::NotesPages => notes_out = true,
                    ViewCommand::ApplyOutline { slides, current } => {
                        ed.mutate(pres, &mut out, |tx, sel| {
                            outline::cmd_apply(tx, sel, &slides, current)
                        });
                        outline_out = true;
                    }
                    ViewCommand::AddSection {
                        name,
                        slide,
                        first_name,
                    } => {
                        let first = first_name.unwrap_or_else(|| name.clone());
                        ed.mutate(pres, &mut out, |tx, sel| {
                            sections::cmd_add(tx, sel, &name, slide, &first)
                        });
                    }
                    ViewCommand::RenameSection { index, name } => {
                        ed.mutate(pres, &mut out, |tx, _| {
                            sections::cmd_rename(tx, index, &name)
                        });
                    }
                    ViewCommand::RemoveSection { index, slides } => {
                        ed.mutate(pres, &mut out, |tx, sel| {
                            sections::cmd_remove(tx, sel, index, slides)
                        });
                    }
                    ViewCommand::RemoveAllSections => {
                        ed.mutate(pres, &mut out, |tx, _| sections::cmd_remove_all(tx));
                    }
                    ViewCommand::MoveSection { index, to } => {
                        ed.mutate(pres, &mut out, |tx, sel| {
                            sections::cmd_move(tx, sel, index, to)
                        });
                    }
                }
                if edits {
                    ed.sections_sync(pres, mark, false);
                }
            }
            ed.clamp(pres);
        }
        let mut r = self.finish_slide_command(out)?;
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let normal = !self.slides.in_any_master_view();
        if outline_out && normal {
            r.outline = Some(outline::outline(pres));
        }
        if notes_out && normal {
            r.notes_pages = Some(page_master::notes_pages(pres));
        }
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }
}
