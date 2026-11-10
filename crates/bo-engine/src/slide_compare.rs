//! Compare two versions of a presentation (CORE-VERSION-001): per slide, the
//! slides and shapes added, removed and changed.
//!
//! Our own design (no spec covers comparing decks):
//!
//! * slides are paired by `p:sldId/@id` (ECMA-376 Part 1, §19.2.1.33), which a
//!   presentation keeps for a slide across saves, when both decks have unique
//!   ids; otherwise by aligning slide signatures (titles and texts) with
//!   [`bo_sheet::diff::align`]. A paired slide whose place changed relative to
//!   the others is reported as moved;
//! * shapes of a paired slide (group members included) are paired by their
//!   `cNvPr/@id` (§20.1.2.2.8), then by name; for each pair the text, the
//!   position / size / rotation (`a:xfrm`, §20.1.7.6), the formatting (shape
//!   properties §20.1.2.2.35, shape style, paragraph and run properties) and
//!   the content (image bytes, table cells, chart data) are compared;
//! * slide layout, background (§19.3.1.1), transition (§19.3.1.50) and speaker
//!   notes (§19.3.1.26) changes are reported per slide.

use std::collections::HashMap;

use bo_model::slide::{
    GraphicContent, Paragraph, Presentation, ShapeNode, Slide, TextBody, TextRun, Xfrm,
};
use bo_sheet::diff::{align, Step};
use serde::Serialize;

/// What changed in a deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DeckChangeKind {
    /// A slide exists only in the new deck.
    SlideAdded,
    /// A slide exists only in the old deck.
    SlideRemoved,
    /// A slide changed its place in the deck.
    SlideMoved,
    /// Another slide layout.
    LayoutChanged,
    /// Another background.
    BackgroundChanged,
    /// Another transition.
    TransitionChanged,
    /// Speaker notes changed.
    NotesChanged,
    /// A shape exists only on the new slide.
    ShapeAdded,
    /// A shape exists only on the old slide.
    ShapeRemoved,
    /// Text of a shape (or table) changed.
    TextChanged,
    /// Position changed.
    ShapeMoved,
    /// Size changed.
    ShapeResized,
    /// Rotation or flip changed.
    ShapeRotated,
    /// Fill, outline, effects, geometry, style or text formatting changed.
    FormatChanged,
    /// Image, chart or other embedded content changed.
    ContentChanged,
}

/// One difference between two decks.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckChange {
    /// Kind of change.
    pub kind: DeckChangeKind,
    /// Zero-based slide index in the new deck (`None`: removed slide).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide: Option<usize>,
    /// Zero-based slide index in the old deck (`None`: added slide).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_slide: Option<usize>,
    /// Shape name (shape changes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    /// Shape id in the new slide (`cNvPr/@id`; the old id for removed shapes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape_id: Option<u32>,
    /// Old value (text, position in points, slide title …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    /// New value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
}

impl DeckChange {
    fn new(kind: DeckChangeKind, slide: Option<usize>, old_slide: Option<usize>) -> Self {
        DeckChange {
            kind,
            slide,
            old_slide,
            shape: None,
            shape_id: None,
            old: None,
            new: None,
        }
    }
}

/// Longest text shown as an old / new value.
const MAX_TEXT: usize = 200;

fn clip(s: &str) -> String {
    if s.chars().count() <= MAX_TEXT {
        s.to_owned()
    } else {
        let mut t: String = s.chars().take(MAX_TEXT).collect();
        t.push('…');
        t
    }
}

fn hash_of<T: std::hash::Hash + ?Sized>(v: &T) -> u64 {
    use std::hash::Hasher as _;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// A shape of a slide, flattened out of its groups.
struct Flat<'a> {
    id: u32,
    name: String,
    node: &'a ShapeNode,
}

fn flatten<'a>(nodes: &'a [ShapeNode], out: &mut Vec<Flat<'a>>) {
    for n in nodes {
        if let Some(nv) = n.nv() {
            out.push(Flat {
                id: nv.id,
                name: nv.name.clone(),
                node: n,
            });
        }
        if let ShapeNode::Group(g) = n {
            flatten(&g.children, out);
        }
    }
}

fn table_text(t: &bo_model::slide::Table) -> String {
    t.rows
        .iter()
        .map(|r| {
            r.cells
                .iter()
                .map(|c| c.text.as_ref().map(TextBody::text).unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn text_of(n: &ShapeNode) -> Option<String> {
    match n {
        ShapeNode::Shape(s) => s.text.as_ref().map(TextBody::text),
        ShapeNode::Frame(f) => match &f.content {
            GraphicContent::Table(t) => Some(table_text(t)),
            GraphicContent::Other(_) => None,
        },
        _ => None,
    }
}

fn xfrm_of(n: &ShapeNode) -> Option<&Xfrm> {
    match n {
        ShapeNode::Shape(s) => s.props.xfrm.as_ref(),
        ShapeNode::Picture(p) => p.props.xfrm.as_ref(),
        ShapeNode::Group(g) => g.props.xfrm.as_ref(),
        ShapeNode::Connector(c) => c.props.xfrm.as_ref(),
        ShapeNode::Frame(f) => Some(&f.xfrm),
        ShapeNode::Other(_) => None,
    }
}

/// Paragraph and run formatting of a text body with the text left out.
fn text_format(t: &TextBody) -> String {
    let para = |p: &Paragraph| {
        let runs: Vec<String> = p
            .runs
            .iter()
            .map(|r| match r {
                TextRun::Run { props, .. } | TextRun::Break { props } => format!("{props:?}"),
                TextRun::Field { props, kind, .. } => format!("{kind:?}{props:?}"),
                TextRun::Other(x) => format!("{x:?}"),
            })
            .collect();
        format!("{:?}{runs:?}{:?}", p.props, p.end_props)
    };
    format!(
        "{:?}{:?}{:?}",
        t.body_pr,
        t.list_style,
        t.paragraphs.iter().map(para).collect::<Vec<_>>()
    )
}

/// Formatting of a shape (everything but its position, text and content).
fn format_of(n: &ShapeNode) -> String {
    let props = |p: &bo_model::slide::ShapeProps| {
        format!("{:?}{:?}{:?}{:?}", p.geometry, p.fill, p.line, p.effects)
    };
    match n {
        ShapeNode::Shape(s) => format!(
            "{}{:?}{}",
            props(&s.props),
            s.style,
            s.text.as_ref().map(text_format).unwrap_or_default()
        ),
        ShapeNode::Picture(p) => format!(
            "{}{:?}{:?}",
            props(&p.props),
            p.style,
            (p.blip.src_rect, &p.blip.blip_children)
        ),
        ShapeNode::Connector(c) => format!("{}{:?}", props(&c.props), c.style),
        ShapeNode::Group(g) => props(&g.props),
        ShapeNode::Frame(f) => match &f.content {
            GraphicContent::Table(t) => {
                let cells: Vec<String> = t
                    .rows
                    .iter()
                    .flat_map(|r| r.cells.iter())
                    .map(|c| {
                        format!(
                            "{:?}{:?}{}",
                            c.props,
                            (c.grid_span, c.row_span),
                            c.text.as_ref().map(text_format).unwrap_or_default()
                        )
                    })
                    .collect();
                format!("{:?}{:?}{cells:?}", t.props, t.grid)
            }
            GraphicContent::Other(_) => String::new(),
        },
        ShapeNode::Other(_) => String::new(),
    }
}

/// Embedded content: image bytes, chart / diagram XML.
fn content_of(n: &ShapeNode, slide: &Slide, pres: &Presentation) -> u64 {
    match n {
        ShapeNode::Picture(p) => {
            let part = p.blip.embed.as_ref().and_then(|id| {
                let rel = slide.info.rels.iter().find(|r| &r.id == id)?;
                let name = bo_opc::resolve_target(&slide.info.part_name, &rel.target);
                pres.part_data(&name).map(hash_of)
            });
            part.unwrap_or_else(|| hash_of(&format!("{:?}{:?}", p.blip.embed, p.blip.link)))
        }
        ShapeNode::Frame(f) => match &f.content {
            GraphicContent::Other(x) => hash_of(&format!("{}{x:?}", f.uri)),
            GraphicContent::Table(_) => 0,
        },
        ShapeNode::Other(x) => hash_of(&format!("{x:?}")),
        _ => 0,
    }
}

/// Title of a slide (first title placeholder, else its first text).
fn title_of(s: &Slide) -> String {
    let mut flat = Vec::new();
    flatten(&s.data.shapes, &mut flat);
    let title = flat.iter().find(|f| {
        f.node
            .nv()
            .and_then(|nv| nv.placeholder.as_ref())
            .and_then(|p| p.kind.as_deref())
            .is_some_and(|k| k == "title" || k == "ctrTitle")
    });
    title
        .or_else(|| {
            flat.iter()
                .find(|f| text_of(f.node).is_some_and(|t| !t.trim().is_empty()))
        })
        .and_then(|f| text_of(f.node))
        .map(|t| clip(t.lines().next().unwrap_or("")))
        .unwrap_or_default()
}

/// Texts of a slide, for pairing slides without usable ids.
fn texts_of(s: &Slide) -> Vec<String> {
    let mut flat = Vec::new();
    flatten(&s.data.shapes, &mut flat);
    let mut v: Vec<String> = flat.iter().filter_map(|f| text_of(f.node)).collect();
    v.sort_unstable();
    v
}

fn texts_sim(a: &[String], b: &[String]) -> f64 {
    let max = a.len().max(b.len());
    if max == 0 {
        return 1.0;
    }
    let common = a.iter().filter(|t| b.contains(t)).count();
    common as f64 / max as f64
}

fn emu_pt(v: i64) -> String {
    let pt = v as f64 / 12_700.0;
    if pt.fract().abs() < 0.05 {
        format!("{}", pt.round() as i64)
    } else {
        format!("{pt:.1}")
    }
}

fn pair_text(p: Option<(i64, i64)>) -> Option<String> {
    p.map(|(a, b)| format!("{} × {} pt", emu_pt(a), emu_pt(b)))
}

/// Pairs the slides of two decks: `(old index, new index)` in new order, plus
/// the unpaired ones.
fn pair_slides(old: &Presentation, new: &Presentation) -> Vec<Step> {
    let ids_ok = |p: &Presentation| {
        let mut ids: Vec<u32> = p.slides.iter().map(|s| s.slide_id).collect();
        ids.sort_unstable();
        ids.iter().all(|&i| i != 0) && ids.windows(2).all(|w| w[0] != w[1])
    };
    if ids_ok(old) && ids_ok(new) {
        let by_id: HashMap<u32, usize> = old
            .slides
            .iter()
            .enumerate()
            .map(|(i, s)| (s.slide_id, i))
            .collect();
        let mut steps: Vec<Step> = new
            .slides
            .iter()
            .enumerate()
            .map(|(j, s)| {
                by_id
                    .get(&s.slide_id)
                    .map_or(Step::New(j), |&i| Step::Both(i, j))
            })
            .collect();
        let paired: std::collections::HashSet<usize> = steps
            .iter()
            .filter_map(|s| match s {
                Step::Both(i, _) => Some(*i),
                _ => None,
            })
            .collect();
        steps.extend(
            (0..old.slides.len())
                .filter(|i| !paired.contains(i))
                .map(Step::Old),
        );
        return steps;
    }
    let ot: Vec<Vec<String>> = old.slides.iter().map(texts_of).collect();
    let nt: Vec<Vec<String>> = new.slides.iter().map(texts_of).collect();
    let oh: Vec<u64> = ot.iter().map(hash_of).collect();
    let nh: Vec<u64> = nt.iter().map(hash_of).collect();
    align(&oh, &nh, &|i, j| texts_sim(&ot[i], &nt[j]))
}

/// Old indices of paired slides that keep their relative order (longest
/// increasing run); the others moved.
fn kept_order(pairs: &[(usize, usize)]) -> std::collections::HashSet<usize> {
    // pairs are in new order; find the LIS of the old indices.
    let mut tails: Vec<usize> = Vec::new();
    let mut prev = vec![usize::MAX; pairs.len()];
    for k in 0..pairs.len() {
        let pos = tails.partition_point(|&t| pairs[t].0 < pairs[k].0);
        if pos > 0 {
            prev[k] = tails[pos - 1];
        }
        if pos == tails.len() {
            tails.push(k);
        } else {
            tails[pos] = k;
        }
    }
    let mut keep = std::collections::HashSet::new();
    let mut k = tails.last().copied().unwrap_or(usize::MAX);
    while k != usize::MAX {
        keep.insert(pairs[k].0);
        k = prev[k];
    }
    keep
}

/// Differences between two decks (see the module docs), in new slide order;
/// removed slides first.
pub fn diff_decks(old: &Presentation, new: &Presentation) -> Vec<DeckChange> {
    let steps = pair_slides(old, new);
    let mut out = Vec::new();
    for s in &steps {
        if let Step::Old(i) = *s {
            out.push(DeckChange {
                old: Some(title_of(&old.slides[i])),
                ..DeckChange::new(DeckChangeKind::SlideRemoved, None, Some(i))
            });
        }
    }
    let mut pairs: Vec<(usize, usize)> = steps
        .iter()
        .filter_map(|s| match *s {
            Step::Both(i, j) => Some((i, j)),
            _ => None,
        })
        .collect();
    pairs.sort_by_key(|p| p.1);
    let keep = kept_order(&pairs);
    let old_of: HashMap<usize, usize> = pairs.iter().map(|&(i, j)| (j, i)).collect();
    for j in 0..new.slides.len() {
        let Some(&i) = old_of.get(&j) else {
            out.push(DeckChange {
                new: Some(title_of(&new.slides[j])),
                ..DeckChange::new(DeckChangeKind::SlideAdded, Some(j), None)
            });
            continue;
        };
        if !keep.contains(&i) {
            out.push(DeckChange {
                old: Some(format!("{}", i + 1)),
                new: Some(format!("{}", j + 1)),
                ..DeckChange::new(DeckChangeKind::SlideMoved, Some(j), Some(i))
            });
        }
        diff_slide(old, new, i, j, &mut out);
    }
    out
}

fn layout_name(p: &Presentation, s: &Slide) -> String {
    p.layouts
        .get(s.layout)
        .map(|l| {
            l.data
                .name
                .clone()
                .or_else(|| l.kind.clone())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

fn diff_slide(
    old: &Presentation,
    new: &Presentation,
    i: usize,
    j: usize,
    out: &mut Vec<DeckChange>,
) {
    let (os, ns) = (&old.slides[i], &new.slides[j]);
    let at = |kind| DeckChange::new(kind, Some(j), Some(i));
    let (ol, nl) = (layout_name(old, os), layout_name(new, ns));
    if ol != nl {
        out.push(DeckChange {
            old: Some(ol),
            new: Some(nl),
            ..at(DeckChangeKind::LayoutChanged)
        });
    }
    if os.data.background != ns.data.background {
        out.push(at(DeckChangeKind::BackgroundChanged));
    }
    if os.transition != ns.transition {
        out.push(at(DeckChangeKind::TransitionChanged));
    }
    let notes = |s: &Slide| {
        s.notes
            .as_ref()
            .map(bo_model::slide::Notes::text)
            .unwrap_or_default()
    };
    let (on, nn) = (notes(os), notes(ns));
    if on.trim() != nn.trim() {
        out.push(DeckChange {
            old: Some(clip(&on)),
            new: Some(clip(&nn)),
            ..at(DeckChangeKind::NotesChanged)
        });
    }
    let (mut of, mut nf) = (Vec::new(), Vec::new());
    flatten(&os.data.shapes, &mut of);
    flatten(&ns.data.shapes, &mut nf);
    // Pair shapes: by id (ids unique on both sides), then by name.
    let mut used = vec![false; of.len()];
    let mut pair: Vec<Option<usize>> = vec![None; nf.len()];
    let unique_id = |v: &[Flat], id: u32| v.iter().filter(|f| f.id == id).count() == 1;
    for (b, f) in nf.iter().enumerate() {
        if !unique_id(&nf, f.id) || !unique_id(&of, f.id) {
            continue;
        }
        if let Some(a) = of.iter().position(|o| o.id == f.id) {
            pair[b] = Some(a);
            used[a] = true;
        }
    }
    for (b, f) in nf.iter().enumerate() {
        if pair[b].is_some() || f.name.is_empty() {
            continue;
        }
        if let Some(a) = (0..of.len()).find(|&a| !used[a] && of[a].name == f.name) {
            pair[b] = Some(a);
            used[a] = true;
        }
    }
    let shape = |kind, f: &Flat| DeckChange {
        shape: Some(f.name.clone()),
        shape_id: Some(f.id),
        ..at(kind)
    };
    for (a, f) in of.iter().enumerate() {
        if !used[a] {
            out.push(DeckChange {
                old: text_of(f.node).map(|t| clip(&t)),
                ..shape(DeckChangeKind::ShapeRemoved, f)
            });
        }
    }
    for (b, f) in nf.iter().enumerate() {
        let Some(a) = pair[b] else {
            out.push(DeckChange {
                new: text_of(f.node).map(|t| clip(&t)),
                ..shape(DeckChangeKind::ShapeAdded, f)
            });
            continue;
        };
        let o = &of[a];
        let (ot, nt) = (text_of(o.node), text_of(f.node));
        if ot != nt {
            out.push(DeckChange {
                old: ot.map(|t| clip(&t)),
                new: nt.map(|t| clip(&t)),
                ..shape(DeckChangeKind::TextChanged, f)
            });
        }
        let (ox, nx) = (xfrm_of(o.node), xfrm_of(f.node));
        let get = |x: Option<&Xfrm>, k: fn(&Xfrm) -> Option<(i64, i64)>| x.and_then(k);
        let (ooff, noff) = (get(ox, |x| x.off), get(nx, |x| x.off));
        if ooff != noff {
            out.push(DeckChange {
                old: pair_text(ooff),
                new: pair_text(noff),
                ..shape(DeckChangeKind::ShapeMoved, f)
            });
        }
        let (oext, next) = (get(ox, |x| x.ext), get(nx, |x| x.ext));
        if oext != next {
            out.push(DeckChange {
                old: pair_text(oext),
                new: pair_text(next),
                ..shape(DeckChangeKind::ShapeResized, f)
            });
        }
        let turn = |x: Option<&Xfrm>| {
            x.map(|x| {
                (
                    x.rot.unwrap_or(0),
                    x.flip_h.unwrap_or(false),
                    x.flip_v.unwrap_or(false),
                )
            })
        };
        let (or, nr) = (turn(ox), turn(nx));
        if or.unwrap_or_default() != nr.unwrap_or_default() {
            let deg = |r: Option<(i32, bool, bool)>| {
                r.map(|(rot, _, _)| format!("{}°", f64::from(rot) / 60_000.0))
            };
            out.push(DeckChange {
                old: deg(or),
                new: deg(nr),
                ..shape(DeckChangeKind::ShapeRotated, f)
            });
        }
        if format_of(o.node) != format_of(f.node) {
            out.push(shape(DeckChangeKind::FormatChanged, f));
        }
        if content_of(o.node, os, old) != content_of(f.node, ns, new) {
            out.push(shape(DeckChangeKind::ContentChanged, f));
        }
    }
}

/// At most this many changes are returned.
pub const MAX_CHANGES: usize = 5_000;

/// Reads another version of a deck: `.pptx`, or `.odp` / `.ppt` converted.
pub fn read_deck(bytes: &[u8]) -> crate::Result<Presentation> {
    use crate::EngineError;
    let is_zip = bytes.starts_with(b"PK\x03\x04");
    let spec = if is_zip && bo_pptx::is_pptx(bytes) {
        return Ok(bo_pptx::read_pptx(bytes)?);
    } else if is_zip && bo_odf::detect_mimetype(bytes).as_deref() == Some(bo_odf::MIME_ODP) {
        let d = bo_odf::read_odp_deck(bytes).map_err(|e| EngineError::Format(e.to_string()))?;
        crate::legacy_slides::odp_deck(&d)
    } else if bytes.starts_with(crate::CFB_SIGNATURE)
        && matches!(bo_binary::detect(bytes), Some(bo_binary::LegacyKind::Ppt))
    {
        let d = bo_binary::read_ppt(bytes).map_err(|e| EngineError::Format(e.to_string()))?;
        crate::legacy_slides::ppt_deck(&d)
    } else {
        return Err(EngineError::Format("not a presentation".into()));
    };
    let pptx = bo_pptx::build::build_pptx(&spec).map_err(|e| EngineError::Format(e.to_string()))?;
    Ok(bo_pptx::read_pptx(&pptx)?)
}

impl crate::Engine {
    /// `{"cmd":"compareDeck","name","base64"}` (a query): the open deck is the
    /// new side → `{ deckChanges: [DeckChange], total, truncated }`. `None`
    /// for other commands.
    pub(crate) fn compare_deck_json(&self, json: &str) -> Option<crate::Result<String>> {
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        if v.get("cmd")?.as_str()? != "compareDeck" {
            return None;
        }
        Some((|| {
            let new = self.presentation().ok_or(crate::EngineError::NoDocument)?;
            let b64 = v.get("base64").and_then(|b| b.as_str()).unwrap_or("");
            let bytes = crate::merge::base64_decode(b64)
                .ok_or_else(|| crate::EngineError::BadCommand("base64".into()))?;
            let old = read_deck(&bytes)?;
            let changes = diff_decks(&old, new);
            let total = changes.len();
            let shown: Vec<_> = changes.into_iter().take(MAX_CHANGES).collect();
            Ok(serde_json::json!({
                "deckChanges": shown,
                "total": total,
                "truncated": total > MAX_CHANGES,
            })
            .to_string())
        })())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::slide::{NonVisual, Shape, ShapeProps};

    fn text(s: &str) -> TextBody {
        TextBody {
            paragraphs: s
                .split('\n')
                .map(|p| Paragraph {
                    runs: vec![TextRun::Run {
                        props: bo_model::slide::RunProps::default(),
                        text: p.to_owned(),
                    }],
                    ..Paragraph::default()
                })
                .collect(),
            ..TextBody::default()
        }
    }

    fn shape(id: u32, name: &str, t: &str, off: (i64, i64)) -> ShapeNode {
        ShapeNode::Shape(Box::new(Shape {
            nv: NonVisual {
                id,
                name: name.to_owned(),
                ..NonVisual::default()
            },
            props: ShapeProps {
                xfrm: Some(Xfrm {
                    off: Some(off),
                    ext: Some((127_000, 63_500)),
                    ..Xfrm::default()
                }),
                ..ShapeProps::default()
            },
            text: Some(text(t)),
            ..Shape::default()
        }))
    }

    fn slide(id: u32, shapes: Vec<ShapeNode>) -> Slide {
        let mut s = Slide {
            slide_id: id,
            ..Slide::default()
        };
        s.data.shapes = shapes;
        s
    }

    fn deck(slides: Vec<Slide>) -> Presentation {
        Presentation {
            slides,
            ..Presentation::default()
        }
    }

    fn summary(ch: &[DeckChange]) -> Vec<String> {
        ch.iter()
            .map(|c| {
                format!(
                    "{:?} {}{}{}{}",
                    c.kind,
                    c.slide.map_or("-".into(), |s| (s + 1).to_string()),
                    c.shape
                        .as_ref()
                        .map(|s| format!(" {s}"))
                        .unwrap_or_default(),
                    c.old
                        .as_ref()
                        .map(|o| format!(" [{o}]"))
                        .unwrap_or_default(),
                    c.new
                        .as_ref()
                        .map(|n| format!(" -> [{n}]"))
                        .unwrap_or_default(),
                )
            })
            .collect()
    }

    fn base() -> Presentation {
        deck(vec![
            slide(256, vec![shape(2, "Title 1", "Kế hoạch", (0, 0))]),
            slide(
                257,
                vec![
                    shape(2, "Title 1", "Mục tiêu", (0, 0)),
                    shape(3, "Body", "Doanh thu", (0, 254_000)),
                ],
            ),
            slide(258, vec![shape(2, "Title 1", "Kết luận", (0, 0))]),
        ])
    }

    #[test]
    fn same_deck_has_no_changes() {
        assert_eq!(diff_decks(&base(), &base()), Vec::<DeckChange>::new());
    }

    #[test]
    fn slides_added_removed_and_moved() {
        let old = base();
        let mut new = base();
        // Slide 3 moved to the front, slide 1 removed, a new slide appended.
        let last = new.slides.remove(2);
        new.slides.insert(0, last);
        new.slides.remove(1);
        new.slides
            .push(slide(300, vec![shape(2, "Title 1", "Phụ lục", (0, 0))]));
        // New order: Kết luận (old 3), Mục tiêu (old 2), Phụ lục (new).
        assert_eq!(
            summary(&diff_decks(&old, &new)),
            vec![
                "SlideRemoved - [Kế hoạch]",
                "SlideMoved 1 [3] -> [1]",
                "SlideAdded 3 -> [Phụ lục]",
            ]
        );
    }

    #[test]
    fn shape_text_position_size_and_format() {
        let old = base();
        let mut new = base();
        let shapes = &mut new.slides[1].data.shapes;
        if let ShapeNode::Shape(s) = &mut shapes[1] {
            s.text = Some(text("Doanh thu 2026"));
            let x = s.props.xfrm.as_mut().expect("xfrm");
            // Moved 1 in (72 pt) right and made twice as wide.
            x.off = Some((914_400, 254_000));
            x.ext = Some((254_000, 63_500));
            x.rot = Some(5_400_000);
        }
        if let ShapeNode::Shape(s) = &mut shapes[0] {
            if let Some(TextRun::Run { props, .. }) = s
                .text
                .as_mut()
                .and_then(|t| t.paragraphs[0].runs.first_mut())
            {
                props.b = Some(true);
            }
        }
        shapes.push(shape(4, "Note", "Mới", (0, 508_000)));
        new.slides[2].data.shapes.clear();
        assert_eq!(
            summary(&diff_decks(&old, &new)),
            vec![
                "FormatChanged 2 Title 1",
                "TextChanged 2 Body [Doanh thu] -> [Doanh thu 2026]",
                "ShapeMoved 2 Body [0 × 20 pt] -> [72 × 20 pt]",
                "ShapeResized 2 Body [10 × 5 pt] -> [20 × 5 pt]",
                "ShapeRotated 2 Body [0°] -> [90°]",
                "ShapeAdded 2 Note -> [Mới]",
                "ShapeRemoved 3 Title 1 [Kết luận]",
            ]
        );
    }

    #[test]
    fn compare_deck_command_reads_pptx_and_ppt() {
        use base64::Engine as _;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/slide");
        let pptx = std::fs::read(dir.join("slide-text-bullets.pptx")).expect("corpus");
        let mut e = crate::Engine::new();
        e.open_pptx(&pptx, "a.pptx").expect("open");
        let enc = base64::engine::general_purpose::STANDARD.encode(&pptx);
        let r: serde_json::Value = serde_json::from_str(
            &e.slide_command_json(&format!(
                r#"{{"cmd":"compareDeck","name":"a.pptx","base64":"{enc}"}}"#
            ))
            .expect("compare"),
        )
        .expect("json");
        assert_eq!(r["total"], 0);
        // A converted legacy deck differs from this one: every slide pairing fails or changes.
        let ppt = std::fs::read(dir.join("binary-ppt-01-shapes.ppt")).expect("corpus");
        let enc = base64::engine::general_purpose::STANDARD.encode(&ppt);
        let r: serde_json::Value = serde_json::from_str(
            &e.slide_command_json(&format!(
                r#"{{"cmd":"compareDeck","name":"b.ppt","base64":"{enc}"}}"#
            ))
            .expect("compare"),
        )
        .expect("json");
        assert!(r["total"].as_u64().unwrap_or(0) > 0);
        assert!(e
            .slide_command_json(r#"{"cmd":"compareDeck","name":"x.pptx","base64":"bm9wZQ=="}"#)
            .is_err());
    }

    #[test]
    fn decks_without_ids_pair_slides_by_text() {
        let strip = |mut p: Presentation| {
            for s in &mut p.slides {
                s.slide_id = 0;
            }
            p
        };
        let old = strip(base());
        let mut new = strip(base());
        new.slides
            .insert(1, slide(0, vec![shape(2, "Title 1", "Bối cảnh", (0, 0))]));
        if let ShapeNode::Shape(s) = &mut new.slides[3].data.shapes[0] {
            s.text = Some(text("Tổng kết"));
        }
        assert_eq!(
            summary(&diff_decks(&old, &new)),
            vec![
                "SlideAdded 2 -> [Bối cảnh]",
                "TextChanged 4 Title 1 [Kết luận] -> [Tổng kết]",
            ]
        );
    }
}
