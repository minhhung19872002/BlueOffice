//! Page decoration: the page colour (ECMA-376 Part 1, §17.2.1 background) and the
//! page borders of each section (§17.6.10 pgBorders) — line borders as rules,
//! art borders (§17.18.2 `ST_Border` art values) as repeated vector motifs of
//! BlueOffice's own design; unknown art values fall back to a plain line.

use crate::types::{Layer, LineSource, Page, PageDecor, PageItem, PathCmd, ShapePath};
use bo_model::{Border, Color, Document, PageBorder, PageBorderDisplay, PageBorders, PageSetup};

/// A repeated motif of an art border (BlueOffice designs: simple geometric shapes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtMotif {
    /// Filled round dots.
    Dots,
    /// Hollow rings.
    Rings,
    /// Filled squares.
    Squares,
    /// Hollow squares.
    HollowSquares,
    /// Filled diamonds.
    Diamonds,
    /// Filled triangles pointing away from the page.
    Triangles,
    /// A zigzag line.
    Zigzag,
    /// A wavy line.
    Waves,
    /// Two-by-two checks.
    Checks,
    /// Saw teeth along the edge.
    Sawtooth,
}

impl ArtMotif {
    /// Every motif, in gallery order.
    pub const ALL: [ArtMotif; 10] = [
        ArtMotif::Dots,
        ArtMotif::Rings,
        ArtMotif::Squares,
        ArtMotif::HollowSquares,
        ArtMotif::Diamonds,
        ArtMotif::Triangles,
        ArtMotif::Zigzag,
        ArtMotif::Waves,
        ArtMotif::Checks,
        ArtMotif::Sawtooth,
    ];

    /// The motif drawn for an `ST_Border` art value (§17.18.2); `None` for line
    /// styles and art values without a BlueOffice motif.
    pub fn of(style: &str) -> Option<ArtMotif> {
        Some(match style {
            "basicBlackDots" | "couponCutoutDots" => ArtMotif::Dots,
            "basicWhiteDots" | "rings" | "ovals" => ArtMotif::Rings,
            "basicBlackSquares" | "shadowedSquares" => ArtMotif::Squares,
            "basicWhiteSquares" | "eclipsingSquares1" => ArtMotif::HollowSquares,
            "diamondsGray" | "doubleDiamonds" | "gems" => ArtMotif::Diamonds,
            "triangles" | "triangle1" | "triangle2" | "pyramids" | "pyramidsAbove" => {
                ArtMotif::Triangles
            }
            "zigZag" | "zigZagStitch" => ArtMotif::Zigzag,
            "waveline" | "classicalWave" => ArtMotif::Waves,
            "checkered" | "checkedBarBlack" | "quadrants" => ArtMotif::Checks,
            "sawtooth" | "sawtoothGray" | "sharksTeeth" => ArtMotif::Sawtooth,
            _ => return None,
        })
    }

    /// The `ST_Border` value BlueOffice writes for the motif.
    pub fn style(self) -> &'static str {
        match self {
            ArtMotif::Dots => "basicBlackDots",
            ArtMotif::Rings => "basicWhiteDots",
            ArtMotif::Squares => "basicBlackSquares",
            ArtMotif::HollowSquares => "basicWhiteSquares",
            ArtMotif::Diamonds => "diamondsGray",
            ArtMotif::Triangles => "triangles",
            ArtMotif::Zigzag => "zigZag",
            ArtMotif::Waves => "waveline",
            ArtMotif::Checks => "checkered",
            ArtMotif::Sawtooth => "sawtooth",
        }
    }
}

/// Width of a border in points: `w:sz` is in eighths of a point for line styles
/// (§17.18.2 ST_EighthPointMeasure) and in points for art borders.
pub fn border_width(b: &Border) -> f32 {
    if b.is_art() {
        b.size.clamp(1, 31) as f32
    } else {
        (b.size as f32 / 8.0).max(0.25)
    }
}

/// Positions (points) of the centre lines of the four edges: top y, left x,
/// bottom y, right x (§17.6.10: `w:space` from the page edge with
/// `offsetFrom="page"`, else from the text area given by the margins).
pub fn border_lines(pb: &PageBorders, setup: &PageSetup) -> [f32; 4] {
    let (pw, ph) = (setup.width.to_pt(), setup.height.to_pt());
    let half = |e: Option<&PageBorder>| {
        e.filter(|b| b.line.is_visible())
            .map_or((0.0, 0.0), |b| (border_width(&b.line), b.line.space as f32))
    };
    let [t, l, b, r] = pb.edges().map(half);
    if pb.offset_from_page {
        [
            t.1 + t.0 / 2.0,
            l.1 + l.0 / 2.0,
            ph - b.1 - b.0 / 2.0,
            pw - r.1 - r.0 / 2.0,
        ]
    } else {
        let mt = setup.margin_top.to_pt().abs();
        let mb = setup.margin_bottom.to_pt().abs();
        let ml = setup.margin_left.to_pt();
        let mr = setup.margin_right.to_pt();
        [
            mt - t.1 - t.0 / 2.0,
            ml - l.1 - l.0 / 2.0,
            ph - mb + b.1 + b.0 / 2.0,
            pw - mr + r.1 + r.0 / 2.0,
        ]
    }
}

/// Set the page colour and page borders of every page (cheap: no shaping);
/// returns the indexes of the pages whose decoration changed.
pub(crate) fn decorate_pages(doc: &Document, pages: &mut [Page]) -> Vec<usize> {
    let mut changed = Vec::new();
    let mut first_of = vec![usize::MAX; doc.sections.len()];
    for (pi, p) in pages.iter().enumerate() {
        if let Some(f) = first_of.get_mut(p.section) {
            if *f == usize::MAX {
                *f = pi;
            }
        }
    }
    for (pi, p) in pages.iter_mut().enumerate() {
        let mut decor = PageDecor {
            background: doc.design.background,
            ..PageDecor::default()
        };
        if let Some(sec) = doc.sections.get(p.section) {
            if let Some(pb) = sec.page_borders.as_ref().filter(|pb| !pb.is_empty()) {
                let first = first_of.get(p.section) == Some(&pi);
                let shown = match pb.display {
                    PageBorderDisplay::AllPages => true,
                    PageBorderDisplay::FirstPage => first,
                    PageBorderDisplay::NotFirstPage => !first,
                };
                if shown {
                    decor.borders = border_items(pb, &sec.page);
                    decor.borders_behind = pb.behind;
                }
            }
        }
        if p.decor != decor {
            p.decor = decor;
            changed.push(pi);
        }
    }
    changed
}

/// Page items drawing `pb` on a page of `setup`.
pub fn border_items(pb: &PageBorders, setup: &PageSetup) -> Vec<PageItem> {
    fn edge(e: Option<&PageBorder>) -> Option<&PageBorder> {
        e.filter(|b| b.line.is_visible())
    }
    let [top, left, bottom, right] = border_lines(pb, setup);
    let w = |e: Option<&PageBorder>| edge(e).map_or(0.0, |b| border_width(&b.line));
    let (wt, wl, wb, wr) = (
        w(pb.top.as_ref()),
        w(pb.left.as_ref()),
        w(pb.bottom.as_ref()),
        w(pb.right.as_ref()),
    );
    let layer = if pb.behind { Layer::Back } else { Layer::Front };
    let mut items = Vec::new();
    // (edge, horizontal, fixed coordinate, start, end, outward direction sign)
    let sides = [
        (
            pb.top.as_ref(),
            true,
            top,
            left - wl / 2.0,
            right + wr / 2.0,
            -1.0,
        ),
        (
            pb.bottom.as_ref(),
            true,
            bottom,
            left - wl / 2.0,
            right + wr / 2.0,
            1.0,
        ),
        (
            pb.left.as_ref(),
            false,
            left,
            top - wt / 2.0,
            bottom + wb / 2.0,
            -1.0,
        ),
        (
            pb.right.as_ref(),
            false,
            right,
            top - wt / 2.0,
            bottom + wb / 2.0,
            1.0,
        ),
    ];
    for (e, horizontal, at, from, to, out) in sides {
        let Some(b) = edge(e) else { continue };
        let color = b.line.color.unwrap_or(Color::BLACK);
        let width = border_width(&b.line);
        let motif = b
            .line
            .is_art()
            .then(|| ArtMotif::of(&b.line.style))
            .flatten();
        if let Some(m) = motif {
            // The top and bottom edges hold the corner tiles.
            let (from, to) = if horizontal {
                (from, to)
            } else {
                (from + wt, to - wb)
            };
            if to - from > 0.5 {
                items.push(art_edge(m, horizontal, at, from, to, width, color, layer));
            }
            continue;
        }
        // Line styles, and the plain-line fallback of art borders without a motif.
        let (style, lw) = if b.line.is_art() {
            ("single".to_owned(), (width / 6.0).max(0.5))
        } else {
            (b.line.style.clone(), width)
        };
        let rule = |off: f32, lw: f32| {
            if horizontal {
                PageItem::Rule {
                    x1: from,
                    y1: at + off,
                    x2: to,
                    y2: at + off,
                    width: lw,
                    color,
                    style: style.clone(),
                }
            } else {
                PageItem::Rule {
                    x1: at + off,
                    y1: from,
                    x2: at + off,
                    y2: to,
                    width: lw,
                    color,
                    style: style.clone(),
                }
            }
        };
        items.push(rule(0.0, lw));
        // w:shadow: a second line of the same width just outside the bottom and
        // right edges (the light comes from the top left).
        if b.shadow && out > 0.0 {
            items.push(rule(lw, lw));
        }
    }
    items
}

/// One edge of an art border: the motif repeated in square tiles of side `size`
/// along `from..to` (tile size stretched so a whole number fits), centred on `at`.
#[allow(clippy::too_many_arguments)]
fn art_edge(
    m: ArtMotif,
    horizontal: bool,
    at: f32,
    from: f32,
    to: f32,
    size: f32,
    color: Color,
    layer: Layer,
) -> PageItem {
    let len = (to - from).max(size);
    let n = (len / size).round().max(1.0);
    let step = len / n;
    let mut paths = Vec::new();
    for i in 0..n as usize {
        let c = from + step * (i as f32 + 0.5);
        // Tile centre and the axes: `u` along the edge, `v` across it.
        let (cx, cy) = if horizontal { (c, at) } else { (at, c) };
        paths.extend(motif_paths(m, cx, cy, step, size, horizontal, color));
    }
    PageItem::Shape {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
        paths,
        source: LineSource::Other,
        layer,
        alpha: 1.0,
    }
}

/// Paths of one tile centred on (`cx`, `cy`), `along` long and `across` wide.
fn motif_paths(
    m: ArtMotif,
    cx: f32,
    cy: f32,
    along: f32,
    across: f32,
    horizontal: bool,
    color: Color,
) -> Vec<ShapePath> {
    // Tile coordinates (u along the edge, v across it, both from the centre) →
    // page coordinates.
    let p = |u: f32, v: f32| {
        if horizontal {
            (cx + u, cy + v)
        } else {
            (cx + v, cy + u)
        }
    };
    let poly = |pts: &[(f32, f32)], close: bool| {
        let mut segs = Vec::with_capacity(pts.len() + 1);
        for (i, &(u, v)) in pts.iter().enumerate() {
            let (x, y) = p(u, v);
            segs.push(if i == 0 {
                PathCmd::M(x, y)
            } else {
                PathCmd::L(x, y)
            });
        }
        if close {
            segs.push(PathCmd::Z);
        }
        segs
    };
    let fill = |segs: Vec<PathCmd>| ShapePath {
        segs,
        fill: Some(color),
        stroke: None,
    };
    let line = |segs: Vec<PathCmd>, w: f32| ShapePath {
        segs,
        fill: None,
        stroke: Some((w, color, None)),
    };
    let (a, s) = (along / 2.0, across / 2.0);
    let r = s.min(a);
    match m {
        ArtMotif::Dots => vec![fill(circle(p(0.0, 0.0), r * 0.6))],
        ArtMotif::Rings => vec![line(circle(p(0.0, 0.0), r * 0.55), (r * 0.25).max(0.3))],
        ArtMotif::Squares => {
            let k = r * 0.6;
            vec![fill(poly(&[(-k, -k), (k, -k), (k, k), (-k, k)], true))]
        }
        ArtMotif::HollowSquares => {
            let k = r * 0.55;
            vec![line(
                poly(&[(-k, -k), (k, -k), (k, k), (-k, k)], true),
                (r * 0.2).max(0.3),
            )]
        }
        ArtMotif::Diamonds => {
            let (ku, kv) = (a * 0.85, s * 0.85);
            vec![fill(poly(
                &[(-ku, 0.0), (0.0, -kv), (ku, 0.0), (0.0, kv)],
                true,
            ))]
        }
        ArtMotif::Triangles => {
            let (ku, kv) = (a * 0.8, s * 0.75);
            vec![fill(poly(&[(-ku, kv), (0.0, -kv), (ku, kv)], true))]
        }
        ArtMotif::Zigzag => {
            let kv = s * 0.6;
            vec![line(
                poly(&[(-a, kv), (0.0, -kv), (a, kv)], false),
                (across * 0.14).max(0.4),
            )]
        }
        ArtMotif::Waves => {
            let kv = s * 0.55;
            let (x0, y0) = p(-a, 0.0);
            let (c1x, c1y) = p(-a / 2.0, -kv * 2.0);
            let (xm, ym) = p(0.0, 0.0);
            let (c2x, c2y) = p(a / 2.0, kv * 2.0);
            let (x1, y1) = p(a, 0.0);
            vec![line(
                vec![
                    PathCmd::M(x0, y0),
                    PathCmd::Q(c1x, c1y, xm, ym),
                    PathCmd::Q(c2x, c2y, x1, y1),
                ],
                (across * 0.14).max(0.4),
            )]
        }
        ArtMotif::Checks => {
            let (ku, kv) = (a, s);
            vec![
                fill(poly(
                    &[(-ku, -kv), (0.0, -kv), (0.0, 0.0), (-ku, 0.0)],
                    true,
                )),
                fill(poly(&[(0.0, 0.0), (ku, 0.0), (ku, kv), (0.0, kv)], true)),
            ]
        }
        ArtMotif::Sawtooth => vec![fill(poly(&[(-a, s), (a, -s), (a, s)], true))],
    }
}

/// A circle of radius `r` around `c` (four cubic arcs, k = 0.5523).
fn circle((x, y): (f32, f32), r: f32) -> Vec<PathCmd> {
    let k = r * 0.552_284_8;
    vec![
        PathCmd::M(x + r, y),
        PathCmd::C(x + r, y + k, x + k, y + r, x, y + r),
        PathCmd::C(x - k, y + r, x - r, y + k, x - r, y),
        PathCmd::C(x - r, y - k, x - k, y - r, x, y - r),
        PathCmd::C(x + k, y - r, x + r, y - k, x + r, y),
        PathCmd::Z,
    ]
}
