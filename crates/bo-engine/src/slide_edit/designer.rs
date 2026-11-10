//! BlueSlide design suggestions (CORE-DESIGN-001, "Gợi ý thiết kế").
//!
//! The current slide's content is analysed (title only, title and bullets, long text,
//! pictures, one chart / table / graphic, section header) and a handful of alternative
//! arrangements is computed by rules: the existing shapes are moved and resized, pictures
//! are cropped to fill their new frames without distortion, and optional accent shapes in
//! the theme's colours are added behind the content. Nothing is invented: no text, picture
//! or data is added. Every layout here is BlueOffice's own.
//!
//! Commands (`cmd`):
//! - `designIdeas { width? }` → `designIdeas`: the content kind and the suggestions, each
//!   with a thumbnail display list drawn from the slide as it would look.
//! - `designApply { idea }`: applies one suggestion as ONE undoable transaction.
//!
//! Accent shapes are named [`ACCENT_NAME`]`…`; applying another suggestion replaces them.

use super::geom::{place_slide, BoxR, Placed, Rect};
use super::ops::{node_at, Tx};
use super::{apply_box, build, next_shape_id, slide_size, EditError, Outcome, SlideEditResult, R};
use crate::{EngineError, Result};
use bo_model::picture::{fill_crop, PCT};
use bo_model::slide::{Presentation, ShapeNode};
use serde::Serialize;
use serde_json::Value;

/// Name prefix of the accent shapes a suggestion adds.
pub const ACCENT_NAME: &str = "Design Accent";

/// Default thumbnail width in points.
const THUMB_WIDTH: f32 = 192.0;

/// A design suggestions command.
#[derive(Debug, Clone, PartialEq)]
pub enum DesignCommand {
    /// Analyse the current slide and list suggestions with thumbnails `width` points wide.
    Ideas {
        /// Thumbnail width in points.
        width: Option<f64>,
    },
    /// Apply suggestion `idea` (an id from `designIdeas`) to the current slide.
    Apply {
        /// Suggestion id.
        idea: String,
    },
}

impl DesignCommand {
    /// The command in a JSON request, if it is one.
    pub fn parse(v: &Value) -> Option<DesignCommand> {
        match v.get("cmd")?.as_str()? {
            "designIdeas" => Some(DesignCommand::Ideas {
                width: v.get("width").and_then(Value::as_f64),
            }),
            "designApply" => Some(DesignCommand::Apply {
                idea: v.get("idea")?.as_str()?.to_owned(),
            }),
            _ => None,
        }
    }
}

/// What the slide holds, as far as its arrangement is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentKind {
    /// A title and nothing else.
    TitleOnly,
    /// A title slide or section header (title with a short subtitle).
    Section,
    /// A title with a few paragraphs.
    Bullets,
    /// A title with much text.
    LongText,
    /// Pictures (and other graphics side by side), with optional title and text.
    Pictures,
    /// One chart, table or graphic, with optional title and text.
    Visual,
}

/// One shape the suggestions arrange.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// `cNvPr@id`.
    pub id: u32,
    /// Its current box on the slide (points).
    pub rect: Rect,
    /// Width / height of the uncropped picture (pictures) or of the current box.
    pub aspect: f64,
    /// A picture (cropped to fill its frame).
    pub picture: bool,
    /// Characters of text (text shapes).
    pub chars: usize,
}

/// The analysed content of a slide.
#[derive(Debug, Clone, PartialEq)]
pub struct Content {
    /// Content kind.
    pub kind: ContentKind,
    /// The title placeholder with text.
    pub title: Option<Item>,
    /// Other shapes with text (subtitle, body, text boxes), top to bottom.
    pub texts: Vec<Item>,
    /// Pictures, charts, tables and other graphics, left to right and top to bottom.
    pub media: Vec<Item>,
    /// Paragraphs of the non-title text.
    pub paragraphs: usize,
}

/// A coloured accent shape behind the content.
#[derive(Debug, Clone, PartialEq)]
pub struct Accent {
    /// Box in slide points.
    pub rect: Rect,
    /// Theme colour (§20.1.10.54 ST_SchemeColorVal).
    pub color: &'static str,
    /// Opacity in 1/1000 % (§20.1.2.3.1 alpha); `None` = opaque.
    pub alpha: Option<i32>,
}

/// One suggestion: new boxes of the shapes, picture crops and accents.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Stable id (`kind.name`).
    pub id: String,
    /// New box of each arranged shape.
    pub places: Vec<(u32, Rect)>,
    /// New `srcRect` (l, t, r, b in 1/1000 %) of pictures.
    pub crops: Vec<(u32, [i32; 4])>,
    /// Accent shapes, back to front.
    pub accents: Vec<Accent>,
}

/// A suggestion as seen by the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaOut {
    /// Suggestion id for `designApply`.
    pub id: String,
    /// Thumbnail display list of the slide with the suggestion applied.
    pub page: Value,
}

/// Answer of `designIdeas`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignIdeasOut {
    /// Content kind of the slide (`None`: nothing to arrange).
    pub kind: Option<ContentKind>,
    /// The suggestions (4–8 when the slide has content).
    pub ideas: Vec<IdeaOut>,
}

fn is_accent(n: &ShapeNode) -> bool {
    n.nv().is_some_and(|nv| nv.name.starts_with(ACCENT_NAME))
}

/// Uncropped aspect ratio of a picture from its frame and `srcRect`.
fn picture_aspect(rect: &Rect, crop: Option<[i32; 4]>) -> f64 {
    let [l, t, r, b] = crop.unwrap_or([0; 4]).map(f64::from);
    let w = rect.w / ((PCT - l - r) / PCT).max(1e-3);
    let h = rect.h / ((PCT - t - b) / PCT).max(1e-3);
    if h > 0.0 {
        w / h
    } else {
        1.0
    }
}

/// Pixel aspect of the picture part of `pic`, when it can be read.
fn pixel_aspect(pres: &Presentation, slide: usize, pic: &bo_model::slide::Picture) -> Option<f64> {
    let part = super::picture::rel_part(pres, slide, pic.blip.embed.as_deref()?)?;
    let (w, h) = crate::edit::image_size(pres.part_data(&part)?)?;
    (w > 0 && h > 0).then(|| f64::from(w) / f64::from(h))
}

fn layout_kind(pres: &Presentation, slide: usize) -> Option<&str> {
    let s = pres.slides.get(slide)?;
    pres.layouts.get(s.layout)?.kind.as_deref()
}

/// Analyse the top-level shapes of `slide`; `None` when there is nothing to arrange.
pub fn analyse(pres: &Presentation, slide: usize) -> Option<Content> {
    let placed = place_slide(pres, slide);
    let mut title = None;
    let mut ctr_title = false;
    let mut subtitle = false;
    let mut texts = Vec::new();
    let mut media = Vec::new();
    let mut paragraphs = 0;
    for p in &placed {
        let Some(n) = node_at(pres, slide, &p.path) else {
            continue;
        };
        if is_accent(n) {
            continue;
        }
        let rect = p.b.bounds();
        let aspect = if rect.h > 0.0 { rect.w / rect.h } else { 1.0 };
        match n {
            ShapeNode::Shape(s) => {
                let body = s.text.as_ref();
                let text = body
                    .map(bo_model::slide::TextBody::text)
                    .unwrap_or_default();
                let chars = text.trim().chars().count();
                if chars == 0 {
                    // Empty placeholders and plain decorations stay where they are.
                    continue;
                }
                let ph = s.nv.placeholder.as_ref().map(|ph| ph.kind.as_deref());
                let item = Item {
                    id: p.id,
                    rect,
                    aspect,
                    picture: false,
                    chars,
                };
                // §19.7.10 ST_PlaceholderType: title / ctrTitle / subTitle.
                match ph {
                    Some(Some("title" | "ctrTitle")) if title.is_none() => {
                        ctr_title = ph == Some(Some("ctrTitle"));
                        title = Some(item);
                    }
                    _ => {
                        subtitle |= ph == Some(Some("subTitle"));
                        paragraphs += body.map_or(0, |b| {
                            b.paragraphs
                                .iter()
                                .filter(|para| !para.text().trim().is_empty())
                                .count()
                        });
                        texts.push(item);
                    }
                }
            }
            ShapeNode::Picture(pic) => {
                let aspect = pixel_aspect(pres, slide, pic)
                    .unwrap_or_else(|| picture_aspect(&rect, pic.blip.src_rect));
                media.push(Item {
                    id: p.id,
                    rect,
                    aspect,
                    picture: true,
                    chars: 0,
                });
            }
            ShapeNode::Frame(_) | ShapeNode::Group(_) => media.push(Item {
                id: p.id,
                rect,
                aspect,
                picture: false,
                chars: 0,
            }),
            ShapeNode::Connector(_) | ShapeNode::Other(_) => {}
        }
    }
    if title.is_none() && texts.is_empty() && media.is_empty() {
        return None;
    }
    texts.sort_by(|a, b| a.rect.y.total_cmp(&b.rect.y));
    media.sort_by(|a, b| {
        (a.rect.y + a.rect.h / 2.0)
            .total_cmp(&(b.rect.y + b.rect.h / 2.0))
            .then(a.rect.x.total_cmp(&b.rect.x))
    });
    let chars: usize = texts.iter().map(|t| t.chars).sum();
    let section_layout = matches!(layout_kind(pres, slide), Some("title" | "secHead"));
    let kind = if media.iter().any(|m| m.picture) || media.len() > 1 {
        ContentKind::Pictures
    } else if media.len() == 1 {
        ContentKind::Visual
    } else if texts.is_empty() {
        if section_layout || ctr_title {
            ContentKind::Section
        } else {
            ContentKind::TitleOnly
        }
    } else if (section_layout || ctr_title || subtitle) && chars <= 160 && paragraphs <= 2 {
        ContentKind::Section
    } else if chars > 450 || paragraphs > 10 {
        ContentKind::LongText
    } else {
        ContentKind::Bullets
    };
    Some(Content {
        kind,
        title,
        texts,
        media,
        paragraphs,
    })
}

// ---------------------------------------------------------------------------
// Layout rules
// ---------------------------------------------------------------------------

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x,
        y,
        w: w.max(1.0),
        h: h.max(1.0),
    }
}

/// `r` shrunk by `d` on every side.
fn inset(r: Rect, d: f64) -> Rect {
    rect(r.x + d, r.y + d, r.w - 2.0 * d, r.h - 2.0 * d)
}

/// Largest box of aspect `a` centred in `r`.
fn fit(a: f64, r: Rect) -> Rect {
    let a = if a.is_finite() && a > 0.0 { a } else { 1.0 };
    let (w, h) = if r.w / r.h > a {
        (r.h * a, r.h)
    } else {
        (r.w, r.w / a)
    };
    rect(r.x + (r.w - w) / 2.0, r.y + (r.h - h) / 2.0, w, h)
}

/// Cells of a grid of `n` boxes in `r` with gaps `gap`, row by row.
fn grid(n: usize, r: Rect, gap: f64) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    let cols = if n <= 3 {
        n
    } else {
        (n as f64).sqrt().ceil() as usize
    };
    let rows = n.div_ceil(cols);
    let cw = (r.w - gap * (cols - 1) as f64) / cols as f64;
    let ch = (r.h - gap * (rows - 1) as f64) / rows as f64;
    (0..n)
        .map(|i| {
            let (row, col) = (i / cols, i % cols);
            // The last row's cells are centred when it is not full.
            let in_row = if row == rows - 1 {
                n - row * cols
            } else {
                cols
            };
            let shift = (cols - in_row) as f64 * (cw + gap) / 2.0;
            rect(
                r.x + shift + col as f64 * (cw + gap),
                r.y + row as f64 * (ch + gap),
                cw,
                ch,
            )
        })
        .collect()
}

/// Boxes of shapes stacked top to bottom in `r`, heights in proportion to their text.
fn stack(items: &[Item], r: Rect, gap: f64) -> Vec<(u32, Rect)> {
    if items.is_empty() {
        return Vec::new();
    }
    let weights: Vec<f64> = items.iter().map(|i| (i.chars.max(40)) as f64).collect();
    let total: f64 = weights.iter().sum();
    let free = r.h - gap * (items.len() - 1) as f64;
    let mut y = r.y;
    items
        .iter()
        .zip(weights)
        .map(|(it, w)| {
            let h = free * w / total;
            let out = (it.id, rect(r.x, y, r.w, h));
            y += h + gap;
            out
        })
        .collect()
}

/// Geometry shared by the rules of one slide.
struct Frame {
    sw: f64,
    sh: f64,
    /// Margin around the content.
    m: f64,
    /// Gap between shapes.
    g: f64,
}

impl Frame {
    fn new(sw: f64, sh: f64) -> Frame {
        let base = sw.min(sh);
        Frame {
            sw,
            sh,
            m: base * 0.07,
            g: base * 0.03,
        }
    }
    /// Content width inside the margins.
    fn w(&self) -> f64 {
        self.sw - 2.0 * self.m
    }
}

struct Builder<'a> {
    c: &'a Content,
    plan: Plan,
}

impl<'a> Builder<'a> {
    fn new(c: &'a Content, id: &str) -> Builder<'a> {
        Builder {
            c,
            plan: Plan {
                id: id.to_owned(),
                places: Vec::new(),
                crops: Vec::new(),
                accents: Vec::new(),
            },
        }
    }
    fn accent(&mut self, r: Rect, color: &'static str, alpha: Option<i32>) -> &mut Self {
        self.plan.accents.push(Accent {
            rect: r,
            color,
            alpha,
        });
        self
    }
    fn title(&mut self, r: Rect) -> &mut Self {
        if let Some(t) = &self.c.title {
            self.plan.places.push((t.id, r));
        }
        self
    }
    fn texts(&mut self, r: Rect, gap: f64) -> &mut Self {
        self.plan.places.extend(stack(&self.c.texts, r, gap));
        self
    }
    /// The title in `t` and the texts in `b`; without texts the title takes both, without a
    /// title the texts do.
    fn title_texts(&mut self, t: Rect, b: Rect, gap: f64) -> &mut Self {
        match (&self.c.title, self.c.texts.is_empty()) {
            (Some(_), false) => self.title(t).texts(b, gap),
            (Some(_), true) => self.title(t),
            (None, _) => self.texts(t.union(&b), gap),
        }
    }
    /// A media item filling `r`: pictures are cropped to its aspect.
    fn fill(&mut self, it: &Item, r: Rect) -> &mut Self {
        self.plan.places.push((it.id, r));
        if it.picture {
            self.plan
                .crops
                .push((it.id, fill_crop(it.aspect, 1.0, r.w, r.h)));
        }
        self
    }
    /// A media item fitted in `r` without cropping.
    fn fit_in(&mut self, it: &Item, r: Rect) -> &mut Self {
        let b = fit(it.aspect, r);
        self.plan.places.push((it.id, b));
        if it.picture {
            self.plan.crops.push((it.id, [0; 4]));
        }
        self
    }
    fn grid_fill(&mut self, r: Rect, gap: f64) -> &mut Self {
        let items = self.c.media.clone();
        for (it, cell) in items.iter().zip(grid(items.len(), r, gap)) {
            self.fill(it, cell);
        }
        self
    }
    fn done(&mut self) -> Plan {
        std::mem::replace(
            &mut self.plan,
            Plan {
                id: String::new(),
                places: Vec::new(),
                crops: Vec::new(),
                accents: Vec::new(),
            },
        )
    }
}

fn title_ideas(c: &Content, f: &Frame, prefix: &str) -> Vec<Plan> {
    let (sw, sh, m, g) = (f.sw, f.sh, f.m, f.g);
    let id = |n: &str| format!("{prefix}.{n}");
    vec![
        // Centred title with a short accent rule under it.
        Builder::new(c, &id("centered"))
            .accent(
                rect(sw / 2.0 - sw * 0.08, sh * 0.555, sw * 0.16, sh * 0.012),
                "accent1",
                None,
            )
            .title_texts(
                rect(2.0 * m, sh * 0.28, sw - 4.0 * m, sh * 0.26),
                rect(2.0 * m, sh * 0.6, sw - 4.0 * m, sh * 0.18),
                g,
            )
            .done(),
        // A solid band on the left, the words on the right.
        Builder::new(c, &id("leftBand"))
            .accent(rect(0.0, 0.0, sw * 0.34, sh), "accent1", None)
            .title_texts(
                rect(sw * 0.39, sh * 0.24, sw * 0.61 - m, sh * 0.3),
                rect(sw * 0.39, sh * 0.58, sw * 0.61 - m, sh * 0.2),
                g,
            )
            .done(),
        // Title low on the left above a bar along the bottom edge.
        Builder::new(c, &id("bottomBar"))
            .accent(rect(0.0, sh * 0.93, sw, sh * 0.07), "accent2", None)
            .accent(rect(m, m, sh * 0.08, sh * 0.08), "accent1", None)
            .title_texts(
                rect(m, sh * 0.48, sw * 0.72, sh * 0.24),
                rect(m, sh * 0.73, sw * 0.72, sh * 0.15),
                g,
            )
            .done(),
        // A light tinted frame around a centred title.
        Builder::new(c, &id("tintFrame"))
            .accent(
                rect(m / 2.0, m / 2.0, sw - m, sh - m),
                "accent1",
                Some(15_000),
            )
            .title_texts(
                rect(2.0 * m, sh * 0.3, sw - 4.0 * m, sh * 0.22),
                rect(2.0 * m, sh * 0.55, sw - 4.0 * m, sh * 0.16),
                g,
            )
            .done(),
        // A band on the right, the words on the left.
        Builder::new(c, &id("rightBand"))
            .accent(rect(sw * 0.8, 0.0, sw * 0.2, sh), "accent2", None)
            .title_texts(
                rect(m, sh * 0.28, sw * 0.8 - 2.0 * m, sh * 0.26),
                rect(m, sh * 0.58, sw * 0.8 - 2.0 * m, sh * 0.18),
                g,
            )
            .done(),
    ]
}

fn text_ideas(c: &Content, f: &Frame, prefix: &str, title_h: f64) -> Vec<Plan> {
    let (sw, sh, m, g) = (f.sw, f.sh, f.m, f.g);
    let w = f.w();
    let th = sh * title_h;
    let id = |n: &str| format!("{prefix}.{n}");
    let below = |top: f64| rect(m, top, w, sh - m - top);
    vec![
        // Title on top, a short accent rule, the text below.
        Builder::new(c, &id("classic"))
            .accent(
                rect(m, m + th + g * 0.4, sw * 0.12, sh * 0.008),
                "accent1",
                None,
            )
            .title_texts(rect(m, m, w, th), below(m + th + g * 1.4), g)
            .done(),
        // The title in a column on the left, a thin rule, the text on the right.
        Builder::new(c, &id("sideTitle"))
            .accent(
                rect(sw * 0.335, m, (sw * 0.004).max(1.5), sh - 2.0 * m),
                "accent1",
                None,
            )
            .title_texts(
                rect(m, m, sw * 0.3 - m, sh - 2.0 * m),
                rect(sw * 0.37, m, sw * 0.63 - m, sh - 2.0 * m),
                g,
            )
            .done(),
        // The text on a light tinted panel under the title.
        Builder::new(c, &id("panel"))
            .accent(below(m + th + g), "accent1", Some(12_000))
            .title_texts(rect(m, m, w, th), inset(below(m + th + g), g * 1.2), g)
            .done(),
        // A coloured strip down the left edge.
        Builder::new(c, &id("strip"))
            .accent(rect(0.0, 0.0, sw * 0.035, sh), "accent1", None)
            .title_texts(
                rect(m + sw * 0.035, m, w - sw * 0.035, th),
                rect(
                    m + sw * 0.035,
                    m + th + g,
                    w - sw * 0.035,
                    sh - 2.0 * m - th - g,
                ),
                g,
            )
            .done(),
        // A tinted header band holding the title.
        Builder::new(c, &id("header"))
            .accent(rect(0.0, 0.0, sw, m + th + g), "accent1", Some(20_000))
            .title_texts(
                rect(m, m * 0.6, w, th + m * 0.4),
                below(m + th + 2.0 * g),
                g,
            )
            .done(),
    ]
}

fn picture_ideas(c: &Content, f: &Frame) -> Vec<Plan> {
    let (sw, sh, m, g) = (f.sw, f.sh, f.m, f.g);
    let w = f.w();
    let words = c.title.is_some() || !c.texts.is_empty();
    let th = sh * 0.15;
    if let [one] = c.media.as_slice() {
        let half = |x: f64| rect(x + m, m, sw * 0.5 - 2.0 * m, sh * 0.2);
        let half_body = |x: f64| {
            rect(
                x + m,
                m + sh * 0.2 + g,
                sw * 0.5 - 2.0 * m,
                sh * 0.8 - 2.0 * m - g,
            )
        };
        let mut v = vec![
            // Full-bleed picture on the right half, words on the left.
            Builder::new(c, "pictures.right")
                .fill(one, rect(sw * 0.5, 0.0, sw * 0.5, sh))
                .title_texts(half(0.0), half_body(0.0), g)
                .done(),
            // The same, mirrored.
            Builder::new(c, "pictures.left")
                .fill(one, rect(0.0, 0.0, sw * 0.5, sh))
                .title_texts(half(sw * 0.5), half_body(sw * 0.5), g)
                .done(),
            // Picture across the top, words below.
            Builder::new(c, "pictures.top")
                .fill(one, rect(0.0, 0.0, sw, if words { sh * 0.6 } else { sh }))
                .accent(rect(m, sh * 0.62, sw * 0.1, sh * 0.008), "accent1", None)
                .title_texts(
                    rect(m, sh * 0.64, w, sh * 0.13),
                    rect(m, sh * 0.78, w, sh * 0.22 - m),
                    g,
                )
                .done(),
            // The whole picture, framed on a tinted panel under the title.
            Builder::new(c, "pictures.framed")
                .accent(
                    rect(0.0, m + th + g, sw, sh - m - th - g),
                    "accent1",
                    Some(12_000),
                )
                .fit_in(
                    one,
                    inset(rect(0.0, m + th + g, sw, sh - m - th - g), g * 1.5),
                )
                .title_texts(rect(m, m * 0.7, w, th), rect(m, m * 0.7, w, th), g)
                .done(),
        ];
        if words {
            // A narrow picture column on the right third, words in the rest.
            v.push(
                Builder::new(c, "pictures.third")
                    .fill(one, rect(sw * 0.64, m, sw * 0.36 - m, sh - 2.0 * m))
                    .accent(rect(0.0, 0.0, sw * 0.02, sh), "accent2", None)
                    .title_texts(
                        rect(m, m, sw * 0.6 - m, sh * 0.2),
                        rect(m, m + sh * 0.2 + g, sw * 0.6 - m, sh * 0.8 - 2.0 * m - g),
                        g,
                    )
                    .done(),
            );
        }
        return v;
    }
    let body_top = if words { m + th + g } else { m };
    let mut v = vec![
        // A grid of equal cells under the title.
        Builder::new(c, "pictures.grid")
            .title(rect(m, m, w, th))
            .texts(rect(m, sh - m - sh * 0.12, w, sh * 0.12), g)
            .grid_fill(
                rect(
                    m,
                    body_top,
                    w,
                    sh - m
                        - body_top
                        - if c.texts.is_empty() {
                            0.0
                        } else {
                            sh * 0.12 + g
                        },
                ),
                g,
            )
            .done(),
        // Edge-to-edge mosaic under a tinted title band.
        Builder::new(c, "pictures.mosaic")
            .accent(rect(0.0, 0.0, sw, body_top), "accent1", Some(18_000))
            .title(rect(m, m * 0.5, w, th))
            .texts(rect(m, sh * 0.86, w, sh * 0.14 - m * 0.3), g)
            .grid_fill(
                rect(
                    0.0,
                    body_top,
                    sw,
                    sh - body_top - if c.texts.is_empty() { 0.0 } else { sh * 0.14 },
                ),
                sh * 0.006,
            )
            .done(),
        // Words on the left, the pictures in a grid on the right.
        Builder::new(c, "pictures.side")
            .title_texts(
                rect(m, m, sw * 0.34 - m, sh * 0.25),
                rect(m, m + sh * 0.25 + g, sw * 0.34 - m, sh * 0.75 - 2.0 * m - g),
                g,
            )
            .grid_fill(
                if words {
                    rect(sw * 0.38, m, sw * 0.62 - m, sh - 2.0 * m)
                } else {
                    rect(m, m, w, sh - 2.0 * m)
                },
                g,
            )
            .done(),
    ];
    // One featured picture, the others in a column beside it.
    let mut b = Builder::new(c, "pictures.featured");
    b.title(rect(m, m, w, th))
        .texts(rect(m, sh - m - sh * 0.1, w, sh * 0.1), g);
    let bottom = sh
        - m
        - if c.texts.is_empty() {
            0.0
        } else {
            sh * 0.1 + g
        };
    let area = rect(m, body_top, w, bottom - body_top);
    if let Some((first, rest)) = c.media.split_first() {
        b.fill(first, rect(area.x, area.y, area.w * 0.62, area.h));
        let col = rect(
            area.x + area.w * 0.62 + g,
            area.y,
            area.w * 0.38 - g,
            area.h,
        );
        let n = rest.len();
        let ch = (col.h - g * n.saturating_sub(1) as f64) / n.max(1) as f64;
        for (i, it) in rest.iter().enumerate() {
            b.fill(it, rect(col.x, col.y + i as f64 * (ch + g), col.w, ch));
        }
    }
    v.push(b.done());
    v
}

fn visual_ideas(c: &Content, f: &Frame) -> Vec<Plan> {
    let (sw, sh, m, g) = (f.sw, f.sh, f.m, f.g);
    let w = f.w();
    let th = sh * 0.15;
    let Some(vis) = c.media.first() else {
        return Vec::new();
    };
    let words = c.title.is_some() || !c.texts.is_empty();
    let top = if c.title.is_some() { m + th + g } else { m };
    let text_h = if c.texts.is_empty() { 0.0 } else { sh * 0.12 };
    let side = |x: f64| {
        (
            rect(x, m, sw * 0.36 - m, sh * 0.22),
            rect(x, m + sh * 0.22 + g, sw * 0.36 - m, sh * 0.78 - 2.0 * m - g),
        )
    };
    let (lt, lb) = side(m);
    let (rt, rb) = side(sw * 0.64);
    vec![
        // Title on top, the visual large below.
        Builder::new(c, "visual.below")
            .title(rect(m, m, w, th))
            .texts(rect(m, top, w, text_h.max(1.0)), g)
            .fill(
                vis,
                rect(
                    m,
                    top + if text_h > 0.0 { text_h + g } else { 0.0 },
                    w,
                    sh - m - top - if text_h > 0.0 { text_h + g } else { 0.0 },
                ),
            )
            .done(),
        // Words on the left, the visual on the right.
        Builder::new(c, "visual.right")
            .accent(
                rect(sw * 0.355, m, (sw * 0.004).max(1.5), sh - 2.0 * m),
                "accent1",
                None,
            )
            .title_texts(lt, lb, g)
            .fill(
                vis,
                if words {
                    rect(sw * 0.39, m, sw * 0.61 - m, sh - 2.0 * m)
                } else {
                    rect(m, m, w, sh - 2.0 * m)
                },
            )
            .done(),
        // The visual on the left, the words on the right.
        Builder::new(c, "visual.left")
            .title_texts(rt, rb, g)
            .fill(
                vis,
                if words {
                    rect(m, m, sw * 0.6 - m, sh - 2.0 * m)
                } else {
                    rect(m, m, w, sh - 2.0 * m)
                },
            )
            .done(),
        // The visual on a tinted panel under the title, its proportions kept.
        Builder::new(c, "visual.panel")
            .accent(rect(0.0, top, sw, sh - top), "accent1", Some(12_000))
            .title(rect(m, m, w, th))
            .texts(rect(m, sh - m - text_h.max(1.0), w, text_h.max(1.0)), g)
            .fit_in(
                vis,
                inset(
                    rect(
                        0.0,
                        top,
                        sw,
                        sh - top - if text_h > 0.0 { text_h + m } else { 0.0 },
                    ),
                    g * 1.5,
                ),
            )
            .done(),
    ]
}

/// The suggestions for `content` on a `sw` × `sh` point slide.
pub fn plans(content: &Content, sw: f64, sh: f64) -> Vec<Plan> {
    let f = Frame::new(sw, sh);
    let mut v = match content.kind {
        ContentKind::TitleOnly => title_ideas(content, &f, "titleOnly"),
        ContentKind::Section => title_ideas(content, &f, "section"),
        ContentKind::Bullets => text_ideas(content, &f, "bullets", 0.16),
        ContentKind::LongText => text_ideas(content, &f, "longText", 0.11),
        ContentKind::Pictures => picture_ideas(content, &f),
        ContentKind::Visual => visual_ideas(content, &f),
    };
    v.truncate(8);
    v
}

// ---------------------------------------------------------------------------
// Applying a suggestion
// ---------------------------------------------------------------------------

/// §19.3.1.43 sp: a rectangle filled with a theme colour, no outline, no text.
fn accent_node(id: u32, n: usize, a: &Accent) -> Option<ShapeNode> {
    let emu = |v: f64| (v * super::geom::EMU_PT).round() as i64;
    let alpha = a
        .alpha
        .map(|v| format!(r#"<a:alpha val="{v}"/>"#))
        .unwrap_or_default();
    build::node(&format!(
        concat!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name} {n}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>"#,
            r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#,
            r#"<a:solidFill><a:schemeClr val="{color}">{alpha}</a:schemeClr></a:solidFill>"#,
            r#"<a:ln><a:noFill/></a:ln></p:spPr></p:sp>"#
        ),
        id = id,
        name = ACCENT_NAME,
        n = n,
        x = emu(a.rect.x),
        y = emu(a.rect.y),
        cx = emu(a.rect.w).max(1),
        cy = emu(a.rect.h).max(1),
        color = a.color,
        alpha = alpha,
    ))
}

/// Paths of the accent shapes of earlier suggestions, last first.
fn accent_paths(pres: &Presentation, slide: usize) -> Vec<Vec<usize>> {
    let mut v: Vec<Vec<usize>> = pres
        .slides
        .get(slide)
        .map(|s| {
            s.data
                .shapes
                .iter()
                .enumerate()
                .filter(|(_, n)| is_accent(n))
                .map(|(i, _)| vec![i])
                .collect()
        })
        .unwrap_or_default();
    v.reverse();
    v
}

/// Apply `plan` to `slide` inside a transaction.
pub fn apply_plan(tx: &mut Tx<'_>, slide: usize, plan: &Plan) -> R<()> {
    for p in accent_paths(tx.pres, slide) {
        tx.remove_node(slide, &p)?;
    }
    let placed = place_slide(tx.pres, slide);
    for (id, r) in &plan.places {
        let Some(p): Option<&Placed> = placed.iter().find(|p| p.id == *id) else {
            continue;
        };
        let mut n = node_at(tx.pres, slide, &p.path)
            .cloned()
            .ok_or(EditError::Refused("noShape"))?;
        let b = BoxR {
            rot: p.b.rot,
            ..BoxR::from_rect(*r)
        };
        apply_box(tx.pres, slide, &mut n, p, &b);
        if let (ShapeNode::Picture(pic), Some((_, crop))) =
            (&mut n, plan.crops.iter().find(|(c, _)| c == id))
        {
            // §20.1.8.55 srcRect: the visible part of the picture.
            pic.blip.src_rect = crop.iter().any(|v| *v != 0).then_some(*crop);
        }
        tx.replace_node(slide, &p.path, n)?;
    }
    let first = next_shape_id(tx.pres, slide);
    let mut nodes = Vec::new();
    for (id, (i, a)) in (first..).zip(plan.accents.iter().enumerate()) {
        nodes.push(accent_node(id, i + 1, a).ok_or(EditError::Refused("build"))?);
    }
    if !nodes.is_empty() {
        // Accents go to the back of the shape tree (§19.3.1.45: earlier siblings are below).
        tx.insert_nodes(slide, &[], 0, nodes)?;
    }
    Ok(())
}

impl crate::Engine {
    /// Run a design suggestions command on the current slide.
    pub fn slide_design_command(&mut self, cmd: DesignCommand) -> Result<SlideEditResult> {
        let mut out = Outcome::default();
        let mut ideas_out = None;
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            // Asking for ideas keeps text editing going (the pane follows the typing).
            if matches!(cmd, DesignCommand::Apply { .. }) {
                ed.text = None;
                ed.table = None;
            }
            let slide = ed.sel.slide;
            let (sw, sh) = slide_size(pres);
            match cmd {
                DesignCommand::Ideas { width } => {
                    let content = if ed.in_any_master_view() {
                        None
                    } else {
                        analyse(pres, slide)
                    };
                    let mut ideas = Vec::new();
                    if let Some(c) = &content {
                        let width = width.map_or(THUMB_WIDTH, |w| w.clamp(48.0, 640.0) as f32);
                        for plan in plans(c, sw, sh) {
                            let mut tx = Tx::new(pres);
                            let page = match apply_plan(&mut tx, slide, &plan) {
                                Ok(()) => bo_pptx::render_thumbnail(
                                    tx.pres,
                                    slide,
                                    &mut self.fonts,
                                    &mut self.images,
                                    width,
                                )
                                .ok(),
                                Err(_) => None,
                            };
                            tx.rollback();
                            if let Some(page) = page {
                                ideas.push(IdeaOut {
                                    id: plan.id,
                                    page: serde_json::from_str(&page.to_json())
                                        .unwrap_or(Value::Null),
                                });
                            }
                        }
                    }
                    ideas_out = Some(DesignIdeasOut {
                        kind: content.map(|c| c.kind),
                        ideas,
                    });
                }
                DesignCommand::Apply { idea } => {
                    if ed.in_any_master_view() {
                        out.error = Some("masterView".into());
                    } else {
                        ed.mutate(pres, &mut out, |tx, sel| {
                            let c = analyse(tx.pres, sel.slide)
                                .ok_or(EditError::Refused("nothingToDesign"))?;
                            let plan = plans(&c, sw, sh)
                                .into_iter()
                                .find(|p| p.id == idea)
                                .ok_or(EditError::Refused("noDesignIdea"))?;
                            apply_plan(tx, sel.slide, &plan)?;
                            sel.ids.clear();
                            Ok(())
                        });
                    }
                }
            }
        }
        let mut r = self.finish_slide_command(out)?;
        r.design_ideas = ideas_out;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u32, chars: usize, picture: bool, aspect: f64) -> Item {
        Item {
            id,
            rect: rect(0.0, 0.0, 100.0, 100.0),
            aspect,
            picture,
            chars,
        }
    }

    #[test]
    fn grid_cells_stay_inside_and_do_not_overlap() {
        let area = rect(10.0, 20.0, 600.0, 300.0);
        for n in 1..=7 {
            let cells = grid(n, area, 8.0);
            assert_eq!(cells.len(), n);
            for (i, a) in cells.iter().enumerate() {
                assert!(a.x >= area.x - 1e-6 && a.right() <= area.right() + 1e-6);
                assert!(a.y >= area.y - 1e-6 && a.bottom() <= area.bottom() + 1e-6);
                for b in &cells[i + 1..] {
                    let overlap = a.x < b.right() - 1e-6
                        && b.x < a.right() - 1e-6
                        && a.y < b.bottom() - 1e-6
                        && b.y < a.bottom() - 1e-6;
                    assert!(!overlap, "{n} cells: {a:?} overlaps {b:?}");
                }
            }
        }
    }

    #[test]
    fn fit_keeps_the_aspect() {
        let r = fit(2.0, rect(0.0, 0.0, 100.0, 100.0));
        assert!((r.w / r.h - 2.0).abs() < 1e-9);
        assert!((r.w - 100.0).abs() < 1e-9);
        assert!((r.y - 25.0).abs() < 1e-9);
    }

    #[test]
    fn uncropped_aspect_of_a_cropped_picture() {
        // 25 % cut from left and right: the visible half of a 2:1 picture is square.
        let a = picture_aspect(&rect(0.0, 0.0, 100.0, 100.0), Some([25_000, 0, 25_000, 0]));
        assert!((a - 2.0).abs() < 1e-9);
    }

    #[test]
    fn every_kind_gets_four_to_eight_ideas_inside_the_slide() {
        let (sw, sh) = (960.0, 540.0);
        let t = Some(item(2, 20, false, 1.0));
        let cases = [
            (ContentKind::TitleOnly, vec![], vec![]),
            (ContentKind::Section, vec![item(3, 30, false, 1.0)], vec![]),
            (ContentKind::Bullets, vec![item(3, 200, false, 1.0)], vec![]),
            (
                ContentKind::LongText,
                vec![item(3, 900, false, 1.0)],
                vec![],
            ),
            (ContentKind::Pictures, vec![], vec![item(4, 0, true, 1.5)]),
            (
                ContentKind::Pictures,
                vec![item(3, 50, false, 1.0)],
                (4..8).map(|i| item(i, 0, true, 0.75)).collect(),
            ),
            (
                ContentKind::Visual,
                vec![item(3, 80, false, 1.0)],
                vec![item(4, 0, false, 1.6)],
            ),
        ];
        for (kind, texts, media) in cases {
            let c = Content {
                kind,
                title: t.clone(),
                texts,
                media,
                paragraphs: 2,
            };
            let ps = plans(&c, sw, sh);
            assert!((4..=8).contains(&ps.len()), "{kind:?}: {} ideas", ps.len());
            let mut ids: Vec<&str> = ps.iter().map(|p| p.id.as_str()).collect();
            ids.dedup();
            assert_eq!(ids.len(), ps.len(), "{kind:?}: ids are unique");
            for p in &ps {
                // Every shape of the slide is placed, inside the slide.
                let mut want: Vec<u32> = c
                    .title
                    .iter()
                    .chain(&c.texts)
                    .chain(&c.media)
                    .map(|i| i.id)
                    .collect();
                let mut got: Vec<u32> = p.places.iter().map(|(id, _)| *id).collect();
                want.sort_unstable();
                got.sort_unstable();
                assert_eq!(got, want, "{}", p.id);
                for (_, r) in p.places.iter().chain(
                    p.accents
                        .iter()
                        .map(|a| (0, a.rect))
                        .collect::<Vec<_>>()
                        .iter(),
                ) {
                    assert!(r.x >= -1e-6 && r.y >= -1e-6, "{}: {r:?}", p.id);
                    assert!(
                        r.right() <= sw + 1e-6 && r.bottom() <= sh + 1e-6,
                        "{}: {r:?}",
                        p.id
                    );
                    assert!(r.w >= 1.0 && r.h >= 1.0);
                }
                // Cropped pictures keep their proportions: the visible part has the frame's aspect.
                for (id, crop) in &p.crops {
                    let it = c.media.iter().find(|m| m.id == *id).unwrap();
                    let (_, r) = p.places.iter().find(|(i, _)| i == id).unwrap();
                    let [l, t, rr, b] = crop.map(f64::from);
                    let visible = it.aspect * (PCT - l - rr) / (PCT - t - b);
                    assert!(
                        (visible - r.w / r.h).abs() < 0.01,
                        "{}: {visible} vs {}",
                        p.id,
                        r.w / r.h
                    );
                }
            }
        }
    }
}
