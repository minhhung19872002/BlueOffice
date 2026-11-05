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

#[cfg(feature = "bundled-fonts")]
mod bundled;

/// Engine errors (surfaced to JS / Tauri as strings).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("unsupported file type: {0}")]
    Unsupported(String),
    #[error("cannot read document: {0}")]
    Docx(#[from] bo_docx::DocxError),
    #[error("layout failed: {0}")]
    Layout(#[from] bo_layout::LayoutError),
    #[error("no document is open")]
    NoDocument,
    #[error("page {0} out of range")]
    PageOutOfRange(usize),
    #[error("PDF export failed: {0}")]
    Pdf(#[from] bo_pdf::PdfError),
    #[error("rendering failed: {0}")]
    Raster(#[from] bo_raster::RasterError),
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
    doc: Option<Document>,
    layout: Option<LayoutDocument>,
    display: Option<DisplayList>,
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
        let doc = if bytes.starts_with(b"PK\x03\x04") {
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
        self.doc = Some(doc);
        self.layout = Some(layout);
        self.display = Some(display);
        self.info()
    }

    /// Summary of the open document.
    pub fn info(&self) -> Result<DocInfo> {
        let display = self.display.as_ref().ok_or(EngineError::NoDocument)?;
        let doc = self.doc.as_ref().ok_or(EngineError::NoDocument)?;
        Ok(DocInfo {
            name: self.name.clone(),
            page_count: display.pages.len(),
            pages: display
                .pages
                .iter()
                .map(|p| PageSize {
                    width: p.width,
                    height: p.height,
                })
                .collect(),
            word_count: doc.plain_text().split_whitespace().count(),
        })
    }

    /// The open document model.
    pub fn document(&self) -> Option<&Document> {
        self.doc.as_ref()
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
        Ok(bo_pdf::write_pdf(
            display,
            &self.fonts,
            &bo_pdf::PdfMeta { title: Some(title) },
        )?)
    }

    /// Render one page to PNG at `scale` pixels per point.
    pub fn render_png(&self, index: usize, scale: f32) -> Result<Vec<u8>> {
        Ok(bo_raster::render_page_png(
            self.page(index)?,
            &self.fonts,
            scale,
        )?)
    }
}

/// Build a document from plain text: one paragraph per line, default page setup.
pub fn plain_text_document(text: &str) -> Document {
    let blocks = text
        .lines()
        .map(|l| {
            Block::Paragraph(Paragraph {
                props: bo_model::ParaProps::default(),
                runs: if l.is_empty() {
                    vec![]
                } else {
                    vec![Run::text(l, RunProps::default())]
                },
            })
        })
        .collect();
    Document {
        sections: vec![Section {
            page: bo_model::PageSetup::default(),
            blocks,
        }],
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(bo_model::HalfPoints(26)),
            ..Default::default()
        },
        ..Default::default()
    }
}
