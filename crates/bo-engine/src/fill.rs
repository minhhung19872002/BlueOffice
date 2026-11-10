//! Template filling for headless report generation (CORE-API-001): a list of edits
//! addressed by name — content controls by tag or title, bookmarks, text placeholders,
//! document properties, workbook cells by reference or defined name — applied through
//! the ordinary editing commands (`bo-cli edit`, the server's REST API).

use crate::edit::Command;
use crate::{Engine, EngineError, SheetEngine};
use bo_edit::{FindOptions, Pos, Selection};
use bo_model::index::{body_paragraphs, item_text, para_at};
use bo_model::{ContentControl, ControlKind, Document, RunContent};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One edit of a template.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum FillEdit {
    /// Fill every inline content control (ECMA-376 Part 1, §17.5.2 sdt) with this tag
    /// (§17.5.2.42) and / or title (§17.5.2.1 alias): text, a list item, an ISO 8601
    /// date, or `true` / `false` for a check box.
    SetControl {
        /// Tag of the control.
        #[serde(default)]
        tag: Option<String>,
        /// Title (alias) of the control.
        #[serde(default)]
        title: Option<String>,
        /// The new value.
        #[serde(default)]
        value: Value,
    },
    /// Replace the text of a bookmark (§17.13.6.2 bookmarkStart) of the main text.
    SetBookmark {
        /// Bookmark name.
        name: String,
        /// The new text.
        #[serde(default)]
        text: String,
    },
    /// Replace every occurrence of a placeholder text (`\n` in `with` starts a paragraph).
    ReplaceText {
        /// Text to look for (literal).
        find: String,
        /// Replacement text (literal).
        #[serde(default)]
        with: String,
        /// Compare case-sensitively.
        #[serde(default)]
        match_case: bool,
        /// Match whole words only.
        #[serde(default)]
        whole_word: bool,
    },
    /// Set document properties (title, subject, author, keywords, …).
    SetProperties {
        /// Properties to change (absent: unchanged, empty: removed).
        props: crate::props::PropsEdit,
    },
    /// Workbooks: set a cell by reference (`B7`, `'Sổ 1'!B7`) or defined name
    /// (§18.2.5 definedName); a range receives the value in its first cell. A string
    /// starting with `=` is a formula.
    SetCell {
        /// Cell reference or defined name.
        #[serde(rename = "ref")]
        target: String,
        /// The new value (string, number, boolean or null to clear).
        #[serde(default)]
        value: Value,
    },
}

/// What an edit changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillOutcome {
    /// The edit's `op`.
    pub op: &'static str,
    /// What it addressed (tag, bookmark name, placeholder, cell).
    pub target: String,
    /// Number of places changed (0: nothing matched).
    pub count: usize,
}

/// Parse the JSON array of edits.
pub fn parse_edits(json: &str) -> crate::Result<Vec<FillEdit>> {
    serde_json::from_str(json).map_err(|e| EngineError::BadCommand(format!("edits: {e}")))
}

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::BadCommand(msg.into())
}

/// Text of a JSON value typed into a document.
fn value_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// A JSON value as a check box state.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "x" | "on"
        ),
        _ => false,
    }
}

/// Literal text as a find / replace pattern: `^` introduces special characters there.
fn literal(text: &str) -> String {
    text.replace('^', "^^")
}

/// An inline content control: its paragraph, an offset inside it and its identity.
struct ControlSite {
    para: usize,
    offset: usize,
    id: Option<i64>,
    tag: Option<String>,
    alias: Option<String>,
}

fn control_matches(cc: &ContentControl, tag: Option<&str>, title: Option<&str>) -> bool {
    tag.is_none_or(|t| cc.tag.as_deref() == Some(t))
        && title.is_none_or(|t| cc.alias.as_deref() == Some(t))
}

/// Inline controls with this tag / title, in document order.
fn control_sites(doc: &Document, tag: Option<&str>, title: Option<&str>) -> Vec<ControlSite> {
    let mut out = Vec::new();
    for (pid, path) in body_paragraphs(doc).iter().enumerate() {
        let Some(p) = para_at(doc, path) else {
            continue;
        };
        let mut text = String::new();
        let mut open: Vec<(&ContentControl, usize)> = Vec::new();
        for c in p.runs.iter().flat_map(|r| r.content.iter()) {
            match c {
                RunContent::ControlStart(cc) if !cc.block => open.push((cc, text.len())),
                RunContent::ControlEnd(false) => {
                    if let Some((cc, start)) = open.pop() {
                        if control_matches(cc, tag, title) {
                            // Inside the content when there is some: the boundary offsets are
                            // shared with the neighbouring controls.
                            let inner = text[start..].chars().next().map_or(0, char::len_utf8);
                            out.push(ControlSite {
                                para: pid,
                                offset: start + inner,
                                id: cc.id,
                                tag: cc.tag.clone(),
                                alias: cc.alias.clone(),
                            });
                        }
                    }
                }
                _ => {}
            }
            text.push_str(&item_text(c));
        }
    }
    out
}

/// Value of attribute `name` in a raw tag.
fn xml_attr(xml: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let i = xml.find(&key)? + key.len();
    let raw = &xml[i..i + xml[i..].find('"')?];
    Some(
        raw.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&"),
    )
}

/// A bookmark inside one paragraph: index-text range and the length of its first character.
struct BookmarkSite {
    para: usize,
    start: usize,
    end: usize,
    first: usize,
}

/// The bookmark `name` of the main text; an error when it runs over several paragraphs.
fn bookmark_site(doc: &Document, name: &str) -> crate::Result<Option<BookmarkSite>> {
    for (pid, path) in body_paragraphs(doc).iter().enumerate() {
        let Some(p) = para_at(doc, path) else {
            continue;
        };
        let mut text = String::new();
        let mut open: Option<(usize, String)> = None;
        for c in p.runs.iter().flat_map(|r| r.content.iter()) {
            if let RunContent::Markup(raw) = c {
                let x = &raw.xml;
                if x.contains("bookmarkStart") {
                    if open.is_none() && xml_attr(x, "w:name").as_deref() == Some(name) {
                        open = Some((text.len(), xml_attr(x, "w:id").unwrap_or_default()));
                    }
                } else if x.contains("bookmarkEnd") {
                    if let Some((start, id)) = &open {
                        if xml_attr(x, "w:id").as_deref() == Some(id.as_str()) {
                            let first = text[*start..].chars().next().map_or(0, char::len_utf8);
                            return Ok(Some(BookmarkSite {
                                para: pid,
                                start: *start,
                                end: text.len(),
                                first,
                            }));
                        }
                    }
                }
            }
            text.push_str(&item_text(c));
        }
        if open.is_some() {
            return Err(bad(format!(
                "bookmark '{name}' spans several paragraphs (not supported)"
            )));
        }
    }
    Ok(None)
}

impl Engine {
    /// Apply `edits` to the open document in order; one outcome per edit.
    pub fn fill(&mut self, edits: &[FillEdit]) -> crate::Result<Vec<FillOutcome>> {
        edits.iter().map(|e| self.fill_one(e)).collect()
    }

    /// Run a command; a refused command is an error.
    fn fill_cmd(&mut self, cmd: Command) -> crate::Result<crate::EditResult> {
        let r = self.command(cmd)?;
        match &r.error {
            Some(e) => Err(bad(e.clone())),
            None => Ok(r),
        }
    }

    fn fill_select(&mut self, para: usize, from: usize, to: usize) -> crate::Result<()> {
        self.fill_cmd(Command::SetSelection {
            selection: Selection {
                anchor: Pos::new(para, from),
                focus: Pos::new(para, to),
            },
        })?;
        Ok(())
    }

    fn fill_one(&mut self, edit: &FillEdit) -> crate::Result<FillOutcome> {
        match edit {
            FillEdit::SetControl { tag, title, value } => {
                if tag.is_none() && title.is_none() {
                    return Err(bad("setControl needs 'tag' or 'title'"));
                }
                let count = self.fill_controls(tag.as_deref(), title.as_deref(), value)?;
                Ok(FillOutcome {
                    op: "setControl",
                    target: tag.clone().or_else(|| title.clone()).unwrap_or_default(),
                    count,
                })
            }
            FillEdit::SetBookmark { name, text } => Ok(FillOutcome {
                op: "setBookmark",
                target: name.clone(),
                count: usize::from(self.fill_bookmark(name, text)?),
            }),
            FillEdit::ReplaceText {
                find,
                with,
                match_case,
                whole_word,
            } => {
                if find.is_empty() {
                    return Err(bad("replaceText needs a non-empty 'find'"));
                }
                let r = self.fill_cmd(Command::ReplaceAll {
                    query: literal(find),
                    options: FindOptions {
                        match_case: *match_case,
                        whole_word: *whole_word,
                        ..FindOptions::default()
                    },
                    with: literal(with),
                    format: None,
                })?;
                Ok(FillOutcome {
                    op: "replaceText",
                    target: find.clone(),
                    count: r.count.unwrap_or(0),
                })
            }
            FillEdit::SetProperties { props } => {
                self.fill_cmd(Command::SetDocProperties {
                    props: props.clone(),
                })?;
                Ok(FillOutcome {
                    op: "setProperties",
                    target: String::new(),
                    count: 1,
                })
            }
            FillEdit::SetCell { .. } => Err(bad("setCell applies to workbooks only")),
        }
    }

    fn fill_controls(
        &mut self,
        tag: Option<&str>,
        title: Option<&str>,
        value: &Value,
    ) -> crate::Result<usize> {
        let doc = self.document().ok_or(EngineError::NoDocument)?;
        let sites = control_sites(doc, tag, title);
        let mut count = 0;
        // Last first: filling a control moves the offsets after it.
        for site in sites.iter().rev() {
            self.fill_select(site.para, site.offset, site.offset)?;
            let Some(cc) = self
                .session
                .as_ref()
                .and_then(bo_edit::Session::control_at_caret)
            else {
                continue;
            };
            if cc.id != site.id || cc.tag != site.tag || cc.alias != site.alias {
                continue;
            }
            if let ControlKind::Checkbox { checked, .. } = cc.kind {
                // The command toggles: only when the state differs.
                if checked == truthy(value) {
                    count += 1;
                    continue;
                }
            }
            let r = self.fill_cmd(Command::SetControlValue {
                value: value_text(value),
            })?;
            count += r.count.unwrap_or(0);
        }
        Ok(count)
    }

    fn fill_bookmark(&mut self, name: &str, text: &str) -> crate::Result<bool> {
        let doc = self.document().ok_or(EngineError::NoDocument)?;
        let Some(b) = bookmark_site(doc, name)? else {
            return Ok(false);
        };
        if b.end == b.start {
            // An empty bookmark marks a place: the text goes there.
            if !text.is_empty() {
                self.fill_select(b.para, b.start, b.start)?;
                self.fill_cmd(Command::InsertText { text: text.into() })?;
            }
            return Ok(true);
        }
        if text.is_empty() {
            self.fill_select(b.para, b.start, b.end)?;
            self.fill_cmd(Command::DeleteForward)?;
            return Ok(true);
        }
        // Keep the text between the bookmark's marks (and the formatting of its first
        // character): type after the first character, then remove the old text around.
        let inner = b.start + b.first;
        if b.end > inner {
            self.fill_select(b.para, inner, b.end)?;
            self.fill_cmd(Command::DeleteForward)?;
        }
        self.fill_select(b.para, inner, inner)?;
        self.fill_cmd(Command::InsertText { text: text.into() })?;
        self.fill_select(b.para, b.start, inner)?;
        self.fill_cmd(Command::DeleteForward)?;
        Ok(true)
    }
}

impl SheetEngine {
    /// Apply `edits` to the open workbook in order; one outcome per edit.
    pub fn fill(&mut self, edits: &[FillEdit]) -> crate::Result<Vec<FillOutcome>> {
        let mut out = Vec::with_capacity(edits.len());
        for e in edits {
            let FillEdit::SetCell { target, value } = e else {
                return Err(bad("only setCell applies to workbooks"));
            };
            let wb = self.workbook().ok_or(EngineError::NoDocument)?;
            let count = match bo_sheet::cells::resolve_ref(wb, 0, target) {
                Some((sheet, range)) => {
                    let input = match value {
                        Value::Bool(true) => "TRUE".to_owned(),
                        Value::Bool(false) => "FALSE".to_owned(),
                        other => value_text(other),
                    };
                    self.set_input(sheet, range.first.row, range.first.col, &input)?;
                    1
                }
                None => 0,
            };
            out.push(FillOutcome {
                op: "setCell",
                target: target.clone(),
                count,
            });
        }
        Ok(out)
    }
}
