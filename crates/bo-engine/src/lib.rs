//! `bo-engine` — one API over the whole core pipeline
//! (open → layout → display list → PNG/PDF).
//!
//! Hosts: `bo-wasm` (browser, inside a Web Worker), the Tauri desktop shell
//! (native) and `bo-cli`. Because every host calls this same code, web and
//! desktop produce byte-identical display lists (see ADR-0002).
#![forbid(unsafe_code)]

use bo_layout::{IncrementalLayout, LayoutCache, LayoutDocument, LayoutOptions, RevisionView};
use bo_model::change::BlockChanges;
use bo_model::{Block, Document, Paragraph, Run, RunProps, Section};
use bo_render::{build_page_list, DisplayList, PageList};
use bo_text::{outline_svg_path, FaceId, FontManager};
use serde::Serialize;
use std::cell::OnceCell;
use std::collections::BTreeMap;

#[cfg(feature = "bundled-fonts")]
mod bundled;
pub mod chart_design;
mod clip_picture;
pub mod collab;
pub mod design;
mod doc_picture;
pub mod edit;
pub mod equation;
pub mod fonts;
pub mod forms;
pub mod gallery;
pub mod inspect;
mod legacy_slides;
pub mod markup;
pub mod media;
pub mod merge;
pub mod nav;
mod notes;
mod page_layout;
pub mod perm;
pub mod picture_tools;
pub mod proof;
pub mod review;
pub mod security;
mod security_cmd;
pub mod sheet;
pub mod sigline;
pub mod slide_edit;
pub mod slide_export;
pub mod smartart;
pub mod spell;
pub mod story;
pub mod templates;
pub mod textbox;
pub mod views;

/// Installed OS fonts (native hosts, see [`Engine::set_system_fonts`]).
#[cfg(not(target_arch = "wasm32"))]
pub use bo_text::SystemFontIndex;
pub use collab::{CollabChange, CollabOut, TextOp};
pub use edit::{Command, EditResult};
pub use sheet::SheetEngine;
pub use slide_edit::{SlideCommand, SlideEditResult};

/// Engine errors (surfaced to JS / Tauri as strings).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The file type cannot be opened.
    #[error("unsupported file type: {0}")]
    Unsupported(String),
    /// Reading or writing a `.docx` failed.
    #[error("cannot read document: {0}")]
    Docx(#[from] bo_docx::DocxError),
    /// Reading or writing a `.pptx` failed.
    #[error("cannot read presentation: {0}")]
    Pptx(#[from] bo_pptx::PptxError),
    /// Layout failed.
    #[error("layout failed: {0}")]
    Layout(#[from] bo_layout::LayoutError),
    /// The command needs an open document.
    #[error("no document is open")]
    NoDocument,
    /// Page (or sheet / slide) index out of range.
    #[error("page {0} out of range")]
    PageOutOfRange(usize),
    /// PDF export failed.
    #[error("PDF export failed: {0}")]
    Pdf(#[from] bo_pdf::PdfError),
    /// Spreadsheet read / write error.
    #[error("spreadsheet error: {0}")]
    Sheet(String),
    /// PNG rendering failed.
    #[error("rendering failed: {0}")]
    Raster(#[from] bo_raster::RasterError),
    /// The JSON command is malformed or does not apply.
    #[error("invalid command: {0}")]
    BadCommand(String),
    /// The file is encrypted ([MS-OFFCRYPTO]) and no password was given.
    /// Hosts match the message `password required`.
    #[error("password required")]
    PasswordRequired,
    /// Hosts match the message `wrong password`.
    #[error("wrong password")]
    WrongPassword,
    /// Encryption or decryption failed.
    #[error("encryption error: {0}")]
    Crypto(String),
    /// Reading a legacy or ODF document (`.doc`, `.odt`, …) failed.
    #[error("cannot read document: {0}")]
    Format(String),
}

/// [MS-CFB] §2.2 compound file signature.
const CFB_SIGNATURE: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// Result alias.
pub type Result<T> = std::result::Result<T, EngineError>;

/// Page size in points.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct PageSize {
    /// Width in points.
    pub width: f32,
    /// Height in points.
    pub height: f32,
}

/// Summary returned after opening a document.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocInfo {
    /// File name of the document.
    pub name: String,
    /// App id of the open file: `doc` (BlueDoc) or `slide` (BlueSlide).
    pub kind: String,
    /// Number of laid-out pages (slides for BlueSlide).
    pub page_count: usize,
    /// Size of each page.
    pub pages: Vec<PageSize>,
    /// Words in the main story.
    pub word_count: usize,
    /// The file contains a VBA project or XLM macros ([MS-OVBA]); kept, never run.
    pub macros: bool,
    /// The document is saved with a password ([MS-OFFCRYPTO] Agile Encryption).
    pub protected: bool,
    /// Runs converted from TCVN3 / VNI to Unicode on open (0 = none); the
    /// conversion is the first undo step.
    pub legacy_converted: usize,
}

impl DocInfo {
    /// JSON encoding for JS hosts.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// A glyph outline for the web canvas: SVG path in font units (y down).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlyphOutline {
    /// Font face id (index into the registered faces).
    pub face: u32,
    /// Glyph id within the face.
    pub glyph: u16,
    /// Font design units per em.
    pub units_per_em: u16,
    /// SVG path data in font units (y down).
    pub path: String,
}

/// Word count of the main story kept per top-level block (DocInfo after each edit).
#[derive(Default)]
struct WordCount {
    blocks: Vec<Vec<usize>>,
    total: usize,
}

impl WordCount {
    fn block(b: &Block) -> usize {
        bo_model::write::blocks_word_count(std::slice::from_ref(b))
    }

    /// Recount the changed blocks; everything when `changes` says so.
    fn update(&mut self, doc: &Document, changes: &BlockChanges) {
        let consistent = self.blocks.len() == doc.sections.len()
            && doc.sections.iter().enumerate().all(|(si, s)| {
                let delta = changes.section(si).map_or(0, |c| c.delta);
                self.blocks[si].len() as isize + delta == s.blocks.len() as isize
            });
        if changes.is_all() || !consistent {
            self.blocks = doc
                .sections
                .iter()
                .map(|s| s.blocks.iter().map(Self::block).collect())
                .collect();
        } else {
            for c in changes.sections() {
                let new = doc.sections[c.section].blocks[c.start..c.end]
                    .iter()
                    .map(Self::block);
                let v = &mut self.blocks[c.section];
                let e = c.old_end().min(v.len());
                v.splice(c.start.min(e)..e, new);
            }
        }
        self.total = self.blocks.iter().flatten().sum();
    }

    /// Words of `doc` (as `Document::plain_text` counts them).
    fn get(&self, doc: &Document) -> usize {
        // A deleted paragraph mark joins two paragraphs' text: counts are not additive.
        let joined = doc.sections.iter().any(|s| {
            s.blocks
                .iter()
                .any(|b| matches!(b, Block::Paragraph(p) if p.props.mark_del.is_some()))
        });
        if joined {
            doc.plain_text().split_whitespace().count()
        } else {
            self.total
        }
    }
}

/// The engine.
#[derive(Default)]
pub struct Engine {
    fonts: FontManager,
    cache: LayoutCache,
    name: String,
    session: Option<bo_edit::Session>,
    /// Document as last opened/saved (`modified` flag).
    saved_doc: Option<Document>,
    /// Blocks changed since `saved_doc` (only those are compared for `modified`).
    unsaved: BlockChanges,
    /// Layout of the open document, updated incrementally after each edit.
    layout: IncrementalLayout,
    /// Display list of each page of the open document, built when first requested.
    doc_display: Vec<OnceCell<PageList>>,
    /// The layout shows a mail-merge preview instead of the document.
    laid_out_preview: bool,
    words: WordCount,
    /// Display lists of the open presentation's slides.
    display: Option<DisplayList>,
    /// Column kept across vertical caret moves.
    goal_x: Option<f32>,
    /// Open presentation (BlueSlide); pages of `display` are its slides.
    pres: Option<bo_model::slide::Presentation>,
    /// Images referenced by slide `Image` commands (part name -> bytes).
    images: BTreeMap<String, Vec<u8>>,
    /// Selection and undo history of the open presentation.
    slides: slide_edit::SlideEditor,
    /// How tracked changes are displayed (follows `markup.mode`).
    view: RevisionView,
    /// Review display mode and markup options (BlueDoc).
    markup: markup::MarkupOptions,
    /// Markup (change bars, balloons) of each laid-out page; empty when none is drawn.
    page_markup: Vec<bo_layout::PageMarkup>,
    /// Field codes are shown instead of field results (Alt+F9).
    field_codes: bool,
    /// Author of new revisions and comments (kept across documents).
    author: Option<String>,
    /// Mail-merge data source and the previewed record.
    merge: Option<bo_edit::merge::MergeData>,
    merge_preview: Option<usize>,
    /// The open file has macros (warning bar; they are preserved, never executed).
    macros: bool,
    /// Runs the last `ConvertLegacyText` converted (reported in [`DocInfo`]).
    pub(crate) legacy_converted: usize,
    /// Password used by `save_docx` (Agile Encryption); `None` saves unencrypted.
    save_password: Option<String>,
    /// Live collaboration binding of the open document (BlueDoc).
    collab: Option<collab::DocCollab>,
    /// Live collaboration binding of the open presentation (BlueSlide).
    slide_collab: Option<slide_edit::collab::SlideCollab>,
    /// Slide clipboard (kept across decks of this engine).
    slide_clip: Option<slide_edit::clip::Clip>,
    /// Counts opened presentations (clipboard origin).
    deck_seq: u64,
    /// Open header/footer editor (BlueDoc).
    hf: Option<story::HfEdit>,
    /// Open footnote/endnote editor (BlueDoc).
    note: Option<notes::NoteEdit>,
    /// Open text box editor (BlueDoc).
    textbox: Option<textbox::BoxEdit>,
    /// Spelling as you type (BlueDoc).
    spell: spell::SpellState,
    /// AutoCorrect as you type (BlueDoc main text); off until a host sets options.
    autocorrect: Option<bo_edit::AutoCorrectOptions>,
    /// Package parts the document inspector drops on the next save.
    scrub: inspect::package::Scrub,
    /// Final mark, write protection, restricted editing, labels (CORE-SEC-001).
    pub(crate) sec: security::FileSecurity,
    /// Names of the current user for editable regions (kept across documents).
    pub(crate) identity: Vec<String>,
    /// Print / web / draft layout of the view (BlueDoc).
    view_layout: views::ViewLayout,
    /// The view layout changed since the last layout (lay out everything again).
    view_changed: bool,
    /// Formatting marks travel with the page display lists of the view.
    show_marks: bool,
    /// Slide show video / GIF export in progress (BlueSlide).
    video: Option<Box<slide_edit::video::VideoSession>>,
}

impl Engine {
    /// Engine without fonts (the host adds them with [`Engine::add_font`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// Engine preloaded with the fonts in `assets/fonts`.
    #[cfg(feature = "bundled-fonts")]
    pub fn with_bundled_fonts() -> Self {
        let mut e = Self::new();
        for f in bundled::FONTS {
            e.add_font(f.to_vec());
        }
        // English spelling (SCOWL en_US, assets/dictionaries); Vietnamese needs no data.
        let _ = e.load_dictionary(bundled::EN_US_AFF, bundled::EN_US_DIC);
        e
    }

    /// Register a font file; returns the number of faces added.
    pub fn add_font(&mut self, bytes: Vec<u8>) -> usize {
        let n = self.fonts.add_font(bytes).len();
        self.cache.clear();
        self.layout.reset();
        n
    }

    /// Number of loaded faces.
    pub fn face_count(&self) -> usize {
        self.fonts.len()
    }

    /// Lets documents use the installed OS fonts (desktop): a family that is not
    /// bundled loads from the index on first use, with face ids after the bundled ones.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_system_fonts(&mut self, index: bo_text::SystemFontIndex) {
        self.fonts.set_system_fonts(index);
        self.cache.clear();
        self.layout.reset();
    }

    /// Access the font manager.
    pub fn fonts(&self) -> &FontManager {
        &self.fonts
    }

    /// Open a document from bytes. The format is detected from content, then the name.
    pub fn open(&mut self, bytes: &[u8], name: &str) -> Result<DocInfo> {
        self.open_with_password(bytes, name, None)
    }

    /// Open a document, decrypting it with `password` when it is an encrypted package
    /// ([MS-OFFCRYPTO] §2.3.4.10 Agile / §2.3.4.5 Standard Encryption). Formats:
    /// `.docx`/`.docm`, `.doc` ([MS-DOC], `bo-binary`), `.odt` (ODF 1.3, `bo-odf`),
    /// `.pptx`, plain text.
    pub fn open_with_password(
        &mut self,
        bytes: &[u8],
        name: &str,
        password: Option<&str>,
    ) -> Result<DocInfo> {
        let lower = name.to_ascii_lowercase();
        let plain = decrypt_package(bytes, password)?;
        let encrypted = plain.is_some();
        let bytes = plain.as_deref().unwrap_or(bytes);
        let macros = bo_binary::has_macros(bytes);
        let is_zip = bytes.starts_with(b"PK\x03\x04");
        // Decks: `.pptx`, and `.ppt` / `.odp` converted to PresentationML.
        let deck = if is_zip && bo_pptx::is_pptx(bytes) {
            Some((None, name.to_owned()))
        } else if is_zip && bo_odf::detect_mimetype(bytes).as_deref() == Some(bo_odf::MIME_ODP) {
            let d = bo_odf::read_odp_deck(bytes).map_err(|e| EngineError::Format(e.to_string()))?;
            Some((Some(legacy_slides::odp_deck(&d)), with_ext(name, "pptx")))
        } else if bytes.starts_with(CFB_SIGNATURE)
            && matches!(bo_binary::detect(bytes), Some(bo_binary::LegacyKind::Ppt))
        {
            let d = bo_binary::read_ppt(bytes).map_err(|e| EngineError::Format(e.to_string()))?;
            Some((Some(legacy_slides::ppt_deck(&d)), with_ext(name, "pptx")))
        } else {
            None
        };
        if let Some((spec, deck_name)) = deck {
            match spec {
                None => {
                    self.open_pptx(bytes, &deck_name)?;
                    self.open_security(bytes);
                }
                Some(spec) => {
                    let pptx = bo_pptx::build::build_pptx(&spec)
                        .map_err(|e| EngineError::Format(e.to_string()))?;
                    self.open_pptx(&pptx, &deck_name)?;
                }
            }
            self.macros = macros;
            // An encrypted deck is saved with the same password unless changed.
            self.save_password = if encrypted {
                password.map(str::to_owned)
            } else {
                None
            };
            return self.info();
        }
        let doc = if is_zip && bo_odf::detect_mimetype(bytes).as_deref() == Some(bo_odf::MIME_ODT) {
            bo_odf::read_odt(bytes).map_err(|e| EngineError::Format(e.to_string()))?
        } else if is_zip {
            let mut d = bo_docx::read_docx(bytes)?;
            smartart::render_smartart(&mut d, &mut self.fonts);
            media::convert_media(&mut d, &mut self.fonts);
            d
        } else if bytes.starts_with(CFB_SIGNATURE) {
            match bo_binary::detect(bytes) {
                Some(bo_binary::LegacyKind::Doc) => {
                    bo_binary::read_doc(bytes).map_err(|e| EngineError::Format(e.to_string()))?
                }
                _ => return Err(EngineError::Unsupported(name.to_owned())),
            }
        } else if lower.ends_with(".txt")
            || lower.ends_with(".md")
            || std::str::from_utf8(bytes).is_ok()
        {
            plain_text_document(&String::from_utf8_lossy(bytes))
        } else {
            return Err(EngineError::Unsupported(name.to_owned()));
        };
        // Legacy and ODF documents are saved as .docx.
        let name = match lower.rsplit_once('.') {
            Some((_, "doc" | "odt")) => {
                format!("{}.docx", name.rsplit_once('.').map_or(name, |(b, _)| b))
            }
            _ => name.to_owned(),
        };
        self.load(doc, &name)?;
        // Pictures of signed signature lines (CORE-SIGLINE-001).
        self.register_signature_images();
        // Fields read the document properties (§17.16.5: AUTHOR, TITLE, SAVEDATE, …).
        let props = document_properties(if is_zip { bytes } else { &[] }, &name, bytes.len());
        if let Some(s) = self.session.as_mut() {
            s.set_field_env(bo_edit::fields::FieldEnv {
                vi: true,
                props,
                ..bo_edit::fields::FieldEnv::default()
            });
        }
        // Text typed in TCVN3 / VNI fonts (.docx, .odt): converted to Unicode as
        // the first undo step, reported in DocInfo (.doc converts while reading).
        if is_zip {
            self.command(edit::Command::ConvertLegacyText)?;
            self.open_security(bytes);
        }
        self.macros = macros;
        // An encrypted document is saved with the same password unless changed.
        self.save_password = if encrypted {
            password.map(str::to_owned)
        } else {
            None
        };
        self.info()
    }

    /// Password for the next `.docx` saves (`None` or empty: no encryption).
    pub fn set_save_password(&mut self, password: Option<&str>) {
        self.save_password = password.filter(|p| !p.is_empty()).map(str::to_owned);
    }

    /// Load an in-memory document model.
    pub fn load(&mut self, doc: Document, name: &str) -> Result<DocInfo> {
        let opts = self.options();
        self.layout.reset();
        let all = BlockChanges::everything();
        let shown = views::view_document(&doc, self.view_layout);
        self.view_changed = false;
        self.layout.update(
            shown.as_ref().unwrap_or(&doc),
            &mut self.fonts,
            &mut self.cache,
            opts,
            &all,
        )?;
        self.doc_display = (0..self.layout.document().pages.len())
            .map(|_| OnceCell::new())
            .collect();
        self.laid_out_preview = false;
        self.words.update(&doc, &all);
        self.name = name.to_owned();
        self.saved_doc = Some(doc.clone());
        self.unsaved = BlockChanges::default();
        let mut session = bo_edit::Session::new(doc);
        session.take_changes();
        if let Some(a) = &self.author {
            session.set_author(a);
        }
        // Fields: the file name until the package's properties are read (`open`).
        session.set_field_env(bo_edit::fields::FieldEnv {
            vi: true,
            props: document_properties(&[], name, 0),
            ..bo_edit::fields::FieldEnv::default()
        });
        self.session = Some(session);
        self.merge_preview = None;
        self.macros = false;
        self.legacy_converted = 0;
        self.save_password = None;
        self.display = None;
        self.goal_x = None;
        self.pres = None;
        self.images.clear();
        self.collab = None;
        self.slide_collab = None;
        self.hf = None;
        self.note = None;
        self.textbox = None;
        self.sec = security::FileSecurity::default();
        self.page_markup.clear();
        self.refresh_markup(&mut Vec::new());
        self.scrub = inspect::package::Scrub::default();
        self.info()
    }

    /// Start a new blank document (A4, Times New Roman 13 pt).
    pub fn new_document(&mut self, name: &str) -> Result<DocInfo> {
        self.load(plain_text_document(""), name)
    }

    /// Layout options from the view settings.
    fn options(&self) -> LayoutOptions {
        LayoutOptions {
            view: self.view,
            field_codes: self.field_codes,
            marks: self.markup.filter(),
        }
    }

    /// Review display mode and markup options.
    pub fn markup_options(&self) -> &markup::MarkupOptions {
        &self.markup
    }

    /// Update the markup options; returns whether they changed (the caller lays out
    /// again).
    pub(crate) fn set_markup(&mut self, u: markup::MarkupUpdate) -> bool {
        let changed = self.markup.apply(u);
        self.view = self.markup.layout_view();
        changed
    }

    /// Recompute the markup of every page; pages whose markup changed are added to
    /// `dirty` (the host draws them again).
    fn refresh_markup(&mut self, dirty: &mut Vec<usize>) {
        let next = match &self.session {
            Some(s) => {
                let marks = self.markup.marks(&s.revisions(), &s.comments());
                if marks.is_empty() {
                    Vec::new()
                } else {
                    bo_layout::page_markup(
                        self.layout.document(),
                        &marks,
                        &self.markup.style(&marks),
                    )
                }
            }
            None => Vec::new(),
        };
        let empty = bo_layout::PageMarkup::default();
        for i in 0..self.layout.document().pages.len() {
            let a = self.page_markup.get(i).unwrap_or(&empty);
            let b = next.get(i).unwrap_or(&empty);
            if a != b && !dirty.contains(&i) {
                dirty.push(i);
            }
        }
        self.page_markup = next;
    }

    /// Markup of a page (change bars and balloons drawn beside it), if any.
    pub fn page_markup(&self, index: usize) -> Option<&bo_layout::PageMarkup> {
        self.page_markup.get(index).filter(|m| !m.is_empty())
    }

    /// How tracked changes are displayed.
    pub fn revision_view(&self) -> RevisionView {
        self.view
    }

    /// Lay out again after an edit (incrementally: only the changed blocks and the
    /// pages they affect, prompt §5.4); returns the pages whose content changed.
    fn relayout(&mut self) -> Result<Vec<usize>> {
        let opts = self.options();
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let mut changes = s.take_changes();
        let doc = s.doc();
        self.unsaved.extend(&changes);
        self.words.update(doc, &changes);
        // Mail-merge preview: the record's values in the merge fields (§17.16.5.35).
        let preview = match (&self.merge, self.merge_preview) {
            (Some(m), Some(r)) => Some(bo_edit::merge::preview(doc, m, Some(r))),
            _ => None,
        };
        if preview.is_some() || self.laid_out_preview || self.view_changed {
            changes = BlockChanges::everything();
        }
        self.view_changed = false;
        self.laid_out_preview = preview.is_some();
        let doc = preview.as_ref().unwrap_or(doc);
        // Web and draft layouts lay out a view copy (same blocks, other page setup).
        let shown = views::view_document(doc, self.view_layout);
        let doc = shown.as_ref().unwrap_or(doc);
        let dirty = self
            .layout
            .update(doc, &mut self.fonts, &mut self.cache, opts, &changes)?;
        let n = self.layout.document().pages.len();
        self.doc_display.truncate(n);
        self.doc_display.resize_with(n, OnceCell::new);
        for &i in &dirty {
            if let Some(c) = self.doc_display.get_mut(i) {
                *c = OnceCell::new();
            }
        }
        let mut dirty = dirty;
        self.refresh_markup(&mut dirty);
        Ok(dirty)
    }

    /// The document differs from the last opened/saved one. Only blocks changed since
    /// then are compared (every other part changes through ops that mark everything).
    fn is_modified(&self) -> bool {
        if self.sec.dirty() {
            return true;
        }
        let (Some(saved), Some(s)) = (&self.saved_doc, &self.session) else {
            return self.session.is_some();
        };
        let doc = s.doc();
        let mut ch = self.unsaved.clone();
        ch.extend(s.pending_changes());
        if ch.is_empty() && !ch.parts_changed() {
            return false;
        }
        if ch.parts_changed()
            || ch.headers().is_some()
            || saved.sections.len() != doc.sections.len()
        {
            return saved != doc;
        }
        ch.sections().iter().any(|c| {
            let (a, b) = (
                &doc.sections[c.section].blocks,
                &saved.sections[c.section].blocks,
            );
            a.len() as isize != b.len() as isize + c.delta
                || a.get(c.start..c.end) != b.get(c.start..c.old_end())
        })
    }

    /// Mark the current state as saved (clears `modified`).
    pub fn mark_saved(&mut self) {
        self.saved_doc = self.session.as_ref().map(|s| s.doc().clone());
        self.unsaved = BlockChanges::default();
        self.slides.mark_saved();
        self.sec.mark_saved();
    }

    /// Open a `.pptx` presentation (BlueSlide).
    pub fn open_pptx(&mut self, bytes: &[u8], name: &str) -> Result<DocInfo> {
        let pres = bo_pptx::read_pptx(bytes)?;
        self.load_presentation(pres, name)
    }

    /// Load an in-memory presentation and render every slide.
    pub fn load_presentation(
        &mut self,
        pres: bo_model::slide::Presentation,
        name: &str,
    ) -> Result<DocInfo> {
        let mut images = BTreeMap::new();
        let mut pages = Vec::with_capacity(pres.slides.len());
        for i in 0..pres.slides.len() {
            pages.push(bo_pptx::render_slide(
                &pres,
                i,
                &mut self.fonts,
                &mut images,
            )?);
        }
        self.name = name.to_owned();
        self.session = None;
        self.saved_doc = None;
        self.goal_x = None;
        self.layout.reset();
        self.doc_display.clear();
        self.page_markup.clear();
        self.display = Some(DisplayList { pages });
        self.pres = Some(pres);
        self.images = images;
        self.slides = slide_edit::SlideEditor::default();
        self.macros = false;
        self.deck_seq += 1;
        self.collab = None;
        self.slide_collab = None;
        self.macros = false;
        self.save_password = None;
        self.scrub = inspect::package::Scrub::default();
        self.sec = security::FileSecurity::default();
        self.info()
    }

    /// Re-render one slide after the presentation model was edited.
    pub fn refresh_slide(&mut self, index: usize) -> Result<&PageList> {
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let page = slide_edit::master::render_page(
            pres,
            self.slides.in_master_view(),
            index,
            &mut self.fonts,
            &mut self.images,
        )?;
        let display = self.display.as_mut().ok_or(EngineError::NoDocument)?;
        let slot = display
            .pages
            .get_mut(index)
            .ok_or(EngineError::PageOutOfRange(index))?;
        *slot = page;
        Ok(slot)
    }

    /// The open presentation.
    pub fn presentation(&self) -> Option<&bo_model::slide::Presentation> {
        self.pres.as_ref()
    }

    /// Mutable access to the open presentation (call [`Engine::refresh_slide`] afterwards).
    pub fn presentation_mut(&mut self) -> Option<&mut bo_model::slide::Presentation> {
        self.pres.as_mut()
    }

    /// Number of slides (0 when no presentation is open).
    pub fn slide_count(&self) -> usize {
        self.pres.as_ref().map_or(0, |p| p.slides.len())
    }

    /// Display list of a slide.
    pub fn slide(&self, index: usize) -> Result<&PageList> {
        if self.pres.is_none() {
            return Err(EngineError::NoDocument);
        }
        self.page(index)
    }

    /// Display list of a slide as JSON.
    pub fn slide_json(&self, index: usize) -> Result<String> {
        Ok(self.slide(index)?.to_json())
    }

    /// Thumbnail display list of a slide scaled to `width` points, as JSON.
    pub fn slide_thumbnail_json(&mut self, index: usize, width: f32) -> Result<String> {
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let page = if self.slides.in_master_view() {
            let p = slide_edit::master::render_page(
                pres,
                true,
                index,
                &mut self.fonts,
                &mut self.images,
            )?;
            slide_edit::master::scaled(p, width)
        } else {
            bo_pptx::render_thumbnail(pres, index, &mut self.fonts, &mut self.images, width)?
        };
        Ok(page.to_json())
    }

    /// Speaker notes text of a slide.
    pub fn slide_notes(&self, index: usize) -> Result<String> {
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let s = pres
            .slides
            .get(index)
            .ok_or(EngineError::PageOutOfRange(index))?;
        Ok(s.notes
            .as_ref()
            .map(bo_model::slide::Notes::text)
            .unwrap_or_default())
    }

    /// Bytes of an image referenced by an `Image` command.
    pub fn image_bytes(&self, id: &str) -> Option<&[u8]> {
        self.media(id)
    }

    /// Save the open presentation as `.pptx`, encrypted with the save password
    /// when one is set ([MS-OFFCRYPTO] §2.3.4.10 Agile Encryption).
    pub fn save_pptx(&self) -> Result<Vec<u8>> {
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        // While the master view is open the deck's slides are set aside.
        let assembled = slide_edit::master::assemble(pres, &self.slides)
            .or_else(|| slide_edit::page_master::assemble(pres, &self.slides));
        let bytes = bo_pptx::write_pptx(assembled.as_ref().unwrap_or(pres))?;
        let bytes = inspect::package::scrub(&bytes, &self.scrub)?;
        let bytes = self.sec.apply(bytes)?;
        match &self.save_password {
            Some(pw) => {
                bo_crypto::encrypt(&bytes, pw).map_err(|e| EngineError::Crypto(e.to_string()))
            }
            None => Ok(bytes),
        }
    }

    /// Image sources for backends: the document's media parts, then slide images.
    fn image_source(&self) -> impl bo_render::ImageSource + '_ {
        static EMPTY: std::sync::OnceLock<bo_render::Media> = std::sync::OnceLock::new();
        let media = self
            .document()
            .map_or_else(|| EMPTY.get_or_init(bo_render::Media::new), |d| &d.media);
        bo_render::image::Chain(media, &self.images)
    }

    /// Summary of the open document.
    pub fn info(&self) -> Result<DocInfo> {
        let (kind, words, pages): (&str, usize, Vec<PageSize>) = match (self.document(), &self.pres)
        {
            (Some(doc), _) => (
                "doc",
                self.words.get(doc),
                self.layout
                    .document()
                    .pages
                    .iter()
                    .map(|p| PageSize {
                        width: p.width,
                        height: p.height,
                    })
                    .collect(),
            ),
            (None, Some(p)) => (
                "slide",
                presentation_words(p),
                self.display
                    .as_ref()
                    .ok_or(EngineError::NoDocument)?
                    .pages
                    .iter()
                    .map(|p| PageSize {
                        width: p.width,
                        height: p.height,
                    })
                    .collect(),
            ),
            (None, None) => return Err(EngineError::NoDocument),
        };
        Ok(DocInfo {
            name: self.name.clone(),
            kind: kind.into(),
            page_count: pages.len(),
            pages,
            word_count: words,
            macros: self.macros,
            protected: self.save_password.is_some(),
            legacy_converted: self.legacy_converted,
        })
    }

    /// The open document model.
    pub fn document(&self) -> Option<&Document> {
        self.session.as_ref().map(bo_edit::Session::doc)
    }

    /// The editing session of the open document.
    pub fn session(&self) -> Option<&bo_edit::Session> {
        self.session.as_ref()
    }

    /// The current layout.
    pub fn layout(&self) -> Option<&LayoutDocument> {
        self.session.as_ref().map(|_| self.layout.document())
    }

    /// How the last layout update was done (full or incremental).
    pub fn last_layout_update(&self) -> bo_layout::UpdateKind {
        self.layout.last_update()
    }

    /// Let the incremental layout stop once page breaks re-synchronise (default), or
    /// make it re-paginate to the end of the document (worst-case benchmarks).
    pub fn set_layout_resync(&mut self, enabled: bool) {
        self.layout.set_resync(enabled);
    }

    /// Display list of a page (built from the layout when first requested after a change).
    pub fn page(&self, index: usize) -> Result<&PageList> {
        if self.session.is_none() {
            return self
                .display
                .as_ref()
                .ok_or(EngineError::NoDocument)?
                .pages
                .get(index)
                .ok_or(EngineError::PageOutOfRange(index));
        }
        let page = self
            .layout
            .document()
            .pages
            .get(index)
            .ok_or(EngineError::PageOutOfRange(index))?;
        let cell = self
            .doc_display
            .get(index)
            .ok_or(EngineError::PageOutOfRange(index))?;
        Ok(cell.get_or_init(|| build_page_list(page, &self.fonts)))
    }

    /// Display list of the whole open document or presentation.
    fn display_list(&self) -> Result<std::borrow::Cow<'_, DisplayList>> {
        if self.session.is_none() {
            return Ok(std::borrow::Cow::Borrowed(
                self.display.as_ref().ok_or(EngineError::NoDocument)?,
            ));
        }
        let pages = (0..self.layout.document().pages.len())
            .map(|i| self.page(i).cloned())
            .collect::<Result<Vec<_>>>()?;
        Ok(std::borrow::Cow::Owned(DisplayList { pages }))
    }

    /// Display list of a page as JSON (the bridge format for JS hosts).
    pub fn page_json(&self, index: usize) -> Result<String> {
        // The markup travels with the page for the view only (printing and PDF export
        // use the display list alone).
        #[derive(serde::Serialize)]
        struct WithMarkup<'a> {
            #[serde(flatten)]
            page: &'a PageList,
            #[serde(skip_serializing_if = "Option::is_none")]
            markup: Option<&'a bo_layout::PageMarkup>,
            /// Formatting marks (¶ · →) when shown (view only).
            #[serde(skip_serializing_if = "Vec::is_empty")]
            marks: Vec<views::FormatMark>,
        }
        let page = self.page(index)?;
        let markup = self.session.as_ref().and(self.page_markup(index));
        let marks = if self.show_marks {
            self.formatting_marks(index)
        } else {
            Vec::new()
        };
        if markup.is_none() && marks.is_empty() {
            return Ok(page.to_json());
        }
        Ok(serde_json::to_string(&WithMarkup {
            page,
            markup,
            marks,
        })
        .unwrap_or_default())
    }

    /// Glyph outlines for the canvas renderer.
    pub fn glyph_outlines(&self, face: u32, glyphs: &[u16]) -> Vec<GlyphOutline> {
        if face as usize >= self.fonts.len() {
            return Vec::new();
        }
        let id = FaceId(face);
        let upem = self.fonts.info(id).units_per_em;
        glyphs
            .iter()
            .map(|&g| GlyphOutline {
                face,
                glyph: g,
                units_per_em: upem,
                path: outline_svg_path(&self.fonts.outline(id, g)),
            })
            .collect()
    }

    /// JSON variant of [`Engine::glyph_outlines`].
    pub fn glyph_outlines_json(&self, face: u32, glyphs: &[u16]) -> String {
        serde_json::to_string(&self.glyph_outlines(face, glyphs)).unwrap_or_else(|_| "[]".into())
    }

    /// Export the whole document as PDF.
    pub fn export_pdf(&self) -> Result<Vec<u8>> {
        let display = self.display_list()?;
        let display = display.as_ref();
        let title = self
            .name
            .rsplit_once('.')
            .map_or(self.name.clone(), |(b, _)| b.to_owned());
        Ok(bo_pdf::write_pdf_with_images(
            display,
            &self.fonts,
            &bo_pdf::PdfMeta { title: Some(title) },
            &self.image_source(),
        )?)
    }

    /// Render one page to PNG at `scale` pixels per point.
    pub fn render_png(&self, index: usize, scale: f32) -> Result<Vec<u8>> {
        Ok(bo_raster::render_page_png_with(
            self.page(index)?,
            &self.fonts,
            scale,
            &self.image_source(),
        )?)
    }

    /// Bytes of a document media part or slide image referenced by an `image` display command.
    pub fn media(&self, name: &str) -> Option<&[u8]> {
        self.document()
            .and_then(|d| d.media.get(name))
            .map(|b| b.as_slice())
            .or_else(|| self.images.get(name).map(Vec::as_slice))
            // Slide media parts (video / audio clips played by the hosts).
            .or_else(|| self.pres.as_ref().and_then(|p| p.part_data(name)))
    }

    /// Bytes of a media part or of a derived picture id (`media#fx:…`: the picture
    /// with its colour adjustments applied, PNG; CORE-PIC-001).
    pub fn media_resolved(&self, name: &str) -> Option<std::borrow::Cow<'_, [u8]>> {
        if let Some(b) = self.media(name) {
            return Some(std::borrow::Cow::Borrowed(b));
        }
        let (base, _) = bo_model::picture::split_fx_id(name)?;
        bo_raster::picfx::derive_id_png(self.media(base)?, name).map(std::borrow::Cow::Owned)
    }

    /// Set the mail-merge data source (CSV or XLSX); returns the number of records.
    pub fn set_merge_data(&mut self, bytes: &[u8], name: &str) -> Result<usize> {
        let d = merge::read_data(bytes, name)?;
        let n = d.rows.len();
        self.merge = Some(d);
        Ok(n)
    }

    /// Serialize the open document as `.docx` (lossless for unchanged parts, ADR-0005),
    /// encrypted with the save password when one is set ([MS-OFFCRYPTO] §2.3.4.10).
    pub fn save_docx(&self) -> Result<Vec<u8>> {
        // Hosts call `mark_saved` once the bytes are stored.
        let doc = self.document().ok_or(EngineError::NoDocument)?;
        // §17.15.1.89 trackRevisions follows the session's track-changes mode.
        let tracked = self
            .session
            .as_ref()
            .map(bo_edit::Session::doc_with_track_setting);
        let doc = tracked.as_deref().unwrap_or(doc);
        let bytes = bo_docx::write_docx(doc)?;
        // Inspector removals of package parts (comment parts only once the comments are gone).
        let mut scrub = self.scrub.clone();
        scrub.comments &= doc.comments.is_empty();
        let bytes = inspect::package::scrub(&bytes, &scrub)?;
        let bytes = self.sec.apply(bytes)?;
        match &self.save_password {
            Some(pw) => {
                bo_crypto::encrypt(&bytes, pw).map_err(|e| EngineError::Crypto(e.to_string()))
            }
            None => Ok(bytes),
        }
    }
}

/// `name` with its extension replaced by `ext`.
fn with_ext(name: &str, ext: &str) -> String {
    format!("{}.{ext}", name.rsplit_once('.').map_or(name, |(b, _)| b))
}

/// Decrypts an encrypted OOXML package ([MS-OFFCRYPTO] §2.3.4.10 Agile /
/// §2.3.4.5 Standard Encryption, a CFB with `EncryptionInfo` + `EncryptedPackage`).
/// `Ok(None)` for files that are not encrypted.
pub(crate) fn decrypt_package(bytes: &[u8], password: Option<&str>) -> Result<Option<Vec<u8>>> {
    if !(bytes.starts_with(CFB_SIGNATURE) && bo_crypto::is_encrypted(bytes)) {
        return Ok(None);
    }
    let pw = password
        .filter(|p| !p.is_empty())
        .ok_or(EngineError::PasswordRequired)?;
    bo_crypto::decrypt(bytes, pw)
        .map(Some)
        .map_err(|e| match e {
            bo_crypto::CryptoError::WrongPassword
            | bo_crypto::CryptoError::IntegrityCheckFailed => EngineError::WrongPassword,
            other => EngineError::Crypto(other.to_string()),
        })
}

/// Words in all text bodies of all slides.
fn presentation_words(p: &bo_model::slide::Presentation) -> usize {
    let mut n = 0;
    for s in &p.slides {
        bo_pptx::visit(&s.data.shapes, &mut |node| {
            if let bo_model::slide::ShapeNode::Shape(sh) = node {
                if let Some(t) = &sh.text {
                    n += t.text().split_whitespace().count();
                }
            }
        });
    }
    n
}

/// Build a document from plain text: one paragraph per line, default page setup.
pub fn plain_text_document(text: &str) -> Document {
    let mut blocks: Vec<Block> = text
        .lines()
        .map(|l| {
            Block::Paragraph(Paragraph::new(
                bo_model::ParaProps::default(),
                if l.is_empty() {
                    vec![]
                } else {
                    vec![Run::text(l, RunProps::default())]
                },
            ))
        })
        .collect();
    if blocks.is_empty() {
        blocks.push(Block::Paragraph(Paragraph::default()));
    }
    Document {
        sections: vec![Section::new(blocks)],

        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(bo_model::HalfPoints(26)),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A word-processing document read from `.docx`, `.odt` or `.doc` bytes (for comparing).
pub(crate) fn word_document(bytes: &[u8], name: &str) -> Result<Document> {
    let is_zip = bytes.starts_with(b"PK");
    if is_zip && bo_odf::detect_mimetype(bytes).as_deref() == Some(bo_odf::MIME_ODT) {
        return bo_odf::read_odt(bytes).map_err(|e| EngineError::Format(e.to_string()));
    }
    if is_zip {
        return Ok(bo_docx::read_docx(bytes)?);
    }
    if bytes.starts_with(CFB_SIGNATURE)
        && matches!(bo_binary::detect(bytes), Some(bo_binary::LegacyKind::Doc))
    {
        return bo_binary::read_doc(bytes).map_err(|e| EngineError::Format(e.to_string()));
    }
    Err(EngineError::Unsupported(name.to_owned()))
}

/// Text of the first element `local` (any prefix) of a small XML part, entities decoded.
fn element_text(xml: &str, local: &str) -> Option<String> {
    let mut at = 0;
    while let Some(k) = xml[at..].find('<') {
        let start = at + k + 1;
        let end = start + xml[start..].find('>')?;
        let tag = &xml[start..end];
        let name = tag.split_whitespace().next().unwrap_or("");
        let bare = name.rsplit(':').next().unwrap_or(name);
        if bare == local && !tag.starts_with('/') && !tag.ends_with('/') {
            let close = xml[end + 1..].find("</")? + end + 1;
            let v = &xml[end + 1..close];
            return Some(
                v.replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&quot;", "\"")
                    .replace("&apos;", "'")
                    .replace("&amp;", "&"),
            );
        }
        at = end + 1;
    }
    None
}

/// Document properties for fields: core (§ OPC Part 2, docProps/core.xml) and extended
/// (docProps/app.xml) properties by lower-case field name, plus the file name and size.
pub(crate) fn document_properties(
    bytes: &[u8],
    name: &str,
    size: usize,
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    out.insert(
        "filename".to_owned(),
        name.rsplit(['/', '\\']).next().unwrap_or(name).to_owned(),
    );
    out.insert("filepath".to_owned(), name.to_owned());
    out.insert("filesize".to_owned(), size.to_string());
    let Ok(pkg) = bo_opc::Package::read(bytes, &bo_opc::Limits::default()) else {
        return out;
    };
    let text = |part: &str| {
        pkg.part(part)
            .map(|p| String::from_utf8_lossy(&p.data).into_owned())
            .unwrap_or_default()
    };
    let core = text("/docProps/core.xml");
    for (tag, key) in [
        ("title", "title"),
        ("subject", "subject"),
        ("creator", "author"),
        ("keywords", "keywords"),
        ("description", "comments"),
        ("lastModifiedBy", "lastsavedby"),
        ("revision", "revnum"),
        ("created", "createdate"),
        ("modified", "savedate"),
        ("lastPrinted", "printdate"),
        ("category", "category"),
    ] {
        if let Some(v) = element_text(&core, tag).filter(|v| !v.trim().is_empty()) {
            out.insert(key.to_owned(), v.trim().to_owned());
        }
    }
    let app = text("/docProps/app.xml");
    for (tag, key) in [
        ("Template", "template"),
        ("TotalTime", "edittime"),
        ("Company", "company"),
        ("Manager", "manager"),
    ] {
        if let Some(v) = element_text(&app, tag).filter(|v| !v.trim().is_empty()) {
            out.insert(key.to_owned(), v.trim().to_owned());
        }
    }
    out
}
