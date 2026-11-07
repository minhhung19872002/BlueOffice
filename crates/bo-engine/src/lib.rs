//! `bo-engine` — one API over the whole core pipeline
//! (open → layout → display list → PNG/PDF).
//!
//! Hosts: `bo-wasm` (browser, inside a Web Worker), the Tauri desktop shell
//! (native) and `bo-cli`. Because every host calls this same code, web and
//! desktop produce byte-identical display lists (see ADR-0002).
#![forbid(unsafe_code)]

use bo_layout::{layout_document, LayoutCache, LayoutDocument};
use bo_model::{Block, Document, Paragraph, Run, RunProps, Section};
use bo_render::{build_display_list, DisplayList, PageList};
use bo_text::{outline_svg_path, FaceId, FontManager};
use serde::Serialize;
use std::collections::BTreeMap;

#[cfg(feature = "bundled-fonts")]
mod bundled;
pub mod edit;
pub mod nav;
pub mod sheet;
pub mod slide_edit;

pub use edit::{Command, EditResult};
pub use sheet::SheetEngine;
pub use slide_edit::{SlideCommand, SlideEditResult};

/// Engine errors (surfaced to JS / Tauri as strings).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("unsupported file type: {0}")]
    Unsupported(String),
    #[error("cannot read document: {0}")]
    Docx(#[from] bo_docx::DocxError),
    #[error("cannot read presentation: {0}")]
    Pptx(#[from] bo_pptx::PptxError),
    #[error("layout failed: {0}")]
    Layout(#[from] bo_layout::LayoutError),
    #[error("no document is open")]
    NoDocument,
    #[error("page {0} out of range")]
    PageOutOfRange(usize),
    #[error("PDF export failed: {0}")]
    Pdf(#[from] bo_pdf::PdfError),
    #[error("spreadsheet error: {0}")]
    Sheet(String),
    #[error("rendering failed: {0}")]
    Raster(#[from] bo_raster::RasterError),
    #[error("invalid command: {0}")]
    BadCommand(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, EngineError>;

/// Page size in points.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct PageSize {
    pub width: f32,
    pub height: f32,
}

/// Summary returned after opening a document.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocInfo {
    pub name: String,
    /// App id of the open file: `doc` (BlueDoc) or `slide` (BlueSlide).
    pub kind: String,
    pub page_count: usize,
    pub pages: Vec<PageSize>,
    pub word_count: usize,
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
    pub face: u32,
    pub glyph: u16,
    pub units_per_em: u16,
    pub path: String,
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
    layout: Option<LayoutDocument>,
    display: Option<DisplayList>,
    /// Column kept across vertical caret moves.
    goal_x: Option<f32>,
    /// Open presentation (BlueSlide); pages of `display` are its slides.
    pres: Option<bo_model::slide::Presentation>,
    /// Images referenced by slide `Image` commands (part name -> bytes).
    images: BTreeMap<String, Vec<u8>>,
    /// Selection and undo history of the open presentation.
    slides: slide_edit::SlideEditor,
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
        e
    }

    /// Register a font file; returns the number of faces added.
    pub fn add_font(&mut self, bytes: Vec<u8>) -> usize {
        let n = self.fonts.add_font(bytes).len();
        self.cache.clear();
        n
    }

    /// Number of loaded faces.
    pub fn face_count(&self) -> usize {
        self.fonts.len()
    }

    /// Access the font manager.
    pub fn fonts(&self) -> &FontManager {
        &self.fonts
    }

    /// Open a document from bytes. The format is detected from content, then the name.
    pub fn open(&mut self, bytes: &[u8], name: &str) -> Result<DocInfo> {
        let lower = name.to_ascii_lowercase();
        let is_zip = bytes.starts_with(b"PK\x03\x04");
        if is_zip && bo_pptx::is_pptx(bytes) {
            return self.open_pptx(bytes, name);
        }
        let doc = if is_zip {
            bo_docx::read_docx(bytes)?
        } else if lower.ends_with(".txt")
            || lower.ends_with(".md")
            || std::str::from_utf8(bytes).is_ok()
        {
            plain_text_document(&String::from_utf8_lossy(bytes))
        } else {
            return Err(EngineError::Unsupported(name.to_owned()));
        };
        self.load(doc, name)
    }

    /// Load an in-memory document model.
    pub fn load(&mut self, doc: Document, name: &str) -> Result<DocInfo> {
        let layout = layout_document(&doc, &mut self.fonts, &mut self.cache)?;
        let display = build_display_list(&layout, &self.fonts);
        self.name = name.to_owned();
        self.saved_doc = Some(doc.clone());
        self.session = Some(bo_edit::Session::new(doc));
        self.layout = Some(layout);
        self.display = Some(display);
        self.goal_x = None;
        self.pres = None;
        self.images.clear();
        self.info()
    }

    /// Start a new blank document (A4, Times New Roman 13 pt).
    pub fn new_document(&mut self, name: &str) -> Result<DocInfo> {
        self.load(plain_text_document(""), name)
    }

    /// Lay out again after an edit; returns the pages whose display list changed.
    fn relayout(&mut self) -> Result<Vec<usize>> {
        let doc = self.session.as_ref().ok_or(EngineError::NoDocument)?.doc();
        let layout = layout_document(doc, &mut self.fonts, &mut self.cache)?;
        let display = build_display_list(&layout, &self.fonts);
        let old = self.display.take().unwrap_or_default();
        let dirty = if old.pages.len() == display.pages.len() {
            (0..display.pages.len())
                .filter(|&i| old.pages[i] != display.pages[i])
                .collect()
        } else {
            (0..display.pages.len()).collect()
        };
        self.layout = Some(layout);
        self.display = Some(display);
        Ok(dirty)
    }

    /// Mark the current state as saved (clears `modified`).
    pub fn mark_saved(&mut self) {
        self.saved_doc = self.session.as_ref().map(|s| s.doc().clone());
        self.slides.mark_saved();
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
        self.layout = None;
        self.display = Some(DisplayList { pages });
        self.pres = Some(pres);
        self.images = images;
        self.slides = slide_edit::SlideEditor::default();
        self.info()
    }

    /// Re-render one slide after the presentation model was edited.
    pub fn refresh_slide(&mut self, index: usize) -> Result<&PageList> {
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let page = bo_pptx::render_slide(pres, index, &mut self.fonts, &mut self.images)?;
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
        let page =
            bo_pptx::render_thumbnail(pres, index, &mut self.fonts, &mut self.images, width)?;
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

    /// Save the open presentation as `.pptx`.
    pub fn save_pptx(&self) -> Result<Vec<u8>> {
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        Ok(bo_pptx::write_pptx(pres)?)
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
        let display = self.display.as_ref().ok_or(EngineError::NoDocument)?;
        let (kind, words) = match (self.document(), &self.pres) {
            (Some(doc), _) => ("doc", doc.plain_text().split_whitespace().count()),
            (None, Some(p)) => ("slide", presentation_words(p)),
            (None, None) => return Err(EngineError::NoDocument),
        };
        Ok(DocInfo {
            name: self.name.clone(),
            kind: kind.into(),
            page_count: display.pages.len(),
            pages: display
                .pages
                .iter()
                .map(|p| PageSize {
                    width: p.width,
                    height: p.height,
                })
                .collect(),
            word_count: words,
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
        self.layout.as_ref()
    }

    /// Display list of a page.
    pub fn page(&self, index: usize) -> Result<&PageList> {
        self.display
            .as_ref()
            .ok_or(EngineError::NoDocument)?
            .pages
            .get(index)
            .ok_or(EngineError::PageOutOfRange(index))
    }

    /// Display list of a page as JSON (the bridge format for JS hosts).
    pub fn page_json(&self, index: usize) -> Result<String> {
        Ok(self.page(index)?.to_json())
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
        let display = self.display.as_ref().ok_or(EngineError::NoDocument)?;
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
    }

    /// Serialize the open document as `.docx` (lossless for unchanged parts, ADR-0005).
    pub fn save_docx(&self) -> Result<Vec<u8>> {
        // Hosts call `mark_saved` once the bytes are stored.
        let doc = self.document().ok_or(EngineError::NoDocument)?;
        Ok(bo_docx::write_docx(doc)?)
    }
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
