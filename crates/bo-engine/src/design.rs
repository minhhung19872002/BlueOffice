//! Design tab commands for BlueDoc: page colour (ECMA-376 Part 1, §17.2.1
//! background), page borders (§17.6.10 pgBorders, art borders drawn with
//! BlueOffice's own motifs), text and picture watermarks (VML shapes in the
//! headers, Part 4 §19.1.2.23 textpath / §19.1.2.11 imagedata), document themes and
//! style sets (see `bo_edit::design`).
//!
//! Watermarks are sized here because the engine owns the fonts: a text watermark's
//! box is the advance of its text by the font's ascender-to-descender height, scaled
//! to the text area (diagonally, the rotated box fits both its width and height).

use crate::edit::{Command, EditResult};
use crate::{Engine, EngineError, Result};
use bo_edit::design::{picture_art, text_art, Watermark, WatermarkInfo, STYLE_SETS, THEMES};
use bo_model::{Color, Emu, PageBorderDisplay};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A watermark requested by the host: text (`text`) or picture (`picture`, base64).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkSpec {
    /// Text of a text watermark.
    #[serde(default)]
    pub text: Option<String>,
    /// Font family of the text.
    #[serde(default = "default_font")]
    pub font: String,
    /// Font size in points; `None` = automatic (fills the text area).
    #[serde(default)]
    pub size: Option<f32>,
    /// `RRGGBB` text colour (default light grey).
    #[serde(default)]
    pub color: Option<String>,
    /// Semi-transparent text (fill opacity 50 %).
    #[serde(default = "yes")]
    pub semitransparent: bool,
    /// Diagonal (rising 45°) rather than horizontal.
    #[serde(default = "yes")]
    pub diagonal: bool,
    /// Bold text.
    #[serde(default)]
    pub bold: bool,
    /// Italic text.
    #[serde(default)]
    pub italic: bool,
    /// Picture bytes (PNG/JPEG…) in base64.
    #[serde(default)]
    pub picture: Option<String>,
    /// Scale of the picture in percent; `None` = automatic (fits the text width).
    #[serde(default)]
    pub scale: Option<f32>,
    /// Wash the picture out.
    #[serde(default = "yes")]
    pub washout: bool,
}

fn default_font() -> String {
    "Calibri".into()
}

fn yes() -> bool {
    true
}

/// Page borders as shown in the dialog (first visible edge, sides drawn).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BordersState {
    /// `ST_Border` style.
    pub style: String,
    /// Width (eighths of a point, points for art).
    pub size: u32,
    /// `RRGGBB` or `None` (automatic).
    pub color: Option<String>,
    /// Distance in points.
    pub space: u32,
    /// Top, left, bottom, right drawn.
    pub sides: [bool; 4],
    /// Measured from the page edge.
    pub offset_from_page: bool,
    /// Pages showing the borders.
    pub display: PageBorderDisplay,
    /// Behind the text.
    pub behind: bool,
    /// With a shadow.
    pub shadow: bool,
    /// The style is an art border.
    pub art: bool,
}

/// A theme of the gallery.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeEntry {
    /// Id sent back with `applyTheme`.
    pub id: String,
    /// dk1, lt1, dk2, lt2, accent1–6, hlink, folHlink.
    pub colors: Vec<String>,
    /// Heading font.
    pub major: String,
    /// Body font.
    pub minor: String,
}

/// Design tab state.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignState {
    /// Page colour `RRGGBB`.
    pub page_color: Option<String>,
    /// Page borders of the caret's section.
    pub borders: Option<BordersState>,
    /// The document's watermark.
    pub watermark: Option<WatermarkInfo>,
    /// Theme applied (BlueOffice theme id).
    pub theme: Option<String>,
    /// Style set applied.
    pub style_set: Option<String>,
    /// Theme gallery.
    pub themes: Vec<ThemeEntry>,
    /// Style set ids.
    pub style_sets: Vec<String>,
    /// Art border styles with a BlueOffice motif (`ST_Border` values).
    pub art_styles: Vec<String>,
}

impl Engine {
    /// The Design tab state of the open document.
    pub(crate) fn design_state(&self) -> Option<DesignState> {
        let s = self.session.as_ref()?;
        let d = &s.doc().design;
        let borders = s.page_borders().and_then(|pb| {
            let sides = pb.edges().map(|e| e.is_some_and(|b| b.line.is_visible()));
            let first = pb
                .edges()
                .into_iter()
                .flatten()
                .find(|b| b.line.is_visible())?;
            Some(BordersState {
                style: first.line.style.clone(),
                size: first.line.size,
                color: first.line.color.map(Color::to_hex),
                space: first.line.space,
                sides,
                offset_from_page: pb.offset_from_page,
                display: pb.display,
                behind: pb.behind,
                shadow: first.shadow,
                art: first.line.is_art(),
            })
        });
        Some(DesignState {
            page_color: d.background.map(Color::to_hex),
            borders,
            watermark: s.watermark(),
            theme: d.theme_id.clone(),
            style_set: d.style_set.clone(),
            themes: THEMES
                .iter()
                .map(|t| ThemeEntry {
                    id: t.id.to_owned(),
                    colors: t.colors.iter().map(|c| (*c).to_owned()).collect(),
                    major: t.major.to_owned(),
                    minor: t.minor.to_owned(),
                })
                .collect(),
            style_sets: STYLE_SETS.iter().map(|s| s.id.to_owned()).collect(),
            art_styles: bo_layout::ArtMotif::ALL
                .iter()
                .map(|m| m.style().to_owned())
                .collect(),
        })
    }

    /// Size and content of a requested watermark.
    fn build_watermark(&mut self, spec: &WatermarkSpec) -> Result<Watermark> {
        let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        let sec = s.doc().sections.first().cloned().unwrap_or_default();
        let p = &sec.page;
        let cw = (p.width.to_pt() - p.margin_left.to_pt() - p.margin_right.to_pt()).max(36.0);
        let ch = (p.height.to_pt() - p.margin_top.to_pt().abs() - p.margin_bottom.to_pt().abs())
            .max(36.0);
        if let Some(b64) = &spec.picture {
            let bytes = crate::merge::base64_decode(b64)
                .ok_or_else(|| EngineError::BadCommand("watermark picture".into()))?;
            let bytes = if crate::edit::image_size(&bytes).is_none() {
                bo_raster::transcode_to_png(&bytes).unwrap_or(bytes)
            } else {
                bytes
            };
            let (pw, ph) = crate::edit::image_size(&bytes)
                .ok_or_else(|| EngineError::Unsupported("watermark picture".into()))?;
            let ext = match bo_render::image::sniff(&bytes) {
                bo_render::image::Format::Jpeg => "jpeg",
                _ => "png",
            };
            // 96 dpi; automatic scale fits the text width.
            let (w0, h0) = (pw as f32 * 0.75, ph as f32 * 0.75);
            let k = match spec.scale {
                Some(pct) => pct.clamp(1.0, 500.0) / 100.0,
                None => (cw / w0).min(ch / h0),
            };
            let doc = s.doc();
            let mut n = doc.media.len() + 1;
            let name = loop {
                let cand = format!("/word/media/watermark{n}.{ext}");
                if !doc.media.contains_key(&cand) {
                    break cand;
                }
                n += 1;
            };
            return Ok(Watermark {
                art: picture_art(&name, spec.washout),
                fill: None,
                width: Emu::from_pt(w0 * k),
                height: Emu::from_pt(h0 * k),
                rotation: 0,
                media: Some((name, Arc::new(bytes))),
            });
        }
        let text = spec
            .text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| EngineError::BadCommand("watermark text".into()))?;
        let face = self
            .fonts
            .resolve(&spec.font, spec.bold, spec.italic)
            .or_else(|| self.fonts.resolve("Calibri", spec.bold, spec.italic))
            .ok_or(EngineError::BadCommand("no fonts".into()))?;
        let adv = self.fonts.measure(face, text, 1.0).max(0.01);
        let info = self.fonts.info(face);
        let lh = (f32::from(info.ascender) - f32::from(info.descender))
            / f32::from(info.units_per_em.max(1));
        let size = match spec.size {
            Some(sz) => sz.clamp(4.0, 400.0),
            // Diagonal at 45°: the rotated box spans (w + h)/√2 both ways.
            None if spec.diagonal => cw.min(ch) * std::f32::consts::SQRT_2 / (adv + lh),
            None => cw / adv,
        };
        let fill = spec
            .color
            .as_deref()
            .and_then(Color::from_hex)
            .unwrap_or(Color {
                r: 192,
                g: 192,
                b: 192,
            });
        Ok(Watermark {
            art: text_art(
                text,
                &spec.font,
                spec.bold,
                spec.italic,
                spec.semitransparent,
            ),
            fill: Some(fill),
            width: Emu::from_pt(adv * size),
            height: Emu::from_pt(lh * size),
            rotation: if spec.diagonal { 315 * 60_000 } else { 0 },
            media: None,
        })
    }

    /// Design tab commands (`None`: not one).
    pub(crate) fn design_command(&mut self, cmd: &Command) -> Result<Option<EditResult>> {
        let edit = matches!(
            cmd,
            Command::SetPageColor { .. }
                | Command::SetPageBorders { .. }
                | Command::SetWatermark { .. }
                | Command::ApplyTheme { .. }
                | Command::ApplyStyleSet { .. }
        );
        if !edit && !matches!(cmd, Command::Design) {
            return Ok(None);
        }
        let mut error = None;
        let mut dirty = Vec::new();
        if edit {
            // Header/footer, note and text box editors mirror into the document:
            // close them before changing it.
            self.close_header_footer()?;
            self.close_note()?;
            self.close_text_box()?;
            let wm = match cmd {
                Command::SetWatermark {
                    watermark: Some(spec),
                } => match self.build_watermark(spec) {
                    Ok(w) => Some(w),
                    Err(e) => {
                        error = Some(e.to_string());
                        None
                    }
                },
                _ => None,
            };
            let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
            let res = match cmd {
                Command::SetPageColor { color } => {
                    s.set_page_color(color.as_deref().and_then(Color::from_hex))
                }
                Command::SetPageBorders { borders, scope } => {
                    s.set_page_borders(borders.as_ref(), *scope)
                }
                Command::SetWatermark { watermark } => {
                    if watermark.is_some() && wm.is_none() {
                        Ok(())
                    } else {
                        s.set_watermark(wm.as_ref())
                    }
                }
                Command::ApplyTheme { id } => s.apply_theme(id),
                Command::ApplyStyleSet { id } => s.apply_style_set(id),
                _ => Ok(()),
            };
            if let Err(e) = res {
                error = Some(e.to_string());
            }
            dirty = self.relayout()?;
        }
        let mut r = self.edit_result(dirty, (None, None, None), error)?;
        r.design = self.design_state().map(Box::new);
        if edit {
            r.collab = self.collab_after_edit();
        }
        Ok(Some(r))
    }
}
