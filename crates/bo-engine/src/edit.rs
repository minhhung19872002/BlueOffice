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
    /// New page.
    NextPage,
    /// Same page.
    Continuous,
    /// Next even-numbered page.
    EvenPage,
    /// Next odd-numbered page.
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
    /// Replace the selection.
    SetSelection {
        /// New anchor and focus positions.
        selection: Selection,
    },
    /// Mouse click: `count` 1 = caret, 2 = word, 3 = paragraph; `extend` = Shift held.
    Click {
        /// 0-based page index.
        page: usize,
        /// Horizontal position in points from the page's left edge.
        x: f32,
        /// Vertical position in points from the page's top edge.
        y: f32,
        /// Extend the selection from its anchor (Shift held).
        #[serde(default)]
        extend: bool,
        /// Click count: 1 = caret, 2 = word, 3 or more = paragraph.
        #[serde(default = "one")]
        count: u8,
    },
    /// Mouse drag to a point (extends the selection).
    DragTo {
        /// 0-based page index.
        page: usize,
        /// Horizontal position in points from the page's left edge.
        x: f32,
        /// Vertical position in points from the page's top edge.
        y: f32,
    },
    /// Keyboard caret motion (arrows, Home/End, word / paragraph steps, …).
    Move {
        /// How the caret moves.
        motion: Motion,
        /// Extend the selection instead of moving the caret (Shift held).
        #[serde(default)]
        extend: bool,
    },
    /// Select the whole body.
    SelectAll,
    /// Type text at the selection (replacing it).
    InsertText {
        /// Text to insert.
        text: String,
    },
    /// Backspace: delete the selection or the character before the caret.
    DeleteBackward,
    /// Delete: delete the selection or the character after the caret.
    DeleteForward,
    /// Enter: split the paragraph at the caret.
    SplitParagraph,
    /// Apply character formatting to the selection.
    FormatText {
        /// Properties to set; absent fields are kept.
        format: CharFormat,
    },
    /// Apply paragraph formatting to the selected paragraphs.
    FormatParagraph {
        /// Properties to set; absent fields are kept.
        format: ParaFormat,
    },
    /// Apply a paragraph style to the selected paragraphs.
    ApplyStyle {
        /// Style id (`w:styleId`).
        id: String,
    },
    /// Define a new multilevel list (ECMA-376 Part 1, §17.9.1 abstractNum, §17.9.6 lvl)
    /// whose levels may be linked to paragraph styles (§17.9.23 pStyle).
    DefineList {
        /// Levels 1… (at most 9; missing levels get defaults).
        levels: Vec<bo_edit::ListLevelSpec>,
        /// Number the selected paragraphs with the new list.
        #[serde(default = "yes")]
        apply: bool,
    },
    /// Set numbering value: restart the list at the caret at a value or continue the
    /// list above (§17.9.15 num, §17.9.8 lvlOverride, §17.9.26 startOverride).
    SetListNumbering {
        /// Restart at a value or continue.
        numbering: bo_edit::ListNumbering,
    },
    /// Insert an empty table at the caret.
    InsertTable {
        /// Number of rows.
        rows: usize,
        /// Number of columns.
        cols: usize,
    },
    /// Insert a page break at the caret.
    InsertPageBreak,
    /// Insert a section break at the caret (ECMA-376 Part 1, §17.6.17).
    InsertSectionBreak {
        /// Where the new section starts.
        kind: BreakKind,
    },
    /// Undo the last edit.
    Undo,
    /// Redo the last undone edit.
    Redo,
    /// Number of undo steps (in `count`; CORE-SCRIPT-001).
    HistoryLen,
    /// Join the undo steps after the first `since` into one (in `count`: the history
    /// length afterwards; a BlueScript run is one undo step, CORE-SCRIPT-001).
    MergeHistory {
        /// History length before the steps to join.
        since: usize,
    },
    /// Find every match of a query (returned in `matches`).
    Find {
        /// Text to look for.
        query: String,
        /// Match case, whole word and regular-expression options.
        #[serde(default)]
        options: FindOptions,
        /// Find by format: only text with this formatting (an empty query finds every
        /// stretch of text with it).
        #[serde(default)]
        format: Option<bo_edit::find::FindFormat>,
        /// Also return the highlight rectangles of every match (`highlights`).
        #[serde(default)]
        highlight: bool,
    },
    /// Select a match returned by `find`.
    SelectMatch {
        /// The match to select.
        #[serde(rename = "match")]
        m: Match,
    },
    /// Replace one match.
    Replace {
        /// The match to replace.
        #[serde(rename = "match")]
        m: Match,
        /// Replacement text.
        with: String,
        /// The query that found the match: groups of `with` (`$1`, `\1`) are expanded
        /// against it; without it only special characters (`^p`, `^&` …) are.
        #[serde(default)]
        query: Option<String>,
        /// Options of `query`.
        #[serde(default)]
        options: FindOptions,
    },
    /// Replace every match of a query (the number replaced is returned in `count`).
    ReplaceAll {
        /// Text to look for.
        query: String,
        /// Match case, whole word and regular-expression options.
        #[serde(default)]
        options: FindOptions,
        /// Replacement text.
        with: String,
        /// Only replace matches with this formatting (find by format); `None` = any.
        #[serde(default)]
        format: Option<bo_edit::find::FindFormat>,
    },
    /// Copy the selection (returned in `clip`).
    Copy,
    /// Cut the selection (returned in `clip`).
    Cut,
    /// Paste: the internal format wins over HTML, HTML over plain text.
    Paste {
        /// BlueOffice internal clipboard payload (from `clip.internal`).
        #[serde(default)]
        internal: Option<String>,
        /// HTML clipboard content.
        #[serde(default)]
        html: Option<String>,
        /// Plain-text clipboard content.
        #[serde(default)]
        text: Option<String>,
        /// Paste Special: keep source formatting (default), merge, or text only.
        #[serde(default)]
        mode: Option<PasteMode>,
    },
    /// Review state only (track changes, revisions, comments).
    Review,
    /// Convert text in legacy Vietnamese fonts (TCVN3 `.Vn*`, VNI-*) to Unicode
    /// and switch those runs to a Unicode font; one undoable step.
    ConvertLegacyText,
    /// Track-changes mode on/off (ECMA-376 Part 1, §17.13.5).
    SetTrackChanges {
        /// `true` records insertions and deletions as revisions.
        on: bool,
    },
    /// Author of new revisions and comments (the signed-in user; empty = "BlueOffice").
    SetAuthor {
        /// Author name.
        name: String,
    },
    /// Markup / final / original display of tracked changes.
    SetRevisionView {
        /// Which version of the text to display.
        view: RevisionView,
    },
    /// Review display mode and markup options (absent fields are kept).
    SetMarkup(crate::markup::MarkupUpdate),
    /// Accept one revision by id, or those at the selection when `id` is absent.
    AcceptRevision {
        /// Revision id.
        #[serde(default)]
        id: Option<i32>,
    },
    /// Reject one revision by id, or those at the selection when `id` is absent.
    RejectRevision {
        /// Revision id.
        #[serde(default)]
        id: Option<i32>,
    },
    /// Move the paragraphs of the selection above the previous paragraph (`up`) or
    /// below the next one; in track-changes mode a tracked move (§17.13.5.22 moveFrom,
    /// §17.13.5.25 moveTo).
    MoveParagraphs {
        /// Up (`true`) or down.
        up: bool,
    },
    /// Compare with an original document (prompt §7.1): this document's differences from it become
    /// tracked changes (`.docx`, `.odt` or `.doc`).
    CompareWith {
        /// File name of the original.
        name: String,
        /// File bytes, base64-encoded.
        base64: String,
    },
    /// Combine with another reviewer's copy of the same original (prompt §7.1): both
    /// reviewers' changes as tracked changes (`.docx`, `.odt` or `.doc`).
    CombineWith {
        /// File name of the other copy (its stem names the reviewer when it has no revisions).
        name: String,
        /// File bytes, base64-encoded.
        base64: String,
    },
    /// Lock (`lock: true`) or unlock the fields in the selection (§17.16.18 fldLock).
    LockFields {
        /// Lock or unlock.
        lock: bool,
    },
    /// Replace the fields in the selection by their results (plain text).
    UnlinkFields,
    /// Show field codes instead of field results (`on`), or the results again.
    ShowFieldCodes {
        /// Codes shown.
        on: bool,
    },
    /// Insert an inline content control (§17.5.2.31 sdt) at the caret.
    InsertControl {
        /// `text` | `richText` | `checkbox` | `dropDown` | `comboBox` | `date`.
        kind: String,
        /// Placeholder text shown until the control is filled.
        #[serde(default)]
        placeholder: String,
        /// Items of a list control.
        #[serde(default)]
        items: Vec<String>,
    },
    /// Fill the content control around the caret: toggle a checkbox, pick an item, set a date
    /// (ISO 8601) or text.
    SetControlValue {
        /// The value (ignored by a checkbox).
        #[serde(default)]
        value: String,
    },
    /// Insert a legacy form field (§17.16.17 ffData) at the caret.
    InsertFormField {
        /// `text` | `checkBox` | `dropDown`.
        kind: String,
    },
    /// Change the properties of the form field at the caret (its type stays).
    SetFormField {
        /// The new field data.
        field: Box<bo_model::FormField>,
    },
    /// Fill in the form field at the caret: a text field's result, a check box (`""` toggles,
    /// `"1"` / `"0"`), a drop-down entry (its text or index).
    SetFormFieldValue {
        /// The value.
        #[serde(default)]
        value: String,
    },
    /// Select the next (`back`: previous) form field that can be filled in.
    NextFormField {
        /// Backwards (Shift+Tab).
        #[serde(default)]
        back: bool,
    },
    /// Typing into the text form field at the caret while filling in a protected form (hosts
    /// send `insertText`; the restriction turns it into this).
    FormType {
        /// Typed text.
        text: String,
    },
    /// Deleting in the text form field at the caret while filling in a protected form.
    FormDelete {
        /// Backwards (Backspace).
        #[serde(default)]
        back: bool,
    },
    /// Start an editing restriction (§17.15.1.29 documentProtection), by default filling in
    /// forms, with an optional password and the sections left editable (§17.6.6 formProt).
    ProtectDocument {
        /// `ST_DocProtect` value (default `forms`).
        #[serde(default)]
        edit: String,
        /// Password needed to stop the restriction.
        #[serde(default)]
        password: Option<String>,
        /// Indices of the sections left editable (`None`: unchanged).
        #[serde(default)]
        unprotected_sections: Option<Vec<usize>>,
    },
    /// Stop the editing restriction (the password must match when one was set).
    UnprotectDocument {
        /// The password.
        #[serde(default)]
        password: Option<String>,
    },
    /// Accept every revision of the document.
    AcceptAllRevisions,
    /// Document properties of the File › Info page (CORE-UI-002) in `docProperties`.
    DocProperties,
    /// Set the editable document properties (written when the file is next saved).
    SetDocProperties {
        /// Fields to change (absent: unchanged, empty: removed).
        props: crate::props::PropsEdit,
    },
    /// Check document (CORE-INSPECT-001): inspector, accessibility and
    /// compatibility report in `inspection`.
    Inspect,
    /// Remove what the inspector found in `categories`: model content as one undo
    /// step, package parts when the document is next saved. Returns the new report.
    InspectRemove {
        /// Inspector categories.
        categories: Vec<crate::inspect::Category>,
    },
    /// Keep package-level categories queued by `inspectRemove` (cancel the removal).
    InspectKeep {
        /// Inspector categories.
        categories: Vec<crate::inspect::Category>,
    },
    /// Apply an accessibility quick fix (alternative text, header row); one undo step.
    InspectFix {
        /// The fix.
        fix: crate::inspect::Fix,
    },
    /// Reject every revision of the document.
    RejectAllRevisions,
    /// Select the next (`forward`) or previous revision.
    NextRevision {
        /// `true` = next, `false` = previous.
        #[serde(default = "yes")]
        forward: bool,
    },
    /// Comment on the selection (ECMA-376 Part 1, §17.13.4).
    AddComment {
        /// Comment text.
        text: String,
        /// @mentions in `text` (UTF-16 ranges; CORE-COMMENT-001).
        #[serde(default)]
        mentions: Vec<bo_model::mention::Mention>,
        /// Task assigned to a mentioned person.
        #[serde(default)]
        task: Option<bo_model::mention::CommentTask>,
    },
    /// Reply to comment `id` ([MS-DOCX] threading).
    ReplyComment {
        /// Id of the comment replied to.
        id: i32,
        /// Reply text.
        text: String,
        /// @mentions in `text`.
        #[serde(default)]
        mentions: Vec<bo_model::mention::Mention>,
        /// Task assigned to a mentioned person.
        #[serde(default)]
        task: Option<bo_model::mention::CommentTask>,
    },
    /// Mark a thread resolved or open again.
    ResolveComment {
        /// Comment id.
        id: i32,
        /// `true` = resolved, `false` = open.
        #[serde(default = "yes")]
        done: bool,
    },
    /// Delete a comment and its replies.
    DeleteComment {
        /// Comment id.
        id: i32,
    },
    /// Select the text a comment is anchored to.
    SelectComment {
        /// Comment id.
        id: i32,
    },
    /// Select the next (`forward`) or previous comment.
    NextComment {
        /// `true` = next, `false` = previous.
        #[serde(default = "yes")]
        forward: bool,
    },
    /// Update TOC, SEQ, REF and PAGEREF fields (ECMA-376 Part 1, §17.16).
    UpdateFields,
    /// Insert a table of contents at the caret.
    InsertToc,
    /// Insert a table of figures (`TOC \c` / `\a`), an index (`INDEX`) or a table of
    /// authorities (`TOA`) at the caret and fill it (ECMA-376 Part 1, §17.16.5.68,
    /// §17.16.5.28, §17.16.5.67).
    InsertReferenceTable {
        /// Field instruction.
        instr: String,
        /// Leader of the right-aligned page numbers: `dot`, `hyphen`, `underscore`, `none`.
        #[serde(default)]
        leader: Option<String>,
        /// Text columns of the table (> 1: its own continuous section).
        #[serde(default)]
        columns: u16,
    },
    /// Mark the selection as an index entry (§17.16.5.72 XE); `all`: the first
    /// occurrence of the selected text in every paragraph too.
    MarkIndexEntry {
        /// The entry.
        mark: bo_edit::IndexMark,
        /// Mark every occurrence.
        #[serde(default)]
        all: bool,
    },
    /// Mark the selection as a citation (§17.16.5.62 TA); `all`: later mentions too.
    MarkCitation {
        /// The citation.
        mark: bo_edit::CitationMark,
        /// Mark every occurrence.
        #[serde(default)]
        all: bool,
    },
    /// Convert footnotes ↔ endnotes, all or those referenced in the selection.
    ConvertNotes {
        /// Direction.
        mode: bo_edit::NoteConversion,
        /// Only the notes referenced inside the selection.
        #[serde(default)]
        selection: bool,
    },
    /// Insert a caption (`label n: text`) after the caret's paragraph.
    InsertCaption {
        /// Caption label (`Figure`, `Table`, …).
        label: String,
        /// Caption text after the number.
        #[serde(default)]
        text: String,
    },
    /// List the cross-reference targets (bookmarks and captions).
    CrossReferenceTargets,
    /// Insert a cross-reference to a target: its text or (`page`) its page number.
    InsertCrossReference {
        /// Target id from `crossReferenceTargets`.
        target: String,
        /// Insert the target's page number instead of its text.
        #[serde(default)]
        page: bool,
    },
    /// Mail-merge data source (CSV or XLSX bytes, base64).
    MergeData {
        /// File name (its extension picks CSV or XLSX).
        name: String,
        /// File bytes, base64-encoded.
        base64: String,
    },
    /// Insert `MERGEFIELD name` (ECMA-376 Part 1, §17.16.5.35) at the caret.
    MergeInsertField {
        /// Merge field (column) name.
        name: String,
    },
    /// Show record `record` (0-based) in the merge fields, or the field names.
    MergePreview {
        /// 0-based record index; `None` shows the field names.
        #[serde(default)]
        record: Option<usize>,
    },
    /// Replace the open document by the merged one (one section per record).
    MergeToDocument,
    /// Mail-merge data handed over by a host data provider (e.g. a SQL query the
    /// server ran): field names and rows of display text.
    MergeTable {
        /// Data source name.
        name: String,
        /// Field names.
        fields: Vec<String>,
        /// Records.
        rows: Vec<Vec<String>>,
    },
    /// Filter, sort and excluded records of the recipient list.
    MergeSetRecipients {
        /// The list options.
        options: bo_edit::merge::recipients::RecipientOptions,
    },
    /// Records of the data source for the recipient list editor (`mergeOut.records`).
    MergeRecords {
        /// First data row.
        #[serde(default)]
        offset: usize,
        /// Number of rows (default 500).
        #[serde(default)]
        limit: Option<usize>,
    },
    /// Groups of duplicate records (`mergeOut.duplicates`); no fields = every field.
    MergeDuplicates {
        /// Fields compared.
        #[serde(default)]
        fields: Vec<String>,
    },
    /// Insert a merge rule field (IF, SKIPIF, NEXTIF, NEXT, MERGEREC, MERGESEQ, ASK,
    /// FILLIN, SET; ECMA-376 Part 1, §17.16.5) at the caret.
    MergeInsertRule {
        /// The rule.
        rule: bo_edit::merge::rules::MergeRule,
    },
    /// ASK / FILLIN prompts of the document (`mergeOut.prompts`).
    MergePrompts,
    /// Answers to the prompts, used by the preview and the next merges.
    MergeSetAnswers {
        /// Prompt key → one answer, or one per record of the recipient list.
        answers: bo_edit::merge::rules::Answers,
    },
    /// One PDF per merged document, zipped (`mergeOut.zip`, `mergeOut.files`).
    MergeToPdfs {
        /// File name pattern (`«Field»` replaced by the record's value).
        #[serde(default)]
        pattern: String,
    },
    /// One e-mail per merged document (`mergeOut.mails`), sent by the host's
    /// `MailProvider`.
    MergeToEmails {
        /// Field holding the address.
        to: String,
        /// Subject pattern (`«Field»` replaced by the record's value).
        subject: String,
        /// Attachment: `none` (default), `pdf` or `docx`.
        #[serde(default)]
        attach: String,
        /// Attachment file name pattern.
        #[serde(default)]
        attachment_name: String,
    },
    /// Replace the open document by an envelope (delivery address lines with
    /// `«Field»` merge fields).
    MergeEnvelope {
        /// Envelope size.
        size: bo_edit::merge::sheets::EnvelopeSize,
        /// Delivery address lines.
        delivery: String,
        /// Return address lines.
        #[serde(default)]
        return_address: String,
        /// Delivery address position from the page's top-left corner, mm.
        #[serde(default)]
        at: Option<(f64, f64)>,
    },
    /// Replace the open document by a label sheet.
    MergeLabels {
        /// Sheet geometry.
        sheet: bo_edit::merge::sheets::LabelSheet,
        /// Label lines with `«Field»` merge fields.
        text: String,
        /// One record per label, or the same label on the whole page.
        mode: bo_edit::merge::sheets::LabelMode,
    },
    /// Label sheet and envelope presets (`mergeOut.presets`).
    MergePresets,
    /// Password for saving ([MS-OFFCRYPTO] Agile Encryption); empty or absent: none.
    SetSavePassword {
        /// New password; `None` or empty saves unencrypted.
        #[serde(default)]
        password: Option<String>,
    },
    /// Package variant of the next saves by file extension (`docx`, `dotx`, `docm`,
    /// `dotm`; CORE-FMT-001). A macro-free variant drops the VBA project on save.
    SaveFormat {
        /// File extension without the dot.
        ext: String,
    },
    /// Tab key: next (`back`: previous) table cell, else a tab character.
    Tab {
        /// Shift+Tab: move to the previous cell.
        #[serde(default)]
        back: bool,
    },
    /// Table editing at the selection (ECMA-376 Part 1, §17.4).
    TableEdit {
        /// The table operation (insert / delete rows and columns, merge, …).
        edit: TableEdit,
    },
    /// Move the floating picture at the selection to offsets `x`, `y` (points) from
    /// its position references (ECMA-376 Part 1, §20.4.2.10 positionH, §20.4.2.11
    /// positionV); a tracked move with track changes on (§17.13.5.22 / §17.13.5.25).
    SetPicturePosition {
        /// Horizontal offset from the position reference, in points.
        x: f32,
        /// Vertical offset from the position reference, in points.
        y: f32,
    },
    /// Page size, orientation, margins and columns (§17.6.13, §17.6.11, §17.6.4).
    SetPageSetup {
        /// Page setup values to change.
        setup: PageSetupSpec,
        /// Apply to the caret's section or the whole document.
        #[serde(default)]
        scope: SetupScope,
    },
    /// "Different first page" (§17.10.6 titlePg) / "different odd & even" (§17.15.1.35).
    SetHeaderOptions {
        /// Different first page; `None` keeps the current value.
        #[serde(default)]
        title_page: Option<bool>,
        /// Different odd and even pages; `None` keeps the current value.
        #[serde(default)]
        even_and_odd: Option<bool>,
    },
    /// "Link to previous" for the open header/footer (§17.10.5 headerReference inheritance).
    SetHeaderLink {
        /// `true` links to the previous section's header/footer.
        linked: bool,
    },
    /// Marks of misspelt words on `pages` (all when absent), in `spelling`.
    Spelling {
        /// 0-based pages.
        #[serde(default)]
        pages: Option<Vec<usize>>,
    },
    /// The misspelt word at a point with suggestions, in `spell`.
    SpellAt {
        /// 0-based page.
        page: usize,
        /// Point on the page (points).
        x: f32,
        /// Point on the page (points).
        y: f32,
    },
    /// Ignore a word everywhere for this session.
    SpellIgnore {
        /// The word.
        word: String,
    },
    /// Add a word to the user dictionary.
    SpellAddWord {
        /// The word.
        word: String,
    },
    /// Insert an equation typed in the UnicodeMath linear format (or LaTeX with `latex`) at the selection;
    /// with `replace`, the selected equation is replaced (CORE-MATH-007).
    InsertEquation {
        /// The equation text.
        text: String,
        /// LaTeX rather than UnicodeMath.
        #[serde(default)]
        latex: bool,
        /// A display equation on its own line (`m:oMathPara`).
        #[serde(default)]
        display: bool,
        /// Replace the equation the selection covers (or the one just before the caret).
        #[serde(default)]
        replace: bool,
        /// Insert as a numbered equation paragraph with this SEQ label (e.g. `Phương_trình`).
        #[serde(default)]
        numbered: Option<String>,
    },
    /// Insert a phonetic guide (ECMA-376 Part 1, §17.3.3.25 ruby) in place of the selection;
    /// with `replace`, the guide the selection covers (or the one before the caret) is replaced.
    /// Every result reports the selected guide in `ruby`.
    InsertRuby {
        /// Base text, guide text, alignment and sizes.
        ruby: bo_edit::charfx::RubySpec,
        /// Replace the selected guide.
        #[serde(default)]
        replace: bool,
    },
    /// The equation templates ([`crate::equation::TEMPLATES`]): `{ "templates": [{id, linear}] }`.
    EquationTemplates,
    /// Report the selected equation (`equation` of the result; every result carries it).
    Equation,
    /// Show the selected equation in the linear form (`linear`) or build it up (professional form).
    EquationFormat {
        /// Linear form.
        linear: bool,
    },
    /// AutoCorrect options (CORE-AUTOCORR-001): `options` replaces them, `reset`
    /// restores the defaults; the result reports the current ones in `autocorrect`.
    AutoCorrect {
        /// New options.
        #[serde(default)]
        options: Option<Box<bo_edit::AutoCorrectOptions>>,
        /// Restore the default options.
        #[serde(default)]
        reset: bool,
    },
    /// Spelling as you type on or off.
    SetSpelling {
        /// On.
        on: bool,
    },
    /// Grammar and style checks as you type on or off (blue underline).
    SetGrammar {
        /// On.
        on: bool,
    },
    /// Ignore a grammar issue (`rule` id on this exact `text`) for this session.
    GrammarIgnore {
        /// Rule id (`bo_spell::grammar::Rule::id`).
        rule: String,
        /// The flagged text.
        text: String,
    },
    /// Check the whole document (every story) for spelling and grammar, in `proof`
    /// (CORE-SPELL-005).
    ProofCheck,
    /// Corrections of the spelling and grammar check (replace words, set their language) in any
    /// story: one undo step.
    ProofEdit {
        /// The corrections.
        edits: Vec<bo_edit::proofing::ProofEdit>,
    },
    /// Proofing options (absent fields are kept); the result reports them in `proofOptions`.
    ProofOptions {
        /// Do not check words in capitals.
        #[serde(default)]
        ignore_caps: Option<bool>,
        /// Do not check words with digits.
        #[serde(default)]
        ignore_digits: Option<bool>,
        /// Detect the language of typed text and tag it (§17.3.2.20 lang).
        #[serde(default)]
        auto_language: Option<bool>,
    },
    /// Open the header (`header`) or footer of a page for editing (§17.10).
    EditHeaderFooter {
        /// 0-based page index.
        page: usize,
        /// `true` = header, `false` = footer.
        header: bool,
    },
    /// Close the header/footer editor.
    CloseHeaderFooter,
    /// Insert a simple field such as `PAGE` or `NUMPAGES` (§17.16.5.43, §17.16.5.40).
    InsertField {
        /// Field instruction (e.g. `PAGE`).
        instr: String,
    },
    /// Insert a footnote (or endnote) at the caret (§17.11.14, §17.11.7).
    InsertNote {
        /// Insert an endnote instead of a footnote.
        #[serde(default)]
        endnote: bool,
        /// Initial note text.
        #[serde(default)]
        text: String,
    },
    /// Replace the text of a footnote or endnote.
    SetNoteText {
        /// `true` = endnote, `false` = footnote.
        #[serde(default)]
        endnote: bool,
        /// Note id.
        id: i32,
        /// New note text.
        text: String,
    },
    /// List the footnotes and endnotes.
    Notes,
    /// Edit a footnote/endnote in place on its page (caret at the end of its text).
    EditNote {
        /// `true` = endnote, `false` = footnote.
        #[serde(default)]
        endnote: bool,
        /// Note id.
        id: i32,
    },
    /// Footnote/endnote numbering and placement of the caret's section or the document
    /// (§17.11.11 footnotePr, §17.11.4 endnotePr).
    SetNoteOptions {
        /// `true` = endnote options, `false` = footnote options.
        #[serde(default)]
        endnote: bool,
        /// Numbering format, start, restart and position.
        options: bo_edit::section::NoteOptions,
        /// Apply to the caret's section or the whole document.
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
    /// Insert a shape (`preset`, §20.1.10.56 ST_ShapeType) or a text box anchored at
    /// the caret, `width` × `height` points; a text box opens for typing.
    InsertShape {
        /// Preset geometry name (default `rect`).
        #[serde(default = "rect")]
        preset: String,
        /// Insert a text box (a shape holding `txbxContent`) instead of a plain shape.
        #[serde(default)]
        text_box: bool,
        /// Width in points.
        width: f32,
        /// Height in points.
        height: f32,
    },
    /// Move the drawing object at (`para`, `at`) by (`dx`, `dy`) points and/or resize
    /// it to `width` × `height` points (§20.4.2.3 anchor, §20.4.2.7 extent).
    SetDrawingFrame {
        /// Body paragraph id of the anchor.
        para: usize,
        /// Index-text offset of the anchor in that paragraph.
        at: usize,
        /// Horizontal move in points (0 = none).
        #[serde(default)]
        dx: f32,
        /// Vertical move in points (0 = none).
        #[serde(default)]
        dy: f32,
        /// New width in points; `None` keeps the width.
        #[serde(default)]
        width: Option<f32>,
        /// New height in points; `None` keeps the height.
        #[serde(default)]
        height: Option<f32>,
    },
    /// Open the text box at (`para`, `at`) for typing (caret at the end of its text).
    EditTextBox {
        /// Body paragraph id of the anchor.
        para: usize,
        /// Index-text offset of the anchor in that paragraph.
        at: usize,
    },
    /// Close the text box editor.
    CloseTextBox,
    /// Paragraph and character styles for the style gallery (§17.7.4.14 qFormat first).
    StyleList,
    /// Replace the open document by a new one from a built-in template.
    NewFromTemplate {
        /// Template id.
        id: String,
        /// File name of the new document.
        name: String,
    },
    /// Check the document against Nghị định 30/2020/NĐ-CP (administrative document
    /// format): `adminFormat` of the result lists the deviations.
    CheckAdminFormat,
    /// Correct the fixable deviations from Nghị định 30/2020/NĐ-CP (font, size, style,
    /// alignment, paper, margins, body spacing, page number from page 2) in one undoable
    /// edit; `adminFormat` of the result is the check after the fix.
    FixAdminFormat {
        /// Issue ids to fix (every occurrence of each); `None`: every fixable issue.
        #[serde(default)]
        ids: Option<Vec<String>>,
    },
    /// Design tab state: page colour, page borders, watermark, themes, style sets.
    Design,
    /// Set (`None`: remove) the page colour (§17.2.1 background), `RRGGBB`.
    SetPageColor {
        /// New colour.
        #[serde(default)]
        color: Option<String>,
    },
    /// Set (`None`: remove) the page borders (§17.6.10 pgBorders).
    SetPageBorders {
        /// New borders.
        #[serde(default)]
        borders: Option<bo_edit::design::PageBordersSpec>,
        /// Apply to the caret's section or the whole document.
        #[serde(default)]
        scope: SetupScope,
    },
    /// Set (`None`: remove) the text or picture watermark of every page.
    SetWatermark {
        /// New watermark.
        #[serde(default)]
        watermark: Option<crate::design::WatermarkSpec>,
    },
    /// Apply a BlueOffice document theme (fonts and colours).
    ApplyTheme {
        /// Theme id (`ocean`, `bamboo`, `sunset`, `slate`).
        id: String,
    },
    /// Apply the theme of a theme file (`.thmx`) or of another document, workbook or
    /// presentation (base64 bytes) — colours, fonts and effects (CORE-FMT-001).
    /// Refused with `noTheme` when the file has none.
    ApplyThemeFile {
        /// File bytes, base64.
        file: String,
    },
    /// The document's theme as a `.thmx` file (`design.themeFile`, base64).
    SaveThemeFile,
    /// Apply a paragraph-spacing style set.
    ApplyStyleSet {
        /// Style set id (`compact`, `balanced`, `airy`, `double`).
        id: String,
    },
    /// View layout: print pages, web (text reflows to `width` points) or draft.
    SetViewLayout {
        /// Layout.
        #[serde(default)]
        mode: crate::views::ViewMode,
        /// Window width in points (web layout; absent keeps the last one).
        #[serde(default)]
        width: Option<f32>,
    },
    /// Show or hide formatting marks (¶ · → ↵) in the view's page display lists.
    ShowFormattingMarks {
        /// Show them.
        on: bool,
    },
    /// Document outline (`outline`): top-level paragraphs with their outline level.
    Outline,
    /// Set the outline level of a paragraph through the heading styles (§17.3.1.20).
    SetOutlineLevel {
        /// Body paragraph id (absent: the caret's paragraph).
        #[serde(default)]
        para: Option<usize>,
        /// 0-based level; absent = body text.
        #[serde(default)]
        level: Option<u8>,
    },
    /// Move a heading with its subordinate content (or a body paragraph) up or down.
    MoveOutline {
        /// Body paragraph id (absent: the caret's paragraph).
        #[serde(default)]
        para: Option<usize>,
        /// Up (`true`) or down.
        up: bool,
    },
    /// Move a heading with its subordinate content before body paragraph `before`
    /// (absent: to the end of the document) — drag and drop in the outline.
    MoveOutlineTo {
        /// Body paragraph id of the heading.
        para: usize,
        /// Body paragraph id of the target block.
        #[serde(default)]
        before: Option<usize>,
    },
    /// Find results with context (`results`, `matches`, `highlights`).
    FindResults {
        /// Text to look for.
        query: String,
        /// Match case, whole word and regular-expression options.
        #[serde(default)]
        options: FindOptions,
    },
    /// Layout tab state: columns, line numbers, hyphenation, text direction, drop cap.
    PageLayout,
    /// Text columns (§17.6.4 cols) of the caret's section, every section or from
    /// the caret's paragraph on.
    SetColumns {
        /// Columns.
        columns: bo_edit::page_layout::ColumnsSpec,
        /// Where they apply.
        #[serde(default)]
        scope: bo_edit::page_layout::LayoutScope,
    },
    /// Line numbering (§17.6.8 lnNumType; `None`: off).
    SetLineNumbering {
        /// Numbering.
        #[serde(default)]
        numbering: Option<bo_model::LineNumbering>,
        /// Caret's section or every section.
        #[serde(default)]
        scope: SetupScope,
    },
    /// Text direction of sections (§17.6.20 textDirection: `tbRl`, `btLr`; `None`:
    /// horizontal).
    SetSectionTextDirection {
        /// `ST_TextDirection` value.
        #[serde(default)]
        dir: Option<String>,
        /// Caret's section or every section.
        #[serde(default)]
        scope: SetupScope,
    },
    /// Hyphenation settings of the document (§17.15.1.10 autoHyphenation, …).
    SetHyphenation {
        /// New settings.
        hyphenation: bo_model::Hyphenation,
    },
    /// Drop cap of the caret's paragraph (§17.3.1.11 framePr; `None`: remove).
    SetDropCap {
        /// New drop cap.
        #[serde(default)]
        drop_cap: Option<bo_edit::page_layout::DropCapSpec>,
    },
    /// Insert a signature line at the caret (ECMA-376 Part 4, §19.2.2.30).
    InsertSignatureLine {
        /// What the dialog asked for.
        #[serde(default)]
        spec: bo_edit::sigline::SignatureLineSpec,
        /// Braced GUID of the line (host-generated); derived when absent.
        #[serde(default)]
        id: Option<String>,
    },
    /// The document's signature lines (answer: `signatureLines`).
    SignatureLines,
    /// Pictures of line `id` as signed and as invalid, for its signature
    /// ([MS-OFFCRYPTO] idValidSigLnImg / idInvalidSigLnImg; answer:
    /// `signatureLineImages`).
    SignatureLineImages {
        /// The line id.
        id: String,
        /// Typed name.
        #[serde(default)]
        text: String,
        /// Signer's picture (PNG / JPEG, base64).
        #[serde(default)]
        image_base64: Option<String>,
        /// Signing date shown on the line (`YYYY-MM-DD`).
        #[serde(default)]
        date: String,
    },
    /// Show the verification result on the signed lines (display only).
    SetSignatureLineStatus {
        /// Result per line id.
        lines: Vec<crate::sigline::LineStatus>,
    },
}

fn yes() -> bool {
    true
}

fn rect() -> String {
    "rect".into()
}

fn one() -> u8 {
    1
}

/// Clipboard payload returned by copy/cut.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClipOut {
    /// Plain-text form.
    pub text: String,
    /// HTML form.
    pub html: String,
    /// BlueOffice internal form (lossless within the suite).
    pub internal: String,
    /// Picture of the copied main-text content as laid out (PNG, base64), for
    /// pasting as a picture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub png: Option<String>,
}

/// Result of a command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    /// Document summary (pages, words, name, …).
    pub info: crate::DocInfo,
    /// Selection after the command.
    pub selection: Selection,
    /// Caret rectangle of the selection focus; `None` when it is not laid out.
    pub caret: Option<CaretRect>,
    /// Highlight rectangles of a range selection.
    pub rects: Vec<SelRect>,
    /// Pages whose display list changed (all pages after a page-count change).
    pub dirty: Vec<usize>,
    /// Ribbon formatting state at the selection.
    pub format: FormatState,
    /// Document changed since it was opened or last saved.
    pub modified: bool,
    /// Matches of `find`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches: Option<Vec<Match>>,
    /// Number of replacements of `replaceAll`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    /// Highlight rectangles of every match (`find` with `highlight`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlights: Option<Vec<SelRect>>,
    /// Clipboard payload of `copy` / `cut`.
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
    /// Mail-merge results (records, prompts, output files and e-mails, presets).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_out: Option<Box<crate::merge_cmd::MergeOut>>,
    /// Change to share and selection offsets while collaborating (Engine::collab_start).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collab: Option<crate::collab::CollabOut>,
    /// The equation the selection covers or the caret follows (CORE-MATH-007).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equation: Option<crate::equation::EquationInfo>,
    /// The phonetic guide the selection covers or the caret follows (§17.3.3.25 ruby).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruby: Option<bo_edit::charfx::RubySpec>,
    /// The content control around the caret (§17.5.2 sdt).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<ControlInfo>,
    /// The legacy form field at the caret (§17.16.17 ffData).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_field: Option<bo_edit::forms::FormFieldInfo>,
    /// The document's editing restriction (§17.15.1.29 documentProtection).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protection: Option<crate::forms::ProtectionInfo>,
    /// Equation templates (`equationTemplates`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub templates: Option<Vec<EquationTemplate>>,
    /// Misspelt words (`spelling`, ignore / add-word commands).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spelling: Option<Vec<crate::spell::SpellMark>>,
    /// Misspelt word at a point (`spellAt`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spell: Option<crate::spell::SpellInfo>,
    /// Open header/footer editor (selection and caret are in it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story: Option<crate::story::StoryState>,
    /// Footnotes and endnotes (`notes`, note commands).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Vec<NoteInfo>>,
    /// Font picker entries (`fontList`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fonts: Option<Vec<crate::fonts::FontChoice>>,
    /// Drawing object (shape, text box) under the last click or just changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawing: Option<crate::textbox::DrawingInfo>,
    /// Style gallery (`styleList`): paragraph and character styles of the document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub styles: Option<Vec<crate::gallery::StyleEntry>>,
    /// Current AutoCorrect options (`autoCorrect` command).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autocorrect: Option<Box<bo_edit::AutoCorrectOptions>>,
    /// The rule AutoCorrect applied after the typed character (one undo step).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corrected: Option<bo_edit::autocorrect::Rule>,
    /// Check document report (`inspect` and the inspector commands).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inspection: Option<Box<crate::inspect::Report>>,
    /// Document properties (`docProperties`, `setDocProperties`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_properties: Option<Box<crate::props::DocProperties>>,
    /// Whole-document spelling and grammar check (`proofCheck`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof: Option<Box<crate::proof::ProofReport>>,
    /// Proofing options (`proofOptions`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof_options: Option<crate::proof::ProofOptionsOut>,
    /// The Format Painter holds picked formatting.
    pub painter: bool,
    /// Design tab state (design commands).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design: Option<Box<crate::design::DesignState>>,
    /// Protection state (security commands; after edits while regions are shown).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<crate::security::SecurityInfo>,
    /// Document outline (`outline` and outline commands).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<Vec<crate::views::OutlineItem>>,
    /// Find results with context (`findResults`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<crate::views::FindResult>>,
    /// Layout tab state (page layout commands).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_layout: Option<Box<bo_edit::page_layout::PageLayoutState>>,
    /// Administrative format check (`checkAdminFormat`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_format: Option<Box<crate::templates::check::FormatReport>>,
    /// Signature lines of the document (signature line commands).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_lines: Option<Vec<bo_edit::sigline::SignatureLineInfo>>,
    /// Pictures for signing a line (`SignatureLineImages`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_line_images: Option<crate::sigline::SignatureLineImages>,
    /// The picture at the selection (picture tools commands, CORE-PIC-001).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<Box<crate::picture_tools::PictureInfo>>,
    /// Custom XML parts and their nodes (custom XML commands, §15.2.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_xml: Option<Vec<bo_edit::custom_xml::CustomXmlInfo>>,
    /// Sources, reference style and citation at the caret (citation commands, CORE-CITE-001).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cite: Option<Box<crate::cite::CiteInfo>>,
    /// Read-aloud text and highlight rectangles (`speechText`, `speechRects`, CORE-LANG-003).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speech: Option<Box<crate::speech::SpeechInfo>>,
    /// Text segments of the body paragraphs (`translationSegments`, CORE-LANG-001).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<Vec<bo_edit::translate::TextSegment>>,
    /// Style management answer (style commands, DOC-STYLE-001).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_mgr: Option<Box<crate::doc_styles::StyleMgrOut>>,
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
        if let Some(sc) = crate::security::SecurityCommand::parse(&v) {
            let r = self.doc_security(sc)?;
            return Ok(serde_json::to_string(&r).unwrap_or_default());
        }
        if let Some(pc) = crate::picture_tools::PictureCommand::parse(&v) {
            let r = self.doc_picture_command(pc)?;
            return Ok(serde_json::to_string(&r).unwrap_or_default());
        }
        // SmartArt (CORE-SMART-005); BlueSlide's go through the slide command bridge.
        if let Some(sc) = crate::doc_smartart::DocSmartArtCommand::parse(&v) {
            return Ok(self.doc_smartart_command(sc)?.to_string());
        }
        if let Some(xc) = crate::custom_xml::CustomXmlCommand::parse(&v) {
            let r = self.doc_custom_xml_command(xc)?;
            return Ok(serde_json::to_string(&r).unwrap_or_default());
        }
        // Citations and bibliography (CORE-CITE-001).
        if let Some(cc) = crate::cite::CiteCommand::parse(&v) {
            let r = self.doc_cite_command(cc)?;
            return Ok(serde_json::to_string(&r).unwrap_or_default());
        }
        // Embedded and linked workbooks (CORE-EMBED-001).
        if let Some(oc) = crate::ole::OleCommand::parse(&v) {
            let r = self.doc_ole_command(oc)?;
            return Ok(serde_json::to_string(&r).unwrap_or_default());
        }
        if let Some(sc) = crate::doc_styles::StyleCommand::parse(&v) {
            let r = self.doc_style_command(sc)?;
            return Ok(serde_json::to_string(&r).unwrap_or_default());
        }
        if let Some(tc) = crate::translate::TranslateCommand::parse(&v) {
            let r = self.doc_translate_command(tc)?;
            return Ok(serde_json::to_string(&r).unwrap_or_default());
        }
        if let Some(sc) = crate::speech::SpeechCommand::parse(&v) {
            let r = self.doc_speech_command(sc)?;
            return Ok(serde_json::to_string(&r).unwrap_or_default());
        }
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
    ///
    /// The clock stays fixed for later commands that carry no host time of their own.
    pub fn set_clock_ms(&mut self, ms: i64) {
        self.clock_fixed = Some(ms);
        self.apply_clock_ms(ms);
    }

    fn apply_clock_ms(&mut self, ms: i64) {
        if let Some(s) = self.session.as_mut() {
            s.set_clock(Some(crate::review::iso_date(ms)));
        }
    }

    /// Run a command at host time `now` (Unix ms; the system clock when absent).
    fn command_at(&mut self, cmd: Command, now: Option<i64>) -> crate::Result<EditResult> {
        let click = match &cmd {
            Command::Click { page, x, y, .. } => Some((*page, *x, *y)),
            _ => None,
        };
        // Copy / cut: a picture of the selection before it changes (paste as picture).
        let picture = matches!(cmd, Command::Copy | Command::Cut)
            .then(|| self.selection_picture())
            .flatten();
        let mut r = self.command_inner(cmd, now)?;
        if let Some(c) = r.clip.as_mut() {
            c.png = picture;
        }
        // A click on a shape reports it so the host can show its frame and handles.
        if let Some((page, x, y)) = click.filter(|_| r.drawing.is_none()) {
            r.drawing = self.drawing_hit(page, x, y);
        }
        Ok(r)
    }

    fn command_inner(&mut self, cmd: Command, now: Option<i64>) -> crate::Result<EditResult> {
        // Filling in forms (§17.15.1.29): only form fields, content controls and unprotected
        // sections change.
        let cmd = match self.session.as_ref() {
            Some(s) if self.hf.is_none() && self.note.is_none() => {
                match crate::forms::gate(cmd, s) {
                    Ok(c) => c,
                    Err(e) => {
                        let mut r = self.query_result()?;
                        r.error = Some(e.to_owned());
                        return Ok(r);
                    }
                }
            }
            _ => cmd,
        };
        if let Command::AutoCorrect { options, reset } = cmd {
            if reset {
                self.autocorrect = Some(bo_edit::AutoCorrectOptions::default());
            } else if let Some(o) = options {
                self.autocorrect = Some(*o);
            }
            // Until a host configures it, AutoCorrect is off (CLI, conversions, tests).
            let current = self
                .autocorrect
                .clone()
                .unwrap_or(bo_edit::AutoCorrectOptions {
                    enabled: false,
                    ..bo_edit::AutoCorrectOptions::default()
                });
            let mut r = self.query_result()?;
            r.autocorrect = Some(Box::new(current));
            return Ok(r);
        }
        // Read-only files and restricted editing: headers, notes and drawings
        // cannot be opened for editing (CORE-SEC-001).
        if (self.sec.read_only || self.sec.regions_enforced())
            && matches!(
                cmd,
                Command::EditHeaderFooter { .. }
                    | Command::EditNote { .. }
                    | Command::EditTextBox { .. }
                    | Command::InsertShape { .. }
                    | Command::SetDrawingFrame { .. }
                    | Command::SetHeaderLink { .. }
            )
        {
            let mut r = self.query_result()?;
            r.error = Some(self.sec.refusal().into());
            return Ok(r);
        }
        if let Some(mut r) = self.special_command(&cmd)? {
            // Header/footer and note editors mirror into the document: share it live.
            if r.collab.is_none() {
                r.collab = self.collab_after_edit();
            }
            return Ok(r);
        }
        let highlight = matches!(
            cmd,
            Command::Find {
                highlight: true,
                ..
            }
        );
        let mut extra = (None, None, None);
        let mut error = None;
        let mut changed = false; // set when the document version moves
        let mut relayout = false; // display options changed
        let review = matches!(
            cmd,
            Command::State
                | Command::Review
                | Command::SetTrackChanges { .. }
                | Command::ProtectDocument { .. }
                | Command::UnprotectDocument { .. }
                | Command::SetAuthor { .. }
                | Command::SetRevisionView { .. }
                | Command::SetMarkup(_)
                | Command::NextRevision { .. }
                | Command::SelectComment { .. }
                | Command::NextComment { .. }
                | Command::CompareWith { .. }
                | Command::CombineWith { .. }
        );
        if let Command::SetRevisionView { view } = cmd {
            relayout = self.set_markup(crate::markup::MarkupUpdate {
                mode: Some(crate::markup::MarkupOptions::mode_of(view)),
                ..Default::default()
            });
        }
        if let Command::SetMarkup(u) = &cmd {
            relayout = self.set_markup(u.clone());
        }
        if let Command::ShowFieldCodes { on } = cmd {
            relayout |= self.field_codes != on;
            self.field_codes = on;
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
            Command::SaveFormat { ext } => {
                if self.set_save_format(ext).is_err() {
                    let mut r = self.query_result()?;
                    r.error = Some("saveFormat".into());
                    return Ok(r);
                }
            }
            Command::MergeData { name, base64 } => {
                match crate::merge::base64_decode(base64)
                    .ok_or_else(|| crate::EngineError::BadCommand("base64".into()))
                    .and_then(|b| crate::merge::read_data(&b, name))
                {
                    Ok(d) => {
                        self.set_merge_source(d);
                        relayout |= self.merge_preview.is_some();
                    }
                    Err(e) => error = Some(e.to_string()),
                }
            }
            Command::MergePreview { record } => {
                let n = self.merge_order.len();
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
                (Some(m), Some(s)) if !self.merge_order.is_empty() => {
                    let run = bo_edit::merge::rules::MergeRun {
                        data: m,
                        order: &self.merge_order,
                        answers: &self.merge_answers,
                    };
                    let merged = bo_edit::merge::combine(
                        s.doc(),
                        bo_edit::merge::rules::merge_each(s.doc(), run),
                    );
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
        if let Some(ms) = now.or(self.clock_fixed).or_else(crate::review::system_ms) {
            self.apply_clock_ms(ms);
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
        let mut corrected = None;
        {
            let layout = self.layout.document();
            let s = self
                .session
                .as_mut()
                .ok_or(crate::EngineError::NoDocument)?;
            let (fonts, cache, opts) = (
                &mut self.fonts,
                &mut self.cache,
                LayoutOptions {
                    view: self.view,
                    field_codes: self.field_codes,
                    marks: self.markup.filter(),
                },
            );
            // Table tools measure the table first (row heights, content widths).
            let mut cmd = cmd;
            if let Command::TableEdit { edit } = &mut cmd {
                measure_for_table_tool(edit, s, fonts, cache);
            }
            // Lays a document out for field updates (page numbers per body paragraph).
            let mut pages_of = |doc: &bo_model::Document| -> Vec<u32> {
                let n = bo_model::index::body_paragraphs(doc).len();
                bo_layout::layout_document_with(doc, fonts, cache, opts)
                    .map(|l| page_numbers(&l, n))
                    .unwrap_or_default()
            };
            let autocorrect = self.autocorrect.as_ref();
            let auto_language = self.spell.auto_language;
            // CORE-SEC-001: edits of a read-only file, or outside the editable
            // regions of a protected one, are applied then taken back.
            let guard = self.sec.read_only || self.sec.regions_enforced();
            let history = s.history_len();
            let step = match cmd {
                Command::Undo => -1,
                Command::Redo => 1,
                _ => 0,
            };
            if guard {
                s.end_typing();
                if !self.sec.read_only && self.sec.skeleton.is_none_or(|(v, _)| v != s.version()) {
                    self.sec.skeleton =
                        Some((s.version(), crate::perm::skeleton(s.doc(), &self.identity)));
                }
            }
            let before_version = s.version();
            let sel = s.selection();
            let cmd_kind = if matches!(cmd, Command::AcceptRevision { .. }) {
                CmdKind::Accept
            } else {
                CmdKind::Other
            };
            // Leaving a text form field while filling in a form applies its format and may
            // recalculate fields (§17.16.6 calcOnExit); a click on a check box toggles it.
            let locked = s.forms_locked();
            let moves = locked
                && matches!(
                    cmd,
                    Command::Click { .. }
                        | Command::Move { .. }
                        | Command::SetSelection { .. }
                        | Command::NextFormField { .. }
                        | Command::DragTo { .. }
                );
            let click = locked
                && matches!(
                    cmd,
                    Command::Click {
                        count: 1,
                        extend: false,
                        ..
                    }
                );
            let tab = matches!(cmd, Command::NextFormField { .. });
            let exit_from = if moves {
                s.form_at(s.selection().focus).filter(|f| f.field.is_text())
            } else {
                None
            };
            let mut form_error: Option<&'static str> = None;
            let res: bo_edit::Result<()> = (|| {
                match cmd {
                    // Handled outside the session (view options, merge state).
                    Command::State
                    | Command::AutoCorrect { .. }
                    | Command::Review
                    | Command::SetRevisionView { .. }
                    | Command::SetMarkup(_)
                    | Command::ShowFieldCodes { .. }
                    | Command::MergeData { .. }
                    | Command::MergePreview { .. }
                    | Command::MergeToDocument
                    | Command::MergeTable { .. }
                    | Command::MergeSetRecipients { .. }
                    | Command::MergeRecords { .. }
                    | Command::MergeDuplicates { .. }
                    | Command::MergePrompts
                    | Command::MergeSetAnswers { .. }
                    | Command::MergeToPdfs { .. }
                    | Command::MergeToEmails { .. }
                    | Command::MergeEnvelope { .. }
                    | Command::MergeLabels { .. }
                    | Command::MergePresets
                    | Command::NewFromTemplate { .. }
                    | Command::CheckAdminFormat
                    | Command::FixAdminFormat { .. }
                    | Command::SetSavePassword { .. }
                    | Command::SaveFormat { .. }
                    | Command::EquationTemplates
                    // Handled by `special_command`.
                    | Command::Spelling { .. }
                    | Command::SpellAt { .. }
                    | Command::SpellIgnore { .. }
                    | Command::SpellAddWord { .. }
                    | Command::SetSpelling { .. }
                    | Command::Equation
                    | Command::SetGrammar { .. }
                    | Command::GrammarIgnore { .. }
                    | Command::ProofCheck
                    | Command::ProofOptions { .. }
                    | Command::EditHeaderFooter { .. }
                    | Command::CloseHeaderFooter
                    | Command::SetHeaderLink { .. }
                    | Command::EditNote { .. }
                    | Command::Notes
                    | Command::FontList
                    | Command::InsertShape { .. }
                    | Command::SetDrawingFrame { .. }
                    | Command::EditTextBox { .. }
                    | Command::CloseTextBox
                    | Command::StyleList
                    | Command::Design
                    | Command::SetPageColor { .. }
                    | Command::SetPageBorders { .. }
                    | Command::SetWatermark { .. }
                    | Command::ApplyTheme { .. }
                    | Command::ApplyThemeFile { .. }
                    | Command::SaveThemeFile
                    | Command::ApplyStyleSet { .. }
                    | Command::InsertSignatureLine { .. }
                    | Command::SignatureLines
                    | Command::SignatureLineImages { .. }
                    | Command::SetSignatureLineStatus { .. }
                    | Command::Inspect
                    | Command::DocProperties
                    | Command::SetDocProperties { .. }
                    | Command::InspectRemove { .. }
                    | Command::InspectKeep { .. }
                    | Command::InspectFix { .. }
                    | Command::SetViewLayout { .. }
                    | Command::ShowFormattingMarks { .. }
                    | Command::Outline
                    | Command::SetOutlineLevel { .. }
                    | Command::MoveOutline { .. }
                    | Command::MoveOutlineTo { .. }
                    | Command::FindResults { .. }
                    | Command::PageLayout
                    | Command::SetColumns { .. }
                    | Command::SetLineNumbering { .. }
                    | Command::SetSectionTextDirection { .. }
                    | Command::SetHyphenation { .. }
                    | Command::SetDropCap { .. } => {}
                    Command::Tab { back } => {
                        if s.next_cell(back)?.is_none() && !back {
                            s.insert_text("\t")?;
                        }
                    }
                    Command::TableEdit { edit } => {
                        s.edit_table(&edit)?;
                    }
                    Command::SetPicturePosition { x, y } => {
                        s.set_picture_position(x, y)?;
                    }
                    Command::SetPageSetup { setup, scope } => s.set_page_setup(setup, scope)?,
                    Command::SetHeaderOptions {
                        title_page,
                        even_and_odd,
                    } => {
                        let si = s.section_state().index;
                        s.set_header_options(si, title_page, even_and_odd)?;
                    }
                    Command::InsertEquation {
                        text,
                        latex,
                        display,
                        replace,
                        numbered: Some(label),
                    } if !replace => {
                        s.insert_numbered_equation(&crate::equation::build(&text, latex, false), &label)?;
                        let _ = display;
                    }
                    Command::InsertEquation {
                        text,
                        latex,
                        display,
                        replace,
                        ..
                    } => {
                        if replace {
                            if let Some((sel, _)) = s.selected_equation() {
                                s.set_selection(sel);
                            }
                        }
                        s.insert_equation(&crate::equation::build(&text, latex, display))?;
                    }
                    Command::InsertRuby { ruby, replace } => {
                        if replace {
                            if let Some((sel, _)) = s.selected_ruby() {
                                s.set_selection(sel);
                            }
                        }
                        s.insert_ruby(&ruby)?;
                    }
                    Command::EquationFormat { linear } => {
                        if let Some((sel, xml)) = s.selected_equation() {
                            if let Some(new) = crate::equation::reformat(&xml, linear) {
                                s.set_selection(sel);
                                s.insert_equation(&new)?;
                            }
                        }
                    }
                    Command::InsertField { instr } => {
                        let cached = if instr.trim().eq_ignore_ascii_case("NUMPAGES") {
                            layout.pages.len().to_string()
                        } else {
                            "1".to_owned()
                        };
                        s.insert_field(&instr, &cached)?;
                        // The new field shows its result at once (§17.16.5).
                        s.update_fields(&mut pages_of)?;
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
                        // AutoCorrect after one typed character (its own undo step).
                        let mut cs = text.chars();
                        if let (Some(c), None, Some(o)) = (cs.next(), cs.next(), autocorrect) {
                            corrected = s.autocorrect(o, c)?;
                        }
                        // Language detection after a word (its own undo step, off by default).
                        if auto_language && text.chars().count() == 1 {
                            s.tag_language(&|t| {
                                bo_spell::lang::detect(t).map(str::to_owned)
                            })?;
                        }
                    }
                    Command::ProofEdit { edits } => {
                        s.proof_edit(&edits)?;
                    }
                    Command::DeleteBackward => {
                        s.delete_backward()?;
                    }
                    Command::DeleteForward => {
                        s.delete_forward()?;
                    }
                    Command::SplitParagraph => {
                        let n = s.paragraph_count();
                        s.split_paragraph()?;
                        // Only a real break (Enter in an empty list item ends the list).
                        if let Some(o) = autocorrect.filter(|_| s.paragraph_count() > n) {
                            corrected = s.autocorrect_break(o)?;
                        }
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
                    Command::DefineList { levels, apply } => {
                        s.define_list(&levels, apply)?;
                    }
                    Command::SetListNumbering { numbering } => {
                        s.set_list_numbering(numbering)?;
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
                    Command::HistoryLen => {
                        extra.1 = Some(s.history_len());
                    }
                    Command::MergeHistory { since } => {
                        extra.1 = Some(s.merge_history_since(since));
                    }
                    Command::Find {
                        query,
                        options,
                        format,
                        ..
                    } => {
                        extra.0 = Some(match &format {
                            Some(f) => s.find_formatted(&query, &options, f)?,
                            None => s.find(&query, &options)?,
                        });
                    }
                    Command::SelectMatch { m } => s.set_selection(Selection {
                        anchor: Pos::new(m.para, m.start),
                        focus: Pos::new(m.last_para(), m.end),
                    }),
                    Command::Replace {
                        m,
                        with,
                        query,
                        options,
                    } => {
                        s.replace_match(m, query.as_deref().map(|q| (q, &options)), &with)?;
                    }
                    Command::ReplaceAll {
                        query,
                        options,
                        with,
                        format,
                    } => {
                        extra.1 = Some(match &format {
                            Some(f) => s.replace_all_formatted(&query, &options, f, &with)?,
                            None => s.replace_all(&query, &options, &with)?,
                        });
                    }
                    Command::Copy => {
                        let c = s.copy();
                        extra.2 = Some(ClipOut {
                            text: c.text,
                            html: c.html,
                            internal: c.internal,
                            png: None,
                        });
                    }
                    Command::Cut => {
                        let c = s.cut()?;
                        extra.2 = Some(ClipOut {
                            text: c.text,
                            html: c.html,
                            internal: c.internal,
                            png: None,
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
                    // Locked tracking (§17.15.1.29 w:edit="trackedChanges").
                    Command::SetTrackChanges { on: false }
                    | Command::AcceptRevision { .. }
                    | Command::RejectRevision { .. }
                    | Command::AcceptAllRevisions
                    | Command::RejectAllRevisions
                        if s.tracking_locked() =>
                    {
                        form_error = Some(bo_edit::TRACKING_LOCKED);
                    }
                    Command::SetTrackChanges { on } => s.set_track_changes(on),
                    Command::MoveParagraphs { up } => {
                        s.move_paragraphs(up)?;
                    }
                    Command::ConvertLegacyText => {
                        self.legacy_converted = s.convert_legacy_text()?;
                    }
                    Command::SetAuthor { name } => s.set_author(&name),
                    Command::AcceptRevision { id } | Command::RejectRevision { id } => {
                        let accept = matches!(cmd_kind, CmdKind::Accept);
                        let ids = id.map_or_else(|| s.revisions_at_selection(), |i| vec![i]);
                        if !ids.is_empty() {
                            extra.1 = Some(s.resolve_revisions(Some(&ids), accept)?);
                        }
                    }
                    Command::CompareWith { name, base64 } => {
                        // A file that cannot be read is refused like an unsupported edit.
                        let original = crate::merge::base64_decode(&base64)
                            .ok_or_else(|| "base64".to_owned())
                            .and_then(|b| {
                                crate::word_document(&b, &name).map_err(|e| e.to_string())
                            })
                            .map_err(bo_edit::EditError::Unsupported)?;
                        extra.1 = Some(s.compare_with_original(&original)?);
                    }
                    Command::CombineWith { name, base64 } => {
                        let other = crate::merge::base64_decode(&base64)
                            .ok_or_else(|| "base64".to_owned())
                            .and_then(|b| {
                                crate::word_document(&b, &name).map_err(|e| e.to_string())
                            })
                            .map_err(bo_edit::EditError::Unsupported)?;
                        let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s);
                        extra.1 = Some(s.combine_with(&other, stem)?);
                    }
                    Command::LockFields { lock } => {
                        extra.1 = Some(s.set_fields_locked(Some(lock))?);
                    }
                    Command::UnlinkFields => {
                        extra.1 = Some(s.set_fields_locked(None)?);
                    }
                    Command::InsertControl {
                        kind,
                        placeholder,
                        items,
                    } => {
                        s.insert_control(&kind, &placeholder, &items)?;
                    }
                    Command::SetControlValue { value } => {
                        extra.1 = Some(usize::from(s.set_control_value(&value, true)?));
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
                    Command::AddComment {
                        text,
                        mentions,
                        task,
                    } => {
                        extra.1 = Some(s.add_comment_with(&text, &mentions, task)?.max(0) as usize);
                    }
                    Command::ReplyComment {
                        id,
                        text,
                        mentions,
                        task,
                    } => {
                        extra.1 =
                            Some(s.reply_comment_with(id, &text, &mentions, task)?.max(0) as usize);
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
                    Command::InsertReferenceTable {
                        instr,
                        leader,
                        columns,
                    } => {
                        let leader = leader.as_deref().map(|l| match l {
                            "none" => bo_model::TabLeader::None,
                            "hyphen" => bo_model::TabLeader::Hyphen,
                            "underscore" => bo_model::TabLeader::Underscore,
                            _ => bo_model::TabLeader::Dot,
                        });
                        extra.1 = Some(s.insert_reference_table(
                            &instr,
                            leader,
                            columns,
                            &mut pages_of,
                        )?);
                    }
                    Command::MarkIndexEntry { mark, all } => {
                        extra.1 = Some(s.mark_index_entry(&mark, all)?);
                    }
                    Command::MarkCitation { mark, all } => {
                        extra.1 = Some(s.mark_citation(&mark, all)?);
                    }
                    Command::ConvertNotes { mode, selection } => {
                        extra.1 = Some(s.convert_notes(mode, selection)?);
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
                    Command::MergeInsertRule { rule } => {
                        let (parts, shown) = rule.parts().ok_or_else(|| {
                            bo_edit::EditError::Unsupported("merge rule".into())
                        })?;
                        s.insert_merge_rule(&parts, &shown)?;
                    }
                    Command::InsertFormField { kind } => {
                        s.insert_form_field(&kind)?;
                    }
                    Command::SetFormField { field } => {
                        extra.1 = Some(usize::from(s.set_form_field(&field)?));
                    }
                    Command::SetFormFieldValue { value } => {
                        extra.1 = Some(usize::from(s.set_form_value(&value)?));
                    }
                    Command::NextFormField { back } => {
                        extra.1 = Some(usize::from(s.next_form_field(back).is_some()));
                    }
                    Command::FormType { text } => {
                        s.form_type(&text)?;
                    }
                    Command::FormDelete { back } => {
                        s.form_delete(back)?;
                    }
                    Command::ProtectDocument {
                        edit,
                        password,
                        unprotected_sections,
                    } => {
                        crate::forms::protect(
                            s,
                            &edit,
                            password.as_deref(),
                            unprotected_sections.as_deref(),
                        )
                        .map_err(bo_edit::EditError::Unsupported)?;
                    }
                    Command::UnprotectDocument { password } => {
                        if let Err(e) = crate::forms::unprotect(s, password.as_deref()) {
                            form_error = Some(e);
                        }
                    }
                }
                if let Some(f) = &exit_from {
                    let now = s.form_at(s.selection().focus);
                    // Tab leaves the field even when it comes back to it (the only one).
                    if tab || now.as_ref().map(|n| (n.para, n.start)) != Some((f.para, f.start)) {
                        match s.form_exit(f) {
                            Ok(true) => {
                                s.update_fields(&mut pages_of)?;
                            }
                            Ok(false) => {}
                            Err(_) => {
                                // Not valid for its type: back into the field.
                                let number = matches!(
                                    &f.field.kind,
                                    bo_model::FormFieldKind::Text(t)
                                        if t.input_type == bo_model::form::TextInputType::Number
                                );
                                form_error = Some(if number {
                                    crate::forms::BAD_NUMBER
                                } else {
                                    crate::forms::BAD_DATE
                                });
                                s.set_selection(Selection {
                                    anchor: Pos::new(f.para, f.from),
                                    focus: Pos::new(f.para, f.to),
                                });
                            }
                        }
                    }
                }
                if click && s.selection().is_collapsed() {
                    if let Some(f) = s.form_at(s.selection().focus) {
                        if matches!(f.field.kind, bo_model::FormFieldKind::CheckBox(_)) {
                            s.set_form_value("")?;
                        }
                    }
                }
                Ok(())
            })();
            if let Err(e) = res {
                error = Some(e.to_string());
            }
            if let Some(e) = form_error {
                error = Some(e.to_owned());
            }
            if guard && s.version() != before_version {
                let refused = self.sec.read_only || {
                    let h = crate::perm::skeleton(s.doc(), &self.identity);
                    let same = self.sec.skeleton.is_some_and(|(_, old)| old == h);
                    if same {
                        self.sec.skeleton = Some((s.version(), h));
                    }
                    !same
                };
                if refused {
                    let old = self.sec.skeleton.map(|(_, h)| h);
                    match step {
                        -1 => {
                            s.redo();
                        }
                        1 => {
                            s.undo();
                        }
                        _ => while s.history_len() > history && s.discard_last() {},
                    }
                    self.sec.skeleton = old.map(|h| (s.version(), h));
                    error = Some(self.sec.refusal().into());
                }
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
        if highlight {
            r.highlights = Some(self.match_rects(r.matches.as_deref().unwrap_or_default()));
        }
        r.targets = targets;
        r.corrected = corrected;
        if note_cmd {
            r.notes = self.session.as_ref().map(bo_edit::Session::notes);
        }
        r.collab = self.collab_after_edit();
        if merge_cmd {
            r.merge = self.merge_state();
        }
        if changed || relayout || review {
            if let Some(s) = self.session.as_ref() {
                r.review = Some(crate::review::review_state(s, &self.markup));
            }
        }
        // Highlighted editable regions move with the text.
        if (changed || relayout) && self.sec.show_regions {
            r.security = Some(self.security_info());
        }
        Ok(r)
    }

    /// Insert a picture (PNG, JPEG; GIF, BMP, TIFF converted to PNG) at the selection;
    /// sized at 96 dpi, capped to the text width.
    pub fn insert_image(&mut self, bytes: Vec<u8>, name: &str) -> crate::Result<EditResult> {
        // CORE-SEC-001: read-only files; restricted editing allows pictures only
        // inside a region the user may edit.
        if self.sec.read_only || self.sec.regions_enforced() {
            let inside = !self.sec.read_only
                && self.session.as_ref().is_some_and(|s| {
                    let (a, b) = s.selection().ordered();
                    crate::perm::regions(s.doc(), &self.identity)
                        .iter()
                        .any(|r| r.mine && r.start <= a && b <= r.end)
                });
            if !inside {
                let mut r = self.query_result()?;
                r.error = Some(self.sec.refusal().into());
                return Ok(r);
            }
        }
        // GIF, BMP and TIFF pictures are stored as PNG.
        let bytes = if image_size(&bytes).is_none() {
            bo_raster::transcode_to_png(&bytes).unwrap_or(bytes)
        } else {
            bytes
        };
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

    /// Highlight rectangles of find matches in the laid-out main text.
    pub fn match_rects(&self, matches: &[Match]) -> Vec<SelRect> {
        let layout = self.layout.document();
        matches
            .iter()
            .flat_map(|m| {
                nav::selection_rects(
                    layout,
                    Selection {
                        anchor: Pos::new(m.para, m.start),
                        focus: Pos::new(m.last_para(), m.end),
                    },
                )
            })
            .collect()
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
            highlights: None,
            clip: extra.2,
            error,
            review: None,
            targets: None,
            merge: None,
            merge_out: None,
            spelling: None,
            spell: None,
            equation: s
                .selected_equation()
                .and_then(|(_, xml)| crate::equation::info(&xml)),
            ruby: s.selected_ruby().map(|(_, r)| r),
            control: s.control_at_caret().map(|c| {
                let mut i = ControlInfo::of(&c);
                i.bound = s.control_binding_status();
                i
            }),
            form_field: s.form_field_info(),
            protection: crate::forms::info(s),
            templates: None,
            collab: None,
            story: None,
            notes: None,
            fonts: None,
            drawing: None,
            styles: None,
            autocorrect: None,
            corrected: None,
            inspection: None,
            doc_properties: None,
            proof: None,
            proof_options: None,
            painter: s.picked_format().is_some(),
            design: None,
            security: None,
            outline: None,
            results: None,
            page_layout: None,
            admin_format: None,
            signature_lines: None,
            signature_line_images: None,
            picture: None,
            custom_xml: None,
            cite: None,
            speech: None,
            translation: None,
            style_mgr: None,
        })
    }
}

/// Fill in the layout measurements a table tool needs (ECMA-376 Part 1, §17.4): row
/// heights for distributing rows, content widths for AutoFit to the contents. A
/// measurement that fails leaves the command as it is (the tool then falls back to
/// the table's own sizes).
fn measure_for_table_tool(
    edit: &mut TableEdit,
    s: &bo_edit::Session,
    fonts: &mut bo_text::FontManager,
    cache: &mut bo_layout::LayoutCache,
) {
    let wants = match edit {
        TableEdit::DistributeRows { heights } => heights.is_empty(),
        TableEdit::AutoFit {
            mode: bo_edit::table_tools::AutoFit::Contents,
            widths,
        } => widths.is_empty(),
        _ => false,
    };
    if !wants {
        return;
    }
    let Some((t, avail)) = s.caret_table() else {
        return;
    };
    let Ok(m) = bo_layout::measure_table(s.doc(), fonts, cache, t, avail.to_pt()) else {
        return;
    };
    let twips = |pt: f32| (pt * 20.0).round() as i32;
    match edit {
        TableEdit::DistributeRows { heights } => {
            *heights = m.row_heights.iter().map(|h| twips(*h)).collect();
        }
        TableEdit::AutoFit { widths, .. } => {
            // Columns as wide as their widest line; when the table would not fit,
            // all columns shrink in proportion to fill the available width.
            let total: f32 = m.max_widths.iter().sum();
            let room = avail.to_pt() - t.props.indent.map_or(0.0, |i| i.to_pt().max(0.0));
            let k = if total > room && total > 0.0 {
                room / total
            } else {
                1.0
            };
            *widths = m.max_widths.iter().map(|w| twips(w * k).max(1)).collect();
        }
        _ => {}
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

/// An equation template: its id (named by the UI) and UnicodeMath text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EquationTemplate {
    /// Template id.
    pub id: String,
    /// UnicodeMath linear form.
    pub linear: String,
}

/// A content control as the host sees it (§17.5.2 sdt).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlInfo {
    /// `text` | `richText` | `picture` | `comboBox` | `dropDown` | `date` | `checkbox` |
    /// `repeatingSection` | `group` | `other`.
    pub kind: &'static str,
    /// Title (§17.5.2.1 alias).
    pub title: Option<String>,
    /// Tag (§17.5.2.42).
    pub tag: Option<String>,
    /// Items of a list control (display texts).
    pub items: Vec<String>,
    /// A checkbox is checked.
    pub checked: Option<bool>,
    /// A date control's date (ISO 8601) and display format.
    pub date: Option<String>,
    /// Display format of a date control.
    pub format: Option<String>,
    /// The content shows the placeholder.
    pub placeholder: bool,
    /// The content cannot be edited (§17.5.2.23 lock).
    pub locked: bool,
    /// Bound node (§17.5.2.6 dataBinding `w:xpath`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    /// The bound node exists (`false`: unbound, the control keeps its content).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound: Option<bool>,
}

impl ControlInfo {
    fn of(c: &bo_model::ContentControl) -> ControlInfo {
        use bo_model::ControlKind as K;
        let (kind, items, checked, date, format) = match &c.kind {
            K::Text { .. } => ("text", vec![], None, None, None),
            K::RichText => ("richText", vec![], None, None, None),
            K::Picture => ("picture", vec![], None, None, None),
            K::ComboBox(i) => (
                "comboBox",
                i.iter().map(|x| x.0.clone()).collect(),
                None,
                None,
                None,
            ),
            K::DropDown(i) => (
                "dropDown",
                i.iter().map(|x| x.0.clone()).collect(),
                None,
                None,
                None,
            ),
            K::Date {
                format, full_date, ..
            } => (
                "date",
                vec![],
                None,
                full_date.clone(),
                Some(format.clone()),
            ),
            K::Checkbox { checked, .. } => ("checkbox", vec![], Some(*checked), None, None),
            K::RepeatingSection => ("repeatingSection", vec![], None, None, None),
            K::Group => ("group", vec![], None, None, None),
            K::Other => ("other", vec![], None, None, None),
        };
        ControlInfo {
            kind,
            title: c.alias.clone(),
            tag: c.tag.clone(),
            items,
            checked,
            date,
            format,
            placeholder: c.placeholder,
            locked: matches!(
                c.lock.as_deref(),
                Some("contentLocked" | "sdtContentLocked")
            ),
            binding: c.data_binding.as_ref().map(|b| b.xpath.clone()),
            bound: None,
        }
    }
}
