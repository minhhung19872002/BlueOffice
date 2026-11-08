//! Slide transitions, the main animation sequence and the slide show
//! (prompt §6 Phase 4 "Transition & animation cơ bản").
//!
//! Commands edit `p:transition` (ECMA-376 Part 1, §19.3.1.50) and the main
//! sequence of `p:timing` (§19.3.1.48, §19.5) through the model of `bo_pptx::anim`;
//! unknown effects keep their XML (they are only re-timed). The slide show gets
//! the slide as layers — the backdrop plus one transparent layer per animated
//! top-level shape, in z-order — so hosts can play the effects by moving,
//! clipping and fading the layers.

use super::geom::{find, place_slide, BoxR, Rect};
use super::ops::Tx;
use super::{refuse, EditError, SlideSel, R};
use bo_model::slide::{
    AnimClass, AnimEffect, AnimTrigger, Animation, Presentation, ShapeNode, Timing, Transition,
    TransitionKind,
};
use serde::{Deserialize, Serialize};

/// Default effect duration in ms (new animations).
pub const DEFAULT_ANIM_MS: u32 = 500;

/// Duration of ST_TransitionSpeed values (§19.7.53) in ms: the schema names
/// the speeds only, these are BlueSlide's own timings.
pub fn speed_ms(speed: Option<&str>) -> u32 {
    match speed {
        Some("slow") => 1000,
        Some("med") => 750,
        _ => 500,
    }
}

/// Transition to set (`kind` = `none` removes it).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionSpec {
    /// `none` | `cut` | `fade` | `push` | `wipe` | `cover` | `pull` | `split` | `random`
    /// (`keep` keeps the current effect and changes the timing only).
    pub kind: String,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub dur_ms: Option<u32>,
    #[serde(default)]
    pub advance_on_click: Option<bool>,
    /// Advance automatically after this many ms (0 = not automatically).
    #[serde(default)]
    pub advance_after_ms: Option<u32>,
}

/// A slide's transition as seen by the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionOut {
    /// Modelled kind (`fade`, `push`, …) or the element name of an unknown one.
    pub kind: String,
    /// The host can play it as is (others play as a fade).
    pub known: bool,
    pub dir: Option<String>,
    /// Effective duration (p14:dur, else the speed).
    pub dur_ms: u32,
    pub advance_on_click: bool,
    pub advance_after_ms: Option<u32>,
}

/// One effect of the main sequence as seen by the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimOut {
    pub index: usize,
    pub shape_id: u32,
    pub shape_name: String,
    /// `appear` | `fade` | `flyIn` | `wipe` | `other`.
    pub effect: &'static str,
    /// `entr` | `exit` | `emph` | `path` | `other`.
    pub class: &'static str,
    pub direction: Option<String>,
    /// `onClick` | `withPrevious` | `afterPrevious`.
    pub trigger: &'static str,
    pub delay_ms: u32,
    pub dur_ms: u32,
    /// Click step (0 = with the slide) and times relative to it (`Timing::timeline`).
    pub click: usize,
    pub start_ms: u32,
    pub end_ms: u32,
    /// Bounds of the animated top-level shape `[x, y, w, h]` in points.
    pub rect: Option<[f64; 4]>,
    /// Show layer holding the shape (slide show only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<usize>,
}

/// One layer of the slide show (bottom to top).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayerOut {
    /// Top-level shapes drawn by the layer.
    pub ids: Vec<u32>,
    /// Whether the layer holds the background and master/layout shapes.
    pub backdrop: bool,
    pub page: bo_render::PageList,
}

/// A video or audio clip on a slide (§20.1.3.6 videoFile, §20.1.3.1 audioFile).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaOut {
    pub id: u32,
    pub name: String,
    /// `video` | `audio`.
    pub kind: &'static str,
    /// Box of the picture frame on the slide (axis-aligned bounds), points.
    pub rect: [f64; 4],
    /// Embedded media part (bytes via `Engine::media`).
    pub part: Option<String>,
    pub mime: Option<String>,
    /// External target of linked media (not played).
    pub link: Option<String>,
}

/// Everything the slide show needs to play a slide.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowOut {
    pub slide: usize,
    pub width: f32,
    pub height: f32,
    pub transition: Option<TransitionOut>,
    pub layers: Vec<LayerOut>,
    pub animations: Vec<AnimOut>,
    /// Clicks needed to play every effect.
    pub clicks: usize,
    pub media: Vec<MediaOut>,
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

fn kind_name(k: &TransitionKind) -> (String, bool) {
    let s = match k {
        TransitionKind::Cut => "cut",
        TransitionKind::Fade => "fade",
        TransitionKind::Push => "push",
        TransitionKind::Wipe => "wipe",
        TransitionKind::Cover => "cover",
        TransitionKind::Pull => "pull",
        TransitionKind::Split => "split",
        TransitionKind::Random => "random",
        TransitionKind::Other(n) => return (n.clone(), false),
    };
    (s.to_owned(), true)
}

/// The transition of a slide for the UI.
pub fn transition_out(pres: &Presentation, slide: usize) -> Option<TransitionOut> {
    let t = pres.slides.get(slide)?.transition.as_ref()?;
    let (kind, known) = t
        .kind
        .as_ref()
        .map_or_else(|| ("cut".to_owned(), true), kind_name);
    Some(TransitionOut {
        kind,
        known,
        dir: t.dir.clone(),
        dur_ms: t.dur_ms.unwrap_or_else(|| speed_ms(t.speed.as_deref())),
        advance_on_click: t.advance_on_click.unwrap_or(true),
        advance_after_ms: t.advance_after_ms,
    })
}

fn effect_name(e: &AnimEffect) -> &'static str {
    match e {
        AnimEffect::Appear => "appear",
        AnimEffect::Fade => "fade",
        AnimEffect::FlyIn => "flyIn",
        AnimEffect::Wipe => "wipe",
        AnimEffect::Other => "other",
    }
}

fn class_name(c: &AnimClass) -> &'static str {
    match c {
        AnimClass::Entrance => "entr",
        AnimClass::Exit => "exit",
        AnimClass::Emphasis => "emph",
        AnimClass::Path => "path",
        AnimClass::Other(_) => "other",
    }
}

fn trigger_name(t: AnimTrigger) -> &'static str {
    match t {
        AnimTrigger::OnClick => "onClick",
        AnimTrigger::WithPrevious => "withPrevious",
        AnimTrigger::AfterPrevious => "afterPrevious",
    }
}

/// Index of the top-level shape holding shape `id` (itself or a group around it).
fn top_index(pres: &Presentation, slide: usize, id: u32) -> Option<usize> {
    let s = pres.slides.get(slide)?;
    s.data.shapes.iter().position(|n| {
        let mut hit = false;
        bo_pptx::visit(std::slice::from_ref(n), &mut |c| {
            hit |= c.nv().is_some_and(|nv| nv.id == id);
        });
        hit
    })
}

fn rect_of(b: &BoxR) -> [f64; 4] {
    let r: Rect = b.bounds();
    [r.x, r.y, r.w, r.h]
}

/// The main sequence of a slide for the UI (animation pane, slide show).
pub fn animations_out(pres: &Presentation, slide: usize) -> Vec<AnimOut> {
    let Some(t) = pres.slides.get(slide).and_then(|s| s.timing.as_ref()) else {
        return Vec::new();
    };
    let placed = place_slide(pres, slide);
    let timeline = t.timeline();
    t.animations
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let entry = timeline.get(i);
            // Geometry of the top-level shape the layer moves.
            let top = top_index(pres, slide, a.shape_id)
                .and_then(|ti| pres.slides.get(slide)?.data.shapes.get(ti)?.nv())
                .map(|nv| nv.id);
            let name = find(&placed, a.shape_id)
                .and_then(|p| super::ops::node_at(pres, slide, &p.path))
                .and_then(ShapeNode::nv)
                .map(|nv| nv.name.clone())
                .unwrap_or_default();
            AnimOut {
                index: i,
                shape_id: a.shape_id,
                shape_name: name,
                effect: effect_name(&a.effect),
                class: class_name(&a.class),
                direction: a.direction.clone(),
                trigger: trigger_name(a.trigger),
                delay_ms: a.delay_ms,
                dur_ms: a.dur_ms,
                click: entry.map_or(0, |e| e.click),
                start_ms: entry.map_or(0, |e| e.start_ms),
                end_ms: entry.map_or(0, |e| e.end_ms),
                rect: top.and_then(|id| find(&placed, id)).map(|p| rect_of(&p.b)),
                layer: None,
            }
        })
        .collect()
}

/// Layers of a slide: ranges of top-level shapes, each animated shape alone.
pub fn layers(pres: &Presentation, slide: usize) -> Vec<bo_pptx::ShapeRange> {
    let Some(s) = pres.slides.get(slide) else {
        return Vec::new();
    };
    let n = s.data.shapes.len();
    let mut animated = vec![false; n];
    for a in s.timing.iter().flat_map(|t| &t.animations) {
        if let Some(i) = top_index(pres, slide, a.shape_id) {
            animated[i] = true;
        }
    }
    let range = |start, end, backdrop| bo_pptx::ShapeRange {
        start,
        end,
        backdrop,
    };
    let mut out = Vec::new();
    let mut start = 0;
    for (i, &anim) in animated.iter().enumerate() {
        if anim {
            if out.is_empty() || start < i {
                out.push(range(start, i, out.is_empty()));
            }
            out.push(range(i, i + 1, false));
            start = i + 1;
        }
    }
    if out.is_empty() || start < n {
        out.push(range(start, n, out.is_empty()));
    }
    out
}

/// Ids of the top-level shapes of a layer.
pub fn layer_ids(pres: &Presentation, slide: usize, r: bo_pptx::ShapeRange) -> Vec<u32> {
    pres.slides
        .get(slide)
        .map(|s| {
            s.data
                .shapes
                .iter()
                .enumerate()
                .filter(|(i, _)| (r.start..r.end).contains(i))
                .filter_map(|(_, n)| n.nv().map(|nv| nv.id))
                .collect()
        })
        .unwrap_or_default()
}

/// The layer holding animated shape `id`.
pub fn layer_of(
    pres: &Presentation,
    slide: usize,
    ls: &[bo_pptx::ShapeRange],
    id: u32,
) -> Option<usize> {
    let ti = top_index(pres, slide, id)?;
    ls.iter().position(|r| (r.start..r.end).contains(&ti))
}

// ---------------------------------------------------------------------------
// Editing
// ---------------------------------------------------------------------------

/// Side directions of push/wipe (§19.7.52 ST_TransitionSideDirectionType) and the
/// eight directions of cover/pull (§19.7.50 ST_TransitionEightDirectionType).
fn valid_dir(kind: &TransitionKind, dir: Option<&str>) -> Option<String> {
    let d = dir?;
    let ok = match kind {
        TransitionKind::Push | TransitionKind::Wipe => matches!(d, "l" | "u" | "r" | "d"),
        TransitionKind::Cover | TransitionKind::Pull => {
            matches!(d, "l" | "u" | "r" | "d" | "lu" | "ru" | "ld" | "rd")
        }
        // §19.3.1.47 split@orient (ST_Direction).
        TransitionKind::Split => matches!(d, "horz" | "vert"),
        _ => false,
    };
    ok.then(|| d.to_owned())
}

fn parse_kind(k: &str) -> R<Option<TransitionKind>> {
    Ok(Some(match k {
        "none" => return Ok(None),
        "cut" => TransitionKind::Cut,
        "fade" => TransitionKind::Fade,
        "push" => TransitionKind::Push,
        "wipe" => TransitionKind::Wipe,
        "cover" => TransitionKind::Cover,
        "pull" => TransitionKind::Pull,
        "split" => TransitionKind::Split,
        "random" => TransitionKind::Random,
        _ => return refuse("badTransition"),
    }))
}

/// The transition after applying `spec` to `old`.
fn with_spec(old: Option<&Transition>, spec: &TransitionSpec) -> R<Option<Transition>> {
    let mut t = old.cloned().unwrap_or_default();
    if spec.kind != "keep" {
        let Some(kind) = parse_kind(&spec.kind)? else {
            return Ok(None);
        };
        if t.kind.as_ref() != Some(&kind) {
            t.dir = None;
        }
        t.kind = Some(kind);
    }
    if let Some(k) = t.kind.clone() {
        if spec.dir.is_some() || !matches!(k, TransitionKind::Other(_)) {
            t.dir = valid_dir(&k, spec.dir.as_deref().or(t.dir.as_deref()));
        }
    }
    if let Some(d) = spec.dur_ms {
        t.dur_ms = Some(d.clamp(1, 60_000));
    }
    if let Some(c) = spec.advance_on_click {
        // advClick defaults to true (§19.3.1.50): only "false" is written.
        t.advance_on_click = (!c).then_some(false);
    }
    if let Some(a) = spec.advance_after_ms {
        t.advance_after_ms = (a > 0).then_some(a.min(3_600_000));
    }
    Ok(Some(t))
}

/// Set the transition of `slides` (default: the current slide; `all`: every slide).
pub fn cmd_set_transition(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    slides: Option<Vec<usize>>,
    all: bool,
    spec: &TransitionSpec,
) -> R<()> {
    let n = tx.pres.slides.len();
    let targets: Vec<usize> = if all {
        (0..n).collect()
    } else {
        slides.unwrap_or_else(|| vec![sel.slide])
    };
    if targets.iter().any(|&i| i >= n) {
        return refuse("noSlide");
    }
    for i in targets {
        let new = with_spec(tx.pres.slides[i].transition.as_ref(), spec)?;
        tx.fields(i, |f| f.transition = new)?;
    }
    Ok(())
}

fn parse_effect(e: &str) -> R<AnimEffect> {
    Ok(match e {
        "appear" => AnimEffect::Appear,
        "fade" => AnimEffect::Fade,
        "flyIn" => AnimEffect::FlyIn,
        "wipe" => AnimEffect::Wipe,
        _ => return refuse("badEffect"),
    })
}

fn parse_class(c: &str) -> R<AnimClass> {
    Ok(match c {
        "entr" => AnimClass::Entrance,
        "exit" => AnimClass::Exit,
        _ => return refuse("badEffect"),
    })
}

fn parse_trigger(t: &str) -> R<AnimTrigger> {
    Ok(match t {
        "onClick" => AnimTrigger::OnClick,
        "withPrevious" => AnimTrigger::WithPrevious,
        "afterPrevious" => AnimTrigger::AfterPrevious,
        _ => return refuse("badTrigger"),
    })
}

/// `l` | `r` | `t` | `b` for fly-in / wipe; nothing for the others.
fn valid_anim_dir(e: &AnimEffect, d: Option<&str>) -> Option<String> {
    match e {
        AnimEffect::FlyIn | AnimEffect::Wipe => Some(
            d.filter(|d| matches!(*d, "l" | "r" | "t" | "b"))
                .unwrap_or("b")
                .to_owned(),
        ),
        _ => None,
    }
}

/// Animation fields of the add/set commands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimSpec {
    #[serde(default)]
    pub effect: Option<String>,
    /// `entr` | `exit`.
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub trigger: Option<String>,
    #[serde(default)]
    pub dur_ms: Option<u32>,
    #[serde(default)]
    pub delay_ms: Option<u32>,
}

fn edit_timing(
    tx: &mut Tx<'_>,
    slide: usize,
    f: impl FnOnce(&mut Vec<Animation>) -> R<()>,
) -> R<()> {
    let s = tx
        .pres
        .slides
        .get(slide)
        .ok_or(EditError::Refused("noSlide"))?;
    let mut timing: Timing = s.timing.clone().unwrap_or_default();
    f(&mut timing.animations)?;
    tx.fields(slide, |fl| fl.timing = Some(timing))?;
    Ok(())
}

/// Add an effect for each selected shape (the first one starts on click unless
/// `trigger` says otherwise, the others with it).
pub fn cmd_add_animation(tx: &mut Tx<'_>, sel: &SlideSel, spec: &AnimSpec) -> R<()> {
    if sel.ids.is_empty() {
        return refuse("noSelection");
    }
    let effect = parse_effect(spec.effect.as_deref().unwrap_or("fade"))?;
    let class = parse_class(spec.class.as_deref().unwrap_or("entr"))?;
    let first = parse_trigger(spec.trigger.as_deref().unwrap_or("onClick"))?;
    let direction = valid_anim_dir(&effect, spec.direction.as_deref());
    let ids = sel.ids.clone();
    edit_timing(tx, sel.slide, |anims| {
        for (k, id) in ids.into_iter().enumerate() {
            anims.push(Animation {
                shape_id: id,
                class: class.clone(),
                effect: effect.clone(),
                direction: direction.clone(),
                trigger: if k == 0 {
                    first
                } else {
                    AnimTrigger::WithPrevious
                },
                delay_ms: spec.delay_ms.unwrap_or(0).min(600_000),
                dur_ms: spec.dur_ms.unwrap_or(DEFAULT_ANIM_MS).clamp(1, 600_000),
                preset_id: None,
                preset_subtype: None,
                source: None,
            });
        }
        Ok(())
    })
}

/// Change effect `index` of the current slide.
pub fn cmd_set_animation(tx: &mut Tx<'_>, sel: &SlideSel, index: usize, spec: &AnimSpec) -> R<()> {
    let effect = spec.effect.as_deref().map(parse_effect).transpose()?;
    let class = spec.class.as_deref().map(parse_class).transpose()?;
    let trigger = spec.trigger.as_deref().map(parse_trigger).transpose()?;
    edit_timing(tx, sel.slide, |anims| {
        let a = anims
            .get_mut(index)
            .ok_or(EditError::Refused("noAnimation"))?;
        let shape_changed = effect.is_some() || class.is_some() || spec.direction.is_some();
        if let Some(e) = effect {
            a.effect = e;
        }
        if let Some(c) = class {
            a.class = c;
        }
        if shape_changed {
            if a.effect == AnimEffect::Other {
                return refuse("badEffect");
            }
            a.direction = valid_anim_dir(
                &a.effect,
                spec.direction.as_deref().or(a.direction.as_deref()),
            );
            // A different effect: written from the model (§19.5) with its own preset.
            a.preset_id = None;
            a.preset_subtype = None;
            a.source = None;
        }
        if let Some(t) = trigger {
            a.trigger = t;
        }
        if let Some(d) = spec.dur_ms {
            a.dur_ms = d.clamp(1, 600_000);
        }
        if let Some(d) = spec.delay_ms {
            a.delay_ms = d.min(600_000);
        }
        Ok(())
    })
}

pub fn cmd_remove_animation(tx: &mut Tx<'_>, sel: &SlideSel, index: usize) -> R<()> {
    edit_timing(tx, sel.slide, |anims| {
        if index >= anims.len() {
            return refuse("noAnimation");
        }
        anims.remove(index);
        Ok(())
    })
}

pub fn cmd_move_animation(tx: &mut Tx<'_>, sel: &SlideSel, from: usize, to: usize) -> R<()> {
    edit_timing(tx, sel.slide, |anims| {
        if from >= anims.len() || to >= anims.len() {
            return refuse("noAnimation");
        }
        let a = anims.remove(from);
        anims.insert(to, a);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(kind: &str, dir: Option<&str>) -> TransitionSpec {
        TransitionSpec {
            kind: kind.into(),
            dir: dir.map(Into::into),
            dur_ms: Some(800),
            advance_on_click: Some(true),
            advance_after_ms: Some(0),
        }
    }

    #[test]
    fn transition_spec_keeps_valid_directions_only() {
        let t = with_spec(None, &spec("push", Some("u"))).unwrap().unwrap();
        assert_eq!(
            (t.kind.clone(), t.dir.as_deref(), t.dur_ms),
            (Some(TransitionKind::Push), Some("u"), Some(800))
        );
        let t = with_spec(Some(&t), &spec("fade", Some("u")))
            .unwrap()
            .unwrap();
        assert_eq!(
            (t.kind.clone(), t.dir.clone()),
            (Some(TransitionKind::Fade), None)
        );
        let t = with_spec(Some(&t), &spec("split", Some("vert")))
            .unwrap()
            .unwrap();
        assert_eq!(t.dir.as_deref(), Some("vert"));
        assert!(t.advance_on_click.is_none() && t.advance_after_ms.is_none());
        assert!(with_spec(Some(&t), &spec("none", None)).unwrap().is_none());
        assert!(with_spec(None, &spec("zoom", None)).is_err());
        let mut keep = spec("keep", None);
        keep.advance_after_ms = Some(3000);
        keep.advance_on_click = Some(false);
        let k = with_spec(Some(&t), &keep).unwrap().unwrap();
        assert_eq!(
            (k.kind, k.advance_after_ms, k.advance_on_click),
            (Some(TransitionKind::Split), Some(3000), Some(false))
        );
        assert_eq!(speed_ms(Some("slow")), 1000);
    }
}
