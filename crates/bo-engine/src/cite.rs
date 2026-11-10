//! BlueDoc citations and bibliography (CORE-CITE-001): sources in the bibliography part
//! (ECMA-376 Part 1, §22.6), `CITATION` (§17.16.5.8) and `BIBLIOGRAPHY` (§17.16.5.7) fields
//! formatted by BlueOffice's own reference styles, BibTeX / RIS import and BibTeX export.
//! Every edit is one undo step of the editing session (see `bo_edit::cite`).

use crate::edit::EditResult;
use crate::{EngineError, Result};
use bo_edit::cite::{bibtex, ris, Citation, CiteState, Source, SourceInfo, StyleId};
use serde::{Deserialize, Serialize};

/// A citation command (`cmd` starts with `cite`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CiteCommand {
    /// The sources, the style and the citation at the caret.
    CiteState,
    /// Select the reference style: citations and bibliography are rewritten.
    CiteSetStyle {
        /// The style.
        style: StyleId,
    },
    /// Add sources (or replace the ones with the same tag).
    CiteSaveSources {
        /// The sources.
        sources: Vec<Source>,
        /// Tag of the source being edited (its citations follow a changed tag).
        #[serde(default)]
        replace: Option<String>,
    },
    /// Delete a source that is not cited.
    CiteDeleteSource {
        /// Its tag.
        tag: String,
    },
    /// Insert a citation at the caret.
    CiteInsert {
        /// The citation.
        citation: Citation,
        /// Sources to add first (a new source or placeholder made while citing).
        #[serde(default)]
        sources: Vec<Source>,
    },
    /// Change the citation at the caret.
    CiteEdit {
        /// The citation.
        citation: Citation,
    },
    /// Insert a bibliography at the caret.
    CiteBibliography {
        /// Heading paragraph before it.
        #[serde(default)]
        heading: Option<String>,
        /// Language (LCID) of the words the style adds.
        #[serde(default)]
        lcid: Option<String>,
    },
    /// Parse BibTeX or RIS text into sources (the document is not changed).
    CiteParse {
        /// `bibtex` or `ris`.
        format: String,
        /// The text.
        text: String,
        /// Tags already in use (the new tags avoid them and the document's).
        #[serde(default)]
        taken: Vec<String>,
    },
    /// BibTeX text of `sources` (default: the document's sources).
    CiteExport {
        /// The sources to export.
        #[serde(default)]
        sources: Option<Vec<Source>>,
    },
    /// Entries and citations of sources outside the document (the master list) in a style.
    CitePreview {
        /// The style (default: the document's).
        #[serde(default)]
        style: Option<StyleId>,
        /// The sources.
        sources: Vec<Source>,
    },
}

impl CiteCommand {
    /// Parses a JSON command when it is a citation command.
    pub fn parse(v: &serde_json::Value) -> Option<CiteCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !c.starts_with("cite") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }

    fn read_only(&self) -> bool {
        matches!(
            self,
            CiteCommand::CiteState
                | CiteCommand::CiteParse { .. }
                | CiteCommand::CiteExport { .. }
                | CiteCommand::CitePreview { .. }
        )
    }
}

/// What a citation command returns.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CiteInfo {
    /// The document's sources, style and the citation at the caret.
    #[serde(flatten)]
    pub state: CiteState,
    /// Number of fields whose result the command changed.
    pub changed: usize,
    /// Sources read from BibTeX / RIS (`citeParse`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parsed: Option<Vec<Source>>,
    /// BibTeX text (`citeExport`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exported: Option<String>,
    /// Entries of sources outside the document (`citePreview`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<Vec<SourceInfo>>,
}

impl crate::Engine {
    /// Run a citation command on the document. Failures are reported in `error` (message
    /// keys `cited`, `noSource`, `noCitation`, `tagTaken`, `inTable`, `badFormat`,
    /// `protected`, `readOnly`).
    pub fn doc_cite_command(&mut self, cmd: CiteCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let read_only = self.sec.read_only;
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let key = |e: bo_edit::EditError| match e {
            bo_edit::EditError::Unsupported(k) => k,
            other => other.to_string(),
        };
        let mut parsed = None;
        let mut exported = None;
        let mut preview = None;
        let res: std::result::Result<usize, String> = if read_only && !cmd.read_only() {
            Err(crate::security::READ_ONLY.into())
        } else if !cmd.read_only() && s.forms_locked() {
            Err("protected".into())
        } else {
            match cmd {
                CiteCommand::CiteState => Ok(0),
                CiteCommand::CiteSetStyle { style } => s.cite_set_style(style).map_err(key),
                CiteCommand::CiteSaveSources { sources, replace } => s
                    .cite_save_sources(sources, replace.as_deref())
                    .map_err(key),
                CiteCommand::CiteDeleteSource { tag } => s.cite_delete_source(&tag).map_err(key),
                CiteCommand::CiteInsert { citation, sources } => {
                    s.cite_insert(&citation, sources).map_err(key)
                }
                CiteCommand::CiteEdit { citation } => s.cite_edit(&citation).map_err(key),
                CiteCommand::CiteBibliography { heading, lcid } => s
                    .cite_insert_bibliography(heading.as_deref(), lcid.as_deref())
                    .map_err(key),
                CiteCommand::CiteParse {
                    format,
                    text,
                    mut taken,
                } => {
                    taken.extend(s.cite_state().sources.into_iter().map(|i| i.source.tag));
                    match format.as_str() {
                        "bibtex" => {
                            parsed = Some(bibtex::import(&text, &taken));
                            Ok(0)
                        }
                        "ris" => {
                            parsed = Some(ris::import(&text, &taken));
                            Ok(0)
                        }
                        _ => Err("badFormat".into()),
                    }
                }
                CiteCommand::CiteExport { sources } => {
                    let list = sources.unwrap_or_else(|| {
                        s.cite_state()
                            .sources
                            .into_iter()
                            .map(|i| i.source)
                            .collect()
                    });
                    exported = Some(bibtex::export(&list));
                    Ok(0)
                }
                CiteCommand::CitePreview { style, sources } => {
                    let style = style.unwrap_or_else(|| s.cite_state().style);
                    preview = Some(bo_edit::cite::preview(style, &sources));
                    Ok(0)
                }
            }
        };
        let state = s.cite_state();
        let dirty = self.relayout()?;
        let (changed, error) = match res {
            Ok(n) => (n, None),
            Err(e) => (0, Some(e)),
        };
        let mut r = self.story_result(dirty, error)?;
        r.cite = Some(Box::new(CiteInfo {
            state,
            changed,
            parsed,
            exported,
            preview,
        }));
        r.collab = self.collab_after_edit();
        Ok(r)
    }
}
