//! Style gallery: the paragraph and character styles of the open document
//! (ECMA-376 Part 1, §17.7.4.17 style) with what a preview needs. Quick styles
//! (§17.7.4.14 qFormat) come first in style-sheet order, then the others by name.

use bo_model::{Document, RunProps, StyleKind};
use serde::Serialize;

/// Formatting shown by a gallery preview.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StylePreview {
    /// Font family (theme fonts resolved; the layout substitutes missing families).
    pub font: String,
    /// Size in points.
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// `RRGGBB`.
    pub color: String,
}

/// One style of the gallery.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleEntry {
    pub id: String,
    /// `w:name` (§17.7.4.9), the id when absent.
    pub name: String,
    /// `paragraph` or `character`.
    pub kind: &'static str,
    /// §17.7.4.14 qFormat.
    pub quick: bool,
    pub preview: StylePreview,
}

fn preview(doc: &Document, rp: &RunProps) -> StylePreview {
    let rp = rp.inherit(&doc.default_run);
    let font = rp
        .theme_ascii
        .as_deref()
        .and_then(|t| doc.theme_fonts.resolve(t))
        .or(rp.font.as_deref())
        .unwrap_or("Times New Roman")
        .to_owned();
    StylePreview {
        font,
        size: rp.size.map_or(10.0, bo_model::HalfPoints::to_pt),
        bold: rp.bold.unwrap_or(false),
        italic: rp.italic.unwrap_or(false),
        underline: !matches!(rp.underline, None | Some(bo_model::Underline::None)),
        color: rp.color.unwrap_or(bo_model::Color::BLACK).to_hex(),
    }
}

/// Gallery entries of `doc`.
pub fn styles(doc: &Document) -> Vec<StyleEntry> {
    let mut quick = Vec::new();
    let mut rest = Vec::new();
    for s in &doc.styles.styles {
        let (kind, rp) = match s.kind {
            StyleKind::Paragraph => ("paragraph", doc.styles.resolve_paragraph(&s.id).1),
            StyleKind::Character => ("character", doc.styles.resolve_character(&s.id)),
            _ => continue,
        };
        let e = StyleEntry {
            id: s.id.clone(),
            name: if s.name.is_empty() {
                s.id.clone()
            } else {
                s.name.clone()
            },
            kind,
            quick: s.quick,
            preview: preview(doc, &rp),
        };
        if s.quick {
            quick.push(e);
        } else {
            rest.push(e);
        }
    }
    // Built-in styles BlueDoc creates on first use (`bo_edit::builtin_style`).
    for id in ["Normal", "Title", "Heading1", "Heading2", "Heading3"] {
        if doc.style(id).is_some() {
            continue;
        }
        if let Some(s) = bo_edit::builtin_style(id) {
            quick.push(StyleEntry {
                id: s.id.clone(),
                name: s.name.clone(),
                kind: "paragraph",
                quick: true,
                preview: preview(doc, &s.rpr),
            });
        }
    }
    rest.sort_by_key(|e| e.name.to_lowercase());
    quick.extend(rest);
    quick
}
