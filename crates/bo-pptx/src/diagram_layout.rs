//! SmartArt without a drawing part: the diagram is laid out from its data model (ECMA-376 Part 1,
//! §21.4.3 dataModel: `dgm:pt` points and `dgm:cxn` parent-of connections) with BlueOffice's own layouts
//! for the layout categories of §21.4.2 (`dgm:cat@type`: list, process, cycle, hierarchy, matrix,
//! pyramid, relationship). The category comes from the layout part's category list or, without one, the
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

/// BlueOffice diagram colours (one per top-level node, repeating).
const PALETTE: [&str; 6] = ["1F6FB2", "2E8B57", "C2571A", "7B4BA0", "B8860B", "2A9D8F"];

struct Out {
    xml: String,
    n: usize,
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
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl Out {
    fn shape(&mut self, s: &Spec<'_>) {
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
        let line = if s.prst == "line" {
            r#"<a:ln w="19050"><a:solidFill><a:srgbClr val="7F7F7F"/></a:solidFill></a:ln>"#
        } else {
            r#"<a:ln w="12700"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:ln>"#
        };
        let _ = write!(
            self.xml,
            r#"<dsp:sp modelId="{{{n}}}"><dsp:nvSpPr><dsp:cNvPr id="{n}" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm{rot}><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="{prst}"><a:avLst/></a:prstGeom>{fill}{line}</dsp:spPr>"#,
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
            for (i, t) in s.text.iter().enumerate() {
                let size = if i == 0 { pt * 100 } else { pt * 80 };
                let _ = write!(
                    self.xml,
                    r#"<a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="vi-VN" sz="{size}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r></a:p>"#,
                    s.color,
                    esc(t)
                );
            }
            self.xml.push_str("</dsp:txBody>");
        }
        self.xml.push_str("</dsp:sp>");
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
#[allow(clippy::too_many_lines)]
pub fn layout_shapes(nodes: &[DiagramNode], cat: Category, w: i64, h: i64) -> Vec<ShapeNode> {
    let mut out = Out {
        xml: String::new(),
        n: 0,
    };
    let n = nodes.len() as i64;
    if n > 0 && w > 0 && h > 0 {
        match cat {
            Category::List => {
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
                    });
                }
            }
            Category::Process => {
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
                        });
                    }
                }
            }
            Category::Cycle => {
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
                        text: &node.text,
                        color: "FFFFFF",
                        rot: 0,
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
                        });
                    }
                }
            }
            Category::Hierarchy => {
                // Tree: leaves share the width, a parent centred over its children, levels top-down.
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
                    color: &str,
                ) -> i64 {
                    let span = leaves(n) as i64;
                    let cx = first_leaf as i64 * lw + span * lw / 2;
                    let top = level * lh + (lh - bh) / 2;
                    out.shape(&Spec {
                        rect: (cx - bw / 2, top, bw, bh),
                        prst: "roundRect",
                        fill: Some(color),
                        alpha: 100_000,
                        text: &n.text,
                        color: "FFFFFF",
                        rot: 0,
                    });
                    let mut leaf = first_leaf;
                    let mut centres = Vec::new();
                    for c in &n.children {
                        centres.push(place(out, c, leaf, level + 1, lw, lh, bw, bh, color));
                        leaf += leaves(c);
                    }
                    if let (Some(&a), Some(&b)) = (centres.first(), centres.last()) {
                        // Elbow connectors: down from the parent, across, down into each child.
                        let mid = top + bh + (lh - bh) / 2;
                        out.line(cx, top + bh, cx, mid);
                        if a != b {
                            out.line(a, mid, b, mid);
                        }
                        let child_top = (level + 1) * lh + (lh - bh) / 2;
                        for c in centres {
                            out.line(c, mid, c, child_top);
                        }
                    }
                    cx
                }
                let total: usize = nodes.iter().map(leaves).sum();
                let levels = nodes.iter().map(depth).max().unwrap_or(1) as i64;
                let lw = w / total.max(1) as i64;
                let lh = h / levels;
                let (bw, bh) = (lw * 8 / 10, lh * 6 / 10);
                let mut leaf = 0;
                for (i, node) in nodes.iter().enumerate() {
                    let color = PALETTE[i % PALETTE.len()];
                    place(&mut out, node, leaf, 0, lw, lh, bw, bh, color);
                    leaf += leaves(node);
                }
            }
            Category::Matrix => {
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
                    });
                }
            }
            Category::Pyramid => {
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
                        text: &node.text,
                        color: "FFFFFF",
                        rot: 0,
                    });
                }
            }
            Category::Relationship => {
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
                        text: &node.text,
                        color: "1F1F1F",
                        rot: 0,
                    });
                }
            }
        }
    }
    let xml = format!(
        r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dsp:spTree><dsp:nvGrpSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr><dsp:grpSpPr/>{}</dsp:spTree></dsp:drawing>"#,
        out.xml
    );
    crate::diagram::drawing_shapes(xml.as_bytes(), "/diagram-layout.xml").unwrap_or_default()
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
    let shapes = layout_shapes(&nodes, category(lo.as_deref(), layout_xml), w, h);
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
}
