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
    TransitionKind, TransitionSound,
};
use bo_pptx::transitions;
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
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionSpec {
    /// `none` or a [`bo_pptx::transitions::CATALOG`] id (`fade`, `vortex`, `fallOver`, …); `keep`
    /// keeps the current effect and changes the timing only.
    pub kind: String,
    /// Variant of the effect (its direction: `l` | `u` | `r` | `d`, corners, `horz` | `vert`,
    /// `horzIn`, `black`, spokes, …; see `TransitionOut::dirs`); invalid ones fall back to the default.
    #[serde(default)]
    pub dir: Option<String>,
    /// Duration in ms (`p14:dur`); `None` keeps the current one.
    #[serde(default)]
    pub dur_ms: Option<u32>,
    /// Advance on mouse click (§19.3.1.50 transition@advClick); `None` keeps it.
    #[serde(default)]
    pub advance_on_click: Option<bool>,
    /// Advance automatically after this many ms (0 = not automatically).
    #[serde(default)]
    pub advance_after_ms: Option<u32>,
    /// Sound (§19.3.1.42 sndAc): `none` removes it, `stop` stops the sound playing; `None` keeps it.
    #[serde(default)]
    pub sound: Option<String>,
    /// A new sound: WAV bytes in base64 (§19.3.1.44 snd embeds WAV audio).
    #[serde(default)]
    pub sound_data: Option<String>,
    /// Name of the new sound.
    #[serde(default)]
    pub sound_name: Option<String>,
    /// Loop the sound until the next one (`stSnd@loop`); `None` keeps it.
    #[serde(default)]
    pub sound_loop: Option<bool>,
}

/// The sound of a transition as seen by the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundOut {
    /// The transition stops the sound playing (§19.3.1.19 endSnd).
    pub stop: bool,
    /// Embedded WAV part (bytes via `Engine::media`).
    pub part: Option<String>,
    /// Name of the sound.
    pub name: String,
    /// Plays until the next sound.
    #[serde(rename = "loop")]
    pub looped: bool,
}

/// A slide's transition as seen by the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionOut {
    /// Modelled kind (`fade`, `push`, …) or the element name of an unknown one.
    pub kind: String,
    /// The host can play it as is (others play as a fade).
    pub known: bool,
    /// Variant of the effect, if any (see [`TransitionSpec::dir`]).
    pub dir: Option<String>,
    /// Variants the effect offers (first: the default).
    pub dirs: Vec<&'static str>,
    /// Sound played with the transition.
    pub sound: Option<SoundOut>,
    /// Effective duration (p14:dur, else the speed).
    pub dur_ms: u32,
    /// Advance on mouse click (`advClick`).
    pub advance_on_click: bool,
    /// Advance automatically after this many ms (`advTm`); `None` = not automatically.
    pub advance_after_ms: Option<u32>,
}

/// One effect of the main sequence as seen by the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimOut {
    /// 0-based position in the main sequence.
    pub index: usize,
    /// Id of the animated shape (`p:cNvPr/@id`).
    pub shape_id: u32,
    /// Name of the animated shape (`p:cNvPr/@name`).
    pub shape_name: String,
    /// `appear` | `fade` | `flyIn` | `wipe`, an id of `bo_pptx::anim_catalog` (`zoom`, `spin`,
    /// `lineDown`, …) or `other`.
    pub effect: String,
    /// `entr` | `exit` | `emph` | `path` | `other`.
    pub class: &'static str,
    /// Variant of the effect (direction, …), if any.
    pub direction: Option<String>,
    /// Variants the effect offers (first: the default).
    pub dirs: Vec<&'static str>,
    /// `onClick` | `withPrevious` | `afterPrevious`.
    pub trigger: &'static str,
    /// Delay before the effect starts, in ms.
    pub delay_ms: u32,
    /// Duration of the effect, in ms.
    pub dur_ms: u32,
    /// Click step (0 = with the slide) and times relative to it (`Timing::timeline`).
    pub click: usize,
    /// Start time relative to the click step, in ms.
    pub start_ms: u32,
    /// End time relative to the click step, in ms.
    pub end_ms: u32,
    /// Bounds of the animated top-level shape `[x, y, w, h]` in points.
    pub rect: Option<[f64; 4]>,
    /// Show layer holding the shape (slide show only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<usize>,
    /// What the effect does, read from its behaviours (§19.5): the slide show plays it.
    pub play: bo_pptx::anim_play::EffectPlay,
}

/// One layer of the slide show (bottom to top).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayerOut {
    /// Top-level shapes drawn by the layer.
    pub ids: Vec<u32>,
    /// Whether the layer holds the background and master/layout shapes.
    pub backdrop: bool,
    /// Display list of the layer (same format as a rendered page).
    pub page: bo_render::PageList,
}

/// A video or audio clip on a slide (§20.1.3.6 videoFile, §20.1.3.1 audioFile).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaOut {
    /// Shape id of the picture frame (`p:cNvPr/@id`).
    pub id: u32,
    /// Shape name (`p:cNvPr/@name`).
    pub name: String,
    /// `video` | `audio`.
    pub kind: &'static str,
    /// Box of the picture frame on the slide (axis-aligned bounds), points.
    pub rect: [f64; 4],
    /// Embedded media part (bytes via `Engine::media`).
    pub part: Option<String>,
    /// MIME type of the media part (its content type, else from the file extension).
    pub mime: Option<String>,
    /// External target of linked media (not played).
    pub link: Option<String>,
}

/// Everything the slide show needs to play a slide.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowOut {
    /// 0-based slide index.
    pub slide: usize,
    /// Slide width in points.
    pub width: f32,
    /// Slide height in points.
    pub height: f32,
    /// Transition into this slide; `None` = none.
    pub transition: Option<TransitionOut>,
    /// Layers to draw, bottom to top.
    pub layers: Vec<LayerOut>,
    /// Effects of the main sequence with their layer.
    pub animations: Vec<AnimOut>,
    /// Clicks needed to play every effect.
    pub clicks: usize,
    /// Video and audio clips of the slide.
    pub media: Vec<MediaOut>,
    /// Morph from the previous slide ([MS-PPTX] p159:morph), when that is the transition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub morph: Option<super::morph::MorphOut>,
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Catalog id of the transition's effect (`known`), else its element name.
fn kind_name(t: &Transition) -> (String, bool, Vec<&'static str>) {
    let Some(k) = &t.kind else {
        return ("cut".to_owned(), true, Vec::new());
    };
    let element = match k {
        TransitionKind::Cut => "p:cut".to_owned(),
        TransitionKind::Fade => "p:fade".to_owned(),
        TransitionKind::Push => "p:push".to_owned(),
        TransitionKind::Wipe => "p:wipe".to_owned(),
        TransitionKind::Cover => "p:cover".to_owned(),
        TransitionKind::Pull => "p:pull".to_owned(),
        TransitionKind::Split => "p:split".to_owned(),
        TransitionKind::Random => "p:random".to_owned(),
        TransitionKind::Other(n) if n.contains(':') => n.clone(),
        TransitionKind::Other(n) => format!("p:{n}"),
    };
    match transitions::recognise(&element, &t.attrs) {
        Some(d) => (d.id.to_owned(), true, transitions::variants(d)),
        None => (
            element.trim_start_matches("p:").to_owned(),
            false,
            Vec::new(),
        ),
    }
}

/// The transition of a slide for the UI.
pub fn transition_out(pres: &Presentation, slide: usize) -> Option<TransitionOut> {
    let s = pres.slides.get(slide)?;
    let t = s.transition.as_ref()?;
    let (kind, known, dirs) = kind_name(t);
    let sound = t.sound.as_ref().map(|snd| match snd {
        TransitionSound::Stop => SoundOut {
            stop: true,
            part: None,
            name: String::new(),
            looped: false,
        },
        TransitionSound::Start {
            embed,
            name,
            looped,
        } => SoundOut {
            stop: false,
            part: s
                .info
                .rels
                .iter()
                .find(|r| &r.id == embed && !r.external)
                .map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target)),
            name: name.clone(),
            looped: *looped,
        },
    });
    Some(TransitionOut {
        kind,
        known,
        dir: t
            .dir
            .clone()
            .or_else(|| dirs.first().map(|d| (*d).to_owned())),
        dirs,
        sound,
        dur_ms: t.dur_ms.unwrap_or_else(|| speed_ms(t.speed.as_deref())),
        advance_on_click: t.advance_on_click.unwrap_or(true),
        advance_after_ms: t.advance_after_ms,
    })
}

fn effect_name(e: &AnimEffect) -> String {
    match e {
        AnimEffect::Appear => "appear".into(),
        AnimEffect::Fade => "fade".into(),
        AnimEffect::FlyIn => "flyIn".into(),
        AnimEffect::Wipe => "wipe".into(),
        AnimEffect::Named(n) => n.clone(),
        AnimEffect::Other => "other".into(),
    }
}

/// Variants of an effect: the catalog's, or the four sides of fly-in and wipe.
fn effect_dirs(e: &AnimEffect) -> Vec<&'static str> {
    match e {
        AnimEffect::FlyIn | AnimEffect::Wipe => vec!["b", "t", "l", "r"],
        AnimEffect::Named(n) => bo_pptx::anim_catalog::by_id(n)
            .map(|d| d.dirs.iter().map(|(v, _)| *v).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
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
    let (sw, sh) = super::slide_size(pres);
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
            // §19.5.1 `#ppt_x` … of the animated shape, in slide widths / heights.
            let own = find(&placed, a.shape_id).map(|p| p.b);
            let base = own.map_or(
                bo_pptx::anim_play::ShapeBase {
                    x: 0.5,
                    y: 0.5,
                    w: 0.2,
                    h: 0.2,
                },
                |b| bo_pptx::anim_play::ShapeBase {
                    x: b.cx / sw.max(1.0),
                    y: b.cy / sh.max(1.0),
                    w: b.w / sw.max(1.0),
                    h: b.h / sh.max(1.0),
                },
            );
            let play = bo_pptx::anim_play::effect_play(&bo_pptx::anim::effect_xml(a), &base);
            AnimOut {
                index: i,
                shape_id: a.shape_id,
                shape_name: name,
                effect: effect_name(&a.effect),
                class: class_name(&a.class),
                direction: a.direction.clone(),
                dirs: effect_dirs(&a.effect),
                trigger: trigger_name(a.trigger),
                delay_ms: a.delay_ms,
                dur_ms: a.dur_ms,
                click: entry.map_or(0, |e| e.click),
                start_ms: entry.map_or(0, |e| e.start_ms),
                end_ms: entry.map_or(0, |e| e.end_ms),
                rect: top.and_then(|id| find(&placed, id)).map(|p| rect_of(&p.b)),
                layer: None,
                play,
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

/// The model kind of catalog entry `d` (the eight ECMA effects BlueSlide models by name, the others
/// by element).
fn kind_of(d: &transitions::TransitionDef) -> TransitionKind {
    match d.element {
        "p:cut" => TransitionKind::Cut,
        "p:fade" => TransitionKind::Fade,
        "p:push" => TransitionKind::Push,
        "p:wipe" => TransitionKind::Wipe,
        "p:cover" => TransitionKind::Cover,
        "p:pull" => TransitionKind::Pull,
        "p:split" => TransitionKind::Split,
        "p:random" => TransitionKind::Random,
        e => TransitionKind::Other(e.trim_start_matches("p:").to_owned()),
    }
}

/// The transition after applying `spec` to `old`.
fn with_spec(old: Option<&Transition>, spec: &TransitionSpec) -> R<Option<Transition>> {
    let mut t = old.cloned().unwrap_or_default();
    // The catalog entry of the new or current effect.
    let def = if spec.kind == "keep" {
        let (id, known, _) = kind_name(&t);
        known.then(|| transitions::by_id(&id)).flatten()
    } else {
        if spec.kind == "none" {
            return Ok(None);
        }
        let d = transitions::by_id(&spec.kind).ok_or(EditError::Refused("badTransition"))?;
        let kind = kind_of(d);
        let (old_id, _, _) = kind_name(&t);
        if t.kind.as_ref() != Some(&kind) || old_id != d.id {
            t.dir = None;
            t.attrs.clear();
        }
        t.kind = Some(kind);
        Some(d)
    };
    if let Some(d) = def {
        let wanted = spec.dir.as_deref().or(t.dir.as_deref());
        let variant = transitions::variants(d)
            .into_iter()
            .find(|v| Some(*v) == wanted)
            .or_else(|| transitions::variants(d).first().copied());
        t.attrs = transitions::attrs_for(d, variant);
        t.dir = variant.map(str::to_owned);
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
    match spec.sound.as_deref() {
        Some("none") => t.sound = None,
        Some("stop") => t.sound = Some(TransitionSound::Stop),
        _ => {}
    }
    if let (Some(l), Some(TransitionSound::Start { looped, .. })) =
        (spec.sound_loop, t.sound.as_mut())
    {
        *looped = l;
    }
    Ok(Some(t))
}

/// A WAV file (RIFF … WAVE), the only audio §19.3.1.44 snd embeds.
fn is_wav(b: &[u8]) -> bool {
    b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WAVE"
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
    // A new sound: one WAV part, an audio relationship from each slide.
    let wav = match &spec.sound_data {
        Some(data) => {
            use base64::Engine as _;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data.trim())
                .map_err(|_| EditError::Refused("badSound"))?;
            if !is_wav(&bytes) {
                return refuse("badSound");
            }
            let part = super::build::unique_part(tx.pres, "/ppt/media/audio", "wav");
            let at = tx.pres.parts.len();
            tx.op(super::ops::SlideOp::Parts {
                index: at,
                remove: vec![],
                insert: vec![bo_model::slide::RawPart {
                    name: part.clone(),
                    content_type: Some("audio/wav".into()),
                    data: bytes,
                }],
            })?;
            Some(part)
        }
        None => None,
    };
    for i in targets {
        let mut new = with_spec(tx.pres.slides[i].transition.as_ref(), spec)?;
        if let (Some(part), Some(t)) = (&wav, new.as_mut()) {
            let rels = &tx.pres.slides[i].info.rels;
            let id = (1..=rels.len() + 1)
                .map(|n| format!("rId{n}"))
                .find(|id| rels.iter().all(|r| &r.id != id))
                .unwrap_or_default();
            let target = format!("../media/{}", part.rsplit('/').next().unwrap_or_default());
            let rel_id = id.clone();
            tx.fields(i, |f| {
                // §19.3.1.44 snd: r:embed of an audio relationship.
                f.rels.push(bo_model::slide::Rel {
                    id: rel_id,
                    rel_type: format!("{}audio", bo_pptx::RELATIONSHIP_NS),
                    target,
                    external: false,
                });
            })?;
            t.sound = Some(TransitionSound::Start {
                embed: id,
                name: spec
                    .sound_name
                    .clone()
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| "sound.wav".into()),
                looped: spec.sound_loop.unwrap_or(false),
            });
        }
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
        other if bo_pptx::anim_catalog::by_id(other).is_some() => {
            AnimEffect::Named(other.to_owned())
        }
        _ => return refuse("badEffect"),
    })
}

fn parse_class(c: &str) -> R<AnimClass> {
    Ok(match c {
        "entr" => AnimClass::Entrance,
        "exit" => AnimClass::Exit,
        "emph" => AnimClass::Emphasis,
        "path" => AnimClass::Path,
        _ => return refuse("badEffect"),
    })
}

/// The class an effect needs: emphasis and motion-path effects have their own; entrance effects
/// play backwards as exits.
fn class_for(e: &AnimEffect, asked: Option<&AnimClass>) -> AnimClass {
    use bo_pptx::anim_catalog::Class;
    let own = match e {
        AnimEffect::Named(n) => bo_pptx::anim_catalog::by_id(n).map(|d| d.class),
        _ => Some(Class::EntranceExit),
    };
    match own {
        Some(Class::Emphasis) => AnimClass::Emphasis,
        Some(Class::Path) => AnimClass::Path,
        // An entrance effect replacing an emphasis or a path comes in again.
        _ => match asked {
            Some(AnimClass::Exit) => AnimClass::Exit,
            _ => AnimClass::Entrance,
        },
    }
}

fn parse_trigger(t: &str) -> R<AnimTrigger> {
    Ok(match t {
        "onClick" => AnimTrigger::OnClick,
        "withPrevious" => AnimTrigger::WithPrevious,
        "afterPrevious" => AnimTrigger::AfterPrevious,
        _ => return refuse("badTrigger"),
    })
}

/// `l` | `r` | `t` | `b` for fly-in / wipe, the catalog's variants for its effects; nothing for
/// the others.
fn valid_anim_dir(e: &AnimEffect, d: Option<&str>) -> Option<String> {
    if let AnimEffect::Named(_) = e {
        let dirs = effect_dirs(e);
        return dirs
            .iter()
            .find(|v| Some(**v) == d)
            .or_else(|| dirs.first())
            .map(|v| (*v).to_owned());
    }
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
    /// Effect: `appear` | `fade` | `flyIn` | `wipe` or an id of `bo_pptx::anim_catalog`.
    #[serde(default)]
    pub effect: Option<String>,
    /// `entr` | `exit` (emphasis and motion-path effects have their own class).
    #[serde(default)]
    pub class: Option<String>,
    /// Direction of fly-in / wipe effects (`l` | `r` | `t` | `b`, default `b`).
    #[serde(default)]
    pub direction: Option<String>,
    /// `onClick` | `withPrevious` | `afterPrevious`.
    #[serde(default)]
    pub trigger: Option<String>,
    /// Duration in ms (default 500 for a new effect).
    #[serde(default)]
    pub dur_ms: Option<u32>,
    /// Delay before the effect, in ms.
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
    let asked = spec.class.as_deref().map(parse_class).transpose()?;
    let class = class_for(&effect, asked.as_ref());
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
            a.class = class_for(&a.effect, Some(&a.class));
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

/// Removes effect `index` of the slide's main sequence (refused when out of range).
pub fn cmd_remove_animation(tx: &mut Tx<'_>, sel: &SlideSel, index: usize) -> R<()> {
    edit_timing(tx, sel.slide, |anims| {
        if index >= anims.len() {
            return refuse("noAnimation");
        }
        anims.remove(index);
        Ok(())
    })
}

/// Moves effect `from` to position `to` of the slide's main sequence (refused when out of range).
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
            ..TransitionSpec::default()
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
        // Fade has no direction: the default variant (smoothly).
        assert_eq!(
            (t.kind.clone(), t.dir.as_deref()),
            (Some(TransitionKind::Fade), Some("smooth"))
        );
        assert_eq!(t.attrs, Vec::<(String, String)>::new());
        let t = with_spec(Some(&t), &spec("split", Some("vert")))
            .unwrap()
            .unwrap();
        assert_eq!(t.dir.as_deref(), Some("vert"));
        assert!(t.advance_on_click.is_none() && t.advance_after_ms.is_none());
        assert!(with_spec(Some(&t), &spec("none", None)).unwrap().is_none());
        assert!(with_spec(None, &spec("nope", None)).is_err());
        // §19.3.1.47 split: orientation and direction from the variant.
        let s = with_spec(None, &spec("split", Some("vertIn")))
            .unwrap()
            .unwrap();
        assert_eq!(
            s.attrs,
            vec![
                ("orient".into(), "vert".into()),
                ("dir".into(), "in".into())
            ]
        );
        // [MS-PPTX] effects by element; p15 presets by their prst.
        let v = with_spec(None, &spec("vortex", Some("d")))
            .unwrap()
            .unwrap();
        assert_eq!(v.kind, Some(TransitionKind::Other("p14:vortex".into())));
        let o = with_spec(None, &spec("origami", Some("r")))
            .unwrap()
            .unwrap();
        assert_eq!(
            o.attrs,
            vec![
                ("prst".into(), "origami".into()),
                ("invX".into(), "1".into())
            ]
        );
        // Changing p15 presets keeps nothing of the old one.
        let c = with_spec(Some(&o), &spec("crush", None)).unwrap().unwrap();
        assert_eq!(c.attrs, vec![("prst".into(), "crush".into())]);
        assert_eq!(c.dir.as_deref(), Some("l"));
        // §19.3.1.42 sndAc: stop the sound, then none.
        let mut snd = spec("keep", None);
        snd.sound = Some("stop".into());
        let st = with_spec(Some(&c), &snd).unwrap().unwrap();
        assert_eq!(st.sound, Some(TransitionSound::Stop));
        snd.sound = Some("none".into());
        assert_eq!(with_spec(Some(&st), &snd).unwrap().unwrap().sound, None);
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
