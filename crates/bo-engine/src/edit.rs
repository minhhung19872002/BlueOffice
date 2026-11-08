//! Editing commands over the JSON bridge (ADR-0002, ADR-0008).
//!
//! Hosts send one [`Command`] as JSON and receive an [`EditResult`]: the new
//! selection with its geometry, ribbon state and the pages whose display list
//! changed (so the canvas only repaints those).

use crate::nav::{self, CaretRect, Motion, SelRect};
use crate::review::ReviewState;
use bo_edit::section::NoteInfo;
use bo_edit::{
    CharFormat, FindOptions, FormatState, Match, PageSetupSpec, ParaFormat, PasteMode, Pos,
    Selection, SetupScope, TableEdit,
};
use bo_layout::{LayoutOptions, RevisionView};
use bo_model::SectionBreak;
use serde::{Deserialize, Serialize};

/// Section break kinds accepted from hosts (§17.18.77 `ST_SectionMark`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BreakKind {
    NextPage,
    Continuous,
    EvenPage,
    OddPage,
}

impl From<BreakKind> for SectionBreak {
    fn from(k: BreakKind) -> Self {
        match k {
            BreakKind::NextPage => SectionBreak::NextPage,
            BreakKind::Continuous => SectionBreak::Continuous,
            BreakKind::EvenPage => SectionBreak::EvenPage,
            BreakKind::OddPage => SectionBreak::OddPage,
        }
    }
}

/// An editing command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    /// Return the current state without changing anything.
    State,
    SetSelection {
        selection: Selection,
    },
    /// Mouse click: `count` 1 = caret, 2 = word, 3 = paragraph; `extend` = Shift held.
    Click {
        page: usize,
        x: f32,
        y: f32,
        #[serde(default)]
        extend: bool,
        #[serde(default = "one")]
        count: u8,
    },
    /// Mouse drag to a point (extends the selection).
    DragTo {
        page: usize,
        x: f32,
        y: f32,
    },
    Move {
        motion: Motion,
        #[serde(default)]
        extend: bool,
    },
    SelectAll,
    InsertText {
        text: String,
    },
    DeleteBackward,
    DeleteForward,
    SplitParagraph,
    FormatText {
        format: CharFormat,
    },
    FormatParagraph {
        format: ParaFormat,
    },
    ApplyStyle {
        id: String,
    },
    InsertTable {
        rows: usize,
        cols: usize,
    },
    InsertPageBreak,
    InsertSectionBreak {
        kind: BreakKind,
    },
    Undo,
    Redo,
    Find {
        query: String,
        #[serde(default)]
        options: FindOptions,
    },
    /// Select a match returned by `find`.
    SelectMatch {
        #[serde(rename = "match")]
        m: Match,
    },
    Replace {
        #[serde(rename = "match")]
        m: Match,
        with: String,
    },
    ReplaceAll {
        query: String,
        #[serde(default)]
        options: FindOptions,
        with: String,
    },
    Copy,
    Cut,
    /// Paste: the internal format wins over HTML, HTML over plain text.
    Paste {
        #[serde(default)]
        internal: Option<String>,
        #[serde(default)]
        html: Option<String>,
        #[serde(default)]
        text: Option<String>,
        /// Paste Special: keep source formatting (default), merge, or text only.
        #[serde(default)]
        mode: Option<PasteMode>,
    },
    /// Review state only (track changes, revisions, comments).
    Review,
    /// Track-changes mode on/off (ECMA-376 Part 1, §17.13.5).
    SetTrackChanges {
        on: bool,
    },
    /// Author of new revisions and comments (the signed-in user; empty = "BlueOffice").
    SetAuthor {
        name: String,
    },
    /// Markup / final / original display of tracked changes.
    SetRevisionView {
        view: RevisionView,
    },
    /// Accept one revision by id, or those at the selection when `id` is absent.
    AcceptRevision {
        #[serde(default)]
        id: Option<i32>,
    },
    /// Reject one revision by id, or those at the selection when `id` is absent.
    RejectRevision {
        #[serde(default)]
        id: Option<i32>,
    },
    AcceptAllRevisions,
    RejectAllRevisions,
    /// Select the next (`forward`) or previous revision.
    NextRevision {
        #[serde(default = "yes")]
        forward: bool,
    },
    /// Comment on the selection (ECMA-376 Part 1, §17.13.4).
    AddComment {
        text: String,
    },
    /// Reply to comment `id` ([MS-DOCX] threading).
    ReplyComment {
        id: i32,
        text: String,
    },
    /// Mark a thread resolved or open again.
    ResolveComment {
        id: i32,
        #[serde(default = "yes")]
        done: bool,
    },
    /// Delete a comment and its replies.
    DeleteComment {
        id: i32,
    },
    /// Select the text a comment is anchored to.
    SelectComment {
        id: i32,
    },
    /// Select the next (`forward`) or previous comment.
    NextComment {
        #[serde(default = "yes")]
        forward: bool,
    },
    /// Update TOC, SEQ, REF and PAGEREF fields (ECMA-376 Part 1, §17.16).
    UpdateFields,
    /// Insert a table of contents at the caret.
    InsertToc,
    /// Insert a caption (`label n: text`) after the caret's paragraph.
    InsertCaption {
        label: String,
        #[serde(default)]
        text: String,
    },
    /// List the cross-reference targets (bookmarks and captions).
    CrossReferenceTargets,
    /// Insert a cross-reference to a target: its text or (`page`) its page number.
    InsertCrossReference {
        target: String,
        #[serde(default)]
        page: bool,
    },
    /// Mail-merge data source (CSV or XLSX bytes, base64).
    MergeData {
        name: String,
        base64: String,
    },
    /// Insert `MERGEFIELD name` (ECMA-376 Part 1, §17.16.5.35) at the caret.
    MergeInsertField {
        name: String,
    },
    /// Show record `record` (0-based) in the merge fields, or the field names.
    MergePreview {
        #[serde(default)]
        record: Option<usize>,
    },
    /// Replace the open document by the merged one (one section per record).
    MergeToDocument,
    /// Password for saving ([MS-OFFCRYPTO] Agile Encryption); empty or absent: none.
    SetSavePassword {
        #[serde(default)]
        password: Option<String>,
    },
    /// Tab key: next (`back`: previous) table cell, else a tab character.
    Tab {
        #[serde(default)]
        back: bool,
    },
    /// Table editing at the selection (ECMA-376 Part 1, §17.4).
    TableEdit {
        edit: TableEdit,
    },
    /// Page size, orientation, margins and columns (§17.6.13, §17.6.11, §17.6.4).
    SetPageSetup {
        setup: PageSetupSpec,
        #[serde(default)]
        scope: SetupScope,
    },
    /// "Different first page" (§17.10.6 titlePg) / "different odd & even" (§17.15.1.35).
    SetHeaderOptions {
        #[serde(default)]
        title_page: Option<bool>,
        #[serde(default)]
        even_and_odd: Option<bool>,
    },
    /// "Link to previous" for the open header/footer (§17.10.5 headerReference inheritance).
    SetHeaderLink {
        linked: bool,
    },
    /// Open the header (`header`) or footer of a page for editing (§17.10).
    EditHeaderFooter {
        page: usize,
        header: bool,
    },
    /// Close the header/footer editor.
    CloseHeaderFooter,
    /// Insert a simple field such as `PAGE` or `NUMPAGES` (§17.16.5.43, §17.16.5.40).
    InsertField {
        instr: String,
    },
    /// Insert a footnote (or endnote) at the caret (§17.11.14, §17.11.7).
    InsertNote {
        #[serde(default)]
        endnote: bool,
        #[serde(default)]
        text: String,
    },
    /// Replace the text of a footnote or endnote.
    SetNoteText {
        #[serde(default)]
        endnote: bool,
        id: i32,
        text: String,
    },
    /// List the footnotes and endnotes.
    Notes,
    /// Edit a footnote/endnote in place on its page (caret at the end of its text).
    EditNote {
        #[serde(default)]
        endnote: bool,
        id: i32,
    },
    /// Footnote/endnote numbering and placement of the caret's section or the document
    /// (§17.11.11 footnotePr, §17.11.4 endnotePr).
    SetNoteOptions {
        #[serde(default)]
        endnote: bool,
        options: bo_edit::section::NoteOptions,
        #[serde(default)]
        scope: SetupScope,
    },
    /// Clear character and paragraph formatting of the selection.
    ClearFormatting,
    /// Format Painter: pick the formatting at the selection.
    PickFormat,
    /// Format Painter: apply the picked formatting to the selection.
    ApplyFormat,
    /// Format Painter: forget the picked formatting.
    CancelFormat,
    /// Fonts for the font picker (bundled faces and substituted names).
    FontList,
    /// Replace the open document by a new one from a built-in template.
    NewFromTemplate {
        id: String,
        name: String,
    },
}

fn yes() -> bool {
    true
}

fn one() -> u8 {
    1
}

/// Clipboard payload returned by copy/cut.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClipOut {
    pub text: String,
    pub html: String,
    pub internal: String,
}

/// Result of a command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    pub info: crate::DocInfo,
    pub selection: Selection,
    pub caret: Option<CaretRect>,
    pub rects: Vec<SelRect>,
    /// Pages whose display list changed (all pages after a page-count change).
    pub dirty: Vec<usize>,
    pub format: FormatState,
    /// Document changed since it was opened or last saved.
    pub modified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches: Option<Vec<Match>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<ClipOut>,
    /// Error message of a command that could not be applied (state is unchanged).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Review pane state; present after document changes and review commands.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewState>,
    /// Cross-reference targets (`crossReferenceTargets`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<bo_edit::RefTarget>>,
    /// Mail-merge state (after merge commands and `state`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge: Option<crate::merge::MergeState>,
    /// Change to share and selection offsets while collaborating (Engine::collab_start).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collab: Option<crate::collab::CollabOut>,
    /// Open header/footer editor (selection and caret are in it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story: Option<crate::story::StoryState>,
    /// Footnotes and endnotes (`notes`, note commands).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Vec<NoteInfo>>,
    /// Font picker entries (`fontList`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fonts: Option<Vec<crate::fonts::FontChoice>>,
    /// The Format Painter holds picked formatting.
    pub painter: bool,
}

impl crate::Engine {
    /// Run a JSON command and return the [`EditResult`] as JSON. Besides the command
    /// fields the object may carry `now` (Unix milliseconds): the host clock used to date
    /// new revisions and comments (browser WASM has no clock of its own).
    pub fn command_json(&mut self, json: &str) -> crate::Result<String> {
        let mut v: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| crate::EngineError::BadCommand(e.to_string()))?;
        let now = v
            .as_object_mut()
            .and_then(|o| o.remove("now"))
            .and_then(|n| n.as_f64());
        let cmd: Command =
            serde_json::from_value(v).map_err(|e| crate::EngineError::BadCommand(e.to_string()))?;
        let r = self.command_at(cmd, now.map(|ms| ms as i64))?;
        Ok(serde_json::to_string(&r).unwrap_or_default())
    }

    /// Run a command.
    pub fn command(&mut self, cmd: Command) -> crate::Result<EditResult> {
        self.command_at(cmd, None)
    }

    /// Set the clock (Unix milliseconds) used to date new revisions and comments.
    pub fn set_clock_ms(&mut self, ms: i64) {
        if let Some(s) = self.session.as_mut() {
            s.set_clock(Some(crate::review::iso_date(ms)));
        }
    }

    /// Run a command at host time `now` (Unix ms; the system clock when absent).
    fn command_at(&mut self, cmd: Command, now: Option<i64>) -> crate::Result<EditResult> {
        if let Some(mut r) = self.special_command(&cmd)? {
            // Header/footer and note editors mirror into the document: share it live.
            if r.collab.is_none() {
                r.collab = self.collab_after_edit();
            }
            return Ok(r);
        }
        let mut extra = (None, None, None);
        let mut error = None;
        let mut changed = false; // set when the document version moves
        let mut relayout = false; // display options changed
        let review = matches!(
            cmd,
            Command::State
                | Command::Review
                | Command::SetTrackChanges { .. }
                | Command::SetAuthor { .. }
                | Command::SetRevisionView { .. }
                | Command::NextRevision { .. }
                | Command::SelectComment { .. }
                | Command::NextComment { .. }
        );
        if let Command::SetRevisionView { view } = cmd {
            relayout = self.view != view;
            self.view = view;
        }
        if let Command::SetAuthor { name } = &cmd {
            let n = name.trim();
            self.author = (!n.is_empty()).then(|| n.to_owned());
        }
        let note_cmd = matches!(
            cmd,
            Command::InsertNote { .. }
                | Command::SetNoteText { .. }
                | Command::SetNoteOptions { .. }
        );
        let merge_cmd = matches!(
            cmd,
            Command::State
                | Command::MergeData { .. }
                | Command::MergePreview { .. }
                | Command::MergeToDocument
                | Command::MergeInsertField { .. }
        );
        match &cmd {
            Command::SetSavePassword { password } => {
                self.set_save_password(password.as_deref());
            }
            Command::MergeData { name, base64 } => {
                match crate::merge::base64_decode(base64)
                    .ok_or_else(|| crate::EngineError::BadCommand("base64".into()))
                    .and_then(|b| crate::merge::read_data(&b, name))
                {
                    Ok(d) => {
                        self.merge = Some(d);
                        relayout |= self.merge_preview.is_some();
                    }
                    Err(e) => error = Some(e.to_string()),
                }
            }
            Command::MergePreview { record } => {
                let n = self.merge.as_ref().map_or(0, |m| m.rows.len());
                let r = record.filter(|_| n > 0).map(|r| r.min(n - 1));
                relayout |= r != self.merge_preview;
                self.merge_preview = r;
            }
            Command::NewFromTemplate { id, name } => match crate::templates::build(id) {
                Some(doc) => {
                    self.load(doc, name)?;
                    changed = true;
                }
                None => error = Some(format!("unknown template {id}")),
            },
            Command::MergeToDocument => match (&self.merge, self.session.as_ref()) {
                (Some(m), Some(s)) if !m.rows.is_empty() => {
                    let merged = bo_edit::merge::merge_all(s.doc(), m);
                    let stem = self
                        .name
                        .rsplit_once('.')
                        .map_or(self.name.clone(), |(b, _)| b.to_owned());
                    self.load(merged, &format!("{stem}-merged.docx"))?;
                    changed = true;
                }
                _ => error = Some("mail merge: no data source".into()),
            },
            _ => {}
        }
        if let Some(ms) = now.or_else(crate::review::system_ms) {
            self.set_clock_ms(ms);
        }
        let keep_goal = matches!(
            cmd,
            Command::Move {
                motion: Motion::Up | Motion::Down | Motion::PageUp | Motion::PageDown,
                ..
            }
        );
        let mut targets = None;
        let mut new_note = None;
        {
            let layout = self.layout.document();
            let s = self
                .session
                .as_mut()
                .ok_or(crate::EngineError::NoDocument)?;
            let (fonts, cache, opts) = (
                &mut self.fonts,
                &mut self.cache,
                LayoutOptions { view: self.view },
            );
            // Lays a document out for field updates (page numbers per body paragraph).
            let mut pages_of = |doc: &bo_model::Document| -> Vec<u32> {
                let n = bo_model::index::body_paragraphs(doc).len();
                bo_layout::layout_document_with(doc, fonts, cache, opts)
                    .map(|l| page_numbers(&l, n))
                    .unwrap_or_default()
            };
            let before_version = s.version();
            let sel = s.selection();
            let cmd_kind = if matches!(cmd, Command::AcceptRevision { .. }) {
                CmdKind::Accept
            } else {
                CmdKind::Other
            };
            let res: bo_edit::Result<()> = (|| {
                match cmd {
                    // Handled outside the session (view options, merge state).
                    Command::State
                    | Command::Review
                    | Command::SetRevisionView { .. }
                    | Command::MergeData { .. }
                    | Command::MergePreview { .. }
                    | Command::MergeToDocument
                    | Command::NewFromTemplate { .. }
                    | Command::SetSavePassword { .. }
                    // Handled by `special_command`.
                    | Command::EditHeaderFooter { .. }
                    | Command::CloseHeaderFooter
                    | Command::SetHeaderLink { .. }
                    | Command::EditNote { .. }
                    | Command::Notes
                    | Command::FontList => {}
                    Command::Tab { back } => {
                        if s.next_cell(back)?.is_none() && !back {
                            s.insert_text("\t")?;
                        }
                    }
                    Command::TableEdit { edit } => {
                        s.edit_table(&edit)?;
                    }
                    Command::SetPageSetup { setup, scope } => s.set_page_setup(setup, scope)?,
                    Command::SetHeaderOptions {
                        title_page,
                        even_and_odd,
                    } => {
                        let si = s.section_state().index;
                        s.set_header_options(si, title_page, even_and_odd)?;
                    }
                    Command::InsertField { instr } => {
                        let cached = if instr.trim().eq_ignore_ascii_case("NUMPAGES") {
                            layout.pages.len().to_string()
                        } else {
                            "1".to_owned()
                        };
                        s.insert_field(&instr, &cached)?;
                    }
                    Command::InsertNote { endnote, text } => {
                        new_note = Some((endnote, s.insert_note(endnote, &text)?));
                    }
                    Command::SetNoteText { endnote, id, text } => {
                        s.set_note_text(endnote, id, &text)?;
                    }
                    Command::SetNoteOptions {
                        endnote,
                        options,
                        scope,
                    } => s.set_note_options(endnote, &options, scope)?,
                    Command::ClearFormatting => {
                        s.clear_formatting()?;
                    }
                    Command::PickFormat => {
                        s.pick_format();
                    }
                    Command::ApplyFormat => {
                        s.apply_picked_format()?;
                    }
                    Command::CancelFormat => s.clear_picked_format(),
                    Command::SetSelection { selection } => s.set_selection(selection),
                    Command::Click {
                        page,
                        x,
                        y,
                        extend,
                        count,
                    } => {
                        if let Some(p) = nav::hit_test(layout, page, x, y) {
                            let new = match count {
                                2 => s.word_at(p),
                                3.. => s.paragraph_at(p),
                                _ if extend => Selection {
                                    anchor: sel.anchor,
                                    focus: p,
                                },
                                _ => Selection::caret(p),
                            };
                            s.set_selection(new);
                        }
                    }
                    Command::DragTo { page, x, y } => {
                        if let Some(p) = nav::hit_test(layout, page, x, y) {
                            s.set_selection(Selection {
                                anchor: sel.anchor,
                                focus: p,
                            });
                        }
                    }
                    Command::Move { motion, extend } => {
                        let from = sel.focus;
                        let to = match motion {
                            Motion::Left if !extend && !sel.is_collapsed() => sel.ordered().0,
                            Motion::Right if !extend && !sel.is_collapsed() => sel.ordered().1,
                            Motion::Left => s.char_step(from, false),
                            Motion::Right => s.char_step(from, true),
                            Motion::WordLeft => s.word_boundary(from, false),
                            Motion::WordRight => s.word_boundary(from, true),
                            Motion::DocStart => Pos::new(0, 0),
                            Motion::DocEnd => {
                                let last = s.paragraph_count().saturating_sub(1);
                                Pos::new(last, s.para_text(last).map_or(0, |t| t.len()))
                            }
                            _ => match nav::vertical(layout, from, motion, self.goal_x) {
                                Some((p, x)) => {
                                    if keep_goal {
                                        self.goal_x = Some(x);
                                    }
                                    p
                                }
                                None if motion == Motion::Up => Pos::new(from.para, 0),
                                None if motion == Motion::Down => Pos::new(
                                    from.para,
                                    s.para_text(from.para).map_or(0, |t| t.len()),
                                ),
                                None => from,
                            },
                        };
                        s.set_selection(if extend {
                            Selection {
                                anchor: sel.anchor,
                                focus: to,
                            }
                        } else {
                            Selection::caret(to)
                        });
                    }
                    Command::SelectAll => {
                        s.select_all();
                    }
                    Command::InsertText { text } => {
                        s.insert_text(&text)?;
                    }
                    Command::DeleteBackward => {
                        s.delete_backward()?;
                    }
                    Command::DeleteForward => {
                        s.delete_forward()?;
                    }
                    Command::SplitParagraph => {
                        s.split_paragraph()?;
                    }
                    Command::FormatText { format } => {
                        s.format_text(&format)?;
                    }
                    Command::FormatParagraph { format } => {
                        s.format_paragraphs(&format)?;
                    }
                    Command::ApplyStyle { id } => {
                        s.apply_style(&id)?;
                    }
                    Command::InsertTable { rows, cols } => {
                        s.insert_table(rows, cols)?;
                    }
                    Command::InsertPageBreak => {
                        s.insert_page_break()?;
                    }
                    Command::InsertSectionBreak { kind } => {
                        s.insert_section_break(kind.into())?;
                    }
                    Command::Undo => {
                        s.undo();
                    }
                    Command::Redo => {
                        s.redo();
                    }
                    Command::Find { query, options } => {
                        extra.0 = Some(s.find(&query, &options)?);
                    }
                    Command::SelectMatch { m } => s.set_selection(Selection {
                        anchor: Pos::new(m.para, m.start),
                        focus: Pos::new(m.para, m.end),
                    }),
                    Command::Replace { m, with } => {
                        s.replace(m, &with)?;
                    }
                    Command::ReplaceAll {
                        query,
                        options,
                        with,
                    } => {
                        extra.1 = Some(s.replace_all(&query, &options, &with)?);
                    }
                    Command::Copy => {
                        let c = s.copy();
                        extra.2 = Some(ClipOut {
                            text: c.text,
                            html: c.html,
                            internal: c.internal,
                        });
                    }
                    Command::Cut => {
                        let c = s.cut()?;
                        extra.2 = Some(ClipOut {
                            text: c.text,
                            html: c.html,
                            internal: c.internal,
                        });
                    }
                    Command::Paste {
                        internal,
                        html,
                        text,
                        mode: Some(mode),
                    } => {
                        s.paste_with(mode, internal.as_deref(), html.as_deref(), text.as_deref())?;
                    }
                    Command::Paste {
                        internal,
                        html,
                        text,
                        mode: None,
                    } => {
                        if let Some(j) = internal.filter(|j| !j.is_empty()) {
                            s.paste_internal(&j)?;
                        } else if let Some(h) = html.filter(|h| !h.is_empty()) {
                            s.paste_html(&h)?;
                        } else if let Some(t) = text {
                            s.paste_text(&t)?;
                        }
                    }
                    Command::SetTrackChanges { on } => s.set_track_changes(on),
                    Command::SetAuthor { name } => s.set_author(&name),
                    Command::AcceptRevision { id } | Command::RejectRevision { id } => {
                        let accept = matches!(cmd_kind, CmdKind::Accept);
                        let ids = id.map_or_else(|| s.revisions_at_selection(), |i| vec![i]);
                        if !ids.is_empty() {
                            extra.1 = Some(s.resolve_revisions(Some(&ids), accept)?);
                        }
                    }
                    Command::AcceptAllRevisions => {
                        extra.1 = Some(s.resolve_revisions(None, true)?);
                    }
                    Command::RejectAllRevisions => {
                        extra.1 = Some(s.resolve_revisions(None, false)?);
                    }
                    Command::NextRevision { forward } => {
                        extra.1 = Some(usize::from(s.select_revision(forward).is_some()));
                    }
                    Command::AddComment { text } => {
                        extra.1 = Some(s.add_comment(&text)?.max(0) as usize);
                    }
                    Command::ReplyComment { id, text } => {
                        extra.1 = Some(s.reply_comment(id, &text)?.max(0) as usize);
                    }
                    Command::ResolveComment { id, done } => s.resolve_comment(id, done)?,
                    Command::DeleteComment { id } => s.delete_comment(id)?,
                    Command::SelectComment { id } => {
                        extra.1 = Some(usize::from(s.select_comment(id)));
                    }
                    Command::NextComment { forward } => {
                        extra.1 = s.next_comment(forward).map(|i| i.max(0) as usize);
                    }
                    Command::UpdateFields => {
                        extra.1 = Some(s.update_fields(&mut pages_of)?);
                    }
                    Command::InsertToc => {
                        extra.1 = Some(s.insert_toc(&mut pages_of)?);
                    }
                    Command::InsertCaption { label, text } => {
                        s.insert_caption(&label, &text)?;
                    }
                    Command::CrossReferenceTargets => targets = Some(s.ref_targets()),
                    Command::InsertCrossReference { target, page } => {
                        s.insert_cross_reference(&target, page, &mut pages_of)?;
                    }
                    Command::MergeInsertField { name } => {
                        let n = name.trim();
                        let quoted = if n.contains(' ') {
                            format!("\"{n}\"")
                        } else {
                            n.to_owned()
                        };
                        s.insert_field(
                            &format!("MERGEFIELD {quoted} \\* MERGEFORMAT"),
                            &bo_edit::merge::placeholder(n),
                        )?;
                    }
                }
                Ok(())
            })();
            if let Err(e) = res {
                error = Some(e.to_string());
            }
            changed |= s.version() != before_version;
        }
        if !keep_goal {
            self.goal_x = None;
        }
        let dirty = if changed || relayout {
            self.relayout()?
        } else {
            Vec::new()
        };
        if let Some((endnote, id)) = new_note {
            // The caret goes into the new note on the page.
            if let Some(mut r) = self.open_new_note(endnote, id, dirty.clone())? {
                r.notes = self.session.as_ref().map(bo_edit::Session::notes);
                r.collab = self.collab_after_edit();
                return Ok(r);
            }
        }
        let mut r = self.edit_result(dirty, extra, error)?;
        r.targets = targets;
        if note_cmd {
            r.notes = self.session.as_ref().map(bo_edit::Session::notes);
        }
        r.collab = self.collab_after_edit();
        if merge_cmd {
            r.merge = self.merge.as_ref().map(|m| crate::merge::MergeState {
                name: m.name.clone(),
                fields: m.fields.clone(),
                count: m.rows.len(),
                preview: self.merge_preview,
            });
        }
        if changed || relayout || review {
            if let Some(s) = self.session.as_ref() {
                r.review = Some(crate::review::review_state(s, self.view));
            }
        }
        Ok(r)
    }

    /// Insert a picture (PNG or JPEG) at the selection; sized at 96 dpi, capped to the text width.
    pub fn insert_image(&mut self, bytes: Vec<u8>, name: &str) -> crate::Result<EditResult> {
        let (w, h) =
            image_size(&bytes).ok_or_else(|| crate::EngineError::Unsupported(name.to_owned()))?;
        let ext = match bo_render::image::sniff(&bytes) {
            bo_render::image::Format::Jpeg => "jpeg",
            _ => "png",
        };
        let s = self
            .session
            .as_mut()
            .ok_or(crate::EngineError::NoDocument)?;
        let max_w = s.doc().sections.first().map_or(453.0, |sec| {
            (sec.page.width.0 - sec.page.margin_left.0 - sec.page.margin_right.0) as f32 / 20.0
        });
        // 96 dpi: 1 px = 0.75 pt = 9525 EMU.
        let mut wpt = w as f32 * 0.75;
        let mut hpt = h as f32 * 0.75;
        if wpt > max_w {
            hpt *= max_w / wpt;
            wpt = max_w;
        }
        let res = s.insert_image(bytes, ext, (wpt * 12_700.0) as i64, (hpt * 12_700.0) as i64);
        let error = res.err().map(|e| e.to_string());
        let dirty = self.relayout()?;
        let mut r = self.edit_result(dirty, (None, None, None), error)?;
        r.collab = self.collab_after_edit();
        Ok(r)
    }

    pub(crate) fn edit_result(
        &self,
        dirty: Vec<usize>,
        extra: (Option<Vec<Match>>, Option<usize>, Option<ClipOut>),
        error: Option<String>,
    ) -> crate::Result<EditResult> {
        let layout = self.layout.document();
        let s = self
            .session
            .as_ref()
            .ok_or(crate::EngineError::NoDocument)?;
        let sel = s.selection();
        Ok(EditResult {
            info: self.info()?,
            selection: sel,
            caret: nav::caret_rect(layout, sel.focus),
            rects: nav::selection_rects(layout, sel),
            dirty,
            format: s.format_state(),
            modified: self.is_modified(),
            matches: extra.0,
            count: extra.1,
            clip: extra.2,
            error,
            review: None,
            targets: None,
            merge: None,
            collab: None,
            story: None,
            notes: None,
            fonts: None,
            painter: s.picked_format().is_some(),
        })
    }
}

/// Displayed page number of every body paragraph (0 when not laid out).
pub fn page_numbers(layout: &bo_layout::LayoutDocument, paragraphs: usize) -> Vec<u32> {
    let mut v = vec![0u32; paragraphs];
    for p in &layout.pages {
        for l in &p.lines {
            if let bo_layout::LineSource::Body(id) = l.source {
                if let Some(slot) = v.get_mut(id as usize) {
                    if *slot == 0 {
                        *slot = p.number;
                    }
                }
            }
        }
    }
    v
}

/// Distinguishes commands sharing a match arm.
enum CmdKind {
    Accept,
    Other,
}

/// Pixel size of a PNG (IHDR) or JPEG (SOFn) picture.
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    match bo_render::image::sniff(bytes) {
        bo_render::image::Format::Png => {
            // ISO/IEC 15948 §11.2.2 IHDR: width and height follow the chunk header.
            let w = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
            let h = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
            (w > 0 && h > 0).then_some((w, h))
        }
        bo_render::image::Format::Jpeg => {
            bo_render::image::jpeg_info(bytes).map(|(w, h, _)| (w, h))
        }
        bo_render::image::Format::Other => None,
    }
}
