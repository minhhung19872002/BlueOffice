//! BlueDoc commands of the "Check document" panel over the edit command bridge
//! (ADR-0008): `inspect`, `inspectRemove`, `inspectKeep`, `inspectFix`.

use super::{doc, package, Category, Fix, Report, Target};
use crate::edit::{Command, EditResult};
use crate::{EngineError, Result};

impl crate::Engine {
    /// Parts of the package the open document was read from.
    fn source_scan(&self) -> package::PackageScan {
        let Some(src) = self.document().and_then(|d| d.source.as_ref()) else {
            return package::PackageScan::default();
        };
        match bo_opc::Package::read(&src.bytes, &bo_opc::Limits::default()) {
            Ok(pkg) => package::scan(
                pkg.parts()
                    .iter()
                    .map(|p| (p.name.as_str(), p.data.as_slice())),
            ),
            Err(_) => package::PackageScan::default(),
        }
    }

    /// The report of the open document.
    pub fn inspect_document(&self) -> Result<Report> {
        let doc = self.document().ok_or(EngineError::NoDocument)?;
        Ok(doc::report(doc, &self.source_scan(), &self.scrub))
    }

    fn inspect_result(&mut self, dirty: Vec<usize>, error: Option<String>) -> Result<EditResult> {
        let mut r = self.story_result(dirty, error)?;
        r.inspection = Some(Box::new(self.inspect_document()?));
        if r.collab.is_none() {
            r.collab = self.collab_after_edit();
        }
        Ok(r)
    }

    pub(crate) fn inspect_command(&mut self, cmd: &Command) -> Result<Option<EditResult>> {
        match cmd {
            Command::Inspect => Ok(Some(self.inspect_result(Vec::new(), None)?)),
            Command::InspectRemove { categories } => {
                for c in categories {
                    self.scrub.set(*c, true);
                }
                let model: Vec<Category> = categories
                    .iter()
                    .copied()
                    .filter(|c| {
                        matches!(
                            c,
                            Category::Comments
                                | Category::HiddenText
                                | Category::HeadersFooters
                                | Category::Watermarks
                                | Category::InvisibleObjects
                                | Category::Embedded
                        )
                    })
                    .collect();
                let mut error = None;
                if !model.is_empty() {
                    let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
                    let c = doc::remove(s.doc(), &model);
                    if let Err(e) = s.replace_content_and_notes(
                        c.sections,
                        c.comments,
                        (c.footnotes, c.endnotes),
                    ) {
                        error = Some(e.to_string());
                    }
                }
                let dirty = self.relayout()?;
                Ok(Some(self.inspect_result(dirty, error)?))
            }
            Command::InspectKeep { categories } => {
                for c in categories {
                    self.scrub.set(*c, false);
                }
                Ok(Some(self.inspect_result(Vec::new(), None)?))
            }
            Command::InspectFix { fix } => {
                let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
                let res = match fix {
                    Fix::AltText {
                        target: Target::Drawing { para, at },
                        text,
                    } => s
                        .edit_drawing(*para, *at, |img| doc::set_alt_text(img, text.trim()))
                        .map_err(|e| e.to_string()),
                    Fix::HeaderRow {
                        target: Target::Table { para },
                    } => match doc::set_header_row(s.doc(), *para) {
                        Some(sections) => {
                            let comments = s.doc().comments.clone();
                            s.replace_content(sections, comments)
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        }
                        None => Err(format!("no table at paragraph {para}")),
                    },
                    _ => Err("this fix does not apply to a document".to_owned()),
                };
                let dirty = self.relayout()?;
                Ok(Some(self.inspect_result(dirty, res.err())?))
            }
            _ => Ok(None),
        }
    }
}
