//! Reuse slides (SLIDE-ALBUM-001): another presentation is read next to the
//! open deck, its slides are listed with thumbnails, and the chosen ones are
//! inserted after the current slide by the `reuseSlides` command — the same
//! import as pasting slides copied from another deck (layouts matched or
//! imported with their master and theme, ECMA-376 Part 1, §19.3.1.39 sldLayout,
//! §19.3.1.42 sldMaster, §20.1.6.9 theme), without going through the clipboard.
//!
//! * `{"cmd":"reuseOpen","name","base64","width"}` (a query) reads the file
//!   (`.pptx`, or `.odp` / `.ppt` converted) and answers
//!   `{ reuse: { name, slides: [{ title, png }] } }`.
//! * `{"cmd":"reuseClose"}` forgets it.
//! * `reuseSlides { indices, keepSource }` is a [`super::SlideCommand`]: one
//!   undoable step, shared like any other edit while collaborating.

use super::clip;
use bo_model::slide::{Presentation, Slide};
use serde::Serialize;
use std::collections::BTreeMap;

/// Most slides of a source deck that get a thumbnail.
pub const MAX_THUMBNAILS: usize = 500;
/// Default thumbnail width in pixels.
const DEFAULT_WIDTH: f32 = 192.0;

/// One slide of the source deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReuseSlide {
    /// Title text (empty without a title placeholder).
    pub title: String,
    /// Thumbnail, PNG in base64 (absent past [`MAX_THUMBNAILS`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub png: Option<String>,
}

/// The slides `indices` of `src` (in that order, unknown indices skipped) and
/// what they need from their deck.
pub(crate) fn pick(src: &Presentation, indices: &[usize]) -> (Vec<Slide>, clip::SlideSource) {
    let idx: Vec<usize> = indices
        .iter()
        .copied()
        .filter(|&i| i < src.slides.len())
        .collect();
    let slides = idx.iter().map(|&i| src.slides[i].clone()).collect();
    (slides, clip::slide_source(src, &idx))
}

impl crate::Engine {
    /// `reuseOpen` / `reuseClose` (queries, see the module docs); `None` for
    /// other commands.
    pub(crate) fn reuse_json(&mut self, json: &str) -> Option<crate::Result<String>> {
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        match v.get("cmd")?.as_str()? {
            "reuseClose" => {
                self.slide_reuse = None;
                Some(Ok(serde_json::json!({ "reuse": null }).to_string()))
            }
            "reuseOpen" => Some(self.reuse_open(&v)),
            _ => None,
        }
    }

    fn reuse_open(&mut self, v: &serde_json::Value) -> crate::Result<String> {
        use base64::Engine as _;
        let b64 = v.get("base64").and_then(|b| b.as_str()).unwrap_or("");
        let bytes = crate::merge::base64_decode(b64)
            .ok_or_else(|| crate::EngineError::BadCommand("base64".into()))?;
        let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let width = v
            .get("width")
            .and_then(serde_json::Value::as_f64)
            .map_or(DEFAULT_WIDTH, |w| (w as f32).clamp(32.0, 1024.0));
        let src = match crate::slide_compare::read_deck(&bytes) {
            Ok(p) => p,
            Err(_) => {
                self.slide_reuse = None;
                return Ok(serde_json::json!({ "error": "reuseRead" }).to_string());
            }
        };
        let mut images = BTreeMap::new();
        let mut slides = Vec::with_capacity(src.slides.len());
        for (i, s) in src.slides.iter().enumerate() {
            let png = if i < MAX_THUMBNAILS {
                bo_pptx::render_thumbnail(&src, i, &mut self.fonts, &mut images, width)
                    .ok()
                    .and_then(|page| {
                        bo_raster::render_page_png_with(&page, &self.fonts, 1.0, &images).ok()
                    })
                    .map(|png| base64::engine::general_purpose::STANDARD.encode(png))
            } else {
                None
            };
            slides.push(ReuseSlide {
                title: super::outline::slide_outline(s).title,
                png,
            });
        }
        self.slide_reuse = Some(Box::new(src));
        Ok(serde_json::json!({ "reuse": { "name": name, "slides": slides } }).to_string())
    }
}
