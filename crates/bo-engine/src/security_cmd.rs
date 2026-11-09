//! Security commands of BlueDoc and BlueSlide (CORE-SEC-001, see
//! [`crate::security`] and [`crate::perm`]).

use crate::edit::EditResult;
use crate::perm;
use crate::security::{
    has_password, pptx_verifier, set_pptx_verifier, verify_password, FileSecurity, LabelSpec,
    SecurityCommand, SecurityInfo, DOC_PROTECTED, MARK_AS_FINAL, READ_ONLY, UNSUPPORTED_HASH,
    WRITE_PASSWORD_REQUIRED,
};
use crate::sheet::protect::{get, WRONG_PROTECTION_PASSWORD};
use crate::slide_edit::{SlideCommand, SlideEditResult};
use crate::{EngineError, Result};
use bo_edit::Selection;
use bo_model::{Alignment, Block, ParaProps, Paragraph, Raw, Run, RunProps};
use bo_opc::custom::CustomProperty;

/// Trimmed, non-empty, de-duplicated names.
pub(crate) fn clean_names(names: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in names {
        let n = n.trim().to_owned();
        if !n.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&n)) {
            out.push(n);
        }
    }
    out
}

/// Result of the file-level commands every format shares; `Err` = refused
/// with that message. `write`: (attributes of the password to modify, the
/// legacy attribute name).
pub(crate) fn common_command(
    sec: &mut FileSecurity,
    identity: &mut Vec<String>,
    cmd: &SecurityCommand,
    write: Option<(&crate::sheet::protect::Attrs, &str)>,
) -> std::result::Result<bool, String> {
    let write_locked =
        |sec: &FileSecurity| write.is_some_and(|(a, _)| has_password(a)) && !sec.write_unlocked;
    match cmd {
        SecurityCommand::SetIdentity { names } => {
            *identity = clean_names(names.clone());
            sec.skeleton = None;
            Ok(true)
        }
        SecurityCommand::SetReadOnly { on } => {
            if *on {
                sec.read_only = true;
            } else if write_locked(sec) {
                return Err(WRITE_PASSWORD_REQUIRED.into());
            } else if sec.marked_final() {
                // Editing a final file removes the mark (as "edit anyway").
                sec.set_prop(MARK_AS_FINAL, None);
                sec.read_only = false;
            } else {
                sec.read_only = false;
            }
            Ok(true)
        }
        SecurityCommand::UnlockWrite { password } => {
            let Some((a, legacy)) = write else {
                sec.write_unlocked = true;
                sec.read_only = sec.marked_final();
                return Ok(true);
            };
            match verify_password(a, legacy, password) {
                Some(true) => {
                    sec.write_unlocked = true;
                    sec.read_only = sec.marked_final();
                    Ok(true)
                }
                Some(false) => Err(WRONG_PROTECTION_PASSWORD.into()),
                None => Err(UNSUPPORTED_HASH.into()),
            }
        }
        SecurityCommand::MarkFinal { on } => {
            if *on {
                sec.set_prop(
                    MARK_AS_FINAL,
                    Some(CustomProperty::boolean(MARK_AS_FINAL, true)),
                );
                sec.read_only = true;
            } else {
                sec.set_prop(MARK_AS_FINAL, None);
                sec.read_only = write_locked(sec);
            }
            Ok(true)
        }
        SecurityCommand::SetLabel { label } => {
            let clean = label
                .as_ref()
                .map(|l| LabelSpec {
                    id: l.id.clone().filter(|i| !i.trim().is_empty()),
                    name: l.name.trim().to_owned(),
                    marking: l.marking.clone().filter(|m| !m.trim().is_empty()),
                })
                .filter(|l| !l.name.is_empty());
            sec.set_label(clean.as_ref());
            Ok(true)
        }
        _ => Ok(false),
    }
}

impl crate::Engine {
    /// Reads the security metadata of the file just opened; it opens read-only
    /// when it is final, recommends it, or has a password to modify.
    pub(crate) fn open_security(&mut self, bytes: &[u8]) {
        let mut sec = FileSecurity::read(bytes);
        let (recommended, write) = match &self.pres {
            Some(p) => (false, pptx_verifier(p).is_some_and(|a| has_password(&a))),
            None => (sec.docx_recommended(), sec.docx_write_password()),
        };
        sec.read_only = sec.opens_read_only(recommended, write);
        self.slides.read_only = sec.read_only;
        self.sec = sec;
    }

    /// Protection state of the open document or presentation.
    pub fn security_info(&self) -> SecurityInfo {
        let (recommended, write_password, regions, rects) =
            match (&self.pres, self.session.as_ref()) {
                (Some(p), _) => (
                    false,
                    pptx_verifier(p).is_some_and(|a| has_password(&a)),
                    Vec::new(),
                    Vec::new(),
                ),
                (None, Some(s)) => {
                    let regions = perm::regions(s.doc(), &self.identity);
                    let rects = if self.sec.show_regions {
                        let layout = self.layout.document();
                        regions
                            .iter()
                            .filter(|r| r.mine)
                            .flat_map(|r| {
                                crate::nav::selection_rects(
                                    layout,
                                    Selection {
                                        anchor: r.start,
                                        focus: r.end,
                                    },
                                )
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    (
                        self.sec.docx_recommended(),
                        self.sec.docx_write_password(),
                        regions,
                        rects,
                    )
                }
                (None, None) => (false, false, Vec::new(), Vec::new()),
            };
        SecurityInfo {
            marked_final: self.sec.marked_final(),
            read_only_recommended: recommended,
            write_password,
            read_only: self.sec.read_only,
            label: self.sec.label(),
            foreign_labels: self.sec.foreign().to_vec(),
            protection: self.sec.doc_protection(),
            regions,
            region_rects: rects,
            show_regions: self.sec.show_regions,
            identity: self.identity.clone(),
        }
    }

    /// Runs a security command on the open document (BlueDoc).
    pub fn doc_security(&mut self, cmd: SecurityCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        if self.session.is_none() {
            return Err(EngineError::NoDocument);
        }
        let mut error: Option<String> = None;
        let mut dirty = Vec::new();
        if self.sec.read_only && !cmd.is_view() {
            error = Some(READ_ONLY.into());
        } else {
            let write = self.sec.write.clone();
            let old_marking = self.sec.label().and_then(|l| l.marking);
            match common_command(
                &mut self.sec,
                &mut self.identity,
                &cmd,
                write.as_ref().map(|a| (a, "")),
            ) {
                Err(e) => error = Some(e),
                Ok(true) => {
                    if matches!(cmd, SecurityCommand::SetLabel { .. }) {
                        let new = self.sec.label().and_then(|l| l.marking);
                        dirty = self.apply_label_marking(old_marking.as_deref(), new.as_deref())?;
                    }
                }
                Ok(false) => {
                    if let Err(e) = self.doc_only(cmd, &mut dirty) {
                        error = Some(match e {
                            EngineError::BadCommand(m) => m,
                            other => other.to_string(),
                        });
                    }
                }
            }
        }
        let mut r = self.edit_result(dirty, (None, None, None), error)?;
        r.security = Some(self.security_info());
        r.collab = self.collab_after_edit();
        Ok(r)
    }

    fn doc_only(&mut self, cmd: SecurityCommand, dirty: &mut Vec<usize>) -> Result<()> {
        use SecurityCommand as S;
        let enforced = self.sec.regions_enforced();
        match cmd {
            S::SetReadOnlyRecommended { on } => self.sec.set_docx_write(Some(on), None)?,
            S::SetWritePassword { password } => {
                self.sec.set_docx_write(None, Some(&password))?;
                self.sec.write_unlocked = true;
            }
            S::RestrictEditing { password } => self.sec.protect_doc(&password)?,
            S::StopRestrictEditing { password } => self.sec.unprotect_doc(&password)?,
            S::ShowEditableRegions { on } => self.sec.show_regions = on,
            S::NextEditableRegion { forward } => self.next_region(forward),
            S::AddEditableRegion { editor } => {
                if enforced {
                    return Err(EngineError::BadCommand(DOC_PROTECTED.into()));
                }
                let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
                let id = next_perm_id(s.doc());
                // §17.13.7.2 permStart: w:edGrp (everyone) or w:ed (one user).
                let who = match editor
                    .map(|e| e.trim().to_owned())
                    .filter(|e| !e.is_empty())
                {
                    Some(e) => format!("w:ed=\"{}\"", bo_opc::xml::escape(&e)),
                    None => "w:edGrp=\"everyone\"".to_owned(),
                };
                let start = Raw::new(format!("<w:permStart w:id=\"{id}\" {who}/>"), 0);
                let end = Raw::new(format!("<w:permEnd w:id=\"{id}\"/>"), 0);
                s.wrap_selection(start, end)
                    .map_err(|e| EngineError::BadCommand(e.to_string()))?;
                *dirty = self.relayout()?;
            }
            S::RemoveEditableRegion { id } => {
                if enforced {
                    return Err(EngineError::BadCommand(DOC_PROTECTED.into()));
                }
                let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
                s.remove_markup(&|xml| {
                    perm::perm_mark(xml).is_some_and(|(_, a)| get(&a, "id") == Some(id.as_str()))
                })
                .map_err(|e| EngineError::BadCommand(e.to_string()))?;
                *dirty = self.relayout()?;
            }
            S::SetLabel { .. }
            | S::Security
            | S::SetIdentity { .. }
            | S::SetReadOnly { .. }
            | S::UnlockWrite { .. }
            | S::MarkFinal { .. } => {}
            S::ProtectedRanges { .. }
            | S::AddProtectedRange { .. }
            | S::RemoveProtectedRange { .. }
            | S::UnlockProtectedRange { .. } => {
                return Err(EngineError::BadCommand("not a workbook".into()));
            }
        }
        Ok(())
    }

    /// Selects the next (or previous) region the user may edit, wrapping around.
    fn next_region(&mut self, forward: bool) {
        let Some(s) = self.session.as_mut() else {
            return;
        };
        let mine: Vec<perm::Region> = perm::regions(s.doc(), &self.identity)
            .into_iter()
            .filter(|r| r.mine)
            .collect();
        let (a, _) = s.selection().ordered();
        let pick = if forward {
            mine.iter().find(|r| r.start > a).or(mine.first())
        } else {
            mine.iter().rev().find(|r| r.start < a).or(mine.last())
        };
        if let Some(r) = pick {
            s.set_selection(Selection {
                anchor: r.start,
                focus: r.end,
            });
        }
    }

    /// Header marking of a label (BlueDoc): replaces the previous marking
    /// paragraph of each section's own default header (one undo step each).
    pub(crate) fn apply_label_marking(
        &mut self,
        old: Option<&str>,
        new: Option<&str>,
    ) -> Result<Vec<usize>> {
        if old == new {
            return Ok(Vec::new());
        }
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let n = s.doc().sections.len();
        for si in 0..n {
            let sec = &s.doc().sections[si];
            let own = sec.headers.default.clone();
            if si > 0 && own.is_none() {
                continue;
            }
            let mut blocks = own.clone().unwrap_or_default();
            if let Some(o) = old.filter(|o| !o.is_empty()) {
                blocks.retain(
                    |b| !matches!(b, Block::Paragraph(p) if p.plain_text().trim() == o.trim()),
                );
            }
            if let Some(m) = new.filter(|m| !m.trim().is_empty()) {
                let p = Paragraph::new(
                    ParaProps {
                        align: Some(Alignment::Center),
                        ..ParaProps::default()
                    },
                    vec![Run::text(
                        m.trim(),
                        RunProps {
                            bold: Some(true),
                            ..RunProps::default()
                        },
                    )],
                );
                blocks.insert(0, Block::Paragraph(p));
            }
            if own.is_none() && blocks.is_empty() {
                continue;
            }
            if blocks.is_empty() {
                blocks.push(Block::Paragraph(Paragraph::default()));
            }
            s.set_header_footer(si, true, bo_edit::section::HfVariant::Default, blocks)
                .map_err(|e| EngineError::BadCommand(e.to_string()))?;
        }
        self.relayout()
    }

    /// Runs a security command on the open presentation (BlueSlide).
    pub fn slide_security(&mut self, cmd: SecurityCommand) -> Result<SlideEditResult> {
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let verifier = pptx_verifier(pres);
        let mut error: Option<String> = None;
        if self.sec.read_only && !cmd.is_view() {
            error = Some(READ_ONLY.into());
        } else {
            match common_command(
                &mut self.sec,
                &mut self.identity,
                &cmd,
                verifier.as_ref().map(|a| (a, "")),
            ) {
                Err(e) => error = Some(e),
                Ok(true) => {}
                Ok(false) => match cmd {
                    SecurityCommand::SetWritePassword { password } => {
                        if let Some(p) = self.pres.as_mut() {
                            set_pptx_verifier(p, &password)?;
                            self.sec.write_unlocked = true;
                            self.sec.model_changed = true;
                        }
                    }
                    SecurityCommand::Security => {}
                    // PresentationML defines no read-only recommendation
                    // (§19.2.1.19 holds only the password to modify).
                    _ => error = Some("unsupported".into()),
                },
            }
        }
        self.slides.read_only = self.sec.read_only;
        let mut r = self.slide_command(SlideCommand::State)?;
        r.modified |= self.sec.dirty();
        if r.error.is_none() {
            r.error = error;
        }
        r.security = Some(self.security_info());
        Ok(r)
    }
}

/// Next numeric `w:id` for a permission range.
fn next_perm_id(doc: &bo_model::Document) -> u64 {
    let mut max = 0;
    for path in bo_model::index::body_paragraphs(doc) {
        if let Some(p) = bo_model::index::para_at(doc, &path) {
            for r in &p.runs {
                for c in &r.content {
                    if let bo_model::RunContent::Markup(raw) = c {
                        if let Some((_, a)) = perm::perm_mark(&raw.xml) {
                            if let Some(n) = get(&a, "id").and_then(|v| v.parse::<u64>().ok()) {
                                max = max.max(n);
                            }
                        }
                    }
                }
            }
        }
    }
    max + 1
}
