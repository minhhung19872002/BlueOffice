//! Extended mail merge commands (DOC-MERGE-001): recipient list editing, rule fields
//! and prompts, data handed over by host providers, and merge outputs — one PDF per
//! recipient (zipped), one e-mail per recipient (for the host's `MailProvider`),
//! envelopes and label sheets.

use crate::edit::{Command, EditResult};
use crate::Engine;
use base64::Engine as _;
use bo_edit::merge::recipients::{self, RecipientOptions};
use bo_edit::merge::rules::{self, MergeRun, MergedDoc, Prompt};
use bo_edit::merge::sheets::{self, EnvelopeSize, LabelSheet};
use bo_edit::merge::MergeData;
use bo_model::{Block, Document};
use std::io::Write as _;

/// Default number of records `mergeRecords` returns.
pub const RECORDS_PAGE: usize = 500;

/// A record of the recipient list editor.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordRow {
    /// Data row (0-based).
    pub index: usize,
    /// Values, one per field.
    pub values: Vec<String>,
    /// Not excluded by hand.
    pub included: bool,
    /// Position in the recipient list (after filter and sort), if it is in it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
}

/// Records of the data source (`mergeRecords`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Records {
    /// Field names.
    pub fields: Vec<String>,
    /// Number of records in the data source.
    pub total: usize,
    /// Records in the recipient list.
    pub active: usize,
    /// The requested rows, in data source order.
    pub rows: Vec<RecordRow>,
}

/// A produced file.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeFile {
    /// File name inside the zip.
    pub name: String,
    /// Number of pages.
    pub pages: usize,
}

/// An attachment of a merged e-mail.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAttachment {
    /// File name.
    pub name: String,
    /// MIME type.
    pub content_type: String,
    /// File bytes, base64.
    pub base64: String,
}

/// A merged e-mail for the host's `MailProvider`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeMail {
    /// Recipient address.
    pub to: String,
    /// Subject line.
    pub subject: String,
    /// HTML body: the merged document.
    pub html: String,
    /// Attachment, if requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment: Option<MailAttachment>,
}

/// Label sheet and envelope presets.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Presets {
    /// Label sheets.
    pub labels: Vec<LabelSheet>,
    /// Envelope sizes.
    pub envelopes: Vec<EnvelopeSize>,
}

/// Results of the extended mail-merge commands.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeOut {
    /// `mergeRecords`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub records: Option<Records>,
    /// `mergeDuplicates`: groups of data rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicates: Option<Vec<Vec<usize>>>,
    /// `mergePrompts`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<Vec<Prompt>>,
    /// `mergeToPdfs`: the files in the zip.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<MergeFile>>,
    /// `mergeToPdfs`: the zip, base64.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<String>,
    /// `mergeToEmails`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mails: Option<Vec<MergeMail>>,
    /// `mergeToEmails`: merged documents left out because their address is empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<usize>,
    /// `mergePresets`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presets: Option<Presets>,
}

/// `pattern` with every `«Field»` replaced by the value of data row `row`.
pub fn fill_pattern(pattern: &str, data: &MergeData, row: usize) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(i) = rest.find('«') {
        let after = &rest[i + '«'.len_utf8()..];
        let Some(j) = after.find('»') else { break };
        out.push_str(&rest[..i]);
        out.push_str(data.value(row, after[..j].trim()).unwrap_or(""));
        rest = &after[j + '»'.len_utf8()..];
    }
    out.push_str(rest);
    out
}

/// A file name from `text`: characters not allowed in Windows / POSIX file names and
/// control characters become `_`; leading / trailing dots and spaces go.
pub fn safe_file_name(text: &str) -> String {
    let s: String = text
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let s = s.trim_matches(|c: char| c == '.' || c.is_whitespace());
    s.chars().take(120).collect()
}

/// Unique file names: `stem.ext`, then `stem (2).ext`, …; an empty stem is the number.
pub fn unique_name<S: std::hash::BuildHasher>(
    stem: &str,
    ext: &str,
    n: usize,
    used: &mut std::collections::HashSet<String, S>,
) -> String {
    let stem = if stem.is_empty() {
        n.to_string()
    } else {
        stem.to_owned()
    };
    let mut name = format!("{stem}.{ext}");
    let mut k = 2;
    while !used.insert(name.to_lowercase()) {
        name = format!("{stem} ({k}).{ext}");
        k += 1;
    }
    name
}

/// Write `files` as a deflated zip.
fn zip(files: &[(String, Vec<u8>)]) -> crate::Result<Vec<u8>> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let err = |e: &dyn std::fmt::Display| crate::EngineError::Format(format!("zip: {e}"));
    for (name, data) in files {
        w.start_file(name.as_str(), opts).map_err(|e| err(&e))?;
        w.write_all(data).map_err(|e| err(&e))?;
    }
    Ok(w.finish().map_err(|e| err(&e))?.into_inner())
}

/// Paragraphs of `blocks`, table cells flattened (the e-mail body).
fn flat_paragraphs(blocks: &[Block], out: &mut Vec<Block>) {
    for b in blocks {
        match b {
            Block::Paragraph(_) => out.push(b.clone()),
            Block::Table(t) => {
                for r in &t.rows {
                    for c in &r.cells {
                        flat_paragraphs(&c.blocks, out);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// HTML body of a merged document.
pub fn mail_html(doc: &Document) -> String {
    let mut blocks = Vec::new();
    for s in &doc.sections {
        flat_paragraphs(&s.blocks, &mut blocks);
    }
    bo_edit::html::to_html(doc, &blocks)
}

impl Engine {
    /// Set the mail-merge data source; the recipient list starts unfiltered.
    pub(crate) fn set_merge_source(&mut self, d: MergeData) {
        self.merge = Some(d);
        self.merge_opts = RecipientOptions::default();
        self.refresh_merge_order();
    }

    fn refresh_merge_order(&mut self) {
        self.merge_order = self
            .merge
            .as_ref()
            .map(|m| recipients::active(m, &self.merge_opts))
            .unwrap_or_default();
        if let Some(p) = self.merge_preview {
            self.merge_preview = match self.merge_order.len() {
                0 => None,
                n => Some(p.min(n - 1)),
            };
        }
    }

    /// Mail-merge state reported to hosts.
    pub(crate) fn merge_state(&self) -> Option<crate::merge::MergeState> {
        self.merge.as_ref().map(|m| crate::merge::MergeState {
            name: m.name.clone(),
            fields: m.fields.clone(),
            count: m.rows.len(),
            preview: self.merge_preview,
            active: self.merge_order.len(),
            options: self.merge_opts.clone(),
        })
    }

    /// The open document merged once per output document (rules applied).
    fn merged_docs(&self) -> crate::Result<(Vec<MergedDoc>, &MergeData)> {
        let m = self
            .merge
            .as_ref()
            .filter(|_| !self.merge_order.is_empty())
            .ok_or_else(|| crate::EngineError::BadCommand("mail merge: no data source".into()))?;
        let doc = self.document().ok_or(crate::EngineError::NoDocument)?;
        let run = MergeRun {
            data: m,
            order: &self.merge_order,
            answers: &self.merge_answers,
        };
        Ok((rules::merge_each(doc, run), m))
    }

    /// Lay out `doc` with the engine's fonts and write it as PDF; returns the bytes
    /// and the page count.
    pub(crate) fn pdf_of(
        &mut self,
        doc: &Document,
        title: &str,
    ) -> crate::Result<(Vec<u8>, usize)> {
        let layout = bo_layout::layout_document_with(
            doc,
            &mut self.fonts,
            &mut self.cache,
            bo_layout::LayoutOptions::default(),
        )?;
        let list = bo_render::build_display_list(&layout, &self.fonts);
        let images = bo_render::image::Chain(&doc.media, &self.images);
        let pdf = bo_pdf::write_pdf_with_images(
            &list,
            &self.fonts,
            &bo_pdf::PdfMeta {
                title: Some(title.to_owned()),
            },
            &images,
        )?;
        Ok((pdf, layout.pages.len()))
    }

    /// One PDF per merged document: (file name, bytes, pages).
    pub fn merge_pdfs(&mut self, pattern: &str) -> crate::Result<Vec<(String, Vec<u8>, usize)>> {
        let (docs, m) = self.merged_docs()?;
        let names: Vec<(String, Document)> = {
            let mut used = std::collections::HashSet::new();
            docs.into_iter()
                .enumerate()
                .map(|(n, d)| {
                    let row = d
                        .records
                        .first()
                        .and_then(|p| self.merge_order.get(*p))
                        .copied();
                    let stem = row
                        .map(|r| safe_file_name(&fill_pattern(pattern, m, r)))
                        .unwrap_or_default();
                    (unique_name(&stem, "pdf", n + 1, &mut used), d.doc)
                })
                .collect()
        };
        let mut out = Vec::new();
        for (name, doc) in names {
            let title = name.trim_end_matches(".pdf").to_owned();
            let (pdf, pages) = self.pdf_of(&doc, &title)?;
            out.push((name, pdf, pages));
        }
        Ok(out)
    }

    /// One e-mail per merged document with an address in field `to`; returns the
    /// mails and the number left out.
    pub fn merge_emails(
        &mut self,
        to: &str,
        subject: &str,
        attach: &str,
        attachment_name: &str,
    ) -> crate::Result<(Vec<MergeMail>, usize)> {
        let (docs, m) = self.merged_docs()?;
        let mut prepared = Vec::new();
        let mut skipped = 0;
        let mut used = std::collections::HashSet::new();
        for (n, d) in docs.into_iter().enumerate() {
            let Some(row) = d
                .records
                .first()
                .and_then(|p| self.merge_order.get(*p))
                .copied()
            else {
                continue;
            };
            let address = m.value(row, to).unwrap_or("").trim().to_owned();
            if address.is_empty() || !address.contains('@') {
                skipped += 1;
                continue;
            }
            let pattern = if attachment_name.trim().is_empty() {
                subject
            } else {
                attachment_name
            };
            let stem = safe_file_name(&fill_pattern(pattern, m, row));
            prepared.push((
                address,
                fill_pattern(subject, m, row).replace(['\r', '\n'], " "),
                mail_html(&d.doc),
                stem,
                n + 1,
                d.doc,
            ));
        }
        let mut mails = Vec::new();
        for (to, subject, html, stem, n, doc) in prepared {
            let attachment = match attach {
                "pdf" => {
                    let name = unique_name(&stem, "pdf", n, &mut used);
                    let (bytes, _) = self.pdf_of(&doc, &stem)?;
                    Some(MailAttachment {
                        name,
                        content_type: "application/pdf".into(),
                        base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                    })
                }
                "docx" => {
                    let name = unique_name(&stem, "docx", n, &mut used);
                    let bytes = bo_docx::write_docx(&doc)?;
                    Some(MailAttachment {
                        name,
                        content_type:
                            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                                .into(),
                        base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                    })
                }
                _ => None,
            };
            mails.push(MergeMail {
                to,
                subject,
                html,
                attachment,
            });
        }
        Ok((mails, skipped))
    }

    /// Records of the data source for the recipient list editor.
    fn merge_records(&self, offset: usize, limit: usize) -> Option<Records> {
        let m = self.merge.as_ref()?;
        let pos: std::collections::HashMap<usize, usize> = self
            .merge_order
            .iter()
            .enumerate()
            .map(|(p, r)| (*r, p))
            .collect();
        Some(Records {
            fields: m.fields.clone(),
            total: m.rows.len(),
            active: self.merge_order.len(),
            rows: m
                .rows
                .iter()
                .enumerate()
                .skip(offset)
                .take(limit)
                .map(|(i, v)| RecordRow {
                    index: i,
                    values: v.clone(),
                    included: !self.merge_opts.excluded.contains(&i),
                    position: pos.get(&i).copied(),
                })
                .collect(),
        })
    }

    fn load_merge_template(&mut self, doc: Document, stem: &str) -> crate::Result<()> {
        self.load(doc, &format!("{stem}.docx"))?;
        // A data source stays attached: the envelope / labels merge with it.
        Ok(())
    }

    /// Extended mail-merge commands; `None` = not one of them.
    pub(crate) fn mail_merge_command(
        &mut self,
        cmd: &Command,
    ) -> crate::Result<Option<EditResult>> {
        let mut out = MergeOut::default();
        let mut error = None;
        let mut relayout = false;
        let mut reload = false;
        match cmd {
            Command::MergeTable { name, fields, rows } => {
                self.set_merge_source(crate::merge_sources::from_table(name, fields, rows));
                relayout = self.merge_preview.is_some();
            }
            Command::MergeSetRecipients { options } => {
                let mut o = options.clone();
                o.sort.truncate(recipients::MAX_SORT_KEYS);
                o.excluded.sort_unstable();
                o.excluded.dedup();
                self.merge_opts = o;
                self.refresh_merge_order();
                relayout = self.merge_preview.is_some();
            }
            Command::MergeRecords { offset, limit } => {
                out.records = self.merge_records(*offset, limit.unwrap_or(RECORDS_PAGE));
            }
            Command::MergeDuplicates { fields } => {
                out.duplicates = Some(
                    self.merge
                        .as_ref()
                        .map(|m| recipients::duplicates(m, fields))
                        .unwrap_or_default(),
                );
            }
            Command::MergePrompts => {
                out.prompts = Some(self.document().map(rules::prompts).unwrap_or_default());
            }
            Command::MergeSetAnswers { answers } => {
                self.merge_answers = answers.clone();
                relayout = self.merge_preview.is_some();
            }
            Command::MergeToPdfs { pattern } => match self.merge_pdfs(pattern) {
                Ok(files) => {
                    out.files = Some(
                        files
                            .iter()
                            .map(|(name, _, pages)| MergeFile {
                                name: name.clone(),
                                pages: *pages,
                            })
                            .collect(),
                    );
                    let z = zip(&files
                        .into_iter()
                        .map(|(n, b, _)| (n, b))
                        .collect::<Vec<_>>())?;
                    out.zip = Some(base64::engine::general_purpose::STANDARD.encode(z));
                }
                Err(e) => error = Some(e.to_string()),
            },
            Command::MergeToEmails {
                to,
                subject,
                attach,
                attachment_name,
            } => match self.merge_emails(to, subject, attach, attachment_name) {
                Ok((mails, skipped)) => {
                    out.mails = Some(mails);
                    out.skipped = Some(skipped);
                }
                Err(e) => error = Some(e.to_string()),
            },
            Command::MergeEnvelope {
                size,
                delivery,
                return_address,
                at,
            } => match sheets::envelope_document(size, delivery, return_address, *at) {
                Ok(doc) => {
                    self.load_merge_template(doc, &format!("envelope-{}", size.id))?;
                    reload = true;
                }
                Err(e) => error = Some(e),
            },
            Command::MergeLabels { sheet, text, mode } => {
                match sheets::labels_document(sheet, text, *mode) {
                    Ok(doc) => {
                        self.load_merge_template(doc, &format!("labels-{}", sheet.id))?;
                        reload = true;
                    }
                    Err(e) => error = Some(e),
                }
            }
            Command::MergePresets => {
                out.presets = Some(Presets {
                    labels: sheets::label_presets(),
                    envelopes: sheets::envelope_presets(),
                });
            }
            _ => return Ok(None),
        }
        let dirty = if relayout || reload {
            self.relayout()?
        } else {
            Vec::new()
        };
        let dirty = if reload {
            (0..self.layout.document().pages.len()).collect()
        } else {
            dirty
        };
        let mut r = self.edit_result(dirty, (None, None, None), error)?;
        r.merge = self.merge_state();
        r.merge_out = Some(Box::new(out));
        Ok(Some(r))
    }
}

#[cfg(test)]
mod tests;
