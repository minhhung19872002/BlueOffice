//! Builds: one effect of a shape split into effects on its paragraphs, chart elements or SmartArt
//! nodes (ECMA-376 Part 1, §19.5 bldLst with bldP / bldGraphic; the effects target §19.5 spTgt
//! `txEl` / `bg` / `graphicEl` and share the group of their build, §19.5.29 cTn@grpId).
//!
//! The slide show plays such effects on pieces of the shape's layer: the layer is drawn once with
//! every piece left out (the body) and once per piece; what a piece's drawing adds to the body's
//! is the piece ([`subtract`]).

use bo_model::slide::{
    AnimAfter, AnimTarget, AnimTrigger, Animation, Build, Color, ColorMod, Fill, GraphicBuild,
    GraphicContent, GraphicSub, ParaBuild, Presentation, ShapeNode, TextBody, TextRun, Timing,
};
use bo_render::{Command, PageList, PathSeg};
use serde::Serialize;

use super::{refuse, R};

/// What a shape can be built from.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// A shape with text: outline level (0-based `lvl`) of each paragraph and whether it has text.
    Text(Vec<(u8, bool)>),
    /// A chart: series (across plot groups) and categories; `maskable`: the show can draw its
    /// points one by one.
    Chart {
        /// Number of series.
        series: usize,
        /// Number of categories (points of the longest series).
        cats: usize,
        /// The chart kind draws point by point (`bo_sheet::chart::maskable`).
        maskable: bool,
    },
    /// A SmartArt: its nodes and the node of each drawn shape.
    Diagram(bo_pptx::diagram::DiagramBuild),
}

/// The shape `id` of a slide (anywhere in the shape tree).
fn node_by_id(pres: &Presentation, slide: usize, id: u32) -> Option<&ShapeNode> {
    let s = pres.slides.get(slide)?;
    let mut hit = None;
    bo_pptx::visit(&s.data.shapes, &mut |n| {
        if hit.is_none() && n.nv().is_some_and(|nv| nv.id == id) {
            hit = Some(n);
        }
    });
    hit
}

fn has_text(p: &bo_model::slide::Paragraph) -> bool {
    p.runs.iter().any(|r| match r {
        TextRun::Run { text, .. } | TextRun::Field { text, .. } => !text.trim().is_empty(),
        TextRun::Break { .. } | TextRun::Other(_) => false,
    })
}

/// The chart of a chart frame on a slide.
fn frame_chart(pres: &Presentation, slide: usize, n: &ShapeNode) -> Option<bo_model::sheet::Chart> {
    let part = super::chart::chart_part_of(pres, slide, n)?;
    bo_xlsx::chart_write::parse_chart_part(pres.part_data(&part)?, &part)
}

/// What shape `id` can be built from (`None`: only as one object).
#[must_use]
pub fn kind_of(pres: &Presentation, slide: usize, id: u32) -> Option<Kind> {
    let n = node_by_id(pres, slide, id)?;
    match n {
        ShapeNode::Shape(s) => {
            let t = s.text.as_ref()?;
            let paras: Vec<(u8, bool)> = t
                .paragraphs
                .iter()
                .map(|p| {
                    (
                        p.props.as_ref().and_then(|x| x.lvl).unwrap_or(0),
                        has_text(p),
                    )
                })
                .collect();
            paras.iter().any(|p| p.1).then_some(Kind::Text(paras))
        }
        ShapeNode::Frame(f) if f.uri == bo_pptx::diagram::URI_DIAGRAM => {
            let GraphicContent::Other(gd) = &f.content else {
                return None;
            };
            let s = pres.slides.get(slide)?;
            let get = |p: &str| pres.part_data(p).map(<[u8]>::to_vec);
            let (data, drawing) =
                bo_pptx::diagram::frame_parts(gd, &s.info.rels, &s.info.part_name, &get)?;
            let b = bo_pptx::diagram::diagram_build(
                pres.part_data(&data)?,
                &data,
                pres.part_data(&drawing)?,
                &drawing,
            )?;
            (!b.nodes.is_empty()).then_some(Kind::Diagram(b))
        }
        ShapeNode::Frame(_) => {
            let c = frame_chart(pres, slide, n)?;
            let series: usize = c.plots.iter().map(|p| p.series.len()).sum();
            let cats = c
                .plots
                .iter()
                .flat_map(|p| &p.series)
                .map(|s| s.values.len())
                .max()
                .unwrap_or(0);
            (series > 0 && cats > 0).then(|| Kind::Chart {
                series,
                cats,
                maskable: bo_sheet::chart::maskable(&c),
            })
        }
        _ => None,
    }
}

/// The build of an effect as the UI shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildOut {
    /// `text` | `chart` | `diagram`.
    pub kind: &'static str,
    /// Text: `whole` | `allAtOnce` | `p1` … `p5` (by 1st … 5th level paragraphs); chart: `whole` |
    /// `allAtOnce` | `series` | `category` | `seriesEl` | `categoryEl`; SmartArt: `whole` |
    /// `allAtOnce` | `one` | `lvlAtOnce` | `lvlOne`.
    pub value: String,
    /// In reverse order (bldP@rev, bldDgm@rev).
    pub reverse: bool,
    /// The shape (chart background) is animated too (bldP@animBg, bldChart@animBg).
    pub bg: bool,
    /// The first effect of its build (the pane shows the build's options there).
    pub head: bool,
    /// Deepest outline level of the text (1-based), for the "by n-th level" choices.
    pub levels: u32,
}

/// The effects of the build effect `index` belongs to (itself when it has no group).
#[must_use]
pub fn members(anims: &[Animation], index: usize) -> Vec<usize> {
    let Some(a) = anims.get(index) else {
        return Vec::new();
    };
    match a.opts.group_id {
        Some(g) => anims
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.shape_id == a.shape_id
                    && b.opts.group_id == Some(g)
                    && b.trigger_shape == a.trigger_shape
                    && b.class == a.class
            })
            .map(|(i, _)| i)
            .collect(),
        None => vec![index],
    }
}

fn build_of(t: &Timing, shape: u32, group: Option<u32>) -> Option<&Build> {
    let g = group?;
    t.builds.iter().find(|b| b.key() == Some((shape, g)))
}

/// The build of effect `index` for the UI (`None`: the shape builds as one object only).
#[must_use]
pub fn build_out(pres: &Presentation, slide: usize, t: &Timing, index: usize) -> Option<BuildOut> {
    let a = t.animations.get(index)?;
    if a.trigger_shape.is_some() && a.class == bo_model::slide::AnimClass::Other("mediacall".into())
    {
        return None;
    }
    let kind = kind_of(pres, slide, a.shape_id)?;
    build_with(&kind, t, index)
}

/// The current build value of effect `index` (see [`BuildOut::value`]).
#[must_use]
pub fn build_out_of(t: &Timing, kind: Option<&Kind>, index: usize) -> Option<String> {
    build_with(kind?, t, index).map(|b| b.value)
}

fn build_with(kind: &Kind, t: &Timing, index: usize) -> Option<BuildOut> {
    let a = t.animations.get(index)?;
    let head = members(&t.animations, index).first() == Some(&index);
    let b = build_of(t, a.shape_id, a.opts.group_id);
    let split = a.opts.target.is_some();
    let (kind, value, reverse, bg, levels) = match kind {
        Kind::Text(paras) => {
            let levels = paras
                .iter()
                .filter(|p| p.1)
                .map(|p| u32::from(p.0) + 1)
                .max()
                .unwrap_or(1)
                .min(5);
            match b {
                Some(Build::Paragraph(p)) if split || p.build != "whole" => {
                    let v = if p.build == "allAtOnce" {
                        "allAtOnce".to_owned()
                    } else if p.build == "whole" {
                        "whole".to_owned()
                    } else {
                        format!("p{}", p.level.clamp(1, 5))
                    };
                    ("text", v, p.reverse, p.anim_bg, levels)
                }
                _ if split => ("text", "p1".to_owned(), false, false, levels),
                _ => ("text", "whole".to_owned(), false, true, levels),
            }
        }
        Kind::Chart { .. } => match b {
            Some(Build::Graphic(GraphicBuild {
                sub: Some(GraphicSub::Chart { build, anim_bg }),
                ..
            })) => ("chart", build.clone(), false, *anim_bg, 1),
            _ if split => ("chart", "series".to_owned(), false, false, 1),
            _ => ("chart", "whole".to_owned(), false, true, 1),
        },
        Kind::Diagram(_) => match b {
            Some(Build::Graphic(GraphicBuild {
                sub: Some(GraphicSub::Diagram { build, reverse }),
                ..
            })) => ("diagram", build.clone(), *reverse, false, 1),
            _ if split => ("diagram", "one".to_owned(), false, false, 1),
            _ => ("diagram", "whole".to_owned(), false, false, 1),
        },
    };
    Some(BuildOut {
        kind,
        value,
        reverse,
        bg,
        head,
        levels,
    })
}

/// The next trigger of a build's steps: on click when the build started on click, else after the
/// step before.
fn step_trigger(first: AnimTrigger) -> AnimTrigger {
    match first {
        AnimTrigger::OnClick => AnimTrigger::OnClick,
        _ => AnimTrigger::AfterPrevious,
    }
}

/// A copy of `rep` on `target` started by `trigger` (only the first step keeps the delay and the
/// sound).
fn step(
    rep: &Animation,
    target: Option<AnimTarget>,
    trigger: AnimTrigger,
    first: bool,
) -> Animation {
    let mut a = rep.clone();
    a.opts.target = target;
    a.trigger = trigger;
    if !first {
        a.delay_ms = 0;
        a.opts.sound = None;
    }
    a
}

/// The effects and build of `rep` built as `value` (see [`BuildOut::value`]).
///
/// # Errors
/// `badBuild` for a value the shape cannot be built as.
pub fn expand(
    rep: &Animation,
    kind: &Kind,
    group: u32,
    value: &str,
    reverse: bool,
    bg: bool,
) -> R<(Vec<Animation>, Build)> {
    let mut rep = rep.clone();
    rep.opts.group_id = Some(group);
    let first = rep.trigger;
    let next = step_trigger(first);
    let spid = rep.shape_id;
    let whole = |sub: Option<GraphicSub>| -> (Vec<Animation>, Build) {
        let a = step(&rep, None, first, true);
        let b = match kind {
            Kind::Text(_) => Build::Paragraph(ParaBuild {
                shape_id: spid,
                group_id: group,
                build: "whole".into(),
                level: 1,
                reverse: false,
                anim_bg: true,
                attrs: Vec::new(),
            }),
            _ => Build::Graphic(GraphicBuild {
                shape_id: spid,
                group_id: group,
                sub,
            }),
        };
        (vec![a], b)
    };
    if value == "whole" {
        return Ok(whole(None));
    }
    match kind {
        Kind::Text(paras) => {
            let level = match value {
                "allAtOnce" => None,
                v => Some(
                    v.strip_prefix('p')
                        .and_then(|n| n.parse::<u8>().ok())
                        .filter(|n| (1..=5).contains(n))
                        .ok_or(super::EditError::Refused("badBuild"))?,
                ),
            };
            // Groups of paragraphs: a paragraph up to the level starts one, deeper ones (and
            // paragraphs without text) join the one before.
            let mut starts: Vec<u32> = Vec::new();
            for (i, (lvl, text)) in paras.iter().enumerate() {
                let opens = *text && level.is_none_or(|n| *lvl < n);
                if opens || starts.is_empty() && *text {
                    starts.push(u32::try_from(i).unwrap_or(u32::MAX));
                }
            }
            let last = u32::try_from(paras.len().saturating_sub(1)).unwrap_or(0);
            let mut ranges: Vec<(u32, u32)> = starts
                .iter()
                .enumerate()
                .map(|(k, s)| (*s, starts.get(k + 1).map_or(last, |n| n - 1)))
                .collect();
            if reverse {
                ranges.reverse();
            }
            let mut out = Vec::new();
            if bg {
                out.push(step(&rep, Some(AnimTarget::Background), first, true));
            }
            for (k, (start, end)) in ranges.into_iter().enumerate() {
                let trig = match (out.is_empty(), level) {
                    (true, _) => first,
                    // The first paragraph comes with the shape.
                    (false, _) if k == 0 => AnimTrigger::WithPrevious,
                    (false, None) => AnimTrigger::WithPrevious,
                    (false, Some(_)) => next,
                };
                let is_first = out.is_empty();
                out.push(step(
                    &rep,
                    Some(AnimTarget::Paragraphs { start, end }),
                    trig,
                    is_first,
                ));
            }
            let b = Build::Paragraph(ParaBuild {
                shape_id: spid,
                group_id: group,
                build: if level.is_some() { "p" } else { "allAtOnce" }.into(),
                level: level.map_or(1, u32::from),
                reverse,
                anim_bg: bg,
                attrs: Vec::new(),
            });
            Ok((out, b))
        }
        Kind::Chart { series, cats, .. } => {
            let (s, c) = (
                i32::try_from(*series).unwrap_or(i32::MAX),
                i32::try_from(*cats).unwrap_or(i32::MAX),
            );
            let t = |series: i32, category: i32, step: &str| AnimTarget::Chart {
                series,
                category,
                step: step.into(),
            };
            let targets: Vec<AnimTarget> = match value {
                "allAtOnce" => {
                    return Ok(whole(Some(GraphicSub::Chart {
                        build: "allAtOnce".into(),
                        anim_bg: bg,
                    })))
                }
                "series" => (0..s).map(|i| t(i, -1, "series")).collect(),
                "category" => (0..c).map(|i| t(-1, i, "category")).collect(),
                "seriesEl" => (0..s)
                    .flat_map(|i| (0..c).map(move |j| (i, j)))
                    .map(|(i, j)| t(i, j, "ptInSeries"))
                    .collect(),
                "categoryEl" => (0..c)
                    .flat_map(|j| (0..s).map(move |i| (i, j)))
                    .map(|(i, j)| t(i, j, "ptInCategory"))
                    .collect(),
                _ => return refuse("badBuild"),
            };
            let mut out = Vec::new();
            if bg {
                out.push(step(&rep, Some(t(-1, -1, "gridLegend")), first, true));
            }
            for x in targets {
                let trig = if out.is_empty() { first } else { next };
                let is_first = out.is_empty();
                out.push(step(&rep, Some(x), trig, is_first));
            }
            let b = Build::Graphic(GraphicBuild {
                shape_id: spid,
                group_id: group,
                sub: Some(GraphicSub::Chart {
                    build: value.into(),
                    anim_bg: bg,
                }),
            });
            Ok((out, b))
        }
        Kind::Diagram(d) => {
            // Steps of nodes (indices into `d.nodes`).
            let mut steps: Vec<Vec<usize>> = match value {
                "allAtOnce" => {
                    return Ok(whole(Some(GraphicSub::Diagram {
                        build: "allAtOnce".into(),
                        reverse,
                    })))
                }
                "one" => (0..d.nodes.len()).map(|i| vec![i]).collect(),
                "lvlOne" | "lvlAtOnce" => {
                    let mut levels: Vec<u32> = d.nodes.iter().map(|n| n.1).collect();
                    levels.sort_unstable();
                    levels.dedup();
                    let mut v = Vec::new();
                    for l in levels {
                        let at: Vec<usize> =
                            (0..d.nodes.len()).filter(|i| d.nodes[*i].1 == l).collect();
                        if value == "lvlAtOnce" {
                            v.push(at);
                        } else {
                            v.extend(at.into_iter().map(|i| vec![i]));
                        }
                    }
                    v
                }
                _ => return refuse("badBuild"),
            };
            if reverse {
                steps.reverse();
            }
            let mut out = Vec::new();
            for st in steps {
                for (k, i) in st.into_iter().enumerate() {
                    let trig = match (out.is_empty(), k) {
                        (true, _) => first,
                        (false, 0) => next,
                        _ => AnimTrigger::WithPrevious,
                    };
                    let is_first = out.is_empty();
                    out.push(step(
                        &rep,
                        Some(AnimTarget::Diagram {
                            id: d.nodes[i].0.clone(),
                            step: "sp".into(),
                        }),
                        trig,
                        is_first,
                    ));
                }
            }
            let b = Build::Graphic(GraphicBuild {
                shape_id: spid,
                group_id: group,
                sub: Some(GraphicSub::Diagram {
                    build: value.into(),
                    reverse,
                }),
            });
            Ok((out, b))
        }
    }
}

/// Rebuild the effects of the build effect `index` belongs to as `value`: its effects are replaced
/// (where the first of them was) and the build list updated.
///
/// # Errors
/// `noAnimation`, `noBuild` (the shape builds as one object only), `badBuild`.
pub fn rebuild(
    t: &mut Timing,
    kind: Option<&Kind>,
    index: usize,
    value: &str,
    reverse: Option<bool>,
    bg: Option<bool>,
) -> R<()> {
    let Some(kind) = kind else {
        return refuse("noBuild");
    };
    let group = members(&t.animations, index);
    let Some(&at) = group.first() else {
        return refuse("noAnimation");
    };
    // The effect itself: the first step on the shape's content (not the background step).
    let rep = group
        .iter()
        .map(|&i| &t.animations[i])
        .find(|a| {
            !matches!(
                &a.opts.target,
                Some(AnimTarget::Background)
            ) && !matches!(&a.opts.target, Some(AnimTarget::Chart { step, .. }) if step == "gridLegend")
        })
        .unwrap_or(&t.animations[at])
        .clone();
    let mut rep = rep;
    // The build starts as its first effect did.
    rep.trigger = t.animations[at].trigger;
    rep.delay_ms = t.animations[at].delay_ms;
    if rep.opts.sound.is_none() {
        rep.opts.sound.clone_from(&t.animations[at].opts.sound);
    }
    let old = build_of(t, rep.shape_id, rep.opts.group_id).cloned();
    let (old_rev, old_bg) = match &old {
        Some(Build::Paragraph(p)) => (p.reverse, p.anim_bg),
        Some(Build::Graphic(GraphicBuild {
            sub: Some(GraphicSub::Chart { anim_bg, .. }),
            ..
        })) => (false, *anim_bg),
        Some(Build::Graphic(GraphicBuild {
            sub: Some(GraphicSub::Diagram { reverse, .. }),
            ..
        })) => (*reverse, false),
        _ => (false, matches!(kind, Kind::Chart { .. })),
    };
    let reverse = reverse.unwrap_or(old_rev);
    let bg = bg.unwrap_or(old_bg);
    // A group id of its own on the shape: the one it has, else the first one free.
    let gid = rep.opts.group_id.unwrap_or_else(|| {
        (0..=u32::MAX)
            .find(|g| {
                !t.animations
                    .iter()
                    .any(|a| a.shape_id == rep.shape_id && a.opts.group_id == Some(*g))
                    && !t.builds.iter().any(|b| b.key() == Some((rep.shape_id, *g)))
            })
            .unwrap_or(0)
    });
    let (new, build) = expand(&rep, kind, gid, value, reverse, bg)?;
    // Options that cannot be shared: hiding on the next click hides each step at the next step.
    let mut keep = Vec::with_capacity(t.animations.len() + new.len());
    for (i, a) in t.animations.drain(..).enumerate() {
        if i == at {
            keep.extend(new.iter().cloned());
        }
        if !group.contains(&i) {
            keep.push(a);
        }
    }
    t.animations = keep;
    t.builds.retain(|b| b.key() != Some((rep.shape_id, gid)));
    t.builds.push(build);
    Ok(())
}

/// The dim colour attributes of an effect on shape `id`: its text (`style.color`) when it has
/// some or the effect targets paragraphs, else its fill (`fillcolor`).
#[must_use]
pub fn dim_attrs(pres: &Presentation, slide: usize, a: &Animation) -> Vec<String> {
    let text = matches!(a.opts.target, Some(AnimTarget::Paragraphs { .. }))
        || matches!(kind_of(pres, slide, a.shape_id), Some(Kind::Text(_)));
    vec![if text { "style.color" } else { "fillcolor" }.to_owned()]
}

/// After-effect choice of the UI: `none` | `dim` | `hide` | `hideOnClick`.
#[must_use]
pub fn after_name(a: Option<&AnimAfter>) -> Option<&'static str> {
    match a? {
        AnimAfter::Dim { .. } => Some("dim"),
        AnimAfter::Hide => Some("hide"),
        AnimAfter::HideOnClick => Some("hideOnClick"),
    }
}

// ---------------------------------------------------------------------------
// The slide show: pieces of a layer
// ---------------------------------------------------------------------------

/// One drawn piece of a shape built element by element: a paragraph (`(i, 0)`), a chart point
/// (`(series, point)`) or a SmartArt drawing shape (`(i, 0)`).
pub type Atom = (usize, usize);

/// The atoms target `t` covers in a shape of kind `k`; `None` = the shape's background (the body).
fn atoms_of(k: &Kind, t: &AnimTarget) -> Option<Vec<Atom>> {
    match (k, t) {
        (_, AnimTarget::Background) => None,
        (Kind::Text(paras), AnimTarget::Paragraphs { start, end }) => Some(
            (*start as usize..=*end as usize)
                .filter(|i| *i < paras.len())
                .map(|i| (i, 0))
                .collect(),
        ),
        (
            Kind::Chart { series, cats, .. },
            AnimTarget::Chart {
                series: s,
                category: c,
                step,
            },
        ) => {
            if step == "gridLegend" {
                return None;
            }
            let s = usize::try_from(*s).ok();
            let c = usize::try_from(*c).ok();
            Some(
                (0..*series)
                    .flat_map(|i| (0..*cats).map(move |j| (i, j)))
                    .filter(|(i, j)| match step.as_str() {
                        "series" => Some(*i) == s,
                        "category" => Some(*j) == c,
                        "ptInSeries" | "ptInCategory" => Some(*i) == s && Some(*j) == c,
                        _ => true,
                    })
                    .collect(),
            )
        }
        (Kind::Diagram(d), AnimTarget::Diagram { id, step }) => {
            if step == "bg" {
                return None;
            }
            let node = d.nodes.iter().position(|(n, _)| n == id)?;
            Some(
                d.shapes
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| **s == Some(node))
                    .map(|(i, _)| (i, 0))
                    .collect(),
            )
        }
        _ => Some(Vec::new()),
    }
}

/// Every atom of a shape of kind `k`.
fn all_atoms(k: &Kind) -> Vec<Atom> {
    match k {
        Kind::Text(p) => (0..p.len()).map(|i| (i, 0)).collect(),
        Kind::Chart { series, cats, .. } => (0..*series)
            .flat_map(|i| (0..*cats).map(move |j| (i, j)))
            .collect(),
        Kind::Diagram(d) => (0..d.shapes.len()).map(|i| (i, 0)).collect(),
    }
}

/// How the show splits the layer of one shape: the atoms of the body (index 0) and of each
/// piece, and the pieces each effect plays on.
#[derive(Debug, Clone, PartialEq)]
pub struct PartsPlan {
    /// The shape.
    pub shape_id: u32,
    /// What it is built from.
    pub kind: Kind,
    /// Atoms of the body (drawn with every piece), then of each piece.
    pub atoms: Vec<Vec<Atom>>,
    /// Effect index → pieces it plays on (0 = the body).
    pub effects: Vec<(usize, Vec<usize>)>,
}

/// The pieces of shape `id`'s layer for its effects that target parts of it (`None`: none do, or
/// the shape cannot be drawn piece by piece).
#[must_use]
pub fn plan(pres: &Presentation, slide: usize, t: &Timing, id: u32) -> Option<PartsPlan> {
    let targeted: Vec<(usize, &AnimTarget)> = t
        .animations
        .iter()
        .enumerate()
        .filter(|(_, a)| a.shape_id == id)
        .filter_map(|(i, a)| a.opts.target.as_ref().map(|x| (i, x)))
        .collect();
    if targeted.is_empty() {
        return None;
    }
    let kind = kind_of(pres, slide, id)?;
    if let Kind::Chart {
        maskable: false, ..
    } = kind
    {
        return None;
    }
    // Distinct targets and the atoms each covers.
    let mut targets: Vec<&AnimTarget> = Vec::new();
    for (_, x) in &targeted {
        if !targets.contains(x) {
            targets.push(x);
        }
    }
    let covers: Vec<Option<Vec<Atom>>> = targets.iter().map(|x| atoms_of(&kind, x)).collect();
    // Atoms grouped by the targets covering them (their signature); none: the body.
    let mut sigs: Vec<Vec<usize>> = Vec::new();
    let mut atoms: Vec<Vec<Atom>> = vec![Vec::new()];
    for at in all_atoms(&kind) {
        let sig: Vec<usize> = covers
            .iter()
            .enumerate()
            .filter(|(_, c)| c.as_ref().is_some_and(|c| c.contains(&at)))
            .map(|(k, _)| k)
            .collect();
        if sig.is_empty() {
            atoms[0].push(at);
            continue;
        }
        if let Some(p) = sigs.iter().position(|s| *s == sig) {
            atoms[p + 1].push(at);
        } else {
            sigs.push(sig);
            atoms.push(vec![at]);
        }
    }
    let effects = targeted
        .iter()
        .map(|(i, x)| {
            let k = targets.iter().position(|y| y == x).unwrap_or(0);
            let parts: Vec<usize> = match &covers[k] {
                None => vec![0],
                Some(_) => sigs
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.contains(&k))
                    .map(|(p, _)| p + 1)
                    .collect(),
            };
            (*i, parts)
        })
        .collect();
    Some(PartsPlan {
        shape_id: id,
        kind,
        atoms,
        effects,
    })
}

/// A fully transparent colour (DrawingML alpha 0).
fn clear_color() -> Color {
    Color {
        kind: bo_model::slide::ColorKind::Srgb(0),
        mods: vec![ColorMod {
            name: "alpha".into(),
            val: Some(0),
        }],
    }
}

fn clear() -> Fill {
    Fill::Solid(clear_color())
}

/// Text of the paragraphs not `shown` drawn transparent (the text keeps its layout).
fn clear_paragraphs(t: &mut TextBody, shown: &[usize]) {
    for (i, p) in t.paragraphs.iter_mut().enumerate() {
        if shown.contains(&i) {
            continue;
        }
        for r in &mut p.runs {
            match r {
                TextRun::Run { props, .. }
                | TextRun::Field { props, .. }
                | TextRun::Break { props } => props.fill = Some(clear()),
                TextRun::Other(_) => {}
            }
        }
        if let Some(e) = &mut p.end_props {
            e.fill = Some(clear());
        }
        p.props.get_or_insert_with(Default::default).bu_clr = Some(Some(clear_color()));
    }
}

/// Shape `id` inside top-level node `n` with only paragraphs `shown` of its text drawn.
pub fn mask_text(n: &mut ShapeNode, id: u32, shown: &[usize]) -> bool {
    if n.nv().is_some_and(|nv| nv.id == id) {
        if let ShapeNode::Shape(s) = n {
            if let Some(t) = &mut s.text {
                clear_paragraphs(t, shown);
                return true;
            }
        }
        return false;
    }
    if let ShapeNode::Group(g) = n {
        return g.children.iter_mut().any(|c| mask_text(c, id, shown));
    }
    false
}

/// Drop what draws nothing (transparent text and fills) from a display list.
pub fn strip_clear(page: &mut PageList) {
    page.commands.retain(|c| match c {
        Command::Glyphs { color, .. } | Command::FillRect { color, .. } => color.3 > 0,
        Command::Path { fill, stroke, .. } => {
            let f = match fill {
                Some(bo_render::Paint::Solid { color }) => color.3 > 0,
                Some(_) => true,
                None => false,
            };
            f || stroke.as_ref().is_some_and(|s| s.color.3 > 0)
        }
        _ => true,
    });
}

fn structural(c: &Command) -> bool {
    matches!(
        c,
        Command::PushTransform { .. }
            | Command::PopTransform
            | Command::PushClip { .. }
            | Command::PushClipPath { .. }
            | Command::PopClip
            | Command::PushOpacity { .. }
            | Command::PopOpacity
    )
}

/// What `full` draws beyond `base` (`base` drawn with some pieces left out): `full`'s transforms,
/// clips and opacities, and its drawing commands `base` does not have. `None` when `base` is not
/// `full` with commands left out.
#[must_use]
pub fn subtract(full: &PageList, base: &PageList) -> Option<PageList> {
    let mut j = 0;
    let mut out = Vec::new();
    for c in &full.commands {
        if base.commands.get(j) == Some(c) {
            j += 1;
            if structural(c) {
                out.push(c.clone());
            }
        } else {
            out.push(c.clone());
        }
    }
    (j == base.commands.len()).then_some(PageList {
        width: full.width,
        height: full.height,
        commands: out,
    })
}

type Mat = [f64; 6];

fn mul(m: Mat, n: Mat) -> Mat {
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}

/// Bounds `[x, y, w, h]` (points) of what a display list draws (`None`: nothing).
#[must_use]
pub fn bounds(page: &PageList) -> Option<[f64; 4]> {
    let mut m: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut stack: Vec<Mat> = Vec::new();
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut add = |m: Mat, x: f64, y: f64| {
        let (u, v) = (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        b = [b[0].min(u), b[1].min(v), b[2].max(u), b[3].max(v)];
    };
    let segs = |m: Mat, s: &[PathSeg], add: &mut dyn FnMut(Mat, f64, f64)| {
        for seg in s {
            match *seg {
                PathSeg::M(x, y) | PathSeg::L(x, y) => add(m, f64::from(x), f64::from(y)),
                PathSeg::Q(a, b, x, y) => {
                    add(m, f64::from(a), f64::from(b));
                    add(m, f64::from(x), f64::from(y));
                }
                PathSeg::C(a, b, c, d, x, y) => {
                    add(m, f64::from(a), f64::from(b));
                    add(m, f64::from(c), f64::from(d));
                    add(m, f64::from(x), f64::from(y));
                }
                PathSeg::Z => {}
            }
        }
    };
    for c in &page.commands {
        match c {
            Command::PushTransform { m: t } => {
                stack.push(m);
                m = mul(m, t.map(f64::from));
            }
            Command::PopTransform => m = stack.pop().unwrap_or(m),
            Command::FillRect { x, y, w, h, .. } | Command::Image { x, y, w, h, .. } => {
                let (x, y, w, h) = (f64::from(*x), f64::from(*y), f64::from(*w), f64::from(*h));
                for (px, py) in [(x, y), (x + w, y), (x, y + h), (x + w, y + h)] {
                    add(m, px, py);
                }
            }
            Command::Line { x1, y1, x2, y2, .. } => {
                add(m, f64::from(*x1), f64::from(*y1));
                add(m, f64::from(*x2), f64::from(*y2));
            }
            Command::Path { segs: s, .. } | Command::BlurPath { segs: s, .. } => {
                segs(m, s, &mut add);
            }
            Command::Glyphs {
                size, x, y, glyphs, ..
            } => {
                let (x, y, size) = (f64::from(*x), f64::from(*y), f64::from(*size));
                for g in glyphs {
                    let (gx, gy) = (x + f64::from(g.1), y + f64::from(g.2));
                    add(m, gx, gy - size * 0.9);
                    add(m, gx + size * 0.6, gy + size * 0.25);
                }
            }
            _ => {}
        }
    }
    (b[0] <= b[2]).then(|| [b[0], b[1], b[2] - b[0], b[3] - b[1]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::slide::{AnimClass, AnimEffect, AnimOptions};
    use bo_render::Rgba;

    fn fade(trigger: AnimTrigger) -> Animation {
        Animation {
            shape_id: 4,
            class: AnimClass::Entrance,
            effect: AnimEffect::Fade,
            direction: None,
            trigger,
            delay_ms: 250,
            dur_ms: 500,
            preset_id: None,
            preset_subtype: None,
            repeat: None,
            trigger_shape: None,
            trigger_bookmark: None,
            iterate: None,
            opts: AnimOptions::default(),
            source: None,
        }
    }

    fn paras(a: &[Animation]) -> Vec<(Option<AnimTarget>, AnimTrigger)> {
        a.iter()
            .map(|x| (x.opts.target.clone(), x.trigger))
            .collect()
    }

    #[allow(clippy::unnecessary_wraps)]
    fn p(start: u32, end: u32) -> Option<AnimTarget> {
        Some(AnimTarget::Paragraphs { start, end })
    }

    // source: §19.5 bldP build `p` with bldLvl (paragraphs up to the level step one by one, deeper
    // ones with their parent), `allAtOnce`, `rev` and `animBg` (the shape first, the first
    // paragraph with it); paragraphs without text join the group before.
    #[test]
    fn text_builds_by_level() {
        // Levels 0, 1, 1, (empty), 0, 2.
        let k = Kind::Text(vec![
            (0, true),
            (1, true),
            (1, true),
            (0, false),
            (0, true),
            (2, true),
        ]);
        let rep = fade(AnimTrigger::OnClick);
        let (a, b) = expand(&rep, &k, 0, "p1", false, false).unwrap();
        assert_eq!(
            paras(&a),
            vec![
                (p(0, 3), AnimTrigger::OnClick),
                (p(4, 5), AnimTrigger::OnClick)
            ]
        );
        assert_eq!(a[0].delay_ms, 250);
        assert_eq!(a[1].delay_ms, 0);
        let Build::Paragraph(pb) = b else { panic!() };
        assert_eq!((pb.build.as_str(), pb.level, pb.anim_bg), ("p", 1, false));
        let (a, _) = expand(&rep, &k, 0, "p2", true, true).unwrap();
        assert_eq!(
            paras(&a),
            vec![
                (Some(AnimTarget::Background), AnimTrigger::OnClick),
                (p(4, 5), AnimTrigger::WithPrevious),
                (p(2, 3), AnimTrigger::OnClick),
                (p(1, 1), AnimTrigger::OnClick),
                (p(0, 0), AnimTrigger::OnClick),
            ]
        );
        let (a, b) = expand(
            &fade(AnimTrigger::AfterPrevious),
            &k,
            3,
            "allAtOnce",
            false,
            false,
        )
        .unwrap();
        assert_eq!(a.len(), 5);
        assert!(a[1..]
            .iter()
            .all(|x| x.trigger == AnimTrigger::WithPrevious));
        assert!(a.iter().all(|x| x.opts.group_id == Some(3)));
        assert_eq!(b.key(), Some((4, 3)));
        let (a, b) = expand(&rep, &k, 0, "whole", false, false).unwrap();
        assert_eq!(paras(&a), vec![(None, AnimTrigger::OnClick)]);
        assert!(matches!(b, Build::Paragraph(ParaBuild { ref build, .. }) if build == "whole"));
        assert!(expand(&rep, &k, 0, "p9", false, false).is_err());
    }

    // source: DrawingML §20.1.2.2 bldChart (series, category, seriesEl, categoryEl; animBg: the
    // gridLegend step first) and bldDgm (one, lvlOne, lvlAtOnce, rev).
    #[test]
    fn chart_and_diagram_builds() {
        let k = Kind::Chart {
            series: 2,
            cats: 3,
            maskable: true,
        };
        let rep = fade(AnimTrigger::OnClick);
        let steps = |v: &str, bg: bool| -> Vec<(i32, i32, String)> {
            expand(&rep, &k, 0, v, false, bg)
                .unwrap()
                .0
                .iter()
                .map(|a| match &a.opts.target {
                    Some(AnimTarget::Chart {
                        series,
                        category,
                        step,
                    }) => (*series, *category, step.clone()),
                    _ => (9, 9, String::new()),
                })
                .collect()
        };
        assert_eq!(
            steps("series", true),
            vec![
                (-1, -1, "gridLegend".into()),
                (0, -1, "series".into()),
                (1, -1, "series".into())
            ]
        );
        assert_eq!(steps("category", false).len(), 3);
        assert_eq!(steps("seriesEl", false)[1], (0, 1, "ptInSeries".into()));
        assert_eq!(steps("categoryEl", false)[1], (1, 0, "ptInCategory".into()));
        let d = Kind::Diagram(bo_pptx::diagram::DiagramBuild {
            nodes: vec![("A".into(), 1), ("A1".into(), 2), ("B".into(), 1)],
            shapes: vec![Some(0), Some(1), Some(2)],
        });
        let ids = |v: &str, rev: bool| -> Vec<(String, AnimTrigger)> {
            expand(&rep, &d, 0, v, rev, false)
                .unwrap()
                .0
                .iter()
                .map(|a| match &a.opts.target {
                    Some(AnimTarget::Diagram { id, .. }) => (id.clone(), a.trigger),
                    _ => (String::new(), a.trigger),
                })
                .collect()
        };
        let c = AnimTrigger::OnClick;
        let w = AnimTrigger::WithPrevious;
        assert_eq!(
            ids("one", false),
            vec![("A".into(), c), ("A1".into(), c), ("B".into(), c)]
        );
        assert_eq!(
            ids("lvlAtOnce", false),
            vec![("A".into(), c), ("B".into(), w), ("A1".into(), c)]
        );
        assert_eq!(
            ids("lvlOne", true),
            vec![("A1".into(), c), ("B".into(), c), ("A".into(), c)]
        );
        assert!(expand(&rep, &d, 0, "series", false, false).is_err());
    }

    // source: a layer split into its body and pieces — atoms grouped by the targets covering them;
    // a background step plays on the body.
    #[test]
    fn plans_group_atoms_by_target() {
        let k = Kind::Chart {
            series: 2,
            cats: 2,
            maskable: true,
        };
        let series = |s| AnimTarget::Chart {
            series: s,
            category: -1,
            step: "series".into(),
        };
        let bg = AnimTarget::Chart {
            series: -1,
            category: -1,
            step: "gridLegend".into(),
        };
        assert_eq!(atoms_of(&k, &series(1)), Some(vec![(1, 0), (1, 1)]));
        assert_eq!(atoms_of(&k, &bg), None);
        let text = Kind::Text(vec![(0, true); 3]);
        assert_eq!(
            atoms_of(&text, &AnimTarget::Paragraphs { start: 1, end: 5 }),
            Some(vec![(1, 0), (2, 0)])
        );
    }

    // source: what a drawing adds to one with pieces left out, keeping the transforms around it.
    #[test]
    fn pieces_are_what_their_drawing_adds() {
        let r = |x: f32, a: u8| Command::FillRect {
            x,
            y: 0.0,
            w: 10.0,
            h: 5.0,
            color: Rgba(1, 2, 3, a),
        };
        let page = |c: Vec<Command>| PageList {
            width: 100.0,
            height: 100.0,
            commands: c,
        };
        let t = Command::PushTransform {
            m: [1.0, 0.0, 0.0, 1.0, 20.0, 0.0],
        };
        let full = page(vec![
            r(0.0, 255),
            t.clone(),
            r(5.0, 255),
            Command::PopTransform,
        ]);
        let mut base = page(vec![
            r(0.0, 255),
            t.clone(),
            r(5.0, 0),
            Command::PopTransform,
        ]);
        strip_clear(&mut base);
        let piece = subtract(&full, &base).unwrap();
        assert_eq!(piece.commands, vec![t, r(5.0, 255), Command::PopTransform]);
        let b = bounds(&piece).unwrap();
        assert!(
            (b[0] - 25.0).abs() < 1e-9 && (b[2] - 10.0).abs() < 1e-9,
            "{b:?}"
        );
        // Not a drawing with pieces left out.
        assert!(subtract(&base, &full).is_none());
    }
}
