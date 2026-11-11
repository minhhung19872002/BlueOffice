//! Recorded slide shows (SLIDE-REC-001): narration, camera, ink and timings
//! recorded while presenting, stored in the deck so the show plays them back.
//!
//! What a recording of one slide becomes:
//! - **Narration**: an embedded audio clip (§19.3.1.37 pic with §20.1.3.1 audioFile and the
//!   [MS-PPTX] §2.3.2 `p14:media` extension), its media time node (ECMA-376 Part 1, §19.5.9
//!   audio, CT_TLMediaNodeAudio@`isNarration="1"`; §19.5 cMediaNode `showWhenStopped="0"`:
//!   the icon is hidden in the show) and a start effect playing it with the slide (§19.5.29 cTn
//!   `presetClass="mediacall"`, §19.5 cmd `playFrom(0.0)`). `p:sndTgt` (§19.5 sndTgt) is not
//!   used: it names an embedded WAV *sound* (§19.3.1.44 snd), while recordings are compressed
//!   audio the browser records. The show setup's `showNarration` (§19.2.1.30 showPr) turns
//!   narrations on.
//! - **Camera**: an embedded video clip in a corner of the slide (§20.1.3.6 videoFile +
//!   `p14:media`), its §19.5 video media node and a start effect with the slide.
//! - **Ink**: one InkML content part per stroke ([MS-ODRAWXML] §2.1.4, CORE-INK-001) whose
//!   time channel `T` holds ms from the start of the slide, plus an "appear" entrance effect
//!   (§19.5.66 set of `style.visibility`) at the time the stroke was started.
//! - **Clicks**: effects of the main sequence started by a recorded click start at that time
//!   instead (`withEffect` + `delay`, §19.5.29 cTn@nodeType / §19.5.25 cond@delay); click
//!   steps not reached while recording stay on click.
//! - **Timing**: `p:transition@advTm` (§19.3.1.50); `advClick` is kept.
//!
//! Clips and ink objects of a recording carry a BlueOffice marker in their `cNvPr` extension
//! list (`a:extLst/a:ext[@uri=REC_EXT_URI]`, §20.1.2.2 ext — an extension other producers
//! ignore), so "clear recording" removes exactly them.
//!
//! Commands (`cmd`): `recordingInfo`, `saveRecording`, `clearRecording`; saving and clearing
//! are one undo step each.

use super::build;
use super::ink::{self, InkStrokeIn};
use super::media::{poster_page, REL_MEDIA};
use super::media_playback::{new_media_node, parse_el, start_effect};
use super::ops::{OpError, SlideOp, Tx};
use super::present::{self, ShowSetup, SlideTiming};
use super::{next_shape_id, slide_size, EditError, Outcome, SlideEditResult, SlideSel, R};
use crate::{EngineError, Result};
use bo_model::slide::{
    AnimClass, AnimEffect, AnimTrigger, Animation, Presentation, RawPart, Rel, ShapeNode, Timing,
    XmlElement, XmlNode,
};
use serde::{Deserialize, Serialize};

/// Extension URI of the recording marker (BlueOffice's own GUID).
pub const REC_EXT_URI: &str = "{6C1D9B52-3E0A-4F7C-8D41-B10E5A1DEC01}";
/// Namespace of the recording marker element.
pub const NS_REC: &str = "http://schemas.bluestar.vn/blueoffice/2026/slide/recording";
/// [MS-PPTX] §2.3.2 extension URI of `p14:media`.
const MEDIA_EXT_URI: &str = "{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}";
const NS_P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
/// Largest recorded clip accepted (bytes, decoded).
const MAX_CLIP: usize = 512 * 1024 * 1024;
/// Longest slide recording (ms): one day.
const MAX_MS: u32 = 86_400_000;

/// What a recorded object is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecKind {
    /// Recorded narration (audio).
    Narration,
    /// Recorded camera (video).
    Camera,
    /// Ink drawn while recording.
    Ink,
}

impl RecKind {
    fn as_str(self) -> &'static str {
        match self {
            RecKind::Narration => "narration",
            RecKind::Camera => "camera",
            RecKind::Ink => "ink",
        }
    }

    fn parse(s: &str) -> Option<RecKind> {
        Some(match s {
            "narration" => RecKind::Narration,
            "camera" => RecKind::Camera,
            "ink" => RecKind::Ink,
            _ => return None,
        })
    }
}

/// Corner of the slide for the camera picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Corner {
    /// Top left.
    TopLeft,
    /// Top right.
    TopRight,
    /// Bottom left.
    BottomLeft,
    /// Bottom right.
    #[default]
    BottomRight,
}

/// A recorded clip: the bytes in base64 and their MIME type.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipIn {
    /// Base64 of the recorded file.
    pub data: String,
    /// MIME type (`audio/webm;codecs=opus`, `video/mp4`, …; parameters are dropped).
    pub mime: String,
}

/// The camera clip of a slide.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraIn {
    /// The clip.
    #[serde(flatten)]
    pub clip: ClipIn,
    /// Where the picture goes.
    #[serde(default)]
    pub corner: Corner,
    /// Width / height of the camera picture (default 4:3).
    #[serde(default)]
    pub aspect: Option<f64>,
}

/// The recording of one slide.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideRecordingIn {
    /// 0-based slide.
    pub slide: usize,
    /// Time spent on the slide (ms) → `advTm`; `None` keeps the slide's timing.
    #[serde(default)]
    pub duration_ms: Option<u32>,
    /// Times of the clicks that played the slide's effects (ms from the slide's start).
    #[serde(default)]
    pub clicks: Vec<u32>,
    /// Ink strokes; their `times` are ms from the slide's start.
    #[serde(default)]
    pub ink: Vec<InkStrokeIn>,
    /// Narration.
    #[serde(default)]
    pub narration: Option<ClipIn>,
    /// Camera.
    #[serde(default)]
    pub camera: Option<CameraIn>,
}

/// Names of the objects a recording adds (translated by the host).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecNames {
    /// Narration clip.
    pub narration: String,
    /// Camera clip.
    pub camera: String,
    /// Ink objects.
    pub ink: String,
}

impl Default for RecNames {
    fn default() -> Self {
        Self {
            narration: "Narration".into(),
            camera: "Camera".into(),
            ink: "Ink".into(),
        }
    }
}

/// Which slides "clear" works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClearScope {
    /// The current slide.
    #[default]
    Current,
    /// Every slide.
    All,
}

/// What "clear" removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClearWhat {
    /// Narration, camera and ink.
    #[default]
    Recording,
    /// The slides' `advTm`.
    Timings,
    /// Both.
    All,
}

/// Recording commands.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RecordCommand {
    /// What every slide has recorded → `recording`.
    RecordingInfo,
    /// Store a recording (each slide's previous recording is replaced).
    SaveRecording {
        /// Recorded slides.
        slides: Vec<SlideRecordingIn>,
        /// Object names.
        #[serde(default)]
        names: RecNames,
    },
    /// Clear recordings and / or timings.
    ClearRecording {
        /// Slides.
        #[serde(default)]
        scope: ClearScope,
        /// What goes.
        #[serde(default)]
        what: ClearWhat,
    },
}

/// Command names of [`RecordCommand`].
pub const COMMANDS: &[&str] = &["recordingInfo", "saveRecording", "clearRecording"];

impl RecordCommand {
    /// Parses a JSON command when it is a recording command.
    pub fn parse(v: &serde_json::Value) -> Option<RecordCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !COMMANDS.contains(&c) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// What one slide has recorded.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideRecordingOut {
    /// 0-based slide.
    pub slide: usize,
    /// Shape id of the narration clip.
    pub narration: Option<u32>,
    /// Shape id of the camera clip.
    pub camera: Option<u32>,
    /// Shape ids of the recorded ink objects.
    pub ink: Vec<u32>,
    /// `advTm` (ms).
    pub timing_ms: Option<u32>,
}

/// Recordings of the deck (slides with a recording or a timing).
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecordingOut {
    /// Slides in order.
    pub slides: Vec<SlideRecordingOut>,
}

// ---------------------------------------------------------------------------
// Marker
// ---------------------------------------------------------------------------

/// The marker extension list of a recorded object (§20.1.2.2 ext in a:extLst).
fn marker(kind: RecKind) -> R<XmlElement> {
    parse_el(&format!(
        r#"<a:extLst><a:ext uri="{REC_EXT_URI}"><bor:recording xmlns:bor="{NS_REC}" kind="{}"/></a:ext></a:extLst>"#,
        kind.as_str()
    ))
}

/// The recording kind in an element holding (or being) an `a:extLst`.
fn marker_in(el: &XmlElement) -> Option<RecKind> {
    let lists: Vec<&XmlElement> = if el.local() == "extLst" {
        vec![el]
    } else {
        el.elements().filter(|e| e.local() == "extLst").collect()
    };
    lists
        .into_iter()
        .flat_map(XmlElement::elements)
        .filter(|e| e.local() == "ext" && e.attr("uri") == Some(REC_EXT_URI))
        .flat_map(XmlElement::elements)
        .find(|e| e.local() == "recording")
        .and_then(|e| e.attr("kind"))
        .and_then(RecKind::parse)
}

/// The recording kind of a clip.
pub fn picture_kind(p: &bo_model::slide::Picture) -> Option<RecKind> {
    p.nv.cnv_extra.iter().find_map(|x| marker_in(&x.xml))
}

/// The recording kind of a top-level shape, with its id.
pub fn recorded(n: &ShapeNode) -> Option<(RecKind, u32)> {
    match n {
        ShapeNode::Picture(p) => picture_kind(p).map(|k| (k, p.nv.id)),
        ShapeNode::Other(_) => {
            let cp = bo_pptx::ink::node_content_part(n)?;
            let nv = cp.elements().find(|e| e.local() == "nvContentPartPr")?;
            let c = nv.elements().find(|e| e.local() == "cNvPr")?;
            let id = c.attr("id")?.parse().ok()?;
            marker_in(c).map(|k| (k, id))
        }
        _ => None,
    }
}

/// What every slide has recorded.
pub fn recording_out(pres: &Presentation) -> RecordingOut {
    let mut out = RecordingOut::default();
    for (i, s) in pres.slides.iter().enumerate() {
        let mut r = SlideRecordingOut {
            slide: i,
            narration: None,
            camera: None,
            ink: Vec::new(),
            timing_ms: s.transition.as_ref().and_then(|t| t.advance_after_ms),
        };
        for n in &s.data.shapes {
            match recorded(n) {
                Some((RecKind::Narration, id)) => r.narration = Some(id),
                Some((RecKind::Camera, id)) => r.camera = Some(id),
                Some((RecKind::Ink, id)) => r.ink.push(id),
                None => {}
            }
        }
        if r.narration.is_some() || r.camera.is_some() || !r.ink.is_empty() || r.timing_ms.is_some()
        {
            out.slides.push(r);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Clearing
// ---------------------------------------------------------------------------

fn collect_rids(el: &XmlElement, out: &mut Vec<String>) {
    for (k, v) in &el.attrs {
        let rel = k.starts_with("r:") || k.ends_with(":embed") || k.ends_with(":link");
        if rel && !v.is_empty() {
            out.push(v.clone());
        }
    }
    for c in el.elements() {
        collect_rids(c, out);
    }
}

/// Relationship ids a recorded object uses.
fn rids_of(n: &ShapeNode) -> Vec<String> {
    let mut out = Vec::new();
    match n {
        ShapeNode::Picture(p) => {
            out.extend(p.blip.embed.iter().cloned());
            out.extend(p.blip.link.iter().cloned());
            for x in &p.nv.nv_extra {
                collect_rids(&x.xml, &mut out);
            }
        }
        ShapeNode::Other(_) => {
            if let Some(r) =
                bo_pptx::ink::node_content_part(n).and_then(bo_pptx::ink::content_part_rid)
            {
                out.push(r.to_owned());
            }
        }
        _ => {}
    }
    out
}

/// Remove the recorded objects of `slide` (their effects, media nodes, relationships and the
/// parts nothing else uses). True when something was removed.
fn clear_slide(tx: &mut Tx<'_>, slide: usize) -> R<bool> {
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    let found: Vec<(usize, u32)> = s
        .data
        .shapes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| recorded(n).map(|(_, id)| (i, id)))
        .collect();
    if found.is_empty() {
        return Ok(false);
    }
    let mut rids = Vec::new();
    let mut ids = Vec::new();
    for (i, id) in found.iter().rev() {
        let n = tx.remove_node(slide, &[*i])?;
        rids.extend(rids_of(&n));
        ids.push(*id);
    }
    // Relationships still used by the remaining shapes stay.
    let s = &tx.pres.slides[slide];
    let mut used = Vec::new();
    for n in &s.data.shapes {
        used.extend(rids_of(n));
    }
    rids.retain(|r| !used.contains(r));
    let gone: Vec<String> = s
        .info
        .rels
        .iter()
        .filter(|r| rids.contains(&r.id) && !r.external)
        .map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target))
        .collect();
    tx.fields(slide, |f| {
        f.rels.retain(|r| !rids.contains(&r.id));
        if let Some(t) = &mut f.timing {
            t.animations.retain(|a| !ids.contains(&a.shape_id));
            super::media_playback::forget_clips(t, &ids);
        }
    })?;
    drop_unused_parts(tx, &gone)?;
    Ok(true)
}

/// Remove the parts `names` when no slide relationship targets them any more.
fn drop_unused_parts(tx: &mut Tx<'_>, names: &[String]) -> R<()> {
    for name in names {
        let used = tx.pres.slides.iter().any(|s| {
            s.info.rels.iter().any(|r| {
                !r.external
                    && bo_opc::resolve_target(&s.info.part_name, &r.target)
                        .eq_ignore_ascii_case(name)
            })
        });
        if used {
            continue;
        }
        if let Some(i) = tx
            .pres
            .parts
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
        {
            let part = tx.pres.parts[i].clone();
            tx.op(SlideOp::Parts {
                index: i,
                remove: vec![part],
                insert: vec![],
            })?;
        }
    }
    Ok(())
}

fn cmd_clear(tx: &mut Tx<'_>, sel: &SlideSel, scope: ClearScope, what: ClearWhat) -> R<()> {
    let slides: Vec<usize> = match scope {
        ClearScope::Current => vec![sel.slide],
        ClearScope::All => (0..tx.pres.slides.len()).collect(),
    };
    let mut changed = false;
    if matches!(what, ClearWhat::Recording | ClearWhat::All) {
        for &s in &slides {
            changed |= clear_slide(tx, s)?;
        }
    }
    if matches!(what, ClearWhat::Timings | ClearWhat::All) {
        let timed: Vec<SlideTiming> = slides
            .iter()
            .filter(|&&s| {
                tx.pres.slides.get(s).is_some_and(|x| {
                    x.transition
                        .as_ref()
                        .is_some_and(|t| t.advance_after_ms.is_some())
                })
            })
            .map(|&slide| SlideTiming { slide, ms: None })
            .collect();
        changed |= !timed.is_empty();
        present::cmd_set_timings(tx, &timed, None)?;
    }
    if changed {
        Ok(())
    } else {
        Err(EditError::Refused("noRecording"))
    }
}

// ---------------------------------------------------------------------------
// Saving
// ---------------------------------------------------------------------------

/// `audio` / `video`, content type and file extension of a recorded clip.
fn clip_type(mime: &str, want: &str) -> R<(String, &'static str)> {
    let base = mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let (major, sub) = base.split_once('/').ok_or(EditError::Refused("badMedia"))?;
    if major != want {
        return Err(EditError::Refused("badMedia"));
    }
    let ext = match (major, sub) {
        (_, "webm") => "webm",
        ("audio", "ogg") => "oga",
        ("video", "ogg") => "ogv",
        ("audio", "mp4" | "x-m4a" | "aac") => "m4a",
        ("video", "mp4") => "mp4",
        ("audio", "mpeg") => "mp3",
        ("audio", "wav" | "x-wav" | "wave") => "wav",
        ("video", "quicktime") => "mov",
        _ => return Err(EditError::Refused("badMedia")),
    };
    Ok((base, ext))
}

fn decode(data: &str) -> R<Vec<u8>> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| EditError::Refused("badMedia"))?;
    if bytes.is_empty() || bytes.len() > MAX_CLIP {
        return Err(EditError::Refused("badMedia"));
    }
    Ok(bytes)
}

fn new_rids(rels: &[Rel], n: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut k = 1;
    while out.len() < n {
        let id = format!("rId{k}");
        if rels.iter().all(|r| r.id != id) {
            out.push(id);
        }
        k += 1;
    }
    out
}

/// A clip to insert.
struct NewClip<'a> {
    bytes: Vec<u8>,
    /// `audio` | `video`.
    kind: &'static str,
    mime: String,
    ext: &'static str,
    name: &'a str,
    poster: Vec<u8>,
    rect: (f64, f64, f64, f64),
    rec: RecKind,
}

/// Insert a recorded clip on top of `slide`; returns its shape id.
fn insert_clip(tx: &mut Tx<'_>, slide: usize, c: NewClip<'_>) -> R<u32> {
    let media_part = build::unique_part(tx.pres, "/ppt/media/media", c.ext);
    let poster_part = build::unique_part_except(
        tx.pres,
        "/ppt/media/image",
        "png",
        std::slice::from_ref(&media_part),
    );
    let file = |p: &str| format!("../media/{}", p.rsplit('/').next().unwrap_or_default());
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    let ids = new_rids(&s.info.rels, 3);
    let (link_id, embed_id, image_id) = (ids[0].clone(), ids[1].clone(), ids[2].clone());
    let id = next_shape_id(tx.pres, slide);
    let tag = if c.kind == "video" {
        "a:videoFile"
    } else {
        "a:audioFile"
    };
    let emu = |pt: f64| (pt * super::geom::EMU_PT).round() as i64;
    let name = format!("{} {id}", c.name.trim());
    let (x, y, w, h) = c.rect;
    let xml = format!(
        concat!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{name}"><a:hlinkClick r:id="" action="ppaction://media"/>"#,
            r#"<a:extLst><a:ext uri="{rec_uri}"><bor:recording xmlns:bor="{rec_ns}" kind="{rec}"/></a:ext></a:extLst></p:cNvPr>"#,
            r#"<p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><{tag} r:link="{link}"/>"#,
            r#"<p:extLst><p:ext uri="{uri}"><p14:media xmlns:p14="{p14}" r:embed="{embed}"/></p:ext></p:extLst></p:nvPr></p:nvPicPr>"#,
            r#"<p:blipFill><a:blip r:embed="{image}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>"#,
            r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
        ),
        id = id,
        name = build::esc(&name),
        rec_uri = REC_EXT_URI,
        rec_ns = NS_REC,
        rec = c.rec.as_str(),
        tag = tag,
        link = link_id,
        uri = MEDIA_EXT_URI,
        p14 = NS_P14,
        embed = embed_id,
        image = image_id,
        x = emu(x),
        y = emu(y),
        cx = emu(w).max(1),
        cy = emu(h).max(1),
    );
    let node = build::node(&xml).ok_or(EditError::Refused("build"))?;
    let at = tx.pres.parts.len();
    tx.op(SlideOp::Parts {
        index: at,
        remove: vec![],
        insert: vec![
            RawPart {
                name: media_part.clone(),
                content_type: Some(c.mime),
                data: c.bytes,
            },
            RawPart {
                name: poster_part.clone(),
                content_type: Some("image/png".into()),
                data: c.poster,
            },
        ],
    })?;
    let kind = c.kind;
    tx.fields(slide, |f| {
        // §15.2.2 audio / §15.2.17 video relationship, the [MS-PPTX] media one, the poster.
        f.rels.push(Rel {
            id: link_id,
            rel_type: format!("{}{kind}", bo_pptx::RELATIONSHIP_NS),
            target: file(&media_part),
            external: false,
        });
        f.rels.push(Rel {
            id: embed_id,
            rel_type: REL_MEDIA.into(),
            target: file(&media_part),
            external: false,
        });
        f.rels.push(Rel {
            id: image_id,
            rel_type: format!("{}image", bo_pptx::RELATIONSHIP_NS),
            target: file(&poster_part),
            external: false,
        });
    })?;
    let len = tx.pres.slides[slide].data.shapes.len();
    tx.insert_nodes(slide, &[], len, vec![node])?;
    Ok(id)
}

/// Box of the camera picture: a quarter of the slide's width in `corner`, 12 pt from the edges.
fn camera_rect(slide: (f64, f64), corner: Corner, aspect: Option<f64>) -> (f64, f64, f64, f64) {
    let a = aspect
        .filter(|a| a.is_finite() && *a > 0.2 && *a < 5.0)
        .unwrap_or(4.0 / 3.0);
    let w = (slide.0 * 0.25).max(24.0);
    let h = w / a;
    let m = 12.0;
    let x = match corner {
        Corner::TopLeft | Corner::BottomLeft => m,
        Corner::TopRight | Corner::BottomRight => slide.0 - w - m,
    };
    let y = match corner {
        Corner::TopLeft | Corner::TopRight => m,
        Corner::BottomLeft | Corner::BottomRight => slide.1 - h - m,
    };
    (x, y, w, h)
}

/// The "appear" entrance effect of a recorded ink object.
fn ink_appear(id: u32) -> Animation {
    Animation {
        shape_id: id,
        class: AnimClass::Entrance,
        effect: AnimEffect::Appear,
        direction: None,
        trigger: AnimTrigger::WithPrevious,
        delay_ms: 0,
        dur_ms: 1,
        preset_id: None,
        preset_subtype: None,
        repeat: None,
        trigger_shape: None,
        trigger_bookmark: None,
        iterate: None,
        opts: bo_model::slide::AnimOptions::default(),
        source: None,
    }
}

/// Re-time the slide's effects to the recording: effects of the main sequence on click steps
/// reached while recording start at their click's time, the recording's own effects (clip
/// starts, ink) at theirs; everything runs as one timed sequence (`withEffect` + delay).
/// Click steps not reached stay on click after it.
pub fn retime(t: &mut Timing, clicks: &[u32], starts: Vec<Animation>, ink: Vec<(u32, Animation)>) {
    let tl = t.timeline();
    let reached = clicks.len();
    // (absolute start, rank, order, effect): rank 0 = clip starts, 1 = the slide's, 2 = ink.
    let mut timed: Vec<(u64, u8, usize, Animation)> = Vec::new();
    let mut later = Vec::new();
    let mut others = Vec::new();
    for (i, a) in std::mem::take(&mut t.animations).into_iter().enumerate() {
        if a.trigger_shape.is_some() || a.trigger_bookmark.is_some() {
            others.push(a);
            continue;
        }
        let (click, start) = tl
            .iter()
            .find(|e| e.animation == i)
            .map_or((0, 0), |e| (e.click, e.start_ms));
        if click > reached {
            later.push(a);
            continue;
        }
        let base = if click == 0 {
            0
        } else {
            u64::from(clicks[click - 1])
        };
        timed.push((base + u64::from(start), 1, i, a));
    }
    for (k, a) in starts.into_iter().enumerate() {
        timed.push((0, 0, k, a));
    }
    for (k, (at, a)) in ink.into_iter().enumerate() {
        timed.push((u64::from(at), 2, k, a));
    }
    timed.sort_by_key(|a| (a.0, a.1, a.2));
    let mut prev = 0u64;
    let mut out = Vec::with_capacity(timed.len() + later.len() + others.len());
    for (at, _, _, mut a) in timed {
        // §19.5.29 cTn nodeType withEffect: starts with the previous effect, plus its delay.
        a.trigger = AnimTrigger::WithPrevious;
        a.delay_ms = u32::try_from(at - prev).unwrap_or(u32::MAX);
        prev = at;
        out.push(a);
    }
    out.extend(later);
    out.extend(others);
    t.animations = out;
}

/// Posters of new clips (PNG): the narration icon and the camera frame.
pub struct Posters {
    /// Audio icon.
    pub audio: Vec<u8>,
    /// Video frame.
    pub video: Vec<u8>,
}

fn valid_stroke(s: &InkStrokeIn) -> Option<u32> {
    let times = s.times.as_ref()?;
    if times.len() != s.points.len() || times.iter().any(|t| !t.is_finite() || *t < 0.0) {
        return None;
    }
    let first = times.first().copied()?;
    Some(first.min(f64::from(MAX_MS)).round() as u32)
}

fn save_slide(tx: &mut Tx<'_>, r: &SlideRecordingIn, names: &RecNames, posters: &Posters) -> R<()> {
    let slide = r.slide;
    if slide >= tx.pres.slides.len() {
        return Err(OpError::NoSlide(slide).into());
    }
    if r.duration_ms.is_some_and(|d| d > MAX_MS) || r.clicks.iter().any(|c| *c > MAX_MS) {
        return Err(EditError::Refused("badTime"));
    }
    clear_slide(tx, slide)?;
    let size = slide_size(tx.pres);
    let mut starts = Vec::new();
    let mut nodes = Vec::new();
    if let Some(n) = &r.narration {
        let (mime, ext) = clip_type(&n.mime, "audio")?;
        let s = 32.0;
        let id = insert_clip(
            tx,
            slide,
            NewClip {
                bytes: decode(&n.data)?,
                kind: "audio",
                mime,
                ext,
                name: &names.narration,
                poster: posters.audio.clone(),
                rect: (size.0 - s - 12.0, size.1 - s - 12.0, s, s),
                rec: RecKind::Narration,
            },
        )?;
        let mut node = new_media_node(id, false)?;
        // §19.5.9 audio@isNarration; §19.5 cMediaNode@showWhenStopped: no icon in the show.
        node.attrs.push(("isNarration".into(), "1".into()));
        if let Some(XmlNode::Element(cm)) = node
            .children
            .iter_mut()
            .find(|c| matches!(c, XmlNode::Element(e) if e.local() == "cMediaNode"))
        {
            cm.attrs.push(("showWhenStopped".into(), "0".into()));
        }
        nodes.push(node);
        starts.push(start_effect(id, "auto")?);
    }
    if let Some(c) = &r.camera {
        let (mime, ext) = clip_type(&c.clip.mime, "video")?;
        let id = insert_clip(
            tx,
            slide,
            NewClip {
                bytes: decode(&c.clip.data)?,
                kind: "video",
                mime,
                ext,
                name: &names.camera,
                poster: posters.video.clone(),
                rect: camera_rect(size, c.corner, c.aspect),
                rec: RecKind::Camera,
            },
        )?;
        nodes.push(new_media_node(id, true)?);
        starts.push(start_effect(id, "auto")?);
    }
    let mark = marker(RecKind::Ink)?;
    let mut inks = Vec::new();
    for s in &r.ink {
        let Some(at) = valid_stroke(s) else {
            return Err(EditError::Refused("badInk"));
        };
        let id = ink::add_stroke(tx, slide, s, &names.ink, Some(&mark))?;
        inks.push((at, ink_appear(id)));
    }
    if !starts.is_empty() || !inks.is_empty() || !r.clicks.is_empty() {
        let s = &tx.pres.slides[slide];
        let mut t = s.timing.clone().unwrap_or_default();
        t.others.extend(nodes);
        retime(&mut t, &r.clicks, starts, inks);
        tx.fields(slide, |f| f.timing = Some(t))?;
    }
    if let Some(ms) = r.duration_ms {
        present::cmd_set_timings(
            tx,
            &[SlideTiming {
                slide,
                ms: Some(ms),
            }],
            None,
        )?;
    }
    Ok(())
}

fn cmd_save(
    tx: &mut Tx<'_>,
    slides: &[SlideRecordingIn],
    names: &RecNames,
    posters: &Posters,
) -> R<()> {
    if slides.is_empty() {
        return Err(EditError::Refused("noRecording"));
    }
    for r in slides {
        save_slide(tx, r, names, posters)?;
    }
    // §19.2.1.30 showPr: play narrations, use the recorded timings.
    let setup = present::show_setup(tx.pres);
    let narrated = slides
        .iter()
        .any(|r| r.narration.is_some() || r.camera.is_some());
    let want = ShowSetup {
        narration: setup.narration || narrated,
        use_timings: true,
        ..setup.clone()
    };
    if want != setup {
        present::cmd_set_show_setup(tx, &want)?;
    }
    Ok(())
}

impl crate::Engine {
    /// Run a recording command.
    pub fn slide_record_command(&mut self, cmd: RecordCommand) -> Result<SlideEditResult> {
        let _salt = build::salt_names(
            self.slide_collab
                .as_ref()
                .map(|c| build::salt_of(c.client_id())),
        );
        let posters = if matches!(cmd, RecordCommand::SaveRecording { .. }) {
            let png = |k: &str| {
                bo_raster::render_page_png(&poster_page(k), &self.fonts, 2.0).unwrap_or_default()
            };
            Posters {
                audio: png("audio"),
                video: png("video"),
            }
        } else {
            Posters {
                audio: Vec::new(),
                video: Vec::new(),
            }
        };
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            ed.text = None;
            ed.table = None;
            let blocked = !matches!(cmd, RecordCommand::RecordingInfo) && ed.in_any_master_view();
            if blocked {
                out.error = Some("masterView".into());
            } else {
                match cmd {
                    RecordCommand::RecordingInfo => {}
                    RecordCommand::SaveRecording { slides, names } => {
                        ed.mutate(pres, &mut out, |tx, sel| {
                            cmd_save(tx, &slides, &names, &posters)?;
                            sel.ids.clear();
                            Ok(())
                        });
                    }
                    RecordCommand::ClearRecording { scope, what } => {
                        ed.mutate(pres, &mut out, |tx, sel| {
                            cmd_clear(tx, sel, scope, what)?;
                            sel.ids.clear();
                            Ok(())
                        });
                    }
                }
            }
            ed.clamp(pres);
        }
        let mut r = self.finish_slide_command(out)?;
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        r.recording = Some(recording_out(pres));
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_types() {
        assert_eq!(
            clip_type("audio/webm;codecs=opus", "audio").unwrap(),
            ("audio/webm".to_owned(), "webm")
        );
        assert_eq!(
            clip_type("video/mp4", "video").unwrap(),
            ("video/mp4".to_owned(), "mp4")
        );
        assert!(clip_type("video/webm", "audio").is_err());
        assert!(clip_type("text/plain", "audio").is_err());
    }

    #[test]
    fn camera_corners() {
        let (x, y, w, h) = camera_rect((960.0, 540.0), Corner::BottomRight, None);
        assert_eq!((w, h), (240.0, 180.0));
        assert_eq!((x, y), (708.0, 348.0));
        let (x, y, _, _) = camera_rect((960.0, 540.0), Corner::TopLeft, Some(16.0 / 9.0));
        assert_eq!((x, y), (12.0, 12.0));
    }

    fn anim(id: u32, trigger: AnimTrigger, delay: u32, dur: u32) -> Animation {
        Animation {
            trigger,
            delay_ms: delay,
            dur_ms: dur,
            ..ink_appear(id)
        }
    }

    #[test]
    fn retime_puts_clicks_and_ink_on_one_timeline() {
        // Click 1: shape 10 (500 ms), after it shape 11; click 2: shape 12; click 3 not reached.
        let mut t = Timing {
            animations: vec![
                anim(10, AnimTrigger::OnClick, 0, 500),
                anim(11, AnimTrigger::AfterPrevious, 0, 500),
                anim(12, AnimTrigger::OnClick, 0, 500),
                anim(13, AnimTrigger::OnClick, 0, 500),
            ],
            ..Timing::default()
        };
        retime(
            &mut t,
            &[2000, 5000],
            vec![anim(1, AnimTrigger::AfterPrevious, 0, 1)],
            vec![(3000, ink_appear(20)), (1000, ink_appear(21))],
        );
        let tl = t.timeline();
        let at = |id: u32| {
            let i = t.animations.iter().position(|a| a.shape_id == id).unwrap();
            let e = tl.iter().find(|e| e.animation == i).unwrap();
            (e.click, e.start_ms)
        };
        assert_eq!(at(1), (0, 0));
        assert_eq!(at(21), (0, 1000));
        assert_eq!(at(10), (0, 2000));
        assert_eq!(at(11), (0, 2500));
        assert_eq!(at(20), (0, 3000));
        assert_eq!(at(12), (0, 5000));
        // The step not reached still waits for a click.
        assert_eq!(at(13), (1, 0));
    }

    #[test]
    fn markers_round_trip() {
        let m = marker(RecKind::Camera).unwrap();
        assert_eq!(marker_in(&m), Some(RecKind::Camera));
        let mut c = XmlElement::new("p14:cNvPr");
        c.children.push(XmlNode::Element(m));
        assert_eq!(marker_in(&c), Some(RecKind::Camera));
    }
}
