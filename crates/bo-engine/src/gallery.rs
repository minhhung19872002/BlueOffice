//! Style gallery: the paragraph and character styles of the open document
//! (ECMA-376 Part 1, §17.7.4.17 style) with what a preview needs. Quick styles
//! (§17.7.4.14 qFormat) come first in style-sheet order, then the others by name.
//! A linked paragraph / character pair (§17.7.4.6 link) is listed once, as the
//! paragraph style. The Styles pane filters on `recommended` (§17.7.4.16 semiHidden,
//! §17.7.4.20 unhideWhenUsed, §17.7.4.4 hidden) and `inUse`, ordered by `uiPriority`.

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
    /// Bold (§17.3.2.1 b).
    pub bold: bool,
    /// Italic (§17.3.2.16 i).
    pub italic: bool,
    /// Underlined, any style but `none` (§17.3.2.40 u).
    pub underline: bool,
    /// `RRGGBB`.
    pub color: String,
}

/// One style of the gallery.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleEntry {
    /// Style id (`w:styleId`, §17.7.4.17), used to apply the style.
    pub id: String,
    /// `w:name` (§17.7.4.9), the id when absent.
    pub name: String,
    /// `paragraph` or `character`.
    pub kind: &'static str,
    /// §17.7.4.14 qFormat.
    pub quick: bool,
    /// How the style's name is rendered in the gallery.
    pub preview: StylePreview,
    /// §17.7.4.19 uiPriority.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_priority: Option<u32>,
    /// Shown in the recommended list: not hidden, and not semi-hidden unless it
    /// unhides when used and is used.
    pub recommended: bool,
    /// The document's content uses the style.
    pub in_use: bool,
    /// Linked character style (§17.7.4.6 link) folded into this entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked: Option<String>,
    /// §17.7.4.3 basedOn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    /// The default style of its type (cannot be deleted).
    pub is_default: bool,
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
    let used = bo_edit::styles::styles_in_use(doc);
    for s in &doc.styles.styles {
        let (kind, rp) = match s.kind {
            StyleKind::Paragraph => ("paragraph", doc.styles.resolve_paragraph(&s.id).1),
            StyleKind::Character => ("character", doc.styles.resolve_character(&s.id)),
            _ => continue,
        };
        // The character half of a linked pair is shown with its paragraph style.
        let partner = s
            .linked
            .as_deref()
            .and_then(|l| doc.style(l))
            .filter(|l| l.kind != s.kind && l.linked.as_deref() == Some(s.id.as_str()));
        if s.kind == StyleKind::Character && partner.is_some() {
            continue;
        }
        let in_use = used.contains(&s.id) || partner.is_some_and(|p| used.contains(&p.id));
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
            ui_priority: s.ui_priority,
            recommended: !s.hidden && (!s.semi_hidden || (s.unhide_when_used && in_use)),
            in_use,
            linked: partner.map(|p| p.id.clone()),
            based_on: s.based_on.clone(),
            is_default: s.is_default,
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
                ui_priority: None,
                recommended: true,
                in_use: false,
                linked: None,
                based_on: s.based_on.clone(),
                is_default: s.is_default,
            });
        }
    }
    rest.sort_by_key(|e| e.name.to_lowercase());
    quick.extend(rest);
    quick
}
