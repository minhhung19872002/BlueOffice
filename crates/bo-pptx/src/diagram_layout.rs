//! SmartArt without a drawing part: the diagram is laid out from its data model (ECMA-376 Part 1,
//! §21.4.3 dataModel: `dgm:pt` points and `dgm:cxn` parent-of connections) with BlueOffice's own layouts
//! for the layout categories of §21.4.2 (`dgm:cat@type`: list, process, cycle, hierarchy, matrix,
//! pyramid, relationship, picture). Each category has one or more layouts of the BlueOffice library
//! ([`Layout`]); a diagram of another producer is drawn with the first layout of its category. The category comes from the layout part's category list or, without one, the
//! layout id named on the document point (`dgm:prSet@loTypeId`).
//!
//! The layouts are drawn here from simple geometric rules (rows, chains with arrows, rings, trees,
//! grids, stacked levels, overlapping circles); they are not the layout definitions' own algorithms. The
//! result is a list of DrawingML shapes, as the drawing part would hold, so the usual shape renderer
//! draws it.

use bo_model::slide::{ShapeNode, XmlElement, XmlNode};
use std::fmt::Write;

/// One node of the diagram: its text paragraphs and its children (`parOf` connections, in order).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagramNode {
    /// Text paragraphs of the point (`dgm:t`).
    pub text: Vec<String>,
    /// Child nodes.
    pub children: Vec<DiagramNode>,
}

/// Layout category (§21.4.2.3 cat).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// Blocks in rows.
    List,
    /// Steps in a row joined by arrows.
    Process,
    /// Steps around a ring.
    Cycle,
    /// Organisation chart: a tree, root on top.
    Hierarchy,
    /// 2 × 2 (or n × n) grid.
    Matrix,
    /// Stacked levels, narrow at the top.
    Pyramid,
    /// Overlapping circles.
    Relationship,
    /// Picture frames with captions.
    Picture,
}

/// A layout of the BlueOffice library (designed here; named in `dgm:layoutDef@uniqueId` and
/// `dgm:prSet@loTypeId` as `urn:bluestar.vn/blueoffice/layout/<name>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Blocks in rows.
    List,
    /// One row per item: its title beside its sub-items.
    ListVertical,
    /// Steps in a row joined by arrows.
    Process,
    /// Steps as a row of interlocking chevrons.
    ProcessChevron,
    /// Steps top to bottom joined by arrows.
    ProcessVertical,
    /// Steps around a ring.
    Cycle,
    /// The first item in the middle, the others around it.
    CycleRadial,
    /// A tree, root on top.
    Hierarchy,
    /// A tree, root on the left.
    HierarchyHorizontal,
    /// Grid of quadrants.
    Matrix,
    /// Grid of quadrants around the first item.
    MatrixCentered,
    /// Stacked levels, narrow at the top.
    Pyramid,
    /// Stacked levels, narrow at the bottom.
    PyramidInverted,
    /// Overlapping circles.
    Relationship,
    /// Rings around the first item, labels beside them.
    RelationshipTarget,
    /// Picture frames over captions.
    Picture,
    /// One row per item: a round picture frame beside its caption.
    PictureStrip,
}

impl Layout {
    /// Every layout of the library, grouped by category.
    pub const ALL: [Layout; 17] = [
        Layout::List,
        Layout::ListVertical,
        Layout::Process,
        Layout::ProcessChevron,
        Layout::ProcessVertical,
        Layout::Cycle,
        Layout::CycleRadial,
        Layout::Hierarchy,
        Layout::HierarchyHorizontal,
        Layout::Matrix,
        Layout::MatrixCentered,
        Layout::Pyramid,
        Layout::PyramidInverted,
        Layout::Relationship,
        Layout::RelationshipTarget,
        Layout::Picture,
        Layout::PictureStrip,
    ];

    /// Name used in ids and commands (the first layout of a category has the category's name).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Layout::List => "list",
            Layout::ListVertical => "listVertical",
            Layout::Process => "process",
            Layout::ProcessChevron => "processChevron",
            Layout::ProcessVertical => "processVertical",
            Layout::Cycle => "cycle",
            Layout::CycleRadial => "cycleRadial",
            Layout::Hierarchy => "hierarchy",
            Layout::HierarchyHorizontal => "hierarchyHorizontal",
            Layout::Matrix => "matrix",
            Layout::MatrixCentered => "matrixCentered",
            Layout::Pyramid => "pyramid",
            Layout::PyramidInverted => "pyramidInverted",
            Layout::Relationship => "relationship",
            Layout::RelationshipTarget => "relationshipTarget",
            Layout::Picture => "picture",
            Layout::PictureStrip => "pictureStrip",
        }
    }

    /// The layout of a name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Layout> {
        Layout::ALL.into_iter().find(|l| l.name() == name)
    }

    /// The category of the layout (§21.4.2.3 cat).
    #[must_use]
    pub fn category(self) -> Category {
        match self {
            Layout::List | Layout::ListVertical => Category::List,
            Layout::Process | Layout::ProcessChevron | Layout::ProcessVertical => Category::Process,
            Layout::Cycle | Layout::CycleRadial => Category::Cycle,
            Layout::Hierarchy | Layout::HierarchyHorizontal => Category::Hierarchy,
            Layout::Matrix | Layout::MatrixCentered => Category::Matrix,
            Layout::Pyramid | Layout::PyramidInverted => Category::Pyramid,
            Layout::Relationship | Layout::RelationshipTarget => Category::Relationship,
            Layout::Picture | Layout::PictureStrip => Category::Picture,
        }
    }
}

impl From<Category> for Layout {
    /// The first layout of a category.
    fn from(c: Category) -> Layout {
        match c {
            Category::List => Layout::List,
            Category::Process => Layout::Process,
            Category::Cycle => Layout::Cycle,
            Category::Hierarchy => Layout::Hierarchy,
            Category::Matrix => Layout::Matrix,
            Category::Pyramid => Layout::Pyramid,
            Category::Relationship => Layout::Relationship,
            Category::Picture => Layout::Picture,
        }
    }
}

fn kids<'a>(el: &'a XmlElement, local: &'a str) -> impl Iterator<Item = &'a XmlElement> {
    el.elements().filter(move |e| e.local() == local)
}

fn kid<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    el.elements().find(|e| e.local() == local)
}

fn text_of(el: &XmlElement) -> String {
    el.children
        .iter()
        .map(|n| match n {
            XmlNode::Text(t) => t.clone(),
            XmlNode::Element(e) => text_of(e),
        })
        .collect()
}

/// Paragraph texts of a `dgm:t` text body (`a:p` / `a:r` / `a:t`).
fn paragraphs(t: &XmlElement) -> Vec<String> {
    kids(t, "p")
        .map(|p| {
            p.elements()
                .filter(|r| matches!(r.local(), "r" | "fld"))
                .filter_map(|r| kid(r, "t"))
                .map(text_of)
                .collect::<String>()
        })
        .filter(|s| !s.trim().is_empty())
        .collect()
}

/// A point of the data model (§21.4.3.8 pt): id, type and text.
struct Pt {
    id: String,
    kind: String,
    text: Vec<String>,
}

/// Content children of point `id` through `parOf` links (ordered), recursively.
fn build(
    id: &str,
    pts: &[Pt],
    links: &[(String, u32, String)],
    is_content: &dyn Fn(&str) -> bool,
    depth: usize,
) -> Vec<DiagramNode> {
    if depth > 32 {
        return Vec::new();
    }
    links
        .iter()
        .filter(|l| l.0 == id)
        .filter_map(|l| pts.iter().find(|p| p.id == l.2))
        .filter(|p| is_content(&p.kind))
        .map(|p| DiagramNode {
            text: p.text.clone(),
            children: build(&p.id, pts, links, is_content, depth + 1),
        })
        .collect()
}

/// The top-level nodes of a data model and the layout id of its document point.
#[must_use]
pub fn parse_data_model(xml: &[u8], part: &str) -> Option<(Vec<DiagramNode>, Option<String>)> {
    let root = bo_drawingml::dom::parse(xml, part).ok()?;
    if root.local() != "dataModel" {
        return None;
    }
    let pts: Vec<Pt> = kid(&root, "ptLst")
        .map(|l| {
            kids(l, "pt")
                .map(|p| Pt {
                    id: p.attr("modelId").unwrap_or_default().to_owned(),
                    kind: p.attr("type").unwrap_or("node").to_owned(),
                    text: kid(p, "t").map(paragraphs).unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    let lo_type = kid(&root, "ptLst")
        .and_then(|l| kids(l, "pt").find(|p| p.attr("type") == Some("doc")))
        .and_then(|p| kid(p, "prSet"))
        .and_then(|s| s.attr("loTypeId"))
        .map(str::to_owned);
    // parOf connections (§21.4.7.15 ST_CxnType, default parOf): parent → (order, child).
    let mut links: Vec<(String, u32, String)> = kid(&root, "cxnLst")
        .map(|l| {
            kids(l, "cxn")
                .filter(|c| c.attr("type").is_none_or(|t| t == "parOf"))
                .map(|c| {
                    (
                        c.attr("srcId").unwrap_or_default().to_owned(),
                        c.attr("srcOrd").and_then(|o| o.parse().ok()).unwrap_or(0),
                        c.attr("destId").unwrap_or_default().to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    links.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let is_content = |kind: &str| matches!(kind, "node" | "asst");
    let doc = pts.iter().find(|p| p.kind == "doc")?;
    let mut nodes = build(&doc.id, &pts, &links, &is_content, 0);
    // Points not connected to the document point (simple producers): every content point in order.
    if nodes.is_empty() {
        nodes = pts
            .iter()
            .filter(|p| is_content(&p.kind) && !links.iter().any(|l| l.2 == p.id))
            .map(|p| DiagramNode {
                text: p.text.clone(),
                children: build(&p.id, &pts, &links, &is_content, 0),
            })
            .collect();
    }
    Some((nodes, lo_type))
}

/// The layout category: the layout part's `dgm:catLst/dgm:cat@type` (the first known one), else words of
/// the layout id, else a list.
#[must_use]
pub fn category(lo_type: Option<&str>, layout_xml: Option<&[u8]>) -> Category {
    let from = |t: &str| -> Option<Category> {
        let t = t.to_ascii_lowercase();
        Some(if t.contains("hierarchy") || t.contains("orgchart") {
            Category::Hierarchy
        } else if t.contains("cycle") || t.contains("radial") {
            Category::Cycle
        } else if t.contains("process") || t.contains("chevron") || t.contains("arrow") {
            Category::Process
        } else if t.contains("matrix") {
            Category::Matrix
        } else if t.contains("pyramid") {
            Category::Pyramid
        } else if t.contains("relationship") || t.contains("venn") {
            Category::Relationship
        } else if t.contains("picture") {
            Category::Picture
        } else if t.contains("list") {
            Category::List
        } else {
            return None;
        })
    };
    if let Some(root) = layout_xml.and_then(|x| bo_drawingml::dom::parse(x, "/layout.xml").ok()) {
        if let Some(c) = kid(&root, "catLst")
            .into_iter()
            .flat_map(|l| kids(l, "cat"))
            .filter_map(|c| c.attr("type"))
            .find_map(from)
        {
            return c;
        }
        if let Some(c) = root.attr("uniqueId").and_then(from) {
            return c;
        }
    }
    lo_type.and_then(from).unwrap_or(Category::List)
}

/// The layout a diagram is drawn with: the BlueOffice layout its layout part (`layoutDef@uniqueId`)
/// or document point (`prSet@loTypeId`) names, else the first layout of its [`category`].
#[must_use]
pub fn layout_of(lo_type: Option<&str>, layout_xml: Option<&[u8]>) -> Layout {
    let own = |id: &str| id.strip_prefix(LAYOUT_ID).and_then(Layout::parse);
    let from_part = layout_xml
        .and_then(|x| bo_drawingml::dom::parse(x, "/layout.xml").ok())
        .and_then(|root| root.attr("uniqueId").and_then(own));
    from_part
        .or_else(|| lo_type.and_then(own))
        .unwrap_or_else(|| category(lo_type, layout_xml).into())
}

const LAYOUT_ID: &str = "urn:bluestar.vn/blueoffice/layout/";

/// BlueOffice diagram colours (one per top-level node, repeating).
const PALETTE: [&str; 6] = ["1F6FB2", "2E8B57", "C2571A", "7B4BA0", "B8860B", "2A9D8F"];

/// Colours of a diagram (BlueOffice palettes, named in `dgm:prSet@csTypeId` / `colorsDef@uniqueId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Palette {
    /// A different colour per node.
    #[default]
    Colorful,
    /// Every node blue.
    Blue,
    /// Every node green.
    Green,
    /// Every node orange.
    Orange,
    /// Every node purple.
    Purple,
    /// Every node grey.
    Gray,
}

/// Look of the nodes (named in `dgm:prSet@qsTypeId` / `styleDef@uniqueId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fill {
    /// Filled shapes, white text.
    #[default]
    Solid,
    /// Coloured outline, no fill, coloured text.
    Outline,
}

/// Colours and look of a diagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Look {
    /// Colours.
    pub palette: Palette,
    /// Fill or outline.
    pub fill: Fill,
}

impl Palette {
    /// Name used in ids and commands.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Palette::Colorful => "colorful",
            Palette::Blue => "blue",
            Palette::Green => "green",
            Palette::Orange => "orange",
            Palette::Purple => "purple",
            Palette::Gray => "gray",
        }
    }

    /// The palette of a name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Palette> {
        Some(match name {
            "colorful" => Palette::Colorful,
            "blue" => Palette::Blue,
            "green" => Palette::Green,
            "orange" => Palette::Orange,
            "purple" => Palette::Purple,
            "gray" => Palette::Gray,
            _ => return None,
        })
    }

    /// The colour of a node drawn `default` in the colourful palette.
    fn color(self, default: &'static str) -> &'static str {
        match self {
            Palette::Colorful => default,
            Palette::Blue => "1F6FB2",
            Palette::Green => "2E8B57",
            Palette::Orange => "C2571A",
            Palette::Purple => "7B4BA0",
            Palette::Gray => "5F6B7A",
        }
    }
}

impl Fill {
    /// Name used in ids and commands.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Fill::Solid => "fill",
            Fill::Outline => "outline",
        }
    }

    /// The look of a name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Fill> {
        Some(match name {
            "fill" => Fill::Solid,
            "outline" => Fill::Outline,
            _ => return None,
        })
    }
}

const COLORS_ID: &str = "urn:bluestar.vn/blueoffice/colors/";
const STYLE_ID: &str = "urn:bluestar.vn/blueoffice/quickstyle/";

/// The look named on the document point of a data model (defaults for other producers' ids).
#[must_use]
pub fn look_of(data_xml: &[u8], part: &str) -> Look {
    let Ok(root) = bo_drawingml::dom::parse(data_xml, part) else {
        return Look::default();
    };
    let set = kid(&root, "ptLst")
        .and_then(|l| kids(l, "pt").find(|p| p.attr("type") == Some("doc")))
        .and_then(|p| kid(p, "prSet"));
    let id = |a: &str, prefix: &str| {
        set.and_then(|s| s.attr(a))
            .and_then(|v| v.strip_prefix(prefix))
            .map(str::to_owned)
    };
    Look {
        palette: id("csTypeId", COLORS_ID)
            .and_then(|n| Palette::parse(&n))
            .unwrap_or_default(),
        fill: id("qsTypeId", STYLE_ID)
            .and_then(|n| Fill::parse(&n))
            .unwrap_or_default(),
    }
}

struct Out {
    xml: String,
    n: usize,
    look: Look,
    /// Text of the next shapes starts at the left instead of being centred (labels beside a shape).
    left: bool,
}

/// A shape: (x, y, w, h) EMU, preset geometry, fill (`None`: no fill), alpha (100000 = opaque), text,
/// text colour, rotation (60000ths of a degree).
struct Spec<'a> {
    rect: (i64, i64, i64, i64),
    prst: &'a str,
    fill: Option<&'a str>,
    alpha: u32,
    text: &'a [String],
    color: &'a str,
    rot: i64,
    /// Coloured outline (2 pt) instead of the thin white one.
    outline: Option<&'a str>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl Out {
    fn shape(&mut self, s: &Spec<'_>) {
        // Node colours follow the palette; the outline look draws them as coloured frames.
        let node = s
            .fill
            .and_then(|c| PALETTE.iter().copied().find(|p| *p == c));
        let looked;
        let s = match node {
            Some(c) => {
                let c = self.look.palette.color(c);
                looked = match self.look.fill {
                    Fill::Solid => Spec {
                        fill: Some(c),
                        ..*s
                    },
                    Fill::Outline => Spec {
                        fill: Some("FFFFFF"),
                        alpha: 100_000,
                        color: c,
                        outline: Some(c),
                        ..*s
                    },
                };
                &looked
            }
            None => s,
        };
        self.n += 1;
        let (x, y, w, h) = s.rect;
        let rot = if s.rot == 0 {
            String::new()
        } else {
            format!(r#" rot="{}""#, s.rot)
        };
        let fill = match s.fill {
            Some(c) if s.alpha < 100_000 => format!(
                r#"<a:solidFill><a:srgbClr val="{c}"><a:alpha val="{}"/></a:srgbClr></a:solidFill>"#,
                s.alpha
            ),
            Some(c) => format!(r#"<a:solidFill><a:srgbClr val="{c}"/></a:solidFill>"#),
            None => "<a:noFill/>".to_owned(),
        };
        let line = match (s.prst, s.outline) {
            ("line", _) => {
                r#"<a:ln w="19050"><a:solidFill><a:srgbClr val="7F7F7F"/></a:solidFill></a:ln>"#
                    .to_owned()
            }
            (_, Some(c)) => {
                format!(
                    r#"<a:ln w="25400"><a:solidFill><a:srgbClr val="{c}"/></a:solidFill></a:ln>"#
                )
            }
            // A text box over other shapes: no frame.
            _ if s.fill.is_none() => "<a:ln><a:noFill/></a:ln>".to_owned(),
            _ => r#"<a:ln w="12700"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:ln>"#
                .to_owned(),
        };
        let _ = write!(
            self.xml,
            r#"<dsp:sp modelId="{{B1E50FF2-0000-4000-8000-{n:012X}}}"><dsp:nvSpPr><dsp:cNvPr id="{n}" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm{rot}><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="{prst}"><a:avLst/></a:prstGeom>{fill}{line}</dsp:spPr>"#,
            n = self.n,
            prst = s.prst
        );
        if !s.text.is_empty() {
            // Text size from the box: about a quarter of its height per line, 8–20 pt.
            let lines = s.text.len().max(1) as i64;
            let pt = ((h / 12_700) / (3 * lines + 1)).clamp(8, 20);
            let _ = write!(
                self.xml,
                r#"<dsp:txBody><a:bodyPr anchor="ctr" wrap="square" lIns="45720" rIns="45720" tIns="45720" bIns="45720"><a:normAutofit/></a:bodyPr><a:lstStyle/>"#
            );
            let algn = if self.left { "l" } else { "ctr" };
            for (i, t) in s.text.iter().enumerate() {
                let size = if i == 0 { pt * 100 } else { pt * 80 };
                let _ = write!(
                    self.xml,
                    r#"<a:p><a:pPr algn="{algn}"/><a:r><a:rPr lang="vi-VN" sz="{size}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r></a:p>"#,
                    s.color,
                    esc(t)
                );
            }
            self.xml.push_str("</dsp:txBody>");
        }
        self.xml.push_str("</dsp:sp>");
    }

    /// The colour of top-level node `i` in the diagram's palette.
    fn tone(&self, i: usize) -> &'static str {
        self.look.palette.color(PALETTE[i % PALETTE.len()])
    }

    /// The colour of text drawn over node `i` by a separate text box.
    fn ink(&self, i: usize) -> &'static str {
        match self.look.fill {
            Fill::Solid => "FFFFFF",
            Fill::Outline => self.tone(i),
        }
    }

    /// Node `i` of the diagram: a shape in its palette colour with white text.
    fn node(&mut self, rect: (i64, i64, i64, i64), prst: &str, i: usize, text: &[String]) {
        self.shape(&Spec {
            rect,
            prst,
            fill: Some(PALETTE[i % PALETTE.len()]),
            alpha: 100_000,
            text,
            color: "FFFFFF",
            rot: 0,
            outline: None,
        });
    }

    /// A shape that is not a node (`fill`: `None` = a text box).
    fn plain(
        &mut self,
        rect: (i64, i64, i64, i64),
        prst: &str,
        fill: Option<&str>,
        text: &[String],
        color: &str,
    ) {
        self.shape(&Spec {
            rect,
            prst,
            fill,
            alpha: 100_000,
            text,
            color,
            rot: 0,
            outline: None,
        });
    }

    /// A picture frame of node `i`: a light box framed in the node's colour with a small landscape
    /// mark (BlueOffice's own placeholder drawing).
    fn frame(&mut self, (x, y, w, h): (i64, i64, i64, i64), prst: &str, i: usize) {
        let tone = self.tone(i);
        self.shape(&Spec {
            rect: (x, y, w, h),
            prst,
            fill: Some("E7ECF2"),
            alpha: 100_000,
            text: &[],
            color: "000000",
            rot: 0,
            outline: Some(tone),
        });
        let m = w.min(h);
        self.plain(
            (x + w / 2 - m / 4, y + h / 2 - m / 10, m / 2, m * 3 / 10),
            "triangle",
            Some("B4BFCC"),
            &[],
            "000000",
        );
        self.plain(
            (x + w / 2 + m / 10, y + h / 2 - m / 4, m / 8, m / 8),
            "ellipse",
            Some("B4BFCC"),
            &[],
            "000000",
        );
    }

    /// A straight bar from one point to another (any direction), `t` thick.
    fn bar(&mut self, (x1, y1): (f64, f64), (x2, y2): (f64, f64), t: f64) {
        let len = (x2 - x1).hypot(y2 - y1);
        let (mx, my) = (f64::midpoint(x1, x2), f64::midpoint(y1, y2));
        let deg = (y2 - y1).atan2(x2 - x1).to_degrees().rem_euclid(360.0);
        self.shape(&Spec {
            rect: (
                (mx - len / 2.0) as i64,
                (my - t / 2.0) as i64,
                len as i64,
                t as i64,
            ),
            prst: "rect",
            fill: Some("A6A6A6"),
            alpha: 100_000,
            text: &[],
            color: "000000",
            rot: (deg * 60_000.0) as i64,
            outline: None,
        });
    }

    fn line(&mut self, x1: i64, y1: i64, x2: i64, y2: i64) {
        let (x, w) = (x1.min(x2), (x2 - x1).abs());
        let (y, h) = (y1.min(y2), (y2 - y1).abs());
        self.shape(&Spec {
            rect: (x, y, w, h),
            prst: "line",
            fill: None,
            alpha: 100_000,
            text: &[],
            color: "000000",
            rot: 0,
            outline: None,
        });
    }
}

/// The text of a node with its children's texts as further paragraphs (list and process layouts).
fn with_children(n: &DiagramNode) -> Vec<String> {
    let mut t = n.text.clone();
    for c in &n.children {
        t.extend(c.text.iter().map(|s| format!("• {s}")));
    }
    t
}

/// Shapes (EMU, relative to a `w` × `h` frame) laying out `nodes` in `cat`.
#[must_use]
pub fn layout_shapes(
    nodes: &[DiagramNode],
    layout: Layout,
    look: Look,
    w: i64,
    h: i64,
) -> Vec<ShapeNode> {
    let xml = drawing_xml(nodes, layout, look, w, h);
    crate::diagram::drawing_shapes(xml.as_bytes(), "/diagram-layout.xml").unwrap_or_default()
}

/// The diagram drawing part ([MS-ODRAWXML] `dsp:drawing`) laying out `nodes` with `layout` in a
/// `w` × `h` EMU frame.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn drawing_xml(nodes: &[DiagramNode], layout: Layout, look: Look, w: i64, h: i64) -> String {
    let mut out = Out {
        xml: String::new(),
        n: 0,
        look,
        left: false,
    };
    let n = nodes.len() as i64;
    if n > 0 && w > 0 && h > 0 {
        match layout {
            Layout::ListVertical => list_vertical(&mut out, nodes, w, h),
            Layout::ProcessChevron => process_chevron(&mut out, nodes, w, h),
            Layout::ProcessVertical => process_vertical(&mut out, nodes, w, h),
            Layout::CycleRadial => cycle_radial(&mut out, nodes, w, h),
            Layout::MatrixCentered => matrix_centered(&mut out, nodes, w, h),
            Layout::PyramidInverted => pyramid_inverted(&mut out, nodes, w, h),
            Layout::RelationshipTarget => relationship_target(&mut out, nodes, w, h),
            Layout::Picture => picture_cards(&mut out, nodes, w, h),
            Layout::PictureStrip => picture_strip(&mut out, nodes, w, h),
            Layout::List => {
                // Rows of blocks, as square as the frame allows.
                let cols = ((n as f64 * w as f64 / h as f64).sqrt().round() as i64).clamp(1, n);
                let rows = (n + cols - 1) / cols;
                let (cw, ch) = (w / cols, h / rows);
                for (i, node) in nodes.iter().enumerate() {
                    let (r, c) = (i as i64 / cols, i as i64 % cols);
                    out.shape(&Spec {
                        rect: (c * cw + cw / 20, r * ch + ch / 20, cw * 9 / 10, ch * 9 / 10),
                        prst: "roundRect",
                        fill: Some(PALETTE[i % PALETTE.len()]),
                        alpha: 100_000,
                        text: &with_children(node),
                        color: "FFFFFF",
                        rot: 0,
                        outline: None,
                    });
                }
            }
            Layout::Process => {
                // Boxes 1, arrows 0.4 wide, in one row; the row is centred vertically.
                let unit = (w as f64 / (n as f64 + 0.4 * (n - 1) as f64)) as i64;
                let bh = h.min(unit * 3 / 4).max(1);
                let y = (h - bh) / 2;
                for (i, node) in nodes.iter().enumerate() {
                    let x = (i as f64 * 1.4 * unit as f64) as i64;
                    out.shape(&Spec {
                        rect: (x, y, unit, bh),
                        prst: "roundRect",
                        fill: Some(PALETTE[i % PALETTE.len()]),
                        alpha: 100_000,
                        text: &with_children(node),
                        color: "FFFFFF",
                        rot: 0,
                        outline: None,
                    });
                    if (i as i64) + 1 < n {
                        let aw = unit * 3 / 10;
                        let ah = bh.min(unit) * 2 / 5;
                        out.shape(&Spec {
                            rect: (x + unit + unit / 20, y + (bh - ah) / 2, aw, ah),
                            prst: "rightArrow",
                            fill: Some("A6A6A6"),
                            alpha: 100_000,
                            text: &[],
                            color: "000000",
                            rot: 0,
                            outline: None,
                        });
                    }
                }
            }
            Layout::Cycle => {
                // Circles evenly on a ring (first at the top, clockwise), arrows between them.
                let side = w.min(h) as f64;
                let d = (side * 0.3).min(std::f64::consts::PI * side * 0.35 / n as f64 * 1.2);
                let r = side / 2.0 - d / 2.0;
                let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
                let at = |k: f64| {
                    let a =
                        -std::f64::consts::FRAC_PI_2 + 2.0 * std::f64::consts::PI * k / n as f64;
                    (cx + r * a.cos(), cy + r * a.sin(), a)
                };
                for (i, node) in nodes.iter().enumerate() {
                    let (x, y, _) = at(i as f64);
                    out.shape(&Spec {
                        rect: (
                            (x - d / 2.0) as i64,
                            (y - d / 2.0) as i64,
                            d as i64,
                            d as i64,
                        ),
                        prst: "ellipse",
                        fill: Some(PALETTE[i % PALETTE.len()]),
                        alpha: 100_000,
                        text: &with_children(node),
                        color: "FFFFFF",
                        rot: 0,
                        outline: None,
                    });
                }
                if n > 1 {
                    for i in 0..n {
                        let (x, y, a) = at(i as f64 + 0.5);
                        let (aw, ah) = (d * 0.35, d * 0.2);
                        // Pointing along the ring (tangent, clockwise).
                        let deg = (a + std::f64::consts::FRAC_PI_2).to_degrees();
                        out.shape(&Spec {
                            rect: (
                                (x - aw / 2.0) as i64,
                                (y - ah / 2.0) as i64,
                                aw as i64,
                                ah as i64,
                            ),
                            prst: "rightArrow",
                            fill: Some("A6A6A6"),
                            alpha: 100_000,
                            text: &[],
                            color: "000000",
                            rot: (deg.rem_euclid(360.0) * 60_000.0) as i64,
                            outline: None,
                        });
                    }
                }
            }
            Layout::Hierarchy | Layout::HierarchyHorizontal => {
                // Tree: leaves share the width, a parent centred over its children, levels top-down
                // (`flip`: the same tree turned, root on the left).
                fn leaves(n: &DiagramNode) -> usize {
                    if n.children.is_empty() {
                        1
                    } else {
                        n.children.iter().map(leaves).sum()
                    }
                }
                fn depth(n: &DiagramNode) -> usize {
                    1 + n.children.iter().map(depth).max().unwrap_or(0)
                }
                #[allow(clippy::too_many_arguments)]
                fn place(
                    out: &mut Out,
                    n: &DiagramNode,
                    first_leaf: usize,
                    level: i64,
                    lw: i64,
                    lh: i64,
                    bw: i64,
                    bh: i64,
                    (color, flip): (&str, bool),
                ) -> i64 {
                    let turn = |(x, y, w, h): (i64, i64, i64, i64)| {
                        if flip {
                            (y, x, h, w)
                        } else {
                            (x, y, w, h)
                        }
                    };
                    let line = |out: &mut Out, x1: i64, y1: i64, x2: i64, y2: i64| {
                        if flip {
                            out.line(y1, x1, y2, x2);
                        } else {
                            out.line(x1, y1, x2, y2);
                        }
                    };
                    let span = leaves(n) as i64;
                    let cx = first_leaf as i64 * lw + span * lw / 2;
                    let top = level * lh + (lh - bh) / 2;
                    out.shape(&Spec {
                        rect: turn((cx - bw / 2, top, bw, bh)),
                        prst: "roundRect",
                        fill: Some(color),
                        alpha: 100_000,
                        text: &n.text,
                        color: "FFFFFF",
                        rot: 0,
                        outline: None,
                    });
                    let mut leaf = first_leaf;
                    let mut centres = Vec::new();
                    for c in &n.children {
                        centres.push(place(
                            out,
                            c,
                            leaf,
                            level + 1,
                            lw,
                            lh,
                            bw,
                            bh,
                            (color, flip),
                        ));
                        leaf += leaves(c);
                    }
                    if let (Some(&a), Some(&b)) = (centres.first(), centres.last()) {
                        // Elbow connectors: down from the parent, across, down into each child.
                        let mid = top + bh + (lh - bh) / 2;
                        line(out, cx, top + bh, cx, mid);
                        if a != b {
                            line(out, a, mid, b, mid);
                        }
                        let child_top = (level + 1) * lh + (lh - bh) / 2;
                        for c in centres {
                            line(out, c, mid, c, child_top);
                        }
                    }
                    cx
                }
                let flip = layout == Layout::HierarchyHorizontal;
                let (across, along) = if flip { (h, w) } else { (w, h) };
                let total: usize = nodes.iter().map(leaves).sum();
                let levels = nodes.iter().map(depth).max().unwrap_or(1) as i64;
                let lw = across / total.max(1) as i64;
                let lh = along / levels;
                let (bw, bh) = (lw * 8 / 10, lh * 6 / 10);
                let mut leaf = 0;
                for (i, node) in nodes.iter().enumerate() {
                    let color = PALETTE[i % PALETTE.len()];
                    place(&mut out, node, leaf, 0, lw, lh, bw, bh, (color, flip));
                    leaf += leaves(node);
                }
            }
            Layout::Matrix => {
                // Square grid of quadrants.
                let side = ((n as f64).sqrt().ceil() as i64).max(2);
                let (cw, ch) = (w / side, h / side);
                for (i, node) in nodes.iter().enumerate() {
                    let (r, c) = (i as i64 / side, i as i64 % side);
                    out.shape(&Spec {
                        rect: (
                            c * cw + cw / 40,
                            r * ch + ch / 40,
                            cw * 19 / 20,
                            ch * 19 / 20,
                        ),
                        prst: "roundRect",
                        fill: Some(PALETTE[i % PALETTE.len()]),
                        alpha: 100_000,
                        text: &with_children(node),
                        color: "FFFFFF",
                        rot: 0,
                        outline: None,
                    });
                }
            }
            Layout::Pyramid => {
                // Levels top to bottom, each wider; the top one a triangle.
                let lh = h / n;
                for (i, node) in nodes.iter().enumerate() {
                    let k = i as i64;
                    let (top_w, bot_w) = (w * k / n, w * (k + 1) / n);
                    let prst = if k == 0 { "triangle" } else { "trapezoid" };
                    let bw = bot_w.max(top_w);
                    out.shape(&Spec {
                        rect: ((w - bw) / 2, k * lh, bw, lh),
                        prst,
                        fill: Some(PALETTE[i % PALETTE.len()]),
                        alpha: 100_000,
                        text: &with_children(node),
                        color: "FFFFFF",
                        rot: 0,
                        outline: None,
                    });
                }
            }
            Layout::Relationship => {
                // Overlapping translucent circles in a row.
                let d = (w as f64 / (1.0 + 0.7 * (n - 1) as f64)).min(h as f64) as i64;
                let step = d * 7 / 10;
                let x0 = (w - (d + step * (n - 1))) / 2;
                for (i, node) in nodes.iter().enumerate() {
                    out.shape(&Spec {
                        rect: (x0 + i as i64 * step, (h - d) / 2, d, d),
                        prst: "ellipse",
                        fill: Some(PALETTE[i % PALETTE.len()]),
                        alpha: 50_000,
                        text: &with_children(node),
                        color: "1F1F1F",
                        rot: 0,
                        outline: None,
                    });
                }
            }
        }
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<dsp:drawing xmlns:dgm="{DGM}" xmlns:dsp="{DSP}" xmlns:a="{A}"><dsp:spTree><dsp:nvGrpSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr><dsp:grpSpPr/>{}</dsp:spTree></dsp:drawing>"#,
        out.xml
    )
}

/// The texts of a node's children as bullet lines.
fn bullets(n: &DiagramNode) -> Vec<String> {
    n.children
        .iter()
        .flat_map(|c| c.text.iter().map(|s| format!("• {s}")))
        .collect()
}

/// One row per item: the title on the left, its sub-items in a light box beside it (a title without
/// sub-items takes the whole row).
fn list_vertical(out: &mut Out, nodes: &[DiagramNode], w: i64, h: i64) {
    let rh = h / nodes.len() as i64;
    for (i, node) in nodes.iter().enumerate() {
        let (y, bh) = (i as i64 * rh + rh / 20, rh * 9 / 10);
        if node.children.is_empty() {
            out.node((0, y, w, bh), "roundRect", i, &node.text);
            continue;
        }
        let tw = w * 3 / 10;
        out.node((0, y, tw, bh), "roundRect", i, &node.text);
        let gap = w / 50;
        out.plain(
            (tw + gap, y, w - tw - gap, bh),
            "rect",
            Some("F2F2F2"),
            &bullets(node),
            "1F1F1F",
        );
    }
}

/// A row of chevrons, each reaching 15 % of its width into the next; the first has a flat back.
fn process_chevron(out: &mut Out, nodes: &[DiagramNode], w: i64, h: i64) {
    let n = nodes.len() as i64;
    let cw = (w as f64 / (1.0 + 0.85 * (n - 1) as f64)) as i64;
    let bh = h.min(cw * 3 / 5).max(1);
    for (i, node) in nodes.iter().enumerate() {
        let prst = if i == 0 { "homePlate" } else { "chevron" };
        out.node(
            (i as i64 * cw * 85 / 100, (h - bh) / 2, cw, bh),
            prst,
            i,
            &with_children(node),
        );
    }
}

/// Steps top to bottom (boxes 1, arrows 0.4 high).
fn process_vertical(out: &mut Out, nodes: &[DiagramNode], w: i64, h: i64) {
    let n = nodes.len() as i64;
    let unit = (h as f64 / (n as f64 + 0.4 * (n - 1) as f64)) as i64;
    let bw = w * 8 / 10;
    for (i, node) in nodes.iter().enumerate() {
        let y = (i as f64 * 1.4 * unit as f64) as i64;
        out.node((w / 10, y, bw, unit), "roundRect", i, &with_children(node));
        if (i as i64) + 1 < n {
            let (aw, ah) = (unit.min(bw) * 2 / 5, unit * 3 / 10);
            out.plain(
                ((w - aw) / 2, y + unit + unit / 20, aw, ah),
                "downArrow",
                Some("A6A6A6"),
                &[],
                "000000",
            );
        }
    }
}

/// The first item in the middle, the others evenly on a ring around it (first at the top,
/// clockwise), each joined to the middle by a spoke.
fn cycle_radial(out: &mut Out, nodes: &[DiagramNode], w: i64, h: i64) {
    use std::f64::consts::{FRAC_PI_2, PI};
    let Some((first, rest)) = nodes.split_first() else {
        return;
    };
    let m = rest.len();
    let side = w.min(h) as f64;
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    let d = (side * 0.28).min(PI * side * 0.36 / m.max(1) as f64 * 1.1);
    let r = side / 2.0 - d / 2.0;
    let at = |k: usize| {
        let a = -FRAC_PI_2 + 2.0 * PI * k as f64 / m as f64;
        (cx + r * a.cos(), cy + r * a.sin())
    };
    for k in 0..m {
        out.bar((cx, cy), at(k), d * 0.06);
    }
    for (k, node) in rest.iter().enumerate() {
        let (x, y) = at(k);
        out.node(
            (
                (x - d / 2.0) as i64,
                (y - d / 2.0) as i64,
                d as i64,
                d as i64,
            ),
            "ellipse",
            k + 1,
            &with_children(node),
        );
    }
    // The ring circles are at most 0.28 of the side, so 0.34 in the middle never touches them.
    let c = if m == 0 { side * 0.6 } else { side * 0.34 };
    out.node(
        (
            (cx - c / 2.0) as i64,
            (cy - c / 2.0) as i64,
            c as i64,
            c as i64,
        ),
        "ellipse",
        0,
        &with_children(first),
    );
}

/// The first item in a dark ellipse over the middle of a grid of the other items.
fn matrix_centered(out: &mut Out, nodes: &[DiagramNode], w: i64, h: i64) {
    let Some((first, rest)) = nodes.split_first() else {
        return;
    };
    let side = ((rest.len() as f64).sqrt().ceil() as i64).max(2);
    let (cw, ch) = (w / side, h / side);
    for (k, node) in rest.iter().enumerate() {
        let (r, c) = (k as i64 / side, k as i64 % side);
        out.node(
            (
                c * cw + cw / 40,
                r * ch + ch / 40,
                cw * 19 / 20,
                ch * 19 / 20,
            ),
            "roundRect",
            k + 1,
            &with_children(node),
        );
    }
    let (bw, bh) = if rest.is_empty() {
        (w * 8 / 10, h * 8 / 10)
    } else {
        (w / 3, h / 3)
    };
    out.plain(
        ((w - bw) / 2, (h - bh) / 2, bw, bh),
        "ellipse",
        Some("3F4752"),
        &with_children(first),
        "FFFFFF",
    );
}

/// Levels top to bottom, each narrower; the last a triangle pointing down. The shapes are the
/// pyramid's turned half a turn, so each text is a separate upright box over its level.
fn pyramid_inverted(out: &mut Out, nodes: &[DiagramNode], w: i64, h: i64) {
    let n = nodes.len() as i64;
    let lh = h / n;
    for (i, node) in nodes.iter().enumerate() {
        let k = i as i64;
        let bw = w * (n - k) / n;
        let rect = ((w - bw) / 2, k * lh, bw, lh);
        out.shape(&Spec {
            rect,
            prst: if k == n - 1 { "triangle" } else { "trapezoid" },
            fill: Some(PALETTE[i % PALETTE.len()]),
            alpha: 100_000,
            text: &[],
            color: "FFFFFF",
            rot: 180 * 60_000,
            outline: None,
        });
        let ink = out.ink(i);
        out.plain(rect, "rect", None, &with_children(node), ink);
    }
}

/// Rings around the first item (one ring per further item, the last the largest) on the left; each
/// label on the right, joined by a leader to the top of its ring.
fn relationship_target(out: &mut Out, nodes: &[DiagramNode], w: i64, h: i64) {
    let n = nodes.len();
    let d = (w * 55 / 100).min(h) as f64;
    let (cx, cy) = (d / 2.0, h as f64 / 2.0);
    for k in (0..n).rev() {
        let dk = d * (k + 1) as f64 / n as f64;
        out.node(
            (
                (cx - dk / 2.0) as i64,
                (cy - dk / 2.0) as i64,
                dk as i64,
                dk as i64,
            ),
            "ellipse",
            k,
            &[],
        );
    }
    // Ring `k` shows as a band `d / 2n` wide; its label sits level with the band's top middle.
    let band = d / (2 * n) as f64;
    let lx = d + w as f64 / 25.0;
    out.left = true;
    for (k, node) in nodes.iter().enumerate() {
        let y = cy - band * (k as f64 + 0.5);
        out.bar((cx, y), (lx, y), (band * 0.06).max(9_525.0));
        out.plain(
            (
                lx as i64,
                (y - band / 2.0) as i64,
                w - lx as i64,
                band as i64,
            ),
            "rect",
            None,
            &with_children(node),
            "1F1F1F",
        );
    }
    out.left = false;
}

/// A grid of cards: a picture frame over the caption.
fn picture_cards(out: &mut Out, nodes: &[DiagramNode], w: i64, h: i64) {
    let n = nodes.len() as i64;
    let cols = ((n as f64 * w as f64 / h as f64).sqrt().round() as i64).clamp(1, n);
    let rows = (n + cols - 1) / cols;
    let (cw, ch) = (w / cols, h / rows);
    for (i, node) in nodes.iter().enumerate() {
        let (x, y) = (
            i as i64 % cols * cw + cw / 20,
            i as i64 / cols * ch + ch / 20,
        );
        let (bw, bh) = (cw * 9 / 10, ch * 9 / 10);
        out.frame((x, y, bw, bh * 65 / 100), "rect", i);
        out.node(
            (x, y + bh * 70 / 100, bw, bh * 30 / 100),
            "roundRect",
            i,
            &with_children(node),
        );
    }
}

/// One row per item: a round picture frame on the left, the caption beside it.
fn picture_strip(out: &mut Out, nodes: &[DiagramNode], w: i64, h: i64) {
    let rh = h / nodes.len() as i64;
    for (i, node) in nodes.iter().enumerate() {
        let (y, bh) = (i as i64 * rh + rh / 20, rh * 9 / 10);
        let d = bh.min(w / 3);
        out.frame((0, y + (bh - d) / 2, d, d), "ellipse", i);
        let gap = w / 40;
        out.node(
            (d + gap, y, w - d - gap, bh),
            "roundRect",
            i,
            &with_children(node),
        );
    }
}

const DGM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
const DSP: &str = "http://schemas.microsoft.com/office/drawing/2008/diagram";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

impl Category {
    /// The category name of §21.4.2.3 `cat@type`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Category::List => "list",
            Category::Process => "process",
            Category::Cycle => "cycle",
            Category::Hierarchy => "hierarchy",
            Category::Matrix => "matrix",
            Category::Pyramid => "pyramid",
            Category::Relationship => "relationship",
            Category::Picture => "picture",
        }
    }

    /// The category of a name (`list`, `process`, …).
    #[must_use]
    pub fn parse(name: &str) -> Option<Category> {
        Some(match name {
            "list" => Category::List,
            "process" => Category::Process,
            "cycle" => Category::Cycle,
            "hierarchy" => Category::Hierarchy,
            "matrix" => Category::Matrix,
            "pyramid" => Category::Pyramid,
            "relationship" => Category::Relationship,
            "picture" => Category::Picture,
            _ => return None,
        })
    }
}

/// Nodes of an outline: (text, level) lines, level 0 = top level; a line deeper than the one before
/// it becomes that line's child (levels are clamped to one more than the previous line).
#[must_use]
pub fn nodes_from_outline(lines: &[(String, usize)]) -> Vec<DiagramNode> {
    fn take(lines: &[(String, usize)], i: &mut usize, level: usize) -> Vec<DiagramNode> {
        let mut out = Vec::new();
        while *i < lines.len() && lines[*i].1 >= level {
            let text = lines[*i].0.clone();
            *i += 1;
            let children = if *i < lines.len() && lines[*i].1 > level {
                take(lines, i, level + 1)
            } else {
                Vec::new()
            };
            out.push(DiagramNode {
                text: vec![text],
                children,
            });
        }
        out
    }
    let mut clamped = Vec::with_capacity(lines.len());
    let mut prev = 0usize;
    for (k, (t, l)) in lines.iter().enumerate() {
        let l = if k == 0 { 0 } else { (*l).min(prev + 1) };
        clamped.push((t.clone(), l));
        prev = l;
    }
    let mut i = 0;
    take(&clamped, &mut i, 0)
}

/// The outline of nodes (inverse of [`nodes_from_outline`]): one line per node, paragraphs joined.
#[must_use]
pub fn outline_of(nodes: &[DiagramNode]) -> Vec<(String, usize)> {
    fn walk(n: &[DiagramNode], level: usize, out: &mut Vec<(String, usize)>) {
        for d in n {
            out.push((d.text.join(" "), level));
            walk(&d.children, level + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(nodes, 0, &mut out);
    out
}

/// The parts of a new SmartArt (ECMA-376 Part 1, §21.4): data model, layout definition, quick style,
/// colours and the drawing part.
pub struct DiagramParts {
    /// `dgm:dataModel` (its `dsp:dataModelExt@relId` names `drawing_rel` of the host part).
    pub data: Vec<u8>,
    /// `dgm:layoutDef` naming the BlueOffice layout and its category.
    pub layout: Vec<u8>,
    /// `dgm:styleDef`.
    pub style: Vec<u8>,
    /// `dgm:colorsDef`.
    pub colors: Vec<u8>,
    /// `dsp:drawing` laid out for the frame.
    pub drawing: Vec<u8>,
}

/// Layout id of a BlueOffice layout (`dgm:layoutDef@uniqueId`, `dgm:prSet@loTypeId`).
#[must_use]
pub fn layout_id(layout: Layout) -> String {
    format!("{LAYOUT_ID}{}", layout.name())
}

fn xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The data model of `nodes` (§21.4.3): the document point, one point per node and `parOf`
/// connections in order, plus the extension naming the drawing part relationship `drawing_rel` (none
/// when it is empty).
#[must_use]
pub fn data_model_xml(
    nodes: &[DiagramNode],
    layout: Layout,
    look: Look,
    drawing_rel: &str,
) -> String {
    fn id(n: usize) -> String {
        format!("{{B1E50FF1-0000-4000-8000-{n:012X}}}")
    }
    fn walk(
        nodes: &[DiagramNode],
        parent: &str,
        next: &mut usize,
        pts: &mut String,
        cxns: &mut String,
    ) {
        for (ord, n) in nodes.iter().enumerate() {
            *next += 1;
            let me = id(*next);
            let paras: String = if n.text.is_empty() {
                r#"<a:p><a:endParaRPr lang="vi-VN"/></a:p>"#.to_owned()
            } else {
                n.text.iter().fold(String::new(), |mut acc, t| {
                    let _ = write!(
                        acc,
                        r#"<a:p><a:r><a:rPr lang="vi-VN"/><a:t>{}</a:t></a:r></a:p>"#,
                        xml_text(t)
                    );
                    acc
                })
            };
            let _ = write!(
                pts,
                r#"<dgm:pt modelId="{me}"><dgm:prSet phldrT="[Text]"/><dgm:spPr/><dgm:t><a:bodyPr/><a:lstStyle/>{paras}</dgm:t></dgm:pt>"#
            );
            *next += 1;
            let _ = write!(
                cxns,
                r#"<dgm:cxn modelId="{}" srcId="{parent}" destId="{me}" srcOrd="{ord}" destOrd="0"/>"#,
                id(*next)
            );
            walk(&n.children, &me, next, pts, cxns);
        }
    }
    let doc = id(0);
    let mut pts = format!(
        r#"<dgm:pt modelId="{doc}" type="doc"><dgm:prSet loTypeId="{}" loCatId="{}" qsTypeId="{STYLE_ID}{}" csTypeId="{COLORS_ID}{}"/><dgm:spPr/><dgm:t><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="vi-VN"/></a:p></dgm:t></dgm:pt>"#,
        layout_id(layout),
        layout.category().name(),
        look.fill.name(),
        look.palette.name()
    );
    let mut cxns = String::new();
    let mut next = 0;
    walk(nodes, &doc, &mut next, &mut pts, &mut cxns);
    // Without a drawing part (empty `drawing_rel`) the data model names none.
    let ext = if drawing_rel.is_empty() {
        String::new()
    } else {
        format!(
            r#"<dgm:extLst><a:ext uri="http://schemas.microsoft.com/office/drawing/2008/diagram"><dsp:dataModelExt xmlns:dsp="{DSP}" relId="{drawing_rel}" minVer="http://schemas.openxmlformats.org/drawingml/2006/diagram"/></a:ext></dgm:extLst>"#
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<dgm:dataModel xmlns:dgm="{DGM}" xmlns:a="{A}"><dgm:ptLst>{pts}</dgm:ptLst><dgm:cxnLst>{cxns}</dgm:cxnLst><dgm:bg/><dgm:whole/>{ext}</dgm:dataModel>"#
    )
}

/// The layout definition part of a BlueOffice layout: its id and category (§21.4.2.15 layoutDef; the
/// layout itself is BlueOffice's own, so the definition carries no layout algorithm).
#[must_use]
pub fn layout_def_xml(layout: Layout) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<dgm:layoutDef xmlns:dgm="{DGM}" xmlns:a="{A}" uniqueId="{}"><dgm:title val="BlueOffice {}"/><dgm:catLst><dgm:cat type="{}" pri="1000"/></dgm:catLst><dgm:layoutNode name="diagram"/></dgm:layoutDef>"#,
        layout_id(layout),
        layout.name(),
        layout.category().name()
    )
}

/// All parts of a new SmartArt for a `w` × `h` EMU frame.
#[must_use]
pub fn build_parts(
    nodes: &[DiagramNode],
    layout: Layout,
    look: Look,
    (w, h): (i64, i64),
    drawing_rel: &str,
) -> DiagramParts {
    DiagramParts {
        data: data_model_xml(nodes, layout, look, drawing_rel).into_bytes(),
        layout: layout_def_xml(layout).into_bytes(),
        style: format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<dgm:styleDef xmlns:dgm="{DGM}" xmlns:a="{A}" uniqueId="{STYLE_ID}{}"><dgm:styleLbl name="node0"/></dgm:styleDef>"#,
            look.fill.name()
        )
        .into_bytes(),
        colors: format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<dgm:colorsDef xmlns:dgm="{DGM}" xmlns:a="{A}" uniqueId="{COLORS_ID}{}"/>"#,
            look.palette.name()
        )
        .into_bytes(),
        drawing: drawing_xml(nodes, layout, look, w, h).into_bytes(),
    }
}

/// Shapes of a diagram laid out from its data model part (`layout_xml`: the layout part, if any).
#[must_use]
pub fn synth(
    data_xml: &[u8],
    data_part: &str,
    layout_xml: Option<&[u8]>,
    w: i64,
    h: i64,
) -> Option<Vec<ShapeNode>> {
    let (nodes, lo) = parse_data_model(data_xml, data_part)?;
    if nodes.is_empty() {
        return None;
    }
    let look = look_of(data_xml, data_part);
    let shapes = layout_shapes(&nodes, layout_of(lo.as_deref(), layout_xml), look, w, h);
    (!shapes.is_empty()).then_some(shapes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod tests {
    use super::*;
    use bo_model::slide::Geometry;

    /// A data model (ECMA-376 Part 1, §21.4.3) with a document point (layout id `lo`) and nodes given as
    /// (text, parent index or `None` for top level).
    pub(crate) fn data_model(lo: &str, nodes: &[(&str, Option<usize>)]) -> String {
        let mut pts = format!(
            r#"<dgm:pt modelId="{{D}}" type="doc"><dgm:prSet loTypeId="{lo}"/><dgm:spPr/><dgm:t><a:bodyPr/><a:p/></dgm:t></dgm:pt>"#
        );
        let mut cxns = String::new();
        let mut ord = std::collections::HashMap::new();
        for (i, (t, parent)) in nodes.iter().enumerate() {
            let _ = write!(
                pts,
                r#"<dgm:pt modelId="{{N{i}}}"><dgm:prSet/><dgm:spPr/><dgm:t><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="vi-VN"/><a:t>{t}</a:t></a:r></a:p></dgm:t></dgm:pt>"#
            );
            // Transitions and presentation points are not content.
            let _ = write!(
                pts,
                r#"<dgm:pt modelId="{{P{i}}}" type="pres"><dgm:prSet/></dgm:pt>"#
            );
            let src = parent.map_or("D".to_owned(), |p| format!("N{p}"));
            let o = ord.entry(src.clone()).or_insert(0u32);
            let _ = write!(
                cxns,
                r#"<dgm:cxn modelId="{{C{i}}}" srcId="{{{src}}}" destId="{{N{i}}}" srcOrd="{o}" destOrd="0"/><dgm:cxn modelId="{{Q{i}}}" type="presOf" srcId="{{N{i}}}" destId="{{P{i}}}" srcOrd="0" destOrd="0"/>"#
            );
            *o += 1;
        }
        format!(
            r#"<dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dgm:ptLst>{pts}</dgm:ptLst><dgm:cxnLst>{cxns}</dgm:cxnLst><dgm:bg/><dgm:whole/></dgm:dataModel>"#
        )
    }

    /// (preset, x, y, w, h, text) of every shape.
    fn boxes(shapes: &[ShapeNode]) -> Vec<(String, i64, i64, i64, i64, String)> {
        shapes
            .iter()
            .filter_map(|s| match s {
                ShapeNode::Shape(s) => {
                    let x = s.props.xfrm.as_ref()?;
                    let (off, ext) = (x.off?, x.ext?);
                    let prst = match &s.props.geometry {
                        Some(Geometry::Preset { name, .. }) => name.clone(),
                        _ => String::new(),
                    };
                    let text = s
                        .text
                        .as_ref()
                        .map(bo_model::slide::TextBody::text)
                        .unwrap_or_default();
                    Some((prst, off.0, off.1, ext.0, ext.1, text.trim().to_owned()))
                }
                _ => None,
            })
            .collect()
    }

    const W: i64 = 6_000_000;
    const H: i64 = 3_000_000;

    // source: ECMA-376 Part 1 §21.4.3 data model; layout rules of this module.
    #[test]
    fn reads_the_tree_in_order() {
        let x = data_model(
            "urn:x/layout/orgChart1",
            &[
                ("Giám đốc", None),
                ("Phó A", Some(0)),
                ("Phó B", Some(0)),
                ("Tổ 1", Some(1)),
            ],
        );
        let (nodes, lo) = parse_data_model(x.as_bytes(), "/d.xml").unwrap();
        assert_eq!(lo.as_deref(), Some("urn:x/layout/orgChart1"));
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].text, ["Giám đốc"]);
        let kids: Vec<_> = nodes[0]
            .children
            .iter()
            .map(|c| c.text[0].as_str())
            .collect();
        assert_eq!(kids, ["Phó A", "Phó B"]);
        assert_eq!(nodes[0].children[0].children[0].text, ["Tổ 1"]);
        // A data model with only the document point (the drawing part holds everything) has no nodes.
        let empty = data_model("urn:x/layout/default", &[]);
        assert_eq!(
            parse_data_model(empty.as_bytes(), "/d.xml").unwrap().0,
            Vec::new()
        );
        assert!(parse_data_model(b"<p:sld/>", "/d.xml").is_none());
    }

    #[test]
    fn categories_from_layout_part_or_id() {
        let layout = br#"<dgm:layoutDef xmlns:dgm="d" uniqueId="urn:x/custom"><dgm:catLst><dgm:cat type="cycle" pri="1000"/></dgm:catLst></dgm:layoutDef>"#;
        assert_eq!(
            category(Some("urn:x/layout/process1"), Some(layout)),
            Category::Cycle
        );
        assert_eq!(
            category(Some("urn:x/layout/process1"), None),
            Category::Process
        );
        assert_eq!(
            category(Some("urn:x/layout/hierarchy2"), None),
            Category::Hierarchy
        );
        assert_eq!(
            category(Some("urn:x/layout/basicPyramid1"), None),
            Category::Pyramid
        );
        assert_eq!(
            category(Some("urn:x/layout/venn1"), None),
            Category::Relationship
        );
        assert_eq!(
            category(Some("urn:x/layout/matrix1"), None),
            Category::Matrix
        );
        assert_eq!(category(Some("urn:x/layout/default"), None), Category::List);
        assert_eq!(category(None, None), Category::List);
    }

    #[test]
    fn process_is_a_row_of_steps_with_arrows() {
        let x = data_model(
            "urn:x/layout/process1",
            &[("Bước 1", None), ("Bước 2", None), ("Bước 3", None)],
        );
        let b = boxes(&synth(x.as_bytes(), "/d.xml", None, W, H).unwrap());
        let steps: Vec<_> = b.iter().filter(|s| s.0 == "roundRect").collect();
        let arrows: Vec<_> = b.iter().filter(|s| s.0 == "rightArrow").collect();
        assert_eq!(steps.len(), 3);
        assert_eq!(arrows.len(), 2);
        assert_eq!(
            steps.iter().map(|s| s.5.as_str()).collect::<Vec<_>>(),
            ["Bước 1", "Bước 2", "Bước 3"]
        );
        // Left to right, same row, inside the frame; each arrow between its two steps.
        assert!(steps
            .windows(2)
            .all(|p| p[0].1 + p[0].3 < p[1].1 && p[0].2 == p[1].2));
        assert!(steps
            .iter()
            .all(|s| s.1 >= 0 && s.1 + s.3 <= W && s.2 >= 0 && s.2 + s.4 <= H));
        for (i, a) in arrows.iter().enumerate() {
            assert!(a.1 > steps[i].1 + steps[i].3 - 1 && a.1 + a.3 < steps[i + 1].1);
        }
    }

    #[test]
    fn cycle_puts_nodes_on_a_ring() {
        let x = data_model(
            "urn:x/layout/cycle2",
            &[("A", None), ("B", None), ("C", None), ("D", None)],
        );
        let b = boxes(&synth(x.as_bytes(), "/d.xml", None, W, H).unwrap());
        let nodes: Vec<_> = b.iter().filter(|s| s.0 == "ellipse").collect();
        assert_eq!(nodes.len(), 4);
        assert_eq!(b.iter().filter(|s| s.0 == "rightArrow").count(), 4);
        let centre = |s: &(String, i64, i64, i64, i64, String)| (s.1 + s.3 / 2, s.2 + s.4 / 2);
        let dist: Vec<f64> = nodes
            .iter()
            .map(|s| {
                let (x, y) = centre(s);
                (((x - W / 2) as f64).powi(2) + ((y - H / 2) as f64).powi(2)).sqrt()
            })
            .collect();
        // Same distance from the centre; the first at the top.
        assert!(dist.iter().all(|d| (d - dist[0]).abs() < 2.0), "{dist:?}");
        assert!(centre(nodes[0]).1 < centre(nodes[2]).1);
        assert!((centre(nodes[0]).0 - W / 2).abs() < 2);
    }

    #[test]
    fn hierarchy_centres_parents_over_children() {
        let x = data_model(
            "urn:x/layout/orgChart1",
            &[
                ("CEO", None),
                ("A", Some(0)),
                ("B", Some(0)),
                ("A1", Some(1)),
                ("A2", Some(1)),
            ],
        );
        let b = boxes(&synth(x.as_bytes(), "/d.xml", None, W, H).unwrap());
        let at = |t: &str| b.iter().find(|s| s.5 == t).unwrap().clone();
        let cx = |s: &(String, i64, i64, i64, i64, String)| s.1 + s.3 / 2;
        let (ceo, a, bb, a1, a2) = (at("CEO"), at("A"), at("B"), at("A1"), at("A2"));
        // Levels go down; a parent sits over the middle of its children.
        assert!(ceo.2 < a.2 && a.2 == bb.2 && a.2 < a1.2 && a1.2 == a2.2);
        assert!((cx(&a) - i64::midpoint(cx(&a1), cx(&a2))).abs() <= 1);
        assert!((cx(&ceo) - i64::midpoint(cx(&a), cx(&bb))).abs() <= W / 6);
        // Connectors: lines exist between the levels.
        assert!(b.iter().filter(|s| s.0 == "line").count() >= 6);
    }

    #[test]
    fn matrix_pyramid_relationship_and_list() {
        let four = [("Q1", None), ("Q2", None), ("Q3", None), ("Q4", None)];
        let m = boxes(
            &synth(
                data_model("urn:x/layout/matrix3", &four).as_bytes(),
                "/d.xml",
                None,
                W,
                H,
            )
            .unwrap(),
        );
        assert_eq!(m.len(), 4);
        // 2 × 2: two distinct x and two distinct y positions.
        let mut xs: Vec<i64> = m.iter().map(|s| s.1).collect();
        xs.dedup();
        xs.sort_unstable();
        xs.dedup();
        assert_eq!(xs.len(), 2);
        let p = boxes(
            &synth(
                data_model("urn:x/layout/pyramid1", &four[..3]).as_bytes(),
                "/d.xml",
                None,
                W,
                H,
            )
            .unwrap(),
        );
        assert_eq!(p[0].0, "triangle");
        assert!(p[0].3 < p[1].3 && p[1].3 < p[2].3 && p[0].2 < p[1].2);
        let v = boxes(
            &synth(
                data_model("urn:x/layout/venn1", &four[..3]).as_bytes(),
                "/d.xml",
                None,
                W,
                H,
            )
            .unwrap(),
        );
        assert!(v.iter().all(|s| s.0 == "ellipse"));
        // Overlapping: each circle starts before the previous one ends.
        assert!(v.windows(2).all(|q| q[1].1 < q[0].1 + q[0].3));
        let l = boxes(
            &synth(
                data_model("urn:x/layout/default", &[("Mục", None), ("Ý con", Some(0))]).as_bytes(),
                "/d.xml",
                None,
                W,
                H,
            )
            .unwrap(),
        );
        assert_eq!(l.len(), 1);
        assert!(
            l[0].5.contains("Mục") && l[0].5.contains("• Ý con"),
            "{:?}",
            l[0].5
        );
    }

    #[test]
    fn outline_round_trip_and_parts() {
        let lines: Vec<(String, usize)> = [
            ("A", 0),
            ("A1", 1),
            ("A2", 1),
            ("A2a", 2),
            ("B", 0),
            ("C", 3),
        ]
        .iter()
        .map(|(t, l)| ((*t).to_owned(), *l))
        .collect();
        let nodes = nodes_from_outline(&lines);
        // Top level: A and B (C becomes B's child).
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].children.len(), 2);
        assert_eq!(nodes[0].children[1].children[0].text, ["A2a"]);
        // "C" at level 3 after a level-0 line is clamped to level 1 (child of B).
        assert_eq!(nodes[1].children[0].text, ["C"]);
        let back = outline_of(&nodes);
        assert_eq!(back[5], ("C".to_owned(), 1));
        // Written parts read back as the same tree and category.
        let look = Look {
            palette: Palette::Green,
            fill: Fill::Outline,
        };
        let p = build_parts(&nodes, Layout::Cycle, look, (4_000_000, 3_000_000), "rId9");
        assert_eq!(look_of(&p.data, "/d.xml"), look);
        // Outline look: coloured 2 pt frames on white, coloured text.
        let d = String::from_utf8(p.drawing.clone()).unwrap();
        assert!(
            d.contains(r#"<a:ln w="25400"><a:solidFill><a:srgbClr val="2E8B57"/>"#),
            "{d}"
        );
        let (read, lo) = parse_data_model(&p.data, "/d.xml").unwrap();
        assert_eq!(read, nodes);
        assert_eq!(category(lo.as_deref(), Some(&p.layout)), Category::Cycle);
        assert_eq!(
            crate::diagram::drawing_rel_id(&p.data, "/d.xml").as_deref(),
            Some("rId9")
        );
        let shapes = crate::diagram::drawing_shapes(&p.drawing, "/x.xml").unwrap();
        assert_ne!(shapes.len(), 0);
        // ST_ModelId is an integer or a GUID ([MS-ODRAWXML] dsp:sp@modelId).
        let d = String::from_utf8(p.drawing.clone()).unwrap();
        assert!(
            d.contains(r#"modelId="{B1E50FF2-0000-4000-8000-000000000001}""#),
            "{d}"
        );
        for c in [
            "list",
            "process",
            "cycle",
            "hierarchy",
            "matrix",
            "pyramid",
            "relationship",
            "picture",
        ] {
            assert_eq!(Category::parse(c).unwrap().name(), c);
            // The first layout of a category has the category's name.
            assert_eq!(Layout::from(Category::parse(c).unwrap()).name(), c);
        }
    }

    fn tree(spec: &[(&str, usize)]) -> Vec<DiagramNode> {
        let lines: Vec<(String, usize)> = spec.iter().map(|(t, l)| ((*t).to_owned(), *l)).collect();
        nodes_from_outline(&lines)
    }

    fn laid(layout: Layout, spec: &[(&str, usize)]) -> Vec<(String, i64, i64, i64, i64, String)> {
        boxes(&layout_shapes(&tree(spec), layout, Look::default(), W, H))
    }

    // source: this module's layout rules (BlueOffice's own library); §21.4.2.3 cat for the names.
    #[test]
    fn every_layout_of_the_library_shows_every_item_inside_the_frame() {
        let spec = [
            ("Một", 0),
            ("Một a", 1),
            ("Hai", 0),
            ("Ba", 0),
            ("Ba a", 1),
            ("Bốn", 0),
            ("Năm", 0),
        ];
        let mut names = std::collections::HashSet::new();
        for layout in Layout::ALL {
            assert!(names.insert(layout.name()), "{layout:?}");
            assert_eq!(Layout::parse(layout.name()), Some(layout));
            for n in 1..=spec.len() {
                let b = laid(layout, &spec[..n]);
                assert_ne!(b.len(), 0, "{layout:?} {n}");
                for (t, _) in &spec[..n] {
                    assert!(b.iter().any(|s| s.5.contains(t)), "{layout:?} {n}: {t}");
                }
                for s in &b {
                    assert!(
                        s.1 >= 0 && s.2 >= 0 && s.1 + s.3 <= W && s.2 + s.4 <= H,
                        "{layout:?} {n}: {s:?}"
                    );
                }
            }
            // Written and read back: the same layout, its category in the layout part.
            let p = build_parts(&tree(&spec), layout, Look::default(), (W, H), "rId5");
            let (read, lo) = parse_data_model(&p.data, "/d.xml").unwrap();
            assert_eq!(read, tree(&spec));
            assert_eq!(layout_of(lo.as_deref(), Some(&p.layout)), layout);
            assert_eq!(layout_of(lo.as_deref(), None), layout);
            assert_eq!(category(None, Some(&p.layout)), layout.category());
        }
        // Another producer's layout: the first layout of its category.
        assert_eq!(
            layout_of(Some("urn:x/layout/chevron2"), None),
            Layout::Process
        );
        let pic = br#"<dgm:layoutDef xmlns:dgm="d" uniqueId="urn:x/custom"><dgm:catLst><dgm:cat type="picture" pri="1000"/></dgm:catLst></dgm:layoutDef>"#;
        assert_eq!(layout_of(Some("urn:x/custom"), Some(pic)), Layout::Picture);
    }

    #[test]
    fn list_and_process_variants() {
        // Vertical list: one row per item, sub-items in a box right of the title.
        let b = laid(
            Layout::ListVertical,
            &[("A", 0), ("a1", 1), ("a2", 1), ("B", 0)],
        );
        assert_eq!(b.len(), 3);
        assert_eq!((b[0].5.as_str(), b[2].5.as_str()), ("A", "B"));
        assert!(b[1].5.contains("• a1") && b[1].5.contains("• a2"));
        assert!(b[1].1 > b[0].1 + b[0].3 - 1 && b[1].2 == b[0].2);
        assert!(b[2].2 >= b[0].2 + b[0].4 && b[2].3 == W);
        // Chevrons: a flat-backed first step, each next one starting inside the one before.
        let c = laid(Layout::ProcessChevron, &[("1", 0), ("2", 0), ("3", 0)]);
        assert_eq!(
            c.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
            ["homePlate", "chevron", "chevron"]
        );
        assert!(c
            .windows(2)
            .all(|p| p[1].1 > p[0].1 && p[1].1 < p[0].1 + p[0].3));
        assert!(W - (c[2].1 + c[2].3) < 10);
        // Vertical process: steps downwards with a down arrow between two steps.
        let v = laid(Layout::ProcessVertical, &[("1", 0), ("2", 0), ("3", 0)]);
        let steps: Vec<_> = v.iter().filter(|s| s.0 == "roundRect").collect();
        let arrows: Vec<_> = v.iter().filter(|s| s.0 == "downArrow").collect();
        assert_eq!((steps.len(), arrows.len()), (3, 2));
        for (i, a) in arrows.iter().enumerate() {
            assert!(a.2 >= steps[i].2 + steps[i].4 && a.2 + a.4 <= steps[i + 1].2);
        }
    }

    #[test]
    fn radial_cycle_and_centred_matrix_put_the_first_item_in_the_middle() {
        let centre = |s: &(String, i64, i64, i64, i64, String)| (s.1 + s.3 / 2, s.2 + s.4 / 2);
        let b = laid(
            Layout::CycleRadial,
            &[("Tâm", 0), ("A", 0), ("B", 0), ("C", 0), ("D", 0)],
        );
        let at = |t: &str| b.iter().find(|s| s.5 == t).unwrap();
        let (mx, my) = centre(at("Tâm"));
        assert!((mx - W / 2).abs() <= 1 && (my - H / 2).abs() <= 1);
        // The others at one distance from the middle, the first of them straight above it.
        let dist: Vec<f64> = ["A", "B", "C", "D"]
            .iter()
            .map(|t| {
                let (x, y) = centre(at(t));
                (((x - W / 2) as f64).powi(2) + ((y - H / 2) as f64).powi(2)).sqrt()
            })
            .collect();
        assert!(dist.iter().all(|d| (d - dist[0]).abs() < 2.0), "{dist:?}");
        assert!((centre(at("A")).0 - W / 2).abs() <= 1 && centre(at("A")).1 < my);
        // No circle of the ring reaches the middle one: radii sum below the distance.
        assert!(dist[0] > (at("Tâm").3 / 2 + at("A").3 / 2) as f64);
        // One spoke per outer item.
        assert_eq!(b.iter().filter(|s| s.0 == "rect").count(), 4);
        let m = laid(
            Layout::MatrixCentered,
            &[("Tâm", 0), ("Q1", 0), ("Q2", 0), ("Q3", 0), ("Q4", 0)],
        );
        // Drawn last (on top), in the middle; the four others in a 2 × 2 grid.
        assert_eq!(m.last().unwrap().5, "Tâm");
        assert_eq!(centre(m.last().unwrap()), (W / 2, H / 2));
        let mut xs: Vec<i64> = m[..4].iter().map(|s| s.1).collect();
        xs.sort_unstable();
        xs.dedup();
        assert_eq!(xs.len(), 2);
    }

    #[test]
    fn horizontal_hierarchy_is_the_tree_turned() {
        let spec = [("CEO", 0), ("A", 1), ("A1", 2), ("A2", 2), ("B", 1)];
        let down = laid(Layout::Hierarchy, &spec);
        let across = laid(Layout::HierarchyHorizontal, &spec);
        assert_eq!(down.len(), across.len());
        let at = |t: &str| across.iter().find(|s| s.5 == t).unwrap().clone();
        let cy = |s: &(String, i64, i64, i64, i64, String)| s.2 + s.4 / 2;
        let (ceo, a, b, a1, a2) = (at("CEO"), at("A"), at("B"), at("A1"), at("A2"));
        // Levels go right; a parent is level with the middle of its children.
        assert!(ceo.1 < a.1 && a.1 == b.1 && a.1 < a1.1 && a1.1 == a2.1);
        assert!((cy(&a) - i64::midpoint(cy(&a1), cy(&a2))).abs() <= 1);
        assert!(cy(&a1) < cy(&a2) && cy(&a) < cy(&b));
    }

    #[test]
    fn inverted_pyramid_target_and_pictures() {
        let three = [("Rộng", 0), ("Vừa", 0), ("Hẹp", 0)];
        let p = laid(Layout::PyramidInverted, &three);
        // A turned shape and an upright text box per level, narrowing downwards to a triangle.
        let levels: Vec<_> = p.iter().filter(|s| s.0 != "rect").collect();
        assert_eq!(
            levels.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
            ["trapezoid", "trapezoid", "triangle"]
        );
        assert!(levels
            .windows(2)
            .all(|q| q[0].3 > q[1].3 && q[0].2 < q[1].2));
        assert_eq!(levels[0].3, W);
        let texts: Vec<_> = p.iter().filter(|s| s.0 == "rect").collect();
        assert_eq!(
            texts.iter().map(|s| s.5.as_str()).collect::<Vec<_>>(),
            ["Rộng", "Vừa", "Hẹp"]
        );
        for (t, l) in texts.iter().zip(&levels) {
            assert_eq!((t.1, t.2, t.3, t.4), (l.1, l.2, l.3, l.4));
        }
        let xml = drawing_xml(
            &tree(&three),
            Layout::PyramidInverted,
            Look::default(),
            W,
            H,
        );
        assert_eq!(xml.matches(r#"<a:xfrm rot="10800000">"#).count(), 3);
        assert_eq!(xml.matches("<a:ln><a:noFill/></a:ln>").count(), 3);
        // Target: concentric rings, the largest drawn first; one label per ring to the right.
        let t = laid(Layout::RelationshipTarget, &three);
        let rings: Vec<_> = t.iter().filter(|s| s.0 == "ellipse").collect();
        assert_eq!(rings.len(), 3);
        assert!(rings.windows(2).all(|q| q[0].3 > q[1].3));
        let c = |s: &(String, i64, i64, i64, i64, String)| (s.1 + s.3 / 2, s.2 + s.4 / 2);
        assert!(rings
            .iter()
            .all(|s| (c(s).0 - c(rings[0]).0).abs() <= 1 && (c(s).1 - c(rings[0]).1).abs() <= 1));
        let labels: Vec<_> = t.iter().filter(|s| !s.5.is_empty()).collect();
        assert_eq!(labels.len(), 3);
        assert!(labels.iter().all(|l| l.1 >= rings[0].1 + rings[0].3));
        // The label of the first item (the innermost ring) is the lowest.
        assert!(labels[0].2 > labels[1].2 && labels[1].2 > labels[2].2);
        // Pictures: a frame above each caption; the strip has the frame left of the caption.
        let two = [("Ảnh 1", 0), ("Ảnh 2", 0)];
        let cards = laid(Layout::Picture, &two);
        let frames: Vec<_> = cards.iter().filter(|s| s.0 == "rect").collect();
        let caps: Vec<_> = cards.iter().filter(|s| s.0 == "roundRect").collect();
        assert_eq!((frames.len(), caps.len()), (2, 2));
        for (f, cap) in frames.iter().zip(&caps) {
            assert!(f.1 == cap.1 && f.3 == cap.3 && f.2 + f.4 <= cap.2);
        }
        let strip = laid(Layout::PictureStrip, &two);
        let frames: Vec<_> = strip
            .iter()
            .filter(|s| s.0 == "ellipse" && s.3 > W / 20)
            .collect();
        let caps: Vec<_> = strip.iter().filter(|s| s.0 == "roundRect").collect();
        assert_eq!((frames.len(), caps.len()), (2, 2));
        for (f, cap) in frames.iter().zip(&caps) {
            assert!(f.1 + f.3 <= cap.1 && f.2 >= cap.2 && f.2 + f.4 <= cap.2 + cap.4);
        }
        // The frame follows the palette: its outline is the node's colour.
        let green = Look {
            palette: Palette::Green,
            fill: Fill::Solid,
        };
        let xml = drawing_xml(&tree(&two), Layout::Picture, green, W, H);
        assert_eq!(
            xml.matches(r#"<a:ln w="25400"><a:solidFill><a:srgbClr val="2E8B57"/>"#)
                .count(),
            2
        );
    }
}
