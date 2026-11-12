//! BlueDoc style management commands (DOC-STYLE-001): the Styles pane's per-style
//! actions (modify, new, delete, select all instances, update to match the
//! selection), the Style Inspector, Reveal Formatting and the Organizer between the
//! open document and another `.docx` / `.dotx` (ECMA-376 Part 1, §17.7). Edits are
//! one undo step each (see `bo_edit::styles`); results carry the refreshed style
//! list (`styles`) and the answer in `styleMgr`.

use crate::edit::EditResult;
use crate::{EngineError, Result};
use bo_edit::styles::{ClearLevel, InspectorInfo, RevealItem, StyleSpec, StyleType};
use serde::{Deserialize, Serialize};

/// Organizer action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrganizerAction {
    /// Copy styles to the other side.
    Copy,
    /// Delete styles (content reassigned to their base).
    Delete,
    /// Rename one style (`newName`).
    Rename,
}

/// A style management command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StyleCommand {
    /// Dialog values of a style → `styleMgr.spec`.
    StyleDetails {
        /// Style id.
        id: String,
    },
    /// Modify a style from the dialog.
    StyleModify {
        /// Style id.
        id: String,
        /// New values.
        spec: StyleSpec,
    },
    /// New style (→ `styleMgr.created`); `fromSelection` takes the formatting at the
    /// caret and applies the style to the selection.
    StyleNew {
        /// Dialog values.
        spec: StyleSpec,
        /// Create from the selection.
        #[serde(default)]
        from_selection: bool,
    },
    /// Delete a style; its content takes the style it is based on.
    StyleDelete {
        /// Style id.
        id: String,
    },
    /// Select every instance (→ `matches`, `highlights`; the selection moves to the first).
    StyleSelectAll {
        /// Style id.
        id: String,
    },
    /// Update the style to match the selection.
    StyleUpdateToMatch {
        /// Style id.
        id: String,
    },
    /// Style Inspector at the caret → `styleMgr.inspector`.
    StyleInspector,
    /// Clear one formatting level of the selection (Style Inspector).
    StyleClear {
        /// Level.
        level: ClearLevel,
    },
    /// Reveal Formatting at the caret → `styleMgr.reveal`.
    RevealFormatting,
    /// Organizer: the styles of both documents → `styleMgr.organizer`.
    StyleOrganizer {
        /// The other document (`.docx` / `.dotx` bytes, base64).
        file: String,
    },
    /// Organizer: copy / delete / rename styles. `toFile`: copy from this document
    /// into the file (or delete / rename in the file) and return the changed file in
    /// `styleMgr.organizer.output`; otherwise this document changes (one undo step).
    StyleOrganizerApply {
        /// The other document (base64).
        file: String,
        /// What to do.
        action: OrganizerAction,
        /// Style ids (of the source side for `copy`).
        ids: Vec<String>,
        /// The file is the target.
        #[serde(default)]
        to_file: bool,
        /// New name (`rename`).
        #[serde(default)]
        new_name: Option<String>,
    },
}

const NAMES: &[&str] = &[
    "styleDetails",
    "styleModify",
    "styleNew",
    "styleDelete",
    "styleSelectAll",
    "styleUpdateToMatch",
    "styleInspector",
    "styleClear",
    "revealFormatting",
    "styleOrganizer",
    "styleOrganizerApply",
];

impl StyleCommand {
    /// Parses a JSON command when it is a style management command.
    pub fn parse(v: &serde_json::Value) -> Option<StyleCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !NAMES.contains(&c) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }

    fn edits(&self) -> bool {
        match self {
            StyleCommand::StyleModify { .. }
            | StyleCommand::StyleNew { .. }
            | StyleCommand::StyleDelete { .. }
            | StyleCommand::StyleUpdateToMatch { .. }
            | StyleCommand::StyleClear { .. } => true,
            StyleCommand::StyleOrganizerApply { to_file, .. } => !to_file,
            _ => false,
        }
    }
}

/// One style of an Organizer list.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizerStyle {
    /// Style id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Type.
    #[serde(rename = "type")]
    pub kind: StyleType,
    /// Default of its type.
    pub is_default: bool,
}

/// Organizer state.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizerInfo {
    /// Styles of the open document.
    pub current: Vec<OrganizerStyle>,
    /// Styles of the other file.
    pub file: Vec<OrganizerStyle>,
    /// The changed file (base64 `.docx`/`.dotx`) after a `toFile` action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Ids an action copied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copied: Option<Vec<String>>,
}

/// Answer of a style management command.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleMgrOut {
    /// Dialog values (`styleDetails`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<StyleSpec>,
    /// Id of the style `styleNew` created.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Style Inspector values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inspector: Option<InspectorInfo>,
    /// Reveal Formatting values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reveal: Option<Vec<RevealItem>>,
    /// Organizer lists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizer: Option<OrganizerInfo>,
}

fn organizer_list(doc: &bo_model::Document) -> Vec<OrganizerStyle> {
    let mut v: Vec<OrganizerStyle> = doc
        .styles
        .styles
        .iter()
        .map(|s| OrganizerStyle {
            id: s.id.clone(),
            name: if s.name.is_empty() {
                s.id.clone()
            } else {
                s.name.clone()
            },
            kind: match s.kind {
                bo_model::StyleKind::Paragraph => StyleType::Paragraph,
                bo_model::StyleKind::Character => StyleType::Character,
                bo_model::StyleKind::Table => StyleType::Table,
                bo_model::StyleKind::Numbering => StyleType::Numbering,
            },
            is_default: s.is_default,
        })
        .collect();
    v.sort_by_key(|s| s.name.to_lowercase());
    v
}

fn read_other(file: &str) -> std::result::Result<bo_model::Document, String> {
    let bytes = crate::merge::base64_decode(file).ok_or_else(|| "badFile".to_owned())?;
    bo_docx::read_docx(&bytes).map_err(|_| "badFile".to_owned())
}

impl crate::Engine {
    /// Run a style management command. Failures are reported in `error` (keys
    /// `noStyle`, `defaultStyle`, `badName`, `nameTaken`, `basedOnLoop`, `badFile`,
    /// `readOnly`, `protected`).
    pub fn doc_style_command(&mut self, cmd: StyleCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let read_only = self.sec.read_only;
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let mut out = StyleMgrOut::default();
        let mut matches = None;
        let mut multi: Vec<bo_edit::Selection> = Vec::new();
        let before_sel = s.selection();
        let before_len = s.history_len();
        let key = |e: bo_edit::EditError| match e {
            bo_edit::EditError::Unsupported(k) => k,
            other => other.to_string(),
        };
        let res: std::result::Result<(), String> = if read_only && cmd.edits() {
            Err(crate::security::READ_ONLY.into())
        } else if cmd.edits() && s.forms_locked() {
            Err("protected".into())
        } else {
            match cmd {
                StyleCommand::StyleDetails { id } => s.style_spec(&id).map(|sp| {
                    out.spec = Some(sp);
                }),
                StyleCommand::StyleModify { id, spec } => s.modify_style(&id, &spec).map(|_| ()),
                StyleCommand::StyleNew {
                    spec,
                    from_selection,
                } => s.new_style(&spec, from_selection).map(|id| {
                    out.created = Some(id);
                }),
                StyleCommand::StyleDelete { id } => s.delete_style(&id).map(|_| ()),
                StyleCommand::StyleSelectAll { id } => {
                    let m = s.style_instances(&id);
                    // Every instance is selected (a multiple selection: formatting and
                    // styles then apply to all of them).
                    multi = m
                        .iter()
                        .map(|x| bo_edit::Selection {
                            anchor: bo_edit::Pos::new(x.para, x.start),
                            focus: bo_edit::Pos::new(x.para, x.end),
                        })
                        .collect();
                    if let Some(first) = multi.first() {
                        s.set_selection(*first);
                    }
                    matches = Some(m);
                    Ok(())
                }
                StyleCommand::StyleUpdateToMatch { id } => s.update_style_to_match(&id).map(|_| ()),
                StyleCommand::StyleInspector => Ok(()),
                StyleCommand::StyleClear { level } => s.clear_level(level).map(|_| ()),
                StyleCommand::RevealFormatting => {
                    out.reveal = Some(s.reveal_formatting());
                    Ok(())
                }
                StyleCommand::StyleOrganizer { file } => match read_other(&file) {
                    Ok(other) => {
                        out.organizer = Some(OrganizerInfo {
                            current: organizer_list(s.doc()),
                            file: organizer_list(&other),
                            output: None,
                            copied: None,
                        });
                        Ok(())
                    }
                    Err(e) => Err(bo_edit::EditError::Unsupported(e)),
                },
                StyleCommand::StyleOrganizerApply {
                    file,
                    action,
                    ids,
                    to_file,
                    new_name,
                } => organizer_apply(s, &file, action, &ids, to_file, new_name.as_deref())
                    .map(|o| out.organizer = Some(o))
                    .map_err(bo_edit::EditError::Unsupported),
            }
            .map_err(key)
        };
        let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        if multi.len() > 1 {
            self.multi_sel = multi;
        } else if s.selection() != before_sel || s.history_len() != before_len {
            self.multi_sel.clear();
        }
        let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        out.inspector = Some(s.inspector());
        let gallery = crate::gallery::styles(s.doc());
        let dirty = self.relayout()?;
        let mut r = self.story_result(dirty, res.err())?;
        if !self.multi_sel.is_empty() {
            r.rects = self.multi_rects();
        }
        if let Some(m) = matches {
            r.highlights = Some(self.match_rects(&m));
            r.matches = Some(m);
        }
        r.styles = Some(gallery);
        r.style_mgr = Some(Box::new(out));
        r.collab = self.collab_after_edit();
        Ok(r)
    }
}

fn organizer_apply(
    s: &mut bo_edit::Session,
    file: &str,
    action: OrganizerAction,
    ids: &[String],
    to_file: bool,
    new_name: Option<&str>,
) -> std::result::Result<OrganizerInfo, String> {
    let mut other = read_other(file)?;
    let mut copied = None;
    if to_file {
        match action {
            OrganizerAction::Copy => {
                copied = Some(bo_edit::styles::copy_styles(s.doc(), &mut other, ids));
            }
            OrganizerAction::Delete => {
                for id in ids {
                    bo_edit::styles::delete_style_in(&mut other, id)?;
                }
            }
            OrganizerAction::Rename => {
                let id = ids.first().ok_or_else(|| "noStyle".to_owned())?;
                bo_edit::styles::rename_style_in(&mut other, id, new_name.unwrap_or(""))?;
            }
        }
    } else {
        let key = |e: bo_edit::EditError| match e {
            bo_edit::EditError::Unsupported(k) => k,
            other => other.to_string(),
        };
        match action {
            OrganizerAction::Copy => {
                copied = Some(s.organizer_copy_in(&other, ids).map_err(key)?);
            }
            OrganizerAction::Delete => {
                s.delete_styles(ids).map_err(key)?;
            }
            OrganizerAction::Rename => {
                let id = ids.first().ok_or_else(|| "noStyle".to_owned())?;
                s.rename_style(id, new_name.unwrap_or("")).map_err(key)?;
            }
        }
    }
    let output = if to_file {
        let bytes = bo_docx::write_docx(&other).map_err(|e| e.to_string())?;
        Some(crate::ole::encode(&bytes))
    } else {
        None
    };
    Ok(OrganizerInfo {
        current: organizer_list(s.doc()),
        file: organizer_list(&other),
        output,
        copied,
    })
}
