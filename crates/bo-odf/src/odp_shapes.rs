//! `.odp` slides with their drawing (ODF 1.3 Part 3): page size (§16.5
//! `style:page-layout`), shapes (§10.3 `draw:rect`, `draw:ellipse`, `draw:line`,
//! `draw:custom-shape` with §10.6.2 `draw:enhanced-geometry`, §10.4.2
//! `draw:frame` holding a §10.4.3 `draw:text-box` or a §10.4.4 `draw:image`),
//! positions (§19.541 `svg:x`/`svg:y`, §19.229 `draw:transform`), graphic styles
//! (§20.110 `draw:fill`, §20.114 `draw:fill-color`, §20.155 `draw:stroke`,
//! §20.540 `svg:stroke-color`, §20.541 `svg:stroke-width`), text styles
//! (§20.183 `fo:font-weight`, §20.181 `fo:font-style`, §20.179 `fo:font-size`,
//! §20.174 `fo:color`, §20.380 `style:text-underline-style`, §20.216
//! `fo:text-align`) and slide backgrounds.

use crate::dom::{self, Element, Node};
use crate::ns::{DRAW, FO, OFFICE, PRESENTATION, STYLE, SVG, TEXT};
use crate::package::OdfPackage;
use crate::units::parse_length;
use crate::{OdfError, Result, MIME_ODP};
use std::collections::HashMap;

const XLINK: &str = "http://www.w3.org/1999/xlink";
/// Shapes kept per slide.
const MAX_SHAPES: usize = 4096;
/// Style inheritance followed at most this deep.
const MAX_STYLE_DEPTH: usize = 16;

/// A presentation with its drawing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OdpDeck {
    /// Page size in EMU.
    pub width: i64,
    /// Page height in EMU.
    pub height: i64,
    /// Slides in document order.
    pub slides: Vec<OdpSlide>,
}

/// One `draw:page`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OdpSlide {
    /// `draw:name` of the page.
    pub name: String,
    /// Shapes in z-order (back to front).
    pub shapes: Vec<OdpShape>,
    /// Paragraphs of the speaker notes (`presentation:notes`).
    pub notes: Vec<String>,
    /// Solid background `RRGGBB`.
    pub background: Option<String>,
}

/// Fill or stroke state from the graphic style.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Paint {
    /// Not specified (inherited).
    #[default]
    Inherit,
    /// Explicitly not painted (`draw:fill="none"` / `draw:stroke="none"`).
    None,
    /// Solid colour `RRGGBB`.
    Solid(String),
}

/// A shape, in EMU.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OdpShape {
    /// DrawingML preset geometry name (ECMA-376 §20.1.10.56).
    pub geometry: String,
    /// `presentation:class` of a placeholder frame.
    pub class: Option<String>,
    /// Left edge in EMU (`svg:x`).
    pub x: i64,
    /// Top edge in EMU (`svg:y`).
    pub y: i64,
    /// Width in EMU (`svg:width`).
    pub cx: i64,
    /// Height in EMU (`svg:height`).
    pub cy: i64,
    /// Clockwise rotation in 60 000ths of a degree.
    pub rotation: i32,
    /// Fill (`draw:fill`, `draw:fill-color`).
    pub fill: Paint,
    /// Stroke (`draw:stroke`, `svg:stroke-color`).
    pub stroke: Paint,
    /// Stroke width in EMU.
    pub stroke_width: Option<i64>,
    /// Picture data and extension (`png`, `jpeg`, `gif`, …).
    pub image: Option<(Vec<u8>, String)>,
    /// Text paragraphs of the shape.
    pub paragraphs: Vec<OdpParagraph>,
    /// `t`, `ctr`, `b` (§20.192 `draw:textarea-vertical-align`).
    pub anchor: Option<String>,
}

/// A paragraph.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OdpParagraph {
    /// Formatted runs (`text:span`).
    pub runs: Vec<OdpRun>,
    /// `l`, `ctr`, `r`, `just`.
    pub align: Option<String>,
    /// List nesting level (0 = not in a list).
    pub level: u8,
    /// Inside a `text:list`.
    pub bullet: bool,
}

/// A run of text with its formatting.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OdpRun {
    /// Text of the run.
    pub text: String,
    /// Bold (`fo:font-weight`).
    pub bold: bool,
    /// Italic (`fo:font-style`).
    pub italic: bool,
    /// Underlined (`style:text-underline-style`).
    pub underline: bool,
    /// Hundredths of a point.
    pub size: Option<u32>,
    /// `RRGGBB`.
    pub color: Option<String>,
    /// Font family (`fo:font-family` / `style:font-name`).
    pub font: Option<String>,
}

/// Properties collected from one style (and its parents).
#[derive(Debug, Clone, Default)]
struct Props {
    fill: Paint,
    stroke: Paint,
    stroke_width: Option<i64>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    size: Option<u32>,
    color: Option<String>,
    font: Option<String>,
    align: Option<String>,
    anchor: Option<String>,
}

impl Props {
    /// `self` with unset fields taken from `base`.
    fn over(&self, base: &Props) -> Props {
        Props {
            fill: if self.fill == Paint::Inherit {
                base.fill.clone()
            } else {
                self.fill.clone()
            },
            stroke: if self.stroke == Paint::Inherit {
                base.stroke.clone()
            } else {
                self.stroke.clone()
            },
            stroke_width: self.stroke_width.or(base.stroke_width),
            bold: self.bold.or(base.bold),
            italic: self.italic.or(base.italic),
            underline: self.underline.or(base.underline),
            size: self.size.or(base.size),
            color: self.color.clone().or_else(|| base.color.clone()),
            font: self.font.clone().or_else(|| base.font.clone()),
            align: self.align.clone().or_else(|| base.align.clone()),
            anchor: self.anchor.clone().or_else(|| base.anchor.clone()),
        }
    }
}

/// Named styles: name → (own properties, parent).
#[derive(Default)]
struct Styles {
    map: HashMap<String, (Props, Option<String>)>,
    fonts: HashMap<String, String>,
}

fn hex(c: &str) -> Option<String> {
    let h = c.trim().strip_prefix('#')?;
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_ascii_uppercase())
}

/// EMU of an ODF length.
fn emu(s: Option<&str>) -> Option<i64> {
    parse_length(s?).map(|t| i64::from(t.0) * 635)
}

impl Styles {
    fn collect(&mut self, root: &Element) {
        for container in root.elements() {
            if container.is(OFFICE, "font-face-decls") {
                for f in container.children_named(STYLE, "font-face") {
                    if let (Some(n), Some(fam)) =
                        (f.attr(STYLE, "name"), f.attr(SVG, "font-family"))
                    {
                        self.fonts
                            .insert(n.to_owned(), fam.trim_matches(['\'', '"']).to_owned());
                    }
                }
            }
            if !(container.is(OFFICE, "styles") || container.is(OFFICE, "automatic-styles")) {
                continue;
            }
            for st in container.children_named(STYLE, "style") {
                let Some(name) = st.attr(STYLE, "name") else {
                    continue;
                };
                let parent = st.attr(STYLE, "parent-style-name").map(str::to_owned);
                self.map.insert(name.to_owned(), (self.props(st), parent));
            }
        }
    }

    fn props(&self, st: &Element) -> Props {
        let mut p = Props::default();
        for e in st.elements() {
            if e.is(STYLE, "graphic-properties") || e.is(STYLE, "drawing-page-properties") {
                p.fill = match e.attr(DRAW, "fill") {
                    Some("none") => Paint::None,
                    Some(v) if v != "solid" => Paint::Inherit,
                    // `solid`, or a fill colour without `draw:fill`.
                    _ => e
                        .attr(DRAW, "fill-color")
                        .and_then(hex)
                        .map_or(Paint::Inherit, Paint::Solid),
                };
                p.stroke = match e.attr(DRAW, "stroke") {
                    Some("none") => Paint::None,
                    Some(_) | None => e
                        .attr(SVG, "stroke-color")
                        .and_then(hex)
                        .map_or(Paint::Inherit, Paint::Solid),
                };
                if e.attr(DRAW, "stroke") == Some("solid") && p.stroke == Paint::Inherit {
                    p.stroke = Paint::Solid("000000".into());
                }
                p.stroke_width = emu(e.attr(SVG, "stroke-width"));
                p.anchor = e.attr(DRAW, "textarea-vertical-align").map(|a| {
                    match a {
                        "middle" => "ctr",
                        "bottom" => "b",
                        _ => "t",
                    }
                    .to_owned()
                });
            }
            if e.is(STYLE, "text-properties") {
                p.bold = e
                    .attr(FO, "font-weight")
                    .map(|w| w == "bold" || w.parse::<u32>().is_ok_and(|n| n >= 600));
                p.italic = e
                    .attr(FO, "font-style")
                    .map(|s| s == "italic" || s == "oblique");
                p.underline = e.attr(STYLE, "text-underline-style").map(|u| u != "none");
                p.size = e
                    .attr(FO, "font-size")
                    .and_then(parse_length)
                    .and_then(|t| u32::try_from(t.0).ok())
                    .map(|tw| tw * 5);
                p.color = e.attr(FO, "color").and_then(hex);
                p.font = e
                    .attr(STYLE, "font-name")
                    .and_then(|n| self.fonts.get(n).cloned())
                    .or_else(|| {
                        e.attr(FO, "font-family")
                            .map(|f| f.trim_matches(['\'', '"']).to_owned())
                    });
            }
            if e.is(STYLE, "paragraph-properties") {
                p.align = e.attr(FO, "text-align").map(|a| {
                    match a {
                        "center" => "ctr",
                        "end" | "right" => "r",
                        "justify" => "just",
                        _ => "l",
                    }
                    .to_owned()
                });
            }
        }
        p
    }

    /// Resolved properties of a style name (parents applied).
    fn resolve(&self, name: Option<&str>) -> Props {
        let mut out = Props::default();
        let mut cur = name.map(str::to_owned);
        for _ in 0..MAX_STYLE_DEPTH {
            let Some(n) = cur else { break };
            let Some((p, parent)) = self.map.get(&n) else {
                break;
            };
            out = out.over(p);
            cur = parent.clone();
        }
        out
    }
}

/// DrawingML preset for an enhanced-geometry `draw:type` (ODF §19.145).
fn preset_of(kind: &str) -> &'static str {
    match kind {
        "ellipse" | "circle" => "ellipse",
        "round-rectangle" | "round-square" => "roundRect",
        "diamond" | "flowchart-decision" => "diamond",
        "isosceles-triangle" => "triangle",
        "right-triangle" => "rtTriangle",
        "parallelogram" | "flowchart-data" => "parallelogram",
        "trapezoid" => "trapezoid",
        "hexagon" => "hexagon",
        "octagon" => "octagon",
        "pentagon" => "pentagon",
        "star4" => "star4",
        "star5" => "star5",
        "star8" => "star8",
        "star24" => "star24",
        "right-arrow" => "rightArrow",
        "left-arrow" => "leftArrow",
        "up-arrow" => "upArrow",
        "down-arrow" => "downArrow",
        "left-right-arrow" => "leftRightArrow",
        "up-down-arrow" => "upDownArrow",
        "heart" => "heart",
        "smiley" => "smileyFace",
        "moon" => "moon",
        "sun" => "sun",
        "cloud" => "cloud",
        "can" | "flowchart-magnetic-disk" => "can",
        "cube" => "cube",
        "cross" => "plus",
        "block-arc" => "blockArc",
        "flowchart-terminator" => "flowChartTerminator",
        "flowchart-alternate-process" => "flowChartAlternateProcess",
        "chevron" => "chevron",
        "lightning" => "lightningBolt",
        _ => "rect",
    }
}

/// `draw:transform` "rotate (a) translate (x y)" (§19.229): rotation in
/// radians (counter-clockwise) and translation of the shape's origin.
fn transform(t: &str) -> (f64, Option<(i64, i64)>) {
    let mut angle = 0.0;
    let mut tr = None;
    let mut rest = t;
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim();
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let args: Vec<&str> = rest[open + 1..open + close]
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .collect();
        match name {
            "rotate" => {
                angle += args
                    .first()
                    .and_then(|a| a.parse::<f64>().ok())
                    .unwrap_or(0.0);
            }
            "translate" => {
                tr = match (emu(args.first().copied()), emu(args.get(1).copied())) {
                    (Some(x), Some(y)) => Some((x, y)),
                    _ => tr,
                };
            }
            _ => {}
        }
        rest = &rest[open + close + 1..];
    }
    (angle, tr)
}

struct Ctx<'a> {
    styles: &'a Styles,
    pkg: &'a OdfPackage,
}

impl Ctx<'_> {
    fn picture(&self, href: &str) -> Option<(Vec<u8>, String)> {
        let href = href.trim_start_matches("./");
        let data = self.pkg.file(href)?.to_vec();
        let ext = if data.starts_with(&[0x89, b'P', b'N', b'G']) {
            "png"
        } else if data.starts_with(&[0xFF, 0xD8]) {
            "jpeg"
        } else if data.starts_with(b"GIF8") {
            "gif"
        } else if data.starts_with(b"BM") {
            "bmp"
        } else {
            return None;
        };
        Some((data, ext.to_owned()))
    }

    /// Runs of a paragraph-like element with the inherited `base` style.
    fn runs(&self, el: &Element, base: &Props, out: &mut Vec<OdpRun>, depth: usize) {
        if depth > dom::MAX_DEPTH {
            return;
        }
        for n in &el.children {
            match n {
                Node::Text(t) => {
                    let mut s = String::new();
                    let mut last_space = out.last().is_some_and(|r| r.text.ends_with(' '));
                    for c in t.chars() {
                        if matches!(c, ' ' | '\t' | '\n' | '\r') {
                            if !last_space {
                                s.push(' ');
                                last_space = true;
                            }
                        } else {
                            s.push(c);
                            last_space = false;
                        }
                    }
                    push_run(out, s, base);
                }
                Node::Element(e) if e.is(TEXT, "span") => {
                    let p = self.styles.resolve(e.attr(TEXT, "style-name")).over(base);
                    self.runs(e, &p, out, depth + 1);
                }
                Node::Element(e) if e.is(TEXT, "s") => {
                    let c = e
                        .attr(TEXT, "c")
                        .and_then(|c| c.parse().ok())
                        .unwrap_or(1usize)
                        .clamp(1, 1000);
                    push_run(out, " ".repeat(c), base);
                }
                Node::Element(e) if e.is(TEXT, "tab") => push_run(out, "\t".into(), base),
                Node::Element(e) if e.is(TEXT, "line-break") => {
                    push_run(out, "\u{2028}".into(), base);
                }
                Node::Element(e) if e.ns == TEXT && e.name == "note" => {}
                Node::Element(e) => self.runs(e, base, out, depth + 1),
            }
        }
    }

    fn paragraphs(
        &self,
        el: &Element,
        base: &Props,
        level: u8,
        out: &mut Vec<OdpParagraph>,
        depth: usize,
    ) {
        if depth > dom::MAX_DEPTH {
            return;
        }
        for e in el.elements() {
            if e.is(TEXT, "p") || e.is(TEXT, "h") {
                let ps = self.styles.resolve(e.attr(TEXT, "style-name")).over(base);
                let mut runs = Vec::new();
                self.runs(e, &ps, &mut runs, depth + 1);
                if let Some(last) = runs.last_mut() {
                    let t = last.text.trim_end_matches(' ').len();
                    last.text.truncate(t);
                }
                out.push(OdpParagraph {
                    runs,
                    align: ps.align.clone(),
                    level: level.saturating_sub(1),
                    bullet: level > 0,
                });
            } else if e.is(TEXT, "list") {
                self.paragraphs(e, base, level.saturating_add(1).min(9), out, depth + 1);
            } else if e.ns == TEXT
                && matches!(e.name.as_str(), "list-item" | "list-header" | "section")
            {
                self.paragraphs(e, base, level, out, depth + 1);
            }
        }
    }

    fn shapes(&self, el: &Element, out: &mut Vec<OdpShape>, depth: usize) {
        if depth > dom::MAX_DEPTH {
            return;
        }
        for e in el.elements() {
            if e.ns != DRAW || out.len() >= MAX_SHAPES {
                continue;
            }
            match e.name.as_str() {
                // §10.3.15 draw:g
                "g" => self.shapes(e, out, depth + 1),
                "frame" | "rect" | "ellipse" | "circle" | "custom-shape" | "line" | "polygon" => {
                    if let Some(s) = self.shape(e, depth) {
                        out.push(s);
                    }
                }
                _ => {}
            }
        }
    }

    fn shape(&self, e: &Element, depth: usize) -> Option<OdpShape> {
        let style = self
            .styles
            .resolve(e.attr(PRESENTATION, "style-name"))
            .over(&self.styles.resolve(e.attr(DRAW, "style-name")));
        let mut s = OdpShape {
            class: e.attr(PRESENTATION, "class").map(str::to_owned),
            fill: style.fill.clone(),
            stroke: style.stroke.clone(),
            stroke_width: style.stroke_width,
            anchor: style.anchor.clone(),
            ..OdpShape::default()
        };
        if s.class.as_deref() == Some("page") {
            return None;
        }
        let w = emu(e.attr(SVG, "width")).unwrap_or(0);
        let h = emu(e.attr(SVG, "height")).unwrap_or(0);
        let (mut x, mut y) = (
            emu(e.attr(SVG, "x")).unwrap_or(0),
            emu(e.attr(SVG, "y")).unwrap_or(0),
        );
        let mut cx = w;
        let mut cy = h;
        match e.name.as_str() {
            "line" => {
                let x1 = emu(e.attr(SVG, "x1")).unwrap_or(0);
                let y1 = emu(e.attr(SVG, "y1")).unwrap_or(0);
                let x2 = emu(e.attr(SVG, "x2")).unwrap_or(0);
                let y2 = emu(e.attr(SVG, "y2")).unwrap_or(0);
                x = x1.min(x2);
                y = y1.min(y2);
                cx = (x2 - x1).abs();
                cy = (y2 - y1).abs();
                s.geometry = "line".into();
                if s.stroke == Paint::Inherit {
                    s.stroke = Paint::Solid("000000".into());
                }
            }
            "ellipse" | "circle" => s.geometry = "ellipse".into(),
            "custom-shape" => {
                let kind = e
                    .child(DRAW, "enhanced-geometry")
                    .and_then(|g| g.attr(DRAW, "type"))
                    .unwrap_or("rectangle");
                s.geometry = preset_of(kind).into();
            }
            "frame" => {
                s.geometry = "rect".into();
                if let Some(img) = e.child(DRAW, "image") {
                    if let Some(href) = img.attr(XLINK, "href") {
                        s.image = self.picture(href);
                    }
                }
            }
            _ => s.geometry = "rect".into(),
        }
        if let Some(t) = e.attr(DRAW, "transform") {
            let (angle, tr) = transform(t);
            if let Some((tx, ty)) = tr {
                // Centre of the rotated box, then the unrotated box about it.
                #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
                let (ccx, ccy) = {
                    let (hw, hh) = (cx as f64 / 2.0, cy as f64 / 2.0);
                    (
                        tx as f64 + hw * angle.cos() + hh * angle.sin(),
                        ty as f64 - hw * angle.sin() + hh * angle.cos(),
                    )
                };
                #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
                {
                    x = (ccx - cx as f64 / 2.0).round() as i64;
                    y = (ccy - cy as f64 / 2.0).round() as i64;
                }
            }
            let deg = (-angle.to_degrees()).rem_euclid(360.0);
            #[allow(clippy::cast_possible_truncation)]
            {
                s.rotation = (deg * 60_000.0).round() as i32;
            }
        }
        (s.x, s.y, s.cx, s.cy) = (x, y, cx.max(0), cy.max(0));
        let text_root = e.child(DRAW, "text-box").unwrap_or(e);
        self.paragraphs(text_root, &style, 0, &mut s.paragraphs, depth + 1);
        if e.name == "frame" && s.image.is_none() && s.paragraphs.is_empty() && s.class.is_none() {
            return None;
        }
        Some(s)
    }
}

fn push_run(out: &mut Vec<OdpRun>, text: String, p: &Props) {
    if text.is_empty() {
        return;
    }
    let run = OdpRun {
        text,
        bold: p.bold.unwrap_or(false),
        italic: p.italic.unwrap_or(false),
        underline: p.underline.unwrap_or(false),
        size: p.size,
        color: p.color.clone(),
        font: p.font.clone(),
    };
    if let Some(last) = out.last_mut() {
        let same = last.bold == run.bold
            && last.italic == run.italic
            && last.underline == run.underline
            && last.size == run.size
            && last.color == run.color
            && last.font == run.font;
        if same {
            last.text.push_str(&run.text);
            return;
        }
    }
    out.push(run);
}

/// Reads an `.odp` with its drawing.
pub fn read_odp_deck(bytes: &[u8]) -> Result<OdpDeck> {
    let pkg = OdfPackage::read(bytes)?;
    pkg.expect_mimetype(MIME_ODP)?;
    let content = dom::parse(pkg.require("content.xml")?, "content.xml")?;
    let styles_doc = match pkg.file("styles.xml") {
        Some(b) => Some(dom::parse(b, "styles.xml")?),
        None => None,
    };
    let mut styles = Styles::default();
    if let Some(sd) = &styles_doc {
        styles.collect(sd);
    }
    styles.collect(&content);
    // Page size: the page layout of the first master page (§16.9, §16.5).
    let mut deck = OdpDeck {
        width: 28 * 360_000,
        height: 15_750 * 360,
        slides: Vec::new(),
    };
    if let Some(sd) = &styles_doc {
        let layouts: HashMap<&str, &Element> = sd
            .elements()
            .filter(|c| c.is(OFFICE, "automatic-styles") || c.is(OFFICE, "styles"))
            .flat_map(|c| c.children_named(STYLE, "page-layout"))
            .filter_map(|l| Some((l.attr(STYLE, "name")?, l)))
            .collect();
        let master = sd
            .child(OFFICE, "master-styles")
            .and_then(|m| m.child(STYLE, "master-page"));
        if let Some(pl) = master
            .and_then(|m| m.attr(STYLE, "page-layout-name"))
            .and_then(|n| layouts.get(n))
            .and_then(|l| l.child(STYLE, "page-layout-properties"))
        {
            if let (Some(w), Some(h)) = (
                emu(pl.attr(FO, "page-width")),
                emu(pl.attr(FO, "page-height")),
            ) {
                deck.width = w;
                deck.height = h;
            }
        }
    }
    let pres = content
        .child(OFFICE, "body")
        .and_then(|b| b.child(OFFICE, "presentation"))
        .ok_or_else(|| OdfError::Missing("office:body/office:presentation".into()))?;
    let ctx = Ctx {
        styles: &styles,
        pkg: &pkg,
    };
    for page in pres.children_named(DRAW, "page") {
        let mut slide = OdpSlide {
            name: page.attr(DRAW, "name").unwrap_or_default().to_owned(),
            ..OdpSlide::default()
        };
        if let Paint::Solid(c) = styles.resolve(page.attr(DRAW, "style-name")).fill {
            slide.background = Some(c);
        }
        ctx.shapes(page, &mut slide.shapes, 0);
        if let Some(notes) = page.child(PRESENTATION, "notes") {
            let mut shapes = Vec::new();
            ctx.shapes(notes, &mut shapes, 0);
            slide.notes = shapes
                .iter()
                .flat_map(|s| &s.paragraphs)
                .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect())
                .collect();
        }
        deck.slides.push(slide);
    }
    Ok(deck)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Automatic styles shared by the generated decks.
    const STYLES: &str = r##"<style:style style:name="dp1" style:family="drawing-page"><style:drawing-page-properties draw:fill="solid" draw:fill-color="#eeeeee"/></style:style>
<style:style style:name="dp2" style:family="drawing-page"><style:drawing-page-properties draw:fill="solid" draw:fill-color="#fff2cc"/></style:style>
<style:style style:name="gr1" style:family="graphic"><style:graphic-properties draw:fill="solid" draw:fill-color="#c00000" draw:stroke="solid" svg:stroke-color="#000000" svg:stroke-width="0.1cm" draw:textarea-vertical-align="middle"/></style:style>
<style:style style:name="gr2" style:family="graphic" style:parent-style-name="gr1"><style:graphic-properties draw:fill="none" draw:stroke="none"/></style:style>
<style:style style:name="gr3" style:family="graphic"><style:graphic-properties draw:fill="solid" draw:fill-color="#2e75b6" draw:stroke="none" draw:textarea-vertical-align="middle"/></style:style>
<style:style style:name="gr4" style:family="graphic"><style:graphic-properties draw:fill="solid" draw:fill-color="#70ad47" draw:stroke="solid" svg:stroke-color="#385723" svg:stroke-width="0.05cm"/></style:style>
<style:style style:name="P1" style:family="paragraph"><style:paragraph-properties fo:text-align="center"/></style:style>
<style:style style:name="P2" style:family="paragraph"><style:paragraph-properties fo:text-align="end"/></style:style>
<style:style style:name="T1" style:family="text"><style:text-properties fo:font-weight="bold" fo:font-size="24pt" fo:color="#ffffff" style:font-name="Lib"/></style:style>
<style:style style:name="T2" style:family="text"><style:text-properties fo:font-style="italic" style:text-underline-style="solid"/></style:style>
<style:style style:name="T3" style:family="text"><style:text-properties fo:color="#c00000" fo:font-size="32pt"/></style:style>"##;

    /// An `.odp` package with the shared styles, `pages` (`draw:page` elements)
    /// and the pictures `Pictures/<name>`.
    pub fn odp_package(pages: &str, pictures: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {} xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" office:version="1.3"><office:font-face-decls><style:font-face style:name="Lib" svg:font-family="'Liberation Sans'"/></office:font-face-decls><office:automatic-styles>{STYLES}</office:automatic-styles><office:body><office:presentation>{pages}</office:presentation></office:body></office:document-content>"#,
            crate::odt_write::XMLNS
        );
        let styles = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {} office:version="1.3"><office:automatic-styles><style:page-layout style:name="PM1"><style:page-layout-properties fo:page-width="25.4cm" fo:page-height="19.05cm"/></style:page-layout></office:automatic-styles><office:master-styles><style:master-page style:name="Default" style:page-layout-name="PM1"/></office:master-styles></office:document-styles>"#,
            crate::odt_write::XMLNS
        );
        let mut files: Vec<(String, &str, Vec<u8>)> = vec![
            ("content.xml".into(), "text/xml", content.into_bytes()),
            ("styles.xml".into(), "text/xml", styles.into_bytes()),
        ];
        for (name, data) in pictures {
            files.push((format!("Pictures/{name}"), "image/png", data.clone()));
        }
        let refs: Vec<(&str, &str, Vec<u8>)> = files
            .iter()
            .map(|(n, t, d)| (n.as_str(), *t, d.clone()))
            .collect();
        OdfPackage::write(MIME_ODP, &refs).unwrap_or_default()
    }

    /// A PNG of `w × h` pixels (generated).
    fn png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgb);
            if let Ok(mut wr) = enc.write_header() {
                let px: Vec<u8> = (0..w * h)
                    .flat_map(|i| [rgb[0], rgb[1] ^ u8::try_from(i % 200).unwrap_or(0), rgb[2]])
                    .collect();
                let _ = wr.write_image_data(&px);
            }
        }
        out
    }

    const PAGE1: &str = r#"<draw:page draw:name="Một" draw:style-name="dp1" draw:master-page-name="Default">
  <draw:frame presentation:class="title" svg:x="1cm" svg:y="1cm" svg:width="20cm" svg:height="2cm"><draw:text-box><text:p>Tiêu đề</text:p></draw:text-box></draw:frame>
  <draw:custom-shape draw:style-name="gr1" svg:x="2cm" svg:y="4cm" svg:width="6cm" svg:height="3cm"><text:p text:style-name="P1"><text:span text:style-name="T1">Đậm</text:span> và <text:span text:style-name="T2">nghiêng</text:span></text:p><draw:enhanced-geometry draw:type="ellipse"/></draw:custom-shape>
  <draw:rect draw:style-name="gr2" draw:transform="rotate (-1.5707963267949) translate (12cm 4cm)" svg:width="4cm" svg:height="2cm"/>
  <draw:line svg:x1="1cm" svg:y1="9cm" svg:x2="5cm" svg:y2="8cm"/>
  <draw:frame svg:x="15cm" svg:y="6cm" svg:width="3cm" svg:height="3cm"><draw:image xlink:href="Pictures/a.png"/></draw:frame>
  <draw:frame presentation:class="outline" svg:x="1cm" svg:y="10cm" svg:width="20cm" svg:height="4cm"><draw:text-box><text:list><text:list-item><text:p>Mục 1</text:p><text:list><text:list-item><text:p>Mục 1.1</text:p></text:list-item></text:list></text:list-item></text:list></draw:text-box></draw:frame>
  <presentation:notes><draw:page-thumbnail presentation:class="page"/><draw:frame presentation:class="notes"><draw:text-box><text:p>Ghi chú</text:p></draw:text-box></draw:frame></presentation:notes>
</draw:page>"#;

    /// A styled `.odp`: page size, background, shapes, rotation, image, text styles.
    pub fn styled_odp() -> Vec<u8> {
        odp_package(PAGE1, &[("a.png", png(24, 16, [0x2E, 0x75, 0xB6]))])
    }

    /// The `.odp` corpus (`tests/corpus/slide/odf-odp-*`): hand-written ODF 1.3 XML.
    pub fn odp_corpus() -> Vec<(&'static str, Vec<u8>)> {
        let text = r#"<draw:page draw:name="Chữ" draw:master-page-name="Default"><draw:frame presentation:class="title" svg:x="1cm" svg:y="0.8cm" svg:width="23cm" svg:height="2.5cm"><draw:text-box><text:p text:style-name="P1"><text:span text:style-name="T3">Định dạng chữ</text:span></text:p></draw:text-box></draw:frame><draw:frame presentation:class="outline" svg:x="1cm" svg:y="4cm" svg:width="23cm" svg:height="9cm"><draw:text-box><text:list><text:list-item><text:p>Cấp 1 <text:span text:style-name="T2">nghiêng gạch chân</text:span></text:p><text:list><text:list-item><text:p>Cấp 2</text:p></text:list-item></text:list></text:list-item><text:list-item><text:p text:style-name="P2">Căn phải</text:p></text:list-item></text:list></draw:text-box></draw:frame><presentation:notes><draw:page-thumbnail presentation:class="page"/><draw:frame presentation:class="notes"><draw:text-box><text:p>Ghi chú trang chữ</text:p></draw:text-box></draw:frame></presentation:notes></draw:page>
<draw:page draw:name="Hộp chữ" draw:style-name="dp2" draw:master-page-name="Default"><draw:frame svg:x="2cm" svg:y="2cm" svg:width="10cm" svg:height="3cm"><draw:text-box><text:p>Hộp chữ tự do<text:line-break/>dòng hai</text:p></draw:text-box></draw:frame><draw:custom-shape draw:style-name="gr3" svg:x="13cm" svg:y="2cm" svg:width="8cm" svg:height="4cm"><text:p text:style-name="P1"><text:span text:style-name="T1">Chữ trắng đậm</text:span></text:p><draw:enhanced-geometry draw:type="round-rectangle"/></draw:custom-shape></draw:page>"#;
        let pictures = r#"<draw:page draw:name="Ảnh" draw:style-name="dp1" draw:master-page-name="Default"><draw:frame presentation:class="title" svg:x="1cm" svg:y="0.8cm" svg:width="23cm" svg:height="2.5cm"><draw:text-box><text:p>Ảnh và hình xoay</text:p></draw:text-box></draw:frame><draw:frame svg:x="1cm" svg:y="4cm" svg:width="8cm" svg:height="6cm"><draw:image xlink:href="Pictures/p1.png"/></draw:frame><draw:frame svg:x="10cm" svg:y="4cm" svg:width="4cm" svg:height="4cm"><draw:image xlink:href="Pictures/p2.png"/></draw:frame><draw:custom-shape draw:style-name="gr4" draw:transform="rotate (-0.523598775598) translate (16cm 4cm)" svg:width="6cm" svg:height="3cm"><draw:enhanced-geometry draw:type="right-arrow"/></draw:custom-shape><draw:custom-shape draw:style-name="gr1" svg:x="16cm" svg:y="10cm" svg:width="4cm" svg:height="4cm"><draw:enhanced-geometry draw:type="star5"/></draw:custom-shape><presentation:notes><draw:page-thumbnail presentation:class="page"/><draw:frame presentation:class="notes"><draw:text-box><text:p>Ảnh tự sinh</text:p></draw:text-box></draw:frame></presentation:notes></draw:page>"#;
        vec![
            ("slide/odf-odp-01-shapes.odp", styled_odp()),
            ("slide/odf-odp-02-text-styles.odp", odp_package(text, &[])),
            (
                "slide/odf-odp-03-pictures-rotation.odp",
                odp_package(
                    pictures,
                    &[
                        ("p1.png", png(40, 30, [0xC0, 0x30, 0x10])),
                        ("p2.png", png(20, 20, [0x10, 0x90, 0x40])),
                    ],
                ),
            ),
        ]
    }

    /// `BO_WRITE_CORPUS=1 cargo test -p bo-odf odp_corpus` writes the files;
    /// otherwise the committed ones must read like the generator's output.
    #[test]
    fn odp_corpus_files() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
        for (name, bytes) in odp_corpus() {
            let path = dir.join(name);
            if std::env::var_os("BO_WRITE_CORPUS").is_some() {
                std::fs::write(&path, &bytes).unwrap();
                continue;
            }
            let file = std::fs::read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(
                read_odp_deck(&file).unwrap(),
                read_odp_deck(&bytes).unwrap(),
                "{name}"
            );
        }
    }

    #[test]
    fn reads_shapes_styles_and_pictures() {
        let d = read_odp_deck(&styled_odp()).unwrap();
        let cm = |v: f64| emu(Some(&format!("{v}cm"))).unwrap();
        assert_eq!((d.width, d.height), (9_144_000, 6_858_000));
        let s = &d.slides[0];
        assert_eq!(s.background.as_deref(), Some("EEEEEE"));
        assert_eq!(s.shapes.len(), 6);
        assert_eq!(s.shapes[0].class.as_deref(), Some("title"));
        let e = &s.shapes[1];
        assert_eq!(e.geometry, "ellipse");
        assert_eq!(e.fill, Paint::Solid("C00000".into()));
        assert_eq!(e.stroke, Paint::Solid("000000".into()));
        assert_eq!(e.stroke_width, Some(cm(0.1)));
        assert_eq!(e.anchor.as_deref(), Some("ctr"));
        let p = &e.paragraphs[0];
        assert_eq!(p.align.as_deref(), Some("ctr"));
        assert_eq!(p.runs[0].text, "Đậm");
        assert!(p.runs[0].bold);
        assert_eq!(p.runs[0].size, Some(2400));
        assert_eq!(p.runs[0].color.as_deref(), Some("FFFFFF"));
        assert_eq!(p.runs[0].font.as_deref(), Some("Liberation Sans"));
        assert_eq!(p.runs[1].text, " và ");
        assert!(p.runs[2].italic && p.runs[2].underline);
        // Rotated rect: inherits gr1 but switches fill/stroke off; 90° clockwise.
        let r = &s.shapes[2];
        assert_eq!(r.fill, Paint::None);
        assert_eq!(r.rotation, 90 * 60_000);
        assert_eq!((r.cx, r.cy), (cm(4.0), cm(2.0)));
        let l = &s.shapes[3];
        assert_eq!(l.geometry, "line");
        assert_eq!(
            (l.x, l.y, l.cx, l.cy),
            (cm(1.0), cm(8.0), cm(5.0) - cm(1.0), cm(9.0) - cm(8.0))
        );
        assert_eq!(
            s.shapes[4].image.as_ref().map(|i| i.1.as_str()),
            Some("png")
        );
        let o = &s.shapes[5];
        assert_eq!(o.paragraphs.len(), 2);
        assert!(o.paragraphs[0].bullet);
        assert_eq!(o.paragraphs[1].level, 1);
        assert_eq!(s.notes, vec!["Ghi chú"]);
    }

    #[test]
    fn plain_odp_still_reads() {
        let d = read_odp_deck(&crate::odp::tests::sample_bytes()).unwrap();
        assert_eq!(d.slides.len(), 2);
        assert_eq!(d.slides[0].notes, vec!["Ghi chú"]);
    }
}
