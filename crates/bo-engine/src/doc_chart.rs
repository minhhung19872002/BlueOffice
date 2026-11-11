//! Charts in BlueDoc (CORE-CHART-007 / CORE-CHART-004): insert a chart of any type BlueSlide
//! offers (classic `c:` types and the chartex `cx:` layouts) inline or floating at the selection,
//! read the selected one's type, titles, legend and data grid, change them, apply chart styles,
//! colours, quick layouts and templates, and change how text wraps around it.
//!
//! The chart model, the chart part writers and the design presets are BlueSlide's
//! (`slide_edit::chart`, `bo_xlsx::chart_write` / `bo_xlsx::chartex`, `chart_design`): a chart is a
//! `w:drawing` (ECMA-376 Part 1, §17.3.3.9) shown as an [`Image`] whose media is its chart part
//! (§21.2.2.29 chartSpace or [MS-ODRAWXML] 2.24 `cx:chartSpace`), the embedded workbook
//! (§21.2.2.63 externalData) being the media part [`chart_part::workbook_of`] names. Every edit
//! writes both parts and the frame in one undo step, through the document's edit history, so it
//! syncs to collaborators like any other edit. A chart read from a file keeps its parts verbatim
//! until it is edited; an edit then writes new parts beside them.
//!
//! The chart is drawn by the shared chart renderer (`bo_sheet::chart` through BlueSlide's frame
//! renderer) with the document theme's colours: as vector commands in the page display list and as
//! a PNG picture of its media part (exports).

use crate::edit::EditResult;
use crate::slide_edit::chart::{self as sc, ChartData};
use crate::{EngineError, Result};
use bo_model::chart_part;
use bo_model::sheet::Chart;
use bo_model::slide::{
    Color, ColorScheme, CommonSlideData, PartInfo, Presentation, RawPart, Rel, Slide, SlideLayout,
    SlideMaster, SlideSize, Theme,
};
use bo_model::{Anchor, Emu, Image, Wrap};
use bo_text::FontManager;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

const EMU_PT: f64 = 12_700.0;
/// Default chart size: 15 × 9 cm (360 000 EMU per centimetre, DrawingML coordinates).
const DEFAULT_CX: i64 = 15 * 360_000;
const DEFAULT_CY: i64 = 9 * 360_000;

/// Chart commands of BlueDoc (`cmd` = `insertChart` | `setChart` | `chartInfo` | `chartGallery`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum DocChartCommand {
    /// Insert a chart at the selection.
    #[serde(rename_all = "camelCase")]
    InsertChart {
        /// Chart type (BlueSlide's names: `column`, `line`, `waterfall`, …).
        kind: String,
        /// Chart title; absent or empty = none.
        #[serde(default)]
        title: Option<String>,
        /// Category / series grid; absent = the sample grid.
        #[serde(default)]
        data: Option<ChartData>,
        /// Legend `r` | `l` | `t` | `b` | `tr` | `none`; absent = bottom (right for pies).
        #[serde(default)]
        legend: Option<String>,
        /// Chart template to format the new chart with.
        #[serde(default)]
        template: Option<bo_sheet::chart_style::ChartTemplate>,
        /// Titles the template adds.
        #[serde(default)]
        texts: bo_sheet::chart_style::LayoutTexts,
        /// Width in points (default 15 cm).
        #[serde(default)]
        width: Option<f64>,
        /// Height in points (default 9 cm).
        #[serde(default)]
        height: Option<f64>,
        /// Text wrapping (see [`WRAPS`]); absent = inline.
        #[serde(default)]
        wrap: Option<String>,
    },
    /// Change the chart at the selection (absent fields keep their value).
    SetChart {
        /// Type, title, legend, data and design (BlueSlide's `setChart` fields).
        #[serde(flatten)]
        spec: sc::ChartSpec,
        /// Text wrapping (see [`WRAPS`]).
        #[serde(default)]
        wrap: Option<String>,
        /// Width in points.
        #[serde(default)]
        width: Option<f64>,
        /// Height in points.
        #[serde(default)]
        height: Option<f64>,
    },
    /// The chart at the selection.
    ChartInfo,
    /// Chart Design gallery of the chart at the selection (previews `width` × `height` points).
    ChartGallery {
        /// Preview width in points.
        #[serde(default)]
        width: Option<f64>,
        /// Preview height in points.
        #[serde(default)]
        height: Option<f64>,
    },
}

impl DocChartCommand {
    /// Parses a JSON command when it is a BlueDoc chart command.
    pub fn parse(v: &serde_json::Value) -> Option<DocChartCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !matches!(c, "insertChart" | "setChart" | "chartInfo" | "chartGallery") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// Text wrapping choices: in line with text (§20.4.2.8 inline), or floating (§20.4.2.3 anchor)
/// with §20.4.2.17 wrapSquare, §20.4.2.20 wrapTight, wrapTopAndBottom, or wrapNone behind / in
/// front of the text (`behindDoc`).
pub const WRAPS: &[&str] = &[
    "inline",
    "square",
    "tight",
    "topAndBottom",
    "behind",
    "inFront",
];

/// The chart at the selection as the chart tools show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocChart {
    /// BlueSlide's type name (`other` for types the dialog does not offer).
    pub kind: String,
    /// Chart title; `None` = no title.
    pub title: Option<String>,
    /// `r` | `l` | `t` | `b` | `tr`; `None` = no legend.
    pub legend: Option<String>,
    /// Data grid (from the cached values).
    pub data: ChartData,
    /// The chart part has an embedded workbook.
    pub embedded: bool,
    /// Frame width in points.
    pub width: f64,
    /// Frame height in points.
    pub height: f64,
    /// Text wrapping ([`WRAPS`]).
    pub wrap: String,
    /// Chart part name.
    pub part: String,
}

/// The wrapping name of a frame.
fn wrap_of(img: &Image) -> &'static str {
    match &img.anchor {
        None => "inline",
        Some(a) => match a.wrap {
            Wrap::Square => "square",
            Wrap::Tight => "tight",
            Wrap::TopAndBottom => "topAndBottom",
            Wrap::None if a.behind => "behind",
            Wrap::None => "inFront",
        },
    }
}

/// The anchor of a frame wrapped as `wrap`, keeping `old`'s position.
fn anchor_for(wrap: &str, old: Option<&Anchor>) -> std::result::Result<Option<Anchor>, String> {
    if !WRAPS.contains(&wrap) {
        return Err("badChart".into());
    }
    if wrap == "inline" {
        return Ok(None);
    }
    let mut a = old.cloned().unwrap_or_else(|| Anchor {
        h_relative: "column".into(),
        v_relative: "paragraph".into(),
        dist: [Emu(0), Emu(0), Emu(114_300), Emu(114_300)],
        ..Anchor::default()
    });
    a.behind = wrap == "behind";
    a.wrap = match wrap {
        "square" => Wrap::Square,
        "tight" => Wrap::Tight,
        "topAndBottom" => Wrap::TopAndBottom,
        _ => Wrap::None,
    };
    if a.wrap != Wrap::Tight {
        a.polygon = None;
        a.polygon_points.clear();
    }
    Ok(Some(a))
}

/// The document theme as a DrawingML theme (§20.1.6.9): the one BlueOffice authored, else the
/// source package's theme part.
pub(crate) fn doc_theme(doc: &bo_model::Document) -> Option<Theme> {
    const REL_THEME: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
    if let Some(x) = &doc.design.theme_xml {
        return bo_drawingml::theme::parse_theme(x.as_bytes(), "/word/theme/theme1.xml").ok();
    }
    let src = doc.source.as_ref()?;
    let pkg = bo_opc::Package::read(&src.bytes, &bo_opc::Limits::default()).ok()?;
    let name = pkg.related_part(&src.main_part, REL_THEME).ok()??;
    let part = pkg.part(&name)?;
    bo_drawingml::theme::parse_theme(&part.data, &name).ok()
}

/// A theme with BlueOffice's default colours (documents without a theme part).
fn default_theme() -> Theme {
    let mut colors: Vec<(String, Color)> = vec![
        ("dk1".into(), Color::srgb(0x00_0000)),
        ("lt1".into(), Color::srgb(0xFF_FFFF)),
        ("dk2".into(), Color::srgb(0x44_546A)),
        ("lt2".into(), Color::srgb(0xE7_E6E6)),
    ];
    for (i, v) in bo_sheet::chart_style::DEFAULT_ACCENTS.iter().enumerate() {
        colors.push((format!("accent{}", i + 1), Color::srgb(*v)));
    }
    Theme {
        colors: ColorScheme {
            name: "BlueOffice".into(),
            colors,
            extra: Vec::new(),
        },
        ..Theme::default()
    }
}

/// Draw chart part `bytes` (`w`×`h` EMU) with the shared chart renderer: BlueSlide's chart frame
/// on a page of the chart's size, colours from `theme`.
pub(crate) fn render(
    theme: Option<&Theme>,
    bytes: &[u8],
    w: i64,
    h: i64,
    fonts: &mut FontManager,
) -> Option<bo_render::PageList> {
    if w <= 0 || h <= 0 || !chart_part::is_chart_xml(bytes) {
        return None;
    }
    let ex = chart_part::is_chartex_xml(bytes);
    let (uri, el) = if ex {
        (chart_part::NS_CX, "cx:chart xmlns:cx")
    } else {
        (chart_part::URI_CHART, "c:chart xmlns:c")
    };
    let xml = format!(
        concat!(
            // Shape-tree markup as BlueSlide writes it (prefixes bound by the slide part).
            r#"<p:graphicFrame>"#,
            r#"<p:nvGraphicFramePr><p:cNvPr id="2" name="Chart"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>"#,
            r#"<p:xfrm><a:off x="0" y="0"/><a:ext cx="{w}" cy="{h}"/></p:xfrm>"#,
            r#"<a:graphic><a:graphicData uri="{uri}"><{el}="{uri}" r:id="rId1"/></a:graphicData></a:graphic></p:graphicFrame>"#
        ),
        w = w,
        h = h,
        uri = uri,
        el = el,
    );
    let node = crate::slide_edit::build::node(&xml)?;
    let part = if ex {
        "/ppt/charts/chartEx1.xml"
    } else {
        "/ppt/charts/chart1.xml"
    };
    let pres = Presentation {
        slide_size: SlideSize {
            cx: w,
            cy: h,
            kind: None,
        },
        themes: vec![theme.cloned().unwrap_or_else(default_theme)],
        masters: vec![SlideMaster::default()],
        layouts: vec![SlideLayout::default()],
        slides: vec![Slide {
            info: PartInfo {
                part_name: "/ppt/slides/slide1.xml".into(),
                rels: vec![Rel {
                    id: "rId1".into(),
                    rel_type: if ex {
                        chart_part::REL_CHARTEX.into()
                    } else {
                        chart_part::REL_CHART.into()
                    },
                    target: format!("../charts/{}", part.rsplit('/').next().unwrap_or_default()),
                    external: false,
                }],
                ..PartInfo::default()
            },
            data: CommonSlideData {
                shapes: vec![node],
                ..CommonSlideData::default()
            },
            ..Slide::default()
        }],
        parts: vec![RawPart {
            name: part.into(),
            content_type: None,
            data: bytes.to_vec(),
        }],
        ..Presentation::default()
    };
    let mut images = BTreeMap::new();
    bo_pptx::render_slide(&pres, 0, fonts, &mut images).ok()
}

/// Parts of the source package (chart, embedding folders), so new parts do not take their names.
fn source_parts(doc: &bo_model::Document) -> Vec<String> {
    let Some(pkg) = doc
        .source
        .as_ref()
        .and_then(|s| bo_opc::Package::read(&s.bytes, &bo_opc::Limits::default()).ok())
    else {
        return Vec::new();
    };
    pkg.parts()
        .iter()
        .map(|p| p.name.to_ascii_lowercase())
        .filter(|n| n.contains("/charts/") || n.contains("/embeddings/"))
        .collect()
}

/// A free chart part name (with its workbook) among the document's media and `taken`.
fn new_part(doc: &bo_model::Document, ex: bool, taken: &[String]) -> String {
    let used = |n: &str| {
        let l = n.to_ascii_lowercase();
        doc.media.keys().any(|k| k.to_ascii_lowercase() == l) || taken.contains(&l)
    };
    let mut n = 1;
    loop {
        let name = chart_part::chart_name(n, ex);
        if !used(&name) && !used(&chart_part::workbook_of(&name)) {
            return name;
        }
        n += 1;
    }
}

/// Parts (name, bytes) of a chart.
type Parts = Vec<(String, Arc<Vec<u8>>)>;

/// The two parts of chart `c` (data `d`) under chart part name `name`.
fn parts_of(name: &str, c: &Chart, d: &ChartData) -> std::result::Result<Parts, String> {
    let book = sc::workbook(d).map_err(|e| e.to_string())?;
    Ok(vec![
        (
            name.to_owned(),
            Arc::new(sc::chart_xml(c, chart_part::WORKBOOK_REL_ID)),
        ),
        (chart_part::workbook_of(name), Arc::new(book)),
    ])
}

/// Does chart part `part` of the source package have an embedded workbook (§21.2.2.63
/// externalData → §15.2.11 package relationship)?
fn file_workbook(doc: &bo_model::Document, part: &str) -> bool {
    doc.source
        .as_ref()
        .and_then(|s| bo_opc::Package::read(&s.bytes, &bo_opc::Limits::default()).ok())
        .and_then(|pkg| pkg.relationships(part).ok())
        .is_some_and(|rels| {
            rels.iter()
                .any(|r| r.rel_type == bo_model::ole::REL_PACKAGE)
        })
}

/// Read the chart of frame `img`.
fn read(doc: &bo_model::Document, img: &Image) -> Option<Chart> {
    let bytes = doc.media.get(&img.media)?;
    bo_xlsx::chart_write::parse_chart_part(bytes, &img.media)
}

/// Frame size in EMU from optional point sizes (`base` when absent).
fn size(width: Option<f64>, height: Option<f64>, base: (i64, i64)) -> (i64, i64) {
    let emu = |pt: f64| (pt.clamp(18.0, 1_500.0) * EMU_PT).round() as i64;
    (width.map_or(base.0, emu), height.map_or(base.1, emu))
}

impl crate::Engine {
    /// The chart at the selection of the open document.
    pub fn doc_chart(&self) -> Option<DocChart> {
        let s = self.session.as_ref()?;
        let img = s.chart()?;
        let doc = s.doc();
        let c = read(doc, &img)?;
        let book = chart_part::workbook_of(&img.media);
        let embedded = doc.media.contains_key(&book) || file_workbook(doc, &img.media);
        Some(DocChart {
            kind: sc::kind_name(&c).to_owned(),
            title: c.title.clone(),
            legend: c.legend.clone(),
            data: sc::data_of(&c),
            embedded,
            width: img.width.0 as f64 / EMU_PT,
            height: img.height.0 as f64 / EMU_PT,
            wrap: wrap_of(&img).to_owned(),
            part: img.media.clone(),
        })
    }

    /// Theme accents of the open document (chart colours).
    fn doc_accents(&self) -> [u32; 6] {
        let theme = self.session.as_ref().and_then(|s| doc_theme(s.doc()));
        sc::accents_of(theme.as_ref())
    }

    /// Run a BlueDoc chart command; the result is the edit result plus `chart` (the chart at the
    /// selection afterwards) and, for `chartGallery`, `chartGallery`.
    pub fn doc_chart_command(&mut self, cmd: DocChartCommand) -> Result<serde_json::Value> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let read_only = self.sec.read_only;
        let accents = self.doc_accents();
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let mut gallery = None;
        let res: std::result::Result<(), String> = match cmd {
            DocChartCommand::ChartInfo => Ok(()),
            DocChartCommand::ChartGallery { width, height } => (|| {
                let img = s.chart().ok_or("noChart")?;
                let c = read(s.doc(), &img).ok_or("noChart")?;
                gallery = Some(sc::gallery_for(
                    &c,
                    &accents,
                    width.unwrap_or(100.0),
                    height.unwrap_or(68.0),
                ));
                Ok(())
            })(),
            _ if read_only => Err(crate::security::READ_ONLY.into()),
            DocChartCommand::InsertChart {
                kind,
                title,
                data,
                legend,
                template,
                texts,
                width,
                height,
                wrap,
            } => (|| {
                let (c, d) = sc::new_chart(
                    &kind,
                    title.clone(),
                    legend.as_deref(),
                    data,
                    template,
                    texts,
                    &accents,
                )
                .map_err(|e| e.to_string())?;
                let name = new_part(s.doc(), sc::is_ex(&c), &source_parts(s.doc()));
                let parts = parts_of(&name, &c, &d)?;
                let (cx, cy) = size(width, height, (DEFAULT_CX, DEFAULT_CY));
                let img = Image {
                    media: name,
                    width: Emu(cx),
                    height: Emu(cy),
                    anchor: anchor_for(wrap.as_deref().unwrap_or("inline"), None)?,
                    crop: None,
                    rotation: 0,
                    description: c.title.clone().unwrap_or_default(),
                    shape: None,
                    format: None,
                    src: None,
                    ole: None,
                };
                s.insert_chart(parts, img)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            })(),
            DocChartCommand::SetChart {
                spec,
                wrap,
                width,
                height,
            } => (|| {
                let img = s.chart().ok_or("noChart")?;
                let anchor = match wrap.as_deref() {
                    Some(w) => Some(anchor_for(w, img.anchor.as_ref())?),
                    None => None,
                };
                let (cx, cy) = size(width, height, (img.width.0, img.height.0));
                let chart_change = spec.kind.is_some()
                    || spec.title.is_some()
                    || spec.legend.is_some()
                    || spec.data.is_some()
                    || !spec.design.is_empty();
                if !chart_change {
                    if anchor.is_none() && (cx, cy) == (img.width.0, img.height.0) {
                        return Err("noChange".into());
                    }
                    return s
                        .update_chart(Vec::new(), move |i| {
                            if let Some(a) = anchor {
                                i.anchor = a;
                            }
                            i.width = Emu(cx);
                            i.height = Emu(cy);
                        })
                        .map(|_| ())
                        .map_err(|e| e.to_string());
                }
                let c = read(s.doc(), &img).ok_or("noChart")?;
                let (c, d) = sc::edit_chart(c, &spec, &accents).map_err(|e| e.to_string())?;
                // A file's chart part stays as read: the edit gets new parts beside it.
                let taken = source_parts(s.doc());
                let in_source = taken.contains(&img.media.to_ascii_lowercase());
                let name = if in_source {
                    new_part(s.doc(), sc::is_ex(&c), &taken)
                } else {
                    img.media.clone()
                };
                let parts = parts_of(&name, &c, &d)?
                    .into_iter()
                    .map(|(n, b)| (n, Some(b)))
                    .collect();
                let alt = c.title.clone().unwrap_or_default();
                s.update_chart(parts, move |i| {
                    i.media = name;
                    i.src = None;
                    i.description = alt;
                    if let Some(a) = anchor {
                        i.anchor = a;
                    }
                    i.width = Emu(cx);
                    i.height = Emu(cy);
                })
                .map(|_| ())
                .map_err(|e| e.to_string())
            })(),
        };
        let dirty = self.relayout()?;
        let mut r: EditResult = self.story_result(dirty, res.err())?;
        r.collab = self.collab_after_edit();
        let mut v = serde_json::to_value(&r).unwrap_or_default();
        if let Some(o) = v.as_object_mut() {
            o.insert(
                "chart".into(),
                serde_json::to_value(self.doc_chart()).unwrap_or_default(),
            );
            if let Some(g) = gallery {
                o.insert("chartGallery".into(), g);
            }
        }
        Ok(v)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // source: default theme accents (bo_sheet::chart_style::DEFAULT_ACCENTS, §20.1.6.2 clrScheme)
    // colour the first series; the frame is drawn on a page of its own size (EMU / 12 700 = pt).
    #[test]
    fn a_chart_part_is_drawn_with_the_theme_accents() {
        let (c, _) = sc::new_chart(
            "column",
            Some("T".into()),
            None,
            None,
            None,
            bo_sheet::chart_style::LayoutTexts::default(),
            &bo_sheet::chart_style::DEFAULT_ACCENTS,
        )
        .unwrap();
        let xml = sc::chart_xml(&c, "rId1");
        let mut fonts = FontManager::new();
        for f in crate::bundled::FONTS {
            fonts.add_font(f.to_vec());
        }
        let p = render(None, &xml, 5_400_000, 3_240_000, &mut fonts).unwrap();
        assert!((p.width - 425.196_85).abs() < 0.01 && (p.height - 255.118_1).abs() < 0.01);
        let a1 = bo_sheet::chart_style::DEFAULT_ACCENTS[0];
        let accent = bo_render::Rgba((a1 >> 16) as u8, (a1 >> 8) as u8, a1 as u8, 255);
        let painted = p.commands.iter().any(|c| match c {
            bo_render::Command::FillRect { color, .. }
            | bo_render::Command::Path {
                fill: Some(bo_render::Paint::Solid { color }),
                ..
            } => *color == accent,
            _ => false,
        });
        assert!(painted, "{:?}", p.commands.len());
        assert!(p
            .commands
            .iter()
            .any(|c| matches!(c, bo_render::Command::Glyphs { .. })));
        assert!(render(None, b"<x/>", 10, 10, &mut fonts).is_none());
    }
}
