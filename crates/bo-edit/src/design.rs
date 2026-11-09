//! Design tab: page colour (ECMA-376 Part 1, §17.2.1 background with §17.15.1.32
//! displayBackgroundShape), page borders (§17.6.10 pgBorders), watermarks (a VML
//! text-path or picture shape behind the text in the headers, Part 4 §19.1),
//! document themes (DrawingML §20.1.6.9 theme with BlueOffice's own palettes and
//! font pairs) and paragraph-spacing style sets. Every change is one undo step.

use crate::{EditError, Op, Result, Session, SetupScope};
use bo_model::{
    Anchor, Block, Border, Color, DocDesign, Emu, Image, LineSpacing, PageBorder,
    PageBorderDisplay, PageBorders, ParaProps, Paragraph, Run, RunContent, RunProps, Section,
    Shape, ShapeArt, Style, StyleKind, StyleSheet, TextPath, ThemeFonts, Twips, Wrap,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::sync::Arc;

/// Page borders sent by hosts: one line or art style on the chosen edges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageBordersSpec {
    /// §17.18.2 `ST_Border` value: a line style (`single`, `double`, `dotted`, …)
    /// or an art border (`basicBlackDots`, …).
    pub style: String,
    /// Width: eighths of a point for line styles, points (1–31) for art borders.
    pub size: u32,
    /// `RRGGBB`; absent = automatic.
    #[serde(default)]
    pub color: Option<String>,
    /// Distance from the text or the page edge, in points (0–31).
    #[serde(default)]
    pub space: u32,
    /// Edges drawn: top, left, bottom, right (default all four).
    #[serde(default = "all_sides")]
    pub sides: [bool; 4],
    /// Measure `space` from the page edge (§17.18.64 `page`) instead of the text.
    #[serde(default)]
    pub offset_from_page: bool,
    /// Pages showing the borders.
    #[serde(default)]
    pub display: PageBorderDisplay,
    /// Draw behind the text (§17.18.65 `back`).
    #[serde(default)]
    pub behind: bool,
    /// Shadow on the edges (`w:shadow`).
    #[serde(default)]
    pub shadow: bool,
}

fn all_sides() -> [bool; 4] {
    [true; 4]
}

impl PageBordersSpec {
    /// The model page borders (spacing clamped to the schema's 0–31 points).
    pub fn to_model(&self) -> PageBorders {
        let line = Border {
            style: self.style.clone(),
            size: self
                .size
                .clamp(1, if is_art(&self.style) { 31 } else { 96 }),
            color: self.color.as_deref().and_then(Color::from_hex),
            space: self.space.min(31),
        };
        let edge = |on: bool| {
            on.then(|| PageBorder {
                line: line.clone(),
                shadow: self.shadow,
                frame: false,
            })
        };
        PageBorders {
            top: edge(self.sides[0]),
            left: edge(self.sides[1]),
            bottom: edge(self.sides[2]),
            right: edge(self.sides[3]),
            offset_from_page: self.offset_from_page,
            display: self.display,
            behind: self.behind,
        }
    }
}

fn is_art(style: &str) -> bool {
    Border {
        style: style.to_owned(),
        size: 1,
        color: None,
        space: 0,
    }
    .is_art()
}

/// A watermark ready to insert: what it draws and its box (sized by the host from
/// font metrics or the picture's pixel size).
#[derive(Debug, Clone, PartialEq)]
pub struct Watermark {
    /// Text path, or picture with washout.
    pub art: ShapeArt,
    /// Fill of the text (ignored for pictures).
    pub fill: Option<Color>,
    /// Box width.
    pub width: Emu,
    /// Box height.
    pub height: Emu,
    /// Rotation in 60 000ths of a degree (315° = diagonal rising to the right).
    pub rotation: i32,
    /// New picture bytes to add as a media part (part name, bytes).
    pub media: Option<(String, Arc<Vec<u8>>)>,
}

/// Watermark found in the document (for the Design tab).
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkInfo {
    /// Text of a text watermark.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Font of a text watermark.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// `RRGGBB` fill of a text watermark.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// A picture watermark.
    pub picture: bool,
    /// The picture is washed out.
    pub washout: bool,
    /// Opacity below 1 (semi-transparent text).
    pub semitransparent: bool,
    /// Rotated (diagonal) rather than horizontal.
    pub diagonal: bool,
}

/// A BlueOffice document theme: palette and font pair of its own design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeDef {
    /// Id used by hosts.
    pub id: &'static str,
    /// Name written in the theme part (`a:theme/@name`).
    pub name: &'static str,
    /// dk1, lt1, dk2, lt2, accent1–6, hlink, folHlink (`RRGGBB`).
    pub colors: [&'static str; 12],
    /// Heading (major) typeface.
    pub major: &'static str,
    /// Body (minor) typeface.
    pub minor: &'static str,
}

/// The BlueOffice themes (own names, palettes and font pairs).
pub const THEMES: &[ThemeDef] = &[
    ThemeDef {
        id: "ocean",
        name: "BlueOffice Ocean",
        colors: [
            "000000", "FFFFFF", "1B3A5C", "E7EEF5", "1F6FB2", "2BA3A0", "F2A541", "D9534F",
            "6C5CA8", "7A8B99", "1F6FB2", "6C5CA8",
        ],
        major: "Cambria",
        minor: "Calibri",
    },
    ThemeDef {
        id: "bamboo",
        name: "BlueOffice Bamboo",
        colors: [
            "000000", "FFFFFF", "23412B", "EEF3E8", "3C7D3A", "8DB03F", "C9A227", "7B5E3B",
            "2F7F79", "A04E3A", "2F7F79", "7B5E3B",
        ],
        major: "Times New Roman",
        minor: "Times New Roman",
    },
    ThemeDef {
        id: "sunset",
        name: "BlueOffice Sunset",
        colors: [
            "000000", "FFFFFF", "4A2B2B", "FBF0E6", "D9603B", "F2994A", "E0B33A", "A23B5C",
            "6E4B8B", "3B7EA1", "3B7EA1", "6E4B8B",
        ],
        major: "Arial",
        minor: "Arial",
    },
    ThemeDef {
        id: "slate",
        name: "BlueOffice Slate",
        colors: [
            "000000", "FFFFFF", "2F3A44", "ECEFF1", "4F6D7A", "9AA5AE", "C5A880", "6B8F71",
            "A86B6B", "56748F", "4F6D7A", "A86B6B",
        ],
        major: "Calibri",
        minor: "Calibri",
    },
];

/// A paragraph-spacing style set: Normal's space after and line spacing, and the
/// space before Heading 1 (twips; line in 240ths).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleSetDef {
    /// Id used by hosts.
    pub id: &'static str,
    /// Space after a Normal paragraph (twips).
    pub after: i32,
    /// Line spacing of Normal (240ths of a line).
    pub line: u32,
    /// Space before Heading 1 (twips).
    pub heading_before: i32,
}

/// The BlueOffice style sets, from tight to loose.
pub const STYLE_SETS: &[StyleSetDef] = &[
    StyleSetDef {
        id: "compact",
        after: 0,
        line: 240,
        heading_before: 240,
    },
    StyleSetDef {
        id: "balanced",
        after: 120,
        line: 276,
        heading_before: 360,
    },
    StyleSetDef {
        id: "airy",
        after: 200,
        line: 360,
        heading_before: 480,
    },
    StyleSetDef {
        id: "double",
        after: 240,
        line: 480,
        heading_before: 480,
    },
];

/// Theme part (DrawingML §20.1.6.9 theme: §20.1.4.1.9 clrScheme, §20.1.4.1.18
/// fontScheme, §20.1.4.1.14 fmtScheme) of a BlueOffice theme.
pub fn theme_xml(t: &ThemeDef) -> String {
    const NAMES: [&str; 12] = [
        "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5",
        "accent6", "hlink", "folHlink",
    ];
    let mut clr = String::new();
    for (n, c) in NAMES.iter().zip(t.colors) {
        // §20.1.2.3.32 srgbClr for every slot (dk1/lt1 included).
        let _ = write!(clr, "<a:{n}><a:srgbClr val=\"{c}\"/></a:{n}>");
    }
    let font = |tag: &str, face: &str| {
        format!("<a:{tag}><a:latin typeface=\"{face}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:{tag}>")
    };
    // §20.1.4.1.14 fmtScheme: three fill, line, effect and background fill styles.
    let fills = "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>".repeat(3);
    let mut lines = String::new();
    for w in [6350, 12700, 19050] {
        let _ = write!(
            lines,
            "<a:ln w=\"{w}\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>"
        );
    }
    let effects = "<a:effectStyle><a:effectLst/></a:effectStyle>".repeat(3);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"{name}\"><a:themeElements><a:clrScheme name=\"{name}\">{clr}</a:clrScheme><a:fontScheme name=\"{name}\">{major}{minor}</a:fontScheme><a:fmtScheme name=\"{name}\"><a:fillStyleLst>{fills}</a:fillStyleLst><a:lnStyleLst>{lines}</a:lnStyleLst><a:effectStyleLst>{effects}</a:effectStyleLst><a:bgFillStyleLst>{fills}</a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>",
        name = t.name,
        major = font("majorFont", t.major),
        minor = font("minorFont", t.minor),
    )
}

/// Heading-like paragraph styles recoloured by a theme (accent 1; the title in dk2).
const HEADINGS: &[&str] = &[
    "Title", "Subtitle", "Heading1", "Heading2", "Heading3", "Heading4", "Heading5", "Heading6",
    "Heading7", "Heading8", "Heading9",
];

fn set_theme_font(r: &mut RunProps, slot: &str) {
    r.font = None;
    r.font_h_ansi = None;
    r.theme_ascii = Some(slot.to_owned());
    r.theme_h_ansi = Some(slot.to_owned());
}

/// `sheet` with the fonts and colours of theme `t`: headings in the major font and
/// accent colour, Normal in the minor font.
fn themed_styles(sheet: &StyleSheet, t: &ThemeDef) -> StyleSheet {
    let mut out = sheet.clone();
    let accent = Color::from_hex(t.colors[4]);
    let dk2 = Color::from_hex(t.colors[2]);
    let linked: Vec<(String, Option<String>)> = out
        .styles
        .iter()
        .map(|s| (s.id.clone(), s.linked.clone()))
        .collect();
    let link_of = |id: &str| {
        linked
            .iter()
            .find(|(i, _)| i == id)
            .and_then(|(_, l)| l.clone())
    };
    for s in &mut out.styles {
        let heading = HEADINGS.contains(&s.id.as_str())
            || HEADINGS
                .iter()
                .any(|h| link_of(h).as_deref() == Some(s.id.as_str()));
        if heading && matches!(s.kind, StyleKind::Paragraph | StyleKind::Character) {
            set_theme_font(&mut s.rpr, "majorHAnsi");
            let title = s.id.starts_with("Title");
            s.rpr.color = if title { dk2 } else { accent };
        } else if s.kind == StyleKind::Paragraph && s.is_default {
            set_theme_font(&mut s.rpr, "minorHAnsi");
        }
    }
    out
}

/// `sheet` with the spacing of style set `set` (Normal created when missing).
fn spaced_styles(sheet: &StyleSheet, set: &StyleSetDef) -> StyleSheet {
    let mut out = sheet.clone();
    let found = out
        .styles
        .iter()
        .position(|s| s.kind == StyleKind::Paragraph && s.is_default);
    let normal = found.unwrap_or_else(|| {
        let mut n = Style::new("Normal", "Normal", StyleKind::Paragraph);
        n.is_default = true;
        out.styles.insert(0, n);
        0
    });
    let p = &mut out.styles[normal].ppr;
    p.space_after = Some(Twips(set.after));
    p.line_spacing = Some(LineSpacing::Auto(set.line));
    if let Some(h) = out.styles.iter_mut().find(|s| s.id == "Heading1") {
        h.ppr.space_before = Some(Twips(set.heading_before));
    }
    out
}

/// Is `img` a watermark shape?
fn is_watermark(img: &Image) -> bool {
    img.shape
        .as_ref()
        .and_then(|s| s.art.as_ref())
        .is_some_and(|a| a.watermark)
}

/// `blocks` without watermark shapes (runs left empty are dropped); `None` when
/// there was none.
fn strip_watermarks(blocks: &[Block]) -> Option<Vec<Block>> {
    let mut out = blocks.to_vec();
    let mut found = false;
    for b in &mut out {
        if let Block::Paragraph(p) = b {
            p.runs = std::mem::take(&mut p.runs)
                .into_iter()
                .filter_map(|mut r| {
                    let before = r.content.len();
                    r.content
                        .retain(|c| !matches!(c, RunContent::Image(i) if is_watermark(i)));
                    if r.content.len() != before {
                        found = true;
                        // A run that held only the watermark goes too.
                        if r.content.is_empty() {
                            return None;
                        }
                    }
                    Some(r)
                })
                .collect();
        }
    }
    found.then_some(out)
}

/// The watermark shapes of `blocks`.
fn watermarks_in(blocks: &[Block]) -> impl Iterator<Item = &Image> {
    blocks.iter().flat_map(|b| {
        b.as_paragraph()
            .into_iter()
            .flat_map(|p| p.runs.iter())
            .flat_map(|r| r.content.iter())
            .filter_map(|c| match c {
                RunContent::Image(i) if is_watermark(i) => Some(&**i),
                _ => None,
            })
    })
}

/// `blocks` with the watermark run at the start of the first paragraph.
fn with_watermark(blocks: &[Block], run: &Run, empty_para: &Paragraph) -> Vec<Block> {
    let mut out = blocks.to_vec();
    if let Some(Block::Paragraph(p)) = out.first_mut() {
        p.runs.insert(0, run.clone());
    } else {
        let mut p = empty_para.clone();
        p.runs.insert(0, run.clone());
        out.insert(0, Block::Paragraph(p));
    }
    out
}

impl Session {
    /// Set or remove the page colour (§17.2.1 background); a colour also turns on
    /// §17.15.1.32 displayBackgroundShape so other applications show it.
    pub fn set_page_color(&mut self, color: Option<Color>) -> Result<()> {
        let old = (self.doc.design.clone(), self.doc.theme_fonts.clone());
        let mut new = old.clone();
        new.0.background = color;
        if color.is_some() {
            new.0.display_background_shape = true;
        }
        self.design_op(old, new)
    }

    /// Apply a theme part read from a theme file (`.thmx`) or another document
    /// (DrawingML §20.1.6.9 theme, written as the document's theme part §14.2.7)
    /// with its font scheme (§20.1.4.1.18) in one undo step (CORE-FMT-001).
    pub fn apply_theme_xml(&mut self, xml: &str, fonts: ThemeFonts) -> Result<()> {
        let old = (self.doc.design.clone(), self.doc.theme_fonts.clone());
        let mut new = old.clone();
        new.0.theme_xml = Some(xml.into());
        new.0.theme_id = None;
        new.1 = fonts;
        self.design_op(old, new)
    }

    /// Apply BlueOffice theme `id` (theme part, theme fonts, heading colours and
    /// fonts) in one undo step.
    pub fn apply_theme(&mut self, id: &str) -> Result<()> {
        let t = THEMES
            .iter()
            .find(|t| t.id == id)
            .ok_or(EditError::BadPath)?;
        let old = (self.doc.design.clone(), self.doc.theme_fonts.clone());
        let mut new = old.clone();
        new.0.theme_xml = Some(theme_xml(t).into());
        new.0.theme_id = Some(t.id.to_owned());
        new.1 = ThemeFonts {
            major_latin: t.major.to_owned(),
            minor_latin: t.minor.to_owned(),
            ..old.1.clone()
        };
        let styles = themed_styles(&self.doc.styles, t);
        self.edit(|tx, sel| {
            tx.push(Op::Design {
                old: Box::new(old),
                new: Box::new(new),
            })?;
            if styles != tx.doc.styles {
                tx.push(Op::Styles {
                    old: Box::new(tx.doc.styles.clone()),
                    new: Box::new(styles),
                })?;
            }
            Ok(sel)
        })?;
        Ok(())
    }

    /// Apply style set `id` (paragraph spacing of Normal and Heading 1).
    pub fn apply_style_set(&mut self, id: &str) -> Result<()> {
        let set = STYLE_SETS
            .iter()
            .find(|s| s.id == id)
            .ok_or(EditError::BadPath)?;
        let styles = spaced_styles(&self.doc.styles, set);
        let old = (self.doc.design.clone(), self.doc.theme_fonts.clone());
        let mut new = old.clone();
        new.0.style_set = Some(set.id.to_owned());
        self.edit(|tx, sel| {
            if styles != tx.doc.styles {
                tx.push(Op::Styles {
                    old: Box::new(tx.doc.styles.clone()),
                    new: Box::new(styles),
                })?;
            }
            if new != old {
                tx.push(Op::Design {
                    old: Box::new(old),
                    new: Box::new(new),
                })?;
            }
            Ok(sel)
        })?;
        Ok(())
    }

    fn design_op(
        &mut self,
        old: (DocDesign, ThemeFonts),
        new: (DocDesign, ThemeFonts),
    ) -> Result<()> {
        if old == new {
            return Ok(());
        }
        self.edit(|tx, sel| {
            tx.push(Op::Design {
                old: Box::new(old),
                new: Box::new(new),
            })?;
            Ok(sel)
        })?;
        Ok(())
    }

    /// Page borders of the caret's section.
    pub fn page_borders(&self) -> Option<&PageBorders> {
        let si = self.section_state().index;
        self.doc.sections.get(si)?.page_borders.as_ref()
    }

    /// Set (`None`: remove) the page borders of the caret's section or of every
    /// section (§17.6.10; one undo step).
    pub fn set_page_borders(
        &mut self,
        spec: Option<&PageBordersSpec>,
        scope: SetupScope,
    ) -> Result<()> {
        let current = self.section_state().index;
        let targets: Vec<usize> = match scope {
            SetupScope::Section => vec![current],
            SetupScope::Document => (0..self.doc.sections.len()).collect(),
        };
        let pb = spec
            .filter(|s| !matches!(s.style.as_str(), "none" | "nil") && s.sides.contains(&true))
            .map(PageBordersSpec::to_model);
        self.edit(|tx, sel| {
            for si in targets {
                let old = tx.doc.sections.get(si).cloned().ok_or(EditError::BadPath)?;
                let mut new = old.clone();
                new.page_borders.clone_from(&pb);
                if new != old {
                    tx.push(Op::Sections {
                        index: si,
                        remove: vec![old],
                        insert: vec![new],
                    })?;
                }
            }
            Ok(sel)
        })?;
        Ok(())
    }

    /// The watermark of the document (the first one found in a header).
    pub fn watermark(&self) -> Option<WatermarkInfo> {
        let img = self.doc.sections.iter().find_map(|s| {
            [&s.headers.default, &s.headers.first, &s.headers.even]
                .into_iter()
                .flatten()
                .find_map(|b| watermarks_in(b).next())
        })?;
        let shape = img.shape.as_ref()?;
        let art = shape.art.as_ref()?;
        Some(WatermarkInfo {
            text: art.text.as_ref().map(|t| t.text.clone()),
            font: art.text.as_ref().map(|t| t.font.clone()),
            color: art.text.as_ref().and(shape.fill).map(Color::to_hex),
            picture: art.picture.is_some(),
            washout: art.washout,
            semitransparent: art.opacity < 1000,
            diagonal: img.rotation != 0,
        })
    }

    /// Replace the watermarks of every header by `wm` (`None`: remove them): the
    /// default header of the first section (created if needed) and every header a
    /// section defines get it, so it shows on every page (one undo step).
    pub fn set_watermark(&mut self, wm: Option<&Watermark>) -> Result<()> {
        let header_style = self.doc.style("Header").map(|_| "Header".to_owned());
        let empty_para = Paragraph::new(
            ParaProps {
                style: header_style,
                ..ParaProps::default()
            },
            Vec::new(),
        );
        let run = wm.map(|w| {
            let mut art = w.art.clone();
            art.watermark = true;
            let img = Image {
                media: String::new(),
                width: w.width,
                height: w.height,
                // Centred on the margins, behind the text, no wrapping.
                anchor: Some(Anchor {
                    h_relative: "margin".into(),
                    v_relative: "margin".into(),
                    h_align: Some("center".into()),
                    v_align: Some("center".into()),
                    wrap: Wrap::None,
                    behind: true,
                    ..Anchor::default()
                }),
                crop: None,
                rotation: w.rotation,
                description: art
                    .text
                    .as_ref()
                    .map_or_else(String::new, |t| t.text.clone()),
                shape: Some(Box::new(Shape {
                    preset: "rect".into(),
                    fill: if art.picture.is_some() { None } else { w.fill },
                    insets: [Emu(0); 4],
                    vml: true,
                    art: Some(Box::new(art)),
                    ..Shape::default()
                })),
                format: None,
                src: None,
            };
            Run {
                props: RunProps::default(),
                content: vec![RunContent::Image(Box::new(img))],
            }
        });
        let media = wm.and_then(|w| w.media.clone());
        self.edit(|tx, sel| {
            if let Some((name, bytes)) = &media {
                if !tx.doc.media.contains_key(name) {
                    tx.push(Op::Media {
                        name: name.clone(),
                        old: None,
                        new: Some(bytes.clone()),
                    })?;
                }
            }
            let n = tx.doc.sections.len();
            for si in 0..n {
                let old = tx.doc.sections[si].clone();
                let mut new: Section = old.clone();
                for set in [
                    &mut new.headers.default,
                    &mut new.headers.first,
                    &mut new.headers.even,
                ] {
                    if let Some(b) = set.as_ref().and_then(|b| strip_watermarks(b)) {
                        *set = Some(b);
                    }
                }
                if let Some(run) = &run {
                    if si == 0 && new.headers.default.is_none() {
                        new.headers.default = Some(Vec::new());
                    }
                    for set in [
                        &mut new.headers.default,
                        &mut new.headers.first,
                        &mut new.headers.even,
                    ] {
                        if let Some(b) = set.as_ref() {
                            *set = Some(with_watermark(b, run, &empty_para));
                        }
                    }
                }
                if new != old {
                    tx.push(Op::Sections {
                        index: si,
                        remove: vec![old],
                        insert: vec![new],
                    })?;
                }
            }
            Ok(sel)
        })?;
        Ok(())
    }
}

/// Text watermark content (Part 4, §19.1.2.23 textpath) for hosts building a
/// [`Watermark`].
pub fn text_art(
    text: &str,
    font: &str,
    bold: bool,
    italic: bool,
    semitransparent: bool,
) -> ShapeArt {
    ShapeArt {
        text: Some(TextPath {
            text: text.to_owned(),
            font: font.to_owned(),
            bold,
            italic,
        }),
        opacity: if semitransparent { 500 } else { 1000 },
        watermark: true,
        ..ShapeArt::default()
    }
}

/// Picture watermark content (§19.1.2.11 imagedata) for media part `name`.
pub fn picture_art(name: &str, washout: bool) -> ShapeArt {
    ShapeArt {
        picture: Some(name.to_owned()),
        washout,
        watermark: true,
        ..ShapeArt::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{Document, HeaderFooter};

    fn para(text: &str) -> Block {
        Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run::text(text, RunProps::default())],
        ))
    }

    fn session(sections: usize) -> Session {
        let mut styles = StyleSheet::default();
        let mut normal = Style::new("Normal", "Normal", StyleKind::Paragraph);
        normal.is_default = true;
        styles.styles.push(normal);
        let mut h1 = Style::new("Heading1", "heading 1", StyleKind::Paragraph);
        h1.based_on = Some("Normal".into());
        h1.rpr.color = Color::from_hex("112233");
        styles.styles.push(h1);
        Session::new(Document {
            sections: (0..sections)
                .map(|i| Section::new(vec![para(&format!("Phần {i}"))]))
                .collect(),
            styles,
            ..Document::default()
        })
    }

    fn text_wm(text: &str) -> Watermark {
        Watermark {
            art: text_art(text, "Calibri", false, false, true),
            fill: Color::from_hex("C0C0C0"),
            width: Emu::from_pt(400.0),
            height: Emu::from_pt(100.0),
            rotation: 315 * 60_000,
            media: None,
        }
    }

    fn count_watermarks(s: &Session) -> usize {
        s.doc()
            .sections
            .iter()
            .flat_map(|sec| [&sec.headers.default, &sec.headers.first, &sec.headers.even])
            .flatten()
            .map(|b| watermarks_in(b).count())
            .sum()
    }

    #[test]
    fn page_colour_undo_redo() {
        let mut s = session(1);
        s.set_page_color(Color::from_hex("FDF6E3")).unwrap();
        assert_eq!(s.doc().design.background, Color::from_hex("FDF6E3"));
        assert!(s.doc().design.display_background_shape);
        assert!(s.undo());
        assert_eq!(s.doc().design.background, None);
        assert!(!s.doc().design.display_background_shape);
        assert!(s.redo());
        assert_eq!(s.doc().design.background, Color::from_hex("FDF6E3"));
        s.set_page_color(None).unwrap();
        assert_eq!(s.doc().design.background, None);
    }

    #[test]
    fn page_borders_scope_and_removal() {
        let mut s = session(2);
        let spec = PageBordersSpec {
            style: "basicBlackDots".into(),
            size: 50,
            color: Some("1F6FB2".into()),
            space: 40,
            sides: [true, false, true, false],
            offset_from_page: true,
            display: PageBorderDisplay::FirstPage,
            behind: true,
            shadow: false,
        };
        s.set_page_borders(Some(&spec), SetupScope::Document)
            .unwrap();
        for sec in &s.doc().sections {
            let pb = sec.page_borders.as_ref().unwrap();
            // Art width clamped to 31 pt, spacing to 31 pt (§17.6.10 ranges).
            let top = pb.top.as_ref().unwrap();
            assert_eq!((top.line.size, top.line.space), (31, 31));
            assert!(pb.left.is_none() && pb.bottom.is_some() && pb.right.is_none());
            assert!(pb.behind && pb.offset_from_page);
        }
        assert!(s.undo());
        assert!(s
            .doc()
            .sections
            .iter()
            .all(|sec| sec.page_borders.is_none()));
        assert!(s.redo());
        // Removing on the caret's section only.
        s.set_page_borders(None, SetupScope::Section).unwrap();
        assert!(s.doc().sections[0].page_borders.is_none());
        assert!(s.doc().sections[1].page_borders.is_some());
    }

    #[test]
    fn watermark_in_every_header_then_removed() {
        let mut s = session(2);
        // Section 1 has its own default and first-page headers.
        s.doc.sections[1].headers = HeaderFooter {
            default: Some(vec![para("Tiêu đề 2")]),
            first: Some(vec![para("Trang đầu")]),
            even: None,
        };
        s.set_watermark(Some(&text_wm("MẬT"))).unwrap();
        // Section 0 gets a default header; section 1 both of its headers.
        assert_eq!(count_watermarks(&s), 3);
        assert_eq!(s.watermark().and_then(|w| w.text).as_deref(), Some("MẬT"));
        // The existing header text stays after the watermark run.
        let h = s.doc().sections[1].headers.default.as_ref().unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(
            h[0].as_paragraph().unwrap().plain_text(),
            "\u{FFFC}Tiêu đề 2"
        );
        // Replacing keeps one watermark per header.
        s.set_watermark(Some(&text_wm("BẢN SAO"))).unwrap();
        assert_eq!(count_watermarks(&s), 3);
        assert_eq!(
            s.watermark().and_then(|w| w.text).as_deref(),
            Some("BẢN SAO")
        );
        s.set_watermark(None).unwrap();
        assert_eq!(count_watermarks(&s), 0);
        assert_eq!(
            s.doc().sections[1].headers.default.as_ref().unwrap()[0]
                .as_paragraph()
                .unwrap()
                .plain_text(),
            "Tiêu đề 2"
        );
        // Three undo steps back to the first watermark.
        assert!(s.undo());
        assert!(s.undo());
        assert_eq!(s.watermark().and_then(|w| w.text).as_deref(), Some("MẬT"));
        assert!(s.undo());
        assert_eq!(count_watermarks(&s), 0);
        assert!(s.doc().sections[0].headers.default.is_none());
    }

    #[test]
    fn picture_watermark_adds_its_media() {
        let mut s = session(1);
        let wm = Watermark {
            art: picture_art("/word/media/watermark1.png", true),
            fill: None,
            width: Emu::from_pt(200.0),
            height: Emu::from_pt(100.0),
            rotation: 0,
            media: Some(("/word/media/watermark1.png".into(), Arc::new(vec![1, 2, 3]))),
        };
        s.set_watermark(Some(&wm)).unwrap();
        assert!(s.doc().media.contains_key("/word/media/watermark1.png"));
        let info = s.watermark().unwrap();
        assert!(info.picture && info.washout);
        assert!(s.undo());
        assert!(!s.doc().media.contains_key("/word/media/watermark1.png"));
    }

    #[test]
    fn theme_and_style_set() {
        let mut s = session(1);
        s.apply_theme("bamboo").unwrap();
        let d = s.doc();
        assert_eq!(d.design.theme_id.as_deref(), Some("bamboo"));
        assert_eq!(d.theme_fonts.major_latin, "Times New Roman");
        let xml = d.design.theme_xml.as_deref().unwrap();
        assert!(xml.contains("name=\"BlueOffice Bamboo\""));
        assert!(xml.contains("<a:accent1><a:srgbClr val=\"3C7D3A\"/></a:accent1>"));
        // Heading 1 takes accent 1 and the major font; Normal the minor font.
        let h1 = d.style("Heading1").unwrap();
        assert_eq!(h1.rpr.color, Color::from_hex("3C7D3A"));
        assert_eq!(h1.rpr.theme_ascii.as_deref(), Some("majorHAnsi"));
        assert_eq!(
            d.style("Normal").unwrap().rpr.theme_ascii.as_deref(),
            Some("minorHAnsi")
        );
        // One undo step restores fonts, colours and the theme part.
        assert!(s.undo());
        assert_eq!(s.doc().design.theme_xml, None);
        assert_eq!(
            s.doc().style("Heading1").unwrap().rpr.color,
            Color::from_hex("112233")
        );
        s.apply_style_set("airy").unwrap();
        let n = s.doc().style("Normal").unwrap();
        assert_eq!(n.ppr.space_after, Some(Twips(200)));
        assert_eq!(n.ppr.line_spacing, Some(LineSpacing::Auto(360)));
        assert_eq!(
            s.doc().style("Heading1").unwrap().ppr.space_before,
            Some(Twips(480))
        );
        assert!(s.apply_theme("nope").is_err());
    }

    #[test]
    fn every_theme_part_parses() {
        for t in THEMES {
            let xml = theme_xml(t);
            let th =
                bo_drawingml::theme::parse_theme(xml.as_bytes(), "/word/theme/theme1.xml").unwrap();
            assert_eq!(th.fonts.major.latin.typeface, t.major);
            assert_eq!(th.fonts.minor.latin.typeface, t.minor);
        }
    }
}
