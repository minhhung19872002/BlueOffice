//! A new, empty deck for the photo album (SLIDE-ALBUM-001): the album's
//! pictures are then inserted by the ordinary slide commands.
//!
//! `{"cmd":"newDeck","title"}` (a query, also without an open document) answers
//! `{ file }`: a `.pptx` (base64) with one title slide made from the title
//! layout (ECMA-376 Part 1, §19.3.1.39 sldLayout, §19.7.15 ST_SlideLayoutType
//! `title`), its placeholders empty.

use super::build;
use bo_pptx::build::{DeckSpec, LayoutKind, SlideSpec};

/// Widescreen slide size in EMU (§19.2.1.39 sldSz): 13.333 in × 7.5 in.
const WIDE: (i64, i64) = (12_192_000, 6_858_000);

/// A deck with one empty title slide; `title` becomes `dc:title`.
pub fn new_deck(title: &str) -> crate::Result<Vec<u8>> {
    let spec = DeckSpec {
        width: WIDE.0,
        height: WIDE.1,
        slides: vec![SlideSpec {
            layout: LayoutKind::Title,
            ..SlideSpec::default()
        }],
        title: Some(title.to_owned()).filter(|t| !t.is_empty()),
    };
    let bytes = bo_pptx::build::build_pptx(&spec)
        .map_err(|e| crate::EngineError::Format(e.to_string()))?;
    let mut pres = bo_pptx::read_pptx(&bytes)?;
    // The title slide gets the placeholders of its layout.
    if let Some(first) = pres.slides.first().cloned() {
        let mut slide = build::slide_from_layout(&pres, first.layout);
        slide.info = first.info;
        slide.slide_id = first.slide_id;
        slide.pres_rel_id = first.pres_rel_id;
        pres.slides[0] = slide;
    }
    Ok(bo_pptx::write_pptx(&pres)?)
}

impl crate::Engine {
    /// `newDeck` (see the module docs); `None` for other commands.
    pub(crate) fn new_deck_json(json: &str) -> Option<crate::Result<String>> {
        use base64::Engine as _;
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        if v.get("cmd")?.as_str()? != "newDeck" {
            return None;
        }
        let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("");
        Some(new_deck(title).map(|b| {
            serde_json::json!({ "file": base64::engine::general_purpose::STANDARD.encode(b) })
                .to_string()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // source: ECMA-376 Part 1, §19.3.1.36 ph (ctrTitle / subTitle of the title layout).
    #[test]
    fn new_deck_has_one_title_slide_with_empty_placeholders() {
        let bytes = new_deck("Kỳ nghỉ hè").expect("deck");
        let p = bo_pptx::read_pptx(&bytes).expect("read");
        assert_eq!(p.slides.len(), 1);
        assert_eq!((p.slide_size.cx, p.slide_size.cy), WIDE);
        let s = &p.slides[0];
        assert_eq!(p.layouts[s.layout].kind.as_deref(), Some("title"));
        let o = super::super::outline::slide_outline(s);
        assert!(o.has_title && o.has_body);
        assert_eq!(o.title, "");
        // It opens and renders like any deck.
        let mut e = crate::Engine::with_bundled_fonts();
        e.open(&bytes, "album.pptx").expect("open");
        assert_eq!(e.slide_count(), 1);
        let r = crate::Engine::new_deck_json(r#"{"cmd":"newDeck","title":"A"}"#)
            .expect("handled")
            .expect("ok");
        assert!(r.contains("\"file\""));
        assert!(crate::Engine::new_deck_json(r#"{"cmd":"state"}"#).is_none());
    }
}
