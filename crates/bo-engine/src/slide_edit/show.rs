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
    AnimClass, AnimEffect, AnimIterate, AnimTrigger, Animation, Presentation, ShapeNode, Timing,
    Transition, TransitionKind, TransitionSound,
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
    /// Runs of the effect (§19.5.29 repeatCount); 0 = until the next click.
    pub repeat: u32,
    /// Shape whose click starts the effect (interactive sequence), if any.
    pub trigger_shape: Option<u32>,
    /// Bookmark of the trigger shape's media that starts the effect ([MS-PPTX] bmkTgt).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_bookmark: Option<String>,
    /// The §19.5 `p:cmd` of a media call effect (`playFrom(0.0)`, `togglePause`, …): the slide
    /// show plays the clip instead of animating the shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_cmd: Option<String>,
    /// Show layer holding the shape (slide show only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<usize>,
    /// What the effect does, read from its behaviours (§19.5): the slide show plays it.
    pub play: bo_pptx::anim_play::EffectPlay,
    /// §19.5.4 animMotion@path of a motion path (offsets in slide widths / heights).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The motion path stays where it is on the slide when the shape moves (§19.5.4
    /// pathEditMode `fixed`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub path_locked: bool,
    /// §19.5 iterate: `wd` (by word) | `lt` (by letter) | `el`; `None` = the shape as a whole.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_by: Option<String>,
    /// Time between the starts of two words / letters, ms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_step_ms: Option<u32>,
    /// The last word / letter first (§19.5 iterate@backwards).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub text_backwards: bool,
    /// Slide show: the shape's layer drawn in the colours the effect's §19.5.2 animClr
    /// behaviours end on (faded in over the plain layer while they run).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tint: Option<bo_render::PageList>,
    /// The part of the shape the effect animates (paragraphs, background, chart element,
    /// SmartArt node); `None` = the shape as a whole.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetOut>,
    /// How the shape builds (paragraphs, chart or SmartArt elements), when it can.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<super::anim_build::BuildOut>,
    /// After the effect: `dim` | `hide` | `hideOnClick`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<&'static str>,
    /// Dim colour `RRGGBB` (`after` = `dim`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dim_color: Option<String>,
    /// The shape goes back to how it was once the effect is done (§19.5.29 fill `remove`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rewind: bool,
    /// Sound played as the effect starts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound: Option<SoundOut>,
    /// Slide show: the pieces of the layer's split (`LayerOut::parts`, 0 = the body) the effect
    /// plays on; empty = the whole layer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<usize>,
}

/// The part of a shape an effect animates.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetOut {
    /// `paragraphs` | `background` | `chart` | `diagram`.
    pub kind: &'static str,
    /// First / last paragraph (0-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<u32>,
    /// Last paragraph.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<u32>,
    /// Chart series (0-based), when the step is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<u32>,
    /// Chart category (0-based), when the step is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<u32>,
    /// Chart build step (`series`, `gridLegend`, …) or SmartArt step (`sp`, `bg`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    /// SmartArt node: 1-based position in reading order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<u32>,
}

/// A layer cut into pieces (an effect built by paragraph, chart element or SmartArt node).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayerParts {
    /// The drawings: the body (index 0, drawn whatever is shown) then each piece.
    pub pages: Vec<bo_render::PageList>,
    /// Bounds `[x, y, w, h]` of each drawing, points (`None`: draws nothing).
    pub rects: Vec<Option<[f64; 4]>>,
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
    /// The layer's text cut into letters, for effects run word by word or letter by letter
    /// (§19.5 iterate); only on layers that have such effects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<super::anim_text::TextSplit>,
    /// The layer cut into its body and pieces, for effects on parts of its shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parts: Option<LayerParts>,
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
    /// How the clip plays (trim, fades, bookmarks, start, options, captions).
    pub playback: super::media_playback::MediaPlayback,
    /// Narration / camera of a slide show recording (SLIDE-REC-001).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded: Option<super::record::RecKind>,
}

/// A top-level shape of a slide.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObjectOut {
    /// Shape id (`cNvPr@id`).
    pub id: u32,
    /// Shape name (`cNvPr@name`).
    pub name: String,
}

/// The top-level shapes of a slide.
pub fn objects_out(pres: &Presentation, slide: usize) -> Vec<ObjectOut> {
    pres.slides.get(slide).map_or_else(Vec::new, |s| {
        s.data
            .shapes
            .iter()
            .filter_map(ShapeNode::nv)
            .map(|nv| ObjectOut {
                id: nv.id,
                name: nv.name.clone(),
            })
            .collect()
    })
}

/// A shape whose click starts effects.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerOut {
    /// Shape id (`cNvPr@id`).
    pub shape_id: u32,
    /// Bounds `[x, y, w, h]` in points.
    pub rect: [f64; 4],
}

/// The trigger shapes of a slide's interactive sequences.
pub fn triggers_out(pres: &Presentation, slide: usize) -> Vec<TriggerOut> {
    let Some(t) = pres.slides.get(slide).and_then(|s| s.timing.as_ref()) else {
        return Vec::new();
    };
    let placed = place_slide(pres, slide);
    let mut out: Vec<TriggerOut> = Vec::new();
    // Clicks on the shape (not its media bookmarks); a clip whose click only plays or pauses
    // itself is no trigger button: the clip handles the click.
    let clicks = |id: u32| {
        t.animations
            .iter()
            .filter(move |a| a.trigger_shape == Some(id) && a.trigger_bookmark.is_none())
    };
    let own_media = |a: &bo_model::slide::Animation, id: u32| {
        a.shape_id == id && a.class == bo_model::slide::AnimClass::Other("mediacall".into())
    };
    for id in t
        .animations
        .iter()
        .filter(|a| a.trigger_bookmark.is_none())
        .filter_map(|a| a.trigger_shape)
    {
        if out.iter().any(|o| o.shape_id == id) || clicks(id).all(|a| own_media(a, id)) {
            continue;
        }
        if let Some(p) = find(&placed, id) {
            out.push(TriggerOut {
                shape_id: id,
                rect: rect_of(&p.b),
            });
        }
    }
    out
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
    /// Shapes that start effects when clicked (interactive sequences), with their boxes.
    pub triggers: Vec<TriggerOut>,
    /// Shapes whose hyperlink the show follows (custom shows, jumps).
    pub links: Vec<super::present::LinkOut>,
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
pub(super) fn top_index(pres: &Presentation, slide: usize, id: u32) -> Option<usize> {
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
    // Effects run word by word / letter by letter last as long as their text needs.
    let timeline = t.timeline_with(|_, a| {
        a.iterate.as_ref().map_or(1, |it| {
            super::anim_text::piece_count(pres, slide, a.shape_id, &it.kind)
        })
    });
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
            let xml = bo_pptx::anim::effect_xml(a);
            let play = bo_pptx::anim_play::effect_play(&xml, &base);
            let path = (a.class == AnimClass::Path)
                .then(|| bo_pptx::anim::motion_path(&xml))
                .flatten();
            let custom = path.is_some() && a.effect == AnimEffect::Other;
            let iter = a.iterate.as_ref().filter(|it| it.kind != "el");
            AnimOut {
                index: i,
                shape_id: a.shape_id,
                shape_name: name,
                effect: if custom {
                    CUSTOM_PATH.to_owned()
                } else {
                    effect_name(&a.effect)
                },
                path_locked: path.is_some() && bo_pptx::anim::path_locked(&xml),
                path,
                text_by: iter.map(|it| it.kind.clone()),
                text_step_ms: iter.map(|it| it.step_ms(a.dur_ms)),
                text_backwards: iter.is_some_and(|it| it.backwards),
                tint: None,
                target: target_out(pres, slide, a),
                build: super::anim_build::build_out(pres, slide, t, i),
                after: super::anim_build::after_name(a.opts.after.as_ref()),
                dim_color: match &a.opts.after {
                    Some(bo_model::slide::AnimAfter::Dim { rgb, .. }) => Some(format!("{rgb:06X}")),
                    _ => None,
                },
                rewind: a.opts.rewind,
                sound: a.opts.sound.as_ref().map(|snd| sound_of(pres, slide, snd)),
                parts: Vec::new(),
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
                repeat: match a.repeat {
                    Some(u32::MAX) => 0,
                    Some(r) => (r / 1000).max(1),
                    None => 1,
                },
                trigger_shape: a.trigger_shape,
                trigger_bookmark: a.trigger_bookmark.clone(),
                media_cmd: super::media_playback::media_cmd(a).map(str::to_owned),
                layer: None,
                play,
            }
        })
        .collect()
}

/// The target of an effect for the UI.
fn target_out(pres: &Presentation, slide: usize, a: &Animation) -> Option<TargetOut> {
    use bo_model::slide::AnimTarget;
    let none = TargetOut {
        kind: "background",
        start: None,
        end: None,
        series: None,
        category: None,
        step: None,
        node: None,
    };
    Some(match a.opts.target.as_ref()? {
        AnimTarget::Background => none,
        AnimTarget::Paragraphs { start, end } => TargetOut {
            kind: "paragraphs",
            start: Some(*start),
            end: Some(*end),
            ..none
        },
        AnimTarget::Chart {
            series,
            category,
            step,
        } => TargetOut {
            kind: "chart",
            series: u32::try_from(*series).ok(),
            category: u32::try_from(*category).ok(),
            step: Some(step.clone()),
            ..none
        },
        AnimTarget::Diagram { id, step } => TargetOut {
            kind: "diagram",
            step: Some(step.clone()),
            node: match super::anim_build::kind_of(pres, slide, a.shape_id) {
                Some(super::anim_build::Kind::Diagram(d)) => d
                    .nodes
                    .iter()
                    .position(|(n, _)| n == id)
                    .and_then(|p| u32::try_from(p + 1).ok()),
                _ => None,
            },
            ..none
        },
    })
}

/// The sound of an effect (its WAV part through the slide's relationship).
fn sound_of(pres: &Presentation, slide: usize, snd: &bo_model::slide::AnimSound) -> SoundOut {
    let part = pres.slides.get(slide).and_then(|s| {
        s.info
            .rels
            .iter()
            .find(|r| r.id == snd.embed && !r.external)
            .map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target))
    });
    SoundOut {
        stop: false,
        part,
        name: snd.name.clone(),
        looped: false,
    }
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
    /// Runs of the effect (1 = once, 0 = until the next click).
    #[serde(default)]
    pub repeat: Option<u32>,
    /// Shape whose click starts the effect (0 = the main sequence).
    #[serde(default)]
    pub trigger_shape: Option<u32>,
    /// A motion path drawn by the author (§19.5.4 path syntax, offsets in slide widths / heights
    /// from the shape's position): with effect [`CUSTOM_PATH`] for a new effect, or the new
    /// points of a motion path (Edit Points).
    #[serde(default)]
    pub path: Option<String>,
    /// Run the motion path the other way (Reverse Path Direction).
    #[serde(default)]
    pub reverse_path: Option<bool>,
    /// Lock the motion path on the slide (§19.5.4 pathEditMode `fixed`) or let it move with the
    /// shape (`relative`).
    #[serde(default)]
    pub lock_path: Option<bool>,
    /// Animate text: `all` (as one object), `word` or `letter` (§19.5 iterate `wd` / `lt`).
    #[serde(default)]
    pub text_by: Option<String>,
    /// Delay between words / letters as a percentage of the effect's duration (default 10 for
    /// words, 5 for letters).
    #[serde(default)]
    pub text_delay_pct: Option<u32>,
    /// Build the shape (see `anim_build::BuildOut::value`): text `whole` | `allAtOnce` | `p1` …
    /// `p5`; chart `whole` | `allAtOnce` | `series` | `category` | `seriesEl` | `categoryEl`;
    /// SmartArt `whole` | `allAtOnce` | `one` | `lvlAtOnce` | `lvlOne`.
    #[serde(default)]
    pub build: Option<String>,
    /// Build in reverse order (text, SmartArt).
    #[serde(default)]
    pub build_reverse: Option<bool>,
    /// Animate the shape (text) or the chart background as the build's first step.
    #[serde(default)]
    pub build_bg: Option<bool>,
    /// After the effect: `none` | `dim` | `hide` | `hideOnClick`.
    #[serde(default)]
    pub after: Option<String>,
    /// Dim colour `RRGGBB` (default 808080).
    #[serde(default)]
    pub dim_color: Option<String>,
    /// Rewind when done playing.
    #[serde(default)]
    pub rewind: Option<bool>,
    /// Sound: `none` removes it.
    #[serde(default)]
    pub sound: Option<String>,
    /// A new sound: WAV bytes in base64.
    #[serde(default)]
    pub sound_data: Option<String>,
    /// Name of the new sound.
    #[serde(default)]
    pub sound_name: Option<String>,
}

/// Effect id of a motion path drawn by the author.
pub const CUSTOM_PATH: &str = "customPath";

/// What a `text_by` choice does to an effect's §19.5 iterate.
enum IterEdit {
    /// Unchanged.
    Keep,
    /// The new iterate (`None`: the shape as one object).
    Set(Option<AnimIterate>),
}

/// The §19.5 iterate of a `text_by` choice.
fn iterate_of(spec: &AnimSpec, old: Option<&AnimIterate>) -> R<IterEdit> {
    let kind = match spec.text_by.as_deref() {
        None => match (old, spec.text_delay_pct) {
            (Some(o), Some(_)) => o.kind.clone(),
            _ => return Ok(IterEdit::Keep),
        },
        Some("all") => return Ok(IterEdit::Set(None)),
        Some("word") => "wd".to_owned(),
        Some("letter") => "lt".to_owned(),
        Some(_) => return refuse("badTextBy"),
    };
    let pct = spec
        .text_delay_pct
        .map(|p| p.min(1000) * 1000)
        .or(old.filter(|o| o.kind == kind).and_then(|o| o.pct))
        .unwrap_or(if kind == "lt" { 5000 } else { 10_000 });
    Ok(IterEdit::Set(Some(AnimIterate {
        kind,
        pct: Some(pct),
        abs_ms: None,
        backwards: old.is_some_and(|o| o.backwards),
    })))
}

/// A §19.5.4 path of the author: `M`, `L`, `C`, `Z`, `E` commands and numbers only.
fn valid_path(p: &str) -> R<String> {
    let ok = !p.trim().is_empty()
        && p.len() <= 20_000
        && p.chars()
            .all(|c| c.is_ascii_digit() || " ,.-+eEMLCZmlcz".contains(c))
        && bo_pptx::anim_play::motion_points(p).len() >= 2;
    if ok {
        Ok(p.trim().to_owned())
    } else {
        refuse("badPath")
    }
}

/// Apply the motion-path parts of `spec` to effect `a` (its effect node becomes the kept source).
fn edit_path(a: &mut Animation, spec: &AnimSpec) -> R<()> {
    if spec.path.is_none() && spec.reverse_path != Some(true) && spec.lock_path.is_none() {
        return Ok(());
    }
    if a.class != AnimClass::Path {
        return refuse("notAPath");
    }
    let mut par = bo_pptx::anim::effect_xml(a);
    let Some(mut path) = bo_pptx::anim::motion_path(&par) else {
        return refuse("notAPath");
    };
    let mut changed = false;
    if let Some(p) = spec.path.as_deref() {
        path = valid_path(p)?;
        changed = true;
    }
    if spec.reverse_path == Some(true) {
        path = bo_pptx::anim::reverse_path(&path);
        changed = true;
    }
    if changed {
        bo_pptx::anim::set_motion_path(&mut par, &path);
        if let Some(bo_model::slide::XmlNode::Element(ctn)) = par.children.first_mut() {
            ctn.attrs
                .retain(|(k, _)| k != "presetID" && k != "presetSubtype");
        }
        // No longer the catalog's path: a path of the author.
        a.effect = AnimEffect::Other;
        a.preset_id = None;
        a.preset_subtype = None;
        a.direction = None;
    }
    if let Some(l) = spec.lock_path {
        bo_pptx::anim::set_path_locked(&mut par, l);
    }
    a.source = Some(par);
    Ok(())
}

/// The model repeat (§19.5.29 repeatCount in 1000ths) of a count of runs.
fn repeat_of(n: u32) -> Option<u32> {
    match n {
        0 => Some(u32::MAX),
        1 => None,
        n => Some(n.min(1000) * 1000),
    }
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
    // A motion path drawn by the author: an animMotion of its points (§19.5.4).
    let custom = match spec.effect.as_deref() {
        Some(CUSTOM_PATH) => Some(valid_path(
            spec.path.as_deref().ok_or(EditError::Refused("badPath"))?,
        )?),
        _ => None,
    };
    let effect = if custom.is_some() {
        AnimEffect::Other
    } else {
        parse_effect(spec.effect.as_deref().unwrap_or("fade"))?
    };
    let asked = spec.class.as_deref().map(parse_class).transpose()?;
    let class = if custom.is_some() {
        AnimClass::Path
    } else {
        class_for(&effect, asked.as_ref())
    };
    let first = parse_trigger(spec.trigger.as_deref().unwrap_or("onClick"))?;
    let direction = valid_anim_dir(&effect, spec.direction.as_deref());
    let iterate = match iterate_of(spec, None)? {
        IterEdit::Set(it) => it,
        IterEdit::Keep => None,
    };
    let dur = spec.dur_ms.unwrap_or(if custom.is_some() {
        2000
    } else {
        DEFAULT_ANIM_MS
    });
    let ids = sel.ids.clone();
    edit_timing(tx, sel.slide, |anims| {
        for (k, id) in ids.into_iter().enumerate() {
            anims.push(Animation {
                source: custom
                    .as_deref()
                    .map(|p| bo_pptx::anim::custom_path_effect(id, dur.clamp(1, 600_000), p)),
                iterate: iterate.clone(),
                opts: bo_model::slide::AnimOptions::default(),
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
                dur_ms: dur.clamp(1, 600_000),
                preset_id: None,
                preset_subtype: None,
                repeat: spec.repeat.and_then(repeat_of),
                trigger_shape: spec.trigger_shape.filter(|t| *t != 0),
                trigger_bookmark: None,
            });
        }
        Ok(())
    })
}

/// Change effect `index` of the current slide. The effect, its timing and what happens after it
/// apply to every effect of its build (§19.5 bldLst); the trigger, the delay and the sound to
/// this one. A new build (`spec.build`) splits or joins the shape's effects.
pub fn cmd_set_animation(tx: &mut Tx<'_>, sel: &SlideSel, index: usize, spec: &AnimSpec) -> R<()> {
    let effect = spec.effect.as_deref().map(parse_effect).transpose()?;
    let class = spec.class.as_deref().map(parse_class).transpose()?;
    let trigger = spec.trigger.as_deref().map(parse_trigger).transpose()?;
    let slide = sel.slide;
    let (shape, kind) = {
        let a = tx
            .pres
            .slides
            .get(slide)
            .and_then(|s| s.timing.as_ref())
            .and_then(|t| t.animations.get(index))
            .ok_or(EditError::Refused("noAnimation"))?;
        (
            a.clone(),
            super::anim_build::kind_of(tx.pres, slide, a.shape_id),
        )
    };
    let after = match spec.after.as_deref() {
        None => None,
        Some("none") => Some(None),
        Some("hide") => Some(Some(bo_model::slide::AnimAfter::Hide)),
        Some("hideOnClick") => Some(Some(bo_model::slide::AnimAfter::HideOnClick)),
        Some("dim") => {
            let rgb = match spec.dim_color.as_deref() {
                Some(c) => parse_rgb(c)?,
                None => match &shape.opts.after {
                    Some(bo_model::slide::AnimAfter::Dim { rgb, .. }) => *rgb,
                    _ => 0x80_80_80,
                },
            };
            Some(Some(bo_model::slide::AnimAfter::Dim {
                rgb,
                attrs: super::anim_build::dim_attrs(tx.pres, slide, &shape),
            }))
        }
        Some(_) => return refuse("badAfter"),
    };
    let after = match (after, spec.dim_color.as_deref(), &shape.opts.after) {
        // A new dim colour for an effect dimmed already.
        (None, Some(c), Some(bo_model::slide::AnimAfter::Dim { attrs, .. })) => {
            Some(Some(bo_model::slide::AnimAfter::Dim {
                rgb: parse_rgb(c)?,
                attrs: attrs.clone(),
            }))
        }
        (a, _, _) => a,
    };
    let sound = match (spec.sound.as_deref(), &spec.sound_data) {
        (_, Some(data)) => {
            let embed = super::action::ensure_sound(tx, slide, data)?;
            Some(Some(bo_model::slide::AnimSound {
                embed,
                name: spec
                    .sound_name
                    .clone()
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| "sound.wav".into()),
            }))
        }
        (Some("none"), None) => Some(None),
        (Some(_), None) => return refuse("badSound"),
        (None, None) => None,
    };
    edit_timing_full(tx, slide, |t| {
        let group = super::anim_build::members(&t.animations, index);
        if group.is_empty() {
            return refuse("noAnimation");
        }
        let shape_changed = effect.is_some() || class.is_some() || spec.direction.is_some();
        for &k in &group {
            let a = &mut t.animations[k];
            if let Some(e) = &effect {
                a.effect = e.clone();
            }
            if let Some(c) = &class {
                a.class = c.clone();
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
            if let Some(d) = spec.dur_ms {
                a.dur_ms = d.clamp(1, 600_000);
            }
            if let Some(r) = spec.repeat {
                a.repeat = repeat_of(r);
            }
            if let IterEdit::Set(it) = iterate_of(spec, a.iterate.as_ref())? {
                a.iterate = it;
            }
            if let Some(x) = &after {
                a.opts.after.clone_from(x);
            }
            if let Some(r) = spec.rewind {
                a.opts.rewind = r;
            }
        }
        let a = &mut t.animations[index];
        if let Some(tr) = trigger {
            a.trigger = tr;
        }
        if let Some(d) = spec.delay_ms {
            a.delay_ms = d.min(600_000);
        }
        if let Some(tg) = spec.trigger_shape {
            a.trigger_shape = (tg != 0).then_some(tg);
            a.trigger_bookmark = None;
        }
        if let Some(snd) = &sound {
            a.opts.sound.clone_from(snd);
        }
        edit_path(a, spec)?;
        if spec.build.is_some() || spec.build_reverse.is_some() || spec.build_bg.is_some() {
            let value = match spec.build.clone() {
                Some(v) => v,
                None => super::anim_build::build_out_of(t, kind.as_ref(), index)
                    .ok_or(EditError::Refused("noBuild"))?,
            };
            super::anim_build::rebuild(
                t,
                kind.as_ref(),
                index,
                &value,
                spec.build_reverse,
                spec.build_bg,
            )?;
        }
        Ok(())
    })
}

/// `RRGGBB` (with or without `#`) → `0xRRGGBB`.
fn parse_rgb(c: &str) -> R<u32> {
    let h = c.trim().trim_start_matches('#');
    if h.len() != 6 {
        return refuse("badColor");
    }
    u32::from_str_radix(h, 16).map_err(|_| EditError::Refused("badColor"))
}

/// Edit the whole timing of `slide` (effects and builds).
fn edit_timing_full(tx: &mut Tx<'_>, slide: usize, f: impl FnOnce(&mut Timing) -> R<()>) -> R<()> {
    let s = tx
        .pres
        .slides
        .get(slide)
        .ok_or(EditError::Refused("noSlide"))?;
    let mut timing: Timing = s.timing.clone().unwrap_or_default();
    f(&mut timing)?;
    tx.fields(slide, |fl| fl.timing = Some(timing))?;
    Ok(())
}

/// Offset (`a:off`, EMU) of a top-level node.
fn offset_of(n: &ShapeNode) -> Option<(i64, i64)> {
    match n {
        ShapeNode::Shape(s) => s.props.xfrm.as_ref()?.off,
        ShapeNode::Picture(p) => p.props.xfrm.as_ref()?.off,
        ShapeNode::Group(g) => g.props.xfrm.as_ref()?.off,
        ShapeNode::Connector(c) => c.props.xfrm.as_ref()?.off,
        ShapeNode::Frame(f) => f.xfrm.off,
        ShapeNode::Other(_) => None,
    }
}

/// A top-level shape moved by a transaction: slide, id, offset before and after (EMU).
type Moved = (usize, u32, (i64, i64), (i64, i64));

/// Locked motion paths (§19.5.4 pathEditMode `fixed`) stay where they are on the slide when the
/// transaction moves their top-level shape: the path moves back by the shape's move.
pub fn keep_locked_paths(tx: &mut Tx<'_>) -> R<()> {
    // (slide, id) → (offset before, offset after), from the top-level node splices.
    let mut moved: Vec<Moved> = Vec::new();
    for op in &tx.ops {
        let super::ops::SlideOp::Nodes {
            slide,
            parent,
            remove,
            insert,
            ..
        } = op
        else {
            continue;
        };
        if !parent.is_empty() {
            continue;
        }
        for r in remove {
            let (Some(nv), Some(off)) = (r.nv(), offset_of(r)) else {
                continue;
            };
            if !moved.iter().any(|m| m.0 == *slide && m.1 == nv.id) {
                moved.push((*slide, nv.id, off, off));
            }
        }
        for n in insert {
            let (Some(nv), Some(off)) = (n.nv(), offset_of(n)) else {
                continue;
            };
            if let Some(m) = moved.iter_mut().find(|m| m.0 == *slide && m.1 == nv.id) {
                m.3 = off;
            }
        }
    }
    let (cx, cy) = (
        tx.pres.slide_size.cx.max(1) as f64,
        tx.pres.slide_size.cy.max(1) as f64,
    );
    for (slide, id, before, after) in moved {
        if before == after {
            continue;
        }
        let has = tx
            .pres
            .slides
            .get(slide)
            .and_then(|s| s.timing.as_ref())
            .is_some_and(|t| {
                t.animations.iter().any(|a| {
                    a.shape_id == id
                        && a.class == AnimClass::Path
                        && bo_pptx::anim::path_locked(&bo_pptx::anim::effect_xml(a))
                })
            });
        if !has {
            continue;
        }
        let (dx, dy) = (
            (after.0 - before.0) as f64 / cx,
            (after.1 - before.1) as f64 / cy,
        );
        edit_timing(tx, slide, |anims| {
            for a in anims
                .iter_mut()
                .filter(|a| a.shape_id == id && a.class == AnimClass::Path)
            {
                let mut par = bo_pptx::anim::effect_xml(a);
                if !bo_pptx::anim::path_locked(&par) {
                    continue;
                }
                if let Some(p) = bo_pptx::anim::motion_path(&par) {
                    bo_pptx::anim::set_motion_path(
                        &mut par,
                        &bo_pptx::anim::shift_path(&p, -dx, -dy),
                    );
                    a.source = Some(par);
                    a.effect = AnimEffect::Other;
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

/// Shape ids of `p:spTgt@spid` in an effect as read, from `from` to `to` (§19.5.72 spTgt).
fn retarget(el: &mut bo_model::slide::XmlElement, from: u32, to: u32) {
    if el.name == "p:spTgt" {
        for (k, v) in &mut el.attrs {
            if k == "spid" && v.parse::<u32>().ok() == Some(from) {
                *v = to.to_string();
            }
        }
    }
    for c in &mut el.children {
        if let bo_model::slide::XmlNode::Element(e) = c {
            retarget(e, from, to);
        }
    }
}

/// Animation painter: the effects of shape `from` appended for each selected shape.
pub fn cmd_copy_animations(tx: &mut Tx<'_>, sel: &SlideSel, from: u32) -> R<()> {
    let targets: Vec<u32> = sel.ids.iter().copied().filter(|id| *id != from).collect();
    if targets.is_empty() {
        return refuse("noSelection");
    }
    edit_timing(tx, sel.slide, |anims| {
        let src: Vec<Animation> = anims
            .iter()
            .filter(|a| a.shape_id == from)
            .cloned()
            .collect();
        if src.is_empty() {
            return refuse("noAnimation");
        }
        for to in targets {
            for a in &src {
                let mut c = a.clone();
                c.shape_id = to;
                if let Some(x) = c.source.as_mut() {
                    retarget(x, from, to);
                }
                anims.push(c);
            }
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
